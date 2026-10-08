// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-audio`: the client's audio (`docs/ARCHITECTURE.md` §14, ROADMAP M6).
//!
//! | Module | Purpose | Spec |
//! |---|---|---|
//! | [`pcm`] | Shared decoded-audio types: whole buffers and streaming sources | — |
//! | [`decode`] | Sound payloads (0x0A): Ogg Vorbis played range, RIFF WAVE, classification | `sound.md` §3, §12.2 |
//! | [`engine`] | Output backend, mixer thread, channels and volumes, voices, gain chain, distance law, pitch, looping, ramp-out, voice limit, listener and panning, request gates, reverb environments | `sound.md` §6–§8, §12.3 |
//! | [`abc`] | Player music: the original client's ABC dialect, limits, timing and note events | `abc-dialect.md` |
//! | [`instruments`] | Player music: instrument table, note → sample, playback queue, sustain and release, instance limit, synchronised start | `instruments.md` |
//!
//! The output backend is cpal behind the `device` feature (decision 0019); without it the
//! crate builds with no system audio libraries and offers the null and offline backends of
//! [`engine`].

pub mod abc;
pub mod decode;
pub mod engine;
pub mod instruments;
pub mod pcm;

pub use engine::{AudioEngine, EngineConfig, EngineError, SoundRequest};
pub use pcm::{Pcm, PcmCursor, PcmSource};
