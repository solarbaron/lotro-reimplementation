// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-input` key-map importer (docs/specs/keymap.md §12): hostile bytes as the
//! original client's key-mapping file (lexer, node tree, the four-part reader, the merge of
//! §12.8 and the translation of §12.14), and as key-map (0x1D) and action-map (0x51)
//! record payloads. Nothing may panic or grow without bound. Checked on every run: the
//! imported document is a valid Tessera keymap (it serialises and reads back strictly to
//! the same keymap) and compiles against the built-in registry; a key map the reader
//! accepted, written back by the original's layout (§12.13), is accepted again (I6:
//! excluded cases only drop bindings or change activations, never reject the file).
//! Vocabulary and records are synthetic.

#![no_main]

use std::collections::HashMap;

use libfuzzer_sys::fuzz_target;
use tessera_input::import::{
    Device, Guid, ImportOptions, InstallData, KeyMap, KeyMapRecord, NameTable, Vocabulary,
    decode_action_map, decode_key_map_record, effective_key_map, import_key_map, parse_file,
    read_key_map, write_key_map,
};
use tessera_input::{InputMapper, Keymap, default_registry};

/// The standard device GUIDs of §12.5.2 (DirectInput system devices and the four pads).
const GUIDS: [&str; 8] = [
    "6F1D2B61-D5A0-11CF-BFC7-444553540000",
    "6F1D2B60-D5A0-11CF-BFC7-444553540000",
    "C8854010-4D36-445B-85ED-5DA62CBA27E4",
    "1E55E717-F1D3-47AF-8321-996C5CE0C8FD",
    "7C20E1A8-78B5-4D0A-83D3-E1D4ACF2B862",
    "57F7ADAD-9A51-4037-ABD1-E9C3F54C9CF9",
    "AD847413-69AD-4F20-B003-CC44E72AE90E",
    "0113552B-7D27-49E4-AFC7-8B47B54FB2B7",
];
const TYPES: [u8; 8] = [1, 2, 4, 3, 3, 3, 3, 3];

/// Invented IDs for names of §12.14.2 (no game data).
const COMMANDS: &[(&str, u32)] = &[
    ("DoNothing", 1),
    ("MovementForward", 10),
    ("MovementBackup", 12),
    ("MovementStrafeLeft", 13),
    ("MovementTurnOrStrafeLeft", 14),
    ("CameraReset", 15),
    ("CameraInstantMouseSteer", 18),
    ("SelectLeft", 20),
    ("SelectRight", 21),
    ("SelectDblRight", 22),
    ("TapLeft", 23),
    ("ScrollUp", 30),
    ("ScrollDown", 31),
    ("CursorNextPage", 32),
    ("CursorCharLeft", 33),
    ("CopyText", 34),
    ("EscapeKey", 35),
    ("PointerX", 36),
    ("MovementLongJump", 37),
    ("AcceptInput", 38),
    ("FocusNext", 39),
];
const MAPS: &[(&str, u32)] = &[
    ("DialogBoxes", 1),
    ("ScrollableControls_Mouse", 2),
    ("EditControls", 6),
    ("CopyAndPasteControls", 8),
    ("Main", 0xA),
];

fn vocabulary() -> Vocabulary {
    Vocabulary {
        commands: NameTable::new(COMMANDS.iter().copied()),
        input_maps: NameTable::new(MAPS.iter().copied()),
        toggle_types: Some(HashMap::from([(33, 3), (20, 1), (23, 2), (10, 6)])),
        standard_devices: GUIDS
            .iter()
            .zip(TYPES)
            .map(|(g, kind)| Device {
                kind,
                guid: Guid::parse(g.as_bytes()).expect("GUID"),
            })
            .collect(),
    }
}

fuzz_target!(|data: &[u8]| {
    let vocab = vocabulary();

    // Record payloads.
    let record = decode_key_map_record(data).ok();
    let _ = decode_action_map(data);

    // The user file through the reader.
    let mut reports = Vec::new();
    let file_map = parse_file(data, &mut reports)
        .ok()
        .and_then(|nodes| read_key_map(&nodes, &vocab, &mut reports).ok());
    if let Some(km) = &file_map {
        let written = write_key_map(km, &vocab);
        let mut r = Vec::new();
        let nodes = parse_file(&written, &mut r)
            .unwrap_or_else(|e| panic!("own layout does not parse: {e}"));
        let again = read_key_map(&nodes, &vocab, &mut r)
            .unwrap_or_else(|e| panic!("own layout is rejected: {e}"));
        assert!(again.binding_count() <= km.binding_count());
    }

    // The merge and the translation, with a fuzzed record as one of the defaults.
    let mut key_maps = Vec::new();
    if let Some(rec) = record {
        key_maps.push(rec);
    }
    key_maps.push(KeyMapRecord {
        did: 0x1D00_0001,
        key_map: KeyMap {
            devices: vocab.standard_devices.clone(),
            meta_keys: Vec::new(),
            sections: Vec::new(),
        },
    });
    let defaults: Vec<KeyMap> = key_maps.iter().map(|r| r.key_map.clone()).collect();
    let eff = effective_key_map(file_map.as_ref(), &defaults);
    let _ = write_key_map(&eff, &vocab);
    let install = InstallData {
        key_maps,
        action_map: None,
        vocabulary: vocab,
        problems: Vec::new(),
    };
    let options = ImportOptions {
        pad_deadzone: data.first().map(|b| f32::from(*b) / 200.0),
    };
    let (doc, report) = import_key_map(Some(data), &install, &options);
    assert!(report.warnings.len() <= tessera_input::MAX_KEYMAP_WARNINGS + 1);
    let text = doc
        .to_toml_string()
        .unwrap_or_else(|e| panic!("imported keymap does not serialise: {e}"));
    let back = Keymap::parse(&text).unwrap_or_else(|e| panic!("imported keymap: {e}\n{text}"));
    assert_eq!(back, doc, "imported keymap changed in a round trip");
    let (mut m, _) = InputMapper::new(default_registry(), &doc);
    m.update(std::time::Duration::ZERO);
});
