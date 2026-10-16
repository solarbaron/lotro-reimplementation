// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-gamedata`: the gameplay presentation data layer of the parity track
//! (`docs/decisions/0024-parity-track.md` §3).
//!
//! Typed stores over `tessera-props` and `tessera-wstate` for items, skills, effects,
//! creatures, quests, economy, map and progression records, and the resolution of
//! scenario selectors (`docs/specs/scenario-feed.md` §8) against the user's install.
//! The P1 models live in modules: item tooltips ([`tooltip`]), the creation screens'
//! lists and names ([`chargen`]) and the local player's inventory ([`inventory`]); P2 adds
//! skill and effect tooltips to [`tooltip`] and the combat data of [`combat`]; the P3
//! models are the quest journal ([`quest`]) and the NPC windows ([`npc`]); the P5 models
//! are the map windows ([`map`]), the travel window ([`travel`]), the instance finder
//! ([`instance`]) and mounts ([`mount`]); the P6 models are the crafting panels
//! ([`crafting`]) and the progression panels ([`progression`]). Layer
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
//! | [`quest`] | Quest log, details, progress texts and counters, tracker, quest chat lines, NPC rings and markers, census (P3) | `quest-journal.md` |
//! | [`npc`] | Dialogue, quest offers, vendor, barter and loot entries, coins, wallet, census (P3) | `npc-windows.md` |
//! | [`map`] | Map tree, every map region with its census category and test point, table notes of one map, icon tables, place lookup (P5) | `map-windows.md`, `map-data.md` §3.4, §4.8 |
//! | [`travel`] | Travel window rows, grouping, search, selection display, travel presentation, destination census (P5) | `travel-windows.md` §4, §5.2, §8.1 |
//! | [`instance`] | Instance-finder tree with B1–B3, menus, join and queue displays, instance-travel window, template census (P5) | `travel-windows.md` §4.11, §6, §8.2; `instances.md` §15.1 |
//! | [`mount`] | Mount effect name, icon, type, war-steed flag and plugin class; offered mounts by body; gait multipliers; mount-effect census (P5) | `mounts.md` §2, §14.10, §14.11; `game-controls.md` §5 |
//! | [`GameDataStore::map_regions`], [`GameDataStore::mount_types`], [`GameDataStore::scan_objects`], [`ObjectScan`] | P5 entry points: the `map-region` family, the effect attributes `mount-type` and `war-steed`, record scans for the censuses | `scenario-feed.md` §8.3 (P5 travel) |
//! | [`crafting`] | Profession tabs, recipe tree, tier marks, search, recipe page, quantity and craft request, crafting bar and result lines, scroll rows, crafting trainers, gathering nodes and facilities, census (P6) | `crafting-panels.md` §4–§10; `economy.md` §5, §11.5 |
//! | [`progression`] | Experience bar and level-up rule, trait-tree pages, trait tooltips, trainer rows, trait-tree census (P6, provisional) | `combat-data.md` §8, §9; `player-character.md` §9, §13; `scenario-feed.md` §6.6 |
//! | [`Family::Vocation`], [`Family::CraftGuild`], the P6 [`Attribute`]s, [`LabelContext::TraitBranch`], [`LabelContext::RecipeCategory`] | P6 selector families, attributes and label contexts; recipe scrolls' recipe rows in [`tooltip`] | `scenario-feed.md` §8.3, §8.6 (P6 progression); `crafting-panels.md` §7 |
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
pub mod crafting;
mod error;
mod filter;
mod index;
pub mod instance;
pub mod inventory;
pub mod map;
pub mod mount;
pub mod npc;
mod order;
pub mod progression;
mod query;
pub mod quest;
mod store;
mod tables;
pub mod tooltip;
pub mod travel;
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
    ITEM_KINDS, LabelContext, Location, MAP_REGION_KINDS, MAX_DID_LIST, Resolution, ResolveNote,
    SelectorQuery, attribute, attributes,
};
pub use store::{
    GENDER_FEMALE, GENDER_MALE, GameDataStore, InstallSource, ItemRecord, MAX_CANDIDATES,
    MAX_DISPLAY_NAME, MAX_RESOLVED_TOTAL, MAX_SCAN_FAILURES, ObjectScan, QuestEntry, resolve_with,
    resolve_without_install,
};
pub use tables::position_location;
pub use vocab::{EQUIPMENT_SLOT_LABEL, ICON_LAYER_PREFIX};

/// Types of other crates that appear in this crate's API.
pub use tessera_formats::Did;
