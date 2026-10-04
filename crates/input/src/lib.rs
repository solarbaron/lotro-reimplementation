// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-input`: windowing-agnostic input for the Tessera client
//! (ARCH §16, `docs/specs/keymap.md`).
//!
//! Pipeline:
//!
//! ```text
//! window/device events ──(winit_adapter, feature "winit")──► RawEvent
//!     ──► InputMapper (keymap bindings, context stack) ──► ActionState per action
//!     ──► StandardActions::intent / look ──► MoveIntent
//!     ──► FixedTimestep ──► FlyController / WalkController (HeightQuery)
//! ```
//!
//! * [`Registry`]: the actions and contexts code defines
//!   ([`default_registry`] holds Tessera's built-ins).
//! * [`Keymap`]: Tessera's own TOML keymap file (`keymap.toml` next to
//!   the settings file), laid over [`default_keymap`].
//! * [`InputMapper`]: resolves presses through the active contexts
//!   (priority, then most recent; opaque contexts swallow), supports
//!   modifiers, simultaneous chords (`MouseLeft+MouseRight`), sequences
//!   (`Ctrl+K Ctrl+C`), modifier holds (`Shift`) and analog axes.
//! * Controllers: [`FlyController`] (free camera) and [`WalkController`]
//!   (ground following with gravity, step-up, slope limit and jump), both
//!   meant for a fixed tick ([`FixedTimestep`], 30 Hz by default).
//!
//! The crate has no window-system dependency unless the `winit` feature is
//! enabled.

/// The vector math crate used in the controller API.
pub use glam;

mod action;
mod chord;
pub mod controller;
mod defaults;
mod device;
mod error;
mod keymap;
mod mapper;
mod timestep;
#[cfg(feature = "winit")]
pub mod winit_adapter;

pub use action::{
    ActionDef, ActionId, ActionKind, ContextDef, ContextId, MAX_ACTIONS, MAX_CONTEXTS,
    MAX_NAME_LEN, ModifierPolicy, Registry, is_valid_name,
};
pub use chord::{Chord, MAX_CHORD_STROKES, MAX_CHORD_TEXT, MAX_STROKE_BUTTONS, Stroke};
pub use controller::{
    FlyConfig, FlyController, GroundState, HeightQuery, IntentLatch, MoveIntent, WalkConfig,
    WalkController, WalkEvent,
};
pub use defaults::{LookSettings, StandardActions, contexts, default_keymap, default_registry};
pub use device::{AnalogAxis, Button, KeyCode, Modifiers, MouseButton, PadButton, WheelDirection};
pub use error::{InputError, KeymapWarning, Result};
pub use keymap::{
    BindingSpec, DEFAULT_SEQUENCE_TIMEOUT_MS, FORMAT_TAG, KEYMAP_FILE_ENV, KEYMAP_FILE_NAME,
    KEYMAP_VERSION, Keymap, KeymapOptions, MAX_ANALOG_SCALE, MAX_BINDINGS_PER_ACTION, MAX_DEADZONE,
    MAX_KEYMAP_ACTIONS, MAX_KEYMAP_CONTEXTS, MAX_KEYMAP_SIZE, MAX_KEYMAP_WARNINGS, ParsedKeymap,
    SEQUENCE_TIMEOUT_RANGE_MS, default_keymap_path,
};
pub use mapper::{
    ANALOG_PRESS_THRESHOLD, ANALOG_RELEASE_HYSTERESIS, ActionState, InputMapper, MAX_HELD_BUTTONS,
    MAX_HOLDS, MAX_MOTION_PER_EVENT, MAX_TEXT_PER_FRAME, MAX_WHEEL_CLICKS_PER_EVENT, RawEvent,
};
pub use timestep::{DEFAULT_MAX_TICKS_PER_FRAME, DEFAULT_TICK_RATE, FixedTimestep};
