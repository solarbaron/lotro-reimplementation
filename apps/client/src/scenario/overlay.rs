// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless overlay runs (`--scenario FILE --server CONNECT --headless`,
//! `scenario-feed.md` §12.5, decision 0025 D1): the client's world session with the overlay
//! backend, paced by the wall clock at one scenario tick per frame (§12.5.3: overlay runs
//! depend on the server's timing; their logs are never normative). The run ends when the
//! feed finishes (exit 0), fails or loses the server (exit 5), or after
//! [`MAX_OVERLAY_RUN`] without finishing (exit 5). `--scenario-log` writes the log with
//! wall-clock times, as a windowed run does.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_net::SessionState;
use tessera_net::scenario::{ResolvedScenario, ScenarioStatus};

use super::{EXIT_FAILURE, EXIT_OK, EXIT_RUNTIME};
use crate::cli::Args;
use crate::net::World;

/// Longest wall-clock time a headless overlay run waits for its scenario to finish.
pub const MAX_OVERLAY_RUN: Duration = Duration::from_secs(3600);

/// Runs `scenario` as a headless overlay over the test server of `args` and returns the
/// exit code.
#[must_use]
pub fn run_headless(args: &Args, scenario: Arc<ResolvedScenario>) -> u8 {
    let settings = match crate::cli::remote_settings_from_env(args) {
        Ok(Some(s)) => s,
        Ok(None) => {
            eprintln!("tessera-client: an overlay run needs --server");
            return EXIT_FAILURE;
        }
        Err(e) => {
            eprintln!("tessera-client: {e:#}");
            return EXIT_FAILURE;
        }
    };
    let tick = Duration::from_millis(scenario.tick_ms().max(1));
    let plays_input = scenario.plays_input();
    let now = Instant::now();
    let mut world = match World::overlay(now, &settings, Arc::clone(&scenario), None, None) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("tessera-client: {e}");
            return EXIT_FAILURE;
        }
    };
    // `--scenario-log`: the windowed run's log with wall-clock times (§12.5.3: never
    // normative).
    let slot = match super::open_log(args, None) {
        Ok(l) => l.map(super::live_log::LiveLogSlot::new),
        Err(e) => {
            eprintln!("tessera-client: {e}");
            return EXIT_RUNTIME;
        }
    };
    if let Some(log) = slot.as_ref().and_then(|s| s.take(&scenario)) {
        world.set_scenario_log(log, now);
    }
    if plays_input {
        world.set_scenario_input(tessera_net::scenario::InputPlayer::new(scenario));
    }
    let code = drive(&mut world, tick, MAX_OVERLAY_RUN);
    world.shutdown();
    drop(world);
    super::windowed_exit_code(code, slot.as_ref())
}

/// Updates `world` once per `tick` of wall-clock time until its scenario finishes (0),
/// fails or the session closes (5), or `limit` passes (5).
pub fn drive(world: &mut World, tick: Duration, limit: Duration) -> u8 {
    let start = Instant::now();
    // Nothing here takes the player-state changes (no panels, no plugins).
    world.set_collect_player_changes(false);
    loop {
        let now = Instant::now();
        world.update(now);
        match world.scenario_status() {
            Some(ScenarioStatus::Finished { reason, time_ms }) => {
                tracing::info!(?reason, time_ms, "overlay scenario finished");
                return EXIT_OK;
            }
            Some(ScenarioStatus::Failed { error, time_ms }) => {
                tracing::error!(%error, time_ms, "overlay scenario failed");
                return EXIT_RUNTIME;
            }
            _ => {}
        }
        if world.state() == SessionState::Closed {
            tracing::error!("the test server's session ended before the scenario finished");
            return EXIT_RUNTIME;
        }
        if now.duration_since(start) > limit {
            tracing::error!("the overlay scenario did not finish in {limit:?}");
            return EXIT_RUNTIME;
        }
        std::thread::sleep(tick);
    }
}
