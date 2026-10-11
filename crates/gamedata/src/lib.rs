// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-gamedata`: the gameplay presentation data layer of the parity track
//! (`docs/decisions/0024-parity-track.md` §3).
//!
//! Typed stores over `tessera-props` and `tessera-wstate` for items, skills, effects,
//! creatures, quests, economy, map and progression records, and the resolution of
//! scenario selectors (`docs/specs/scenario-feed.md` §8) against the user's install.
//! The P1 models live in modules: item tooltips ([`tooltip`]), the creation screens'
//! lists and names ([`chargen`]) and the local player's inventory ([`inventory`]); the P3
//! models are the quest journal ([`quest`]) and the NPC windows ([`npc`]). Layer
//! L3; it depends on `tessera-formats`, `tessera-props`, `tessera-text` and
//! `tessera-wstate` only.
//!
//! Client and tools only: `tessera-server` must never link this crate (LEGAL.md §11.1
//! rule 5; enforced by a CI check on the server's dependency graph).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`GameDataStore`] | The install, read-only: record-family index, tables, selectors, typed records | `scenario-feed.md` §8.6 |
//! | [`RecordIndex`], [`RecordFailure`], [`FailureKind`] | Every object record sorted into the gameplay families in DID order; undecodable records with a categorised reason | §8.3; decision 0024 §5 |
//! | [`SelectorQuery`], [`Family`], [`Attribute`], [`Filter`], [`FilterValue`], [`Count`], [`Bindings`] | The selector language, mirrored from `tessera-net` | §8.2, §8.3 |
//! | [`Candidate`], [`CandidateOutcome`], [`Resolution`], [`ResolveNote`] | Candidates and resolved lists with their notes | §8.4, §8.6 |
//! | [`SplitMix64`], [`sample`], [`canonical_cmp`], [`sort_dedup`], [`apply_count`] | Canonical order and seeded sampling | §8.4, §8.5 |
//! | [`ItemRecord`], [`QuestEntry`] and the per-family methods of [`GameDataStore`] | Minimal typed entry points that later milestones grow | `items.md`, `combat-data.md`, `creatures.md`, `quests.md`, `economy.md`, `map-data.md` |
//! | [`tooltip`] | Item examination tooltip model (P1) | `item-tooltips.md` |
//! | [`chargen`] | Selection and creation lists, names (P1) | `character-screens.md`, `player-character.md` |
//! | [`inventory`] | The local player's inventory model (P1) | `character-panels.md`, `lua-plugin-api.md` §15.1 |
//! | [`quest`] | Quest log, details, progress texts and counters, tracker, quest chat lines, NPC rings and markers, census (P3) | `quest-journal.md` |
//! | [`npc`] | Dialogue, quest offers, vendor, barter and loot entries, coins, wallet, census (P3) | `npc-windows.md` |
//!
//! # Hostile input
//!
//! No byte parser lives here: every record is decoded by `tessera-formats` and
//! `tessera-wstate`, which bound allocation and never panic. This crate adds bounded work:
//! the index refuses more than [`MAX_OBJECT_RECORDS`] records and lists at most
//! [`MAX_LISTED_FAILURES`] failures, a selector yields at most [`MAX_CANDIDATES`]
//! candidates, a scenario at most [`MAX_RESOLVED_TOTAL`] resolved records, a `did` list at
//! most [`MAX_DID_LIST`] DIDs, and sampling works in O(*n* log *n*) for a count of *n*.
//!
//! # Example
//!
//! ```no_run
//! use tessera_gamedata::{Count, Family, GameDataStore, SelectorQuery};
//!
//! # fn main() -> Result<(), tessera_gamedata::GameDataError> {
//! let data = GameDataStore::open("/path/to/install")?;
//! let quests = data.resolve(&SelectorQuery::all(Family::Quest).count(Count::Sample { n: 5, seed: 7 }), &[])?;
//! println!("{} quests", quests.records.len());
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

pub mod chargen;
mod error;
mod filter;
mod index;
pub mod inventory;
pub mod npc;
mod order;
mod query;
pub mod quest;
mod store;
mod tables;
pub mod tooltip;
mod vocab;

pub use error::{GameDataError, Result};
pub use filter::{area_keyword, effect_role_keyword, item_kind_keyword, template_kind_keyword};
pub use index::{
    FailureKind, IndexOptions, MAX_INDEX_THREADS, MAX_LISTED_FAILURES, MAX_OBJECT_RECORDS,
    RecordFailure, RecordIndex,
};
pub use order::{SplitMix64, apply_count, canonical_cmp, sample, sort_dedup};
pub use query::{
    AREA_LEVELS, Alt, Anchor, AttrType, Attribute, Bindings, CREATURE_KINDS, Candidate,
    CandidateOutcome, CandidateStatus, Count, EFFECT_ROLES, Family, Filter, FilterValue,
    ITEM_KINDS, LabelContext, Location, MAX_DID_LIST, Resolution, ResolveNote, SelectorQuery,
    attribute, attributes,
};
pub use store::{
    GENDER_FEMALE, GENDER_MALE, GameDataStore, InstallSource, ItemRecord, MAX_CANDIDATES,
    MAX_DISPLAY_NAME, MAX_RESOLVED_TOTAL, QuestEntry, resolve_with, resolve_without_install,
};
pub use tables::position_location;
pub use vocab::{EQUIPMENT_SLOT_LABEL, ICON_LAYER_PREFIX};

/// Types of other crates that appear in this crate's API.
pub use tessera_formats::Did;
