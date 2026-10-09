// SPDX-License-Identifier: GPL-3.0-or-later
//! Light selection per object and Light entities (`lighting.md` §8.3–§8.5).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`LightPids`], [`LightParams`], [`DayNightVariant`], [`LightEntity`], [`light_entities`], [`waveform_curve`] | Light static entities of landblock info and their fields, with the defaults of a new light and the day/night variants | §8.3, `entity-descriptor.md` §9 |
//! | [`LightRecord`], [`LightKind`], [`attenuation`] | The per-frame light record and the attenuation rule | §8.3 |
//! | [`glow`] ([`glow::GlowPids`], [`glow::GlowThresholds`], [`glow::running_variants`], [`glow::glow_light`]) | Day and night lights of placed lamps and windows | `instance-scripts.md` §6, §9.4 |
//! | [`select_lights`], [`LightSelection`], [`LightRef`], [`SelectionOptions`], [`object_light_list`], [`LightReach`] | Which lights an object receives: main light, key lights, per-vertex lists, the limit of 8 | §8.5 |
//!
//! The sun, ambient and fog inputs of `lighting.md` §8.1 come from the environment
//! ([`crate::environment::SkyValues::lighting`]). Projector lights (§8.4) are OPEN and not
//! modelled. Intensity curves are evaluated by the caller on the global clock
//! (`tessera-material` owns the curve evaluator); [`LightParams::constant_intensity`] covers
//! constant curves.
//!
//! # Hostile input
//!
//! Property values of the wrong type or non-finite floats fall back to the defaults of a
//! new light; ranges are kept within [`MIN_RANGE`] – [`MAX_RANGE`] (so *R*² stays finite);
//! attenuation of non-finite inputs is 0;
//! selections never hold more than [`MAX_LIGHTS`] lights.

mod entities;
pub mod glow;
mod record;
mod select;

pub use entities::{
    DEFAULT_FALLOFF, DEFAULT_INTENSITY, DEFAULT_RANGE, DayNightVariant, LightEntity, LightParams,
    LightPids, light_entities, waveform_curve,
};
pub use record::{
    FADE_START, FALLOFF_SCALE, LightKind, LightRecord, MAX_RANGE, MIN_RANGE, WHITE_TOLERANCE,
    attenuation,
};
pub use select::{
    LightReach, LightRef, LightSelection, MAX_LIGHTS, SelectionOptions, object_light_list,
    select_lights,
};

#[cfg(test)]
mod tests;
