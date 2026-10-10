// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for review findings on the test-server backend: the movement schedule
//! under vertical motion and harness clock steps (§8.2), the correction path (§8.4), the
//! goodbye on drop (§12.3), the known-servers file failing closed (§4.5), and one shared
//! network thread for every session (ARCH §19.4).

#![cfg(feature = "remote")]

mod remote_support;

use std::time::{Duration, Instant};

use remote_support::*;
use tessera_net::remote::{
    AppErrorCode, ClientFailure, FirstUse, HarnessInput, InboundHarness, Password, RemoteConfig,
    RemoteFailure,
};
use tessera_net::{
    Command, DisconnectReason, EntityId, Event, Location, LocationCause, RegionId, Session, Vec3,
};
use tessera_protocol::connect::{ServerKey, format_entry};
use tessera_protocol::frame::frame_to_vec;
use tessera_protocol::message::{
    HelloAck, LocationCause as Cause, LocationReport, LoginOk, Message, ServerCaps, WorldEnter,
};
use tessera_protocol::types::{Appearance, Body, CharacterSummary, Location as WLoc, Pose};

/// One 30 Hz tick, as the harness counts it (1 s / 30, truncated to whole µs).
const TICK: Duration = Duration::from_micros(33_333);

fn frame(m: Message) -> Vec<u8> {
    frame_to_vec(&m).unwrap()
}

fn start() -> Location {
    Location::new(RegionId(1), 20_250.5, 22_500.25, 401.0)
}

/// Inputs that log in and, after `EnterWorld`, enter the world at [`start`] with epoch 1.
fn into_world() -> Vec<HarnessInput> {
    vec![
        HarnessInput::Control(frame(Message::HelloAck(HelloAck {
            minor: 0,
            server_caps: ServerCaps::NONE,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "h".into(),
            motd: String::new(),
        }))),
        HarnessInput::Control(frame(Message::LoginOk(LoginOk {
            account: "fuzz".into(),
            characters: vec![CharacterSummary {
                name: "Aldith".into(),
                body: Body::Character { species: 1, sex: 1 },
            }],
            looks: Vec::new(),
        }))),
        HarnessInput::OpenStream(0x43),
        HarnessInput::OpenStream(0x57),
        HarnessInput::Stream {
            world: true,
            data: frame(Message::WorldEnter(WorldEnter {
                player: 42,
                name: "Aldith".into(),
                instance: 1,
                epoch: 1,
                server_time: 5_000_000,
                pose: Pose {
                    location: WLoc::new(1, 20_250.5, 22_500.25, 401.0),
                    heading: 0.0,
                    anchor: None,
                },
                appearance: Appearance::new(Body::Character { species: 1, sex: 1 }),
                look: tessera_protocol::types::Look::none(),
            })),
        },
    ]
}

/// A harness session in the world, with `then` queued after the world entry.
fn in_world(then: impl IntoIterator<Item = HarnessInput>) -> (Session, Vec<Event>) {
    let mut inputs = into_world();
    inputs.extend(then);
    let mut s = Session::new(InboundHarness::new(inputs));
    s.send(Instant::now(), Command::Connect).unwrap();
    let mut events = Vec::new();
    for _ in 0..3 {
        s.poll_into(Instant::now(), &mut events);
    }
    s.send(
        Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    for _ in 0..2 {
        s.poll_into(Instant::now(), &mut events);
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. })),
        "{events:#?}"
    );
    (s, events)
}

fn harness(s: &Session) -> &InboundHarness {
    s.backend::<InboundHarness>().unwrap()
}

fn move_inputs(s: &Session) -> Vec<tessera_protocol::message::MoveInput> {
    harness(s)
        .sent()
        .filter_map(|m| match m {
            Message::MoveInput(mi) => Some(*mi),
            _ => None,
        })
        .collect()
}

/// Moves the player for `frames` frames of `step` each with `velocity`, one
/// `Command::Move` and one `Advance(step)` per frame. Returns the `MoveInput`s sent meanwhile.
fn drive(velocity: Vec3, step: Duration, frames: u32) -> Vec<tessera_protocol::message::MoveInput> {
    let (mut s, _) = in_world((0..frames).map(|_| HarnessInput::Advance(step)));
    // Settle the world-entry send first.
    let before = move_inputs(&s).len();
    let mut loc = start();
    for _ in 0..frames {
        let dt = step.as_secs_f64();
        loc.x += f64::from(velocity.x) * dt;
        loc.y += f64::from(velocity.y) * dt;
        loc.z += f64::from(velocity.z) * dt;
        s.send(
            Instant::now(),
            Command::Move {
                location: loc,
                heading: 0.0,
                velocity,
            },
        )
        .unwrap();
        let mut out = Vec::new();
        s.poll_into(Instant::now(), &mut out);
    }
    move_inputs(&s).split_off(before)
}

#[test]
fn vertical_motion_is_sent_at_15_hz() {
    // Finding: only horizontal speed counted as moving, so a jump in place or a fall went
    // out as the 1 Hz heartbeat (1 MoveInput in 1 s) and observers never saw it.
    let vertical = drive(Vec3::new(0.0, 0.0, 7.0), TICK, 30);
    assert!(
        (14..=16).contains(&vertical.len()),
        "{} MoveInputs in 1 s of vertical motion",
        vertical.len()
    );
    let horizontal = drive(Vec3::new(4.0, 0.0, 0.0), TICK, 30);
    assert!(
        (14..=16).contains(&horizontal.len()),
        "{} MoveInputs in 1 s of walking",
        horizontal.len()
    );
}

#[test]
fn advances_off_the_tick_grid_keep_the_real_rate() {
    // Finding: an Advance not a multiple of the tick ran an extra schedule tick, so
    // 33.334 ms frames produced 30 MoveInputs per second instead of 15.
    let sent = drive(Vec3::new(4.0, 0.0, 0.0), Duration::from_micros(33_334), 30);
    assert!((14..=16).contains(&sent.len()), "{}", sent.len());
    // Many tiny advances: at most one tick per whole tick of virtual time.
    let sent = drive(Vec3::new(4.0, 0.0, 0.0), Duration::from_millis(5), 200);
    assert!((14..=16).contains(&sent.len()), "{}", sent.len());
}

#[test]
fn correction_adopts_the_new_epoch_and_drops_older_moves() {
    let corrected = WLoc::new(1, 20_240.0, 22_500.0, 401.0);
    let report = HarnessInput::Stream {
        world: true,
        data: frame(Message::LocationReport(LocationReport {
            cause: Cause::Corrected,
            request_id: 0,
            epoch: 2,
            server_time: 6_000_000,
            pose: Pose {
                location: corrected,
                heading: 1.0,
                anchor: None,
            },
        })),
    };
    let (mut s, _) = in_world([
        HarnessInput::Advance(Duration::from_millis(100)),
        report,
        HarnessInput::Advance(Duration::from_millis(300)),
        HarnessInput::Advance(Duration::from_millis(300)),
    ]);
    // A move made before the client saw the correction (stamped with epoch 1).
    let mut ahead = start();
    ahead.x += 0.5;
    let stale = Command::Move {
        location: ahead,
        heading: 0.0,
        velocity: Vec3::new(4.0, 0.0, 0.0),
    };
    s.send(Instant::now(), stale).unwrap();
    let mut events = Vec::new();
    s.poll_into(Instant::now(), &mut events); // Advance 100 ms: epoch-1 moves go out
    let before = move_inputs(&s).len();
    assert!(move_inputs(&s).iter().all(|m| m.epoch == 1));
    events.clear();
    s.poll_into(Instant::now(), &mut events); // the correction
    // The mapper surfaces the move first, then the report (Appendix A).
    let moved = events.iter().position(|e| {
        matches!(
            e,
            Event::EntityMoved {
                id: EntityId(42),
                ..
            }
        )
    });
    let report = events.iter().position(|e| {
        matches!(
            e,
            Event::LocationReport {
                cause: LocationCause::Corrected,
                ..
            }
        )
    });
    assert!(
        matches!((moved, report), (Some(a), Some(b)) if a < b),
        "{events:#?}"
    );
    // A MoveInput with the new epoch and the corrected pose went out at once.
    let after = move_inputs(&s).split_off(before);
    let first = after.first().expect("no MoveInput on the correction");
    assert_eq!(first.epoch, 2);
    assert_eq!(first.location, corrected);
    assert_eq!(first.heading, 1.0);
    // The stale epoch-1 move still in the slot is never sent again.
    s.poll_into(Instant::now(), &mut events); // Advance 300 ms
    let after = move_inputs(&s).split_off(before);
    assert!(after.iter().all(|m| m.epoch == 2), "{after:?}");
    assert!(after.iter().all(|m| m.location == corrected), "{after:?}");
    // A move made after the correction is stamped with epoch 2 and sent.
    let mut next = Location::new(RegionId(1), 20_240.0, 22_500.0, 401.0);
    next.x += 0.2;
    s.send(
        Instant::now(),
        Command::Move {
            location: next,
            heading: 1.0,
            velocity: Vec3::new(4.0, 0.0, 0.0),
        },
    )
    .unwrap();
    let n = move_inputs(&s).len();
    s.poll_into(Instant::now(), &mut events); // Advance 300 ms
    let fresh = move_inputs(&s).split_off(n);
    assert!(!fresh.is_empty());
    assert!(fresh.iter().all(|m| m.epoch == 2));
    assert!(fresh.iter().any(|m| m.location != corrected));
}

#[test]
fn dropping_the_backend_in_world_says_goodbye() {
    // §12.3: dropping the backend asks the network thread to send Disconnect(NO_ERROR).
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        loop {
            match p.recv().await {
                Some(m @ Message::Disconnect(_)) => return Some(m),
                Some(_) => {}
                None => return None,
            }
        }
    });
    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    s.send(
        Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    pump(&mut s, |e| matches!(e, Event::WorldEntered { .. }));
    let _ = pump_for(&mut s, Duration::from_millis(100));
    drop(s);
    match peer.join() {
        Some(Message::Disconnect(d)) => assert_eq!(d.code, AppErrorCode::NoError),
        other => panic!("expected Disconnect(NO_ERROR), got {other:?}"),
    }
}

/// A connection with no pin, the given known-servers file and trust on first use, to an
/// address nothing listens on: the client must fail before any network traffic.
fn connect_with_file(file: std::path::PathBuf) -> Session {
    let config = RemoteConfig::new(
        "tessera://127.0.0.1:9",
        "alice",
        Password::new("correct horse"),
    )
    .unwrap()
    .with_trust(trust(file, FirstUse::Accept));
    connect(config)
}

fn assert_unreadable_file_refused(s: &mut Session) {
    let started = Instant::now();
    let events = pump(s, |e| matches!(e, Event::Disconnected { .. }));
    match events.last() {
        Some(Event::Disconnected {
            reason: DisconnectReason::TransportFailed(t),
        }) => {
            assert!(t.starts_with("KEY_UNKNOWN"), "{t}");
            assert!(t.contains("known-servers file"), "{t}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        backend(s).failure(),
        Some(&RemoteFailure::Connect {
            failure: ClientFailure::KeyUnknown,
            presented: None,
        })
    );
    // Before any handshake (an attempt would wait for the 10 s timeout).
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn oversized_known_servers_file_fails_closed() {
    // Finding: an entry past the 1 MiB read cap was missed silently, so trust on first use
    // accepted a key the file contradicts, and the key was appended a second time.
    let dir = temp_dir("regress-big");
    let file = dir.join("known_servers");
    let key = ServerKey::parse("127.0.0.1:9").unwrap();
    let other = tessera_protocol::fingerprint::Fingerprint::parse(
        "sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9",
    )
    .unwrap();
    let mut text = "#".repeat(1 << 20);
    text.push('\n');
    text.push_str(&format_entry(&key, &other));
    std::fs::write(&file, &text).unwrap();
    let mut s = connect_with_file(file.clone());
    assert_unreadable_file_refused(&mut s);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        text,
        "file changed"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unreadable_known_servers_file_fails_closed() {
    // Finding: a file that could not be read (permissions, I/O error) loaded as empty.
    // A directory in its place fails to read for any user, root included.
    let dir = temp_dir("regress-unreadable");
    let file = dir.join("known_servers");
    std::fs::create_dir_all(&file).unwrap();
    let mut s = connect_with_file(file);
    assert_unreadable_file_refused(&mut s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Threads of this process named like the backend's network thread.
#[cfg(target_os = "linux")]
fn network_threads() -> usize {
    std::fs::read_dir("/proc/self/task")
        .unwrap()
        .filter_map(|t| std::fs::read_to_string(t.ok()?.path().join("comm")).ok())
        .filter(|name| name.trim_end() == "tessera-net")
        .count()
}

/// Regression (M7 exit check): every backend in the process shares one network thread. A
/// thread and runtime per session left memory in the allocator's per-thread arenas at every
/// reconnection, so a client or load test that reconnected often grew without bound
/// (`docs/perf/m7-soak.md`).
#[cfg(target_os = "linux")]
#[test]
fn sessions_share_one_network_thread() {
    // A UDP port nobody answers on: each session stays in its handshake until dropped.
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = sock.local_addr().unwrap();
    let pin = format!("sha256:{}", "ab".repeat(32));
    for _round in 0..3 {
        let sessions: Vec<Session> = (0..8)
            .map(|_| {
                let config = RemoteConfig::new(
                    &format!("tessera://{addr}?pin={pin}"),
                    "alice",
                    Password::new("x"),
                )
                .unwrap();
                connect(config)
            })
            .collect();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(network_threads(), 1, "one network thread for every session");
        drop(sessions);
    }
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        network_threads(),
        1,
        "the network thread outlives its sessions"
    );
    drop(sock);
}
