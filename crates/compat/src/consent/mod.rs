// SPDX-License-Identifier: GPL-3.0-or-later
//! The ban-risk consent of official play (`docs/specs/official-entry.md` §5 and §6.1,
//! decision 0034 §1 item 4).
//!
//! This module has no network code and is independent of the crate's protocol modules. It
//! owns:
//!
//! - the warning's string entries and Tessera's other texts for official play
//!   ([`strings`], §5.2), and the shipped [`WarningText`];
//! - the **warning digest** ([`WarningText::digest`], §5.3);
//! - the **acceptance record** (§5.6): the strict check ([`check`], [`spawn_check`]), the
//!   writer ([`accept`]) and withdrawing ([`withdraw`]);
//! - the **consent proof** ([`ConsentProof`], §6.1): made only from an [`Acceptance`], which
//!   only a check that gave accepted or the writer after a confirmation on the warning
//!   screen ([`UserConfirmation`], §6.3) produce. Neither type can be cloned, copied,
//!   defaulted, serialised or built from parts outside this module; there is no public
//!   constructor, test builds included.
//!
//! The record's location is derived from the platform's directory conventions only
//! ([`default_record_dir`]); no code here reads an environment variable itself.
//!
//! **Windows stores no record** (§5.6 "Windows"). The record's binding needs the user's
//! security identifier, which this crate cannot read without unsafe code or a new dependency;
//! on Windows the check gives not accepted (`not_stored`) without opening any file, the writer
//! writes neither file ([`WriteError::NotStored`]), an acceptance counts for the run only, and
//! the client shows `official.consent.not_stored`; the warning is shown on every run.
//!
//! **File systems without hard links** (FAT, some network file systems) cannot take a new key
//! file (it is linked into place so that it never replaces another): the write fails closed and
//! the acceptance counts for the run only (`official.consent.saved_failed`).

mod binding;
mod crypto;
mod record;
pub mod strings;
pub mod time;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use unicode_normalization::UnicodeNormalization as _;

use binding::Binding;
pub use binding::BindingError;
use record::RecordValues;
pub use record::{
    KEY_BYTES, MAX_RECORD_BYTES, MAX_REVISION, SCHEMA, WithdrawReport, WriteError,
    is_valid_language, withdraw, withdraw_report,
};

/// The ASCII text that starts the digest's byte string (§5.3 item 1).
const DIGEST_LABEL: &[u8] = b"tessera-official-consent-v1";

/// Whether this platform stores acceptance records: not on Windows (§5.6 "Windows").
pub const STORES_RECORDS: bool = !cfg!(windows);

/// The longest a record check may take before it gives not accepted (§5.6).
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(1);

/// A warning digest (§5.3): SHA-256 of the shown text's byte string.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Digest([u8; 32]);

impl Digest {
    /// The digest as 64 lowercase hexadecimal digits (the record's `text_sha256`).
    #[must_use]
    pub fn hex(&self) -> String {
        crypto::hex(&self.0)
    }

    /// The digest's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Digest({})", self.hex())
    }
}

/// The shown warning text (§5.2): one language's entry set and its revision. The client
/// shows [`WarningText::shipped`]; tests build other sets (OE-T3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarningText {
    /// The language tag of the entry set (§5.3 item 2).
    pub language: String,
    /// The warning revision (§5.2).
    pub revision: u32,
    /// `official.consent.title`.
    pub title: String,
    /// `official.consent.body` (paragraphs separated by two line feeds).
    pub body: String,
    /// `official.consent.checkbox`.
    pub checkbox: String,
    /// `official.consent.accept`.
    pub accept: String,
    /// `official.consent.decline`.
    pub decline: String,
}

impl WarningText {
    /// The text compiled into this binary (§5.2), English, revision
    /// [`strings::WARNING_REVISION`].
    #[must_use]
    pub fn shipped() -> Self {
        Self {
            language: strings::LANGUAGE.to_owned(),
            revision: strings::WARNING_REVISION,
            title: strings::text("official.consent.title").to_owned(),
            body: strings::text("official.consent.body").to_owned(),
            checkbox: strings::text("official.consent.checkbox").to_owned(),
            accept: strings::text("official.consent.accept").to_owned(),
            decline: strings::text("official.consent.decline").to_owned(),
        }
    }

    /// The warning digest of this text (§5.3): SHA-256 over the length-prefixed items
    /// `tessera-official-consent-v1`, the language tag, the revision in decimal, then the
    /// title, body, check-box label, accept and decline labels, each in Unicode
    /// normalisation form C.
    #[must_use]
    pub fn digest(&self) -> Digest {
        let nfc = |s: &str| s.nfc().collect::<String>();
        let mut items = crypto::Items::new();
        items
            .push(DIGEST_LABEL)
            .push(self.language.as_bytes())
            .push(self.revision.to_string().as_bytes())
            .push(nfc(&self.title).as_bytes())
            .push(nfc(&self.body).as_bytes())
            .push(nfc(&self.checkbox).as_bytes())
            .push(nfc(&self.accept).as_bytes())
            .push(nfc(&self.decline).as_bytes());
        Digest(crypto::sha256(items.bytes()))
    }
}

/// The digest of the shipped warning text: the one every consent proof must carry for the
/// compatibility backend to be built (§6.1).
#[must_use]
pub fn current_digest() -> Digest {
    WarningText::shipped().digest()
}

/// Tessera's per-user config directory for the record (§5.6): the platform's configuration
/// directory followed by `tessera` (on Linux `$XDG_CONFIG_HOME/tessera`). `None` when the
/// platform names none. Never taken from an option, the settings file or the install.
#[must_use]
pub fn default_record_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("tessera"))
}

/// Why a record was not accepted (§5.6; the names are the log's reasons).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotAcceptedReason {
    /// No record document.
    Missing,
    /// A file is a link, not a regular file, writable by others, owned by another user, could
    /// not be read in time, or no binding could be made.
    Unreadable,
    /// The document is too large, not UTF-8, or does not parse strictly.
    Invalid,
    /// The authentication code does not match (another binding, a changed document, another
    /// key, or no key of the right size).
    WrongBindingOrCode,
    /// The record is valid but for another warning digest: the screen shows
    /// `official.consent.changed`.
    TextChanged,
    /// This platform stores no record (Windows, §5.6 "Windows"): nothing was opened.
    NotStored,
}

impl NotAcceptedReason {
    /// The log name of the reason (§5.6).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unreadable => "unreadable",
            Self::Invalid => "invalid",
            Self::WrongBindingOrCode => "wrong_binding_or_code",
            Self::TextChanged => "text_changed",
            Self::NotStored => "not_stored",
        }
    }
}

/// A valid acceptance for one warning digest: from a record that passed every check, or from
/// a confirmation on the warning screen in this run (§5.5, §9). The consent module's only
/// source of [`ConsentProof`]s. No `Clone`, no public constructor.
#[derive(Debug)]
pub struct Acceptance {
    digest: Digest,
    accepted_at: String,
}

impl Acceptance {
    /// A new consent proof for this acceptance (one per backend constructed, §9).
    #[must_use]
    pub fn proof(&self) -> ConsentProof {
        ConsentProof {
            digest: self.digest,
        }
    }

    /// The digest accepted.
    #[must_use]
    pub fn digest(&self) -> Digest {
        self.digest
    }

    /// When the user activated Continue (RFC 3339, UTC).
    #[must_use]
    pub fn accepted_at(&self) -> &str {
        &self.accepted_at
    }

    /// The date part (`YYYY-MM-DD`) of [`Acceptance::accepted_at`], for the start screen.
    #[must_use]
    pub fn accepted_date(&self) -> &str {
        time::date_part(&self.accepted_at)
    }
}

/// The consent proof (§6.1): an in-memory value without which no official network
/// operation exists. It carries the warning digest it was made for.
///
/// It cannot be cloned, defaulted or built outside this module. Each `compile_fail` example
/// below differs from the compiling one only in the step it is about (the error code is
/// checked by nightly rustdoc):
///
/// ```
/// use tessera_compat::consent::{ConsentProof, Digest, current_digest};
/// fn read(p: &ConsentProof) -> Digest {
///     let _ = current_digest();
///     p.digest()
/// }
/// ```
///
/// No copy (`ConsentProof: Clone` is not satisfied):
///
/// ```compile_fail,E0277
/// use tessera_compat::consent::ConsentProof;
/// fn copy(p: &ConsentProof) -> ConsentProof {
///     Clone::clone(p)
/// }
/// ```
///
/// No default value (`ConsentProof: Default` is not satisfied):
///
/// ```compile_fail,E0277
/// use tessera_compat::consent::ConsentProof;
/// fn made() -> ConsentProof {
///     Default::default()
/// }
/// ```
///
/// No construction from parts (the field is private):
///
/// ```compile_fail,E0451
/// use tessera_compat::consent::{ConsentProof, current_digest};
/// fn made() -> ConsentProof {
///     ConsentProof { digest: current_digest() }
/// }
/// ```
pub struct ConsentProof {
    digest: Digest,
}

impl ConsentProof {
    /// The warning digest the proof was made for.
    #[must_use]
    pub fn digest(&self) -> Digest {
        self.digest
    }

    /// Whether the proof is for the shipped warning text.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.digest == current_digest()
    }
}

impl std::fmt::Debug for ConsentProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConsentProof(..)")
    }
}

/// The user's confirmation on the warning screen (§6.3): the acceptance writer takes it. It is
/// created only in the warning screen's Continue handler, from the device-input event that
/// activated Continue with the box ticked; a source-scan test of the client (OE-T6) fails if
/// it is created anywhere else outside test code.
#[derive(Debug)]
pub struct UserConfirmation {
    _private: (),
}

impl UserConfirmation {
    /// The confirmation of the Continue handler (see the type's documentation).
    #[must_use]
    pub fn confirmed_by_user() -> Self {
        Self { _private: () }
    }
}

/// The outcome of a record check (§5.6).
#[derive(Debug)]
pub enum CheckOutcome {
    /// Every check passed.
    Accepted(Acceptance),
    /// The warning is shown (§5.1).
    NotAccepted(NotAcceptedReason),
}

impl CheckOutcome {
    /// Whether the outcome is [`CheckOutcome::Accepted`].
    #[must_use]
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted(_))
    }

    /// The reason of [`CheckOutcome::NotAccepted`].
    #[must_use]
    pub fn reason(&self) -> Option<NotAcceptedReason> {
        match self {
            Self::NotAccepted(r) => Some(*r),
            Self::Accepted(_) => None,
        }
    }
}

/// Reads and evaluates the record of `dir` with the binding `binding_of` gives.
fn check_inner(
    dir: &Path,
    text: &WarningText,
    binding_of: impl FnOnce(&Path) -> Result<Binding, BindingError>,
) -> Result<RecordValues, NotAcceptedReason> {
    let (doc, key) = record::read_record_files(dir)?;
    let binding = binding_of(dir).map_err(|_| NotAcceptedReason::Unreadable)?;
    record::evaluate(&doc, &key, &binding, &text.digest())
}

/// Checks the record in `dir` against `text` with this OS user's and machine's binding
/// (§5.6). Never repairs, rewrites or deletes a record it rejects. Runs on the caller's
/// thread; the client runs it on a worker with [`spawn_check`].
#[must_use]
pub fn check(dir: &Path, text: &WarningText) -> CheckOutcome {
    if !STORES_RECORDS {
        // §5.6 "Windows": no file is opened.
        let _ = (dir, text);
        tracing::info!(
            reason = "not_stored",
            "official-server warning: no record on this system"
        );
        return CheckOutcome::NotAccepted(NotAcceptedReason::NotStored);
    }
    match check_inner(dir, text, Binding::current) {
        Ok(values) => {
            tracing::info!(
                revision = values.revision,
                "official-server warning: acceptance record accepted"
            );
            CheckOutcome::Accepted(Acceptance {
                digest: text.digest(),
                accepted_at: values.accepted_at,
            })
        }
        Err(reason) => {
            tracing::info!(
                reason = reason.as_str(),
                "official-server warning: acceptance record not accepted"
            );
            CheckOutcome::NotAccepted(reason)
        }
    }
}

/// The verdict of a check with an injected binding (tests: OE-T5's records bound to another
/// user, machine or directory). It gives no acceptance.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn check_with_binding(
    dir: &Path,
    text: &WarningText,
    binding: &Binding,
) -> Result<RecordValues, NotAcceptedReason> {
    check_inner(dir, text, |_| Ok(binding.clone()))
}

/// A record check running on a worker thread (§9 step 2), bounded by [`CHECK_TIMEOUT`].
#[derive(Debug)]
pub struct PendingCheck {
    rx: Option<mpsc::Receiver<CheckOutcome>>,
    deadline: Instant,
}

/// Starts [`check`] of `dir` against `text` on a worker thread.
#[must_use]
pub fn spawn_check(dir: PathBuf, text: Arc<WarningText>) -> PendingCheck {
    let (tx, rx) = mpsc::sync_channel(1);
    let spawned = std::thread::Builder::new()
        .name("tessera-consent-check".into())
        .spawn(move || {
            let _ = tx.send(check(&dir, &text));
        });
    PendingCheck {
        rx: spawned.is_ok().then_some(rx),
        deadline: Instant::now() + CHECK_TIMEOUT,
    }
}

impl PendingCheck {
    /// The outcome once the check has finished; not accepted (`unreadable`) when the worker
    /// could not start or did not answer within [`CHECK_TIMEOUT`]. `None` while it runs.
    pub fn poll(&mut self) -> Option<CheckOutcome> {
        let Some(rx) = self.rx.as_ref() else {
            return Some(CheckOutcome::NotAccepted(NotAcceptedReason::Unreadable));
        };
        match rx.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(CheckOutcome::NotAccepted(NotAcceptedReason::Unreadable))
            }
            Err(mpsc::TryRecvError::Empty) if Instant::now() >= self.deadline => {
                tracing::info!("official-server warning: record check took longer than 1 s");
                Some(CheckOutcome::NotAccepted(NotAcceptedReason::Unreadable))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }

    /// Waits for the outcome, at most until the deadline.
    #[must_use]
    pub fn wait(mut self) -> CheckOutcome {
        let Some(rx) = self.rx.take() else {
            return CheckOutcome::NotAccepted(NotAcceptedReason::Unreadable);
        };
        let left = self.deadline.saturating_duration_since(Instant::now());
        rx.recv_timeout(left)
            .unwrap_or(CheckOutcome::NotAccepted(NotAcceptedReason::Unreadable))
    }
}

/// The record write started by [`accept`], on a worker thread (§5.5).
#[derive(Debug)]
pub struct PendingWrite {
    rx: Option<mpsc::Receiver<Result<(), WriteError>>>,
}

impl PendingWrite {
    /// The write's result once it has finished; `None` while it runs.
    pub fn poll(&mut self) -> Option<Result<(), WriteError>> {
        let Some(rx) = self.rx.as_ref() else {
            return Some(Err(WriteError::Worker));
        };
        match rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(WriteError::Worker)),
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }

    /// Waits for the write's result.
    pub fn wait(mut self) -> Result<(), WriteError> {
        match self.rx.take() {
            Some(rx) => rx.recv().unwrap_or(Err(WriteError::Worker)),
            None => Err(WriteError::Worker),
        }
    }
}

/// A withdrawal (§5.5 "Withdrawing") running on a consent worker thread, so the frame thread
/// neither waits for a record write nor deletes files (ARCH §21).
#[derive(Debug)]
pub struct PendingWithdraw {
    rx: Option<mpsc::Receiver<WithdrawReport>>,
}

/// Nothing removed: the worker could not start or ended without a report.
const NOTHING_REMOVED: WithdrawReport = WithdrawReport {
    record_removed: false,
    key_removed: false,
};

/// Starts a withdrawal of the record in `dir` on a worker thread (§5.5 "Withdrawing"). A
/// record write still running (`write`) finishes there first, so it cannot bring the record
/// back afterwards. Without a record directory nothing is stored and the report is complete.
///
/// The caller ends the run's acceptance when it starts the withdrawal, whatever the report
/// says, and counts no record for the rest of the run until the user accepts again (§5.5
/// "Withdraw always ends the run's acceptance", "Withdrawn for the run"). A report that is not
/// [`WithdrawReport::complete`] is a failed or partial withdrawal; calling this again retries
/// the deletion.
#[must_use]
pub fn spawn_withdraw(dir: Option<PathBuf>, write: Option<PendingWrite>) -> PendingWithdraw {
    let (tx, rx) = mpsc::sync_channel(1);
    let spawned = std::thread::Builder::new()
        .name("tessera-consent-withdraw".into())
        .spawn(move || {
            if let Some(w) = write {
                let _ = w.wait();
            }
            let report = match dir {
                Some(d) => withdraw_report(&d),
                None => WithdrawReport {
                    record_removed: true,
                    key_removed: true,
                },
            };
            let _ = tx.send(report);
        });
    PendingWithdraw {
        rx: spawned.is_ok().then_some(rx),
    }
}

impl PendingWithdraw {
    /// The report once the withdrawal has finished; `None` while it runs.
    pub fn poll(&mut self) -> Option<WithdrawReport> {
        let Some(rx) = self.rx.as_ref() else {
            return Some(NOTHING_REMOVED);
        };
        match rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Disconnected) => Some(NOTHING_REMOVED),
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }

    /// Waits for the report (tests, and the end of the run).
    #[must_use]
    pub fn wait(mut self) -> WithdrawReport {
        match self.rx.take() {
            Some(rx) => rx.recv().unwrap_or(NOTHING_REMOVED),
            None => NOTHING_REMOVED,
        }
    }
}

/// The acceptance writer (§5.5): the user activated Continue with the box ticked. The
/// acceptance holds at once (the flow goes on to §9 step 4); the record for `text`'s digest
/// is written to `dir` on a worker thread. If the write fails, the acceptance still counts
/// for this run (the caller shows `official.consent.saved_failed`). Without a record
/// directory nothing is written ([`WriteError::NoDirectory`]); outside test builds (and the
/// feature `test-support`) a record is written only for [`WarningText::shipped`]
/// ([`WriteError::NotShipped`]).
#[must_use]
pub fn accept(
    dir: Option<&Path>,
    text: &WarningText,
    confirmation: UserConfirmation,
) -> (Acceptance, PendingWrite) {
    let UserConfirmation { _private: () } = confirmation;
    let accepted_at = time::format_utc(SystemTime::now());
    let digest = text.digest();
    let acceptance = Acceptance {
        digest,
        accepted_at: accepted_at.clone(),
    };
    let refused = |e: WriteError| {
        tracing::warn!("official-server warning: acceptance not saved: {e}");
        let (tx, rx) = mpsc::sync_channel(1);
        let _ = tx.send(Err(e));
        PendingWrite { rx: Some(rx) }
    };
    if !STORES_RECORDS {
        return (acceptance, refused(WriteError::NotStored));
    }
    let Some(dir) = dir else {
        return (acceptance, refused(WriteError::NoDirectory));
    };
    let may_write = cfg!(any(test, feature = "test-support")) || *text == WarningText::shipped();
    if !may_write {
        return (acceptance, refused(WriteError::NotShipped));
    }
    let values = values_for(text, &accepted_at);
    let dir = dir.to_path_buf();
    let (tx, rx) = mpsc::sync_channel(1);
    let spawned = std::thread::Builder::new()
        .name("tessera-consent-write".into())
        .spawn(move || {
            let r = record::write_record(&dir, &values, Binding::current);
            match &r {
                Ok(()) => tracing::info!("official-server warning: acceptance saved"),
                Err(e) => tracing::warn!("official-server warning: acceptance not saved: {e}"),
            }
            let _ = tx.send(r);
        });
    (
        acceptance,
        PendingWrite {
            rx: spawned.is_ok().then_some(rx),
        },
    )
}

/// The record values for an acceptance of `text` at `accepted_at`.
fn values_for(text: &WarningText, accepted_at: &str) -> RecordValues {
    RecordValues {
        schema: SCHEMA,
        revision: i64::from(text.revision),
        language: text.language.clone(),
        text_sha256: text.digest().hex(),
        accepted_at: accepted_at.to_owned(),
    }
}

/// Writes, synchronously, the record a confirmation of `text` at `accepted_at` gives, bound
/// to `binding` instead of the current one (tests: records for another user, machine or
/// directory, OE-T5). It gives no acceptance.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn write_with_binding(
    dir: &Path,
    text: &WarningText,
    confirmation: UserConfirmation,
    accepted_at: &str,
    binding: &Binding,
) -> Result<(), WriteError> {
    let UserConfirmation { _private: () } = confirmation;
    record::write_record(dir, &values_for(text, accepted_at), |_| Ok(binding.clone()))
}

/// Record-building helpers for fuzzing and other crates' tests (feature `test-support`, which
/// release builds never enable): records for injected bindings, the authentication code, the
/// document renderer and the pure evaluation. None of them gives an [`Acceptance`] or a
/// [`ConsentProof`]; a record they build is accepted only by the check of a directory whose
/// binding it was made for (§5.8).
#[cfg(feature = "test-support")]
pub mod test_support {
    use std::path::Path;

    pub use super::binding::Binding;
    pub use super::record::RecordValues;
    use super::{NotAcceptedReason, UserConfirmation, WarningText, WriteError, record};

    /// The record document's file name (§5.6).
    pub const RECORD_FILE: &str = record::RECORD_FILE;
    /// The key file's name (§5.6).
    pub const KEY_FILE: &str = record::KEY_FILE;

    /// A binding from given values.
    #[must_use]
    pub fn binding(user: &[u8], machine: &[u8], directory: &[u8]) -> Binding {
        Binding::new(user, machine, directory)
    }

    /// The binding of this OS user and machine for `dir`.
    ///
    /// # Errors
    ///
    /// As the check.
    pub fn current_binding(dir: &Path) -> Result<Binding, super::BindingError> {
        Binding::current(dir)
    }

    /// The authentication code of §5.6, as 64 lowercase hexadecimal digits.
    #[must_use]
    pub fn record_mac(
        key: &[u8; super::KEY_BYTES],
        values: &RecordValues,
        binding: &Binding,
    ) -> String {
        record::record_mac(key, values, binding)
    }

    /// The document Tessera writes for `values` and `mac`.
    #[must_use]
    pub fn render_document(values: &RecordValues, mac: &str) -> String {
        record::render_document(values, mac)
    }

    /// The pure evaluation of a record document and key.
    ///
    /// # Errors
    ///
    /// The reason the record is not accepted.
    pub fn evaluate(
        doc: &[u8],
        key: &[u8],
        binding: &Binding,
        digest: &super::Digest,
    ) -> Result<RecordValues, NotAcceptedReason> {
        record::evaluate(doc, key, binding, digest)
    }

    /// The verdict of a check with an injected binding.
    ///
    /// # Errors
    ///
    /// The reason the record is not accepted.
    pub fn check_with_binding(
        dir: &Path,
        text: &WarningText,
        binding: &Binding,
    ) -> Result<RecordValues, NotAcceptedReason> {
        super::check_with_binding(dir, text, binding)
    }

    /// Writes the record of a confirmation of `text` at `accepted_at`, bound to `binding`.
    ///
    /// # Errors
    ///
    /// As the writer.
    pub fn write_with_binding(
        dir: &Path,
        text: &WarningText,
        confirmation: UserConfirmation,
        accepted_at: &str,
        binding: &Binding,
    ) -> Result<(), WriteError> {
        super::write_with_binding(dir, text, confirmation, accepted_at, binding)
    }
}

#[cfg(test)]
mod tests;
