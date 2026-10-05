// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-entity`: the client's entity storage, owned by the main thread
//! (`docs/ARCHITECTURE.md` §20.3, §21).
//!
//! A plain project-owned store. Decision record `docs/decisions/0008-entity-storage.md`
//! made the choice against a standalone `bevy_ecs`, as decision 0005 requires; the
//! benchmark and its numbers are there.
//!
//! | Item | Purpose |
//! |---|---|
//! | [`Entity`], [`Entities`], [`MAX_ENTITIES`] | Generational IDs (slot index + generation), their allocator with last-in first-out slot reuse and a hard slot cap |
//! | [`Column`] | One component type as a sparse set: O(1) insert, lookup and remove, dense iteration |
//! | [`EntityStore`] | The client's component sets: [`Transform`], [`NetId`], render instance `R`, animation instance `A`, [`LandblockTag`] membership of streamed statics; their queries |
//! | [`Changes`] | Per-frame change and despawn lists for the renderer and the animation scheduler, with removed payloads |
//! | [`Deferred`], [`DeferredOp`], [`Applied`] | Structural changes recorded during a query and applied after it |
//! | [`EntityError`], [`Rejected`] | Why an operation was refused; a refused render or animation payload handed back with the reason |
//!
//! # Example: a frame
//!
//! ```
//! use tessera_entity::{Changes, EntityStore, LandblockTag, NetId, Transform};
//!
//! // R = the renderer's instance handle, A = the animation state (here plain numbers).
//! let mut store: EntityStore<u32, f32> = EntityStore::new();
//! let mut changes = Changes::new();
//!
//! // Streaming: a landblock arrives with its statics, another leaves.
//! let block = LandblockTag(0x8001_7376);
//! for i in 0..3 {
//!     store
//!         .spawn_static(block, Transform::from_position([f64::from(i), 0.0, 0.0]), Some(i))
//!         .map_err(|refused| refused.error)?; // `refused.value`: an instance to release
//! }
//! // Network: a character appears and moves.
//! let npc = store.spawn_networked(NetId(42), Transform::IDENTITY)?;
//! store.set_render(npc, 100).map_err(|r| r.error)?;
//! store.set_anim(npc, 0.0).map_err(|r| r.error)?;
//! store.set_transform(npc, Transform::from_position_yaw([1.0, 2.0, 0.0], 0.5))?;
//!
//! // Animation: advance every animated entity.
//! for (_, clock, _) in store.animated_mut() {
//!     *clock += 1.0 / 60.0;
//! }
//!
//! // Render and animation mirror the store through the change lists.
//! store.take_changes(&mut changes);
//! assert_eq!(changes.render_added.len(), 4);
//! assert_eq!(changes.moved, vec![npc]);
//!
//! store.despawn_landblock(block);
//! store.take_changes(&mut changes);
//! assert_eq!(changes.render_removed.len(), 3); // payloads to release
//! # Ok::<(), tessera_entity::EntityError>(())
//! ```
//!
//! # Threading
//!
//! [`EntityStore`] is neither `Send` nor `Sync`: it lives on the main thread
//! (ARCHITECTURE §21: no shared mutable game state outside it). Pose sampling and other
//! worker jobs receive copies or immutable `Arc`s, never the store.
//!
//! # Hostile input
//!
//! Network events reach the store, so nothing here panics on any sequence of calls or
//! IDs. IDs are generation-checked (a stale ID never reaches the slot's newer occupant,
//! in the store or in a [`Column`]); network IDs are unique among live entities; a
//! payload an operation refuses comes back in its [`Rejected`] error.
//!
//! The number of slots is capped by [`StoreConfig::max_entities`] (at most
//! [`MAX_ENTITIES`]), which bounds the slots, component columns and indexes. The pending
//! change lists are not bounded by the cap: they grow by an entry per operation since the
//! last [`EntityStore::take_changes`] (spawn/despawn churn on one reused slot adds
//! entries every time). Memory there is proportional to the events applied, so the
//! client takes the changes every frame and bounds the events it applies per frame.
//!
//! # Optional feature
//!
//! `bevy-bench` compiles the standalone `bevy_ecs` used only by the decision benchmark
//! (`examples/entity_bench`). It is off by default and nothing in the library uses it.

#![warn(missing_docs)]

mod changes;
mod column;
mod entity;
mod error;
mod store;

pub use changes::{Applied, Changes, Deferred, DeferredOp};
pub use column::Column;
pub use entity::{Entities, Entity, MAX_ENTITIES};
pub use error::{EntityError, Rejected, Result};
pub use store::{EntityStore, LandblockTag, NetId, StoreConfig, Transform};
