// SPDX-License-Identifier: GPL-3.0-or-later
//! The initial exchange of the login connection (`official-session.md` §9.3.1–§9.3.5,
//! decision 0046): the server's `0xDD`, the client's install advertisement `0xEC`, the
//! completion exchange `0xF5` and the bounded wait for the character list `0x6C`. Sans I/O:
//! the session driver ([`crate::connector::session_phase`]) sends what this module builds,
//! and the install's values reach it through [`crate::connector::data::InstallData`].
//!
//! # What is whose
//!
//! `official-session.md` §9.3.4 states the wire layouts and the policy `P10-10E` this module
//! follows. It leaves questions open. They are marked in the code as
//! `pending spec pass 10 fix N`, each with the choice taken until the spec answers:
//!
//! - fix 4: `0xDD` flags other than `0` are rejected; only the first valid `0xDD` on queue 1
//!   of the primary login connection triggers `0xEC`.
//! - fix 7: iteration runs are written again under the §9.3.2 serializer rule, with stamp 1
//!   meaning `K = 0`.
//! - fix 8: the order of [`advertisement_order`] (decision 0046 item 3) with its labelled
//!   tie-break.
//! - fix 10: `0xF5` is answered once, only after `0xEC` was sent, only when its data is `00`.
//! - fix 2: the deadlines of [`Negotiation`] and the checks before the first datagram.
//! - fix 1: which archives the inventory lists and which problems refuse it (the client's
//!   `archive_inventory`) and the caps here.
//! - fix 9: `0xEC` and the client's `0xF5` stay resendable for a bounded window
//!   ([`crate::session::RETAINED_BLOB_RESEND_WINDOW_SEC`]).
//! - fix 13: a sequence gap that stays open ends the attempt
//!   ([`crate::session::MAX_GAP_AGE_SEC`]).
//!
//! No rule here comes from the spec's description of the original's own walk over its
//! archives (§9.3.4, the paragraph that says it is not a rule). Everything sent is the user's
//! real data (decision 0046 item 2); a rejected attempt is not retried with varied data.
//!
//! # Hostile input
//!
//! Every count is checked against the bytes left before anything is read or allocated. A
//! message that fails is rejected whole: the transport has already taken its datagram (§9.3.4).

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
/// Tessera's cap on `0xDD`'s trailing count (§9.3.4: a resource policy, not an official limit).
pub const MAX_DD_TRAILING: usize = 4096;

/// The grouping remainder of decision 0046 item 3 (§9.3.4: an interim relation, not an
/// archive count and not the official algorithm).
pub const ADVERTISEMENT_ORDER_MODULUS: u64 = 23;

/// Most archives one advertisement lists. pending spec pass 10 fix 1: labelled engineering
/// choice; an install's table and region keys give far fewer, and more is refused as
/// unsupported, never cut.
pub const MAX_ADVERTISED_ARCHIVES: usize = 64;
/// Longest advertisement data. pending spec pass 10 fix 1: labelled engineering choice, far
/// above a real install's; more is refused as unsupported, never cut.
pub const MAX_ADVERTISEMENT_BYTES: usize = 256 * 1024;

/// Wait from State 5 to `0xDD`. pending spec pass 10 fix 2: labelled engineering choice,
/// generous so a slow server does not fail; the spec only says not to wait indefinitely
/// (§9.3.4).
pub const DD_WAIT_SEC: f64 = 180.0;
/// Wait from `0xEC` to the server's `0xF5`. pending spec pass 10 fix 2: as [`DD_WAIT_SEC`].
pub const COMPLETION_WAIT_SEC: f64 = 120.0;
/// Wait from the client's `0xF5` to `0x6C`. pending spec pass 10 fix 2: as [`DD_WAIT_SEC`].
pub const LIST_WAIT_SEC: f64 = 120.0;
/// Wait from an actual `0x12F` to `0xDD` or `0x6C` on the historical version path (§9.3).
/// pending spec pass 10 fix 2: as [`DD_WAIT_SEC`]; §9.3 does not lift §9.3.4's rule against
/// waiting indefinitely for a character list.
pub const VERSION_PATH_WAIT_SEC: f64 = 180.0;

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
    /// The shared flags byte is not `0` (pending spec pass 10 fix 4).
    #[error("unsupported flags byte {0:#04x}")]
    UnsupportedFlags(u8),
    /// The counted array cannot fit the remaining bytes, or passes [`MAX_DD_TRAILING`].
    #[error("trailing count {count} is out of bounds")]
    TrailingCount {
        /// The count on the wire.
        count: u32,
    },
    /// Bytes follow the counted array (§9.3.4: exact exhaustion).
    #[error("{0} trailing bytes")]
    TrailingBytes(usize),
    /// The advertisement has no archive, more than [`MAX_ADVERTISED_ARCHIVES`], or the
    /// language key is `0`.
    #[error("the advertisement's inputs are not usable")]
    UnusableInputs,
    /// Two archives have one resource identifier (§9.3.4 item 2).
    #[error("two archives have one resource identifier")]
    DuplicateIdentifier,
    /// An archive's iteration metadata is not usable: runs that do not add up to its count.
    #[error("iteration metadata is not usable")]
    UnusableIteration,
    /// The data would pass [`MAX_ADVERTISEMENT_BYTES`].
    #[error("the advertisement is too large")]
    TooLarge,
}

/// A decoded server `0xDD` (§9.3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    /// The opaque server configuration word: kept, never compared with a local version.
    pub server_config: u32,
    /// The name-rule key, selecting a rule set of the installed name-filter table. Not a
    /// language code.
    pub name_rule_key: u32,
    /// The trailing fields; `P10-10E` supports an empty array only.
    pub trailing: Vec<u32>,
}

impl InstallRequest {
    /// Decodes the complete reassembled data, its flags byte at offset `0` (§9.3.4).
    ///
    /// pending spec pass 10 fix 4: flags other than `0` are rejected.
    ///
    /// # Errors
    ///
    /// [`InitialError`] for truncation, other flags, an out-of-bounds count or trailing bytes.
    pub fn from_data(data: &[u8]) -> Result<Self, InitialError> {
        let Some((&flags, body)) = data.split_first() else {
            return Err(InitialError::Truncated {
                needed: DD_FIXED_BYTES,
                available: 0,
            });
        };
        if flags != 0 {
            return Err(InitialError::UnsupportedFlags(flags));
        }
        Self::from_envelope_body(body)
    }

    /// Decodes the data after its flags byte (what [`crate::session::decode_message`] leaves
    /// of an uncompressed, unrouted `0xDD`).
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

/// The folded language code of a language archive's subset (`dat-container.md` §3.4): the
/// high 16 bits OR-ed into the low 16 bits.
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
/// iteration metadata (§9.3.4 item 3). The `Debug` form names no value: it can reach a log.
#[derive(Clone, PartialEq, Eq)]
pub struct AdvertisedArchive {
    /// Superblock bytes `0x14C..=0x153`, as stored.
    pub resource_id: [u8; 8],
    /// The expanded iteration count `N`.
    pub iteration_count: u32,
    /// The run list, in record order.
    pub spans: Vec<IterationSpan>,
    /// The trailing entries. A stamp-1 record has none (pending spec pass 10 fix 7: `K = 0`).
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
}

/// The sort key of decision 0046 item 3 for a wire identifier.
///
/// pending spec pass 10 fix 8: ascending remainder of the `u64` LE value at entry offset `0`
/// divided by [`ADVERTISEMENT_ORDER_MODULUS`]; ties by numeric order of the same value, a
/// labelled Tessera tie-break. An interim relation fitting the owner's captures, not the
/// official algorithm.
#[must_use]
pub fn advertisement_order(wire_identifier: [u8; 8]) -> (u64, u64) {
    let value = u64::from_le_bytes(wire_identifier);
    (value % ADVERTISEMENT_ORDER_MODULUS, value)
}

/// Writes one merged run (`start`, `len`) of the iteration description.
fn write_run(run: Option<(i64, u64)>, out: &mut Vec<u8>) -> Result<(), InitialError> {
    let Some((start, len)) = run else {
        return Ok(());
    };
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

/// Writes the iteration description of §9.3.2.
///
/// pending spec pass 10 fix 7: written again under the §9.3.2 serializer rule as it stands:
/// consecutive increasing iterations form one run, a run of at least three values is a
/// negative length token and its start, a shorter one is individual tokens; the trailing
/// list is written even when empty. With stamp 1 the archive has no notes (`K = 0`).
fn write_iteration(archive: &AdvertisedArchive, out: &mut Vec<u8>) -> Result<(), InitialError> {
    let mut expanded = 0u64;
    let mut run: Option<(i64, u64)> = None;
    out.extend_from_slice(&archive.iteration_count.to_le_bytes());
    for span in &archive.spans {
        if span.len == 0 || span.start < 0 {
            return Err(InitialError::UnusableIteration);
        }
        if out.len() > MAX_ADVERTISEMENT_BYTES {
            return Err(InitialError::TooLarge);
        }
        expanded = expanded.saturating_add(u64::from(span.len));
        let start = i64::from(span.start);
        run = match run {
            Some((first, len)) if first + i64::try_from(len).unwrap_or(i64::MAX) == start => {
                Some((first, len + u64::from(span.len)))
            }
            other => {
                write_run(other, out)?;
                Some((start, u64::from(span.len)))
            }
        };
    }
    write_run(run, out)?;
    if expanded != u64::from(archive.iteration_count) {
        return Err(InitialError::UnusableIteration);
    }
    let k = u32::try_from(archive.notes.len()).map_err(|_| InitialError::TooLarge)?;
    out.extend_from_slice(&k.to_le_bytes());
    for note in &archive.notes {
        if out.len() > MAX_ADVERTISEMENT_BYTES {
            return Err(InitialError::TooLarge);
        }
        out.extend_from_slice(&note.iteration.to_le_bytes());
        out.extend_from_slice(&note.opaque);
    }
    Ok(())
}

/// The complete reassembled data of `0xEC` (§9.3.2): one flags byte `0`, the full language
/// key, status `1`, the entries in the order of [`advertisement_order`], `M = 0` and the
/// terminal `0`.
///
/// # Errors
///
/// [`InitialError`] when the inputs are not usable or the data passes
/// [`MAX_ADVERTISEMENT_BYTES`]; nothing is dropped from a list to make it fit.
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
    let mut out = Vec::with_capacity(32);
    out.push(0);
    out.extend_from_slice(&language_key.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    let n = u32::try_from(ordered.len()).map_err(|_| InitialError::UnusableInputs)?;
    out.extend_from_slice(&n.to_le_bytes());
    for archive in ordered {
        out.extend_from_slice(&archive.wire_identifier());
        write_iteration(archive, &mut out)?;
        if out.len() > MAX_ADVERTISEMENT_BYTES {
            return Err(InitialError::TooLarge);
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    if out.len() > MAX_ADVERTISEMENT_BYTES {
        return Err(InitialError::TooLarge);
    }
    Ok(out)
}

/// Whether the `languages` list of the name rule `0xDD` selected allows `language_key`
/// (§9.3.4): an empty list fails, and no other language or rule is substituted.
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
    /// A base archive the policy needs is not mounted.
    ArchiveMissing,
    /// An archive is present but failed to open, or its set is incomplete.
    ArchiveFailed,
    /// An archive of a family the policy does not know is present.
    ArchiveUnknown,
    /// An archive's resource identifier does not agree with its file name.
    ArchiveIdentifier,
    /// An overlay lies next to a base archive: the set's records may be out of date.
    ArchiveOverlay,
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
            Self::ArchiveOverlay => "overlay next to an archive",
            Self::ArchiveIteration => "iteration metadata unusable",
            Self::ArchiveCount => "too many archives",
            Self::InventoryMaps => "inventory maps unreadable",
        }
    }
}

/// One explicit unsupported result of the initial exchange (§9.3.4: report unsupported, do
/// not fabricate `0xEC` or wait indefinitely).
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
    /// `0xDD` has a non-empty trailing array (§9.3.4 item 1).
    #[error("install request with a trailing array")]
    RequestArray,
    /// The name-rule table or rule of `0xDD` cannot be used.
    #[error("name rule: {}", .0.label())]
    NameRule(InstallUnsupported),
    /// The rule's language list is empty (§9.3.4).
    #[error("empty name rule language list")]
    NameRuleEmpty,
    /// The rule's language list lacks the selected language (§9.3.4).
    #[error("selected language not allowed by the name rule")]
    LanguageNotAllowed,
    /// `0xEC` could not be queued.
    #[error("the advertisement could not be sent")]
    AdvertisementNotSent,
    /// A server `0xF5` before `0xEC`, or not exactly `00` on queue 1 (pending fix 10).
    #[error("unexpected completion message")]
    UnexpectedCompletion,
    /// The client's `0xF5` could not be queued.
    #[error("the completion reply could not be sent")]
    CompletionNotSent,
    /// No `0xDD` within [`DD_WAIT_SEC`] of State 5.
    #[error("no install request arrived in time")]
    RequestDeadline,
    /// No server `0xF5` within [`COMPLETION_WAIT_SEC`] of `0xEC`.
    #[error("no completion arrived in time")]
    CompletionDeadline,
    /// No `0x6C` within [`LIST_WAIT_SEC`] of the client's `0xF5`.
    #[error("no character list arrived in time")]
    ListDeadline,
    /// No `0xDD` or `0x6C` within [`VERSION_PATH_WAIT_SEC`] of an actual `0x12F`.
    #[error("the version exchange did not lead to a character list in time")]
    VersionPathDeadline,
    /// `0x6C` did not decode (§9.3.5: list display stops; no character is invented).
    #[error("the character list cannot be read")]
    ListDecode,
    /// `0x6C` is compressed or routed (pending spec pass 10 fix 16).
    #[error("a compressed or routed character list is not supported")]
    ListEnvelope,
    /// A sequence gap stayed open because its packet cannot be taken, while the server kept
    /// sending (pending spec pass 10 fix 13).
    #[error("a packet of the server could not be taken")]
    TrafficRefused,
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
            Self::CompletionNotSent => "completion not sent",
            Self::RequestDeadline => "install request deadline",
            Self::CompletionDeadline => "completion deadline",
            Self::ListDeadline => "character list deadline",
            Self::VersionPathDeadline => "version path deadline",
            Self::ListDecode => "character list decode",
            Self::ListEnvelope => "character list envelope",
            Self::TrafficRefused => "sequence gap stayed open",
        }
    }

    /// The line under the refusal on the client's error screen. Tessera's own text, never the
    /// service's: it goes in `LoginRefused`'s `note`, not its `detail`.
    #[must_use]
    pub const fn detail(self) -> &'static str {
        match self {
            Self::LanguageKey(_) | Self::Inventory(_) | Self::Advertisement => {
                "This install's data could not be prepared for the official login."
            }
            Self::MalformedRequest
            | Self::RequestArray
            | Self::UnexpectedCompletion
            | Self::TrafficRefused => {
                "The server sent a message of the official login Tessera does not support yet."
            }
            // The install's own table, not the server's answer.
            Self::NameRule(InstallUnsupported::NameTable) => {
                "This install's name-rule table could not be read."
            }
            Self::NameRule(InstallUnsupported::NameRuleMissing) => {
                "This install has no name rule for the one the server selected."
            }
            Self::NameRule(_) => "This install's name rules are not usable.",
            Self::NameRuleEmpty | Self::LanguageNotAllowed => {
                "The name rule the server selected does not allow this install's language."
            }
            Self::AdvertisementNotSent | Self::CompletionNotSent => {
                "A message of the official login could not be sent."
            }
            Self::RequestDeadline
            | Self::CompletionDeadline
            | Self::ListDeadline
            | Self::VersionPathDeadline => {
                "The server did not continue the official login in time."
            }
            Self::ListDecode | Self::ListEnvelope => {
                "The server's character list could not be read by Tessera yet."
            }
        }
    }
}

/// The install's advertisement, built before the first datagram: what §9.3.4 does not make
/// depend on `0xDD` can fail without spending the ticket (pending spec pass 10 fix 2,
/// labelled engineering choice). The `Debug` form names no value.
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
/// count (§9.3.1, §9.3.5). A skipped message is already bounded and reassembled; nothing of
/// it is kept.
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
    /// Any other opcode of this window (§9.3.5's bounded unknown-message policy).
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

/// Where the initial exchange stands.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    /// State 5 has not been reached.
    Connecting,
    /// State 5; waiting for `0xDD` since the time held.
    AwaitingRequest(f64),
    /// `0xEC` sent; waiting for the server's `0xF5` since the time held.
    AwaitingCompletion(f64),
    /// The client's `0xF5` sent; waiting for `0x6C` since the time held.
    AwaitingList(f64),
    /// An actual `0x12F` arrived at the time held: the historical version path (§9.3).
    Historical(f64),
    /// The list arrived.
    Finished,
}

/// The initial exchange's state on the login connection (sans I/O, the driver acts).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Negotiation {
    phase: Phase,
    stages: InitialStages,
    /// The login queue says the account waits: the client shows its queue panel, whose watchdog
    /// does not run (`official-entry.md` §8.4), until the queue ends or the next step.
    in_queue: bool,
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
            in_queue: false,
        }
    }

    /// How far the exchange got.
    #[must_use]
    pub const fn stages(&self) -> InitialStages {
        self.stages
    }

    /// Whether a first `0xDD` may still trigger `0xEC` (pending spec pass 10 fix 4).
    #[must_use]
    pub const fn wants_request(&self) -> bool {
        !self.stages.dd_seen && !matches!(self.phase, Phase::Finished)
    }

    /// The login connection reached State 5 at `now`.
    pub fn connected(&mut self, now: f64) {
        if matches!(self.phase, Phase::Connecting) {
            self.phase = Phase::AwaitingRequest(now);
        }
    }

    /// An actual `0x12F` arrived at `now`: only then does the historical version path apply
    /// (§9.3), with a wait of its own.
    pub fn version_requested(&mut self, now: f64) {
        if matches!(self.phase, Phase::Connecting | Phase::AwaitingRequest(_)) {
            self.phase = Phase::Historical(now);
        }
    }

    /// The current wait starts again at `now`.
    fn restart_wait(&mut self, now: f64) {
        self.phase = match self.phase {
            Phase::AwaitingRequest(_) => Phase::AwaitingRequest(now),
            Phase::AwaitingCompletion(_) => Phase::AwaitingCompletion(now),
            Phase::AwaitingList(_) => Phase::AwaitingList(now),
            Phase::Historical(_) => Phase::Historical(now),
            other => other,
        };
    }

    /// The login queue says the account still waits: the current wait starts again.
    pub fn queued(&mut self, now: f64) {
        self.in_queue = true;
        self.restart_wait(now);
    }

    /// The login queue accepted the account: the wait of a queued login starts again at `now`.
    pub fn queue_left(&mut self, now: f64) {
        if self.in_queue {
            self.in_queue = false;
            self.restart_wait(now);
        }
    }

    /// A valid `0xDD` arrived.
    pub fn request_seen(&mut self) {
        self.stages.dd_seen = true;
        self.in_queue = false;
    }

    /// `0xEC` was queued at `now`.
    pub fn advertisement_sent(&mut self, now: f64) {
        self.stages.ec_sent = true;
        self.in_queue = false;
        self.phase = Phase::AwaitingCompletion(now);
    }

    /// A server `0xF5` of the login connection arrived, whether or not it can be answered.
    pub fn completion_seen(&mut self) {
        self.stages.f5_received = true;
    }

    /// The client's `0xF5` was queued at `now`.
    pub fn completion_sent(&mut self, now: f64) {
        self.stages.f5_sent = true;
        self.in_queue = false;
        self.phase = Phase::AwaitingList(now);
    }

    /// The character list was decoded.
    pub fn list_received(&mut self) {
        self.stages.list_received = true;
        self.in_queue = false;
        self.phase = Phase::Finished;
    }

    /// Counts one skipped message.
    pub fn skipped(&mut self, opcode: u32, len: usize) {
        self.stages.skipped.note(opcode, len);
    }

    /// Counts one `0xDD` or `0xF5` that makes no reply.
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
            Phase::Connecting | Phase::Finished => None,
        }
    }

    /// Whether the driver keeps the client's progress screen alive: a wait of this module is
    /// the bound, not the screen's shorter watchdog (`official-entry.md` §8.3). Not while the
    /// queue panel shows, which a progress event would replace (§8.4).
    #[must_use]
    pub fn keeps_progress(&self) -> bool {
        self.deadline().is_some() && !self.in_queue
    }

    /// The unsupported result of a wait that ended by `now`.
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
        assert_eq!(n.deadline(), Some(10.0 + DD_WAIT_SEC));
        assert_eq!(n.expired(10.0 + DD_WAIT_SEC - 1.0), None);
        assert_eq!(
            n.expired(10.0 + DD_WAIT_SEC),
            Some(UnsupportedNegotiation::RequestDeadline)
        );
        n.queued(50.0);
        assert_eq!(n.deadline(), Some(50.0 + DD_WAIT_SEC));
        n.advertisement_sent(60.0);
        assert_eq!(
            n.expired(60.0 + COMPLETION_WAIT_SEC),
            Some(UnsupportedNegotiation::CompletionDeadline)
        );
        n.completion_sent(70.0);
        assert_eq!(
            n.expired(70.0 + LIST_WAIT_SEC),
            Some(UnsupportedNegotiation::ListDeadline)
        );
        n.list_received();
        assert_eq!(n.deadline(), None);
        let mut h = Negotiation::new();
        h.connected(0.0);
        h.version_requested(5.0);
        assert_eq!(h.deadline(), Some(5.0 + VERSION_PATH_WAIT_SEC));
        assert_eq!(
            h.expired(5.0 + VERSION_PATH_WAIT_SEC),
            Some(UnsupportedNegotiation::VersionPathDeadline)
        );
        h.queued(50.0);
        assert_eq!(h.deadline(), Some(50.0 + VERSION_PATH_WAIT_SEC));
    }

    #[test]
    fn the_queue_panel_holds_the_keep_alive_until_the_queue_or_the_next_step_ends_it() {
        let mut n = Negotiation::new();
        assert!(!n.keeps_progress(), "nothing awaited yet");
        n.connected(0.0);
        assert!(n.keeps_progress());
        n.queued(10.0);
        assert!(!n.keeps_progress());
        n.queue_left(20.0);
        assert!(n.keeps_progress());
        assert_eq!(n.deadline(), Some(20.0 + DD_WAIT_SEC));
        // Leaving a queue that never began changes no wait.
        n.queue_left(25.0);
        assert_eq!(n.deadline(), Some(20.0 + DD_WAIT_SEC));
        n.queued(30.0);
        n.request_seen();
        assert!(n.keeps_progress());
        n.list_received();
        assert!(!n.keeps_progress(), "nothing awaited any more");
    }

    #[test]
    fn a_completion_counts_as_seen_before_it_is_answered() {
        let mut n = Negotiation::new();
        n.completion_seen();
        let stages = n.stages();
        assert!(stages.f5_received && !stages.f5_sent);
    }

    #[test]
    fn local_failures_of_the_name_table_are_not_the_servers_refusal() {
        let table = UnsupportedNegotiation::NameRule(InstallUnsupported::NameTable).detail();
        let refused = UnsupportedNegotiation::LanguageNotAllowed.detail();
        assert_ne!(table, refused);
        assert!(table.contains("install"));
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
