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
        "detect", "info", "ls", "stat", "cat", "extract", "validate", "strings", "diff", "snapshot",
    ] {
        assert!(out.contains(cmd), "missing {cmd} in help");
    }
    let a = tool().arg("--help").assert().success();
    let long = String::from_utf8(a.get_output().stdout.clone()).unwrap();
    assert!(long.contains("Exit status"), "{long}");
}

#[test]
fn runtime_errors_exit_with_status_2() {
    tool()
        .args(["cat", "/nonexistent/client_general.dat", "41000001"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("error:"));
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
        .stdout(predicate::str::contains("total (2)"))
        .stdout(
            predicate::str::is_match(r"ARCHIVE +KIND +REGION +BLOCK +ENG +DATA +MINOR +ENTRIES")
                .unwrap(),
        );
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
    // The same {dir, archives} shape as for a directory.
    let rows = v["archives"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert!(v["dir"].is_string());
    let r = &rows[0];
    assert_eq!(r["stats"]["entries"], 3);
    assert_eq!(r["stats"]["compressed"], 1);
    assert_eq!(r["header"]["dataset"], 0x0109_0000);
    assert_eq!(r["identity"]["kind"], "surface");
    assert!(r["identity"]["region"].is_null());
    assert!(r["header"]["block_size"].as_u64().unwrap() > 0);
    assert!(r["header"].get("min_alloc").is_none());
    assert_eq!(r["membership"]["part_count"], 1);
    // Directory rows carry membership too.
    let a = tool()
        .args(["--json", "info", s(&f.game)])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    let surf = v["archives"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["archive"] == "surface")
        .unwrap();
    assert_eq!(surf["membership"]["part_count"], 1);
}

#[test]
fn info_reports_kind_and_region_of_cell_archives() {
    let dir = tempfile::tempdir().unwrap();
    for (file, subset) in [("client_cell_3.dat", 3), ("client_map_14.dat", 7)] {
        let mut b = Builder::new();
        b.dataset = if file.contains("cell") {
            0x0203_0000
        } else {
            0x020A_0000
        };
        b.subset = subset;
        b.stored(0x0100_0001, common::payload(0x0100_0001, 32));
        std::fs::write(dir.path().join(file), b.build().bytes).unwrap();
    }
    let a = tool()
        .args(["--json", "info", s(dir.path())])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    let rows = v["archives"].as_array().unwrap();
    let cell = rows.iter().find(|r| r["archive"] == "cell_3").unwrap();
    assert_eq!(cell["identity"]["kind"], "cell");
    assert_eq!(cell["identity"]["region"], 3);
    assert_eq!(cell["identity"]["region_mismatch"], false);
    let map = rows.iter().find(|r| r["archive"] == "map_14").unwrap();
    assert_eq!(map["identity"]["kind"], "map");
    // The header word wins; the disagreement with the file name is flagged.
    assert_eq!(map["identity"]["region"], 7);
    assert_eq!(map["identity"]["region_mismatch"], true);
    tool()
        .args(["info", s(dir.path())])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "map_14: header region (0x150) differs",
        ));
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
fn ls_says_limit_reached_only_when_entries_were_cut_off() {
    let f = fixture();
    let general = f.game.join("client_general.dat");
    // 22 entries: 20 + the string table + the set-membership record.
    tool()
        .args(["ls", s(&general), "-n", "22"])
        .assert()
        .success()
        .stdout(predicate::str::contains("-- 22 entries"))
        .stdout(predicate::str::contains("limit reached").not());
    tool()
        .args(["ls", s(&general), "-n", "21"])
        .assert()
        .success()
        .stdout(predicate::str::contains("-- 21 entries"))
        .stdout(predicate::str::contains("limit reached"));
}

/// A copy of `general` whose root's first child pointer lies past the end of the file, so
/// every directory walk stops at once.
fn broken_walk(f: &Fixture) -> PathBuf {
    let mut corrupt = f.general.clone();
    let (root_off, _) = corrupt.root();
    corrupt.put_u32(Built::child_ptr_pos(root_off, 0), 0x7FFF_FF00);
    let bad = f.root.join("badwalk");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("client_general.dat"), &corrupt.bytes).unwrap();
    bad
}

#[test]
fn a_broken_directory_walk_fails_every_command() {
    let f = fixture();
    let bad = broken_walk(&f);
    let file = bad.join("client_general.dat");
    for target in [&file, &bad] {
        tool()
            .args(["info", s(target)])
            .assert()
            .code(1)
            .stdout(predicate::str::contains("stop").or(predicate::str::contains("WALK ERROR")));
        tool().args(["--json", "info", s(target)]).assert().code(1);
        tool()
            .args(["stat", s(target)])
            .assert()
            .code(1)
            .stdout(predicate::str::contains("directory walk stopped"));
        tool()
            .args(["stat", "--totals", s(target)])
            .assert()
            .code(1)
            .stdout(predicate::str::contains("directory walk stopped"));
    }
    tool()
        .args(["info", s(&file)])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("WALK ERROR"));
    tool().args(["ls", s(&file)]).assert().code(2);
    tool()
        .args(["snapshot", s(&file), "-o", s(&f.out.join("bad.snap"))])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("stopped early"));
    // Log lines on a redirected stderr carry no colour codes.
    let a = tool()
        .env("RUST_LOG", "warn")
        .args(["snapshot", s(&file), "-o", s(&f.out.join("bad2.snap"))])
        .assert()
        .code(1);
    let err = String::from_utf8(a.get_output().stderr.clone()).unwrap();
    assert!(err.contains("directory walk stopped early"), "{err}");
    assert!(!err.contains('\u{1b}'), "{err:?}");
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
    // --stored gives the on-disk (zlib) bytes of a compressed record.
    let a = tool()
        .args(["cat", s(&f.game), "0x41000002", "--raw", "--stored"])
        .assert()
        .success();
    // Exactly what the writer stored: the record's compressed framing, not the payload.
    assert_eq!(
        a.get_output().stdout,
        common::zlib(&surface(SURFACE_Z_DID), Compression::default())
    );
    tool()
        .args(["cat", s(&f.game), "0x41000002", "--stored"])
        .assert()
        .success()
        .stdout(predicate::str::contains("decoded:").not());
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
    // --stored writes the on-disk bytes; for an uncompressed record they equal the payload.
    let st = f.out.join("stored.bin");
    tool()
        .args(["extract", s(&f.game), "41000002", "-o", s(&st), "--stored"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stored bytes"));
    assert_eq!(
        std::fs::read(&st).unwrap(),
        common::zlib(&surface(SURFACE_Z_DID), Compression::default())
    );
    let st1 = f.out.join("stored1.bin");
    tool()
        .args(["extract", s(&f.game), "41000001", "-o", s(&st1), "--stored"])
        .assert()
        .success();
    assert_eq!(std::fs::read(&st1).unwrap(), surface(SURFACE_DID));
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
    // The message names the directory accurately for a single-archive input.
    tool()
        .args([
            "extract",
            s(&f.game.join("client_surface.dat")),
            "41000001",
            "-o",
            s(&f.game.join("y.bin")),
        ])
        .assert()
        .stderr(predicate::str::contains(
            "the directory holding the input archive",
        ));
}

#[test]
fn a_dotdot_after_a_missing_dir_creates_nothing_in_the_game_dir() {
    let f = fixture();
    let listing = || {
        let mut v: Vec<String> = std::fs::read_dir(&f.game)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    };
    let before = listing();
    // `game/sub` does not exist, so the path means out/x.bin: written there, and no
    // `game/sub` is created on the way.
    let typed = f.game.join("sub/../../out/x.bin");
    tool()
        .args(["extract", s(&f.game), "41000001", "-o", s(&typed)])
        .assert()
        .success();
    let typed = f.game.join("sub2/../../out/x.snap");
    tool()
        .args(["snapshot", s(&f.game), "-o", s(&typed), "--force"])
        .assert()
        .success();
    assert_eq!(before, listing());
    assert!(f.out.join("x.bin").is_file());
    assert!(f.out.join("x.snap").is_file());
}

#[cfg(unix)]
mod links {
    use super::*;
    use std::os::unix::fs::symlink;

    fn game_files(f: &Fixture) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&f.game)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn dangling_symlink_into_the_game_dir_is_refused_with_force() {
        let f = fixture();
        let before = game_files(&f);
        let link = f.out.join("link.bin");
        symlink(f.game.join("planted.bin"), &link).unwrap();
        let slink = f.out.join("link.snap");
        symlink(f.game.join("planted.snap"), &slink).unwrap();
        let archive = f.game.join("client_surface.dat");
        for force in [false, true] {
            let mut c = tool();
            c.args(["extract", s(&archive), "41000001", "-o", s(&link)]);
            if force {
                c.arg("--force");
            }
            c.assert()
                .code(2)
                .stderr(predicate::str::contains("refusing"));
            let mut c = tool();
            c.args(["snapshot", s(&f.game), "-o", s(&slink)]);
            if force {
                c.arg("--force");
            }
            c.assert()
                .code(2)
                .stderr(predicate::str::contains("refusing"));
        }
        assert_eq!(before, game_files(&f));
    }

    #[test]
    fn symlink_reached_through_missing_dir_and_dotdot_is_refused() {
        let f = fixture();
        let before = game_files(&f);
        symlink(&f.game, f.out.join("gamelink")).unwrap();
        let archive = f.game.join("client_surface.dat");
        for target in [
            "gamelink/a.bin",
            "nope/../gamelink/b.bin",
            "nope/../gamelink/newdir/x",
        ] {
            tool()
                .args([
                    "extract",
                    s(&archive),
                    "41000001",
                    "-o",
                    s(&f.out.join(target)),
                ])
                .assert()
                .code(2)
                .stderr(predicate::str::contains("refusing"));
            tool()
                .args(["snapshot", s(&archive), "-o", s(&f.out.join(target))])
                .assert()
                .code(2);
        }
        assert_eq!(before, game_files(&f));
        assert!(!f.out.join("nope").exists());
    }

    #[test]
    fn force_does_not_truncate_a_hard_linked_archive() {
        let f = fixture();
        let archive = f.game.join("client_surface.dat");
        let original = std::fs::read(&archive).unwrap();
        let hard = f.out.join("h.bin");
        std::fs::hard_link(&archive, &hard).unwrap();
        tool()
            .args(["extract", s(&archive), "41000001", "-o", s(&hard)])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("--force"));
        tool()
            .args([
                "extract",
                s(&archive),
                "41000001",
                "-o",
                s(&hard),
                "--force",
            ])
            .assert()
            .success();
        assert_eq!(std::fs::read(&archive).unwrap(), original);
        assert_eq!(std::fs::read(&hard).unwrap(), surface(SURFACE_DID));
    }

    #[test]
    fn the_real_directory_of_a_symlinked_archive_is_protected() {
        let f = fixture();
        let link_dir = f.root.join("linkdir");
        std::fs::create_dir_all(&link_dir).unwrap();
        symlink(
            f.game.join("client_surface.dat"),
            link_dir.join("client_surface.dat"),
        )
        .unwrap();
        let before = game_files(&f);
        tool()
            .args([
                "extract",
                s(&link_dir.join("client_surface.dat")),
                "41000001",
                "-o",
                s(&f.game.join("x.bin")),
            ])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("refusing"));
        // A set made of symlinked archives protects their real directory too.
        tool()
            .args(["snapshot", s(&link_dir), "-o", s(&f.game.join("x.snap"))])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("refusing"));
        assert_eq!(before, game_files(&f));
    }
}

#[test]
fn the_env_game_dir_is_protected_even_when_discovery_rejects_it() {
    let f = fixture();
    // A directory discovery rejects (too few archives), named by the environment.
    let env_game = f.root.join("envgame");
    std::fs::create_dir_all(&env_game).unwrap();
    std::fs::copy(
        f.game.join("client_surface.dat"),
        env_game.join("client_surface.dat"),
    )
    .unwrap();
    tool()
        .env("TESSERA_GAME_DIR", &env_game)
        .args([
            "extract",
            s(&f.game.join("client_general.dat")),
            "25000007",
            "-o",
            s(&env_game.join("x.bin")),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("TESSERA_GAME_DIR"));
    assert!(!env_game.join("x.bin").exists());
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
fn validate_counts_an_archive_that_cannot_be_opened_as_an_error() {
    let f = fixture();
    // An interrupted patch left 16 junk bytes where client_general.dat was.
    std::fs::write(f.game.join("client_general.dat"), [0xA5u8; 16]).unwrap();
    let a = tool().args(["validate", s(&f.game)]).assert().code(1);
    let out = String::from_utf8(a.get_output().stdout.clone()).unwrap();
    assert!(out.contains("UNOPENED"), "{out}");
    assert!(out.contains("could not be opened"), "{out}");
    assert!(out.contains("FAILED"), "{out}");
    assert!(out.contains("2 archives"), "{out}");
    let a = tool()
        .args(["--json", "validate", s(&f.game)])
        .assert()
        .code(1);
    let v: serde_json::Value = serde_json::from_slice(&a.get_output().stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["errors"], 1);
    assert_eq!(v["unopened"][0]["archive"], "general");
    // The same file named directly fails too (it cannot even be opened).
    tool()
        .args(["validate", s(&f.game.join("client_general.dat"))])
        .assert()
        .code(2);
    // A junk file with a name that is no archive kind is only a warning.
    let g = f.root.join("g2");
    std::fs::create_dir_all(&g).unwrap();
    std::fs::copy(
        f.game.join("client_surface.dat"),
        g.join("client_surface.dat"),
    )
    .unwrap();
    std::fs::write(g.join("client_notakind.dat"), [0xA5u8; 16]).unwrap();
    tool()
        .args(["validate", s(&g)])
        .assert()
        .success()
        .stdout(predicate::str::contains("OK"));
}

#[test]
fn validate_inflate_and_no_tiling() {
    let f = fixture();
    // --inflate inflates the compressed surface; a clean set stays clean.
    tool()
        .args(["validate", "--inflate", s(&f.game)])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 errors"));
    tool()
        .args(["validate", "--no-tiling", s(&f.game)])
        .assert()
        .success();
    // A compressed record whose zlib stream is broken passes the structural checks and is
    // caught only when inflated.
    let mut b = Builder::new();
    b.compressed(
        SURFACE_Z_DID,
        &surface(SURFACE_Z_DID),
        Compression::default(),
    );
    let mut built = b.build();
    let len = built.bytes.len();
    let (root_off, _) = built.root();
    let data_off = built.get_u32(Built::entry_pos(root_off, 0) + 8) as usize;
    // Corrupt the deflate data a few bytes into the payload (past the length prefix and
    // zlib header).
    for k in 8..16 {
        if data_off + k < len {
            built.bytes[data_off + k] ^= 0xFF;
        }
    }
    let z = f.root.join("zbad");
    std::fs::create_dir_all(&z).unwrap();
    std::fs::write(z.join("client_surface.dat"), &built.bytes).unwrap();
    tool().args(["validate", s(&z)]).assert().success();
    tool()
        .args(["validate", "--inflate", s(&z)])
        .assert()
        .failure();
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
    let p = tool()
        .args(["--game-dir", s(&f.game), "detect", "--path"])
        .assert()
        .success();
    let printed = String::from_utf8(p.get_output().stdout.clone()).unwrap();
    assert_eq!(
        std::fs::canonicalize(printed.trim_end()).unwrap(),
        std::fs::canonicalize(&f.game).unwrap()
    );
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
