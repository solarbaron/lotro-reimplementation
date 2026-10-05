// SPDX-License-Identifier: GPL-3.0-or-later
//! Structural depth bounds for settings documents.
//!
//! The TOML parser limits bracket nesting and the parts of one dotted key
//! separately, but the two limits multiply: inside each of many nested
//! inline tables, a long dotted key adds as many levels again. A few
//! kilobytes of text can then describe a tree thousands of levels deep, and
//! building, cloning, serializing or dropping that tree recurses deeply
//! enough to overflow a small thread stack (an abort, not a panic).
//!
//! [`parse_bounded`] guards parsing in two steps that measure the same thing
//! as the save path, so that whatever loads cleanly also re-loads after it
//! is saved:
//!
//! 1. [`check_text`], a lexical pre-scan run before the document is handed
//!    to the parser. It skips strings and comments and adds up, along the
//!    current path, every table header part, every dotted-key part and every
//!    open `{`/`[`. A table header counts two levels per part (in case each
//!    part is an array of tables), so the sum never under-estimates the real
//!    depth and over-estimates it at most twofold. The scan therefore runs
//!    with the looser bound [`PRESCAN_LIMIT`] (twice the real limit): it only
//!    keeps the tree small enough to build and drop safely, and never rejects
//!    a document whose real depth is within [`MAX_NESTING_DEPTH`], whichever
//!    way it is written (the serializer turns dotted keys and inline tables
//!    into table headers, which the scan counts double).
//! 2. [`table_exceeds`] on the parsed tree, without recursion, enforcing the
//!    real depth bound. The save path checks unknown values with the same
//!    function and the same measure.

/// Deepest structure a settings document may have: the root table is level
/// 1 and each nested table or array adds one level (scalars add none),
/// however the document spells it (table headers, dotted keys or inline
/// values). Tessera's own files nest four levels at most; deeper documents
/// are rejected as a whole (a syntax warning for the lenient loader, a parse
/// error for the strict one), and deeper unknown values are not serialized.
pub const MAX_NESTING_DEPTH: usize = 64;

/// Bound for the lexical pre-scan [`check_text`]. It over-counts a table
/// header twofold, so it runs at twice [`MAX_NESTING_DEPTH`]; every
/// document of real depth up to [`MAX_NESTING_DEPTH`] passes it. Trees this
/// deep are still safe to build and drop on a 1 MiB stack.
pub(crate) const PRESCAN_LIMIT: usize = 2 * MAX_NESTING_DEPTH;

/// Parse `text` into a table, rejecting documents deeper than
/// [`MAX_NESTING_DEPTH`] (see the module docs). The error is the message to
/// report; parser messages may span several lines.
pub(crate) fn parse_bounded(text: &str) -> Result<toml::Table, String> {
    check_text(text, PRESCAN_LIMIT)?;
    let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    if table_exceeds(&table, MAX_NESTING_DEPTH) {
        return Err(format!(
            "the document nests deeper than {MAX_NESTING_DEPTH} levels"
        ));
    }
    Ok(table)
}

#[derive(Debug, Clone, Copy)]
struct Frame {
    /// `{`: keys inside; `[`: values only.
    table: bool,
    /// Dots seen in the key of the current element.
    dots: usize,
    /// Still reading the key of the current element.
    in_key: bool,
}

/// Fail with a message when `text` may nest deeper than `limit`.
pub(crate) fn check_text(text: &str, limit: usize) -> Result<(), String> {
    let deep = || format!("the document nests deeper than {limit} levels");
    let b = text.as_bytes();
    let mut frames: Vec<Frame> = Vec::new();
    // The top-level key/value element of the current line.
    let mut top = Frame {
        table: true,
        dots: 0,
        in_key: true,
    };
    let mut line_start = true;
    let mut header_depth = 0usize;
    let mut i = 0usize;
    while let Some(&c) = b.get(i) {
        i += 1;
        match c {
            b'#' => {
                while b.get(i).is_some_and(|&c| c != b'\n') {
                    i += 1;
                }
            }
            b'"' | b'\'' => {
                i = skip_string(b, i, c);
                line_start = false;
            }
            b'\n' => {
                if frames.is_empty() {
                    top.dots = 0;
                    top.in_key = true;
                    line_start = true;
                }
            }
            b' ' | b'\t' | b'\r' => {}
            b'[' if frames.is_empty() && line_start => {
                // Table header: `[a.b]` or `[[a.b]]`.
                if b.get(i) == Some(&b'[') {
                    i += 1;
                }
                let mut parts = 1usize;
                while let Some(&c) = b.get(i) {
                    i += 1;
                    match c {
                        b'.' => parts = parts.saturating_add(1),
                        b'"' | b'\'' => i = skip_string(b, i, c),
                        b']' => {
                            if b.get(i) == Some(&b']') {
                                i += 1;
                            }
                            break;
                        }
                        b'\n' => {
                            i -= 1;
                            break;
                        }
                        _ => {}
                    }
                    if parts.saturating_mul(2) > limit {
                        return Err(deep());
                    }
                }
                header_depth = parts.saturating_mul(2);
                if header_depth > limit {
                    return Err(deep());
                }
                line_start = false;
                top.in_key = false;
            }
            _ => {
                line_start = false;
                match c {
                    b'{' | b'[' => frames.push(Frame {
                        table: c == b'{',
                        dots: 0,
                        in_key: c == b'{',
                    }),
                    b'}' | b']' => {
                        frames.pop();
                    }
                    b',' => {
                        if let Some(f) = frames.last_mut() {
                            f.dots = 0;
                            f.in_key = f.table;
                        }
                    }
                    b'.' | b'=' => {
                        let cur = frames.last_mut().unwrap_or(&mut top);
                        if c == b'=' {
                            cur.in_key = false;
                        } else if cur.in_key {
                            cur.dots = cur.dots.saturating_add(1);
                        }
                    }
                    _ => {}
                }
                let depth = frames.iter().fold(1 + header_depth + top.dots, |acc, f| {
                    acc.saturating_add(1).saturating_add(f.dots)
                });
                if depth > limit {
                    return Err(deep());
                }
            }
        }
    }
    Ok(())
}

/// Skip a string whose opening quote `q` was just consumed (`i` is the index
/// after it); returns the index after the closing quote. A single-line
/// string ends at the end of the line even when unterminated, as the TOML
/// lexer recovers there; a multi-line one runs to its closing triple quote
/// (plus up to two more quotes that belong to the content) or the end.
fn skip_string(b: &[u8], mut i: usize, q: u8) -> usize {
    let escapes = q == b'"';
    if b.get(i) == Some(&q) && b.get(i + 1) == Some(&q) {
        i += 2;
        while let Some(&c) = b.get(i) {
            i += 1;
            if escapes && c == b'\\' {
                i += 1;
            } else if c == q && b.get(i) == Some(&q) && b.get(i + 1) == Some(&q) {
                i += 2;
                for _ in 0..2 {
                    if b.get(i) == Some(&q) {
                        i += 1;
                    }
                }
                return i;
            }
        }
        return i.min(b.len());
    }
    while let Some(&c) = b.get(i) {
        match c {
            b'\n' => return i,
            b'\\' if escapes && b.get(i + 1).is_some_and(|&n| n != b'\n') => i += 2,
            _ if c == q => return i + 1,
            _ => i += 1,
        }
    }
    i.min(b.len())
}

/// Whether `table` nests deeper than `limit` levels of tables and arrays
/// (the table itself is level 1; scalars add no level, matching
/// [`check_text`]). Iterative, so it is safe on arbitrarily deep values.
pub(crate) fn table_exceeds(table: &toml::Table, limit: usize) -> bool {
    if limit == 0 {
        return true;
    }
    let mut stack: Vec<(&toml::Value, usize)> = table.values().map(|v| (v, 2)).collect();
    while let Some((v, depth)) = stack.pop() {
        match v {
            toml::Value::Table(t) if depth <= limit => {
                stack.extend(t.values().map(|v| (v, depth + 1)));
            }
            toml::Value::Array(a) if depth <= limit => {
                stack.extend(a.iter().map(|v| (v, depth + 1)));
            }
            toml::Value::Table(_) | toml::Value::Array(_) => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_layout_is_shallow() {
        let doc = "schema_version = 1\n[game]\nknown_dirs = [\"a\"]\n\
                   [[explorer.bookmarks]]\ndid = \"0x1\"\n[windows.explorer]\nx = 1.5\n";
        check_text(doc, 8).unwrap();
    }

    #[test]
    fn counts_multiply() {
        check_text("a.b.c = { d.e = [ { f = 1 } ] }", 12).unwrap();
        assert!(check_text("a.b.c = { d.e = [ { f = 1 } ] }", 6).is_err());
        // Header depth persists over the following lines.
        assert!(check_text("[a.b.c]\nd.e.f = 1", 8).is_err());
        check_text("[a.b.c]\n[d]\ne.f.g = 1", 8).unwrap();
        // Values (floats, times) do not add key parts.
        check_text("a = 1.5\nb = 07:32:00.999\nc = [1.5, 2.5]", 4).unwrap();
    }

    #[test]
    fn strings_and_comments_are_skipped() {
        let open = "{".repeat(100);
        for doc in [
            format!("a = \"{open}\"\n"),
            format!("a = '{open}'\n"),
            format!("a = \"\"\"\n{open}\\\"\n\"\"\"\"\n"),
            format!("a = '''\n{open}\n'''''\n"),
            format!("# {open}\na = 1\n"),
            format!("\"{open}\" = 1\n"),
        ] {
            check_text(&doc, 4).unwrap();
        }
        // An unterminated single-line string ends at the line end.
        assert!(check_text(&format!("a = \"x\\\n{open}"), 8).is_err());
        assert!(check_text(&format!("a = 'x\n{open}"), 8).is_err());
    }

    #[test]
    fn value_depth() {
        let mut t = toml::Table::new();
        t.insert(
            "a".into(),
            toml::Value::Array(vec![toml::Value::Integer(1)]),
        );
        assert!(!table_exceeds(&t, 2));
        assert!(table_exceeds(&t, 1));
        assert!(table_exceeds(&t, 0));
        assert!(!table_exceeds(&toml::Table::new(), 1));
    }
}
