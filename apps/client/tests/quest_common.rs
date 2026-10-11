// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared support of the quest journal tests (decision 0024 P3): playing a scenario file
//! in-process on the headless driver (the path of `--scenario … --headless`) with every
//! poll's events applied to the client's player state.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser as _;
use tessera_client::character::PlayerState;
use tessera_client::character::state::NO_INSTALL_BAGS;
use tessera_client::cli::Args;
use tessera_client::scenario::{InstallChoice, headless_config, prepare};
use tessera_gamedata::inventory::BagLayout;
use tessera_net::sandbox::SandboxBackend;
use tessera_net::scenario::{DriverOutcome, HeadlessDriver};
use tessera_net::{Event, Session};

/// The committed quest-journal scenario.
#[must_use]
pub fn committed() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/quest-log.scenario")
}

/// Plays the scenario at `path` headless (with the install at `install`, or none), applies
/// every poll's events to a player state with `bags` and calls `each` after every poll
/// with its index, events and the state. Returns the outcome, the canonical log and the
/// final state.
pub fn play_path(
    path: &Path,
    install: Option<&Path>,
    bags: Option<BagLayout>,
    mut each: impl FnMut(usize, &[Event], &PlayerState),
) -> (DriverOutcome, Vec<u8>, PlayerState) {
    let mut args = Args::parse_from(["tessera-client", "--no-data"]);
    args.scenario = Some(path.to_path_buf());
    args.headless = true;
    args.no_data = install.is_none();
    let choice = install.map_or(InstallChoice::None, |p| {
        InstallChoice::Found(p.to_path_buf())
    });
    let resolved = Arc::new(prepare(&args, &choice).expect("the scenario prepares"));
    let config = headless_config(&args, Arc::clone(&resolved)).expect("config");
    let backend = SandboxBackend::new(config).expect("sandbox");
    let mut driver: HeadlessDriver<Vec<u8>> =
        HeadlessDriver::new(Session::new(backend), resolved, Some(Vec::new()));
    let mut state = PlayerState::new(bags.unwrap_or(NO_INSTALL_BAGS));
    let mut index = 0usize;
    let outcome = driver.run_with(|events| {
        let mut changes = Vec::new();
        for ev in events {
            state.apply(ev, &mut changes);
        }
        each(index, events, &state);
        index += 1;
    });
    let log = driver.into_log().unwrap_or_default();
    (outcome, log, state)
}
