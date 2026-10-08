<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0030 — Transport for the Tessera protocol: QUIC (quinn, rustls on ring)

**Status:** Accepted, 2026-10-06

The number 0030 is out of sequence on purpose: parallel milestone branches take 0017 onwards.

## Context
M7 builds Tessera's own test server and its own protocol (`docs/ROADMAP.md` M7,
[0004](0004-server-strategy.md): nothing about official servers). `docs/ARCHITECTURE.md`
§19.2 asks for "its own handshake, its own encryption (standard, documented algorithms)",
and §19.4 for socket I/O on a dedicated network thread that runs a single-threaded async
runtime and talks to the main thread over bounded channels. Exit criterion 2 asks that a
second implementation can be written from the spec alone.

The project already has TRD v1 (`docs/specs/transport-design.md`, `tessera_net::transport`):
a tested, poll-based reliable-datagram layer. Its own open items rule it out for anything
beyond loopback and LAN as it stands. It has no handshake, no peer authentication, no
encryption (the CRC only catches accidents), no congestion control or pacing, one ordered
stream (so bulk entity state blocks chat), and a 32-segment selective-ack bitmap.

Candidates:

**(a) QUIC (RFC 9000, 9001, 9002) via `quinn` 0.11, TLS 1.3 from `rustls` 0.23 on the `ring`
provider.** TLS 1.3 gives the handshake, key exchange and AEAD (RFC 8446). The server is
authenticated by its own key, pinned by fingerprint. QUIC brings loss recovery and congestion
control (CUBIC by default in quinn, RFC 9438; NewReno per RFC 9002 is available). Its
independent streams remove TRD's head-of-line blocking, and unreliable datagrams (RFC 9221)
suit movement. Connection IDs survive NAT rebinding. ALPN (RFC 7301) carries the major
version.

**(b) TRD v2 plus a Noise handshake (`snow`, Noise Protocol Framework rev. 34, for example
`Noise_XX_25519_ChaChaPoly_BLAKE2s`) over UDP.** This keeps the poll-based, thread-free
design of TRD v1. We would have to add, specify and test ourselves: a handshake state
machine and its packet formats, per-packet AEAD with nonce and replay windows, rekeying,
congestion control and pacing, PMTU handling, multiple ordered channels, wider selective
acks, and address migration.

| Criterion | (a) QUIC | (b) TRD v2 + Noise |
|---|---|---|
| Standard documented crypto | TLS 1.3 (RFC 8446) inside QUIC (RFC 9001); AEAD and key schedule are standards with many audited implementations | Noise framework (documented, not an IETF standard); the record layer around it (nonces, replay window, rekey) would be our own design, which is the risky part |
| Internet readiness | Congestion control, pacing, PMTU discovery, anti-amplification, address validation and migration come built in | Congestion control, pacing and PMTU would be ours to design; LAN-only until then |
| LAN-first goal (M7 exit criterion 1) | Works on LAN and loopback with no extra work | Works on LAN today (v1), needs v2 for security |
| Second implementation from the spec | Any conforming QUIC stack (quiche, ngtcp2, msquic, quic-go, aioquic, ...) plus our application spec. Our spec only has to define the application layer | Needs a byte-exact re-implementation of TRD v2's reliability, the record layer and the Noise binding: a much larger spec surface with more room for divergence |
| Dependency and licence footprint | quinn, quinn-proto, quinn-udp (MIT OR Apache-2.0), rustls (Apache-2.0 OR ISC OR MIT), ring (Apache-2.0 AND ISC), rustls-pki-types/webpki (ISC/MIT/Apache), tokio (MIT), rcgen for key generation (MIT OR Apache-2.0). All are in the `deny.toml` allow list. `aws-lc-rs`/`aws-lc-sys` (OpenSSL licence, rejected) are excluded by turning default features off and selecting `rustls-ring`/`ring` | snow plus pure-Rust primitives (Apache-2.0 OR MIT), no runtime. Smaller |
| Build time | ring compiles C and assembly; the workspace already needs a C compiler (`cc` for `lua-src`). Roughly a dozen more crates. Acceptable | Smallest |
| Testability | The protocol crate stays I/O-free (codec tests, property tests, fuzzing). Transport tests run quinn over loopback UDP; loss is injected through a custom UDP socket or an external impairment tool. Wireshark can decrypt our own traffic with an `SSLKEYLOGFILE` key log in debug builds | Fully deterministic in-memory tests (TRD v1 style), but every new mechanism needs its own test suite |
| ARCH §19.4 (network thread, single-threaded async runtime, bounded channels) | Matches it exactly: tokio `current_thread` on the network thread | Would not need the runtime at all, which is simpler but differs from §19.4 |
| MSRV 1.90 | quinn 0.11.12 needs 1.85, rcgen 0.14 1.88, rustls 1.71: fine | snow 0.10 needs 1.85: fine |

## Decision
1. **Tessera protocol v1 runs over QUIC version 1 (RFC 9000) using `quinn` 0.11 with
   `rustls` 0.23 on the `ring` crypto provider.** Default features are off, so `aws-lc-rs`
   is never built (as the root `Cargo.toml` already configures). No `openssl`.
2. **Authentication.** The server authenticates with a project-generated Ed25519 key in a
   self-signed certificate. The client checks only the SHA-256 fingerprint of the
   certificate's SubjectPublicKeyInfo against a pin, given in the connect string or recorded
   on first use, plus the TLS 1.3 CertificateVerify signature. No web PKI, no certificate
   authorities, no client certificates. Accounts are authenticated inside the encrypted
   connection with an account name and password, stored server-side as Argon2id
   (`test-server.md`). There is no 0-RTT and no session resumption. The details are in
   `docs/specs/tessera-protocol.md` §4.
3. **Channels.** One client-opened bidirectional control stream, two server-opened
   unidirectional streams (world state and chat), and RFC 9221 datagrams for movement and
   time sync (`tessera-protocol.md` §2).
4. **Runtime and threading.**
   - *Client* (`tessera-net` test-server backend): one dedicated network thread runs a
     tokio `current_thread` runtime with the quinn endpoint. It decodes messages with
     `tessera-protocol` and posts domain `Event`s to the main thread over a bounded
     channel. Commands go the other way over a second bounded channel. `Backend::poll`
     drains without blocking and `Backend::submit` never blocks (a full channel is a
     `NetError`), as ARCH §19.4 requires. The sandbox backend is unchanged and inline.
   - *Server* (`tessera-server`): one network thread with a tokio `current_thread` runtime
     owns the quinn endpoint and one task per connection. One simulation thread runs the
     fixed 30 Hz tick (ARCH §20.3) and owns all world state. Bounded channels connect them in
     both directions, with drop or disconnect rules for slow consumers
     (`tessera-protocol.md` §9.5). A multi-threaded runtime is a later option and needs no
     protocol change.
   - `tessera-protocol` stays free of I/O and of async code: schema, framing, encode and
     decode, version negotiation and fingerprint formatting only.
5. **TRD v1** (`transport-design.md`) is not used by the Tessera protocol. It stays in
   `tessera_net::transport` with its tests and fuzz targets, frozen at v1, as a tested
   in-memory and loopback reliability layer. A later decision may remove it once nothing
   uses it. Its open items (handshake, encryption, congestion control, one stream,
   32-segment SACK) are closed as "answered by QUIC" for Tessera's network use. There will
   be no TRD v2 unless a new decision asks for one.
6. **Ports.** UDP, chosen by the operator. The project defines no default port and no port
   tied to any other service. A connect string always names the port.

## Consequences
- The protocol spec only has to define the application layer. A second implementation can
  use any RFC 9000 stack that supports ALPN, RFC 9221 datagrams, Ed25519 server keys
  (RFC 8410, TLS signature scheme `ed25519`) and a custom certificate check.
- Runtime dependencies grow (tokio, quinn, rustls, ring). They are confined to
  `tessera-net`'s test-server backend and `tessera-server`. The protocol crate, the
  sandbox, the explorer and the offline walker do not need them, and should depend on them
  only through optional features where practical.
- `cargo deny` must keep passing. Any dependency that pulls `aws-lc-sys` or `openssl` is a
  build break, not a waiver.
- Deterministic network-impairment tests at the transport level are harder than with TRD's
  in-memory pipe. They are replaced by codec-level fuzzing, loopback integration tests, and
  load and soak runs (`apps/loadtest`).
- Internet play becomes possible later without a protocol redesign. It still stays T2
  (`docs/LEGAL.md` §8): a project-owned server and protocol, with content limited by
  `server-content.md`.
