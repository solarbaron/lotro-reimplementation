// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-physics`: collision and movement for the offline world (ROADMAP M4.5).
//!
//! Specs: `docs/specs/collision.md` (geometry §3–§9, movement data §10–§11, Tessera's
//! controller §12) and `docs/specs/water.md` §7.4 (water movement states).
//!
//! | Module | Purpose |
//! |---|---|
//! | [`geometry`] | Terrain and ceiling collision triangles with the client's numbering, the terrain radius and plane queries, cell volumes and point-in-cell, per-landblock collision sets, Tessera's bounding-volume hierarchy and capsule sweeps |
//! | [`controller`] | The character controller: parameter layering, float height, contacts, step-up, fixed 1/60 s step, water states; the war-steed controller (`mounts.md` §5) |
//!
//! Layer L3 (`docs/ARCHITECTURE.md` §3.1): depends only on `tessera-formats` and
//! `tessera-props`.

#![warn(missing_docs)]

pub mod controller;
mod error;
pub mod geometry;

/// The controller's entry points: everything needed to build a [`Controller`], step it and
/// read the result (the rest is under [`controller`]).
pub use controller::{
    CharacterShape, Controller, ControllerConfig, LayerConditions, LiquidWater,
    MAX_STEPS_PER_FRAME, MoveInput, MovementPids, MovementState, MoverState, NoWater,
    PlayerMovementRecord, STEP_SECONDS, STEPS_PER_SECOND, StepEnv, StepOutcome, StepReport,
    WaterQuery,
};
pub use error::{PhysicsError, Result};
