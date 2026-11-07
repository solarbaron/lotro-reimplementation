// SPDX-License-Identifier: GPL-3.0-or-later
//! The restricted resource-set report `0x13D` of decision 0046 item 13 (`official-session.md`
//! §9.3.12 "Resource-set report source boundary"): pure functions over the install data the client
//! read, so the rule can be tested and fuzzed without an archive.
//!
//! # The rule
//!
//! Item 13 lets a user past the warning and consent gate send ONE install-resolved default set in
//! both fields of `0x13D`, after the first complete character list and before the selection. It is a
//! labelled, revocable owner exception ([`ITEM_13_RESOURCE_REPORT`]), not a claim that the server
//! needs the report. Everything comes from the user's install at run time: no label, key, DID or
//! mask is built in. Only the routing constant and the dictionary property IDs with `BIN`
//! provenance in the spec appear here.
//!
//! 1. Route (the client, [`InstallData::resource_sets`]): the master DID map from the superblock,
//!    root key `7`, sub-key [`SUB_KEY_RESOURCE_SETS`], both map groups; the target and every base
//!    must be an enum mapper (never a DID map read as one).
//! 2. Chain ([`resolve_resource_sets`]): at most [`RESOURCE_SET_CHAIN_LIMIT`] records, no repeated
//!    DID, a zero final base.
//! 3. Names: a record repeating a key in its names list is malformed; the child's name shadows its
//!    base's; only names, never display text. The ultimate base must have exactly one entry, with a
//!    name and a non-zero key: that is the default candidate. Its effective name must equal its base
//!    label up to ASCII case.
//! 4. Matching: ASCII case-insensitive, no localisation or whitespace normalisation; the label must
//!    be ASCII, and the same folded label at two effective keys is ambiguous.
//! 5. Configuration ([`resource_set_config`]): no list is the single built-in default; a supplied
//!    list splits on commas with only ASCII space and tab trimmed at token ends; an empty, non-ASCII
//!    or unmatched token, more than one token (even repeats) or another set omits.
//! 6. Dictionary ([`check_dictionary`]): both report properties must be Type 22 with the route's
//!    mapper as their data reference; never their printed DID as a fallback.
//!
//! Then the minimum mount check ([`check_mounted`]): at least one mounted base archive, and every
//! base's archive-header variant equal to the key minus one. Any failure omits the WHOLE report and
//! keeps the login going (item 5); a key outside `1..=4096` gives no report, never a clamped one.
//!
//! # Labelled choices (OPEN for the writer)
//!
//! - Tessera has no resource-set launch option and no sourced launcher key, so the effective
//!   configuration is always the built-in default ([`EFFECTIVE_RESOURCE_SET_OPTION`]); the
//!   tokeniser is exercised on synthetic input only.
//! - "Exactly one positive named entry" is read as: the ultimate base's names list has exactly one
//!   entry, its name is not empty and its key is not zero; a key-0 or unnamed entry makes it not
//!   unique rather than being ignored.
//! - A repeated key counts in the names list only; display text is never read.
//! - The mounted bases are every base archive of the `0xEC` inventory snapshot (portal, region,
//!   language and, when mounted, `highres`).
//!
//! [`InstallData::resource_sets`]: crate::connector::InstallData::resource_sets

use std::collections::{BTreeMap, HashSet};

use crate::initial::AdvertisedArchive;
use crate::world::{RESOURCE_SET_MAX_BITS, ResourceSetMask, ResourceSetReport};

/// Decision 0046 item 13 (owner, 2026-11-06): the restricted `0x13D` candidate may be sent. A
/// labelled, revocable switch: `false` restores the item-5 omission for every login
/// ([`ReportOmission::Withdrawn`]). It ends when the writer settles OPEN-SESSION-20 or the owner
/// withdraws it.
pub const ITEM_13_RESOURCE_REPORT: bool = true;

/// The report's opcode (`official-session.md` §5.6.2, §9.3.12).
pub const OPCODE_RESOURCE_SET_REPORT: u32 = ResourceSetReport::EVENT_ID;

/// The explicit protocol destination of `0x13D` (`official-session.md` §9.3.12, BIN-L2-10L-SETS): a
/// built-in routing constant, not a captured resource, account or install value. Always in the
/// explicit eight-byte recipient form.
pub const RESOURCE_REPORT_DESTINATION: u64 = 0x0600_0000_0000_0403;

/// The sub-key of the ResourceSets enum under root key `7` of the master DID map
/// (`official-session.md` §9.3.12 step 1).
pub const SUB_KEY_RESOURCE_SETS: u32 = 0x8C;

/// Most enum-mapper records followed from the route's target through its bases (§9.3.12 step 2).
pub const RESOURCE_SET_CHAIN_LIMIT: usize = 16;

/// The available-resource-sets property (`official-messages-world.md` §3.1.3), checked against the
/// route in step 6.
pub const PID_AVAILABLE_RESOURCE_SETS: u32 = 0x0000_07C4;

/// The loaded-resource-sets property (`official-messages-world.md` §3.1.3), checked as
/// [`PID_AVAILABLE_RESOURCE_SETS`].
pub const PID_LOADED_RESOURCE_SETS: u32 = 0x0000_07C8;

/// The Type 22 code (variable-width bitfield, `official-messages-world.md` §2.7.1) both report
/// properties must have.
pub const TYPE_BITFIELD: u32 = 22;

/// Most effective names kept across the chain. Engineering choice (spec silent): the checked
/// install has two; more is refused as a decode error rather than kept without bound.
pub const MAX_RESOURCE_SET_NAMES: usize = 4096;

/// The resource-set list of this Tessera launch: Tessera has no such option and no launcher key is
/// sourced (§9.3.12 step 5), so it is always absent and the built-in default applies. Labelled
/// choice, OPEN for the writer.
pub const EFFECTIVE_RESOURCE_SET_OPTION: Option<&str> = None;

/// Why the report is omitted: a kind only, never a key, label, mask or DID. Each one omits the whole
/// `0x13D` under decision 0046 item 5 and never ends the login.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportOmission {
    /// [`ITEM_13_RESOURCE_REPORT`] is off.
    Withdrawn,
    /// The install data did not read the resource sets (the trait's default).
    NotRead,
    /// The master map, root key `7`, the sub-key or a mapper record is missing.
    RouteMissing,
    /// The route's target or a base link is not an enum mapper.
    WrongFamily,
    /// A mapper record failed to decode, or the chain has too many names.
    DecodeError,
    /// One record's names list repeats a key.
    DuplicateKey,
    /// A repeated DID or more than [`RESOURCE_SET_CHAIN_LIMIT`] records.
    CycleOrDepth,
    /// The ultimate base has no single named entry.
    BaseDefaultNotUnique,
    /// The default entry's key is zero.
    ZeroKey,
    /// The default key's effective name differs from its base label other than by ASCII case.
    LabelChanged,
    /// The default label is not ASCII.
    NonAsciiLabel,
    /// The same folded label names two effective keys.
    AmbiguousLabel,
    /// The supplied list has an empty token.
    ConfigEmpty,
    /// The supplied list has a non-ASCII token.
    ConfigNonAscii,
    /// The supplied token names no set.
    ConfigUnmatched,
    /// The supplied list has more than one token.
    ConfigMultiple,
    /// The supplied token names another set than the default.
    ConfigOtherSet,
    /// A report property has no definition in the dictionary.
    DictionaryMissing,
    /// A report property is not Type 22.
    DictionaryType,
    /// A report property names another mapper than the route's.
    DictionaryMismatch,
    /// No mounted base archive.
    NoMountedBase,
    /// A mounted base archive's variant is not the key minus one.
    VariantMismatch,
    /// The key is outside `1..=RESOURCE_SET_MAX_BITS`.
    KeyOutOfRange,
    /// The report could not be encoded.
    Encoding,
    /// The report could not be queued.
    Enqueue,
}

impl ReportOmission {
    /// A neutral label for a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Withdrawn => "withdrawn",
            Self::NotRead => "not read",
            Self::RouteMissing => "route missing",
            Self::WrongFamily => "wrong family",
            Self::DecodeError => "decode error",
            Self::DuplicateKey => "duplicate key",
            Self::CycleOrDepth => "cycle or depth",
            Self::BaseDefaultNotUnique => "base default not unique",
            Self::ZeroKey => "zero key",
            Self::LabelChanged => "label changed",
            Self::NonAsciiLabel => "non-ASCII label",
            Self::AmbiguousLabel => "ambiguous label",
            Self::ConfigEmpty => "configuration empty token",
            Self::ConfigNonAscii => "configuration non-ASCII token",
            Self::ConfigUnmatched => "configuration unmatched token",
            Self::ConfigMultiple => "configuration multiple sets",
            Self::ConfigOtherSet => "configuration other set",
            Self::DictionaryMissing => "dictionary missing",
            Self::DictionaryType => "dictionary type",
            Self::DictionaryMismatch => "dictionary mismatch",
            Self::NoMountedBase => "no mounted base",
            Self::VariantMismatch => "variant mismatch",
            Self::KeyOutOfRange => "key out of range",
            Self::Encoding => "encoding failure",
            Self::Enqueue => "enqueue failure",
        }
    }

    /// The `withholding_reason` of the evidence log and the withheld list
    /// (`official-session.md` §9.3.11): decision 0046 item 5 with the item-13 kind.
    #[must_use]
    pub const fn withholding_reason(self) -> &'static str {
        match self {
            Self::Withdrawn => "decision 0046 item 5; item 13 withdrawn",
            Self::NotRead => "decision 0046 item 5; item 13 not read",
            Self::RouteMissing => "decision 0046 item 5; item 13 route missing",
            Self::WrongFamily => "decision 0046 item 5; item 13 wrong family",
            Self::DecodeError => "decision 0046 item 5; item 13 decode error",
            Self::DuplicateKey => "decision 0046 item 5; item 13 duplicate key",
            Self::CycleOrDepth => "decision 0046 item 5; item 13 cycle or depth",
            Self::BaseDefaultNotUnique => "decision 0046 item 5; item 13 base default not unique",
            Self::ZeroKey => "decision 0046 item 5; item 13 zero key",
            Self::LabelChanged => "decision 0046 item 5; item 13 label changed",
            Self::NonAsciiLabel => "decision 0046 item 5; item 13 non-ASCII label",
            Self::AmbiguousLabel => "decision 0046 item 5; item 13 ambiguous label",
            Self::ConfigEmpty => "decision 0046 item 5; item 13 configuration empty token",
            Self::ConfigNonAscii => "decision 0046 item 5; item 13 configuration non-ASCII token",
            Self::ConfigUnmatched => "decision 0046 item 5; item 13 configuration unmatched token",
            Self::ConfigMultiple => "decision 0046 item 5; item 13 configuration multiple sets",
            Self::ConfigOtherSet => "decision 0046 item 5; item 13 configuration other set",
            Self::DictionaryMissing => "decision 0046 item 5; item 13 dictionary missing",
            Self::DictionaryType => "decision 0046 item 5; item 13 dictionary type",
            Self::DictionaryMismatch => "decision 0046 item 5; item 13 dictionary mismatch",
            Self::NoMountedBase => "decision 0046 item 5; item 13 no mounted base",
            Self::VariantMismatch => "decision 0046 item 5; item 13 variant mismatch",
            Self::KeyOutOfRange => "decision 0046 item 5; item 13 key out of range",
            Self::Encoding => "decision 0046 item 5; item 13 encoding failure",
            Self::Enqueue => "decision 0046 item 5; item 13 enqueue failure",
        }
    }
}

/// One enum-mapper record as the resolver reads it: its base link (`0`: none) and its names list in
/// stored order (`records.md` §7). Display text is never part of it.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct MapperNames {
    /// The base mapper's DID, `0` for none.
    pub base: u32,
    /// `(key, name)` in stored order.
    pub names: Vec<(u32, String)>,
}

impl std::fmt::Debug for MapperNames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapperNames")
            .field("names", &self.names.len())
            .finish_non_exhaustive()
    }
}

/// What the install says about its resource sets, once the route, chain, names and dictionary
/// passed (§9.3.12 steps 1–3 and 6): the default key and the effective names. Built only by
/// [`resolve_resource_sets`]. Its `Debug` names no value.
#[derive(Clone, PartialEq, Eq)]
pub struct ResourceSetNames {
    default_key: u32,
    /// Effective `(key, name)`, the child's name shadowing its base's, by key.
    effective: Vec<(u32, String)>,
}

impl std::fmt::Debug for ResourceSetNames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceSetNames")
            .field("names", &self.effective.len())
            .finish_non_exhaustive()
    }
}

impl ResourceSetNames {
    /// The default set's key in the ResourceSets enum.
    #[must_use]
    pub const fn default_key(&self) -> u32 {
        self.default_key
    }

    /// How many effective names the chain has.
    #[must_use]
    pub fn name_count(&self) -> usize {
        self.effective.len()
    }

    /// The one effective key whose name equals `label` up to ASCII case (step 4).
    ///
    /// # Errors
    ///
    /// [`ReportOmission::ConfigUnmatched`] for none, [`ReportOmission::AmbiguousLabel`] for two.
    pub fn key_of(&self, label: &str) -> Result<u32, ReportOmission> {
        let mut found = self
            .effective
            .iter()
            .filter(|(_, name)| name.eq_ignore_ascii_case(label))
            .map(|(key, _)| *key);
        match (found.next(), found.next()) {
            (None, _) => Err(ReportOmission::ConfigUnmatched),
            (Some(key), None) => Ok(key),
            (Some(_), Some(_)) => Err(ReportOmission::AmbiguousLabel),
        }
    }
}

/// One property definition of the dictionary as step 6 reads it: its type code and its data
/// reference (the mapper DID for a bitfield).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DictionaryEntry {
    /// The property type code (`official-messages-world.md` §2.7.1).
    pub type_code: u32,
    /// The definition's data reference.
    pub data: u32,
}

/// Steps 2–3 and 6 of §9.3.12 from the route's target `start` (step 1 is the caller's).
///
/// `read_mapper` gives one record: [`ReportOmission::RouteMissing`] when it is absent,
/// [`ReportOmission::WrongFamily`] when the DID is not an enum mapper, or
/// [`ReportOmission::DecodeError`]. `dictionary` gives the definition of a property ID, `None` when
/// the dictionary has none.
///
/// # Errors
///
/// [`ReportOmission`] as a kind; every one omits the whole report.
pub fn resolve_resource_sets(
    start: u32,
    read_mapper: &mut dyn FnMut(u32) -> Result<MapperNames, ReportOmission>,
    dictionary: &dyn Fn(u32) -> Option<DictionaryEntry>,
) -> Result<ResourceSetNames, ReportOmission> {
    if start == 0 {
        return Err(ReportOmission::RouteMissing);
    }
    let mut effective: BTreeMap<u32, String> = BTreeMap::new();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut did = start;
    let mut base_names: Option<Vec<(u32, String)>> = None;
    for _ in 0..RESOURCE_SET_CHAIN_LIMIT {
        if !seen.insert(did) {
            return Err(ReportOmission::CycleOrDepth);
        }
        let record = read_mapper(did)?;
        let mut keys: HashSet<u32> = HashSet::with_capacity(record.names.len());
        if record.names.iter().any(|(key, _)| !keys.insert(*key)) {
            return Err(ReportOmission::DuplicateKey);
        }
        for (key, name) in &record.names {
            if !effective.contains_key(key) {
                if effective.len() >= MAX_RESOURCE_SET_NAMES {
                    return Err(ReportOmission::DecodeError);
                }
                effective.insert(*key, name.clone());
            }
        }
        if record.base == 0 {
            base_names = Some(record.names);
            break;
        }
        did = record.base;
    }
    let Some(base) = base_names else {
        return Err(ReportOmission::CycleOrDepth);
    };
    let (key, label) = match base.as_slice() {
        [(key, label)] if !label.is_empty() => (*key, label.as_str()),
        _ => return Err(ReportOmission::BaseDefaultNotUnique),
    };
    if key == 0 {
        return Err(ReportOmission::ZeroKey);
    }
    if !label.is_ascii() {
        return Err(ReportOmission::NonAsciiLabel);
    }
    let shadowed = effective.get(&key).map(String::as_str);
    if !shadowed.is_some_and(|name| name.eq_ignore_ascii_case(label)) {
        return Err(ReportOmission::LabelChanged);
    }
    let names = ResourceSetNames {
        default_key: key,
        effective: effective.into_iter().collect(),
    };
    // The default must resolve uniquely through the effective names.
    if names.key_of(label)? != key {
        return Err(ReportOmission::AmbiguousLabel);
    }
    check_dictionary(start, dictionary)?;
    Ok(names)
}

/// Step 6 of §9.3.12: both report properties must be Type 22 with `target` (the route's mapper) as
/// their data reference. The printed DID of the world spec is never a fallback.
///
/// # Errors
///
/// [`ReportOmission::DictionaryMissing`], [`ReportOmission::DictionaryType`] or
/// [`ReportOmission::DictionaryMismatch`].
pub fn check_dictionary(
    target: u32,
    dictionary: &dyn Fn(u32) -> Option<DictionaryEntry>,
) -> Result<(), ReportOmission> {
    for pid in [PID_AVAILABLE_RESOURCE_SETS, PID_LOADED_RESOURCE_SETS] {
        let entry = dictionary(pid).ok_or(ReportOmission::DictionaryMissing)?;
        if entry.type_code != TYPE_BITFIELD {
            return Err(ReportOmission::DictionaryType);
        }
        if entry.data != target {
            return Err(ReportOmission::DictionaryMismatch);
        }
    }
    Ok(())
}

/// The effective resource-set configuration of a launch (§9.3.12 steps 4–5).
#[derive(Clone, PartialEq, Eq)]
pub enum ResourceSetConfig {
    /// No list was supplied: the single built-in default.
    Default,
    /// One supplied token, trimmed, ASCII and not empty.
    Single(String),
}

impl std::fmt::Debug for ResourceSetConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => f.write_str("Default"),
            Self::Single(_) => f.write_str("Single(..)"),
        }
    }
}

/// The configuration of `option`, a supplied comma-separated list or `None` (§9.3.12 steps 4–5):
/// tokens are split on commas, and only ASCII space and tab are trimmed at their ends.
///
/// # Errors
///
/// [`ReportOmission::ConfigEmpty`] or [`ReportOmission::ConfigNonAscii`] for such a token, and
/// [`ReportOmission::ConfigMultiple`] for more than one token, repeats included.
pub fn resource_set_config(option: Option<&str>) -> Result<ResourceSetConfig, ReportOmission> {
    let Some(list) = option else {
        return Ok(ResourceSetConfig::Default);
    };
    let mut tokens = 0usize;
    let mut first: Option<&str> = None;
    for raw in list.split(',') {
        let token = raw.trim_matches([' ', '\t']);
        if token.is_empty() {
            return Err(ReportOmission::ConfigEmpty);
        }
        if !token.is_ascii() {
            return Err(ReportOmission::ConfigNonAscii);
        }
        tokens = tokens.saturating_add(1);
        first.get_or_insert(token);
    }
    match (tokens, first) {
        (1, Some(token)) => Ok(ResourceSetConfig::Single(token.to_owned())),
        _ => Err(ReportOmission::ConfigMultiple),
    }
}

/// The set the configuration selects, which item 13 allows only when it is the default (step 5).
///
/// # Errors
///
/// [`ReportOmission::ConfigUnmatched`], [`ReportOmission::AmbiguousLabel`] or
/// [`ReportOmission::ConfigOtherSet`].
pub fn selected_key(
    names: &ResourceSetNames,
    config: &ResourceSetConfig,
) -> Result<u32, ReportOmission> {
    match config {
        ResourceSetConfig::Default => Ok(names.default_key),
        ResourceSetConfig::Single(token) => {
            let key = names.key_of(token)?;
            if key == names.default_key {
                Ok(key)
            } else {
                Err(ReportOmission::ConfigOtherSet)
            }
        }
    }
}

/// The archive-header variant of a base archive: the `u16` LE at superblock `0x14C`
/// (`dat-container.md` §3.4), the first two bytes of its stored resource identifier.
#[must_use]
pub fn archive_variant(archive: &AdvertisedArchive) -> u16 {
    let [a, b, ..] = archive.resource_id;
    u16::from_le_bytes([a, b])
}

/// The minimum mount check of §9.3.12 over the preflight's base archives: `key` in
/// `1..=RESOURCE_SET_MAX_BITS`, at least one base, and every base's variant equal to `key − 1`
/// (the enum key is one greater than the variant).
///
/// # Errors
///
/// [`ReportOmission::KeyOutOfRange`], [`ReportOmission::NoMountedBase`] or
/// [`ReportOmission::VariantMismatch`].
pub fn check_mounted(key: u32, archives: &[AdvertisedArchive]) -> Result<(), ReportOmission> {
    if key == 0 || key > RESOURCE_SET_MAX_BITS {
        return Err(ReportOmission::KeyOutOfRange);
    }
    if archives.is_empty() {
        return Err(ReportOmission::NoMountedBase);
    }
    let variant = key - 1;
    if archives
        .iter()
        .any(|a| u32::from(archive_variant(a)) != variant)
    {
        return Err(ReportOmission::VariantMismatch);
    }
    Ok(())
}

/// The item-13 candidate: the default set in both fields, after every check (§9.3.12).
///
/// # Errors
///
/// [`ReportOmission`] as a kind.
pub fn candidate_report(
    names: &ResourceSetNames,
    option: Option<&str>,
    archives: &[AdvertisedArchive],
) -> Result<ResourceSetReport, ReportOmission> {
    if !ITEM_13_RESOURCE_REPORT {
        return Err(ReportOmission::Withdrawn);
    }
    let config = resource_set_config(option)?;
    let key = selected_key(names, &config)?;
    check_mounted(key, archives)?;
    let mask = ResourceSetMask::singleton(key).map_err(|_| ReportOmission::KeyOutOfRange)?;
    Ok(ResourceSetReport {
        available: mask.clone(),
        loaded: mask,
    })
}

/// The complete data of the candidate (one shared zero and both fields), ready for the envelope.
///
/// # Errors
///
/// As [`candidate_report`], and [`ReportOmission::Encoding`].
pub fn candidate_data(
    names: Result<ResourceSetNames, ReportOmission>,
    option: Option<&str>,
    archives: &[AdvertisedArchive],
) -> Result<Vec<u8>, ReportOmission> {
    if !ITEM_13_RESOURCE_REPORT {
        return Err(ReportOmission::Withdrawn);
    }
    let report = candidate_report(&names?, option, archives)?;
    report.to_bytes().map_err(|_| ReportOmission::Encoding)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A child's names, its base's names and the omission they give.
    type NameCase<'a> = (&'a [(u32, &'a str)], &'a [(u32, &'a str)], ReportOmission);

    const START: u32 = 0x2300_1000;
    const BASE: u32 = 0x2300_1001;

    fn names(list: &[(u32, &str)]) -> Vec<(u32, String)> {
        list.iter().map(|(k, n)| (*k, (*n).to_owned())).collect()
    }

    /// A child with one name over a base with one name: the checked install's shape.
    fn chain(child: &[(u32, &str)], base: &[(u32, &str)]) -> BTreeMap<u32, MapperNames> {
        BTreeMap::from([
            (
                START,
                MapperNames {
                    base: BASE,
                    names: names(child),
                },
            ),
            (
                BASE,
                MapperNames {
                    base: 0,
                    names: names(base),
                },
            ),
        ])
    }

    fn good_dictionary(pid: u32) -> Option<DictionaryEntry> {
        (pid == PID_AVAILABLE_RESOURCE_SETS || pid == PID_LOADED_RESOURCE_SETS).then_some(
            DictionaryEntry {
                type_code: TYPE_BITFIELD,
                data: START,
            },
        )
    }

    fn resolve(records: &BTreeMap<u32, MapperNames>) -> Result<ResourceSetNames, ReportOmission> {
        resolve_resource_sets(
            START,
            &mut |did| {
                records
                    .get(&did)
                    .cloned()
                    .ok_or(ReportOmission::RouteMissing)
            },
            &good_dictionary,
        )
    }

    fn archive(variant: u16) -> AdvertisedArchive {
        let [a, b] = variant.to_le_bytes();
        AdvertisedArchive {
            resource_id: [a, b, 3, 1, 0, 0, 0, 0],
            iteration_count: 0,
            spans: Vec::new(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn the_default_resolves_through_a_child_chain() {
        let names = resolve(&chain(&[(7, "Extra")], &[(3, "Base")])).expect("resolves");
        assert_eq!(names.default_key(), 3);
        assert_eq!(names.name_count(), 2);
        let data = candidate_data(Ok(names), None, &[archive(2), archive(2)]).expect("built");
        // One shared zero, then count 3 and bit 2 in each field.
        assert_eq!(data, [0x00, 0x03, 0x04, 0x03, 0x04]);
    }

    #[test]
    fn a_child_may_change_only_the_case_of_the_default_label() {
        let names = resolve(&chain(&[(3, "BASE")], &[(3, "Base")])).expect("case only");
        assert_eq!(names.default_key(), 3);
        assert_eq!(
            resolve(&chain(&[(3, "Other")], &[(3, "Base")])),
            Err(ReportOmission::LabelChanged)
        );
    }

    #[test]
    fn every_chain_failure_omits() {
        let missing = BTreeMap::new();
        assert_eq!(resolve(&missing), Err(ReportOmission::RouteMissing));
        assert_eq!(
            resolve_resource_sets(0, &mut |_| unreachable!(), &good_dictionary),
            Err(ReportOmission::RouteMissing)
        );
        // A base that is not there.
        let mut broken = chain(&[(7, "Extra")], &[(3, "Base")]);
        broken.remove(&BASE);
        assert_eq!(resolve(&broken), Err(ReportOmission::RouteMissing));
        // The reader's family and decode verdicts pass through.
        for kind in [ReportOmission::WrongFamily, ReportOmission::DecodeError] {
            assert_eq!(
                resolve_resource_sets(START, &mut |_| Err(kind), &good_dictionary),
                Err(kind)
            );
        }
        // A cycle.
        let mut cycle = chain(&[(7, "Extra")], &[(3, "Base")]);
        if let Some(base) = cycle.get_mut(&BASE) {
            base.base = START;
        }
        assert_eq!(resolve(&cycle), Err(ReportOmission::CycleOrDepth));
        // Seventeen records, each with a base.
        let mut deep = BTreeMap::new();
        for i in 0..=RESOURCE_SET_CHAIN_LIMIT as u32 {
            let base = if i as usize == RESOURCE_SET_CHAIN_LIMIT {
                0
            } else {
                START + i + 1
            };
            deep.insert(
                START + i,
                MapperNames {
                    base,
                    names: names(&[(i + 1, "N")]),
                },
            );
        }
        assert_eq!(resolve(&deep), Err(ReportOmission::CycleOrDepth));
        // Sixteen is the limit and still resolves when the names are unique.
        let mut sixteen = BTreeMap::new();
        for i in 0..RESOURCE_SET_CHAIN_LIMIT as u32 {
            let last = i as usize + 1 == RESOURCE_SET_CHAIN_LIMIT;
            let label = format!("N{i}");
            sixteen.insert(
                START + i,
                MapperNames {
                    base: if last { 0 } else { START + i + 1 },
                    names: vec![(i + 1, label)],
                },
            );
        }
        assert!(resolve(&sixteen).is_ok());
    }

    #[test]
    fn every_name_failure_omits() {
        let cases: [NameCase<'_>; 8] = [
            (
                &[(7, "A"), (7, "B")],
                &[(3, "Base")],
                ReportOmission::DuplicateKey,
            ),
            (
                &[(7, "A")],
                &[(3, "Base"), (3, "Again")],
                ReportOmission::DuplicateKey,
            ),
            (&[(7, "A")], &[], ReportOmission::BaseDefaultNotUnique),
            (
                &[(7, "A")],
                &[(3, "Base"), (4, "Two")],
                ReportOmission::BaseDefaultNotUnique,
            ),
            (
                &[(7, "A")],
                &[(3, "Base"), (0, "None")],
                ReportOmission::BaseDefaultNotUnique,
            ),
            (
                &[(7, "A")],
                &[(3, "")],
                ReportOmission::BaseDefaultNotUnique,
            ),
            (&[(7, "A")], &[(0, "Base")], ReportOmission::ZeroKey),
            (&[(7, "A")], &[(3, "Bäse")], ReportOmission::NonAsciiLabel),
        ];
        for (child, base, want) in cases {
            assert_eq!(resolve(&chain(child, base)), Err(want), "{want:?}");
        }
        // The same folded label at another effective key is ambiguous.
        assert_eq!(
            resolve(&chain(&[(7, "base")], &[(3, "Base")])),
            Err(ReportOmission::AmbiguousLabel)
        );
        // A child that removes nothing but names the key with a space is a changed label.
        assert_eq!(
            resolve(&chain(&[(3, "Base ")], &[(3, "Base")])),
            Err(ReportOmission::LabelChanged)
        );
    }

    #[test]
    fn every_dictionary_failure_omits() {
        let records = chain(&[(7, "Extra")], &[(3, "Base")]);
        let read = |did: u32| {
            records
                .get(&did)
                .cloned()
                .ok_or(ReportOmission::RouteMissing)
        };
        let missing =
            |pid: u32| (pid == PID_AVAILABLE_RESOURCE_SETS).then(|| good_dictionary(pid))?;
        let wrong_type = |pid: u32| {
            good_dictionary(pid).map(|e| DictionaryEntry {
                type_code: if pid == PID_LOADED_RESOURCE_SETS {
                    18
                } else {
                    e.type_code
                },
                ..e
            })
        };
        let other_mapper = |pid: u32| {
            good_dictionary(pid).map(|e| DictionaryEntry {
                data: if pid == PID_LOADED_RESOURCE_SETS {
                    BASE
                } else {
                    e.data
                },
                ..e
            })
        };
        for (dictionary, want) in [
            (
                &missing as &dyn Fn(u32) -> Option<DictionaryEntry>,
                ReportOmission::DictionaryMissing,
            ),
            (&wrong_type, ReportOmission::DictionaryType),
            (&other_mapper, ReportOmission::DictionaryMismatch),
        ] {
            assert_eq!(
                resolve_resource_sets(START, &mut |d| read(d), dictionary),
                Err(want)
            );
        }
    }

    #[test]
    fn the_tokeniser_follows_steps_four_and_five() {
        assert_eq!(resource_set_config(None), Ok(ResourceSetConfig::Default));
        assert_eq!(
            resource_set_config(Some(" \tBase\t ")),
            Ok(ResourceSetConfig::Single("Base".to_owned()))
        );
        // Only space and tab are trimmed; internal whitespace stays.
        assert_eq!(
            resource_set_config(Some("Ba se")),
            Ok(ResourceSetConfig::Single("Ba se".to_owned()))
        );
        assert_eq!(
            resource_set_config(Some("\nBase")),
            Ok(ResourceSetConfig::Single("\nBase".to_owned()))
        );
        for (list, want) in [
            ("", ReportOmission::ConfigEmpty),
            (" ", ReportOmission::ConfigEmpty),
            ("Base,", ReportOmission::ConfigEmpty),
            (",Base", ReportOmission::ConfigEmpty),
            ("Bäse", ReportOmission::ConfigNonAscii),
            ("Base,Base", ReportOmission::ConfigMultiple),
            ("Base, base", ReportOmission::ConfigMultiple),
            ("Base,Extra", ReportOmission::ConfigMultiple),
        ] {
            assert_eq!(resource_set_config(Some(list)), Err(want), "{list:?}");
        }
    }

    #[test]
    fn a_single_token_must_name_the_default() {
        let names = resolve(&chain(&[(7, "Extra")], &[(3, "Base")])).expect("resolves");
        let pick =
            |list: &str| resource_set_config(Some(list)).and_then(|c| selected_key(&names, &c));
        assert_eq!(pick("base"), Ok(3));
        assert_eq!(pick(" BASE\t"), Ok(3));
        assert_eq!(pick("Extra"), Err(ReportOmission::ConfigOtherSet));
        assert_eq!(pick("Nothing"), Err(ReportOmission::ConfigUnmatched));
        assert_eq!(pick("Base\n"), Err(ReportOmission::ConfigUnmatched));
        assert_eq!(
            candidate_data(Ok(names), Some("Extra"), &[archive(2)]),
            Err(ReportOmission::ConfigOtherSet)
        );
    }

    #[test]
    fn the_mount_check_needs_every_base_at_key_minus_one() {
        assert_eq!(check_mounted(3, &[archive(2)]), Ok(()));
        assert_eq!(check_mounted(1, &[archive(0), archive(0)]), Ok(()));
        assert_eq!(check_mounted(3, &[]), Err(ReportOmission::NoMountedBase));
        assert_eq!(
            check_mounted(3, &[archive(2), archive(3)]),
            Err(ReportOmission::VariantMismatch)
        );
        // The variant itself is not the key.
        assert_eq!(
            check_mounted(3, &[archive(3)]),
            Err(ReportOmission::VariantMismatch)
        );
        assert_eq!(
            check_mounted(0, &[archive(0)]),
            Err(ReportOmission::KeyOutOfRange)
        );
        assert_eq!(
            check_mounted(RESOURCE_SET_MAX_BITS + 1, &[archive(4096)]),
            Err(ReportOmission::KeyOutOfRange)
        );
        assert_eq!(
            check_mounted(RESOURCE_SET_MAX_BITS, &[archive(4095)]),
            Ok(())
        );
    }

    #[test]
    fn a_key_past_the_cap_gives_no_report() {
        let names = resolve(&chain(&[(7, "Extra")], &[(4097, "Base")])).expect("names resolve");
        assert_eq!(
            candidate_data(Ok(names), None, &[archive(4096)]),
            Err(ReportOmission::KeyOutOfRange)
        );
    }

    #[test]
    fn a_failed_read_or_the_default_install_omits() {
        assert_eq!(
            candidate_data(Err(ReportOmission::NotRead), None, &[archive(0)]),
            Err(ReportOmission::NotRead)
        );
    }

    #[test]
    fn debug_names_no_value() {
        let names = resolve(&chain(&[(7, "Extra")], &[(3, "Base")])).expect("resolves");
        let shown = format!("{names:?} {:?}", resource_set_config(Some("Base")));
        assert!(
            !shown.contains("Base") && !shown.contains("Extra"),
            "{shown}"
        );
        let report = candidate_report(&names, None, &[archive(2)]).expect("built");
        let shown = format!("{report:?}");
        assert!(!shown.contains('3'), "{shown}");
    }

    #[test]
    fn every_reason_names_item_five_and_its_kind() {
        let all = [
            ReportOmission::Withdrawn,
            ReportOmission::NotRead,
            ReportOmission::RouteMissing,
            ReportOmission::WrongFamily,
            ReportOmission::DecodeError,
            ReportOmission::DuplicateKey,
            ReportOmission::CycleOrDepth,
            ReportOmission::BaseDefaultNotUnique,
            ReportOmission::ZeroKey,
            ReportOmission::LabelChanged,
            ReportOmission::NonAsciiLabel,
            ReportOmission::AmbiguousLabel,
            ReportOmission::ConfigEmpty,
            ReportOmission::ConfigNonAscii,
            ReportOmission::ConfigUnmatched,
            ReportOmission::ConfigMultiple,
            ReportOmission::ConfigOtherSet,
            ReportOmission::DictionaryMissing,
            ReportOmission::DictionaryType,
            ReportOmission::DictionaryMismatch,
            ReportOmission::NoMountedBase,
            ReportOmission::VariantMismatch,
            ReportOmission::KeyOutOfRange,
            ReportOmission::Encoding,
            ReportOmission::Enqueue,
        ];
        for kind in all {
            let reason = kind.withholding_reason();
            assert!(
                reason.starts_with("decision 0046 item 5; item 13 "),
                "{reason}"
            );
            assert!(reason.ends_with(kind.label()), "{reason}");
        }
    }
}
