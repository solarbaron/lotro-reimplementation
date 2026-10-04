// SPDX-License-Identifier: GPL-3.0-or-later
//! Grammar tags: value tag groups, selector tag lists, tag sets and option selection.
//!
//! Implements `docs/specs/string-table.md` §4.3 (tag-list syntax), §4.9 (value tag
//! groups), §5 (tag alphabet, derived tags, numeric tags) and §6 (option selection). The
//! selection rule is language-neutral: no tag letter is special-cased except the derived
//! tags `E` and `b` and numeric tags (spec §10.2).

/// Derived tag: the value's display text is not empty (spec §5.3).
pub const TAG_NON_EMPTY: char = 'E';
/// Derived tag: the value's display text is empty (spec §5.3).
pub const TAG_BLANK: char = 'b';

/// What one tag tests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TagAtom {
    /// A single character, compared case-sensitively with the value's tag set.
    Letter(char),
    /// A numeric tag: satisfied when the value is a number equal to this one (spec §5.4).
    Number(f64),
}

/// One tag of a selector option's tag list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tag {
    /// `!` before the tag: satisfied when the plain test fails.
    pub negated: bool,
    /// The test.
    pub atom: TagAtom,
}

/// Parses a selector option's tag list (the characters between `[` and `]`), spec §4.3
/// step 5: `,` separates, `!` negates the next tag, a run of ASCII digits optionally
/// followed by `.` and more digits is one numeric tag, any other character is a letter tag.
#[must_use]
pub fn parse_tag_list(raw: &str) -> Vec<Tag> {
    let mut out = Vec::new();
    let mut negated = false;
    let b = raw.as_bytes();
    let mut i = 0;
    while let Some(c) = raw[i..].chars().next() {
        if c == ',' {
            i += 1;
            continue;
        }
        if c == '!' {
            negated = !negated;
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if b.get(i) == Some(&b'.') && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
                i += 1;
                while b.get(i).is_some_and(u8::is_ascii_digit) {
                    i += 1;
                }
            }
            // A digit string always parses as f64 (very long runs become infinity).
            let n = raw[start..i].parse::<f64>().unwrap_or(f64::NAN);
            out.push(Tag {
                negated,
                atom: TagAtom::Number(n),
            });
        } else {
            out.push(Tag {
                negated,
                atom: TagAtom::Letter(c),
            });
            i += c.len_utf8();
        }
        negated = false;
    }
    out
}

/// One option of a selector (spec §4.3).
#[derive(Debug, Clone, PartialEq)]
pub struct SelectorOption {
    /// Option text, raw (escapes and markup are decoded later), spaces kept.
    pub text: String,
    /// The tag list; `None` when the option has no `[`, which makes it a default candidate.
    pub tags: Option<Vec<Tag>>,
}

/// Splits a selector body on `|` into options (spec §4.3 steps 2–4).
#[must_use]
pub fn parse_options(body: &str) -> Vec<SelectorOption> {
    body.split('|')
        .map(|opt| match opt.find('[') {
            None => SelectorOption {
                text: opt.to_owned(),
                tags: None,
            },
            Some(open) => {
                let after = &opt[open + 1..];
                // Up to the next ']', or to the end when there is none; the rest is ignored.
                let list = after.find(']').map_or(after, |close| &after[..close]);
                SelectorOption {
                    text: opt[..open].to_owned(),
                    tags: Some(parse_tag_list(list)),
                }
            }
        })
        .collect()
}

/// The tag set of a value (spec §5.3): stored tags, derived `E`/`b`, and the numeric value
/// used by numeric tags.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TagSet {
    letters: Vec<char>,
    number: Option<f64>,
}

impl TagSet {
    /// The tag set of an absent value: `{b}`.
    #[must_use]
    pub fn absent() -> Self {
        Self {
            letters: vec![TAG_BLANK],
            number: None,
        }
    }

    /// Builds a tag set from stored tag letters, the emptiness of the display text and an
    /// optional numeric value.
    #[must_use]
    pub fn new(stored: &str, display_empty: bool, number: Option<f64>) -> Self {
        let mut letters: Vec<char> = stored.chars().collect();
        letters.push(if display_empty {
            TAG_BLANK
        } else {
            TAG_NON_EMPTY
        });
        letters.sort_unstable();
        letters.dedup();
        Self { letters, number }
    }

    /// Whether the set holds the letter tag `c` (case-sensitive).
    #[must_use]
    pub fn contains(&self, c: char) -> bool {
        self.letters.binary_search(&c).is_ok()
    }

    /// The letters in the set, sorted.
    #[must_use]
    pub fn letters(&self) -> &[char] {
        &self.letters
    }

    /// The value's numeric value, if it is a number.
    #[must_use]
    pub fn number(&self) -> Option<f64> {
        self.number
    }

    /// Whether `tag` is satisfied by this set (spec §6 step 1, §5.4).
    #[must_use]
    pub fn satisfies(&self, tag: &Tag) -> bool {
        let hit = match tag.atom {
            TagAtom::Letter(c) => self.contains(c),
            TagAtom::Number(n) => self.number == Some(n),
        };
        hit != tag.negated
    }
}

/// Chooses an option (spec §6): the first option with the highest score > 0 among options
/// with a tag list; else the first option without a tag list; else `None` (renders as
/// nothing). The score counts satisfied tags, repeated tags each time.
#[must_use]
pub fn select_option(options: &[SelectorOption], set: &TagSet) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None;
    for (i, opt) in options.iter().enumerate() {
        if let Some(tags) = &opt.tags {
            let score = tags.iter().filter(|t| set.satisfies(t)).count();
            if score > 0 && best.is_none_or(|(_, s)| score > s) {
                best = Some((i, score));
            }
        }
    }
    best.map(|(i, _)| i)
        .or_else(|| options.iter().position(|o| o.tags.is_none()))
}

/// Whether the byte at `pos` is escaped by an odd number of backslashes before it.
fn is_escaped(bytes: &[u8], pos: usize) -> bool {
    let run = bytes[..pos]
        .iter()
        .rev()
        .take_while(|&&b| b == b'\\')
        .count();
    run % 2 == 1
}

/// Detects a value tag group (spec §4.9) on raw text and splits it off.
///
/// Returns the display raw text and the group's characters. A group is `[`, one or more
/// ASCII letters or digits, `]` at the very end (trailing U+0020 allowed). A `]` or `[`
/// stored escaped (`\]`, `\[`) does not delimit a group. For display the group is removed
/// together with spaces directly before `[` and after `]`.
#[must_use]
pub fn split_tag_group(raw: &str) -> (&str, Option<&str>) {
    let s = raw.trim_end_matches(' ');
    let b = s.as_bytes();
    if b.last() != Some(&b']') || is_escaped(b, b.len() - 1) {
        return (raw, None);
    }
    let close = b.len() - 1;
    let Some(open) = s[..close].rfind('[') else {
        return (raw, None);
    };
    let group = &s[open + 1..close];
    if group.is_empty() || !group.bytes().all(|c| c.is_ascii_alphanumeric()) || is_escaped(b, open)
    {
        return (raw, None);
    }
    (s[..open].trim_end_matches(' '), Some(group))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(raw: &str) -> TagSet {
        let (display, group) = split_tag_group(raw);
        TagSet::new(group.unwrap_or(""), display.is_empty(), None)
    }

    fn pick(body: &str, s: &TagSet) -> Option<String> {
        let opts = parse_options(body);
        select_option(&opts, s).map(|i| opts[i].text.clone())
    }

    #[test]
    fn tag_list_parsing() {
        let t = parse_tag_list("!b1");
        assert_eq!(t.len(), 2);
        assert!(t[0].negated && t[0].atom == TagAtom::Letter('b'));
        assert!(!t[1].negated && t[1].atom == TagAtom::Number(1.0));
        assert_eq!(parse_tag_list("mps").len(), 3);
        let t = parse_tag_list("51,0");
        assert_eq!(t[0].atom, TagAtom::Number(51.0));
        assert_eq!(t[1].atom, TagAtom::Number(0.0));
        assert_eq!(parse_tag_list("1.0")[0].atom, TagAtom::Number(1.0));
        // '.' not followed by a digit is a letter tag.
        let t = parse_tag_list("1.x");
        assert_eq!(t[1].atom, TagAtom::Letter('.'));
        assert_eq!(parse_tag_list("").len(), 0);
    }

    #[test]
    fn options() {
        let o = parse_options("She[f]|He[m] |They");
        assert_eq!(o.len(), 3);
        assert_eq!(o[0].text, "She");
        assert_eq!(o[1].text, "He");
        assert!(o[2].tags.is_none());
        let o = parse_options(" an[v");
        assert_eq!(o[0].text, " an");
        assert_eq!(o[0].tags.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn spec_table_rows() {
        let gender = "she[f]|he[m]|they";
        assert_eq!(
            pick(gender, &set("Wren Hollow[fn]")).as_deref(),
            Some("she")
        );
        assert_eq!(pick(gender, &set("Tobin Marsh[mn]")).as_deref(), Some("he"));
        assert_eq!(pick(gender, &set("Ash Owl[v]")).as_deref(), Some("they"));
        assert_eq!(
            pick("the [!n]", &set("Copper Kettle[e]")).as_deref(),
            Some("the ")
        );
        assert_eq!(pick("the [!n]", &set("Tobin Marsh[mn]")), None);
        let art = " [n]| [p]| an[v]| a";
        assert_eq!(pick(art, &set("Ash Owl[v]")).as_deref(), Some(" an"));
        assert_eq!(pick(art, &set("Copper Kettle[e]")).as_deref(), Some(" a"));
        assert_eq!(pick(art, &set("Owls[vp]")).as_deref(), Some(" "));
        let one = TagSet::new("", false, Some(1.0));
        let four = TagSet::new("", false, Some(4.0));
        assert_eq!(pick("acorn[1]|acorns", &one).as_deref(), Some("acorn"));
        assert_eq!(pick("acorn[1]|acorns", &four).as_deref(), Some("acorns"));
        assert_eq!(pick("acorn[1.0]|acorns", &one).as_deref(), Some("acorn"));
        assert_eq!(
            pick("'s|'[s]", &set("Moss Hills[ns]")).as_deref(),
            Some("'")
        );
        assert_eq!(pick(" [E]", &set("")), None);
        assert_eq!(pick(" [E]", &TagSet::absent()), None);
    }

    #[test]
    fn multi_tag_and_no_default() {
        let blank = TagSet::absent();
        let one = TagSet::new("", false, Some(1.0));
        let two = TagSet::new("", false, Some(2.0));
        let body = "hour[1!b]|hours[!b]";
        assert_eq!(pick(body, &one).as_deref(), Some("hour"));
        // Count rule as specified (§6): both options score 1 for the value 2 and the tie
        // goes to the earlier option. See the OPEN note in string-table.md §6.
        assert_eq!(pick(body, &two).as_deref(), Some("hour"));
        assert_eq!(pick(body, &blank), None);
        let body = "hour[!b1]|hours[!b]";
        assert_eq!(pick(body, &one).as_deref(), Some("hour"));
        // Count rule: either alternative number selects the option.
        let zero = TagSet::new("", false, Some(0.0));
        assert_eq!(pick("x[51,0]|y", &zero).as_deref(), Some("x"));
        // A tagged option with score 0 is never the default.
        assert_eq!(pick("x[m]|y[f]", &set("Ash[v]")), None);
        // Case-sensitive.
        assert_eq!(pick("a[P]|b[p]", &set("Owls[p]")).as_deref(), Some("b"));
    }

    #[test]
    fn tag_groups() {
        assert_eq!(split_tag_group("Reed Boots[p]"), ("Reed Boots", Some("p")));
        assert_eq!(
            split_tag_group("Wren Hollow [fn]  "),
            ("Wren Hollow", Some("fn"))
        );
        assert_eq!(split_tag_group(r"chat \[x\]"), (r"chat \[x\]", None));
        assert_eq!(split_tag_group(r"a[b\]"), (r"a[b\]", None));
        assert_eq!(split_tag_group(r"a\[b]"), (r"a\[b]", None));
        assert_eq!(split_tag_group(r"a\\[b]"), (r"a\\", Some("b")));
        assert_eq!(split_tag_group("x[a b]"), ("x[a b]", None));
        assert_eq!(split_tag_group("x[=]"), ("x[=]", None));
        assert_eq!(split_tag_group("x[]"), ("x[]", None));
        assert_eq!(split_tag_group("[p]"), ("", Some("p")));
        assert_eq!(split_tag_group("x[ABC]"), ("x", Some("ABC")));
        assert_eq!(split_tag_group("]"), ("]", None));
        assert_eq!(split_tag_group(""), ("", None));
    }
}
