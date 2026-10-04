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
