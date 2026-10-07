// SPDX-License-Identifier: GPL-3.0-or-later
//! The word check of `test-server.md` §5.6 item 4 (`server-content.md` §9.3).
//!
//! Every text and name of a content file and of the configuration's `[server] name` and
//! `motd` is checked, ignoring case, against the words and phrases of LEGAL.md §10 rule 1.
//! A word matches only at word boundaries: a boundary is the start, the end, or a character
//! that is not a letter or digit. The text is split into words at boundaries, and a listed
//! phrase matches a run of consecutive words, so `Middle-earth` also matches `middle earth`
//! and `Helm's Deep` matches `helm’s deep`. Ordinary words that merely contain a listed word
//! do not match.
//!
//! The list is project policy compiled into the server, not game data. It cannot catch every
//! invented word; the human review items of `server-content.md` §9.3 cover the rest.

/// The words and phrases of LEGAL.md §10 rule 1, as written there.
pub const RULE_ONE: &[&str] = &[
    "LOTRO",
    "Lord of the Rings",
    "LotR",
    "Middle-earth",
    "Tolkien",
    "Hobbit",
    "Shire",
    "Mordor",
    "Angmar",
    "Moria",
    "Mirkwood",
    "Isengard",
    "Rohan",
    "Helm's Deep",
    "Mithril",
    "Turbine",
    "Standing Stone",
    "SSG",
    "Daybreak",
];

/// The text's words: maximal runs of letters and digits, lowercased.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The first listed word or phrase that occurs in `text` (as written in [`RULE_ONE`]).
#[must_use]
pub fn find_rule_one(text: &str) -> Option<&'static str> {
    let ws = words(text);
    if ws.is_empty() {
        return None;
    }
    RULE_ONE.iter().copied().find(|phrase| {
        let p = words(phrase);
        !p.is_empty() && ws.windows(p.len()).any(|w| w == p.as_slice())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries() {
        assert_eq!(find_rule_one("A shire test"), Some("Shire"));
        assert_eq!(find_rule_one("SHIRE"), Some("Shire"));
        assert_eq!(find_rule_one("shires and hobbits"), None);
        assert_eq!(
            find_rule_one("the middle earth of it"),
            Some("Middle-earth")
        );
        assert_eq!(find_rule_one("Helm’s deep"), Some("Helm's Deep"));
        assert_eq!(find_rule_one("standing stones"), None);
        assert_eq!(find_rule_one("a standing stone"), Some("Standing Stone"));
        assert_eq!(
            find_rule_one("lord of the rings"),
            Some("Lord of the Rings")
        );
        assert_eq!(find_rule_one("Tessera test world"), None);
        assert_eq!(find_rule_one(""), None);
        assert_eq!(find_rule_one("x-ssg-y"), Some("SSG"));
        assert_eq!(find_rule_one("turbines"), None);
    }
}
