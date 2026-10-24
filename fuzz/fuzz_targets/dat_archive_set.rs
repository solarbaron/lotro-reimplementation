// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera_dat::ArchiveSet` (`docs/specs/dat-container.md` §2, §7.2, §8): opening a
//! directory of archives, names, one file per archive, language choice, set membership,
//! DID routing, record reads (`locate`, `read`, `read_located`, `read_with_entry`,
//! `Archive::read_raw`) and the set validator, on hostile files.
//!
//! `dat_archive` fuzzes one archive from memory; the set only exists on disk, so this
//! target writes up to four files into a private temporary directory (a new one per run,
//! removed when the run ends; never the install) and opens it.
//!
//! Input layout: byte 0 holds options (bit 0: legacy layout, bit 1: inflate while
//! validating, bit 2: no fallback search, bits 3-4: preferred language), byte 1 the file
//! count. Each file then takes a name byte (an index into [`NAMES`], archive-name grammar
//! of `install-layout.md` §3.1, plus a few that do not match it) and a mode byte. Mode
//! bit 0 clear: a well-formed synthetic archive (header §3.3, a leaf root node §5.1, one
//! single-block record per entry §4.1, §5.2) whose entries come from the input, among them
//! usually a set-membership record (§7.2) with an input-chosen identifier, index and count.
//! Mode bit 0 set: a length byte pair and that many raw bytes as the whole file.
//!
//! Nothing may panic. A located record reads the same through every read path, and a
//! reported member is one of the files written.

#![no_main]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use libfuzzer_sys::fuzz_target;
use tessera_dat::{
    ArchiveOptions, ArchiveSet, CURRENT_ENGINE_VERSION, ENTRY_LEN, HEADER_LEN,
    LEGACY_ENGINE_VERSION, MAGIC, MIN_NODE_ALLOC, NODE_PAYLOAD_LEN, SET_MEMBERSHIP_DID, SetOptions,
    ValidateOptions,
};

/// Read caps; real limits are far higher, but a fuzzer must not spend memory on them.
const CAP: u64 = 1 << 20;
/// Most files per run.
const MAX_FILES: usize = 4;
/// Most entries in a synthetic root node (a leaf holds up to 61).
const MAX_ENTRIES: usize = 12;
/// Longest synthetic record payload.
const MAX_PAYLOAD: usize = 256;

/// File names a run can use: bases, aux parts, case variants, languages, regions and
/// names outside the grammar (ignored by the set).
const NAMES: [&str; 16] = [
    "client_general.dat",
    "client_general_aux_1.datx",
    "client_general_aux_2.datx",
    "client_GENERAL.dat",
    "client_local_English.dat",
    "client_local_DE.dat",
    "client_cell_1.dat",
    "client_cell_2.dat",
    "client_map_1.dat",
    "client_surface.dat",
    "client_surface_aux_1.datx",
    "client_gamelogic.dat",
    "client_misc.dat",
    "client_general_aux_10.datx",
    "client_general_aux_1.dat",
    "notes.txt",
];

/// DIDs a synthetic entry can carry (routing prefixes of §8 plus metadata records).
const DIDS: [u32; 12] = [
    0x0100_0001,
    0x0100_0002,
    0x0500_0010,
    0x0600_0001,
    0x2500_0001,
    0x4100_0001,
    0x7000_0001,
    0x8000_0001,
    0xFFFF_0001,
    0xFFFF_0003,
    SET_MEMBERSHIP_DID,
    0xFFFF_FFFF,
];

/// Set identifiers a membership record can carry.
const SET_IDS: [[u8; 16]; 3] = [[0; 16], [0x11; 16], [0x22; 16]];

const LANGUAGES: [Option<&str>; 4] = [None, Some("English"), Some("DE"), Some("FR")];

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos = self.pos.saturating_add(1);
        b
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.u8(), self.u8()])
    }

    fn take(&mut self, n: usize) -> Vec<u8> {
        let start = self.pos.min(self.data.len());
        let end = start.saturating_add(n).min(self.data.len());
        self.pos = self.pos.saturating_add(n);
        self.data.get(start..end).unwrap_or_default().to_vec()
    }
}

fn put(buf: &mut [u8], off: usize, v: u32) {
    if let Some(s) = buf.get_mut(off..off + 4) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

/// One entry of a synthetic archive: DID, flags, stamp, payload.
type SynthEntry = (u32, u16, u16, Vec<u8>);

/// A well-formed archive file: header, a leaf root node right after it, then one
/// single-block allocation per entry (8-byte header {0, 0}, then the payload).
fn archive(entries: &[SynthEntry], legacy: bool, tail: &[u8]) -> Vec<u8> {
    let header_len = HEADER_LEN as usize;
    let root_alloc = MIN_NODE_ALLOC as usize;
    let mut file = vec![0u8; header_len + root_alloc];
    let payload_at = header_len + 8;
    let n = entries.len().min(MAX_ENTRIES);
    put(&mut file, payload_at + 496, n as u32);
    for (i, (did, flags, stamp, payload)) in entries.iter().take(n).enumerate() {
        let offset = file.len();
        let alloc = 8 + payload.len();
        file.extend_from_slice(&[0; 8]);
        file.extend_from_slice(payload);
        let e = payload_at + 500 + i * ENTRY_LEN;
        if let Some(s) = file.get_mut(e..e + 4) {
            s[..2].copy_from_slice(&flags.to_le_bytes());
            s[2..].copy_from_slice(&stamp.to_le_bytes());
        }
        put(&mut file, e + 4, *did);
        put(&mut file, e + 8, offset as u32);
        put(&mut file, e + 12, payload.len() as u32);
        put(&mut file, e + 24, alloc as u32);
    }
    debug_assert!(payload_at + NODE_PAYLOAD_LEN <= header_len + root_alloc);
    file.extend_from_slice(tail);
    let len = u32::try_from(file.len()).unwrap_or(u32::MAX);
    // Empty journal pattern (§3.2) and superblock (§3.3), as in `dat_node`.
    file[0x100..0x104].copy_from_slice(&[0x00, 0x50, 0x4C, 0x00]);
    put(&mut file, 0x140, MAGIC);
    put(&mut file, 0x144, 0x400);
    put(&mut file, 0x148, len);
    put(&mut file, 0x14C, 0x0103_0000);
    put(&mut file, 0x160, HEADER_LEN as u32);
    let version = if legacy {
        LEGACY_ENGINE_VERSION
    } else {
        CURRENT_ENGINE_VERSION
    };
    put(&mut file, 0x174, version);
    put(&mut file, 0x178, 3);
    put(&mut file, 0x190, 0x1A8 - 0x140);
    put(&mut file, 0x194, ENTRY_LEN as u32);
    put(&mut file, 0x198, 8);
    put(&mut file, 0x1A4, MIN_NODE_ALLOC);
    file
}

fn synthetic(b: &mut Bytes<'_>, legacy: bool) -> Vec<u8> {
    let n = usize::from(b.u8()) % (MAX_ENTRIES + 1);
    let mut entries: Vec<SynthEntry> = Vec::new();
    if b.u8() & 3 != 0 {
        // A membership record: identifier, part index, part count (and maybe a cut).
        let mut p = SET_IDS[usize::from(b.u8()) % SET_IDS.len()].to_vec();
        p.extend_from_slice(&u32::from(b.u8() % 5).to_le_bytes());
        p.extend_from_slice(&u32::from(b.u8() % 6).to_le_bytes());
        let cut = b.u8();
        if cut & 0x80 != 0 {
            p.truncate(usize::from(cut & 0x1F));
        }
        let stamp = if cut & 0x40 != 0 { 2 } else { 1 };
        entries.push((SET_MEMBERSHIP_DID, 0, stamp, p));
    }
    for _ in 0..n {
        let did = DIDS[usize::from(b.u8()) % DIDS.len()];
        let flags = u16::from(b.u8() & 3);
        let stamp = u16::from(b.u8() % 4);
        let len = usize::from(b.u8()) % (MAX_PAYLOAD + 1);
        let mut payload = did.to_le_bytes().to_vec();
        payload.extend(b.take(len));
        entries.push((did, flags, stamp, payload));
    }
    // A leaf's entries are in ascending DID order with no repeats (§5.4).
    entries.sort_by_key(|e| e.0);
    entries.dedup_by_key(|e| e.0);
    let tail_len = usize::from(b.u8() % 16);
    let tail = b.take(tail_len);
    archive(&entries, legacy, &tail)
}

/// Runs made by this process; part of each run's directory name.
static RUNS: AtomicU64 = AtomicU64::new(0);

/// One run's private directory, removed when the run ends (a crashing input leaves its
/// files behind for inspection). It is made with `create_dir`, never `create_dir_all`, so
/// a path that already exists (a stale directory or a planted symlink) is never reused:
/// the run is skipped instead.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Option<Self> {
        let run = RUNS.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("tessera-fuzz-dat-set-{}-{run}", std::process::id()));
        std::fs::create_dir(&dir).ok()?;
        Some(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes { data, pos: 0 };
    let flags = b.u8();
    let files = usize::from(b.u8()) % (MAX_FILES + 1);
    // Declared first, so it is dropped (and the directory removed) after the set.
    let Some(scratch) = Scratch::new() else {
        return;
    };
    let dir = scratch.0.clone();
    let mut written = Vec::new();
    for _ in 0..files {
        let name = NAMES[usize::from(b.u8()) % NAMES.len()];
        let mode = b.u8();
        let bytes = if mode & 1 == 0 {
            synthetic(&mut b, mode & 2 != 0)
        } else {
            let len = usize::from(b.u16()) % 8192;
            b.take(len)
        };
        if std::fs::write(dir.join(name), bytes).is_ok() {
            written.push(dir.join(name));
        }
    }

    let options = SetOptions {
        archive: ArchiveOptions {
            max_record_len: CAP,
            max_inflated_len: CAP,
            legacy_layout: flags & 1 != 0,
            node_cache_capacity: 16,
            ..ArchiveOptions::default()
        },
        fallback_search_all: flags & 4 == 0,
        language: LANGUAGES[usize::from(flags >> 3) & 3].map(str::to_owned),
        ..SetOptions::default()
    };
    let Ok(set) = ArchiveSet::open(&dir, options) else {
        return;
    };
    for m in set.members() {
        assert!(written.contains(&m.path), "a member is a written file");
        let _ = set.find(&m.id.kind, m.id.part);
        let _ = m.archive.membership();
    }
    let _ = set.problems();
    let _ = set.language();
    let _ = set.routing();
    for did in DIDS.into_iter().chain([0, 0x0A00_0001, 0x0D00_0001]) {
        let _ = set.candidates(did);
        let located = set.locate(did);
        let read = set.read(did);
        let with_entry = set.read_with_entry(did);
        if let (Ok(a), Ok((_, b))) = (&read, &with_entry) {
            assert_eq!(a, b, "read and read_with_entry agree");
        }
        if let Ok(Some(l)) = located {
            let bytes = set.read_located(&l);
            if let (Ok(a), Ok(b)) = (&bytes, &read) {
                assert_eq!(a, b, "read_located and read agree");
            }
            if let Some(m) = set.archive_of(&l) {
                let _ = m.archive.layout(&l.entry);
                let _ = m.archive.read_raw(&l.entry);
                let _ = m.archive.read_entry(&l.entry);
            }
        }
    }
    let _ = set.validate(&ValidateOptions {
        inflate_compressed: flags & 2 != 0,
        max_tracked: 1 << 12,
        ..ValidateOptions::default()
    });
});
