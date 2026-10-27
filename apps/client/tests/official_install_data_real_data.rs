// SPDX-License-Identifier: GPL-3.0-or-later
//! Real data (`TESSERA_GAME_DIR`, read-only; skipped when it is unset): the official
//! connector's install readers (`tessera_client::official::install_data`). The `0xCC`
//! state schemas built from the install match the verification tables of
//! `official-messages-world.md` §4.3.1; the chat category enum has the categories
//! `official-messages-play.md` §6.7 names; references resolve through the string tables;
//! template baselines read (`property-system.md` §6.2). Counts only are printed.

use std::path::{Path, PathBuf};

use tessera_client::official::install_data::{
    CHAT_CATEGORY_ENUM, MAX_SCHEMA_NODES, install_data_at, portal_state_schema_build,
    portal_state_schemas,
};
use tessera_compat::connector::InstallData;
use tessera_compat::world::{DEFAULT_PORTAL_STATE_SCHEMA_ID, LocalizedTokenText};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::{EnumMapper, Record as _, StringSource};
use tessera_props::{PROPERTY_DICTIONARY_DID, PropertyRegistry};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TESSERA_GAME_DIR")?;
    Some(PathBuf::from(dir))
}

fn language(root: &Path) -> String {
    tessera_compat::login::resolve_install_language(root, None).expect("a language archive")
}

/// The verification facts of `official-messages-world.md` §4.3.1 (test only; the code
/// builds the schemas from the install).
const DEFAULT_BOOLS: [u32; 8] = [
    0x0000_068A,
    0x1000_3EA0,
    0x1000_6676,
    0x1000_4157,
    0x1000_0ED8,
    0x1000_1A27,
    0x1000_0ED7,
    0x1000_0443,
];
const DEFAULT_ENUMS: [u32; 6] = [
    0x0000_0126,
    0x0000_0201,
    0x0000_0687,
    0x1000_136E,
    0x1000_00FC,
    0x1000_0469,
];

#[test]
fn the_state_schemas_from_the_install_match_the_verification_tables() {
    let Some(root) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR unset: skipped");
        return;
    };
    let data = install_data_at(&root, &language(&root)).expect("the install opens");
    let schemas = data.schemas();
    eprintln!("state schemas built: {}", schemas.len());
    let default = data
        .portal_state_schema(DEFAULT_PORTAL_STATE_SCHEMA_ID)
        .expect("the default schema");
    assert_eq!(default.bool_pids, DEFAULT_BOOLS);
    assert_eq!(default.enum_pids, DEFAULT_ENUMS);
    for id in [0x1000_0E50u32, 0x1000_0E51] {
        let s = data.portal_state_schema(id).expect("a mounted schema");
        assert!(s.bool_pids.is_empty() && s.enum_pids.is_empty(), "{id:#x}");
    }
    // The reference install gives five: the three of the verification table and two more
    // script keys of `0x07000029` that are keys of the script-key enum. §4.3.1 lists only
    // three, so the others are not named here; whether every such key is a
    // `state_schema_id` is the W §4.3.1 question in `docs/m8-remediation.md` §15. A change
    // in this count means that question needs a new look. (Until 2026-10-27 one of the two
    // was left out at the per-key traversal bound, because a scriptlet was walked once per
    // path that named it; it is entered once per schema now, and the count went from four
    // to five.)
    assert_eq!(schemas.len(), 5);
    let unlisted: Vec<(usize, usize)> = schemas
        .iter()
        .filter(|(k, _)| ![DEFAULT_PORTAL_STATE_SCHEMA_ID, 0x1000_0E50, 0x1000_0E51].contains(k))
        .map(|(_, s)| (s.bool_pids.len(), s.enum_pids.len()))
        .collect();
    eprintln!("schemas outside the verification table (booleans, enums): {unlisted:?}");
    assert_eq!(unlisted.len(), 2);
}

#[test]
fn no_state_schema_is_left_out_at_a_traversal_bound() {
    let Some(root) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR unset: skipped");
        return;
    };
    let set = ArchiveSet::open(
        &root,
        SetOptions {
            language: Some(language(&root)),
            ..SetOptions::default()
        },
    )
    .expect("archives");
    let dict = set.read(PROPERTY_DICTIONARY_DID.0).expect("the dictionary");
    let registry = PropertyRegistry::parse(&dict).expect("decodes");
    let build = portal_state_schema_build(&set, &registry);
    eprintln!(
        "state schemas built: {}, left out: {}, nodes visited: {}",
        build.schemas.len(),
        build.left_out.len(),
        build.visited
    );
    // Each scriptlet is entered once per schema: every key stays far below its bound.
    assert!(build.left_out.is_empty(), "{:x?}", build.left_out);
    assert!(build.visited < MAX_SCHEMA_NODES, "{}", build.visited);
}

#[test]
fn chat_categories_texts_and_templates_read_from_the_install() {
    let Some(root) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR unset: skipped");
        return;
    };
    let lang = language(&root);
    let data = install_data_at(&root, &lang).expect("the install opens");
    let keys = data.chat_category_keys().expect("the chat category enum");
    eprintln!("chat category keys: {}", keys.len());
    for k in [5u32, 6, 0x1C, 0x1D, 0x1E, 0x1F, 0x27, 0x28, 0x29, 0x2A] {
        assert!(keys.contains(&k), "category {k:#x}");
    }

    // The enum's own display texts are string-table references: they resolve.
    let set = ArchiveSet::open(
        &root,
        SetOptions {
            language: Some(lang),
            ..SetOptions::default()
        },
    )
    .expect("archives");
    let bytes = set.read(CHAT_CATEGORY_ENUM).expect("the enum record");
    let mapper = EnumMapper::parse_expecting(&bytes, CHAT_CATEGORY_ENUM).expect("decodes");
    let mut resolved = 0usize;
    let mut references = 0usize;
    for (_, info) in mapper.displays() {
        if let StringSource::Table { token, table } = &info.source {
            references += 1;
            if data
                .resolve_text(&LocalizedTokenText::reference(*token, table.0))
                .is_some_and(|t| !t.is_empty())
            {
                resolved += 1;
            }
        }
    }
    eprintln!("chat category display references: {references}, resolved: {resolved}");
    assert!(references > 0 && resolved > 0);
    assert!(
        data.resolve_text(&LocalizedTokenText::reference(0, 0))
            .is_none()
    );
    // A reference into a record that is not a string table is refused unread
    // (`official-messages-world.md` §2.6 row 2b, `string-table.md` §9.1); a literal that
    // shows nothing is no text.
    assert!(
        data.resolve_text(&LocalizedTokenText::reference(1, CHAT_CATEGORY_ENUM))
            .is_none()
    );
    assert!(data.text_cached(CHAT_CATEGORY_ENUM));
    assert!(
        data.resolve_text(&LocalizedTokenText::literal("[hidden]"))
            .is_none()
    );
    // A table read once is cached afterwards.
    let (_, first) = mapper
        .displays()
        .iter()
        .find_map(|(k, info)| match &info.source {
            StringSource::Table { table, .. } => Some((*k, table.0)),
            StringSource::Literal(_) => None,
        })
        .expect("a reference");
    assert!(data.text_cached(first));

    // Template baselines: the first object property sets of the install give values.
    let dict = set.read(PROPERTY_DICTIONARY_DID.0).expect("dictionary");
    let registry = PropertyRegistry::parse(&dict).expect("decodes");
    let mut checked = 0usize;
    let mut with_value = 0usize;
    'members: for m in set.members() {
        for entry in m.archive.entries_range(0x7900_0000..0x7A00_0000) {
            let Ok(entry) = entry else { continue };
            let object = entry.did - 0x0900_0000;
            let Ok(bytes) = set.read(entry.did) else {
                continue;
            };
            let Ok(rec) = tessera_formats::property_set::PropertySetRecord::parse_expecting(
                &bytes,
                entry.did,
                registry.dictionary(),
            ) else {
                continue;
            };
            checked += 1;
            if rec
                .set
                .iter()
                .any(|e| data.template_value(object, e.pid).is_some())
            {
                with_value += 1;
            }
            if checked >= 64 {
                break 'members;
            }
        }
    }
    eprintln!("template sets checked: {checked}, with a readable value: {with_value}");
    assert!(checked > 0 && with_value > 0);
    // The builder is deterministic: built again, the same schemas.
    assert_eq!(&portal_state_schemas(&set, &registry), data.schemas());
}
