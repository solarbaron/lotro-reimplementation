// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-scriptgraph`: runs script-table node graphs continuously, the way the original
//! plays music, ambience, reverb environments and positional emitters (`docs/specs/sound.md`
//! §9–§11, `docs/specs/script-table.md` §7, and the normative
//! `docs/specs/script-runtime.md`).
//!
//! The static clip resolver of `tessera_formats::script_table::resolve` evaluates a graph
//! once for a fixed property state; this crate keeps node instances alive over time and
//! hands effects to an [`EffectSink`] that the client wires to `tessera-audio`. It needs no
//! area-specific code: music, ambient beds and reverb follow from the generic evaluator
//! and the player properties the client sets (`sound.md` §10.1–§10.2).
//!
//! # Model
//!
//! - A [`Runtime`] holds entities ([`EntityId`]s chosen by the caller), each with its
//!   tables, a layered property store and its runs. [`Runtime::spawn_entity`] runs
//!   `Startup` and the start-up scripts (`script-table.md` §8.1–§8.2); a script key is
//!   looked up through the whole table closure, every defining table contributing its
//!   roots (`script-runtime.md` §12.2).
//! - Nodes start by the generic rule of `script-runtime.md` §3.1 (persistent-node gate,
//!   `Probability` draw, the same-node rule, driven values, intensity, kind start,
//!   registration) and complete or stop synchronously (§3.4–§3.5). Every control-flow kind
//!   of §6–§17 is implemented: Switch (with Delay, RampOut cross-fades, transition and
//!   ForCurrentValue ports), When, Random (with the shared NoRepeats memory), Collection,
//!   Label, Sequence (with pass pacing), Timer, Run (with Stop), WhenRunning, SetProperty,
//!   Goto, OnStop, Scriptlet and ScriptletParameter. Sound and SoundEnvironment nodes follow
//!   `sound.md` §9.2 and §9.4; Animation asks the sink how long it runs; Particle nodes
//!   follow `particle-runtime.md` §5.4–§5.8 (start gates, the never-ending instance with its
//!   stop hook and redirect listener) and leave the effect to the sink; While runs its
//!   branch while its property test holds and PhysicsImpulse sends its impulse to the sink
//!   (§30); other effect kinds complete at once (§18).
//! - [`Runtime::set_property`] re-evaluates Switch and When nodes and updates driven
//!   fields and intensities synchronously (§20.3); [`Runtime::advance`] moves the one
//!   scheduler (Timers, Sequence pacing, Switch delays and cross-fades, deferred Runs,
//!   §20); [`Runtime::sound_completed`] completes tracked Sound nodes (`sound.md` §9.2 step
//!   12); [`Runtime::despawn`] stops every run with the teardown reason (§19).
//! - Property names and enum labels come from the data ([`RuntimeVocabulary::from_data`],
//!   [`PropertyCatalog`]); no PID or key is hard-coded.
//! - Hostile graphs are bounded by [`RuntimeLimits`] (instances per entity and overall,
//!   instance depth across Run chains, synchronous nesting, starts per call and per
//!   advance, events per advance, scriptlet nesting, lookup size, compiled nodes, Timer
//!   cycles that make no progress or crowd one advance); problems are logged
//!   with `tracing` and listed as [`RuntimeIssue`]s. Nothing panics on malformed data.
//! - Random draws use one seeded [`rand::rngs::SmallRng`] per runtime ([`Runtime::with_seed`]),
//!   like the original's single global generator (`script-runtime.md` §8, R40); a Random
//!   node picks the whole part of *u* × the weight sum. An entity spawned with
//!   [`EntityInit::random_seed`] (a placed figure seeded from its instance ID, the §8
//!   OPEN-18 Decision) draws its Random picks from its own generator instead.

// Unit tests reuse the integration tests' graph builders, which name the crate.
#[cfg(test)]
extern crate self as tessera_scriptgraph;

mod error;
mod graph;
mod runtime;
mod sched;
pub mod sink;
pub mod source;
mod values;
pub mod vocab;

pub use error::{RuntimeError, RuntimeIssue};
pub use runtime::{
    EntityInit, INTENSITY_EPSILON, LAG_CATCH_UP, MAX_ADVANCE, MIN_LOOP_INTERVAL, Runtime,
    RuntimeLimits, RuntimeStats, ScriptStart,
};
pub use sink::{
    AnimationHandle, AnimationOutcome, EffectSink, EntityId, EnvironmentHandle, EnvironmentRequest,
    ImpulseRequest, JUMP_AIR_TIME_OFFSET, NullSink, ParticleHandle, ParticleStart, RecordingSink,
    SinkEvent, SoundHandle, SoundPositioner, SoundStart, SoundStartOutcome,
};
pub use source::{FetchSource, MemorySource, ScriptSource};
pub use vocab::{PropertyCatalog, RuntimeVocabulary, SyntheticCatalog};
