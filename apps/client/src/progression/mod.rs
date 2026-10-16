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
//! | [`state`] | [`state::P6State`]: level, experience, traits, skills, trainers, professions, recipes, the crafting bar (in [`crate::net::World::p6`]) | SF §6.6 |
//! | [`data`] | [`data::P6Data`]: the install facts the panels read (`tessera-gamedata`'s progression and crafting presenters) | CP, combat-data.md |
//! | [`present`] | the UI window views built from the state, the presenters and the player's holdings | CP §4–§8; progression windows provisional |
//! | [`commands`] | the `--open-panel` names and the mapping from a window's action to its session command | CP §13, SF §9.4 |
//!
//! **Provisional (`progression-panels.md` is not in the tree, decision 0037 Open).** The
//! crafting half rests on the accepted `crafting-panels.md`. The progression half (the
//! experience bar, the level-up presentation, the trait panel and skill training) follows
//! the accepted feed facts (SF §6.6), `combat-data.md` §8.5 for the trait records and the
//! Tessera rules of `tessera-ui::game::progression`; sections those windows would take from
//! `progression-panels.md` (layout labels, exact texts, the trait-cell grid) are marked
//! `OPEN` where they arise.
//!
//! This layer is the client's presentation logic: it builds the UI window views from the
//! session state and the install, and maps a window's action back to its command. It is
//! exercised in process by `apps/client/tests/p6_panels.rs` (decision 0037 §3 item 4) and by
//! the unit tests here.
//!
//! > **OPEN** (remaining P6 client wiring, built on this layer): the runtime frame glue that
//! > attaches the live windows and drives them each frame (windowed and headless `--ui`), the
//! > walker's gathering-hotspot actors beyond the P3 use / P2 induction-bar / P3 loot paths a
//! > gathering node already drives, and feeding [`state::P6Change`] into
//! > `tessera_plugins::ProgressionModel` for the `lua-plugin-api.md` §15.6 plugin source.
//! > As part of this, the P6 `--open-panel` names ([`commands::parse_p6_panel`]: `crafting`,
//! > `crafting-recipe`, `vocation-trainer`, `profession-trainer`, `craftguild-trainer`) are
//! > parsed but not yet routed by the client to open a window, so `--open-panel crafting` opens
//! > nothing today (`crafting-panels.md` §3 Tessera rule, marked OPEN there). The P6 exit
//! > criteria do not depend on this: decision 0037 §2 captures every P6 golden in-process on
//! > `HeadlessDriver`, and the `--open-panel` names serve inspection only.

pub mod commands;
pub mod data;
pub mod present;
pub mod state;

pub use commands::{
    P6Handles, P6Panel, craft_command, craft_command_of, craft_trainer_command, parse_p6_panel,
    progression_command,
};
pub use data::{ApiKeys, NoData, P6Data};
pub use present::{
    CraftPresentation, NoIcons, P6Icons, PlayerHoldings, craft_panel_view, craft_trainer_view,
    experience_view, recipe_page_view, skill_trainer_view, trait_panel_view,
};
pub use state::{P6Change, P6State};
