// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data tests against a user's own install (read-only), skipped unless
//! `TESSERA_GAME_DIR` is set:
//!
//! - the property registry loads, and every definition's label source is checked;
//! - the root DID map leads to the EMAPPER map and the DB-object-type mapper by label;
//! - 1,000 random game objects (0x70…) are resolved through the +0x09000000 rule with
//!   names, enum labels, bit labels, PID names and DB-object-type names, and the rates
//!   are reported.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test -p tessera-props --test real_data --
//! --nocapture`. `TESSERA_SAMPLE` overrides the sample size and `TESSERA_SEED` the
//! random seed. Only counts, rates, DIDs and PIDs are printed; no game data leaves the
//! install.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use tessera_props::{Did, GameData, Label, PropertyType};

fn game_data() -> Option<&'static GameData> {
    static DATA: OnceLock<Option<GameData>> = OnceLock::new();
    DATA.get_or_init(|| {
        let dir = match std::env::var_os("TESSERA_GAME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                eprintln!("TESSERA_GAME_DIR not set; skipping");
                return None;
            }
        };
        Some(GameData::open(&dir).expect("open install"))
    })
    .as_ref()
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        100.0
    } else {
        n as f64 * 100.0 / d as f64
    }
}

#[test]
fn registry_loads() {
    let Some(gd) = game_data() else { return };
    let reg = gd.registry().expect("registry");
    assert!(!reg.is_empty());
    let mut by_type: BTreeMap<u32, usize> = BTreeMap::new();
    let mut unnamed = 0;
    let (mut sources, mut sources_present) = (0, 0);
    for (def, name) in reg.iter() {
        *by_type.entry(def.ty.code()).or_default() += 1;
        unnamed += usize::from(name.is_none());
        assert!(
            !matches!(def.ty, PropertyType::Unknown(_)),
            "PID {:#x} has unknown type",
            def.pid
        );
        if let Some(src) = reg.enum_source(def.pid) {
            sources += 1;
            let present = match src {
                tessera_props::EnumSource::Mapper(d) => {
                    gd.enum_mapper(d).expect("mapper").is_some()
                }
                tessera_props::EnumSource::DidMap(d) => gd.did_map(d).expect("did map").is_some(),
            };
            sources_present += usize::from(present);
        }
    }
    println!(
        "registry: {} definitions, {} without a name; types {:?}",
        reg.len(),
        unnamed,
        by_type
    );
    println!(
        "label sources: {sources_present}/{sources} present ({:.2}%), {} records cached",
        pct(sources_present, sources),
        gd.cached_records()
    );
    // MP3: every defined PID has a name.
    assert_eq!(unnamed, 0);
    assert_eq!(by_type.keys().copied().max(), Some(22));
    assert!(pct(sources_present, sources) >= 99.0);
}

#[test]
fn did_map_navigation() {
    let Some(gd) = game_data() else { return };
    let root = gd.root_did_map().expect("root map");
    println!("root DID map: {} entries", root.len());
    let emapper = gd
        .did_map_by_label("EMAPPER")
        .expect("EMAPPER")
        .expect("EMAPPER present");
    println!("EMAPPER: {} entries", emapper.len());
    let (mut live, mut total, mut maps) = (0, 0, 0);
    for e in emapper.iter().filter(|e| !e.value.is_null()) {
        total += 1;
        if gd.did_map(e.value).ok().flatten().is_some() {
            maps += 1;
        } else {
            live += usize::from(gd.enum_mapper(e.value).ok().flatten().is_some());
        }
    }
    println!(
        "EMAPPER targets: {live} existing mappers, {maps} DID maps, {} dangling (of {total})",
        total - live - maps
    );
    let dbo = gd
        .enum_mapper_by_name("DBOType")
        .expect("DBOType")
        .expect("DBOType mapper present");
    println!("DBOType mapper {} (base {})", dbo.did, dbo.base);
    // Every DB-object-type code declared by a type-20 definition has a label.
    let reg = gd.registry().expect("registry");
    let mut codes: Vec<u8> = reg.iter().filter_map(|(d, _)| d.dbo_type()).collect();
    codes.sort_unstable();
    codes.dedup();
    let named = codes
        .iter()
        .filter(|&&c| gd.dbo_type_name(c).ok().flatten().is_some())
        .count();
    println!("DB-object-type codes: {named}/{} named", codes.len());
    assert_eq!(named, codes.len());
}

#[derive(Debug, Default)]
struct Stats {
    objects: usize,
    with_set: usize,
    no_set: usize,
    errors: usize,
    values: usize,
    named: usize,
    enums: usize,
    enums_ok: usize,
    map_keys: usize,
    map_keys_ok: usize,
    bits: usize,
    bits_ok: usize,
    pids: usize,
    pids_ok: usize,
    dids: usize,
    dids_ok: usize,
    max_depth: usize,
}

#[test]
fn resolve_random_objects() {
    let Some(gd) = game_data() else { return };
    let set = gd.source();
    let mut dids: Vec<u32> = Vec::new();
    for m in set.members() {
        for e in m.archive.entries_range(0x7000_0000..0x7100_0000) {
            dids.push(e.expect("directory entry").did);
        }
    }
    dids.sort_unstable();
    dids.dedup();
    assert!(!dids.is_empty(), "no 0x70 objects found");

    let sample = (env_u64("TESSERA_SAMPLE", 1000) as usize).min(dids.len());
    let mut seed = env_u64("TESSERA_SEED", 0x5EED_0F7E_55E7_A001).max(1);
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    // Partial Fisher-Yates: the first `sample` slots become the sample.
    for i in 0..sample {
        let j = i + (next() as usize) % (dids.len() - i);
        dids.swap(i, j);
    }

    let mut s = Stats::default();
    let mut failures: Vec<(u32, String)> = Vec::new();
    for &did in &dids[..sample] {
        s.objects += 1;
        let props = match gd.properties(Did(did)) {
            Ok(Some(p)) => p,
            Ok(None) => {
                s.no_set += 1;
                continue;
            }
            Err(e) => {
                s.errors += 1;
                failures.push((did, e.to_string()));
                continue;
            }
        };
        s.with_set += 1;
        let resolved = gd.resolve(&props).expect("resolve");
        let mut depth_of = Vec::new();
        props.walk(|d, _| depth_of.push(d));
        s.max_depth = s.max_depth.max(depth_of.into_iter().max().unwrap_or(0));
        for r in &resolved {
            r.walk(|_, p| {
                s.values += 1;
                s.named += usize::from(p.name.is_some());
                let ok = p.label.is_resolved() == Some(true);
                match &p.label {
                    Label::Enum { .. } => {
                        s.enums += 1;
                        s.enums_ok += usize::from(ok);
                    }
                    Label::DidMapKey { .. } => {
                        s.map_keys += 1;
                        s.map_keys_ok += usize::from(ok);
                    }
                    Label::Bits { bits, .. } => {
                        s.bits += bits.len();
                        s.bits_ok += bits.iter().filter(|b| b.label.is_resolved()).count();
                    }
                    Label::Property { pid, .. } if *pid != 0 => {
                        s.pids += 1;
                        s.pids_ok += usize::from(ok);
                    }
                    Label::DidRef { did, .. } if !did.is_null() => {
                        s.dids += 1;
                        s.dids_ok += usize::from(ok);
                    }
                    // §7.4, §4.9: PID 0 and the null DID mean "none" and need no label,
                    // although the dictionary names PID 0 (MP3).
                    Label::Property { name, .. } => {
                        assert!(name.is_none() && ok, "PID 0 labelled {name:?}");
                        assert_eq!(p.label.to_string(), "none");
                    }
                    Label::DidRef { .. } => assert!(ok, "null DID unresolved"),
                    _ => {}
                }
            });
        }
    }

    println!("objects sampled:       {} of {}", s.objects, dids.len());
    println!(
        "  with property set:   {} ({:.2}%), none: {}, errors: {}",
        s.with_set,
        pct(s.with_set, s.objects),
        s.no_set,
        s.errors
    );
    println!(
        "  values (all depths): {} (max depth {})",
        s.values, s.max_depth
    );
    println!(
        "  named:               {}/{} ({:.3}%)",
        s.named,
        s.values,
        pct(s.named, s.values)
    );
    println!(
        "  enum labels:         {}/{} ({:.3}%)",
        s.enums_ok,
        s.enums,
        pct(s.enums_ok, s.enums)
    );
    println!(
        "  DID-map enum keys:   {}/{} ({:.3}%)",
        s.map_keys_ok,
        s.map_keys,
        pct(s.map_keys_ok, s.map_keys)
    );
    println!(
        "  bit labels:          {}/{} ({:.3}%)",
        s.bits_ok,
        s.bits,
        pct(s.bits_ok, s.bits)
    );
    println!(
        "  PID-value names:     {}/{} ({:.3}%)",
        s.pids_ok,
        s.pids,
        pct(s.pids_ok, s.pids)
    );
    println!(
        "  DID-ref type names:  {}/{} ({:.3}%)",
        s.dids_ok,
        s.dids,
        pct(s.dids_ok, s.dids)
    );
    for (did, e) in failures.iter().take(10) {
        println!("  failure {did:#010x}: {e}");
    }
    println!("  records cached:      {}", gd.cached_records());

    // `property-system.md` §6.2 and §11: every 0x70 object has a 0x79 set and all decode.
    assert!(pct(s.with_set, s.objects) >= 99.5);
    assert_eq!(s.errors, 0);
    // §2.5 MP3: every PID that has a definition has a name.
    assert_eq!(s.named, s.values);
    // §7.1, §7.3, §7.4: labels resolve almost always (a few enum keys are unlabelled).
    assert!(pct(s.enums_ok, s.enums) >= 99.0);
    assert!(pct(s.bits_ok, s.bits) >= 99.0);
    assert_eq!(s.pids_ok, s.pids);
    assert!(pct(s.dids_ok, s.dids) >= 99.0);
}
