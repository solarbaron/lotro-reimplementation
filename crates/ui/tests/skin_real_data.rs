// SPDX-License-Identifier: GPL-3.0-or-later
//! Skins against the user's own install (read-only), skipped unless `TESSERA_GAME_DIR` is
//! set (`skin-definition.md` §11 SI1–SI9, and the skin side of the §12.7 procedure).
//!
//! SI10–SI13 need the §8.6 panel rows resolved into element trees (`ui-layout.md` §7.1)
//! and belong to the element-tree tests of the native panels; see the OPEN note in
//! `skin-definition.md` §11.
//!
//! The §12.7 part generates a synthetic skin (CC0, written by this test) in a temporary
//! skins root (D24): it maps the art assets of the lowest base-table DIDs to one-colour TGA
//! files of the replaced surfaces' own sizes, adds a missing-file mapping, an unknown
//! label and a panel override, then checks discovery, activation, the per-image
//! precedence, the decoded pixels, the placeholder case, the override lookup and
//! deactivation. Building and rendering the native panels (§12.7 steps 2–3, 9–10) belongs
//! to the element tree and is not done here. Nothing in the install or the user's settings
//! folder is touched; no DID, label or count is hard-coded.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test -p tessera-ui --release --test skin_real_data
//! -- --nocapture`.

mod skin_common;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use skin_common::{tga_solid, write};
use std::collections::{HashMap, HashSet};

use tessera_formats::ui_layout::{BaseRef, LayoutRef, MediaKind};
use tessera_formats::{PropertyValue, Record, Surface, UiLayout};
use tessera_props::{Did, GameData};
use tessera_ui::skin::{
    ART_ASSET_LABEL, BASE_SKIN_LABEL, DiagnosticKind, ELEMENT_ID_LABEL, EMAPPER_LABEL, ImageChoice,
    LabelIndex, MappingPids, SkinEnvironment, SkinImageError, SkinSelection, SkinSet, SkinTables,
    UNIQUEDB_LABEL, names,
};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// SI2: the labels of one chain, total and distinct after ASCII case folding.
fn chain_counts(gd: &GameData, label: &str) -> (usize, usize) {
    let mut did = gd
        .lookup_path(&[EMAPPER_LABEL, label])
        .unwrap()
        .expect("SI1: EMAPPER entry");
    let mut total = 0;
    let mut folded = BTreeSet::new();
    let mut seen = BTreeSet::new();
    while !did.is_null() && seen.insert(did) {
        let Some(m) = gd.enum_mapper(did).unwrap() else {
            break;
        };
        for (_, name) in m.names() {
            total += 1;
            folded.insert(name.to_ascii_lowercase());
        }
        did = m.base;
    }
    (total, folded.len())
}

#[test]
fn real_tables_and_synthetic_skin() {
    let Some(dir) = game_dir() else { return };
    let gd = GameData::open(&dir).expect("open install");
    let tables = SkinTables::load(&gd).expect("skin tables");

    // SI1, SI2.
    assert!(!tables.art_assets.is_empty() && !tables.elements.is_empty());
    for label in [ART_ASSET_LABEL, ELEMENT_ID_LABEL] {
        let did = gd
            .lookup_path(&[EMAPPER_LABEL, label])
            .unwrap()
            .expect("SI1: EMAPPER entry");
        assert_eq!(did.prefix(), 0x23, "SI1: {label} is not an enum mapper");
        let (total, distinct) = chain_counts(&gd, label);
        assert_eq!(total, distinct, "SI2: ASCII-case collision in {label}");
        eprintln!("{label}: {total} labels");
    }
    // SI3: the record's shape.
    assert!(!tables.base.is_empty(), "SI3: base table empty");
    let registry = gd.registry().unwrap().clone();
    let pid = |name: &str| {
        registry
            .pid(name)
            .unwrap_or_else(|| panic!("SI3: {name} not in the property dictionary"))
    };
    let base_did = gd
        .lookup_path(&[UNIQUEDB_LABEL, BASE_SKIN_LABEL])
        .unwrap()
        .expect("SI3: UNIQUEDB/BaseSkinMappings");
    assert_eq!(base_did.prefix(), 0x78, "SI3: not a property set");
    let base_set = gd.property_set(base_did).unwrap().expect("SI3: record");
    let array_pid = pid(names::MAPPING_ARRAY);
    let top: Vec<u32> = base_set.iter().map(|p| p.pid).collect();
    assert_eq!(top, [array_pid], "SI3: other top-level properties");
    let elements = base_set
        .get(array_pid)
        .and_then(|v| v.as_array())
        .expect("SI3: the mapping array is an array");
    let mut fields = vec![pid(names::ART_ASSET_FIELD), pid(names::DATA_ID_FIELD)];
    fields.sort_unstable();
    let struct_pid = pid(names::MAPPING_STRUCT);
    for e in elements {
        assert_eq!(e.pid, struct_pid, "SI3: element is not a mapping struct");
        let s = e.value.as_struct().expect("SI3: element is a struct");
        let mut got: Vec<u32> = s.iter().map(|p| p.pid).collect();
        got.sort_unstable();
        assert_eq!(got, fields, "SI3: struct fields");
    }
    // SI4: the table drops repeated DIDs and art values, so compare it with the raw pairs.
    let pairs = MappingPids::from_registry(&registry).pairs(&base_set);
    assert_eq!(pairs.len(), elements.len(), "SI4: a zero art value or DID");
    let distinct_dids: BTreeSet<u32> = pairs.iter().map(|p| p.1).collect();
    assert_eq!(distinct_dids.len(), pairs.len(), "SI4: repeated DID");
    assert_eq!(
        tables.base.len(),
        pairs.len(),
        "SI4: repeated DID or art value"
    );
    let mut arts = BTreeSet::new();
    for (did, art) in tables.base.iter() {
        assert!(arts.insert(art), "SI4: art-asset value {art:#x} repeated");
        assert_eq!(tables.base.did(art), Some(did));
        assert!(
            tables.art_assets.label(art).is_some(),
            "SI4: art value {art:#x} has no label"
        );
        assert_eq!(did >> 24, 0x41, "SI4: {did:#010x} is not a render surface");
        assert!(
            gd.read(Did(did)).unwrap().is_some(),
            "SI4: {did:#010x} missing"
        );
    }
    eprintln!(
        "base table: {} entries; game skins (label, mappings): {:?}",
        tables.base.len(),
        tables
            .game_skins
            .iter()
            .map(|g| (g.label.as_str(), g.len()))
            .collect::<Vec<_>>()
    );

    // §12.7 steps 4–8 and 11 at the skin level.
    let mut dids: Vec<u32> = tables.base.iter().map(|(d, _)| d).collect();
    dids.sort_unstable();
    let mut chosen: Vec<(u32, String, u32, u32, u32)> = Vec::new();
    let mut missing: Option<(u32, String)> = None;
    for did in dids {
        let bytes = gd.read(Did(did)).unwrap().unwrap();
        let Ok(s) = Surface::parse_expecting(&bytes, did) else {
            continue;
        };
        if s.width == 0 || s.height == 0 || s.width > 1024 || s.height > 1024 {
            continue;
        }
        let art = tables.base.art_asset(did).unwrap();
        let label = tables.art_assets.label(art).unwrap().to_owned();
        if chosen.len() < 8 {
            let n = chosen.len() as u32;
            let colour = 0xFF00_0000 | (n * 0x1F) << 16 | (255 - n * 0x1F) << 8 | 0x40;
            chosen.push((did, label, s.width, s.height, colour));
        } else {
            missing = Some((did, label));
            break;
        }
    }
    assert!(!chosen.is_empty(), "no skinnable surface found");

    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("skins");
    let skin_dir = root.join("TestSkin");
    let mut xml = String::from("<opt>\n<SkinName Name=\"Tessera synthetic test\"/>\n");
    for (did, label, w, h, colour) in &chosen {
        write(
            &skin_dir.join(format!("art/{did:08x}.tga")),
            &tga_solid(*w as u16, *h as u16, *colour),
        );
        writeln!(
            xml,
            "<Mapping ArtAssetID=\"{label}\" FileName=\"art\\{did:08x}.tga\"/>"
        )
        .unwrap();
    }
    if let Some((did, label)) = &missing {
        writeln!(
            xml,
            "<Mapping ArtAssetID=\"{label}\" FileName=\"art\\missing_{did:08x}.tga\"/>"
        )
        .unwrap();
    }
    xml.push_str("<Mapping ArtAssetID=\"Tessera_No_Such_Label_Xyz\" FileName=\"x.tga\"/>\n");
    let element = lowest_game_label(&tables.elements);
    writeln!(
        xml,
        "<PanelFile ID=\"test\"><Element ID=\"{}\" X=\"11\" Y=\"22\" Width=\"333\" Height=\"44\"/></PanelFile>",
        element.1
    )
    .unwrap();
    xml.push_str("</opt>\n");
    write(&skin_dir.join("SkinDefinition.xml"), xml.as_bytes());

    // Step 7.
    let tables = Arc::new(tables);
    let set = Arc::new(SkinSet::discover(&root, tables.as_ref()));
    assert_eq!(set.skins().len(), 1, "rejected: {:?}", set.rejected());
    let skin = &set.skins()[0];
    assert_eq!(skin.name(), "Tessera synthetic test");
    assert!(
        skin.definition
            .diagnostics
            .iter()
            .any(|d| matches!(&d.kind, DiagnosticKind::UnknownArtAsset(n) if n == "Tessera_No_Such_Label_Xyz"))
    );
    assert_eq!(
        skin.definition.mappings().len(),
        chosen.len() + usize::from(missing.is_some())
    );

    // Step 8.
    let mut sel = SkinSelection::new(
        Arc::clone(&set),
        Arc::clone(&tables),
        SkinEnvironment::default(),
    );
    let overlay = sel.apply_option("TESSERA SYNTHETIC TEST").current.unwrap();
    for (did, _, w, h, colour) in &chosen {
        let ImageChoice::SkinFile(r) = overlay.resolve_image(*did) else {
            panic!("{did:#010x} not substituted");
        };
        let img = r.file.as_ref().unwrap().load().unwrap();
        assert_eq!((img.width(), img.height()), (*w, *h), "{did:#010x}");
        assert_eq!(img.pixel(0, 0), Some(*colour));
    }
    if let Some((did, _)) = &missing {
        let ImageChoice::SkinFile(r) = overlay.resolve_image(*did) else {
            panic!("missing-file DID not substituted");
        };
        assert_eq!(
            r.file.as_ref().unwrap().load().unwrap_err(),
            SkinImageError::NotFound
        );
    } else {
        eprintln!("missing-file case not exercised on real data");
    }
    let unmapped = tables
        .base
        .iter()
        .map(|(d, _)| d)
        .find(|d| !chosen.iter().any(|c| c.0 == *d) && missing.as_ref().is_none_or(|m| m.0 != *d))
        .unwrap();
    assert_eq!(overlay.resolve_image(unmapped), ImageChoice::Base(unmapped));
    let ov = overlay
        .find_override(&[], element.0)
        .expect("panel override");
    assert_eq!((ov.x, ov.y, ov.width, ov.height), (11, 22, 333, 44));

    // Step 11.
    let off = sel.deactivate();
    assert!(off.current.is_none() && sel.active().is_none());
    eprintln!(
        "synthetic skin: {} mapped DIDs, missing-file case {}, panel override on {}",
        chosen.len(),
        if missing.is_some() {
            "exercised"
        } else {
            "not exercised"
        },
        element.1
    );
}

/// The element label with the lowest game-level key (at least 0x10000000).
fn lowest_game_label(ix: &LabelIndex) -> (u32, String) {
    ix.iter()
        .filter(|(k, l)| {
            *k >= 0x1000_0000 && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
        .min_by_key(|(k, _)| *k)
        .map(|(k, l)| (k, l.to_owned()))
        .expect("an element label")
}

/// The first ten items, in hexadecimal.
fn first<T: std::fmt::Debug>(v: &[T]) -> String {
    format!("{:x?}", &v[..v.len().min(10)])
}

/// SI5–SI9 over every UI layout record of the install.
#[test]
fn layout_side_invariants() {
    let Some(dir) = game_dir() else { return };
    let gd = GameData::open(&dir).expect("open install");
    let tables = SkinTables::load(&gd).expect("skin tables");
    let registry = gd.registry().unwrap().clone();
    let mut dids = BTreeSet::new();
    for m in gd.source().members() {
        for e in m.archive.entries_range(0x2200_0000..=0x22FF_FFFF) {
            dids.insert(e.expect("directory walk").did);
        }
    }
    let mut layouts: HashMap<Did, UiLayout> = HashMap::new();
    for did in dids {
        let payload = gd.read(Did(did)).unwrap().expect("listed record");
        // The decode census of every record is `tessera-formats`' ui_layout_real_data.
        if let Ok(l) = UiLayout::parse_expecting(&payload, did, registry.as_ref()) {
            layouts.insert(Did(did), l);
        }
    }
    assert!(!layouts.is_empty(), "no UI layout decoded");
    let cannot: Vec<u32> = [
        names::CANNOT_BE_MOVED,
        names::CANNOT_BE_RESIZED,
        names::CANNOT_BE_DETACHED,
    ]
    .iter()
    .filter_map(|n| registry.pid(n))
    .collect();
    let carries = |e: &tessera_formats::ui_layout::ElementDesc| {
        e.blocks()
            .any(|b| cannot.iter().any(|&p| b.properties.get(p).is_some()))
    };

    let mut images: HashSet<u32> = HashSet::new();
    let mut bases: Vec<BaseRef> = Vec::new();
    let mut si6 = Vec::new();
    let mut si7 = Vec::new();
    let mut si8 = Vec::new();
    let mut sorted: Vec<&UiLayout> = layouts.values().collect();
    sorted.sort_by_key(|l| l.did());
    for l in &sorted {
        l.walk(|e, _| {
            if tables.elements.label(e.element_id).is_none() {
                si6.push((l.did().0, e.element_id));
            }
            for b in e.blocks() {
                for &p in &cannot {
                    if b.properties
                        .get(p)
                        .is_some_and(|v| !matches!(v, PropertyValue::Bool(_)))
                    {
                        si7.push((l.did().0, e.element_id, p));
                    }
                }
            }
        });
        l.for_each_reference(|r| match r {
            LayoutRef::Media {
                kind: MediaKind::Image,
                did,
            } => {
                images.insert(did.0);
            }
            LayoutRef::Media {
                kind: kind @ (MediaKind::Animation | MediaKind::Cursor),
                did,
            } if tables.base.art_asset(did.0).is_some() => {
                si8.push((l.did().0, kind.name(), did.0));
            }
            LayoutRef::Base(b) => bases.push(b),
            _ => {}
        });
    }
    let si5: Vec<u32> = tables
        .base
        .iter()
        .map(|(d, _)| d)
        .filter(|d| !images.contains(d))
        .collect();
    let si9: Vec<BaseRef> = bases
        .iter()
        .copied()
        .filter(|b| {
            layouts
                .get(&b.layout)
                .and_then(|l| l.top_level(b.element))
                .is_some_and(carries)
        })
        .collect();
    eprintln!(
        "layouts: {}; Image DIDs: {}; base references: {}",
        layouts.len(),
        images.len(),
        bases.len()
    );
    assert!(
        si5.is_empty(),
        "SI5: {} base DIDs never used by an Image item: {}",
        si5.len(),
        first(&si5)
    );
    assert!(
        si6.is_empty(),
        "SI6: {} element IDs without a label: {}",
        si6.len(),
        first(&si6)
    );
    assert!(
        si7.is_empty(),
        "SI7: {} non-boolean protections: {}",
        si7.len(),
        first(&si7)
    );
    assert!(
        si8.is_empty(),
        "SI8: {} Animation/Cursor DIDs in the base table: {}",
        si8.len(),
        first(&si8)
    );
    assert!(
        si9.is_empty(),
        "SI9: {} protected elements used as bases: {}",
        si9.len(),
        first(&si9)
    );
}
