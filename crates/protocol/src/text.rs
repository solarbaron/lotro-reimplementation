// SPDX-License-Identifier: GPL-3.0-or-later
//! Text classes of `tessera-protocol.md` §5.2: content checks and sanitising.
//!
//! The decoder enforces only the wire rules (UTF-8, length range). For text a client sends,
//! the content rules are the server's to check with these functions, because breaking them
//! gets the request's own failure code rather than a protocol violation. Text from the server
//! is sanitised by the decoder.

use std::borrow::Cow;

use crate::limits::{MAX_ACCOUNT_LEN, MAX_NAME_LEN, MAX_PASSWORD_LEN, MAX_PLACE_LEN, MAX_TEXT_LEN};

/// Whether `s` holds a control character (Unicode general category Cc: U+0000–U+001F and
/// U+007F–U+009F).
#[must_use]
pub fn has_control(s: &str) -> bool {
    s.chars().any(char::is_control)
}

/// `s` with every control character replaced by U+0020 (§5.2 "sanitised"). Borrows when
/// nothing changes. Unlike `tessera_net::model::sanitize_text` this never shortens the text;
/// lengths are checked before.
#[must_use]
pub fn sanitise(s: &str) -> Cow<'_, str> {
    if has_control(s) {
        Cow::Owned(
            s.chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect(),
        )
    } else {
        Cow::Borrowed(s)
    }
}

/// `NAME`: 1–64 bytes, no control characters.
#[must_use]
pub fn is_valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_NAME_LEN && !has_control(s)
}

/// `TEXT`: 1–1,024 bytes, no control characters.
#[must_use]
pub fn is_valid_text(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_TEXT_LEN && !has_control(s)
}

/// `ACCOUNT`: 1–32 bytes of ASCII letters, digits, `.`, `_` and `-`.
#[must_use]
pub fn is_valid_account(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_ACCOUNT_LEN
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// `PASSWORD`: 1–256 bytes of any UTF-8.
#[must_use]
pub fn is_valid_password(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_PASSWORD_LEN
}

/// `PLACE`: 1–256 bytes, no control characters, not only whitespace.
#[must_use]
pub fn is_valid_place(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_PLACE_LEN && !has_control(s) && !s.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes() {
        assert!(is_valid_name("Aldith"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("a\u{7}b"));
        assert!(!is_valid_name(&"x".repeat(65)));
        assert!(is_valid_account("alice.b_c-9"));
        assert!(!is_valid_account("al ice"));
        assert!(!is_valid_account("ålice"));
        assert!(!is_valid_account(&"a".repeat(33)));
        assert!(is_valid_place("origin"));
        assert!(!is_valid_place("   "));
        assert!(!is_valid_place("a\u{85}"));
        assert!(is_valid_password("ü"));
        assert!(!is_valid_password(""));
        assert!(is_valid_text("hello"));
        assert!(!is_valid_text(""));
    }

    #[test]
    fn sanitising() {
        assert_eq!(sanitise("a\nb\u{9f}c"), "a b c");
        assert!(matches!(sanitise("plain"), Cow::Borrowed(_)));
    }
}
