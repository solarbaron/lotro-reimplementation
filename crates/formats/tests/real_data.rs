// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks against a user's own install (read-only).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install directory. Archives are opened
//! read-only through `tessera-dat`; nothing is ever written. The tests assert the
//! invariants of `docs/specs/records.md`, not observed counts, because counts change with
//! every game patch.
//!
//! By default record families with many members are sampled. Set `TESSERA_FULL=1` to
//! decode every record (slow). Every archive of a family is visited, including the
//! `_aux_N.datx` archives; archives missing from the install are skipped.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use tessera_dat::{Archive, Entry};
use tessera_formats::{
    AnyRecord, AudioContainer, DataIdMap, Did, EnumMapper, FormatError, PropertyDictionary,
    PropertyType, Record, SoundRecord, StringTable, Surface, SurfaceFormat, WaveInfo,
    is_catalogued_prefix, name_hash_utf16,
};

/// Archives that hold render surfaces (spec §2.1, prefix 0x41).
const SURFACE_ARCHIVES: &[&str] = &[
    "surface.dat",
    "surface_aux_1.datx",
    "surface_aux_2.datx",
    "highres.dat",
    "highres_aux_1.datx",
    "highres_aux_2.datx",
    "highres_aux_3.datx",
    "local_English.dat",
];

/// Archives that hold sound records (spec §2.1, prefix 0x0A).
const SOUND_ARCHIVES: &[&str] = &["sound.dat", "sound_aux_1.datx", "local_English.dat"];

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TESSERA_GAME_DIR")?;
    let dir = PathBuf::from(dir);
    dir.is_dir().then_some(dir)
}

fn full() -> bool {
    std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

/// Opens `client_<name>` read-only, or `None` (test skipped) when no install is configured.
fn open(name: &str) -> Option<Archive> {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return None;
    };
    let path = dir.join(format!("client_{name}"));
    Some(Archive::open(&path).unwrap_or_else(|e| panic!("open {}: {e}", path.display())))
}

/// Opens every archive of `names` that exists in the install, read-only. Empty when no
/// install is configured.
fn open_family(names: &[&'static str]) -> Vec<(&'static str, Archive)> {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return Vec::new();
    };
    names
        .iter()
        .filter(|name| dir.join(format!("client_{name}")).exists())
        .filter_map(|&name| open(name).map(|a| (name, a)))
        .collect()
}

/// Every directory entry of `archive` whose DID has the given prefix.
fn entries_with_prefix(archive: &Archive, prefix: u8) -> Vec<Entry> {
    archive
        .entries()
        .map(|e| e.expect("directory walk"))
        .filter(|e| Did(e.did).prefix() == prefix)
        .collect()
}

/// Every `stride`-th element (all of them in full mode), plus every element `keep` selects.
fn sample<T: Copy>(items: &[T], stride: usize, keep: impl Fn(&T) -> bool) -> Vec<T> {
    let stride = if full() { 1 } else { stride.max(1) };
    items
        .iter()
        .enumerate()
        .filter(|(i, item)| i % stride == 0 || keep(item))
        .map(|(_, item)| *item)
        .collect()
}

#[test]
fn property_dictionary_invariants() {
    let Some(gamelogic) = open("gamelogic.dat") else {
        return;
    };
    let did = Did(0x3400_0000);
    let payload = gamelogic.read(did.0).expect("read 0x34000000");
    // MP1, MP2 are enforced by the parser.
    let dict = PropertyDictionary::parse_expecting(&payload, did).expect("decode dictionary");
    assert!(!dict.definitions().is_empty());
    // MP3: every PID with a definition has a name.
    for def in dict.definitions() {
        assert!(
            dict.name(def.pid).is_some(),
            "definition {:#x} without a name",
            def.pid
        );
        // MP5 (`property-system.md` §2.5, corrected): the data field matches the type.
        let prefix = def.data.prefix();
        match def.ty {
            PropertyType::Enum => assert!(prefix == 0x23 || prefix == 0x28, "{def:?}"),
            PropertyType::Bitfield64 | PropertyType::Bitfield32 | PropertyType::BitfieldAny => {
                assert_eq!(prefix, 0x23, "{def:?}");
            }
            PropertyType::PropertyId => assert_eq!(def.data.0, 0x3400_0000, "{def:?}"),
            // A DB-object-type code (§9): non-zero and below 0x100.
            PropertyType::DidRef => {
                assert!(0 < def.data.0 && def.data.0 < 0x100, "{def:?}");
                assert_eq!(def.dbo_type().map(u32::from), Some(def.data.0), "{def:?}");
            }
            _ => {
                assert!(def.data.is_null(), "{def:?}");
                assert_eq!(def.dbo_type(), None);
            }
        }
        assert!(!matches!(def.ty, PropertyType::Unknown(_)), "{def:?}");
        // Subset PIDs are children.
        for s in &def.subset {
            assert!(def.children.contains(s), "{def:?}");
        }
    }
    // MP4: two PID families.
    for (pid, _) in dict.names() {
        assert!(*pid < 0x1000_0000 || (*pid >> 16) == 0x1000, "pid {pid:#x}");
    }
}

/// Decodes every DID map in `general`, keyed by DID.
fn all_did_maps(general: &Archive) -> HashMap<u32, DataIdMap> {
    entries_with_prefix(general, 0x28)
        .iter()
        .map(|e| {
            let payload = general.read_entry(e).expect("read DID map");
            let m = DataIdMap::parse_expecting(&payload, e.did)
                .unwrap_or_else(|err| panic!("{:#010x}: {err}", e.did));
            (e.did, m)
        })
        .collect()
}

#[test]
fn did_maps_decode_and_resolve() {
    let Some(general) = open("general.dat") else {
        return;
    };
    let decoded = all_did_maps(&general);
    assert!(!decoded.is_empty());
    for m in decoded.values() {
        // DM1: every value is 0 or a DID whose prefix is catalogued in spec §2.1, either
        // with a known meaning or listed as OPEN.
        for entry in m.entries() {
            assert!(
                entry.value.is_null() || is_catalogued_prefix(entry.value.prefix()),
                "DM1 {}: {entry:?}",
                m.did
            );
        }
    }
    let root = decoded.get(&0x2800_0000).expect("root DID map");
    let st = root.by_label("STRINGTABLE").expect("STRINGTABLE label");
    assert_eq!(st.prefix(), 0x28);
    assert!(
        decoded.contains_key(&st.0),
        "STRINGTABLE map {st} not present"
    );
}

#[test]
fn enum_mappers_decode_and_chain() {
    let Some(general) = open("general.dat") else {
        return;
    };
    let mut mappers = HashMap::new();
    for e in entries_with_prefix(&general, 0x23) {
        let payload = general.read_entry(&e).expect("read enum mapper");
        let m = EnumMapper::parse_expecting(&payload, e.did)
            .unwrap_or_else(|err| panic!("{:#010x}: {err}", e.did));
        // EM2: displays never outnumber names.
        assert!(m.displays().len() <= m.names().len(), "{:#010x}", e.did);
        mappers.insert(Did(e.did), m);
    }
    assert!(!mappers.is_empty());
    // Every base chain resolves without cycles to mappers that exist.
    for m in mappers.values() {
        if !m.base.is_null() {
            assert!(mappers.contains_key(&m.base), "missing base {}", m.base);
        }
        let _ = m.resolve(|d| mappers.get(&d), |x| x.name(u32::MAX));
    }
}

#[test]
fn string_table_invariants() {
    let Some(local) = open("local_English.dat") else {
        return;
    };
    let tables = entries_with_prefix(&local, 0x25);
    assert!(!tables.is_empty());
    let mut checked = 0usize;
    for e in sample(&tables, 97, |_| false) {
        let payload = local.read_entry(&e).expect("read string table");
        // ST1 is enforced by the parser.
        let t = StringTable::parse_expecting(&payload, e.did)
            .unwrap_or_else(|err| panic!("{:#010x}: {err}", e.did));
        for entry in &t {
            assert_eq!(entry.token_high, 0, "ST2 {:#010x}", e.did); // ST2
            assert_eq!(
                entry.arg_ids.len(),
                entry.pieces.len().saturating_sub(1),
                "ST3 {:#010x}/{}",
                e.did,
                entry.token
            );
            assert!(entry.arg_names_match_ids(), "ST4 {:#010x}", e.did);
            for id in &entry.arg_ids {
                assert!(*id < 0x1000_0000, "H1");
            }
            if let Some(names) = entry.arg_names() {
                for n in names {
                    assert!(name_hash_utf16(n.units()).is_some());
                }
            }
        }
        // ST5: tokens unique.
        assert!(t.iter().all(|x| t.get(x.token) == Some(x)));
        checked += 1;
    }
    eprintln!("checked {checked} of {} string tables", tables.len());
}

#[test]
fn surfaces_decode() {
    let archives = open_family(SURFACE_ARCHIVES);
    if archives.is_empty() {
        return;
    }
    let mut per_format: HashMap<SurfaceFormat, usize> = HashMap::new();
    let mut rendered: HashMap<SurfaceFormat, usize> = HashMap::new();
    let mut per_archive: Vec<(&str, usize)> = Vec::new();
    for (name, archive) in &archives {
        let surfaces = entries_with_prefix(archive, 0x41);
        let picked = sample(&surfaces, 499, |_| false);
        per_archive.push((name, picked.len()));
        for e in picked {
            let payload = archive.read_entry(&e).expect("read surface");
            // RS1, RS2 are enforced by the parser.
            let s = Surface::parse_expecting(&payload, e.did)
                .unwrap_or_else(|err| panic!("{name} {:#010x}: {err}", e.did));
            *per_format.entry(s.format).or_default() += 1;
            let n = rendered.entry(s.format).or_default();
            if *n < 8 || full() {
                let img = match s.to_rgba8() {
                    Ok(img) => img,
                    // A build without the `jpeg` feature cannot render JPEG surfaces.
                    Err(FormatError::FeatureDisabled(_)) if s.is_jpeg() => continue,
                    Err(err) => panic!("{name} {:#010x} {}: {err}", e.did, s.format),
                };
                assert_eq!(
                    img.pixels.len() as u64,
                    u64::from(img.width) * u64::from(img.height) * 4
                );
                if s.is_jpeg() {
                    assert!(img.width > 0 && img.height > 0);
                } else {
                    assert_eq!((img.width, img.height), (s.width, s.height));
                }
                *n += 1;
            }
        }
    }
    eprintln!("surfaces checked per archive: {per_archive:?}");
    eprintln!("surface formats sampled: {per_format:?}; rendered: {rendered:?}");
    assert!(!per_format.is_empty());
}

#[test]
fn sounds_sniff() {
    let archives = open_family(SOUND_ARCHIVES);
    if archives.is_empty() {
        return;
    }
    // Sound DIDs named by any DID map (this includes the DMAP records of spec §9.2) are
    // always checked, so rare containers are covered without decoding every record.
    let mapped: HashSet<u32> = open("general.dat")
        .map(|general| {
            all_did_maps(&general)
                .values()
                .flat_map(|m| m.entries().iter().map(|e| e.value))
                .filter(|d| d.prefix() == 0x0A)
                .map(u32::from)
                .collect()
        })
        .unwrap_or_default();
    let mut kinds: HashMap<AudioContainer, usize> = HashMap::new();
    let mut per_archive: Vec<(&str, usize)> = Vec::new();
    for (name, archive) in &archives {
        let sounds = entries_with_prefix(archive, 0x0A);
        let picked = sample(&sounds, 101, |e| mapped.contains(&e.did));
        per_archive.push((name, picked.len()));
        for e in picked {
            let payload = archive.read_entry(&e).expect("read sound");
            // SN1 is enforced by the parser.
            let s = SoundRecord::parse_expecting(&payload, e.did)
                .unwrap_or_else(|err| panic!("{name} {:#010x}: {err}", e.did));
            let kind = s.container();
            assert_ne!(kind, AudioContainer::Unknown, "{name} {:#010x}", e.did);
            if kind == AudioContainer::Wave {
                WaveInfo::parse(s.audio)
                    .unwrap_or_else(|err| panic!("{name} {:#010x}: {err}", e.did));
            }
            *kinds.entry(kind).or_default() += 1;
        }
    }
    eprintln!("sounds checked per archive: {per_archive:?}");
    eprintln!(
        "audio containers sampled: {kinds:?} ({} DID-map targets)",
        mapped.len()
    );
    assert!(!kinds.is_empty());
}

#[test]
fn any_record_dispatch() {
    let Some(general) = open("general.dat") else {
        return;
    };
    // One record of every decodable prefix in general.dat goes through the dispatcher.
    let mut seen = HashSet::new();
    for e in general.entries().map(|e| e.expect("directory walk")) {
        if !AnyRecord::is_decodable(e.did) || !seen.insert(Did(e.did).prefix()) {
            continue;
        }
        let payload = general.read_entry(&e).expect("read record");
        let rec = AnyRecord::decode(e.did, &payload)
            .unwrap_or_else(|err| panic!("{:#010x}: {err}", e.did))
            .expect("decodable prefix");
        assert_eq!(rec.did(), Did(e.did));
    }
    assert!(!seen.is_empty());
}
