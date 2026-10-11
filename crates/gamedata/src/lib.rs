// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-gamedata`: the gameplay presentation data layer of the parity track
//! (`docs/decisions/0024-parity-track.md` §3).
//!
//! Typed stores over `tessera-props` and `tessera-wstate` for items, skills, effects,
//! creatures, quests, economy, map and progression records, and the resolution of
//! scenario selectors (`docs/specs/scenario-feed.md` §8) against the user's install.
//! The P1 models live in modules: item tooltips ([`tooltip`]), the creation screens'
//! lists and names ([`chargen`]) and the local player's inventory ([`inventory`]); P2 adds
//! skill and effect tooltips to [`tooltip`] and the combat data of [`combat`]. Layer
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
//! | [`tooltip`] (P2) | Skill and effect examination tooltips, effect lines, durations and timers, the census categories; item effect lines, skill-item rows and the `SkillGrantingItem` comparison | `skill-tooltips.md`; `item-tooltips.md` §6.16, §6.20, §8.3 |
//! | [`combat`] | Typed skill and effect records; combat-panel data (consideration and name colours, vital display overrides, target-frame decoration, effect icons, cast-bar labels, defeat records, floating-text settings); combat-visual script references | `combat-data.md`, `combat-ui.md`, `combat-visuals.md`, `game-controls.md` §5.8, §7.1, §7.5, `interaction.md` §3.8, §3.9, §8 |
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
pub mod combat;
mod error;
mod filter;
mod index;
pub mod inventory;
mod order;
mod query;
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
