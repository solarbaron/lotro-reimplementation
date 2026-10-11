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
//! | [`commands`] | window actions → session commands; the one entry point for using an entity ([`use_entity`]) with the use range, the input action "use nearest NPC in range" | NW §3, §10.1; `interaction.md` §3.4.1 |
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
//! `UseEntity`. Its callers are the input action "use nearest NPC in range"
//! ([`commands::USE_KEY`], keyboard U) and, after the P2 merge, the selection's `USE`, left
//! double-click and right click (`npc-windows.md` §3.1). The scenario input script's `use`
//! is sent by the input player itself.

pub mod commands;
pub mod data;
pub mod hub;
pub mod present;
pub mod refs;
pub mod state;
pub mod texts;
pub mod ui;
pub mod wallet;

/// The NPC windows' state as the player state holds it: [`state::NpcWindows`] (reached
/// through `Deref`) together with the quest journal's session state (event routing hook
/// of decision 0024 P3; [`crate::quests::session`] module docs).
pub use crate::quests::SessionWindows as NpcWindows;
pub use commands::{UseOutcome, command_of, use_entity};
pub use data::{NpcData, NpcDataError};
pub use hub::{NpcHub, parse_npc_panel};
pub use refs::NpcRefs;
pub use state::{FeedKind, NpcChange, WindowContent};
pub use ui::NpcUi;
pub use wallet::SessionWallet;
