// SPDX-License-Identifier: GPL-3.0-or-later
//! Enum mappers, DID prefix 0x23 (spec §7).

use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::did::{Did, DidKind};
use crate::error::Result;
use crate::reader::{PREALLOC_CAP, Reader, TSize, hash_index_bytes, index_u32};
use crate::record::{Record, read_prologue};
use crate::string_info::StringInfo;

/// Maximum length of a base chain followed by [`EnumMapper::resolve`].
pub const ENUM_BASE_CHAIN_LIMIT: usize = 32;

/// An enum mapper: symbolic names and localized display text per key.
///
/// The decoded lists are read-only so that the key indexes always agree with them.
#[derive(Clone, PartialEq)]
pub struct EnumMapper {
    /// Record DID.
    pub did: Did,
    /// Parent mapper whose entries are inherited, or the null DID.
    pub base: Did,
    /// TSize header of the name list.
    pub names_size: TSize,
    /// TSize header of the display list.
    pub displays_size: TSize,
    names: Vec<(u32, String)>,
    displays: Vec<(u32, StringInfo)>,
    name_index: HashMap<u32, u32>,
    display_index: HashMap<u32, u32>,
}

impl fmt::Debug for EnumMapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnumMapper")
            .field("did", &self.did)
            .field("base", &self.base)
            .field("names_size", &self.names_size)
            .field("names", &self.names)
            .field("displays_size", &self.displays_size)
            .field("displays", &self.displays)
            .finish_non_exhaustive()
    }
}

impl EnumMapper {
    /// Key → internal symbolic name, in stored order.
    #[must_use]
    pub fn names(&self) -> &[(u32, String)] {
        &self.names
    }

    /// Key → display text, in stored order.
    #[must_use]
    pub fn displays(&self) -> &[(u32, StringInfo)] {
        &self.displays
    }

    /// Symbolic name of a key in this mapper only.
    #[must_use]
    pub fn name(&self, key: u32) -> Option<&str> {
        self.name_index
            .get(&key)
            .and_then(|&i| self.names.get(i as usize))
            .map(|(_, n)| n.as_str())
    }

    /// The key whose symbolic name is `name`, in this mapper only (the first stored one if
    /// several keys share it). A linear search: labels are looked up once and cached by
    /// callers.
    #[must_use]
    pub fn key_of(&self, name: &str) -> Option<u32> {
        self.names.iter().find(|(_, n)| n == name).map(|(k, _)| *k)
    }

    /// The key named `name` in this mapper or its base chain. `fetch` loads a base mapper
    /// by DID and may return it owned, borrowed or shared (anything that borrows as an
    /// [`EnumMapper`]), so that callers which load mappers on demand need no cache.
    /// Cycles and chains longer than [`ENUM_BASE_CHAIN_LIMIT`] (this mapper included) end
    /// the search with `None`, as in [`EnumMapper::resolve`].
    pub fn key_in_chain<M: Borrow<EnumMapper>>(
        &self,
        fetch: impl FnMut(Did) -> Option<M>,
        name: &str,
    ) -> Option<u32> {
        if let Some(k) = self.key_of(name) {
            return Some(k);
        }
        Self::chain_lookup(self.base, Some(self.did), 1, fetch, name)
    }

    /// [`EnumMapper::key_in_chain`] starting from the mapper `start` (for example the one a
    /// property definition names), which `fetch` loads first. `None` for the null DID.
    pub fn key_in_chain_from<M: Borrow<EnumMapper>>(
        start: Did,
        fetch: impl FnMut(Did) -> Option<M>,
        name: &str,
    ) -> Option<u32> {
        Self::chain_lookup(start, None, 0, fetch, name)
    }

    fn chain_lookup<M: Borrow<EnumMapper>>(
        mut did: Did,
        visited: Option<Did>,
        used: usize,
        mut fetch: impl FnMut(Did) -> Option<M>,
        name: &str,
    ) -> Option<u32> {
        let mut seen: HashSet<Did> = visited.into_iter().collect();
        for _ in used..ENUM_BASE_CHAIN_LIMIT {
            if did.is_null() {
                return None;
            }
            if !seen.insert(did) {
                tracing::warn!(%did, "enum mapper base chain has a cycle");
                return None;
            }
            let m = fetch(did)?;
            let m = m.borrow();
            if let Some(k) = m.key_of(name) {
                return Some(k);
            }
            did = m.base;
        }
        tracing::warn!(%did, "enum mapper base chain too long");
        None
    }

    /// Display text of a key in this mapper only.
    #[must_use]
    pub fn display(&self, key: u32) -> Option<&StringInfo> {
        self.display_index
            .get(&key)
            .and_then(|&i| self.displays.get(i as usize))
            .map(|(_, d)| d)
    }

    /// Walks this mapper and its base chain, returning the first mapper for which `pick`
    /// yields a value. `fetch` loads a base mapper by DID. Cycles and chains longer than
    /// [`ENUM_BASE_CHAIN_LIMIT`] end the search.
    pub fn resolve<'m, T: ?Sized + 'm>(
        &'m self,
        mut fetch: impl FnMut(Did) -> Option<&'m EnumMapper>,
        pick: impl Fn(&'m EnumMapper) -> Option<&'m T>,
    ) -> Option<&'m T> {
        let mut seen = HashSet::new();
        let mut current = self;
        for _ in 0..ENUM_BASE_CHAIN_LIMIT {
            if !seen.insert(current.did) {
                tracing::warn!(did = %current.did, "enum mapper base chain has a cycle");
                return None;
            }
            if let Some(v) = pick(current) {
                return Some(v);
            }
            if current.base.is_null() {
                return None;
            }
            current = fetch(current.base)?;
        }
        tracing::warn!(did = %self.did, "enum mapper base chain too long");
        None
    }
}

impl Record<'_> for EnumMapper {
    const NAME: &'static str = "enum mapper";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::EnumMapper)?;
        let base = r.did()?;
        let names_size = r.tsize()?;
        let mut names = r.alloc_vec(names_size.count, 5, PREALLOC_CAP)?;
        for _ in 0..names_size.count {
            let key = r.u32()?;
            let name = r.pascal_string()?;
            r.push(&mut names, (key, name), names_size.count)?;
        }
        let displays_size = r.tsize()?;
        // Smallest StringInfo is an empty literal: bool8 + VLE 0 + bool8 + terminator = 5
        // bytes, so a display entry (u32 key + StringInfo) is at least 9 bytes.
        let mut displays = r.alloc_vec(displays_size.count, 9, PREALLOC_CAP)?;
        for _ in 0..displays_size.count {
            let key = r.u32()?;
            let info = StringInfo::read(&mut r)?;
            r.push(&mut displays, (key, info), displays_size.count)?;
        }
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        // Indexes are built once at their final size, so they never grow by doubling.
        r.charge(
            hash_index_bytes::<u32, u32>(names.len())
                .saturating_add(hash_index_bytes::<u32, u32>(displays.len())),
        )?;
        let mut name_index = HashMap::with_capacity(names.len());
        for (i, (key, _)) in names.iter().enumerate() {
            name_index.entry(*key).or_insert(index_u32(i));
        }
        let mut display_index = HashMap::with_capacity(displays.len());
        for (i, (key, _)) in displays.iter().enumerate() {
            display_index.entry(*key).or_insert(index_u32(i));
        }
        Ok(Self {
            did,
            base,
            names_size,
            names,
            displays_size,
            displays,
            name_index,
            display_index,
        })
    }
}
