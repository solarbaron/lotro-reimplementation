// SPDX-License-Identifier: GPL-3.0-or-later
//! Localized string tables, DID prefix 0x25 (spec §3).

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

use crate::did::{Did, DidKind, name_hash_utf16};
use crate::error::Result;
use crate::reader::{Reader, TSize, Utf16String};
use crate::record::{Record, read_prologue};

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
        let cap = r.ensure_elements(piece_count, 1)?;
        let mut pieces = Vec::with_capacity(cap);
        for _ in 0..piece_count {
            pieces.push(r.utf16_string()?);
        }
        let arg_count = r.u32()?;
        let cap = r.ensure_elements(arg_count, 4)?;
        let mut arg_ids = Vec::with_capacity(cap);
        for _ in 0..arg_count {
            arg_ids.push(r.u32()?);
        }
        let group_count = r.u8()?;
        let mut arg_name_groups = Vec::with_capacity(usize::from(group_count).min(r.remaining()));
        for _ in 0..group_count {
            let name_count = r.u32()?;
            let cap = r.ensure_elements(name_count, 1)?;
            let mut names = Vec::with_capacity(cap);
            for _ in 0..name_count {
                names.push(r.utf16_string()?);
            }
            arg_name_groups.push(names);
        }
        Ok(Self {
            token,
            token_high,
            pieces,
            arg_ids,
            arg_name_groups,
        })
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
    index: HashMap<u32, usize>,
}

impl StringTable {
    /// Looks up an entry by token. A missing token is a miss, not an error (ST7).
    #[must_use]
    pub fn get(&self, token: u32) -> Option<&StringTableEntry> {
        self.index.get(&token).and_then(|&i| self.entries.get(i))
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
        let cap = r.ensure_elements(size.count, 17)?;
        let mut entries = Vec::with_capacity(cap);
        let mut index = HashMap::with_capacity(cap);
        // Anomalies are counted and logged once per table, so hostile input cannot flood
        // the log with one event per entry.
        let mut duplicates = 0usize;
        let mut first_duplicate = None;
        let mut high_tokens = 0usize;
        for i in 0..size.count as usize {
            let entry = StringTableEntry::read(&mut r)?;
            if entry.token_high != 0 {
                high_tokens += 1;
            }
            // Keep the first occurrence of a duplicated token (ST5 says there are none).
            if let std::collections::hash_map::Entry::Vacant(v) = index.entry(entry.token) {
                v.insert(i);
            } else {
                duplicates += 1;
                first_duplicate.get_or_insert(entry.token);
            }
            entries.push(entry);
        }
        r.expect_end(Self::NAME)?;
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
