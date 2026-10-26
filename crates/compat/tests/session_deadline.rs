// SPDX-License-Identifier: GPL-3.0-or-later
//! `SessionMultiplexer::next_deadline` (`docs/specs/official-session.md` §5.2–§5.4, §6.1,
//! §8): the earliest timer of the connections, so a caller can sleep until then. Each test
//! checks that a tick just before the deadline does nothing and a tick at it does the
//! timed work, on a virtual clock.

use std::net::{Ipv4Addr, SocketAddrV4};

use tessera_compat::connector::worker::{LOOP_WAIT_MAX, LOOP_WAIT_MIN, loop_wait};
use tessera_compat::session::{
    BLOB_REASSEMBLY_EXPIRY_SEC, BlobFragment, CLOSING_CLEANUP_SEC, ConnectChallengeHeader,
    ConnectionState, DISCONNECT_FLUSH_LIMIT_SEC, DISCONNECT_SENT_LIMIT_SEC, DatagramHeader,
    EXTENDED_SILENCE_TIMEOUT_SEC, IsaacCipher, OptionalHeaders, SessionAuthHeader, SessionEvent,
    SessionMultiplexer, WireCodecs, encode_datagram, split_blob,
};

const SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 40), 9000);
const OTHER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 41), 9000);
const ASSIGNED: u16 = 0x0111;
const SEED: u32 = 0xBEEF_0042;
/// Less than any timer step and more than float noise.
const JUST_BEFORE: f64 = 1e-3;

fn near(a: Option<f64>, b: f64) -> bool {
    a.is_some_and(|a| (a - b).abs() < 1e-9)
}

/// A multiplexer with one connection whose `SessionAuthHeader` went out at `t`.
fn at_login(t: f64) -> (SessionMultiplexer, u16) {
    let mut mux = SessionMultiplexer::new();
    let id = mux.create_connection(SERVER, t).expect("id");
    let conn = mux.connection_mut(id).expect("conn");
    conn.start_login(SessionAuthHeader::for_login("A", None, 0).expect("auth"), t)
        .expect("login");
    drain(&mut mux, id);
    (mux, id)
}

/// Datagrams queued on `id` (dropped).
fn drain(mux: &mut SessionMultiplexer, id: u16) -> usize {
    let conn = mux.connection_mut(id).expect("conn");
    let mut n = 0;
    while conn.pop_outbound_datagram().is_some() {
        n += 1;
    }
    while conn.pop_event().is_some() {}
    n
}

fn challenge() -> Vec<u8> {
    encode_datagram(
        DatagramHeader {
            rec_id: ASSIGNED,
            ..DatagramHeader::default()
        },
        &OptionalHeaders {
            connect_challenge: Some(ConnectChallengeHeader {
                server_time: 1.0,
                assigned_client_rec_id: ASSIGNED,
                server_rec_id: 0x0222,
                outgoing_seed: 0xCAFE_1234,
                incoming_seed: SEED,
                reserved_dword_14: 0,
                client_conn_idx: 4,
                reserved_word_1a: 0,
                codec_uuid: [0u8; 16],
            }),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .expect("challenge")
}

/// The server's sequenced packets, masked as §7.2 has it.
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

    /// The next sequenced packet; `skip` numbers are left out before it (a gap).
    fn packet(&mut self, skip: u32) -> Vec<u8> {
        for _ in 0..skip {
            self.seq += 1;
            let _ = self.isaac.next_mask();
        }
        let frags =
            split_blob(2, ((self.seq + 1) & 0xFFFF) as u16, 1, 0, None, 0x10, b"x").expect("split");
        self.carrying(&frags)
    }

    /// The next sequenced packet, carrying `frags`.
    fn carrying(&mut self, frags: &[BlobFragment]) -> Vec<u8> {
        self.seq += 1;
        let mask = self.isaac.next_mask();
        encode_datagram(
            DatagramHeader {
                rec_id: ASSIGNED,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            frags,
            Some(mask),
            None,
        )
        .expect("packet")
    }
}

fn receive(mux: &mut SessionMultiplexer, d: &[u8], t: f64) -> u16 {
    mux.receive_from_peer(d, SERVER, t, WireCodecs::NONE)
        .expect("accepted")
        .0
}

/// Ticks at `deadline - JUST_BEFORE` (nothing queued) and at `deadline` (something
/// queued); returns the datagrams of the second tick.
fn fires_at(mux: &mut SessionMultiplexer, id: u16, deadline: f64) -> usize {
    mux.tick(deadline - JUST_BEFORE).expect("tick");
    assert_eq!(drain(mux, id), 0, "nothing before {deadline}");
    mux.tick(deadline).expect("tick");
    let n = drain(mux, id);
    assert!(n > 0, "the timer of {deadline} fired");
    n
}

#[test]
fn an_empty_multiplexer_has_no_deadline() {
    let mut mux = SessionMultiplexer::new();
    assert_eq!(mux.next_deadline(0.0), None);
    // A connection never started (State 0) has no timer either.
    mux.create_connection(SERVER, 0.0).expect("id");
    assert_eq!(mux.next_deadline(5.0), None);
}

#[test]
fn the_session_auth_retransmit_and_handshake_timeout_are_deadlines() {
    let (mut mux, id) = at_login(0.0);
    let mut t = 0.0;
    for k in 1..=9 {
        let due = f64::from(k) * 2.0;
        assert!(near(mux.next_deadline(t), due), "retransmit {k}");
        fires_at(&mut mux, id, due);
        t = due;
    }
    // The last retransmit and the timeout coincide at 20 s (§6.1).
    assert!(near(mux.next_deadline(t), 20.0));
    mux.tick(20.0).expect("tick");
    let conn = mux.connection_mut(id).expect("conn");
    assert_eq!(conn.state(), ConnectionState::Closing);
    assert!(matches!(
        conn.pop_event(),
        Some(SessionEvent::Disconnected { .. })
    ));
    drain(&mut mux, id);
    // Then the cleanup of State 8, 20 s later (§6.1).
    assert!(near(mux.next_deadline(20.0), 40.0));
    assert!(mux.tick(40.0 - JUST_BEFORE).expect("tick").is_empty());
    assert_eq!(mux.tick(40.0).expect("tick"), vec![id]);
    assert_eq!(mux.next_deadline(40.0), None);
}

#[test]
fn the_connect_accept_retransmit_is_a_deadline() {
    let (mut mux, id) = at_login(0.0);
    let id2 = receive(&mut mux, &challenge(), 1.0);
    assert_eq!(id2, ASSIGNED);
    assert_ne!(id, id2);
    drain(&mut mux, id2);
    assert!(near(mux.next_deadline(1.0), 1.6));
    fires_at(&mut mux, id2, 1.6);
    assert!(near(mux.next_deadline(1.6), 2.2));
}

#[test]
fn acknowledgements_keep_alives_and_the_silence_timeout_are_deadlines() {
    let (mut mux, _) = at_login(0.0);
    let id = receive(&mut mux, &challenge(), 0.0);
    let mut server = Server::new();
    receive(&mut mux, &server.packet(0), 0.1);
    assert_eq!(
        mux.connection(id).expect("conn").state(),
        ConnectionState::Connected
    );
    drain(&mut mux, id);
    // A sequenced packet arrived: the cumulative acknowledgement, 2 s after the last
    // (§5.2; the first counts from the connection's start).
    assert!(near(mux.next_deadline(0.1), 2.0));
    fires_at(&mut mux, id, 2.0);
    assert!(near(mux.next_deadline(2.0), 4.0));
    // A gap: the negative acknowledgement every 0.6 s instead (§5.3).
    receive(&mut mux, &server.packet(1), 2.5);
    drain(&mut mux, id);
    assert!(near(mux.next_deadline(2.5), 2.6));
    fires_at(&mut mux, id, 2.6);
    assert!(near(mux.next_deadline(2.6), 3.2));
    // A time already past is "now".
    assert!(near(mux.next_deadline(10.0), 10.0));
}

#[test]
fn an_idle_connection_wakes_only_for_its_clock_sync_and_silence_timeout() {
    let (mut mux, _) = at_login(0.0);
    let id = receive(&mut mux, &challenge(), 0.0);
    // An unsequenced packet ends the handshake: no acknowledgement is owed (§5.2).
    let ctl = encode_datagram(
        DatagramHeader {
            rec_id: ASSIGNED,
            seq_num: 1,
            ..DatagramHeader::default()
        },
        &OptionalHeaders {
            cumulative_ack: Some(0),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .expect("control");
    receive(&mut mux, &ctl, 5.0);
    assert_eq!(
        mux.connection(id).expect("conn").state(),
        ConnectionState::Connected
    );
    drain(&mut mux, id);
    // The ClockSyncHeader 30 s after the one sent on entering State 5 (§8.1).
    assert!(near(mux.next_deadline(5.0), 35.0));
    fires_at(&mut mux, id, 35.0);
    assert!(near(mux.next_deadline(35.0), 65.0));
    // The next keep-alive and the silence timeout (60 s after the last packet, §8.2)
    // coincide at 65 s; the timeout fires once the limit is exceeded.
    mux.tick(64.0).expect("tick");
    drain(&mut mux, id);
    assert!(near(mux.next_deadline(64.0), 65.0));
    mux.tick(65.0 + JUST_BEFORE).expect("tick");
    assert_eq!(
        mux.connection(id).expect("conn").state(),
        ConnectionState::Closing
    );
}

#[test]
fn the_disconnect_flush_retransmit_and_end_are_deadlines() {
    let (mut mux, _) = at_login(0.0);
    let id = receive(&mut mux, &challenge(), 0.0);
    let mut server = Server::new();
    receive(&mut mux, &server.packet(0), 0.0);
    drain(&mut mux, id);
    mux.connection_mut(id)
        .expect("conn")
        .request_disconnect(1.0);
    // Nothing waits for an acknowledgement: the flush ends on the next tick (§6.1 State 6).
    assert!(near(mux.next_deadline(1.0), 1.0));
    mux.tick(1.0).expect("tick");
    assert_eq!(
        mux.connection(id).expect("conn").state(),
        ConnectionState::DisconnectSent
    );
    drain(&mut mux, id);
    // DisconnectHeader every 1.0 s (§8.2), for 20 s.
    assert!(near(mux.next_deadline(1.0), 2.0));
    fires_at(&mut mux, id, 2.0);
    assert!(near(mux.next_deadline(2.0), 3.0));
    for k in 3..=20 {
        let due = f64::from(k);
        assert!(
            near(mux.next_deadline(due - 1.0), due),
            "retransmit at {due}"
        );
        fires_at(&mut mux, id, due);
    }
    // State 7 ends 20 s after it began (§6.1 table); the retransmit due then is not sent.
    let end = 1.0 + DISCONNECT_SENT_LIMIT_SEC;
    assert!(near(mux.next_deadline(20.0), end));
    mux.tick(end - JUST_BEFORE).expect("tick");
    assert_eq!(state(&mux, id), ConnectionState::DisconnectSent);
    drain(&mut mux, id);
    mux.tick(end).expect("tick");
    assert_eq!(state(&mux, id), ConnectionState::Closing);
    assert_eq!(drain(&mut mux, id), 0, "no DisconnectHeader once closed");
    // Then the cleanup of State 8.
    assert!(near(mux.next_deadline(end), end + CLOSING_CLEANUP_SEC));
    assert_eq!(mux.tick(end + CLOSING_CLEANUP_SEC).expect("tick"), vec![id]);
    assert_eq!(mux.next_deadline(end + CLOSING_CLEANUP_SEC), None);
}

/// The state of connection `id`.
fn state(mux: &SessionMultiplexer, id: u16) -> ConnectionState {
    mux.connection(id).expect("conn").state()
}

/// An unsequenced control packet from the server carrying `opt`.
fn control(opt: OptionalHeaders) -> Vec<u8> {
    encode_datagram(
        DatagramHeader {
            rec_id: ASSIGNED,
            seq_num: 1,
            ..DatagramHeader::default()
        },
        &opt,
        &[],
        None,
        None,
    )
    .expect("control")
}

/// A connection in State 5 since `t`, reached with an unsequenced packet (so no
/// acknowledgement is owed), with nothing queued.
fn connected_at(t: f64) -> (SessionMultiplexer, u16) {
    let (mut mux, _) = at_login(0.0);
    let id = receive(&mut mux, &challenge(), 0.0);
    receive(
        &mut mux,
        &control(OptionalHeaders {
            cumulative_ack: Some(0),
            ..OptionalHeaders::default()
        }),
        t,
    );
    assert_eq!(state(&mux, id), ConnectionState::Connected);
    drain(&mut mux, id);
    (mux, id)
}

/// The connector's loop on a virtual clock from `t` until `until` or until `done` holds:
/// tick, then wait as [`loop_wait`] says. Asserts that no wait is the shortest unless a
/// deadline is that close (no spin). Returns the pass times and the time it stopped.
fn run_loop(
    mux: &mut SessionMultiplexer,
    id: u16,
    mut t: f64,
    until: f64,
    mut done: impl FnMut(&mut SessionMultiplexer, f64) -> bool,
) -> (Vec<f64>, f64) {
    let mut passes = Vec::new();
    while t < until {
        mux.tick(t).expect("tick");
        passes.push(t);
        if done(mux, t) {
            return (passes, t);
        }
        if mux.connection(id).is_some() {
            drain(mux, id);
        }
        let deadline = mux.next_deadline(t);
        let wait = loop_wait(t, deadline);
        if wait == LOOP_WAIT_MIN {
            let left = deadline.map_or(f64::INFINITY, |d| d - t);
            assert!(left <= LOOP_WAIT_MIN.as_secs_f64(), "spun at {t}");
        }
        t += wait.as_secs_f64();
    }
    (passes, t)
}

/// Whether a pass at `pass` is on time for a timer due at `due`: not before it and late by
/// at most the shortest wait (a pass that lands a hair before its timer waits 1 ms more).
fn on_time(pass: f64, due: f64) -> bool {
    pass >= due - 1e-9 && pass - due < 1.1e-3
}

/// Fewer passes than `secs` at the longest wait plus `extra`.
fn few(passes: &[f64], secs: f64, extra: usize) -> bool {
    passes.len() <= (secs / LOOP_WAIT_MAX.as_secs_f64()) as usize + extra
}

#[test]
fn out_of_order_acknowledgements_waiting_are_due_at_once() {
    let (mut mux, id) = connected_at(5.0);
    assert!(mux.next_deadline(5.0).is_some_and(|d| d > 5.5));
    // The server asks for a packet the client never sent: it goes on the
    // OutOfOrderAckHeader queue (§5.3), flushed on the next tick (§5.4).
    receive(
        &mut mux,
        &control(OptionalHeaders {
            sequence_nak: Some(vec![50]),
            ..OptionalHeaders::default()
        }),
        6.0,
    );
    drain(&mut mux, id);
    assert!(near(mux.next_deadline(6.0), 6.0), "due now");
    mux.tick(6.0).expect("tick");
    assert_eq!(drain(&mut mux, id), 1, "the OutOfOrderAckHeader");
    // Flushed: no longer due, so the loop does not stay at its shortest wait.
    assert!(mux.next_deadline(6.0).is_some_and(|d| d > 6.5));
}

#[test]
fn the_expiry_of_an_incomplete_blob_is_a_deadline() {
    let blob = vec![0x5Au8; 600];
    let frags = split_blob(2, 1, 7, 0, None, 0x10, &blob).expect("split");
    assert_eq!(frags.len(), 2);
    let (mut mux, _) = at_login(0.0);
    let id = receive(&mut mux, &challenge(), 0.0);
    let mut server = Server::new();
    receive(&mut mux, &server.packet(0), 0.1);
    receive(&mut mux, &server.carrying(&frags[..1]), 5.0);
    drain(&mut mux, id);
    let expiry = 5.0 + BLOB_REASSEMBLY_EXPIRY_SEC;
    // The acknowledgements every 2 s (at 34 and 36 s) and the clock sync (30.1 s) are
    // the other timers: a pass at 35 s is the expiry's.
    let (passes, end) = run_loop(&mut mux, id, 5.0, expiry + 1.0, |_, t| t > expiry);
    // Purged within the shortest wait of its time, not up to the longest wait late.
    assert!(on_time(end, expiry), "purged at {end}");
    assert!(few(&passes, expiry - 5.0, 40), "{} passes", passes.len());
    // Purged: the second fragment starts a new blob instead of completing it.
    receive(&mut mux, &server.carrying(&frags[1..]), end);
    let conn = mux.connection_mut(id).expect("conn");
    while let Some(e) = conn.pop_event() {
        assert!(
            !matches!(e, SessionEvent::BlobReceived(_)),
            "the expired blob completed"
        );
    }
}

#[test]
fn an_incomplete_blob_completes_before_its_expiry() {
    let blob = vec![0x5Au8; 600];
    let frags = split_blob(2, 1, 7, 0, None, 0x10, &blob).expect("split");
    let (mut mux, _) = at_login(0.0);
    let id = receive(&mut mux, &challenge(), 0.0);
    let mut server = Server::new();
    receive(&mut mux, &server.packet(0), 0.1);
    receive(&mut mux, &server.carrying(&frags[..1]), 5.0);
    drain(&mut mux, id);
    let expiry = 5.0 + BLOB_REASSEMBLY_EXPIRY_SEC;
    let (_, end) = run_loop(&mut mux, id, 5.0, expiry - 0.5, |_, _| false);
    receive(&mut mux, &server.carrying(&frags[1..]), end);
    let conn = mux.connection_mut(id).expect("conn");
    let mut done = false;
    while let Some(e) = conn.pop_event() {
        done |= matches!(e, SessionEvent::BlobReceived(_));
    }
    assert!(done, "the blob completed (the control of the expiry test)");
}

#[test]
fn the_extended_silence_limit_is_a_deadline() {
    let (mut mux, id) = connected_at(5.0);
    mux.connection_mut(id)
        .expect("conn")
        .set_extended_silence_timeout(true);
    // The last packet at 7 s: the limit ends at 307 s, apart from the clock syncs (every
    // 30 s from 5 s, §8.1).
    receive(
        &mut mux,
        &control(OptionalHeaders {
            cumulative_ack: Some(0),
            ..OptionalHeaders::default()
        }),
        7.0,
    );
    drain(&mut mux, id);
    let limit = 7.0 + EXTENDED_SILENCE_TIMEOUT_SEC;
    let (passes, end) = run_loop(&mut mux, id, 7.0, limit + 1.0, |m, _| {
        state(m, id) == ConnectionState::Closing
    });
    // Past the normal 60 s limit it was still connected (it ran on).
    assert!(passes.iter().any(|&p| p > 70.0));
    // The limit fires once exceeded, within the shortest wait of its time (§8.2).
    assert!(end > limit && on_time(end, limit), "closed at {end}");
    assert!(few(&passes, limit - 7.0, 20), "{} passes", passes.len());
}

#[test]
fn the_flush_waits_twenty_seconds_for_unacknowledged_packets() {
    let (mut mux, _) = at_login(0.0);
    let id = receive(&mut mux, &challenge(), 0.0);
    let mut server = Server::new();
    receive(&mut mux, &server.packet(0), 0.1);
    let conn = mux.connection_mut(id).expect("conn");
    conn.send_blob(1, 0, 0x10, b"unacknowledged", 1.0, None)
        .expect("blob");
    drain(&mut mux, id);
    mux.connection_mut(id)
        .expect("conn")
        .request_disconnect(3.0);
    assert_eq!(state(&mux, id), ConnectionState::DisconnectingFlush);
    // The retransmit table is not empty: State 6 waits up to 20 s (§6.1 table, §8.2), and
    // that is its only timer.
    let end = 3.0 + DISCONNECT_FLUSH_LIMIT_SEC;
    assert!(near(mux.next_deadline(3.0), end));
    let (passes, at) = run_loop(&mut mux, id, 3.0, end + 1.0, |m, _| {
        state(m, id) == ConnectionState::DisconnectSent
    });
    assert!(on_time(at, end), "the DisconnectHeader at {at}");
    assert!(few(&passes, end - 3.0, 2), "{} passes", passes.len());
}

#[test]
fn the_multiplexer_takes_the_earliest_of_its_connections() {
    let mut mux = SessionMultiplexer::new();
    let a = mux.create_connection(SERVER, 0.0).expect("a");
    let b = mux.create_connection(OTHER, 0.0).expect("b");
    mux.connection_mut(a)
        .expect("a")
        .start_login(
            SessionAuthHeader::for_login("A", None, 0).expect("auth"),
            0.0,
        )
        .expect("login");
    mux.connection_mut(b)
        .expect("b")
        .start_login(
            SessionAuthHeader::for_login("B", None, 0).expect("auth"),
            0.7,
        )
        .expect("login");
    assert!(near(mux.next_deadline(0.7), 2.0));
    mux.tick(2.0).expect("tick");
    assert!(near(mux.next_deadline(2.0), 2.7));
    drain(&mut mux, a);
    drain(&mut mux, b);
}

/// The connector's wait: until the deadline, at least 1 ms, at most 50 ms.
#[test]
fn the_loop_waits_until_the_deadline_within_its_bounds() {
    assert_eq!(loop_wait(0.0, None), LOOP_WAIT_MAX);
    assert_eq!(loop_wait(0.0, Some(10.0)), LOOP_WAIT_MAX);
    assert_eq!(loop_wait(1.0, Some(1.0)), LOOP_WAIT_MIN);
    assert_eq!(loop_wait(1.0, Some(0.5)), LOOP_WAIT_MIN);
    assert_eq!(loop_wait(1.0, Some(f64::NAN)), LOOP_WAIT_MIN);
    let w = loop_wait(1.0, Some(1.02));
    assert!((w.as_secs_f64() - 0.02).abs() < 2e-6, "{w:?}");
}

/// A virtual-clock run of the connector's loop on an idle connection in its handshake:
/// every pass waits the longest wait or until a timer, never less (no busy wake-ups), and
/// each retransmit goes out within a millisecond of its time.
#[test]
fn an_idle_loop_does_not_spin_and_its_timers_fire_on_time() {
    let (mut mux, id) = at_login(0.0);
    let mut t = 0.0f64;
    let mut passes = 0usize;
    let mut sent_at = Vec::new();
    while t < 19.5 {
        mux.tick(t).expect("tick");
        if drain(&mut mux, id) > 0 {
            sent_at.push(t);
        }
        passes += 1;
        let deadline = mux.next_deadline(t);
        let wait = loop_wait(t, deadline);
        let to_deadline = deadline.map_or(f64::INFINITY, |d| d - t);
        // The wait is the cap, or ends at the deadline (1 ms at least).
        assert!(
            wait == LOOP_WAIT_MAX
                || (wait.as_secs_f64() - to_deadline).abs() < 2e-6
                || wait == LOOP_WAIT_MIN,
            "pass at {t}: {wait:?} for a deadline {to_deadline} s away"
        );
        if wait == LOOP_WAIT_MIN {
            assert!(to_deadline <= LOOP_WAIT_MIN.as_secs_f64(), "spun at {t}");
        }
        t += wait.as_secs_f64();
    }
    // Each retransmit 2 s after the one before (§6.1), late by at most the shortest wait
    // (a pass that lands a hair before its timer waits 1 ms more).
    assert_eq!(sent_at.len(), 9, "{sent_at:?}");
    let mut due = 2.0;
    for sent in &sent_at {
        assert!(
            *sent >= due && *sent - due < 1.1e-3,
            "sent at {sent}, due {due}"
        );
        due = sent + 2.0;
    }
    // 19.5 s at 50 ms, plus at most two passes per timer.
    assert!(passes <= 390 + 2 * sent_at.len() + 1, "{passes} passes");
}
