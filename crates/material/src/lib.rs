// SPDX-License-Identifier: GPL-3.0-or-later
//! Materials and images built from render surfaces.
//!
//! | Module | Purpose | Spec |
//! |---|---|---|
//! | [`icon`] | Layered icon composition: layer properties, order, straight-alpha source-over, canvas and centring | `docs/specs/icon-layers.md` |
//! | [`texture`] | Mip-chain helpers on top of `tessera-formats` texture lists: effective chain with a resolution cap, level selection, chain classes, GPU upload plans, level invariants | `docs/specs/texture-list.md` §4, §5 |
//!
//! The crate works on record payloads supplied by the caller (for example from
//! `tessera-dat`'s archive set) and decodes them with `tessera-formats`; it does not open
//! archives itself. Inputs are treated as hostile: nothing panics on malformed data,
//! decoded layers are limited to [`icon::MAX_LAYER_DIMENSION`] pixels a side by default
//! (a caller may raise it up to [`icon::MAX_LAYER_DIMENSION_CEILING`]), checked before any
//! pixel is decoded, stacks to [`icon::MAX_STACK_LAYERS`] layers, and the decoded-layer
//! cache to a fixed entry count and byte budget.

#![warn(missing_docs)]

mod error;
pub mod icon;
pub mod texture;

pub use error::{MaterialError, Result, SourceFailure};
