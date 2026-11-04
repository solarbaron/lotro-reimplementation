// SPDX-License-Identifier: GPL-3.0-or-later
//! The command of `docs/DEVELOPMENT.md` "Reading a live official run" captures the evidence: the
//! client's log goes to standard output, so that is what the command redirects, and a log file holds
//! the plain field forms the section tells the owner to look for. The real binary runs here on a
//! committed synthetic scenario, with no install and no network: an official run writes through the
//! same writer, and no test connects to an official server. The section's own words are checked in
//! the client's library tests (`the_documented_log_command_redirects_what_the_client_writes_to`),
//! because the tests here may not name the option that starts an official run.

use std::path::Path;
use std::process::{Command, Stdio};

const LOG_FILTER: &str = "info,official_evidence=debug";

/// As the command runs it: the log in a file, on standard output, with no colour code, and the same
/// filter.
#[test]
fn a_redirected_log_fills_its_file_with_plain_lines() {
    let dir = tempfile::tempdir().expect("a directory");
    let log_path = dir.path().join("official-run.log");
    let scenario_log = dir.path().join("scenario.log");
    let scenario = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/chargen.scenario");
    let log = std::fs::File::create(&log_path).expect("the log file");
    let status = Command::new(env!("CARGO_BIN_EXE_tessera-client"))
        .env_remove("TESSERA_GAME_DIR")
        .env_remove("NO_COLOR")
        .env("RUST_LOG", LOG_FILTER)
        .arg("--headless")
        .arg("--no-data")
        .arg("--scenario")
        .arg(&scenario)
        .arg("--scenario-log")
        .arg(&scenario_log)
        .arg("--data-dir")
        .arg(dir.path().join("data"))
        .stdout(Stdio::from(log))
        .stderr(Stdio::piped())
        .output()
        .expect("runs the client");
    assert!(status.status.success(), "{status:?}");
    let written = std::fs::read(&log_path).expect("the log file");
    let text = String::from_utf8_lossy(&written);
    assert!(text.contains(" INFO "), "nothing in the file:\n{text}");
    assert!(
        !written.contains(&0x1B),
        "colour codes in the file:\n{text}"
    );
    let stderr = String::from_utf8_lossy(&status.stderr);
    assert!(
        !stderr.contains(" INFO "),
        "the log went elsewhere:\n{stderr}"
    );
}
