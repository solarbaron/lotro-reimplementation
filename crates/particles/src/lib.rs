// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-particles` — the particle runtime.
//!
//! Specs: `docs/specs/particles.md` (records and the simulation of one 0x39 system, §8) and
//! `docs/specs/particle-runtime.md` (clock, random source, starting, attaching, stopping,
//! lingering and drawing effects).
//!
//! | Module | Purpose | Spec |
//! |---|---|---|
//! | [`rng`] | [`Ran2`], the shared random generator, and its range draws | runtime §3.3, §3.4 |
//! | [`curve`] | curves evaluated over time; mode 3 draws from [`Ran2`] | `material.md` §5.3.1, runtime §3.3 |
//! | [`sim`] | [`SystemSim`]: start, restart, update cadence, emission, caps, spawning, channels, controllers, forces | particles §8.1–§8.9, §8.16, §8.17; runtime §9.2 |
//! | [`draw`] | distance fade, the not-drawn state, billboards, atlas frames, vertex order, system draw order | particles §8.10–§8.15; runtime §9 |
//! | [`wind`] | the eased global wind | particles §8.8 |
//! | [`fork`] | fork effects: lifetime, fade, effect matrix, shader table (not simulated, rule §10.2) | particles §6.5; runtime §10 |
//! | [`runtime`] | classification, node start gates, start gating, attachment frames, owner scale vectors, the effect manager with sharing, release, lingering and housekeeping | runtime §4–§7 |
//!
//! All input is hostile: a decoded record may hold any value its layout allows. The
//! simulation never panics, keeps every position finite (a particle whose state stops being
//! finite dies), and bounds its work: per emitter at most 100,000 live particles
//! (`particles.md` §10.8), per system [`SimConfig::max_live_particles`], per draw
//! [`draw::MAX_VERTICES`] vertices, bounded controller loops.
//!
//! ```
//! use std::sync::Arc;
//! use tessera_formats::gameplay::particles::{Emitter, Keyframe, ParticleSystem};
//! use tessera_particles::{CameraView, Environment, OriginAttachments, Placement, Ran2, SystemSim};
//! use tessera_particles::glam::Vec3;
//!
//! # let desc = ParticleSystem::decode(&[
//! #     0xcd, 0xcc, 0x4c, 0x3d, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0xf0, 0x41, 0, 0,
//! #     0x70, 0x42, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
//! #     0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 1, 0x34, 0x12, 0, 0x31, 0, 0, 0, 0,
//! # ]).unwrap();
//! let mut sim = SystemSim::new(Arc::new(desc), 0.0);
//! let mut rng = Ran2::new(42);
//! let camera = CameraView::look_at(Vec3::new(0.0, -10.0, 1.0), Vec3::ZERO);
//! for frame in 0..60u64 {
//!     let now = frame as f64 / 30.0;
//!     sim.place(frame, now, Placement::at(Vec3::ZERO), &Environment::default(), &mut rng, &OriginAttachments);
//!     let _draw = sim.draw(now, &camera, &mut rng);
//! }
//! ```

// Comparisons are written so that a NaN from a hostile record fails the positive test the
// spec states (`!(a > b)` is true for NaN); `partial_cmp` chains would hide that.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

pub mod curve;
pub mod draw;
mod error;
pub mod fork;
pub mod rng;
pub mod runtime;
pub mod sim;
pub mod wind;

pub use draw::{
    CameraView, InstanceDraw, MAX_VERTICES, SystemDraw, Vertex, distance_fade, draw_order,
};
pub use error::{ParticleError, Result};
pub use fork::{ForkEffect, ForkFade, fork_shader_key};
pub use glam;
pub use rng::Ran2;
pub use runtime::{
    Classification, Effect, EffectHandle, EffectManager, ParticleNode, StartPath, SystemKey,
    classify_fork, classify_system, start_path,
};
pub use sim::{
    AttachmentSource, Environment, NoAttachments, OriginAttachments, Particle, Placement,
    SimConfig, SimStats, SystemSim,
};
pub use wind::Wind;

use std::sync::Arc;

use tessera_formats::gameplay::particles::{ForkSystem, ParticleSystem};

/// Decodes a 0x39 or 0x44 payload and makes the effect for it, created at `now`. Fork
/// records are decoded with the version-3 reading and an empty property schema
/// (`particles.md` §6.1).
///
/// # Errors
///
/// [`ParticleError::NotAnEffect`] for other prefixes; [`ParticleError::Decode`] when the
/// payload does not decode.
pub fn effect_from_payload(did: u32, payload: &[u8], now: f64, fade: ForkFade) -> Result<Effect> {
    match runtime::EffectFamily::of(did) {
        Some(runtime::EffectFamily::Particles) => {
            let ps = ParticleSystem::decode(payload)?;
            Ok(Effect::Particles(SystemSim::new(Arc::new(ps), now)))
        }
        Some(runtime::EffectFamily::Fork) => {
            let fs = ForkSystem::decode(payload, &tessera_formats::NoSchema)?;
            Ok(Effect::Fork(ForkEffect::new(&fs, now, fade)))
        }
        None => Err(ParticleError::NotAnEffect(did)),
    }
}
