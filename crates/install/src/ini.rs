// SPDX-License-Identifier: GPL-3.0-or-later
//! A tolerant, read-only INI reader for `UserPreferences.ini`.
//!
//! Spec-Ref: docs/specs/install-layout.md#83-userpreferencesini
//!
//! Sections and keys keep their original case and order (so a future writer
//! can preserve them), while lookups are ASCII case-insensitive. Repeated keys:
//! the last one wins, also across repeated section headers.

use std::path::Path;

use crate::error::Result;
use crate::fsutil;

/// Largest INI file accepted.
pub const MAX_INI_SIZE: u64 = 1 << 20;

/// One `key=value` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniEntry {
    /// Key, original case.
    pub key: String,
    /// Value, trimmed, untyped.
    pub value: String,
}

/// One `[section]` and its entries, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniSection {
    /// Section name, original case. Empty for lines before any header.
    pub name: String,
    /// Entries in file order.
    pub entries: Vec<IniEntry>,
}

/// A parsed INI document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ini {
    /// Sections in file order. A section name may appear more than once.
    pub sections: Vec<IniSection>,
}

impl Ini {
    /// Parse INI text. Never fails: lines that are not headers or
    /// `key=value` pairs are ignored.
    pub fn parse(text: &str) -> Self {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut sections: Vec<IniSection> = Vec::new();
        for raw in text.split('\n') {
            let line = raw.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                sections.push(IniSection {
                    name: inner.trim().to_owned(),
                    entries: Vec::new(),
                });
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let key = k.trim();
            if key.is_empty() {
                continue;
            }
            if sections.is_empty() {
                sections.push(IniSection {
                    name: String::new(),
                    entries: Vec::new(),
                });
            }
            if let Some(s) = sections.last_mut() {
                s.entries.push(IniEntry {
                    key: key.to_owned(),
                    value: v.trim().to_owned(),
                });
            }
        }
        Self { sections }
    }

    /// Parse bytes (UTF-8 with or without BOM; invalid sequences replaced).
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::parse(&String::from_utf8_lossy(bytes))
    }

    /// Read and parse a file, refusing files over [`MAX_INI_SIZE`].
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = fsutil::read_capped(path, MAX_INI_SIZE)?;
        Ok(Self::from_bytes(&bytes))
    }

    /// The value of `key` in `section` (both case-insensitive). The last
    /// occurrence wins.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .iter()
            .rev()
            .filter(|s| s.name.eq_ignore_ascii_case(section))
            .find_map(|s| {
                s.entries
                    .iter()
                    .rev()
                    .find(|e| e.key.eq_ignore_ascii_case(key))
            })
            .map(|e| e.value.as_str())
    }

    /// `True`/`False` (case-insensitive), also `1`/`0`.
    pub fn get_bool(&self, section: &str, key: &str) -> Option<bool> {
        let v = self.get(section, key)?;
        if v.eq_ignore_ascii_case("true") || v == "1" {
            Some(true)
        } else if v.eq_ignore_ascii_case("false") || v == "0" {
            Some(false)
        } else {
            None
        }
    }

    /// An integer value.
    pub fn get_i64(&self, section: &str, key: &str) -> Option<i64> {
        self.get(section, key)?.parse().ok()
    }

    /// A decimal value.
    pub fn get_f64(&self, section: &str, key: &str) -> Option<f64> {
        self.get(section, key)?
            .parse()
            .ok()
            .filter(|f: &f64| f.is_finite())
    }

    /// A `WxH` resolution.
    pub fn get_resolution(&self, section: &str, key: &str) -> Option<(u32, u32)> {
        let v = self.get(section, key)?;
        let (w, h) = v.split_once(['x', 'X'])?;
        Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
    }

    /// All section names, deduplicated case-insensitively, in first-seen order.
    pub fn section_names(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for s in &self.sections {
            if !out.iter().any(|n| n.eq_ignore_ascii_case(&s.name)) {
                out.push(&s.name);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_typical_file() {
        let text = "\u{feff}[Render]\r\nGraphicsCore=D3D11\r\nBrightness=0.30\r\n\
                    [Display]\r\nResolution=1024x768\r\nFullScreen=True\r\n\
                    garbage line\r\n[render]\r\ngraphicscore=OpenGL\r\n";
        let ini = Ini::parse(text);
        assert_eq!(ini.get("RENDER", "GraphicsCore"), Some("OpenGL"));
        assert_eq!(ini.get_f64("render", "brightness"), Some(0.30));
        assert_eq!(
            ini.get_resolution("Display", "Resolution"),
            Some((1024, 768))
        );
        assert_eq!(ini.get_bool("Display", "FullScreen"), Some(true));
        assert_eq!(ini.get("Display", "garbage line"), None);
        assert_eq!(ini.section_names(), vec!["Render", "Display"]);
        assert_eq!(ini.sections.len(), 3);
    }

    #[test]
    fn keys_before_section_and_duplicates() {
        let ini = Ini::parse("a=1\n[S]\nk=1\nK = 2 \n; c=3\n=x\n");
        assert_eq!(ini.get("", "a"), Some("1"));
        assert_eq!(ini.get("s", "k"), Some("2"));
        assert_eq!(ini.get("s", "c"), None);
        assert_eq!(ini.get_i64("s", "k"), Some(2));
    }

    #[test]
    fn size_cap() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("UserPreferences.ini");
        std::fs::write(&p, vec![b'a'; (MAX_INI_SIZE + 1) as usize]).unwrap();
        assert!(Ini::read(&p).is_err());
        std::fs::write(&p, b"[A]\nb=c\n").unwrap();
        assert_eq!(Ini::read(&p).unwrap().get("a", "B"), Some("c"));
    }

    proptest::proptest! {
        #[test]
        fn never_panics(b in proptest::collection::vec(proptest::num::u8::ANY, 0..512)) {
            let ini = Ini::from_bytes(&b);
            let _ = ini.get("a", "b");
            let _ = ini.get_resolution("a", "b");
        }
    }
}
