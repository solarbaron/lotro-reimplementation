// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests against synthetic archives produced by the test-only writer in `common`.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use common::{Builder, Built, Layout, TempDir, noise, payload};
use flate2::Compression;
use tessera_dat::{
    Archive, ArchiveId, ArchiveKind, ArchiveOptions, ArchiveSet, Error, RecordLayout, SetOptions,
    ValidateOptions,
};

fn open(b: &Built) -> Archive {
    Archive::from_bytes(b.bytes.clone(), ArchiveOptions::default()).unwrap()
}

fn open_with(b: &Built, o: ArchiveOptions) -> Archive {
    Archive::from_bytes(b.bytes.clone(), o).unwrap()
}

fn full_validate() -> ValidateOptions {
    ValidateOptions {
        inflate_compressed: true,
        ..ValidateOptions::default()
    }
}

/// A mixed archive: stored, compressed (both zlib levels), fragmented, slack records.
fn mixed(n: u32) -> (Builder, BTreeMap<u32, Vec<u8>>) {
    let mut b = Builder::new();
    let mut expect = BTreeMap::new();
    for i in 0..n {
        let did = 0x0100_0000 + i * 3;
        let data = payload(did, 20 + (i as usize * 37) % 900);
        match i % 5 {
            0 | 1 => {
                b.stored(did, data.clone());
            }
            2 => {
                b.compressed(did, &data, Compression::default());
            }
            3 => {
                b.compressed(did, &data, Compression::best());
            }
            _ => {
                b.add(did, 2, data.clone(), Layout::Slack(64));
            }
        }
        expect.insert(did, data);
    }
    // Fragmented records: stored and compressed.
    let big = payload(0x0200_0000, 5000);
    b.fragmented(0x0200_0000, big.clone(), 600, vec![1000, 700, 1]);
    expect.insert(0x0200_0000, big);
    let raw = noise(9, 7000);
    let z = common::zlib(&raw, Compression::fast());
    b.add(
        0x0200_0001,
        3,
        z,
        Layout::Fragmented {
            first: 100,
            chunks: vec![300, 300, 300, 300, 300, 300, 300, 1],
        },
    );
    expect.insert(0x0200_0001, raw);
    (b, expect)
}

#[test]
fn archive_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Archive>();
    assert_send_sync::<ArchiveSet>();
}

#[test]
fn header_fields() {
    let (b, _) = mixed(10);
    let built = b.build();
    let a = open(&built);
    let h = a.header();
    assert_eq!(h.min_alloc, 0x100);
    assert_eq!(u64::from(h.file_size), a.file_len());
    assert_eq!(h.dataset_type(), 1);
    assert_eq!(h.archive_kind_code(), 3);
    assert_eq!(h.engine_version, 112);
    assert!(h.journal_is_empty());
    assert!(h.layout_sizes_are_standard());
    assert_eq!((h.root_offset, h.root_size), built.root());
    assert_eq!(h.free_count, 2);
}

#[test]
fn lookup_read_and_iterate_mixed() {
    let (b, expect) = mixed(2000);
    let built = b.build();
    let a = open(&built);
    // Depth ≥ 1: the root must be an interior node.
    assert!(!built.node_children[built.root_index].is_empty());

    for (did, data) in &expect {
        assert_eq!(&a.read(*did).unwrap(), data, "DID {did:#x}");
    }
    // Misses: below, between and above keys.
    for did in [
        0,
        0x0100_0001,
        0x0100_0002,
        0x01FF_FFFF,
        0x0300_0000,
        0xFFFF_FFFF,
    ] {
        assert_eq!(a.lookup(did).unwrap(), None);
        assert!(matches!(a.read(did), Err(Error::NotFound { .. })));
    }
    let dids: Vec<u32> = a.entries().map(|e| e.unwrap().did).collect();
    let mut want: Vec<u32> = expect.keys().copied().collect();
    want.push(0xFFFF_0005);
    assert_eq!(dids, want, "in-order iteration must be sorted and complete");

    let m = a.membership().unwrap().unwrap();
    assert_eq!((m.part_index, m.part_count), (0, 1));

    let frag = a.lookup(0x0200_0001).unwrap().unwrap();
    match a.layout(&frag).unwrap() {
        RecordLayout::Modern { fragments, .. } => assert_eq!(fragments.len(), 8),
        other => panic!("unexpected layout {other:?}"),
    }
    assert!(frag.is_compressed());
    assert!(frag.alloc_size < frag.length + 8);
}

#[test]
fn validate_clean_archive() {
    let (b, _) = mixed(3000);
    let a = open(&b.build());
    let r = a.validate_with(&full_validate());
    assert!(r.issues.is_empty(), "{:#?}", r.issues);
    assert_eq!(r.errors() + r.warnings(), 0);
    assert_eq!(r.stats.entries, 3003);
    assert_eq!(r.stats.fragmented, 2);
    assert_eq!(r.stats.max_fragments, 8);
    assert_eq!(r.stats.compressed, 3000 / 5 * 2 + 1);
    assert_eq!(r.stats.inflated, r.stats.compressed);
    assert_eq!(r.stats.slack_records, 600);
    assert_eq!(r.stats.free_extents, 2);
    assert_eq!(r.stats.tiling_gaps + r.stats.tiling_overlaps, 0);
    assert!(r.stats.max_depth >= 1);
}

#[test]
fn tiny_and_deep_trees() {
    // One metadata record only (like the placeholder cell/map archives).
    let b = Builder::new();
    let a = open(&b.build());
    assert_eq!(a.entries().count(), 1);
    assert!(a.validate().is_ok());

    // Fanout 3 forces a deep tree; nodes under 30 entries only warn.
    let mut b = Builder::new();
    b.fanout = 3;
    for i in 0..2000u32 {
        b.stored(0x0400_0000 + i, payload(i, 12));
    }
    let built = b.build();
    let a = open(&built);
    for i in (0..2000u32).step_by(7) {
        assert_eq!(a.read(0x0400_0000 + i).unwrap(), payload(i, 12));
    }
    assert_eq!(a.entries().filter(|e| e.is_ok()).count(), 2001);
    let r = a.validate();
    assert!(r.is_ok(), "{:#?}", r.issues);
    assert!(r.stats.max_depth >= 4, "depth {}", r.stats.max_depth);
    assert!(r.has(10));
}

#[test]
fn large_nodes_and_min_alloc() {
    // Minimum allocation above the node size, plus reused bigger node extents.
    let mut b = Builder::new();
    b.min_alloc = 0x1000;
    b.node_extra = 12;
    b.stale_garbage = false;
    for i in 0..300u32 {
        b.stored(0x0600_0000 + i, payload(i, 3000));
    }
    let a = open(&b.build());
    assert_eq!(a.header().root_size, 0x1000 + 12);
    assert_eq!(a.read(0x0600_0007).unwrap(), payload(7, 3000));
    let r = a.validate();
    assert!(r.issues.is_empty(), "{:#?}", r.issues);
}

#[test]
fn legacy_chain_is_gated() {
    let data = payload(0x0700_0001, 1000);
    let mut b = Builder::new();
    b.stored(0x0700_0000, payload(1, 50));
    b.legacy(0x0700_0001, data.clone(), 300);
    let built = b.build();

    let a = open(&built);
    assert!(matches!(
        a.read(0x0700_0001),
        Err(Error::UnsupportedLegacyLayout { .. })
    ));
    assert!(a.read(0x0700_0000).is_ok());
    let r = a.validate();
    assert!(!r.is_ok() && r.has(18));

    let a = open_with(
        &built,
        ArchiveOptions {
            legacy_layout: true,
            ..ArchiveOptions::default()
        },
    );
    assert_eq!(a.read(0x0700_0001).unwrap(), data);
    let e = a.lookup(0x0700_0001).unwrap().unwrap();
    assert!(
        matches!(a.layout(&e).unwrap(), RecordLayout::Legacy { ref blocks } if blocks.len() == 4)
    );
    let r = a.validate();
    assert!(r.is_ok(), "{:#?}", r.issues);
    assert_eq!(r.stats.legacy, 1);
    assert_eq!(r.stats.tiling_gaps, 0);
}

#[test]
fn file_backed_open_and_parallel_reads() {
    let (b, expect) = mixed(1500);
    let dir = TempDir::new("file");
    let path = dir.0.join("client_general.dat");
    std::fs::write(&path, b.build().bytes).unwrap();
    let a = Arc::new(Archive::open(&path).unwrap());
    assert_eq!(a.path(), Some(path.as_path()));
    let expect = Arc::new(expect);
    std::thread::scope(|s| {
        for t in 0..8usize {
            let a = Arc::clone(&a);
            let expect = Arc::clone(&expect);
            s.spawn(move || {
                for (did, data) in expect.iter().skip(t).step_by(8) {
                    assert_eq!(&a.read(*did).unwrap(), data);
                }
            });
        }
    });
    assert!(a.validate().is_ok());
}

#[test]
fn caps_are_enforced() {
    let mut b = Builder::new();
    b.compressed(0x0100_0000, &vec![0u8; 100_000], Compression::best());
    b.stored(0x0100_0001, vec![1u8; 10_000]);
    let built = b.build();
    let a = open_with(
        &built,
        ArchiveOptions {
            max_inflated_len: 50_000,
            max_record_len: 5_000,
            ..ArchiveOptions::default()
        },
    );
    assert!(matches!(a.read(0x0100_0000), Err(Error::TooLarge { .. })));
    assert!(matches!(a.read(0x0100_0001), Err(Error::TooLarge { .. })));
}

// ---- corruption: errors, never panics ------------------------------------------------

fn base() -> Built {
    let (b, _) = mixed(500);
    b.build()
}

#[test]
fn rejects_bad_headers() {
    let built = base();
    let mut x = built.bytes.clone();
    x[0x140] ^= 0xFF;
    assert!(matches!(
        Archive::from_bytes(x, ArchiveOptions::default()),
        Err(Error::BadMagic { .. })
    ));
    assert!(matches!(
        Archive::from_bytes(built.bytes[..0x3FF].to_vec(), ArchiveOptions::default()),
        Err(Error::TooSmall { .. })
    ));
    let mut x = built.clone();
    x.put_u32(0x194, 40);
    assert!(Archive::from_bytes(x.bytes, ArchiveOptions::default()).is_err());
    let mut x = built.clone();
    x.put_u32(0x1A4, 100);
    assert!(Archive::from_bytes(x.bytes, ArchiveOptions::default()).is_err());
    let mut x = built.clone();
    x.put_u32(0x160, 0xFFFF_FF00);
    assert!(Archive::from_bytes(x.bytes, ArchiveOptions::default()).is_err());
    let mut x = built.clone();
    x.put_u32(0x144, 0);
    assert!(Archive::from_bytes(x.bytes, ArchiveOptions::default()).is_err());
    // Truncated body: header still claims the old length.
    let short = built.bytes[..built.bytes.len() / 2].to_vec();
    if let Ok(a) = Archive::from_bytes(short, ArchiveOptions::default()) {
        let r = a.validate();
        assert!(r.has(3));
        let _ = a.entries().count();
    }
}

#[test]
fn detects_cycles_and_bad_nodes() {
    let built = base();
    let (root_off, root_size) = built.root();
    let first_child = built.node_children[built.root_index][0];
    let (child_off, _) = built.nodes[first_child];

    // Child pointer 1 of the root points back to the root: a cycle.
    let mut x = built.clone();
    x.put_u32(Built::child_ptr_pos(root_off, 1), root_size);
    x.put_u32(Built::child_ptr_pos(root_off, 1) + 4, root_off);
    let a = open(&x);
    let res: Vec<_> = a.entries().collect();
    assert!(
        res.iter()
            .any(|r| matches!(r, Err(Error::TreeCycle { .. } | Error::Corrupt { .. })))
    );
    assert!(matches!(res.last(), Some(Err(_))));
    let r = a.validate();
    assert!(!r.is_ok() && r.has(13));

    // Entry count > 61.
    let mut x = built.clone();
    x.put_u32(Built::count_pos(child_off), 62);
    let a = open(&x);
    assert!(a.entries().any(|r| r.is_err()));
    assert!(a.validate().has(10));

    // Child pointer out of bounds / too small.
    let mut x = built.clone();
    x.put_u32(Built::child_ptr_pos(root_off, 0) + 4, 0xFFFF_0000);
    let a = open(&x);
    assert!(a.entries().any(|r| r.is_err()));
    assert!(a.validate().has(12));
    let mut x = built.clone();
    x.put_u32(Built::child_ptr_pos(root_off, 0), 100);
    assert!(open(&x).validate().has(12));

    // Leaf with a non-zero pointer 1 (I-11).
    let mut x = built.clone();
    x.put_u32(Built::child_ptr_pos(child_off, 1), 7);
    let a = open(&x);
    assert!(a.entries().any(|r| r.is_err()));
    assert!(a.validate().has(11));

    // Non-zero node allocation header (I-9) is reported.
    let mut x = built.clone();
    x.put_u32(child_off as usize + 4, 0x1234);
    assert!(open(&x).validate().has(9));
}

#[test]
fn detects_unsorted_and_entry_problems() {
    let built = base();
    let first_child = built.node_children[built.root_index][0];
    let (child_off, _) = built.nodes[first_child];

    // Swap the DIDs of entries 0 and 1 of a leaf.
    let mut x = built.clone();
    let (p0, p1) = (
        Built::entry_pos(child_off, 0) + 4,
        Built::entry_pos(child_off, 1) + 4,
    );
    let (d0, d1) = (x.get_u32(p0), x.get_u32(p1));
    x.put_u32(p0, d1);
    x.put_u32(p1, d0);
    let a = open(&x);
    assert!(a.entries().any(|r| r.is_err()));
    assert!(a.validate().has(15));

    // Reserved field, flags, first extent out of range, length beyond allocation.
    let e = Built::entry_pos(child_off, 2);
    let mut x = built.clone();
    x.put_u32(e + 28, 1);
    let r = open(&x).validate();
    assert!(r.has(16) && r.is_ok());
    let mut x = built.clone();
    x.bytes[e] = 0x10;
    assert!(!open(&x).validate().is_ok());
    let mut x = built.clone();
    x.put_u32(e + 8, 0xFFFF_FFF0);
    let a = open(&x);
    let did = x.get_u32(e + 4);
    assert!(matches!(a.read(did), Err(Error::OutOfBounds { .. })));
    assert!(a.validate().has(18));
    let mut x = built.clone();
    x.put_u32(e + 12, 0x0010_0000);
    let a = open(&x);
    assert!(a.read(did).is_err());
    assert!(a.validate().has(18));
}

#[test]
fn detects_overlap_and_bad_compression() {
    let built = base();
    let first_child = built.node_children[built.root_index][0];
    let (child_off, _) = built.nodes[first_child];
    let e0 = Built::entry_pos(child_off, 0);
    let e1 = Built::entry_pos(child_off, 1);

    // Entry 1 points at entry 0's extent: overlap (I-20 error).
    let mut x = built.clone();
    let off0 = x.get_u32(e0 + 8);
    x.put_u32(e1 + 8, off0);
    let r = open(&x).validate();
    assert!(r.has(20) && !r.is_ok());
    assert!(r.stats.tiling_overlaps > 0);

    // Corrupt the length prefix of a compressed record.
    let (b, _) = mixed(20);
    let mut x = b.build();
    let did = 0x0100_0000 + 2 * 3; // i = 2 → compressed
    let (off, _) = x.records[&did];
    let prefix = x.get_u32(off as usize + 8);
    x.put_u32(off as usize + 8, prefix + 1);
    let a = open(&x);
    assert!(matches!(a.read(did), Err(Error::Decompress { did: d, .. }) if d == did));
    let r = a.validate_with(&full_validate());
    assert!(r.has(23));
    // Without the inflate option the structural validation is clean.
    assert!(a.validate().is_ok());

    // Fragment pointing outside the file.
    let (b, _) = mixed(20);
    let mut x = b.build();
    let (off, size) = x.records[&0x0200_0000];
    let table = (off + size - 3 * 8) as usize;
    x.put_u32(table + 4, 0xFFFF_0000);
    let a = open(&x);
    assert!(matches!(
        a.read(0x0200_0000),
        Err(Error::OutOfBounds { .. })
    ));
    assert!(a.validate().has(19));
    // Absurd fragment count.
    x.put_u32(off as usize, 0x4000_0000);
    assert!(open(&x).read(0x0200_0000).is_err());
}

#[test]
fn free_table_checks() {
    let built = base();
    let mut x = built.clone();
    let free_bytes = x.get_u32(0x19C);
    x.put_u32(0x19C, free_bytes + 4);
    assert!(open(&x).validate().has(22));
    let mut x = built.clone();
    x.put_u32(0x15C, 10_000);
    let r = open(&x).validate();
    assert!(r.has(21));
    let ft = open(&built).free_table().unwrap();
    assert_eq!(ft.extents.len(), 2);
    assert_eq!(ft.terminator, Some(0));
    assert!(ft.extents[0].offset < ft.extents[1].offset);
}

#[test]
fn random_mutations_never_panic() {
    let (b, _) = mixed(400);
    let built = b.build();
    let len = built.bytes.len();
    let positions = noise(77, 6 * 600);
    for (k, chunk) in positions.chunks_exact(6).enumerate() {
        let mut x = built.bytes.clone();
        // Mutate 1-4 bytes, biased towards the header and the node area.
        let span = if k % 3 == 0 { 0x400 + 8 * 2460 } else { len };
        for j in 0..(1 + k % 4) {
            let pos = (u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[j % 4]]) as usize
                + j * 131)
                % span;
            x[pos] = chunk[4].wrapping_add(chunk[5].wrapping_mul(j as u8));
        }
        let Ok(a) = Archive::from_bytes(
            x,
            ArchiveOptions {
                legacy_layout: k % 2 == 0,
                max_inflated_len: 1 << 20,
                ..ArchiveOptions::default()
            },
        ) else {
            continue;
        };
        let mut seen = 0;
        for e in a.entries() {
            let Ok(e) = e else { break };
            let _ = a.read_entry(&e);
            seen += 1;
            assert!(seen < 10_000);
        }
        let _ = a.lookup(0x0100_0030);
        let _ = a.validate_with(&full_validate());
    }
}

// ---- archive set ---------------------------------------------------------------------

fn write(dir: &TempDir, name: &str, b: &Builder) {
    std::fs::write(dir.0.join(name), b.build().bytes).unwrap();
}

#[test]
fn archive_set_routing_and_problems() {
    let dir = TempDir::new("set");
    let mut general = Builder::new();
    general.membership = Some(([1; 16], 0, 1));
    general.stored(0x0100_0001, payload(0x0100_0001, 40));
    general.stored(0x0E00_0004, payload(0x0E00_0004, 40));
    // Misplaced on purpose: 0x05 routes to anim, found only by the fallback.
    general.stored(0x0500_0001, payload(0x0500_0001, 40));
    write(&dir, "client_general.dat", &general);

    let mut local = Builder::new();
    local.dataset = 0x0303_0000;
    local.subset = 2;
    local.membership = Some(([2; 16], 0, 1));
    local.stored(0x2500_0001, payload(0x2500_0001, 40));
    write(&dir, "client_local_English.dat", &local);

    // Mesh set declares 3 parts; part 2 is missing.
    for part in 0..2u32 {
        let mut mesh = Builder::new();
        mesh.dataset = 0x0102_0000;
        mesh.membership = Some(([3; 16], part, 3));
        mesh.compressed(
            0x0600_0000 + part,
            &payload(part, 3000),
            Compression::default(),
        );
        let name = if part == 0 {
            "client_mesh.dat".to_string()
        } else {
            format!("client_mesh_aux_{part}.datx")
        };
        write(&dir, &name, &mesh);
    }
    for (r, kind) in [(1u32, "cell"), (14, "cell"), (1, "map")] {
        let mut c = Builder::new();
        c.dataset = if kind == "cell" {
            0x0203_0000
        } else {
            0x020A_0000
        };
        c.subset = r;
        c.membership = Some(([10 + r as u8; 16], 0, 1));
        let class = if kind == "cell" { 0 } else { 1 };
        let did = 0x8000_0000 | ((class << 4 | r) << 16) | 0x0102;
        c.stored(did, payload(did, 64));
        write(&dir, &format!("client_{kind}_{r}.dat"), &c);
    }
    // Not archives: wrong prefix, wrong magic, other extension.
    std::fs::write(dir.0.join("icudtl.dat"), noise(1, 5000)).unwrap();
    std::fs::write(dir.0.join("client_broken.dat"), noise(2, 5000)).unwrap();
    std::fs::write(dir.0.join("client_general.txt"), b"x").unwrap();

    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let ids: Vec<String> = set.members().iter().map(|m| m.id.to_string()).collect();
    assert_eq!(
        ids,
        [
            "general",
            "local_English",
            "mesh",
            "mesh_aux_1",
            "cell_1",
            "cell_14",
            "map_1"
        ]
    );
    let problems = set.problems();
    assert!(
        problems
            .iter()
            .any(|p| p.message.contains("missing part 2"))
    );
    assert!(problems.iter().any(|p| {
        p.path
            .as_ref()
            .is_some_and(|x| x.ends_with("client_broken.dat"))
    }));

    // Routed hits.
    let l = set.locate(0x0600_0001).unwrap().unwrap();
    assert!(l.routed);
    assert_eq!(set.member(l.member).id.to_string(), "mesh_aux_1");
    assert_eq!(set.read(0x0600_0001).unwrap(), payload(1, 3000));
    let cell14 = 0x800E_0102;
    let l = set.locate(cell14).unwrap().unwrap();
    assert!(l.routed);
    assert_eq!(set.member(l.member).id.kind, ArchiveKind::Cell(14));
    let map1 = 0x8011_0102;
    assert_eq!(
        set.member(set.locate(map1).unwrap().unwrap().member)
            .id
            .kind,
        ArchiveKind::Map(1)
    );
    assert_eq!(set.candidates(map1).len(), 1);
    assert_eq!(set.read(0x2500_0001).unwrap(), payload(0x2500_0001, 40));

    // Fallback.
    let l = set.locate(0x0500_0001).unwrap().unwrap();
    assert!(!l.routed);
    assert_eq!(set.member(l.member).id.kind, ArchiveKind::General);
    let strict = ArchiveSet::open(
        &dir.0,
        SetOptions {
            fallback_search_all: false,
            ..SetOptions::default()
        },
    )
    .unwrap();
    assert!(strict.locate(0x0500_0001).unwrap().is_none());
    assert!(matches!(set.read(0x0999_0000), Err(Error::NotFound { .. })));

    // Set validation: all archives clean, membership problems as I-24 warnings, no duplicates.
    let r = set.validate(&full_validate());
    assert!(r.is_ok(), "{r:#?}");
    assert_eq!(r.archives.len(), 7);
    assert_eq!(r.duplicate_dids, 0);
    assert!(r.issues.iter().any(|i| i.invariant == 24));
}

#[test]
fn archive_set_detects_duplicates() {
    let dir = TempDir::new("dup");
    let mut g = Builder::new();
    g.stored(0x0100_0001, payload(1, 10));
    write(&dir, "client_general.dat", &g);
    let mut l = Builder::new();
    l.membership = Some(([9; 16], 0, 1));
    l.stored(0x0100_0001, payload(2, 10));
    write(&dir, "client_local_English.dat", &l);
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let r = set.validate(&ValidateOptions::default());
    assert_eq!(r.duplicate_dids, 1);
    assert!(r.issues.iter().any(|i| i.invariant == 25));
    // Routed to general first.
    assert_eq!(set.read(0x0100_0001).unwrap(), payload(1, 10));
}

#[test]
fn empty_dir_is_an_error() {
    let dir = TempDir::new("empty");
    assert!(matches!(
        ArchiveSet::open(&dir.0, SetOptions::default()),
        Err(Error::NoArchives { .. })
    ));
    assert!(matches!(
        ArchiveSet::open(dir.0.join("nope"), SetOptions::default()),
        Err(Error::Io { .. })
    ));
}

#[test]
fn file_names() {
    let p = |s| ArchiveId::from_file_name(s);
    assert_eq!(
        p("client_surface_aux_2.datx"),
        Some(ArchiveId {
            kind: ArchiveKind::Surface,
            part: 2
        })
    );
    assert_eq!(p("client_cell_14.dat").unwrap().kind, ArchiveKind::Cell(14));
    assert_eq!(p("client_map_15.dat").unwrap().kind, ArchiveKind::Map(15));
    assert_eq!(
        p("client_local_English.dat").unwrap().kind,
        ArchiveKind::Local("English".into())
    );
    assert_eq!(
        p("client_local_DE_aux_1.datx").unwrap(),
        ArchiveId {
            kind: ArchiveKind::Local("DE".into()),
            part: 1
        }
    );
    assert_eq!(
        p("client_newthing.dat").unwrap().kind,
        ArchiveKind::Other("newthing".into())
    );
    assert_eq!(p("icudtl.dat"), None);
    assert_eq!(p("client_.dat"), None);
    assert_eq!(p("client_general.txt"), None);
    assert_eq!(p("client_general.dat").unwrap().to_string(), "general");
    assert_eq!(
        p("client_mesh_aux_1.datx").unwrap().to_string(),
        "mesh_aux_1"
    );
}

#[test]
fn bench_iterate_synthetic() {
    let mut b = Builder::new();
    for i in 0..100_000u32 {
        b.stored(0x2500_0000 + i * 2, payload(i, 16));
    }
    let built = b.build();
    let a = open(&built);
    let t = Instant::now();
    let n = a.entries().filter(|e| e.is_ok()).count();
    let iter = t.elapsed();
    let t = Instant::now();
    let mut bytes = 0usize;
    for i in (0..100_000u32).step_by(10) {
        bytes += a.read(0x2500_0000 + i * 2).unwrap().len();
    }
    let reads = t.elapsed();
    let t = Instant::now();
    let r = a.validate();
    let val = t.elapsed();
    assert_eq!(n, 100_001);
    assert!(r.is_ok());
    eprintln!(
        "bench(synthetic): iterate {n} entries in {iter:?}; 10000 lookups+reads ({bytes} B) in \
         {reads:?}; validate in {val:?}"
    );
}

// ---- regression tests for review findings ---------------------------------------------

/// A 40-record archive whose root is an interior node (fanout 5).
fn interior_root() -> Built {
    let mut b = Builder::new();
    b.fanout = 5;
    for i in 0..40u32 {
        b.stored(0x0100_0000 + i, payload(i, 24));
    }
    let built = b.build();
    assert!(!built.node_children[built.root_index].is_empty());
    built
}

#[test]
fn empty_interior_root_is_an_error_but_empty_leaf_root_is_not() {
    let mut x = interior_root();
    let (root_off, _) = x.root();
    x.put_u32(Built::count_pos(root_off), 0);
    let a = open(&x);
    assert!(matches!(a.lookup(0x0100_0000), Err(Error::Corrupt { .. })));
    assert!(matches!(a.entries().last(), Some(Err(_))));
    let r = a.validate();
    assert!(!r.is_ok(), "{:#?}", r.issues);
    assert!(
        r.counts[&10].1 >= 1,
        "I-10 must be an error: {:#?}",
        r.issues
    );

    // A leaf root without entries is an empty archive: readable, I-10 warning only.
    let mut b = Builder::new();
    b.membership = None;
    let a = open(&b.build());
    assert_eq!(a.entries().count(), 0);
    assert_eq!(a.lookup(0x0100_0000).unwrap(), None);
    let r = a.validate_with(&ValidateOptions {
        check_membership: false,
        ..ValidateOptions::default()
    });
    assert!(r.is_ok(), "{:#?}", r.issues);
    assert_eq!(r.counts[&10], (1, 0));
}

#[test]
fn unsorted_error_points_at_the_node() {
    let built = base();
    let first_child = built.node_children[built.root_index][0];
    let (child_off, _) = built.nodes[first_child];
    let mut x = built.clone();
    let (p0, p1) = (
        Built::entry_pos(child_off, 0) + 4,
        Built::entry_pos(child_off, 1) + 4,
    );
    let (d0, d1) = (x.get_u32(p0), x.get_u32(p1));
    x.put_u32(p0, d1);
    x.put_u32(p1, d0);
    let a = open(&x);
    let err = a.entries().find_map(Result::err).unwrap();
    assert!(
        matches!(err, Error::Corrupt { offset, .. } if offset == u64::from(child_off)),
        "{err}"
    );
}

#[test]
fn fragment_table_is_capped_before_allocation() {
    let mut b = Builder::new();
    b.stored(0x0100_0000, payload(0, 1 << 20));
    let mut x = b.build();
    let (off, size) = x.records[&0x0100_0000];
    // The largest fragment count that fits the allocation: (size − 8) / 8.
    x.put_u32(off as usize, (size - 8) / 8);
    let a = open_with(
        &x,
        ArchiveOptions {
            max_record_len: 4096,
            ..ArchiveOptions::default()
        },
    );
    let e = a.lookup(0x0100_0000).unwrap().unwrap();
    for r in [a.layout(&e).map(|_| ()), a.read(0x0100_0000).map(|_| ())] {
        assert!(
            matches!(
                r,
                Err(Error::TooLarge {
                    what: "fragment table",
                    ..
                })
            ),
            "{r:?}"
        );
    }
    let r = a.validate();
    assert!(r.has(18) && !r.is_ok());
    // Within the cap the count is accepted (and then fails on the bogus pairs).
    let a = open_with(
        &x,
        ArchiveOptions {
            max_fragments: u64::from(size),
            ..ArchiveOptions::default()
        },
    );
    assert!(a.layout(&e).is_ok());
}

#[test]
fn free_table_count_is_capped() {
    let mut x = base();
    x.put_u32(0x15C, 1 << 20);
    let a = open(&x);
    assert!(matches!(
        a.free_table(),
        Err(Error::TooLarge {
            what: "free-space table",
            ..
        })
    ));
    assert!(a.validate().has(21));
}

#[test]
fn validator_accounts_shared_allocations_once() {
    // One leaf root with 61 entries; every entry is redirected to the fragmented record.
    let mut b = Builder::new();
    b.membership = None;
    b.fragmented(0x0100_0000, payload(0, 3000), 200, vec![500, 500, 500, 500]);
    for i in 1..61u32 {
        b.stored(0x0100_0000 + i, payload(i, 16));
    }
    let mut x = b.build();
    assert!(x.node_children[x.root_index].is_empty());
    let (off, size) = x.records[&0x0100_0000];
    let (root_off, _) = x.root();
    for i in 1..61 {
        let e = Built::entry_pos(root_off, i);
        x.put_u32(e + 8, off);
        x.put_u32(e + 12, 16);
        x.put_u32(e + 24, size);
    }
    let r = open(&x).validate();
    assert!(!r.is_ok());
    assert_eq!(r.stats.entries, 61);
    assert_eq!(r.stats.fragmented, 1, "the fragment table is parsed once");
    assert_eq!(r.stats.tiling_overlaps, 60);
    assert!(
        r.issues
            .iter()
            .any(|i| i.invariant == 20 && i.message.contains("shared"))
    );
}

#[test]
fn validator_tracking_is_capped() {
    let (b, _) = mixed(300);
    let a = open(&b.build());
    let r = a.validate_with(&ValidateOptions {
        max_tracked: 10,
        ..ValidateOptions::default()
    });
    assert!(r.is_ok(), "{:#?}", r.issues);
    assert!(
        r.issues
            .iter()
            .any(|i| i.invariant == 20 && i.message.contains("skipped"))
    );

    let dir = TempDir::new("cap");
    write(&dir, "client_general.dat", &b);
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let r = set.validate(&ValidateOptions {
        max_tracked: 10,
        ..ValidateOptions::default()
    });
    assert!(r.archives[0].1.has(25));
}

#[test]
fn absurd_set_part_count_is_reported_not_enumerated() {
    let dir = TempDir::new("parts");
    let mut g = Builder::new();
    g.membership = Some(([7; 16], 0, u32::MAX));
    g.stored(0x0100_0001, payload(1, 10));
    write(&dir, "client_general.dat", &g);
    let mut m = Builder::new();
    m.membership = Some(([8; 16], 0, 5));
    write(&dir, "client_mesh.dat", &m);
    let mut m = Builder::new();
    m.membership = Some(([8; 16], 3, 5));
    write(&dir, "client_mesh_aux_3.datx", &m);

    let t = Instant::now();
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    assert!(t.elapsed().as_secs() < 5);
    let msgs: Vec<&str> = set.problems().iter().map(|p| p.message.as_str()).collect();
    assert!(
        msgs.iter().any(|m| m.contains("implausible part count")),
        "{msgs:?}"
    );
    assert!(
        msgs.contains(&"missing parts 1-2, 4 of 5 for mesh"),
        "{msgs:?}"
    );
    assert!(msgs.len() <= 4, "{msgs:?}");
}

#[test]
fn duplicate_dids_are_counted_per_did_across_archives_only() {
    // Same DID in three archives: one duplicate, one issue naming all three.
    let dir = TempDir::new("dup3");
    for (name, sid) in [
        ("client_general.dat", 1u8),
        ("client_gamelogic.dat", 2),
        ("client_local_English.dat", 3),
    ] {
        let mut b = Builder::new();
        b.membership = Some(([sid; 16], 0, 1));
        b.stored(0x0100_0001, payload(u32::from(sid), 10));
        write(&dir, name, &b);
    }
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let r = set.validate(&ValidateOptions::default());
    assert_eq!(r.duplicate_dids, 1);
    let i25: Vec<_> = r.issues.iter().filter(|i| i.invariant == 25).collect();
    assert_eq!(i25.len(), 1, "{i25:?}");
    for name in ["general", "gamelogic", "local_English"] {
        assert!(i25[0].message.contains(name), "{}", i25[0].message);
    }

    // A DID repeated inside one archive is I-15 there, not a cross-archive I-25.
    let dir = TempDir::new("dup1");
    let mut b = Builder::new();
    b.stored(0x0100_0000, payload(0, 10));
    b.stored(0x0100_0001, payload(1, 10));
    let mut x = b.build();
    let (root_off, _) = x.root();
    let d0 = x.get_u32(Built::entry_pos(root_off, 0) + 4);
    x.put_u32(Built::entry_pos(root_off, 1) + 4, d0);
    std::fs::write(dir.0.join("client_general.dat"), &x.bytes).unwrap();
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let r = set.validate(&ValidateOptions::default());
    assert_eq!(r.duplicate_dids, 0);
    assert!(r.issues.iter().all(|i| i.invariant != 25), "{:?}", r.issues);
    assert!(r.archives[0].1.has(15));
}

#[test]
fn located_reads_without_panicking() {
    let dir = TempDir::new("loc");
    let mut g = Builder::new();
    g.stored(0x0100_0001, payload(1, 10));
    write(&dir, "client_general.dat", &g);
    let set = ArchiveSet::open(&dir.0, SetOptions::default()).unwrap();
    let (l, bytes) = set.read_with_entry(0x0100_0001).unwrap();
    assert_eq!(bytes, payload(1, 10));
    assert_eq!(set.read_located(&l).unwrap(), bytes);
    assert_eq!(set.archive_of(&l).unwrap().id.kind, ArchiveKind::General);
    assert!(set.get(l.member).is_some());
    let foreign = tessera_dat::Located { member: 99, ..l };
    assert!(set.archive_of(&foreign).is_none());
    assert!(set.get(99).is_none());
    assert!(matches!(
        set.read_located(&foreign),
        Err(Error::NoSuchMember { index: 99 })
    ));
    assert!(matches!(
        set.read_with_entry(0x0100_0002),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn range_iteration_matches_filtered_walk() {
    use std::ops::{Bound, RangeBounds};
    let mut b = Builder::new();
    b.fanout = 5; // many node boundaries
    for i in 0..300u32 {
        b.stored(0x0100_0000 + i * 3, payload(i, 8));
    }
    let a = open(&b.build());
    let all: Vec<u32> = a.entries().map(|e| e.unwrap().did).collect();
    assert_eq!(all.len(), 301);
    let want = |lo: Bound<u32>, hi: Bound<u32>| -> Vec<u32> {
        all.iter()
            .copied()
            .filter(|d| (lo, hi).contains(d))
            .collect()
    };
    let got = |lo: Bound<u32>, hi: Bound<u32>| -> Vec<u32> {
        a.entries_range((lo, hi)).map(|e| e.unwrap().did).collect()
    };
    // Every key, every gap before a key, below the first and above the last.
    let mut starts: Vec<u32> = all.iter().flat_map(|&d| [d, d.wrapping_sub(1)]).collect();
    starts.extend([0, 0x0100_0000 + 900, u32::MAX]);
    for &s in &starts {
        let from: Vec<u32> = a.entries_from(s).map(|e| e.unwrap().did).collect();
        assert_eq!(
            from,
            want(Bound::Included(s), Bound::Unbounded),
            "from {s:#x}"
        );
    }
    for (k, &s) in starts.iter().enumerate().step_by(7) {
        let e = starts[(k * 31) % starts.len()];
        for lo in [Bound::Included(s), Bound::Excluded(s)] {
            for hi in [Bound::Included(e), Bound::Excluded(e), Bound::Unbounded] {
                assert_eq!(got(lo, hi), want(lo, hi), "{lo:?}..{hi:?}");
            }
        }
    }
    assert_eq!(a.entries_range(..).count(), 301);
    assert_eq!(a.entries_range(5..5).count(), 0);
    assert_eq!(
        a.entries_range((Bound::Excluded(u32::MAX), Bound::Unbounded))
            .count(),
        0
    );
    // A class slice: only DIDs of one prefix.
    let slice: Vec<u32> = a
        .entries_range(0xFFFF_0000..)
        .map(|e| e.unwrap().did)
        .collect();
    assert_eq!(slice, [0xFFFF_0005]);
}
