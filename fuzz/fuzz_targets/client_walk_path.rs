// SPDX-License-Identifier: GPL-3.0-or-later
//! The scripted-walk format of `tessera-client --walk-path` (`apps/client/src/walk_path.rs`,
//! included by path so this crate does not pull in the window and GPU stack;
//! `docs/checklists/m4-5-visual.md` §2.1–§2.2).
//!
//! Input layout: byte 0 selects the form (even: one walk file; odd: a Markdown text holding
//! the walk table), the rest is (lossy) UTF-8 text.
//!
//! Nothing may panic. Every parsed walk keeps the format's bounds (id length, token,
//! waypoint and riser counts, finite numbers within `MAX_COORDINATE`, a duration within
//! `MAX_DURATION`, no `stop`/`turn` with waypoints), its file form parses back to the same
//! walk (also at the field limits: every field fits when written back), and its per-step script
//! yields finite commands for the whole duration (sampled) at hostile feet positions.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/client/src/walk_path.rs"]
mod walk_path;

use libfuzzer_sys::fuzz_target;
use walk_path::{
    Action, Expect, MAX_COORDINATE, MAX_DURATION, MAX_ID_LEN, MAX_INPUT_TOKENS, MAX_RISERS,
    MAX_WAYPOINTS, Walk, WalkScript, parse_walk_file, parse_walk_table,
};

fn finite(v: &[f64]) -> bool {
    v.iter().all(|x| x.is_finite() && x.abs() <= MAX_COORDINATE)
}

fn check(w: &Walk) {
    assert!(!w.id.is_empty() && w.id.chars().count() <= MAX_ID_LEN);
    assert!(w.region > 0);
    assert!(finite(&w.start) && w.heading.is_finite());
    assert!(w.duration.is_finite() && (0.0..=MAX_DURATION).contains(&w.duration));
    assert!(w.inputs.timed.len() + w.inputs.waypoints.len() <= MAX_INPUT_TOKENS);
    assert!(w.inputs.waypoints.len() <= MAX_WAYPOINTS);
    assert!(w.inputs.timed.windows(2).all(|p| p[0].t <= p[1].t));
    assert!(w.inputs.waypoints.iter().all(|p| finite(p)));
    // Waypoints steer every step: `stop` and `turn` are refused with them.
    assert!(
        w.inputs.waypoints.is_empty()
            || w.inputs
                .timed
                .iter()
                .all(|t| !matches!(t.action, Action::Stop | Action::Turn(_)))
    );
    match &w.expect {
        Expect::Reach { e, d, risers } => {
            assert!(finite(e) && *d >= 0.0 && risers.len() <= MAX_RISERS);
            assert!(!risers.is_empty() && risers.iter().all(|r| *r >= 0.0));
        }
        Expect::Blocked { p, d } => assert!(finite(p) && *d >= 0.0),
        Expect::NoClimb { b, zmax, rmax } => assert!(finite(b) && zmax.is_finite() && *rmax >= 0.0),
        Expect::Cells { e, d, .. } | Expect::Pass { e, d } => assert!(finite(e) && *d >= 0.0),
        Expect::Follow { e, d, dz, headroom } => {
            assert!(finite(e) && *d >= 0.0 && *dz >= 0.0 && *headroom >= 0.0);
        }
        Expect::WadeSwim {
            entry,
            swim,
            h,
            fh,
            d,
        } => assert!(finite(entry) && finite(swim) && h.is_finite() && *fh >= 0.0 && *d >= 0.0),
    }
    let text = w.to_file_text();
    let again = parse_walk_file(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(&again, w, "{text}");

    let mut script = WalkScript::new(w);
    let feet = [
        [0.0, 0.0],
        [f64::NAN, 1.0],
        [1.0e300, -1.0e300],
        w.start.first().map_or([0.0, 0.0], |x| [*x, w.start[1]]),
    ];
    let steps = 64u32;
    for k in 0..=steps {
        let t = w.duration * f64::from(k) / f64::from(steps);
        let c = script.command(t, feet[k as usize % feet.len()]);
        assert!(c.forward.is_finite() && (-1.0..=1.0).contains(&c.forward));
        assert!(c.strafe.is_finite() && (-1.0..=1.0).contains(&c.strafe));
        assert!(c.heading.is_none_or(f64::is_finite));
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let text = String::from_utf8_lossy(rest);
    if mode % 2 == 0 {
        if let Ok(w) = parse_walk_file(&text) {
            check(&w);
        }
    } else if let Ok(walks) = parse_walk_table(&text) {
        for w in &walks {
            check(w);
        }
    }
});
