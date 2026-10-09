// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's own persistent settings file.
//!
//! This is Tessera's file format, not one of the game's, so it has no
//! clean-room spec: this rustdoc is its definition. The file is TOML, kept at
//! `<platform config dir>/tessera/settings.toml` (on Linux
//! `$XDG_CONFIG_HOME/tessera/settings.toml`, usually
//! `~/.config/tessera/settings.toml`), or wherever
//! [`SETTINGS_FILE_ENV`] points.
//!
//! ```toml
//! schema_version = 1
//!
//! [game]
//! active_dir = "/games/install"
//! known_dirs = ["/games/install", "/old/install"]
//!
//! [explorer]
//! last_archive = "client_surface.dat"
//! last_did = "0x41000123"
//! recent_dids = ["0x41000123", "0x0A000001"]
//!
//! [[explorer.bookmarks]]
//! did = "0x0A000001"
//! label = "start"
//!
//! [windows.explorer]
//! x = 10
//! y = 20
//! width = 1280
//! height = 800
//! maximized = false
//! ```
//!
//! * [`Settings`] is the in-memory model with helpers that keep lists
//!   deduplicated and bounded ([`Settings::remember_game_dir`],
//!   [`Settings::push_recent_did`], [`Settings::add_bookmark`],
//!   [`Settings::set_window`], ...).
//! * [`SettingsStore::load`] is tolerant: it never fails, never moves or
//!   modifies the file, resets only what is broken, copies damaged files to
//!   a backup next to the original and reports [`LoadWarning`]s.
//!   [`SettingsStore::try_load`] is the strict variant.
//! * [`SettingsStore::save`] writes atomically (temporary file, flush,
//!   rename) and [`SettingsStore::update`] does a locked read-modify-write.
//!   No write of this crate ever lands inside a game install (recorded game
//!   directories, `TESSERA_GAME_DIR`, or any directory holding
//!   `client_general.dat`): the game install is read-only.
//! * [`UiPrefs`] reads and writes the game-UI preferences of the `[UI]` section
//!   (skin, walker overlays, cursor, scale, tooltips, U5 budgets, chat window).
//! * [`InputPrefs`] reads and writes the `[Input]` section (double-click time and
//!   rectangle, `keymap.md` §7.7).
//! * [`SCHEMA_VERSION`] and [`migrate()`] form the migration hook for future
//!   incompatible changes. Unknown keys are preserved with their exact TOML
//!   type at every level (the root, `[game]`, `[explorer]`, each bookmark
//!   and each `[windows.<key>]` table), so files written by a newer Tessera
//!   survive a round trip through an older one.
//!
//! * [`AudioSettings`] reads and writes the additive `[audio]` section (kept in
//!   [`Settings::extra`]) and imports the original client's `[Sound]` volumes read-only.
//!
//! All reads are bounded by [`MAX_FILE_SIZE`], the structural depth of a
//! document by [`MAX_NESTING_DEPTH`], every list by its `MAX_*`
//! constant, and the reported warnings by [`MAX_WARNINGS_PER_SECTION`] per
//! section. The bounds are chosen so that any settings the helpers accept
//! serialize well below [`MAX_FILE_SIZE`] (unknown keys carried over from a
//! file are not bounded beyond that file's own size).

/// Re-export of the `toml` crate, for working with [`Settings::extra`] tables.
pub use toml;

mod audio;
mod error;
mod input;
mod migrate;
mod model;
mod nesting;
mod store;
mod ui;

pub use audio::{
    AudioSettings, MAX_CHANNEL_LABEL_LEN, MAX_CHANNEL_VOLUMES, MAX_PLAYING_SOUNDS_LIMIT,
    SECTION_AUDIO, SoundImport, audio_keys,
};
pub use error::{LoadWarning, Result, SettingsError};
pub use input::{
    DEFAULT_DOUBLE_CLICK_MS, DEFAULT_DOUBLE_CLICK_PX, DOUBLE_CLICK_MS_RANGE, DOUBLE_CLICK_PX_RANGE,
    INPUT_SECTION, InputPrefs,
};
pub use migrate::{
    MIGRATIONS, Migration, MigrationFn, SCHEMA_VERSION, SCHEMA_VERSION_KEY, migrate, migrate_with,
};
pub use model::{
    Bookmark, ExplorerSettings, GameSettings, MAX_BOOKMARKS, MAX_GAME_DIRS, MAX_LABEL_BYTES,
    MAX_PATH_BYTES, MAX_RECENT_DIDS, MAX_WARNINGS_PER_SECTION, MAX_WINDOW_DIM, MAX_WINDOW_KEY_LEN,
    MAX_WINDOW_POS, MAX_WINDOWS, MIN_WINDOW_DIM, Parsed, Settings, WindowGeometry, format_did,
    parse_did,
};
pub use nesting::MAX_NESTING_DEPTH;
pub use store::{
    APP_DIR_NAME, FILE_NAME, LoadStatus, Loaded, MAX_BACKUPS, MAX_FILE_SIZE, SETTINGS_FILE_ENV,
    SettingsStore, check_not_inside_game_dirs, default_path,
};
pub use ui::{
    BUDGET_MIB_RANGE, ChatPrefs, CursorMode, MAX_UI_TEXT_BYTES, MinimapMode, SCALE_RANGE,
    ScaleFilter, UI_SECTION, USER_SKIN_KEY, UiPrefs,
};
