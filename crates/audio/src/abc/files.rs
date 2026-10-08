// SPDX-License-Identifier: GPL-3.0-or-later
//! Song files: lookup in the music folder (§3.1), reading, and the song listing for
//! `/playlist` (§11). Everything here only reads.
//!
//! All functions here do blocking file I/O: call them from a worker, never from the frame
//! or audio thread.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::{Component, Path, PathBuf};

use super::chars::{Bom, Ch, CharStream, detect_bom, is_letter};
use super::error::{AbcError, AbcErrorKind, NOTICE_TITLE_MAX_CHARS};
use super::{PartRequest, Performance, parse};

/// Name of the music folder inside the settings folder (`abc-dialect.md` §3.1;
/// `install-layout.md` §8.1).
pub const MUSIC_FOLDER: &str = "Music";

/// Largest song file read, bytes (Tessera's bound, §14.2). A 10,000-event part needs well
/// under 1 MiB.
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// Most files listed by [`list_songs`] (Tessera's bound).
pub const MAX_LISTED_FILES: usize = 4096;

/// Most parts listed per file by [`list_songs`] (Tessera's bound).
pub const MAX_LISTED_PARTS: usize = 1024;

/// Most parts listed by one [`list_songs`] call across all files (Tessera's bound).
pub const MAX_LISTED_PARTS_TOTAL: usize = 65_536;

/// Most bytes read by one [`list_songs`] call across all files (Tessera's bound).
pub const MAX_LISTING_BYTES: u64 = 128 * 1024 * 1024;

/// Longest title kept by [`list_songs`], in characters: the length the "now playing"
/// notice shows (§6.5, §10.4).
pub const LISTING_TITLE_MAX_CHARS: usize = NOTICE_TITLE_MAX_CHARS;

/// Finds a song in the music folder (§3.1): the name joined to the folder, with the
/// extension `.abc`, else `.txt`. The last extension of the name, if any, is replaced
/// (the reading the spec's OPEN note in §3.1 recommends).
///
/// Letter case: for each extension the exact path is tried first; if it does not exist,
/// the folder that would hold it is searched for a file name equal to it ignoring ASCII
/// letter case (the first such name in sort order), as the original's file system does
/// (§3.1 OPEN). Only the last component is matched this way.
///
/// Tessera keeps lookups inside the folder: an empty name, an absolute name or a name with
/// `..` components is not found.
///
/// # Errors
/// [`AbcErrorKind::FileNotFound`] with the `.txt` path (the last one tried) as its path.
pub fn find_song(music_dir: &Path, name: &str) -> Result<PathBuf, AbcError> {
    let rel = Path::new(name);
    let base = music_dir.join(rel);
    let not_found = |base: &Path| AbcError {
        kind: AbcErrorKind::FileNotFound,
        position: None,
        path: Some(base.with_extension("txt")),
    };
    let inside = !name.is_empty()
        && rel
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
        && rel.file_name().is_some();
    if !inside {
        return Err(not_found(&base));
    }
    for ext in ["abc", "txt"] {
        let p = base.with_extension(ext);
        if p.exists() {
            return Ok(p);
        }
        if let Some(p) = find_ignoring_case(&p) {
            return Ok(p);
        }
    }
    Err(not_found(&base))
}

/// The entry of `target`'s folder whose name equals `target`'s file name ignoring ASCII
/// letter case; the first in sort order if several do.
fn find_ignoring_case(target: &Path) -> Option<PathBuf> {
    let want = target.file_name()?.as_encoded_bytes();
    let parent = target.parent()?;
    let mut best: Option<OsString> = None;
    for entry in std::fs::read_dir(parent).ok()?.flatten() {
        let name = entry.file_name();
        if name.as_encoded_bytes().eq_ignore_ascii_case(want)
            && best.as_ref().is_none_or(|b| name < *b)
        {
            best = Some(name);
        }
    }
    best.map(|name| parent.join(name))
}

/// Reads a regular file read-only, refusing files over `limit` bytes.
///
/// Anything that is not a regular file after following symbolic links (a FIFO, a device, a
/// folder) is refused with [`AbcErrorKind::Io`]`(`[`ErrorKind::InvalidInput`]`)` before it
/// is opened, so that opening cannot block. (A path swapped for such a file between the
/// check and the open can still block the open; the music folder is the user's own.)
fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, AbcError> {
    let err = |kind| AbcError {
        kind,
        position: None,
        path: Some(path.to_path_buf()),
    };
    let io = |e: std::io::Error| err(AbcErrorKind::Io(e.kind()));
    let not_regular = || err(AbcErrorKind::Io(ErrorKind::InvalidInput));
    let meta = std::fs::metadata(path).map_err(io)?;
    if !meta.is_file() {
        return Err(not_regular());
    }
    if meta.len() > limit {
        return Err(err(AbcErrorKind::FileTooLarge));
    }
    let file = File::open(path).map_err(io)?;
    if !file.metadata().map_err(io)?.is_file() {
        return Err(not_regular());
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() as u64 > limit {
        return Err(err(AbcErrorKind::FileTooLarge));
    }
    Ok(bytes)
}

/// Finds, reads and parses one part of a song (§3, §11: what `/play <name> [part]` does
/// before the performance starts). Blocking file I/O: not for the frame or audio thread.
///
/// # Errors
/// Any [`AbcError`]; file-level errors (not found, encoding, size, I/O) carry the path. A
/// path that is not a regular file (a FIFO, a device, a folder) gives
/// [`AbcErrorKind::Io`]`(`[`ErrorKind::InvalidInput`]`)`.
pub fn read_song(
    music_dir: &Path,
    name: &str,
    request: PartRequest,
) -> Result<Performance, AbcError> {
    let path = find_song(music_dir, name)?;
    let bytes = read_bounded(&path, MAX_FILE_BYTES)?;
    tracing::debug!(path = %path.display(), bytes = bytes.len(), ?request, "reading song");
    parse(&bytes, request).map_err(|mut e| {
        if matches!(
            e.kind,
            AbcErrorKind::EncodingNotSupported | AbcErrorKind::FileNotFound
        ) {
            e.path = Some(path.clone());
        }
        e
    })
}

/// One part found by [`list_songs`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartInfo {
    /// The X: value, converted as a request finds it (C base detection, §5.3 step 5;
    /// saturated to the `i32` range like a request).
    pub number: i32,
    /// The first non-empty T: after the X: line and before the next one, trimmed and cut to
    /// [`LISTING_TITLE_MAX_CHARS`] characters.
    pub title: Option<String>,
}

impl PartInfo {
    /// The request that selects this part (§5.3).
    pub fn request(&self) -> PartRequest {
        PartRequest::Number(self.number)
    }
}

/// One song file found by [`list_songs`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongListing {
    /// File name with extension.
    pub file_name: String,
    /// The name to pass to `/play` ([`find_song`]): the file name without its extension,
    /// or the whole file name when that stem itself contains a `.` (the lookup replaces
    /// the last extension, §3.1). It finds this file unless a file with the same stem and
    /// the extension `.abc` shadows a `.txt` one (§3.1 order), or the file name is not
    /// valid Unicode.
    pub name: String,
    /// Full path.
    pub path: PathBuf,
    /// The first non-empty T: before the first X: line, if any, trimmed and cut to
    /// [`LISTING_TITLE_MAX_CHARS`] characters.
    pub file_title: Option<String>,
    /// X: lines with their titles, in file order (at most [`MAX_LISTED_PARTS`]).
    pub parts: Vec<PartInfo>,
    /// `false` if the file could not be read in full (too large, unreadable, not a regular
    /// file, a byte-order mark the reader refuses), had more parts than are listed, or was
    /// not read because the call's budget ([`MAX_LISTING_BYTES`],
    /// [`MAX_LISTED_PARTS_TOTAL`]) was used up.
    pub complete: bool,
}

/// Lists the songs of the music folder for `/playlist` (§11): files whose extension is
/// `.abc` or `.txt` in any letter case, sorted by file name, each with its X: numbers and
/// the first T: after each. Sub-folders are not entered (§11 OPEN 2).
///
/// Reads only; at most [`MAX_LISTED_FILES`] files of at most [`MAX_FILE_BYTES`] each, at
/// most [`MAX_LISTING_BYTES`] and [`MAX_LISTED_PARTS_TOTAL`] parts in all. Blocking file
/// I/O: not for the frame or audio thread.
///
/// # Errors
/// [`AbcErrorKind::Io`] if the folder cannot be read.
pub fn list_songs(music_dir: &Path) -> Result<Vec<SongListing>, AbcError> {
    let io = |e: std::io::Error| AbcError {
        kind: AbcErrorKind::Io(e.kind()),
        position: None,
        path: Some(music_dir.to_path_buf()),
    };
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(music_dir).map_err(io)? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let ext_ok = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("abc") || e.eq_ignore_ascii_case("txt"));
        if !ext_ok || !path.is_file() {
            continue;
        }
        found.push((entry.file_name().to_string_lossy().into_owned(), path));
    }
    found.sort();
    found.truncate(MAX_LISTED_FILES);
    let mut bytes_left = MAX_LISTING_BYTES;
    let mut parts_left = MAX_LISTED_PARTS_TOTAL;
    Ok(found
        .into_iter()
        .map(|(file_name, path)| {
            let mut listing = SongListing {
                name: play_name(&path),
                file_name,
                path: path.clone(),
                file_title: None,
                parts: Vec::new(),
                complete: false,
            };
            if parts_left > 0
                && let Ok(bytes) = read_bounded(&path, MAX_FILE_BYTES.min(bytes_left))
            {
                bytes_left = bytes_left.saturating_sub(bytes.len() as u64);
                scan_listing(&bytes, &mut listing, MAX_LISTED_PARTS.min(parts_left));
                parts_left = parts_left.saturating_sub(listing.parts.len());
            }
            listing
        })
        .collect())
}

/// The name that [`find_song`] resolves to `path` (see [`SongListing::name`]).
fn play_name(path: &Path) -> String {
    let lossy = |s: &OsStr| s.to_string_lossy().into_owned();
    match path.file_stem() {
        Some(stem) if !stem.as_encoded_bytes().contains(&b'.') => lossy(stem),
        _ => path.file_name().map(lossy).unwrap_or_default(),
    }
}

/// Scans a file for X: and T: lines (letter + colon at the start of a line, after optional
/// white space; either letter case), listing at most `max_parts` parts.
fn scan_listing(bytes: &[u8], out: &mut SongListing, max_parts: usize) {
    let mut chars = match detect_bom(bytes) {
        Bom::Plain => CharStream::new(bytes, false),
        Bom::Utf8 => CharStream::new(bytes.get(3..).unwrap_or_default(), true),
        Bom::Utf16 | Bom::Unsupported => return,
    };
    let mut complete = true;
    loop {
        chars.skip_spaces();
        let c = match chars.next_char().0 {
            Ch::End => break,
            Ch::Break => continue,
            Ch::Char(c) => c,
        };
        if !(is_letter(c) && chars.peek().0 == Ch::Char(':')) {
            chars.skip_line();
            continue;
        }
        chars.next_char();
        match c {
            'X' | 'x' => {
                let v = chars.line_strtol();
                let number = i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX });
                if out.parts.len() < max_parts {
                    out.parts.push(PartInfo {
                        number,
                        title: None,
                    });
                } else {
                    complete = false;
                }
            }
            'T' | 't' => {
                let text = line_text(&mut chars);
                let slot = match out.parts.last_mut() {
                    Some(p) => &mut p.title,
                    None => &mut out.file_title,
                };
                if slot.is_none() && !text.is_empty() {
                    *slot = Some(text);
                }
            }
            _ => chars.skip_line(),
        }
    }
    out.complete = complete;
}

/// The rest of the line (taking the line break), trimmed of spaces, tabs and form feeds,
/// at most [`LISTING_TITLE_MAX_CHARS`] characters.
fn line_text(chars: &mut CharStream<'_>) -> String {
    let mut text = String::new();
    let mut kept = 0usize;
    while let Ch::Char(c) = chars.next_char().0 {
        if text.is_empty() && matches!(c, ' ' | '\t' | '\u{0C}') {
            continue;
        }
        if kept < LISTING_TITLE_MAX_CHARS {
            text.push(c);
            kept += 1;
        }
    }
    text.trim_end_matches([' ', '\t', '\u{0C}']).to_owned()
}
