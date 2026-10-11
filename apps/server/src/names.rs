// SPDX-License-Identifier: GPL-3.0-or-later
//! Account and character names (`test-server.md` §6.1, §6.4), chat-channel names (§9.1) and
//! content identifiers (§5.2, §5.3).

use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::UnicodeNormalization;

/// Shortest character name, in Unicode scalar values (§6.4).
pub const MIN_CHARACTER_NAME: usize = 2;
/// Longest character name, in Unicode scalar values (§6.4).
pub const MAX_CHARACTER_NAME: usize = 24;
/// Names reserved by the server (§6.4), compared by name key.
pub const RESERVED_NAMES: &[&str] = &["System", "Server"];

/// Longest chat-channel name, in Unicode scalar values (§9.1).
pub const MAX_CHANNEL_NAME: usize = 50;
/// Channel-name keys reserved by the server (§9.1), so that a channel never reads like a
/// built-in one.
pub const RESERVED_CHANNEL_KEYS: &[&str] = &[
    "say",
    "yell",
    "emote",
    "tell",
    "system",
    "server",
    "fellowship",
    "raid",
    "kinship",
    "world",
];

/// The canonical form of an account name: its ASCII lowercase (§6.1). `None` when `name` is
/// not an `ACCOUNT` string (`tessera-protocol.md` §5.2).
#[must_use]
pub fn canonical_account(name: &str) -> Option<String> {
    tessera_protocol::text::is_valid_account(name).then(|| name.to_ascii_lowercase())
}

fn is_letter(c: char) -> bool {
    matches!(
        get_general_category(c),
        GeneralCategory::UppercaseLetter
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::TitlecaseLetter
            | GeneralCategory::ModifierLetter
            | GeneralCategory::OtherLetter
    )
}

fn is_joiner(c: char) -> bool {
    matches!(c, ' ' | '\'' | '\u{2019}' | '-')
}

/// Applies the character name rule (§6.4): returns the NFC form, or `None` when the name
/// breaks the rule. 2–24 scalar values, starts with a letter, only letters (general category
/// L), single spaces, apostrophes (U+0027, U+2019) and hyphens, no two non-letters next to
/// each other, no non-letter at the end, no digits, and not a reserved name.
///
/// The NFC form must also still be a protocol `NAME` (at most 64 bytes, `tessera-protocol.md`
/// §5.2): a few letters grow under NFC (U+FA6C becomes the 4-byte U+242EE), and a stored
/// name that no message can carry would make every later answer naming it unencodable.
#[must_use]
pub fn character_name(name: &str) -> Option<String> {
    let nfc: String = name.nfc().collect();
    if !tessera_protocol::text::is_valid_name(&nfc) {
        return None;
    }
    let chars: Vec<char> = nfc.chars().collect();
    if chars.len() < MIN_CHARACTER_NAME || chars.len() > MAX_CHARACTER_NAME {
        return None;
    }
    if !chars.first().copied().is_some_and(is_letter)
        || !chars.last().copied().is_some_and(is_letter)
    {
        return None;
    }
    let mut prev_letter = true;
    for &c in &chars {
        if is_letter(c) {
            prev_letter = true;
        } else if is_joiner(c) {
            if !prev_letter {
                return None;
            }
            prev_letter = false;
        } else {
            return None;
        }
    }
    let key = name_key(&nfc);
    if RESERVED_NAMES.iter().any(|r| name_key(r) == key) {
        return None;
    }
    Some(nfc)
}

/// The name key: the full Unicode lowercase mapping of the NFC name (§6.4). Names are unique
/// by key; `Tell` recipients and `@name` teleports are matched by key.
#[must_use]
pub fn name_key(name: &str) -> String {
    let nfc: String = name.nfc().collect();
    nfc.to_lowercase()
}

fn is_channel_inner(c: char) -> bool {
    matches!(c, ' ' | '\'' | '\u{2019}' | '-' | '_')
}

fn is_letter_or_digit(c: char) -> bool {
    is_letter(c) || get_general_category(c) == GeneralCategory::DecimalNumber
}

/// Applies the chat-channel name rule (§9.1): returns the NFC form (the display name a new
/// channel gets), or `None` when the name breaks the rule (`INVALID_NAME`). 1–50 scalar
/// values of letters (general category L), decimal digits (Nd), single spaces, apostrophes
/// (U+0027, U+2019), hyphens and underscores; starts and ends with a letter or digit; the
/// NFC form is a protocol `NAME` (at most 64 bytes); its key is not reserved.
#[must_use]
pub fn channel_name(name: &str) -> Option<String> {
    let nfc: String = name.nfc().collect();
    if !tessera_protocol::text::is_valid_name(&nfc) {
        return None;
    }
    let chars: Vec<char> = nfc.chars().collect();
    if chars.is_empty() || chars.len() > MAX_CHANNEL_NAME {
        return None;
    }
    if !chars.first().copied().is_some_and(is_letter_or_digit)
        || !chars.last().copied().is_some_and(is_letter_or_digit)
    {
        return None;
    }
    let mut prev_space = false;
    for &c in &chars {
        if c == ' ' {
            if prev_space {
                return None;
            }
            prev_space = true;
        } else if is_letter_or_digit(c) || is_channel_inner(c) {
            prev_space = false;
        } else {
            return None;
        }
    }
    let key = channel_key(&nfc);
    if RESERVED_CHANNEL_KEYS.contains(&key.as_str()) {
        return None;
    }
    Some(nfc)
}

/// The channel key (§9.1): the full Unicode lowercase mapping of the NFC name, as for
/// character names. Requests and lines are matched to a channel by key.
#[must_use]
pub fn channel_key(name: &str) -> String {
    name_key(name)
}

/// A content identifier (`[world] id`): 1–32 characters of `a–z`, `0–9`, `-`.
#[must_use]
pub fn is_content_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts() {
        assert_eq!(
            canonical_account("Alice.B_1-x").as_deref(),
            Some("alice.b_1-x")
        );
        assert_eq!(canonical_account(""), None);
        assert_eq!(canonical_account("a b"), None);
        assert_eq!(canonical_account(&"a".repeat(33)), None);
    }

    #[test]
    fn character_names() {
        assert_eq!(character_name("Aldith").as_deref(), Some("Aldith"));
        assert_eq!(
            character_name("Ann-Marie O'Neil").as_deref(),
            Some("Ann-Marie O'Neil")
        );
        assert!(character_name("A").is_none());
        assert!(character_name("Ab1").is_none());
        assert!(character_name("Ab-").is_none());
        assert!(character_name("A--b").is_none());
        assert!(character_name("A  b").is_none());
        assert!(character_name("-Ab").is_none());
        assert!(character_name("system").is_none());
        assert!(character_name("SERVER").is_none());
        assert!(character_name(&"a".repeat(25)).is_none());
        assert!(character_name("Zoë").is_some());
        // NFD input is stored as NFC.
        assert_eq!(character_name("Zoe\u{308}").as_deref(), Some("Zo\u{eb}"));
        assert_eq!(name_key("ZOË"), name_key("zoë"));
    }

    #[test]
    fn nfc_growth_beyond_the_name_limit_is_refused() {
        // U+FA6C is 3 bytes; its NFC form U+242EE is 4. 21 of them: 63 bytes in, 84 out.
        let name: String = std::iter::repeat_n('\u{FA6C}', 21).collect();
        assert_eq!(name.len(), 63);
        assert!(tessera_protocol::text::is_valid_name(&name));
        assert!(character_name(&name).is_none());
        // 16 of them grow to 64 bytes: still a NAME.
        let name: String = std::iter::repeat_n('\u{FA6C}', 16).collect();
        let nfc = character_name(&name).expect("64 bytes after NFC");
        assert_eq!(nfc.len(), 64);
    }

    #[test]
    fn channel_names() {
        assert_eq!(channel_name("Lore").as_deref(), Some("Lore"));
        assert_eq!(channel_name("7").as_deref(), Some("7"));
        assert_eq!(
            channel_name("Bree-land Trade_2 O'Hare").as_deref(),
            Some("Bree-land Trade_2 O'Hare")
        );
        assert_eq!(channel_name("Zoe\u{308}s").as_deref(), Some("Zo\u{eb}s"));
        for bad in [
            "",
            " Lore",
            "Lore ",
            "Lo  re",
            "-Lore",
            "Lore_",
            "Lo.re",
            "Lo\u{7}re",
            "Lore!",
            "say",
            "FELLOWSHIP",
            "World",
            "Kinship",
        ] {
            assert!(channel_name(bad).is_none(), "{bad:?}");
        }
        assert!(channel_name(&"a".repeat(50)).is_some());
        assert!(channel_name(&"a".repeat(51)).is_none());
        // 50 scalar values, but more than 64 bytes.
        assert!(channel_name(&"é".repeat(33)).is_none());
        // Reserved words only as whole keys.
        assert!(channel_name("Say Hello").is_some());
        assert_eq!(channel_key("LORE"), channel_key("lore"));
        assert_eq!(channel_key("Zoë"), channel_key("ZOE\u{308}"));
    }

    #[test]
    fn ids() {
        assert!(is_content_id("default"));
        assert!(is_content_id("spawn-3"));
        assert!(!is_content_id("Spawn"));
        assert!(!is_content_id(""));
        assert!(!is_content_id(&"a".repeat(33)));
    }
}
