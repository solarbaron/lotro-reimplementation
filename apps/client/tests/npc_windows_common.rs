// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared support of the NPC window tests (decision 0024 P3): playing a committed scenario
//! in-process on the headless driver (the path of `--scenario … --headless`) with every
//! poll's events applied to the client's player state, painting a window of the game UI
//! into an RGBA picture, and building the UI events a click on an element makes.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser as _;
use tessera_client::character::state::NO_INSTALL_BAGS;
use tessera_client::character::{PlayerChange, PlayerState};
use tessera_client::cli::Args;
use tessera_client::scenario::{InstallChoice, headless_config, prepare};
use tessera_gamedata::inventory::BagLayout;
use tessera_net::sandbox::SandboxBackend;
use tessera_net::scenario::{DriverOutcome, HeadlessDriver};
use tessera_net::{Event, Session};
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::paint::Unlimited;
use tessera_ui::{Command, ElementKey, Ui, UiEvent};

/// The committed NPC-window scenarios.
pub const SCENARIOS: [&str; 3] = ["dialogue", "vendor", "loot"];

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

/// One poll of a run: its events and the player-state changes they made.
#[derive(Debug)]
pub struct Poll<'a> {
    /// The poll's number (0 = the first poll after the driver started).
    pub index: usize,
    /// The events.
    pub events: &'a [Event],
    /// The player-state changes, in order.
    pub changes: &'a [PlayerChange],
    /// The player state after the poll.
    pub state: &'a PlayerState,
}

/// Plays scenario `name` headless (with the install at `install`, or none), applies every
/// poll's events to a player state with `bags` and calls `each` after every poll. Returns
/// the outcome, the canonical log and the final state.
pub fn play(
    name: &str,
    install: Option<&Path>,
    bags: Option<BagLayout>,
    mut each: impl FnMut(&Poll<'_>),
) -> (DriverOutcome, Vec<u8>, PlayerState) {
    let mut args = Args::parse_from(["tessera-client", "--no-data"]);
    args.scenario = Some(scenario_path(name));
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
        each(&Poll {
            index,
            events,
            changes: &changes,
            state: &state,
        });
        index += 1;
    });
    let log = driver.into_log().unwrap_or_default();
    (outcome, log, state)
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

/// The first element with ID `id` at or below `k`.
#[must_use]
pub fn part(ui: &Ui, k: ElementKey, id: u32) -> Option<ElementKey> {
    if ui.element_id(k) == Some(id) {
        return Some(k);
    }
    tessera_ui::game::find_below(ui, k, id)
}

/// Paints `ui` at fixed times until settled (no wall clock in the picture).
pub fn settle(ui: &mut Ui) {
    let mut budget = Unlimited;
    for t in 0..8u64 {
        ui.paint(
            Duration::from_secs(3600) + Duration::from_millis(16 * t),
            &mut budget,
        );
    }
}

/// An RGBA picture of panel `k` (canvas pixels are `0xAARRGGBB`), read from the surface
/// of `k` or, when `k` paints into an ancestor's surface (a dialog inside its screen-sized
/// container), from that surface at `k`'s place.
#[must_use]
pub fn capture(ui: &mut Ui, k: ElementKey) -> Option<(u32, u32, Vec<u8>)> {
    settle(ui);
    let mut owner = k;
    while ui.surface_pixel(owner, 0, 0).is_none() {
        owner = ui.parent(owner)?;
    }
    let b = ui.displayed_box(k);
    let o = ui.displayed_box(owner);
    let (dx, dy) = (b.l - o.l, b.t - o.t);
    let (w, h) = (
        u32::try_from(b.w().max(1)).ok()?,
        u32::try_from(b.h().max(1)).ok()?,
    );
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let p = ui.surface_pixel(owner, x + dx, y + dy)?;
            data.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8]);
        }
    }
    Some((w, h, data))
}
