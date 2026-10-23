// SPDX-License-Identifier: GPL-3.0-or-later
//! OE-T22 (`docs/specs/official-entry.md` §6.3 "Release builds"): the release feature guard
//! passes on the workspace and fails on a fixture in which a normal dependency enables
//! `tessera-compat`'s test-support feature (the script's self-test builds that fixture, and
//! the variants through another crate and through a client feature).

#![cfg(unix)]

use std::path::PathBuf;
use std::process::Command;

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/check-release-features.sh")
}

fn run(args: &[&str]) -> (bool, String) {
    let out = Command::new("bash")
        .arg(script())
        .args(args)
        .output()
        .expect("bash runs the check script");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

#[test]
fn oe_t22_official_release_features_workspace_passes() {
    let (ok, out) = run(&[]);
    assert!(ok, "{out}");
    assert!(
        out.contains("(--all-features): test-support not in the normal graph"),
        "{out}"
    );
}

#[test]
fn oe_t22_official_release_features_fixture_fails() {
    let (ok, out) = run(&["--self-test"]);
    assert!(ok, "{out}");
    for case in ["normal (exit 1)", "indirect (exit 1)", "feature (exit 1)"] {
        assert!(out.contains(&format!("ok: {case}")), "{case}: {out}");
    }
}
