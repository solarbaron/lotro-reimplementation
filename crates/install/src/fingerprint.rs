// SPDX-License-Identifier: GPL-3.0-or-later
//! Version fingerprints.
//!
//! Spec-Ref: docs/specs/install-layout.md#94-fingerprint-algorithm
//!
//! * Level 0 ([`SchemaFingerprint`]) uses header fields only and is computed
//!   by this crate directly.
//! * Level 1 ([`ContentFingerprint`]) needs three housekeeping records per
//!   archive. This crate does not read archive directories, so the caller (the
//!   dat container layer) supplies the decoded records as [`ContentEntry`]
//!   values.
//!
//! Serialisations are this project's own convention and must stay stable,
//! because caches key on them.

use sha2::{Digest, Sha256};

use crate::archive::ArchiveFile;
use crate::records::{InstallStamp, IterationRecord, SetMembership, Stamp, hex, stamp_hex};

/// Hex digits in a short fingerprint.
pub const SHORT_LEN: usize = 16;

fn sha256_hex(text: &str) -> String {
    hex(&Sha256::digest(text.as_bytes()))
}

/// Level-0 per-archive tuple (§9.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaEntry {
    /// Lower-case file name.
    pub file_name: String,
    /// Header 0x14C.
    pub kind_word: u32,
    /// Header 0x150.
    pub sub_code: u32,
    /// Header 0x174.
    pub field_174: u32,
    /// Header 0x178.
    pub field_178: u32,
    /// Header 0x144.
    pub block_size: u32,
    /// Header 0x17C.
    pub identifier_17c: [u8; 16],
    /// Header 0x18C.
    pub field_18c: u32,
}

/// Level-0 "schema identity" fingerprint: header fields only. The same across
/// content updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaFingerprint {
    /// Per-archive tuples, sorted by file name.
    pub entries: Vec<SchemaEntry>,
    /// The serialised text that was hashed.
    pub text: String,
    /// Full SHA-256 of `text`, lower-case hex.
    pub sha256: String,
}

impl SchemaFingerprint {
    /// Compute from the archives of an install. Archives whose header could
    /// not be read are left out.
    pub fn compute(archives: &[ArchiveFile]) -> Self {
        let mut entries: Vec<SchemaEntry> = archives
            .iter()
            .filter_map(|a| {
                let h = a.header.as_ref()?;
                Some(SchemaEntry {
                    file_name: a.file_name().to_ascii_lowercase(),
                    kind_word: h.kind_word,
                    sub_code: h.sub_code,
                    field_174: h.field_174,
                    field_178: h.field_178,
                    block_size: h.block_size,
                    identifier_17c: h.identifier_17c,
                    field_18c: h.field_18c,
                })
            })
            .collect();
        entries.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        let mut text = String::new();
        for e in &entries {
            text.push_str(&format!(
                "{} {} {} {} {} {} {} {}\n",
                e.file_name,
                e.kind_word,
                e.sub_code,
                e.field_174,
                e.field_178,
                e.block_size,
                hex(&e.identifier_17c),
                e.field_18c
            ));
        }
        let sha256 = sha256_hex(&text);
        Self {
            entries,
            text,
            sha256,
        }
    }

    /// The first [`SHORT_LEN`] hex digits of the hash (the whole field when
    /// a hand-built value is shorter).
    pub fn short(&self) -> &str {
        self.sha256.get(..SHORT_LEN).unwrap_or(&self.sha256)
    }
}

/// Level-1 inputs for one archive, decoded by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentEntry {
    /// File name (lower-cased when serialised).
    pub file_name: String,
    /// Decoded 0xFFFF0001, if readable.
    pub iteration: Option<IterationRecord>,
    /// Decoded 0xFFFF0005, if readable.
    pub membership: Option<SetMembership>,
    /// Decoded 0xFFFF0003, if present.
    pub install_stamp: Option<InstallStamp>,
}

/// The install-wide S1/S2 value of level 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallWideStamp {
    /// Every archive carrying 0xFFFF0003 has this pair.
    Uniform(Stamp, Stamp),
    /// The pairs differ between archives.
    Mixed,
    /// No archive carries the record.
    Absent,
}

/// Level-1 "content version" fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFingerprint {
    /// The install-wide S1/S2 pair.
    pub install_stamp: InstallWideStamp,
    /// The serialised text that was hashed.
    pub text: String,
    /// Full SHA-256 of `text`, lower-case hex.
    pub sha256: String,
}

impl ContentFingerprint {
    /// Compute from per-archive records.
    ///
    /// Serialisation: one line per archive sorted by lower-case file name,
    /// fields separated by one space: name, `N`, `HI`, set stamp, set index,
    /// set count, then S1, S2 and the label when 0xFFFF0003 is present.
    /// Integers are decimal, stamps 32 lower-case hex digits; an unavailable
    /// value is written as `-`. A final line `install <S1> <S2>`,
    /// `install mixed` or `install none` carries the install-wide value.
    pub fn compute(entries: &[ContentEntry]) -> Self {
        let mut sorted: Vec<&ContentEntry> = entries.iter().collect();
        sorted.sort_by_key(|e| e.file_name.to_ascii_lowercase());
        let dash = || "-".to_owned();
        let mut text = String::new();
        let mut wide: Option<(Stamp, Stamp)> = None;
        let mut mixed = false;
        for e in sorted {
            let (n, hi) = match &e.iteration {
                Some(it) => (
                    it.count.to_string(),
                    it.highest().map_or_else(dash, |h| h.to_string()),
                ),
                None => (dash(), dash()),
            };
            let (ss, si, sc) = match &e.membership {
                Some(m) => (
                    stamp_hex(&m.stamp),
                    m.index.to_string(),
                    m.count.to_string(),
                ),
                None => (dash(), dash(), dash()),
            };
            let mut line = format!(
                "{} {n} {hi} {ss} {si} {sc}",
                e.file_name.to_ascii_lowercase()
            );
            if let Some(st) = &e.install_stamp {
                line.push_str(&format!(
                    " {} {} {}",
                    stamp_hex(&st.s1),
                    stamp_hex(&st.s2),
                    if st.label.is_empty() { "-" } else { &st.label }
                ));
                match wide {
                    None => wide = Some((st.s1, st.s2)),
                    Some(w) if w != (st.s1, st.s2) => mixed = true,
                    Some(_) => {}
                }
            }
            text.push_str(&line);
            text.push('\n');
        }
        let install_stamp = match (wide, mixed) {
            (_, true) => InstallWideStamp::Mixed,
            (Some((a, b)), false) => InstallWideStamp::Uniform(a, b),
            (None, false) => InstallWideStamp::Absent,
        };
        match &install_stamp {
            InstallWideStamp::Uniform(a, b) => {
                text.push_str(&format!("install {} {}\n", stamp_hex(a), stamp_hex(b)));
            }
            InstallWideStamp::Mixed => text.push_str("install mixed\n"),
            InstallWideStamp::Absent => text.push_str("install none\n"),
        }
        let sha256 = sha256_hex(&text);
        Self {
            install_stamp,
            text,
            sha256,
        }
    }

    /// The first [`SHORT_LEN`] hex digits of the hash (the whole field when
    /// a hand-built value is shorter).
    pub fn short(&self) -> &str {
        self.sha256.get(..SHORT_LEN).unwrap_or(&self.sha256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tests::synthetic_header;
    use crate::archive::{ArchiveHeader, ArchiveStatus, parse_archive_name};
    use crate::records::IterationRange;

    fn archive(name: &str, kind_word: u32) -> ArchiveFile {
        let n = parse_archive_name(name).unwrap();
        ArchiveFile {
            kind: n.kind,
            aux_index: n.aux_index,
            path: name.into(),
            size: 4096,
            header: Some(ArchiveHeader::parse(&synthetic_header(4096, 256, kind_word, 0)).unwrap()),
            status: ArchiveStatus::Ok,
        }
    }

    #[test]
    fn short_never_panics_on_hand_built_values() {
        let c = ContentFingerprint {
            install_stamp: InstallWideStamp::Absent,
            text: String::new(),
            sha256: String::new(),
        };
        assert_eq!(c.short(), "");
        let s = SchemaFingerprint {
            entries: Vec::new(),
            text: String::new(),
            sha256: "abc".into(),
        };
        assert_eq!(s.short(), "abc");
        let s = SchemaFingerprint {
            entries: Vec::new(),
            text: String::new(),
            sha256: "é".repeat(20),
        };
        let _ = s.short();
    }

    #[test]
    fn schema_is_order_independent_and_stable() {
        let a = archive("client_general.dat", 0x0103_0000);
        let b = archive("client_gamelogic.dat", 0x0101_0000);
        let f1 = SchemaFingerprint::compute(&[a.clone(), b.clone()]);
        let f2 = SchemaFingerprint::compute(&[b, a]);
        assert_eq!(f1, f2);
        assert_eq!(f1.short().len(), SHORT_LEN);
        assert!(
            f1.text
                .starts_with("client_gamelogic.dat 16842752 0 112 3 256 abab")
        );
    }

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn content_fingerprint_text() {
        let it = IterationRecord {
            count: 3,
            ranges: vec![IterationRange { start: 1, len: 3 }],
            trailing: vec![],
        };
        let st = InstallStamp {
            s1: [1; 16],
            s2: [2; 16],
            label: "5758".into(),
        };
        let m = SetMembership {
            stamp: [3; 16],
            index: 0,
            count: 1,
        };
        let e1 = ContentEntry {
            file_name: "client_General.dat".into(),
            iteration: Some(it),
            membership: Some(m),
            install_stamp: Some(st.clone()),
        };
        let e2 = ContentEntry {
            file_name: "client_mesh_aux_1.datx".into(),
            iteration: None,
            membership: None,
            install_stamp: None,
        };
        let f = ContentFingerprint::compute(&[e2.clone(), e1.clone()]);
        let expected = format!(
            "client_general.dat 3 3 {} 0 1 {} {} 5758\nclient_mesh_aux_1.datx - - - - -\ninstall {} {}\n",
            "03".repeat(16),
            "01".repeat(16),
            "02".repeat(16),
            "01".repeat(16),
            "02".repeat(16)
        );
        assert_eq!(f.text, expected);
        assert_eq!(f.install_stamp, InstallWideStamp::Uniform([1; 16], [2; 16]));

        let mut e3 = e1.clone();
        e3.file_name = "client_anim.dat".into();
        e3.install_stamp = Some(InstallStamp { s1: [9; 16], ..st });
        let f = ContentFingerprint::compute(&[e1, e3]);
        assert_eq!(f.install_stamp, InstallWideStamp::Mixed);
        assert!(f.text.ends_with("install mixed\n"));
    }
}
