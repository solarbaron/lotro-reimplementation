// SPDX-License-Identifier: GPL-3.0-or-later
//! Gameplay record decoders: typed views over the property sets of game objects (0x70
//! records and their 0x79 sets), plus the few gameplay records with their own byte layout.
//!
//! Most gameplay data lives in property sets that the generic decoder of
//! [`crate::property_set`] already reads. The family modules turn those raw trees into typed
//! views. They find properties **by label** through the dictionary at run time (never by
//! hard-coded PID), recognise record kinds by their characteristic properties or by class
//! ancestry, and never hard-code DIDs or class indices of one install. [`common`] holds what
//! the families share: the object-record header, class ancestry, label-resolved property
//! IDs, typed property access, signatures and the DID-reference graph.
//!
//! | Module | Records | Spec |
//! |---|---|---|
//! | [`common`] | shared helpers | `wstate.md` §3.1, `wlib.md` §9, `property-system.md` §2, §6.2, §8.1 |
//! | [`items`] | item records and tables | `items.md` |
//! | [`combat`] | skills, effects, traits, progressions | `combat-data.md` |
//! | [`creatures`] | templates, AI tables, spawning, encounter table (0x14) | `creatures.md` |
//! | [`quests`] | quests, deeds, dialogue, quest map notes (0x0E400000) | `quests.md` |
//! | [`economy`] | loot, crafting, vendors, barter, currency | `economy.md` |
//! | [`map`] | places, map notes, travel, housing | `map-data.md` |
//! | [`particles`] | particle systems (0x39), fork systems (0x44) | `particles.md` |
//! | [`misc`] | compositors, UI scenes, imposters, filters, metadata objects | `misc-records.md` |

pub mod combat;
pub mod common;
pub mod creatures;
pub mod economy;
pub mod items;
pub mod map;
pub mod misc;
pub mod particles;
pub mod quests;
