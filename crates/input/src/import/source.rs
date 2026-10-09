// SPDX-License-Identifier: GPL-3.0-or-later
//! Finding the importer's data in the install through the DID maps
//! (`keymap.md` §12.2). Records are reached by DID-map **key**, with the
//! label checked; nothing is hard-coded but the root map and the keys and
//! labels the original resolves (K21). Reading is done by the caller's
//! `fetch` closure, so this crate needs no archive access of its own.

use std::collections::{HashMap, HashSet};

use tessera_formats::{DataIdMap, Did, decode_did_map, decode_enum_mapper};

use super::model::{NameTable, Vocabulary};
use super::record::{ActionMap, KeyMapRecord, decode_action_map, decode_key_map_record};

/// The root DID map (`records.md` §8.2).
pub const ROOT_DID_MAP: u32 = 0x2800_0000;

/// Root keys and labels the original resolves (§12.2).
const ROOT_KEYMAP: (u32, &str) = (8, "KEYMAP");
const ROOT_ACTIONMAP: (u32, &str) = (6, "ACTIONMAP");
const ROOT_EMAPPER: (u32, &str) = (7, "EMAPPER");
/// KEYMAP keys in the order the original merges them (§12.2, §12.8).
const KEYMAP_KEYS: [(u32, &str); 4] = [
    (0x1000_0002, "gmPartner"),
    (0x1000_0005, "gmPCDefault"),
    (0x1000_0001, "gmDefault"),
    (0x2, "Default"),
];
/// ACTIONMAP key of the default action map.
const ACTIONMAP_DEFAULT: (u32, &str) = (1, "default");
/// EMAPPER keys of the command and input-map name chains.
const EMAPPER_INPUT_ACTIONS: (u32, &str) = (0x1B, "InputActions");
const EMAPPER_INPUT_MAP: (u32, &str) = (0x3E, "InputMap");
/// Longest enum-mapper base chain followed.
const MAX_CHAIN: usize = 16;

/// What the importer reads from the install.
#[derive(Debug, Clone, Default)]
pub struct InstallData {
    /// The default key maps, in merge order (§12.8 step 4); missing ones are
    /// left out.
    pub key_maps: Vec<KeyMapRecord>,
    /// The default action map (ACTIONMAP key 1), if found.
    pub action_map: Option<ActionMap>,
    /// Names, toggle types and standard devices.
    pub vocabulary: Vocabulary,
    /// Sources that were skipped, and why.
    pub problems: Vec<String>,
}

impl InstallData {
    /// The command IDs of toggle type 3, for
    /// [`InputMapper::set_repeat_commands`](crate::InputMapper::set_repeat_commands).
    #[must_use]
    pub fn auto_repeat_commands(&self) -> Option<Vec<u32>> {
        self.action_map
            .as_ref()
            .map(ActionMap::auto_repeat_commands)
    }

    /// `(name, id)` of every InputActions command, for
    /// [`InputMapper::set_command_ids`](crate::InputMapper::set_command_ids).
    #[must_use]
    pub fn command_ids(&self) -> Vec<(String, u32)> {
        self.vocabulary
            .commands
            .iter()
            .map(|(id, n)| (n.to_owned(), id))
            .collect()
    }
}

/// Read the key-map and action-map records and the name chains through
/// `fetch` (DID → record bytes; `None` when absent). Every missing record,
/// undecodable record or label mismatch is noted in
/// [`InstallData::problems`] and that source is skipped.
pub fn load_install_data(mut fetch: impl FnMut(u32) -> Option<Vec<u8>>) -> InstallData {
    let mut out = InstallData::default();
    let mut map = |did: u32, problems: &mut Vec<String>| -> Option<DataIdMap> {
        let Some(bytes) = fetch(did) else {
            problems.push(format!("DID map {did:#010x} is missing"));
            return None;
        };
        match decode_did_map(&bytes) {
            Ok(m) => Some(m),
            Err(e) => {
                problems.push(format!("DID map {did:#010x}: {e}"));
                None
            }
        }
    };
    let Some(root) = map(ROOT_DID_MAP, &mut out.problems) else {
        return out;
    };
    let entry =
        |m: &DataIdMap, (key, label): (u32, &str), problems: &mut Vec<String>| match m.get(key) {
            Some(e) if &*e.label == label && !e.value.is_null() => Some(e.value.0),
            Some(e) => {
                problems.push(format!(
                    "DID map {:#010x} key {key:#x} has label {:?}, expected {label:?}",
                    m.did.0, &*e.label
                ));
                None
            }
            None => {
                problems.push(format!("DID map {:#010x} has no key {key:#x}", m.did.0));
                None
            }
        };
    let keymap =
        entry(&root, ROOT_KEYMAP, &mut out.problems).and_then(|d| map(d, &mut out.problems));
    let actionmap =
        entry(&root, ROOT_ACTIONMAP, &mut out.problems).and_then(|d| map(d, &mut out.problems));
    let emapper =
        entry(&root, ROOT_EMAPPER, &mut out.problems).and_then(|d| map(d, &mut out.problems));

    // Name chains first: nothing else needs them, but they come cheap.
    if let Some(em) = &emapper {
        if let Some(d) = entry(em, EMAPPER_INPUT_ACTIONS, &mut out.problems) {
            out.vocabulary.commands = chain(d, &mut fetch, &mut out.problems);
        }
        if let Some(d) = entry(em, EMAPPER_INPUT_MAP, &mut out.problems) {
            out.vocabulary.input_maps = chain(d, &mut fetch, &mut out.problems);
        }
    }
    if let Some(km) = &keymap {
        for k in KEYMAP_KEYS {
            let Some(did) = entry(km, k, &mut out.problems) else {
                continue;
            };
            match fetch(did).map(|b| decode_key_map_record(&b)) {
                Some(Ok(rec)) => out.key_maps.push(rec),
                Some(Err(e)) => out.problems.push(format!("key map {did:#010x}: {e}")),
                None => out
                    .problems
                    .push(format!("key map {did:#010x} ({}) is missing", k.1)),
            }
        }
    }
    if let Some(am) = &actionmap
        && let Some(did) = entry(am, ACTIONMAP_DEFAULT, &mut out.problems)
    {
        match fetch(did).map(|b| decode_action_map(&b)) {
            Some(Ok(m)) => out.action_map = Some(m),
            Some(Err(e)) => out.problems.push(format!("action map {did:#010x}: {e}")),
            None => out
                .problems
                .push(format!("action map {did:#010x} is missing")),
        }
    }
    out.vocabulary.toggle_types = out.action_map.as_ref().map(|m| {
        m.toggle_types
            .iter()
            .copied()
            .collect::<HashMap<u32, u32>>()
    });
    // The standard devices: positions 0-7 of the records' device lists.
    if let Some(rec) = out.key_maps.iter().find(|r| r.key_map.devices.len() >= 8) {
        out.vocabulary.standard_devices = rec.key_map.devices[..8].to_vec();
    } else if !out.key_maps.is_empty() {
        out.problems
            .push("no key-map record lists the eight standard devices".into());
    }
    out
}

/// Every name of an enum-mapper chain (the mapper, then its bases; a name
/// already seen for an ID is kept).
fn chain(
    start: u32,
    fetch: &mut impl FnMut(u32) -> Option<Vec<u8>>,
    problems: &mut Vec<String>,
) -> NameTable {
    let mut pairs: Vec<(String, u32)> = Vec::new();
    let mut seen = HashSet::new();
    let mut did = Did(start);
    for _ in 0..MAX_CHAIN {
        if did.is_null() || !seen.insert(did) {
            break;
        }
        let Some(bytes) = fetch(did.0) else {
            problems.push(format!("enum mapper {:#010x} is missing", did.0));
            break;
        };
        match decode_enum_mapper(&bytes) {
            Ok(m) => {
                pairs.extend(m.names().iter().map(|(k, n)| (n.clone(), *k)));
                did = m.base;
            }
            Err(e) => {
                problems.push(format!("enum mapper {:#010x}: {e}", did.0));
                break;
            }
        }
    }
    NameTable::new(pairs)
}
