// SPDX-License-Identifier: GPL-3.0-or-later
//! The character controller (`collision.md` §10–§12): parameter layering, float height,
//! walkable/slide/wall contacts, step-up, the fixed 1/60 s step, and the water states of
//! `water.md` §7.4.
//!
//! | Item | Spec |
//! |---|---|
//! | [`MovementLayer`], [`MovementParams`], [`MovementPids`], [`MOVEMENT_FIELD_NAMES`], [`controller_set`], [`ControllerKind`] | §10.1, §10.2 (the movement parameter unit, controller sets) |
//! | [`PlayerMovementRecord`], [`LayerKind`], [`MoveToLayer`] | §10.3 (player movement record), read through `tessera-props` |
//! | [`LayerConditions`], [`PlayerMovementRecord::compose`], [`ComposedParams`] | §11.1 (layering; worked example §14.6) |
//! | [`float_height`] | §10.5, §11.2 |
//! | [`MovementState`], [`Support`] | §10.4 |
//! | [`CharacterShape`] | §10.6 (capsule × model scale), §11.3 (support probe radius) |
//! | [`classify`], [`ContactClass`], [`best_support`], [`is_player_instance`] | §11.3 |
//! | [`Controller`], [`MoveInput`], [`MoverState`], [`StepEnv`], [`STEP_SECONDS`] | §12.2–§12.3; decision 0013 |
//! | [`WaterQuery`], [`LiquidWater`], [`NoWater`] (and `WaterQuery` for `CollisionSet` / `CollisionView`) | §12.3 step 7, `water.md` §7.4 |
//!
//! **Using it.** Read the player movement record once ([`PlayerMovementRecord::locate`] or
//! [`PlayerMovementRecord::from_set`]), build a [`Controller`] with the avatar's capsule and
//! conditions, and call [`Controller::step`] a whole number of times per frame (at most
//! [`MAX_STEPS_PER_FRAME`]), each with the latched input and a [`StepEnv`] over the
//! resident collision sets ([`StepEnv::from_view`] over a [`crate::geometry::CollisionView`]). The step takes no length:
//! every caller steps exactly [`STEP_SECONDS`] (decision 0013). Same start state, same
//! per-step inputs and same sets give the same states, bit for bit.

mod contact;
mod params;
mod shape;
mod state;
mod step;
mod water;

pub use contact::{
    COS_OTHER_WALKABLE, COS_PLAYER_WALKABLE, COS_WALL, ContactClass, NORMAL_MERGE_TOLERANCE,
    OTHER_WALKABLE_DEGREES, PLAYER_WALKABLE_DEGREES, SUPPORT_MIN_RADIUS, WALL_DEGREES,
    best_support, chosen_normal, classify, is_player_instance, normals_merge, walkable_cos,
};
pub use params::{
    ComposedParams, ControllerKind, FALLBACK_FLOAT_HEIGHT, LayerConditions, LayerKind, LayerList,
    MAX_ACCELERATION, MAX_FLOAT_HEIGHT, MAX_GRAVITY, MAX_JUMP_ACCELERATION, MAX_MOVE_TO_FIELDS,
    MAX_RAMP_TIME, MAX_SPEED, MAX_STEP_UP, MOVEMENT_FIELD_NAMES, MoveToLayer, MovementLayer,
    MovementParams, MovementPids, PHYSICS_CONTROLLER_MAP, PlayerMovementRecord, TROLL_SPECIES,
    controller_set, float_height, is_move_to_scalar,
};
pub use shape::{CharacterShape, MAX_CAPSULE_LENGTH, MAX_CAPSULE_RADIUS};
pub use state::{MovementState, Support, WaterDepth};
pub use step::{
    Controller, ControllerConfig, FLOAT_TOLERANCE, MAX_DEPENETRATION, MAX_SLIDE_ITERATIONS,
    MAX_STEP_DISPLACEMENT, MAX_STEPS_PER_FRAME, MIN_SINK_ACCELERATION, MoveInput, MoverState,
    SCRATCH_KEEP, SNAP_DISTANCE, STEP_OVER_FRACTION, STEP_SECONDS, STEP_UP_MIN_RISE,
    STEPS_PER_SECOND, SUPPORT_MARGIN, StepEnv, StepOutcome, StepReport, WaterContact,
};
pub use water::{
    LiquidWater, MAX_COLUMN_BOXES, NoWater, SURFACE_SEARCH_RANGE, WaterQuery, WaterSample,
};
