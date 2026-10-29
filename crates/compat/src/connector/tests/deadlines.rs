// SPDX-License-Identifier: GPL-3.0-or-later
//! `SessionDriver::next_deadline` and the session loop's wait ([`loop_wait`]) on a virtual
//! clock: an idle session does not spin, and its timers (the connections' retransmits and
//! keep-alives, the entry deadline) fire on time.

use super::*;
use crate::connector::session_phase::{CHAT_RETRY_SEC, ENTER_WORLD_TIMEOUT_SEC};
use crate::connector::worker::{LOOP_WAIT_MAX, LOOP_WAIT_MIN, loop_wait};

/// Whether a pass at `pass` is on time for a timer due at `due`: not before it and late by
/// at most the shortest wait (a pass that lands a hair before its timer waits 1 ms more).
fn on_time(pass: f64, due: f64) -> bool {
    pass >= due - 1e-9 && pass - due < 1.1e-3
}

/// `RefreshTicket` requests the synthetic service received.
fn refreshes(world: &World) -> usize {
    world
        .requests()
        .iter()
        .filter(|r| r.2.as_deref().is_some_and(|a| a.contains("RefreshTicket")))
        .count()
}

/// The session loop's next pass time after a pass at `t` (`busy`: something happened).
fn next_pass(d: &SessionDriver<FakeNet>, t: f64, busy: bool) -> f64 {
    let deadline = if busy { Some(t) } else { d.next_deadline(t) };
    t + loop_wait(t, deadline).as_secs_f64()
}

#[test]
fn the_session_auth_retransmit_is_the_drivers_deadline() {
    let (world, mut d) = driver_at_login(Service::default(), 0);
    assert_eq!(world.take_sent().len(), 1, "the SessionAuthHeader");
    let next = d.next_deadline(0.0).expect("a timer");
    assert!((next - 2.0).abs() < 1e-9, "{next}");
    d.step(next - 1e-3, true, &go);
    assert!(world.take_sent().is_empty(), "nothing before the deadline");
    d.step(next, true, &go);
    assert_eq!(world.take_sent().len(), 1, "the retransmit at the deadline");
}

#[test]
fn an_idle_login_loop_does_not_spin_and_retransmits_on_time() {
    let (world, mut d) = driver_at_login(Service::default(), 0);
    world.take_sent();
    let mut t = 0.0f64;
    let mut passes = 0usize;
    let mut sent_at = Vec::new();
    while t < 9.0 {
        let busy = d.step(t, true, &go);
        passes += 1;
        if !world.take_sent().is_empty() {
            sent_at.push(t);
        }
        let next = next_pass(&d, t, busy);
        assert!(next - t >= LOOP_WAIT_MIN.as_secs_f64() - 1e-9);
        t = next;
    }
    // Each retransmit 2 s after the one before (`official-session.md` §6.1), late by at
    // most the shortest wait (a pass that lands a hair before its timer waits 1 ms more).
    assert_eq!(sent_at.len(), 4, "{sent_at:?}");
    let mut due = 2.0;
    for sent in &sent_at {
        assert!(
            *sent >= due && *sent - due < 1.1e-3,
            "sent at {sent}, due {due}"
        );
        due = sent + 2.0;
    }
    // 9 s at the longest wait (50 ms), plus at most two passes per timer.
    let cap = (9.0 / LOOP_WAIT_MAX.as_secs_f64()) as usize;
    assert!(passes <= cap + 2 * sent_at.len() + 1, "{passes} passes");
}

#[test]
fn the_entry_deadline_fires_on_time() {
    let (world, mut d, login) = driver_at_characters(Service::default(), 0);
    let enter = Command::EnterWorld {
        character: "Aldo".to_owned(),
    };
    d.command(&enter, 1.0).expect("enter");
    let due = 1.0 + ENTER_WORLD_TIMEOUT_SEC;
    let mut t = 1.0f64;
    let mut ping = 1.0f64;
    let mut failed_at = None;
    while t < due + 1.0 && failed_at.is_none() {
        if t >= ping {
            // The login server stays alive meanwhile.
            world.deliver(
                login.control(OptionalHeaders {
                    cumulative_ack: Some(0),
                    ..OptionalHeaders::default()
                }),
                LOGIN_SERVER,
            );
            ping += 10.0;
        }
        let busy = d.step(t, true, &go);
        world.take_sent();
        if d.take_events()
            .iter()
            .any(|e| matches!(e, Event::EnterWorldFailed { .. }))
        {
            failed_at = Some(t);
        } else {
            let deadline = d.next_deadline(t).expect("timed work");
            assert!(
                deadline <= due.max(t) + 1e-9,
                "the entry deadline is counted"
            );
        }
        t = next_pass(&d, t, busy);
    }
    let failed_at = failed_at.expect("the entry failed");
    assert!(
        failed_at >= due && failed_at - due < 1.1e-3,
        "failed at {failed_at}, due {due}"
    );
}

#[test]
fn an_ended_session_has_no_deadline() {
    let (_world, mut d) = driver_at_login(Service::default(), 0);
    d.abandon(0.0);
    assert!(d.ended());
    // Abandoned: every connection was closed without a word; nothing is timed.
    assert_eq!(d.next_deadline(1.0), None);
}

/// A refresh that `TicketRefresher::poll_refresh_request` would not send (an empty ticket,
/// account name or refresh URL) is no deadline, even long past its time: the loop never
/// stays at its shortest wait for it.
#[test]
fn a_refresh_that_cannot_be_sent_is_no_deadline() {
    let auth = "https://auth.example.invalid/login";
    let sub = "SUB0000000000000000000000";
    let cases = [("", sub, TICKET), (auth, "", TICKET), (auth, sub, "")];
    for (url, account, ticket) in cases {
        let mut start = session_start(60, None, Arc::default());
        start.refresher = TicketRefresher::new(url, account, ticket, 60, 0.0);
        let (world, mut d, login) = driver_at_characters_from(Service::default(), start);
        world.take_sent();
        // Past the first refresh time (30 s of a 60 s ticket).
        let mut t = 31.0f64;
        let mut ping = t;
        let mut shortest = 0usize;
        let mut idle_shortest = false;
        let mut passes = 0usize;
        while t < 41.0 {
            if t >= ping {
                // The login server stays alive meanwhile.
                world.deliver(
                    login.control(OptionalHeaders {
                        cumulative_ack: Some(0),
                        ..OptionalHeaders::default()
                    }),
                    LOGIN_SERVER,
                );
                ping += 5.0;
            }
            let busy = d.step(t, true, &go);
            world.take_sent();
            d.take_events();
            passes += 1;
            let deadline = d.next_deadline(t).expect("the session's timers");
            let wait = loop_wait(t, if busy { Some(t) } else { Some(deadline) });
            if wait == LOOP_WAIT_MIN {
                shortest += 1;
                // A timer a hair ahead of the pass (float rounding) gets one shortest wait
                // and then fires; a deadline nobody acts on would get them one after another.
                assert!(
                    busy || !idle_shortest,
                    "{url:?}/{account:?}: still due at {deadline} after a pass at {t}"
                );
            }
            idle_shortest = !busy && wait == LOOP_WAIT_MIN;
            t += wait.as_secs_f64();
        }
        assert_eq!(refreshes(&world), 0, "nothing was sent");
        // The passes right after a keep-alive and at most one per session timer (an
        // acknowledgement every 2 s): with the refresh counted as a deadline every wait in
        // these 10 s would be the shortest.
        assert!(
            shortest <= 20,
            "{shortest} shortest waits in {passes} passes"
        );
        let cap = (10.0 / LOOP_WAIT_MAX.as_secs_f64()) as usize;
        assert!(passes <= cap + 2 * 10, "{passes} passes");
    }
}

/// The next ticket refresh is the driver's deadline, and once it is running it is not.
#[test]
fn the_ticket_refresh_is_the_drivers_deadline() {
    // A 2 s ticket: the first refresh at 1 s (`official-login.md` §4.3), before the
    // session's first acknowledgement (2 s).
    let (world, mut d, _login) = driver_at_characters(Service::default(), 2);
    world.take_sent();
    let due = d.next_deadline(0.1).expect("timed work");
    assert!((due - 1.0).abs() < 1e-9, "{due}");
    let mut t = 0.1f64;
    while !d.refresh_running() {
        assert!(t < 1.5, "no refresh by {t}");
        let busy = d.step(t, true, &go);
        if d.refresh_running() {
            break;
        }
        t = next_pass(&d, t, busy);
    }
    assert!(on_time(t, 1.0), "the refresh started at {t}");
    // Running: its answer has no deadline here; the acknowledgement at 2 s is next.
    let next = d.next_deadline(t).expect("timed work");
    assert!(next > 1.5, "{next}");
    d.begin_shutdown(t);
}

/// A failed chat-server attachment is tried again after [`CHAT_RETRY_SEC`], and that time
/// is the driver's deadline.
#[test]
fn the_chat_retry_is_the_drivers_deadline() {
    let (world, mut d, mut login) =
        driver_at_characters_with(Service::default(), 0, Some(CHAT_SERVER), Arc::default());
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    world.deliver(login.event(0x90, &session_ready()), LOGIN_SERVER);
    d.step(1.1, true, &go);
    assert!(d.in_world());
    world.take_sent();
    d.take_events();
    // The first attempt is due at once.
    assert!(d.wants_chat(1.2).is_some());
    assert_eq!(d.next_deadline(1.2), Some(1.2));
    d.chat_failed(1.2);
    let due = 1.2 + CHAT_RETRY_SEC;
    let mut t = 1.2f64;
    let mut ping = t;
    let mut passes = 0usize;
    let retried_at = loop {
        assert!(t < due + 1.0, "no retry by {t}");
        if t >= ping {
            world.deliver(
                login.control(OptionalHeaders {
                    cumulative_ack: Some(0),
                    ..OptionalHeaders::default()
                }),
                LOGIN_SERVER,
            );
            ping += 10.0;
        }
        let busy = d.step(t, true, &go);
        world.take_sent();
        d.take_events();
        passes += 1;
        if d.wants_chat(t).is_some() {
            break t;
        }
        let deadline = d.next_deadline(t).expect("timed work");
        assert!(deadline <= due + 1e-9, "the retry is counted: {deadline}");
        t = next_pass(&d, t, busy);
    };
    assert!(
        on_time(retried_at, due),
        "retried at {retried_at}, due {due}"
    );
    let cap = (CHAT_RETRY_SEC / LOOP_WAIT_MAX.as_secs_f64()) as usize;
    assert!(passes <= cap + 40, "{passes} passes");
}
