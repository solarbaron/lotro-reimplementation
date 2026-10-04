// SPDX-License-Identifier: GPL-3.0-or-later
//! Localized string tables, DID prefix 0x25 (spec §3).
//!
//! This module decodes the byte layout (`records.md` §3) and exposes the message model
//! (pieces and argument slots). Interpreting the text inside pieces (grammar tags,
//! selectors, escapes, markup) and StringInfo resolution belong to `tessera-text`
//! (`docs/specs/string-table.md`).

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

use crate::did::{Did, DidKind, name_hash_utf16};
use crate::error::Result;
use crate::reader::{
    PREALLOC_CAP, Reader, TSize, Utf16String, hash_index_bytes, index_u32, keys_in_bucket_order,
};
use crate::record::{Record, read_prologue};

/// One part of a message in rendering order (spec §3.4): pieces and argument slots
/// alternate, starting and ending with a piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagePart<'a> {
    /// A literal text piece, as stored (markup and escapes are not interpreted here).
    Piece(&'a Utf16String),
    /// An argument slot, identified by the argument ID (name hash) that fills it.
    Slot(u32),
}

/// A string-table invariant that a decoded table violates (spec §3.5). Violations do not
/// stop decoding; they are reported by [`StringTable::issues`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StringTableIssue {
    /// ST2: an entry's `token_high` is not 0.
    TokenHigh {
        /// The entry's token.
        token: u32,
    },
    /// ST3: an entry's argument count is not max(pieces − 1, 0).
    SlotCount {
        /// The entry's token.
        token: u32,
        /// Number of pieces.
        pieces: usize,
        /// Number of argument IDs.
        args: usize,
    },
    /// ST4: an argument-name group whose length or hashes disagree with the argument IDs.
    ArgNames {
        /// The entry's token.
        token: u32,
    },
    /// The name-group count byte is above 1 (observed 0 or 1).
    NameGroupCount {
        /// The entry's token.
        token: u32,
        /// Number of groups.
        groups: usize,
    },
    /// ST5: a token occurs more than once in the table.
    DuplicateToken {
        /// The repeated token.
        token: u32,
    },
    /// ST5 / spec §1.3.1: entries are not in bucket order for the table's hint.
    BucketOrder,
}

/// One message of a string table (spec §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringTableEntry {
    /// Message key, unique within the table.
    pub token: u32,
    /// Upper half of the stored 64-bit key; always 0 in observed data.
    pub token_high: u32,
    /// Literal text pieces, in order.
    pub pieces: Vec<Utf16String>,
    /// Argument IDs (name hashes of the argument variable names).
    pub arg_ids: Vec<u32>,
    /// Argument-name groups; each lists variable names parallel to `arg_ids`.
    pub arg_name_groups: Vec<Vec<Utf16String>>,
}

impl StringTableEntry {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let token = r.u32()?;
        let token_high = r.u32()?;
        let piece_count = r.u32()?;
        let mut pieces = r.alloc_vec(piece_count, 1, PREALLOC_CAP)?;
        for _ in 0..piece_count {
            let piece = r.utf16_string()?;
            r.push(&mut pieces, piece, piece_count)?;
        }
        let arg_count = r.u32()?;
        let mut arg_ids = r.alloc_vec(arg_count, 4, PREALLOC_CAP)?;
        for _ in 0..arg_count {
            let id = r.u32()?;
            r.push(&mut arg_ids, id, arg_count)?;
        }
        let group_count = r.u8()?;
        let mut arg_name_groups = r.alloc_vec(u32::from(group_count), 4, PREALLOC_CAP)?;
        for _ in 0..group_count {
            let name_count = r.u32()?;
            let mut names = r.alloc_vec(name_count, 1, PREALLOC_CAP)?;
            for _ in 0..name_count {
                let name = r.utf16_string()?;
                r.push(&mut names, name, name_count)?;
            }
            r.push(&mut arg_name_groups, names, u32::from(group_count))?;
        }
        Ok(Self {
            token,
            token_high,
            pieces,
            arg_ids,
            arg_name_groups,
        })
    }

    /// The message parts in rendering order (spec §3.4): piece 0, slot 0, piece 1, …
    ///
    /// If the entry stores more argument IDs than there are gaps between pieces (ST3 is
    /// violated), the extra slots follow the last piece, matching [`Self::render`].
    pub fn parts(&self) -> MessageParts<'_> {
        MessageParts {
            pieces: self.pieces.iter(),
            args: self.arg_ids.iter(),
            gaps: self.slot_count(),
            index: 0,
            pending: None,
        }
    }

    /// Number of argument slots between pieces: max(pieces − 1, 0).
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.pieces.len().saturating_sub(1)
    }

    /// Whether the entry is a single empty piece (renders as empty text, not a miss).
    #[must_use]
    pub fn is_empty_text(&self) -> bool {
        self.pieces.iter().all(Utf16String::is_empty) && self.arg_ids.is_empty()
    }

    /// Per-entry invariant violations (ST2, ST3, ST4 and the name-group count).
    pub fn issues(&self) -> impl Iterator<Item = StringTableIssue> + '_ {
        let token = self.token;
        [
            (self.token_high != 0).then_some(StringTableIssue::TokenHigh { token }),
            (self.arg_ids.len() != self.slot_count()).then_some(StringTableIssue::SlotCount {
                token,
                pieces: self.pieces.len(),
                args: self.arg_ids.len(),
            }),
            (self.arg_name_groups.len() > 1).then_some(StringTableIssue::NameGroupCount {
                token,
                groups: self.arg_name_groups.len(),
            }),
            (!self.arg_names_match_ids()).then_some(StringTableIssue::ArgNames { token }),
        ]
        .into_iter()
        .flatten()
    }

    /// Variable names for the arguments, if the entry carries a name group.
    #[must_use]
    pub fn arg_names(&self) -> Option<&[Utf16String]> {
        self.arg_name_groups.first().map(Vec::as_slice)
    }

    /// Checks invariant ST4: every stored argument name hashes to its argument ID.
    #[must_use]
    pub fn arg_names_match_ids(&self) -> bool {
        self.arg_name_groups.iter().all(|group| {
            group.len() == self.arg_ids.len()
                && group
                    .iter()
                    .zip(&self.arg_ids)
                    .all(|(name, &id)| name_hash_utf16(name.units()) == Some(id))
        })
    }

    /// Renders the message (spec §3.4): pieces and argument slots alternate, starting with a
    /// piece. `arg` supplies the text for an argument ID; `None` leaves the slot empty.
    ///
    /// If the entry has more arguments than slots between pieces, the extras are appended.
    pub fn render<'s, F>(&self, mut arg: F) -> String
    where
        F: FnMut(u32) -> Option<Cow<'s, str>>,
    {
        let mut out = String::new();
        let mut args = self.arg_ids.iter();
        for (i, piece) in self.pieces.iter().enumerate() {
            if i > 0
                && let Some(&id) = args.next()
                && let Some(text) = arg(id)
            {
                out.push_str(&text);
            }
            out.push_str(&piece.to_string_lossy());
        }
        for &id in args {
            if let Some(text) = arg(id) {
                out.push_str(&text);
            }
        }
        out
    }

    /// The pieces joined without argument values.
    #[must_use]
    pub fn plain_text(&self) -> String {
        self.render(|_| None)
    }
}

/// Iterator over the parts of a message in rendering order; see
/// [`StringTableEntry::parts`]. It allocates nothing.
#[derive(Debug, Clone)]
pub struct MessageParts<'a> {
    pieces: std::slice::Iter<'a, Utf16String>,
    args: std::slice::Iter<'a, u32>,
    /// Number of slots between pieces.
    gaps: usize,
    /// Index of the next piece.
    index: usize,
    /// A piece to yield after the slot just yielded.
    pending: Option<&'a Utf16String>,
}

impl<'a> Iterator for MessageParts<'a> {
    type Item = MessagePart<'a>;

    fn next(&mut self) -> Option<MessagePart<'a>> {
        if let Some(p) = self.pending.take() {
            return Some(MessagePart::Piece(p));
        }
        if let Some(p) = self.pieces.next() {
            let i = self.index;
            self.index += 1;
            if i > 0
                && i <= self.gaps
                && let Some(&id) = self.args.next()
            {
                self.pending = Some(p);
                return Some(MessagePart::Slot(id));
            }
            return Some(MessagePart::Piece(p));
        }
        // Extra argument IDs (ST3 violated) follow the last piece.
        self.args.next().map(|&id| MessagePart::Slot(id))
    }
}

/// A decoded string table (spec §3.1).
///
/// The entry list is read-only so that the token index always agrees with it.
#[derive(Clone, PartialEq, Eq)]
pub struct StringTable {
    /// Table DID.
    pub did: Did,
    /// Table flag at offset 4 (meaning OPEN; observed 0 or 1).
    pub flag: u32,
    /// TSize header of the entry list.
    pub size: TSize,
    entries: Vec<StringTableEntry>,
    index: HashMap<u32, u32>,
}

impl StringTable {
    /// Looks up an entry by token. A missing token is a miss, not an error (ST7).
    #[must_use]
    pub fn get(&self, token: u32) -> Option<&StringTableEntry> {
        self.index
            .get(&token)
            .and_then(|&i| self.entries.get(i as usize))
    }

    /// Entries in stored (bucket) order, including any duplicated tokens.
    #[must_use]
    pub fn entries(&self) -> &[StringTableEntry] {
        &self.entries
    }

    /// Iterates the entries in stored order.
    pub fn iter(&self) -> std::slice::Iter<'_, StringTableEntry> {
        self.entries.iter()
    }

    /// Number of stored entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether the entries are in bucket order for the table's hint (ST5, spec §1.3.1);
    /// `None` for a hint with no known bucket count.
    #[must_use]
    pub fn is_bucket_ordered(&self) -> Option<bool> {
        keys_in_bucket_order(self.size.hint, self.entries.iter().map(|e| e.token))
    }

    /// Every invariant violation in the table (spec §3.5), in entry order, then duplicate
    /// tokens, then bucket order. Empty for every table in observed data.
    #[must_use]
    pub fn issues(&self) -> Vec<StringTableIssue> {
        let mut out: Vec<StringTableIssue> = self
            .entries
            .iter()
            .flat_map(StringTableEntry::issues)
            .collect();
        if self.index.len() != self.entries.len() {
            let mut seen = std::collections::HashSet::with_capacity(self.entries.len());
            for e in &self.entries {
                if !seen.insert(e.token) {
                    out.push(StringTableIssue::DuplicateToken { token: e.token });
                }
            }
        }
        if self.is_bucket_ordered() == Some(false) {
            out.push(StringTableIssue::BucketOrder);
        }
        out
    }

    /// Consumes the table and returns its entries in stored order.
    #[must_use]
    pub fn into_entries(self) -> Vec<StringTableEntry> {
        self.entries
    }
}

impl<'t> IntoIterator for &'t StringTable {
    type Item = &'t StringTableEntry;
    type IntoIter = std::slice::Iter<'t, StringTableEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl fmt::Debug for StringTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StringTable")
            .field("did", &self.did)
            .field("flag", &self.flag)
            .field("size", &self.size)
            .field("entries", &self.entries)
            .finish_non_exhaustive()
    }
}

impl Record<'_> for StringTable {
    const NAME: &'static str = "string table";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        // Only 0x25 is a confirmed string-table prefix (spec §3.1); 0x26 is Reported only.
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::StringTable)?;
        let flag = r.u32()?;
        let size = r.tsize()?;
        // Smallest entry: token + token_high + piece_count + arg_count + group byte = 17.
        let mut entries = r.alloc_vec(size.count, 17, PREALLOC_CAP)?;
        for _ in 0..size.count {
            let entry = StringTableEntry::read(&mut r)?;
            r.push(&mut entries, entry, size.count)?;
        }
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        // The index is built once at its final size, so it never grows by doubling.
        r.charge(hash_index_bytes::<u32, u32>(entries.len()))?;
        let mut index = HashMap::with_capacity(entries.len());
        // Anomalies are counted and logged once per table, so hostile input cannot flood
        // the log with one event per entry.
        let mut duplicates = 0usize;
        let mut first_duplicate = None;
        let mut high_tokens = 0usize;
        for (i, entry) in entries.iter().enumerate() {
            if entry.token_high != 0 {
                high_tokens += 1;
            }
            // Keep the first occurrence of a duplicated token (ST5 says there are none).
            if let std::collections::hash_map::Entry::Vacant(v) = index.entry(entry.token) {
                v.insert(index_u32(i));
            } else {
                duplicates += 1;
                first_duplicate.get_or_insert(entry.token);
            }
        }
        if let Some(token) = first_duplicate {
            tracing::warn!(%did, duplicates, first = token, "duplicate string-table tokens");
        }
        if high_tokens != 0 {
            tracing::debug!(%did, count = high_tokens, "string-table entries with non-zero token_high");
        }
        Ok(Self {
            did,
            flag,
            size,
            entries,
            index,
        })
    }
}
