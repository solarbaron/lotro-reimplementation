// SPDX-License-Identifier: GPL-3.0-or-later
//! Characters, inventory and equipment in the client (decision 0024 P1): the local
//! player's state fed from the session's events, the install facts the panels read, the
//! plugin inventory views, item tooltips and the character panel with the six bag
//! windows.
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`state`] | [`PlayerState`]: the gamedata inventory model, scripted properties, header, vitals, wallet, worn look, fed by session events | `character-panels.md` §9; `scenario-feed.md` §9.2 |
//! | [`hub`] | [`CharacterHub`]: loads the facts off the frame, feeds the plugin views each frame, keeps changes for the UI | `lua-plugin-api.md` §15 |
//! | [`data`] | [`CharacterData`]: bag layout, paperdoll table, slot rules, tooltip engine, per-record item facts | `character-panels.md` §3.3, §4.2, §6; `icon-layers.md` §3 |
//! | [`plugin`] | [`PluginInventory`]: the `Backpack`/`Equipment`/`Item` source and events | `lua-plugin-api.md` §15.1 |
//! | [`tooltip`] | item tooltip requests from the model and their conversion for the UI panel | `item-tooltips.md` §3–§11 |
//! | [`settings`] | the P1 entries of Tessera's settings: the comparison switch, the bag windows' column settings | `item-tooltips.md` §8.3; `character-panels.md` §4.1 |
//! | [`routed`] | the `keymap.md` §8.3 commands the UI declined that open other windows (`USE`, wallet, quest log, traits, skills), queued for their hubs | `keymap.md` §8.3; `npc-windows.md` §3.1, §9.4; `quest-journal.md` §4.1; `progression-panels.md` §1.4 |
//! | [`panel`] | the character panel (`Journal_Window`), the bag windows, item-slot interactions as session commands, hover and pinned tooltips | `character-panels.md` §2–§6; `ui-behaviour.md` §6, §7 |

pub mod data;
pub mod hub;
pub mod panel;
pub mod plugin;
pub mod routed;
pub mod settings;
pub mod state;
pub mod tooltip;

pub use data::{CharacterData, CharacterDataError, ItemFacts};
pub use hub::CharacterHub;
pub use plugin::{InventoryView, PluginInventory};
pub use state::{PlayerChange, PlayerState, VitalKind};
