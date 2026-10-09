// SPDX-License-Identifier: GPL-3.0-or-later
//! Names: the name-rule control, the random-name generator, the client's phonetic-shift
//! rule and Tessera's name assembly (`player-character.md` §8).

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

/// Characters of `s` plus one, saturating.
fn length_key(s: &str) -> u32 {
    u32::try_from(s.chars().count())
        .unwrap_or(u32::MAX)
        .saturating_add(1)
}

fn eq_ci(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

fn text_eq_ci(a: &str, b: &str) -> bool {
    a.chars().count() == b.chars().count() && a.chars().zip(b.chars()).all(|(x, y)| eq_ci(x, y))
}

/// Whether `s` ends with `suffix`, without regard to case; `Some(n)` = the suffix's
/// character count.
fn ends_with_ci(s: &[char], suffix: &str) -> Option<usize> {
    let n = suffix.chars().count();
    let start = s.len().checked_sub(n)?;
    let tail = s.get(start..)?;
    tail.iter()
        .zip(suffix.chars())
        .all(|(&x, y)| eq_ci(x, y))
        .then_some(n)
}

fn starts_with_ci(s: &[char], prefix: &str) -> Option<usize> {
    let n = prefix.chars().count();
    let head = s.get(..n)?;
    head.iter()
        .zip(prefix.chars())
        .all(|(&x, y)| eq_ci(x, y))
        .then_some(n)
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

    /// Every entry as (left length, *E* lower-cased, right length, *S*, *L′*, *R′*),
    /// sorted: a canonical form for comparing two tables (PC11).
    #[must_use]
    pub fn canonical_entries(&self) -> Vec<(u32, String, u32, String, String, String)> {
        let mut v = Vec::new();
        for (l, groups) in &self.by_left {
            for g in groups {
                let e = g.text.to_lowercase();
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

    /// Whether every key equals its text's character count plus one, and both length
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

    /// Applies the table at the join of `a` and `b` (§8.4 steps 2–5): the matching entry
    /// with the longest *E*, then the longest *S*, replaces the end of `a` and the start
    /// of `b`. Returns the new pair, or `None` when no entry matches (both unchanged).
    #[must_use]
    pub fn apply(&self, a: &str, b: &str) -> Option<(String, String)> {
        let ac: Vec<char> = a.chars().collect();
        let bc: Vec<char> = b.chars().collect();
        for (_, groups) in &self.by_left {
            for g in groups {
                let Some(e_len) = ends_with_ci(&ac, &g.text) else {
                    continue;
                };
                for (_, triples) in &g.by_right {
                    for t in triples {
                        let Some(s_len) = starts_with_ci(&bc, &t.start) else {
                            continue;
                        };
                        let keep_a = ac.get(..ac.len().saturating_sub(e_len)).unwrap_or(&[]);
                        let keep_b = bc.get(s_len..).unwrap_or(&[]);
                        let mut new_a: String = keep_a.iter().collect();
                        new_a.push_str(&t.left);
                        let mut new_b = t.right.clone();
                        new_b.extend(keep_b);
                        return Some((new_a, new_b));
                    }
                }
            }
        }
        None
    }

    /// Joins `a` and `b` after one application of the table.
    #[must_use]
    pub fn join(&self, a: &str, b: &str) -> String {
        match self.apply(a, b) {
            Some((x, y)) => x + &y,
            None => format!("{a}{b}"),
        }
    }
}

/// Attempts of [`generate_name`]: the first plus at most 20 restarts (Decision D2 step 5).
pub const MAX_NAME_ATTEMPTS: usize = 21;

/// Generates a name for (species, sex) by Decision D2 (§8.5): draw an option by weight,
/// one string per syllable table, join left to right applying the option's shift table
/// once per join, and retry while `accept` rejects the result.
///
/// `random(n)` must return a value in `0..n` (it is reduced modulo `n` anyway). Options
/// with a weight below 1 are never drawn. Returns `None` when (species, sex) has no
/// category, nothing can be drawn, or no attempt was accepted. An option naming several
/// shift tables uses the first; one naming an undefined table gets no shifts (§8.4 step 1).
pub fn generate_name(
    generator: &NameGenerator,
    text: &NameText,
    species: u32,
    sex: u32,
    mut random: impl FnMut(u32) -> u32,
    mut accept: impl FnMut(&str) -> bool,
) -> Option<String> {
    let category = generator.category(generator.category_for(species, sex)?)?;
    let total: u64 = category
        .options
        .iter()
        .map(|o| u64::try_from(o.weight.max(0)).unwrap_or(0))
        .sum();
    let total = u32::try_from(total).ok().filter(|&t| t > 0)?;
    for _ in 0..MAX_NAME_ATTEMPTS {
        let mut r = random(total) % total;
        let mut chosen = None;
        for o in &category.options {
            let w = u32::try_from(o.weight.max(0)).unwrap_or(0);
            if r < w {
                chosen = Some(o);
                break;
            }
            r -= w;
        }
        let option = chosen?;
        let shifts = option.shift_tables.first().and_then(|k| text.shifts.get(k));
        let mut name: Option<String> = None;
        for key in &option.syllable_tables {
            let strings = text.syllables.get(key).filter(|v| !v.is_empty())?;
            let n = u32::try_from(strings.len()).unwrap_or(u32::MAX);
            let pick = usize::try_from(random(n) % n).unwrap_or(0);
            let part = strings.get(pick)?;
            name = Some(match name {
                None => part.clone(),
                Some(a) => match shifts {
                    Some(t) => t.join(&a, part),
                    None => a + part,
                },
            });
        }
        let name = name?;
        if accept(&name) {
            return Some(name);
        }
    }
    None
}
