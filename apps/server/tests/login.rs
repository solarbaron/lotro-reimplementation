// SPDX-License-Identifier: GPL-3.0-or-later
//! Handshake and login (`tessera-protocol.md` §4, `test-server.md` §6.2, §6.3).

mod support;

use std::time::Duration;

use support::*;
use tessera_protocol::codes::{AppErrorCode, LoginFailedCode};
use tessera_protocol::message::{Credentials, Message, ServerCaps};
use tessera_server::Content;

#[tokio::test(flavor = "current_thread")]
async fn hello_and_login_success() {
    let s = TestServer::start(TestServer::config(), Content::default_world(), &["alice"]);
    let mut c = s.client().await;
    let ack = c.hello().await;
    assert_eq!(ack.tick_rate, 30);
    assert_eq!(ack.snapshot_rate, 15);
    assert_eq!(ack.interest_radius, 2);
    assert!(ack.server_caps.contains(ServerCaps::CHARACTER_CREATION));
    assert!(!ack.server_caps.contains(ServerCaps::REGISTRATION));
    assert!(ack.motd.contains("Tessera"));
    // The account name is canonicalised.
    let ok = c.login("ALICE", PASSWORD).await.expect("login");
    assert_eq!(ok.account, "alice");
    assert!(ok.characters.is_empty());
    let created = c.create("Aldith", appearance(23)).await.expect("create");
    assert_eq!(created.characters.len(), 1);
    assert_eq!(created.characters[0].name, "Aldith");
    let w = c.enter("aldith").await.expect("enter by name key");
    assert_eq!(w.name, "Aldith");
    assert_eq!(w.instance, 1);
    assert_eq!(w.epoch, 1);
    assert_eq!(w.pose.location.region, 1);
    // WorldClock and the player's own speeds follow WorldEnter.
    let clock = c
        .world_msg(|m| match m {
            Message::WorldClock(w) => Some(*w),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(clock.rate, 1.0);
    let speeds = c
        .world_msg(|m| match m {
            Message::EntityUpdate(u) if u.id == w.player => Some(u.clone()),
            _ => None,
        })
        .await
        .unwrap();
    assert_eq!(speeds.changes.len(), 2);
    assert_eq!(s.wait_stats(|st| st.in_world, 1).await, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn login_failures_and_auth_failed() {
    let s = TestServer::start(TestServer::config(), Content::default_world(), &["alice"]);
    let mut c = s.client().await;
    c.hello().await;
    let e = c.login("alice", "wrong password").await.unwrap_err();
    assert_eq!(e.code, LoginFailedCode::BadCredentials);
    // Unknown accounts fail the same way.
    let e = c.login("nobody", PASSWORD).await.unwrap_err();
    assert_eq!(e.code, LoginFailedCode::BadCredentials);
    // An invalid account name is refused without a hash.
    c.send(&Message::Login(Credentials {
        account: "no spaces".into(),
        password: "x".into(),
    }))
    .await;
    match c.control().await {
        Some(Message::LoginFailed(f)) => assert_eq!(f.code, LoginFailedCode::InvalidAccountName),
        other => panic!("{other:?}"),
    }
    // The third failure ends the connection.
    assert_eq!(
        c.disconnect_code(WAIT).await,
        Some(AppErrorCode::AuthFailed)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn disabled_account_and_lockout() {
    let mut cfg = TestServer::config();
    cfg.accounts.lockout_after = 2;
    let s = TestServer::start(cfg, Content::default_world(), &["alice", "bob"]);
    tessera_server::accounts::modify_accounts(&s.data_dir(), |f| {
        f.accounts.get_mut("bob").unwrap().disabled = true;
        Ok(())
    })
    .unwrap();
    let mut c = s.client().await;
    c.hello().await;
    // Wrong password never reveals that the account is disabled.
    let e = c.login("bob", "wrong password").await.unwrap_err();
    assert_eq!(e.code, LoginFailedCode::BadCredentials);
    let e = c.login("bob", PASSWORD).await.unwrap_err();
    assert_eq!(e.code, LoginFailedCode::AccountDisabled);

    // Two failures lock alice for 30 s.
    let mut c = s.client().await;
    c.hello().await;
    assert_eq!(
        c.login("alice", "bad1 password").await.unwrap_err().code,
        LoginFailedCode::BadCredentials
    );
    assert_eq!(
        c.login("alice", "bad2 password").await.unwrap_err().code,
        LoginFailedCode::BadCredentials
    );
    let mut c = s.client().await;
    c.hello().await;
    let e = c.login("alice", PASSWORD).await.unwrap_err();
    assert_eq!(e.code, LoginFailedCode::RateLimited);
    assert!(
        e.retry_after >= 1 && e.retry_after <= 30,
        "{}",
        e.retry_after
    );
}

#[tokio::test(flavor = "current_thread")]
async fn address_budget() {
    let mut cfg = TestServer::config();
    cfg.accounts.attempts_per_address_per_min = 2;
    let s = TestServer::start(cfg, Content::default_world(), &["alice"]);
    let mut c = s.client().await;
    c.hello().await;
    assert!(c.login("alice", "bad password 1").await.is_err());
    assert!(c.login("alice", "bad password 2").await.is_err());
    let mut c = s.client().await;
    c.hello().await;
    let e = c.login("alice", PASSWORD).await.unwrap_err();
    assert_eq!(e.code, LoginFailedCode::RateLimited);
    assert!(e.retry_after >= 1);
}

#[tokio::test(flavor = "current_thread")]
async fn duplicate_login_replaces_or_refuses() {
    let s = TestServer::start(TestServer::config(), Content::default_world(), &["alice"]);
    let (mut first, _) = s.player("alice", "Aldith").await;
    let mut second = s.client().await;
    second.hello().await;
    let ok = second.login("alice", PASSWORD).await.expect("replaces");
    assert_eq!(ok.characters.len(), 1);
    assert_eq!(
        first.disconnect_code(WAIT).await,
        Some(AppErrorCode::LoggedInElsewhere)
    );
    // The character left the world and can be entered again.
    second.enter("Aldith").await.expect("enter after replace");
    drop(s);

    let mut cfg = TestServer::config();
    cfg.server.duplicate_login = tessera_server::config::DuplicateLogin::Refuse;
    let s = TestServer::start(cfg, Content::default_world(), &["alice"]);
    let mut first = s.client().await;
    first.hello().await;
    first.login("alice", PASSWORD).await.unwrap();
    let mut second = s.client().await;
    second.hello().await;
    let e = second.login("alice", PASSWORD).await.unwrap_err();
    assert_eq!(e.code, LoginFailedCode::AlreadyOnline);
}

#[tokio::test(flavor = "current_thread")]
async fn registration_when_enabled() {
    let mut cfg = TestServer::config();
    cfg.server.registration = true;
    let s = TestServer::start(cfg, Content::default_world(), &["alice"]);
    let mut c = s.client().await;
    let ack = c.hello().await;
    assert!(ack.server_caps.contains(ServerCaps::REGISTRATION));
    let reg = |a: &str, p: &str| {
        Message::Register(Credentials {
            account: a.into(),
            password: p.into(),
        })
    };
    c.send(&reg("alice", "long enough password")).await;
    match c.control().await {
        Some(Message::LoginFailed(f)) => assert_eq!(f.code, LoginFailedCode::AccountExists),
        other => panic!("{other:?}"),
    }
    c.send(&reg("newbie", "short")).await;
    match c.control().await {
        Some(Message::LoginFailed(f)) => assert_eq!(f.code, LoginFailedCode::PasswordRejected),
        other => panic!("{other:?}"),
    }
    let mut c = s.client().await;
    c.hello().await;
    c.send(&reg("Newbie", "long enough password")).await;
    match c.control().await {
        Some(Message::LoginOk(ok)) => assert_eq!(ok.account, "newbie"),
        other => panic!("{other:?}"),
    }
    // The account is on disk and works for a normal login.
    let file = tessera_server::accounts::read_accounts(&s.data_dir()).unwrap();
    assert!(file.accounts.contains_key("newbie"));
    let mut c2 = s.client().await;
    c2.hello().await;
    assert!(c2.login("newbie", "long enough password").await.is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn hello_timeout_closes_silent_clients() {
    let s = TestServer::start(TestServer::config(), Content::default_world(), &[]);
    let mut c = s.client().await;
    // Nothing sent: the server closes after 10 s (AUTH_TIMEOUT).
    let code = c.closed_code(Duration::from_secs(15)).await;
    assert_eq!(code, Some(u64::from(AppErrorCode::AuthTimeout.to_u16())));
}
