// SPDX-License-Identifier: GPL-3.0-or-later
//! The `--walk-path` format (`apps/client/src/walk_path.rs`, `m4-5-visual.md` §2.1) and its
//! command line, through the public API: the file form's round trip at the field limits,
//! error lines, the byte-order mark, the bounded table split, inputs that the waypoint
//! steering would ignore, and the bounded read of the walk file.

use clap::Parser as _;

use tessera_client::Args;
use tessera_client::walk_path::{
    MAX_FIELD_LEN, WALK_TABLE_HEADER, parse_inputs, parse_walk_file, parse_walk_table,
};
use tessera_client::walker::movement::run::read_walks;

fn file_with_inputs(inputs: &str) -> String {
    format!(
        "id = T1\ncategory = wall\nregion = 1\nlandblock = 0xA197\ncell = 0\n\
         start = 127.81,89.59,389.26\nloc = r1 synthetic\nheading = 0.0\nmode = run\n\
         inputs = {inputs}\nduration = 3\nexpect = blocked P=127.81,94.77,389.26 d=0.10\n"
    )
}

/// Every accepted walk's file form parses back to the same walk.
fn round_trips(text: &str) {
    let w = parse_walk_file(text).expect("parses");
    let again = w.to_file_text();
    assert_eq!(parse_walk_file(&again).as_ref(), Ok(&w), "{again}");
}

#[test]
fn short_exponents_round_trip_at_the_field_limit() {
    // 256 timed tokens of `1e-20` (2,815 bytes): written back in the exponent form, not as
    // 0.00000000000000000001 (7,167 bytes, beyond the 4,096-byte field limit).
    let timed = vec!["1e-20:jump"; 256].join(";");
    assert!(timed.len() < MAX_FIELD_LEN);
    round_trips(&file_with_inputs(&timed));
    // The same with 256 waypoints (3,074 bytes).
    let wp = format!("wp={}", vec!["1e-20,1e-20"; 256].join(";"));
    assert!(wp.len() < MAX_FIELD_LEN);
    round_trips(&file_with_inputs(&wp));
}

#[test]
fn fields_that_grow_when_written_back_are_refused() {
    // `-.12345678901234567` is written `-0.12345678901234566`: 100 waypoints fit in the raw
    // field (about 4,000 bytes) but not when written back.
    let n = "-.12345678901234567";
    let wp = format!(
        "wp={}",
        (0..100)
            .map(|_| format!("{n},{n}"))
            .collect::<Vec<_>>()
            .join(";")
    );
    assert!(wp.len() <= MAX_FIELD_LEN, "{}", wp.len());
    let e = parse_walk_file(&file_with_inputs(&wp)).unwrap_err();
    assert!(e.message.contains("written back"), "{e}");
    assert_eq!(e.line, Some(10), "{e}");
    // Fewer of them are accepted and round-trip.
    let short = format!("wp={n},{n};{n},{n}");
    round_trips(&file_with_inputs(&short));
    round_trips(&file_with_inputs(".5:fwd;wp=.5,-.5"));
}

#[test]
fn every_field_error_names_its_line() {
    let base = file_with_inputs("0:fwd");
    for (from, to, line) in [
        ("category = wall", "category = walls", 2),
        ("region = 1", "region = 0", 3),
        ("landblock = 0xA197", "landblock = A197", 4),
        ("cell = 0", "cell = x", 5),
        ("start = 127.81,89.59,389.26", "start = 1,2", 6),
        ("loc = r1 synthetic", "loc = a|b", 7),
        ("heading = 0.0", "heading = inf", 8),
        ("mode = run", "mode = sprint", 9),
        ("inputs = 0:fwd", "inputs = 0:dance", 10),
        ("duration = 3", "duration = -1", 11),
        (
            "expect = blocked P=127.81,94.77,389.26 d=0.10",
            "expect = blocked P=1,2 d=0.1",
            12,
        ),
        (
            "expect = blocked P=127.81,94.77,389.26 d=0.10",
            "expect = blocked P=1,2,3 d=-1",
            12,
        ),
        ("id = T1", "id = T-1", 1),
    ] {
        assert!(base.contains(from), "{from}");
        let e = parse_walk_file(&base.replace(from, to)).unwrap_err();
        assert_eq!(e.line, Some(line), "{to}: {e}");
    }
}

#[test]
fn a_byte_order_mark_is_skipped() {
    let text = format!("\u{feff}{}", file_with_inputs("0:fwd"));
    assert!(parse_walk_file(&text).is_ok());
    // Only at the start.
    let inside = file_with_inputs("0:fwd").replace("mode = run", "\u{feff}mode = run");
    assert!(parse_walk_file(&inside).is_err());
}

#[test]
fn table_rows_are_split_with_bounded_memory() {
    let text = |row: &str| {
        format!(
            "<a id=\"s2-3\"></a>\n\n{WALK_TABLE_HEADER}\n|---|---|---|---|---|---|---|---|---|---|---|---|\n{row}\n"
        )
    };
    let pipes = "|".repeat(1 << 20);
    let e = parse_walk_table(&text(&pipes)).unwrap_err();
    assert_eq!(e.message, "expected 12 cells, got more");
    assert_eq!(e.line, Some(5));
    let e = parse_walk_table(&text("| a | b |")).unwrap_err();
    assert_eq!(e.message, "expected 12 cells, got 2");
    // A long delimiter row is still a delimiter row.
    let rows = format!(
        "|{}\n| WL1 | wall | 1 | 0xA197 | 0 | 1,2,3 | `r1` | 0 | run | `0:fwd` | 3 | `blocked P=1,2,3 d=0.1` |",
        "---|".repeat(40)
    );
    assert_eq!(parse_walk_table(&text(&rows)).expect("table").len(), 1);
    // A table row's field error names the row's line.
    let bad = "| WL1 | wall | 1 | 0xA197 | 0 | 1,2,3 | `r1` | 0 | sprint | `0:fwd` | 3 | `blocked P=1,2,3 d=0.1` |";
    assert_eq!(parse_walk_table(&text(bad)).unwrap_err().line, Some(5));
}

#[test]
fn stop_and_turn_are_refused_with_waypoints() {
    for inputs in ["wp=0,10;1:stop", "1:turn=90;wp=0,10", "wp=0,10;3,4;2:stop"] {
        let e = parse_inputs(inputs).unwrap_err();
        assert!(e.message.contains("wp="), "{inputs}: {e}");
    }
    // `fwd` and `jump` keep their meaning with waypoints.
    let i = parse_inputs("0:fwd;1:jump;wp=0,10").expect("inputs");
    assert_eq!((i.timed.len(), i.waypoints.len()), (2, 1));
    assert!(parse_inputs("0:fwd;1:turn=90;2:stop").is_ok());
}

#[cfg(unix)]
#[test]
fn walk_files_are_read_through_the_limit() {
    // A device reports length 0; it must not be read without bound.
    let e = read_walks(std::path::Path::new("/dev/zero")).unwrap_err();
    assert!(format!("{e:#}").contains("too large"), "{e:#}");
}

#[test]
fn walk_files_read_back() {
    let dir = std::env::temp_dir().join(format!("tessera-walk-path-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("w.txt");
    std::fs::write(&path, file_with_inputs("0:fwd")).unwrap();
    let walks = read_walks(&path).expect("walk file");
    assert_eq!(walks.len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn walk_path_needs_the_install() {
    assert!(Args::try_parse_from(["t", "--no-data", "--walk-path", "w"]).is_err());
    assert!(Args::try_parse_from(["t", "--walk-path", "w", "--no-data"]).is_err());
    let a = Args::try_parse_from(["t", "--walk-path", "w"]).expect("walk path");
    assert!(a.walker_headless());
}
