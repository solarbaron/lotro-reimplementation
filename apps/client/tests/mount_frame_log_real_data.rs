// SPDX-License-Identifier: GPL-3.0-or-later
//! P5 exit criterion 2 end to end on the user's install (read-only; decision 0024 §5 P5,
//! decision 0036 §3, `walker.md` §16.2, §16.10, §16.11): the client binary runs the walker
//! headless in its frame loop with `--mount steed:medium` (mounted at once, the command
//! line's default timing), `--autorun run` (forward held) and `--frame-log`, and the frame
//! log's mount columns (found by their header names) show the rider mounted on the
//! controller's war-steed layer, rising through the gears 4, 5, 6 and 7 in that order while
//! forward is held, with the rider's and the mount's clips playing and the feet moving.
//! A second run checks `walker.md` §16.12 resolution (c): `--command "/ride off"` with
//! `--mount` ends the walker on foot, because the frame loop holds the `/ride` until the
//! `--mount` choice has been applied, so the dismount acts after the initial mount.
//!
//! No DID is written here; only column values the walker logs are compared. Skipped
//! (reported as passed) unless `TESSERA_GAME_DIR` is set; needs a GPU adapter (the headless
//! frame loop renders offscreen). Run with
//! `cargo test --release -p tessera-client --test mount_frame_log_real_data -- --nocapture`.

mod travel_common;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use travel_common::game_dir;

/// The frame log as rows of named columns.
fn read_log(path: &Path) -> Vec<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(path).expect("frame log");
    let mut lines = text.lines();
    let header: Vec<String> = lines
        .next()
        .expect("header")
        .split(',')
        .map(str::to_owned)
        .collect();
    lines
        .map(|l| {
            header
                .iter()
                .cloned()
                .zip(l.split(',').map(str::to_owned))
                .collect()
        })
        .collect()
}

fn num(row: &BTreeMap<String, String>, col: &str) -> f64 {
    let v = row.get(col).unwrap_or_else(|| panic!("column {col}"));
    if let Some(h) = v.strip_prefix("0x") {
        return u64::from_str_radix(h, 16).map_or(f64::NAN, |x| x as f64);
    }
    v.parse::<f64>().unwrap_or(f64::NAN)
}

/// Runs the client headless mounted on `spec` and returns the frame log's rows.
fn ride(dir: &Path, spec: &str) -> Vec<BTreeMap<String, String>> {
    ride_with(dir, spec, &[])
}

/// Runs the client headless with `--mount spec` and `extra` arguments, returning the frame
/// log's rows.
fn ride_with(dir: &Path, spec: &str, extra: &[&str]) -> Vec<BTreeMap<String, String>> {
    let tmp = tempfile::tempdir().expect("tempdir");
    let log = tmp.path().join("frames.csv");
    let data = tmp.path().join("data");
    let log_str = log.to_str().expect("utf-8");
    let data_str = data.to_str().expect("utf-8");
    let mut args: Vec<&str> = vec![
        "--headless",
        "--avatar",
        "human,m",
        "--mount",
        spec,
        "--autorun",
        "run",
        "--duration",
        "8",
        "--unpaced",
        "--frame-log",
        log_str,
        "--data-dir",
        data_str,
    ];
    args.extend_from_slice(extra);
    let out = Command::new(env!("CARGO_BIN_EXE_tessera-client"))
        .env("TESSERA_GAME_DIR", dir)
        .env("RUST_LOG", "warn")
        .args(&args)
        .output()
        .expect("runs the client");
    assert!(
        out.status.success(),
        "the headless run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    read_log(&log)
}

#[test]
fn a_war_steed_rides_at_the_gallop_in_the_frame_loop() {
    let Some(dir) = game_dir() else { return };
    let rows = ride(&dir, "steed:medium");
    assert!(rows.len() > 60, "frames logged: {}", rows.len());
    let mut states: BTreeMap<u32, usize> = BTreeMap::new();
    for r in &rows {
        *states.entry(num(r, "mount_state") as u32).or_default() += 1;
    }
    println!("frames per mount state: {states:?}");
    // Resolution and evaluation run on worker threads (`walker.md` §16.12 OPEN (a)), so
    // the log may begin on foot; once mounted the rider stays mounted.
    let first = rows
        .iter()
        .position(|r| num(r, "mount_state") == 3.0)
        .expect("the rider mounts (instant from the command line, §16.2)");
    assert!(
        rows[..first].iter().all(|r| num(r, "mount_state") == 0.0),
        "on foot until mounted: {states:?}"
    );
    assert!(
        rows[first..].iter().all(|r| num(r, "mount_state") == 3.0),
        "mounted from then on: {states:?}"
    );
    assert!(rows.len() - first > 60, "mounted frames: {states:?}");
    let rows: Vec<BTreeMap<String, String>> = rows[first..].to_vec();
    assert!(
        rows.iter().all(|r| num(r, "mount_effect") > 0.0),
        "a mount effect while mounted"
    );
    let layers: std::collections::BTreeSet<&str> = rows
        .iter()
        .filter_map(|r| r.get("move_layer").map(String::as_str))
        .collect();
    println!("layers while mounted: {layers:?}");
    assert!(
        layers.contains("Warsteed"),
        "the war-steed controller's layer (§16.6)"
    );
    let gaits: std::collections::BTreeSet<u32> =
        rows.iter().map(|r| num(r, "gait") as u32).collect();
    println!("gaits logged: {gaits:?}");
    assert!(
        gaits.contains(&7),
        "the gallop while forward is held: {gaits:?}"
    );
    // When the log starts before the gallop, the gears rise one at a time (§16.11 check 3).
    let mut order: Vec<u32> = Vec::new();
    for r in &rows {
        let g = num(r, "gait") as u32;
        if (4..=7).contains(&g) && order.last() != Some(&g) {
            order.push(g);
        }
    }
    assert!(
        order.windows(2).all(|w| w[1] == w[0] + 1),
        "gears rise one at a time: {order:?}"
    );
    let speeds: Vec<f64> = rows.iter().map(|r| num(r, "control_speed")).collect();
    let top = speeds.iter().copied().fold(0.0_f64, f64::max);
    let last = speeds.last().copied().unwrap_or(0.0);
    let moved = rows
        .iter()
        .map(|r| num(r, "ground_speed"))
        .fold(0.0_f64, f64::max);
    println!("control speed top {top:.2} m/s, last {last:.2} m/s; top ground speed {moved:.2} m/s");
    assert!(top > 1.0, "the steed moves forward");
    assert!(
        speeds.iter().filter(|v| (top - **v).abs() < 0.05).count() * 2 >= speeds.len(),
        "steady speed at the gallop in most frames (last {last:.2})"
    );
    assert!(
        rows.iter()
            .any(|r| num(r, "rider_clip") > 0.0 && num(r, "mount_clip") > 0.0),
        "rider and mount clips play (§16.7)"
    );
    assert!(
        rows.iter().any(|r| num(r, "mount_alpha") > 0.99),
        "the mount is drawn at full opacity"
    );
}

#[test]
fn a_start_up_ride_off_dismounts_the_command_line_mount() {
    // `walker.md` §16.12 resolution (c): `--command "/ride off"` with `--mount` ends on foot,
    // because the frame loop holds the `/ride` until `--mount` has been requested, so the
    // dismount cancels the initial mount instead of running before it.
    let Some(dir) = game_dir() else { return };
    let rows = ride_with(&dir, "steed:medium", &["--command", "/ride off"]);
    assert!(rows.len() > 60, "frames logged: {}", rows.len());
    let states: std::collections::BTreeSet<u32> =
        rows.iter().map(|r| num(r, "mount_state") as u32).collect();
    println!("mount states with --command \"/ride off\": {states:?}");
    // The run ends on foot and never settles into the mounted state at its end.
    let tail = rows.len().saturating_sub(30);
    assert!(
        rows[tail..].iter().all(|r| num(r, "mount_state") == 0.0),
        "on foot at the end: {states:?}"
    );
    assert_eq!(
        rows.last().map(|r| num(r, "mount_effect")),
        Some(0.0),
        "no mount effect at the end"
    );
}

#[test]
fn a_locked_gear_holds_the_canter() {
    let Some(dir) = game_dir() else { return };
    let rows = ride(&dir, "steed:medium@lock=canter");
    let rows: Vec<&BTreeMap<String, String>> = rows
        .iter()
        .filter(|r| num(r, "mount_state") == 3.0)
        .collect();
    assert!(!rows.is_empty(), "the rider mounts");
    let gaits: std::collections::BTreeSet<u32> =
        rows.iter().map(|r| num(r, "gait") as u32).collect();
    let locks: std::collections::BTreeSet<u32> =
        rows.iter().map(|r| num(r, "locked_gear") as u32).collect();
    println!("gaits {gaits:?}, locked gears {locks:?}");
    assert_eq!(
        locks,
        [6].into_iter().collect(),
        "the selected gear is held"
    );
    assert!(!gaits.contains(&7), "never the gallop: {gaits:?}");
    assert!(gaits.contains(&6), "the canter: {gaits:?}");
}
