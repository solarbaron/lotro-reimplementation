// SPDX-License-Identifier: GPL-3.0-or-later
//! Small helpers: hex argument parsing, number/time formatting, text tables, hexdumps and a
//! scoped parallel map.

use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub use tessera_dattool::hexarg::{parse_did, parse_prefix};

/// `1536` → `1.5 KiB`.
pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if n < 1024 {
        return format!("{n} B");
    }
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u + 1 < UNITS.len() {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.1} {}", UNITS[u])
}

/// Days since 1970-01-01 → (year, month, day) in the proleptic Gregorian calendar.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Unix seconds (UTC) → `YYYY-MM-DD HH:MM:SS`; `-` for 0.
pub fn format_time(secs: u32) -> String {
    if secs == 0 {
        return "-".into();
    }
    let s = i64::from(secs);
    let (y, mo, d) = civil_from_days(s.div_euclid(86_400));
    let r = s.rem_euclid(86_400);
    format!(
        "{y:04}-{mo:02}-{d:02} {:02}:{:02}:{:02}",
        r / 3600,
        (r / 60) % 60,
        r % 60
    )
}

/// Column alignment for [`Table`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Pad on the right.
    Left,
    /// Pad on the left.
    Right,
}

/// A plain-text table with aligned columns.
#[derive(Debug)]
pub struct Table {
    headers: Vec<&'static str>,
    align: Vec<Align>,
    rows: Vec<Vec<String>>,
}

impl Table {
    /// A table with the given `(header, alignment)` columns.
    pub fn new(cols: &[(&'static str, Align)]) -> Self {
        Self {
            headers: cols.iter().map(|c| c.0).collect(),
            align: cols.iter().map(|c| c.1).collect(),
            rows: Vec::new(),
        }
    }

    /// Appends a row; missing cells are blank, extra cells are dropped.
    pub fn row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    /// Writes the table with two spaces between columns.
    pub fn write(&self, w: &mut dyn Write) -> io::Result<()> {
        let n = self.headers.len();
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.chars().count()).collect();
        for r in &self.rows {
            for (i, c) in r.iter().take(n).enumerate() {
                widths[i] = widths[i].max(c.chars().count());
            }
        }
        let line = |w: &mut dyn Write, cells: &mut dyn Iterator<Item = &str>| -> io::Result<()> {
            let mut out = String::new();
            for (i, c) in cells.take(n).enumerate() {
                let pad = widths[i].saturating_sub(c.chars().count());
                let last = i + 1 == n;
                if i > 0 {
                    out.push_str("  ");
                }
                match self.align[i] {
                    Align::Right => {
                        out.extend(std::iter::repeat_n(' ', pad));
                        out.push_str(c);
                    }
                    Align::Left => {
                        out.push_str(c);
                        if !last {
                            out.extend(std::iter::repeat_n(' ', pad));
                        }
                    }
                }
            }
            writeln!(w, "{}", out.trim_end())
        };
        line(w, &mut self.headers.iter().copied())?;
        let rule: Vec<String> = widths.iter().map(|&x| "-".repeat(x)).collect();
        line(w, &mut rule.iter().map(String::as_str))?;
        for r in &self.rows {
            let mut cells = r.iter().map(String::as_str).chain(std::iter::repeat(""));
            line(w, &mut cells)?;
        }
        Ok(())
    }
}

/// Classic 16-bytes-per-line hexdump with an ASCII column; offsets start at `base`.
pub fn hexdump(w: &mut dyn Write, data: &[u8], base: usize) -> io::Result<()> {
    for (i, chunk) in data.chunks(16).enumerate() {
        let mut hex = String::with_capacity(50);
        for (j, b) in chunk.iter().enumerate() {
            if j == 8 {
                hex.push(' ');
            }
            hex.push_str(&format!("{b:02x} "));
        }
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        writeln!(w, "{:08x}  {hex:<49} |{ascii}|", base + i * 16)?;
    }
    Ok(())
}

/// Maps `f` over `items` on up to `available_parallelism` threads, keeping input order.
pub fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(1, |p| p.get())
        .min(items.len())
        .max(1);
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else { break };
                    let r = f(item);
                    if let Ok(mut g) = out.lock()
                        && let Some(slot) = g.get_mut(i)
                    {
                        *slot = Some(r);
                    }
                }
            });
        }
    });
    out.into_inner()
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect()
}

/// `0x41000001` style formatting.
pub fn did_hex(did: u32) -> String {
    format!("0x{did:08X}")
}

/// A path as a JSON string. `serde_json::json!` unwraps the serialisation of its values and
/// a `Path` that is not UTF-8 (legal on Unix) fails to serialise, so paths always go
/// through this lossy conversion instead.
pub fn path_json(p: &Path) -> serde_json::Value {
    serde_json::Value::String(p.to_string_lossy().into_owned())
}

/// [`path_json`] of an optional path (`null` when absent).
pub fn opt_path_json(p: Option<&Path>) -> serde_json::Value {
    p.map_or(serde_json::Value::Null, path_json)
}

/// Lower-case hex string of a byte slice.
pub fn hex_bytes(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_become_lossy_json_strings() {
        use std::os::unix::ffi::OsStrExt;
        let p = Path::new(std::ffi::OsStr::from_bytes(b"/tmp/x\xff.bin"));
        // What the old code did: json!(path) panics on this path.
        assert!(serde_json::to_value(p).is_err());
        assert_eq!(path_json(p), "/tmp/x\u{fffd}.bin");
        assert_eq!(opt_path_json(Some(p)), path_json(p));
        assert!(opt_path_json(None).is_null());
    }

    #[test]
    fn dids_parse_with_and_without_prefix() {
        assert_eq!(parse_did("0x41000001"), Ok(0x4100_0001));
        assert_eq!(parse_did("41000001"), Ok(0x4100_0001));
        assert_eq!(parse_did("0X4100_0001"), Ok(0x4100_0001));
        assert_eq!(parse_did("ffff0005"), Ok(0xFFFF_0005));
        assert!(parse_did("").is_err());
        assert!(parse_did("0x").is_err());
        assert!(parse_did("123456789").is_err());
        assert!(parse_did("zz").is_err());
        assert_eq!(parse_prefix("0x41"), Ok(0x41));
        assert_eq!(parse_prefix("7"), Ok(7));
        assert!(parse_prefix("100").is_err());
    }

    #[test]
    fn times_format_in_utc() {
        assert_eq!(format_time(0), "-");
        assert_eq!(format_time(1), "1970-01-01 00:00:01");
        assert_eq!(format_time(951_782_400), "2000-02-29 00:00:00");
        assert_eq!(format_time(1_700_000_000), "2023-11-14 22:13:20");
        assert_eq!(format_time(u32::MAX), "2106-02-07 06:28:15");
    }

    #[test]
    fn sizes_are_human() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1536), "1.5 KiB");
        assert_eq!(human_bytes(3 << 30), "3.0 GiB");
    }

    #[test]
    fn hexdump_shape() {
        let mut out = Vec::new();
        hexdump(&mut out, b"ABCDEFGHIJKLMNOPQ", 0x10).unwrap();
        let s = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("00000010  41 42"));
        assert!(lines[0].ends_with("|ABCDEFGHIJKLMNOP|"));
        assert!(lines[1].starts_with("00000020  51 "));
    }

    #[test]
    fn par_map_keeps_order() {
        let v: Vec<u32> = (0..100).collect();
        assert_eq!(
            par_map(&v, |x| x * 2),
            (0..100).map(|x| x * 2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn table_aligns() {
        let mut t = Table::new(&[("A", Align::Left), ("NUM", Align::Right)]);
        t.row(vec!["long-name".into(), "1".into()]);
        t.row(vec!["x".into(), "1234".into()]);
        let mut out = Vec::new();
        t.write(&mut out).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert_eq!(
            s,
            "A           NUM\n---------  ----\nlong-name     1\nx          1234\n"
        );
    }
}
