// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end tests of `tessera-dattool diff` and `snapshot` against synthetic archives
//! produced by the project's own spec-conformant test writer. No game data is involved.

#[allow(dead_code, clippy::all)]
#[path = "../../../crates/dat/tests/common/mod.rs"]
mod common;

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use common::{Builder, Built};
use flate2::Compression;
use predicates::prelude::*;
use serde_json::Value;

const SURFACE_DID: u32 = 0x4100_0001;

struct Fx {
    _dir: tempfile::TempDir,
    a: PathBuf,
    b: PathBuf,
    out: PathBuf,
}

/// Install A, and install B with these differences in `general`:
/// - 0x01000003 removed, 0x01000100 added,
/// - 0x01000005 payload grew (size change),
/// - 0x01000007 iteration bumped (same bytes),
/// - 0x01000009 same length and iteration, different bytes (content-only change).
///
/// `surface` is identical on both sides.
fn fixture() -> Fx {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_owned();
    let (a, b, out) = (root.join("a"), root.join("b"), root.join("out"));
    for d in [&a, &b, &out] {
        std::fs::create_dir_all(d).unwrap();
    }

    let general = |newer: bool| {
        let mut g = Builder::new();
        g.fanout = 4;
        for i in 0..20u32 {
            let did = 0x0100_0000 + i;
            if newer && did == 0x0100_0003 {
                continue;
            }
            let mut data = common::payload(did, 40 + i as usize);
            if newer && did == 0x0100_0005 {
                data.extend_from_slice(b"grown");
            }
            if newer && did == 0x0100_0009 {
                for x in &mut data {
                    *x ^= 0x5A;
                }
            }
            g.stored(did, data);
            if newer && did == 0x0100_0007 {
                g.records.last_mut().unwrap().iteration += 1;
            }
        }
        if newer {
            g.stored(0x0100_0100, common::payload(0x0100_0100, 33));
        }
        g.build().bytes
    };
    let surface = || {
        let mut s = Builder::new();
        s.dataset = 0x0109_0000;
        s.membership = Some(([7; 16], 0, 1));
        s.compressed(
            SURFACE_DID,
            &common::payload(SURFACE_DID, 300),
            Compression::default(),
        );
        s.build().bytes
    };
    std::fs::write(a.join("client_general.dat"), general(false)).unwrap();
    std::fs::write(b.join("client_general.dat"), general(true)).unwrap();
    std::fs::write(a.join("client_surface.dat"), surface()).unwrap();
    std::fs::write(b.join("client_surface.dat"), surface()).unwrap();
    Fx {
        _dir: dir,
        a,
        b,
        out,
    }
}

fn tool() -> Command {
    let mut c = Command::cargo_bin("tessera-dattool").unwrap();
    c.env_remove("TESSERA_GAME_DIR").env("RUST_LOG", "error");
    c
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn json_of(c: &mut Command) -> Value {
    let a = c.assert().success();
    serde_json::from_slice(&a.get_output().stdout).expect("JSON output")
}

fn dids(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x["did"].as_str().unwrap().to_owned())
        .collect()
}

fn general(v: &Value) -> &Value {
    v["archives"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["archive"] == "general")
        .expect("general archive in report")
}

/// The expected metadata-only differences.
fn check_metadata_diff(v: &Value) {
    assert_eq!(v["totals"]["added"], 1);
    assert_eq!(v["totals"]["removed"], 1);
    assert_eq!(v["totals"]["changed"], 2);
    let g = general(v);
    assert_eq!(dids(&g["added_dids"]), ["0x01000100"]);
    assert_eq!(dids(&g["removed_dids"]), ["0x01000003"]);
    assert_eq!(dids(&g["changed_dids"]), ["0x01000005", "0x01000007"]);
    assert_eq!(g["changed_dids"][0]["reasons"], serde_json::json!(["size"]));
    assert_eq!(
        g["changed_dids"][1]["reasons"],
        serde_json::json!(["iteration"])
    );
    let surf = v["archives"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["archive"] == "surface")
        .unwrap();
    // The surface record plus the set-membership record the writer adds.
    assert_eq!(surf["unchanged"], 2);
    assert_eq!(surf["status"], "both");
    let types = v["by_type"].as_array().unwrap();
    assert_eq!(types.len(), 1);
    assert_eq!(types[0]["prefix"], "0x01");
    assert_eq!(v["identical"], false);
}

#[test]
fn live_diff_reports_added_removed_changed() {
    let f = fixture();
    let v = json_of(tool().args(["--json", "diff", s(&f.a), s(&f.b)]));
    check_metadata_diff(&v);
    assert_eq!(v["compared"], serde_json::json!(["iteration", "size"]));
    // 21 records (20 + set membership) - 1 removed - 2 changed.
    assert_eq!(general(&v)["unchanged"], 18);

    // --content also catches the same-size, same-iteration rewrite.
    let v = json_of(tool().args(["--json", "diff", "--content", s(&f.a), s(&f.b)]));
    assert_eq!(v["totals"]["changed"], 3);
    let g = general(&v);
    assert_eq!(
        dids(&g["changed_dids"]),
        ["0x01000005", "0x01000007", "0x01000009"]
    );
    assert_eq!(
        g["changed_dids"][2]["reasons"],
        serde_json::json!(["content"])
    );
    assert!(g["changed_dids"][2]["a"]["hash"].is_string());
    assert_eq!(
        v["compared"],
        serde_json::json!(["iteration", "size", "content"])
    );
}

#[test]
fn text_output_and_exit_code() {
    let f = fixture();
    let a = tool()
        .args(["diff", s(&f.a), s(&f.b), "--max-list", "1"])
        .assert()
        .success();
    let out = String::from_utf8(a.get_output().stdout.clone()).unwrap();
    assert!(out.contains("by DID type:"), "{out}");
    assert!(out.contains("+ 0x01000100"), "{out}");
    assert!(out.contains("- 0x01000003"), "{out}");
    assert!(out.contains("~ 0x01000005"), "{out}");
    assert!(out.contains("size "), "{out}");
    assert!(out.contains("1 more changed"), "{out}");
    assert!(out.lines().any(|l| l.starts_with("TOTAL")), "{out}");

    tool()
        .args(["diff", s(&f.a), s(&f.b), "--exit-code", "--summary"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("0x01000100").not());
    tool()
        .args(["diff", s(&f.a), s(&f.a), "--exit-code"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no differences"));
}

#[test]
fn snapshot_stands_in_for_an_install() {
    let f = fixture();
    let snap = f.out.join("a.snap");
    let v = json_of(tool().args(["--json", "snapshot", s(&f.a), "-o", s(&snap)]));
    assert_eq!(v["archives"], 2);
    // 20 + 1 records plus one set-membership record per archive.
    assert_eq!(v["records"], 23);
    assert_eq!(v["hashes"], false);

    // snapshot vs live == live vs live.
    let v = json_of(tool().args(["--json", "diff", s(&snap), s(&f.b)]));
    check_metadata_diff(&v);
    assert_eq!(v["a"]["origin"], "snapshot");
    assert_eq!(v["b"]["origin"], "install");

    // A snapshot of the same install shows no differences, in either position.
    let v = json_of(tool().args(["--json", "diff", s(&snap), s(&f.a)]));
    assert_eq!(v["identical"], true);
    let v = json_of(tool().args(["--json", "diff", s(&f.a), s(&snap)]));
    assert_eq!(v["identical"], true);

    // Without hashes in the snapshot, --content cannot see the content-only change.
    let v = json_of(tool().args(["--json", "diff", "--content", s(&snap), s(&f.b)]));
    assert_eq!(v["totals"]["changed"], 2);
    assert_eq!(v["compared"], serde_json::json!(["iteration", "size"]));

    // A hashed snapshot can.
    let snapc = f.out.join("a-content.snap");
    tool()
        .args(["snapshot", "--content", s(&f.a), "-o", s(&snapc)])
        .assert()
        .success()
        .stdout(predicate::str::contains("content hashes"));
    let v = json_of(tool().args(["--json", "diff", "--content", s(&snapc), s(&f.b)]));
    assert_eq!(v["totals"]["changed"], 3);
    // Two hashed snapshots compare without any archive present.
    let snapb = f.out.join("b-content.snap");
    tool()
        .args(["snapshot", "--content", s(&f.b), "-o", s(&snapb)])
        .assert()
        .success();
    let v = json_of(tool().args(["--json", "diff", "--content", s(&snapc), s(&snapb)]));
    assert_eq!(v["totals"]["changed"], 3);
    assert_eq!(v["totals"]["added"], 1);
}

#[test]
fn snapshot_refuses_game_dir_and_existing_files() {
    let f = fixture();
    tool()
        .args(["snapshot", s(&f.a), "-o", s(&f.a.join("x.snap"))])
        .assert()
        .failure()
        .stderr(predicate::str::contains("game directory"));
    assert!(!f.a.join("x.snap").exists());
    let snap = f.out.join("s.snap");
    tool()
        .args(["snapshot", s(&f.a), "-o", s(&snap)])
        .assert()
        .success();
    tool()
        .args(["snapshot", s(&f.a), "-o", s(&snap)])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--force"));
    tool()
        .args(["snapshot", s(&f.a), "-o", s(&snap), "--force"])
        .assert()
        .success();
}

#[test]
fn single_archives_and_prefix_filter() {
    let f = fixture();
    let ga = f.a.join("client_general.dat");
    let gb = f.b.join("client_general.dat");
    // Two archive files.
    let v = json_of(tool().args(["--json", "diff", s(&ga), s(&gb)]));
    check_metadata_diff_general_only(&v);
    // An archive file against a whole install: compared with the same-named member.
    let v = json_of(tool().args(["--json", "diff", s(&ga), s(&f.b)]));
    check_metadata_diff_general_only(&v);
    // A snapshot of one archive.
    let snap = f.out.join("g.snap");
    tool()
        .args(["snapshot", s(&ga), "-o", s(&snap)])
        .assert()
        .success();
    let v = json_of(tool().args(["--json", "diff", s(&snap), s(&gb)]));
    check_metadata_diff_general_only(&v);
    // The prefix filter hides everything outside 0x41.
    let v = json_of(tool().args(["--json", "diff", "--prefix", "0x41", s(&f.a), s(&f.b)]));
    assert_eq!(v["identical"], true);
    assert_eq!(v["prefix"], "0x41");
}

fn check_metadata_diff_general_only(v: &Value) {
    assert_eq!(v["archives"].as_array().unwrap().len(), 1);
    assert_eq!(v["totals"]["added"], 1);
    assert_eq!(v["totals"]["removed"], 1);
    assert_eq!(v["totals"]["changed"], 2);
}

#[test]
fn corrupt_snapshots_fail_cleanly() {
    let f = fixture();
    let snap = f.out.join("s.snap");
    tool()
        .args(["snapshot", s(&f.a), "-o", s(&snap)])
        .assert()
        .success();
    let mut bytes = std::fs::read(&snap).unwrap();
    let n = bytes.len();
    bytes[n - 3] ^= 0xFF; // breaks the zlib stream / its checksum
    let bad = f.out.join("bad.snap");
    std::fs::write(&bad, &bytes).unwrap();
    tool()
        .args(["diff", s(&bad), s(&f.a)])
        .assert()
        .failure()
        .stderr(predicate::str::contains("snapshot"));
    let trunc = f.out.join("trunc.snap");
    std::fs::write(&trunc, &bytes[..14]).unwrap();
    tool()
        .args(["diff", s(&trunc), s(&f.a)])
        .assert()
        .failure()
        .stderr(predicate::str::contains("snapshot"));
    // A file that is neither a snapshot nor an archive.
    let junk = f.out.join("junk.bin");
    std::fs::write(&junk, b"hello").unwrap();
    tool().args(["diff", s(&junk), s(&f.a)]).assert().failure();
    // Bytes after the zlib stream make a snapshot invalid.
    let mut trail = std::fs::read(&snap).unwrap();
    trail.extend_from_slice(b"GARBAGE-APPENDED-AFTER-STREAM");
    let t = f.out.join("trail.snap");
    std::fs::write(&t, &trail).unwrap();
    tool()
        .args(["diff", s(&t), s(&f.a)])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("after the zlib stream"));
}

#[test]
fn errors_exit_2_and_differences_exit_1() {
    let f = fixture();
    tool()
        .args([
            "diff",
            s(&f.out.join("nonexistent.snap")),
            s(&f.a),
            "--exit-code",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("error:"));
    tool()
        .args(["diff", s(&f.a), s(&f.b), "--exit-code", "--summary"])
        .assert()
        .code(1);
    tool()
        .args(["diff", s(&f.a), s(&f.a), "--exit-code"])
        .assert()
        .code(0);
}

#[test]
fn hashed_snapshots_compare_content_without_the_flag() {
    let f = fixture();
    let (sa, sb) = (f.out.join("a.snap"), f.out.join("b.snap"));
    for (src, dst) in [(&f.a, &sa), (&f.b, &sb)] {
        tool()
            .args(["snapshot", "--content", s(src), "-o", s(dst)])
            .assert()
            .success();
    }
    let v = json_of(tool().args(["--json", "diff", s(&sa), s(&sb)]));
    assert_eq!(v["totals"]["changed"], 3);
    assert_eq!(
        v["compared"],
        serde_json::json!(["iteration", "size", "content"])
    );
    // A snapshot of the same install with only the content-only change: still caught.
    let v = json_of(tool().args(["--json", "diff", s(&sa), s(&sa)]));
    assert_eq!(v["identical"], true);
}

#[test]
fn content_hashing_is_limited_to_what_can_be_paired() {
    let f = fixture();
    let ga = f.a.join("client_general.dat");
    // A single archive against a whole install: only `general` is hashed on the install
    // side, and the content-only change is still found.
    let v = json_of(tool().args(["--json", "diff", "--content", s(&ga), s(&f.b)]));
    assert_eq!(v["totals"]["changed"], 3);
    assert_eq!(v["b"]["hashes"], true);
    // With a prefix that excludes every changed DID, nothing is reported.
    let v = json_of(tool().args([
        "--json",
        "diff",
        "--content",
        "--prefix",
        "0x41",
        s(&f.a),
        s(&f.b),
    ]));
    assert_eq!(v["identical"], true);
    assert_eq!(v["b"]["unreadable"], 0);
}

#[test]
fn housekeeping_records_are_not_moves() {
    let f = fixture();
    // Two installs holding different archives that both carry membership records.
    let (x, y) = (f.out.join("x"), f.out.join("y"));
    for (dir, name) in [(&x, "client_cell_15.dat"), (&y, "client_map_15.dat")] {
        std::fs::create_dir_all(dir).unwrap();
        let mut b = Builder::new();
        b.dataset = 0x0203_0000;
        b.subset = 15;
        b.membership = Some(([3; 16], 0, 1));
        b.stored(0x0100_0001, common::payload(0x0100_0001, 32));
        std::fs::write(dir.join(name), b.build().bytes).unwrap();
    }
    let v = json_of(tool().args(["--json", "diff", s(&x), s(&y)]));
    let moved = v["moved"].as_array().unwrap();
    assert!(
        moved
            .iter()
            .all(|m| !m["did"].as_str().unwrap().starts_with("0xFFFF")),
        "{moved:?}"
    );
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert_eq!(moved[0]["did"], "0x01000001");
}

#[test]
fn an_incomplete_walk_is_trouble_not_a_difference() {
    let f = fixture();
    // A copy of A's general whose root's first child pointer lies past the end.
    let mut g = Builder::new();
    g.fanout = 4;
    for i in 0..20u32 {
        let did = 0x0100_0000 + i;
        g.stored(did, common::payload(did, 40 + i as usize));
    }
    let mut built: Built = g.build();
    let (root_off, _) = built.root();
    built.put_u32(Built::child_ptr_pos(root_off, 0), 0x7FFF_FF00);
    let bad = f.out.join("bad");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("client_general.dat"), &built.bytes).unwrap();
    let good = f.a.join("client_general.dat");
    let bad_file = bad.join("client_general.dat");

    // Without --exit-code too: status 2, the walk error printed, nothing called removed.
    let a = tool()
        .args(["diff", s(&good), s(&bad_file), "--summary"])
        .assert()
        .code(2);
    let out = String::from_utf8(a.get_output().stdout.clone()).unwrap();
    assert!(out.contains("directory walk of B stopped early"), "{out}");
    assert!(out.contains("not compared"), "{out}");
    let a = tool()
        .args(["--json", "diff", s(&good), s(&bad_file), "--exit-code"])
        .assert()
        .code(2);
    let v: Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["incomplete"], true);
    assert_eq!(v["totals"]["removed"], 0);
    assert!(v["totals"]["not_compared"].as_u64().unwrap() > 0);
    assert!(v["archives"][0]["b_walk_error"].is_string());
    // Both sides broken the same way: not "no differences" with status 0.
    tool()
        .args(["diff", s(&bad_file), s(&bad_file), "--exit-code"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains(
            "no differences among the records compared",
        ));
}
