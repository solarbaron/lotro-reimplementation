// SPDX-License-Identifier: GPL-3.0-or-later
//! The sky description (prefix 0x15, DB type 0x3F SKY_DESC; `environment.md` §5.1).
//!
//! One record (0x15000007 in the observed install, named by slot 4 of every region
//! descriptor) maps sky keys (`Environment_LocalSky` values, §7.1) to weighted lists of day
//! descriptions (prefix 0x1A), per day type (the day groups) and as a fallback (the local
//! default table). [`SkyDescription::sky_entry`] performs the look-up of §11.2 steps 1–2 and
//! [`SkyDescription::resolve_sky_list`] follows a named list reference; the weighted pick
//! itself (§11.2 step 4) is left to the consumer.

use super::put;
use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::{PREALLOC_CAP, Reader};
use crate::record::{Record, read_prologue};

/// Type prefix of sky descriptions.
pub const SKY_DESCRIPTION_PREFIX: u8 = 0x15;

/// The only `version` a reader accepts (`environment.md` §5.1.1, §13).
pub const SKY_DESCRIPTION_VERSION: u32 = 1;

/// Smallest stored weighted-list body: its count (`environment.md` §5.1.3).
const MIN_BODY_LEN: usize = 4;
/// Smallest stored list reference: a named key (4 bytes) or an inline key and empty body (8).
const MIN_LIST_REF_LEN: usize = 4;
/// Smallest stored sky entry: key, flag, named reference.
const MIN_SKY_ENTRY_LEN: usize = 4 + 1 + MIN_LIST_REF_LEN;
/// Smallest stored day group: day type, chance, flag, named follow reference, entry count.
const MIN_DAY_GROUP_LEN: usize = 4 + 4 + 1 + MIN_LIST_REF_LEN + 4;
/// Smallest stored named list: key and an empty body.
const MIN_NAMED_LIST_LEN: usize = 4 + MIN_BODY_LEN;

/// One (value, weight) pair of a weighted list (`environment.md` §5.1.3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Weighted<T> {
    /// The value: a day type in follow lists, a day-description DID in sky lists.
    pub value: T,
    /// Relative weight (kept as stored; may be any float in hostile data).
    pub weight: f32,
}

/// A keyed weighted list (`environment.md` §5.1.3, §5.1.4): an inline list (key 0 in the
/// observed install) or a named list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WeightedList<T> {
    /// The list's key (the name of a named list; 0 in every inline list observed).
    pub key: u32,
    /// Pairs in stored order.
    pub pairs: Vec<Weighted<T>>,
}

impl<T: Copy> WeightedList<T> {
    /// Sum of the weights in stored order (`environment.md` §11.2 step 4, *W*).
    #[must_use]
    pub fn total_weight(&self) -> f32 {
        self.pairs.iter().map(|p| p.weight).sum()
    }

    /// The values in stored order.
    pub fn values(&self) -> impl Iterator<Item = T> + '_ {
        self.pairs.iter().map(|p| p.value)
    }
}

/// A weighted list stored inline or named by key (`environment.md` §5.1.2 field 4, §5.1.3
/// field 3).
#[derive(Debug, Clone, PartialEq)]
pub enum ListRef<T> {
    /// Flag 1: the key of a named list (§5.1.4).
    Named(u32),
    /// Flag 0: the list itself.
    Inline(WeightedList<T>),
}

impl<T> Default for ListRef<T> {
    fn default() -> Self {
        Self::Inline(WeightedList {
            key: 0,
            pairs: Vec::new(),
        })
    }
}

/// One sky entry: a sky key and the weighted list of day descriptions it selects
/// (`environment.md` §5.1.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SkyEntry {
    /// An `Environment_LocalSky` value (§7.1).
    pub sky_key: u32,
    /// Weighted list of day-description DIDs (prefix 0x1A).
    pub list: ListRef<Did>,
}

/// A day group: the sky entries used on days of one day type (`environment.md` §5.1.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DayGroup {
    /// A day-type value (enum mapper 0x230000E8 in the observed install).
    pub day_type: u32,
    /// Relative frequency of this day type.
    pub chance: f32,
    /// Weights of the next day's type (Inferred meaning).
    pub follow: ListRef<u32>,
    /// Sky entries in stored order.
    pub entries: Vec<SkyEntry>,
}

impl DayGroup {
    /// The first entry with `sky_key` (stored order, §11.2 step 1).
    #[must_use]
    pub fn entry(&self, sky_key: u32) -> Option<&SkyEntry> {
        self.entries.iter().find(|e| e.sky_key == sky_key)
    }
}

/// The sky description (prefix 0x15; `environment.md` §5.1).
///
/// [`Default`] gives an empty description with `version` [`SKY_DESCRIPTION_VERSION`], so a
/// description built as `SkyDescription { did, ..Default::default() }` encodes to bytes
/// that decode again.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyDescription {
    /// Stored `version`; always [`SKY_DESCRIPTION_VERSION`] in a decoded record and in
    /// [`Default`]. [`SkyDescription::to_bytes`] writes it as it is: any other value
    /// produces bytes that the decoder rejects (`environment.md` §5.1.1, §13).
    pub version: u32,
    /// Own DID.
    pub did: Did,
    /// Lower bound of the lighting update interval, game-clock seconds (§5.1.1).
    pub light_interval_min: f64,
    /// Upper bound of the lighting update interval, game-clock seconds.
    pub light_interval_max: f64,
    /// Lower bound of the sky re-check interval, game-clock seconds.
    pub sky_interval_min: f64,
    /// Upper bound of the sky re-check interval, game-clock seconds.
    pub sky_interval_max: f64,
    /// The game-time record (prefix 0x35, §6).
    pub game_time: Did,
    /// Day groups in stored order.
    pub groups: Vec<DayGroup>,
    /// Named follow lists (§5.1.4), keyed by [`WeightedList::key`].
    pub named_follow_lists: Vec<WeightedList<u32>>,
    /// Named sky lists (§5.1.4), keyed by [`WeightedList::key`].
    pub named_sky_lists: Vec<WeightedList<Did>>,
    /// The local default table: sky entries used when no day group applies.
    pub local_defaults: Vec<SkyEntry>,
}

impl Default for SkyDescription {
    fn default() -> Self {
        Self {
            version: SKY_DESCRIPTION_VERSION,
            did: Did::default(),
            light_interval_min: 0.0,
            light_interval_max: 0.0,
            sky_interval_min: 0.0,
            sky_interval_max: 0.0,
            game_time: Did::default(),
            groups: Vec::new(),
            named_follow_lists: Vec::new(),
            named_sky_lists: Vec::new(),
            local_defaults: Vec::new(),
        }
    }
}

impl SkyDescription {
    /// The first day group with `day_type` (stored order).
    #[must_use]
    pub fn group(&self, day_type: u32) -> Option<&DayGroup> {
        self.groups.iter().find(|g| g.day_type == day_type)
    }

    /// The first local default entry with `sky_key` (stored order, §11.2 step 2).
    #[must_use]
    pub fn local_default(&self, sky_key: u32) -> Option<&SkyEntry> {
        self.local_defaults.iter().find(|e| e.sky_key == sky_key)
    }

    /// The sky entry for sky key `sky_key` on a day of type `day_type`
    /// (`environment.md` §11.2 steps 1–2): the first day group with that day type, if it has
    /// an entry for the key, else the local default table. `None` if neither has one (§11.2
    /// step 3).
    #[must_use]
    pub fn sky_entry(&self, day_type: u32, sky_key: u32) -> Option<&SkyEntry> {
        self.group(day_type)
            .and_then(|g| g.entry(sky_key))
            .or_else(|| self.local_default(sky_key))
    }

    /// The first named sky list with `key`.
    #[must_use]
    pub fn named_sky_list(&self, key: u32) -> Option<&WeightedList<Did>> {
        self.named_sky_lists.iter().find(|l| l.key == key)
    }

    /// The first named follow list with `key`.
    #[must_use]
    pub fn named_follow_list(&self, key: u32) -> Option<&WeightedList<u32>> {
        self.named_follow_lists.iter().find(|l| l.key == key)
    }

    /// The list a sky-list reference denotes; `None` for a named reference whose key has no
    /// named sky list.
    #[must_use]
    pub fn resolve_sky_list<'s>(&'s self, r: &'s ListRef<Did>) -> Option<&'s WeightedList<Did>> {
        match r {
            ListRef::Inline(l) => Some(l),
            ListRef::Named(k) => self.named_sky_list(*k),
        }
    }

    /// The list a follow-list reference denotes; `None` for a named reference whose key has
    /// no named follow list.
    #[must_use]
    pub fn resolve_follow_list<'s>(&'s self, r: &'s ListRef<u32>) -> Option<&'s WeightedList<u32>> {
        match r {
            ListRef::Inline(l) => Some(l),
            ListRef::Named(k) => self.named_follow_list(*k),
        }
    }

    /// Every day-description DID stored in the record (inline lists of the day groups and
    /// the local default table, then the named sky lists), with repeats, in stored order.
    pub fn day_descriptions(&self) -> impl Iterator<Item = Did> + '_ {
        fn inline(e: &SkyEntry) -> &[Weighted<Did>] {
            match &e.list {
                ListRef::Inline(l) => &l.pairs,
                ListRef::Named(_) => &[],
            }
        }
        self.groups
            .iter()
            .flat_map(|g| g.entries.iter())
            .chain(self.local_defaults.iter())
            .flat_map(|e| inline(e).iter().map(|p| p.value))
            .chain(
                self.named_sky_lists
                    .iter()
                    .flat_map(|l| l.pairs.iter().map(|p| p.value)),
            )
    }

    /// Re-encodes the record (`environment.md` §5.1). Byte-exact for a decoded record except
    /// that the two named-list counts are written as shortest VLEs.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put::u32(&mut out, self.version);
        put::did(&mut out, self.did);
        put::f64(&mut out, self.light_interval_min);
        put::f64(&mut out, self.light_interval_max);
        put::f64(&mut out, self.sky_interval_min);
        put::f64(&mut out, self.sky_interval_max);
        put::did(&mut out, self.game_time);
        put::count(&mut out, self.groups.len());
        for g in &self.groups {
            put::u32(&mut out, g.day_type);
            put::f32(&mut out, g.chance);
            write_list_ref(&mut out, &g.follow, |v| v);
            write_entries(&mut out, &g.entries);
        }
        put::vle_count(&mut out, self.named_follow_lists.len());
        for l in &self.named_follow_lists {
            write_list(&mut out, l, |v| v);
        }
        put::vle_count(&mut out, self.named_sky_lists.len());
        for l in &self.named_sky_lists {
            write_list(&mut out, l, |v: Did| v.0);
        }
        write_entries(&mut out, &self.local_defaults);
        out
    }
}

impl Record<'_> for SkyDescription {
    const NAME: &'static str = "sky description";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let version_at = r.position();
        let version = r.u32()?;
        if version != SKY_DESCRIPTION_VERSION {
            return Err(FormatError::Unsupported {
                what: "sky description version",
                offset: version_at,
                value: version,
            });
        }
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::SkyDescription)?;
        let light_interval_min = r.f64()?;
        let light_interval_max = r.f64()?;
        let sky_interval_min = r.f64()?;
        let sky_interval_max = r.f64()?;
        let game_time = r.did()?;

        let group_count = r.u32()?;
        let mut groups = r.alloc_vec(group_count, MIN_DAY_GROUP_LEN, PREALLOC_CAP)?;
        for _ in 0..group_count {
            let day_type = r.u32()?;
            let chance = r.f32()?;
            let follow = read_list_ref(&mut r, |v| v)?;
            let entries = read_entries(&mut r)?;
            let g = DayGroup {
                day_type,
                chance,
                follow,
                entries,
            };
            r.push(&mut groups, g, group_count)?;
        }

        let named_follow_lists = read_named_lists(&mut r, |v| v)?;
        let named_sky_lists = read_named_lists(&mut r, Did)?;
        let local_defaults = read_entries(&mut r)?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            version,
            did,
            light_interval_min,
            light_interval_max,
            sky_interval_min,
            sky_interval_max,
            game_time,
            groups,
            named_follow_lists,
            named_sky_lists,
            local_defaults,
        })
    }
}

/// Reads a weighted-list body: u32 count, then (u32 value, f32 weight) pairs.
fn read_body<T>(r: &mut Reader<'_>, conv: fn(u32) -> T) -> Result<Vec<Weighted<T>>> {
    let count = r.u32()?;
    let mut pairs = r.alloc_vec(count, 8, PREALLOC_CAP)?;
    for _ in 0..count {
        let value = conv(r.u32()?);
        let weight = r.f32()?;
        r.push(&mut pairs, Weighted { value, weight }, count)?;
    }
    Ok(pairs)
}

/// Reads a keyed list: u32 key and a body.
fn read_list<T>(r: &mut Reader<'_>, conv: fn(u32) -> T) -> Result<WeightedList<T>> {
    let key = r.u32()?;
    let pairs = read_body(r, conv)?;
    Ok(WeightedList { key, pairs })
}

/// Reads a bool8 flag, then a named key (1) or an inline keyed list (0).
fn read_list_ref<T>(r: &mut Reader<'_>, conv: fn(u32) -> T) -> Result<ListRef<T>> {
    if r.bool8()? {
        Ok(ListRef::Named(r.u32()?))
    } else {
        read_list(r, conv).map(ListRef::Inline)
    }
}

/// Reads a u32 count and that many sky entries.
fn read_entries(r: &mut Reader<'_>) -> Result<Vec<SkyEntry>> {
    let count = r.u32()?;
    let mut entries = r.alloc_vec(count, MIN_SKY_ENTRY_LEN, PREALLOC_CAP)?;
    for _ in 0..count {
        let sky_key = r.u32()?;
        let list = read_list_ref(r, Did)?;
        r.push(&mut entries, SkyEntry { sky_key, list }, count)?;
    }
    Ok(entries)
}

/// Reads a VLE count and that many named lists (§5.1.4).
fn read_named_lists<T>(r: &mut Reader<'_>, conv: fn(u32) -> T) -> Result<Vec<WeightedList<T>>> {
    let count = r.vle()?;
    let mut lists = r.alloc_vec(count, MIN_NAMED_LIST_LEN, PREALLOC_CAP)?;
    for _ in 0..count {
        let l = read_list(r, conv)?;
        r.push(&mut lists, l, count)?;
    }
    Ok(lists)
}

fn write_list<T: Copy>(out: &mut Vec<u8>, l: &WeightedList<T>, raw: fn(T) -> u32) {
    put::u32(out, l.key);
    put::count(out, l.pairs.len());
    for p in &l.pairs {
        put::u32(out, raw(p.value));
        put::f32(out, p.weight);
    }
}

fn write_list_ref<T: Copy>(out: &mut Vec<u8>, r: &ListRef<T>, raw: fn(T) -> u32) {
    match r {
        ListRef::Named(k) => {
            put::bool8(out, true);
            put::u32(out, *k);
        }
        ListRef::Inline(l) => {
            put::bool8(out, false);
            write_list(out, l, raw);
        }
    }
}

fn write_entries(out: &mut Vec<u8>, entries: &[SkyEntry]) {
    put::count(out, entries.len());
    for e in entries {
        put::u32(out, e.sky_key);
        write_list_ref(out, &e.list, |d: Did| d.0);
    }
}

/// Decodes a sky description payload (for fuzz targets and tools).
pub fn decode_sky_description(data: &[u8]) -> Result<SkyDescription> {
    SkyDescription::parse(data)
}
