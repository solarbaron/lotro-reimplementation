// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-loadtest`: load and soak tools for Tessera's own protocol and test server
//! (ROADMAP M7, `docs/specs/test-server.md` §13).
//!
//! Many headless **bots** run through the client's real network path, a
//! [`tessera_net::Session`] over the test-server backend
//! ([`tessera_net::remote::RemoteBackend`]), against an in-process `tessera-server` on
//! loopback (accounts and characters created for the run) or a remote one. They walk and run
//! scripted paths across landblock borders, stop, turn, teleport, change appearance, chat and
//! reconnect. The run measures:
//!
//! - **desync** ([`desync`]): each bot's true trajectory against the interpolated position
//!   other bots show at their render time, entities missing or left over against the
//!   interest-management rules (far teleports and walks take pairs out of range), appearance
//!   and stance mismatches, heading and gait of moving bots, cheating moves an observer
//!   shows, corrections of bots within the limits, and chat delivery ([`chat`]);
//! - latency (login, world entry, ping RTT, chat) and traffic;
//! - **leaks** ([`leak`]): resident memory, the server's object and queue counters and the
//!   bots' session counts over time, with bounds, a return to the baseline and a fitted
//!   growth slope.
//!
//! The **hostile mode** ([`hostile`]) adds clients that send random bytes, malformed,
//! truncated and oversized frames, out-of-order handshakes and floods over real UDP while
//! the honest bots keep running.
//!
//! [`runner::run`] does a whole run and returns a [`report::Report`] with a text summary,
//! CSV samples, JSON and a verdict. Timing numbers are marked **indicative** unless the
//! operator declares the reference machine under quiet conditions (decision 0010). The tool
//! measures its own main loop: a run whose desync sampling fell below 9 Hz is invalid.
//!
//! ```no_run
//! use std::time::Duration;
//! use tessera_loadtest::runner::{RunOptions, Scenario, run};
//!
//! let mut opts = RunOptions::new(Scenario::Walk, 4);
//! opts.duration = Duration::from_secs(20);
//! let report = run(&opts)?;
//! print!("{}", report.text());
//! assert!(report.passed());
//! # Ok::<(), tessera_loadtest::LoadtestError>(())
//! ```

pub mod bot;
pub mod chat;
pub mod desync;
mod error;
pub mod hostile;
pub mod leak;
pub mod metrics;
pub mod raw;
pub mod report;
pub mod rng;
pub mod runner;
pub mod sysinfo;
pub mod truth;

pub use error::LoadtestError;
pub use report::Report;
pub use runner::{RunOptions, Scenario, TargetOptions, run};
