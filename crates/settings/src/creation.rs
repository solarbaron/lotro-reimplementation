// SPDX-License-Identifier: GPL-3.0-or-later
//! The creation choices of characters made on Tessera's creation screen, kept where the
//! session cannot carry them (`character-screens.md` §7.1, Tessera rule; decision 0025 D2):
//! on a protocol-1.0 connection to a test server and across restarts of the plain sandbox.
//! The client applies them to its own figure of that character only.
//!
//! The section is an array of tables in [`Settings::extra`] under [`CREATED_SECTION`], so
//! files written by this version round-trip unchanged through older ones; unknown keys of an
//! entry are dropped when the section is rewritten (the section is Tessera's own).
//!
//! ```toml
//! [[CreatedCharacters]]
//! server = "tessera://127.0.0.1:7777"   # the session's key (`sandbox` offline)
//! name = "Cora"                          # the character's name, byte for byte
//! species = 81                           # Body::Character species key
//! sex = 4096                             # Body::Character sex value
//! class = 23                             # optional: the class key
//! nationality = 5                        # optional: the nationality key
//! look = [                               # optional: (property ID, kind, value), ascending IDs
//!   { property = 268435473, kind = "int", value = 3 },
//!   { property = 268435480, kind = "colour", value = [200, 150, 120, 255] },
//! ]
//! ```
//!
//! Every value is bounded: at most [`MAX_CREATED_CHARACTERS`] entries (the oldest are
//! dropped first), [`MAX_STORED_LOOK_ENTRIES`] look entries per character, names and server
//! keys of at most [`MAX_KEY_BYTES`] bytes, look strings of at most
//! [`MAX_LOOK_STRING_BYTES`] bytes of U+0021–U+007E, finite floats, and the whole section
//! at most [`MAX_CREATED_SECTION_BYTES`] bytes as written (the oldest entries are dropped
//! until it fits), so the section keeps the crate's promise that accepted settings
//! serialize well below [`crate::MAX_FILE_SIZE`]. A malformed entry is skipped with a
//! warning; a malformed look entry is dropped with a warning (at most
//! [`crate::MAX_WARNINGS_PER_SECTION`] warnings are made).

use crate::Settings;

/// Key of the section in [`Settings::extra`].
pub const CREATED_SECTION: &str = "CreatedCharacters";
/// Most characters remembered (the oldest are dropped first).
pub const MAX_CREATED_CHARACTERS: usize = 256;
/// Most look entries per character (the protocol's look limit, decision 0025 D2).
pub const MAX_STORED_LOOK_ENTRIES: usize = 48;
/// Longest server key or character name, in bytes.
pub const MAX_KEY_BYTES: usize = 256;
/// Longest look string, in bytes (U+0021–U+007E only).
pub const MAX_LOOK_STRING_BYTES: usize = 128;
/// Most bytes the section takes as written (its entries' TOML text): the oldest entries
/// are dropped until it fits. One entry takes at most about 17 KiB, so at least a dozen
/// worst-case entries (and a few hundred typical ones) fit.
pub const MAX_CREATED_SECTION_BYTES: usize = 256 * 1024;
/// Bytes counted per entry beyond its own text (the `[[CreatedCharacters]]` header).
const ENTRY_OVERHEAD_BYTES: usize = 32;

/// One stored value of a look entry: the kinds a look may carry (decision 0025 D2).
#[derive(Debug, Clone, PartialEq)]
pub enum StoredValue {
    /// `bool`.
    Bool(bool),
    /// `int`: a 32-bit integer.
    Int(i32),
    /// `int64`.
    Int64(i64),
    /// `float`: finite.
    Float(f32),
    /// `enum`: an enum key.
    Enum(u32),
    /// `bitfield32`.
    Bitfield32(u32),
    /// `bitfield64` (stored as a hexadecimal string, since TOML integers are signed).
    Bitfield64(u64),
    /// `did`: a data ID.
    Did(u32),
    /// `string`: at most [`MAX_LOOK_STRING_BYTES`] bytes of U+0021–U+007E.
    String(String),
    /// `colour`: red, green, blue, alpha.
    Colour([u8; 4]),
}

impl StoredValue {
    /// The kind keyword written to the file.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Int64(_) => "int64",
            Self::Float(_) => "float",
            Self::Enum(_) => "enum",
            Self::Bitfield32(_) => "bitfield32",
            Self::Bitfield64(_) => "bitfield64",
            Self::Did(_) => "did",
            Self::String(_) => "string",
            Self::Colour(_) => "colour",
        }
    }

    /// True when a float is finite and a string fits the look rule.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Float(v) => v.is_finite(),
            Self::String(s) => {
                s.len() <= MAX_LOOK_STRING_BYTES && s.bytes().all(|b| (0x21..=0x7E).contains(&b))
            }
            _ => true,
        }
    }

    fn to_toml(&self) -> toml::Value {
        use toml::Value as V;
        match self {
            Self::Bool(b) => V::Boolean(*b),
            Self::Int(v) => V::Integer(i64::from(*v)),
            Self::Int64(v) => V::Integer(*v),
            Self::Float(v) => V::Float(f64::from(*v)),
            Self::Enum(v) | Self::Bitfield32(v) | Self::Did(v) => V::Integer(i64::from(*v)),
            Self::Bitfield64(v) => V::String(format!("{v:#x}")),
            Self::String(s) => V::String(s.clone()),
            Self::Colour(c) => V::Array(c.iter().map(|b| V::Integer(i64::from(*b))).collect()),
        }
    }

    fn from_toml(kind: &str, v: &toml::Value) -> Option<Self> {
        let int = || v.as_integer();
        let u32_ = || int().and_then(|i| u32::try_from(i).ok());
        let value = match kind {
            "bool" => Self::Bool(v.as_bool()?),
            "int" => Self::Int(i32::try_from(int()?).ok()?),
            "int64" => Self::Int64(int()?),
            "float" => {
                let f = v.as_float().or_else(|| int().map(|i| i as f64))?;
                // Range-checked before narrowing; a finite f64 beyond f32 is refused.
                if !f.is_finite() || f.abs() > f64::from(f32::MAX) {
                    return None;
                }
                Self::Float(f as f32)
            }
            "enum" => Self::Enum(u32_()?),
            "bitfield32" => Self::Bitfield32(u32_()?),
            "did" => Self::Did(u32_()?),
            "bitfield64" => {
                let s = v.as_str()?;
                let digits = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
                if digits.is_empty() || digits.len() > 16 {
                    return None;
                }
                Self::Bitfield64(u64::from_str_radix(digits, 16).ok()?)
            }
            "string" => Self::String(v.as_str()?.to_owned()),
            "colour" => {
                let a = v.as_array()?;
                if a.len() != 4 {
                    return None;
                }
                let mut c = [0u8; 4];
                for (o, x) in c.iter_mut().zip(a) {
                    *o = u8::try_from(x.as_integer()?).ok()?;
                }
                Self::Colour(c)
            }
            _ => return None,
        };
        value.is_valid().then_some(value)
    }
}

/// One stored look entry: a data property ID of the user's install and its value.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredLookEntry {
    /// The property ID (non-zero).
    pub property: u32,
    /// The value.
    pub value: StoredValue,
}

/// The creation choices of one character, keyed by server and name.
#[derive(Debug, Clone, PartialEq)]
pub struct CreationChoices {
    /// The session's key: a test server's connect string, or `sandbox`.
    pub server: String,
    /// The character's name, byte for byte as created.
    pub name: String,
    /// The body's species key.
    pub species: u32,
    /// The body's sex value.
    pub sex: u32,
    /// The class key, when chosen.
    pub class: Option<u32>,
    /// The nationality key, when chosen.
    pub nationality: Option<u32>,
    /// The look entries, strictly ascending by property ID, at most
    /// [`MAX_STORED_LOOK_ENTRIES`].
    pub look: Vec<StoredLookEntry>,
}

impl CreationChoices {
    /// Whether the key fits: server and name non-empty, at most [`MAX_KEY_BYTES`] bytes, no
    /// control characters.
    #[must_use]
    pub fn key_is_valid(&self) -> bool {
        let ok =
            |s: &str| !s.is_empty() && s.len() <= MAX_KEY_BYTES && !s.chars().any(char::is_control);
        ok(&self.server) && ok(&self.name)
    }

    /// The look made canonical: invalid values and property 0 dropped, sorted by property
    /// ID, of two entries with the same ID the later one kept, cut to
    /// [`MAX_STORED_LOOK_ENTRIES`].
    fn normalise_look(&mut self) {
        let mut v: Vec<(usize, StoredLookEntry)> = std::mem::take(&mut self.look)
            .into_iter()
            .filter(|e| e.property != 0 && e.value.is_valid())
            .enumerate()
            .collect();
        v.sort_by_key(|(i, e)| (e.property, std::cmp::Reverse(*i)));
        v.dedup_by_key(|(_, e)| e.property);
        self.look = v
            .into_iter()
            .map(|(_, e)| e)
            .take(MAX_STORED_LOOK_ENTRIES)
            .collect();
    }

    fn to_toml(&self) -> toml::Value {
        use toml::Value as V;
        let mut t = toml::Table::new();
        t.insert("server".into(), V::String(self.server.clone()));
        t.insert("name".into(), V::String(self.name.clone()));
        t.insert("species".into(), V::Integer(i64::from(self.species)));
        t.insert("sex".into(), V::Integer(i64::from(self.sex)));
        if let Some(c) = self.class {
            t.insert("class".into(), V::Integer(i64::from(c)));
        }
        if let Some(n) = self.nationality {
            t.insert("nationality".into(), V::Integer(i64::from(n)));
        }
        if !self.look.is_empty() {
            let look = self
                .look
                .iter()
                .map(|e| {
                    let mut l = toml::Table::new();
                    l.insert("property".into(), V::Integer(i64::from(e.property)));
                    l.insert("kind".into(), V::String(e.value.kind().into()));
                    l.insert("value".into(), e.value.to_toml());
                    V::Table(l)
                })
                .collect();
            t.insert("look".into(), V::Array(look));
        }
        V::Table(t)
    }

    /// The entry's size as the settings file writes it (pretty TOML, under its own
    /// `[[CreatedCharacters]]` header), plus a margin.
    fn written_size(v: &toml::Value) -> usize {
        let mut doc = toml::Table::new();
        doc.insert(CREATED_SECTION.into(), toml::Value::Array(vec![v.clone()]));
        toml::to_string_pretty(&doc)
            .map_or(usize::MAX, |t| t.len().saturating_add(ENTRY_OVERHEAD_BYTES))
    }

    /// Reads one entry; warnings are added only while `warn` holds fewer than
    /// [`crate::MAX_WARNINGS_PER_SECTION`] (none is formatted beyond that).
    fn from_toml(v: &toml::Value, warn: &mut Vec<String>) -> Option<Self> {
        let room = |w: &Vec<String>| w.len() < crate::MAX_WARNINGS_PER_SECTION;
        let t = v.as_table()?;
        let text = |k: &str| t.get(k).and_then(toml::Value::as_str).map(str::to_owned);
        let key = |k: &str| {
            t.get(k)
                .and_then(toml::Value::as_integer)
                .and_then(|i| u32::try_from(i).ok())
        };
        let mut c = Self {
            server: text("server")?,
            name: text("name")?,
            species: key("species")?,
            sex: key("sex")?,
            class: key("class"),
            nationality: key("nationality"),
            look: Vec::new(),
        };
        if !c.key_is_valid() {
            return None;
        }
        if let Some(look) = t.get("look") {
            let Some(a) = look.as_array() else {
                if room(warn) {
                    warn.push(format!(
                        "[[{CREATED_SECTION}]] look is not an array; dropped"
                    ));
                }
                return Some(c);
            };
            for e in a.iter().take(MAX_STORED_LOOK_ENTRIES.saturating_mul(4)) {
                let entry = e.as_table().and_then(|l| {
                    let property = l
                        .get("property")?
                        .as_integer()
                        .and_then(|i| u32::try_from(i).ok())?;
                    let kind = l.get("kind")?.as_str()?;
                    let value = StoredValue::from_toml(kind, l.get("value")?)?;
                    Some(StoredLookEntry { property, value })
                });
                match entry {
                    Some(e) => c.look.push(e),
                    None if room(warn) => warn.push(format!(
                        "[[{CREATED_SECTION}]] a look entry of {:?} is malformed; dropped",
                        c.name
                    )),
                    None => {}
                }
            }
            c.normalise_look();
        }
        Some(c)
    }
}

/// The `[[CreatedCharacters]]` section: creation choices by server and character name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CreatedCharacters {
    /// The entries, oldest first; keys (server, name) unique.
    pub entries: Vec<CreationChoices>,
}

impl CreatedCharacters {
    /// Reads the section. Never fails: a malformed entry is skipped with a warning; at most
    /// [`MAX_CREATED_CHARACTERS`] entries are kept (the newest).
    #[must_use]
    pub fn from_settings(settings: &Settings) -> (Self, Vec<String>) {
        let mut out = Self::default();
        let mut warn = Vec::new();
        let Some(section) = settings.extra.get(CREATED_SECTION) else {
            return (out, warn);
        };
        let Some(a) = section.as_array() else {
            warn.push(format!("[[{CREATED_SECTION}]] is not an array; ignored"));
            return (out, warn);
        };
        let skip = a.len().saturating_sub(MAX_CREATED_CHARACTERS);
        if skip > 0 {
            warn.push(format!(
                "[[{CREATED_SECTION}]] has more than {MAX_CREATED_CHARACTERS} entries; the oldest {skip} ignored"
            ));
        }
        for v in &a[skip..] {
            match CreationChoices::from_toml(v, &mut warn) {
                Some(c) => {
                    out.remember_unbounded(c);
                }
                None => {
                    if warn.len() < crate::MAX_WARNINGS_PER_SECTION {
                        warn.push(format!(
                            "[[{CREATED_SECTION}]] a malformed entry was skipped"
                        ));
                    }
                }
            }
        }
        let dropped = out.fit_budget();
        if dropped > 0 && warn.len() < crate::MAX_WARNINGS_PER_SECTION {
            warn.push(format!(
                "[[{CREATED_SECTION}]] is over {MAX_CREATED_SECTION_BYTES} bytes; the oldest {dropped} entries ignored"
            ));
        }
        warn.truncate(crate::MAX_WARNINGS_PER_SECTION);
        (out, warn)
    }

    /// The entries as written, newest last, without the oldest that do not fit
    /// [`MAX_CREATED_SECTION_BYTES`] (and at most [`MAX_CREATED_CHARACTERS`]).
    fn written(&self) -> Vec<toml::Value> {
        let mut total = 0usize;
        let mut out: Vec<toml::Value> = Vec::new();
        for c in self.entries.iter().rev().take(MAX_CREATED_CHARACTERS) {
            let v = c.to_toml();
            total = total.saturating_add(CreationChoices::written_size(&v));
            if total > MAX_CREATED_SECTION_BYTES {
                break;
            }
            out.push(v);
        }
        out.reverse();
        out
    }

    /// Drops the oldest entries that do not fit the section's bounds; returns how many.
    fn fit_budget(&mut self) -> usize {
        let keep = self.written().len();
        let excess = self.entries.len().saturating_sub(keep);
        self.entries.drain(..excess);
        excess
    }

    /// Writes the section into `settings`, replacing it (an empty list removes it). Only
    /// the newest entries that fit [`MAX_CREATED_SECTION_BYTES`] are written (entries put
    /// into [`CreatedCharacters::entries`] directly are bounded here too).
    pub fn store(&self, settings: &mut Settings) {
        let written = self.written();
        if written.is_empty() {
            settings.extra.remove(CREATED_SECTION);
            return;
        }
        settings
            .extra
            .insert(CREATED_SECTION.to_owned(), toml::Value::Array(written));
    }

    /// The choices of `name` on `server` (names compared byte for byte).
    #[must_use]
    pub fn get(&self, server: &str, name: &str) -> Option<&CreationChoices> {
        self.entries
            .iter()
            .find(|c| c.server == server && c.name == name)
    }

    /// Remembers `choices`, replacing an entry with the same key and making it the newest;
    /// drops the oldest beyond [`MAX_CREATED_CHARACTERS`] or beyond
    /// [`MAX_CREATED_SECTION_BYTES`]. Returns false (and keeps nothing) when the key is not
    /// valid ([`CreationChoices::key_is_valid`]). The look is made canonical (sorted,
    /// de-duplicated, invalid values dropped, cut to [`MAX_STORED_LOOK_ENTRIES`]).
    pub fn remember(&mut self, choices: CreationChoices) -> bool {
        if !self.remember_unbounded(choices) {
            return false;
        }
        self.fit_budget();
        true
    }

    /// [`CreatedCharacters::remember`] without the byte budget (applied once by the
    /// caller).
    fn remember_unbounded(&mut self, mut choices: CreationChoices) -> bool {
        if !choices.key_is_valid() {
            return false;
        }
        choices.normalise_look();
        self.entries
            .retain(|c| !(c.server == choices.server && c.name == choices.name));
        self.entries.push(choices);
        let excess = self.entries.len().saturating_sub(MAX_CREATED_CHARACTERS);
        self.entries.drain(..excess);
        true
    }

    /// Forgets `name` on `server`; true when an entry was removed.
    pub fn forget(&mut self, server: &str, name: &str) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|c| !(c.server == server && c.name == name));
        self.entries.len() != before
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn sample(name: &str) -> CreationChoices {
        CreationChoices {
            server: "sandbox".into(),
            name: name.into(),
            species: 0x51,
            sex: 0x1000,
            class: Some(23),
            nationality: None,
            look: vec![
                StoredLookEntry {
                    property: 9,
                    value: StoredValue::Colour([1, 2, 3, 255]),
                },
                StoredLookEntry {
                    property: 3,
                    value: StoredValue::Int(4),
                },
                StoredLookEntry {
                    property: 5,
                    value: StoredValue::Bitfield64(u64::MAX),
                },
                StoredLookEntry {
                    property: 7,
                    value: StoredValue::Float(0.25),
                },
                StoredLookEntry {
                    property: 8,
                    value: StoredValue::String("80FF".into()),
                },
            ],
        }
    }

    #[test]
    fn round_trip_through_toml_text() {
        let mut created = CreatedCharacters::default();
        assert!(created.remember(sample("Cora")));
        let mut s = Settings::default();
        created.store(&mut s);
        let text = toml::to_string(&s.extra).unwrap();
        let back = Settings {
            extra: toml::from_str(&text).unwrap(),
            ..Settings::default()
        };
        let (read, warn) = CreatedCharacters::from_settings(&back);
        assert!(warn.is_empty(), "{warn:?}");
        let c = read.get("sandbox", "Cora").unwrap();
        assert_eq!(c.class, Some(23));
        let ids: Vec<u32> = c.look.iter().map(|e| e.property).collect();
        assert_eq!(ids, vec![3, 5, 7, 8, 9], "sorted ascending");
        assert_eq!(c.look[1].value, StoredValue::Bitfield64(u64::MAX));
        assert_eq!(read, {
            let mut r = CreatedCharacters::default();
            r.remember(sample("Cora"));
            r
        });
    }

    #[test]
    fn replace_bound_and_forget() {
        let mut c = CreatedCharacters::default();
        for i in 0..(MAX_CREATED_CHARACTERS + 10) {
            assert!(c.remember(sample(&format!("N{i}"))));
        }
        assert_eq!(c.entries.len(), MAX_CREATED_CHARACTERS);
        assert!(c.get("sandbox", "N0").is_none(), "oldest dropped");
        let mut again = sample("N20");
        again.species = 0x17;
        c.remember(again);
        assert_eq!(c.entries.last().unwrap().species, 0x17);
        assert_eq!(c.entries.len(), MAX_CREATED_CHARACTERS);
        assert!(c.forget("sandbox", "N20"));
        assert!(!c.forget("sandbox", "N20"));
        let mut bad = sample("x");
        bad.name = String::new();
        assert!(!c.remember(bad));
        let mut ctl = sample("y");
        ctl.server = "a\nb".into();
        assert!(!c.remember(ctl));
    }

    #[test]
    fn look_is_canonical_and_bounded() {
        let mut c = sample("Dup");
        c.look = (0..100)
            .map(|i| StoredLookEntry {
                property: 100 - (i % 60),
                value: StoredValue::Int(i32::try_from(i).unwrap()),
            })
            .chain([
                StoredLookEntry {
                    property: 0,
                    value: StoredValue::Int(1),
                },
                StoredLookEntry {
                    property: 1,
                    value: StoredValue::Float(f32::NAN),
                },
                StoredLookEntry {
                    property: 2,
                    value: StoredValue::String("a b".into()),
                },
            ])
            .collect();
        let mut created = CreatedCharacters::default();
        created.remember(c);
        let look = &created.entries[0].look;
        assert_eq!(look.len(), MAX_STORED_LOOK_ENTRIES);
        assert!(look.windows(2).all(|w| w[0].property < w[1].property));
        // Properties 41..=100; the lowest 48 are kept. Property 88 appears at i = 12 and
        // i = 72: the later one is kept.
        assert_eq!(look[0].property, 41);
        assert_eq!(look.last().map(|e| e.property), Some(88));
        assert_eq!(
            look.iter().find(|e| e.property == 88).map(|e| &e.value),
            Some(&StoredValue::Int(72))
        );
    }

    #[test]
    fn hostile_sections_never_fail() {
        let mut s = Settings::default();
        s.extra
            .insert(CREATED_SECTION.into(), toml::Value::Integer(3));
        let (c, w) = CreatedCharacters::from_settings(&s);
        assert!(c.entries.is_empty() && w.len() == 1);
        let text = r#"
CreatedCharacters = [
  { server = "s", name = "A", species = 1, sex = 2, look = [
      { property = 1, kind = "float", value = 1e300 },
      { property = 2, kind = "colour", value = [1, 2, 3] },
      { property = 3, kind = "bitfield64", value = "0x1ffffffffffffffff" },
      { property = 4, kind = "int", value = 4294967296 },
      { property = 5, kind = "nope", value = 1 },
      { property = 6, kind = "did", value = 7 },
  ] },
  { server = "s", name = "B", species = -1, sex = 2 },
  { server = "s", sex = 2 },
  "text",
]
"#;
        s.extra = toml::from_str(text).unwrap();
        let (c, w) = CreatedCharacters::from_settings(&s);
        assert_eq!(c.entries.len(), 1);
        assert_eq!(
            c.entries[0].look,
            vec![StoredLookEntry {
                property: 6,
                value: StoredValue::Did(7)
            }]
        );
        assert!(!w.is_empty() && w.len() <= crate::MAX_WARNINGS_PER_SECTION);
        let empty = CreatedCharacters::default();
        empty.store(&mut s);
        assert!(!s.extra.contains_key(CREATED_SECTION));
    }

    /// The longest valid entry: keys of quotes (escaped when written), 48 look strings of
    /// quotes and backslashes.
    pub(crate) fn worst_entry(i: usize) -> CreationChoices {
        let nasty = |n: usize| -> String { "\"\\".chars().cycle().take(n).collect() };
        let mut name = format!("{i}");
        name.push_str(&nasty(MAX_KEY_BYTES - name.len()));
        CreationChoices {
            server: nasty(MAX_KEY_BYTES),
            name,
            species: u32::MAX,
            sex: u32::MAX,
            class: Some(u32::MAX),
            nationality: Some(u32::MAX),
            look: (0..MAX_STORED_LOOK_ENTRIES)
                .map(|p| StoredLookEntry {
                    property: u32::MAX - p as u32,
                    value: StoredValue::String(nasty(MAX_LOOK_STRING_BYTES)),
                })
                .collect(),
        }
    }

    /// Review finding: a full section of worst-case entries stays within its byte budget
    /// (the newest kept), so the settings file stays below its size limit.
    #[test]
    fn a_worst_case_section_fits_its_budget() {
        let mut c = CreatedCharacters::default();
        for i in 0..MAX_CREATED_CHARACTERS {
            assert!(c.remember(worst_entry(i)));
        }
        assert!(!c.entries.is_empty());
        assert!(
            c.entries.len() < MAX_CREATED_CHARACTERS,
            "the budget dropped the oldest"
        );
        let newest = format!("{}", MAX_CREATED_CHARACTERS - 1);
        assert!(c.entries.last().unwrap().name.starts_with(&newest));
        let mut s = Settings::default();
        c.store(&mut s);
        let text = s.to_toml_string().unwrap();
        assert!(
            text.len() <= MAX_CREATED_SECTION_BYTES + 1024,
            "{} bytes",
            text.len()
        );
        // Entries put in directly are bounded when written.
        let direct = CreatedCharacters {
            entries: (0..MAX_CREATED_CHARACTERS).map(worst_entry).collect(),
        };
        let mut s2 = Settings::default();
        direct.store(&mut s2);
        let text2 = s2.to_toml_string().unwrap();
        assert!(text2.len() <= MAX_CREATED_SECTION_BYTES + 1024);
        // Reading it back keeps every written entry.
        let back = Settings::from_toml_str(&text).unwrap();
        let (read, _) = CreatedCharacters::from_settings(&back);
        assert_eq!(read.entries.len(), c.entries.len());
    }

    /// Review finding: malformed look entries make at most the section's warning count,
    /// none formatted beyond it.
    #[test]
    fn warnings_are_bounded_while_reading() {
        let name: String = "\"".repeat(MAX_KEY_BYTES);
        let bad = toml::Value::Array(vec![toml::Value::Integer(1); MAX_STORED_LOOK_ENTRIES * 4]);
        let entry = |i: usize| {
            let mut t = toml::Table::new();
            t.insert("server".into(), toml::Value::String("s".into()));
            t.insert(
                "name".into(),
                toml::Value::String(format!("{i}{}", &name[..MAX_KEY_BYTES - 4])),
            );
            t.insert("species".into(), toml::Value::Integer(1));
            t.insert("sex".into(), toml::Value::Integer(2));
            t.insert("look".into(), bad.clone());
            toml::Value::Table(t)
        };
        let mut s = Settings::default();
        s.extra.insert(
            CREATED_SECTION.into(),
            toml::Value::Array((0..MAX_CREATED_CHARACTERS).map(entry).collect()),
        );
        let mut warn = Vec::new();
        for v in s.extra[CREATED_SECTION].as_array().unwrap() {
            CreationChoices::from_toml(v, &mut warn);
            assert!(warn.len() <= crate::MAX_WARNINGS_PER_SECTION);
        }
        let (c, w) = CreatedCharacters::from_settings(&s);
        assert_eq!(c.entries.len(), MAX_CREATED_CHARACTERS);
        assert!(w.len() <= crate::MAX_WARNINGS_PER_SECTION);
    }
}
