// SPDX-License-Identifier: GPL-3.0-or-later
//! Every committed synthetic scenario of the client (`apps/client/tests/scenarios/`, all
//! `install none`) replays through the feed of this crate and writes its committed log
//! byte for byte (`docs/specs/scenario-feed.md` §10.5; the "P6 progression" rule of §10.3
//! that fields added to existing variants are written only off their defaults). The
//! client's own test replays them through the binary; this one guards the crate alone.
#![cfg(feature = "scenario")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tessera_net::Session;
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{EmptyResolver, HeadlessDriver, Scenario};

fn scenarios_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/client/tests/scenarios")
}

fn play(text: &[u8]) -> Vec<u8> {
    let scenario = Scenario::parse(text).expect("parse");
    let resolved = Arc::new(scenario.resolve(&mut EmptyResolver).expect("resolve"));
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let session = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let mut driver = HeadlessDriver::new(session, resolved, Some(Vec::new()));
    let _ = driver.run();
    driver.into_log().expect("log")
}

#[test]
fn committed_logs_replay_byte_identical() {
    let Ok(dir) = std::fs::read_dir(scenarios_dir()) else {
        // The crate alone (no client tree next to it): nothing to replay.
        return;
    };
    let mut files: Vec<PathBuf> = dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "scenario"))
        .collect();
    files.sort();
    let mut checked = 0;
    for path in files {
        let log_path = path.with_extension("log");
        let Ok(want) = std::fs::read(&log_path) else {
            continue;
        };
        let text = std::fs::read(&path).expect("scenario");
        let got = play(&text);
        if got != want {
            let got = String::from_utf8_lossy(&got);
            let want = String::from_utf8_lossy(&want);
            let first = got
                .lines()
                .zip(want.lines())
                .enumerate()
                .find(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| format!("line {}:\n  got:  {a}\n  want: {b}", i + 1))
                .unwrap_or_else(|| "lengths differ".to_owned());
            panic!("{} differs from its committed log: {first}", path.display());
        }
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} committed logs found");
}
