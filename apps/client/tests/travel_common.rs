// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared support of the P5 window tests (decision 0024 P5): playing a committed scenario
//! in-process on the headless driver (the path of `--scenario … --headless`) with every
//! poll's events applied to the client's player state and P5 state, and the UI events a
//! click makes.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser as _;
use tessera_client::character::PlayerState;
use tessera_client::character::state::NO_INSTALL_BAGS;
use tessera_client::cli::Args;
use tessera_client::scenario::{InstallChoice, headless_config, prepare};
use tessera_client::travel::{P5Change, P5State};
use tessera_net::sandbox::SandboxBackend;
use tessera_net::scenario::{DriverOutcome, HeadlessDriver};
use tessera_net::{Event, Session};
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::{Command, ElementKey, Ui, UiEvent};

/// The committed scenario `name`.
#[must_use]
pub fn scenario_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/scenarios")
        .join(format!("{name}.scenario"))
}

/// `TESSERA_GAME_DIR`, when set (real-data tests skip without it).
#[must_use]
pub fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// One poll of a run.
#[derive(Debug)]
pub struct Poll<'a> {
    /// The poll's number (0 = the first).
    pub index: usize,
    /// The scenario clock at the poll, in seconds (one tick per poll).
    pub now: f64,
    /// The events.
    pub events: &'a [Event],
    /// The P5 changes of the poll.
    pub changes: &'a [P5Change],
    /// The P5 state after the poll.
    pub p5: &'a P5State,
    /// The player state after the poll.
    pub state: &'a PlayerState,
}

/// Plays a scenario file headless (with the install at `install`, or none), applies every
/// poll's events and calls `each` after every poll. Returns the outcome and the log.
pub fn play_file(
    file: &Path,
    install: Option<&Path>,
    tick: f64,
    mut each: impl FnMut(&Poll<'_>),
) -> (DriverOutcome, Vec<u8>) {
    let mut args = Args::parse_from(["tessera-client", "--no-data"]);
    args.scenario = Some(file.to_path_buf());
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
    let mut state = PlayerState::new(NO_INSTALL_BAGS);
    let mut p5 = P5State::new();
    let mut index = 0usize;
    let outcome = driver.run_with(|events| {
        let mut changes = Vec::new();
        let mut p5c = Vec::new();
        for ev in events {
            state.apply(ev, &mut changes);
            p5.apply(ev, &mut p5c);
        }
        #[allow(clippy::cast_precision_loss)]
        let now = index as f64 * tick;
        each(&Poll {
            index,
            now,
            events,
            changes: &p5c,
            p5: &p5,
            state: &state,
        });
        index += 1;
    });
    let log = driver.into_log().unwrap_or_default();
    (outcome, log)
}

/// [`play_file`] of the committed scenario `name` (tick 100 ms).
pub fn play(
    name: &str,
    install: Option<&Path>,
    each: impl FnMut(&Poll<'_>),
) -> (DriverOutcome, Vec<u8>) {
    play_file(&scenario_path(name), install, 0.1, each)
}

/// A game-code message from `k`.
#[must_use]
pub fn message(ui: &Ui, k: ElementKey, msg: u32, command: Option<&str>) -> UiEvent {
    UiEvent::Game {
        message: Message {
            seq: 0,
            source: k,
            source_id: ui.element_id(k).unwrap_or(0),
            msg,
            arg: MsgArg {
                command: command.map(Command::named),
                ..MsgArg::default()
            },
        },
    }
}

/// `Button_click` from `k`.
#[must_use]
pub fn button(ui: &Ui, k: ElementKey) -> UiEvent {
    let m = ui.vocab().msgs.button_click;
    message(ui, k, m, None)
}

/// A left mouse click on `k`.
#[must_use]
pub fn left(ui: &Ui, k: ElementKey) -> UiEvent {
    let m = ui.vocab().msgs.mouse_click;
    message(ui, k, m, Some("SelectLeft"))
}

/// A right mouse click on `k`.
#[must_use]
pub fn right(ui: &Ui, k: ElementKey) -> UiEvent {
    let m = ui.vocab().msgs.mouse_click;
    message(ui, k, m, Some("SelectRight"))
}
