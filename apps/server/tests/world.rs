// SPDX-License-Identifier: GPL-3.0-or-later
//! World behaviour over loopback: creates, movement relay and correction, interest by
//! landblock, chat, properties, teleports and persistence (`tessera-protocol.md` §7–§10,
//! `test-server.md` §8, §9, §12).

mod support;

use std::time::Duration;

use support::*;
use tessera_protocol::codes::{AppErrorCode, ChatRejectedCode, TeleportRefusedCode};
use tessera_protocol::message::*;
use tessera_protocol::types::{Destination, Gait, Location, Motion, Vec3};
use tessera_protocol::value::{Stance, pid};
use tessera_server::Content;

/// A content file without wanderers, with one extra spawn far away.
fn plain_content() -> Content {
    Content::parse(
        "format = 1\n[world]\nid = \"plain\"\ntitle = \"Plain test world\"\nmotd = \"\"\ndefault_spawn = \"start\"\n\
         [[spawn]]\nname = \"start\"\nregion = 1\nblock = [126, 140]\noffset = [80.0, 20.0, 440.0]\n\
         [[spawn]]\nname = \"far\"\nregion = 1\nblock = [140, 140]\noffset = [80.0, 80.0, 440.0]\n\
         [[spawn]]\nname = \"other-region\"\nregion = 2\nlocation = [100.0, 100.0, 0.0]\n",
    )
    .unwrap()
}

fn fast_config() -> tessera_server::Config {
    let mut c = TestServer::config();
    c.movement.run_speed = 100.0;
    c.movement.tolerance = 3.0;
    c
}

#[tokio::test(flavor = "current_thread")]
async fn two_players_see_each_other_move_and_chat() {
    let s = TestServer::start(TestServer::config(), plain_content(), &["alice", "bob"]);
    let (mut a, wa) = s.player("alice", "Aldith").await;
    let (mut b, wb) = s.player("bob", "Bram").await;
    // Each sees the other created as a Player with its appearance.
    let cb = a
        .created(EntityKind::Player)
        .await
        .expect("alice sees bram");
    assert_eq!(cb.id, wb.player);
    assert_eq!(cb.name, "Bram");
    let ca = b
        .created(EntityKind::Player)
        .await
        .expect("bram sees aldith");
    assert_eq!(ca.id, wa.player);

    // Bram walks 1 m east; Aldith gets it in a MotionBatch.
    let mut l = wb.pose.location;
    l.x += 1.0;
    b.move_to(
        l,
        Vec3::new(2.0, 0.0, 0.0),
        Motion::new(Gait::Walking, false),
    );
    let entry = a
        .wait_for(WAIT, |r| match r {
            Received::Datagram(Message::MotionBatch(m)) => m
                .entries
                .iter()
                .find(|e| e.id == wb.player && (f64::from(e.position.x) - l.x).abs() < 0.01)
                .cloned(),
            _ => None,
        })
        .await
        .expect("motion relayed");
    assert_eq!(entry.motion.gait, Gait::Walking);
    assert!(entry.velocity.x > 1.9);

    // Say reaches Aldith (within 40 m) and echoes to Bram.
    b.chat(ChatChannel::Say, "hello there").await;
    let line = a
        .chat_msg(|m| match m {
            Message::ChatMessage(c) if c.text == "hello there" => Some(c.clone()),
            _ => None,
        })
        .await
        .expect("say delivered");
    assert_eq!(line.sender, "Bram");
    assert_eq!(line.sender_entity, wb.player);
    assert!(!line.echo);
    let echo = b
        .chat_msg(|m| match m {
            Message::ChatMessage(c) if c.text == "hello there" => Some(c.clone()),
            _ => None,
        })
        .await
        .expect("echo");
    assert!(echo.echo);

    // Tell by name key, with the echo naming the recipient.
    a.chat(
        ChatChannel::Tell {
            peer: "bram".into(),
        },
        "psst",
    )
    .await;
    let tell = b
        .chat_msg(|m| match m {
            Message::ChatMessage(c) if c.text == "psst" => Some(c.clone()),
            _ => None,
        })
        .await
        .expect("tell delivered");
    assert_eq!(
        tell.channel,
        ChatChannel::Tell {
            peer: "Aldith".into()
        }
    );
    let echo = a
        .chat_msg(|m| match m {
            Message::ChatMessage(c) if c.text == "psst" => Some(c.clone()),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(
        echo.channel,
        ChatChannel::Tell {
            peer: "Bram".into()
        }
    );
    a.chat(
        ChatChannel::Tell {
            peer: "Nobody".into(),
        },
        "hi",
    )
    .await;
    let rej = a
        .chat_msg(|m| match m {
            Message::ChatRejected(f) => Some(f.code),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(rej, ChatRejectedCode::UnknownRecipient);

    // Stance and emote replicate as properties; the emote stops when moving.
    b.send(&Message::StanceSet(Stance::Ready)).await;
    let u = a
        .world_msg(|m| match m {
            Message::EntityUpdate(u) if u.id == wb.player => Some(u.clone()),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(u.changes[0].id, pid::STANCE);
    b.send(&Message::EmoteSet(EmoteSet {
        anim: 77,
        looping: true,
    }))
    .await;
    let u = a
        .world_msg(|m| match m {
            Message::EntityUpdate(u) if u.id == wb.player && u.changes[0].id == pid::EMOTE => {
                Some(u.clone())
            }
            _ => None,
        })
        .await
        .unwrap();
    assert!(matches!(
        u.changes[0].value,
        tessera_protocol::value::Value::Struct(_)
    ));

    // Appearance change is answered to the owner and relayed.
    b.send(&Message::AppearanceSet(appearance(99))).await;
    let mine = b
        .world_msg(|m| match m {
            Message::EntityAppearance(x) if x.id == wb.player => Some(x.clone()),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(mine.appearance, appearance(99));
    let seen = a
        .world_msg(|m| match m {
            Message::EntityAppearance(x) if x.id == wb.player => Some(x.clone()),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(seen.appearance, appearance(99));

    // Leaving the world destroys the entity for observers with REMOVED.
    b.send(&Message::Disconnect(Disconnect {
        code: AppErrorCode::NoError,
        text: String::new(),
    }))
    .await;
    let d = a
        .world_msg(|m| match m {
            Message::EntityDestroy(d) if d.id == wb.player => Some(d.reason),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(d, DestroyReason::Removed);
}

#[tokio::test(flavor = "current_thread")]
async fn movement_correction() {
    let s = TestServer::start(TestServer::config(), plain_content(), &["alice"]);
    let (mut a, w) = s.player("alice", "Aldith").await;
    let mut far = w.pose.location;
    far.x += 100.0;
    a.move_to(
        far,
        Vec3::new(6.0, 0.0, 0.0),
        Motion::new(Gait::Running, false),
    );
    let r = a
        .world_msg(|m| match m {
            Message::LocationReport(r) => Some(r.clone()),
            _ => None,
        })
        .await
        .expect("corrected");
    assert_eq!(r.cause, LocationCause::Corrected);
    assert_eq!(r.epoch, 2);
    assert_eq!(r.request_id, 0);
    assert_eq!(r.pose.location, w.pose.location);
    // Stale-epoch input is ignored; a valid move in the new epoch is accepted.
    a.epoch = 2;
    let mut near = w.pose.location;
    near.x += 2.0;
    a.move_to(
        near,
        Vec3::new(2.0, 0.0, 0.0),
        Motion::new(Gait::Walking, false),
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    a.send(&Message::QueryLocation { request_id: 5 }).await;
    let r = a
        .world_msg(|m| match m {
            Message::LocationReport(r) if r.cause == LocationCause::Requested => Some(r.clone()),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(r.request_id, 5);
    assert_eq!(r.epoch, 2);
    assert!((r.pose.location.x - near.x).abs() < 1e-9);
    let desc = a
        .chat_msg(|m| match m {
            Message::ChatMessage(c) if c.channel == ChatChannel::System => Some(c.text.clone()),
            _ => None,
        })
        .await
        .unwrap();
    assert!(
        desc.contains("landblock (126, 140)") && desc.contains("near spawn \"start\""),
        "{desc}"
    );
    assert!(s.handle().stats().corrections >= 1);
}

#[tokio::test(flavor = "current_thread")]
async fn interest_enter_and_leave_across_landblocks() {
    let mut cfg = fast_config();
    cfg.world.interest_radius = 0;
    let s = TestServer::start(cfg, plain_content(), &["alice", "bob"]);
    let (mut a, _) = s.player("alice", "Aldith").await;
    let (mut b, wb) = s.player("bob", "Bram").await;
    a.created(EntityKind::Player)
        .await
        .expect("same block: created");
    let start = wb.pose.location;
    // One block south (distance 1 = R + 1): stays (hysteresis).
    let step = |dy: f64| Location::new(1, start.x, start.y + dy, start.z);
    b.move_to(
        step(-40.0),
        Vec3::new(0.0, -40.0, 0.0),
        Motion::new(Gait::Running, false),
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    let destroyed_early = a
        .drain(Duration::from_millis(300))
        .await
        .iter()
        .any(|r| matches!(r, Received::World(Message::EntityDestroy(d)) if d.id == wb.player));
    assert!(!destroyed_early, "R + 1 must keep the entity");
    // Two blocks south: leaves.
    b.move_to(
        step(-200.0),
        Vec3::new(0.0, -100.0, 0.0),
        Motion::new(Gait::Running, false),
    );
    let d = a
        .world_msg(|m| match m {
            Message::EntityDestroy(d) if d.id == wb.player => Some(d.reason),
            _ => None,
        })
        .await
        .expect("left interest");
    assert_eq!(d, DestroyReason::LeftInterest);
    // Back into the same block: created again with the same ID.
    b.move_to(
        step(-100.0),
        Vec3::new(0.0, 100.0, 0.0),
        Motion::new(Gait::Running, false),
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    b.move_to(
        step(0.0),
        Vec3::new(0.0, 100.0, 0.0),
        Motion::new(Gait::Running, false),
    );
    let again = a.created(EntityKind::Player).await.expect("re-entered");
    assert_eq!(again.id, wb.player);
}

#[tokio::test(flavor = "current_thread")]
async fn teleports_reset_interest() {
    let s = TestServer::start(TestServer::config(), plain_content(), &["alice", "bob"]);
    let (mut a, _) = s.player("alice", "Aldith").await;
    let (mut b, wb) = s.player("bob", "Bram").await;
    a.created(EntityKind::Player).await.unwrap();
    b.created(EntityKind::Player).await.unwrap();
    // A far teleport: InterestReset, then the report.
    b.send(&Message::TeleportRequest(TeleportRequest {
        request_id: 1,
        destination: Destination::Named {
            name: " FAR ".into(),
        },
    }))
    .await;
    let first = b
        .world_msg(|m| match m {
            Message::InterestReset | Message::LocationReport(_) => Some(m.clone()),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(first, Message::InterestReset);
    let r = b
        .world_msg(|m| match m {
            Message::LocationReport(r) => Some(r.clone()),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(r.cause, LocationCause::Teleported);
    assert_eq!(r.request_id, 1);
    assert_eq!(r.epoch, wb.epoch + 1);
    let d = a
        .world_msg(|m| match m {
            Message::EntityDestroy(d) if d.id == wb.player => Some(d.reason),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(d, DestroyReason::LeftInterest);
    // Unknown places and players are refused; a region change works.
    b.send(&Message::TeleportRequest(TeleportRequest {
        request_id: 2,
        destination: Destination::Named {
            name: "nowhere".into(),
        },
    }))
    .await;
    let code = b
        .world_msg(|m| match m {
            Message::TeleportRefused(t) if t.request_id == 2 => Some(t.code),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(code, TeleportRefusedCode::UnknownPlace);
    b.send(&Message::TeleportRequest(TeleportRequest {
        request_id: 3,
        destination: Destination::Named {
            name: "@aldith".into(),
        },
    }))
    .await;
    let r = b
        .world_msg(|m| match m {
            Message::LocationReport(r) if r.request_id == 3 => Some(r.clone()),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(r.pose.location.region, 1);
    let again = a
        .created(EntityKind::Player)
        .await
        .expect("back in interest");
    assert_eq!(again.id, wb.player);
}

#[tokio::test(flavor = "current_thread")]
async fn chat_rate_limit_and_flooding() {
    let s = TestServer::start(TestServer::config(), plain_content(), &["alice"]);
    let (mut a, _) = s.player("alice", "Aldith").await;
    for i in 0..6 {
        a.chat(ChatChannel::Say, &format!("line {i}")).await;
    }
    let code = a
        .chat_msg(|m| match m {
            Message::ChatRejected(f) => Some(f.code),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(code, ChatRejectedCode::RateLimited);
    for i in 0..40 {
        a.chat(ChatChannel::Say, &format!("flood {i}")).await;
    }
    assert_eq!(a.disconnect_code(WAIT).await, Some(AppErrorCode::Flooding));
}

#[tokio::test(flavor = "current_thread")]
async fn wanderers_follow_the_first_player() {
    let s = TestServer::start(TestServer::config(), Content::default_world(), &["alice"]);
    let (mut a, w) = s.player("alice", "Aldith").await;
    let mut seen = Vec::new();
    while seen.len() < 3 {
        let c = a
            .created(EntityKind::NonPlayer)
            .await
            .expect("wanderer created");
        assert_eq!(c.appearance, appearance(23));
        assert_eq!(c.motion.gait, Gait::Walking);
        seen.push(c.name.clone());
    }
    seen.sort();
    assert_eq!(seen, ["Wanderer 1", "Wanderer 2", "Wanderer 3"]);
    assert_eq!(s.handle().stats().entities, 4);
    let _ = w;
}

#[tokio::test(flavor = "current_thread")]
async fn positions_persist_across_restart() {
    let mut s = TestServer::start(TestServer::config(), plain_content(), &["alice"]);
    let (mut a, w) = s.player("alice", "Aldith").await;
    let mut l = w.pose.location;
    l.x += 3.0;
    l.y -= 1.0;
    a.move_to(l, Vec3::ZERO, Motion::STANDING);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let report = s.shutdown();
    assert_eq!(report.saved, 1);
    assert_eq!(report.failures, 0);
    assert_eq!(
        a.disconnect_code(WAIT).await,
        Some(AppErrorCode::ServerShutdown)
    );
    s.restart(TestServer::config(), plain_content());
    let mut c = s.client().await;
    c.hello().await;
    let ok = c.login("alice", PASSWORD).await.unwrap();
    assert_eq!(ok.characters.len(), 1);
    let w2 = c.enter("Aldith").await.unwrap();
    assert!((w2.pose.location.x - l.x).abs() < 1e-9);
    assert!((w2.pose.location.y - l.y).abs() < 1e-9);
    assert_eq!(w2.instance, 1);
}

/// Finding: session-made chat rejections overtook the simulation's answer to an earlier
/// `ChatSend` (`tessera-protocol.md` §6.4.3).
#[tokio::test(flavor = "current_thread")]
async fn chat_rejections_arrive_in_order() {
    let s = TestServer::start(TestServer::config(), plain_content(), &["alice"]);
    let (mut a, _) = s.player("alice", "Aldith").await;
    // A Tell to an offline name (rejected by the simulation, up to a tick later), then
    // Says until the protocol's bucket is empty (the last rejected by the session at once).
    a.chat(
        ChatChannel::Tell {
            peer: "Nobody".into(),
        },
        "are you there",
    )
    .await;
    for i in 0..5 {
        a.chat(ChatChannel::Say, &format!("line {i}")).await;
    }
    let mut codes = Vec::new();
    for _ in 0..2 {
        codes.push(
            a.chat_msg(|m| match m {
                Message::ChatRejected(f) => Some(f.code),
                _ => None,
            })
            .await
            .unwrap(),
        );
    }
    assert_eq!(
        codes,
        vec![
            ChatRejectedCode::UnknownRecipient,
            ChatRejectedCode::RateLimited
        ]
    );
}

/// Finding: a `ChatSend` sent just before `Disconnect` was dropped with the connection.
#[tokio::test(flavor = "current_thread")]
async fn last_line_before_disconnect_is_delivered() {
    let s = TestServer::start(TestServer::config(), plain_content(), &["alice", "bob"]);
    let (mut b, _) = s.player("bob", "Bram").await;
    for round in 0..3 {
        let mut a = if round == 0 {
            s.player("alice", "Aldith").await.0
        } else {
            let mut c = s.client().await;
            c.hello().await;
            c.login("alice", PASSWORD).await.unwrap();
            c.enter("Aldith").await.unwrap();
            c
        };
        let text = format!("bye {round}");
        a.chat(ChatChannel::Say, &text).await;
        a.send(&Message::Disconnect(Disconnect {
            code: AppErrorCode::NoError,
            text: String::new(),
        }))
        .await;
        let got = b
            .chat_msg(|m| match m {
                Message::ChatMessage(c) if c.text == text => Some(()),
                _ => None,
            })
            .await;
        assert!(got.is_some(), "line {round} lost");
        let _ = a.closed_code(WAIT).await;
    }
}

/// Finding: with a grace of 0 s no `ServerNotice` was sent (§12.1 step 2).
#[tokio::test(flavor = "current_thread")]
async fn shutdown_notice_without_grace() {
    let mut s = TestServer::start(TestServer::config(), plain_content(), &["alice"]);
    let (mut a, _) = s.player("alice", "Aldith").await;
    let report = s.shutdown();
    assert_eq!(report.failures, 0);
    let notice = a
        .chat_msg(|m| match m {
            Message::ServerNotice(n) => Some((n.level, n.seconds)),
            _ => None,
        })
        .await;
    assert_eq!(notice, Some((NoticeLevel::Shutdown, 0)));
    assert_eq!(
        a.disconnect_code(WAIT).await,
        Some(AppErrorCode::ServerShutdown)
    );
}
