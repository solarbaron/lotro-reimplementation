// SPDX-License-Identifier: GPL-3.0-or-later
//! The settings data model and its TOML mapping.
//!
//! Every table is split by hand: known keys are removed and decoded, and the
//! remaining keys are kept verbatim as `extra` tables. Values never pass
//! through serde's buffered representation, so unknown keys keep their exact
//! TOML type (including datetimes) across a load/save round trip.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{LoadWarning, Result, SettingsError};
use crate::migrate::{self, SCHEMA_VERSION, SCHEMA_VERSION_KEY};
use crate::nesting::{self, MAX_NESTING_DEPTH};

/// Maximum number of remembered game directories.
pub const MAX_GAME_DIRS: usize = 16;
/// Maximum length of the recent-DID list.
pub const MAX_RECENT_DIDS: usize = 64;
/// Maximum number of bookmarks.
///
/// Together with [`MAX_LABEL_BYTES`] and [`MAX_PATH_BYTES`] this keeps the
/// serialized document of any settings the helpers accept well below
/// [`crate::MAX_FILE_SIZE`].
pub const MAX_BOOKMARKS: usize = 256;
/// Maximum bookmark label length, in UTF-8 bytes. Longer labels are cut on a
/// character boundary; control characters are removed.
pub const MAX_LABEL_BYTES: usize = 256;
/// Maximum length of any stored path (game directories, archives), in UTF-8
/// bytes. Longer paths, and paths containing control characters, are not
/// stored.
pub const MAX_PATH_BYTES: usize = 1024;
/// Maximum number of saved window geometries.
pub const MAX_WINDOWS: usize = 64;
/// Maximum length of a window key, in bytes. Keys must also be free of
/// control characters.
pub const MAX_WINDOW_KEY_LEN: usize = 64;
/// Smallest width/height kept for a saved window, in logical pixels.
pub const MIN_WINDOW_DIM: u32 = 64;
/// Largest width/height kept for a saved window, in logical pixels.
pub const MAX_WINDOW_DIM: u32 = 16_384;
/// Largest absolute window position kept, in logical pixels.
pub const MAX_WINDOW_POS: i32 = 100_000;
/// At most this many problems are reported per section; the rest are
/// summarized in one [`LoadWarning::Suppressed`].
pub const MAX_WARNINGS_PER_SECTION: usize = 16;

const SECTION_GAME: &str = "game";
const SECTION_EXPLORER: &str = "explorer";
const SECTION_WINDOWS: &str = "windows";

const KEY_ACTIVE_DIR: &str = "active_dir";
const KEY_KNOWN_DIRS: &str = "known_dirs";
const GAME_KEYS: [&str; 2] = [KEY_ACTIVE_DIR, KEY_KNOWN_DIRS];

const KEY_LAST_ARCHIVE: &str = "last_archive";
const KEY_LAST_DID: &str = "last_did";
const KEY_RECENT_DIDS: &str = "recent_dids";
const KEY_BOOKMARKS: &str = "bookmarks";
const EXPLORER_KEYS: [&str; 4] = [
    KEY_LAST_ARCHIVE,
    KEY_LAST_DID,
    KEY_RECENT_DIDS,
    KEY_BOOKMARKS,
];

const KEY_DID: &str = "did";
const KEY_LABEL: &str = "label";
const KEY_ARCHIVE: &str = "archive";
const BOOKMARK_KEYS: [&str; 3] = [KEY_DID, KEY_LABEL, KEY_ARCHIVE];

const KEY_X: &str = "x";
const KEY_Y: &str = "y";
const KEY_WIDTH: &str = "width";
const KEY_HEIGHT: &str = "height";
const KEY_MAXIMIZED: &str = "maximized";
const WINDOW_KEYS: [&str; 5] = [KEY_X, KEY_Y, KEY_WIDTH, KEY_HEIGHT, KEY_MAXIMIZED];

const ROOT_KEYS: [&str; 4] = [
    SCHEMA_VERSION_KEY,
    SECTION_GAME,
    SECTION_EXPLORER,
    SECTION_WINDOWS,
];

/// Header comment written at the top of every saved file.
const FILE_HEADER: &str = "# Tessera settings. Written by Tessera; safe to edit while it is not running.\n\
# DIDs may be written as \"0x0A000123\" strings or plain integers.\n\n";

/// The whole settings document.
///
/// All fields have defaults, so a partial or empty file is valid. Use the
/// helper methods to keep the lists deduplicated and bounded; [`Settings::sanitize`]
/// enforces the same invariants on arbitrary values and runs on every load
/// and save.
///
/// Unknown keys are preserved at every level that has an `extra` table: the
/// document root, `[game]`, `[explorer]`, each bookmark and each
/// `[windows.<key>]` table. An `extra` entry whose name collides with a known
/// key is discarded by [`Settings::sanitize`]; it can never override the
/// typed field.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    /// Game install directories.
    pub game: GameSettings,
    /// Asset explorer state.
    pub explorer: ExplorerSettings,
    /// Saved window geometry, keyed by window name (for example `"explorer"`
    /// or `"client"`).
    pub windows: BTreeMap<String, WindowGeometry>,
    /// Unknown top-level keys from the loaded file (for example sections a
    /// newer Tessera added). They are written back unchanged on save.
    pub extra: toml::Table,
}

/// Remembered game install directories. They are only ever read from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameSettings {
    /// The install currently in use.
    pub active_dir: Option<PathBuf>,
    /// Every install the user has opened, most recent first.
    pub known_dirs: Vec<PathBuf>,
    /// Unknown keys in this section, preserved on save.
    pub extra: toml::Table,
}

/// State of the asset explorer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExplorerSettings {
    /// The last opened archive: a file name relative to the active game
    /// directory (for example `client_surface.dat`) or an absolute path.
    pub last_archive: Option<PathBuf>,
    /// The last selected DID.
    pub last_did: Option<u32>,
    /// Recently viewed DIDs, most recent first.
    pub recent_dids: Vec<u32>,
    /// User bookmarks, in user order.
    pub bookmarks: Vec<Bookmark>,
    /// Unknown keys in this section, preserved on save.
    pub extra: toml::Table,
}

/// A bookmarked record.
#[derive(Debug, Clone, PartialEq)]
pub struct Bookmark {
    /// The bookmarked DID.
    pub did: u32,
    /// User label (may be empty).
    pub label: String,
    /// Archive the DID was found in, when the user pinned one (same
    /// convention as [`ExplorerSettings::last_archive`]).
    pub archive: Option<PathBuf>,
    /// Unknown keys of this bookmark, preserved on save.
    pub extra: toml::Table,
}

impl Bookmark {
    /// A bookmark with a label and no pinned archive.
    pub fn new(did: u32, label: impl Into<String>) -> Self {
        Self {
            did,
            label: label.into(),
            archive: None,
            extra: toml::Table::new(),
        }
    }
}

/// Saved geometry of one top-level window, in logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowGeometry {
    /// Left edge, when known.
    pub x: Option<i32>,
    /// Top edge, when known.
    pub y: Option<i32>,
    /// Inner width.
    pub width: u32,
    /// Inner height.
    pub height: u32,
    /// Whether the window was maximized.
    pub maximized: bool,
    /// Unknown keys of this window table, preserved on save.
    pub extra: toml::Table,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: 1280,
            height: 800,
            maximized: false,
            extra: toml::Table::new(),
        }
    }
}

impl WindowGeometry {
    /// Clamp size and position into the accepted ranges.
    pub fn clamped(self) -> Self {
        Self {
            x: self.x.map(|v| v.clamp(-MAX_WINDOW_POS, MAX_WINDOW_POS)),
            y: self.y.map(|v| v.clamp(-MAX_WINDOW_POS, MAX_WINDOW_POS)),
            width: self.width.clamp(MIN_WINDOW_DIM, MAX_WINDOW_DIM),
            height: self.height.clamp(MIN_WINDOW_DIM, MAX_WINDOW_DIM),
            maximized: self.maximized,
            extra: self.extra,
        }
    }
}

/// Result of lenient parsing: the settings plus everything that was wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    /// The recovered settings (sanitized).
    pub settings: Settings,
    /// Problems found. Empty for a clean document. At most
    /// [`MAX_WARNINGS_PER_SECTION`] detailed warnings per section.
    pub warnings: Vec<LoadWarning>,
    /// The schema version the document declared (or the one assumed).
    pub schema_version: u32,
    /// True when nothing in the document could be used (syntax error or
    /// failed migration) and `settings` is all defaults.
    pub defaulted: bool,
    /// True when a migration from an older schema ran.
    pub migrated: bool,
}

impl Settings {
    /// Make `dir` the active game directory and move it to the front of
    /// [`GameSettings::known_dirs`].
    pub fn remember_game_dir(&mut self, dir: impl Into<PathBuf>) {
        let dir = dir.into();
        if dir.as_os_str().is_empty() {
            return;
        }
        self.game.known_dirs.retain(|d| d != &dir);
        self.game.known_dirs.insert(0, dir.clone());
        self.game.known_dirs.truncate(MAX_GAME_DIRS);
        self.game.active_dir = Some(dir);
    }

    /// Forget a game directory. Clears the active directory if it matches.
    pub fn forget_game_dir(&mut self, dir: &Path) {
        self.game.known_dirs.retain(|d| d != dir);
        if self.game.active_dir.as_deref() == Some(dir) {
            self.game.active_dir = None;
        }
    }

    /// Every game directory the settings mention (active first, deduplicated).
    pub fn game_dirs(&self) -> Vec<&Path> {
        let mut out: Vec<&Path> = Vec::new();
        for d in self
            .game
            .active_dir
            .iter()
            .chain(self.game.known_dirs.iter())
        {
            if !out.contains(&d.as_path()) {
                out.push(d);
            }
        }
        out
    }

    /// Record the last opened archive and DID in the explorer. A DID is also
    /// pushed onto the recent list.
    pub fn set_last_opened(&mut self, archive: Option<PathBuf>, did: Option<u32>) {
        self.explorer.last_archive = archive.filter(|a| !a.as_os_str().is_empty());
        self.explorer.last_did = did;
        if let Some(did) = did {
            self.push_recent_did(did);
        }
    }

    /// Move `did` to the front of the recent list, keeping it bounded.
    pub fn push_recent_did(&mut self, did: u32) {
        let r = &mut self.explorer.recent_dids;
        r.retain(|&d| d != did);
        r.insert(0, did);
        r.truncate(MAX_RECENT_DIDS);
    }

    /// Add a bookmark, or update the label of the existing bookmark with the
    /// same DID and archive. The label is cleaned (control characters
    /// removed, cut to [`MAX_LABEL_BYTES`]) and an archive path that cannot
    /// be stored (see [`MAX_PATH_BYTES`]) is dropped. Returns `false` when
    /// the list is full.
    pub fn add_bookmark(&mut self, mut bookmark: Bookmark) -> bool {
        clean_label(&mut bookmark.label);
        bookmark.archive = bookmark.archive.filter(|p| path_ok(p));
        if let Some(b) = self
            .explorer
            .bookmarks
            .iter_mut()
            .find(|b| b.did == bookmark.did && b.archive == bookmark.archive)
        {
            b.label = bookmark.label;
            return true;
        }
        if self.explorer.bookmarks.len() >= MAX_BOOKMARKS {
            return false;
        }
        self.explorer.bookmarks.push(bookmark);
        true
    }

    /// Remove every bookmark for `did` in `archive` (`None` matches bookmarks
    /// without a pinned archive). Returns how many were removed.
    pub fn remove_bookmark(&mut self, did: u32, archive: Option<&Path>) -> usize {
        let before = self.explorer.bookmarks.len();
        self.explorer
            .bookmarks
            .retain(|b| !(b.did == did && b.archive.as_deref() == archive));
        before - self.explorer.bookmarks.len()
    }

    /// Saved geometry of window `key`.
    pub fn window(&self, key: &str) -> Option<&WindowGeometry> {
        self.windows.get(key)
    }

    /// Save the geometry of window `key` (clamped). Returns `false`, without
    /// storing anything, when the key is empty, too long or contains control
    /// characters, or the table is full.
    pub fn set_window(&mut self, key: &str, geometry: WindowGeometry) -> bool {
        if !valid_window_key(key)
            || (!self.windows.contains_key(key) && self.windows.len() >= MAX_WINDOWS)
        {
            return false;
        }
        self.windows.insert(key.to_owned(), geometry.clamped());
        true
    }

    /// Enforce the invariants: lists deduplicated and bounded, paths that
    /// cannot be stored dropped (empty, non-UTF-8 since TOML only holds
    /// UTF-8, longer than [`MAX_PATH_BYTES`], or containing control
    /// characters), labels cleaned, window keys valid and geometry clamped,
    /// the active game directory listed in `known_dirs`, and every `extra`
    /// entry that collides with a known key removed.
    pub fn sanitize(&mut self) {
        let g = &mut self.game;
        g.active_dir = g.active_dir.take().filter(|p| path_ok(p));
        let mut dirs: Vec<PathBuf> = Vec::new();
        for d in &g.known_dirs {
            if path_ok(d) && !dirs.contains(d) {
                dirs.push(d.clone());
                if dirs.len() == MAX_GAME_DIRS {
                    break;
                }
            }
        }
        if let Some(active) = &g.active_dir
            && !dirs.contains(active)
        {
            dirs.insert(0, active.clone());
            dirs.truncate(MAX_GAME_DIRS);
        }
        g.known_dirs = dirs;
        strip_keys(&mut g.extra, &GAME_KEYS);

        let e = &mut self.explorer;
        e.last_archive = e.last_archive.take().filter(|p| path_ok(p));
        let mut recent: Vec<u32> = Vec::new();
        for &d in &e.recent_dids {
            if !recent.contains(&d) {
                recent.push(d);
                if recent.len() == MAX_RECENT_DIDS {
                    break;
                }
            }
        }
        e.recent_dids = recent;
        let mut marks: Vec<Bookmark> = Vec::new();
        for mut b in e.bookmarks.drain(..) {
            if marks.len() == MAX_BOOKMARKS {
                break;
            }
            b.archive = b.archive.filter(|p| path_ok(p));
            clean_label(&mut b.label);
            strip_keys(&mut b.extra, &BOOKMARK_KEYS);
            if !marks
                .iter()
                .any(|m| m.did == b.did && m.archive == b.archive)
            {
                marks.push(b);
            }
        }
        e.bookmarks = marks;
        strip_keys(&mut e.extra, &EXPLORER_KEYS);

        let mut windows = BTreeMap::new();
        for (k, v) in std::mem::take(&mut self.windows) {
            if windows.len() == MAX_WINDOWS {
                break;
            }
            if valid_window_key(&k) {
                let mut v = v.clamped();
                strip_keys(&mut v.extra, &WINDOW_KEYS);
                windows.insert(k, v);
            }
        }
        self.windows = windows;
        strip_keys(&mut self.extra, &ROOT_KEYS);
    }

    /// Serialize to a TOML document (sanitized, with `schema_version` set to
    /// [`SCHEMA_VERSION`]).
    ///
    /// # Errors
    ///
    /// [`SettingsError::Serialize`] if a value cannot be represented in TOML,
    /// or if an `extra` value nests deeper than [`MAX_NESTING_DEPTH`] in the
    /// document (only possible for values built in memory: loading rejects
    /// such documents).
    pub fn to_toml_string(&self) -> Result<String> {
        // Checked before anything is cloned or serialized, which both recurse.
        self.check_extra_depth()?;
        let mut s = self.clone();
        s.sanitize();
        let mut root = s.extra.clone();
        root.insert(
            SCHEMA_VERSION_KEY.into(),
            toml::Value::Integer(i64::from(SCHEMA_VERSION)),
        );
        root.insert(SECTION_GAME.into(), toml::Value::Table(s.game.to_table()));
        root.insert(
            SECTION_EXPLORER.into(),
            toml::Value::Table(s.explorer.to_table()),
        );
        if !s.windows.is_empty() {
            let w: toml::Table = s
                .windows
                .iter()
                .map(|(k, v)| (k.clone(), toml::Value::Table(v.to_table())))
                .collect();
            root.insert(SECTION_WINDOWS.into(), toml::Value::Table(w));
        }
        let body =
            toml::to_string_pretty(&root).map_err(|e| SettingsError::Serialize(e.to_string()))?;
        Ok(format!("{FILE_HEADER}{body}"))
    }

    /// Fail when an `extra` table would nest deeper than
    /// [`MAX_NESTING_DEPTH`] once placed in the document, measured as the
    /// loader measures it (`crate::nesting::parse_bounded`). The limits account
    /// for the tables above each `extra`: `[game]` and `[explorer]` are level
    /// 2, `[windows.<key>]` level 3 and each bookmark level 4.
    fn check_extra_depth(&self) -> Result<()> {
        let max = MAX_NESTING_DEPTH;
        let deep = [
            (&self.extra, max),
            (&self.game.extra, max.saturating_sub(1)),
            (&self.explorer.extra, max.saturating_sub(1)),
        ]
        .into_iter()
        .chain(
            self.explorer
                .bookmarks
                .iter()
                .map(|b| (&b.extra, max.saturating_sub(3))),
        )
        .chain(
            self.windows
                .values()
                .map(|w| (&w.extra, max.saturating_sub(2))),
        )
        .any(|(t, limit)| nesting::table_exceeds(t, limit));
        if deep {
            return Err(SettingsError::Serialize(format!(
                "an unknown value nests deeper than {max} levels"
            )));
        }
        Ok(())
    }

    /// Strictly parse a settings document: any syntax error, malformed
    /// section or value (including a single bad list entry, bookmark field,
    /// window field or `last_did`), newer schema or failed migration is an
    /// error.
    ///
    /// Silent normalization still happens through [`Settings::sanitize`]
    /// (duplicates removed, lists cut to their `MAX_*` bound, unstorable
    /// paths dropped).
    ///
    /// # Errors
    ///
    /// See [`SettingsError`].
    pub fn from_toml_str(text: &str) -> Result<Self> {
        // Bound the depth before the parser builds (and later code clones,
        // serializes and drops) a tree, with the same measure as
        // `check_extra_depth`: see `crate::nesting`.
        let mut table =
            nesting::parse_bounded(text).map_err(|e| SettingsError::Parse(one_line(&e)))?;
        let version = take_schema_version(&mut table)?;
        migrate::migrate(&mut table, version)?;
        let (settings, diag) = from_table(table);
        if let Some(first) = diag.first_message() {
            return Err(SettingsError::Parse(first));
        }
        Ok(settings)
    }

    /// Leniently parse a settings document. Never fails: a section that is
    /// not a table is reset to its defaults, a malformed value is reset or
    /// a malformed list entry dropped, a document that is not TOML yields
    /// all defaults, and each problem is reported in [`Parsed::warnings`].
    pub fn parse_lenient(text: &str) -> Parsed {
        let mut warnings = Vec::new();
        // A document nested deeper than `MAX_NESTING_DEPTH` is rejected
        // like a syntax error, with the same measure as `check_extra_depth`
        // so that what loads cleanly re-loads after a save (see
        // `crate::nesting`).
        let parsed = nesting::parse_bounded(text);
        let mut table: toml::Table = match parsed {
            Ok(t) => t,
            Err(message) => {
                warnings.push(LoadWarning::Syntax {
                    message: one_line(&message),
                });
                return Parsed {
                    settings: Settings::default(),
                    warnings,
                    schema_version: SCHEMA_VERSION,
                    defaulted: true,
                    migrated: false,
                };
            }
        };
        let version = match take_schema_version(&mut table) {
            Ok(v) => v,
            Err(e) => {
                warnings.push(LoadWarning::BadSchemaVersion {
                    message: e.to_string(),
                    assumed: SCHEMA_VERSION,
                });
                SCHEMA_VERSION
            }
        };
        let mut migrated = false;
        if version > SCHEMA_VERSION {
            warnings.push(LoadWarning::NewerSchema {
                found: version,
                supported: SCHEMA_VERSION,
            });
        } else if version < SCHEMA_VERSION {
            match migrate::migrate(&mut table, version) {
                Ok(_) => migrated = true,
                Err(e) => {
                    let (from, message) = match e {
                        SettingsError::Migration { from, message } => (from, message),
                        other => (version, other.to_string()),
                    };
                    warnings.push(LoadWarning::MigrationFailed { from, message });
                    return Parsed {
                        settings: Settings::default(),
                        warnings,
                        schema_version: version,
                        defaulted: true,
                        migrated: false,
                    };
                }
            }
        }
        let (settings, diag) = from_table(table);
        warnings.extend(diag.finish());
        Parsed {
            settings,
            warnings,
            schema_version: version,
            defaulted: false,
            migrated,
        }
    }
}

/// Remove and validate `schema_version`. A missing key means the current
/// version (hand-written files may omit it).
fn take_schema_version(table: &mut toml::Table) -> Result<u32> {
    match table.remove(SCHEMA_VERSION_KEY) {
        None => Ok(SCHEMA_VERSION),
        Some(toml::Value::Integer(i)) => u32::try_from(i)
            .ok()
            .filter(|&v| v >= 1)
            .ok_or_else(|| SettingsError::BadSchemaVersion(format!("{i} is out of range"))),
        Some(other) => Err(SettingsError::BadSchemaVersion(format!(
            "expected an integer, found {}",
            other.type_str()
        ))),
    }
}

// ---------------------------------------------------------------------------
// Problem collection.

/// Collects decoding problems, at most [`MAX_WARNINGS_PER_SECTION`] detailed
/// ones per section.
#[derive(Debug, Default)]
struct Diag {
    warnings: Vec<LoadWarning>,
    /// Total problem count per section (few distinct sections exist).
    counts: Vec<(&'static str, usize)>,
}

impl Diag {
    fn bump(&mut self, section: &'static str) -> bool {
        let idx = match self.counts.iter().position(|(s, _)| *s == section) {
            Some(i) => i,
            None => {
                self.counts.push((section, 0));
                self.counts.len() - 1
            }
        };
        self.counts[idx].1 += 1;
        self.counts[idx].1 <= MAX_WARNINGS_PER_SECTION
    }

    /// The whole section was unusable and reset.
    fn section(&mut self, section: &'static str, message: impl FnOnce() -> String) {
        if self.bump(section) {
            self.warnings.push(LoadWarning::Section {
                section: section.to_owned(),
                message: one_line(&message()),
            });
        }
    }

    /// One value or list entry was unusable and reset or dropped.
    fn entry(&mut self, section: &'static str, message: impl FnOnce() -> String) {
        if self.bump(section) {
            self.warnings.push(LoadWarning::Entry {
                section: section.to_owned(),
                message: one_line(&message()),
            });
        }
    }

    fn first_message(&self) -> Option<String> {
        self.warnings.first().map(|w| match w {
            LoadWarning::Section { section, message } | LoadWarning::Entry { section, message } => {
                format!("[{section}]: {message}")
            }
            other => other.to_string(),
        })
    }

    fn finish(mut self) -> Vec<LoadWarning> {
        for (section, n) in self.counts {
            if n > MAX_WARNINGS_PER_SECTION {
                self.warnings.push(LoadWarning::Suppressed {
                    section: section.to_owned(),
                    count: n - MAX_WARNINGS_PER_SECTION,
                });
            }
        }
        self.warnings
    }
}

fn found(expected: &str, v: &toml::Value) -> String {
    format!("expected {expected}, found {}", v.type_str())
}

/// Shorten a user-supplied key for use in a message.
fn key_for_message(k: &str) -> String {
    let mut s: String = k.chars().filter(|c| !c.is_control()).take(64).collect();
    if s.len() < k.len() && k.chars().count() > 64 {
        s.push('…');
    }
    s
}

// ---------------------------------------------------------------------------
// Decoding.

/// Split the root table into known sections and extras. Returns the
/// sanitized settings and every problem found.
fn from_table(mut table: toml::Table) -> (Settings, Diag) {
    let mut d = Diag::default();
    let mut s = Settings::default();
    if let Some(v) = table.remove(SECTION_GAME) {
        match v {
            toml::Value::Table(t) => s.game = GameSettings::from_table(t, &mut d),
            other => d.section(SECTION_GAME, || found("a table", &other)),
        }
    }
    if let Some(v) = table.remove(SECTION_EXPLORER) {
        match v {
            toml::Value::Table(t) => s.explorer = ExplorerSettings::from_table(t, &mut d),
            other => d.section(SECTION_EXPLORER, || found("a table", &other)),
        }
    }
    if let Some(v) = table.remove(SECTION_WINDOWS) {
        match v {
            toml::Value::Table(t) => s.windows = windows_from_table(t, &mut d),
            other => d.section(SECTION_WINDOWS, || found("a table", &other)),
        }
    }
    s.extra = table;
    s.sanitize();
    (s, d)
}

fn take_path(
    t: &mut toml::Table,
    key: &str,
    section: &'static str,
    ctx: &str,
    d: &mut Diag,
) -> Option<PathBuf> {
    match t.remove(key)? {
        toml::Value::String(s) => Some(PathBuf::from(s)),
        other => {
            d.entry(section, || {
                format!("{ctx}{key}: {}", found("a string", &other))
            });
            None
        }
    }
}

fn take_did_opt(
    t: &mut toml::Table,
    key: &str,
    section: &'static str,
    ctx: &str,
    d: &mut Diag,
) -> Option<u32> {
    let v = t.remove(key)?;
    let did = did_from_value(&v);
    if did.is_none() {
        d.entry(section, || format!("{ctx}{key}: {}", bad_did(&v)));
    }
    did
}

fn take_array(
    t: &mut toml::Table,
    key: &str,
    section: &'static str,
    d: &mut Diag,
) -> Vec<toml::Value> {
    match t.remove(key) {
        None => Vec::new(),
        Some(toml::Value::Array(a)) => a,
        Some(other) => {
            d.entry(section, || format!("{key}: {}", found("an array", &other)));
            Vec::new()
        }
    }
}

impl GameSettings {
    fn from_table(mut t: toml::Table, d: &mut Diag) -> Self {
        let active_dir = take_path(&mut t, KEY_ACTIVE_DIR, SECTION_GAME, "", d);
        let mut known_dirs = Vec::new();
        for (i, v) in take_array(&mut t, KEY_KNOWN_DIRS, SECTION_GAME, d)
            .into_iter()
            .enumerate()
        {
            match v {
                toml::Value::String(s) => known_dirs.push(PathBuf::from(s)),
                other => d.entry(SECTION_GAME, || {
                    format!("{KEY_KNOWN_DIRS}[{i}]: {}", found("a string", &other))
                }),
            }
        }
        Self {
            active_dir,
            known_dirs,
            extra: t,
        }
    }

    fn to_table(&self) -> toml::Table {
        let mut t = self.extra.clone();
        strip_keys(&mut t, &GAME_KEYS);
        if let Some(p) = self.active_dir.as_deref().and_then(Path::to_str) {
            t.insert(KEY_ACTIVE_DIR.into(), p.into());
        }
        t.insert(KEY_KNOWN_DIRS.into(), path_array(&self.known_dirs));
        t
    }
}

impl ExplorerSettings {
    fn from_table(mut t: toml::Table, d: &mut Diag) -> Self {
        let last_archive = take_path(&mut t, KEY_LAST_ARCHIVE, SECTION_EXPLORER, "", d);
        let last_did = take_did_opt(&mut t, KEY_LAST_DID, SECTION_EXPLORER, "", d);
        let mut recent_dids = Vec::new();
        for (i, v) in take_array(&mut t, KEY_RECENT_DIDS, SECTION_EXPLORER, d)
            .into_iter()
            .enumerate()
        {
            match did_from_value(&v) {
                Some(did) => recent_dids.push(did),
                None => d.entry(SECTION_EXPLORER, || {
                    format!("{KEY_RECENT_DIDS}[{i}]: {}", bad_did(&v))
                }),
            }
        }
        let mut bookmarks = Vec::new();
        for (i, v) in take_array(&mut t, KEY_BOOKMARKS, SECTION_EXPLORER, d)
            .into_iter()
            .enumerate()
        {
            if let Some(b) = Bookmark::from_value(v, i, d) {
                bookmarks.push(b);
            }
        }
        Self {
            last_archive,
            last_did,
            recent_dids,
            bookmarks,
            extra: t,
        }
    }

    fn to_table(&self) -> toml::Table {
        let mut t = self.extra.clone();
        strip_keys(&mut t, &EXPLORER_KEYS);
        if let Some(p) = self.last_archive.as_deref().and_then(Path::to_str) {
            t.insert(KEY_LAST_ARCHIVE.into(), p.into());
        }
        if let Some(did) = self.last_did {
            t.insert(KEY_LAST_DID.into(), format_did(did).into());
        }
        t.insert(
            KEY_RECENT_DIDS.into(),
            toml::Value::Array(
                self.recent_dids
                    .iter()
                    .map(|&d| toml::Value::String(format_did(d)))
                    .collect(),
            ),
        );
        t.insert(
            KEY_BOOKMARKS.into(),
            toml::Value::Array(
                self.bookmarks
                    .iter()
                    .map(|b| toml::Value::Table(b.to_table()))
                    .collect(),
            ),
        );
        t
    }
}

impl Bookmark {
    /// Decode bookmark `i`. A bookmark without a valid DID is dropped; other
    /// bad fields are reset.
    fn from_value(v: toml::Value, i: usize, d: &mut Diag) -> Option<Self> {
        let mut t = match v {
            toml::Value::Table(t) => t,
            other => {
                d.entry(SECTION_EXPLORER, || {
                    format!("{KEY_BOOKMARKS}[{i}]: {}", found("a table", &other))
                });
                return None;
            }
        };
        let ctx = format!("{KEY_BOOKMARKS}[{i}].");
        let Some(raw_did) = t.remove(KEY_DID) else {
            d.entry(SECTION_EXPLORER, || format!("{ctx}{KEY_DID}: missing"));
            return None;
        };
        let Some(did) = did_from_value(&raw_did) else {
            d.entry(SECTION_EXPLORER, || {
                format!("{ctx}{KEY_DID}: {}", bad_did(&raw_did))
            });
            return None;
        };
        let label = match t.remove(KEY_LABEL) {
            None => String::new(),
            Some(toml::Value::String(s)) => s,
            Some(other) => {
                d.entry(SECTION_EXPLORER, || {
                    format!("{ctx}{KEY_LABEL}: {}", found("a string", &other))
                });
                String::new()
            }
        };
        let archive = take_path(&mut t, KEY_ARCHIVE, SECTION_EXPLORER, &ctx, d);
        Some(Self {
            did,
            label,
            archive,
            extra: t,
        })
    }

    fn to_table(&self) -> toml::Table {
        let mut t = self.extra.clone();
        strip_keys(&mut t, &BOOKMARK_KEYS);
        t.insert(KEY_DID.into(), format_did(self.did).into());
        t.insert(KEY_LABEL.into(), self.label.clone().into());
        if let Some(p) = self.archive.as_deref().and_then(Path::to_str) {
            t.insert(KEY_ARCHIVE.into(), p.into());
        }
        t
    }
}

/// Decode `[windows]`. Entries with an invalid key or that are not tables
/// are dropped; bad fields are reset. Stops after [`MAX_WINDOWS`] entries
/// are kept (later ones would be cut by sanitizing anyway).
fn windows_from_table(t: toml::Table, d: &mut Diag) -> BTreeMap<String, WindowGeometry> {
    let mut out = BTreeMap::new();
    for (k, v) in t {
        if out.len() == MAX_WINDOWS {
            break;
        }
        if !valid_window_key(&k) {
            d.entry(SECTION_WINDOWS, || {
                format!(
                    "window key {:?}: must be 1-{MAX_WINDOW_KEY_LEN} bytes without control characters",
                    key_for_message(&k)
                )
            });
            continue;
        }
        let toml::Value::Table(mut wt) = v else {
            d.entry(SECTION_WINDOWS, || {
                format!("{}: {}", key_for_message(&k), found("a table", &v))
            });
            continue;
        };
        let ctx = format!("{}.", key_for_message(&k));
        let mut g = WindowGeometry::default();
        let mut int = |key: &str, d: &mut Diag| -> Option<i64> {
            match wt.remove(key)? {
                toml::Value::Integer(i) => Some(i),
                other => {
                    d.entry(SECTION_WINDOWS, || {
                        format!("{ctx}{key}: {}", found("an integer", &other))
                    });
                    None
                }
            }
        };
        // Out-of-range integers saturate; `clamped` then bounds them.
        let sat_i32 = |i: i64| i32::try_from(i).unwrap_or(if i < 0 { i32::MIN } else { i32::MAX });
        let sat_u32 = |i: i64| u32::try_from(i).unwrap_or(if i < 0 { 0 } else { u32::MAX });
        g.x = int(KEY_X, d).map(sat_i32);
        g.y = int(KEY_Y, d).map(sat_i32);
        if let Some(w) = int(KEY_WIDTH, d) {
            g.width = sat_u32(w);
        }
        if let Some(h) = int(KEY_HEIGHT, d) {
            g.height = sat_u32(h);
        }
        match wt.remove(KEY_MAXIMIZED) {
            None => {}
            Some(toml::Value::Boolean(b)) => g.maximized = b,
            Some(other) => d.entry(SECTION_WINDOWS, || {
                format!("{ctx}{KEY_MAXIMIZED}: {}", found("a boolean", &other))
            }),
        }
        g.extra = wt;
        out.insert(k, g);
    }
    out
}

impl WindowGeometry {
    fn to_table(&self) -> toml::Table {
        let mut t = self.extra.clone();
        strip_keys(&mut t, &WINDOW_KEYS);
        if let Some(x) = self.x {
            t.insert(KEY_X.into(), i64::from(x).into());
        }
        if let Some(y) = self.y {
            t.insert(KEY_Y.into(), i64::from(y).into());
        }
        t.insert(KEY_WIDTH.into(), i64::from(self.width).into());
        t.insert(KEY_HEIGHT.into(), i64::from(self.height).into());
        t.insert(KEY_MAXIMIZED.into(), self.maximized.into());
        t
    }
}

fn path_array(paths: &[PathBuf]) -> toml::Value {
    toml::Value::Array(
        paths
            .iter()
            .filter_map(|p| p.to_str())
            .map(|s| toml::Value::String(s.to_owned()))
            .collect(),
    )
}

fn strip_keys(t: &mut toml::Table, keys: &[&str]) {
    for k in keys {
        t.remove(*k);
    }
}

fn path_ok(p: &Path) -> bool {
    match p.to_str() {
        Some(s) => !s.is_empty() && s.len() <= MAX_PATH_BYTES && !s.chars().any(char::is_control),
        None => false,
    }
}

fn valid_window_key(k: &str) -> bool {
    !k.is_empty() && k.len() <= MAX_WINDOW_KEY_LEN && !k.chars().any(char::is_control)
}

/// Remove control characters and cut to [`MAX_LABEL_BYTES`] on a char
/// boundary.
fn clean_label(s: &mut String) {
    if s.chars().any(char::is_control) {
        s.retain(|c| !c.is_control());
    }
    truncate_bytes(s, MAX_LABEL_BYTES);
}

fn truncate_bytes(s: &mut String, max: usize) {
    if s.len() > max {
        let mut idx = max;
        while !s.is_char_boundary(idx) {
            idx -= 1;
        }
        s.truncate(idx);
    }
}

fn truncate_chars(s: &mut String, max: usize) {
    if let Some((idx, _)) = s.char_indices().nth(max) {
        s.truncate(idx);
    }
}

pub(crate) fn one_line(s: &str) -> String {
    let mut out: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&mut out, 512);
    out
}

// ---------------------------------------------------------------------------
// DIDs.

/// Format a DID the way the settings file stores it.
pub fn format_did(did: u32) -> String {
    format!("0x{did:08X}")
}

/// Parse a DID from `0x`-prefixed hex or decimal text. Surrounding
/// whitespace is ignored; returns `None` for anything else or values above
/// `u32::MAX`.
pub fn parse_did(text: &str) -> Option<u32> {
    let t = text.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        if hex.is_empty() || hex.len() > 8 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(hex, 16).ok()
    } else if !t.is_empty() && t.len() <= 10 && t.bytes().all(|b| b.is_ascii_digit()) {
        t.parse().ok()
    } else {
        None
    }
}

fn did_from_value(v: &toml::Value) -> Option<u32> {
    match v {
        toml::Value::Integer(i) => u32::try_from(*i).ok(),
        toml::Value::String(s) => parse_did(s),
        _ => None,
    }
}

fn bad_did(v: &toml::Value) -> String {
    let shown = match v {
        toml::Value::String(s) => format!("{:?}", key_for_message(s)),
        toml::Value::Integer(i) => i.to_string(),
        other => other.type_str().to_owned(),
    };
    format!("invalid DID {shown}: expected a 32-bit integer or \"0x\" hex string")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Settings {
        let mut s = Settings::default();
        s.remember_game_dir("/old/install");
        s.remember_game_dir("/games/install");
        s.set_last_opened(Some("client_surface.dat".into()), Some(0x4100_0123));
        s.push_recent_did(0x0A00_0001);
        assert!(s.add_bookmark(Bookmark::new(0x0A00_0001, "start")));
        assert!(s.add_bookmark(Bookmark {
            archive: Some("client_general.dat".into()),
            ..Bookmark::new(0x7000_0002, "pinned")
        }));
        assert!(s.set_window(
            "explorer",
            WindowGeometry {
                x: Some(10),
                y: Some(-20),
                width: 1600,
                height: 900,
                maximized: true,
                ..Default::default()
            }
        ));
        s
    }

    fn entry_sections(warnings: &[LoadWarning]) -> Vec<(&str, &str)> {
        warnings
            .iter()
            .filter_map(|w| match w {
                LoadWarning::Entry { section, message } => {
                    Some((section.as_str(), message.as_str()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn round_trip() {
        let s = sample();
        let text = s.to_toml_string().unwrap();
        assert!(text.starts_with('#'));
        assert!(text.contains("schema_version = 1"));
        assert!(text.contains("\"0x41000123\""));
        let back = Settings::from_toml_str(&text).unwrap();
        assert_eq!(back, s);
        let lenient = Settings::parse_lenient(&text);
        assert!(lenient.warnings.is_empty());
        assert_eq!(lenient.settings, s);
        assert_eq!(lenient.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn default_round_trip_and_empty_doc() {
        let d = Settings::default();
        assert_eq!(
            Settings::from_toml_str(&d.to_toml_string().unwrap()).unwrap(),
            d
        );
        assert_eq!(Settings::from_toml_str("").unwrap(), d);
    }

    #[test]
    fn game_dir_helpers() {
        let mut s = sample();
        assert_eq!(
            s.game.active_dir.as_deref(),
            Some(Path::new("/games/install"))
        );
        assert_eq!(
            s.game.known_dirs,
            vec![
                PathBuf::from("/games/install"),
                PathBuf::from("/old/install")
            ]
        );
        s.remember_game_dir("/old/install");
        assert_eq!(s.game.known_dirs[0], PathBuf::from("/old/install"));
        assert_eq!(s.game.known_dirs.len(), 2);
        s.forget_game_dir(Path::new("/old/install"));
        assert_eq!(s.game.active_dir, None);
        assert_eq!(s.game_dirs(), vec![Path::new("/games/install")]);
        s.remember_game_dir("");
        assert_eq!(s.game.known_dirs.len(), 1);
        for i in 0..100 {
            s.remember_game_dir(format!("/d{i}"));
        }
        assert_eq!(s.game.known_dirs.len(), MAX_GAME_DIRS);
        assert_eq!(s.game.known_dirs[0], PathBuf::from("/d99"));
    }

    #[test]
    fn recent_dids_bounded_and_deduplicated() {
        let mut s = Settings::default();
        for i in 0..200u32 {
            s.push_recent_did(i % 80);
        }
        assert_eq!(s.explorer.recent_dids.len(), MAX_RECENT_DIDS);
        assert_eq!(s.explorer.recent_dids[0], 199 % 80);
        let mut seen = s.explorer.recent_dids.clone();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), MAX_RECENT_DIDS);
    }

    #[test]
    fn bookmarks() {
        let mut s = sample();
        assert!(s.add_bookmark(Bookmark::new(0x0A00_0001, "renamed")));
        assert_eq!(s.explorer.bookmarks.len(), 2);
        assert_eq!(s.explorer.bookmarks[0].label, "renamed");
        assert!(s.add_bookmark(Bookmark::new(1, "x".repeat(1000))));
        assert_eq!(s.explorer.bookmarks[2].label.len(), MAX_LABEL_BYTES);
        assert_eq!(s.remove_bookmark(0x7000_0002, None), 0);
        assert_eq!(
            s.remove_bookmark(0x7000_0002, Some(Path::new("client_general.dat"))),
            1
        );
        for i in 0..2000u32 {
            let _ = s.add_bookmark(Bookmark::new(0x1000 + i, ""));
        }
        assert_eq!(s.explorer.bookmarks.len(), MAX_BOOKMARKS);
        assert!(!s.add_bookmark(Bookmark::new(0xFFFF_FFFF, "full")));
    }

    #[test]
    fn labels_are_cleaned_and_cut_on_char_boundaries() {
        let mut l = "\u{00e9}".repeat(300);
        clean_label(&mut l);
        assert_eq!(l.len(), MAX_LABEL_BYTES);
        let mut l = "\u{1F600}".repeat(100);
        clean_label(&mut l);
        assert!(l.len() <= MAX_LABEL_BYTES && l.len() > MAX_LABEL_BYTES - 4);
        let mut l = "a\u{0}b\nc\u{7f}d".to_owned();
        clean_label(&mut l);
        assert_eq!(l, "abcd");
    }

    #[test]
    fn windows_clamped_and_keys_validated() {
        let mut s = Settings::default();
        assert!(s.set_window(
            "w",
            WindowGeometry {
                x: Some(i32::MIN),
                y: Some(i32::MAX),
                width: 0,
                height: u32::MAX,
                ..Default::default()
            }
        ));
        let g = s.window("w").unwrap();
        assert_eq!(g.x, Some(-MAX_WINDOW_POS));
        assert_eq!(g.y, Some(MAX_WINDOW_POS));
        assert_eq!((g.width, g.height), (MIN_WINDOW_DIM, MAX_WINDOW_DIM));
        assert!(!s.set_window("", WindowGeometry::default()));
        assert!(!s.set_window(&"k".repeat(65), WindowGeometry::default()));
        assert!(!s.set_window("a\nb", WindowGeometry::default()));
        for i in 0..100 {
            let _ = s.set_window(&format!("w{i}"), WindowGeometry::default());
        }
        assert_eq!(s.windows.len(), MAX_WINDOWS);
        assert!(
            s.set_window("w", WindowGeometry::default()),
            "update in place"
        );
    }

    #[test]
    fn did_parsing() {
        assert_eq!(parse_did("0x41000123"), Some(0x4100_0123));
        assert_eq!(parse_did(" 0XaBc "), Some(0xABC));
        assert_eq!(parse_did("123"), Some(123));
        assert_eq!(parse_did("4294967295"), Some(u32::MAX));
        for bad in [
            "",
            "0x",
            "0x123456789",
            "4294967296",
            "-1",
            "0xZZ",
            "+1",
            "1.0",
            "0x+1",
        ] {
            assert_eq!(parse_did(bad), None, "{bad:?}");
        }
        assert_eq!(format_did(0xA), "0x0000000A");
    }

    /// Regression: bad entries are reported as `Entry` warnings in lenient
    /// mode (not just logged) and are errors in strict mode, including an
    /// invalid scalar `last_did`.
    #[test]
    fn bad_entries_are_reported_leniently_and_rejected_strictly() {
        let text = r#"
            schema_version = 1
            [explorer]
            last_did = 99999999999
            recent_dids = [1, "0x10", -5, "junk", 4294967296, true]
            bookmarks = [
                { did = 5, label = "ok" },
                { did = "nope", label = "bad did" },
                { label = "no did" },
                "not a table",
                { did = "0x6" },
                { did = 7, label = 3 },
            ]
        "#;
        let p = Settings::parse_lenient(text);
        let s = &p.settings;
        assert_eq!(s.explorer.last_did, None);
        assert_eq!(s.explorer.recent_dids, vec![1, 0x10]);
        assert_eq!(
            s.explorer.bookmarks,
            vec![
                Bookmark::new(5, "ok"),
                Bookmark::new(6, ""),
                Bookmark::new(7, "")
            ]
        );
        let entries = entry_sections(&p.warnings);
        assert_eq!(entries.len(), 1 + 4 + 4, "{entries:?}");
        assert!(entries.iter().all(|(s, _)| *s == "explorer"));
        assert!(entries[0].1.starts_with("last_did:"));
        assert!(
            entries
                .iter()
                .any(|(_, m)| m.starts_with("bookmarks[1].did"))
        );

        assert!(matches!(
            Settings::from_toml_str(text),
            Err(SettingsError::Parse(m)) if m.contains("last_did")
        ));
        assert!(Settings::from_toml_str("[explorer]\nlast_did = \"junk\"\n").is_err());
        assert!(Settings::from_toml_str("[explorer]\nrecent_dids = [\"0xZZ\"]\n").is_err());
        assert!(Settings::from_toml_str("[explorer]\nlast_did = 5\nrecent_dids = [1]\n").is_ok());
    }

    #[test]
    fn unknown_keys_preserved() {
        let text = r#"
            schema_version = 1
            top_level_future = "x"
            [future_section]
            a = [1, 2]
            [game]
            active_dir = "/g"
            future_game_key = 3
            [explorer]
            future_explorer_key = { nested = true }
        "#;
        let s = Settings::from_toml_str(text).unwrap();
        assert_eq!(s.game.known_dirs, vec![PathBuf::from("/g")]);
        assert!(s.extra.contains_key("future_section"));
        assert!(s.extra.contains_key("top_level_future"));
        assert!(s.game.extra.contains_key("future_game_key"));
        assert!(s.explorer.extra.contains_key("future_explorer_key"));
        let again = Settings::from_toml_str(&s.to_toml_string().unwrap()).unwrap();
        assert_eq!(again, s);
    }

    /// Regression: unknown keys inside bookmarks and window tables survive a
    /// round trip.
    #[test]
    fn per_entry_unknown_keys_preserved() {
        let text = r#"
            [[explorer.bookmarks]]
            did = 1
            label = "a"
            color = "red"
            [windows.explorer]
            width = 900
            height = 700
            monitor = "DP-1"
        "#;
        let s = Settings::from_toml_str(text).unwrap();
        assert_eq!(
            s.explorer.bookmarks[0].extra.get("color"),
            Some(&toml::Value::String("red".into()))
        );
        let out = s.to_toml_string().unwrap();
        assert!(out.contains("color = \"red\""), "{out}");
        assert!(out.contains("monitor = \"DP-1\""), "{out}");
        assert_eq!(Settings::from_toml_str(&out).unwrap(), s);
    }

    /// Regression: unknown datetime / local-date / local-time values keep
    /// their TOML type in every table, not just at the root.
    #[test]
    fn unknown_datetimes_keep_their_type() {
        let text = r#"
            top_ts = 1979-05-27T07:32:00Z
            [game]
            future_d = 1979-05-27
            [explorer]
            future_ts = 1979-05-27T07:32:00Z
            future_t = 07:32:00
            [[explorer.bookmarks]]
            did = 1
            when = 1979-05-27T07:32:00
            [windows.main]
            since = 1979-05-27
        "#;
        let s = Settings::from_toml_str(text).unwrap();
        let out = s.to_toml_string().unwrap();
        for line in [
            "top_ts = 1979-05-27T07:32:00Z",
            "future_d = 1979-05-27",
            "future_ts = 1979-05-27T07:32:00Z",
            "future_t = 07:32:00",
            "when = 1979-05-27T07:32:00",
            "since = 1979-05-27",
        ] {
            assert!(out.contains(line), "{line} missing from:\n{out}");
        }
        let back = Settings::from_toml_str(&out).unwrap();
        assert!(matches!(
            back.game.extra.get("future_d"),
            Some(toml::Value::Datetime(_))
        ));
        assert_eq!(back, s);
    }

    #[test]
    fn extras_cannot_shadow_known_sections() {
        let mut s = Settings::default();
        s.extra
            .insert("schema_version".into(), toml::Value::Integer(999));
        s.extra
            .insert("game".into(), toml::Value::String("evil".into()));
        let text = s.to_toml_string().unwrap();
        let p = Settings::parse_lenient(&text);
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);
        assert_eq!(p.schema_version, SCHEMA_VERSION);
    }

    /// Regression: section-level and per-entry extras cannot override known
    /// fields either.
    #[test]
    fn extras_cannot_shadow_known_fields() {
        let mut s = Settings::default();
        s.remember_game_dir("/real");
        s.push_recent_did(1);
        s.game.extra.insert("active_dir".into(), "/evil".into());
        s.game.extra.insert("known_dirs".into(), 3.into());
        s.explorer.extra.insert("recent_dids".into(), 5.into());
        s.explorer.extra.insert("bookmarks".into(), "x".into());
        s.explorer.extra.insert("last_did".into(), "junk".into());
        s.explorer.extra.insert("last_archive".into(), 1.into());
        assert!(s.add_bookmark(Bookmark::new(2, "b")));
        s.explorer.bookmarks[0]
            .extra
            .insert("did".into(), 99.into());
        let mut w = WindowGeometry::default();
        w.extra.insert("width".into(), "huge".into());
        assert!(s.set_window("main", w));

        let text = s.to_toml_string().unwrap();
        assert!(!text.contains("/evil"), "{text}");
        let back = Settings::from_toml_str(&text).unwrap();
        assert_eq!(back.game_dirs(), vec![Path::new("/real")]);
        assert_eq!(back.explorer.recent_dids, vec![1]);
        assert_eq!(back.explorer.bookmarks[0].did, 2);
        assert_eq!(back.window("main").unwrap().width, 1280);

        let mut clean = s.clone();
        clean.sanitize();
        assert!(clean.game.extra.is_empty() && clean.explorer.extra.is_empty());
        assert!(clean.explorer.bookmarks[0].extra.is_empty());
        assert!(clean.windows["main"].extra.is_empty());
    }

    #[test]
    fn lenient_damage_keeps_the_rest() {
        let text = r#"
            [game]
            active_dir = 42
            [explorer]
            last_did = "0x10"
            [windows]
            scalar = 3
            [windows.good]
            width = 800
            height = 600
            [windows.bad]
            width = "wide"
        "#;
        assert!(Settings::from_toml_str(text).is_err());
        let p = Settings::parse_lenient(text);
        assert!(!p.defaulted);
        assert_eq!(p.settings.game, GameSettings::default());
        assert_eq!(p.settings.explorer.last_did, Some(0x10));
        assert_eq!(p.settings.window("good").unwrap().width, 800);
        assert_eq!(p.settings.window("bad").unwrap().width, 1280, "field reset");
        assert!(p.settings.window("scalar").is_none());
        let sections: Vec<_> = entry_sections(&p.warnings)
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        assert_eq!(sections, vec!["game", "windows", "windows"]);
    }

    #[test]
    fn wrong_section_types() {
        let p = Settings::parse_lenient("game = 1\nexplorer = []\nwindows = \"x\"\n");
        assert_eq!(p.warnings.len(), 3);
        assert!(
            p.warnings
                .iter()
                .all(|w| matches!(w, LoadWarning::Section { .. }))
        );
        assert_eq!(p.settings, Settings::default());
    }

    /// Regression: the number of reported problems is bounded per section.
    #[test]
    fn warnings_are_capped_per_section() {
        let mut doc = String::from("[windows]\n");
        for i in 0..5000 {
            doc.push_str(&format!("w{i} = 1\n"));
        }
        doc.push_str("[explorer]\nrecent_dids = [");
        for _ in 0..5000 {
            doc.push_str("\"bad\",");
        }
        doc.push_str("]\n");
        let p = Settings::parse_lenient(&doc);
        assert!(p.warnings.len() <= 2 * (MAX_WARNINGS_PER_SECTION + 1));
        let suppressed: Vec<_> = p
            .warnings
            .iter()
            .filter_map(|w| match w {
                LoadWarning::Suppressed { section, count } => Some((section.as_str(), *count)),
                _ => None,
            })
            .collect();
        assert!(suppressed.contains(&("explorer", 5000 - MAX_WARNINGS_PER_SECTION)));
        // Window decoding stops once MAX_WINDOWS entries are kept; with none
        // kept every entry is examined.
        assert!(suppressed.contains(&("windows", 5000 - MAX_WARNINGS_PER_SECTION)));
    }

    #[test]
    fn schema_version_handling() {
        let p = Settings::parse_lenient("schema_version = \"one\"\n");
        assert!(matches!(
            p.warnings[..],
            [LoadWarning::BadSchemaVersion { assumed: 1, .. }]
        ));
        for bad in [
            "schema_version = 0",
            "schema_version = -3",
            "schema_version = 99999999999",
        ] {
            assert!(matches!(
                Settings::from_toml_str(bad),
                Err(SettingsError::BadSchemaVersion(_))
            ));
        }
        let p = Settings::parse_lenient("schema_version = 7\n[game]\nactive_dir = \"/g\"\n");
        assert_eq!(p.schema_version, 7);
        assert!(matches!(
            p.warnings[..],
            [LoadWarning::NewerSchema {
                found: 7,
                supported: 1
            }]
        ));
        assert_eq!(p.settings.game.active_dir, Some(PathBuf::from("/g")));
        assert!(matches!(
            Settings::from_toml_str("schema_version = 7"),
            Err(SettingsError::NewerSchema { found: 7, .. })
        ));
    }

    #[test]
    fn syntax_error_defaults() {
        let p = Settings::parse_lenient("[game\nactive_dir = ");
        assert!(p.defaulted);
        assert!(matches!(p.warnings[..], [LoadWarning::Syntax { .. }]));
        assert!(matches!(
            Settings::from_toml_str("= ="),
            Err(SettingsError::Parse(_))
        ));
    }

    #[test]
    fn sanitize_repairs_hand_edited_values() {
        let mut s = Settings::default();
        s.game.active_dir = Some("/a".into());
        s.game.known_dirs = vec![
            "/b".into(),
            "".into(),
            "/b".into(),
            "/c".into(),
            "/new\nline".into(),
            PathBuf::from("/".repeat(MAX_PATH_BYTES + 1)),
        ];
        s.explorer.last_archive = Some("".into());
        s.explorer.recent_dids = vec![1, 1, 2];
        s.explorer.bookmarks = vec![Bookmark::new(1, "a"), Bookmark::new(1, "dup")];
        s.windows.insert(String::new(), WindowGeometry::default());
        s.sanitize();
        assert_eq!(
            s.game.known_dirs,
            vec![
                PathBuf::from("/a"),
                PathBuf::from("/b"),
                PathBuf::from("/c")
            ]
        );
        assert_eq!(s.explorer.last_archive, None);
        assert_eq!(s.explorer.recent_dids, vec![1, 2]);
        assert_eq!(s.explorer.bookmarks, vec![Bookmark::new(1, "a")]);
        assert!(s.windows.is_empty());
        let mut many = Settings::default();
        many.game.known_dirs = (0..100).map(|i| PathBuf::from(format!("/k{i}"))).collect();
        many.game.active_dir = Some("/active".into());
        many.sanitize();
        assert_eq!(many.game.known_dirs.len(), MAX_GAME_DIRS);
        assert_eq!(many.game.known_dirs[0], PathBuf::from("/active"));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_are_dropped_not_fatal() {
        use std::os::unix::ffi::OsStrExt;
        let mut s = Settings::default();
        s.remember_game_dir(std::ffi::OsStr::from_bytes(b"/bad\xff").to_owned());
        s.remember_game_dir("/good");
        let back = Settings::from_toml_str(&s.to_toml_string().unwrap()).unwrap();
        assert_eq!(back.game.known_dirs, vec![PathBuf::from("/good")]);
    }

    /// Regression: the largest settings the helpers accept serialize below
    /// the file size limit, even with characters TOML must escape.
    #[test]
    fn worst_case_settings_fit_the_size_limit() {
        let nasty = |n: usize| -> String { "\"'\\".chars().cycle().take(n).collect() };
        let mut s = Settings::default();
        for i in 0..MAX_GAME_DIRS + 1 {
            let mut p = format!("/{i}");
            p.push_str(&nasty(MAX_PATH_BYTES - p.len()));
            s.remember_game_dir(p);
        }
        s.set_last_opened(Some(nasty(MAX_PATH_BYTES).into()), Some(u32::MAX));
        for i in 0..MAX_RECENT_DIDS as u32 {
            s.push_recent_did(u32::MAX - i);
        }
        for i in 0..MAX_BOOKMARKS as u32 {
            let mut archive = format!("{i}");
            archive.push_str(&nasty(MAX_PATH_BYTES - archive.len()));
            let label = format!("{}\u{0}", nasty(MAX_LABEL_BYTES * 2));
            assert!(s.add_bookmark(Bookmark {
                archive: Some(archive.into()),
                ..Bookmark::new(u32::MAX - i, label)
            }));
        }
        for i in 0..MAX_WINDOWS {
            let mut key = format!("{i}");
            key.push_str(&nasty(MAX_WINDOW_KEY_LEN - key.len()));
            assert!(s.set_window(
                &key,
                WindowGeometry {
                    x: Some(-MAX_WINDOW_POS),
                    y: Some(-MAX_WINDOW_POS),
                    width: MAX_WINDOW_DIM,
                    height: MAX_WINDOW_DIM,
                    maximized: true,
                    ..Default::default()
                }
            ));
        }
        // A full [[CreatedCharacters]] section of worst-case entries.
        let mut created = crate::CreatedCharacters::default();
        for i in 0..crate::MAX_CREATED_CHARACTERS {
            assert!(created.remember(crate::creation::tests::worst_entry(i)));
        }
        created.store(&mut s);
        let text = s.to_toml_string().unwrap();
        assert!(
            (text.len() as u64) < crate::MAX_FILE_SIZE * 3 / 4,
            "{} bytes",
            text.len()
        );
        let back = Settings::from_toml_str(&text).unwrap();
        assert_eq!(back.explorer.bookmarks.len(), MAX_BOOKMARKS);
        assert_eq!(back, s);
    }

    #[test]
    fn hostile_documents_do_not_panic() {
        let deep_arrays = format!("a = {}{}", "[".repeat(50_000), "]".repeat(50_000));
        let deep_tables = format!("a = {}{}", "{b=".repeat(50_000), "}".repeat(50_000));
        let dotted = format!("{} = 1", vec!["k"; 50_000].join("."));
        let docs = [
            deep_arrays,
            deep_tables,
            dotted,
            "\u{0}".into(),
            "schema_version = 1\nschema_version = 2".into(),
            "[explorer]\nbookmarks = 5".into(),
            "[[game]]\n[[game]]".into(),
            "windows = { a = { width = -1, x = 99999999999 } }".into(),
            format!("[windows.\"{}\"]\nwidth = 1", "\\u0000".repeat(10_000)),
        ];
        for d in &docs {
            let p = Settings::parse_lenient(d);
            let _ = p.settings.to_toml_string();
            let _ = Settings::from_toml_str(d);
        }
    }

    #[test]
    fn one_line_collapses_and_bounds() {
        assert_eq!(one_line("a\n  b\tc"), "a b c");
        assert_eq!(one_line(&"x ".repeat(2000)).chars().count(), 512);
    }
}
