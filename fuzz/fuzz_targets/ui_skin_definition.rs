// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui::skin` (`skin-definition.md` §4, §6.4, §8.3–§8.4; D4, D5, D8): hostile
//! `SkinDefinition.xml` bytes go through the strict XML reader and the definition parser
//! with a synthetic name resolver (a name resolves, to a value derived from its bytes,
//! unless its hash says otherwise; one label resolves to 0). When the definition is
//! accepted, every override is looked up again through the ID paths of its own tree and
//! through paths that do not exist, applied to a synthetic descriptor tree mirroring it,
//! and every mapping's `FileName` is joined to a skin folder, with and without a Wine
//! prefix (lexically: no file-system call is made, drive letters included). The cache key
//! must be the lower-cased path and must not depend on the letter case of `FileName`. The
//! raw input also goes through `parse_int`. Nothing may panic; parser invariants are
//! checked.
//!
//! Input layout: one option byte (bit 0: also prefix a UTF-16 byte-order mark and widen),
//! then the document bytes.

#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use tessera_formats::ui_layout::{ElementDesc, Rect};
use tessera_ui::skin::{
    Definition, ElementOverride, GeometryPids, NameResolver, apply_override, parse_definition,
    parse_int, skin_file,
};

struct Resolver;

fn hash(name: &str) -> u32 {
    name.bytes()
        .map(|b| b.to_ascii_lowercase())
        .fold(0x811C_9DC5u32, |h, b| {
            (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
        })
}

impl NameResolver for Resolver {
    fn art_asset(&self, name: &str) -> Option<u32> {
        let h = hash(name);
        (name.is_ascii() && !h.is_multiple_of(5)).then_some(h & 0x0FFF_FFFF)
    }
    fn element(&self, name: &str) -> Option<u32> {
        if name.eq_ignore_ascii_case("zero") {
            return Some(0);
        }
        let h = hash(name);
        (name.is_ascii() && !h.is_multiple_of(7)).then_some(h | 0x1000_0000)
    }
}

const PIDS: GeometryPids = GeometryPids {
    cannot_be_moved: Some(1),
    cannot_be_resized: Some(2),
    cannot_be_detached: Some(3),
    scaler_origin_x: Some(4),
    scaler_origin_y: Some(5),
};

fn mirror(ov: &ElementOverride, depth: usize) -> ElementDesc {
    let mut d = ElementDesc {
        element_id: ov.element_id,
        ..ElementDesc::default()
    };
    d.block.rect = Rect {
        x: 1,
        y: 2,
        width: 3,
        height: 4,
    };
    if depth < 70 {
        for c in ov.children() {
            d.children.push(mirror(c, depth + 1));
        }
    }
    d
}

fn check_tree(def: &Definition, path: &mut Vec<u32>, ov: &ElementOverride) {
    assert_ne!(ov.element_id, 0, "a kept override has an ID");
    assert!(
        (ov.width != 0 && ov.height != 0) || !ov.children().is_empty(),
        "a kept override has a size or a child"
    );
    assert!(def.uses_element(ov.element_id));
    let found = def
        .find_override(path, ov.element_id)
        .expect("own path matches");
    assert_eq!(found.element_id, ov.element_id);
    let mut ids: Vec<u32> = ov.children().iter().map(|c| c.element_id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), ov.children().len(), "sibling IDs are unique");
    path.push(ov.element_id);
    for c in ov.children() {
        check_tree(def, path, c);
    }
    path.pop();
}

fuzz_target!(|data: &[u8]| {
    let Some((&option, doc)) = data.split_first() else {
        return;
    };
    let _ = parse_int(&String::from_utf8_lossy(doc));
    let widened: Vec<u8>;
    let bytes = if option & 1 == 1 {
        widened = [0xFF, 0xFE]
            .into_iter()
            .chain(doc.iter().flat_map(|&b| [b, 0]))
            .collect();
        &widened[..]
    } else {
        doc
    };
    let Ok(def) = parse_definition(bytes, &Resolver) else {
        return;
    };
    assert!(!def.mappings().is_empty() || !def.panels().is_empty());
    let mut arts: Vec<u32> = def.mappings().iter().map(|m| m.art_asset).collect();
    arts.sort_unstable();
    arts.dedup();
    assert_eq!(
        arts.len(),
        def.mappings().len(),
        "art-asset values are unique"
    );
    for m in def.mappings() {
        assert!(!m.file_name.is_empty());
        assert_eq!(def.mapping(m.art_asset), Some(m));
        let _ = skin_file(Path::new("/skins/s"), &m.file_name, None);
        let prefix = Some(Path::new("/prefix"));
        let f = skin_file(Path::new("/skins/s"), &m.file_name, prefix);
        let g = skin_file(
            Path::new("/skins/s"),
            &m.file_name.to_ascii_uppercase(),
            prefix,
        );
        if let (Ok(f), Ok(g)) = (&f, &g) {
            let lower = f
                .path()
                .to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase();
            assert_eq!(f.cache_key(), lower);
            assert_eq!(
                f.cache_key(),
                g.cache_key(),
                "key independent of letter case"
            );
        } else {
            assert_eq!(f.is_ok(), g.is_ok());
        }
    }
    let mut top = Vec::new();
    for p in def.panels() {
        for e in p.elements() {
            top.push(e.element_id);
            let mut path = Vec::new();
            check_tree(&def, &mut path, e);
            let mut desc = mirror(e, 0);
            assert!(apply_override(&mut desc, e, &PIDS));
            assert_eq!((desc.block.rect.x, desc.block.rect.y), (e.x, e.y));
            assert!(
                def.find_override(&[e.element_id, 0], e.element_id)
                    .is_none()
            );
        }
    }
    let n = top.len();
    top.sort_unstable();
    top.dedup();
    assert_eq!(top.len(), n, "top-level IDs are unique across panels (D8)");
});
