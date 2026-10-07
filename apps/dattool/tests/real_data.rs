// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-dattool` against a real, user-owned install (read-only). Skipped unless
//! `TESSERA_GAME_DIR` is set.
//!
//! M0 exit criteria 1 and 3 at the command line: `info <dir>` lists every archive with
//! kind, region, block size, version words and entry count, and a DID listed by `ls <archive>`
//! is fetched by `cat <dir> <did>` through routing from the same archive. The exhaustive
//! (every entry) routing check lives in `crates/dat/tests/real_data.rs`; here a sample per
//! archive goes through the binary. Nothing is written into the install (snapshots go to a
//! temporary directory); only counts are printed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;
use tessera_dat::{ArchiveSet, METADATA_DID_BASE, SetOptions};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() && Path::new(&d).is_dir() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping real-data test");
            None
        }
    }
}

fn tool() -> Command {
    let mut c = Command::cargo_bin("tessera-dattool").unwrap();
    c.env_remove("TESSERA_GAME_DIR").env("RUST_LOG", "error");
    c
}

fn json_of(c: &mut Command) -> Value {
    let a = c.assert().success();
    serde_json::from_slice(&a.get_output().stdout).expect("JSON output")
}

/// Criterion 1: one row per `client_*` archive with every required field.
#[test]
fn info_lists_every_archive_with_identity_and_versions() {
    let Some(dir) = game_dir() else { return };
    let dir_s = dir.to_str().expect("UTF-8 game dir");
    let on_disk: BTreeSet<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("client_") && (n.ends_with(".dat") || n.ends_with(".datx")))
        .collect();

    let v = json_of(tool().args(["--json", "info", dir_s]));
    let archives = v["archives"].as_array().expect("archives array");
    assert_eq!(archives.len(), on_disk.len(), "one row per client_* file");
    for a in archives {
        let name = a["archive"].as_str().unwrap();
        let file = Path::new(a["path"].as_str().unwrap())
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(on_disk.contains(&file), "{name}: {file}");
        let id = &a["identity"];
        let kind = id["kind"].as_str().unwrap();
        assert_ne!(kind, "?", "{name}: kind unknown");
        let h = &a["header"];
        assert!(h["block_size"].as_u64().unwrap() > 0, "{name}");
        assert!(h["engine_version"].is_u64(), "{name}");
        assert!(h["game_data_version"].is_u64(), "{name}");
        assert!(h["minor_version"].is_u64(), "{name}");
        assert!(a["stats"]["entries"].as_u64().unwrap() > 0, "{name}");
        assert!(a["stats"]["walk_error"].is_null(), "{name}");
        if kind == "cell" || kind == "map" {
            assert!(id["region"].is_u64(), "{name}: region missing");
            assert_eq!(id["region_mismatch"], false, "{name}");
        } else {
            assert!(id["region"].is_null(), "{name}");
        }
    }

    let a = tool().args(["info", dir_s]).assert().success();
    let text = String::from_utf8(a.get_output().stdout.clone()).unwrap();
    let header = text.lines().nth(1).expect("table header");
    for col in [
        "ARCHIVE", "KIND", "REGION", "BLOCK", "ENG", "DATA", "MINOR", "ENTRIES",
    ] {
        assert!(header.contains(col), "missing column {col}: {header}");
    }
    assert!(text.contains(&format!("total ({})", on_disk.len())));
    eprintln!("criterion 1: info lists {} archives", archives.len());
}

/// DIDs to try in one archive: the first, middle and last non-metadata entries plus the
/// first entry of every DID class (high byte) present.
fn picks(entries: &[u32]) -> Vec<u32> {
    let mut out = BTreeSet::new();
    if let (Some(first), Some(last)) = (entries.first(), entries.last()) {
        out.insert(*first);
        out.insert(*last);
        out.insert(entries[entries.len() / 2]);
    }
    let mut prev = None;
    for d in entries {
        if prev != Some(d >> 24) {
            out.insert(*d);
            prev = Some(d >> 24);
        }
    }
    out.into_iter().collect()
}

/// Criterion 3 through the binary: `ls <archive>` lists the DID, `cat <dir> <did>` fetches
/// it through routing from that same archive.
#[test]
fn ls_then_cat_through_routing() {
    let Some(dir) = game_dir() else { return };
    let dir_s = dir.to_str().expect("UTF-8 game dir");
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open set");
    let (mut checked, mut archives) = (0usize, 0usize);
    for m in set.members() {
        let label = m.id.to_string();
        let path = m.path.to_str().expect("UTF-8 path");
        let dids: Vec<u32> = m
            .archive
            .entries()
            .map(|e| e.expect("directory walk").did)
            .filter(|d| *d < METADATA_DID_BASE)
            .collect();
        let picked = picks(&dids);
        if picked.is_empty() {
            continue;
        }
        archives += 1;
        for did in picked {
            let hex = format!("{did:#010x}");
            // `ls` lists it.
            let listed = json_of(tool().args(["--json", "ls", path, "--from", &hex, "-n", "1"]));
            assert_eq!(
                listed[0]["did"].as_str(),
                Some(format!("{did:#010X}").as_str()),
                "{label}: ls"
            );
            // `cat <dir>` fetches it through routing from the same archive.
            let v = json_of(tool().args(["--json", "cat", dir_s, &hex, "--max-bytes", "16"]));
            assert_eq!(v["archive"].as_str(), Some(label.as_str()), "{label} {hex}");
            assert_eq!(v["routed"], true, "{label} {hex}: found by fallback");
            assert_eq!(v["stored"], listed[0]["stored"], "{label} {hex}");
            let payload = v["payload_len"].as_u64().unwrap();
            if v["compressed"] == false {
                assert_eq!(Some(payload), v["stored"].as_u64(), "{label} {hex}");
            }
            checked += 1;
        }
    }
    // The text form names the archive and says it was routed.
    let first = set
        .members()
        .iter()
        .find_map(|m| {
            m.archive
                .entries()
                .map(|e| e.unwrap())
                .find(|e| e.did < METADATA_DID_BASE)
                .map(|e| (m.id.to_string(), e.did))
        })
        .expect("some record");
    tool()
        .args(["cat", dir_s, &format!("{:x}", first.1), "--max-bytes", "16"])
        .assert()
        .success()
        .stdout(predicates::str::contains(format!(
            "in {} (routed)",
            first.0
        )));
    eprintln!(
        "criterion 3 (cli): {checked} DIDs from {archives} archives: ls → cat <dir> routed to the listing archive"
    );
    assert!(checked > 0);
}

/// M1 `diff`/`snapshot`: a snapshot of the install, diffed against the live install, shows
/// no differences. The snapshot goes to a temp dir, never into the install.
#[test]
fn snapshot_of_the_install_diffs_clean_against_it() {
    let Some(dir) = game_dir() else { return };
    let dir_s = dir.to_str().expect("UTF-8 game dir");
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("install.snap");
    let snap_s = snap.to_str().unwrap();

    let v = json_of(tool().args(["--json", "snapshot", dir_s, "-o", snap_s]));
    let records = v["records"].as_u64().unwrap();
    assert!(records > 0);
    assert_eq!(v["incomplete"].as_array().unwrap().len(), 0);

    let v = json_of(tool().args(["--json", "diff", "--summary", snap_s, dir_s]));
    assert_eq!(v["identical"], true, "{v}");
    assert_eq!(v["totals"]["unchanged"].as_u64().unwrap(), records);
    assert_eq!(v["a"]["origin"], "snapshot");
    eprintln!("snapshot/diff: {records} records, no differences");

    // Content hashing on the smallest archive keeps the test fast.
    let smallest = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("client_") && (n.ends_with(".dat") || n.ends_with(".datx"))
        })
        .min_by_key(|e| e.metadata().map_or(u64::MAX, |m| m.len()))
        .expect("an archive")
        .path();
    let small_s = smallest.to_str().unwrap();
    let snapc = tmp.path().join("small.snap");
    let snapc_s = snapc.to_str().unwrap();
    let v = json_of(tool().args(["--json", "snapshot", "--content", small_s, "-o", snapc_s]));
    assert_eq!(v["hashes"], true);
    assert_eq!(v["unreadable"], 0);
    let v = json_of(tool().args(["--json", "diff", "--content", "--summary", snapc_s, dir_s]));
    assert_eq!(v["identical"], true, "{v}");
    assert_eq!(
        v["compared"],
        serde_json::json!(["iteration", "size", "content"])
    );
}

/// M4.5 exit criterion 1 at the command line: `census` decodes every record of the world
/// kinds to its last byte (a 0x01 record with its embedded render mesh), and every
/// collision tree walks with valid keys (CO4): 0x66 records, cell physics meshes, the
/// levels of the embedded render meshes, the 0x06 mesh levels and the landblock terrain and
/// ceiling trees. Every host must be present with trees walked. Prints each count next to
/// the one the specs recorded for the reference install; a patched install may differ in
/// counts but not in failures.
#[test]
fn census_decodes_every_world_record() {
    let Some(dir) = game_dir() else { return };
    let dir_s = dir.to_str().expect("UTF-8 game dir");
    // (prefix, kind, records and items in ROADMAP.md M4.5 criterion 1)
    let expected: [(&str, u64, Option<u64>); 17] = [
        ("0x01", 1_057, None),
        ("0x02", 3_402, None),
        ("0x08", 11, None),
        ("0x0F", 7, Some(7)),
        ("0x11", 5, None),
        ("0x12", 5, None),
        ("0x15", 1, None),
        ("0x16", 5, Some(125)),
        ("0x17", 5, None),
        ("0x19", 5, Some(4_688)),
        ("0x1A", 963, None),
        ("0x2B", 510, None),
        ("0x30", 24_094, None),
        ("0x31", 13_781, None),
        ("0x32", 1, Some(32)),
        ("0x35", 1, None),
        ("0x66", 16_819, None),
    ];
    let v = json_of(tool().args(["--json", "--game-dir", dir_s, "census", dir_s]));
    assert_eq!(v["ok"], true, "census failures: {}", v["failures"]);
    let kinds = v["kinds"].as_array().expect("kinds");
    for (prefix, records, items) in expected {
        let row = kinds
            .iter()
            .find(|k| k["prefix"] == prefix)
            .unwrap_or_else(|| panic!("no {prefix} records"));
        let n = row["records"].as_u64().unwrap();
        assert_eq!(row["decoded"].as_u64(), Some(n), "{prefix}");
        assert_eq!(row["failed"].as_u64(), Some(0), "{prefix}");
        let got_items = row["items"].as_u64().unwrap();
        let same = n == records && items.is_none_or(|i| i == got_items);
        eprintln!(
            "{prefix} {}: {n} records, {got_items} {} (roadmap: {records}{}){}",
            row["kind"].as_str().unwrap_or(""),
            row["item_label"].as_str().unwrap_or(""),
            items.map_or(String::new(), |i| format!(", {i}")),
            if same { "" } else { " DIFFERS" }
        );
    }
    let records = |prefix: &str| {
        kinds
            .iter()
            .find(|k| k["prefix"] == prefix)
            .and_then(|k| k["records"].as_u64())
            .unwrap_or(0)
    };
    // (host, trees in `collision.md` §3.1, must equal the records of this prefix)
    let hosts = [
        ("0x66 collision meshes", 16_819, Some("0x66")),
        ("0x01 cell physics meshes", 1_010, None),
        ("0x01 embedded mesh levels", 1_280, None),
    ];
    check_trees(&v, &hosts, records);
    let embedded = tree(&v, "0x01 embedded mesh levels");
    eprintln!(
        "0x01 embedded mesh levels: {} renumbered (collision.md §3.5: 3), {} without code (83)",
        embedded["renumbered"], embedded["without_code"]
    );

    // The mesh levels of the 0x06 records.
    let v = json_of(tool().args([
        "--json",
        "--game-dir",
        dir_s,
        "census",
        dir_s,
        "--prefix",
        "06",
    ]));
    assert_eq!(v["ok"], true, "census failures: {}", v["failures"]);
    let row = &v["kinds"][0];
    assert_eq!(row["decoded"], row["records"], "{row}");
    eprintln!("0x06 mesh: {} records (mesh.md: 47,563)", row["records"]);
    check_trees(&v, &[("0x06 mesh levels", 70_956, None)], |_| 0);
    let levels = tree(&v, "0x06 mesh levels");
    eprintln!(
        "0x06 mesh levels: {} renumbered (collision.md §3.5: 12), {} without code (211)",
        levels["renumbered"], levels["without_code"]
    );

    // The landblock family: terrain and ceiling trees, light maps.
    let v = json_of(tool().args([
        "--json",
        "--game-dir",
        dir_s,
        "census",
        dir_s,
        "--prefix",
        "80",
    ]));
    assert_eq!(v["ok"], true, "census failures: {}", v["failures"]);
    let hosts = [
        ("0x80 terrain trees", 58_546, None),
        ("0x80 ceiling trees", 1_095, None),
    ];
    check_trees(&v, &hosts, |_| 0);
}

/// The `trees` row of `host` in a census report.
fn tree(v: &serde_json::Value, host: &str) -> serde_json::Value {
    v["trees"]
        .as_array()
        .expect("trees")
        .iter()
        .find(|t| t["host"] == host)
        .unwrap_or_else(|| panic!("census walked no {host}: {}", v["trees"]))
        .clone()
}

/// Every tree row has no failure; every expected host is present with trees walked, as
/// many as its prefix has records where one is given; each count is printed next to the
/// spec's.
fn check_trees(
    v: &serde_json::Value,
    hosts: &[(&str, u64, Option<&str>)],
    records: impl Fn(&str) -> u64,
) {
    for t in v["trees"].as_array().expect("trees") {
        assert_eq!(t["failed"], 0, "{t}");
        assert_eq!(t["keys_out_of_range"], 0, "{t}");
    }
    for &(host, spec, prefix) in hosts {
        let walked = tree(v, host)["walked"].as_u64().unwrap_or(0);
        assert!(walked > 0, "no {host} walked");
        if let Some(p) = prefix {
            assert_eq!(walked, records(p), "{host}: one tree per {p} record");
        }
        eprintln!(
            "{host}: {walked} walked (collision.md §3.1: {spec}){}",
            if walked == spec { "" } else { " DIFFERS" }
        );
    }
}
