// SPDX-License-Identifier: GPL-3.0-or-later
//! Building blocks of the gameplay cards: sections of text rows with DID links, property
//! rows by dictionary label, and bounded debug dumps of decoded values.
//!
//! Everything here formats data that is already decoded; nothing reads the archive. Every
//! output is bounded: [`MAX_SECTION_ROWS`] rows per section, [`MAX_CELL`] bytes per cell,
//! [`MAX_ROW_LINKS`] links per row and [`MAX_DUMP_BYTES`] of debug text per dump.

use std::collections::HashSet;
use std::fmt::{self, Debug, Display, Write as _};
use std::sync::Arc;

use tessera_dat::ArchiveSet;
use tessera_formats::gameplay::common::Pid;
use tessera_formats::{Did, PropertySet, PropertyValue, StringInfo};
use tessera_props::{GameData, Label, PropertyRegistry, Value};
use tessera_text::{RenderOptions, Strings, Values};

use crate::decoders::{label_text, plausible_did, row_links, string_info_text, value_text};

/// Most rows one section keeps.
pub const MAX_SECTION_ROWS: usize = 5_000;
/// Longest cell text kept (bytes).
pub const MAX_CELL: usize = 400;
/// Most DID links one row keeps.
pub const MAX_ROW_LINKS: usize = 16;
/// Most debug text one dump formats (bytes); the rest is cut.
pub const MAX_DUMP_BYTES: usize = 256 * 1024;
/// Most lines one dump keeps.
pub const MAX_DUMP_LINES: usize = 1_500;

/// One row of a section: its cells and the DIDs it links to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Cells, one per column.
    pub cells: Vec<String>,
    /// Records the row links to (non-null DIDs with a catalogued prefix).
    pub links: Vec<u32>,
}

/// A titled table of a card.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Section {
    /// Heading.
    pub title: String,
    /// Column names.
    pub columns: Vec<&'static str>,
    /// Rows in display order.
    pub rows: Vec<Row>,
    /// Whether rows were cut at [`MAX_SECTION_ROWS`] (or a dump at its limits).
    pub truncated: bool,
    /// Whether the cells are preformatted text (debug dumps).
    pub preformatted: bool,
}

/// The columns of a field/value section.
pub const FIELD_COLUMNS: &[&str] = &["field", "value"];
/// The columns of a property section.
pub const PROPERTY_COLUMNS: &[&str] = &["property", "value", "label"];

impl Section {
    /// An empty section.
    #[must_use]
    pub fn new(title: impl Into<String>, columns: &[&'static str]) -> Self {
        Self {
            title: title.into(),
            columns: columns.to_vec(),
            ..Self::default()
        }
    }

    /// An empty field/value section.
    #[must_use]
    pub fn fields(title: impl Into<String>) -> Self {
        Self::new(title, FIELD_COLUMNS)
    }

    /// Adds a row; cells are clipped to [`MAX_CELL`], links filtered to plausible DIDs.
    pub fn push(&mut self, cells: Vec<String>, links: Vec<u32>) {
        if self.rows.len() >= MAX_SECTION_ROWS {
            self.truncated = true;
            return;
        }
        let cells = cells.into_iter().map(clip).collect();
        let mut kept: Vec<u32> = Vec::new();
        for d in links {
            if plausible_did(d).is_some() && !kept.contains(&d) && kept.len() < MAX_ROW_LINKS {
                kept.push(d);
            }
        }
        self.rows.push(Row { cells, links: kept });
    }

    /// Adds a field/value row.
    pub fn field(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.push(vec![name.into(), value.into()], Vec::new());
    }

    /// Adds a field/value row when `value` is present.
    pub fn opt<T: Display>(&mut self, name: &str, value: Option<T>) {
        if let Some(v) = value {
            self.field(name, v.to_string());
        }
    }

    /// Adds a field row whose value is a DID, linked; skipped when absent or null.
    pub fn did(&mut self, name: impl Into<String>, did: Option<Did>) {
        if let Some(d) = did.filter(|d| !d.is_null()) {
            self.push(vec![name.into(), did_text(d)], vec![d.0]);
        }
    }

    /// One linked row per DID, named `name [i]`.
    pub fn dids(&mut self, name: &str, dids: &[Did]) {
        for (i, d) in dids.iter().enumerate() {
            self.did(format!("{name} [{i}]"), Some(*d));
        }
    }
}

/// A card: the sections of one record, in display order.
#[derive(Debug, Clone, Default)]
pub struct Card {
    /// The sections; empty ones are not kept.
    pub sections: Vec<Section>,
}

impl Card {
    /// Adds `s` unless it has no rows.
    pub fn add(&mut self, s: Section) {
        if !s.rows.is_empty() {
            self.sections.push(s);
        }
    }
}

/// A DID as `0x…` (lower-case, 8 digits), the form the view turns into a link.
#[must_use]
pub fn did_text(d: Did) -> String {
    format!("{:#010x}", d.0)
}

/// An optional DID as text: `—` for absent or null.
#[must_use]
pub fn opt_did_text(d: Option<Did>) -> String {
    d.filter(|d| !d.is_null())
        .map_or_else(|| "—".to_owned(), did_text)
}

/// An optional value as text, `—` when absent.
#[must_use]
pub fn opt_text<T: Display>(v: Option<T>) -> String {
    v.map_or_else(|| "—".to_owned(), |v| v.to_string())
}

/// `yes` / `no`.
#[must_use]
pub fn yes(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

/// An optional flag as text.
#[must_use]
pub fn opt_yes(b: Option<bool>) -> String {
    b.map_or_else(|| "—".to_owned(), |b| yes(b).to_owned())
}

/// Clips `s` to [`MAX_CELL`] bytes at a character boundary.
#[must_use]
pub fn clip(mut s: String) -> String {
    if s.len() > MAX_CELL {
        let mut end = MAX_CELL;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push('…');
    }
    s
}

/// The DIDs written as `Did(0x…)` in debug text, in order of appearance.
#[must_use]
pub fn dids_in(text: &str) -> Vec<u32> {
    const MARK: &str = "Did(0x";
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(MARK) {
        rest = rest.get(at + MARK.len()..).unwrap_or("");
        let hex: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
        if hex.len() == 8
            && rest.get(8..9) == Some(")")
            && let Ok(d) = u32::from_str_radix(&hex, 16)
        {
            out.push(d);
        }
        if out.len() >= MAX_ROW_LINKS {
            break;
        }
    }
    out
}

/// A string that stops accepting text after `limit` bytes.
struct Bounded {
    text: String,
    limit: usize,
    cut: bool,
}

impl fmt::Write for Bounded {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let room = self.limit.saturating_sub(self.text.len());
        if s.len() <= room {
            self.text.push_str(s);
            Ok(())
        } else {
            let mut end = room;
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            self.text.push_str(s.get(..end).unwrap_or(""));
            self.cut = true;
            Err(fmt::Error)
        }
    }
}

/// A section holding the pretty-printed debug form of `value`, one line per row, with the
/// DIDs of each line as links. Bounded by [`MAX_DUMP_BYTES`] and [`MAX_DUMP_LINES`].
#[must_use]
pub fn dump<T: Debug + ?Sized>(title: impl Into<String>, value: &T) -> Section {
    let mut b = Bounded {
        text: String::new(),
        limit: MAX_DUMP_BYTES,
        cut: false,
    };
    let _ = write!(b, "{value:#?}");
    let mut s = Section::new(title, &["decoded"]);
    s.preformatted = true;
    for (i, line) in b.text.lines().enumerate() {
        if i >= MAX_DUMP_LINES {
            s.truncated = true;
            break;
        }
        s.push(vec![line.to_owned()], dids_in(line));
    }
    s.truncated |= b.cut;
    s
}

/// Label sources of the install: property names, enum and DID-map labels, string tables.
/// Every source is optional; without one the raw value is shown.
#[derive(Clone, Copy, Default)]
pub struct Labels<'a> {
    /// The property registry (names, enum sources).
    pub registry: Option<&'a PropertyRegistry>,
    /// Enum mappers and DID maps.
    pub game: Option<&'a GameData<Arc<ArchiveSet>>>,
    /// Strings of the selected language.
    pub strings: Option<&'a Strings>,
}

impl Debug for Labels<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Labels")
            .field("registry", &self.registry.is_some())
            .field("game", &self.game.is_some())
            .field("strings", &self.strings.is_some())
            .finish()
    }
}

impl Labels<'_> {
    /// The registry name of `pid`, or the PID in hex.
    #[must_use]
    pub fn pid_name(&self, pid: u32) -> String {
        self.registry
            .and_then(|r| r.name(pid))
            .map_or_else(|| format!("{pid:#x}"), str::to_owned)
    }

    /// A text value as the selected language renders it, else its table and token.
    #[must_use]
    pub fn text(&self, si: &StringInfo) -> String {
        match self.strings {
            Some(s) => clip(
                s.resolve(si, &Values::new(), &RenderOptions::explorer(None))
                    .plain_text(),
            ),
            None => string_info_text(si),
        }
    }

    /// [`Labels::text`] for an optional value (`—` when absent).
    #[must_use]
    pub fn opt_text(&self, si: Option<&StringInfo>) -> String {
        si.map_or_else(|| "—".to_owned(), |s| self.text(s))
    }

    /// Value text, label text and links of `raw`, a value of property `pid`.
    #[must_use]
    pub fn value(&self, pid: u32, raw: &PropertyValue) -> (String, String, Vec<u32>) {
        let mut links = Vec::new();
        nested_dids(raw, &mut links);
        match Value::from_raw(raw.clone()) {
            Ok(v) => {
                let label = self.game.map_or(Label::None, |g| g.label(pid, &v));
                let mut own: Vec<u32> = row_links(&label, &v).into_iter().map(|(_, d)| d).collect();
                own.extend(links);
                (value_text(&v), label_text(&label, &v, self.strings), own)
            }
            Err(_) => (clip(format!("{raw:?}")), String::new(), links),
        }
    }

    /// An enum key of property `pid` with its label: `key (Name)`.
    #[must_use]
    pub fn enum_key(&self, pid: Pid, key: u32) -> String {
        self.labelled(pid, &PropertyValue::Enum(key), key.to_string())
    }

    /// An optional enum key (`—` when absent).
    #[must_use]
    pub fn opt_enum(&self, pid: Pid, key: Option<u32>) -> String {
        key.map_or_else(|| "—".to_owned(), |k| self.enum_key(pid, k))
    }

    /// A bit field of property `pid` with its bit labels.
    #[must_use]
    pub fn bits(&self, pid: Pid, bits: u64) -> String {
        self.labelled(pid, &PropertyValue::Bitfield64(bits), format!("{bits:#x}"))
    }

    /// An optional bit field (`—` when absent).
    #[must_use]
    pub fn opt_bits(&self, pid: Pid, bits: Option<u64>) -> String {
        bits.map_or_else(|| "—".to_owned(), |b| self.bits(pid, b))
    }

    fn labelled(&self, pid: Pid, raw: &PropertyValue, plain: String) -> String {
        let Some(id) = pid.id() else { return plain };
        let (_, label, _) = self.value(id, raw);
        if label.is_empty() {
            plain
        } else {
            format!("{plain} ({label})")
        }
    }

    /// A property ID with its name (`—` for none).
    #[must_use]
    pub fn property(&self, pid: Option<u32>) -> String {
        match pid {
            None | Some(0) => "—".to_owned(),
            Some(p) => self.pid_name(p),
        }
    }
}

/// Collects the non-null DIDs inside `value` (depth first, at most [`MAX_ROW_LINKS`]).
pub fn nested_dids(value: &PropertyValue, out: &mut Vec<u32>) {
    let mut stack: Vec<&PropertyValue> = vec![value];
    while let Some(v) = stack.pop() {
        if out.len() >= MAX_ROW_LINKS {
            return;
        }
        match v {
            PropertyValue::DidRef(d) if !d.is_null() => out.push(d.0),
            PropertyValue::Struct(_) | PropertyValue::Array(_) => {
                stack.extend(v.children().iter().rev().map(|e| &e.value));
            }
            _ => {}
        }
    }
}

/// A section of the top-level properties of `set` named by `pids`, in that order, by
/// dictionary label. Properties already in `shown` are skipped; the ones shown are added.
#[must_use]
pub fn property_section(
    title: impl Into<String>,
    labels: &Labels<'_>,
    set: &PropertySet,
    pids: &[Pid],
    shown: &mut HashSet<u32>,
) -> Section {
    let mut s = Section::new(title, PROPERTY_COLUMNS);
    for p in pids {
        let Some(id) = p.id() else { continue };
        let Some(v) = set.get(id) else { continue };
        if !shown.insert(id) {
            continue;
        }
        let (text, label, links) = labels.value(id, v);
        s.push(vec![p.label().to_owned(), text, label], links);
    }
    s
}

/// A section of every top-level property of `set` not in `shown`, by registry name.
#[must_use]
pub fn remaining_section(
    title: impl Into<String>,
    labels: &Labels<'_>,
    set: &PropertySet,
    shown: &HashSet<u32>,
) -> Section {
    let mut s = Section::new(title, PROPERTY_COLUMNS);
    for e in set.iter() {
        if shown.contains(&e.pid) {
            continue;
        }
        let (text, label, links) = labels.value(e.pid, &e.value);
        s.push(vec![labels.pid_name(e.pid), text, label], links);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::gameplay::common::build;

    #[test]
    fn did_links_come_from_debug_text() {
        assert_eq!(
            dids_in("a: Did(0x7000ABCD), b: Did(0x12)"),
            vec![0x7000_ABCD]
        );
        assert_eq!(dids_in("Did(0x7000ABCDE)"), Vec::<u32>::new());
        assert!(dids_in("no dids").is_empty());
    }

    #[test]
    fn sections_are_bounded() {
        let mut s = Section::fields("t");
        for i in 0..MAX_SECTION_ROWS + 10 {
            s.field(i.to_string(), "x".repeat(MAX_CELL * 2));
        }
        assert_eq!(s.rows.len(), MAX_SECTION_ROWS);
        assert!(s.truncated);
        assert!(s.rows[0].cells[1].len() <= MAX_CELL + '…'.len_utf8());
        // Null and uncatalogued DIDs are not links.
        s.rows.clear();
        s.truncated = false;
        s.push(vec!["a".into()], vec![0, 0x7000_0001, 0x7000_0001]);
        assert_eq!(s.rows[0].links, vec![0x7000_0001]);
    }

    #[test]
    fn dumps_are_bounded() {
        let big: Vec<u32> = (0..200_000).collect();
        let s = dump("big", &big);
        assert!(s.truncated);
        assert!(s.rows.len() <= MAX_DUMP_LINES);
        let small = dump("small", &Some(Did(0x7900_0001)));
        assert!(!small.truncated);
        assert!(small.rows.iter().any(|r| r.links == vec![0x7900_0001]));
    }

    #[test]
    fn property_rows_by_label() {
        let set = build::set(vec![
            build::entry(10, build::did(0x7000_0002)),
            build::entry(11, PropertyValue::Int(5)),
            build::entry(12, PropertyValue::Bool(true)),
        ]);
        let pids = [
            Pid::with_id("First", 10),
            Pid::with_id("Second", 11),
            Pid::with_id("Absent", 99),
            Pid::unresolved("Unresolved"),
        ];
        let labels = Labels::default();
        let mut shown = HashSet::new();
        let s = property_section("p", &labels, &set, &pids, &mut shown);
        assert_eq!(s.rows.len(), 2);
        assert_eq!(s.rows[0].cells[0], "First");
        assert_eq!(s.rows[0].links, vec![0x7000_0002]);
        assert_eq!(s.rows[1].cells[1], "5");
        let rest = remaining_section("r", &labels, &set, &shown);
        assert_eq!(rest.rows.len(), 1);
        assert_eq!(rest.rows[0].cells[0], "0xc");
        assert_eq!(labels.enum_key(Pid::with_id("E", 3), 4), "4");
    }
}
