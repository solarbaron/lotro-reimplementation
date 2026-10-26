// SPDX-License-Identifier: GPL-3.0-or-later
//! `SessionDriver::next_deadline` and the session loop's wait ([`loop_wait`]) on a virtual
//! clock: an idle session does not spin, and its timers (the connections' retransmits and
//! keep-alives, the entry deadline) fire on time.

use super::*;
use crate::connector::session_phase::ENTER_WORLD_TIMEOUT_SEC;
use crate::connector::worker::{LOOP_WAIT_MAX, LOOP_WAIT_MIN, loop_wait};

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
    d.abandon();
    assert!(d.ended());
    // Abandoned: every connection was closed without a word; nothing is timed.
    assert_eq!(d.next_deadline(1.0), None);
}
