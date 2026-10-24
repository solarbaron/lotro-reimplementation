// SPDX-License-Identifier: GPL-3.0-or-later
//! The Tessera client: wires the install finder, the archive set, record decoders, the
//! renderer, the world session and the plugin host into one frame loop.
//!
//! - [`app`]: windowed client (winit).
//! - [`smoke`]: `--smoke`, the same start-up and frames rendered offscreen without a window.
//! - [`runtime`]: everything except presentation, shared by both.
//! - [`data`]: background install discovery, archive statistics and the showcase texture.
//! - [`net`], [`plugins`], [`scene`], [`input`]: subsystem glue.
//! - [`commands`]: the one command dispatcher (decision 0020): registry, line pipeline,
//!   client built-ins; the chat overlay, `--command` and plugins feed it.
//! - [`walker`]: the offline world walker (ROADMAP M3, M4 characters); [`walker_host`]
//!   wires it into the runtime, [`headless`] runs it offscreen (`--screenshot`,
//!   `--fly-path --headless`, `--duration --headless`, and `--walk-path`, which needs no
//!   GPU); [`avatar`] parses `--avatar`; [`walk_path`] parses the scripted walks of
//!   `--walk-path` (`docs/checklists/m4-5-visual.md` §2.1).
//! - [`audio`]: world audio for the walker (ROADMAP M6): the audio engine driven by the
//!   script graphs (area music, ambience, positional emitters).
//! - [`ui_host`]: the game UI (M5): `tessera-ui`, the native [`panels`], the plugin host and
//!   the input bridge in the frame loop (decisions 0014, 0015).
//! - [`scenario`]: `--scenario` (P0, `docs/specs/scenario-feed.md`): scripted world events
//!   played by the sandbox, selectors resolved against the install, the headless driver
//!   and its canonical log.
//! - [`screens`]: the character selection and creation screens before world entry (P1,
//!   `docs/specs/character-screens.md`); [`pre_world`] runs them at start-up
//!   (`--character-select`).
//! - [`character`]: characters, inventory and equipment (P1): the player's state from the
//!   session, the character panel and bag windows, item tooltips, the plugin inventory
//!   views.
//! - [`combat`]: targeting, skills and combat presentation (P2): the player's combat state
//!   from the session, selection, the combat widgets, skill and effect tooltips, the
//!   plugin combat views, the headless combat screens.
//! - [`portrait`]: the portraits of the player and target frames (P2, `ui-widgets.md`
//!   §12): the entity's figure drawn into a viewport target with its background and
//!   opacity mask, composed as the portrait element's external content.
//! - [`npc`]: NPC windows (P3): using an NPC, dialogue, quest offers, vendors, barter,
//!   loot, the wallet window and the plugin wallet views, fed by the session's events.
//! - [`quests`]: the quest journal (P3): quest log, tracker, objective counters, quest
//!   chat lines, quest markers and NPC indicators, fed by the session's events.
//! - [`map`]: the map windows (P5): the world map with feed markers and notes, the
//!   minimap's feed markers, place lookup (`findplace`) and the map-region census.
//! - [`official`]: official play (M8, `docs/specs/official-entry.md`): the start screen and
//!   `--official`, the ban-risk warning and its acceptance record, the account form and the
//!   login screens over the compatibility backend.
//! - [`travel`]: travel and instances (P5): the travel window, the instance finder with its
//!   queue, the instance-travel window, the travel presentation, the session's mount model
//!   and the plugin mount views, the walker's feed mounts.

pub mod app;
pub mod audio;
pub mod avatar;
pub mod character;
pub mod cli;
pub mod combat;
pub mod commands;
pub mod data;
pub mod headless;
pub mod input;
pub mod map;
pub mod net;
pub mod npc;
pub mod official;
pub mod panels;
pub mod plugins;
pub mod portrait;
pub mod pre_world;
pub mod progression;
pub mod quests;
pub mod render_pref;
pub mod runtime;
pub mod scenario;
pub mod scene;
pub mod screens;
pub mod smoke;
pub mod social;
pub mod timing;
pub mod travel;
pub mod ui_host;
pub mod wait;
pub mod walk_path;
pub mod walker;
pub mod walker_host;

pub use cli::Args;

/// Runs the client as configured by `args` and returns the process exit code: 0 on
/// success, 1 on failure, and for `--scenario` the codes of `scenario-feed.md` §12.2
/// ([`scenario::run`]).
#[must_use]
pub fn run_to_exit_code(args: Args) -> u8 {
    if dispatch(&args) == Dispatch::Scenario {
        return scenario::run(args);
    }
    match run(args) {
        Ok(()) => 0,
        Err(e) => {
            tracing::error!("{e:#}");
            1
        }
    }
}

/// Run the client as configured by `args` (`--scenario` runs go through
/// [`run_to_exit_code`], which keeps their exit codes).
pub fn run(args: Args) -> anyhow::Result<()> {
    let how = dispatch(&args);
    if how == Dispatch::Scenario {
        return match scenario::run(args) {
            0 => Ok(()),
            code => Err(anyhow::anyhow!("scenario run failed (exit code {code})")),
        };
    }
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        smoke = args.smoke,
        "Tessera client starting"
    );
    match how {
        Dispatch::StartScreen => official::window::run(args),
        Dispatch::Smoke => smoke::run(&args).map(|_| ()),
        Dispatch::HeadlessUi => ui_host::run::run(&args).map(|_| ()),
        Dispatch::Headless => headless::run(&args).map(|_| ()),
        Dispatch::Scenario | Dispatch::Windowed => app::run(args),
    }
}

/// Which part of the client a run starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    /// `--start-screen` or `--official`: Tessera's start screen, always windowed
    /// (`official-entry.md` §4); decided before every other mode, so no headless or scripted
    /// path can run instead.
    StartScreen,
    /// `--scenario` ([`scenario::run`]).
    Scenario,
    /// `--smoke`.
    Smoke,
    /// The headless walker with the game UI.
    HeadlessUi,
    /// The headless walker.
    Headless,
    /// The windowed client.
    Windowed,
}

/// The dispatch decision of [`run`] and [`run_to_exit_code`].
#[must_use]
pub fn dispatch(args: &Args) -> Dispatch {
    if args.opens_start_screen() {
        Dispatch::StartScreen
    } else if args.scenario.is_some() {
        Dispatch::Scenario
    } else if args.smoke {
        Dispatch::Smoke
    } else if args.walker_headless() && args.ui_enabled() {
        Dispatch::HeadlessUi
    } else if args.walker_headless() {
        Dispatch::Headless
    } else {
        Dispatch::Windowed
    }
}
