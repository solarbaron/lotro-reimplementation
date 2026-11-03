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

/// The value of `result`. Its error is not printed: it can name a path of the install.
#[track_caller]
fn must<T, E>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("{what}"),
    }
}

/// The archives of `root`, read-only, for the language `lang`.
fn open_set(root: &Path, lang: String) -> ArchiveSet {
    must(
        ArchiveSet::open(
            root,
            SetOptions {
                language: Some(lang),
                ..SetOptions::default()
            },
        ),
        "the archives do not open",
    )
}

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TESSERA_GAME_DIR")?;
    Some(PathBuf::from(dir))
}

fn language(root: &Path) -> String {
    must(
        tessera_compat::login::resolve_install_language(root, None),
        "no language archive",
    )
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
    let data = must(
        install_data_at(&root, &language(&root)),
        "the install does not open",
    );
    let schemas = data.schemas();
    eprintln!("state schemas built: {}", schemas.len());
    let default = data
        .portal_state_schema(DEFAULT_PORTAL_STATE_SCHEMA_ID)
        .expect("the default schema");
    assert!(
        default.bool_pids == DEFAULT_BOOLS,
        "the default schema's booleans differ"
    );
    assert!(
        default.enum_pids == DEFAULT_ENUMS,
        "the default schema's enums differ"
    );
    for id in [0x1000_0E50u32, 0x1000_0E51] {
        let s = data.portal_state_schema(id).expect("a mounted schema");
        assert!(s.bool_pids.is_empty() && s.enum_pids.is_empty(), "{id:#x}");
    }
    // §4.3.1 step 3: only the scripts whose node graph has a `PlayerPhysics` node (kind 47)
    // are schemas, the three of the verification table. (Until pass 9 every script key of
    // `0x07000029` that is a key of the script-key enum was one, five on the reference
    // install.)
    assert_eq!(schemas.len(), 3);
    let unlisted = schemas
        .keys()
        .filter(|k| ![DEFAULT_PORTAL_STATE_SCHEMA_ID, 0x1000_0E50, 0x1000_0E51].contains(k))
        .count();
    assert_eq!(unlisted, 0, "schemas outside the verification table");
}

#[test]
fn no_state_schema_is_left_out_at_a_traversal_bound() {
    let Some(root) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR unset: skipped");
        return;
    };
    let set = open_set(&root, language(&root));
    let dict = must(set.read(PROPERTY_DICTIONARY_DID.0), "no dictionary");
    let registry = must(
        PropertyRegistry::parse(&dict),
        "the dictionary does not decode",
    );
    let build = portal_state_schema_build(&set, &registry);
    eprintln!(
        "state schemas built: {}, left out: {}, nodes visited: {}",
        build.schemas.len(),
        build.left_out.len(),
        build.visited
    );
    // Each scriptlet is entered once per schema: every key stays far below its bound.
    assert_eq!(build.left_out.len(), 0, "schemas left out at a bound");
    // `Startup` (`0x05`) and `ActiveMovement` (`0x0D`) reach no `PlayerPhysics` node
    // (§4.3.1 step 3): lifecycle scripts, not schemas.
    eprintln!(
        "script keys without a PlayerPhysics node: {}",
        build.not_physics.len()
    );
    for key in [0x0000_0005u32, 0x0000_000D] {
        assert!(build.not_physics.contains(&key), "{key:#x}");
    }
    assert!(build.visited < MAX_SCHEMA_NODES, "{}", build.visited);
}

#[test]
fn chat_categories_texts_and_templates_read_from_the_install() {
    let Some(root) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR unset: skipped");
        return;
    };
    let lang = language(&root);
    let data = must(install_data_at(&root, &lang), "the install does not open");
    let keys = data.chat_category_keys().expect("the chat category enum");
    eprintln!("chat category keys: {}", keys.len());
    // Every number `official-messages-play.md` §6.7 names.
    for k in [
        5u32, 6, 7, 11, 12, 13, 19, 23, 27, 38, 0x1C, 0x1D, 0x1E, 0x1F, 0x27, 0x28, 0x29, 0x2A,
    ] {
        assert!(keys.contains(&k), "category {k:#x}");
    }

    // The enum's own display texts are string-table references: they resolve.
    let set = open_set(&root, lang);
    let bytes = must(set.read(CHAT_CATEGORY_ENUM), "no enum record");
    let mapper = must(
        EnumMapper::parse_expecting(&bytes, CHAT_CATEGORY_ENUM),
        "the enum does not decode",
    );
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
    let dict = must(set.read(PROPERTY_DICTIONARY_DID.0), "no dictionary");
    let registry = must(
        PropertyRegistry::parse(&dict),
        "the dictionary does not decode",
    );
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
    assert!(
        &portal_state_schemas(&set, &registry) == data.schemas(),
        "the schemas built twice differ"
    );
}

/// The initial exchange's inputs (`official-session.md` §9.3.2, §9.3.7, decision 0046): built
/// from the install on this thread, checked against the rules without printing a value.
#[test]
fn the_advertisement_is_built_from_the_install_and_follows_the_rules() {
    use tessera_client::official::install_data::{advertised_archive, initial_inputs};
    use tessera_compat::connector::data::prepare_advertisement;
    use tessera_compat::initial::{
        advertisement_order, build_install_advertisement, fold_language_code,
    };
    use tessera_dat::ArchiveKind;
    use tessera_install::DID_ITERATION;

    let Some(root) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR unset: skipped");
        return;
    };
    let lang = language(&root);
    let set = open_set(&root, lang.clone());
    let inputs = initial_inputs(&set);

    // The full key's fold is the selected language archive's subset.
    let key = inputs.language_key.expect("the language key");
    assert!(key != 0, "the language key is zero");
    let selected = set
        .members()
        .iter()
        .find(|m| {
            m.id.part == 0
                && matches!(&m.id.kind, ArchiveKind::Local(l) if l.eq_ignore_ascii_case(&lang))
        })
        .expect("the language's base");
    assert!(
        selected.archive.header().subset == fold_language_code(key),
        "the selected language archive's subset is not the fold of the key"
    );

    // Some installed name rule allows it; every rule's list is read.
    let rules = inputs.name_rules.as_ref().expect("the name rules");
    assert!(rules.iter().any(|(_, languages)| languages.contains(&key)));
    eprintln!("name rules: {}", rules.len());

    // One entry per mounted base of the policy, each with a unique identifier, and within the
    // §9.3.7 caps; the iteration metadata keeps invariant 7 (N may be zero, with K zero).
    let inventory = inputs.inventory.as_ref().expect("the inventory");
    let bases = set
        .members()
        .iter()
        .filter(|m| {
            m.id.part == 0
                && match &m.id.kind {
                    ArchiveKind::Local(l) => l.eq_ignore_ascii_case(&lang),
                    _ => true,
                }
        })
        .count();
    assert_eq!(inventory.len(), bases);
    eprintln!("advertised archives: {}", inventory.len());
    let mut ids: Vec<[u8; 8]> = inventory.iter().map(|a| a.wire_identifier()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), inventory.len(), "unique identifiers");
    assert!(inventory.len() <= tessera_compat::initial::MAX_ADVERTISED_ARCHIVES);
    for archive in inventory {
        assert!(
            archive.check_invariant().is_ok(),
            "an entry breaks invariant 7"
        );
    }

    // Each entry's iteration description, written by the §9.3.7 serializer rule, decodes to the
    // base's stored record, and is that record when it was stored canonically; the entries come
    // in decision 0046's order.
    let data = build_install_advertisement(key, inventory).expect("builds");
    assert!(data.len() <= tessera_compat::initial::MAX_ADVERTISEMENT_BYTES);
    let mut rewritten = 0usize;
    let mut same_bytes = 0usize;
    for member in set.members().iter().filter(|m| m.id.part == 0) {
        let entry = must(member.archive.lookup(DID_ITERATION), "the lookup failed");
        let Some(entry) = entry else { continue };
        let raw = must(
            member.archive.read_entry(&entry),
            "the record does not read",
        );
        let mut id = [0u8; 8];
        id[..4].copy_from_slice(&member.archive.header().dataset.to_le_bytes());
        id[4..].copy_from_slice(&member.archive.header().subset.to_le_bytes());
        let Some(listed) = inventory.iter().find(|a| a.resource_id == id) else {
            continue;
        };
        assert!(
            &advertised_archive(id, &raw, entry.stamp).expect("parses") == listed,
            "the listed entry differs from its base's record"
        );
        let single = build_install_advertisement(key, std::slice::from_ref(listed)).expect("one");
        let description = &single[21..single.len() - 8];
        assert!(
            &advertised_archive(id, description, 2).expect("what was written parses") == listed,
            "the rewritten iteration description differs from the stored record"
        );
        if description == raw.as_slice() {
            same_bytes += 1;
        }
        rewritten += 1;
    }
    assert_eq!(rewritten, inventory.len());
    eprintln!("iteration descriptions byte-identical to the record: {same_bytes} / {rewritten}");
    let mut at = 13usize;
    let mut last = None;
    for _ in 0..inventory.len() {
        let wire: [u8; 8] = data[at..at + 8].try_into().expect("eight");
        let order = advertisement_order(wire);
        assert!(last.is_none_or(|l| l < order), "ascending remainder");
        last = Some(order);
        let listed = inventory
            .iter()
            .find(|a| a.wire_identifier() == wire)
            .expect("listed");
        let single = build_install_advertisement(key, std::slice::from_ref(listed)).expect("one");
        at += single.len() - 21;
    }
    assert_eq!(at + 8, data.len(), "M and the terminal field end the data");
    assert!(
        data[data.len() - 8..] == [0u8; 8],
        "M and the terminal field are not zero"
    );

    // The connector's preparation gets the same data through the trait.
    let archive_data = must(install_data_at(&root, &lang), "the install does not open");
    let prepared = prepare_advertisement(&archive_data).expect("prepared");
    assert!(
        prepared.language_key == key,
        "the connector's preparation has another language key"
    );
    assert!(
        prepared.data == data,
        "the connector's preparation has other advertisement data"
    );
    eprintln!("advertisement bytes: {}", prepared.data.len());
}
