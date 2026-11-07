// SPDX-License-Identifier: GPL-3.0-or-later
//! The initial exchange of the login connection (`official-session.md` §9.3.1–§9.3.8, decision
//! 0046): the server's `0xDD`, the client's install advertisement `0xEC`, the completion
//! exchange `0xF5`, the historical `0x12F` path and the bounded wait for the character list
//! `0x6C`. Sans I/O: the session driver ([`crate::connector::session_phase`]) sends what this
//! module builds, and the install's values reach it through
//! [`crate::connector::data::InstallData`].
//!
//! # What is whose
//!
//! `official-session.md` §9.3.7–§9.3.8 state the rules this module follows. Its descriptions of
//! how the original walks its archives (§9.3.4–§9.3.6) are not rules and nothing here comes from
//! them (decision 0046 item 4). Everything sent is the user's real data (decision 0046 item 2);
//! a rejected attempt is not retried with varied data.
//!
//! Every wait here is Tessera policy on the monotonic clock: 30 s from the first State 5 to `0xDD`,
//! from `0xEC`'s last fragment to the server's `0xF5` and from the client's `0xF5` to the list; on
//! the historical path 30 s for an answer to `0x12C` and for the list, and one absolute five-minute
//! limit for a queued login, never extended. Other traffic restarts none of them, and a complete
//! list cancels every one.
//!
//! # Hostile input
//!
//! Every count is checked against the bytes left before anything is read or allocated. A
//! message that fails is rejected whole: the transport has already taken its datagram (§9.3.8).

use std::collections::BTreeMap;

/// Server opcode `0xDD`: the trigger of the install advertisement (§9.3.2, §9.3.4).
pub const OPCODE_INSTALL_REQUEST: u32 = 0x0000_00DD;
/// Client opcode `0xEC`: the install advertisement (§9.3.2).
pub const OPCODE_INSTALL_ADVERTISEMENT: u32 = 0x0000_00EC;
/// Opcode `0xF5` of the completion exchange, in both directions (§9.3.3).
pub const OPCODE_COMPLETION: u32 = 0x0000_00F5;

/// The server's data of `0xF5` and the client's complete reply: one constant zero flags byte
/// (§9.3.3).
pub const COMPLETION_DATA: [u8; 1] = [0];

/// Bytes of the complete `0xDD` data before its counted array (§9.3.4).
pub const DD_FIXED_BYTES: usize = 13;
/// Tessera's cap on `0xDD`'s trailing count (§9.3.7: a resource policy, not an official limit).
pub const MAX_DD_TRAILING: usize = 4096;

/// The grouping remainder of decision 0046 item 3 (§9.3.7: an interim relation, not an
/// archive count and not the official algorithm).
pub const ADVERTISEMENT_ORDER_MODULUS: u64 = 23;

/// Most archives one advertisement lists (§9.3.7: a Tessera resource bound); more is refused
/// as unsupported, never cut.
pub const MAX_ADVERTISED_ARCHIVES: usize = 256;
/// Longest advertisement data (§9.3.7: a Tessera resource bound); more is refused as
/// unsupported, never cut.
pub const MAX_ADVERTISEMENT_BYTES: usize = 1024 * 1024;

/// Wait from the first State 5 to the first valid `0xDD` (§9.3.8: Tessera policy).
pub const DD_WAIT_SEC: f64 = 30.0;
/// Wait from `0xEC`'s last fragment to the server's `0xF5` (§9.3.8).
pub const COMPLETION_WAIT_SEC: f64 = 30.0;
/// Wait from the client's `0xF5` to a complete `0x6C` (§9.3.8).
pub const LIST_WAIT_SEC: f64 = 30.0;
/// Wait from an actual `0x12F` for the historical answer (§9.3.8).
pub const VERSION_PATH_WAIT_SEC: f64 = 30.0;
/// Wait from `0x12D` status `1` or a ready `0x130` to a complete `0x6C` (§9.3.8).
pub const HISTORICAL_LIST_WAIT_SEC: f64 = 30.0;
/// The one absolute limit of a queued login, from its first queued `0x130`; another `0x130` never
/// extends it (§9.3.8).
pub const QUEUE_WAIT_SEC: f64 = 300.0;

/// Most distinct opcodes the skip counter keeps; the others are counted together (§9.3.7:
/// Tessera policy).
pub const MAX_SKIP_KEYS: usize = 256;

/// Errors of the initial exchange's decoders and builder.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum InitialError {
    /// The data ends before a field.
    #[error("message truncated: {needed} bytes needed, {available} available")]
    Truncated {
        /// Bytes needed.
        needed: usize,
        /// Bytes there are.
        available: usize,
    },
    /// The shared flags byte is `0x02` or more (§9.3.7): no routed form is guessed.
    #[error("unsupported flags byte {0:#04x}")]
    UnsupportedFlags(u8),
    /// Flags `0x01` and the data does not inflate to exactly its stated length (§9.1, §10.1).
    #[error("the compressed data does not inflate to its stated length")]
    Decompression,
    /// The counted array cannot fit the remaining bytes, or passes [`MAX_DD_TRAILING`].
    #[error("trailing count {count} is out of bounds")]
    TrailingCount {
        /// The count on the wire.
        count: u32,
    },
    /// Bytes follow the counted array (§9.3.7: it must fit the buffer exactly).
    #[error("{0} trailing bytes")]
    TrailingBytes(usize),
    /// The advertisement has no archive, more than [`MAX_ADVERTISED_ARCHIVES`], or the
    /// language key is `0`.
    #[error("the advertisement's inputs are not usable")]
    UnusableInputs,
    /// Two archives have one resource identifier (§9.3.7).
    #[error("two archives have one resource identifier")]
    DuplicateIdentifier,
    /// An archive's iteration metadata is not usable: runs that do not add up to its count, or
    /// a trailing list that is not the highest listed iterations (`install-layout.md` §9.6
    /// invariant 7). Nothing is repaired.
    #[error("iteration metadata is not usable")]
    UnusableIteration,
    /// The data would pass [`MAX_ADVERTISEMENT_BYTES`].
    #[error("the advertisement is too large")]
    TooLarge,
}

/// A decoded server `0xDD` (§9.3.4, §9.3.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    /// The opaque server configuration word: kept, never compared with a local version.
    pub server_config: u32,
    /// The name-rule key, selecting a rule set of the installed name-filter table. Not a
    /// language code.
    pub name_rule_key: u32,
    /// The trailing fields; the send policy supports an empty array only (§9.3.7).
    pub trailing: Vec<u32>,
}

impl InstallRequest {
    /// Decodes the complete reassembled data, its flags byte at offset `0` (§9.3.7): flags
    /// `0x00` as it is, flags `0x01` after the bounded inflation of §9.1 and §10.1 item 9 (the
    /// layout is measured on the inflated body), and `0x02` to `0xFF` not at all.
    ///
    /// # Errors
    ///
    /// [`InitialError`] for truncation, other flags, a failed inflation, an out-of-bounds count
    /// or trailing bytes.
    pub fn from_data(data: &[u8]) -> Result<Self, InitialError> {
        let Some((&flags, body)) = data.split_first() else {
            return Err(InitialError::Truncated {
                needed: DD_FIXED_BYTES,
                available: 0,
            });
        };
        match flags {
            0 => Self::from_envelope_body(body),
            1 => {
                let envelope = crate::session::decode_message(
                    OPCODE_INSTALL_REQUEST,
                    data,
                    crate::session::DEFAULT_MAX_DECOMPRESSED_MESSAGE_BYTES,
                )
                .map_err(|_| InitialError::Decompression)?;
                Self::from_envelope_body(&envelope.body)
            }
            other => Err(InitialError::UnsupportedFlags(other)),
        }
    }

    /// Decodes the data after its flags byte (what [`crate::session::decode_message`] leaves
    /// of an unrouted `0xDD`, inflated when it was compressed).
    ///
    /// # Errors
    ///
    /// [`InitialError`] for truncation, an out-of-bounds count or trailing bytes.
    pub fn from_envelope_body(body: &[u8]) -> Result<Self, InitialError> {
        let needed = DD_FIXED_BYTES - 1;
        let (Some(config), Some(rule), Some(count)) = (
            read_u32(body, 0),
            read_u32(body, 4),
            read_u32(body, needed - 4),
        ) else {
            return Err(InitialError::Truncated {
                needed,
                available: body.len(),
            });
        };
        let rest = body.get(needed..).unwrap_or_default();
        // Checked against the remaining-byte quotient and the cap before any iteration.
        let n = usize::try_from(count).map_err(|_| InitialError::TrailingCount { count })?;
        if n > rest.len() / 4 || n > MAX_DD_TRAILING {
            return Err(InitialError::TrailingCount { count });
        }
        let extra = rest.len() - n * 4;
        if extra != 0 {
            return Err(InitialError::TrailingBytes(extra));
        }
        let trailing = rest
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect();
        Ok(Self {
            server_config: config,
            name_rule_key: rule,
            trailing,
        })
    }
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes(b.try_into().ok()?))
}

/// The folded language code a language archive carries as its subset (`dat-container.md` §3.4,
/// `official-session.md` §9.3.7): the union of the key's high and low 16-bit halves. Not a
/// language key: only the full key is tested against an allowlist or sent.
#[must_use]
pub const fn fold_language_code(key: u32) -> u32 {
    (key >> 16) | (key & 0xFFFF)
}

/// One run of an archive's iteration list: a single iteration has `len` `1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IterationSpan {
    /// The first iteration.
    pub start: i32,
    /// How many consecutive iterations.
    pub len: u32,
}

/// One trailing entry of an archive's iteration list (`install-layout.md` §9.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IterationNote {
    /// The iteration.
    pub iteration: u32,
    /// Eight opaque bytes, copied verbatim (§9.3.2).
    pub opaque: [u8; 8],
}

/// What one mounted base archive contributes to `0xEC`: its real identifier and its real
/// iteration metadata (§9.3.7). The `Debug` form names no value: it can reach a log.
#[derive(Clone, PartialEq, Eq)]
pub struct AdvertisedArchive {
    /// Superblock bytes `0x14C..=0x153`, as stored.
    pub resource_id: [u8; 8],
    /// The expanded iteration count `N`.
    pub iteration_count: u32,
    /// The run list, in record order.
    pub spans: Vec<IterationSpan>,
    /// The trailing entries. A stamp-1 record has none (§9.3.7: an explicit `K = 0` in `0xEC`).
    pub notes: Vec<IterationNote>,
}

impl std::fmt::Debug for AdvertisedArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdvertisedArchive")
            .field("spans", &self.spans.len())
            .field("notes", &self.notes.len())
            .finish_non_exhaustive()
    }
}

impl AdvertisedArchive {
    /// The eight bytes of entry offset `0` (§9.3.2): superblock bytes `0x150..=0x153`, then
    /// `0x14C..=0x14F`.
    #[must_use]
    pub fn wire_identifier(&self) -> [u8; 8] {
        let [a, b, c, d, e, f, g, h] = self.resource_id;
        [e, f, g, h, a, b, c, d]
    }

    /// `install-layout.md` §9.3/§9.6 invariant 7, for §9.3.7's sender and inventory check: the
    /// runs add up to the count, a repeated expanded or trailing iteration is malformed (Tessera's
    /// bounded input policy), and the trailing list is the `K` highest listed iterations in strict
    /// increasing order. Stored runs of one or two values are valid input; what is sent is the
    /// canonical form (`write_run`). Nothing is repaired.
    ///
    /// # Errors
    ///
    /// [`InitialError::UnusableIteration`].
    pub fn check_invariant(&self) -> Result<(), InitialError> {
        let mut expanded = 0u64;
        let mut sorted: Vec<(i64, u64)> = Vec::with_capacity(self.spans.len());
        for span in &self.spans {
            if span.len == 0 || span.start < 0 {
                return Err(InitialError::UnusableIteration);
            }
            expanded = expanded.saturating_add(u64::from(span.len));
            sorted.push((i64::from(span.start), u64::from(span.len)));
        }
        if expanded != u64::from(self.iteration_count) {
            return Err(InitialError::UnusableIteration);
        }
        sorted.sort_unstable();
        let repeats = sorted.windows(2).any(|w| {
            matches!(w, [(start, len), (next, _)]
                if start.saturating_add(i64::try_from(*len).unwrap_or(i64::MAX)) > *next)
        });
        if repeats {
            return Err(InitialError::UnusableIteration);
        }
        // Lazily from the top, so at most `K` values are made whatever the runs' lengths.
        let mut highest = sorted.iter().rev().flat_map(|&(start, len)| {
            (0..len).map(move |i| {
                start
                    .saturating_add(i64::try_from(len).unwrap_or(i64::MAX))
                    .saturating_sub(i64::try_from(i).unwrap_or(i64::MAX))
                    .saturating_sub(1)
            })
        });
        for note in self.notes.iter().rev() {
            if highest.next() != Some(i64::from(note.iteration)) {
                return Err(InitialError::UnusableIteration);
            }
        }
        Ok(())
    }
}

/// The sort key of decision 0046 item 3 for a wire identifier (§9.3.7): non-decreasing remainder
/// of the `u64` LE value of the eight wire bytes divided by [`ADVERTISEMENT_ORDER_MODULUS`], ties
/// by the increasing numeric value of the same integer (a labelled Tessera tie-break). An interim
/// relation fitting the owner's captures, not the official algorithm.
#[must_use]
pub fn advertisement_order(wire_identifier: [u8; 8]) -> (u64, u64) {
    let value = u64::from_le_bytes(wire_identifier);
    (value % ADVERTISEMENT_ORDER_MODULUS, value)
}

/// Calls `f` with each maximal run of consecutive increasing iterations of `spans`, in record
/// order: the records' own runs and singles merged where they touch (§9.3.7).
fn for_each_run(
    spans: &[IterationSpan],
    mut f: impl FnMut(i64, u64) -> Result<(), InitialError>,
) -> Result<(), InitialError> {
    let mut run: Option<(i64, u64)> = None;
    for span in spans {
        let start = i64::from(span.start);
        let len = u64::from(span.len);
        run = match run {
            Some((first, n))
                if first.saturating_add(i64::try_from(n).unwrap_or(i64::MAX)) == start =>
            {
                Some((first, n.saturating_add(len)))
            }
            other => {
                if let Some((first, n)) = other {
                    f(first, n)?;
                }
                Some((start, len))
            }
        };
    }
    if let Some((first, n)) = run {
        f(first, n)?;
    }
    Ok(())
}

/// Writes one maximal run: a negative length and its start from three values up, else singles
/// (§9.3.7: a run of two is two singles).
fn write_run(start: i64, len: u64, out: &mut Vec<u8>) -> Result<(), InitialError> {
    let first = i32::try_from(start).map_err(|_| InitialError::UnusableIteration)?;
    if len >= 3 {
        let token = i32::try_from(len)
            .map_err(|_| InitialError::UnusableIteration)?
            .wrapping_neg();
        out.extend_from_slice(&token.to_le_bytes());
        out.extend_from_slice(&first.to_le_bytes());
    } else {
        for i in 0..len {
            let value = start.saturating_add(i64::try_from(i).unwrap_or(i64::MAX));
            let value = i32::try_from(value).map_err(|_| InitialError::UnusableIteration)?;
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(())
}

/// The bytes [`write_iteration`] writes for `archive`: its count, tokens, `K` and trailing list.
fn iteration_bytes(archive: &AdvertisedArchive) -> Result<u64, InitialError> {
    let mut total = 8u64;
    for_each_run(&archive.spans, |_, len| {
        total = total.saturating_add(if len >= 3 { 8 } else { len.saturating_mul(4) });
        Ok(())
    })?;
    let notes = u64::try_from(archive.notes.len()).unwrap_or(u64::MAX);
    Ok(total.saturating_add(notes.saturating_mul(12)))
}

/// Writes the iteration description of §9.3.2 and §9.3.7: decoded and written again rather than
/// copied, the trailing list's values and opaque bytes as they are and written even when empty.
/// [`AdvertisedArchive::check_invariant`] has held.
fn write_iteration(archive: &AdvertisedArchive, out: &mut Vec<u8>) -> Result<(), InitialError> {
    out.extend_from_slice(&archive.iteration_count.to_le_bytes());
    for_each_run(&archive.spans, |start, len| write_run(start, len, out))?;
    let k = u32::try_from(archive.notes.len()).map_err(|_| InitialError::TooLarge)?;
    out.extend_from_slice(&k.to_le_bytes());
    for note in &archive.notes {
        out.extend_from_slice(&note.iteration.to_le_bytes());
        out.extend_from_slice(&note.opaque);
    }
    Ok(())
}

/// The complete reassembled data of `0xEC` (§9.3.2, §9.3.7): one flags byte `0`, the full key of the
/// mounted language (§9.3.2's row and Vectors 10 and 11, not `0xDD`'s word: §9.3.9's sentence on an
/// "opaque iteration" in the first scalar conflicts with them and is reported, not guessed), status
/// `1`, the entries in the order of [`advertisement_order`], `M = 0` and the
/// terminal `0`. The whole size is known before anything is allocated.
///
/// # Errors
///
/// [`InitialError`] when the inputs are not usable, an archive breaks
/// [`AdvertisedArchive::check_invariant`] or the data would pass [`MAX_ADVERTISEMENT_BYTES`];
/// nothing is dropped from a list to make it fit.
pub fn build_install_advertisement(
    language_key: u32,
    archives: &[AdvertisedArchive],
) -> Result<Vec<u8>, InitialError> {
    if language_key == 0 || archives.is_empty() || archives.len() > MAX_ADVERTISED_ARCHIVES {
        return Err(InitialError::UnusableInputs);
    }
    let mut ordered: Vec<&AdvertisedArchive> = archives.iter().collect();
    ordered.sort_by_key(|a| advertisement_order(a.wire_identifier()));
    if ordered
        .windows(2)
        .any(|w| matches!(w, [a, b] if a.wire_identifier() == b.wire_identifier()))
    {
        return Err(InitialError::DuplicateIdentifier);
    }
    // Flags, key, status and `N`, then `M` and the terminal field.
    let mut total = 1u64 + 4 + 4 + 4 + 4 + 4;
    for archive in &ordered {
        archive.check_invariant()?;
        total = total
            .saturating_add(8)
            .saturating_add(iteration_bytes(archive)?);
    }
    let capacity = usize::try_from(total)
        .ok()
        .filter(|t| *t <= MAX_ADVERTISEMENT_BYTES)
        .ok_or(InitialError::TooLarge)?;
    let mut out = Vec::with_capacity(capacity);
    out.push(0);
    out.extend_from_slice(&language_key.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    let n = u32::try_from(ordered.len()).map_err(|_| InitialError::UnusableInputs)?;
    out.extend_from_slice(&n.to_le_bytes());
    for archive in ordered {
        out.extend_from_slice(&archive.wire_identifier());
        write_iteration(archive, &mut out)?;
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    Ok(out)
}

/// Whether the `languages` list of the name rule `0xDD` selected allows `language_key`
/// (§9.3.7): an empty list fails, no other language or rule is substituted, and only the full
/// key is tested (neither its fold nor its low half stands in for it).
///
/// # Errors
///
/// [`UnsupportedNegotiation::NameRuleEmpty`] or
/// [`UnsupportedNegotiation::LanguageNotAllowed`].
pub fn check_language_allowed(
    languages: &[u32],
    language_key: u32,
) -> Result<(), UnsupportedNegotiation> {
    if languages.is_empty() {
        return Err(UnsupportedNegotiation::NameRuleEmpty);
    }
    if languages.contains(&language_key) {
        Ok(())
    } else {
        Err(UnsupportedNegotiation::LanguageNotAllowed)
    }
}

/// Why an install cannot answer the initial exchange's questions: a kind only, never a path,
/// identifier, key or iteration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallUnsupported {
    /// The supplier did not read this (the trait's default).
    NotRead,
    /// The Languages enum mapper or its base chain cannot be read.
    LanguageMapper,
    /// No label of the Languages enum matches the selected language.
    LanguageAbsent,
    /// Labels of two distinct keys match the selected language.
    LanguageAmbiguous,
    /// The matched key is `0`.
    LanguageKeyZero,
    /// The selected language's archive does not carry the fold of the key as its subset.
    LanguageSubset,
    /// The name-filter table cannot be found or read.
    NameTable,
    /// The table has no rule for the key `0xDD` named.
    NameRuleMissing,
    /// The table has two rules for the key `0xDD` named.
    NameRuleDuplicate,
    /// A base archive §9.3.7 requires is not mounted (the optional portal sets excepted).
    ArchiveMissing,
    /// An archive is present but failed to open, or its set is incomplete.
    ArchiveFailed,
    /// An archive of a family the policy does not know is present.
    ArchiveUnknown,
    /// An archive's resource identifier does not agree with its file name.
    ArchiveIdentifier,
    /// An archive uses the older identifier encoding (§9.3.7: neither the raw encoding nor a
    /// conversion is sent).
    ArchiveOlderIdentifier,
    /// An overlay lies next to a base archive: the set's records may be out of date.
    ArchiveOverlay,
    /// An active member's header holds a pending-journal checksum (`dat-container.md` §3.6): an
    /// update was interrupted and the archive may be half-applied.
    ArchivePendingJournal,
    /// A base archive has no usable iteration metadata.
    ArchiveIteration,
    /// More archives than [`MAX_ADVERTISED_ARCHIVES`].
    ArchiveCount,
    /// The DID maps or region keys the inventory needs cannot be read.
    InventoryMaps,
}

impl InstallUnsupported {
    /// A neutral label for a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotRead => "not read",
            Self::LanguageMapper => "language enum unreadable",
            Self::LanguageAbsent => "language label absent",
            Self::LanguageAmbiguous => "language label ambiguous",
            Self::LanguageKeyZero => "language key zero",
            Self::LanguageSubset => "language archive subset differs",
            Self::NameTable => "name-filter table unreadable",
            Self::NameRuleMissing => "name rule missing",
            Self::NameRuleDuplicate => "name rule duplicated",
            Self::ArchiveMissing => "base archive missing",
            Self::ArchiveFailed => "archive failed or set incomplete",
            Self::ArchiveUnknown => "archive of an unknown family",
            Self::ArchiveIdentifier => "archive identifier disagrees",
            Self::ArchiveOlderIdentifier => "older archive identifier encoding",
            Self::ArchiveOverlay => "overlay next to an archive",
            Self::ArchivePendingJournal => "pending journal on an archive",
            Self::ArchiveIteration => "iteration metadata unusable",
            Self::ArchiveCount => "too many archives",
            Self::InventoryMaps => "inventory maps unreadable",
        }
    }
}

/// One explicit failure of the initial exchange (§9.3.7: report it, do not fabricate `0xEC` or
/// wait indefinitely).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UnsupportedNegotiation {
    /// The install cannot supply the language key; checked before the ticket is spent.
    #[error("language key: {}", .0.label())]
    LanguageKey(InstallUnsupported),
    /// The install cannot supply the inventory; checked before the ticket is spent.
    #[error("archive inventory: {}", .0.label())]
    Inventory(InstallUnsupported),
    /// The advertisement cannot be built from the install's values.
    #[error("the advertisement cannot be built")]
    Advertisement,
    /// `0xDD` is malformed, or has an envelope other than plain.
    #[error("malformed install request")]
    MalformedRequest,
    /// The triggering `0xDD` has a non-empty trailing array (§9.3.8).
    #[error("install request with a trailing array")]
    RequestArray,
    /// The name-rule table or rule of `0xDD` cannot be used.
    #[error("name rule: {}", .0.label())]
    NameRule(InstallUnsupported),
    /// The rule's language list is empty (§9.3.7).
    #[error("empty name rule language list")]
    NameRuleEmpty,
    /// The rule's language list lacks the selected language's full key (§9.3.7).
    #[error("selected language not allowed by the name rule")]
    LanguageNotAllowed,
    /// `0xEC` could not be queued.
    #[error("the advertisement could not be sent")]
    AdvertisementNotSent,
    /// A queue-1 `0xF5` before `0xEC`, or not exactly `00` in the guard window (§9.3.8).
    #[error("unexpected completion message")]
    UnexpectedCompletion,
    /// An actual `0x12F` after `0xEC` (§9.3.8: no second or mixed advertisement).
    #[error("a version request after the advertisement")]
    LateVersionRequest,
    /// The client's `0xF5` could not be queued.
    #[error("the completion reply could not be sent")]
    CompletionNotSent,
    /// A dictionary the handshake names is not the install's (§9.3.8: before establishment).
    #[error("the connection's dictionary is not the install's")]
    Dictionary,
    /// A `0xB3` before the list that is not structurally valid (§9.3.9).
    #[error("a refusal that cannot be read")]
    MalformedRefusal,
    /// `0x12D` with a status §9.3 does not define, before the list (§9.3.9).
    #[error("a version status with no meaning in the spec")]
    UnknownVersionStatus,
    /// `0x130` with a result §9.3 does not define, before the list (§9.3.9).
    #[error("a queue status with no meaning in the spec")]
    UnknownQueueStatus,
    /// No `0xDD` within [`DD_WAIT_SEC`] of the first State 5.
    #[error("no install request arrived in time")]
    RequestDeadline,
    /// No server `0xF5` within [`COMPLETION_WAIT_SEC`] of `0xEC`.
    #[error("no completion arrived in time")]
    CompletionDeadline,
    /// No `0x6C` within [`LIST_WAIT_SEC`] of the client's `0xF5`.
    #[error("no character list arrived in time")]
    ListDeadline,
    /// No historical answer within [`VERSION_PATH_WAIT_SEC`] of an actual `0x12F`.
    #[error("no answer to the version exchange arrived in time")]
    VersionPathDeadline,
    /// No `0x6C` within [`HISTORICAL_LIST_WAIT_SEC`] of `0x12D` or a ready `0x130`.
    #[error("no character list arrived in time after the version exchange")]
    HistoricalListDeadline,
    /// The login queue did not end within [`QUEUE_WAIT_SEC`].
    #[error("the login queue did not end in time")]
    QueueDeadline,
    /// `0x6C` did not decode (§9.3.8: no scan and no partial list; no character is invented).
    #[error("the character list cannot be read")]
    ListDecode,
    /// The first `0x6C` decoded, and a name is unusable, an ID or name repeats or it is over its cap
    /// (§9.3.9: the whole list fails, with no partial rows).
    #[error("the character list's names cannot be used")]
    ListNames,
    /// `0x6C` is compressed or routed (§9.3.8: unsupported, never shifted by a guessed byte).
    #[error("a compressed or routed character list is not supported")]
    ListEnvelope,
}

impl UnsupportedNegotiation {
    /// A neutral label for a log line: a kind, never a value.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LanguageKey(_) => "language key",
            Self::Inventory(_) => "archive inventory",
            Self::Advertisement => "advertisement build",
            Self::MalformedRequest => "malformed install request",
            Self::RequestArray => "install request array",
            Self::NameRule(_) => "name rule",
            Self::NameRuleEmpty => "name rule empty",
            Self::LanguageNotAllowed => "language not allowed",
            Self::AdvertisementNotSent => "advertisement not sent",
            Self::UnexpectedCompletion => "unexpected completion",
            Self::LateVersionRequest => "version request after the advertisement",
            Self::CompletionNotSent => "completion not sent",
            Self::Dictionary => "dictionary",
            Self::MalformedRefusal => "malformed refusal",
            Self::UnknownVersionStatus => "unknown version status",
            Self::UnknownQueueStatus => "unknown queue status",
            Self::RequestDeadline => "install request deadline",
            Self::CompletionDeadline => "completion deadline",
            Self::ListDeadline => "character list deadline",
            Self::VersionPathDeadline => "version path deadline",
            Self::HistoricalListDeadline => "historical list deadline",
            Self::QueueDeadline => "queue deadline",
            Self::ListDecode => "character list decode",
            Self::ListNames => "character list names",
            Self::ListEnvelope => "character list envelope",
        }
    }

    /// Whether this is §9.3.9's deadline row: `LoginRefused { UnsupportedStep }`, then
    /// `Disconnected { UnsupportedTimeout }`. Every other result here is its required-progress row,
    /// with `Disconnected { Unsupported }`. Both show Tessera's fixed words (`official-entry.md`
    /// §8.6); the cause is the log's ([`Self::label`]), with no path, identifier, iteration value or
    /// account data.
    #[must_use]
    pub const fn is_deadline(self) -> bool {
        matches!(
            self,
            Self::RequestDeadline
                | Self::CompletionDeadline
                | Self::ListDeadline
                | Self::VersionPathDeadline
                | Self::HistoricalListDeadline
                | Self::QueueDeadline
        )
    }
}

/// The install's advertisement, built before the first datagram: the preflight of §9.3.7, so a
/// failure does not spend the ticket. The `Debug` form names no value.
#[derive(Clone, PartialEq, Eq)]
pub struct PreparedAdvertisement {
    /// The selected language's full key.
    pub language_key: u32,
    /// How many archives it lists.
    pub entries: usize,
    /// The complete reassembled data.
    pub data: Vec<u8>,
}

impl std::fmt::Debug for PreparedAdvertisement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedAdvertisement")
            .field("entries", &self.entries)
            .field("bytes", &self.data.len())
            .finish_non_exhaustive()
    }
}

impl PreparedAdvertisement {
    /// Builds the advertisement of `language_key` and `archives`.
    ///
    /// # Errors
    ///
    /// [`UnsupportedNegotiation::Advertisement`] when it cannot be built.
    pub fn new(
        language_key: u32,
        archives: &[AdvertisedArchive],
    ) -> Result<Self, UnsupportedNegotiation> {
        let data = build_install_advertisement(language_key, archives).map_err(|e| {
            tracing::debug!(error = %e, "official session: advertisement not built");
            UnsupportedNegotiation::Advertisement
        })?;
        Ok(Self {
            language_key,
            entries: archives.len(),
            data,
        })
    }
}

/// How far the initial exchange got, for the log lines of [`crate::connector::diag`]: flags
/// and counts, no value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InitialStages {
    /// A valid `0xDD` arrived.
    pub dd_seen: bool,
    /// `0xEC` was queued.
    pub ec_sent: bool,
    /// A server `0xF5` of the login connection arrived, usable or not.
    pub f5_received: bool,
    /// The client's `0xF5` was queued.
    pub f5_sent: bool,
    /// The character list was decoded.
    pub list_received: bool,
    /// The messages passed over while waiting, by opcode.
    pub skipped: SkippedMessages,
    /// Further `0xDD` and `0xF5` that made no reply (later ones, or on another connection,
    /// queue or envelope level), and messages left unhandled because the attempt was cancelled.
    pub ignored: u64,
}

/// The messages of the login window that are skipped: no dispatch, no reply and no decode
/// count (§9.3.7). A skipped message is already bounded and reassembled; nothing of it is kept.
/// The named opcodes are the log's; [`SkipCounters`] has every opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkippedMessages {
    /// `0x149`.
    pub op_149: u64,
    /// `0xAA`.
    pub op_aa: u64,
    /// `0x30000040`.
    pub op_30000040: u64,
    /// `0x43`.
    pub op_43: u64,
    /// `0xFE`.
    pub op_fe: u64,
    /// `0x54`.
    pub op_54: u64,
    /// Any other opcode of this window (§9.3.7's bounded unknown-message rule).
    pub other: u64,
    /// Bytes of all of them.
    pub bytes: u64,
}

impl SkippedMessages {
    /// Counts one skipped message of `len` bytes.
    pub fn note(&mut self, opcode: u32, len: usize) {
        let n = match opcode {
            0x149 => &mut self.op_149,
            0xAA => &mut self.op_aa,
            0x3000_0040 => &mut self.op_30000040,
            0x43 => &mut self.op_43,
            0xFE => &mut self.op_fe,
            0x54 => &mut self.op_54,
            _ => &mut self.other,
        };
        *n = n.saturating_add(1);
        self.bytes = self
            .bytes
            .saturating_add(u64::try_from(len).unwrap_or(u64::MAX));
    }
}

/// §9.3.7's bounded counter set of skipped messages: one count per opcode, for at most
/// [`MAX_SKIP_KEYS`] distinct opcodes; the others are counted together. No body is kept.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkipCounters {
    keys: BTreeMap<u32, u64>,
    overflow: u64,
}

impl SkipCounters {
    /// Counts one message of `opcode`.
    pub fn note(&mut self, opcode: u32) {
        if let Some(n) = self.keys.get_mut(&opcode) {
            *n = n.saturating_add(1);
        } else if self.keys.len() < MAX_SKIP_KEYS {
            self.keys.insert(opcode, 1);
        } else {
            self.overflow = self.overflow.saturating_add(1);
        }
    }

    /// The count of `opcode`, `0` when it has none of its own.
    #[must_use]
    pub fn count(&self, opcode: u32) -> u64 {
        self.keys.get(&opcode).copied().unwrap_or(0)
    }

    /// How many distinct opcodes have a count of their own.
    #[must_use]
    pub fn keys(&self) -> usize {
        self.keys.len()
    }

    /// The messages counted together, past [`MAX_SKIP_KEYS`].
    #[must_use]
    pub const fn overflow(&self) -> u64 {
        self.overflow
    }
}

/// Where the initial exchange stands.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    /// State 5 has not been reached.
    Connecting,
    /// State 5; waiting for `0xDD` since the time held.
    AwaitingRequest(f64),
    /// `0xEC` sent; waiting for the server's `0xF5` since the time held: the guard window.
    AwaitingCompletion(f64),
    /// The client's `0xF5` sent; waiting for `0x6C` since the time held.
    AwaitingList(f64),
    /// An actual `0x12F` arrived at the time held: waiting for its answer.
    Historical(f64),
    /// `0x12D` status `1` or a ready `0x130` at the time held: waiting for `0x6C`.
    HistoricalList(f64),
    /// Queued since the first queued `0x130`, at the time held (one absolute limit).
    Queued(f64),
    /// The list arrived.
    Finished,
}

/// What to do with a queue-1 `0xF5` of the login connection while the exchange runs (§9.3.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionStep {
    /// In the guard window: exactly `00` gets the one reply, anything else fails.
    Guarded,
    /// Before `0xEC`, or on the historical path where `0xEC` is suppressed: unsupported.
    Unsupported,
    /// After the one reply: counted and ignored.
    Repeat,
}

/// The initial exchange's state on the login connection (sans I/O, the driver acts).
#[derive(Debug, Clone, PartialEq)]
pub struct Negotiation {
    phase: Phase,
    stages: InitialStages,
    skips: SkipCounters,
    /// The latest opaque configuration word of a valid `0xDD`: kept, never compared (§9.3.8).
    server_config: Option<u32>,
    /// When the login first queued: the base of its one absolute limit, kept for the whole attempt
    /// so that no later `0x130` can restart it (§9.3.8).
    queued_since: Option<f64>,
    /// An actual `0x12F` arrived before `0xEC`: `0xEC` is suppressed (§9.3.8).
    historical: bool,
    /// The first valid `0xDD` that arrived while queued, with the recipient it named, held for the
    /// ready status ([`Self::ready_before_request`]).
    held_request: Option<(InstallRequest, u64)>,
}

impl Default for Negotiation {
    fn default() -> Self {
        Self::new()
    }
}

impl Negotiation {
    /// Before State 5.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            skips: SkipCounters {
                keys: BTreeMap::new(),
                overflow: 0,
            },
            server_config: None,
            phase: Phase::Connecting,
            stages: InitialStages {
                dd_seen: false,
                ec_sent: false,
                f5_received: false,
                f5_sent: false,
                list_received: false,
                skipped: SkippedMessages {
                    op_149: 0,
                    op_aa: 0,
                    op_30000040: 0,
                    op_43: 0,
                    op_fe: 0,
                    op_54: 0,
                    other: 0,
                    bytes: 0,
                },
                ignored: 0,
            },
            queued_since: None,
            historical: false,
            held_request: None,
        }
    }

    /// How far the exchange got.
    #[must_use]
    pub const fn stages(&self) -> InitialStages {
        self.stages
    }

    /// The skipped messages by opcode (§9.3.8).
    #[must_use]
    pub const fn skip_counters(&self) -> &SkipCounters {
        &self.skips
    }

    /// The latest valid `0xDD`'s opaque word.
    #[must_use]
    pub const fn server_config(&self) -> Option<u32> {
        self.server_config
    }

    /// Where a queue-1 `0xF5` of the login connection stands (§9.3.9 initial stage × event table,
    /// with the EC-sent and F5-replied history kept across stages): after the one reply, any repeat
    /// skips in every stage; in the `0xEC` completion window, or in a list wait with `0xEC` sent and
    /// no reply yet, the first exact `00` gets its one reply; otherwise (before `0xEC`, or in the
    /// historical-result or queued stage without a reply) it is unsupported.
    #[must_use]
    pub const fn completion_step(&self) -> CompletionStep {
        if self.stages.f5_sent {
            CompletionStep::Repeat
        } else if matches!(self.phase, Phase::AwaitingCompletion(_))
            || (self.stages.ec_sent
                && matches!(
                    self.phase,
                    Phase::AwaitingList(_) | Phase::HistoricalList(_)
                ))
        {
            CompletionStep::Guarded
        } else {
            CompletionStep::Unsupported
        }
    }

    /// Whether a valid `0xDD` on queue 1 of the primary connection triggers `0xEC` now: only the
    /// first, only in the `AwaitDD` stage and never on the historical path (§9.3.9 stage table:
    /// every other stage stores and skips a valid one).
    #[must_use]
    pub const fn wants_request(&self) -> bool {
        !self.stages.dd_seen && !self.historical && matches!(self.phase, Phase::AwaitingRequest(_))
    }

    /// The login connection first reached State 5 at `now`.
    pub fn connected(&mut self, now: f64) {
        if matches!(self.phase, Phase::Connecting) {
            self.phase = Phase::AwaitingRequest(now);
        }
    }

    /// An actual `0x12F` arrived at `now` (§9.3.9 stage table). Only in the `AwaitDD` stage with
    /// `0xEC` not sent does it switch to the historical path and answer `true` (one `0x12C`). In
    /// every other stage without `0xEC` (list wait, historical result, queued) it skips: counted,
    /// no resend and no clock change. With `0xEC` sent the driver reports it unsupported first
    /// ([`Self::version_request_too_late`]).
    pub fn version_requested(&mut self, now: f64) -> bool {
        if self.historical
            || self.stages.ec_sent
            || !matches!(self.phase, Phase::AwaitingRequest(_))
        {
            self.ignored();
            return false;
        }
        self.historical = true;
        self.phase = Phase::Historical(now);
        true
    }

    /// Whether a `0x12F` now comes after `0xEC`, which is unsupported in every stage before
    /// `Connected` (§9.3.9 stage table: `AwaitF5` row, and "EC-sent: Unsupported" in the list,
    /// historical-result and queued rows).
    #[must_use]
    pub const fn version_request_too_late(&self) -> bool {
        self.stages.ec_sent
    }

    /// `0x12D` status `1` at `now`: the §9.3.9 status rule. In the `AwaitDD` stage an unsolicited
    /// one is counted and skipped, keeps the `0xDD` clock and sends nothing. Before the first `0xDD`
    /// it is a ready status, which keeps or restores the `0xDD` wait (labelled C8 choice,
    /// remediation §31 fix 4: `0xEC` goes exactly once on the first valid `0xDD`). In every other
    /// active stage it enters the list wait like a ready `0x130` ([`Self::queue_ready`]); answers
    /// `true` when that started a list clock or went back to the `0xDD` wait.
    pub fn version_accepted(&mut self, now: f64) -> bool {
        if self.awaits_first_request() {
            return self.ready_before_request(now);
        }
        self.list_ready(now)
    }

    /// A queued `0x130` at `now` cancels the running non-queue wait and installs one absolute limit
    /// from the FIRST queued status of this login attempt, which no later `0x130` extends (§9.3.9,
    /// Vector 25): a queued status after a ready one reinstates the SAME limit. Answers whether this
    /// was the first. When the reinstated limit has already passed, [`Self::expired`] answers
    /// [`UnsupportedNegotiation::QueueDeadline`] at once and the driver fails immediately.
    pub fn queued(&mut self, now: f64) -> bool {
        let first = self.queued_since.is_none();
        let since = *self.queued_since.get_or_insert(now);
        self.phase = Phase::Queued(since);
        first
    }

    /// A ready `0x130` at `now`: the same §9.3.9 status rule as [`Self::version_accepted`].
    pub fn queue_ready(&mut self, now: f64) -> bool {
        if self.awaits_first_request() {
            return self.ready_before_request(now);
        }
        self.list_ready(now)
    }

    /// No valid `0xDD` has come, `0xEC` has not gone and the path is not historical, in the `AwaitDD`
    /// or the queued stage: the first valid `0xDD` is still to trigger `0xEC`.
    const fn awaits_first_request(&self) -> bool {
        !self.stages.dd_seen
            && !self.stages.ec_sent
            && !self.historical
            && matches!(self.phase, Phase::AwaitingRequest(_) | Phase::Queued(_))
    }

    /// A ready status (`0x12D` status `1` or a ready `0x130`) before the first valid `0xDD`.
    ///
    /// Labelled choice (C8, pending writer answer to remediation §31 fix 4): §9.3.9's status rule
    /// would enter the list wait, after which a valid `0xDD` is only stored and `0xEC` never goes,
    /// so a queued login could never advertise its install. Tessera keeps `0xEC` exactly once on
    /// the first valid `0xDD`: a ready status before it is counted and skipped and leaves the
    /// stage at `AwaitDD`. In `AwaitDD` its clock is kept; from the queued stage, whose status
    /// cancelled that clock, the `0xDD` wait starts again at `now`, and the queue's absolute limit
    /// keeps its first origin for a later queued status ([`Self::queued`]). Answers `true` for that
    /// return from the queue (a real stage change), `false` for a skip in `AwaitDD`.
    ///
    /// The same choice covers a valid `0xDD` that arrives while queued: the first one is held with
    /// its recipient ([`Self::request_not_wanted`]) instead of being forgotten, and when a ready
    /// status brings the stage back to `AwaitDD` the driver takes it ([`Self::take_held_request`])
    /// and treats it as the first valid `0xDD` at that moment, so `0xEC` goes once for it. Later
    /// ones while queued only update the opaque word, as any later `0xDD`.
    fn ready_before_request(&mut self, now: f64) -> bool {
        self.ignored();
        if matches!(self.phase, Phase::Queued(_)) {
            self.phase = Phase::AwaitingRequest(now);
            return true;
        }
        false
    }

    /// The §9.3.9 status rule for `0x12D` status `1` (outside `AwaitDD`) and a ready `0x130` (in
    /// every active stage): enter the list wait with the EC-sent and F5-replied history kept. It
    /// cancels the prior non-list stage or queue clock and starts the 30-second list clock ONLY if
    /// no current list wait exists; answers `true` then. One while a list wait already runs (a
    /// repeat, or after the `0xF5` reply) is counted and never resets that clock.
    ///
    /// Labelled (question for the writer): a ready status after a re-queue finds no current list
    /// wait (the queued status cancelled it), so it starts a new one; the queue's absolute limit
    /// keeps its first origin, so alternating statuses cannot renew it ([`Self::queued`]).
    fn list_ready(&mut self, now: f64) -> bool {
        let list_wait = matches!(
            self.phase,
            Phase::AwaitingList(_) | Phase::HistoricalList(_) | Phase::Finished | Phase::Connecting
        );
        if list_wait {
            self.ignored();
            return false;
        }
        self.phase = Phase::HistoricalList(now);
        true
    }

    /// The first valid `0xDD` arrived, with its opaque word.
    pub fn request_seen(&mut self, server_config: u32) {
        self.stages.dd_seen = true;
        self.server_config = Some(server_config);
    }

    /// A later valid `0xDD` (§9.3.8): only its word is kept, and it is counted. No wait starts
    /// again, nothing is sent, and the install is not looked at again.
    pub fn later_request(&mut self, server_config: u32) {
        self.server_config = Some(server_config);
        self.ignored();
    }

    /// A valid `0xDD` that does not trigger `0xEC` now ([`Self::wants_request`] is `false`), from
    /// `recipient`. In the queued stage before the first valid `0xDD`, the first one is held for
    /// the ready status (labelled C8 choice, `Negotiation::ready_before_request`); its word is kept.
    /// Any other one is a later `0xDD` ([`Self::later_request`]).
    pub fn request_not_wanted(&mut self, request: InstallRequest, recipient: u64) {
        if self.held_request.is_none()
            && self.awaits_first_request()
            && matches!(self.phase, Phase::Queued(_))
        {
            self.server_config = Some(request.server_config);
            self.held_request = Some((request, recipient));
            return;
        }
        self.later_request(request.server_config);
    }

    /// Whether a valid `0xDD` from the queue is held for the ready status.
    #[must_use]
    pub const fn holds_request(&self) -> bool {
        self.held_request.is_some()
    }

    /// The `0xDD` held while queued, with its recipient, once the stage is back at `AwaitDD`
    /// ([`Self::wants_request`]): it is then the first valid `0xDD` (`dd_seen` set), and the
    /// latest opaque word is kept. `None` while still queued or when none is held.
    pub fn take_held_request(&mut self) -> Option<(InstallRequest, u64)> {
        if !self.wants_request() {
            return None;
        }
        let held = self.held_request.take()?;
        self.stages.dd_seen = true;
        Some(held)
    }

    /// The last fragment of `0xEC` first went out at `now`: the guard window opens.
    pub fn advertisement_sent(&mut self, now: f64) {
        self.stages.ec_sent = true;
        self.phase = Phase::AwaitingCompletion(now);
    }

    /// A queue-1 server `0xF5` of the login connection arrived, whether or not it can be answered.
    pub fn completion_seen(&mut self) {
        self.stages.f5_received = true;
    }

    /// The client's one `0xF5` was allocated at `now`: from the completion window the list wait
    /// starts; a reply inside a list wait that already runs does not extend its clock (§9.3.9
    /// `AwaitList` row).
    pub fn completion_sent(&mut self, now: f64) {
        self.stages.f5_sent = true;
        if !matches!(
            self.phase,
            Phase::AwaitingList(_) | Phase::HistoricalList(_)
        ) {
            self.phase = Phase::AwaitingList(now);
        }
    }

    /// The character list was decoded: every wait ends (§9.3.8).
    pub fn list_received(&mut self) {
        self.stages.list_received = true;
        self.phase = Phase::Finished;
    }

    /// Counts one skipped message.
    pub fn skipped(&mut self, opcode: u32, len: usize) {
        self.stages.skipped.note(opcode, len);
        self.skips.note(opcode);
    }

    /// Counts one `0xDD`, `0xF5`, `0x12F`, `0x12D` or `0x130` that makes no reply and changes no
    /// clock.
    pub fn ignored(&mut self) {
        self.stages.ignored = self.stages.ignored.saturating_add(1);
    }

    /// When the current wait ends, `None` when nothing is awaited.
    #[must_use]
    pub fn deadline(&self) -> Option<f64> {
        match self.phase {
            Phase::AwaitingRequest(t) => Some(t + DD_WAIT_SEC),
            Phase::AwaitingCompletion(t) => Some(t + COMPLETION_WAIT_SEC),
            Phase::AwaitingList(t) => Some(t + LIST_WAIT_SEC),
            Phase::Historical(t) => Some(t + VERSION_PATH_WAIT_SEC),
            Phase::HistoricalList(t) => Some(t + HISTORICAL_LIST_WAIT_SEC),
            Phase::Queued(t) => Some(t + QUEUE_WAIT_SEC),
            Phase::Connecting | Phase::Finished => None,
        }
    }

    /// The result of a wait that ended by `now`. None of them starts again for other traffic, a
    /// notice, a repeated `0xDD`, `0xF5` or `0x130`, or a retransmission (§9.3.8).
    #[must_use]
    pub fn expired(&self, now: f64) -> Option<UnsupportedNegotiation> {
        if self.deadline().is_none_or(|t| now < t) {
            return None;
        }
        match self.phase {
            Phase::AwaitingRequest(_) => Some(UnsupportedNegotiation::RequestDeadline),
            Phase::AwaitingCompletion(_) => Some(UnsupportedNegotiation::CompletionDeadline),
            Phase::AwaitingList(_) => Some(UnsupportedNegotiation::ListDeadline),
            Phase::Historical(_) => Some(UnsupportedNegotiation::VersionPathDeadline),
            Phase::HistoricalList(_) => Some(UnsupportedNegotiation::HistoricalListDeadline),
            Phase::Queued(_) => Some(UnsupportedNegotiation::QueueDeadline),
            Phase::Connecting | Phase::Finished => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(id: [u8; 8]) -> AdvertisedArchive {
        AdvertisedArchive {
            resource_id: id,
            iteration_count: 1,
            spans: vec![IterationSpan { start: 1, len: 1 }],
            notes: Vec::new(),
        }
    }

    #[test]
    fn the_flags_fold_and_order_helpers() {
        assert_eq!(fold_language_code(2), 2);
        assert_eq!(fold_language_code(0x1000_0001), 0x1001);
        assert_eq!(fold_language_code(0x1000_0004), 0x1004);
        // The wire identifier exchanges the two four-byte halves (§9.3.2, Vector 10).
        let a = archive([0, 0, 2, 1, 9, 0, 0, 0]);
        assert_eq!(a.wire_identifier(), [9, 0, 0, 0, 0, 0, 2, 1]);
    }

    #[test]
    fn entries_sort_by_remainder_then_value() {
        // Wire values 23 (remainder 0), 24 (remainder 1) and 46 (remainder 0): 23, 46, 24.
        let wire = |v: u64| {
            let b = v.to_le_bytes();
            [b[4], b[5], b[6], b[7], b[0], b[1], b[2], b[3]]
        };
        let archives = [archive(wire(24)), archive(wire(46)), archive(wire(23))];
        let data = build_install_advertisement(5, &archives).expect("builds");
        let entry = |i: usize| {
            let at = 13 + i * (8 + 4 + 4 + 4);
            u64::from_le_bytes(data[at..at + 8].try_into().expect("eight"))
        };
        assert_eq!((entry(0), entry(1), entry(2)), (23, 46, 24));
    }

    #[test]
    fn a_duplicate_identifier_or_no_input_is_refused() {
        let a = archive([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            build_install_advertisement(5, &[a.clone(), a.clone()]),
            Err(InitialError::DuplicateIdentifier)
        );
        assert_eq!(
            build_install_advertisement(5, &[]),
            Err(InitialError::UnusableInputs)
        );
        assert_eq!(
            build_install_advertisement(0, &[a]),
            Err(InitialError::UnusableInputs)
        );
    }

    #[test]
    fn iteration_runs_follow_the_serializer_rule() {
        // Two record runs that touch become one; a pair stays two tokens.
        let a = AdvertisedArchive {
            resource_id: [0, 0, 3, 1, 0, 0, 0, 0],
            iteration_count: 7,
            spans: vec![
                IterationSpan { start: 1, len: 2 },
                IterationSpan { start: 3, len: 3 },
                IterationSpan { start: 9, len: 2 },
            ],
            notes: Vec::new(),
        };
        let data = build_install_advertisement(2, &[a]).expect("builds");
        let body = &data[21..];
        let mut want: Vec<u8> = Vec::new();
        for v in [7i32, -5, 1, 9, 10, 0, 0, 0] {
            want.extend_from_slice(&v.to_le_bytes());
        }
        assert_eq!(body, want.as_slice());
    }

    /// §9.3.7's caps: refused as unsupported, never cut, and the size is known before anything is
    /// allocated.
    #[test]
    fn the_caps_are_refused_and_nothing_is_cut() {
        let many = |n: usize| -> Vec<AdvertisedArchive> {
            (0..n)
                .map(|i| {
                    let mut a = archive([0; 8]);
                    a.resource_id[..8]
                        .copy_from_slice(&u64::try_from(i).expect("small").to_le_bytes());
                    a
                })
                .collect()
        };
        assert_eq!(MAX_ADVERTISED_ARCHIVES, 256);
        assert_eq!(MAX_ADVERTISEMENT_BYTES, 1024 * 1024);
        assert!(build_install_advertisement(5, &many(256)).is_ok());
        assert_eq!(
            build_install_advertisement(5, &many(257)),
            Err(InitialError::UnusableInputs)
        );
        // Singles, four bytes each: one past the cap, then one that fits it exactly.
        let singles = |n: i32| AdvertisedArchive {
            resource_id: [0; 8],
            iteration_count: u32::try_from(n).expect("positive"),
            spans: (0..n)
                .map(|i| IterationSpan {
                    start: i * 2,
                    len: 1,
                })
                .collect(),
            notes: Vec::new(),
        };
        // Flags, key, status, N, M and terminal: 21; the entry's identifier, N and K: 16.
        let fits = i32::try_from((MAX_ADVERTISEMENT_BYTES - 37) / 4).expect("small");
        let data = build_install_advertisement(5, &[singles(fits)]).expect("fits");
        assert!(data.len() <= MAX_ADVERTISEMENT_BYTES);
        assert_eq!(data.capacity(), data.len(), "the size was known before");
        assert_eq!(
            build_install_advertisement(5, &[singles(fits + 1)]),
            Err(InitialError::TooLarge)
        );
    }

    #[test]
    fn the_skip_counter_counts_per_opcode_and_then_together() {
        let mut c = SkipCounters::default();
        for op in 0..300u32 {
            c.note(op);
        }
        c.note(0);
        assert_eq!(c.keys(), MAX_SKIP_KEYS);
        assert_eq!(c.overflow(), 300 - 256);
        assert_eq!(c.count(0), 2);
        assert_eq!(c.count(299), 0);
        // A later copy of a key it has still counts as itself.
        c.note(255);
        assert_eq!(c.count(255), 2);
        assert_eq!(c.overflow(), 300 - 256);
    }

    /// A later `0xDD` starts no wait again and only updates the word.
    #[test]
    fn a_later_request_starts_no_wait_again() {
        let mut n = Negotiation::new();
        n.connected(0.0);
        n.request_seen(1);
        n.advertisement_sent(1.0);
        n.later_request(2);
        assert_eq!(n.server_config(), Some(2));
        assert_eq!(n.deadline(), Some(1.0 + COMPLETION_WAIT_SEC));
        assert_eq!(n.stages().ignored, 1);
    }

    #[test]
    fn runs_that_do_not_add_up_are_refused() {
        let mut a = archive([0; 8]);
        a.iteration_count = 2;
        assert_eq!(
            build_install_advertisement(5, &[a]),
            Err(InitialError::UnusableIteration)
        );
        let mut a = archive([0; 8]);
        a.spans = vec![IterationSpan { start: -1, len: 1 }];
        assert_eq!(
            build_install_advertisement(5, &[a]),
            Err(InitialError::UnusableIteration)
        );
    }

    #[test]
    fn the_deadlines_follow_the_phases() {
        let mut n = Negotiation::new();
        assert_eq!(n.deadline(), None);
        n.connected(10.0);
        assert_eq!(n.deadline(), Some(40.0));
        assert_eq!(n.expired(39.0), None);
        assert_eq!(
            n.expired(40.0),
            Some(UnsupportedNegotiation::RequestDeadline)
        );
        n.request_seen(0);
        n.advertisement_sent(60.0);
        assert_eq!(n.completion_step(), CompletionStep::Guarded);
        assert_eq!(
            n.expired(90.0),
            Some(UnsupportedNegotiation::CompletionDeadline)
        );
        n.completion_sent(70.0);
        assert_eq!(n.completion_step(), CompletionStep::Repeat);
        assert_eq!(n.expired(100.0), Some(UnsupportedNegotiation::ListDeadline));
        n.list_received();
        assert_eq!(n.deadline(), None);
    }

    /// §9.3.8: the historical path: one `0x12C`, `0xEC` suppressed, and a replacement clock.
    #[test]
    fn the_historical_path_has_its_own_clocks() {
        let mut h = Negotiation::new();
        h.connected(0.0);
        assert_eq!(h.completion_step(), CompletionStep::Unsupported);
        assert!(h.version_requested(5.0));
        assert!(!h.wants_request(), "`0xEC` is suppressed");
        assert_eq!(h.deadline(), Some(35.0));
        // A repeat counts and restarts nothing.
        assert!(!h.version_requested(20.0));
        assert_eq!(h.deadline(), Some(35.0));
        assert_eq!(
            h.expired(35.0),
            Some(UnsupportedNegotiation::VersionPathDeadline)
        );
        assert!(h.version_accepted(30.0));
        assert_eq!(h.deadline(), Some(60.0));
        assert_eq!(
            h.expired(60.0),
            Some(UnsupportedNegotiation::HistoricalListDeadline)
        );
    }

    /// §9.3.8: the queue has one absolute limit; another queued status never extends it.
    #[test]
    fn the_queue_has_one_absolute_limit() {
        let mut n = Negotiation::new();
        n.connected(0.0);
        assert!(n.queued(50.0));
        assert_eq!(n.deadline(), Some(350.0));
        assert!(!n.queued(200.0));
        assert_eq!(n.deadline(), Some(350.0));
        assert_eq!(
            n.expired(350.0),
            Some(UnsupportedNegotiation::QueueDeadline)
        );
        assert!(n.queue_ready(300.0));
        assert_eq!(n.deadline(), Some(330.0));
        n.list_received();
        assert_eq!(n.deadline(), None);
    }

    /// §9.3.9 and Vector 25: queued, ready and queued again keep the first limit, however often
    /// they alternate; a queued status after that limit fails at once.
    #[test]
    fn queued_and_ready_cannot_alternate_past_the_limit() {
        let mut n = Negotiation::new();
        n.connected(0.0);
        // After the first `0xDD` (a ready status before it is the labelled C8 skip, below).
        n.request_seen(1);
        assert!(n.queued(0.0));
        assert!(n.queue_ready(290.0));
        assert_eq!(n.deadline(), Some(320.0));
        assert!(!n.queued(291.0));
        assert_eq!(n.deadline(), Some(300.0), "the first limit, not a new one");
        // Labelled: a ready after the re-queue starts a new list wait.
        assert!(n.queue_ready(292.0));
        assert_eq!(n.deadline(), Some(322.0));
        let ignored = n.stages().ignored;
        assert!(!n.queue_ready(293.0), "a repeat in the list wait");
        assert_eq!(n.deadline(), Some(322.0));
        assert_eq!(n.stages().ignored, ignored + 1);
        // Queued again after the absolute limit: UnsupportedTimeout at once.
        assert!(!n.queued(301.0));
        assert_eq!(
            n.expired(301.0),
            Some(UnsupportedNegotiation::QueueDeadline)
        );
        assert!(UnsupportedNegotiation::QueueDeadline.is_deadline());
    }

    /// Vector 25: queued at 0, ready at 20, queued at 40 restores the absolute queue deadline 300
    /// seconds from the FIRST queued report, not 340.
    #[test]
    fn vector_25_the_queue_deadline_keeps_its_first_origin() {
        let mut n = Negotiation::new();
        n.connected(0.0);
        assert!(n.queued(0.0));
        assert!(n.queue_ready(20.0));
        assert!(!n.queued(40.0));
        assert_eq!(n.deadline(), Some(QUEUE_WAIT_SEC));
        assert_eq!(n.expired(QUEUE_WAIT_SEC - 0.1), None);
        assert_eq!(
            n.expired(QUEUE_WAIT_SEC),
            Some(UnsupportedNegotiation::QueueDeadline)
        );
    }

    /// Vector 25: an unsolicited `0x12D` status 1 while AwaitDD counts and skips, keeps the `0xDD`
    /// clock and starts no list wait.
    #[test]
    fn vector_25_status_1_in_await_dd_is_skipped() {
        let mut n = Negotiation::new();
        n.connected(0.0);
        assert!(!n.version_accepted(5.0));
        assert_eq!(n.stages().ignored, 1);
        assert_eq!(n.deadline(), Some(DD_WAIT_SEC), "the DD clock is kept");
        assert!(n.wants_request(), "still in AwaitDD");
        // Labelled C8 (remediation §31 fix 4): a ready 0x130 before the first 0xDD is skipped too.
        assert!(!n.queue_ready(6.0));
        assert_eq!(n.stages().ignored, 2);
        assert_eq!(n.deadline(), Some(DD_WAIT_SEC));
        assert!(n.wants_request());
    }

    /// Labelled C8 choice (remediation §31 fix 4): a queued login whose ready status comes before
    /// the first `0xDD` goes back to the `0xDD` wait, so the first valid `0xDD` still sends `0xEC`
    /// exactly once; the queue keeps its first origin; after `0xEC` the status rule is §9.3.9's.
    #[test]
    fn a_ready_status_before_the_first_request_keeps_the_advertisement() {
        let rules: [fn(&mut Negotiation, f64) -> bool; 2] =
            [Negotiation::queue_ready, Negotiation::version_accepted];
        for ready in rules {
            let mut n = Negotiation::new();
            n.connected(0.0);
            assert!(n.queued(1.0));
            assert!(
                !n.wants_request(),
                "a 0xDD while queued is stored and skipped"
            );
            assert!(ready(&mut n, 20.0), "back to the 0xDD wait");
            assert_eq!(n.deadline(), Some(20.0 + DD_WAIT_SEC));
            assert!(n.wants_request());
            // A repeat is skipped and keeps that clock.
            assert!(!ready(&mut n, 21.0));
            assert_eq!(n.deadline(), Some(20.0 + DD_WAIT_SEC));
            // Queued again: the same absolute limit from the first queued status.
            assert!(!n.queued(30.0));
            assert_eq!(n.deadline(), Some(1.0 + QUEUE_WAIT_SEC));
            assert!(ready(&mut n, 40.0));
            // The first valid 0xDD sends 0xEC, once.
            assert!(n.wants_request());
            n.request_seen(7);
            n.advertisement_sent(41.0);
            assert!(n.stages().ec_sent);
            assert!(!n.wants_request(), "never a second 0xEC");
            // After 0xEC a ready status follows §9.3.9: the list wait.
            assert!(n.queue_ready(42.0));
            assert_eq!(n.deadline(), Some(42.0 + HISTORICAL_LIST_WAIT_SEC));
        }
    }

    /// Labelled C8 choice (remediation §31 fix 4): a valid `0xDD` that arrives while queued is held,
    /// and the ready status that brings back `AwaitDD` releases it as the first valid `0xDD`, once;
    /// a later `0xDD` while queued only updates the word.
    #[test]
    fn a_request_while_queued_is_held_for_the_ready_status() {
        let rules: [fn(&mut Negotiation, f64) -> bool; 2] =
            [Negotiation::queue_ready, Negotiation::version_accepted];
        let request = |word| InstallRequest {
            server_config: word,
            name_rule_key: 3,
            trailing: Vec::new(),
        };
        for ready in rules {
            let mut n = Negotiation::new();
            n.connected(0.0);
            assert!(n.queued(1.0));
            assert!(!n.wants_request());
            n.request_not_wanted(request(7), 2);
            assert!(n.holds_request());
            assert!(!n.stages().dd_seen);
            assert_eq!(n.take_held_request(), None, "still queued");
            n.request_not_wanted(request(8), 5);
            assert_eq!(n.stages().ignored, 1, "a later one is counted");
            assert_eq!(n.server_config(), Some(8));
            assert!(ready(&mut n, 2.0));
            let (held, recipient) = n.take_held_request().expect("released");
            assert_eq!((held.server_config, recipient), (7, 2), "the first one");
            assert_eq!(n.server_config(), Some(8), "the latest word is kept");
            assert!(n.stages().dd_seen);
            assert!(!n.wants_request());
            assert_eq!(n.take_held_request(), None, "once");
            n.advertisement_sent(2.0);
            assert!(n.stages().ec_sent);
        }
        // Outside the queue a request that is not wanted is a later one, never held.
        let mut n = Negotiation::new();
        n.connected(0.0);
        n.request_seen(1);
        n.request_not_wanted(request(9), 2);
        assert!(!n.holds_request());
        let mut h = Negotiation::new();
        h.connected(0.0);
        assert!(h.version_requested(1.0));
        h.request_not_wanted(request(9), 2);
        assert!(!h.holds_request(), "historical path");
    }

    /// §9.3.9 status rule: only the FIRST `0x12D` status 1 or ready `0x130` starts the list clock;
    /// a repeat of either, or one while a list wait runs, restarts nothing.
    #[test]
    fn a_repeat_or_unsolicited_status_changes_no_clock() {
        let mut h = Negotiation::new();
        h.connected(0.0);
        assert!(h.version_requested(1.0));
        assert!(h.version_accepted(2.0));
        for t in [10.0, 20.0, 31.0] {
            assert!(!h.version_accepted(t));
            assert!(!h.queue_ready(t + 0.5));
            assert_eq!(h.deadline(), Some(32.0));
        }
        assert_eq!(
            h.expired(32.0),
            Some(UnsupportedNegotiation::HistoricalListDeadline)
        );

        // After the first `0xDD` (before it a ready status is skipped, the labelled C8 choice): a
        // ready `0x130` enters the list wait, and the next of either kind is a repeat.
        let mut n = Negotiation::new();
        n.connected(0.0);
        n.request_seen(1);
        assert!(n.queue_ready(5.0));
        assert_eq!(n.deadline(), Some(5.0 + HISTORICAL_LIST_WAIT_SEC));
        assert!(!n.queue_ready(6.0));
        assert!(!n.version_accepted(6.0));
        assert_eq!(n.deadline(), Some(5.0 + HISTORICAL_LIST_WAIT_SEC));
        assert!(
            !n.wants_request(),
            "a valid 0xDD in the list wait is stored and skipped"
        );

        // In the `0xF5` window: the list wait starts and the history is kept, so the first exact
        // `00` still gets its reply, which does not extend the list clock.
        let mut n = Negotiation::new();
        n.connected(0.0);
        n.request_seen(1);
        n.advertisement_sent(10.0);
        assert!(n.queue_ready(12.0));
        assert_eq!(n.deadline(), Some(12.0 + HISTORICAL_LIST_WAIT_SEC));
        assert_eq!(n.completion_step(), CompletionStep::Guarded);
        n.completion_sent(13.0);
        assert_eq!(n.deadline(), Some(12.0 + HISTORICAL_LIST_WAIT_SEC));
        assert_eq!(n.completion_step(), CompletionStep::Repeat);
        assert!(!n.version_accepted(14.0));
        assert_eq!(n.stages().ignored, 1);
    }

    /// §9.3.9 stage table: an `0xF5` with `0xEC` sent and no reply is unsupported in the queued and
    /// historical-result stages; a `0x12F` starts the historical path only in the `0xDD` wait.
    #[test]
    fn the_completion_and_version_request_cells() {
        let mut n = Negotiation::new();
        n.connected(0.0);
        n.request_seen(1);
        n.advertisement_sent(1.0);
        assert!(n.queued(2.0));
        assert_eq!(n.completion_step(), CompletionStep::Unsupported);
        assert!(n.version_request_too_late());

        // Queued before any `0xDD` or `0x12F`: a `0x12F` skips, with no `0x12C`.
        let mut n = Negotiation::new();
        n.connected(0.0);
        assert!(n.queued(1.0));
        assert!(!n.version_request_too_late());
        assert!(!n.version_requested(2.0));
        assert_eq!(n.deadline(), Some(1.0 + QUEUE_WAIT_SEC));
        assert_eq!(n.completion_step(), CompletionStep::Unsupported);
        assert!(!n.wants_request());

        // The historical result: no `0xEC`, so an `0xF5` is unsupported and a repeated `0x12F` skips.
        let mut n = Negotiation::new();
        n.connected(0.0);
        assert!(n.version_requested(1.0));
        assert!(!n.version_requested(2.0));
        assert_eq!(n.completion_step(), CompletionStep::Unsupported);
        assert_eq!(n.deadline(), Some(1.0 + VERSION_PATH_WAIT_SEC));
    }

    /// §9.3.8: the queue's limit keeps its base across the initial steps too.
    #[test]
    fn the_queue_limit_survives_the_initial_steps() {
        let mut n = Negotiation::new();
        n.connected(0.0);
        assert!(n.queued(10.0));
        n.request_seen(1);
        n.advertisement_sent(20.0);
        assert!(!n.queued(25.0));
        assert_eq!(n.deadline(), Some(10.0 + QUEUE_WAIT_SEC));
        n.completion_sent(30.0);
        assert!(!n.queued(35.0));
        assert_eq!(n.deadline(), Some(10.0 + QUEUE_WAIT_SEC));
    }

    #[test]
    fn a_completion_counts_as_seen_before_it_is_answered() {
        let mut n = Negotiation::new();
        n.completion_seen();
        let stages = n.stages();
        assert!(stages.f5_received && !stages.f5_sent);
    }

    #[test]
    fn the_log_form_names_the_kind() {
        // The log's form names the kind.
        let text =
            UnsupportedNegotiation::Inventory(InstallUnsupported::ArchiveMissing).to_string();
        assert!(
            text.contains(InstallUnsupported::ArchiveMissing.label()),
            "{text}"
        );
    }

    #[test]
    fn debug_forms_name_no_values() {
        let a = AdvertisedArchive {
            resource_id: [0xAB; 8],
            iteration_count: 77_777,
            spans: vec![IterationSpan {
                start: 1,
                len: 77_777,
            }],
            notes: vec![IterationNote {
                iteration: 77_777,
                opaque: [0xCD; 8],
            }],
        };
        let p = PreparedAdvertisement::new(5, std::slice::from_ref(&a)).expect("builds");
        for text in [format!("{a:?}"), format!("{p:?}")] {
            assert!(!text.contains("77777"), "{text}");
            assert!(!text.contains("171"), "{text}");
            assert!(!text.contains("205"), "{text}");
        }
    }
}
