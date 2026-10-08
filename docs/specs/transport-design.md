<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Tessera reliable-datagram transport (TRD v1)

| Field | Value |
|---|---|
| Status | **Review**. Implemented in `crates/net` (`tessera_net::transport`); waiting for a reviewer |
| Last updated | 2026-10-04 |
| Author role | Implementer (original design, no dirty-room input) |
| Reviewer | none yet |

> **Status note (2026-10-06):** [Decision 0030](../decisions/0030-tessera-protocol-transport.md)
> picks QUIC (quinn, rustls on ring) as the transport of Tessera protocol v1
> (`tessera-protocol.md`). TRD v1 is not used by that protocol. It stays in
> `tessera_net::transport`, frozen at v1, as a tested in-memory and loopback reliability layer.
> For Tessera's network use, its open items (§10) are answered by QUIC, and no TRD v2 is
> planned. The facts below are unchanged.

## 1. Purpose

This document specifies a small, general-purpose transport that Tessera designed itself:
reliable, ordered, fragmented messages plus optional unreliable messages, carried over any
unreliable datagram medium (UDP, an in-memory loopback). With it, Tessera components (a local
test server, tools, future backends) can share one tested reliability layer.

It is **not** a description of any third-party or official protocol and has no
compatibility goal. Whether Tessera's own multiplayer protocol (ROADMAP M7) uses it or
something else, such as QUIC, is decided by that milestone's decision record. Nothing here
relates to the gated compatibility tier (LEGAL.md §9.3).

## 2. Provenance

| Facts | Source | Mark |
|---|---|---|
| Everything in §3–§9 | Original design by the Tessera project, 2026-10-03. It uses textbook techniques: cumulative plus selective acknowledgement, the smoothed-RTT/RTT-variance retransmission-timer estimator (as in IETF RFC 6298), Karn's rule, serial-number arithmetic (as in RFC 1982), and CRC-32 (IEEE 802.3) | Confirmed (by the test suite in `crates/net/tests/`, 2026-10-03) |

## 3. Model and terms

- **Datagram layer**: a connected, non-blocking pipe that may drop, duplicate, reorder or
  truncate datagrams. It never splits or merges them.
- **Endpoint**: one side of a connection. The two endpoints are symmetric. Neither is a
  client or a server at this layer.
- **Message**: an application byte string. A **reliable** message is delivered exactly
  once, in send order, and intact. An **unreliable** message is delivered intact but possibly
  not at all, out of order, or (if the datagram layer duplicates it) more than once; see
  §9 item 8.
- **Segment**: one fragment of a reliable message, carried in one datagram. Each segment
  has a **sequence number** (32-bit, per direction, increasing by one per segment).
- **MTU**: the largest datagram an endpoint sends. It is the smaller of the configured
  value (default 1200 bytes) and the datagram layer's own limit. It is never below 64
  bytes or above 65 507 bytes.

Both endpoints must agree, out of band, on a 32-bit **connection id** and on the
**initial sequence number** used in both directions (default 0).

> **OPEN:** There is no handshake. A future revision should negotiate the connection id,
> initial sequence numbers (randomised), version and MTU, and should authenticate peers.
> Until then the transport is suitable for loopback, LAN and test use only.

## 4. Packet layout

All integers are little-endian. One packet occupies exactly one datagram. Trailing bytes
are not allowed.

### 4.1 Header (16 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 2 | bytes | Magic, ASCII `TS` (0x54 0x53) |
| 2 | 1 | u8 | Version, 1 |
| 3 | 1 | u8 | Flags, see §4.2 |
| 4 | 4 | u32 | Connection id |
| 8 | 4 | u32 | `ack_next`: the next sequence number the sender of this packet expects; every earlier one has been received |
| 12 | 4 | u32 | `ack_bits`: bit *i* (0 = least significant) set means sequence `ack_next + 1 + i` has been received |

### 4.2 Flags

| Bit | Name | Meaning |
|---|---|---|
| 0 (0x01) | RELIABLE | Body is a reliable segment (§4.3) |
| 1 (0x02) | UNRELIABLE | Body is an unreliable message (§4.4) |
| 2 (0x04) | CLOSE | The sender is closing the connection (§7) |
| 3 (0x08) | PAUSED | The sender is back-pressured: its application is not draining received messages, so it has stopped consuming segments at `ack_next` (§6.4 step 5). Valid with any body |
| 4–7 | reserved | Must be 0 |

PAUSED was a reserved bit in the first draft of v1. The version number was not bumped,
because no implementation of that draft was ever deployed.

RELIABLE and UNRELIABLE are mutually exclusive. With neither set the body is empty: the
packet is an acknowledgement, a keepalive, or a close.

### 4.3 Reliable body (14 + n bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 16 | 4 | u32 | Sequence number |
| 20 | 4 | u32 | Message id (per direction, +1 per reliable message, wraps) |
| 24 | 2 | u16 | Fragment index, 0-based |
| 26 | 2 | u16 | Fragment count, at least 1 |
| 28 | 2 | u16 | Payload length *n* |
| 30 | n | bytes | Payload |

### 4.4 Unreliable body (2 + n bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 16 | 2 | u16 | Payload length *n* |
| 18 | n | bytes | Payload |

### 4.5 Trailer (4 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| end − 4 | 4 | u32 | CRC-32 (IEEE 802.3: reflected polynomial 0xEDB88320, initial value and final XOR 0xFFFFFFFF) of every preceding byte of the packet |

### 4.6 Sizes

- Overhead per reliable segment is 34 bytes, so the fragment payload is MTU − 34
  (1166 bytes at the default MTU).
- Overhead per unreliable message is 22 bytes. The largest unreliable message is MTU − 22
  (1178 bytes at the default MTU). Unreliable messages are never fragmented.
- An empty packet is 20 bytes.

## 5. Decoding and validation

A receiver drops a datagram silently, counting it as malformed, if any of these hold:
it is shorter than 20 bytes; the CRC does not match; the magic or version differs; a
reserved flag bit is set; both body flags are set; a length field points past the end;
bytes remain after the body; or, for a reliable body, the fragment count is 0 or the
fragment index is not below the fragment count.

A valid packet with a different connection id is dropped and counted as foreign. Neither
case is fatal, because corruption and stray traffic are normal on real networks.

## 6. Reliability

### 6.1 Sending

1. A reliable message of length *L* is split into ⌈*L* / fragment payload⌉ segments, at
   least one (an empty message is one empty segment). Segments take consecutive sequence
   numbers and share one message id.
2. The sender refuses a message, without queueing any of it, if *L* exceeds the maximum
   message size (default 256 KiB), if the fragment count exceeds the maximum fragment count
   (default 1024), or if queueing it would push the outbound queue over its byte budget
   (default 8 MiB, charging payload plus 32 bytes per segment). A configuration whose queue
   budget cannot hold the largest allowed message on its own, or whose queue or inbox
   budget is 0, is rejected when the endpoint is created.
3. The send window is a **span of sequence numbers**: only segments whose sequence number
   precedes (oldest unacknowledged + *W*) may be sent, where *W* is the send window (default
   256). Segments that were selectively acknowledged inside that span still occupy their
   place in it. This keeps the sender inside a receiver window of the same size (§6.4 step 1),
   so a send window no larger than the peer's receive window never causes out-of-window
   drops. Segments are first sent in sequence order.
4. Every packet carries the sender's current `ack_next` and `ack_bits`.

### 6.2 Acknowledgement processing

On every valid packet, a segment that has been sent counts as acknowledged if its sequence
number precedes `ack_next`, or if it lies 1–32 places after `ack_next` and its bit in
`ack_bits` is set. The send queue then drops acknowledged segments from its front.

An acknowledgement that claims a sequence number not yet sent (at or after one past the
highest sequence number ever sent), either through `ack_next` or through a set bit in
`ack_bits`, is ignored for those sequence numbers and counted once per packet as invalid.
This does not depend on what is currently queued. It is not fatal.

Every valid packet also updates the sender's view of the peer's PAUSED state (§4.2) to that
packet's flag.

### 6.3 Retransmission timer

- Each sent segment has its own timeout. When a segment is first sent, its timeout is the
  endpoint's current RTO.
- When a segment's timeout expires unacknowledged, the segment is resent with the same
  sequence number, its retransmission count goes up by one, and its timeout doubles, capped
  at the maximum RTO (default 3 s).
- While the peer's most recent packet carried PAUSED, expired segments are **not**
  retransmitted and nothing counts toward the budget. The peer is alive and holding data
  back, so the missing acknowledgement is not loss. Liveness is still enforced by the idle
  timeout (§6.6). When a packet without PAUSED arrives, expired segments are retransmitted
  on the next poll.
- A retransmission counts toward the retransmission budget (default 15) only if no
  acknowledgement has covered any new segment since this segment was last sent ("no
  progress"). A segment that would need more counted retransmissions than the budget fails
  the connection as *peer unresponsive*. A live peer that keeps acknowledging other data,
  for example while it slowly drains a full reorder buffer (§6.4 step 3), is therefore not
  mistaken for a dead one. A peer that acknowledges nothing at all still fails the
  connection after exactly the budget.
- RTT samples come only from segments acknowledged without ever being retransmitted
  (Karn's rule). At most one sample is taken per received packet: the last newly
  acknowledged such segment.
- First sample *R*: SRTT = *R*, RTTVAR = *R*/2. Later samples: RTTVAR = ¾·RTTVAR + ¼·|SRTT − *R*|,
  then SRTT = ⅞·SRTT + ⅛·*R*. RTO = SRTT + max(4·RTTVAR, 1 ms), clamped to
  [minimum RTO, maximum RTO] (defaults 50 ms and 3 s). Before any sample, RTO is the
  initial RTO (default 250 ms).

### 6.4 Receiving and ordering

1. The receiver keeps `next` (the lowest sequence number not yet received) and a buffer of
   up to *R* slots (the receive window, default 256) for sequence numbers `next` to
   `next + R − 1`.
2. Sequence numbers are compared with serial-number arithmetic: *a* precedes *b* when
   (*a* − *b*) mod 2³², read as a signed 32-bit integer, is negative. Wrap-around is
   therefore transparent.
3. A segment that precedes `next`, or whose slot is already filled, is a duplicate: it is
   counted and dropped. A segment at or beyond `next + R` is dropped and counted as out of
   window. A segment for any slot other than `next` is also dropped, without being
   acknowledged, if storing it would push the buffered segments over the **reorder budget**
   (default 1 MiB, charging payload plus 32 bytes per segment). The peer retransmits it
   later. The segment for `next` is always stored, so the budget can delay progress but
   never block it. This bounds receive-side memory by local configuration, whatever MTU the
   peer uses. Every reliable segment received, including a dropped one, makes an
   acknowledgement due.
4. While the slot for `next` is filled, its segment is passed to reassembly (§6.5) and
   `next` advances by one.
5. **Back-pressure**: step 4 pauses while the application's undelivered inbox holds the
   inbox budget or more (default 8 MiB, charging payload plus 64 bytes per message, so
   empty messages are not free). While paused, every packet the receiver sends carries
   PAUSED, and an empty packet is sent promptly whenever the paused state changes. Because
   `next` stops moving, the peer's window fills, and per §6.3 the peer stops retransmitting
   instead of counting the stall as loss. No data is lost however long the application
   stalls, as long as both endpoints keep polling. An unreliable message is added to the
   inbox only if it fits within the budget; otherwise it is dropped and counted.

### 6.5 Reassembly

Segments reach reassembly in strict sequence order, so the fragments of a message arrive
consecutively and at most one partial message exists. Each of the following is a
**protocol violation**, which fails the connection, because a correctly checksummed packet
that breaks these rules comes from a broken or hostile peer:

- a fragment count above the local maximum fragment count;
- a first fragment (no partial message in progress) whose index is not 0;
- a continuing fragment whose message id or fragment count differs from the partial
  message, or whose index is not exactly one more than the previous fragment's;
- a partial or complete message that would exceed the local maximum message size.

When the fragment with index *count* − 1 is appended, the message is delivered.

### 6.6 Acknowledgements, keepalive and idle timeout

- During each poll, after sending segments and unreliable messages, an empty packet is
  sent if an acknowledgement is still due (no packet has carried it since it became due),
  if the local PAUSED state differs from the one last sent, or if nothing has been sent
  for the keepalive interval (default 1 s).
- If no valid packet has been received for the idle timeout (default 15 s), the connection
  fails as *timed out*. The keepalive interval must be shorter than the idle timeout.

## 7. Closing

- A local close sends three empty packets with CLOSE set, best effort and never
  acknowledged, and then refuses further use. Queued, unacknowledged data is discarded. A
  graceful close waits until the send queue is empty first.
- On receiving CLOSE, an endpoint first processes the rest of that packet, delivers any
  segments that are already contiguous, and then enters the closed-by-peer state.
- When an endpoint fails (unresponsive, timed out, protocol violation), it also sends the
  three CLOSE packets.
- Messages already delivered to the inbox stay readable after close or failure.

## 8. Driving model

The endpoint has no threads, no blocking calls and no clock of its own. The application
calls *poll* with the current monotonic time, normally once per frame. Each poll:

1. reads up to 4096 pending datagrams and processes each (§5, §6.2, §6.4, §7);
2. moves contiguous segments through reassembly (§6.4 step 4);
3. checks the idle timeout;
4. retransmits segments whose timer has expired, then sends new segments allowed by the
   window;
5. sends queued unreliable messages (at most 1024 can be queued);
6. sends an empty packet if §6.6 requires one.

Send calls only queue data. Nothing reaches the wire until the next poll.

## 9. Invariants (checked by `crates/net/tests/`)

1. Under 30% independent loss in each direction, with messages from 0 bytes to 40 000
   bytes (up to 35 fragments), every reliable message arrives exactly once, in order, and
   byte-identical (`reliable_ordered_under_30_percent_loss_with_fragmentation`).
2. The same holds with 30% loss, 20% duplication and 20% reordering together
   (`survives_loss_duplication_and_reordering`).
3. The same holds across sequence-number wrap-around with send window 4 and receive window
   8 (`small_windows_and_sequence_wraparound`).
4. Flipping any single bit of a valid packet makes it fail decoding. Arbitrary bytes never
   make the decoder panic (property tests in `transport::packet`).
5. Each §6.5 violation fails the connection. Garbage, foreign and out-of-window packets do
   not (`rejects_*`, `ignores_*`).
6. With no acknowledgements arriving and budget 3, the connection fails after exactly 3
   retransmissions, no earlier than 250 + 500 + 1000 + 2000 ms after the first send
   (`unresponsive_peer_fails_after_retransmit_budget`).
7. Back-pressure never loses data, including a stall of 120 s, which is longer than the
   whole retransmission budget. The sender does not retransmit while the peer is paused
   (`inbox_backpressure_pauses_then_resumes`,
   `backpressure_survives_stall_longer_than_retransmit_budget`). A paused peer that goes
   silent still fails by idle timeout (`paused_peer_that_goes_silent_times_out`).
8. Unreliable messages may be duplicated on the wire, and the receiver does not
   de-duplicate them. Applications that send them must tolerate repeats. This is intended.
9. Floods of empty unreliable or reliable messages keep the inbox within its budget, and
   out-of-order segments of any size keep the reorder buffer within its budget while still
   making progress (`inbox_budget_bounds_floods_of_empty_messages`,
   `reorder_buffer_is_bounded_by_bytes_not_peer_mtu`). Arbitrary sequences of garbage and
   well-formed hostile packets never panic the endpoint or exceed either budget
   (`hostile_datagram_sequences_stay_bounded`, plus the fuzz targets in `fuzz/`).
10. The send window is a sequence span (§6.1 item 3)
    (`send_window_is_a_sequence_span_from_oldest_unacked`). Bogus acknowledgements are
    counted whatever is queued (`bogus_acks_are_counted_even_with_nothing_queued`).

## 10. Open items

> **OPEN:** Handshake, version and MTU negotiation, and peer authentication (§3).

> **OPEN:** No encryption or integrity protection against an active attacker. The CRC only
> catches accidental corruption. A deployment over the internet needs an authenticated
> encryption layer, or a different transport such as QUIC (M7 decision record).

> **OPEN:** Selective acknowledgement covers only 32 segments past `ack_next`. With a large
> window under heavy loss, segments that arrived further ahead are retransmitted needlessly.
> In the 30% loss test a large share of retransmissions turn out to be duplicates at
> the receiver. Options: SACK
> ranges or a larger bitmap in a version 2.

> **OPEN:** No congestion control (the window is fixed) and no pacing. This is acceptable on
> loopback and LAN, but not for the internet.

> **OPEN:** One reliable stream only. Independent ordered channels, so that bulk transfers
> do not cause head-of-line blocking for chat, would need a channel id field.

> **OPEN:** (implementer note, transport fixes) §6.4 steps 3–4 and §8 steps 1–2 read as "store
> every segment of the poll's receive batch, then deliver". `next` then lags behind
> segments already received in the same batch, so in-order segments after the first one
> of a burst were charged to the reorder budget and dropped unacknowledged on a lossless
> link (with a reorder budget of 0, or with large datagrams). The implementation advances
> `next` through contiguous segments after **each** received datagram (still subject to
> back-pressure, §6.4 step 5), so `next` matches step 1's definition ("the lowest sequence
> number not yet received") whenever delivery is not paused, and §8 step 2 only resumes
> delivery after the application drained the inbox. The spec writer should restate §6.4
> steps 3–4 and §8 to say this (`in_order_burst_is_not_charged_to_reorder_budget`,
> `large_datagrams_on_lossless_link_never_retransmit`).

> **OPEN:** (implementer note, transport fixes) §6.3's Karn rule samples any segment acknowledged
> without a retransmission, which includes segments the peer held back while PAUSED: after
> a long stall the sample covers the whole stall and pins the RTO at the maximum. The
> implementation also excludes every segment that was outstanding (sent, unacknowledged)
> when a packet carrying PAUSED arrived, or that was first sent while the peer was
> paused; segments the PAUSED packet itself acknowledges are still sampled. The spec
> writer should add this exclusion to §6.3 (`paused_hold_is_not_an_rtt_sample`).
