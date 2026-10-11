// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-input`: windowing-agnostic input for the Tessera client
//! (ARCH §16, `docs/specs/keymap.md`, decision 0014).
//!
//! Pipeline:
//!
//! ```text
//! window/device events ──(winit_adapter, feature "winit")──► RawEvent
//!     ──► InputMapper (keymap bindings, UI band + game band) ──► ActionState per action
//!          └─(handle_with)──► UiSink: offered Starts, Repeats, Ends, text, moves
//!     ──► StandardActions::intent / look ──► MoveIntent
//!     ──► FixedTimestep ──► FlyController / WalkController (HeightQuery)
//! ```
//!
//! * [`Registry`]: the actions and contexts code defines
//!   ([`default_registry`] holds Tessera's built-ins, §8).
//! * [`Keymap`]: Tessera's own TOML keymap file (`keymap.toml` next to
//!   the settings file), laid over [`default_keymap`].
//! * [`InputMapper`]: resolves presses and releases through the UI band and
//!   the game band (pass-through contexts, `opaque`, `opaque_keys`), with
//!   modifiers, simultaneous chords (`MouseLeft+MouseRight`), sequences
//!   (`Ctrl+K Ctrl+C`), modifier holds (`Shift`), analog axes and the
//!   activation filters `press`, `release`, `tap`, `double` and
//!   `double_nearby`. With a UI attached it offers each event to a
//!   [`UiSink`] in order (§7.8); the sink's verdict decides who owns a hold.
//! * Controllers: [`FlyController`] (free camera) and [`WalkController`]
//!   (ground following with gravity, step-up, slope limit and jump), both
//!   meant for a fixed tick ([`FixedTimestep`], 30 Hz by default).
//! * [`import`]: a read-only importer of the original client's key map
//!   (§12).
//!
//! The crate has no window-system dependency unless the `winit` feature is
//! enabled, and does not depend on the UI crate: the client implements
//! [`UiSink`].

/// The vector math crate used in the controller API.
pub use glam;

mod action;
mod chord;
pub mod combat;
pub mod commands;
pub mod controller;
mod defaults;
mod device;
mod error;
pub mod import;
mod keymap;
mod mapper;
mod offer;
mod timestep;
#[cfg(feature = "winit")]
pub mod winit_adapter;

pub use action::{
    ActionDef, ActionId, ActionKind, Band, ContextDef, ContextId, MAX_ACTIONS, MAX_CONTEXTS,
    MAX_NAME_LEN, ModifierPolicy, Registry, is_valid_name,
};
pub use chord::{Activation, Chord, MAX_CHORD_STROKES, MAX_CHORD_TEXT, MAX_STROKE_BUTTONS, Stroke};
pub use combat::{QUICKSLOT_ACTIONS, QUICKSLOT_COMMANDS, QUICKSLOTS, TARGET_ACTIONS, TargetAction};
pub use controller::{
    FlyConfig, FlyController, GroundState, HeightQuery, IntentLatch, MoveIntent, WalkConfig,
    WalkController, WalkEvent,
};
pub use defaults::{
    LookSettings, StandardActions, UiActions, contexts, default_keymap, default_registry,
};
pub use device::{AnalogAxis, Button, KeyCode, Modifiers, MouseButton, PadButton, WheelDirection};
pub use error::{InputError, KeymapWarning, Result};
pub use keymap::{
    BindingSpec, DEFAULT_SEQUENCE_TIMEOUT_MS, FORMAT_TAG, KEYMAP_FILE_ENV, KEYMAP_FILE_NAME,
    KEYMAP_VERSION, Keymap, KeymapOptions, MAX_ANALOG_SCALE, MAX_BINDINGS_PER_ACTION, MAX_DEADZONE,
    MAX_KEYMAP_ACTIONS, MAX_KEYMAP_CONTEXTS, MAX_KEYMAP_SIZE, MAX_KEYMAP_WARNINGS, ParsedKeymap,
    RETIRED_TEXT_ENTRY_ACTIONS, SEQUENCE_TIMEOUT_RANGE_MS, default_keymap_path,
};
pub use mapper::{
    ANALOG_PRESS_THRESHOLD, ANALOG_RELEASE_HYSTERESIS, ActionState, ActivationTiming,
    DEFAULT_DOUBLE_CLICK_MS, DEFAULT_DOUBLE_CLICK_PX, DOUBLE_CLICK_MS_RANGE, DOUBLE_CLICK_PX_RANGE,
    InputMapper, MAX_HELD_BUTTONS, MAX_HOLDS, MAX_MOTION_PER_EVENT, MAX_PRESS_RECORDS,
    MAX_TEXT_PER_FRAME, MAX_WHEEL_CLICKS_PER_EVENT, RawEvent, scale_pixels,
};
pub use offer::{
    ActionOffer, BandEntry, BandRequest, BandRequests, HoldId, MAX_BAND_ENTRIES,
    MAX_BAND_REQUESTS_PER_OFFER, OfferPhase, OfferedEvent, UiSink, Verdict,
};
pub use timestep::{DEFAULT_MAX_TICKS_PER_FRAME, DEFAULT_TICK_RATE, FixedTimestep};
