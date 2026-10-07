// SPDX-License-Identifier: GPL-3.0-or-later
//! The scripted-walk format of `tessera-client --walk-path` (`apps/client/src/walk_path.rs`,
//! included by path so this crate does not pull in the window and GPU stack;
//! `docs/checklists/m4-5-visual.md` §2.1–§2.2).
//!
//! Input layout: byte 0 selects the form, `% 4`. 0: one walk file and 1: a Markdown text
//! holding the walk table, both the rest of the input as (lossy) UTF-8 text. 2: one walk
//! file and 3: a walk table of up to eight rows, both written by the harness from fields
//! the rest of the input picks (every category, gait, input token and expect kind; numbers
//! in plain, short, exponent and boundary forms; keys in any order), with a value out of
//! range, a missing, duplicate or unknown key or a junk token now and then. Most inputs of
//! the last two forms are whole walks, so they reach the checks below instead of stopping
//! at the first missing key.
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

use std::fmt::Write as _;

use libfuzzer_sys::fuzz_target;
use walk_path::{
    Action, Expect, FIELD_NAMES, MAX_COORDINATE, MAX_DURATION, MAX_ID_LEN, MAX_INPUT_TOKENS,
    MAX_RISERS, MAX_WAYPOINTS, WALK_TABLE_HEADER, Walk, WalkScript, parse_walk_file,
    parse_walk_table,
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

/// Bytes of the input read in order; zeros once they run out.
struct Src<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Src<'_> {
    fn byte(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos = self.pos.saturating_add(1);
        b
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.byte(), self.byte()])
    }

    /// A value in `0..n` (`n` > 0).
    fn below(&mut self, n: u8) -> u8 {
        self.byte() % n
    }

    /// True about once in `n` (`n` > 0).
    fn rarely(&mut self, n: u8) -> bool {
        self.byte() % n == n - 1
    }
}

/// What a generated number should be (it is now and then not).
#[derive(Clone, Copy, PartialEq)]
enum Num {
    /// Any sign, within `MAX_COORDINATE`.
    Any,
    /// Not negative.
    NonNeg,
    /// Within `0..=MAX_DURATION`.
    Time,
}

/// A number text in one of the forms the format accepts, now and then out of range or
/// malformed.
fn number(s: &mut Src, kind: Num) -> String {
    const EDGES: [&str; 10] = [
        "1000000",
        "1e6",
        "1000000.0000001",
        "3600",
        "3600.000001",
        "0",
        "-0",
        "0.1",
        "1e-320",
        "123456789012345678901234567890123456789012345678901234567890.12345",
    ];
    const JUNK: [&str; 10] = [
        "", "nan", "inf", "1e", "--1", "1.2.3", "0x10", "2e6", "7 7", "+5",
    ];
    let style = s.byte();
    let cap = if kind == Num::Time { 3_600 } else { 60_000 };
    let int = u32::from(s.u16()) % cap;
    let frac = s.u16();
    let mut t = match style % 16 {
        0..=4 => int.to_string(),
        5..=9 => format!("{int}.{frac}"),
        10 => format!(".{frac}"),
        11 => format!("{int}."),
        12 => format!("{}e{}", frac % 1000, i32::from(s.below(9)) - 5),
        13 => format!("{}e-{}", int % 10, 300 + u32::from(s.below(30))),
        14 => EDGES[usize::from(s.below(10))].to_owned(),
        _ => JUNK[usize::from(s.below(10))].to_owned(),
    };
    // Negative numbers: freely where allowed, rarely elsewhere.
    let neg = match kind {
        Num::Any => style & 0x80 != 0,
        _ => style == 0xF0,
    };
    if neg && !t.starts_with('-') {
        t.insert(0, '-');
    }
    t
}

/// `n` comma-separated numbers (now and then one too many or too few).
fn vector(s: &mut Src, n: usize) -> String {
    let mut parts: Vec<String> = (0..n).map(|_| number(s, Num::Any)).collect();
    if s.rarely(64) {
        if s.byte() & 1 == 0 {
            parts.pop();
        } else {
            parts.push("1".to_owned());
        }
    }
    parts.join(",")
}

/// A walk id: mostly letters and digits, ending in the row number so rows differ.
fn walk_id(s: &mut Src, row: usize) -> String {
    const ALNUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let len = usize::from(s.below(8)) + 1;
    let mut id: String = (0..len)
        .map(|_| char::from(ALNUM[usize::from(s.byte()) % ALNUM.len()]))
        .collect();
    id.push_str(&row.to_string());
    match s.byte() {
        0xFF => id.clear(),
        0xFE => id.push('_'),
        0xFD => id.push('\u{e9}'), // a letter, not ASCII
        0xFC => id = "x".repeat(MAX_ID_LEN + 1),
        0xFB => id = "y".repeat(MAX_ID_LEN),
        _ => {}
    }
    id
}

/// The `inputs` field: timed tokens and waypoints.
fn inputs(s: &mut Src) -> String {
    // 0: timed tokens only; 1: `fwd`/`jump` and waypoints; 2: anything (`stop` or `turn`
    // with waypoints is refused).
    let flavour = s.below(3);
    let n = if s.rarely(32) {
        250 + usize::from(s.below(16))
    } else {
        usize::from(s.below(8))
    };
    let mut tokens = Vec::new();
    let mut wp = false;
    for _ in 0..n {
        let kind = s.below(8);
        let tok = match (flavour, kind) {
            (1 | 2, 6 | 7) => {
                let p = vector(s, 2);
                if wp && s.byte() & 1 == 0 {
                    p
                } else {
                    wp = true;
                    format!("wp={p}")
                }
            }
            (_, k) => {
                let t = number(s, Num::Time);
                let a = match k {
                    2 => "jump".to_owned(),
                    3 if flavour != 1 => "stop".to_owned(),
                    4 if flavour != 1 => format!("turn={}", number(s, Num::Any)),
                    5 if s.rarely(16) => "hop".to_owned(),
                    _ => "fwd".to_owned(),
                };
                format!("{t}:{a}")
            }
        };
        tokens.push(tok);
    }
    let sep = if s.rarely(4) { " ; " } else { ";" };
    tokens.join(sep)
}

/// The `expect` field: one of the seven kinds with its keys in an order the input picks.
fn expect(s: &mut Src) -> String {
    const KINDS: [&str; 7] = [
        "blocked",
        "reach",
        "noclimb",
        "cells",
        "follow",
        "wade_swim",
        "pass",
    ];
    let kind = s.below(7);
    let mut pairs: Vec<String> = match kind {
        0 => vec![
            format!("P={}", vector(s, 3)),
            format!("d={}", number(s, Num::NonNeg)),
        ],
        1 => {
            let n = if s.rarely(32) {
                MAX_RISERS - 4 + usize::from(s.below(8))
            } else {
                usize::from(s.below(4)) + 1
            };
            let risers: Vec<String> = (0..n).map(|_| number(s, Num::NonNeg)).collect();
            vec![
                format!("E={}", vector(s, 3)),
                format!("d={}", number(s, Num::NonNeg)),
                format!("risers={}", risers.join("/")),
            ]
        }
        2 => vec![
            format!("B={}", vector(s, 3)),
            format!("zmax={}", number(s, Num::Any)),
            format!("rmax={}", number(s, Num::NonNeg)),
        ],
        3 => {
            let from = s.u16();
            let to = if s.rarely(32) {
                70_000
            } else {
                u32::from(s.u16())
            };
            vec![
                format!("{from}>{to}"),
                format!("E={}", vector(s, 3)),
                format!("d={}", number(s, Num::NonNeg)),
            ]
        }
        4 => vec![
            format!("E={}", vector(s, 3)),
            format!("d={}", number(s, Num::NonNeg)),
            format!("dz={}", number(s, Num::NonNeg)),
            format!("headroom={}", number(s, Num::NonNeg)),
        ],
        5 => vec![
            format!("in={}", vector(s, 2)),
            format!("swim={}", vector(s, 2)),
            format!("h={}", number(s, Num::Any)),
            format!("fh={}", number(s, Num::NonNeg)),
            format!("d={}", number(s, Num::NonNeg)),
        ],
        _ => vec![
            format!("E={}", vector(s, 3)),
            format!("d={}", number(s, Num::NonNeg)),
        ],
    };
    if pairs.len() > 1 && s.byte() & 1 == 0 {
        let k = usize::from(s.byte()) % pairs.len();
        pairs.swap(0, k);
    }
    match s.byte() {
        0xFF => {
            pairs.pop();
        }
        0xFE => {
            if let Some(p) = pairs.first().cloned() {
                pairs.push(p);
            }
        }
        0xFD => pairs.push("q=1".to_owned()),
        _ => {}
    }
    format!("{} {}", KINDS[usize::from(kind)], pairs.join(" "))
}

/// The twelve field values of one walk, in `FIELD_NAMES` order.
fn fields(s: &mut Src, row: usize) -> [String; 12] {
    const CATEGORIES: [&str; 10] = [
        "wall",
        "undrawn",
        "generated",
        "step",
        "steep",
        "interior",
        "cave",
        "water",
        "frill",
        "roof",
    ];
    const GAITS: [&str; 5] = ["walk", "run", "jump", "swim", "crawl"];
    let id = walk_id(s, row);
    let category = CATEGORIES[usize::from(s.below(9)) + usize::from(s.rarely(64))].to_owned();
    let region = match s.byte() {
        0xFF => "0".to_owned(),
        0xFE => "256".to_owned(),
        b => b.max(1).to_string(),
    };
    let landblock = match s.byte() {
        0xFF => "0x".to_owned(),
        0xFE => format!("{:04X}", s.u16()),
        0xFD => format!("0x{:05X}", u32::from(s.u16()) + 0x1_0000),
        0xFC => format!("0X{:x}", s.u16()),
        _ => format!("0x{:04X}", s.u16()),
    };
    let cell = if s.rarely(32) {
        "65536".to_owned()
    } else {
        s.u16().to_string()
    };
    let start = vector(s, 3);
    let loc = match s.byte() {
        0xFF => String::new(),
        0xFE => "r1 lx1|2".to_owned(),
        0xFD => "r1\tlx2".to_owned(),
        _ => format!(
            "r{} lx{} ly{} ox{} oy{}",
            s.byte(),
            s.u16(),
            s.u16(),
            s.byte(),
            s.byte()
        ),
    };
    let heading = if s.rarely(32) {
        "3600.5".to_owned()
    } else {
        number(s, Num::Any)
    };
    let gait = GAITS[usize::from(s.below(4)) + usize::from(s.rarely(64))].to_owned();
    let inputs = inputs(s);
    let duration = number(s, Num::Time);
    let expect = expect(s);
    [
        id, category, region, landblock, cell, start, loc, heading, gait, inputs, duration, expect,
    ]
}

/// One walk file from the input: the twelve keys in an order the input picks, with
/// comments, blank lines and a byte-order mark now and then.
fn walk_file(s: &mut Src) -> String {
    let f = fields(s, 0);
    let mut order: Vec<usize> = (0..f.len()).collect();
    if s.byte() & 1 == 0 {
        for i in (1..order.len()).rev() {
            let j = usize::from(s.byte()) % (i + 1);
            order.swap(i, j);
        }
    }
    let mut out = String::new();
    if s.rarely(16) {
        out.push('\u{feff}');
    }
    for i in order {
        let (Some(key), Some(value)) = (FIELD_NAMES.get(i), f.get(i)) else {
            continue;
        };
        match s.byte() {
            0xF0..=0xF7 => out.push_str("# a comment\n\n"),
            0xFE => continue, // a missing key
            0xFF => {
                let _ = writeln!(out, "{key} = {value}"); // a duplicate key
            }
            _ => {}
        }
        let space = if s.byte() & 1 == 0 { " " } else { "" };
        let _ = writeln!(out, "{key}{space}={space}{value}");
    }
    out
}

/// A Markdown text holding a walk table of up to eight rows from the input.
fn walk_table(s: &mut Src) -> String {
    let mut out = String::new();
    if !s.rarely(32) {
        out.push_str("# Walks\n\n<a id=\"s2-3\"></a>\n\n");
    }
    out.push_str(WALK_TABLE_HEADER);
    out.push_str("\n|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    let rows = usize::from(s.below(8)) + 1;
    for row in 0..rows {
        let mut f = fields(s, row);
        if s.rarely(64) {
            f[0] = "dup".to_owned(); // two such rows share an id
        }
        let quote = s.byte() & 1 == 0;
        let short = s.rarely(128);
        out.push('|');
        for (i, v) in f.iter().enumerate() {
            if short && i + 1 == f.len() {
                break; // a cell short
            }
            if quote && matches!(i, 6 | 9 | 11) {
                let _ = write!(out, " `{v}` |");
            } else {
                let _ = write!(out, " {v} |");
            }
        }
        out.push('\n');
    }
    out.push_str("\nAfter the table.\n| not | a | row |\n");
    out
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let walks = match mode % 4 {
        0 => parse_walk_file(&String::from_utf8_lossy(rest)).map(|w| vec![w]),
        1 => parse_walk_table(&String::from_utf8_lossy(rest)),
        2 => parse_walk_file(&walk_file(&mut Src { data: rest, pos: 0 })).map(|w| vec![w]),
        _ => parse_walk_table(&walk_table(&mut Src { data: rest, pos: 0 })),
    };
    if let Ok(walks) = walks {
        for w in &walks {
            check(w);
        }
    }
});
