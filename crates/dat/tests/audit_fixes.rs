// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for audit findings against the container reader, the validator and
//! the archive set (docs/specs/dat-container.md, docs/specs/install-layout.md).
//!
//! Every fixture is synthetic, produced by the test-only writer in `common`.

mod common;

use common::{Builder, Built, Layout, TempDir, payload, zlib};
use flate2::Compression;
use tessera_dat::{
    Archive, ArchiveId, ArchiveKind, ArchiveOptions, ArchiveSet, Error, SetOptions, Severity,
    ValidateOptions,
};

fn open(b: &Built) -> Archive {
    Archive::from_bytes(b.bytes.clone(), ArchiveOptions::default()).unwrap()
}

fn write(dir: &TempDir, name: &str, b: &Builder) {
    std::fs::write(dir.0.join(name), b.build().bytes).unwrap();
}

/// Absolute position of fragment pair `i` of the fragmented record `did`, plus `F`.
fn fragment_pair(x: &Built, did: u32, i: usize) -> (usize, usize) {
    let (off, size) = x.records[&did];
    let f = x.get_u32(off as usize) as usize;
    let table = (off + size) as usize - 8 * f;
    (table + 8 * i, f)
}

// ---- [0] validator inflate amplification through shared fragments --------------------

#[test]
fn validator_inflates_bytes_shared_by_several_records_once() {
    // Ten compressed records, each with its own first extent and one fragment. All
    // fragments are then pointed at the first record's fragment.
    let raw = vec![0u8; 1 << 20];
    let z = zlib(&raw, Compression::best());
    let mut b = Builder::new();
    b.membership = None;
    let n = 10u32;
    for i in 0..n {
        b.add(
            0x0100_0000 + i,
            3,
            z.clone(),
            Layout::Fragmented {
                first: 16,
                chunks: vec![z.len()],
            },
        );
    }
    let mut x = b.build();
    let (pair0, f) = fragment_pair(&x, 0x0100_0000, 0);
    assert_eq!(f, 1);
    let target = x.get_u32(pair0 + 4);
    for i in 1..n {
        let (pair, _) = fragment_pair(&x, 0x0100_0000 + i, 0);
        x.put_u32(pair + 4, target);
    }
    let a = open(&x);
    // Every record still reads (the reader has only per-record caps, spec §9).
    for i in 0..n {
        assert_eq!(a.read(0x0100_0000 + i).unwrap().len(), raw.len());
    }
    let r = a.validate_with(&ValidateOptions {
        inflate_compressed: true,
        ..ValidateOptions::default()
    });
    assert_eq!(r.stats.compressed, u64::from(n));
    assert_eq!(
        r.stats.inflated, 1,
        "shared stored bytes must be inflated once: {:#?}",
        r.issues
    );
    assert!(r.stats.tiling_overlaps >= u64::from(n - 1));
    assert!(!r.is_ok());
    assert_eq!(r.counts[&23], (u64::from(n - 1), 0), "{:#?}", r.issues);
}

#[test]
fn validator_still_inflates_every_distinct_record() {
    let mut b = Builder::new();
    for i in 0..20u32 {
        b.compressed(0x0100_0000 + i, &payload(i, 3000), Compression::default());
    }
    b.add(
        0x0200_0000,
        3,
        zlib(&payload(7, 9000), Compression::fast()),
        Layout::Fragmented {
            first: 100,
            chunks: vec![500, 500, 500, 1],
        },
    );
    let r = open(&b.build()).validate_with(&ValidateOptions {
        inflate_compressed: true,
        ..ValidateOptions::default()
    });
    assert!(r.is_ok(), "{:#?}", r.issues);
    assert_eq!(r.stats.inflated, 21);
}

// ---- [1] / [8] membership record decoded length ---------------------------------------

/// An archive whose 0xFFFF0005 record is `stored` with `flags`.
fn with_membership(flags: u16, stored: Vec<u8>) -> Built {
    let mut b = Builder::new();
    b.membership = None;
    b.stored(0x0100_0000, payload(0, 16));
    b.add(0xFFFF_0005, flags, stored, Layout::Plain);
    b.build()
}

#[test]
fn membership_checks_the_decoded_length() {
    for raw_len in [9usize, 20] {
        let z = zlib(&payload(1, raw_len), Compression::none());
        assert!(z.len() >= 24, "stored length must pass the old check");
        let a = open(&with_membership(3, z));
        let m = a.membership();
        assert!(
            matches!(m, Err(Error::Corrupt { .. })),
            "{raw_len}-byte membership must be corrupt: {m:?}"
        );
    }
    // The stored form is rejected too.
    let a = open(&with_membership(2, payload(1, 9)));
    assert!(matches!(a.membership(), Err(Error::Corrupt { .. })));
    // A well-formed compressed record still decodes.
    let mut m = vec![0x42u8; 16];
    m.extend_from_slice(&0u32.to_le_bytes());
    m.extend_from_slice(&1u32.to_le_bytes());
    let a = open(&with_membership(3, zlib(&m, Compression::default())));
    let got = a.membership().unwrap().unwrap();
    assert_eq!(
        (got.set_id, got.part_index, got.part_count),
        ([0x42; 16], 0, 1)
    );
}

#[test]
fn membership_does_not_inflate_a_huge_record() {
    // 0xFFFF0005 claims 64 MiB inflated: refused before anything that large is allocated.
    let raw = vec![0u8; 64 << 20];
    let z = zlib(&raw, Compression::best());
    let a = open(&with_membership(3, z));
    assert!(matches!(
        a.membership(),
        Err(Error::TooLarge { .. } | Error::Corrupt { .. })
    ));
    // A stored record far larger than the 24-byte layout is refused unread.
    let a = open(&with_membership(2, payload(3, 1 << 20)));
    assert!(matches!(
        a.membership(),
        Err(Error::TooLarge { .. } | Error::Corrupt { .. })
    ));
}

// ---- [2] one I-19 issue per record ----------------------------------------------------

#[test]
fn out_of_range_fragments_are_reported_once_per_record() {
    let mut b = Builder::new();
    b.fragmented(0x0100_0000, payload(0, 3000), 200, vec![500, 500, 500, 500]);
    let mut x = b.build();
    let (_, f) = fragment_pair(&x, 0x0100_0000, 0);
    assert_eq!(f, 4);
    for i in 0..f {
        let (pair, _) = fragment_pair(&x, 0x0100_0000, i);
        x.put_u32(pair + 4, 0xFFFF_0000);
    }
    let r = open(&x).validate_with(&ValidateOptions {
        check_tiling: false,
        ..ValidateOptions::default()
    });
    assert_eq!(r.counts[&19], (0, 1), "{:#?}", r.issues);
    let i19: Vec<_> = r.issues.iter().filter(|i| i.invariant == 19).collect();
    assert!(i19[0].message.contains("4 of 4"), "{}", i19[0].message);
}

#[test]
fn suppressed_issues_are_still_counted() {
    // 30 records with a reserved field set: 30 I-16 warnings, 20 kept, 10 suppressed.
    let mut b = Builder::new();
    for i in 0..30u32 {
        b.stored(0x0100_0000 + i, payload(i, 16));
    }
    let mut x = b.build();
    let (root, _) = x.root();
    for i in 0..30 {
        x.put_u32(Built::entry_pos(root, i) + 28, 1);
    }
    let r = open(&x).validate();
    assert_eq!(r.counts[&16], (30, 0));
    assert_eq!(r.issues.iter().filter(|i| i.invariant == 16).count(), 20);
    assert_eq!(r.suppressed, 10);
}

// ---- [3] / [4] set-level severities ---------------------------------------------------

#[test]
fn unusable_archive_with_valid_magic_is_a_set_error() {
    for (pos, value, inv) in [(0x144usize, 0u32, 4u8), (0x194, 40, 6), (0x1A4, 100, 8)] {
        let dir = TempDir::new("unusable");
        let mut g = Builder::new();
        g.stored(0x0100_0001, payload(1, 10));
        write(&dir, "client_gamelogic.dat", &g);
        let mut x = Builder::new();
        x.membership = Some(([2; 16], 0, 1));
        x.stored(0x0100_0002, payload(2, 10));
        let mut built = x.build();
        built.put_u32(pos, value);
        std::fs::write(dir.0.join("client_general.dat"), &built.bytes).unwrap();
        // Not archives: no magic. These stay warnings.
        std::fs::write(dir.0.join("client_broken.dat"), common::noise(2, 5000)).unwrap();
        std::fs::write(dir.0.join("client_tiny.dat"), b"tiny").unwrap();

        let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
        assert_eq!(set.members().len(), 1);
        let r = set.validate(&ValidateOptions::default());
        assert!(!r.is_ok(), "I-{inv}: {:#?}", r.issues);
        let errors: Vec<_> = r
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert_eq!(errors[0].invariant, inv, "{errors:#?}");
        assert!(errors[0].message.contains("client_general.dat"));
        let warned: Vec<_> = r
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .map(|i| i.message.as_str())
            .collect();
        assert!(warned.iter().any(|m| m.contains("client_broken.dat")));
        assert!(warned.iter().any(|m| m.contains("client_tiny.dat")));
        let p = set
            .problems()
            .iter()
            .find(|p| {
                p.path
                    .as_ref()
                    .is_some_and(|x| x.ends_with("client_general.dat"))
            })
            .unwrap();
        assert_eq!((p.severity, p.invariant), (Severity::Error, inv));
    }
}

#[test]
fn missing_part_is_a_set_error() {
    let dir = TempDir::new("missing");
    let mut m = Builder::new();
    m.dataset = 0x0102_0000;
    m.membership = Some(([3; 16], 0, 2));
    m.stored(0x0600_0000, payload(0, 30));
    write(&dir, "client_mesh.dat", &m);
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let r = set.validate(&ValidateOptions::default());
    assert!(!r.is_ok(), "{:#?}", r.issues);
    assert!(r.archives.iter().all(|(_, a)| a.is_ok()));
    let i = r
        .issues
        .iter()
        .find(|i| i.message.contains("missing part 1 of 2"))
        .unwrap();
    assert_eq!((i.invariant, i.severity), (24, Severity::Error));

    // A complete set is fine.
    let mut a = Builder::new();
    a.dataset = 0x0102_0000;
    a.membership = Some(([3; 16], 1, 2));
    write(&dir, "client_mesh_aux_1.datx", &a);
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let r = set.validate(&ValidateOptions::default());
    assert!(r.is_ok(), "{:#?}", r.issues);
    assert!(set.problems().is_empty(), "{:?}", set.problems());
}

#[test]
fn part_count_disagreement_is_a_set_error() {
    let dir = TempDir::new("count");
    for (name, part, count) in [
        ("client_mesh.dat", 0u32, 2u32),
        ("client_mesh_aux_1.datx", 1, 3),
    ] {
        let mut m = Builder::new();
        m.dataset = 0x0102_0000;
        m.membership = Some(([3; 16], part, count));
        write(&dir, name, &m);
    }
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let r = set.validate(&ValidateOptions::default());
    assert!(
        r.issues
            .iter()
            .any(|i| i.severity == Severity::Error && i.message.contains("part count")),
        "{:#?}",
        r.issues
    );
}

// ---- [5] empty non-root node ----------------------------------------------------------

#[test]
fn reader_rejects_an_empty_non_root_leaf() {
    let mut b = Builder::new();
    b.fanout = 5;
    for i in 0..200u32 {
        b.stored(0x0100_0000 + i, payload(i, 8));
    }
    let mut x = b.build();
    // Find a leaf below the root.
    let leaf = (0..x.nodes.len())
        .find(|&i| i != x.root_index && x.node_children[i].is_empty())
        .unwrap();
    let lost = x.node_dids[leaf][0];
    let (leaf_off, _) = x.nodes[leaf];
    x.put_u32(Built::count_pos(leaf_off), 0);
    let a = open(&x);
    assert!(
        matches!(a.lookup(lost), Err(Error::Corrupt { offset, .. }) if offset == u64::from(leaf_off)),
        "{:?}",
        a.lookup(lost)
    );
    assert!(a.entries().any(|e| e.is_err()));
    let r = a.validate();
    assert!(!r.is_ok() && r.counts[&10].1 >= 1);
}

// ---- [6] language selection -----------------------------------------------------------

fn local(dir: &TempDir, lang: &str, sid: u8, dids: &[u32]) {
    let mut b = Builder::new();
    b.dataset = 0x0303_0000;
    b.membership = Some(([sid; 16], 0, 1));
    for &d in dids {
        b.stored(d, payload(d ^ u32::from(sid), 20));
    }
    write(dir, &format!("client_local_{lang}.dat"), &b);
}

#[test]
fn local_routing_uses_one_language() {
    let dir = TempDir::new("lang");
    let mut g = Builder::new();
    g.stored(0x0100_0001, payload(1, 10));
    write(&dir, "client_general.dat", &g);
    // 0x25000002 exists only in DE.
    local(&dir, "DE", 4, &[0x2500_0001, 0x2500_0002]);
    local(&dir, "English", 2, &[0x2500_0001]);

    // Default: English (install-layout §3.5).
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    assert_eq!(set.language(), Some("English"));
    let (l, bytes) = set.read_with_entry(0x2500_0001).unwrap();
    assert!(l.routed);
    assert_eq!(
        set.member(l.member).id.kind,
        ArchiveKind::Local("English".into())
    );
    assert_eq!(bytes, payload(0x2500_0001 ^ 2, 20));
    // No cross-locale fallback (string-table §10.1).
    assert!(set.locate(0x2500_0002).unwrap().is_none());
    // Copies in another language are not cross-archive duplicates.
    let r = set.validate(&ValidateOptions::default());
    assert_eq!(r.duplicate_dids, 0, "{:#?}", r.issues);
    assert!(r.is_ok(), "{:#?}", r.issues);
    assert_eq!(
        r.archives.len(),
        3,
        "every language archive is still validated"
    );

    // Explicit choice.
    let set = ArchiveSet::open(
        &dir.0,
        SetOptions {
            language: Some("DE".into()),
            ..SetOptions::default()
        },
    )
    .unwrap();
    assert_eq!(set.language(), Some("DE"));
    assert_eq!(set.read(0x2500_0001).unwrap(), payload(0x2500_0001 ^ 4, 20));
    assert!(set.locate(0x2500_0002).unwrap().is_some());

    // A requested language that is absent falls back to the default rule and is reported.
    let set = ArchiveSet::open(
        &dir.0,
        SetOptions {
            language: Some("FR".into()),
            ..SetOptions::default()
        },
    )
    .unwrap();
    assert_eq!(set.language(), Some("English"));
    assert!(set.problems().iter().any(|p| p.message.contains("FR")));
}

#[test]
fn without_english_the_first_language_is_used() {
    let dir = TempDir::new("lang2");
    local(&dir, "FR", 3, &[0x2500_0001]);
    local(&dir, "DE", 4, &[0x2500_0001]);
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    assert_eq!(set.language(), Some("DE"));
    assert_eq!(set.read(0x2500_0001).unwrap(), payload(0x2500_0001 ^ 4, 20));
}

// ---- [7] archive-name grammar ---------------------------------------------------------

#[test]
fn archive_names_follow_the_install_grammar() {
    let p = ArchiveId::from_file_name;
    let id = |kind, part| Some(ArchiveId { kind, part });
    // ASCII case-insensitive (install-layout §2); the language keeps its case.
    assert_eq!(p("CLIENT_GENERAL.DAT"), id(ArchiveKind::General, 0));
    assert_eq!(p("client_general.DAT"), id(ArchiveKind::General, 0));
    assert_eq!(p("client_General.dat"), id(ArchiveKind::General, 0));
    assert_eq!(p("client_mesh_AUX_1.datx"), id(ArchiveKind::Mesh, 1));
    assert_eq!(p("Client_Mesh_Aux_2.DATX"), id(ArchiveKind::Mesh, 2));
    assert_eq!(p("CLIENT_CELL_14.DAT"), id(ArchiveKind::Cell(14), 0));
    assert_eq!(
        p("CLIENT_LOCAL_English.DAT"),
        id(ArchiveKind::Local("English".into()), 0)
    );
    assert_eq!(
        p("client_local_EN_GB.dat"),
        id(ArchiveKind::Local("EN_GB".into()), 0)
    );
    assert_eq!(
        p("client_local_EN_GB_aux_1.datx"),
        id(ArchiveKind::Local("EN_GB".into()), 1)
    );
    assert_eq!(p("client_cell_1_aux_1.datx"), id(ArchiveKind::Cell(1), 1));
    assert_eq!(
        p("client_NewThing.dat"),
        id(ArchiveKind::Other("newthing".into()), 0)
    );
    // Rejected by the grammar (install-layout §3.1).
    for bad in [
        "client_mesh_aux_1.dat",
        "client_mesh_aux_0.datx",
        "client_mesh_aux_01.datx",
        "client_mesh.datx",
        "client_cell_01.dat",
        "client_map_007.dat",
        "client_cell_.dat",
        "client_cell_x.dat",
        "client_local_.dat",
        "client_foo_bar.dat",
        "client_.dat",
        "client_aux_1.datx",
        "client_general.dat.jrnl",
        "icudtl.dat",
    ] {
        assert_eq!(p(bad), None, "{bad}");
    }
    assert_eq!(
        ArchiveId {
            kind: ArchiveKind::Local("English".into()),
            part: 0
        }
        .file_name(),
        "client_local_English.dat"
    );
    assert_eq!(
        ArchiveId {
            kind: ArchiveKind::Surface,
            part: 2
        }
        .file_name(),
        "client_surface_aux_2.datx"
    );
}

#[test]
fn upper_case_install_opens() {
    let dir = TempDir::new("upper");
    let mut g = Builder::new();
    g.stored(0x0100_0001, payload(1, 10));
    write(&dir, "CLIENT_GENERAL.DAT", &g);
    let mut m = Builder::new();
    m.dataset = 0x0102_0000;
    m.membership = Some(([3; 16], 0, 2));
    m.stored(0x0600_0000, payload(6, 10));
    write(&dir, "Client_Mesh.dat", &m);
    let mut a = Builder::new();
    a.dataset = 0x0102_0000;
    a.membership = Some(([3; 16], 1, 2));
    a.stored(0x0600_0001, payload(7, 10));
    write(&dir, "client_mesh_AUX_1.datx", &a);
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let ids: Vec<String> = set.members().iter().map(|m| m.id.to_string()).collect();
    assert_eq!(ids, ["general", "mesh", "mesh_aux_1"]);
    assert!(set.problems().is_empty(), "{:?}", set.problems());
    let l = set.locate(0x0600_0001).unwrap().unwrap();
    assert!(l.routed);
    assert_eq!(set.read(0x0100_0001).unwrap(), payload(1, 10));
}

#[test]
fn case_variants_of_one_archive_open_once() {
    let dir = TempDir::new("variants");
    let mut g = Builder::new();
    g.stored(0x0100_0001, payload(1, 10));
    write(&dir, "client_general.dat", &g);
    let mut g2 = Builder::new();
    g2.stored(0x0100_0001, payload(2, 10));
    write(&dir, "CLIENT_GENERAL.DAT", &g2);
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    // On a case-insensitive file system only one of the two files exists.
    if std::fs::read_dir(&dir.0).unwrap().count() == 2 {
        assert_eq!(set.members().len(), 1);
        assert!(set.members()[0].path.ends_with("client_general.dat"));
        assert!(
            set.problems()
                .iter()
                .any(|p| p.message.contains("CLIENT_GENERAL.DAT")),
            "{:?}",
            set.problems()
        );
    }
    assert_eq!(set.read(0x0100_0001).unwrap(), payload(1, 10));
}

// ---- [9] missing free-space table -----------------------------------------------------

#[test]
fn archive_without_free_table_gets_an_i21_warning() {
    let mut b = Builder::new();
    b.stored(0x0100_0001, payload(1, 10));
    let mut x = b.build();
    for pos in [0x154, 0x15C, 0x19C, 0x1A0] {
        x.put_u32(pos, 0);
    }
    let a = open(&x);
    assert!(a.free_table().is_err());
    let r = a.validate();
    assert!(r.has(21), "{:#?}", r.issues);
    assert_eq!(r.counts[&21].1, 0, "I-21 is a warning");
    // Records still read.
    assert_eq!(a.read(0x0100_0001).unwrap(), payload(1, 10));
}

#[test]
fn free_table_must_hold_its_terminator() {
    // §4.5: 16 + 8 × count ≤ size. Shrink the table so the pairs fit but the terminator
    // does not.
    let mut b = Builder::new();
    b.stored(0x0100_0001, payload(1, 10));
    let mut x = b.build();
    let count = x.get_u32(0x15C);
    x.put_u32(0x1A0, 8 + 8 * count);
    let a = open(&x);
    assert!(matches!(a.free_table(), Err(Error::Corrupt { .. })));
    assert!(a.validate().has(21));
}

// ---- spec §2.3 step 3 / §7.2 (revision 2026-10-06): all-zero base identifier ----------

#[test]
fn all_zero_base_identifier_is_adopted_from_the_aux_parts() {
    let dir = TempDir::new("zero-sid");
    for (name, sid, part) in [
        ("client_mesh.dat", [0u8; 16], 0u32),
        ("client_mesh_aux_1.datx", [5; 16], 1),
        ("client_mesh_aux_2.datx", [5; 16], 2),
    ] {
        let mut m = Builder::new();
        m.dataset = 0x0102_0000;
        m.membership = Some((sid, part, 3));
        write(&dir, name, &m);
    }
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    assert!(set.problems().is_empty(), "{:?}", set.problems());

    // An all-zero identifier on an aux part is a warning.
    let mut m = Builder::new();
    m.dataset = 0x0102_0000;
    m.membership = Some(([0; 16], 2, 3));
    write(&dir, "client_mesh_aux_2.datx", &m);
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let msgs: Vec<_> = set
        .problems()
        .iter()
        .map(|p| (p.severity, p.message.as_str()))
        .collect();
    assert_eq!(
        msgs,
        [(Severity::Warning, "aux part has an all-zero set identifier")]
    );
}
