// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's keymap file: a TOML document mapping `(context, action)` to a
//! list of bindings (`docs/specs/keymap.md` §6).
//!
//! A [`Keymap`] is plain data; it is not tied to a [`Registry`](crate::Registry),
//! so names this build does not know survive a load/save cycle. The user's
//! file normally holds only **overrides**: [`Keymap::overlay`] lays it over
//! [`default_keymap`](crate::default_keymap) and [`Keymap::diff_from`]
//! produces the overrides to save.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write as _};
use std::path::{Path, PathBuf};

use crate::action::is_valid_name;
use crate::chord::{Chord, bad};
use crate::device::AnalogAxis;
use crate::error::{InputError, KeymapWarning, Result};

/// Value of the top-level `format` key.
pub const FORMAT_TAG: &str = "tessera-keymap";
/// Format version this build writes and reads.
pub const KEYMAP_VERSION: i64 = 1;
/// Largest keymap document read or written (1 MiB).
pub const MAX_KEYMAP_SIZE: u64 = 1024 * 1024;
/// Most contexts in one keymap.
pub const MAX_KEYMAP_CONTEXTS: usize = 64;
/// Most actions per context in one keymap.
pub const MAX_KEYMAP_ACTIONS: usize = 1024;
/// Most bindings per action.
pub const MAX_BINDINGS_PER_ACTION: usize = 16;
/// Default sequence timeout in milliseconds.
pub const DEFAULT_SEQUENCE_TIMEOUT_MS: u32 = 1000;
/// Accepted sequence timeouts in milliseconds.
pub const SEQUENCE_TIMEOUT_RANGE_MS: std::ops::RangeInclusive<u32> = 50..=10_000;
/// Largest accepted `|scale|` of an analog binding.
pub const MAX_ANALOG_SCALE: f32 = 10_000.0;
/// Largest accepted dead zone of an analog binding.
pub const MAX_DEADZONE: f32 = 0.95;
/// File name of the keymap next to the settings file.
pub const KEYMAP_FILE_NAME: &str = "keymap.toml";
/// Environment variable overriding the keymap path.
pub const KEYMAP_FILE_ENV: &str = "TESSERA_KEYMAP_FILE";
/// Most warnings recorded while reading or compiling one keymap; past it a
/// single [`KeymapWarning::Truncated`] is added and the rest are dropped.
pub const MAX_KEYMAP_WARNINGS: usize = 64;
/// Most symbolic links followed when resolving the keymap file to write.
const MAX_SYMLINK_HOPS: usize = 40;

const FILE_HEADER: &str = "# Tessera keymap (docs/specs/keymap.md). Lists only the bindings that\n\
# differ from the built-in defaults; an empty list unbinds an action.\n\n";

/// One binding of an action.
#[derive(Debug, Clone, PartialEq)]
pub enum BindingSpec {
    /// A key, button or chord. For axis actions `value` (in `-1..=1`) is
    /// added to the action's value while the binding is held; button actions
    /// ignore it.
    Digital {
        /// What has to be pressed.
        chord: Chord,
        /// Axis contribution while held.
        value: f32,
    },
    /// A continuous source.
    Analog {
        /// The source.
        axis: AnalogAxis,
        /// Multiplier (negative inverts), `|scale| <=` [`MAX_ANALOG_SCALE`].
        scale: f32,
        /// Dead zone for absolute axes in `0..=`[`MAX_DEADZONE`]; ignored
        /// for relative axes.
        deadzone: f32,
    },
}

impl BindingSpec {
    /// A digital binding with value `1.0`.
    #[must_use]
    pub fn chord(chord: Chord) -> Self {
        Self::Digital { chord, value: 1.0 }
    }

    /// Parse binding text into a digital binding with value `1.0`.
    ///
    /// # Errors
    ///
    /// [`InputError::Binding`].
    pub fn parse(text: &str) -> Result<Self> {
        Chord::parse(text).map(Self::chord)
    }

    /// A digital binding with an axis value (clamped to `-1..=1`; non-finite
    /// becomes `1.0`).
    #[must_use]
    pub fn digital(chord: Chord, value: f32) -> Self {
        let value = if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            1.0
        };
        Self::Digital { chord, value }
    }

    /// An analog binding (values sanitised into their ranges).
    #[must_use]
    pub fn analog(axis: AnalogAxis, scale: f32, deadzone: f32) -> Self {
        let scale = if scale.is_finite() {
            scale.clamp(-MAX_ANALOG_SCALE, MAX_ANALOG_SCALE)
        } else {
            1.0
        };
        let deadzone = if deadzone.is_finite() {
            deadzone.clamp(0.0, MAX_DEADZONE)
        } else {
            0.0
        };
        Self::Analog {
            axis,
            scale,
            deadzone,
        }
    }

    /// The same binding with its numbers forced into the ranges a keymap
    /// file accepts, exactly as [`BindingSpec::digital`] and
    /// [`BindingSpec::analog`] do.
    #[must_use]
    pub fn normalized(&self) -> Self {
        match self {
            Self::Digital { chord, value } => Self::digital(chord.clone(), *value),
            Self::Analog {
                axis,
                scale,
                deadzone,
            } => Self::analog(*axis, *scale, *deadzone),
        }
    }

    /// Check that the binding can be written to a keymap file and read
    /// back unchanged: numbers finite and in range (§5.3), and every button
    /// of the chord has a keymap name.
    ///
    /// # Errors
    ///
    /// [`InputError::Binding`] naming the problem.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Digital { chord, value } => {
                if !(value.is_finite() && (-1.0..=1.0).contains(value)) {
                    return Err(bad(&chord.to_string(), "`value` must be in -1..=1"));
                }
                check_nameable(chord)
            }
            Self::Analog {
                axis,
                scale,
                deadzone,
            } => {
                if !(scale.is_finite() && scale.abs() <= MAX_ANALOG_SCALE) {
                    return Err(bad(
                        axis.name(),
                        &format!("`scale` must be within ±{MAX_ANALOG_SCALE}"),
                    ));
                }
                if !(deadzone.is_finite() && (0.0..=MAX_DEADZONE).contains(deadzone)) {
                    return Err(bad(
                        axis.name(),
                        &format!("`deadzone` must be in 0..={MAX_DEADZONE}"),
                    ));
                }
                Ok(())
            }
        }
    }

    fn to_toml(&self) -> String {
        match self {
            Self::Digital { chord, value } if *value == 1.0 => format!("\"{chord}\""),
            Self::Digital { chord, value } => {
                format!("{{ input = \"{chord}\", value = {} }}", float(*value))
            }
            Self::Analog {
                axis,
                scale,
                deadzone,
            } => {
                let mut s = format!("{{ axis = \"{axis}\"");
                if *scale != 1.0 {
                    let _ = write!(s, ", scale = {}", float(*scale));
                }
                if *deadzone != 0.0 {
                    let _ = write!(s, ", deadzone = {}", float(*deadzone));
                }
                s.push_str(" }");
                s
            }
        }
    }
}

/// Global keymap options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeymapOptions {
    /// Time allowed between the strokes of a sequence, in milliseconds;
    /// `None` = not set here (the default applies).
    pub sequence_timeout_ms: Option<u32>,
}

/// A keymap document: bindings per context and action, plus options.
///
/// Equality treats a NaN kept among the unknown top-level keys (§6.1) as
/// equal to another NaN, so that a keymap equals itself and its own
/// round trip; binding numbers are always finite (§6.3).
#[derive(Debug, Clone, Default)]
pub struct Keymap {
    /// Options.
    pub options: KeymapOptions,
    contexts: BTreeMap<String, BTreeMap<String, Vec<BindingSpec>>>,
    /// Unknown top-level keys, kept so a save does not lose them.
    extra: toml::Table,
}

impl PartialEq for Keymap {
    fn eq(&self, other: &Self) -> bool {
        self.options == other.options
            && self.contexts == other.contexts
            && same_table(&self.extra, &other.extra)
    }
}

/// Structural equality of TOML tables with NaN equal to NaN.
fn same_table(a: &toml::Table, b: &toml::Table) -> bool {
    a.len() == b.len()
        && a.iter()
            .all(|(k, va)| b.get(k).is_some_and(|vb| same_value(va, vb)))
}

fn same_value(a: &toml::Value, b: &toml::Value) -> bool {
    use toml::Value as V;
    match (a, b) {
        (V::Float(x), V::Float(y)) => x == y || (x.is_nan() && y.is_nan()),
        (V::Array(x), V::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_value(p, q))
        }
        (V::Table(x), V::Table(y)) => same_table(x, y),
        _ => a == b,
    }
}

/// Result of [`Keymap::parse_lenient`].
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedKeymap {
    /// Everything that could be used.
    pub keymap: Keymap,
    /// What was dropped and why.
    pub warnings: Vec<KeymapWarning>,
}

impl Keymap {
    /// An empty keymap.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The effective sequence timeout: the option clamped into
    /// [`SEQUENCE_TIMEOUT_RANGE_MS`], or [`DEFAULT_SEQUENCE_TIMEOUT_MS`].
    #[must_use]
    pub fn sequence_timeout_ms(&self) -> u32 {
        self.options
            .sequence_timeout_ms
            .map_or(DEFAULT_SEQUENCE_TIMEOUT_MS, |t| {
                t.clamp(
                    *SEQUENCE_TIMEOUT_RANGE_MS.start(),
                    *SEQUENCE_TIMEOUT_RANGE_MS.end(),
                )
            })
    }

    /// Bindings of `(context, action)`, `None` when the keymap does not
    /// mention the pair (an empty slice means "explicitly unbound").
    #[must_use]
    pub fn get(&self, context: &str, action: &str) -> Option<&[BindingSpec]> {
        self.contexts
            .get(context)
            .and_then(|a| a.get(action))
            .map(Vec::as_slice)
    }

    /// Replace the bindings of `(context, action)`. Every binding is
    /// [normalized](BindingSpec::normalized) (so what is stored is what a
    /// save writes and a load reads back), exact duplicates are removed and
    /// the list is cut to [`MAX_BINDINGS_PER_ACTION`].
    ///
    /// # Errors
    ///
    /// [`InputError::InvalidName`] for an invalid name;
    /// [`InputError::Binding`] when a chord holds a button with no keymap
    /// name (it could not be saved); [`InputError::TooMany`] when the
    /// context or action limit is reached. Nothing changes on error.
    pub fn set(&mut self, context: &str, action: &str, bindings: Vec<BindingSpec>) -> Result<()> {
        for n in [context, action] {
            if !is_valid_name(n) {
                return Err(InputError::InvalidName(n.into()));
            }
        }
        let bindings = bindings
            .iter()
            .map(|b| {
                let b = b.normalized();
                b.validate().map(|()| b)
            })
            .collect::<Result<Vec<_>>>()?;
        if !self.contexts.contains_key(context) && self.contexts.len() >= MAX_KEYMAP_CONTEXTS {
            return Err(InputError::TooMany {
                kind: "keymap contexts",
                limit: MAX_KEYMAP_CONTEXTS,
            });
        }
        let actions = self.contexts.entry(context.into()).or_default();
        if !actions.contains_key(action) && actions.len() >= MAX_KEYMAP_ACTIONS {
            return Err(InputError::TooMany {
                kind: "keymap actions",
                limit: MAX_KEYMAP_ACTIONS,
            });
        }
        actions.insert(action.into(), dedup(bindings));
        Ok(())
    }

    /// Forget `(context, action)` (so an overlay keeps the lower layer's
    /// bindings). Returns whether it was present.
    pub fn remove(&mut self, context: &str, action: &str) -> bool {
        let Some(actions) = self.contexts.get_mut(context) else {
            return false;
        };
        let had = actions.remove(action).is_some();
        if actions.is_empty() {
            self.contexts.remove(context);
        }
        had
    }

    /// Every `(context, action, bindings)` in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str, &[BindingSpec])> {
        self.contexts.iter().flat_map(|(c, actions)| {
            actions
                .iter()
                .map(move |(a, b)| (c.as_str(), a.as_str(), b.as_slice()))
        })
    }

    /// Context names mentioned, in order.
    pub fn context_names(&self) -> impl Iterator<Item = &str> {
        self.contexts.keys().map(String::as_str)
    }

    /// `self` with `top` laid over it: every `(context, action)` that `top`
    /// mentions replaces this one's list; options set in `top` win; `top`'s
    /// unknown keys are kept.
    #[must_use]
    pub fn overlay(&self, top: &Keymap) -> Keymap {
        let mut out = self.clone();
        if top.options.sequence_timeout_ms.is_some() {
            out.options.sequence_timeout_ms = top.options.sequence_timeout_ms;
        }
        for (c, a, b) in top.iter() {
            // Names in `top` were validated and are within limits.
            let actions = out.contexts.entry(c.into()).or_default();
            actions.insert(a.into(), b.to_vec());
        }
        for (k, v) in &top.extra {
            out.extra.insert(k.clone(), v.clone());
        }
        out
    }

    /// The overrides that turn `base` into `self`: pairs whose lists differ
    /// (pairs `base` has and `self` lacks become empty lists), and options
    /// that differ. `base.overlay(&self.diff_from(base)) == self` up to
    /// unknown keys.
    #[must_use]
    pub fn diff_from(&self, base: &Keymap) -> Keymap {
        let mut out = Keymap {
            extra: self.extra.clone(),
            ..Keymap::default()
        };
        if self.sequence_timeout_ms() != base.sequence_timeout_ms() {
            out.options.sequence_timeout_ms = Some(self.sequence_timeout_ms());
        }
        for (c, a, b) in self.iter() {
            if base.get(c, a) != Some(b) {
                out.contexts
                    .entry(c.into())
                    .or_default()
                    .insert(a.into(), b.to_vec());
            }
        }
        for (c, a, _) in base.iter() {
            if self.get(c, a).is_none() {
                out.contexts
                    .entry(c.into())
                    .or_default()
                    .insert(a.into(), Vec::new());
            }
        }
        out
    }

    /// Strictly parse a keymap document: any schema problem is an error.
    ///
    /// # Errors
    ///
    /// [`InputError::TooLarge`], [`InputError::Toml`],
    /// [`InputError::NewerVersion`] or [`InputError::Keymap`] with the
    /// first problem.
    pub fn parse(text: &str) -> Result<Keymap> {
        let (keymap, warnings, newer) = parse_inner(text)?;
        if let Some(found) = newer {
            return Err(InputError::NewerVersion {
                found,
                supported: KEYMAP_VERSION,
            });
        }
        match warnings.into_iter().next() {
            Some(w) => Err(InputError::Keymap(w)),
            None => Ok(keymap),
        }
    }

    /// Leniently parse a keymap document. Never fails: a document that is
    /// not TOML (or too large) yields an empty keymap, every bad section or
    /// entry is dropped, and each problem is reported (at most
    /// [`MAX_KEYMAP_WARNINGS`], then one [`KeymapWarning::Truncated`]). An
    /// action whose entries are all unusable is left out, so an overlay
    /// keeps its defaults; only a literally empty list unbinds.
    #[must_use]
    pub fn parse_lenient(text: &str) -> ParsedKeymap {
        match parse_inner(text) {
            Ok((keymap, warnings, _)) => ParsedKeymap { keymap, warnings },
            Err(e) => ParsedKeymap {
                keymap: Keymap::default(),
                warnings: vec![KeymapWarning::BadSection {
                    key: String::new(),
                    reason: e.to_string(),
                }],
            },
        }
    }

    /// Serialize to a keymap document (deterministic: names sorted, binding
    /// order kept). [`Keymap::parse`] of the output equals `self`; anything
    /// that would not read back is refused instead of written.
    ///
    /// # Errors
    ///
    /// [`InputError::Keymap`] when `options.sequence_timeout_ms` is outside
    /// [`SEQUENCE_TIMEOUT_RANGE_MS`]; [`InputError::Toml`] if the preserved
    /// unknown keys cannot be serialized; [`InputError::TooLarge`] past
    /// [`MAX_KEYMAP_SIZE`].
    pub fn to_toml_string(&self) -> Result<String> {
        if let Some(t) = self.options.sequence_timeout_ms
            && !SEQUENCE_TIMEOUT_RANGE_MS.contains(&t)
        {
            return Err(InputError::Keymap(timeout_warning()));
        }
        // Lists only enter through `set` (normalized), the parsers, overlay
        // and diff, so this holds; it is checked so a future path that
        // skips normalization fails loudly instead of writing a file that
        // loses every override on the next load.
        for (_, _, list) in self.iter() {
            for b in list {
                b.validate()?;
            }
        }
        let mut s = String::from(FILE_HEADER);
        let _ = writeln!(s, "format = \"{FORMAT_TAG}\"");
        let _ = writeln!(s, "version = {KEYMAP_VERSION}");
        if !self.extra.is_empty() {
            let extra =
                toml::to_string(&self.extra).map_err(|e| InputError::Toml(e.to_string()))?;
            s.push('\n');
            s.push_str(&extra);
        }
        if let Some(t) = self.options.sequence_timeout_ms {
            let _ = write!(s, "\n[options]\nsequence_timeout_ms = {t}\n");
        }
        for (c, actions) in &self.contexts {
            let _ = write!(s, "\n[bindings.{}]\n", key(c));
            for (a, list) in actions {
                let items: Vec<String> = list.iter().map(BindingSpec::to_toml).collect();
                let _ = writeln!(s, "{} = [{}]", key(a), items.join(", "));
            }
        }
        let size = s.len() as u64;
        if size > MAX_KEYMAP_SIZE {
            return Err(InputError::TooLarge {
                size,
                limit: MAX_KEYMAP_SIZE,
            });
        }
        Ok(s)
    }

    /// Read a keymap file leniently. `Ok(None)` when it does not exist.
    ///
    /// The type is checked before opening, and on Unix the file is opened
    /// non-blocking and checked again, so a FIFO or device node is refused
    /// without ever blocking.
    ///
    /// # Errors
    ///
    /// [`InputError::Io`] when it exists but cannot be read, is not a
    /// regular file, or is not UTF-8; [`InputError::TooLarge`].
    pub fn load_file(path: &Path) -> Result<Option<ParsedKeymap>> {
        let not_regular = || {
            io_err(
                "read",
                path,
                io::Error::new(io::ErrorKind::InvalidInput, "not a regular file"),
            )
        };
        match fs::metadata(path) {
            Ok(m) if !m.is_file() => return Err(not_regular()),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_err("stat", path, e)),
        }
        let file = match open_nonblocking(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_err("open", path, e)),
        };
        // Again on the open handle: the path may have changed in between.
        let meta = file.metadata().map_err(|e| io_err("stat", path, e))?;
        if !meta.is_file() {
            return Err(not_regular());
        }
        if meta.len() > MAX_KEYMAP_SIZE {
            return Err(InputError::TooLarge {
                size: meta.len(),
                limit: MAX_KEYMAP_SIZE,
            });
        }
        let mut bytes = Vec::new();
        file.take(MAX_KEYMAP_SIZE + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| io_err("read", path, e))?;
        if bytes.len() as u64 > MAX_KEYMAP_SIZE {
            return Err(InputError::TooLarge {
                size: bytes.len() as u64,
                limit: MAX_KEYMAP_SIZE,
            });
        }
        let text = String::from_utf8(bytes).map_err(|e| {
            io_err(
                "decode",
                path,
                io::Error::new(io::ErrorKind::InvalidData, e.utf8_error()),
            )
        })?;
        Ok(Some(Self::parse_lenient(&text)))
    }

    /// Atomically write this keymap to `path` (temporary file + rename),
    /// after the read-only install guard of `tessera-settings` has accepted
    /// the destination (`settings` supplies the recorded game directories).
    ///
    /// When `path` is a symbolic link, the file it points to is what gets
    /// written (the link is followed, at most 40 hops), and that is what
    /// the guard checks; the link itself is left alone. The guard runs on
    /// the resolved file before anything is written, and again on the
    /// canonical directory the temporary file is created in.
    ///
    /// # Errors
    ///
    /// [`InputError::Guard`] when the path is inside a game install or
    /// cannot be resolved; serialization errors; [`InputError::Io`] (also
    /// when the destination exists and is not a regular file).
    pub fn save_file(&self, path: &Path, settings: &tessera_settings::Settings) -> Result<()> {
        let target = write_target(path).map_err(|e| io_err("resolve", path, e))?;
        tessera_settings::check_not_inside_game_dirs(&target, settings)?;
        let text = self.to_toml_string()?;
        let name = target
            .file_name()
            .ok_or_else(|| {
                io_err(
                    "resolve",
                    path,
                    io::Error::new(io::ErrorKind::InvalidInput, "path does not name a file"),
                )
            })?
            .to_os_string();
        let dir = match target.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        };
        fs::create_dir_all(&dir).map_err(|e| io_err("create directory", &dir, e))?;
        // Check again on the directory that is really written, now that it
        // exists and resolves fully.
        let real_dir = fs::canonicalize(&dir).map_err(|e| io_err("resolve", &dir, e))?;
        let real = real_dir.join(&name);
        tessera_settings::check_not_inside_game_dirs(&real, settings)?;
        if let Ok(m) = fs::symlink_metadata(&real)
            && !m.is_file()
            && !m.file_type().is_symlink()
        {
            return Err(io_err(
                "replace",
                &real,
                io::Error::new(io::ErrorKind::InvalidInput, "not a regular file"),
            ));
        }
        let shown = name.to_string_lossy().into_owned();
        let pid = std::process::id();
        let mut opened = None;
        for n in 0..16u32 {
            let tmp = real_dir.join(format!(".{shown}.tmp-{pid}-{n}"));
            match OpenOptions::new().write(true).create_new(true).open(&tmp) {
                Ok(f) => {
                    opened = Some((tmp, f));
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(io_err("create", &tmp, e)),
            }
        }
        let Some((tmp, mut file)) = opened else {
            return Err(io_err(
                "create",
                &real_dir,
                io::Error::other("no free temporary file name"),
            ));
        };
        let result = file
            .write_all(text.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|e| io_err("write", &tmp, e))
            .and_then(|()| {
                drop(file);
                fs::rename(&tmp, &real).map_err(|e| io_err("rename", &real, e))
            });
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }
}

/// Where the keymap lives: `$TESSERA_KEYMAP_FILE` when set and non-empty,
/// otherwise `keymap.toml` next to the settings file
/// ([`tessera_settings::default_path`]).
///
/// # Errors
///
/// [`InputError::Guard`] when no config directory is known.
pub fn default_keymap_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os(KEYMAP_FILE_ENV).filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    Ok(tessera_settings::default_path()?.with_file_name(KEYMAP_FILE_NAME))
}

fn io_err(op: &'static str, path: &Path, source: io::Error) -> InputError {
    InputError::Io {
        op,
        path: path.to_path_buf(),
        source,
    }
}

/// Open for reading without blocking on a FIFO (Unix: `O_NONBLOCK`, which
/// does not change how a regular file reads).
fn open_nonblocking(path: &Path) -> io::Result<fs::File> {
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY);
    }
    opts.open(path)
}

/// The file a write to `path` really replaces: symbolic links in the final
/// component are followed (relative ones against the link's directory).
fn write_target(path: &Path) -> io::Result<PathBuf> {
    let names_file = |p: &Path| {
        matches!(
            p.components().next_back(),
            Some(std::path::Component::Normal(_))
        )
    };
    let mut p = path.to_path_buf();
    for _ in 0..MAX_SYMLINK_HOPS {
        if !names_file(&p) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path does not name a file",
            ));
        }
        match fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => {
                let link = fs::read_link(&p)?;
                p = if link.is_absolute() {
                    link
                } else {
                    p.parent().unwrap_or(Path::new("")).join(link)
                };
            }
            _ => return Ok(p),
        }
    }
    Err(io::Error::other("too many levels of symbolic links"))
}

fn check_nameable(chord: &Chord) -> Result<()> {
    for stroke in chord.strokes() {
        if let Some(b) = stroke.buttons().iter().find(|b| b.name().is_none()) {
            return Err(bad(
                &format!("{b:?}"),
                "this button has no keymap name and cannot be saved",
            ));
        }
    }
    Ok(())
}

fn timeout_warning() -> KeymapWarning {
    KeymapWarning::BadSection {
        key: "options.sequence_timeout_ms".into(),
        reason: format!(
            "expected an integer in {}..={}",
            SEQUENCE_TIMEOUT_RANGE_MS.start(),
            SEQUENCE_TIMEOUT_RANGE_MS.end()
        ),
    }
}

/// Record a warning unless [`MAX_KEYMAP_WARNINGS`] were already recorded;
/// the first one past the limit becomes a single `Truncated` marker.
pub(crate) fn push_warning(w: &mut Vec<KeymapWarning>, make: impl FnOnce() -> KeymapWarning) {
    match w.len().cmp(&MAX_KEYMAP_WARNINGS) {
        std::cmp::Ordering::Less => w.push(make()),
        std::cmp::Ordering::Equal => w.push(KeymapWarning::Truncated {
            what: "warnings".into(),
            limit: MAX_KEYMAP_WARNINGS,
        }),
        std::cmp::Ordering::Greater => {}
    }
}

/// TOML key text for a validated name (quoted when it contains `.`).
fn key(name: &str) -> String {
    if name.contains('.') {
        format!("\"{name}\"")
    } else {
        name.to_string()
    }
}

/// A float literal that TOML reads back as the same `f32` (callers pass
/// validated, finite values; anything else is written as `0.0` rather than
/// as text that is not TOML).
///
/// The reader parses the text as `f64` and narrows it to `f32` (§5.3), so
/// the shortest `f32` text can round twice and land on a neighbour. That
/// text is kept only when it survives that path; otherwise the exact `f64`
/// widening is written, whose shortest text parses back to the same `f64`
/// and so narrows back to `v` exactly (§6.3: the strict reader returns
/// exactly the keymap that was written).
fn float(v: f32) -> String {
    if !v.is_finite() {
        return "0.0".into();
    }
    let short = format!("{v:?}");
    let reads_back = short
        .parse::<f64>()
        .is_ok_and(|d| (d as f32).to_bits() == v.to_bits());
    let s = if reads_back {
        short
    } else {
        format!("{:?}", f64::from(v))
    };
    if s.contains(['.', 'e', 'E']) {
        s
    } else {
        format!("{s}.0")
    }
}

fn dedup(list: Vec<BindingSpec>) -> Vec<BindingSpec> {
    let mut out: Vec<BindingSpec> = Vec::with_capacity(list.len().min(MAX_BINDINGS_PER_ACTION));
    for b in list {
        if out.len() == MAX_BINDINGS_PER_ACTION {
            break;
        }
        if !out.contains(&b) {
            out.push(b);
        }
    }
    out
}

fn one_line(s: &str) -> String {
    let mut out: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.len() > 200 {
        let mut end = 200;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
    }
    out
}

type Inner = (Keymap, Vec<KeymapWarning>, Option<i64>);

fn parse_inner(text: &str) -> Result<Inner> {
    let size = text.len() as u64;
    if size > MAX_KEYMAP_SIZE {
        return Err(InputError::TooLarge {
            size,
            limit: MAX_KEYMAP_SIZE,
        });
    }
    let mut root: toml::Table =
        toml::from_str(text).map_err(|e| InputError::Toml(one_line(&e.to_string())))?;
    let mut w = Vec::new();
    let mut newer = None;
    let mut km = Keymap::default();

    if let Some(v) = root.remove("format")
        && v.as_str() != Some(FORMAT_TAG)
    {
        push_warning(&mut w, || KeymapWarning::BadSection {
            key: "format".into(),
            reason: format!("expected \"{FORMAT_TAG}\""),
        });
    }
    if let Some(v) = root.remove("version") {
        match v.as_integer() {
            Some(n) if n > KEYMAP_VERSION => {
                newer = Some(n);
                push_warning(&mut w, || KeymapWarning::BadSection {
                    key: "version".into(),
                    reason: format!("version {n} is newer than {KEYMAP_VERSION}; read best-effort"),
                });
            }
            Some(n) if n >= 1 => {}
            _ => push_warning(&mut w, || KeymapWarning::BadSection {
                key: "version".into(),
                reason: "expected a positive integer".into(),
            }),
        }
    }
    if let Some(v) = root.remove("options") {
        parse_options(v, &mut km.options, &mut w);
    }
    if let Some(v) = root.remove("bindings") {
        match v {
            toml::Value::Table(t) => parse_bindings(t, &mut km, &mut w),
            _ => push_warning(&mut w, || KeymapWarning::BadSection {
                key: "bindings".into(),
                reason: "expected a table".into(),
            }),
        }
    }
    km.extra = root;
    Ok((km, w, newer))
}

fn parse_options(v: toml::Value, out: &mut KeymapOptions, w: &mut Vec<KeymapWarning>) {
    let toml::Value::Table(t) = v else {
        push_warning(w, || KeymapWarning::BadSection {
            key: "options".into(),
            reason: "expected a table".into(),
        });
        return;
    };
    for (k, v) in t {
        match k.as_str() {
            "sequence_timeout_ms" => match v.as_integer().and_then(|n| u32::try_from(n).ok()) {
                Some(n) if SEQUENCE_TIMEOUT_RANGE_MS.contains(&n) => {
                    out.sequence_timeout_ms = Some(n);
                }
                _ => push_warning(w, timeout_warning),
            },
            other => push_warning(w, || KeymapWarning::BadSection {
                key: format!("options.{}", clip_name(other)),
                reason: "unknown option".into(),
            }),
        }
    }
}

fn clip_name(s: &str) -> String {
    let mut end = s.len().min(64);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

fn parse_bindings(t: toml::Table, km: &mut Keymap, w: &mut Vec<KeymapWarning>) {
    for (ctx, v) in t {
        if !is_valid_name(&ctx) {
            push_warning(w, || KeymapWarning::BadSection {
                key: format!("bindings.{}", clip_name(&ctx)),
                reason: "invalid context name".into(),
            });
            continue;
        }
        let toml::Value::Table(actions) = v else {
            push_warning(w, || KeymapWarning::BadSection {
                key: format!("bindings.{ctx}"),
                reason: "expected a table".into(),
            });
            continue;
        };
        if km.contexts.len() >= MAX_KEYMAP_CONTEXTS {
            push_warning(w, || KeymapWarning::Truncated {
                what: "contexts".into(),
                limit: MAX_KEYMAP_CONTEXTS,
            });
            break;
        }
        let mut out = BTreeMap::new();
        for (action, v) in actions {
            if !is_valid_name(&action) {
                push_warning(w, || KeymapWarning::BadSection {
                    key: format!("bindings.{ctx}.{}", clip_name(&action)),
                    reason: "invalid action name".into(),
                });
                continue;
            }
            if out.len() >= MAX_KEYMAP_ACTIONS {
                push_warning(w, || KeymapWarning::Truncated {
                    what: format!("actions in {ctx}"),
                    limit: MAX_KEYMAP_ACTIONS,
                });
                break;
            }
            let items = match v {
                toml::Value::Array(a) => a,
                s @ toml::Value::String(_) => vec![s],
                _ => {
                    push_warning(w, || KeymapWarning::BadSection {
                        key: format!("bindings.{ctx}.{action}"),
                        reason: "expected a list of bindings".into(),
                    });
                    continue;
                }
            };
            let had_entries = !items.is_empty();
            let mut list = Vec::new();
            for (index, item) in items.into_iter().enumerate() {
                if list.len() >= MAX_BINDINGS_PER_ACTION {
                    push_warning(w, || KeymapWarning::Truncated {
                        what: format!("bindings of {ctx}.{action}"),
                        limit: MAX_BINDINGS_PER_ACTION,
                    });
                    break;
                }
                match parse_entry(item) {
                    Ok(b) if list.contains(&b) => {}
                    Ok(b) => list.push(b),
                    Err(reason) => push_warning(w, || KeymapWarning::BadEntry {
                        context: ctx.clone(),
                        action: action.clone(),
                        index,
                        reason,
                    }),
                }
            }
            if had_entries && list.is_empty() {
                // Every entry was unusable (a typo, or device names from a
                // newer build): leave the pair out so the defaults stay,
                // instead of unbinding the action.
                push_warning(w, || KeymapWarning::BadSection {
                    key: format!("bindings.{ctx}.{action}"),
                    reason: "no usable binding; the defaults are kept".into(),
                });
                continue;
            }
            out.insert(action, dedup(list));
        }
        km.contexts.insert(ctx, out);
    }
}

fn parse_entry(item: toml::Value) -> std::result::Result<BindingSpec, String> {
    let reason = |e: InputError| match e {
        InputError::Binding { reason, .. } => reason,
        other => other.to_string(),
    };
    match item {
        toml::Value::String(s) => BindingSpec::parse(&s).map_err(reason),
        toml::Value::Table(t) => {
            let num = |k: &str| -> std::result::Result<Option<f64>, String> {
                match t.get(k) {
                    None => Ok(None),
                    Some(toml::Value::Float(f)) if f.is_finite() => Ok(Some(*f)),
                    Some(toml::Value::Integer(i)) => Ok(Some(*i as f64)),
                    Some(_) => Err(format!("`{k}` must be a finite number")),
                }
            };
            if let Some(k) = t.keys().find(|k| {
                !matches!(
                    k.as_str(),
                    "input" | "value" | "axis" | "scale" | "deadzone" | "invert"
                )
            }) {
                return Err(format!("unknown field `{}`", clip_name(k)));
            }
            match (t.get("input"), t.get("axis")) {
                (Some(toml::Value::String(s)), None) => {
                    if ["scale", "deadzone", "invert"]
                        .iter()
                        .any(|k| t.contains_key(*k))
                    {
                        return Err("`scale`, `deadzone` and `invert` need `axis`".into());
                    }
                    let chord = Chord::parse(s).map_err(reason)?;
                    let value = num("value")?.unwrap_or(1.0) as f32;
                    if !(-1.0..=1.0).contains(&value) {
                        return Err("`value` must be in -1..=1".into());
                    }
                    Ok(BindingSpec::Digital { chord, value })
                }
                (None, Some(toml::Value::String(s))) => {
                    if t.contains_key("value") {
                        return Err("`value` needs `input`".into());
                    }
                    let axis = AnalogAxis::from_name(s)
                        .ok_or_else(|| format!("unknown axis {:?}", clip_name(s)))?;
                    // Ranges are checked on the stored `f32` values, so the
                    // documented bounds (`0.95` written in a file) are
                    // themselves accepted.
                    let mut scale = num("scale")?.unwrap_or(1.0) as f32;
                    if scale.abs() > MAX_ANALOG_SCALE {
                        return Err(format!("`scale` must be within ±{MAX_ANALOG_SCALE}"));
                    }
                    let deadzone = num("deadzone")?.unwrap_or(0.0) as f32;
                    if !(0.0..=MAX_DEADZONE).contains(&deadzone) {
                        return Err(format!("`deadzone` must be in 0..={MAX_DEADZONE}"));
                    }
                    match t.get("invert") {
                        None | Some(toml::Value::Boolean(false)) => {}
                        Some(toml::Value::Boolean(true)) => scale = -scale,
                        Some(_) => return Err("`invert` must be a boolean".into()),
                    }
                    Ok(BindingSpec::Analog {
                        axis,
                        scale,
                        deadzone,
                    })
                }
                _ => Err("expected exactly one of `input` (string) or `axis` (string)".into()),
            }
        }
        _ => Err("expected a string or an inline table".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
format = "tessera-keymap"
version = 1

[options]
sequence_timeout_ms = 750

[bindings.movement]
move_forward = ["W", { input = "S", value = -1 }, { axis = "PadLeftY", deadzone = 0.15, invert = true }]
jump = "Space"

[bindings."ui.dialog"]
"close.all" = ["Ctrl+K Ctrl+W"]
"#;

    #[test]
    fn parses_sample() {
        let k = Keymap::parse(SAMPLE).unwrap();
        assert_eq!(k.sequence_timeout_ms(), 750);
        let f = k.get("movement", "move_forward").unwrap();
        assert_eq!(f.len(), 3);
        assert_eq!(f[0], BindingSpec::parse("W").unwrap());
        assert_eq!(
            f[1],
            BindingSpec::Digital {
                chord: Chord::parse("S").unwrap(),
                value: -1.0
            }
        );
        assert_eq!(
            f[2],
            BindingSpec::Analog {
                axis: AnalogAxis::PadLeftY,
                scale: -1.0,
                deadzone: 0.15
            }
        );
        assert_eq!(k.get("movement", "jump").unwrap().len(), 1);
        assert_eq!(
            k.get("ui.dialog", "close.all").unwrap()[0],
            BindingSpec::parse("Ctrl+K Ctrl+W").unwrap()
        );
    }

    #[test]
    fn round_trip() {
        let k = Keymap::parse(SAMPLE).unwrap();
        let text = k.to_toml_string().unwrap();
        assert_eq!(Keymap::parse(&text).unwrap(), k, "{text}");
        let d = crate::default_keymap();
        let text = d.to_toml_string().unwrap();
        assert_eq!(Keymap::parse(&text).unwrap(), d);
    }

    #[test]
    fn unknown_top_level_kept() {
        let k = Keymap::parse("future = 3\n[later]\nx = \"y\"\n").unwrap();
        let text = k.to_toml_string().unwrap();
        let again = Keymap::parse(&text).unwrap();
        assert_eq!(again, k);
        assert!(text.contains("future = 3"));
    }

    #[test]
    fn lenient_drops_bad_parts() {
        let text = r#"
format = "other"
version = "x"
[options]
sequence_timeout_ms = 5
bogus = 1
[bindings]
Bad = {}
loose = 3
[bindings.movement]
move_forward = ["W", "Nope", 3, { input = "S", value = 2 }, { axis = "MouseX", deadzone = 1.5 }, { input = "A", axis = "MouseX" }, { input = "Q", what = 1 }]
jump = 4
"#;
        let p = Keymap::parse_lenient(text);
        assert_eq!(p.keymap.get("movement", "move_forward").unwrap().len(), 1);
        assert!(p.keymap.get("movement", "jump").is_none());
        assert!(p.warnings.len() >= 10, "{:#?}", p.warnings);
        assert!(Keymap::parse(text).is_err());
    }

    #[test]
    fn errors() {
        assert!(matches!(Keymap::parse("= ="), Err(InputError::Toml(_))));
        assert!(matches!(
            Keymap::parse("version = 99"),
            Err(InputError::NewerVersion { found: 99, .. })
        ));
        let p = Keymap::parse_lenient("version = 99\n[bindings.a]\nb = [\"W\"]");
        assert_eq!(p.warnings.len(), 1);
        assert!(p.keymap.get("a", "b").is_some());
        let big = "x".repeat(MAX_KEYMAP_SIZE as usize + 1);
        assert!(matches!(
            Keymap::parse(&big),
            Err(InputError::TooLarge { .. })
        ));
        assert_eq!(Keymap::parse_lenient("[[[").keymap, Keymap::default());
    }

    #[test]
    fn limits_and_dedup() {
        let mut list = String::new();
        for _ in 0..40 {
            list.push_str("\"W\", \"Q\", ");
        }
        let mut letters = String::new();
        for c in 'A'..='Z' {
            let _ = write!(letters, "\"{c}\", ");
        }
        let text = format!("[bindings.c]\na = [{list}]\nb = [{letters}]\n");
        let p = Keymap::parse_lenient(&text);
        assert_eq!(p.keymap.get("c", "a").unwrap().len(), 2);
        assert_eq!(
            p.keymap.get("c", "b").unwrap().len(),
            MAX_BINDINGS_PER_ACTION
        );
        assert_eq!(p.warnings.len(), 1);
    }

    #[test]
    fn overlay_and_diff() {
        let base = crate::default_keymap();
        let mut user = Keymap::new();
        user.set("movement", "jump", vec![BindingSpec::parse("J").unwrap()])
            .unwrap();
        user.set("movement", "autorun", vec![]).unwrap();
        user.options.sequence_timeout_ms = Some(400);
        let eff = base.overlay(&user);
        assert_eq!(
            eff.get("movement", "jump").unwrap()[0],
            BindingSpec::parse("J").unwrap()
        );
        assert_eq!(eff.get("movement", "autorun").unwrap(), &[]);
        assert_eq!(
            eff.get("movement", "move_forward"),
            base.get("movement", "move_forward")
        );
        let diff = eff.diff_from(&base);
        assert_eq!(diff, user);
        assert_eq!(base.overlay(&diff), eff);
        // A pair removed from the effective map becomes an explicit unbind.
        let mut eff2 = eff.clone();
        assert!(eff2.remove("camera", "zoom"));
        assert_eq!(eff2.diff_from(&base).get("camera", "zoom"), Some(&[][..]));
    }

    #[test]
    fn set_validates() {
        let mut k = Keymap::new();
        assert!(k.set("Bad", "a", vec![]).is_err());
        assert!(k.set("a", "", vec![]).is_err());
    }

    /// `float` text read the way the reader does (f64, then narrowed)
    /// gives back the same bits, over a spread of bit patterns.
    #[test]
    fn float_text_survives_f64_narrowing() {
        let mut bits = 0u32;
        loop {
            let v = f32::from_bits(bits);
            if v.is_finite() {
                let text = float(v);
                let back = text.parse::<f64>().unwrap() as f32;
                assert_eq!(back.to_bits(), bits, "{text}");
            }
            match bits.checked_add(16_411) {
                Some(b) => bits = b,
                None => break,
            }
        }
        let v = f32::from_bits(0x15ae_43fd);
        assert_eq!(
            (float(v).parse::<f64>().unwrap() as f32).to_bits(),
            0x15ae_43fd
        );
    }

    /// A NaN under an unknown top-level key (kept, §6.1) does not make the
    /// keymap unequal to itself or to its round trip.
    #[test]
    fn nan_in_unknown_key_compares_equal() {
        for doc in [
            "a = nan",
            "a = -nan",
            "a = [1.0, nan]\n[x]\ny = { z = nan }",
        ] {
            let k = Keymap::parse(doc).unwrap();
            let lenient = Keymap::parse_lenient(doc);
            assert!(lenient.warnings.is_empty(), "{doc}");
            assert_eq!(lenient.keymap, k, "{doc}");
            assert_eq!(k, k.clone(), "{doc}");
            let again = Keymap::parse(&k.to_toml_string().unwrap()).unwrap();
            assert_eq!(again, k, "{doc}");
        }
        assert_ne!(
            Keymap::parse("a = nan").unwrap(),
            Keymap::parse("a = 1.0").unwrap()
        );
        assert_ne!(
            Keymap::parse("a = nan").unwrap(),
            Keymap::parse("b = nan").unwrap()
        );
    }

    /// Every hostile number a caller can put into a public `BindingSpec`.
    const HOSTILE: [f32; 14] = [
        // Its shortest f32 text rounds to a neighbour when read through f64.
        f32::from_bits(0x15ae_43fd),
        f32::from_bits(0x95ae_43fd),
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        3.0,
        -3.0,
        1e30,
        -1e30,
        0.97,
        -0.0,
        f32::MIN_POSITIVE,
        1.0,
        0.5,
    ];

    #[test]
    fn hostile_specs_round_trip_strictly() {
        use crate::device::{AnalogAxis as Ax, Button, KeyCode, MouseButton};
        for &x in &HOSTILE {
            for &y in &HOSTILE {
                let mut k = Keymap::new();
                k.set("movement", "jump", vec![BindingSpec::parse("J").unwrap()])
                    .unwrap();
                let chord = Chord::parse("A").unwrap();
                k.set(
                    "movement",
                    "turn",
                    vec![BindingSpec::Digital {
                        chord: chord.clone(),
                        value: x,
                    }],
                )
                .unwrap();
                k.set(
                    "camera",
                    "look_x",
                    vec![BindingSpec::Analog {
                        axis: Ax::PadRightX,
                        scale: x,
                        deadzone: y,
                    }],
                )
                .unwrap();
                for (_, _, list) in k.iter() {
                    for b in list {
                        b.validate().unwrap();
                    }
                }
                let text = k.to_toml_string().unwrap();
                assert_eq!(Keymap::parse(&text).unwrap(), k, "{x} {y}: {text}");
                let lenient = Keymap::parse_lenient(&text);
                assert!(lenient.warnings.is_empty());
                assert!(lenient.keymap.get("movement", "jump").is_some());
            }
        }
        // A button without a keymap name is refused, a numbered one is kept.
        let unnamed = crate::Stroke::button(Button::Mouse(MouseButton::Other(3)));
        let mut k = Keymap::new();
        let err = k
            .set(
                "movement",
                "jump",
                vec![BindingSpec::chord(Chord::single(unnamed))],
            )
            .unwrap_err();
        assert!(matches!(err, InputError::Binding { .. }), "{err}");
        assert!(k.get("movement", "jump").is_none(), "nothing changed");
        let mouse40 = crate::Stroke::new(
            crate::Modifiers::empty(),
            [
                Button::Mouse(MouseButton::Other(40)),
                Button::Key(KeyCode::G),
            ],
        )
        .unwrap();
        k.set(
            "movement",
            "jump",
            vec![BindingSpec::chord(Chord::single(mouse40))],
        )
        .unwrap();
        let text = k.to_toml_string().unwrap();
        assert!(text.contains("\"G+Mouse40\""), "{text}");
        assert_eq!(Keymap::parse(&text).unwrap(), k);
    }

    #[test]
    fn documented_bounds_are_accepted() {
        let k = Keymap::parse(
            "[bindings.c]\na = [{ axis = \"PadLeftX\", deadzone = 0.95, scale = -10000 }]\n",
        )
        .unwrap();
        assert_eq!(
            k.get("c", "a").unwrap()[0],
            BindingSpec::analog(AnalogAxis::PadLeftX, -MAX_ANALOG_SCALE, MAX_DEADZONE)
        );
        assert!(
            Keymap::parse("[bindings.c]\na = [{ axis = \"PadLeftX\", deadzone = 0.951 }]").is_err()
        );
    }

    #[test]
    fn bad_options_are_not_written() {
        let mut k = Keymap::new();
        k.options.sequence_timeout_ms = Some(0);
        assert_eq!(k.sequence_timeout_ms(), *SEQUENCE_TIMEOUT_RANGE_MS.start());
        assert!(matches!(k.to_toml_string(), Err(InputError::Keymap(_))));
        k.options.sequence_timeout_ms = Some(u32::MAX);
        assert_eq!(k.sequence_timeout_ms(), *SEQUENCE_TIMEOUT_RANGE_MS.end());
        assert!(k.to_toml_string().is_err());
        k.options.sequence_timeout_ms = Some(300);
        let text = k.to_toml_string().unwrap();
        assert_eq!(Keymap::parse(&text).unwrap(), k);
        assert_eq!(float(f32::NAN), "0.0");
    }

    #[test]
    fn warnings_are_capped() {
        let mut text = String::from("[bindings.c]\na = [");
        while text.len() < (MAX_KEYMAP_SIZE as usize) - 16 {
            text.push_str("3,");
        }
        text.push_str("]\n");
        let p = Keymap::parse_lenient(&text);
        assert_eq!(p.warnings.len(), MAX_KEYMAP_WARNINGS + 1);
        assert!(matches!(
            p.warnings.last(),
            Some(KeymapWarning::Truncated { limit, .. }) if *limit == MAX_KEYMAP_WARNINGS
        ));
        let mut text = String::from("[bindings.c]\n");
        for i in 0..5000 {
            let _ = writeln!(text, "A{i} = 1");
        }
        assert_eq!(
            Keymap::parse_lenient(&text).warnings.len(),
            MAX_KEYMAP_WARNINGS + 1
        );
        // Strict parsing still reports the first problem.
        assert!(matches!(
            Keymap::parse(&text),
            Err(InputError::Keymap(KeymapWarning::BadSection { .. }))
        ));
    }

    #[test]
    fn all_invalid_entries_keep_defaults() {
        let base = crate::default_keymap();
        let p = Keymap::parse_lenient(
            "[bindings.movement]\njump = [\"Spcae\"]\nwalk = []\nautorun = [\"Bogus\", \"T\"]\n",
        );
        assert!(p.keymap.get("movement", "jump").is_none());
        assert_eq!(p.keymap.get("movement", "walk"), Some(&[][..]));
        assert_eq!(p.keymap.get("movement", "autorun").unwrap().len(), 1);
        assert!(
            p.warnings.iter().any(|w| matches!(
                w,
                KeymapWarning::BadSection { key, .. } if key == "bindings.movement.jump"
            )),
            "{:?}",
            p.warnings
        );
        let eff = base.overlay(&p.keymap);
        assert_eq!(eff.get("movement", "jump"), base.get("movement", "jump"));
        assert_eq!(eff.get("movement", "walk"), Some(&[][..]));
    }

    #[cfg(unix)]
    #[test]
    fn fifo_is_refused_without_blocking() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("keymap.toml");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|s| s.success());
        if !made {
            eprintln!("mkfifo unavailable; skipping");
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let path = fifo.clone();
        std::thread::spawn(move || {
            let _ = tx.send(
                Keymap::load_file(&path)
                    .map(|_| ())
                    .map_err(|e| e.to_string()),
            );
        });
        let got = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("load_file blocked on a FIFO");
        assert!(got.unwrap_err().contains("not a regular file"));
    }

    #[cfg(unix)]
    #[test]
    fn save_through_symlinks_writes_the_target_only() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let settings = tessera_settings::Settings::default();
        let install = dir.path().join("install");
        let home = dir.path().join("home");
        fs::create_dir_all(&install).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::write(install.join("client_general.dat"), b"").unwrap();
        let k = Keymap::parse(SAMPLE).unwrap();
        // A link inside the install that points out of it: the target is
        // written, the install (and the link in it) is left alone.
        let inside_link = install.join(KEYMAP_FILE_NAME);
        symlink(home.join(KEYMAP_FILE_NAME), &inside_link).unwrap();
        k.save_file(&inside_link, &settings).unwrap();
        assert!(
            fs::symlink_metadata(&inside_link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let back = Keymap::load_file(&home.join(KEYMAP_FILE_NAME))
            .unwrap()
            .unwrap();
        assert_eq!(back.keymap, k);
        let names: Vec<_> = fs::read_dir(&install)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            names.len(),
            2,
            "no temporary files in the install: {names:?}"
        );
        // A link outside that points into the install (also via a relative
        // chain of links) is refused before anything is written.
        let target = install.join("other.toml");
        let outside_link = home.join("evil.toml");
        symlink(&target, &outside_link).unwrap();
        let chain = home.join("chain.toml");
        symlink("evil.toml", &chain).unwrap();
        for p in [&outside_link, &chain] {
            let err = k.save_file(p, &settings).unwrap_err();
            assert!(matches!(err, InputError::Guard(_)), "{err}");
            assert!(!target.exists());
        }
        // A symlink loop is an error, not a hang.
        let a = home.join("a.toml");
        let b = home.join("b.toml");
        symlink(&b, &a).unwrap();
        symlink(&a, &b).unwrap();
        assert!(k.save_file(&a, &settings).is_err());
        // A directory in the way is not replaced.
        let d = home.join("dir.toml");
        fs::create_dir(&d).unwrap();
        assert!(matches!(
            k.save_file(&d, &settings),
            Err(InputError::Io { .. })
        ));
    }

    #[test]
    fn file_io() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join(KEYMAP_FILE_NAME);
        assert!(Keymap::load_file(&path).unwrap().is_none());
        let k = Keymap::parse(SAMPLE).unwrap();
        k.save_file(&path, &tessera_settings::Settings::default())
            .unwrap();
        let back = Keymap::load_file(&path).unwrap().unwrap();
        assert!(back.warnings.is_empty());
        assert_eq!(back.keymap, k);
        // A fake install directory is refused.
        let install = dir.path().join("install");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("client_general.dat"), b"").unwrap();
        let err = k
            .save_file(
                &install.join(KEYMAP_FILE_NAME),
                &tessera_settings::Settings::default(),
            )
            .unwrap_err();
        assert!(matches!(err, InputError::Guard(_)), "{err}");
        assert!(!install.join(KEYMAP_FILE_NAME).exists());
        // Directories are not keymaps.
        assert!(Keymap::load_file(dir.path()).is_err());
    }
}
