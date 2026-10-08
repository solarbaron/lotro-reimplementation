// SPDX-License-Identifier: GPL-3.0-or-later
//! The conformance table of `docs/specs/abc-dialect.md` §12, one test per row.
//!
//! Test names start with the subsection (`s12_1_` … `s12_7_`) and the row ID, so that
//! `cargo test --test abc_conformance s12_3_` runs one subsection. Every tune is original
//! to the spec (§12) and embedded here as text; nothing is read from a game install.
//!
//! Unless a row says otherwise a tune is the reference header (`X:1`, `T:t`, `M:4/4`,
//! `L:1/8`, `Q:120`, `K:C`) followed by the body, with `⏎` written as `\n` and a final line
//! break. Note-on times are compared within [`TOL`] (1 µs, well under the 1 ms asked for).

use std::path::PathBuf;

use tessera_audio::abc::{
    AbcError, AbcErrorKind, ArgValue, NoteEvent, NoteEventKind, PartRequest, Performance, Position,
    ReadOutcome, SOLO_START_OFFSET, StartMode, parse, read_song,
};

/// Time tolerance, seconds.
const TOL: f64 = 1e-6;

const REF_HEADER: &str = "X:1\nT:t\nM:4/4\nL:1/8\nQ:120\nK:C\n";

/// Reference header + body + final line break.
fn tune(body: &str) -> String {
    format!("{REF_HEADER}{body}\n")
}

/// The reference header with lines replaced: each `(letter, line)` replaces the header line
/// of that field letter; an empty `line` removes it.
fn header_with(changes: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for line in REF_HEADER.lines() {
        match changes
            .iter()
            .find(|(l, _)| line.starts_with(&format!("{l}:")))
        {
            Some((_, "")) => {}
            Some((_, new)) => {
                out.push_str(new);
                out.push('\n');
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

fn tune_with(changes: &[(&str, &str)], body: &str) -> String {
    format!("{}{body}\n", header_with(changes))
}

fn run(text: &str) -> Result<Performance, AbcError> {
    parse(text.as_bytes(), PartRequest::Unspecified)
}

fn run_req(text: &str, req: i32) -> Result<Performance, AbcError> {
    parse(text.as_bytes(), PartRequest::Number(req))
}

fn ok(id: &str, r: Result<Performance, AbcError>) -> Performance {
    match r {
        Ok(p) => p,
        Err(e) => panic!("{id}: expected to play, refused: {e}"),
    }
}

fn ons(p: &Performance) -> Vec<(f64, u8)> {
    p.note_ons().map(|e| (e.time, e.pitch)).collect()
}

fn offs(p: &Performance) -> Vec<(f64, u8)> {
    p.events
        .iter()
        .filter(|e| e.kind == NoteEventKind::Off)
        .map(|e| (e.time, e.pitch))
        .collect()
}

fn assert_pairs(id: &str, got: &[(f64, u8)], want: &[(f64, u8)]) {
    assert_eq!(
        got.len(),
        want.len(),
        "{id}: got {got:?}, want {want:?} (count)"
    );
    for (g, w) in got.iter().zip(want) {
        assert!(
            (g.0 - w.0).abs() <= TOL && g.1 == w.1,
            "{id}: got {got:?}, want {want:?}"
        );
    }
}

/// Plays exactly these note-ons (time, pitch), in this order.
fn plays(id: &str, r: Result<Performance, AbcError>, want: &[(f64, u8)]) -> Performance {
    let p = ok(id, r);
    assert_pairs(id, &ons(&p), want);
    p
}

/// Plays note-ons at these times (pitches not checked).
fn plays_at(id: &str, r: Result<Performance, AbcError>, want: &[f64]) -> Performance {
    let p = ok(id, r);
    let got: Vec<f64> = p.note_ons().map(|e| e.time).collect();
    assert_eq!(got.len(), want.len(), "{id}: got {got:?}, want {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() <= TOL, "{id}: got {got:?}, want {want:?}");
    }
    p
}

/// Plays these pitches, in this order.
fn plays_pitches(id: &str, r: Result<Performance, AbcError>, want: &[u8]) -> Performance {
    let p = ok(id, r);
    let got: Vec<u8> = p.note_ons().map(|e| e.pitch).collect();
    assert_eq!(got, want, "{id}");
    p
}

fn refused(id: &str, r: Result<Performance, AbcError>, kind: AbcErrorKind) -> AbcError {
    match r {
        Ok(p) => panic!("{id}: expected refusal ({kind}), plays {:?}", ons(&p)),
        Err(e) => {
            assert_eq!(e.kind, kind, "{id}: refused with {e}, want {kind}");
            e
        }
    }
}

fn truncated(id: &str, p: &Performance) {
    assert!(
        matches!(p.outcome, ReadOutcome::Truncated { .. }),
        "{id}: expected truncation, outcome {:?}",
        p.outcome
    );
}

fn off_of(p: &Performance, pitch: u8) -> f64 {
    p.events
        .iter()
        .find(|e| e.kind == NoteEventKind::Off && e.pitch == pitch)
        .map(|e| e.time)
        .unwrap_or(f64::NAN)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOL
}

const G: u8 = 67;
const A: u8 = 69;
const B: u8 = 71;
const C5: u8 = 72; // ABC `c`

/// B1's expectation: 0 G, 0.25 A, 0.5 B, 0.75 c.
const B1_PLAYS: [(f64, u8); 4] = [(0.0, G), (0.25, A), (0.5, B), (0.75, C5)];

/// A fresh directory for file-lookup rows (under Cargo's per-target temporary directory),
/// removed again when the guard is dropped. The process ID keeps concurrent runs apart.
struct TempMusicDir(PathBuf);

impl TempMusicDir {
    fn new(tag: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("abc_conformance")
            .join(format!("{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp music dir");
        TempMusicDir(dir)
    }
}

impl std::ops::Deref for TempMusicDir {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl Drop for TempMusicDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_music_dir(tag: &str) -> TempMusicDir {
    TempMusicDir::new(tag)
}

/// Asserts a refusal's position (line, column; §3.5, §10.2).
fn at(id: &str, e: &AbcError, line: u32, column: u32) {
    assert_eq!(e.position, Some(Position { line, column }), "{id}: {e}");
}

/// Asserts truncation at a position.
fn truncated_at(id: &str, p: &Performance, line: u32, column: u32) {
    assert_eq!(
        p.outcome,
        ReadOutcome::Truncated {
            at: Position { line, column }
        },
        "{id}"
    );
}

// ----- §12.1 Files, comments and lines -----

#[test]
fn s12_1_a1_utf8_bom() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(tune("G A|").as_bytes());
    plays(
        "A1",
        parse(&bytes, PartRequest::Unspecified),
        &[(0.0, G), (0.25, A)],
    );
}

#[test]
fn s12_1_a2_utf16_refused() {
    let text = tune("G A|");
    let mut bytes = vec![0xFF, 0xFE];
    for u in text.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    refused(
        "A2",
        parse(&bytes, PartRequest::Unspecified),
        AbcErrorKind::EncodingNotSupported,
    );
    // Through the file layer the argument is the file's path.
    let dir = temp_music_dir("a2");
    let path = dir.join("tune.abc");
    std::fs::write(&path, &bytes).unwrap();
    let e = refused(
        "A2",
        read_song(&dir, "tune", PartRequest::Unspecified),
        AbcErrorKind::EncodingNotSupported,
    );
    assert_eq!(e.path.as_deref(), Some(path.as_path()));
    let m = e.message().unwrap();
    assert_eq!(m.token, 0x0604_2C77);
    assert_eq!(m.args.len(), 1);
    assert_eq!(m.args[0].id, 0x0E0A_22B5);
    assert_eq!(
        m.args[0].value,
        ArgValue::Text(path.to_string_lossy().into_owned())
    );
}

#[test]
fn s12_1_a3_utf7_mark_not_found() {
    let mut bytes = vec![0x2B, 0x2F, 0x76];
    bytes.extend_from_slice(tune("G A|").as_bytes());
    refused(
        "A3",
        parse(&bytes, PartRequest::Unspecified),
        AbcErrorKind::FileNotFound,
    );
}

#[test]
fn s12_1_a4_line_ends() {
    let lf = tune("G A|\nB c|");
    let want = ok("A4", run(&lf)).events;
    for variant in [lf.replace('\n', "\r\n"), lf.replace('\n', "\r")] {
        let got = ok("A4", run(&variant)).events;
        assert_eq!(got, want, "A4: {variant:?}");
    }
}

#[test]
fn s12_1_a5_nul_ends_input() {
    let text = format!("{REF_HEADER}G A|\0B c|\n");
    plays("A5", run(&text), &[(0.0, G), (0.25, A)]);
}

#[test]
fn s12_1_a6_four_byte_utf8_ends_input() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(format!("{REF_HEADER}G A\u{1F3B5}B c|\n").as_bytes());
    plays(
        "A6",
        parse(&bytes, PartRequest::Unspecified),
        &[(0.0, G), (0.25, A)],
    );
}

#[test]
fn s12_1_a7_txt_found() {
    let dir = temp_music_dir("a7");
    std::fs::write(dir.join("tune.txt"), tune("G A|")).unwrap();
    plays(
        "A7",
        read_song(&dir, "tune", PartRequest::Unspecified),
        &[(0.0, G), (0.25, A)],
    );
}

#[test]
fn s12_1_a8_abc_before_txt() {
    let dir = temp_music_dir("a8");
    std::fs::write(dir.join("tune.abc"), tune("G A|")).unwrap();
    std::fs::write(dir.join("tune.txt"), tune("B c|")).unwrap();
    plays(
        "A8",
        read_song(&dir, "tune", PartRequest::Unspecified),
        &[(0.0, G), (0.25, A)],
    );
    // And neither: not found, with the `.txt` path as the argument (§3.1 step 4).
    let e = refused(
        "A8",
        read_song(&dir, "missing", PartRequest::Unspecified),
        AbcErrorKind::FileNotFound,
    );
    assert_eq!(e.path, Some(dir.join("missing.txt")));
}

#[test]
fn s12_1_b1_comment() {
    plays("B1", run(&tune("G A % B c\nB c|")), &B1_PLAYS);
}

#[test]
fn s12_1_b1b_backslash_percent_is_comment() {
    plays("B1b", run(&tune("G A \\% B c\nB c|")), &B1_PLAYS);
}

#[test]
fn s12_1_b2_backslash_line_end() {
    plays("B2", run(&tune("G A \\\nB c|")), &B1_PLAYS);
}

#[test]
fn s12_1_b3_blank_lines() {
    plays("B3", run(&tune("G A|\n\n \nB c|")), &B1_PLAYS);
}

#[test]
fn s12_1_b4_ignored_lyrics_field() {
    plays("B4", run(&tune("G A|\nw:one two\nB c|")), &B1_PLAYS);
}

#[test]
fn s12_1_b5_lower_case_field_applies_part_wide() {
    plays(
        "B5",
        run(&tune("G A|\nl:1/4\nB c|")),
        &[(0.0, G), (0.5, A), (1.0, B), (1.5, C5)],
    );
}

#[test]
fn s12_1_b5b_ignored_fields() {
    plays(
        "B5b",
        run(&tune("G A|\nN:anything\nR:reel\nB c|")),
        &B1_PLAYS,
    );
}

#[test]
fn s12_1_b6_free_text_refused() {
    let e = refused(
        "B6",
        run(&tune("G A|\nPlay it softly\nB c|")),
        AbcErrorKind::ColonExpected,
    );
    // Reported at the token after the letter: line 8 (6 header lines, `G A|`), column 2.
    assert_eq!(e.position, Some(Position { line: 8, column: 2 }));
    let m = e.message().unwrap();
    assert_eq!(m.token, 0x03B3_DC8E);
    assert_eq!(m.table_key, 0x1000_0069);
    let args: Vec<(u32, ArgValue)> = m.args.into_iter().map(|a| (a.id, a.value)).collect();
    assert_eq!(
        args,
        vec![
            (0x0005_0E25, ArgValue::Integer(8)),
            (0x0484_1A1E, ArgValue::Integer(2))
        ]
    );
}

#[test]
fn s12_1_b7_plus_colon_refused() {
    refused(
        "B7",
        run(&tune("G A|\n+:more")),
        AbcErrorKind::UnknownVolumeMark,
    );
}

#[test]
fn s12_1_b8_field_takes_rest_of_line() {
    plays("B8", run(&tune("G A K:D f c|")), &[(0.0, G), (0.25, A)]);
}

#[test]
fn s12_1_b9_space_before_colon_refused() {
    let text = "X:1\nT:t\nM:4/4\nL:1/8\nQ:120\nK :C\nG A|\n";
    refused("B9", run(text), AbcErrorKind::ColonExpected);
}

#[test]
fn s12_1_b10_field_after_leading_spaces() {
    plays("B10", run(&tune("   K:G\nF A|")), &[(0.0, 66), (0.25, A)]);
}

// ----- §12.2 Parts -----

const C_FILE: &str =
    "T:Set\nM:4/4\nL:1/8\nQ:120\nX:1\nK:C\nL:1/4\nG A|\nX:2\nK:C\nB c|\nX:3\nK:C\nd e|\n";

#[test]
fn s12_2_c1_first_part() {
    let p = plays("C1", run(C_FILE), &[(0.0, G), (0.5, A)]);
    assert_eq!(p.part, Some(1));
    assert_eq!(p.title, "Set");
}

#[test]
fn s12_2_c2_second_part() {
    let p = plays("C2", run_req(C_FILE, 2), &[(0.0, B), (0.25, C5)]);
    assert_eq!(p.part, Some(2));
}

#[test]
fn s12_2_c3_missing_part() {
    let e = refused("C3", run_req(C_FILE, 4), AbcErrorKind::PartNotFound);
    let m = e.message().unwrap();
    assert_eq!(m.token, 0x0BCA_2D44);
    assert!(m.args.is_empty());
}

#[test]
fn s12_2_c4_no_x_line() {
    let text = "T:t\nM:4/4\nL:1/8\nQ:120\nK:C\nG A|\n";
    plays("C4", run_req(text, 3), &[(0.0, G), (0.25, A)]);
}

#[test]
fn s12_2_c5_empty_x_ends_selected_part() {
    refused(
        "C5",
        run(&tune("G A|\nX:\nB c|")),
        AbcErrorKind::NumberExpected,
    );
}

#[test]
fn s12_2_c6_empty_x_skipped() {
    plays(
        "C6",
        run_req(&tune("G A|\nX:\nB c|\nX:3\nd e|"), 3),
        &[(0.0, 74), (0.25, 76)],
    );
}

#[test]
fn s12_2_c7_octal_x_found_by_8() {
    plays(
        "C7",
        run_req(&tune("G A|\nX:010\nB c|"), 8),
        &[(0.0, B), (0.25, C5)],
    );
}

#[test]
fn s12_2_c8_octal_x_not_found_by_10() {
    refused(
        "C8",
        run_req(&tune("G A|\nX:010\nB c|"), 10),
        AbcErrorKind::PartNotFound,
    );
}

#[test]
fn s12_2_c9_nothing_after_next_x_is_read() {
    plays(
        "C9",
        run(&tune("G A|\nX:2\n!! [[\nB c|")),
        &[(0.0, G), (0.25, A)],
    );
}

#[test]
fn s12_2_c10_music_before_first_x() {
    let text = "T:t\nM:4/4\nL:1/8\nQ:120\nK:C\nE F|\nX:1\nG A|\nX:2\nB c|\n";
    plays(
        "C10",
        run_req(text, 2),
        &[(0.0, 64), (0.25, 65), (0.5, B), (0.75, C5)],
    );
}

#[test]
fn s12_2_c11_lower_case_x() {
    let text = tune("G A|\nx:2\nB c|").replacen("X:1", "x:1", 1);
    plays("C11", run(&text), &[(0.0, G), (0.25, A)]);
}

// ----- §12.3 Header fields -----

#[test]
fn s12_3_d1_meter_c() {
    plays_at("D1", run(&tune_with(&[("M", "M:C")], "G A|")), &[0.0, 0.25]);
}

#[test]
fn s12_3_d2_meter_cut() {
    plays_at(
        "D2",
        run(&tune_with(&[("M", "M:C|")], "G A|")),
        &[0.0, 0.125],
    );
}

#[test]
fn s12_3_d3_meter_resets_unit() {
    plays_at(
        "D3",
        run(&tune_with(&[("M", "M:3/8"), ("L", "")], "G A|")),
        &[0.0, 0.25],
    );
}

#[test]
fn s12_3_d4_meter_none() {
    refused(
        "D4",
        run(&tune_with(&[("M", "M:none")], "G A|")),
        AbcErrorKind::MeterMalformed,
    );
}

#[test]
fn s12_3_d5_meter_sum() {
    refused(
        "D5",
        run(&tune_with(&[("M", "M:3+2/8")], "G A|")),
        AbcErrorKind::MeterMalformed,
    );
}

#[test]
fn s12_3_d6_meter_after_unit() {
    let text = "X:1\nT:t\nL:1/4\nM:4/4\nQ:120\nK:C\nG A|\n";
    plays_at("D6", run(text), &[0.0, 0.25]);
}

#[test]
fn s12_3_d7_unit_single_number() {
    refused(
        "D7",
        run(&tune_with(&[("L", "L:1")], "G A|")),
        AbcErrorKind::UnitLengthMalformed,
    );
}

#[test]
fn s12_3_d8_tempo_beat_is_meter_denominator() {
    plays_at(
        "D8",
        run(&tune_with(&[("M", "M:6/8"), ("Q", "Q:3/8=90")], "G A|")),
        &[0.0, 2.0 / 3.0],
    );
}

#[test]
fn s12_3_d9_text_before_tempo() {
    refused(
        "D9",
        run(&tune_with(&[("Q", "Q:\"Slow\" 1/4=100")], "G A|")),
        AbcErrorKind::TempoMalformed,
    );
}

#[test]
fn s12_3_d10_text_after_tempo() {
    plays_at(
        "D10",
        run(&tune_with(&[("Q", "Q:1/4=100 \"Slow\"")], "G A|")),
        &[0.0, 0.3],
    );
}

#[test]
fn s12_3_d11_tempo_c_form() {
    plays_at(
        "D11",
        run(&tune_with(&[("Q", "Q:C=100")], "G A|")),
        &[0.0, 0.3],
    );
}

#[test]
fn s12_3_d12_m_is_major() {
    plays_pitches(
        "D12",
        run(&tune_with(&[("K", "K:Em")], "F G c d|")),
        &[66, 68, 73, 75],
    );
}

#[test]
fn s12_3_d13_minor() {
    plays_pitches(
        "D13",
        run(&tune_with(&[("K", "K:E minor")], "F G c d|")),
        &[66, 67, 72, 74],
    );
}

#[test]
fn s12_3_d14_dorian() {
    plays_pitches(
        "D14",
        run(&tune_with(&[("K", "K:Edor")], "F G c d|")),
        &[66, 67, 73, 74],
    );
}

#[test]
fn s12_3_d15_explicit_accidental() {
    plays_pitches(
        "D15",
        run(&tune_with(&[("K", "K:D _B")], "F B c|")),
        &[66, 70, 73],
    );
}

#[test]
fn s12_3_d16_highland_pipes() {
    refused(
        "D16",
        run(&tune_with(&[("K", "K:Hp")], "G A|")),
        AbcErrorKind::KeyWordUnknown,
    );
}

#[test]
fn s12_3_d17_empty_key() {
    refused(
        "D17",
        run(&tune_with(&[("K", "K:")], "G A|")),
        AbcErrorKind::KeyMalformed,
    );
}

#[test]
fn s12_3_d18_clef_word() {
    refused(
        "D18",
        run(&tune_with(&[("K", "K:G treble")], "G A|")),
        AbcErrorKind::KeyWordUnknown,
    );
}

#[test]
fn s12_3_d19_g_flat_uses_row_6() {
    plays_pitches(
        "D19",
        run(&tune_with(&[("K", "K:Gb")], "G B c|")),
        &[68, 71, 73],
    );
}

#[test]
fn s12_3_d20_unit_after_music() {
    plays_at(
        "D20",
        run(&tune("G A B c|\nL:1/16\nd e|")),
        &[0.0, 0.125, 0.25, 0.375, 0.5, 0.625],
    );
}

#[test]
fn s12_3_d21_tempo_after_music() {
    plays_at(
        "D21",
        run(&tune("G A B c|\nQ:60\nd e|")),
        &[0.0, 0.5, 1.0, 1.5, 2.0, 2.5],
    );
}

#[test]
fn s12_3_d22_meter_after_music() {
    plays_at(
        "D22",
        run(&tune("G A|\nM:3/4\nB c|")),
        &[0.0, 0.25, 0.5, 0.75],
    );
}

#[test]
fn s12_3_d23_lower_case_fields() {
    let text = tune_with(
        &[("M", "m:3/4"), ("L", "l:1/4"), ("Q", "q:60"), ("K", "k:G")],
        "F A|",
    );
    plays("D23", run(&text), &[(0.0, 66), (1.0, A)]);
}

#[test]
fn s12_3_d24_first_non_empty_title() {
    let p = ok(
        "D24",
        run(&tune_with(&[("T", "T:\nT:   \nT:Second")], "G|")),
    );
    assert_eq!(p.title, "Second");
}

// ----- §12.4 Notes, accidentals and lengths -----

#[test]
fn s12_4_e1_range_ends() {
    plays_pitches("E1", run(&tune("C, c'|")), &[48, 84]);
}

#[test]
fn s12_4_e2_too_low() {
    // Reported at the token after the octave marks (§7.3): the space.
    let e = refused("E2", run(&tune("B,, c|")), AbcErrorKind::NoteTooLow);
    at("E2", &e, 7, 4);
}

#[test]
fn s12_4_e3_too_high() {
    let e = refused("E3", run(&tune("^c' c|")), AbcErrorKind::NoteTooHigh);
    at("E3", &e, 7, 4);
}

#[test]
fn s12_4_e4_repeated_sharps() {
    plays_pitches("E4", run(&tune("^^^C D|")), &[63, 62]);
}

#[test]
fn s12_4_e5_accidental_same_octave_only() {
    plays_pitches("E5", run(&tune("_B B, B b|")), &[70, 59, 70, 83]);
}

#[test]
fn s12_4_e6_bar_clears() {
    plays_pitches("E6", run(&tune("_e | e|")), &[75, 76]);
}

#[test]
fn s12_4_e7_first_ending_keeps() {
    plays_pitches("E7", run(&tune("_e |1 e|")), &[75, 75]);
}

#[test]
fn s12_4_e8_second_ending_keeps() {
    plays_pitches("E8", run(&tune("_e :|2 e|")), &[75, 75]);
}

#[test]
fn s12_4_e9_mixed_signs() {
    refused("E9", run(&tune("=_e|")), AbcErrorKind::UnexpectedSymbol);
}

#[test]
fn s12_4_e10_natural_remembered() {
    plays_pitches(
        "E10",
        run(&tune_with(&[("K", "K:D")], "f =f f|f|")),
        &[78, 77, 77, 78],
    );
}

#[test]
fn s12_4_f1_lengths() {
    plays_at(
        "F1",
        run(&tune("G/ A// B3/ c3/2 d|")),
        &[0.0, 0.125, 0.1875, 0.9375, 1.3125],
    );
}

#[test]
fn s12_4_f2_zero_denominator() {
    refused(
        "F2",
        run(&tune("G/0 A|")),
        AbcErrorKind::ZeroLengthDenominator,
    );
}

#[test]
fn s12_4_f3_zero_numerator() {
    // Timing errors report the note's letter (§10.2).
    let e = refused("F3", run(&tune("G0 A|")), AbcErrorKind::NoteTooShort);
    at("F3", &e, 7, 1);
}

#[test]
fn s12_4_f4_third_slash() {
    refused("F4", run(&tune("G/// A|")), AbcErrorKind::UnexpectedSymbol);
}

#[test]
fn s12_4_f5_large_numbers() {
    plays_at(
        "F5",
        run(&tune_with(&[("L", "L:1/2000000")], "G250000 A250000|")),
        &[0.0, 0.25],
    );
}

#[test]
fn s12_4_f6_leading_zeros() {
    plays_at("F6", run(&tune("G00002 A|")), &[0.0, 0.5]);
}

#[test]
fn s12_4_f7_saturated_length() {
    refused(
        "F7",
        run(&tune("G99999999999 A|")),
        AbcErrorKind::NoteTooLong,
    );
}

#[test]
fn s12_4_f8_backslash_in_digit_run() {
    plays_at("F8", run(&tune("G1\\2 A|")), &[0.0, 3.0]);
}

#[test]
fn s12_4_g1_rests() {
    plays("G1", run(&tune("z2 G x2 A|")), &[(0.5, G), (1.25, A)]);
}

#[test]
fn s12_4_g2_capital_z() {
    // Colon expected: at the token after the letter (§7.1 rule 2).
    let e = refused("G2", run(&tune("G Z A|")), AbcErrorKind::ColonExpected);
    at("G2", &e, 7, 4);
}

#[test]
fn s12_4_g3_tied_rest() {
    plays("G3", run(&tune("z- G|")), &[(0.25, G)]);
}

// ----- §12.5 Chords, ties and broken rhythm -----

#[test]
fn s12_5_h1_chord() {
    plays(
        "H1",
        run(&tune("[CEG] c|")),
        &[(0.0, 60), (0.0, 64), (0.0, G), (0.25, C5)],
    );
}

#[test]
fn s12_5_h2_shortest_entry_advances() {
    let p = plays(
        "H2",
        run(&tune("[G4B] c|")),
        &[(0.0, G), (0.0, B), (0.25, C5)],
    );
    assert!(close(off_of(&p, G), 1.0), "H2: G off {}", off_of(&p, G));
}

#[test]
fn s12_5_h3_rest_entry_advances() {
    let p = plays("H3", run(&tune("[G4z] c|")), &[(0.0, G), (0.25, C5)]);
    assert!(close(off_of(&p, G), 1.0), "H3: G off {}", off_of(&p, G));
}

#[test]
fn s12_5_h4_seven_notes() {
    refused(
        "H4",
        run(&tune("[CDEFGAB] c|")),
        AbcErrorKind::TooManyNotesInChord,
    );
}

#[test]
fn s12_5_h5_six_notes_and_rest() {
    refused(
        "H5",
        run(&tune("[CDEFGAz] c|")),
        AbcErrorKind::TooManyNotesInChord,
    );
}

#[test]
fn s12_5_h6_rests_count_once() {
    plays(
        "H6",
        run(&tune("[CDEFGzz] c|")),
        &[
            (0.0, 60),
            (0.0, 62),
            (0.0, 64),
            (0.0, 65),
            (0.0, G),
            (0.25, C5),
        ],
    );
}

#[test]
fn s12_5_h7_duplicate_dropped() {
    plays(
        "H7",
        run(&tune("[CDEFGAA] c|")),
        &[
            (0.0, 60),
            (0.0, 62),
            (0.0, 64),
            (0.0, 65),
            (0.0, G),
            (0.0, A),
            (0.25, C5),
        ],
    );
}

#[test]
fn s12_5_h8_duplicate_does_not_advance() {
    let p = plays("H8", run(&tune("[G2G4] c|")), &[(0.0, G), (0.5, C5)]);
    assert!(close(off_of(&p, G), 0.5), "H8: G off {}", off_of(&p, G));
}

#[test]
fn s12_5_h9_length_after_chord() {
    refused("H9", run(&tune("[GB]4 c|")), AbcErrorKind::UnexpectedSymbol);
}

#[test]
fn s12_5_h10_tie_after_chord() {
    refused(
        "H10",
        run(&tune("[GB]- [GB]|")),
        AbcErrorKind::UnexpectedSymbol,
    );
}

#[test]
fn s12_5_h11_space_in_chord() {
    refused("H11", run(&tune("[G B] c|")), AbcErrorKind::ChordNotClosed);
}

#[test]
fn s12_5_h12_unclosed_chord() {
    refused("H12", run(&tune("[GB c|")), AbcErrorKind::ChordNotClosed);
}

#[test]
fn s12_5_h13_empty_chord_moves_back() {
    // The row lists note-on times; the output is sorted by time (§9.2), so A (−0.75 s)
    // comes before G (0 s).
    plays("H13", run(&tune("G [] A|")), &[(-0.75, A), (0.0, G)]);
    let p = ok("H13", run(&tune("G [] A|")));
    let starts: Vec<(f64, Option<u8>)> = p.notes.iter().map(|n| (n.start, n.pitch)).collect();
    assert_eq!(
        starts,
        vec![(0.0, Some(G)), (-0.75, Some(A))],
        "H13 time line"
    );
}

#[test]
fn s12_5_h14_empty_chord_first() {
    refused("H14", run(&tune("[] G A|")), AbcErrorKind::UnexpectedSymbol);
}

#[test]
fn s12_5_i1_tie() {
    let p = plays("I1", run(&tune("G4-G4|")), &[(0.0, G)]);
    assert_pairs("I1", &offs(&p), &[(2.0, G)]);
}

#[test]
fn s12_5_i2_tie_over_other_note() {
    let p = plays(
        "I2",
        run(&tune("G-AG B|")),
        &[(0.0, G), (0.25, A), (0.75, B)],
    );
    assert!(close(off_of(&p, G), 0.5), "I2: G off {}", off_of(&p, G));
}

#[test]
fn s12_5_i3_unfinished_tie() {
    let e = refused("I3", run(&tune("G-A B|")), AbcErrorKind::UnfinishedTie);
    // Reported at the tied note's letter: line 7, column 1.
    assert_eq!(e.position, Some(Position { line: 7, column: 1 }));
}

#[test]
fn s12_5_i4_tie_needs_same_pitch() {
    refused("I4", run(&tune("^G-|G A|")), AbcErrorKind::UnfinishedTie);
}

#[test]
fn s12_5_i5_tie_from_chord() {
    let p = plays(
        "I5",
        run(&tune("[G-B]G c|")),
        &[(0.0, G), (0.0, B), (0.5, C5)],
    );
    assert!(close(off_of(&p, G), 0.5), "I5: G off {}", off_of(&p, G));
}

#[test]
fn s12_5_j1_broken_rhythm() {
    plays_at(
        "J1",
        run(&tune("G2>A2 B2<c2 d4>>e4 f8>>>g8|")),
        &[0.0, 0.75, 1.0, 1.25, 2.0, 3.75, 4.0, 7.75],
    );
}

#[test]
fn s12_5_j2_four_symbols() {
    refused(
        "J2",
        run(&tune("G>>>>A|")),
        AbcErrorKind::BrokenRhythmTooLong,
    );
}

#[test]
fn s12_5_j3_after_chord() {
    refused(
        "J3",
        run(&tune("[GB]>c d|")),
        AbcErrorKind::BrokenRhythmMisplaced,
    );
}

#[test]
fn s12_5_j4_before_chord() {
    let p = plays(
        "J4",
        run(&tune("G>[Bd] c|")),
        &[(0.0, G), (0.375, B), (0.375, 74), (0.5, C5)],
    );
    assert!(close(off_of(&p, B), 0.5), "J4: B off {}", off_of(&p, B));
    assert!(close(off_of(&p, 74), 0.625), "J4: d off {}", off_of(&p, 74));
}

#[test]
fn s12_5_j5_after_space() {
    refused("J5", run(&tune("G >A|")), AbcErrorKind::UnexpectedSymbol);
}

#[test]
fn s12_5_j6_across_line() {
    plays_at("J6", run(&tune("G>\nA B|")), &[0.0, 0.375, 0.5]);
}

// ----- §12.6 Tuplets, prefixes, volume and bars -----

#[test]
fn s12_6_k1_triplet() {
    plays_at(
        "K1",
        run(&tune("(3GAB c|")),
        &[0.0, 1.0 / 6.0, 1.0 / 3.0, 0.5],
    );
}

#[test]
fn s12_6_k2_spaces_in_triplet() {
    // At the token where the problem was seen (§10.2): the space after G.
    let e = refused(
        "K2",
        run(&tune("(3G A B c|")),
        AbcErrorKind::NonNoteInTuplet,
    );
    at("K2", &e, 7, 4);
    // After a prefix, still the token that is not a note (not the prefix).
    let e = refused(
        "K2",
        run(&tune("(3~ G A B|")),
        AbcErrorKind::NonNoteInTuplet,
    );
    at("K2 after a decoration", &e, 7, 4);
}

#[test]
fn s12_6_k3_quintuplet_compound_meter() {
    plays_at(
        "K3",
        run(&tune_with(&[("M", "M:3/4")], "(5GABcd e|")),
        &[0.0, 0.15, 0.3, 0.45, 0.6, 0.75],
    );
}

#[test]
fn s12_6_k4_quintuplet_simple_meter() {
    plays_at(
        "K4",
        run(&tune("(5GABcd e|")),
        &[0.0, 0.1, 0.2, 0.3, 0.4, 0.5],
    );
}

#[test]
fn s12_6_k5_full_tuplet_form() {
    let s = 1.0 / 6.0;
    plays_at(
        "K5",
        run(&tune("(3:2:4GABc d|")),
        &[0.0, s, 2.0 * s, 3.0 * s, 4.0 * s],
    );
}

#[test]
fn s12_6_k6_tuplet_out_of_range() {
    let e = refused(
        "K6",
        run(&tune("(10GABcdefgab c|")),
        AbcErrorKind::TupletNumberOutOfRange,
    );
    at("K6", &e, 7, 2);
}

#[test]
fn s12_6_k7_chord_in_tuplet() {
    plays(
        "K7",
        run(&tune("(3[GB]cd e|")),
        &[
            (0.0, G),
            (0.0, B),
            (1.0 / 6.0, C5),
            (1.0 / 3.0, 74),
            (0.5, 76),
        ],
    );
}

#[test]
fn s12_6_k8_tuplet_factor_checked() {
    refused(
        "K8",
        run(&tune_with(&[("Q", "Q:100")], "(3G/4A/4B/4 c|")),
        AbcErrorKind::NoteTooShort,
    );
}

#[test]
fn s12_6_k9_short_note_accepted() {
    plays_at(
        "K9",
        run(&tune_with(&[("Q", "Q:100")], "G/4 c|")),
        &[0.0, 0.075],
    );
}

#[test]
fn s12_6_l1_chord_symbol() {
    plays("L1", run(&tune("\"Dm\"G A|")), &[(0.0, G), (0.25, A)]);
}

#[test]
fn s12_6_l2_unclosed_quote_first() {
    refused(
        "L2",
        run(&tune("\"Dm G A|\nB c|")),
        AbcErrorKind::UnexpectedSymbol,
    );
}

#[test]
fn s12_6_l3_grace_notes() {
    plays("L3", run(&tune("{ab}G A|")), &[(0.0, G), (0.25, A)]);
}

#[test]
fn s12_6_l4_decorations() {
    plays_pitches("L4", run(&tune("~G .A uB vc|")), &[G, A, B, C5]);
}

#[test]
fn s12_6_l5_two_decorations() {
    refused("L5", run(&tune("~.G A|")), AbcErrorKind::UnexpectedSymbol);
}

#[test]
fn s12_6_l6_letter_decoration() {
    let e = refused("L6", run(&tune("G Hc|")), AbcErrorKind::ColonExpected);
    at("L6", &e, 7, 4);
}

#[test]
fn s12_6_l7_bang_truncates() {
    let p = plays(
        "L7",
        run(&tune("G A !p!B c|\nd e|")),
        &[(0.0, G), (0.25, A)],
    );
    truncated("L7", &p);
}

#[test]
fn s12_6_l8_illegal_characters_truncate() {
    for body in ["G A $ B c|", "G A`B c|"] {
        let p = plays("L8", run(&tune(body)), &[(0.0, G), (0.25, A)]);
        truncated("L8", &p);
    }
}

#[test]
fn s12_6_l9_typographic_quotes() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(tune("\u{201C}G\u{201D} A|").as_bytes());
    let p = plays("L9", parse(&bytes, PartRequest::Unspecified), &[]);
    truncated("L9", &p);
}

#[test]
fn s12_6_l10_volume_marks() {
    let p = ok("L10", run(&tune("+p+G +F+A +FFF+B|")));
    let v: Vec<u8> = p.note_ons().map(|e| e.volume).collect();
    assert_eq!(v, vec![60, 105, 127], "L10");
}

#[test]
fn s12_6_l11_unknown_volume_mark() {
    refused(
        "L11",
        run(&tune("+fermata+G|")),
        AbcErrorKind::UnknownVolumeMark,
    );
}

#[test]
fn s12_6_l12_closing_plus() {
    // At the token where the closing plus should be: the space.
    let e = refused(
        "L12",
        run(&tune("+mf G|")),
        AbcErrorKind::ClosingPlusExpected,
    );
    at("L12", &e, 7, 4);
}

#[test]
fn s12_6_l13_y_letter() {
    let e = refused("L13", run(&tune("G y A|")), AbcErrorKind::ColonExpected);
    at("L13", &e, 7, 4);
}

#[test]
fn s12_6_l14_slur() {
    plays_pitches("L14", run(&tune("(G A) B|")), &[G, A, B]);
}

#[test]
fn s12_6_l15_broken_after_slur() {
    refused(
        "L15",
        run(&tune("(G A)>B c|")),
        AbcErrorKind::UnexpectedSymbol,
    );
}

#[test]
fn s12_6_l16_quote_then_space_first() {
    refused(
        "L16",
        run(&tune("\"Am\" G A|")),
        AbcErrorKind::UnexpectedSymbol,
    );
}

#[test]
fn s12_6_l17_quote_then_space_later() {
    plays_pitches("L17", run(&tune("G \"Am\" A|")), &[G, A]);
}

#[test]
fn s12_6_l18_unclosed_quote_later() {
    // §2.2: an unclosed quote after other music truncates, at the quote.
    let p = plays_pitches("L18", run(&tune("G \"Dm A|\nB c|")), &[G]);
    truncated_at("L18", &p, 7, 3);
    // The same with an unclosed grace group.
    let p = plays_pitches("L18 grace", run(&tune("G {Dm A|\nB c|")), &[G]);
    truncated_at("L18 grace", &p, 7, 3);
    // A closed quote with nothing after it: complete.
    let p = plays_pitches("L18 closed", run(&tune("G \"Dm\"")), &[G]);
    assert_eq!(p.outcome, ReadOutcome::Complete, "L18 closed");
}

#[test]
fn s12_6_l19_second_round_reads_decoration() {
    plays_pitches("L19", run(&tune("A ~.G|")), &[A, G]);
}

#[test]
fn s12_6_l20_grace_then_space() {
    refused("L20", run(&tune("{a} G|")), AbcErrorKind::UnexpectedSymbol);
    plays_pitches("L20", run(&tune("G {a} A|")), &[G, A]);
}

#[test]
fn s12_6_l21_decoration_then_line_break() {
    refused("L21", run(&tune("~\nG|")), AbcErrorKind::UnexpectedSymbol);
    plays_pitches("L21", run(&tune("G ~\nA|")), &[G, A]);
}

#[test]
fn s12_6_m1_repeat_played_once() {
    plays_pitches("M1", run(&tune("|: G A :| B c|")), &[G, A, B, C5]);
}

#[test]
fn s12_6_m2_endings() {
    plays_pitches("M2", run(&tune("|: G |1 A :|2 B |]")), &[G, A, B]);
}

#[test]
fn s12_6_m3_double_repeats() {
    plays_pitches("M3", run(&tune("|: G :: A :||: B :|")), &[G, A, B]);
}

#[test]
fn s12_6_m4_repeat_end_colon() {
    refused(
        "M4",
        run(&tune("|: G :|: A :|")),
        AbcErrorKind::UnexpectedSymbol,
    );
}

#[test]
fn s12_6_m5_repeat_end_bracket() {
    refused(
        "M5",
        run(&tune("|: G :|] A|")),
        AbcErrorKind::UnexpectedSymbol,
    );
}

#[test]
fn s12_6_m6_thick_thin_bracket() {
    refused("M6", run(&tune("G [|] A|")), AbcErrorKind::UnexpectedSymbol);
}

#[test]
fn s12_6_m7_thick_thin_colon() {
    refused(
        "M7",
        run(&tune("G [|: A :|")),
        AbcErrorKind::UnexpectedSymbol,
    );
}

#[test]
fn s12_6_m8_double_bar_colon() {
    refused(
        "M8",
        run(&tune("G ||: A :|")),
        AbcErrorKind::UnexpectedSymbol,
    );
}

#[test]
fn s12_6_m9_double_bar_one() {
    refused("M9", run(&tune("G ||1 A|")), AbcErrorKind::UnexpectedSymbol);
}

#[test]
fn s12_6_m10_bar_two() {
    refused(
        "M10",
        run(&tune("|: G :| |2 A|")),
        AbcErrorKind::UnexpectedSymbol,
    );
}

#[test]
fn s12_6_m11_chord_open_three() {
    refused("M11", run(&tune("G [3 A|")), AbcErrorKind::ChordNotClosed);
}

#[test]
fn s12_6_m12_ending_list() {
    refused(
        "M12",
        run(&tune("G [1,2 A|")),
        AbcErrorKind::UnexpectedSymbol,
    );
}

// ----- §12.7 Timing and events -----

/// Header for rows N2–N3c: M:4/4, L:1/4, Q:`q`.
fn n_tune(q: u32, body: &str) -> String {
    format!("X:1\nT:t\nM:4/4\nL:1/4\nQ:{q}\nK:C\n{body}\n")
}

#[test]
fn s12_7_n1_unit_note() {
    let p = ok("N1", run(&tune("G|")));
    assert_eq!(p.notes.len(), 1);
    assert!(close(p.notes[0].duration, 0.25), "N1");
    assert_pairs("N1", &offs(&p), &[(0.25, G)]);
}

#[test]
fn s12_7_n2_sixty_ms_boundary() {
    // §13.2: the tempos from 1 to 240 for which a note of length q/1000 is refused.
    const REFUSED: [u32; 34] = [
        9, 11, 13, 15, 18, 22, 26, 30, 36, 37, 43, 44, 45, 51, 52, 60, 72, 74, 86, 88, 90, 102,
        104, 120, 141, 144, 148, 172, 173, 176, 180, 204, 208, 240,
    ];
    for q in 1..=240u32 {
        let r = run(&n_tune(q, &format!("G{q}/1000|")));
        if REFUSED.contains(&q) {
            refused(&format!("N2 q={q}"), r, AbcErrorKind::NoteTooShort);
        } else {
            let p = ok(&format!("N2 q={q}"), r);
            assert_eq!(p.note_ons().count(), 1, "N2 q={q}");
        }
    }
}

#[test]
fn s12_7_n3_eight_seconds_accepted() {
    let p = ok("N3", run(&n_tune(60, "G8|")));
    assert_eq!(p.notes[0].duration, 8.0, "N3");
}

#[test]
fn s12_7_n3b_over_eight_seconds() {
    let e = refused("N3b", run(&n_tune(60, "G9|")), AbcErrorKind::NoteTooLong);
    at("N3b", &e, 7, 1);
}

#[test]
fn s12_7_n3c_tied_chain_over_eight_seconds() {
    let p = plays("N3c", run(&n_tune(60, "G8-G8|")), &[(0.0, G)]);
    assert_pairs("N3c", &offs(&p), &[(16.0, G)]);
}

#[test]
fn s12_7_n4_tempo_zero() {
    refused(
        "N4",
        run(&tune_with(&[("Q", "Q:0")], "G|")),
        AbcErrorKind::NoteTooLong,
    );
}

#[test]
fn s12_7_n5_meter_denominator_zero() {
    refused(
        "N5",
        run(&tune_with(&[("M", "M:4/0")], "G|")),
        AbcErrorKind::NoteTooShort,
    );
}

#[test]
fn s12_7_n6_unit_field_after_music() {
    plays_at(
        "N6",
        run(&tune("G A B c|\nL:1/4\nd e f g|")),
        &[0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5],
    );
}

#[test]
fn s12_7_o1_reattack() {
    let p = ok("O1", run(&tune("[G4z] G2 A|")));
    let got: Vec<(NoteEventKind, u8, f64)> = p
        .events
        .iter()
        .filter(|e| e.kind != NoteEventKind::End)
        .map(|e| (e.kind, e.pitch, e.time))
        .collect();
    let want = [
        (NoteEventKind::On, G, 0.0),
        (NoteEventKind::Off, G, 0.25),
        (NoteEventKind::On, G, 0.25),
        (NoteEventKind::Off, G, 0.75),
        (NoteEventKind::On, A, 0.75),
        (NoteEventKind::Off, A, 1.0),
    ];
    assert_eq!(got.len(), want.len(), "O1: {got:?}");
    for (g, w) in got.iter().zip(&want) {
        assert!(g.0 == w.0 && g.1 == w.1 && close(g.2, w.2), "O1: {got:?}");
    }
    let end: &NoteEvent = p.events.last().unwrap();
    assert_eq!(end.kind, NoteEventKind::End);
    assert_eq!(end.delta, 0.0);
}

#[test]
fn s12_7_o2_volumes() {
    let p = ok("O2", run(&tune("+pp+G +mp+A +ff+B|")));
    let v: Vec<u8> = p.note_ons().map(|e| e.volume).collect();
    assert_eq!(v, vec![45, 75, 120], "O2");
}

#[test]
fn s12_7_p1_ten_thousand_events() {
    let p = ok("P1", run(&tune(&"G".repeat(10_000))));
    assert_eq!(p.note_ons().count(), 10_000);
    assert!(close(p.end_time(), 2500.0), "P1: end {}", p.end_time());
}

#[test]
fn s12_7_p2_event_limit() {
    let e = refused(
        "P2",
        run(&tune(&"G".repeat(10_001))),
        AbcErrorKind::TooManyEvents,
    );
    assert!(
        e.message().is_none(),
        "P2: the event-limit notice has no table entry"
    );
    assert_eq!(
        e.position, None,
        "P2: the event-limit notice has no position"
    );
}

#[test]
fn s12_7_s1_solo_start() {
    let p = ok("S1", run(&tune("G A|")));
    let first = p.note_ons().next().unwrap().time;
    let start = Performance::start_offset(StartMode::Solo).unwrap();
    assert_eq!(start, SOLO_START_OFFSET);
    assert!(close(start + first, 0.7), "S1");
    assert_eq!(Performance::start_offset(StartMode::Synced), None);
}
