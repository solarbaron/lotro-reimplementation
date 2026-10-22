// SPDX-License-Identifier: GPL-3.0-or-later
//! Documentation stub for the official-server compatibility adapter.
//!
//! **This module intentionally contains no implementation.** The official-server
//! compatibility backend lives in its own crate, `tessera-compat`
//! (`crates/compat`, `docs/decisions/0034-official-server-compatibility.md`), so
//! that it stays separate from `tessera-net`'s neutral world session model.
//!
//! # Where the adapter plugs in
//!
//! `tessera-compat` implements [`Backend`](crate::Backend): it translates the
//! official wire protocol (`docs/specs/official-login.md`,
//! `docs/specs/official-session.md`, `docs/specs/official-messages-world.md`,
//! `docs/specs/official-messages-play.md`,
//! `docs/specs/official-messages-gameplay.md`) into the neutral
//! [`Event`](crate::Event) / [`Command`](crate::Command) model, so the client,
//! renderer and UI never see wire protocol details.
//!
//! # Clean-room rules (decision 0034)
//!
//! Every protocol fact in `tessera-compat` comes from an accepted clean-room
//! specification under `docs/specs/` written by someone other than the
//! implementer (`docs/CLEAN_ROOM.md`), and no official server hostnames,
//! addresses, ports, keys, certificates or account data appear in code, tests,
//! fixtures or documentation (anything of that kind is read at runtime from the
//! user's own install).
//!
//! Offline development uses the [`sandbox`](crate::sandbox) backend and the
//! [`scenario`](crate::scenario) feed, and multiplayer testing uses Tessera's own
//! test-server protocol ([`remote`](crate::remote), milestone M7).
