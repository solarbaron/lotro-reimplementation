// SPDX-License-Identifier: GPL-3.0-or-later
//! Placeholder for a future server-compatibility protocol adapter.
//!
//! **This module intentionally contains no implementation.**
//!
//! # Where an adapter would plug in
//!
//! A compatibility adapter would be one more [`Backend`](crate::Backend):
//! it would translate a separately specified wire protocol into the neutral
//! [`Event`](crate::Event) / [`Command`](crate::Command) model, so the
//! client, renderer and UI never see protocol details. It could reuse
//! [`transport`](crate::transport) only if the specified protocol happened to
//! fit it; more likely it would bring its own framing behind the
//! [`Datagram`](crate::transport::Datagram) trait.
//!
//! # Preconditions (gating)
//!
//! Work on this module is **blocked on legal review**. `docs/LEGAL.md` §8
//! rates connectivity with official servers as risk tier T4 (the highest),
//! and §9.3 defines the gate that must pass first (see also
//! `docs/ARCHITECTURE.md` §19.3 and milestone M8 in `docs/ROADMAP.md`).
//! In particular, before any code lands here:
//!
//! - the protocol must be described in a clean-room specification under
//!   `docs/specs/`, written by someone other than the implementer, with a
//!   provenance table, following the project's clean-room rules;
//! - no server hostnames, addresses, ports, keys or tables taken from any
//!   vendor binary may be committed. Anything of that kind must be supplied
//!   at runtime by the user (e.g. read from their own install or typed in);
//! - interoperability scope (which servers, under what terms of service)
//!   must be explicitly approved in `docs/LEGAL.md`.
//!
//! Until then, development uses the [`sandbox`](crate::sandbox) backend, and
//! later Tessera's own test-server protocol (milestone M7), which is design
//! work and needs no gate.
