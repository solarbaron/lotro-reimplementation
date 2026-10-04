// SPDX-License-Identifier: GPL-3.0-or-later
//! Hex-dump formatting for the hex view.

use std::fmt::Write as _;

/// Bytes shown per hex-view line.
pub const BYTES_PER_LINE: usize = 16;

/// Number of lines needed for `len` bytes.
#[must_use]
pub fn line_count(len: usize) -> usize {
    len.div_ceil(BYTES_PER_LINE)
}

/// Formats line `line` of `data`: an 8-digit offset, 16 hex bytes in two groups of 8,
/// and the printable-ASCII column. Lines past the end give an empty string.
#[must_use]
pub fn hex_line(data: &[u8], line: usize) -> String {
    let Some(start) = line.checked_mul(BYTES_PER_LINE) else {
        return String::new();
    };
    let Some(chunk) = data.get(start..data.len().min(start.saturating_add(BYTES_PER_LINE))) else {
        return String::new();
    };
    if chunk.is_empty() {
        return String::new();
    }
    let mut s = String::with_capacity(80);
    let _ = write!(s, "{start:08x}  ");
    for i in 0..BYTES_PER_LINE {
        match chunk.get(i) {
            Some(b) => {
                let _ = write!(s, "{b:02x} ");
            }
            None => s.push_str("   "),
        }
        if i == 7 {
            s.push(' ');
        }
    }
    s.push(' ');
    s.push('|');
    for &b in chunk {
        s.push(if b.is_ascii_graphic() || b == b' ' {
            b as char
        } else {
            '.'
        });
    }
    s.push('|');
    s
}

/// Formats a byte count as `123 B`, `12.3 KiB`, `4.0 MiB`.
#[must_use]
pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if n < 1024 {
        return format!("{n} B");
    }
    let mut v = n as f64 / 1024.0;
    let mut unit = 0;
    while v >= 1024.0 && unit + 1 < UNITS.len() {
        v /= 1024.0;
        unit += 1;
    }
    format!("{v:.1} {}", UNITS[unit])
}

/// Formats an integer with thousands separators: `1,234,567`.
#[must_use]
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines() {
        let data: Vec<u8> = (0u8..20).chain(*b"AB ~").collect();
        assert_eq!(line_count(data.len()), 2);
        assert_eq!(line_count(0), 0);
        let l0 = hex_line(&data, 0);
        assert!(l0.starts_with("00000000  00 01 02 03 04 05 06 07  08 "));
        assert!(l0.ends_with("|................|"));
        let l1 = hex_line(&data, 1);
        assert!(l1.starts_with("00000010  10 11 12 13 41 42 20 7e"));
        assert!(l1.ends_with("|....AB ~|"));
        // Both lines have the ASCII column at the same position.
        assert_eq!(l0.find('|'), l1.find('|'));
        assert_eq!(hex_line(&data, 2), "");
        assert_eq!(hex_line(&data, usize::MAX), "");
    }

    #[test]
    fn sizes() {
        assert_eq!(human_bytes(12), "12 B");
        assert_eq!(human_bytes(2048), "2.0 KiB");
        assert_eq!(human_bytes(5 * 1024 * 1024 + 1), "5.0 MiB");
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
