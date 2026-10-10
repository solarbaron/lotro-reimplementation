// SPDX-License-Identifier: GPL-3.0-or-later
//! Name checks (`misc-records.md` §9.1, §9.2; `player-character.md` §8.6) and random
//! names (`player-character.md` §8.5, Decision D2) for the creation screen.
//!
//! The name-filter table's rule sets are decoded by `tessera-formats`
//! ([`NameRuleSet`]); this module applies them. The context bit and rule set used for
//! character names, and the order of the checks, are not known from the original
//! (`misc-records.md` §9.3 OPEN (1)); [`NamePolicy::tessera_default`] is Tessera's choice.
//! The taboo check (`misc-records.md` §10.1) is not applied: its tokenizer is OPEN there
//! and the workspace has no PCRE-compatible matcher.
//!
//! Characters are Unicode scalar values here (the client works on UTF-16 code units; the
//! two agree outside the supplementary planes). Letter, digit and printable are
//! approximated with the standard library's Unicode properties: letter = alphabetic,
//! decimal digit = numeric and not alphabetic, printable = not a control character and
//! not one of the common format characters; the spec's Tessera reading (categories L*,
//! Nd, and "not control, format, surrogate or unassigned") needs tables the workspace does
//! not carry.

use tessera_formats::gameplay::misc::{NameFilterTable, NameRuleSet};
use tessera_wstate::character::{GeneratedName, NameGenerator, NameText, generate_name};

use crate::order::SplitMix64;

/// The vowels of the client's vowel test (`misc-records.md` §9.2; decision 0012
/// constants with BIN provenance there).
pub const VOWELS: [u32; 26] = [
    0x41, 0x45, 0x49, 0x4F, 0x55, 0x61, 0x65, 0x69, 0x6F, 0x75, 0xC0, 0xC1, 0xC4, 0xC8, 0xC9, 0xD6,
    0xDA, 0xDC, 0xE0, 0xE1, 0xE4, 0xE8, 0xE9, 0xF6, 0xF9, 0xFC,
];

/// Why a name was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameCheckFailure {
    /// The name is empty (§9.1 string check).
    Empty,
    /// A character fails the character check of §9.1 (or the rule set is unknown).
    Character {
        /// Its position (in characters).
        index: usize,
        /// The character.
        ch: char,
    },
    /// §9.2 step 1: too many identical characters in a row.
    Repeats,
    /// §9.2 step 3: no vowel early enough.
    NoEarlyVowel,
    /// §9.2 step 4: too many vowels in a row.
    VowelRun,
    /// §9.2 step 5: too many non-vowels in a row.
    ConsonantRun,
    /// §9.2 step 6: a forbidden first character.
    FirstCharacter,
}

impl std::fmt::Display for NameCheckFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("the name is empty"),
            Self::Character { index, ch } => {
                write!(f, "character {ch:?} at position {index} is not allowed")
            }
            Self::Repeats => f.write_str("too many identical characters in a row"),
            Self::NoEarlyVowel => f.write_str("no vowel near the start"),
            Self::VowelRun => f.write_str("too many vowels in a row"),
            Self::ConsonantRun => f.write_str("too many consonants in a row"),
            Self::FirstCharacter => f.write_str("the first character is not allowed"),
        }
    }
}

fn chars(units: &[u16]) -> Vec<char> {
    char::decode_utf16(units.iter().copied())
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// The Unicode simple lower-case mapping of `c` (unchanged when it has none).
///
/// The standard library gives the full mapping; it differs from the simple one only where
/// the full mapping has more than one character, and among those only U+0130 (capital I
/// with dot above, full `i` + U+0307) has a simple mapping (U+0069). Every other
/// multi-character result means "no simple mapping".
pub(crate) fn lower(c: char) -> char {
    if c == '\u{130}' {
        return 'i';
    }
    let mut l = c.to_lowercase();
    match (l.next(), l.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

fn is_letter(c: char) -> bool {
    c.is_alphabetic()
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit() || (c.is_numeric() && !c.is_alphabetic())
}

fn is_printable(c: char) -> bool {
    let format = matches!(
        u32::from(c),
        0xAD | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x180E | 0x200B..=0x200F
            | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB
    );
    !c.is_control() && !format
}

/// Field meanings of a rule set (`misc-records.md` §9 table, rows 1–7).
fn n(rule: &NameRuleSet, i: usize) -> u32 {
    rule.numbers.get(i).copied().unwrap_or(0)
}

/// Whether character `c` is allowed in context `m` under `rule` (`misc-records.md` §9.1).
#[must_use]
pub fn char_allowed(rule: &NameRuleSet, m: u32, c: char) -> bool {
    if c == '[' || c == ']' {
        return false;
    }
    let class = rule.character_classes.iter().find(|k| k.key == m);
    if class.is_some_and(|k| chars(k.characters.units()).contains(&c)) {
        return true;
    }
    let typed = (n(rule, 0) & m != 0 && is_letter(c))
        || (n(rule, 1) & m != 0 && is_digit(c))
        || (n(rule, 2) & m != 0 && is_printable(c));
    if !typed {
        return false;
    }
    let cu = u32::from(c);
    !class.is_some_and(|k| {
        k.strings.iter().any(|r| match r.units() {
            [first, last, ..] => (u32::from(*first)..=u32::from(*last)).contains(&cu),
            _ => false,
        })
    })
}

/// The string check of §9.1 in context `m`: rejects an empty string and any string with a
/// character that fails [`char_allowed`]; an unknown rule set (`None`) allows no character.
///
/// # Errors
///
/// The first failure.
pub fn check_characters(
    rule: Option<&NameRuleSet>,
    m: u32,
    name: &str,
) -> std::result::Result<(), NameCheckFailure> {
    if name.is_empty() {
        return Err(NameCheckFailure::Empty);
    }
    for (index, ch) in name.chars().enumerate() {
        if !rule.is_some_and(|r| char_allowed(r, m, ch)) {
            return Err(NameCheckFailure::Character { index, ch });
        }
    }
    Ok(())
}

/// The name check of §9.2 on a lower-cased copy of `name`. An unknown rule set (`None`)
/// passes every step.
///
/// # Errors
///
/// The first failing step.
pub fn check_name_rules(
    rule: Option<&NameRuleSet>,
    name: &str,
) -> std::result::Result<(), NameCheckFailure> {
    let Some(rule) = rule else {
        return Ok(());
    };
    let s: Vec<char> = name.chars().map(lower).collect();
    let extra = chars(rule.string_a.units());
    let is_vowel = |c: char| VOWELS.contains(&u32::from(lower(c)));
    let vowelish = |c: char| is_vowel(c) || extra.contains(&c);
    // Step 1: repeats.
    let max_repeat = n(rule, 3);
    if max_repeat != 0 {
        let mut run = 0u32;
        for (i, c) in s.iter().enumerate() {
            run = if i > 0 && s.get(i - 1) == Some(c) {
                run + 1
            } else {
                1
            };
            if run > max_repeat {
                return Err(NameCheckFailure::Repeats);
            }
        }
    }
    // Step 2: collapsed copy.
    let groups: Vec<Vec<char>> = rule
        .string_list_a
        .iter()
        .map(|g| chars(g.units()).into_iter().map(lower).collect::<Vec<_>>())
        .filter(|g| !g.is_empty())
        .collect();
    let mut collapsed = Vec::with_capacity(s.len());
    let mut i = 0;
    while let Some(&c) = s.get(i) {
        collapsed.push(c);
        let rest = s.get(i..).unwrap_or(&[]);
        let step = groups
            .iter()
            .find(|g| {
                let m = g.len().min(rest.len());
                g.iter()
                    .zip(rest)
                    .take(m)
                    .all(|(a, b)| lower(*a) == lower(*b))
            })
            .map_or(1, Vec::len);
        i = i.saturating_add(step);
    }
    // Step 3: early vowel.
    let within = n(rule, 5) as usize;
    if within != 0 && !collapsed.iter().take(within).any(|c| vowelish(*c)) {
        return Err(NameCheckFailure::NoEarlyVowel);
    }
    // Step 4: vowel run on the lower-cased name.
    let max_vowels = n(rule, 4);
    if max_vowels != 0 {
        let mut run = 0u32;
        for c in &s {
            run = if is_vowel(*c) && !extra.contains(c) {
                run + 1
            } else {
                0
            };
            if run > max_vowels {
                return Err(NameCheckFailure::VowelRun);
            }
        }
    }
    // Step 5: consonant run on the collapsed copy.
    let k = n(rule, 6) as usize;
    if k != 0 {
        if collapsed.len() < k {
            if !collapsed.iter().any(|c| vowelish(*c)) {
                return Err(NameCheckFailure::ConsonantRun);
            }
        } else if collapsed
            .windows(k)
            .any(|w| w.iter().all(|c| !vowelish(*c)))
        {
            return Err(NameCheckFailure::ConsonantRun);
        }
    }
    // Step 6: first character.
    if let Some(first) = s.first()
        && chars(rule.string_b.units()).contains(first)
    {
        return Err(NameCheckFailure::FirstCharacter);
    }
    Ok(())
}

/// The name check the creation screen applies: one rule set of the name-filter table and a
/// context bit (`misc-records.md` §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamePolicy {
    /// The rule set (`None`: an unknown rule-set key, which allows no character).
    pub rule: Option<NameRuleSet>,
    /// The context bit passed to the character check.
    pub context: u32,
}

impl NamePolicy {
    /// **Tessera rule** (`misc-records.md` §9.3 lets Tessera pick context and order): the
    /// rule set of the first key of the generator's `RandomNameGeneration_ValidNameRules`
    /// (`player-character.md` §8.2) that the name-filter table holds, and as context the
    /// lowest bit of that rule set's `letter_classes` (a context in which letters are
    /// allowed). `None` when no listed key has a rule set or its `letter_classes` is 0.
    #[must_use]
    pub fn tessera_default(table: &NameFilterTable, valid_rules: &[u32]) -> Option<Self> {
        let rule = valid_rules
            .iter()
            .find_map(|k| table.rule_sets.iter().find(|r| r.rule_key == *k))?;
        let letters = n(rule, 0);
        (letters != 0).then(|| Self {
            rule: Some(rule.clone()),
            context: 1 << letters.trailing_zeros(),
        })
    }

    /// Checks a typed or generated name: the string check of §9.1 on the name as given,
    /// then the name check of §9.2 (Tessera's order).
    ///
    /// # Errors
    ///
    /// The first failure.
    pub fn check(&self, name: &str) -> std::result::Result<(), NameCheckFailure> {
        check_characters(self.rule.as_ref(), self.context, name)?;
        check_name_rules(self.rule.as_ref(), name)
    }
}

/// What random names need: the generator, its resolved texts and the name check.
#[derive(Debug, Clone)]
pub struct NameData {
    /// The random-name generator (`player-character.md` §8.2).
    pub generator: NameGenerator,
    /// Its syllables and compiled shift tables.
    pub text: NameText,
    /// The check generated names must pass (`None`: no check, every generated name is
    /// accepted).
    pub policy: Option<NamePolicy>,
}

impl NameData {
    /// A random name for (`species`, `sex`) by `player-character.md` §8.5 under Decision
    /// D2: Tessera's generator ([`SplitMix64`] seeded with `seed`), the policy's check in
    /// place of the client's, at most 100 attempts. The nationality does not choose the
    /// generator (§8.3).
    ///
    /// The result has one of three outcomes:
    ///
    /// - **accepted** (`accepted == true`): `name` passed the policy's check;
    /// - **rejected** (`accepted == false`, `name` not empty): no candidate passed in the
    ///   100 attempts and `name` is the **last generated, unchecked** candidate
    ///   (`player-character.md` §8.5 "leaves the last generated name");
    /// - **failed** (`name` empty): the last generation failed.
    ///
    /// A screen fills the name field without a warning only when `accepted` is true.
    #[must_use]
    pub fn random_name(&self, species: u32, sex: u32, seed: u64) -> GeneratedName {
        let mut rng = SplitMix64::new(seed);
        generate_name(
            &self.generator,
            &self.text,
            species,
            sex,
            |n| u32::try_from(rng.below(u64::from(n))).unwrap_or(0),
            |lowered| {
                self.policy
                    .as_ref()
                    .is_none_or(|p| p.check(lowered).is_ok())
            },
        )
    }

    /// Checks a typed name with the policy (no policy: only non-empty).
    ///
    /// # Errors
    ///
    /// The first failure.
    pub fn check(&self, name: &str) -> std::result::Result<(), NameCheckFailure> {
        match &self.policy {
            Some(p) => p.check(name),
            None if name.is_empty() => Err(NameCheckFailure::Empty),
            None => Ok(()),
        }
    }
}
