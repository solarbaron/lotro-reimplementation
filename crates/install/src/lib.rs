// SPDX-License-Identifier: GPL-3.0-or-later
//! Locate, validate and identify a game install whose data files this project
//! reads, **without ever writing to it**.
//!
//! Spec: `docs/specs/install-layout.md`.
//!
//! * [`discover`] / [`discover_with`]: search the explicit directory,
//!   `TESSERA_GAME_DIR`, Steam libraries, third-party launcher configs,
//!   standalone default paths, Wine/Proton/CrossOver prefixes and a bounded
//!   home scan (§5).
//! * [`validate()`]: a structured [`ValidationReport`] for one directory (§4).
//! * [`GameInstall`]: a usable install with its archives, languages and client
//!   directory.
//! * [`SchemaFingerprint`] (header-only) and [`ContentFingerprint`] (from
//!   housekeeping records the dat layer supplies) (§9.4).
//! * [`find_settings_dir`] and the tolerant [`Ini`] reader (§8).
//!
//! The crate reads directory listings, small metadata files and the first
//! [`HEADER_READ_LEN`] bytes of each archive. It opens every file read-only
//! and has no dependency on the dat container crate.

mod archive;
mod discover;
mod error;
mod fingerprint;
mod fsutil;
mod ini;
mod install;
pub mod launcherconfig;
pub mod onelauncher;
mod records;
mod settings;
pub mod steam;
mod validate;
pub mod vdf;

pub use archive::{
    ARCHIVE_MAGIC, ArchiveFile, ArchiveHeader, ArchiveKind, ArchiveName, ArchiveStatus,
    HEADER_READ_LEN, HeaderTruncated, MAX_AUX_INDEX, MIN_CORE_ARCHIVE_SIZE, Tier,
    parse_archive_name,
};
pub use discover::{
    Discovery, GAME_DIR_ENV, Rejected, SearchEnv, Source, discover, discover_with, normalize_prefix,
};
pub use error::{Error, Result};
pub use fingerprint::{
    ContentEntry, ContentFingerprint, InstallWideStamp, SHORT_LEN, SchemaEntry, SchemaFingerprint,
};
pub use fsutil::translate_windows_path;
pub use ini::{Ini, IniEntry, IniSection, MAX_INI_SIZE};
pub use install::{GameInstall, SteamInfo};
pub use records::{
    DID_FFFF0004, DID_FFFF0008, DID_INSTALL_STAMP, DID_ITERATION, DID_SET_MEMBERSHIP, InstallStamp,
    IterationRange, IterationRecord, MAX_TRAILING_ENTRIES, RecordError, SetMembership, Stamp,
    TrailingEntry, stamp_hex,
};
pub use settings::{
    DEFAULT_DOCUMENT_FOLDER, PREFERENCES_FILE, SettingsDir, SettingsQuery, SettingsSource,
    find_settings_dir, settings_candidates,
};
pub use validate::{
    GameType, MAX_SPLIT_SET_COUNT, ProjectVersion, SplitSet, SplitSetStatus, ValidationProblem,
    ValidationReport, ValidationWarning, detect_game_type, read_project_version, validate,
    verify_split_set,
};
