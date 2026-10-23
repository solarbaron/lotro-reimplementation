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
//! # Clean-room rules (decisions 0034 and 0042)
//!
//! Protocol facts in `tessera-compat` come from clean-room specifications
//! under `docs/specs/` written by someone other than the implementer
//! (`docs/CLEAN_ROOM.md`). Since incident 0041 the M8 specifications are not
//! accepted; the existing code is kept and will be checked against them once
//! they pass (decision 0042 item 6), and only cleanup, hardening and neutral
//! renames land until then. No official server hostnames, addresses, ports,
//! service URLs or path fragments, keys, certificates or account data appear
//! in code, tests, fixtures or documentation: anything of that kind is read at
//! runtime from the user's own install, and when the install does not give a
//! value the client reports it as missing (decision 0042 item 3).
//!
//! Offline development uses the [`sandbox`](crate::sandbox) backend and the
//! [`scenario`](crate::scenario) feed, and multiplayer testing uses Tessera's own
//! test-server protocol ([`remote`](crate::remote), milestone M7).
