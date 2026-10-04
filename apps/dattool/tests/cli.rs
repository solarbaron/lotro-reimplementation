// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end tests of the `tessera-dattool` binary against synthetic archives.
//!
//! The archives are produced by the project's own spec-conformant test writer (shared with
//! `crates/dat/tests`); the record payloads are generated here. No game data is involved.

#[allow(dead_code, clippy::all)]
#[path = "../../../crates/dat/tests/common/mod.rs"]
mod common;

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use common::{Builder, Built};
use flate2::Compression;
use predicates::prelude::*;
use tessera_formats::reader::write_vle;

const SURFACE_DID: u32 = 0x4100_0001;
const SURFACE_Z_DID: u32 = 0x4100_0002;
const TABLE_DID: u32 = 0x2500_0007;

fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn utf16(v: &mut Vec<u8>, s: &str) {
    let units: Vec<u16> = s.encode_utf16().collect();
    write_vle(v, units.len() as u32);
    for u in units {
        v.extend_from_slice(&u.to_le_bytes());
    }
}

/// A 2x2 A8R8G8B8 render surface (spec records §5).
fn surface(did: u32) -> Vec<u8> {
    let mut v = Vec::new();
    u32le(&mut v, did);
    u32le(&mut v, 0); // category
    u32le(&mut v, 2); // width
    u32le(&mut v, 2); // height
    u32le(&mut v, 21); // A8R8G8B8
    u32le(&mut v, 16);
    v.extend_from_slice(&[
        0x00, 0x00, 0xFF, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0x80, 0x80, 0x80,
        0x80,
    ]);
    v
}

/// A string table with a plain entry and one with a named argument (spec records §3).
fn string_table(did: u32) -> Vec<u8> {
    let mut v = Vec::new();
    u32le(&mut v, did);
    u32le(&mut v, 1); // flag
    v.push(0); // TSize hint
    write_vle(&mut v, 2); // count
    // Entry 1: "Plain text".
    u32le(&mut v, 0x11);
    u32le(&mut v, 0);
    u32le(&mut v, 1);
    utf16(&mut v, "Plain text");
    u32le(&mut v, 0);
    v.push(0);
    // Entry 2: "Hello " {NAME} "!".
    u32le(&mut v, 0x22);
    u32le(&mut v, 0);
    u32le(&mut v, 2);
    utf16(&mut v, "Hello ");
    utf16(&mut v, "!\nBye");
    u32le(&mut v, 1);
    u32le(&mut v, 0x1234);
    v.push(1);
    u32le(&mut v, 1);
    utf16(&mut v, "NAME");
    v
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    game: PathBuf,
    out: PathBuf,
    general: Built,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_owned();
    let game = root.join("game");
    let out = root.join("out");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::create_dir_all(&out).unwrap();

    let mut general = Builder::new();
    general.fanout = 4;
    for i in 0..20u32 {
        general.stored(
            0x0100_0000 + i,
            common::payload(0x0100_0000 + i, 40 + i as usize),
        );
    }
    general.stored(TABLE_DID, string_table(TABLE_DID));
    let general_built = general.build();
    std::fs::write(game.join("client_general.dat"), &general_built.bytes).unwrap();

    let mut surf = Builder::new();
    surf.dataset = 0x0109_0000;
    surf.membership = Some(([7; 16], 0, 1));
    surf.stored(SURFACE_DID, surface(SURFACE_DID));
    surf.compressed(
        SURFACE_Z_DID,
        &surface(SURFACE_Z_DID),
        Compression::default(),
    );
    std::fs::write(game.join("client_surface.dat"), surf.build().bytes).unwrap();

    Fixture {
        _dir: dir,
        root,
        game,
        out,
        general: general_built,
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

#[test]
fn help_lists_every_subcommand() {
    let a = tool().arg("--help").assert().success();
    let out = String::from_utf8(a.get_output().stdout.clone()).unwrap();
    for cmd in [
        "detect", "info", "ls", "stat", "cat", "extract", "validate", "strings",
    ] {
        assert!(out.contains(cmd), "missing {cmd} in help");
    }
}

#[test]
fn bad_hex_is_a_usage_error() {
    tool()
        .args(["cat", "/nonexistent", "0xnothex"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("hex"));
}

#[test]
fn info_on_dir_and_file() {
    let f = fixture();
    tool()
        .args(["info", s(&f.game)])
        .assert()
        .success()
        .stdout(predicate::str::contains("general"))
        .stdout(predicate::str::contains("surface"))
        .stdout(predicate::str::contains("total (2)"));
    tool()
        .args(["info", s(&f.game.join("client_general.dat"))])
        .assert()
        .success()
        .stdout(predicate::str::contains("22 (1 metadata)"))
        .stdout(predicate::str::contains("engine version"));
    let a = tool()
        .args(["--json", "info", s(&f.game.join("client_surface.dat"))])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["stats"]["entries"], 3);
    assert_eq!(v["stats"]["compressed"], 1);
    assert_eq!(v["header"]["dataset"], 0x0109_0000);
}

#[test]
fn ls_filters_and_resolves_names() {
    let f = fixture();
    tool()
        .args(["--game-dir", s(&f.game), "ls", "general", "--prefix", "25"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0x25000007"))
        .stdout(predicate::str::contains("0x01000000").not())
        .stdout(predicate::str::contains("-- 1 entries"));
    let a = tool()
        .args([
            "--json",
            "ls",
            s(&f.game.join("client_general.dat")),
            "--prefix",
            "0x01",
            "--from",
            "01000005",
            "-n",
            "3",
        ])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    let dids: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["did"].as_str().unwrap())
        .collect();
    assert_eq!(dids, ["0x01000005", "0x01000006", "0x01000007"]);
    tool()
        .args(["--game-dir", s(&f.game), "ls", "mesh"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no archive of that name"));
}

#[test]
fn stat_counts_prefixes() {
    let f = fixture();
    let a = tool()
        .args(["--json", "stat", s(&f.game)])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["entries"], 25);
    let totals = v["totals"].as_array().unwrap();
    let count = |p: &str| {
        totals
            .iter()
            .find(|b| b["prefix"] == p)
            .map(|b| b["count"].as_u64().unwrap())
    };
    assert_eq!(count("0x01"), Some(20));
    assert_eq!(count("0x25"), Some(1));
    assert_eq!(count("0x41"), Some(2));
    assert_eq!(count("0xFF"), Some(2));
    tool()
        .args(["stat", s(&f.game)])
        .assert()
        .success()
        .stdout(predicate::str::contains("render surface"))
        .stdout(predicate::str::contains("== all 2 archives: 25 entries"));
}

#[test]
fn cat_hexdumps_and_raw_writes_bytes() {
    let f = fixture();
    tool()
        .args(["cat", s(&f.game), "41000002"])
        .assert()
        .success()
        .stdout(predicate::str::contains("in surface"))
        .stdout(predicate::str::contains("(zlib)"))
        .stdout(predicate::str::contains("render surface 2x2 A8R8G8B8"))
        .stdout(predicate::str::contains("00000000  02 00 00 41"));
    let a = tool()
        .args(["cat", s(&f.game), "0x41000002", "--raw"])
        .assert()
        .success();
    assert_eq!(a.get_output().stdout, surface(SURFACE_Z_DID));
    tool()
        .args(["cat", s(&f.game), "0x41000003"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn extract_png_text_and_payload() {
    let f = fixture();
    let png = f.out.join("s.png");
    tool()
        .args(["extract", s(&f.game), "41000002", "-o", s(&png), "--png"])
        .assert()
        .success();
    let bytes = std::fs::read(&png).unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let dec = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut r = dec.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    assert_eq!((info.width, info.height), (2, 2));
    // Stored BGRA 00 00 FF FF -> RGBA FF 00 00 FF (opaque red).
    assert_eq!(&buf[..4], &[0xFF, 0x00, 0x00, 0xFF]);

    let txt = f.out.join("nested/t.txt");
    tool()
        .args(["extract", s(&f.game), "25000007", "-o", s(&txt), "--text"])
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(&txt).unwrap(),
        "0x00000011\tPlain text\n0x00000022\tHello {NAME}!\\nBye\n"
    );

    let bin = f.out.join("p.bin");
    tool()
        .args(["extract", s(&f.game), "41000001", "-o", s(&bin)])
        .assert()
        .success();
    assert_eq!(std::fs::read(&bin).unwrap(), surface(SURFACE_DID));
    // No silent overwrite.
    tool()
        .args(["extract", s(&f.game), "41000001", "-o", s(&bin)])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--force"));
    tool()
        .args(["extract", s(&f.game), "41000001", "-o", s(&bin), "--force"])
        .assert()
        .success();
    // Wrong record type for the conversion.
    tool()
        .args(["extract", s(&f.game), "25000007", "-o", s(&png), "--png"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("render surface"));
}

#[test]
fn extract_never_writes_into_the_game_dir() {
    let f = fixture();
    let count = || std::fs::read_dir(&f.game).unwrap().count();
    let before = count();
    for target in [
        f.game.join("x.bin"),
        f.game.join("sub/x.bin"),
        f.out.join("../game/x.bin"),
    ] {
        tool()
            .args(["extract", s(&f.game), "41000001", "-o", s(&target)])
            .assert()
            .failure()
            .stderr(predicate::str::contains("inside the game directory"));
    }
    // Also when reading a single archive file from that directory.
    tool()
        .args([
            "extract",
            s(&f.game.join("client_surface.dat")),
            "41000001",
            "-o",
            s(&f.game.join("y.bin")),
        ])
        .assert()
        .failure();
    assert_eq!(before, count());
    assert!(!f.game.join("sub").exists());
    assert!(f.root.join("out").exists());
}

#[test]
fn strings_render_arguments() {
    let f = fixture();
    tool()
        .args(["strings", s(&f.game), "0x25000007"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 entries"))
        .stdout(predicate::str::contains("0x00000022  Hello {NAME}!\\nBye"));
    let a = tool()
        .args(["--json", "strings", s(&f.game), "25000007", "--token", "11"])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["entries"][0]["text"], "Plain text");
    assert_eq!(v["entries"].as_array().unwrap().len(), 1);
    tool()
        .args(["strings", s(&f.game), "41000001"])
        .assert()
        .failure();
}

#[test]
fn validate_passes_clean_and_fails_corrupt() {
    let f = fixture();
    tool()
        .args(["validate", s(&f.game)])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 errors"))
        .stdout(predicate::str::contains("OK"));

    // Point one record's first extent past the end of the file.
    let mut corrupt = f.general.clone();
    let (root_off, _) = corrupt.root();
    let pos = Built::entry_pos(root_off, 0) + 8;
    corrupt.put_u32(pos, 0x7FFF_FF00);
    let bad = f.root.join("bad");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("client_general.dat"), &corrupt.bytes).unwrap();
    let a = tool()
        .args(["--json", "validate", s(&bad)])
        .assert()
        .failure();
    let v: serde_json::Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert!(v["errors"].as_u64().unwrap() > 0);
}

#[test]
fn detect_with_an_unusable_dir_fails_cleanly() {
    let f = fixture();
    tool()
        .args(["--game-dir", s(&f.out), "detect"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a usable install"));
}

#[test]
fn detect_reports_a_synthetic_install() {
    let f = fixture();
    for (name, dataset, subset) in [
        ("client_gamelogic.dat", 0x0101_0000u32, 0u32),
        ("client_local_English.dat", 0x0303_0000, 2),
    ] {
        let mut b = Builder::new();
        b.dataset = dataset;
        b.subset = subset;
        b.stored(0x0700_0001, common::payload(0x0700_0001, 64));
        std::fs::write(f.game.join(name), b.build().bytes).unwrap();
    }
    tool()
        .args(["--game-dir", s(&f.game), "detect"])
        .assert()
        .success()
        .stdout(predicate::str::contains("explicit --game-dir"))
        .stdout(predicate::str::contains("English"))
        .stdout(predicate::str::contains("schema fingerprint"))
        .stdout(predicate::str::contains("content fingerprint"));
    let a = tool()
        .args([
            "--json",
            "--game-dir",
            s(&f.game),
            "detect",
            "--schema-only",
        ])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["installs"].as_array().unwrap().len(), 1);
    assert!(v["installs"][0]["content_fingerprint"].is_null());
    assert_eq!(
        v["installs"][0]["schema_fingerprint"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
}

/// Read-only smoke test against a real install; skipped unless `TESSERA_GAME_DIR` is set.
#[test]
fn real_install_smoke() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|v| !v.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let dir = PathBuf::from(dir);
    let a = tool().args(["--json", "info", s(&dir)]).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert!(!v["archives"].as_array().unwrap().is_empty());
    tool()
        .args(["--game-dir", s(&dir), "ls", "general", "-n", "5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("-- 5 entries"));
    tool()
        .args(["stat", s(&dir), "--totals"])
        .assert()
        .success()
        .stdout(predicate::str::contains("string table"));
}
