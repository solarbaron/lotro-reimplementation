// SPDX-License-Identifier: GPL-3.0-or-later
//! The acceptance record (`official-entry.md` §5.6): its two files, the strict check and the
//! atomic writer.
//!
//! The items that could build a valid record without the warning (the file names, the
//! authentication code, the document renderer, the pure evaluation, the record values) are
//! not part of the crate's API: they are reachable only from inside the crate, and for
//! fuzzing and other crates' tests through `consent::test_support` (feature `test-support`,
//! never enabled in release builds).

use std::fs::{File, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use super::binding::Binding;
use super::crypto::{Items, ct_eq, hex, hmac_sha256, parse_hex32};
use super::{Digest, NotAcceptedReason};

/// The record document's file name (§5.6).
pub(crate) const RECORD_FILE: &str = "official-consent.toml";
/// The key file's name (§5.6).
pub(crate) const KEY_FILE: &str = "official-consent.key";
/// Largest record document in bytes (§5.6).
pub const MAX_RECORD_BYTES: u64 = 4096;
/// Exact size of the key file in bytes (§5.6).
pub const KEY_BYTES: usize = 32;
/// The record's `schema` value (§5.6).
pub const SCHEMA: i64 = 1;
/// Largest `revision` value (§5.6).
pub const MAX_REVISION: i64 = 1_000_000;

/// The ASCII text that starts the authentication code's byte string (§5.6).
const MAC_LABEL: &[u8] = b"tessera-official-consent-record-v1";

/// The six keys of a record document, in the order Tessera writes them.
const KEYS: [&str; 6] = [
    "schema",
    "revision",
    "language",
    "text_sha256",
    "accepted_at",
    "mac",
];

/// The five authenticated values of a record document (§5.6), as written in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordValues {
    /// `schema` (1).
    pub(crate) schema: i64,
    /// `revision`, 1–1,000,000.
    pub(crate) revision: i64,
    /// `language`, 2–16 ASCII letters, digits or `-`.
    pub(crate) language: String,
    /// `text_sha256`, 64 lowercase hexadecimal digits.
    pub(crate) text_sha256: String,
    /// `accepted_at`, an RFC 3339 date-time in UTC.
    pub(crate) accepted_at: String,
}

/// Whether `s` is a valid `language` value (§5.6).
#[must_use]
pub fn is_valid_language(s: &str) -> bool {
    (2..=16).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

impl RecordValues {
    /// Record values from their parts (tests and fuzzing).
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn new(
        schema: i64,
        revision: i64,
        language: &str,
        text_sha256: &str,
        accepted_at: &str,
    ) -> Self {
        Self {
            schema,
            revision,
            language: language.to_owned(),
            text_sha256: text_sha256.to_owned(),
            accepted_at: accepted_at.to_owned(),
        }
    }

    /// `text_sha256` (tests and fuzzing).
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn text_sha256(&self) -> &str {
        &self.text_sha256
    }

    /// Whether every value is in range (§5.6 table).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.schema == SCHEMA
            && (1..=MAX_REVISION).contains(&self.revision)
            && is_valid_language(&self.language)
            && parse_hex32(&self.text_sha256).is_some()
            && super::time::is_valid_utc(&self.accepted_at)
    }
}

/// The authentication code of §5.6 over `values` and `binding` under `key`, as 64 lowercase
/// hexadecimal digits. Computing it accepts nothing: only the check of a record on disk can
/// (tests and fuzzing use it to build records; §5.8 says why a same-user program could
/// always do the same).
#[must_use]
pub(crate) fn record_mac(
    key: &[u8; KEY_BYTES],
    values: &RecordValues,
    binding: &Binding,
) -> String {
    hex(&mac_bytes(key, values, binding))
}

fn mac_bytes(key: &[u8; KEY_BYTES], values: &RecordValues, binding: &Binding) -> [u8; 32] {
    let mut items = Items::new();
    items
        .push(MAC_LABEL)
        .push(values.schema.to_string().as_bytes())
        .push(values.revision.to_string().as_bytes())
        .push(values.language.as_bytes())
        .push(values.text_sha256.as_bytes())
        .push(values.accepted_at.as_bytes())
        .push(binding.user())
        .push(binding.machine())
        .push(binding.directory());
    hmac_sha256(key, items.bytes())
}

/// The record document for `values` with its code: the exact text Tessera writes, and the
/// only text the check accepts (byte for byte).
#[must_use]
pub(crate) fn render_document(values: &RecordValues, mac: &str) -> String {
    format!(
        "schema = {}\nrevision = {}\nlanguage = \"{}\"\ntext_sha256 = \"{}\"\naccepted_at = \"{}\"\nmac = \"{}\"\n",
        values.schema,
        values.revision,
        values.language,
        values.text_sha256,
        values.accepted_at,
        mac
    )
}

/// Parses a record document strictly: exactly the six keys, each of its type and range.
fn parse_document(text: &str) -> Result<(RecordValues, [u8; 32]), NotAcceptedReason> {
    // No value of a valid document holds a bracket or brace, so tables, arrays and inline
    // tables are refused before the parser sees them (no nesting to recurse into).
    if text.contains(['[', ']', '{', '}']) {
        return Err(NotAcceptedReason::Invalid);
    }
    let table: toml::Table = toml::from_str(text).map_err(|_| NotAcceptedReason::Invalid)?;
    if table.len() != KEYS.len() || !KEYS.iter().all(|k| table.contains_key(*k)) {
        return Err(NotAcceptedReason::Invalid);
    }
    let int = |k: &str| table.get(k).and_then(toml::Value::as_integer);
    let string = |k: &str| {
        table
            .get(k)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
    };
    let (Some(schema), Some(revision), Some(language), Some(text_sha256), Some(accepted_at)) = (
        int("schema"),
        int("revision"),
        string("language"),
        string("text_sha256"),
        string("accepted_at"),
    ) else {
        return Err(NotAcceptedReason::Invalid);
    };
    let values = RecordValues {
        schema,
        revision,
        language,
        text_sha256,
        accepted_at,
    };
    let mac = string("mac")
        .as_deref()
        .and_then(parse_hex32)
        .ok_or(NotAcceptedReason::Invalid)?;
    if !values.is_valid() {
        return Err(NotAcceptedReason::Invalid);
    }
    Ok((values, mac))
}

/// Evaluates a record document and key against a binding and the current warning digest
/// (§5.6 "Check", after the files were read): parse strictly, compare the code in constant
/// time, then the digest. Pure; used by the check and by fuzzing.
///
/// # Errors
///
/// The reason the record is not accepted.
pub(crate) fn evaluate(
    record: &[u8],
    key: &[u8],
    binding: &Binding,
    digest: &Digest,
) -> Result<RecordValues, NotAcceptedReason> {
    if record.len() as u64 > MAX_RECORD_BYTES {
        return Err(NotAcceptedReason::Invalid);
    }
    let text = std::str::from_utf8(record).map_err(|_| NotAcceptedReason::Invalid)?;
    let (values, mac) = parse_document(text)?;
    // The code covers the values as written: only the document Tessera writes for them is
    // accepted, so no other spelling (`0x1`, `+1`, other quotes, escapes, comments, spacing,
    // line ends) can carry the same code.
    if render_document(&values, &hex(&mac)).as_bytes() != record {
        return Err(NotAcceptedReason::Invalid);
    }
    let key: &[u8; KEY_BYTES] = key
        .try_into()
        .map_err(|_| NotAcceptedReason::WrongBindingOrCode)?;
    if !ct_eq(&mac_bytes(key, &values, binding), &mac) {
        return Err(NotAcceptedReason::WrongBindingOrCode);
    }
    if values.text_sha256 != digest.hex() {
        return Err(NotAcceptedReason::TextChanged);
    }
    Ok(values)
}

/// What went wrong reading one of the two files.
enum ReadFail {
    Missing,
    /// A link, not a regular file, permissions or owner refused, or an I/O error.
    Unreadable,
    TooLarge,
}

/// Opens `path` for reading without following a final symbolic link and without blocking on
/// a FIFO.
fn open_no_follow(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT: open a link itself, never its target (it is then not
        // a regular file and is refused).
        opts.custom_flags(0x0020_0000);
    }
    opts.open(path)
}

/// Unix: refuses a file that group or others can write, or that another user owns (§5.6).
#[cfg(unix)]
fn permissions_ok(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    meta.mode() & 0o022 == 0 && meta.uid() == rustix::process::getuid().as_raw()
}

/// Windows: no permission or owner check in this revision (§5.6, §5.8).
#[cfg(not(unix))]
fn permissions_ok(_meta: &std::fs::Metadata) -> bool {
    true
}

/// Reads one file of the record, up to `limit` bytes plus one.
fn read_file(path: &Path, limit: u64) -> Result<Vec<u8>, ReadFail> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(ReadFail::Missing),
        Err(_) => return Err(ReadFail::Unreadable),
        Ok(m) if !m.file_type().is_file() => return Err(ReadFail::Unreadable),
        Ok(_) => {}
    }
    let file = match open_no_follow(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(ReadFail::Missing),
        Err(_) => return Err(ReadFail::Unreadable),
    };
    let meta = file.metadata().map_err(|_| ReadFail::Unreadable)?;
    if !meta.is_file() || !permissions_ok(&meta) {
        return Err(ReadFail::Unreadable);
    }
    if meta.len() > limit {
        return Err(ReadFail::TooLarge);
    }
    let mut buf = Vec::with_capacity(usize::try_from(meta.len().min(limit)).unwrap_or(0));
    file.take(limit + 1)
        .read_to_end(&mut buf)
        .map_err(|_| ReadFail::Unreadable)?;
    if buf.len() as u64 > limit {
        return Err(ReadFail::TooLarge);
    }
    Ok(buf)
}

/// Reads the record document and the key file of `dir` (§5.6 "Check", first part).
pub(crate) fn read_record_files(dir: &Path) -> Result<(Vec<u8>, Vec<u8>), NotAcceptedReason> {
    let record = match read_file(&dir.join(RECORD_FILE), MAX_RECORD_BYTES) {
        Ok(r) => r,
        Err(ReadFail::Missing) => return Err(NotAcceptedReason::Missing),
        Err(ReadFail::Unreadable) => return Err(NotAcceptedReason::Unreadable),
        Err(ReadFail::TooLarge) => return Err(NotAcceptedReason::Invalid),
    };
    let key = match read_file(&dir.join(KEY_FILE), KEY_BYTES as u64) {
        Ok(k) => k,
        // A record whose key is gone or of another size cannot be authenticated.
        Err(ReadFail::Missing | ReadFail::TooLarge) => {
            return Err(NotAcceptedReason::WrongBindingOrCode);
        }
        Err(ReadFail::Unreadable) => return Err(NotAcceptedReason::Unreadable),
    };
    Ok((record, key))
}

/// Why a record could not be written (§5.5: the acceptance then counts for the run only).
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// No binding could be made (the directory, the OS account or the machine).
    #[error("no binding: {0}")]
    Binding(#[from] super::binding::BindingError),
    /// The key file exists but is not a valid key (another size, a link, not a regular
    /// file, permissions or owner refused); it is never replaced.
    #[error("the key file exists but is not a valid key")]
    KeyFile,
    /// No record directory (the platform names no config directory).
    #[error("no record directory")]
    NoDirectory,
    /// This platform stores no record (Windows, §5.6 "Windows").
    #[error("no record is stored on this system")]
    NotStored,
    /// The text is not the shipped warning text (only tests write records for other texts).
    #[error("the text is not the shipped warning text")]
    NotShipped,
    /// The operating system's random source failed.
    #[error("no random key")]
    Random,
    /// The writer thread could not be started or ended without an answer.
    #[error("the writer did not run")]
    Worker,
    /// An I/O error.
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// Creates `dir` (owner-only on Unix-like systems) if it does not exist.
fn ensure_dir(dir: &Path) -> io::Result<()> {
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(dir)
}

/// Options for a new file: write, create only if absent, owner-only (0600) on Unix-like
/// systems, never through a final symbolic link.
fn new_file_options() -> OpenOptions {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    opts
}

/// A temporary file name in the record directory, unique to this process and moment.
fn temp_path(dir: &Path, base: &str) -> PathBuf {
    let mut r = [0u8; 8];
    let tag = match getrandom::fill(&mut r) {
        Ok(()) => u64::from_le_bytes(r),
        Err(_) => std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64),
    };
    dir.join(format!(".{base}.{tag:016x}.tmp"))
}

/// Writes `bytes` to a new temporary file, flushes it and renames it over `target`.
fn write_atomic(dir: &Path, target: &str, bytes: &[u8]) -> Result<(), WriteError> {
    let tmp = temp_path(dir, target);
    let result = (|| -> Result<(), WriteError> {
        let mut f = new_file_options().open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, dir.join(target))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// The key of `dir`, made only if it does not exist (§5.6 "Write" step 1, invariant 11): a
/// new key goes to a temporary file that is then hard-linked to the key file's name, a rename
/// that never replaces a file that appeared meanwhile. If one did (another Tessera process won
/// the race), it is used when it passes the checks, else the write fails. An existing key file
/// that is not a valid key (another size, a link, not a regular file, permissions or owner
/// refused) is never overwritten or deleted: the write fails closed (the acceptance counts for
/// the run only and the warning is shown again next run), and the log names the file and the
/// reason, never its content. File systems without hard links (FAT, some network file systems)
/// fail the same way: the acceptance is not saved there.
fn load_or_make_key(dir: &Path) -> Result<[u8; KEY_BYTES], WriteError> {
    load_or_make_key_with(dir, || {})
}

/// [`load_or_make_key`] with a hook run between the existence check and the no-replace rename
/// (tests inject a racing key file there, OE-T16).
fn load_or_make_key_with(
    dir: &Path,
    before_link: impl FnOnce(),
) -> Result<[u8; KEY_BYTES], WriteError> {
    let path = dir.join(KEY_FILE);
    let existing = |path: &Path| {
        let why = match read_file(path, KEY_BYTES as u64) {
            Ok(k) => match <[u8; KEY_BYTES]>::try_from(k.as_slice()) {
                Ok(key) => return Ok(key),
                Err(_) => "not 32 bytes",
            },
            Err(ReadFail::TooLarge) => "not 32 bytes",
            Err(ReadFail::Missing) => "missing",
            Err(ReadFail::Unreadable) => {
                "a link, not a regular file, or permissions or owner refused"
            }
        };
        tracing::warn!(
            file = %path.display(),
            reason = why,
            "official-server warning: the key file is not used and not replaced"
        );
        Err(WriteError::KeyFile)
    };
    match std::fs::symlink_metadata(&path) {
        Ok(_) => return existing(&path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let mut key = [0u8; KEY_BYTES];
    getrandom::fill(&mut key).map_err(|_| WriteError::Random)?;
    let tmp = temp_path(dir, KEY_FILE);
    let made = (|| -> Result<(), WriteError> {
        let mut f = new_file_options().open(&tmp)?;
        f.write_all(&key)?;
        f.sync_all()?;
        drop(f);
        before_link();
        std::fs::hard_link(&tmp, &path)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&tmp);
    match made {
        Ok(()) => Ok(key),
        // Another process made it meanwhile: use it if it is a valid key.
        Err(WriteError::Io(e)) if e.kind() == io::ErrorKind::AlreadyExists => existing(&path),
        Err(e) => {
            tracing::warn!(
                file = %path.display(),
                "official-server warning: no key file could be made ({e})"
            );
            Err(e)
        }
    }
}

/// Makes the key of `dir` with a racing hook (tests, OE-T16).
#[cfg(test)]
pub(crate) fn make_key_racing(
    dir: &Path,
    before_link: impl FnOnce(),
) -> Result<[u8; KEY_BYTES], WriteError> {
    load_or_make_key_with(dir, before_link)
}

/// Writes a record for `values` bound to `binding` in `dir` (the writer's work; the binding is
/// the current one in the client, injected in tests).
pub(crate) fn write_record(
    dir: &Path,
    values: &RecordValues,
    binding_of: impl FnOnce(&Path) -> Result<Binding, super::binding::BindingError>,
) -> Result<(), WriteError> {
    ensure_dir(dir)?;
    let binding = binding_of(dir)?;
    let key = load_or_make_key(dir)?;
    let mac = record_mac(&key, values, &binding);
    write_atomic(dir, RECORD_FILE, render_document(values, &mac).as_bytes())?;
    #[cfg(unix)]
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// What [`withdraw_report`] did with each file of the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WithdrawReport {
    /// The record document is gone (deleted now, or it did not exist).
    pub record_removed: bool,
    /// The key file is gone (deleted now, or it did not exist).
    pub key_removed: bool,
}

impl WithdrawReport {
    /// Both files are gone.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.record_removed && self.key_removed
    }
}

/// [`withdraw`], reporting each file instead of the first error (the flow recomputes the
/// start screen's line after a partial deletion).
#[must_use]
pub fn withdraw_report(dir: &Path) -> WithdrawReport {
    WithdrawReport {
        record_removed: remove_entry(&dir.join(RECORD_FILE)).is_ok(),
        key_removed: remove_entry(&dir.join(KEY_FILE)).is_ok(),
    }
}

/// Removes one directory entry of the record (module docs of [`withdraw`]); "not found" is
/// success; a failure is logged with the file's name.
fn remove_entry(path: &Path) -> io::Result<()> {
    let removed = match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => std::fs::remove_dir(path),
        Ok(_) => std::fs::remove_file(path),
        Err(e) => Err(e),
    };
    match removed {
        Err(e) if e.kind() != io::ErrorKind::NotFound => {
            tracing::warn!(
                file = %path.display(),
                "official-server warning: withdrawing could not delete the file ({e})"
            );
            Err(e)
        }
        _ => Ok(()),
    }
}

/// Deletes both files of the record in `dir` (§5.5 "Withdrawing"), whatever their state (an
/// invalid key file included, §5.6 "Key" step 1): the directory entries themselves are removed,
/// never the target of a link; an empty directory in a file's place is removed too, a directory
/// with content is left alone. Files that do not exist are not an error. A file that cannot be
/// deleted (another owner, a read-only directory) is named in the log.
///
/// # Errors
///
/// The first I/O error other than "not found" (both deletions are attempted).
pub fn withdraw(dir: &Path) -> io::Result<()> {
    let mut first = None;
    for name in [RECORD_FILE, KEY_FILE] {
        if let Err(e) = remove_entry(&dir.join(name)) {
            first.get_or_insert(e);
        }
    }
    match first {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
