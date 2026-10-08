// SPDX-License-Identifier: GPL-3.0-or-later
//! Worked examples (`docs/specs/abc-dialect.md` §13), invariants (§14.1), hostile input
//! (§14.2), file lookup and the song listing (§3.1, §11), property tests, and one real-data
//! check of the message table and pitch slots (§9.5, §10).
//!
//! Every tune is original to the spec or written for this file. The real-data test is
//! skipped (reported as passed) unless `TESSERA_GAME_DIR` names an install; it only reads.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use proptest::prelude::*;
use tessera_audio::abc::{
    ARG_TITLE, AbcError, AbcErrorKind, LISTING_TITLE_MAX_CHARS, MAX_DURATION, MAX_EVENTS,
    MAX_LISTED_PARTS, MAX_LISTED_PARTS_TOTAL, MESSAGE_TABLE_KEY, MIN_DURATION, NoteEvent,
    NoteEventKind, Notice, ORDER_TOLERANCE, PITCH_SLOTS, PartRequest, Performance, ReadOutcome,
    StartMode, duration_seconds, find_song, list_songs, note_label, parse, pitch_slot, read_song,
};

const TOL: f64 = 1e-6;

fn run(text: &str) -> Result<Performance, AbcError> {
    parse(text.as_bytes(), PartRequest::Unspecified)
}

fn ok(text: &str) -> Performance {
    match run(text) {
        Ok(p) => p,
        Err(e) => panic!("refused ({e}): {text:?}"),
    }
}

fn ons(p: &Performance) -> Vec<(f64, u8)> {
    p.note_ons().map(|e| (e.time, e.pitch)).collect()
}

fn assert_ons(what: &str, p: &Performance, want: &[(f64, u8)]) {
    let got = ons(p);
    assert_eq!(got.len(), want.len(), "{what}: got {got:?}, want {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(
            (g.0 - w.0).abs() <= TOL && g.1 == w.1,
            "{what}: got {got:?}, want {want:?}"
        );
    }
}

fn tune(body: &str) -> String {
    format!("X:1\nT:t\nM:4/4\nL:1/8\nQ:120\nK:C\n{body}\n")
}

/// Events with times rounded to 1 µs, for comparisons between equivalent files.
fn shape(p: &Performance) -> Vec<(NoteEventKind, u8, u8, i64)> {
    p.events
        .iter()
        .map(|e| (e.kind, e.pitch, e.volume, (e.time * 1e6).round() as i64))
        .collect()
}

/// §14.1 INV1, INV2, INV3, INV9, INV10 on an accepted part.
fn check_invariants(p: &Performance) {
    // INV1.
    for e in p.events.iter().filter(|e| e.kind == NoteEventKind::On) {
        assert!((48..=84).contains(&e.pitch), "INV1 pitch {e:?}");
        assert!((1..=37).contains(&e.slot), "INV1 slot {e:?}");
        assert_eq!(Some(e.slot), pitch_slot(e.pitch), "INV1 slot table");
        assert!((30..=127).contains(&e.volume), "volume {e:?}");
    }
    // INV2.
    for n in &p.notes {
        assert!(
            n.duration >= MIN_DURATION && n.duration <= MAX_DURATION,
            "INV2 {n:?}"
        );
    }
    // INV3.
    if let Some(last) = p.notes.last() {
        assert!(last.event < MAX_EVENTS, "INV3 events");
    }
    let mut per_event = std::collections::HashMap::<usize, usize>::new();
    for n in &p.notes {
        *per_event.entry(n.event).or_default() += 1;
    }
    assert!(per_event.values().all(|&c| c <= 6), "INV3 chord size");
    // INV9: per slot, on/off alternate starting with on; one end event, last.
    let mut sounding = [false; 38];
    let (last, body) = p.events.split_last().expect("end event");
    assert_eq!(last.kind, NoteEventKind::End, "INV9 end last");
    assert_eq!(last.delta, 0.0);
    for e in body {
        let s = &mut sounding[usize::from(e.slot)];
        match e.kind {
            NoteEventKind::On => {
                assert!(!*s, "INV9 on while sounding {e:?}");
                *s = true;
            }
            NoteEventKind::Off => {
                assert!(*s, "INV9 off while silent {e:?}");
                *s = false;
            }
            NoteEventKind::End => panic!("INV9 end before the last event"),
        }
    }
    // INV10 (and deltas): output is sorted by time; events within ORDER_TOLERANCE keep
    // creation order (§9.2), so a time may sit up to that much below its predecessor.
    let mut prev = 0.0;
    for (i, e) in p.events.iter().enumerate() {
        if e.kind != NoteEventKind::End {
            assert!(
                (e.delta - (e.time - prev)).abs() < 1e-9,
                "delta {i} {e:?} prev {prev}"
            );
            if i > 0 {
                assert!(e.time >= prev - ORDER_TOLERANCE, "INV10 at {i}: {e:?}");
            }
        }
        prev = e.time;
    }
}

// ----- §13.1 A two-part file -----

const TWO_PARTS: &str = "% synthetic\nT:Example set\nM:3/4\nL:1/8\nQ:90\n\nX:1\nT:Melody\nK:D\n\
|: d2 fe dc | B4 A2 :|\n[1 d6 :|2 d4 z2 |]\n\nX:2\nT:Bass\nL:1/4\nK:D\nD, A, D, | G,3 |\n";

#[test]
fn s13_1_two_part_file_part_1() {
    let third = 1.0 / 3.0;
    for req in [PartRequest::Unspecified, PartRequest::Number(1)] {
        let p = parse(TWO_PARTS.as_bytes(), req).unwrap();
        assert_eq!(p.title, "Example set");
        assert_eq!(p.part, Some(1));
        assert_eq!(p.timing.meter_numerator, 3);
        assert_eq!(p.timing.meter_denominator, 4);
        assert_eq!(p.timing.unit_length, 0.125);
        assert_ons(
            "§13.1 part 1",
            &p,
            &[
                (0.0, 74),
                (2.0 * third, 78),
                (1.0, 76),
                (4.0 * third, 74),
                (5.0 * third, 73),
                (2.0, 71),
                (10.0 * third, 69),
                (4.0, 74),
                (6.0, 74),
            ],
        );
        assert!(
            (p.end_time() - 22.0 * third).abs() < TOL,
            "{}",
            p.end_time()
        );
        check_invariants(&p);
    }
}

#[test]
fn s13_1_two_part_file_part_2() {
    let p = parse(TWO_PARTS.as_bytes(), PartRequest::Number(2)).unwrap();
    assert_eq!(p.title, "Example set");
    assert_eq!(p.part, Some(2));
    let q = 2.0 / 3.0;
    assert_ons(
        "§13.1 part 2",
        &p,
        &[(0.0, 50), (q, 57), (2.0 * q, 50), (2.0, 55)],
    );
    // G,3 is three quarters of 2/3 s = 2.0 s by §6.2/§8.2. The spec's §13.1 text says
    // "lasting 1.3333 s"; that disagrees with its own rules and is recorded as an OPEN item
    // in the spec. This test follows the rules.
    let g = p.notes.last().unwrap();
    assert_eq!(g.pitch, Some(55));
    assert!((g.duration - 2.0).abs() < TOL, "{}", g.duration);
    check_invariants(&p);
}

#[test]
fn s13_1_two_part_file_part_3() {
    let e = parse(TWO_PARTS.as_bytes(), PartRequest::Number(3)).unwrap_err();
    assert_eq!(e.kind, AbcErrorKind::PartNotFound);
}

// ----- §13.2 The 60 ms boundary -----

#[test]
fn s13_2_boundary_arithmetic() {
    // q = 10: 1.0 × 0.01 = 0.01; × 60 = 0.6; ÷ 10 = 0.06 → accepted.
    assert_eq!(duration_seconds(4, 0.25, 10.0 / 1000.0, 10), 0.06);
    // q = 9: … ÷ 9 = 0.05999999999999999 → refused.
    let d9 = duration_seconds(4, 0.25, 9.0 / 1000.0, 9);
    assert_eq!(d9, 0.059_999_999_999_999_99);
    assert!(d9 < MIN_DURATION);
    // The step values of the spec's q = 9 line.
    assert_eq!(1.0f64 * 0.009, 0.009);
    assert_eq!(0.009f64 * 60.0, 0.539_999_999_999_999_9);
    // Through the reader.
    let text = |q: u32| format!("X:1\nM:4/4\nL:1/4\nQ:{q}\nK:C\nG{q}/1000|\n");
    assert!(run(&text(10)).is_ok());
    assert_eq!(run(&text(9)).unwrap_err().kind, AbcErrorKind::NoteTooShort);
}

#[test]
fn s13_2_refused_tempos() {
    let refused: Vec<u32> = (1..=240u32)
        .filter(|&q| duration_seconds(4, 0.25, f64::from(q) / 1000.0, q) < MIN_DURATION)
        .collect();
    assert_eq!(
        refused,
        vec![
            9, 11, 13, 15, 18, 22, 26, 30, 36, 37, 43, 44, 45, 51, 52, 60, 72, 74, 86, 88, 90, 102,
            104, 120, 141, 144, 148, 172, 173, 176, 180, 204, 208, 240
        ]
    );
}

// ----- §13.3 A unit-length field after the music -----

#[test]
fn s13_3_unit_field_after_music() {
    let after = ok(&tune("G A B c|\nL:1/4\nd e f g|"));
    let times: Vec<f64> = after.note_ons().map(|e| e.time).collect();
    assert_eq!(times, vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5]);
    assert_eq!(after.end_time(), 4.0);
    let header = ok("X:1\nT:t\nM:4/4\nL:1/4\nQ:120\nK:C\nG A B c|\nd e f g|\n");
    assert_eq!(shape(&after), shape(&header));
}

// ----- §14.1 Invariants -----

/// Tunes for the invariant checks (accepted ones), including the §12 bodies that play.
const ACCEPTED: &[&str] = &[
    "G A|\nB c|",
    "[CEG] c|",
    "[G4B] c|",
    "[G4z] G2 A|",
    "G-AG B|",
    "[G-B]G c|",
    "G2>A2 B2<c2 d4>>e4 f8>>>g8|",
    "G>[Bd] c|",
    "(3GAB c|",
    "(3:2:4GABc d|",
    "(3[GB]cd e|",
    "+p+G +F+A +FFF+B|",
    "|: G |1 A :|2 B |]",
    "G [] A|",
    "[c4z] +ff+c2 d2|",
    "[G8c8] G G G G c c|",
    "C, ^C, D, _E, E, F, ^F, G, ^G, A, _B, B, C ^C D ^D E F ^F G ^G A ^A B c ^c d ^d e f ^f g ^g a ^a b c'|",
];

#[test]
fn inv1_2_3_9_10_on_accepted_tunes() {
    for body in ACCEPTED {
        let p = ok(&tune(body));
        check_invariants(&p);
    }
}

#[test]
fn inv4_refused_part_has_no_events() {
    // A refusal is an `Err`: there is no performance to play. An error late in the file
    // (here on the last line) still refuses everything read before it.
    for body in ["G A B c|\nd e f g|\n[CDEFGAB]", "G A|\nG-A B|", "G A|\nQ:0"] {
        let r = run(&tune(body));
        assert!(r.is_err(), "{body:?} should be refused");
    }
}

#[test]
fn inv5_ignored_constructs_change_nothing() {
    let clean = tune("G A B c|d e f g|\nA4 G4|");
    let noisy = "% leading comment\nX:1\nT:t\nM:4/4\nL:1/8\nQ:120\nK:C\nN:a note\n\n   \n\
        \"Am\"G {ga}A ~B .c|[1 (d e) uf vg| % trailing\nw:some lyrics\n%%directive\n\
        [2 \"D7\"{f}~A4 (G4)|\n";
    let a = ok(&clean);
    let b = ok(noisy);
    assert_eq!(shape(&a), shape(&b));
    check_invariants(&b);
}

#[test]
fn inv6_bar_lines_do_not_change_times() {
    let plain = ok(&tune("G A B c d e [CE] (3GAB c2 z G"));
    let barred = ok(&tune("G | A B || c d |: e [CE] :| (3GAB | c2 z |] G"));
    assert_eq!(shape(&plain), shape(&barred));
    // With an accidental, only the pitch that depended on it changes.
    let a = ok(&tune("^F G F A"));
    let b = ok(&tune("^F G | F A"));
    let ta: Vec<f64> = a.note_ons().map(|e| e.time).collect();
    let tb: Vec<f64> = b.note_ons().map(|e| e.time).collect();
    assert_eq!(ta, tb);
    let pa: Vec<u8> = a.note_ons().map(|e| e.pitch).collect();
    let pb: Vec<u8> = b.note_ons().map(|e| e.pitch).collect();
    assert_eq!(pa, vec![66, 67, 66, 69]);
    assert_eq!(pb, vec![66, 67, 65, 69]);
}

#[test]
fn inv7_moving_timing_fields() {
    let a = ok("X:1\nM:6/8\nL:1/8\nQ:90\nK:C\nG A B|c d e|\n");
    let b = ok("X:1\nK:C\nG A B|\nQ:90\nc d e|\nM:6/8\nL:1/8\n");
    let c = ok("X:1\nM:6/8\nK:C\nG A B|\nL:1/8\nc d e|\nQ:90\n");
    assert_eq!(shape(&a), shape(&b));
    assert_eq!(shape(&a), shape(&c));
}

#[test]
fn inv8_lower_case_field_letters() {
    let upper = ok("X:1\nT:Title\nM:3/4\nL:1/4\nQ:100\nK:G\nF G A|\n");
    let lower = ok("x:1\nt:Title\nm:3/4\nl:1/4\nq:100\nk:G\nF G A|\n");
    assert_eq!(shape(&upper), shape(&lower));
    assert_eq!(upper.title, lower.title);
    assert_eq!(upper.part, lower.part);
}

#[test]
fn inv11_part_request_equals_extracted_part() {
    let head = "T:Set\nM:3/4\nL:1/8\nQ:100\n";
    let parts = [
        "X:1\nK:G\nL:1/4\nG A B|\n",
        "X:7\nT:Seven\nK:D\nf e d|\n+ff+[DF] A|\n",
        "X:12\nM:4/4\nQ:60\nK:C\nC E G c|\n",
    ];
    let file = format!("{head}{}", parts.concat());
    for (k, part) in [(1, parts[0]), (7, parts[1]), (12, parts[2])] {
        let whole = parse(file.as_bytes(), PartRequest::Number(k)).unwrap();
        let alone = run(&format!("{head}{part}")).unwrap();
        assert_eq!(shape(&whole), shape(&alone), "part {k}");
        assert_eq!(whole.part, Some(k));
        check_invariants(&whole);
    }
}

// ----- §14.2 Hostile input -----

#[test]
fn hostile_long_digit_runs() {
    // Runs of 128 or more digits are read whole and saturate (§4.1, Tessera's choice).
    for n in [127usize, 128, 129, 168, 10_000] {
        let body = format!("G{}|", "1".repeat(n));
        let e = run(&tune(&body)).unwrap_err();
        assert_eq!(e.kind, AbcErrorKind::NoteTooLong, "{n} digits");
        let zeros = format!("G{}2 A|", "0".repeat(n));
        let p = ok(&tune(&zeros));
        let t: Vec<f64> = p.note_ons().map(|e| e.time).collect();
        assert_eq!(t, vec![0.0, 0.5], "{n} zeros");
    }
    // A huge X: value and a huge tuplet number.
    assert_eq!(
        parse(
            tune("G|")
                .replacen("X:1", &format!("X:{}", "9".repeat(500)), 1)
                .as_bytes(),
            PartRequest::Number(i32::MAX)
        )
        .unwrap()
        .part,
        Some(i32::MAX)
    );
    assert_eq!(
        run(&tune(&format!("({}GAB|", "3".repeat(300))))
            .unwrap_err()
            .kind,
        AbcErrorKind::TupletNumberOutOfRange
    );
}

/// Hostile bodies of about `n` characters: long white-space and backslash runs, an
/// unclosed quote and grace group, a long comment, many duplicate chord entries.
fn hostile_bodies(n: usize) -> Vec<String> {
    vec![
        format!("G{}A|", " ".repeat(n)),
        format!("G{}A|", "\\".repeat(n)),
        format!("G \"{}", "ab|".repeat(n / 3)),
        format!("G {{{}", "ab|\n".repeat(n / 4)),
        format!("G A|%{}\nB|", "x".repeat(n)),
        format!("[{}] c|", "G".repeat(n)),
    ]
}

/// §14.2: the scans stay linear. Each body is timed at n and 4n characters (best of five,
/// to tolerate a loaded machine); a linear reader takes about 4 times as long, a quadratic
/// one 16 times.
#[test]
fn hostile_long_runs_scale_linearly() {
    fn best(text: &str) -> f64 {
        (0..5)
            .map(|_| {
                let t = Instant::now();
                let _ = run(text);
                t.elapsed().as_secs_f64()
            })
            .fold(f64::INFINITY, f64::min)
    }
    let n = 40_000;
    for (small, large) in hostile_bodies(n).iter().zip(hostile_bodies(4 * n).iter()) {
        let (small, large) = (tune(small), tune(large));
        // A scheduling stall on a loaded machine (other test threads, other builds) can
        // spoil one measurement; a quadratic reader fails every attempt, so up to three
        // attempts are made before the assertion.
        let (mut ts, mut tl) = (0.0, f64::INFINITY);
        for _ in 0..3 {
            (ts, tl) = (best(&small), best(&large));
            if tl < 10.0 * ts + 0.02 {
                break;
            }
        }
        assert!(
            tl < 10.0 * ts + 0.02,
            "not linear: {:.4} s at {} bytes, {:.4} s at {} bytes",
            ts,
            small.len(),
            tl,
            large.len()
        );
    }
}

#[test]
fn hostile_long_runs() {
    let start = Instant::now();
    let big = 2_000_000;
    // White space, backslashes, an unclosed quote and an unclosed grace group.
    ok(&tune(&format!("G{}A|", " ".repeat(big))));
    ok(&tune(&format!("G{}A|", "\\".repeat(big))));
    let p = ok(&tune(&format!("G \"{}", "ab|".repeat(big / 3))));
    assert_eq!(p.note_ons().count(), 1);
    assert!(matches!(p.outcome, ReadOutcome::Truncated { .. }));
    let p = ok(&tune(&format!("G {{{}", "ab|\n".repeat(big / 4))));
    assert_eq!(p.note_ons().count(), 1);
    assert!(matches!(p.outcome, ReadOutcome::Truncated { .. }));
    // Comments and long lines.
    ok(&tune(&format!("G A|%{}\nB|", "x".repeat(big))));
    // Long accidental and octave-mark runs do not overflow.
    assert_eq!(
        run(&tune(&format!("{}C|", "^".repeat(big))))
            .unwrap_err()
            .kind,
        AbcErrorKind::NoteTooHigh
    );
    assert_eq!(
        run(&tune(&format!("c{}|", "'".repeat(big))))
            .unwrap_err()
            .kind,
        AbcErrorKind::NoteTooHigh
    );
    assert_eq!(
        run(&tune(&format!("C{}|", ",".repeat(big))))
            .unwrap_err()
            .kind,
        AbcErrorKind::NoteTooLow
    );
    // An accidental cancelled by many octaves stays exact: `^`×24 on C,, is C.
    let p = ok(&tune(&format!("{}C,,|", "^".repeat(24))));
    assert_eq!(ons(&p), vec![(0.0, 60)]);
    // Many duplicate chord entries are dropped without growing anything.
    let p = ok(&tune(&format!("[{}] c|", "G".repeat(big / 2))));
    assert_eq!(p.note_ons().count(), 2);
    let secs = start.elapsed().as_secs_f64();
    eprintln!("hostile long runs: {secs:.2} s (debug build)");
}

#[test]
fn hostile_event_limit_with_chords() {
    let p = ok(&tune(&"[CDEFGA]".repeat(MAX_EVENTS)));
    assert_eq!(p.notes.len(), 6 * MAX_EVENTS);
    check_invariants(&p);
    let e = run(&tune(&"[CDEFGA]".repeat(MAX_EVENTS + 1))).unwrap_err();
    assert_eq!(e.kind, AbcErrorKind::TooManyEvents);
    assert_eq!(e.message(), None);
    assert_eq!(Notice::EventLimit.message(), None);
}

#[test]
fn hostile_undefined_durations() {
    // Unit 0 and tempo 0: 0 ÷ 0 is NaN. The original accepts it (§14.2); Tessera refuses it
    // as "note too long", the error tempo 0 otherwise gives.
    let text = "X:1\nL:0/1\nQ:0\nK:C\nG|\n";
    assert_eq!(run(text).unwrap_err().kind, AbcErrorKind::NoteTooLong);
    // Unit 0 alone: too short.
    let text = "X:1\nL:0/1\nK:C\nG|\n";
    assert_eq!(run(text).unwrap_err().kind, AbcErrorKind::NoteTooShort);
}

#[test]
fn hostile_negative_times() {
    // Empty chords move the time line back by 1 s each; times may go below zero.
    let p = ok(&tune("G [] [] [] A B|"));
    let starts: Vec<f64> = p.notes.iter().map(|n| n.start).collect();
    assert_eq!(starts, vec![0.0, -2.75, -2.5]);
    check_invariants(&p);
}

#[test]
fn hostile_bytes_and_encodings() {
    // Bytes 0x80–0xFF in a plain file: inside a title they are characters, in music they
    // are letters (colon expected) or illegal (truncation).
    let p = parse(b"X:1\nT:Caf\xE9\nK:C\nG|\n", PartRequest::Unspecified).unwrap();
    assert_eq!(p.title, "Caf\u{e9}");
    assert_eq!(
        parse(b"X:1\nK:C\nG \xE9|\n", PartRequest::Unspecified)
            .unwrap_err()
            .kind,
        AbcErrorKind::ColonExpected
    );
    let p = parse(b"X:1\nK:C\nG \xA7|\n", PartRequest::Unspecified).unwrap();
    assert_eq!(p.note_ons().count(), 1);
    // Files shorter than 4 bytes have no byte-order mark: FE FF are the letters þ ÿ here.
    assert_eq!(
        parse(b"\xFE\xFF", PartRequest::Unspecified)
            .unwrap_err()
            .kind,
        AbcErrorKind::ColonExpected
    );
    assert!(parse(b"", PartRequest::Unspecified).unwrap().events.len() == 1);
    // UTF-32 BE mark: not found.
    assert_eq!(
        parse(b"\x00\x00\xFE\xFFG|", PartRequest::Unspecified)
            .unwrap_err()
            .kind,
        AbcErrorKind::FileNotFound
    );
}

// ----- Titles and notices (§6.5, §10.4) -----

#[test]
fn titles_and_notices() {
    let p = ok("X:1\nT:99 Bottles % comment\nK:C\nG|\n");
    assert_eq!(p.title, "Bottles");
    let p = ok("X:1\nT:Waltz\\ in G\nK:C\nG|\n");
    assert_eq!(p.title, "Waltz in G");
    let long = "x".repeat(100);
    let p = ok(&format!("X:1\nT: {long}\nK:C\nG|\n"));
    assert_eq!(p.notice_title().chars().count(), 63);
    match p.start_notice(StartMode::Solo) {
        Notice::NowPlaying { title } => assert_eq!(title.len(), 63),
        other => panic!("{other:?}"),
    }
    let m = p.start_notice(StartMode::Solo).message().unwrap();
    assert_eq!(m.token, 0x00A1_9C55);
    assert_eq!(m.table_key, MESSAGE_TABLE_KEY);
    assert_eq!(m.args[0].id, ARG_TITLE);
    let p = ok("X:1\nK:C\nG|\n");
    assert_eq!(p.start_notice(StartMode::Solo), Notice::NowPlayingUntitled);
    assert_eq!(
        Notice::NowPlayingUntitled.message().unwrap().token,
        0x05D7_A7A7
    );
    assert_eq!(p.start_notice(StartMode::Synced), Notice::SyncedStart);
    assert_eq!(Notice::SyncedStart.message().unwrap().token, 0x012D_D4A4);
}

#[test]
fn packed_words() {
    let p = ok(&tune("+f+A|"));
    let on: &NoteEvent = p.note_ons().next().unwrap();
    assert_eq!(on.slot, 11);
    assert_eq!(on.packed(), 0x6900_000B);
}

// ----- §3.1 lookup and §11 listing -----

/// A fresh directory under Cargo's per-target temporary directory, removed when dropped.
/// The process ID keeps concurrent runs apart.
struct TempDir(PathBuf);

impl std::ops::Deref for TempDir {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_dir(tag: &str) -> TempDir {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("abc_examples")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    TempDir(dir)
}

#[test]
fn lookup_rules() {
    let dir = temp_dir("lookup");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub").join("tune.abc"), tune("G|")).unwrap();
    std::fs::write(dir.join("song.abc"), tune("A|")).unwrap();
    assert_eq!(
        find_song(&dir, "sub/tune").unwrap(),
        dir.join("sub").join("tune.abc")
    );
    // An extension in the name is replaced.
    assert_eq!(find_song(&dir, "song.mid").unwrap(), dir.join("song.abc"));
    assert_eq!(find_song(&dir, "song.abc").unwrap(), dir.join("song.abc"));
    // Names that would leave the folder are not found.
    for name in ["", "../song", "sub/../../song", "/etc/passwd"] {
        let e = find_song(&dir, name).unwrap_err();
        assert_eq!(e.kind, AbcErrorKind::FileNotFound, "{name:?}");
    }
}

#[test]
fn song_listing() {
    let dir = temp_dir("listing");
    std::fs::write(
        dir.join("Song One.ABC"),
        "T:File title\nX:1\nT:\nT:First\nK:C\nG|\nX:0x10\nT: Second \nX:3\n",
    )
    .unwrap();
    std::fs::write(dir.join("two.txt"), "X:5\nK:C\nG|\n").unwrap();
    std::fs::write(dir.join("notes.mid"), "X:9\n").unwrap();
    std::fs::create_dir_all(dir.join("folder.abc")).unwrap();
    std::fs::write(dir.join("wide.abc"), b"\xFF\xFEX\x00:\x001\x00").unwrap();
    let list = list_songs(&dir).unwrap();
    let names: Vec<&str> = list.iter().map(|s| s.file_name.as_str()).collect();
    assert_eq!(names, vec!["Song One.ABC", "two.txt", "wide.abc"]);
    let one = &list[0];
    assert_eq!(one.name, "Song One");
    assert_eq!(one.file_title.as_deref(), Some("File title"));
    let parts: Vec<(i32, Option<&str>)> = one
        .parts
        .iter()
        .map(|p| (p.number, p.title.as_deref()))
        .collect();
    assert_eq!(
        parts,
        vec![(1, Some("First")), (16, Some("Second")), (3, None)]
    );
    assert!(one.complete);
    assert_eq!(list[1].parts.len(), 1);
    assert_eq!(list[1].parts[0].number, 5);
    assert!(list[2].parts.is_empty());
    assert!(!list[2].complete);
    assert!(list_songs(&dir.join("missing")).is_err());
    assert_eq!(list[1].parts[0].request(), PartRequest::Number(5));
    // Every listed name plays the listed file (here: `Song One` finds `Song One.ABC`).
    for song in &list {
        assert_eq!(find_song(&dir, &song.name).unwrap(), song.path, "{song:?}");
    }
}

/// §11 lists `.abc`/`.txt` in any letter case and §3.1 OPEN: on a case-sensitive file
/// system the lookup still finds them by the listed name, and a stem with a dot is listed
/// by its whole file name.
#[test]
fn listed_names_resolve() {
    let dir = temp_dir("roundtrip");
    for name in [
        "Tune.ABC",
        "Other.TXT",
        "Bach.Minuet.abc",
        "v1.2.TxT",
        "lower.abc",
        "Song..abc",
    ] {
        std::fs::write(dir.join(name), tune("G A|")).unwrap();
    }
    let list = list_songs(&dir).unwrap();
    assert_eq!(list.len(), 6);
    let names: Vec<(&str, &str)> = list
        .iter()
        .map(|s| (s.file_name.as_str(), s.name.as_str()))
        .collect();
    assert!(names.contains(&("Tune.ABC", "Tune")), "{names:?}");
    assert!(
        names.contains(&("Bach.Minuet.abc", "Bach.Minuet.abc")),
        "{names:?}"
    );
    assert!(names.contains(&("v1.2.TxT", "v1.2.TxT")), "{names:?}");
    for song in &list {
        assert_eq!(find_song(&dir, &song.name).unwrap(), song.path, "{song:?}");
        let p = read_song(&dir, &song.name, PartRequest::Unspecified).unwrap();
        assert_eq!(p.note_ons().count(), 2, "{song:?}");
    }
    // Letter case of the whole name is ignored, as on the original's file system.
    assert_eq!(find_song(&dir, "tune").unwrap(), dir.join("Tune.ABC"));
    assert_eq!(find_song(&dir, "OTHER").unwrap(), dir.join("Other.TXT"));
}

/// §3.1 order with letter case ignored: `.abc` in any case comes before `.txt`, and an
/// exact match comes before other spellings.
#[test]
fn lookup_letter_case_order() {
    let dir = temp_dir("case");
    std::fs::write(dir.join("Mix.ABC"), tune("G|")).unwrap();
    std::fs::write(dir.join("Mix.txt"), tune("A|")).unwrap();
    assert_eq!(find_song(&dir, "Mix").unwrap(), dir.join("Mix.ABC"));
    std::fs::write(dir.join("Mix.abc"), tune("B|")).unwrap();
    assert_eq!(find_song(&dir, "Mix").unwrap(), dir.join("Mix.abc"));
    let e = find_song(&dir, "Mixx").unwrap_err();
    assert_eq!(e.kind, AbcErrorKind::FileNotFound);
    assert_eq!(e.path, Some(dir.join("Mixx.txt")));
}

/// Only regular files are opened: a folder or FIFO with a song name is refused at once
/// instead of blocking in `open`.
#[test]
fn lookup_refuses_non_regular_files() {
    let dir = temp_dir("special");
    std::fs::create_dir_all(dir.join("folder.abc")).unwrap();
    let e = read_song(&dir, "folder", PartRequest::Unspecified).unwrap_err();
    assert_eq!(e.kind, AbcErrorKind::Io(std::io::ErrorKind::InvalidInput));
    #[cfg(unix)]
    {
        let fifo = dir.join("pipe.abc");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|s| s.success());
        if made {
            let d: PathBuf = dir.to_path_buf();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(read_song(&d, "pipe", PartRequest::Unspecified));
            });
            let r = rx.recv_timeout(Duration::from_secs(5));
            if r.is_err() {
                // Unblock the stuck reader before failing.
                let _ = std::fs::OpenOptions::new().write(true).open(&fifo);
            }
            let e = r.expect("read_song blocked on a FIFO").unwrap_err();
            assert_eq!(e.kind, AbcErrorKind::Io(std::io::ErrorKind::InvalidInput));
            let list = list_songs(&dir).unwrap();
            assert!(list.is_empty(), "{list:?}");
        } else {
            eprintln!("mkfifo unavailable; FIFO case skipped");
        }
    }
}

/// The listing keeps display-length titles and a total part budget per call.
#[test]
fn listing_bounds() {
    let dir = temp_dir("bounds");
    let long = "\u{00E9}".repeat(500);
    std::fs::write(dir.join("a.abc"), format!("T:{long}\nX:1\nT:{long}\n")).unwrap();
    let files = MAX_LISTED_PARTS_TOTAL / MAX_LISTED_PARTS + 1;
    let many: String = (0..MAX_LISTED_PARTS).map(|i| format!("X:{i}\n")).collect();
    for i in 0..files {
        std::fs::write(dir.join(format!("b{i:03}.abc")), &many).unwrap();
    }
    let list = list_songs(&dir).unwrap();
    let a = &list[0];
    let chars = |t: &Option<String>| t.as_deref().map_or(0, |t| t.chars().count());
    assert_eq!(chars(&a.file_title), LISTING_TITLE_MAX_CHARS);
    assert_eq!(chars(&a.parts[0].title), LISTING_TITLE_MAX_CHARS);
    let total: usize = list.iter().map(|s| s.parts.len()).sum();
    assert_eq!(total, MAX_LISTED_PARTS_TOTAL);
    let last = list.last().unwrap();
    assert!(!last.complete);
    assert!(list[1].complete);
}

// ----- Property tests -----

/// Fragments for random tunes: valid and invalid pieces of the dialect.
const FRAGMENTS: &[&str] = &[
    "G",
    "a",
    "^c",
    "_B,",
    "=f",
    "c'",
    "C,",
    "z",
    "x2",
    "c/",
    "d3/2",
    "e//",
    "f>",
    "g<",
    "A>>",
    "[CEG]",
    "[G2B]",
    "[Gz]",
    "[]",
    "(3",
    "(3:2:4",
    "(",
    ")",
    "|",
    "||",
    ":|",
    "|:",
    "::",
    "|]",
    "[|",
    "[1",
    "|1",
    ":|2",
    "\"Am\"",
    "{g}",
    "~",
    ".",
    "u",
    " ",
    "\n",
    "-",
    "+mf+",
    "+p+",
    "M:3/4\n",
    "M:C|\n",
    "L:1/16\n",
    "L:1/4\n",
    "Q:90\n",
    "Q:1/4=200\n",
    "K:D\n",
    "K:Amin\n",
    "K:G ^c\n",
    "X:2\n",
    "T:x\n",
    "%c\n",
    "\\",
    "!",
    "$",
    "7",
    "/",
    ">",
    "w:la\n",
    "G4",
    "B8",
    "c16",
    "z/4",
];

fn fragment_tune() -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::sample::select(FRAGMENTS), 0..80)
        .prop_map(|v| format!("X:1\nT:t\nM:4/4\nL:1/8\nQ:120\nK:C\n{}\n", v.concat()))
}

/// Deterministic companion of the property tests: random fragment tunes are accepted often
/// enough that the invariant checks run on real performances, and every accepted one keeps
/// the invariants.
#[test]
fn random_fragment_tunes_accepted_share() {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let (mut accepted, total) = (0usize, 4000usize);
    for _ in 0..total {
        let len = (next() % 24) as usize;
        let body: String = (0..len)
            .map(|_| FRAGMENTS[(next() % FRAGMENTS.len() as u64) as usize])
            .collect();
        if let Ok(p) = run(&tune(&body)) {
            accepted += 1;
            check_invariants(&p);
        }
    }
    eprintln!("random fragment tunes: {accepted} of {total} accepted");
    assert!(accepted * 10 >= total, "{accepted} of {total} accepted");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Arbitrary bytes never panic; accepted parts satisfy the invariants.
    #[test]
    fn random_bytes_never_panic(
        bytes in proptest::collection::vec(any::<u8>(), 0..1024),
        req in proptest::option::of(-3i32..20),
    ) {
        if let Ok(p) = parse(&bytes, req.into()) {
            check_invariants(&p);
        }
    }

    /// Random tunes made of dialect fragments never panic; accepted parts satisfy the
    /// invariants.
    #[test]
    fn random_tunes_keep_invariants(text in fragment_tune(), req in proptest::option::of(1i32..3)) {
        if let Ok(p) = parse(text.as_bytes(), req.into()) {
            check_invariants(&p);
        }
    }

    /// INV6 on random runs of plain notes: inserting bars never changes the events.
    #[test]
    fn bars_never_change_plain_notes(
        notes in proptest::collection::vec(
            (proptest::sample::select(&["C", "D", "e", "f", "z", "[CE]", "G2", "A/"][..]), any::<bool>()),
            1..40,
        ),
    ) {
        let plain: String = notes.iter().map(|(n, _)| format!("{n} ")).collect();
        let barred: String = notes
            .iter()
            .map(|(n, bar)| if *bar { format!("{n} | ") } else { format!("{n} ") })
            .collect();
        let a = ok(&tune(&plain));
        let b = ok(&tune(&barred));
        prop_assert_eq!(shape(&a), shape(&b));
    }
}

// ----- Real data (gated) -----

/// Checks against the user's install: the message table named by key 0x10000069 holds every
/// token of §10.2 and §10.4 with the listed argument IDs, and every pitch slot equals the key
/// of its note label in the note enum mapper (§9.5). Read-only.
#[test]
fn real_data_message_table_and_slots() {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return;
        }
    };
    use tessera_dat::{ArchiveSet, SetOptions};
    use tessera_formats::{decode_did_map, decode_enum_mapper, decode_string_table};
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set");
    let root = decode_did_map(&set.read(0x2800_0000).expect("root DID map")).unwrap();
    let named = root.by_label("STRINGTABLE").expect("STRINGTABLE label");
    let named = decode_did_map(&set.read(named.0).expect("named tables")).unwrap();
    let table_did = named.get(MESSAGE_TABLE_KEY).expect("key 0x10000069").value;
    let table = decode_string_table(&set.read(table_did.0).expect("message table")).unwrap();
    let sorted = |v: &[u32]| {
        let mut v = v.to_vec();
        v.sort_unstable();
        v
    };
    for kind in AbcErrorKind::TABLE {
        let token = kind.token().unwrap();
        let entry = table
            .get(token)
            .unwrap_or_else(|| panic!("{kind}: token {token:#010x} missing"));
        assert_eq!(sorted(&entry.arg_ids), sorted(kind.arg_ids()), "{kind}");
    }
    for (notice, ids) in [
        (
            Notice::NowPlaying {
                title: String::new(),
            },
            vec![ARG_TITLE],
        ),
        (Notice::NowPlayingUntitled, vec![]),
        (Notice::SyncedStart, vec![]),
    ] {
        let token = notice.message().unwrap().token;
        let entry = table.get(token).expect("notice token");
        assert_eq!(sorted(&entry.arg_ids), ids, "{notice:?}");
    }
    let mapper = decode_enum_mapper(&set.read(0x2300_015E).expect("note enum mapper")).unwrap();
    for p in 48..=84u8 {
        let label = note_label(p).unwrap();
        assert_eq!(
            mapper.key_of(&label),
            pitch_slot(p).map(u32::from),
            "pitch {p} {label}"
        );
    }
    eprintln!(
        "message table {:#010x}: {} error tokens and 3 notices present; {} pitch slots match",
        table_did.0,
        AbcErrorKind::TABLE.len(),
        PITCH_SLOTS.len()
    );
}
