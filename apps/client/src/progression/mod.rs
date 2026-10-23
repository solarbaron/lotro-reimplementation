// SPDX-License-Identifier: GPL-3.0-or-later
//! Progression and crafting in the client (decision 0024 P6, client half only).
//!
//! The client runs no experience, level, training, trait or craft rule (decision 0024 §2
//! principle 4): experience, levels, ranks, points, trained skills, professions, recipes and
//! craft results come from the scenario feed or a server (`scenario-feed.md` §6.6); the
//! client shows them with the install's layouts, texts and icons ([`crate::progression::data`]),
//! and sends the player's commands back ([`commands`]).
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`state`] | [`state::P6State`]: level, experience, traits, skills, trainers, professions, recipes, the crafting bar (in [`crate::net::World::p6`]) | PP §2–§4, SF §6.6 |
//! | [`data`] | [`data::P6Data`]: the install facts the panels read (`tessera-gamedata`'s progression and crafting presenters) | PP, CP, combat-data.md |
//! | [`present`] | the UI window views built from the state, the presenters and the player's holdings | PP §2–§4, CP §4–§8 |
//! | [`commands`] | the `--open-panel` names and the mapping from a window's action to its session command | PP §3.7/§4.4, CP §13, SF §9.4 |
//! | [`refs`] | [`refs::P6Refs`]: the windows' install names (layouts, roots, templates, vocabularies) | CP §3, §4.2 |
//! | [`ui`] | [`ui::P6Ui`]: the live windows on the game UI, filled each frame from the state | PP §2–§4, CP §4–§8 |
//! | [`hub`] | [`hub::P6Hub`]: the runtime glue (loading, the frame's changes, toggles, `--open-panel`) | PP §1.4, CP §3, §4.1, §12 |
//! | [`plugin`] | [`plugin::SessionProgression`]: the plugins' progression and crafting views | `lua-plugin-api.md` §15.6 |
//!
//! PP is `progression-panels.md` (Accepted), CP is `crafting-panels.md`, SF is
//! `scenario-feed.md`. The crafting half rests on `crafting-panels.md`; the progression half
//! (the experience bar §2, the level-up presentation §2.3, the trait panel §3 and skill
//! training §4) rests on `progression-panels.md`, with `combat-data.md` §8.5 for the trait
//! records and the fallback-window rules of `tessera-ui::game::progression` (PP §6).
//!
//! This layer is the client's presentation logic: it builds the UI window views from the
//! session state and the install, and maps a window's action back to its command. The
//! runtime ([`crate::runtime::Runtime::tick`]) feeds each frame's [`state::P6Change`]s to
//! [`hub::P6Hub::after_events`] (the plugin views and the level-up notice), and the
//! per-frame window pass of the game UI (`ui_host::client::window_frames`, windowed and
//! headless `--ui`) runs [`hub::P6Hub::frame`]. The `--open-panel` names of
//! [`commands::parse_p6_panel`] open the windows once the player is in the world; the
//! `toggle_crafting` and `toggle_crafting_1` … `3` actions toggle the crafting panel
//! (CP §4.1, §12). Trainers open on the feed's `TrainerShown` and `CraftTrainerShown`, which
//! the feed sends when the player chooses an NPC's trainer option (NW §5.3); gathering nodes
//! are feed actors used through the P3 use path (CP §9.1 Tessera rule), which the walker draws
//! like any feed actor. It is exercised in process by `apps/client/tests/p6_panels.rs`
//! (decision 0037 §3 item 4), `apps/client/tests/p6_windows.rs` and the unit tests here.
//!
//! The windows resolve no icon yet ([`present::NoIcons`]; the P1 icon layer is not wired to
//! [`present::P6Icons`]), and a recipe row's cooldown timer stays empty (the module OPENs of
//! [`present`]).

pub mod commands;
pub mod data;
pub mod hub;
pub mod plugin;
pub mod present;
pub mod refs;
pub mod state;
pub mod ui;

pub use commands::{
    P6Handles, P6Panel, craft_command, craft_command_of, craft_trainer_command, parse_p6_panel,
    progression_command,
};
pub use data::{ApiKeys, NoData, P6Data};
pub use hub::P6Hub;
pub use plugin::{ProgressionPlayer, SessionProgression};
pub use present::{
    CraftPresentation, NoIcons, P6Icons, PlayerHoldings, craft_panel_view, craft_trainer_view,
    experience_view, recipe_page_view, skill_trainer_view, trait_panel_view,
};
pub use state::{P6Change, P6State};
