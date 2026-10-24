// SPDX-License-Identifier: GPL-3.0-or-later
//! Hex readers of `dattool`'s command-line arguments (DIDs and DID high bytes).
//!
//! They live in the library, not in the binary's `util` module, so that `fuzz/` can drive
//! them (target `dattool_hexarg`, P8 exit criterion 2, decision 0043 §2): the arguments are
//! text the user typed or pasted.

/// Strips an optional `0x`/`0X` prefix and `_` separators.
fn hex_digits(s: &str) -> String {
    let t = s.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    t.chars().filter(|&c| c != '_').collect()
}

/// Parses a 32-bit DID written in hex, with or without `0x` (`0x41000001`, `41000001`,
/// `4100_0001`). Bare digits are always hex here, so a DID copied from any `dattool`
/// output reads back unchanged. (`tessera-settings` reads bare digits as decimal; the two
/// conventions are not shared yet.)
pub fn parse_did(s: &str) -> Result<u32, String> {
    let d = hex_digits(s);
    if d.is_empty() || d.len() > 8 {
        return Err(format!(
            "`{s}` is not a hex DID (1-8 hex digits, optional 0x)"
        ));
    }
    u32::from_str_radix(&d, 16).map_err(|_| format!("`{s}` is not a hex DID"))
}

/// Parses a DID high byte written in hex, with or without `0x` (`0x41`, `41`).
pub fn parse_prefix(s: &str) -> Result<u8, String> {
    let d = hex_digits(s);
    if d.is_empty() || d.len() > 2 {
        return Err(format!(
            "`{s}` is not a hex byte (1-2 hex digits, optional 0x)"
        ));
    }
    u8::from_str_radix(&d, 16).map_err(|_| format!("`{s}` is not a hex byte"))
}
