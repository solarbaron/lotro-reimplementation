// SPDX-License-Identifier: GPL-3.0-or-later
//! NPC windows in the client (decision 0024 P3, `docs/specs/npc-windows.md`): using an
//! NPC, the dialogue windows (conversation, choice, NPC comment), the quest offer, the
//! vendor window with its quantity dialog, sell and buy-back pages, the barter window, the
//! loot window, the wallet window and the plugin wallet views.
//!
//! The client runs no NPC rule (decision 0024 §2 principle 4): which window opens, its
//! entries, prices, quantities and reward amounts come from the scenario feed or a server
//! (`scenario-feed.md` §5.6, §6.3, §9.2); the client shows them with the install's
//! layouts, texts and icons and sends the player's commands back.
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`state`] | [`state::NpcWindows`]: the open feed windows and their content from the session's events (in the player state) | NW §10.1 |
//! | [`commands`] | window actions → session commands; the one entry point for using an entity ([`use_entity`]) with the use range and its cylinders, `USE` on the selection | NW §3, §10.1; `interaction.md` §3.4.1 |
//! | [`prefs`] | Tessera's saved game-window choices (`[GameWindows]`): read off the frame, written by one writer thread | NW §5.2; `quest-journal.md` §4.8, §8 |
//! | [`shapes`] | [`shapes::UseShapes`]: the entities' use-range cylinders, read from their setups on a worker | `interaction.md` §3.4.1 I6a |
//! | [`data`] | [`NpcData`]: `tessera-gamedata`'s NPC model over the P1 facts, wallet item values, use ranges, sale flags | NW §5–§9, §11 |
//! | [`refs`] | [`NpcRefs`]: layouts, roots and row templates by label, the P3 vocabulary | NW §4 |
//! | [`present`] | the windows' views from the state (install texts and icons, or literal refs) | NW §5–§9 |
//! | [`texts`] | labels game code sets (Loot All, coin counts and tooltips) | NW §8.3, §9.2 |
//! | [`ui`] | [`NpcUi`]: the window instances on the game UI, events, tooltips, closes | NW §4–§9 |
//! | [`wallet`] | [`SessionWallet`]: the plugin wallet views' model and source | `lua-plugin-api.md` §15.3 |
//! | [`hub`] | [`NpcHub`]: loading off the frame, the frame's work in the windowed client | — |
//!
//! # Using an NPC
//!
//! [`use_entity`] is the client's one entry point: it checks the use range and sends
//! `UseEntity`. Its callers are the input command `USE` on the selection (the keymap action
//! `use_selection`, default keyboard U, `keymap.md` §8.3; routed to [`NpcHub::frame`] by
//! [`crate::character::routed`]) and P2's world clicks, a left double-click and a right
//! click ([`NpcHub::use_selection`], `npc-windows.md` §3.1). The scenario input script's
//! `use` is sent by the input player itself.

pub mod commands;
pub mod data;
pub mod hub;
pub mod prefs;
pub mod present;
pub mod refs;
pub mod shapes;
pub mod state;
pub mod texts;
pub mod ui;
pub mod wallet;

pub use commands::{UseOutcome, command_of, use_entity};
pub use data::{NpcData, NpcDataError};
pub use hub::{NpcHub, parse_npc_panel};
pub use refs::NpcRefs;
pub use state::{FeedKind, NpcChange, NpcWindows, WindowContent};
pub use ui::NpcUi;
pub use wallet::SessionWallet;
