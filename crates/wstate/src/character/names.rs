// SPDX-License-Identifier: GPL-3.0-or-later
//! Names: the name-rule control, the random-name generator, the client's phonetic-shift
//! rule with its text model, and the client's generation procedure with Tessera's
//! substitutions (`player-character.md` §8.1–§8.5, Decisions D2 and D6).

use std::collections::BTreeMap;

use tessera_formats::StringInfo;
use tessera_props::{PropertyRegistry, PropertySet};

use super::Fields;
use crate::error::{Result, WStateError};
use crate::native::NestedStringTable;

/// Name-rule control (§8.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NameRuleControl {
    /// `NameRule_NameRule` of every `NameRule_NameRuleEntry`, stored order.
    pub rules: Vec<u32>,
}

impl NameRuleControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`WStateError::BadProperty`] for an entry without its rule key or a property of an
    /// unexpected type; [`WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "name-rule control");
        Ok(Self {
            rules: f.struct_list("NameRule_NameRuleArray", |e| {
                e.require("NameRule_NameRule", e.enum_("NameRule_NameRule")?)
            })?,
        })
    }
}

/// One `RandomNameGeneration_RaceToCategories_Entry` (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RaceCategory {
    /// `NameGenerationCategory_Race`: species key.
    pub species: u32,
    /// `NameGenerationCategory_Gender`: sex key.
    pub sex: u32,
    /// `RandomNameGeneration_CategoryType`.
    pub category: u32,
}

/// One `NameGenerationCategory_Option` (§8.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NameOption {
    /// `NameGenerationCategory_Option_Weight` (relative; absent reads as 0).
    pub weight: i32,
    /// `NameGenerationCategory_Option_SyllableTables`: the parts of the name, in order.
    pub syllable_tables: Vec<u32>,
    /// `NameGenerationCategory_Option_PhoneticShiftsTables`.
    pub shift_tables: Vec<u32>,
}

/// One `NameGenerationCategory` (§8.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NameCategory {
    /// `RandomNameGeneration_CategoryType`.
    pub key: u32,
    /// `NameGenerationCategory_Options`.
    pub options: Vec<NameOption>,
}

/// One `RandomNameGeneration_SyllableTable_Definition` (§8.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyllableTable {
    /// `RandomNameGeneration_SyllableTable_Key`.
    pub key: u32,
    /// `RandomNameGeneration_SyllableTable_StringsList`.
    pub strings: Vec<StringInfo>,
}

/// One `RandomNameGeneration_PhoneticShiftsTable_Struct`, unresolved (§8.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShiftEntryInfo {
    /// *E*: `RandomNameGeneration_PhoneticShiftsTable_LeftString_EndMatch`.
    pub end_match: Option<StringInfo>,
    /// *S*: `RandomNameGeneration_PhoneticShiftsTable_RightString_StartMatch`.
    pub start_match: Option<StringInfo>,
    /// *L′*: `RandomNameGeneration_PhoneticShiftsTable_LeftString_Replacement`.
    pub left_replacement: Option<StringInfo>,
    /// *R′*: `RandomNameGeneration_PhoneticShiftsTable_RightString_Replacement`.
    pub right_replacement: Option<StringInfo>,
}

/// One `RandomNameGeneration_PhoneticShiftsTable_Definition` (§8.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShiftTableDefinition {
    /// `RandomNameGeneration_PhoneticShiftsTable_Key`.
    pub key: u32,
    /// `RandomNameGeneration_PhoneticShiftsTable_StructList`, stored order.
    pub entries: Vec<ShiftEntryInfo>,
}

impl ShiftTableDefinition {
    /// Resolves the entries' texts with `resolve` and builds the compiled table (§8.4). A
    /// replacement that does not resolve counts as the empty string.
    ///
    /// # Errors
    ///
    /// [`WStateError::BadProperty`] when an entry's end or start match is absent or does
    /// not resolve (the client reports a failed load then, §8.4).
    pub fn compile(
        &self,
        mut resolve: impl FnMut(&StringInfo) -> Option<String>,
    ) -> Result<ShiftTable> {
        let mut entries = Vec::with_capacity(self.entries.len());
        for e in &self.entries {
            let need = |v: &Option<StringInfo>,
                        name: &'static str,
                        resolve: &mut dyn FnMut(&StringInfo) -> Option<String>|
             -> Result<String> {
                v.as_ref()
                    .and_then(&mut *resolve)
                    .ok_or(WStateError::BadProperty {
                        record: "phonetic-shift table",
                        property: name,
                        problem: "is absent or does not resolve",
                    })
            };
            let end = need(
                &e.end_match,
                "RandomNameGeneration_PhoneticShiftsTable_LeftString_EndMatch",
                &mut resolve,
            )?;
            let start = need(
                &e.start_match,
                "RandomNameGeneration_PhoneticShiftsTable_RightString_StartMatch",
                &mut resolve,
            )?;
            let left = e
                .left_replacement
                .as_ref()
                .and_then(&mut resolve)
                .unwrap_or_default();
            let right = e
                .right_replacement
                .as_ref()
                .and_then(&mut resolve)
                .unwrap_or_default();
            entries.push(ShiftEntry {
                end,
                start,
                left,
                right,
            });
        }
        Ok(ShiftTable::compile(&entries))
    }
}

/// The random-name generator (§8.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NameGenerator {
    /// `RandomNameGeneration_RaceToCategoriesArray`.
    pub race_categories: Vec<RaceCategory>,
    /// `RandomNameGenerationRules`.
    pub categories: Vec<NameCategory>,
    /// `RandomNameGeneration_SyllableTableDefinitions`.
    pub syllable_tables: Vec<SyllableTable>,
    /// `RandomNameGeneration_PhoneticShiftsTableDefinitions`.
    pub shift_tables: Vec<ShiftTableDefinition>,
    /// `RandomNameGeneration_ValidNameRules`.
    pub valid_rules: Vec<u32>,
}

impl NameGenerator {
    /// Reads the generator's property half.
    ///
    /// # Errors
    ///
    /// [`WStateError::BadProperty`] when a key field is absent or a property has an
    /// unexpected type; [`WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "random-name generator");
        let race_categories = f.struct_list("RandomNameGeneration_RaceToCategoriesArray", |e| {
            Ok(RaceCategory {
                species: e.require(
                    "NameGenerationCategory_Race",
                    e.enum_("NameGenerationCategory_Race")?,
                )?,
                sex: e.require(
                    "NameGenerationCategory_Gender",
                    e.enum_("NameGenerationCategory_Gender")?,
                )?,
                category: e.require(
                    "RandomNameGeneration_CategoryType",
                    e.enum_("RandomNameGeneration_CategoryType")?,
                )?,
            })
        })?;
        let categories = f.struct_list("RandomNameGenerationRules", |c| {
            Ok(NameCategory {
                key: c.require(
                    "RandomNameGeneration_CategoryType",
                    c.enum_("RandomNameGeneration_CategoryType")?,
                )?,
                options: c.struct_list("NameGenerationCategory_Options", |o| {
                    Ok(NameOption {
                        weight: o.int("NameGenerationCategory_Option_Weight")?.unwrap_or(0),
                        syllable_tables: o
                            .enum_list("NameGenerationCategory_Option_SyllableTables")?,
                        shift_tables: o
                            .enum_list("NameGenerationCategory_Option_PhoneticShiftsTables")?,
                    })
                })?,
            })
        })?;
        let syllable_tables =
            f.struct_list("RandomNameGeneration_SyllableTableDefinitions", |t| {
                Ok(SyllableTable {
                    key: t.require(
                        "RandomNameGeneration_SyllableTable_Key",
                        t.enum_("RandomNameGeneration_SyllableTable_Key")?,
                    )?,
                    strings: t.map_list("RandomNameGeneration_SyllableTable_StringsList", |v| {
                        v.as_string_info().cloned()
                    })?,
                })
            })?;
        let shift_tables =
            f.struct_list("RandomNameGeneration_PhoneticShiftsTableDefinitions", |t| {
                Ok(ShiftTableDefinition {
                    key: t.require(
                        "RandomNameGeneration_PhoneticShiftsTable_Key",
                        t.enum_("RandomNameGeneration_PhoneticShiftsTable_Key")?,
                    )?,
                    entries: t.struct_list(
                        "RandomNameGeneration_PhoneticShiftsTable_StructList",
                        |e| {
                            Ok(ShiftEntryInfo {
                            end_match: e.string_info(
                                "RandomNameGeneration_PhoneticShiftsTable_LeftString_EndMatch",
                            )?,
                            start_match: e.string_info(
                                "RandomNameGeneration_PhoneticShiftsTable_RightString_StartMatch",
                            )?,
                            left_replacement: e.string_info(
                                "RandomNameGeneration_PhoneticShiftsTable_LeftString_Replacement",
                            )?,
                            right_replacement: e.string_info(
                                "RandomNameGeneration_PhoneticShiftsTable_RightString_Replacement",
                            )?,
                        })
                        },
                    )?,
                })
            })?;
        Ok(Self {
            race_categories,
            categories,
            syllable_tables,
            shift_tables,
            valid_rules: f.enum_list("RandomNameGeneration_ValidNameRules")?,
        })
    }

    /// The category of (species, sex), the first matching entry.
    #[must_use]
    pub fn category_for(&self, species: u32, sex: u32) -> Option<u32> {
        self.race_categories
            .iter()
            .find(|e| e.species == species && e.sex == sex)
            .map(|e| e.category)
    }

    /// The category with key `key`.
    #[must_use]
    pub fn category(&self, key: u32) -> Option<&NameCategory> {
        self.categories.iter().find(|c| c.key == key)
    }

    /// The syllable table with key `key`.
    #[must_use]
    pub fn syllable_table(&self, key: u32) -> Option<&SyllableTable> {
        self.syllable_tables.iter().find(|t| t.key == key)
    }

    /// The shift-table definition with key `key`.
    #[must_use]
    pub fn shift_table(&self, key: u32) -> Option<&ShiftTableDefinition> {
        self.shift_tables.iter().find(|t| t.key == key)
    }

    /// Resolves every syllable and compiles every shift table with `resolve` (a
    /// StringInfo renderer, normally over the install's string tables). Syllables that do
    /// not resolve are left out (and logged).
    ///
    /// # Errors
    ///
    /// As [`ShiftTableDefinition::compile`].
    pub fn resolve(
        &self,
        mut resolve: impl FnMut(&StringInfo) -> Option<String>,
    ) -> Result<NameText> {
        let mut syllables = BTreeMap::new();
        for t in &self.syllable_tables {
            let mut v = Vec::with_capacity(t.strings.len());
            for s in &t.strings {
                match resolve(s) {
                    Some(text) => v.push(text),
                    None => tracing::debug!(table = t.key, "syllable string does not resolve"),
                }
            }
            syllables.insert(t.key, v);
        }
        let mut shifts = BTreeMap::new();
        for t in &self.shift_tables {
            shifts.insert(t.key, t.compile(&mut resolve)?);
        }
        Ok(NameText { syllables, shifts })
    }
}

/// A generator's texts: resolved syllables and compiled shift tables, by key.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NameText {
    /// Syllable-table key → strings, stored order.
    pub syllables: BTreeMap<u32, Vec<String>>,
    /// Shift-table key → compiled table.
    pub shifts: BTreeMap<u32, ShiftTable>,
}

/// One shift-table entry with its texts resolved, as stored (not trimmed).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ShiftEntry {
    /// *E*.
    pub end: String,
    /// *S*.
    pub start: String,
    /// *L′* (empty if it did not resolve).
    pub left: String,
    /// *R′* (empty if it did not resolve).
    pub right: String,
}

/// One (*S*, *L′*, *R′*) triple of a compiled table.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ShiftTriple {
    /// *S*, trimmed.
    pub start: String,
    /// *L′*, as stored.
    pub left: String,
    /// *R′*, as stored.
    pub right: String,
}

/// The entries of a compiled table that share one *E* (equal without regard to case).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShiftGroup {
    /// *E*, trimmed (the first spelling met).
    pub text: String,
    /// Right length → triples (stored order), right lengths descending.
    pub by_right: Vec<(u32, Vec<ShiftTriple>)>,
}

/// A compiled phonetic-shift table (§8.4; the meaning of native class 3053).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShiftTable {
    /// Left length → groups, left lengths descending.
    pub by_left: Vec<(u32, Vec<ShiftGroup>)>,
    /// The distinct left lengths, descending (`list_a`).
    pub left_lengths: Vec<u32>,
    /// The distinct right lengths, descending (`list_b`).
    pub right_lengths: Vec<u32>,
}

/// The white space the client trims from both ends of *E* and *S* (§8.4).
const TRIMMED: [char; 5] = [' ', '\n', '\r', '\t', '\u{0C}'];

fn trim(s: &str) -> &str {
    s.trim_matches(|c| TRIMMED.contains(&c))
}

/// The UTF-16 code units of `s` (the client's text model, §8.4).
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// UTF-16 code units of `s` plus one, saturating (§8.4: left and right lengths).
fn length_key(s: &str) -> u32 {
    u32::try_from(s.encode_utf16().count())
        .unwrap_or(u32::MAX)
        .saturating_add(1)
}

/// The code unit's full mapping by `f` when it is one code unit for one code unit;
/// otherwise the code unit itself (surrogates included).
fn one_to_one<I: Iterator<Item = char>>(u: u16, f: impl FnOnce(char) -> I) -> u16 {
    let Some(c) = char::from_u32(u32::from(u)) else {
        return u;
    };
    let mut it = f(c);
    match (it.next(), it.next()) {
        (Some(m), None) => u16::try_from(u32::from(m)).unwrap_or(u),
        _ => u,
    }
}

/// Simple one-to-one lower-case mapping of one UTF-16 code unit (Decision D6, §8.4).
///
/// Rust's standard library exposes the full mappings; where a full mapping has more than
/// one character the simple mapping differs from "no change" only for U+0130, which maps
/// to U+0069 (Unicode's `UnicodeData.txt`).
#[must_use]
pub fn lower_unit(u: u16) -> u16 {
    if u == 0x0130 {
        return 0x0069;
    }
    one_to_one(u, char::to_lowercase)
}

/// Simple one-to-one upper-case mapping of one UTF-16 code unit (Decision D6, §8.4).
///
/// Where the full mapping has more than one character, Unicode's simple mapping is used
/// for the Greek letters with ypogegrammeni that have one (U+1F80–U+1F87, U+1F90–U+1F97,
/// U+1FA0–U+1FA7, U+1FB3, U+1FC3, U+1FF3); every other such code unit stays.
#[must_use]
pub fn upper_unit(u: u16) -> u16 {
    match u {
        0x1F80..=0x1F87 | 0x1F90..=0x1F97 | 0x1FA0..=0x1FA7 => u + 8,
        0x1FB3 => 0x1FBC,
        0x1FC3 => 0x1FCC,
        0x1FF3 => 0x1FFC,
        _ => one_to_one(u, char::to_uppercase),
    }
}

/// `s` with every code unit lower-cased by [`lower_unit`].
#[must_use]
pub fn lower_text(s: &str) -> String {
    String::from_utf16_lossy(&s.encode_utf16().map(lower_unit).collect::<Vec<_>>())
}

fn eq_ci(a: u16, b: u16) -> bool {
    a == b || lower_unit(a) == lower_unit(b)
}

fn units_eq_ci(a: &[u16], b: &[u16]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| eq_ci(x, y))
}

/// Whether two texts are equal without regard to case (§8.4 text model): code unit by
/// code unit, each folded by [`lower_unit`].
#[must_use]
pub fn text_eq_ci(a: &str, b: &str) -> bool {
    units_eq_ci(&units(a), &units(b))
}

/// Whether `s` ends with `suffix`, without regard to case; `Some(n)` = the suffix's
/// length in code units.
fn ends_with_ci(s: &[u16], suffix: &[u16]) -> Option<usize> {
    let start = s.len().checked_sub(suffix.len())?;
    units_eq_ci(s.get(start..)?, suffix).then_some(suffix.len())
}

fn starts_with_ci(s: &[u16], prefix: &[u16]) -> Option<usize> {
    units_eq_ci(s.get(..prefix.len())?, prefix).then_some(prefix.len())
}

/// The value under `key` in a list kept in descending key order, inserted with `make`
/// when absent.
fn desc_slot<T>(v: &mut Vec<(u32, T)>, key: u32, make: impl FnOnce() -> T) -> Option<&mut T> {
    let i = v.iter().position(|(k, _)| *k <= key).unwrap_or(v.len());
    if v.get(i).is_none_or(|(k, _)| *k != key) {
        v.insert(i, (key, make()));
    }
    v.get_mut(i).map(|(_, t)| t)
}

impl ShiftTable {
    /// Builds the compiled table from resolved entries (§8.4): *E* and *S* trimmed of the
    /// five white-space characters, replacements kept as stored, entries grouped by left
    /// length, then by *E* without regard to case, then by right length.
    #[must_use]
    pub fn compile(entries: &[ShiftEntry]) -> Self {
        let mut t = Self::default();
        for e in entries {
            let end = trim(&e.end);
            let start = trim(&e.start);
            let left_len = length_key(end);
            let right_len = length_key(start);
            let Some(groups) = desc_slot(&mut t.by_left, left_len, Vec::new) else {
                continue;
            };
            let gi = match groups.iter().position(|g| text_eq_ci(&g.text, end)) {
                Some(i) => i,
                None => {
                    groups.push(ShiftGroup {
                        text: end.to_owned(),
                        by_right: Vec::new(),
                    });
                    groups.len() - 1
                }
            };
            if let Some(triples) = groups
                .get_mut(gi)
                .and_then(|g| desc_slot(&mut g.by_right, right_len, Vec::new))
            {
                triples.push(ShiftTriple {
                    start: start.to_owned(),
                    left: e.left.clone(),
                    right: e.right.clone(),
                });
            }
            if !t.right_lengths.contains(&right_len) {
                t.right_lengths.push(right_len);
            }
        }
        t.left_lengths = t.by_left.iter().map(|(k, _)| *k).collect();
        t.right_lengths.sort_unstable_by(|a, b| b.cmp(a));
        t
    }

    /// The table stored in a class-3053 object (§8.4): outer key = left length, group text
    /// = *E*, inner key = right length, triple = (*S*, *L′*, *R′*). Outer and inner
    /// tables are ordered by descending key for matching; `list_a` and `list_b` are kept
    /// as stored.
    #[must_use]
    pub fn from_native(n: &NestedStringTable) -> Self {
        let mut by_left: Vec<(u32, Vec<ShiftGroup>)> = n
            .outer
            .iter()
            .map(|o| {
                let groups = o
                    .groups
                    .iter()
                    .map(|g| {
                        let mut by_right: Vec<(u32, Vec<ShiftTriple>)> = g
                            .inner
                            .iter()
                            .map(|i| {
                                let triples = i
                                    .triples
                                    .iter()
                                    .map(|[s, l, r]| ShiftTriple {
                                        start: s.to_string_lossy(),
                                        left: l.to_string_lossy(),
                                        right: r.to_string_lossy(),
                                    })
                                    .collect();
                                (i.key, triples)
                            })
                            .collect();
                        by_right.sort_by_key(|a| std::cmp::Reverse(a.0));
                        ShiftGroup {
                            text: g.text.to_string_lossy(),
                            by_right,
                        }
                    })
                    .collect();
                (o.key, groups)
            })
            .collect();
        by_left.sort_by_key(|a| std::cmp::Reverse(a.0));
        Self {
            by_left,
            left_lengths: n.list_a.clone(),
            right_lengths: n.list_b.clone(),
        }
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_left
            .iter()
            .flat_map(|(_, g)| g)
            .flat_map(|g| &g.by_right)
            .map(|(_, t)| t.len())
            .sum()
    }

    /// Whether the table has no entry.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every entry as (left length, *E* lower-cased by [`lower_unit`], right length, *S*,
    /// *L′*, *R′*),
    /// sorted: a canonical form for comparing two tables (PC11).
    #[must_use]
    pub fn canonical_entries(&self) -> Vec<(u32, String, u32, String, String, String)> {
        let mut v = Vec::new();
        for (l, groups) in &self.by_left {
            for g in groups {
                let e = lower_text(&g.text);
                for (r, triples) in &g.by_right {
                    for t in triples {
                        v.push((
                            *l,
                            e.clone(),
                            *r,
                            t.start.clone(),
                            t.left.clone(),
                            t.right.clone(),
                        ));
                    }
                }
            }
        }
        v.sort();
        v
    }

    /// Whether every key equals its text's UTF-16 code-unit count plus one, and both length
    /// lists are strictly descending and list exactly the keys used (PC11).
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        let strictly_desc = |v: &[u32]| v.windows(2).all(|w| matches!(w, [a, b] if a > b));
        let mut rights: Vec<u32> = Vec::new();
        for (l, groups) in &self.by_left {
            for g in groups {
                if length_key(&g.text) != *l {
                    return false;
                }
                for (r, triples) in &g.by_right {
                    if triples.iter().any(|t| length_key(&t.start) != *r) {
                        return false;
                    }
                    if !rights.contains(r) {
                        rights.push(*r);
                    }
                }
            }
        }
        rights.sort_unstable_by(|a, b| b.cmp(a));
        let lefts: Vec<u32> = self.by_left.iter().map(|(k, _)| *k).collect();
        strictly_desc(&self.left_lengths)
            && strictly_desc(&self.right_lengths)
            && self.left_lengths == lefts
            && self.right_lengths == rights
    }

    /// Applies the table at the join of `a` and `b` (§8.4 steps 2–5) in the client's
    /// text model: UTF-16 code units, compared without regard to case by [`lower_unit`].
    /// The matching entry with the longest *E*, then the longest *S*, replaces the last
    /// |*E*| code units of `a` with *L′* and the first |*S*| code units of `b` with *R′*.
    /// Returns the new pair, or `None` when no entry matches (both unchanged). An entry
    /// whose trimmed *E* is empty matches every `a`, the empty one included (§8.5 step 4
    /// OPEN, Q16: applied as §8.4 states).
    #[must_use]
    pub fn apply_units(&self, a: &[u16], b: &[u16]) -> Option<(Vec<u16>, Vec<u16>)> {
        for (_, groups) in &self.by_left {
            for g in groups {
                let Some(e_len) = ends_with_ci(a, &units(&g.text)) else {
                    continue;
                };
                for (_, triples) in &g.by_right {
                    for t in triples {
                        let Some(s_len) = starts_with_ci(b, &units(&t.start)) else {
                            continue;
                        };
                        let keep_a = a.get(..a.len().saturating_sub(e_len)).unwrap_or(&[]);
                        let keep_b = b.get(s_len..).unwrap_or(&[]);
                        let mut new_a = keep_a.to_vec();
                        new_a.extend(t.left.encode_utf16());
                        let mut new_b: Vec<u16> = t.right.encode_utf16().collect();
                        new_b.extend_from_slice(keep_b);
                        return Some((new_a, new_b));
                    }
                }
            }
        }
        None
    }

    /// [`Self::apply_units`] on texts. A cut inside a surrogate pair (only possible with an
    /// *E* or *S* that ends in half a pair) comes back as U+FFFD.
    #[must_use]
    pub fn apply(&self, a: &str, b: &str) -> Option<(String, String)> {
        self.apply_units(&units(a), &units(b))
            .map(|(x, y)| (String::from_utf16_lossy(&x), String::from_utf16_lossy(&y)))
    }

    /// Joins `a` and `b` after one application of the table.
    #[must_use]
    pub fn join(&self, a: &str, b: &str) -> String {
        match self.apply_units(&units(a), &units(b)) {
            Some((mut x, y)) => {
                x.extend(y);
                String::from_utf16_lossy(&x)
            }
            None => format!("{a}{b}"),
        }
    }
}

/// Draws made for one part of a name before generation fails: the first and up to 100
/// redraws, counted separately for each part (§8.5 step 3).
pub const MAX_PART_DRAWS: u32 = 101;

/// Attempts of [`generate_name`] in all: the first and up to 99 repeats (§8.5, the
/// creation screen's limit, which Decision D2 keeps for the sandbox's own check).
pub const MAX_NAME_ATTEMPTS: usize = 100;

/// The option drawn by weight (§8.5 step 2): option *i* with probability *wᵢ* ÷ *W*.
/// Options with a weight below 1 are never drawn and a total of 0 fails (Decision D2,
/// substitution 3). `random(n)` must return a value in `0..n` (it is reduced modulo `n`).
/// Mapping the draw onto options in stored order gives the probabilities of the client's
/// weighted tree, which is all the procedure requires (Decision D2, substitution 1).
fn draw_option<'g>(
    category: &'g NameCategory,
    random: &mut impl FnMut(u32) -> u32,
) -> Option<&'g NameOption> {
    let weight = |o: &NameOption| u32::try_from(o.weight).unwrap_or(0);
    let total = category
        .options
        .iter()
        .try_fold(0u32, |acc, o| acc.checked_add(weight(o)))?;
    if total == 0 {
        return None;
    }
    let mut r = random(total) % total;
    for o in &category.options {
        let w = weight(o);
        if r < w {
            return Some(o);
        }
        r -= w;
    }
    None
}

/// The parts of a name (§8.5 step 3): per syllable-table key, in order, one string drawn
/// uniformly; a text equal without regard to case to a part already drawn is rejected and
/// drawn again, at most [`MAX_PART_DRAWS`] draws per part. `None` when a key has no
/// table, a table is empty, or every draw of a part was rejected.
fn draw_parts(
    option: &NameOption,
    text: &NameText,
    random: &mut impl FnMut(u32) -> u32,
) -> Option<Vec<Vec<u16>>> {
    let mut parts: Vec<Vec<u16>> = Vec::new();
    for key in &option.syllable_tables {
        let strings = text.syllables.get(key).filter(|v| !v.is_empty())?;
        let n = u32::try_from(strings.len()).unwrap_or(u32::MAX);
        let mut accepted = None;
        for _ in 0..MAX_PART_DRAWS {
            let pick = usize::try_from(random(n) % n).unwrap_or(0);
            let candidate = units(strings.get(pick)?);
            if !parts.iter().any(|p| units_eq_ci(p, &candidate)) {
                accepted = Some(candidate);
                break;
            }
        }
        parts.push(accepted?);
    }
    Some(parts)
}

/// One generation by the client's procedure (§8.5 steps 1–6): category of (species,
/// sex), option by weight, parts with repeat rejection, joins that try the option's shift
/// tables in stored order until one applies (an undefined key acts as a table without
/// entries; the join is made for the first part too, with an empty name), then the whole
/// name lower-cased and its first code unit upper-cased. A failed generation yields the
/// empty string.
///
/// `random(n)` must return a value in `0..n` (it is reduced modulo `n` anyway). Syllables
/// that did not resolve were left out of `text` by [`NameGenerator::resolve`].
pub fn generate_name_once(
    generator: &NameGenerator,
    text: &NameText,
    species: u32,
    sex: u32,
    mut random: impl FnMut(u32) -> u32,
) -> String {
    let Some(category) = generator
        .category_for(species, sex)
        .and_then(|k| generator.category(k))
    else {
        return String::new();
    };
    let Some(option) = draw_option(category, &mut random) else {
        return String::new();
    };
    let Some(parts) = draw_parts(option, text, &mut random) else {
        return String::new();
    };
    let mut name: Vec<u16> = Vec::new();
    for part in parts {
        let mut b = part;
        for key in &option.shift_tables {
            let Some(table) = text.shifts.get(key) else {
                continue;
            };
            if let Some((na, nb)) = table.apply_units(&name, &b) {
                name = na;
                b = nb;
                break;
            }
        }
        name.extend(b);
    }
    for u in &mut name {
        *u = lower_unit(*u);
    }
    if let Some(first) = name.first_mut() {
        *first = upper_unit(*first);
    }
    String::from_utf16_lossy(&name)
}

/// The result of [`generate_name`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GeneratedName {
    /// The accepted name, or the last generated one when none was accepted (empty when
    /// the last generation failed).
    pub name: String,
    /// Whether a name passed the check.
    pub accepted: bool,
    /// Generations made (1 ..= [`MAX_NAME_ATTEMPTS`]).
    pub attempts: usize,
}

/// The creation screen's random-name action under Decision D2 (§8.5): generate a name
/// with [`generate_name_once`] and pass a lower-cased copy to `accept` (the sandbox's own
/// name check, standing in for the client's), at most [`MAX_NAME_ATTEMPTS`] times in all.
/// When none passes, the result holds the last generated name and `accepted` is false.
/// A failed generation (the empty name) never passes and is not shown to `accept`.
pub fn generate_name(
    generator: &NameGenerator,
    text: &NameText,
    species: u32,
    sex: u32,
    mut random: impl FnMut(u32) -> u32,
    mut accept: impl FnMut(&str) -> bool,
) -> GeneratedName {
    let mut out = GeneratedName::default();
    for attempt in 1..=MAX_NAME_ATTEMPTS {
        out.name = generate_name_once(generator, text, species, sex, &mut random);
        out.attempts = attempt;
        if !out.name.is_empty() && accept(&lower_text(&out.name)) {
            out.accepted = true;
            break;
        }
    }
    out
}
