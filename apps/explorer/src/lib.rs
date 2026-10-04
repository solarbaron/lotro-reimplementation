// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-explorer`: a GUI browser for the archives of an install, read-only.
//!
//! The explorer is the M1 workbench (`docs/ROADMAP.md`): an archive tree (archives →
//! DID type groups → DIDs, virtualized for millions of entries), DID search (hex,
//! prefix, names from DID maps and the property registry), and views for any record:
//! entry information, hex, textures (zoom, channels, alpha checkerboard, mip levels),
//! layered icons, string tables with variable placeholders and markup, property sets
//! with names, enum labels and clickable DID links, and decoded record tables (DID
//! maps, enum mappers, the property dictionary). Back/forward history, bookmarks,
//! recent DIDs and the game directory persist in Tessera's settings file.
//!
//! # Threads
//!
//! The UI thread never reads or decodes a record. Opening the install, indexing the
//! archives and searching run on background threads ([`data::spawn_open`],
//! [`index::Indexer`], [`search::Search`]); every decode runs on `tessera-assets`
//! workers through the decoders in [`decoders`]. A frame-time overlay shows the cost of
//! each frame, and frames over 50 ms are logged ([`frame_stats`]).
//!
//! # Headless checks
//!
//! `--screenshot out.png --select <DID> [--view texture|strings|props|…]` renders until
//! the selection is ready, saves the window as PNG and exits ([`app`]).
//!
//! # Modules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`app`] | The eframe application: panels, views, navigation |
//! | [`cli`] | Command-line options and the [`cli::ViewKind`] list |
//! | [`data`] | The opened install shared by the workers ([`data::DataContext`]) |
//! | [`decoders`] | Worker decoders and their display-ready products |
//! | [`index`] | Archive directory indexes grouped by DID type |
//! | [`search`], [`query`] | DID search |
//! | [`image_ops`] | Channel masking, size limiting, checkerboard |
//! | [`richtext`] | Rendered message text → egui layout |
//! | [`hex`] | Hex-dump and size formatting |
//! | [`history`] | Back/forward history |
//! | [`frame_stats`] | Frame-time statistics |
//! | [`persist`] | Settings persistence (`tessera-settings`) |
//! | [`screenshot`] | PNG output for `--screenshot` |
//! | [`fonts`] | UI fonts from the system (none are embedded) |

#![warn(missing_docs)]

pub mod app;
pub mod cli;
pub mod data;
pub mod decoders;
mod error;
pub mod fonts;
pub mod frame_stats;
pub mod hex;
pub mod history;
pub mod image_ops;
pub mod index;
pub mod persist;
pub mod query;
pub mod richtext;
pub mod screenshot;
pub mod search;
mod views;

pub use app::{AppConfig, ExplorerApp, ScreenshotPlan, run};
pub use error::ExplorerError;
