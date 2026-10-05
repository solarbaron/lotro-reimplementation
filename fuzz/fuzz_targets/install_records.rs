// SPDX-License-Identifier: GPL-3.0-or-later
//! The binary inputs `tessera-install` reads (`docs/specs/install-layout.md` §3.1, §3.4,
//! §9.1-§9.4): archive file names, the archive header prefix, the 0xFFFF0001 / 0003 / 0005
//! housekeeping records, split-set verification over hostile membership records, and both
//! fingerprints computed from what was decoded.
//!
//! Nothing may panic. Accepted archive names must round-trip through their canonical file
//! name, accepted iteration records must agree with their own count, and split-set results
//! must stay bounded by `MAX_SPLIT_SET_COUNT`.

#![no_main]

use std::path::PathBuf;

use libfuzzer_sys::fuzz_target;
use tessera_install::{
    ArchiveFile, ArchiveHeader, ArchiveKind, ArchiveStatus, ContentEntry, ContentFingerprint,
    HEADER_READ_LEN, InstallStamp, IterationRecord, MAX_SPLIT_SET_COUNT, MAX_TRAILING_ENTRIES,
    SchemaFingerprint, SetMembership, SplitSetStatus, parse_archive_name, stamp_hex,
    verify_split_set,
};

fn names(data: &[u8]) {
    let text = String::from_utf8_lossy(data);
    // Try the raw text and the text dressed as an archive name, so the grammar's inner
    // branches are reached without the fuzzer having to discover the prefix.
    for candidate in [
        text.to_string(),
        format!("client_{text}.dat"),
        format!("client_{text}.datx"),
    ] {
        let Some(name) = parse_archive_name(&candidate) else {
            continue;
        };
        let canonical = name.file_name();
        assert_eq!(
            parse_archive_name(&canonical).as_ref(),
            Some(&name),
            "canonical name {canonical:?} round-trips"
        );
        let k = &name.kind;
        let _ = (
            k.key(),
            k.tier(),
            k.may_have_aux(),
            k.observed_kind_word(),
            k.to_string(),
        );
    }
}

fn header(data: &[u8]) -> Option<ArchiveHeader> {
    let h = ArchiveHeader::parse(data).ok()?;
    let _ = (h.magic_ok(), h.family());
    Some(h)
}

fn records(data: &[u8]) -> ContentEntry {
    let iteration = IterationRecord::parse(data).ok();
    if let Some(it) = &iteration {
        let covered: u64 = it.ranges.iter().map(|r| u64::from(r.len)).sum();
        assert_eq!(covered, u64::from(it.count), "ranges cover exactly N");
        assert!(it.trailing.len() <= MAX_TRAILING_ENTRIES as usize);
        assert_eq!(it.runs(), it.ranges.len());
        if it.count == 0 {
            assert!(it.highest().is_none());
        }
    }
    let install_stamp = InstallStamp::parse(data).ok();
    if let Some(st) = &install_stamp {
        assert!(st.label.len() < 0x80);
        assert_eq!(stamp_hex(&st.s1).len(), 32);
    }
    ContentEntry {
        file_name: String::from_utf8_lossy(data.get(..16).unwrap_or(data)).into_owned(),
        iteration,
        membership: SetMembership::parse(data).ok(),
        install_stamp,
    }
}

fn split_set(data: &[u8]) {
    // Up to 18 members (base + MAX_AUX_INDEX + 1), each 24 bytes of record or "missing".
    let mut present = Vec::new();
    let mut recs = Vec::new();
    for (i, chunk) in data.chunks(25).take(18).enumerate() {
        let index = u32::try_from(i).unwrap_or(u32::MAX);
        if chunk[0] & 0x80 != 0 {
            continue; // not on disk
        }
        present.push(index);
        let rec = if chunk[0] & 0x40 != 0 {
            None
        } else {
            SetMembership::parse(&chunk[1..]).ok()
        };
        recs.push((index, rec));
    }
    let set = verify_split_set(ArchiveKind::General, &present, &recs);
    assert!(
        set.members.windows(2).all(|w| w[0] < w[1]),
        "members ascending"
    );
    assert!(set.members.len() + set.stale.len() <= present.len().max(1));
    if let SplitSetStatus::Incomplete { missing } = &set.status {
        assert!(
            missing.len() <= MAX_SPLIT_SET_COUNT as usize,
            "missing list bounded"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&sel, rest)) = data.split_first() else {
        return;
    };
    match sel % 4 {
        0 => names(rest),
        1 => {
            let h = header(rest);
            let a = ArchiveFile {
                kind: ArchiveKind::General,
                aux_index: 0,
                path: PathBuf::from("client_general.dat"),
                size: rest.len() as u64,
                header: h,
                status: ArchiveStatus::Ok,
            };
            let fp = SchemaFingerprint::compute(std::slice::from_ref(&a));
            assert_eq!(fp.sha256.len(), 64);
            let _ = fp.short();
            let _ = HEADER_READ_LEN;
        }
        2 => {
            // Several archives' records, each behind a u16 LE length (cut to what is
            // left). A length prefix rather than a separator byte, so records may hold any
            // byte: iteration runs are negative i32s whose top byte is 0xFF, and stamps
            // are arbitrary.
            let mut entries: Vec<ContentEntry> = Vec::new();
            let mut rest = rest;
            while entries.len() < 8 {
                let Some((len, tail)) = rest.split_first_chunk::<2>() else {
                    break;
                };
                let len = usize::from(u16::from_le_bytes(*len)).min(tail.len());
                let (rec, tail) = tail.split_at(len);
                entries.push(records(rec));
                rest = tail;
            }
            let fp = ContentFingerprint::compute(&entries);
            assert_eq!(fp.sha256.len(), 64);
            assert!(fp.text.ends_with('\n'));
            let _ = fp.short();
        }
        _ => split_set(rest),
    }
});
