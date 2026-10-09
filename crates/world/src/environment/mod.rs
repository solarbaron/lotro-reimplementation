// SPDX-License-Identifier: GPL-3.0-or-later
//! The game clock, day-description choice per sky key, keyframe blending and sky objects
//! (`environment.md` §11), and the property hooks that choose an environment (§7). The
//! clock epoch is OPEN, so the start time is settable.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`Calendar`], [`GameClock`], [`ClockReading`] | The game clock *G* with its rate; year, day, time of day, game day number, month | §6, §11.1 |
//! | [`day_hash`], [`weighted_pick`], [`choose_day`], [`choose_day_with_fallback`], [`DayChoice`] | The day description of a sky key on a game day; the Tessera fallback for keys only the day groups list | §11.2, §10 |
//! | [`keyframe_pair`], [`blend_factor`], [`lerp_angle`], [`toward_sun`], [`SkyValues`], [`LightingInputs`], [`crossfade_factor`], [`transition_seconds`], [`outdoor_sun_present`] | Keyframe blending, defaults, "sun present", the sky-key cross-fade and the lighting inputs | §11.3, §11.6 steps 3–4, §11.7 step 9, `lighting.md` §8.1 |
//! | [`EnvState`], [`SunChangeFade`] | The composed state of the track in use and the 5 s sun-change fade that smooths it, also across the switch between the outdoor and indoor tracks | §11.6 (E34, E37), §11.7 |
//! | [`animate_sky_objects`], [`DrawnSkyObjects`], [`SkyObjectState`], [`object_angle`], [`object_visible`] | Sky objects: visibility, angles, keyframe entries, kept tints | §11.4 |
//! | [`EnvironmentPids`], [`SkyKeyLayers`], [`block_sky_key`], [`set_sky_key`], [`InteriorFog`] | Sky keys of landblocks, cells and monitors; interior fog | §7.1, §7.2, §12 |
//! | [`SkyLibrary`], [`Environment`], [`EnvironmentFrame`] | Records read from the install; the per-frame driver | §12 |
//!
//! [`SkyLibrary::evaluate`] is a pure function of the sky key, the day type, the clock
//! value and the records: the blended values at a given game time are bit-identical from
//! run to run. The keyframes' four float curves are not blended (their use is OPEN, §5.3).
//!
//! # Hostile input
//!
//! Records are decoded by `tessera-formats`. Here: a game-time record that cannot drive a
//! clock (non-positive day length, zero days per year) is rejected; weighted lists whose
//! weights do not sum to a positive finite number pick nothing; keyframe searches are
//! linear and allocation-bounded by the decoded counts; the day-description cache is
//! bounded ([`DEFAULT_DAY_CACHE_ENTRIES`]); non-finite clock values and rates are ignored.
//! The decoder does not check keyframe and sky-object floats for finiteness (§13 has no
//! such rule; OPEN there), so: keyframes whose time is not finite are skipped, a blended
//! or cross-faded value that is not finite is replaced by its default of §11.3 step 8
//! ([`SkyValues::defaults`]), and a sky object with non-finite times is hidden and its
//! non-finite angles count as 0. No NaN or infinity reaches [`LightingInputs`] or
//! [`SkyObjectState`] from the records.

mod blend;
mod choose;
mod clock;
mod fade;
mod hooks;
mod sky_objects;
mod state;

pub use blend::{
    CROSSFADE_EPSILON, DEFAULT_TRANSITION_SECONDS, KeyframePair, LightingInputs, Rgb, SUN_EPSILON,
    SkyValues, blend_direction, blend_factor, crossfade_factor, heading_pitch, keyframe_pair, lerp,
    lerp_angle, lerp_rgb, lerp_rgba, outdoor_sun_present, rgb, toward_sun, transition_seconds,
};
pub use choose::{
    ChoiceSource, DAY_HASH_MUL, DAY_HASH_SUB1, DAY_HASH_SUB2, DayChoice, INITIAL_DAY_TYPE,
    choose_day, choose_day_with_fallback, day_hash, weighted_pick,
};
pub use clock::{Calendar, ClockReading, GameClock, MAX_GAME_SECONDS, MAX_RATE};
pub use fade::{
    EnvState, MIN_SUN_CHANGE_SECONDS, NO_FOG_FAR, NO_FOG_NEAR, SUN_CHANGE_SECONDS, SunChangeFade,
};
pub use hooks::{
    EnvironmentPids, InteriorFog, InteriorFogType, SkyKeyLayers, block_sky_key, property_colour,
    set_indoor_ambiance, set_sky_key, sky_key_of,
};
pub use sky_objects::{
    DrawnSkyObject, DrawnSkyObjects, SkyObjectState, UNTINTED, animate_sky_objects, object_angle,
    object_progress, object_visible,
};
pub use state::{DEFAULT_DAY_CACHE_ENTRIES, Environment, EnvironmentFrame, Evaluation, SkyLibrary};

use crate::error::WorldError;

/// What can go wrong while loading the environment.
#[derive(Debug, thiserror::Error)]
pub enum EnvironmentError {
    /// No region descriptor names a sky description.
    #[error("no region names a sky description")]
    NoSky,
    /// A record's values cannot be used.
    #[error("unusable {0}")]
    Invalid(&'static str),
    /// Reading or decoding a record failed.
    #[error(transparent)]
    World(#[from] WorldError),
}

#[cfg(test)]
mod tests;
