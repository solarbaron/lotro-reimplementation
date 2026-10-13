// SPDX-License-Identifier: GPL-3.0-or-later
//! Travel, the instance finder and mounts in the client (decision 0024 P5, client half
//! only; `docs/specs/travel-windows.md`, `lua-plugin-api.md` §15.5, `walker.md` §16.1).
//!
//! The client runs no travel, eligibility, queue or mount rule (decision 0024 §2 principle
//! 4): which window opens, its rows, costs, levels and flags, the finder's entries and
//! queue states, the start and end of a travel and which mount the player rides in a
//! scenario come from the scenario feed or a server (`scenario-feed.md`, revision "P5
//! travel"); the client shows them with the install's layouts, texts and icons and sends
//! the player's commands back.
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`state`] | [`state::P5State`]: feed markers, the travel and instance windows, the started travel, the instance, the queue (in the world session, [`crate::net::World::p5`]) | SF §6.5, §9.2 |
//! | [`refs`] | [`P5Refs`]: layouts, roots and row templates by label, the P5 vocabulary | TW §3 |
//! | [`present`] | the windows' views from the state (install texts and names, or literal refs) | TW §4.10, §4.11, §6.8 |
//! | [`ui`] | [`TravelUi`]: the travel window, the finder and its queue page and alert, the instance-travel window, the end-travel box and the loading screen on the game UI | TW §4–§7 |
//! | [`mount`] | [`SessionMount`]: the session's mount model and the plugin mount source | LPA §15.5 |
//! | [`hub`] | [`TravelHub`]: loading off the frame, the frame's work, walker mount requests | — |
//!
//! The commands of `travel-windows.md` §6.6 (`leaveinstance`) and `walker.md` §16.3
//! (`ride`) are client built-ins of the dispatcher (`crate::commands`); the map windows
//! and place lookup are [`crate::map`].

pub mod hub;
pub mod mount;
pub mod present;
pub mod refs;
pub mod state;
pub mod ui;

pub use hub::{TravelHub, WalkerMount, parse_travel_panel};
pub use mount::{MountPlayer, SessionMount};
pub use refs::{P5Refs, P5Window};
pub use state::{P5Change, P5State};
pub use ui::{TravelContext, TravelUi};
