// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-server`: a project-owned, headless test server for Tessera clients.
//!
//! It speaks the Tessera protocol (`docs/specs/tessera-protocol.md`), keeps accounts local
//! to the server, hosts world instances with interest management by landblock, relays chat
//! and serves only original test content (`docs/specs/server-content.md`). It never talks
//! to, imitates or connects to any official service, holds no game data, and never reads a
//! game installation (`docs/specs/test-server.md`).
//!
//! # Layout
//!
//! - [`server`]: [`Server::start`] and the [`ServerHandle`] (addresses, fingerprint,
//!   counters, shutdown), for tests, `apps/loadtest` and the binary.
//! - [`config`], [`content`]: the configuration file and content files with their checks;
//!   [`words`] and [`names`]: the word check and name rules.
//! - [`datadir`], [`accounts`], [`characters`]: the data directory, its locks and atomic
//!   writes, the server key, Argon2id accounts, character and world-state files.
//! - [`conn`]: per-connection inbound handling without sockets (fuzzed); `net`: the
//!   network thread; [`sim`]: the simulation; [`workers`]: hash workers and the writer;
//!   [`shared`]: what the threads share.
//! - [`cli`]: the command line.
//!
//! # Example
//!
//! ```no_run
//! use tessera_server::{Config, Content, DataDir, Server, ServerOptions};
//!
//! let mut config = Config::default();
//! config.server.listen = vec!["127.0.0.1:0".parse().unwrap()];
//! let handle = Server::start(ServerOptions {
//!     data_dir: DataDir::new("/tmp/tessera-server-example"),
//!     config,
//!     content: Content::default_world(),
//! })
//! .unwrap();
//! println!("{}", handle.connect_string());
//! handle.shutdown(std::time::Duration::ZERO);
//! ```

pub mod accounts;
pub mod appearance;
pub mod characters;
pub mod cli;
pub mod config;
pub mod conn;
pub mod content;
pub mod datadir;
pub mod error;
pub mod names;
mod net;
pub mod server;
pub mod shared;
pub mod sim;
pub mod tomlx;
pub mod words;
pub mod workers;

pub use config::Config;
pub use content::Content;
pub use datadir::DataDir;
pub use error::ServerError;
pub use server::{Server, ServerHandle, ServerOptions, ShutdownReport};
pub use shared::StatsSnapshot;
