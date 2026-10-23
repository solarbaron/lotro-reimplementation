// SPDX-License-Identifier: GPL-3.0-or-later
//! Test-server backend: world entry, entity lifecycle, properties, chat, movement,
//! teleports, statics from the client's own data, and interpolation
//! (`tessera-protocol.md` §6–§11, Appendix A), against a scripted peer.

#![cfg(feature = "remote")]

mod remote_support;

use std::time::{Duration, Instant};

use remote_support::*;
use tessera_net::interp::SampleKind;
use tessera_net::remote::{RemoteBackend, Stance};
use tessera_net::sandbox::world::{Landblock, Lookup, StaticEntity, WorldSource};
use tessera_net::{
    ChatChannel, Command, DataId, Destination, DisconnectReason, EntityId, Event, Location,
    LocationCause, Placement, RegionId, Rotation, Session, Vec3,
};
use tessera_protocol::message::{
    ChatChannel as WireChannel, ChatMessage, EntityAppearance, EntityUpdate,
    LocationCause as Cause, LocationReport, Message, MessageType,
};
use tessera_protocol::types::{Appearance, Body, Gait, Location as WLoc, Pose};
use tessera_protocol::value::{Property, Value, pid};

fn send(s: &mut Session, c: Command) {
    s.send(Instant::now(), c).unwrap();
}

fn enter(s: &mut Session) {
    pump(s, |e| matches!(e, Event::Connected { .. }));
    send(
        s,
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    );
}

#[derive(Debug, Default)]
struct Seen {
    first_move_epoch: u32,
    first_move_gait: Option<Gait>,
    teleport_move: Option<WLoc>,
    chat: Option<String>,
    emote: Option<u32>,
    stance: Option<Stance>,
}

#[test]
fn world_lifecycle_end_to_end() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        let mut seen = Seen::default();
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        let now = p.now();
        p.world(Message::EntityUpdate(EntityUpdate {
            id: 42,
            changes: vec![Property::new(pid::RUN_SPEED, Value::Float32(8.0))],
        }))
        .await;
        p.world(entity_create(43, now, 20_260.0, 2.0)).await;
        p.world(Message::EntityAppearance(EntityAppearance {
            id: 43,
            appearance: Appearance::new(Body::Object { did: 0x7900_0001 }),
            look: tessera_protocol::types::Look::none(),
        }))
        .await;
        p.chat(Message::ChatMessage(ChatMessage {
            server_time: now,
            channel: WireChannel::Say,
            echo: false,
            sender_entity: 43,
            sender: "Bram".into(),
            text: "hello".into(),
        }))
        .await;
        p.chat(Message::ChatMessage(ChatMessage {
            server_time: now,
            channel: WireChannel::System,
            echo: false,
            sender_entity: 0,
            sender: String::new(),
            text: "Welcome".into(),
        }))
        .await;
        // The client answers the greeting.
        if let Message::ChatSend(c) = p.expect(MessageType::ChatSend).await {
            seen.chat = Some(c.text);
        }
        p.datagram(&motion(p.now(), 43, 20_260.2, 2.0));
        // The client runs: a MoveInput with motion Running at the current epoch.
        let m = p
            .datagram_where(|m| matches!(m, Message::MoveInput(i) if i.velocity.x > 1.0))
            .await;
        if let Message::MoveInput(i) = m {
            seen.first_move_epoch = i.epoch;
            seen.first_move_gait = Some(i.motion.gait);
        }
        // Teleport: reset interest, then the report with epoch + 1.
        let Message::TeleportRequest(t) = p.expect(MessageType::TeleportRequest).await else {
            unreachable!()
        };
        p.world(Message::InterestReset).await;
        let pose = Pose {
            location: WLoc::new(1, 1_000.0, 1_000.0, 10.0),
            heading: 1.0,
            anchor: None,
        };
        p.world(Message::LocationReport(LocationReport {
            cause: Cause::Teleported,
            request_id: t.request_id,
            epoch: 2,
            server_time: p.now(),
            pose,
        }))
        .await;
        // §8.4: the client adopts the pose and sends a MoveInput with the new epoch at once.
        let m = p
            .datagram_where(|m| matches!(m, Message::MoveInput(i) if i.epoch == 2))
            .await;
        if let Message::MoveInput(i) = m {
            seen.teleport_move = Some(i.location);
        }
        let Message::QueryLocation { request_id } = p.expect(MessageType::QueryLocation).await
        else {
            unreachable!()
        };
        p.world(Message::LocationReport(LocationReport {
            cause: Cause::Requested,
            request_id,
            epoch: 2,
            server_time: p.now(),
            pose,
        }))
        .await;
        if let Message::EmoteSet(e) = p.expect(MessageType::EmoteSet).await {
            seen.emote = Some(e.anim);
        }
        if let Message::StanceSet(st) = p.expect(MessageType::StanceSet).await {
            seen.stance = Some(st);
        }
        p.expect(MessageType::Disconnect).await;
        seen
    });

    let mut s = connect(peer.config());
    enter(&mut s);
    let events = pump(&mut s, |e| matches!(e, Event::EntityCreated { .. }));
    // World entry order (Appendix A).
    let i = events
        .iter()
        .position(|e| matches!(e, Event::WorldEntered { .. }))
        .unwrap();
    assert!(
        matches!(&events[i], Event::WorldEntered { player: EntityId(42), name, .. } if name == "Aldith")
    );
    assert!(matches!(
        events[i + 1],
        Event::AppearanceChanged {
            id: EntityId(42),
            ..
        }
    ));
    assert!(matches!(
        events[i + 2],
        Event::LocationReport {
            cause: LocationCause::Entered,
            ..
        }
    ));
    let Some(Event::EntityCreated {
        id,
        name,
        location,
        appearance,
        tag,
        ..
    }) = events.last()
    else {
        unreachable!()
    };
    assert_eq!((*id, name.as_str(), *tag), (EntityId(43), "Bram", None));
    assert_eq!(location.x, 20_260.0);
    assert!(matches!(
        appearance.body,
        tessera_net::Body::Character {
            species: 81,
            sex: 2
        }
    ));
    // Property changes are events too (ARCH §19.1): the player's run speed, and the
    // creation of 43 with a stance other than the default (announced right after its
    // `EntityCreated`).
    assert!(events.iter().any(|e| matches!(e,
        Event::EntityStateChanged { id: EntityId(42), state } if state.run_speed == 8.0)));
    let mut events = events;
    events.extend(pump(&mut s, |e| {
        matches!(
            e,
            Event::EntityStateChanged {
                id: EntityId(43),
                ..
            }
        )
    }));
    assert!(matches!(events.last(),
        Some(Event::EntityStateChanged { id: EntityId(43), state }) if state.stance == Stance::Ready));
    let b = backend(&s);
    assert_eq!(b.entity_state(EntityId(42)).unwrap().run_speed, 8.0);
    assert_eq!(b.entity_state(EntityId(43)).unwrap().stance, Stance::Ready);
    assert_eq!(b.entity_count(), 1);
    let clock = b.game_clock(Instant::now()).unwrap();
    assert!((43_200.0..43_300.0).contains(&clock), "{clock}");

    // The chat stream is not ordered with the world stream (§2.3 rule 6): look at
    // everything seen so far.
    let mut all = events.clone();
    let welcome = |e: &Event| matches!(e, Event::ChatMessage { channel: ChatChannel::System, sender: None, text } if text == "Welcome");
    if !all.iter().any(welcome) {
        all.extend(pump(&mut s, welcome));
    }
    if !all.iter().any(|e| {
        matches!(
            e,
            Event::AppearanceChanged {
                id: EntityId(43),
                ..
            }
        )
    }) {
        all.extend(pump(&mut s, |e| {
            matches!(
                e,
                Event::AppearanceChanged {
                    id: EntityId(43),
                    ..
                }
            )
        }));
    }
    assert!(all.iter().any(|e| matches!(e, Event::AppearanceChanged { id: EntityId(43), appearance } if matches!(appearance.body, tessera_net::Body::Object(DataId(0x7900_0001))))));
    let say = all
        .iter()
        .position(|e| matches!(e,
            Event::ChatMessage { channel: ChatChannel::Say, sender: Some(n), text } if n == "Bram" && text == "hello"))
        .expect("Say line");
    // Lines on one stream keep their order.
    assert!(say < all.iter().position(welcome).unwrap());
    send(
        &mut s,
        Command::Chat {
            channel: ChatChannel::Say,
            text: "hi Bram".into(),
        },
    );
    // The relayed snapshot (x = 20,260.2) after the creation's own move.
    let events = pump(
        &mut s,
        |e| matches!(e, Event::EntityMoved { id: EntityId(43), transform, .. } if transform.position.x > 20_260.1),
    );
    let Some(Event::EntityMoved { velocity, .. }) = events.last() else {
        unreachable!()
    };
    assert_eq!(velocity.x, 2.0);

    // Run east at 6 m/s.
    let (start, _) = s.player_location().unwrap();
    send(
        &mut s,
        Command::Move {
            location: Location {
                x: start.x + 0.1,
                ..start
            },
            heading: 0.0,
            velocity: Vec3::new(6.0, 0.0, 0.0),
            on_ground: true,
        },
    );
    let _ = pump_for(&mut s, Duration::from_millis(150));
    send(
        &mut s,
        Command::Teleport {
            destination: Destination::Location {
                location: Location::new(RegionId(1), 1_000.0, 1_000.0, 10.0),
                heading: 1.0,
            },
        },
    );
    let events = pump(&mut s, |e| {
        matches!(
            e,
            Event::LocationReport {
                cause: LocationCause::Teleported,
                ..
            }
        )
    });
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::EntityDespawned { id: EntityId(43) }))
    );
    let events = pump(&mut s, |e| {
        matches!(
            e,
            Event::EntityMoved {
                id: EntityId(42),
                ..
            }
        )
    });
    assert!(matches!(
        events.last(),
        Some(Event::EntityMoved { velocity, .. }) if *velocity == Vec3::ZERO
    ));
    assert_eq!(s.player_location().unwrap().0.x, 1_000.0);
    assert_eq!(backend(&s).entity_count(), 0);
    // A move sent before the client saw the report is never sent with the new epoch:
    // keep sending moves at the new place.
    send(
        &mut s,
        Command::Move {
            location: Location::new(RegionId(1), 1_000.0, 1_000.0, 10.0),
            heading: 1.0,
            velocity: Vec3::ZERO,
            on_ground: true,
        },
    );
    send(&mut s, Command::QueryLocation);
    pump(&mut s, |e| {
        matches!(
            e,
            Event::LocationReport {
                cause: LocationCause::Requested,
                ..
            }
        )
    });
    backend(&s)
        .handle()
        .unwrap()
        .emote(0x1000_0123, false)
        .unwrap();
    send(
        &mut s,
        Command::Stance {
            stance: Stance::Ready,
        },
    );
    let _ = pump_for(&mut s, Duration::from_millis(100));
    send(&mut s, Command::Disconnect);
    let events = pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    assert!(matches!(
        events.last(),
        Some(Event::Disconnected {
            reason: DisconnectReason::ClientRequested
        })
    ));

    let seen = peer.join();
    assert_eq!(seen.chat.as_deref(), Some("hi Bram"));
    assert_eq!(seen.stance, Some(Stance::Ready));
    assert_eq!(seen.first_move_epoch, 1);
    assert_eq!(seen.first_move_gait, Some(Gait::Running));
    let t = seen.teleport_move.unwrap();
    assert_eq!((t.x, t.y, t.z), (1_000.0, 1_000.0, 10.0));
    assert_eq!(seen.emote, Some(0x1000_0123));
}

/// Two static objects per block, region-qualified.
#[derive(Debug, Default)]
struct Statics {
    asked: usize,
}

impl WorldSource for Statics {
    fn static_entities(
        &mut self,
        block: Landblock,
        limit: usize,
        out: &mut Vec<StaticEntity>,
    ) -> Lookup<()> {
        self.asked += 1;
        for i in 0..2.min(limit) {
            let location = block.location([10.0 + i as f64, 10.0, 0.0]);
            out.push(StaticEntity::new(
                DataId(0x0100_0000 + i as u32),
                Placement::new(location, Rotation::IDENTITY, 1.0),
                u64::from(block.x) << 8 | u64::from(block.y),
            ));
        }
        Lookup::Ready(())
    }
}

#[test]
fn statics_stream_from_the_clients_own_data() {
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        let Message::TeleportRequest(t) = p.expect(MessageType::TeleportRequest).await else {
            unreachable!()
        };
        // A region change: InterestReset first (§9.5).
        p.world(Message::InterestReset).await;
        p.world(Message::LocationReport(LocationReport {
            cause: Cause::Teleported,
            request_id: t.request_id,
            epoch: 2,
            server_time: p.now(),
            pose: Pose {
                location: WLoc::new(2, 500.0, 500.0, 0.0),
                heading: 0.0,
                anchor: None,
            },
        }))
        .await;
        p.linger().await;
    });
    let backend = RemoteBackend::with_world(peer.config(), Statics::default()).unwrap();
    let mut s = Session::new(backend);
    send(&mut s, Command::Connect);
    enter(&mut s);
    // 5 × 5 blocks around the player, two objects each.
    let mut spawned = 0;
    pump(&mut s, |e| {
        if matches!(e, Event::StaticSpawned { .. }) {
            spawned += 1;
        }
        spawned == 50
    });
    let b = s.backend::<RemoteBackend>().unwrap();
    assert_eq!(b.static_stats().resident, 50);
    assert!(b.world_source().is_some());
    send(
        &mut s,
        Command::Teleport {
            destination: Destination::Location {
                location: Location::new(RegionId(2), 500.0, 500.0, 0.0),
                heading: 0.0,
            },
        },
    );
    let events = pump(&mut s, |e| matches!(e, Event::StaticsCleared));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::LocationReport {
            cause: LocationCause::Teleported,
            ..
        }
    )));
    // New statics around the new place, all in region 2.
    let events = pump(&mut s, |e| matches!(e, Event::StaticSpawned { .. }));
    let Some(Event::StaticSpawned { placement, .. }) = events.last() else {
        unreachable!()
    };
    assert_eq!(placement.location.region, RegionId(2));
    drop(s);
    peer.join();
}

#[test]
fn remote_entities_interpolate_at_render_time() {
    const SPEED: f64 = 2.0;
    const X0: f64 = 20_260.0;
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        // Let the client settle its clock first (4 requests 250 ms apart, §11.2).
        tokio::time::sleep(Duration::from_millis(300)).await;
        let t0 = p.now();
        p.world(entity_create(43, t0, X0, SPEED as f32)).await;
        // 15 Hz snapshots for 2 s along +X at 2 m/s.
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_micros(66_667)).await;
            let t = p.now();
            let x = X0 + SPEED * (t - t0) as f64 / 1e6;
            p.datagram(&motion(t, 43, x as f32, SPEED as f32));
        }
        p.linger().await;
        t0
    });
    let mut s = connect(peer.config());
    enter(&mut s);
    pump(&mut s, |e| matches!(e, Event::EntityCreated { .. }));
    let t0 = backend(&s)
        .interpolation()
        .get(EntityId(43))
        .and_then(|buf| buf.newest_time())
        .unwrap();
    // Sample for 1.5 s at ~100 Hz, comparing with the true track at render time.
    let mut interpolated = 0;
    let mut worst: f64 = 0.0;
    let end = Instant::now() + Duration::from_millis(1500);
    let mut sink = Vec::new();
    while Instant::now() < end {
        s.poll_into(Instant::now(), &mut sink);
        sink.clear();
        let now = Instant::now();
        let be = backend(&s);
        if let (Some(sample), Some(rt)) = (be.sample(EntityId(43), now), be.render_time(now))
            && sample.kind == SampleKind::Interpolated
            && rt >= t0
        {
            let truth = X0 + SPEED * (rt - t0) as f64 / 1e6;
            worst = worst.max((f64::from(sample.position.x) - truth).abs());
            interpolated += 1;
            assert_eq!(sample.motion.gait, Gait::Walking);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        interpolated > 50,
        "only {interpolated} interpolated samples"
    );
    // Loopback clock sync is good to well under a millisecond; f32 at 20 km is ~2 mm.
    assert!(worst < 0.05, "worst error {worst} m");
    let be = backend(&s);
    assert!(be.stats().clock_offset_us.is_some());
    assert!(be.stats().time_samples >= 1);
    assert!(be.interpolation_delay() >= Duration::from_millis(150));
    // `MotionBatch` timing (`test-server.md` §15.3). Only structural facts are checked
    // here: arrival times depend on scheduling on a loaded machine (a starved network task
    // drains a burst), so no interval bound holds; the driver's unit tests cover the exact
    // counts. Interpolated samples need two snapshots.
    let st = be.stats();
    assert!(st.motion_snapshots >= 2, "{}", st.motion_snapshots);
    assert!(st.motion_batches >= st.motion_snapshots);
    let last = st.motion_interval_last.unwrap();
    let max = st.motion_interval_max.unwrap();
    assert!(max >= last);
    assert!(u128::from(st.motion_interval_sum_us) >= max.as_micros());
    assert!(st.motion_gaps < 30 * 30, "{}", st.motion_gaps);
    drop(s);
    assert_eq!(peer.join(), t0);
}
