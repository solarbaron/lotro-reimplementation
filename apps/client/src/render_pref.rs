// SPDX-License-Identifier: GPL-3.0-or-later
//! `--render-pref KEY=VALUE` (`walker.md` §14.10, `far-view.md` FD22): the parser of one use
//! of the option and the look-up of a key's value over all uses.
//!
//! | Item | Spec |
//! |---|---|
//! | [`parse_render_pref`], [`RenderPref`], [`RenderPrefError`] | `walker.md` §14.10 (one `KEY=VALUE`, trimmed) |
//! | [`render_pref`] | `walker.md` §14.10, W26 (case-insensitive key, the last use wins) |
//!
//! The value forms (`True`/`False`, level names, numbers) are read by each key's consumer
//! (`walker.md` W26, `far-view.md` FD17, FD18), not here. The bounds on the key and the
//! value ([`MAX_RENDER_PREF_KEY`], [`MAX_RENDER_PREF_VALUE`]) are a **Tessera choice**: the
//! install's `[Render]` keys and values are far shorter.
//!
//! This module depends on `std` only, so the fuzz target `client_render_pref` includes it
//! by path; `cli.rs` hands [`parse_render_pref`] to clap as the option's value parser. The
//! text is hostile: nothing panics and nothing is allocated before the bounds are checked.

use std::fmt;

/// Longest key name of a `--render-pref` (characters; the key is ASCII).
pub const MAX_RENDER_PREF_KEY: usize = 64;
/// Longest value of a `--render-pref` (characters).
pub const MAX_RENDER_PREF_VALUE: usize = 64;

/// One `--render-pref KEY=VALUE` (`walker.md` §14.10): a `[Render]` key and its stored
/// value form for this run, both trimmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPref {
    /// The key name as given (compared case-insensitively).
    pub key: String,
    /// The value as given (`True`/`False`, a level name or a number; read by the key's
    /// consumer).
    pub value: String,
}

/// Why a `--render-pref` text was refused ([`parse_render_pref`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPrefError {
    /// The text has no `=`.
    MissingEquals,
    /// The key is empty after trimming.
    EmptyKey,
    /// The key is longer than [`MAX_RENDER_PREF_KEY`] characters.
    KeyTooLong,
    /// The key holds a character other than an ASCII letter, digit or `_`.
    BadKeyCharacter,
    /// The value is longer than [`MAX_RENDER_PREF_VALUE`] characters.
    ValueTooLong,
    /// The value holds a control character.
    ControlCharacter,
}

impl fmt::Display for RenderPrefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEquals => f.write_str("expected KEY=VALUE"),
            Self::EmptyKey => f.write_str("the key is empty"),
            Self::KeyTooLong => {
                write!(f, "the key is longer than {MAX_RENDER_PREF_KEY} characters")
            }
            Self::BadKeyCharacter => f.write_str("the key may hold only letters, digits and `_`"),
            Self::ValueTooLong => write!(
                f,
                "the value is longer than {MAX_RENDER_PREF_VALUE} characters"
            ),
            Self::ControlCharacter => f.write_str("the value holds a control character"),
        }
    }
}

impl std::error::Error for RenderPrefError {}

/// Parses `KEY=VALUE` (split at the first `=`, both sides trimmed): a key of ASCII letters,
/// digits and `_` (1 to [`MAX_RENDER_PREF_KEY`] characters) and a value of at most
/// [`MAX_RENDER_PREF_VALUE`] characters without control characters. The value may be
/// empty: the key's built-in value then applies, as for any value the key does not accept
/// (`walker.md` W26).
///
/// # Errors
///
/// [`RenderPrefError`] when the text has no `=`, the key is empty, too long or holds other
/// characters, or the value is too long or holds a control character.
pub fn parse_render_pref(s: &str) -> Result<RenderPref, RenderPrefError> {
    let (key, value) = s.split_once('=').ok_or(RenderPrefError::MissingEquals)?;
    let (key, value) = (key.trim(), value.trim());
    if key.is_empty() {
        return Err(RenderPrefError::EmptyKey);
    }
    if key.len() > MAX_RENDER_PREF_KEY {
        return Err(RenderPrefError::KeyTooLong);
    }
    if !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return Err(RenderPrefError::BadKeyCharacter);
    }
    // Counting stops one past the bound, so a long value costs no more than the bound.
    if value.chars().take(MAX_RENDER_PREF_VALUE + 1).count() > MAX_RENDER_PREF_VALUE {
        return Err(RenderPrefError::ValueTooLong);
    }
    if value.chars().any(char::is_control) {
        return Err(RenderPrefError::ControlCharacter);
    }
    Ok(RenderPref {
        key: key.to_owned(),
        value: value.to_owned(),
    })
}

/// The value `--render-pref` gives `key` (case-insensitive): that of its last use, `None`
/// when no use names it.
#[must_use]
pub fn render_pref<'a>(prefs: &'a [RenderPref], key: &str) -> Option<&'a str> {
    prefs
        .iter()
        .rev()
        .find(|p| p.key.eq_ignore_ascii_case(key))
        .map(|p| p.value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pref(key: &str, value: &str) -> RenderPref {
        RenderPref {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Both sides are trimmed, the spelling is kept, and only the first `=` splits.
    #[test]
    fn parses_and_trims() {
        assert_eq!(
            parse_render_pref(" MultiPassLighting = False "),
            Ok(pref("MultiPassLighting", "False"))
        );
        assert_eq!(
            parse_render_pref("AtmosphericsDetail="),
            Ok(pref("AtmosphericsDetail", ""))
        );
        assert_eq!(parse_render_pref("Key=a=b"), Ok(pref("Key", "a=b")));
        assert_eq!(
            parse_render_pref("Name=Très"),
            Ok(pref("Name", "Très")),
            "a non-ASCII value is kept"
        );
    }

    /// The last use of a key wins, compared case-insensitively.
    #[test]
    fn last_use_wins_case_insensitively() {
        let prefs = [
            pref("FarLandscapeNormalMaps", "True"),
            pref("MultiPassLighting", "False"),
            pref("multipasslighting", "True"),
        ];
        assert_eq!(render_pref(&prefs, "FARLANDSCAPENORMALMAPS"), Some("True"));
        assert_eq!(render_pref(&prefs, "MultiPassLighting"), Some("True"));
        assert_eq!(render_pref(&prefs, "Gamma"), None);
        assert_eq!(render_pref(&[], "Gamma"), None);
    }

    /// Malformed texts are refused with the matching error (bounded key and value).
    #[test]
    fn malformed_texts_are_refused() {
        use RenderPrefError as E;
        for (bad, want) in [
            ("NoEquals", E::MissingEquals),
            ("", E::MissingEquals),
            ("=True", E::EmptyKey),
            (" =True", E::EmptyKey),
            ("Bad Key=1", E::BadKeyCharacter),
            ("Bad-Key=1", E::BadKeyCharacter),
            ("Clé=1", E::BadKeyCharacter),
            ("Key=line\nbreak", E::ControlCharacter),
            ("Key=a\u{7f}b", E::ControlCharacter),
        ] {
            assert_eq!(parse_render_pref(bad), Err(want), "{bad:?}");
        }
        let long_key = format!("{}=1", "K".repeat(MAX_RENDER_PREF_KEY + 1));
        assert_eq!(parse_render_pref(&long_key), Err(E::KeyTooLong));
        let ok_key = format!("{}=1", "K".repeat(MAX_RENDER_PREF_KEY));
        assert!(parse_render_pref(&ok_key).is_ok());
        let long_value = format!("Key={}", "v".repeat(MAX_RENDER_PREF_VALUE + 1));
        assert_eq!(parse_render_pref(&long_value), Err(E::ValueTooLong));
        let ok_value = format!("Key={}", "é".repeat(MAX_RENDER_PREF_VALUE));
        assert!(
            parse_render_pref(&ok_value).is_ok(),
            "characters, not bytes"
        );
        for e in [
            E::MissingEquals,
            E::EmptyKey,
            E::KeyTooLong,
            E::BadKeyCharacter,
            E::ValueTooLong,
            E::ControlCharacter,
        ] {
            assert!(!e.to_string().is_empty());
        }
    }

    /// `parse_render_pref` on arbitrary text (a deterministic sweep alongside the fuzz
    /// target `client_render_pref`): it never panics, and what it accepts has a bounded key
    /// of letters, digits and `_` and a bounded value without control characters, both
    /// trimmed.
    #[test]
    fn takes_arbitrary_text() {
        const ALPHABET: &[char] = &[
            'a', 'Z', '0', '_', '=', ' ', '\t', '\n', '-', '.', '\u{0}', 'é', '\u{7f}', '=',
        ];
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..20_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let len = (state >> 58) as usize * 3;
            let mut text = String::new();
            let mut x = state;
            for _ in 0..len {
                x = x
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let i = usize::try_from((x >> 33) % ALPHABET.len() as u64).unwrap_or(0);
                text.push(ALPHABET.get(i).copied().unwrap_or('a'));
            }
            if let Ok(p) = parse_render_pref(&text) {
                assert!(!p.key.is_empty() && p.key.len() <= MAX_RENDER_PREF_KEY);
                assert!(
                    p.key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                );
                assert!(p.value.chars().count() <= MAX_RENDER_PREF_VALUE);
                assert!(!p.value.chars().any(char::is_control));
                assert_eq!(p.value.trim(), p.value);
            }
        }
    }
}
