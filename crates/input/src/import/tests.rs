// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the importer (`keymap.md` §12.17 I6, I7, I9, §12.18).
//! Command and input-map IDs here are invented; the standard device GUIDs
//! are the values §12.5.2 lists for synthetic tests.

use std::collections::HashMap;

use proptest::prelude::*;

use super::model::{
    Control, Device, Guid, KeyBinding, KeyMap, MetaKey, NameTable, Section, Vocabulary,
};
use super::record::KeyMapRecord;
use super::{ImportOptions, InstallData, import_key_map, parse_file, read_key_map, write_key_map};
use crate::chord::{Activation, Chord};
use crate::device::AnalogAxis;
use crate::keymap::BindingSpec;

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

const COMMANDS: &[(&str, u32)] = &[
    ("DoNothing", 1),
    ("MovementForward", 10),
    ("MovementForwardCameraMovement", 11),
    ("MovementBackup", 12),
    ("MovementStrafeLeft", 13),
    ("MovementTurnOrStrafeLeft", 14),
    ("CameraReset", 15),
    ("CameraInstantMoveToward", 16),
    ("CameraInstantMoveAway", 17),
    ("CameraInstantMouseSteer", 18),
    ("SelectLeft", 20),
    ("SelectRight", 21),
    ("SelectDblRight", 22),
    ("TapLeft", 23),
    ("TapRight", 24),
    ("ScrollUp", 30),
    ("ScrollDown", 31),
    ("CursorNextPage", 32),
    ("CursorCharLeft", 33),
    ("CopyText", 34),
    ("EscapeKey", 35),
    ("PointerX", 36),
    ("MovementLongJump", 37),
    ("SELECTION_NEXT_FOE", 40),
];
const MAPS: &[(&str, u32)] = &[
    ("DialogBoxes", 1),
    ("ScrollableControls_Mouse", 2),
    ("EditControls", 6),
    ("CopyAndPasteControls", 8),
    ("Main", 0xA),
];
const MAIN: u32 = 0xA;

fn standard() -> Vec<Device> {
    GUIDS
        .iter()
        .zip(TYPES)
        .map(|(g, kind)| Device {
            kind,
            guid: Guid::parse(g.as_bytes()).unwrap(),
        })
        .collect()
}

fn vocab() -> Vocabulary {
    Vocabulary {
        commands: NameTable::new(COMMANDS.iter().copied()),
        input_maps: NameTable::new(MAPS.iter().copied()),
        toggle_types: Some(HashMap::from([(33, 3), (20, 1), (23, 2)])),
        standard_devices: standard(),
    }
}

fn kc(code: u16) -> Control {
    Control {
        device: 0,
        sub: 0,
        code,
    }
}

fn mc(code: u16) -> Control {
    Control {
        device: 1,
        sub: 0,
        code,
    }
}

/// The meta keys of the `gm` records (§12.5.3).
fn default_meta() -> Vec<MetaKey> {
    let mut m = Vec::new();
    for (bit, a, b) in [
        (1, 0x2A, 0x36),
        (2, 0x1D, 0x9D),
        (3, 0x38, 0xB8),
        (4, 0xDB, 0xDC),
    ] {
        m.push(MetaKey {
            control: kc(a),
            bit,
        });
        m.push(MetaKey {
            control: kc(b),
            bit,
        });
    }
    for (bit, code) in [(5, 0x0F), (6, 0x10), (7, 0x0C), (8, 0x0D)] {
        m.push(MetaKey {
            control: mc(code),
            bit,
        });
    }
    m
}

fn record(sections: Vec<Section>) -> KeyMapRecord {
    KeyMapRecord {
        did: 0x1D00_0000,
        key_map: KeyMap {
            devices: standard(),
            meta_keys: default_meta(),
            sections,
        },
    }
}

fn data(sections: Vec<Section>) -> InstallData {
    InstallData {
        key_maps: vec![record(sections)],
        action_map: None,
        vocabulary: vocab(),
        problems: Vec::new(),
    }
}

fn bind(command: u32, control: Control, mask: u32, activation: u32) -> KeyBinding {
    KeyBinding {
        command,
        control,
        mask,
        activation,
        user: false,
    }
}

fn file(sections: &str) -> Vec<u8> {
    format!(
        "\"x\" [ 00000000-0000-0000-0000-000000000000 ]\nDevices [ ]\nMetaKeys [ ]\nBindings [ {sections} ]\n"
    )
    .into_bytes()
}

fn read(bytes: &[u8]) -> Result<KeyMap, String> {
    let mut r = Vec::new();
    let nodes = parse_file(bytes, &mut r).map_err(|e| e.to_string())?;
    read_key_map(&nodes, &vocab(), &mut r)
}

fn entries(doc: &crate::keymap::Keymap, c: &str, a: &str) -> Vec<String> {
    doc.get(c, a)
        .unwrap_or_else(|| panic!("{c}.{a} missing"))
        .iter()
        .map(|b| match b {
            BindingSpec::Digital { chord, value } => {
                let mut s = chord.to_string();
                if *value != 1.0 {
                    s += &format!(" {value}");
                }
                if chord.activation() != Activation::Press {
                    s += &format!(" on={}", chord.activation());
                }
                s
            }
            BindingSpec::Analog {
                axis,
                scale,
                deadzone,
            } => {
                format!("{axis} {scale} {deadzone}")
            }
        })
        .collect()
}

// ----- §12.18 examples --------------------------------------------------------

#[test]
fn example_c_one_binding() {
    let f = file(
        "Main [ MovementForwardCameraMovement [ User [ 1 DIMOFS_BUTTON0 ] 0x00000080 ] \
         CameraInstantMoveToward [ Old [ 1 DIMOFS_Z AxisPositive ] ] ]",
    );
    let km = read(&f).unwrap();
    let b = km.sections[0].bindings[0];
    assert_eq!(b.control.value(), 0x000C_0001);
    assert_eq!(b.mask, 0x80);
    assert_eq!(b.mask.reverse_bits(), 0x0100_0000);
    assert_eq!(b.activation, 3);
    assert!(b.user);
    // Import: the user binding over a record holding the zoom binding.
    let d = data(vec![Section {
        input_map: MAIN,
        bindings: vec![bind(
            16,
            Control {
                device: 1,
                sub: 1,
                code: 8,
            },
            0,
            3,
        )],
    }]);
    let (doc, report) = import_key_map(Some(&f), &d, &ImportOptions::default());
    assert!(report.file_loaded, "{report:?}");
    assert_eq!(
        entries(&doc, "movement", "move_forward"),
        ["MouseLeft+MouseRight", "PadLeftY 1 0.15"]
    );
    assert_eq!(entries(&doc, "camera", "zoom"), ["WheelY 1 0"]);
    assert_eq!(
        doc.entry_command("movement", "move_forward", 0),
        Some("MovementForwardCameraMovement")
    );
}

#[test]
fn example_d_merge_and_import() {
    let (w, a, q, home) = (0x11, 0x1E, 0x10, 0xC7);
    let d = data(vec![Section {
        input_map: MAIN,
        bindings: vec![
            bind(10, kc(w), 0, 3),
            bind(14, kc(a), 0, 3),
            bind(13, kc(q), 0, 3),
        ],
    }]);
    let f =
        file("Main [ MovementStrafeLeft [ User [ 0 DIK_A ] ] CameraReset [ Old [ 0 DIK_HOME ] ] ]");
    let _ = home;
    let (doc, report) = import_key_map(Some(&f), &d, &ImportOptions::default());
    assert!(report.file_loaded);
    assert_eq!(
        entries(&doc, "movement", "move_forward"),
        ["W", "PadLeftY 1 0.15"]
    );
    assert_eq!(
        entries(&doc, "movement", "strafe"),
        ["A -1", "Q -1", "PadLeftX 1 0.15"]
    );
    assert!(entries(&doc, "movement", "turn").is_empty());
    assert!(entries(&doc, "camera", "reset_camera").is_empty());
}

#[test]
fn example_f_writer_spellings() {
    let v = vocab();
    let mut km = KeyMap {
        devices: standard(),
        meta_keys: vec![],
        sections: vec![Section {
            input_map: MAIN,
            bindings: vec![
                bind(23, mc(0x0C), 0, 4),
                bind(23, mc(0x0D), 0x6, 3),
                bind(23, mc(0x0E), 0, 7),
                bind(23, mc(0x0F), 0, 0),
                bind(
                    23,
                    Control {
                        device: 4,
                        sub: 0,
                        code: 0x30,
                    },
                    0,
                    3,
                ),
                bind(
                    23,
                    Control {
                        device: 4,
                        sub: 0,
                        code: 0x31,
                    },
                    0,
                    3,
                ),
                bind(
                    23,
                    Control {
                        device: 2,
                        sub: 0,
                        code: 0x01,
                    },
                    0,
                    3,
                ),
            ],
        }],
    };
    let text = String::from_utf8(write_key_map(&km, &v)).unwrap();
    for want in [
        "TapLeft [ Old [ 1 DIMOFS_BUTTON0 ] 0x00000000 Tap ]\r\n",
        "TapLeft [ Old [ 1 DIMOFS_BUTTON1 ] 0x00000006 ]\r\n",
        "TapLeft [ Old [ 1 DIMOFS_BUTTON2 ] 0x00000000 Click/Tap ]\r\n",
        "TapLeft [ Old [ 1 DIMOFS_BUTTON3 ] 0x00000000 \"\" ]\r\n",
        "[ 4 \"DIJOFS_BUTTON(0)\" ]",
        "[ 4 DIJOFS_BUTTON1 ]",
        "[ 2 \"\" ]",
        "Keyboard [ GUID_SysKeyboard ]",
    ] {
        assert!(text.contains(want), "{want:?} in\n{text}");
    }
    // Reading it back drops only the virtual-device binding (§12.13).
    let back = read(text.as_bytes()).unwrap();
    km.sections[0].bindings.pop();
    assert_eq!(back.sections, km.sections);
}

#[test]
fn example_h_structure() {
    let empty = read(b"[ GUID_Virtual ] Devices [ ] MetaKeys [ ] Bindings [ ]").unwrap();
    assert_eq!(empty.devices, standard());
    let empty2 = read(b"\"\" [ GUID_Virtual ] Devices MetaKeys Bindings").unwrap();
    assert_eq!(empty2, empty);
    let km = read(&file("Main [ [ User [ 0 DIK_A ] ] ]")).unwrap();
    assert!(km.sections[0].bindings.is_empty(), "empty command: dropped");
    let km = read(&file("Main [ CameraReset [ [ 0 DIK_HOME ] ] ]")).unwrap();
    assert!(!km.sections[0].bindings[0].user, "empty origin: non-user");
    let km = read(&file(
        r#"Main [ CameraReset [ User [ "" DIK_HOME ] 0x00000000 "" ] ]"#,
    ))
    .unwrap();
    assert_eq!(km.sections[0].bindings[0].activation, 0);
    assert_eq!(km.sections[0].bindings[0].control.device, 0);
    assert!(
        read(b"\"\" [ GUID_Virtual ] Devices [ ] MetaKeys [ [ 0 DIK_LSHIFT ] ] Bindings [ ]")
            .is_err()
    );
    let km = read(&file(
        "Main [ CameraReset [ User [ 0 DIK_HOME ( x ) ] 0x00000000 [ y ] Down [ z ] ] ]",
    ))
    .unwrap();
    let b = km.sections[0].bindings[0];
    assert_eq!((b.control.code, b.mask, b.activation), (0xC7, 0, 1));
    let km = read(&file("Main [ CameraReset [ User [ x DIK_HOME ] ] ]")).unwrap();
    assert!(km.sections[0].bindings.is_empty());
    let km = read(&file("Main [ !foo [ User [ 0 DIK_A ] ] ]")).unwrap();
    assert!(km.sections[0].bindings.is_empty());
    let mut with_warning = file("");
    with_warning.extend(b" \"!WARNING\" [ note ]");
    assert!(read(&with_warning).is_ok());
}

// ----- I7 -------------------------------------------------------------------

#[test]
fn i7_rejected_files() {
    let good = file("Main [ CameraReset [ User [ 0 DIK_HOME ] ] ]");
    assert!(read(&good).is_ok());
    let mut five = good.clone();
    five.extend(b" Extra");
    let mut bom = b"\xEF\xBB\xBF".to_vec();
    bom.extend(&good);
    let mut ff = good.clone();
    ff.extend(b" \xFF");
    let mut aa = vec![0xAA; 4];
    aa.extend(&good);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("five nodes", five),
        ("byte-order mark", bom),
        (
            "mismatched",
            file("Main [ CameraReset [ User [ 0 DIK_HOME ) ] ]"),
        ),
        ("unknown section", file("Nowhere [ ]")),
        ("repeated section", file("Main [ ] main [ ]")),
        (
            "unknown activation",
            file("Main [ CameraReset [ User [ 0 DIK_HOME ] 0 Sideways ] ]"),
        ),
        (
            "empty activation word",
            file("Main [ CameraReset [ User [ 0 DIK_HOME ] 0 Up//Tap ] ]"),
        ),
        (
            "mask not a number",
            file("Main [ CameraReset [ User [ 0 DIK_HOME ] Up ] ]"),
        ),
        ("0xFF", ff),
        (
            "empty device",
            b"\"\" [ GUID_Virtual ] Devices [ [ GUID_Virtual ] ] MetaKeys Bindings".to_vec(),
        ),
        ("empty section", file("[ ]")),
        ("!error", file("Main [ ] !error")),
        ("!ERROR in a list", file("Main [ \"!Error\" [ m ] ]")),
        ("0xAA", aa),
        (
            "meta key device beyond the list",
            b"\"\" [ GUID_Virtual ] Devices MetaKeys [ 1 [ 9 DIK_LSHIFT ] ] Bindings".to_vec(),
        ),
        (
            "meta key with an unknown control",
            b"\"\" [ GUID_Virtual ] Devices MetaKeys [ 1 [ 0 DIK_NOPE ] ] Bindings".to_vec(),
        ),
        ("!include", file("!include [ other.keymap ] Main [ ]")),
    ];
    for (what, bytes) in cases {
        assert!(read(&bytes).is_err(), "{what} should be rejected");
    }
    let kept: Vec<(&str, Vec<u8>)> = vec![
        ("unknown command", file("Main [ Fly [ User [ 0 DIK_A ] ] ]")),
        (
            "unknown control",
            file("Main [ CameraReset [ User [ 0 DIK_NOPE ] ] ]"),
        ),
        ("empty binding", file("Main [ [ User [ 0 DIK_A ] ] ]")),
        (
            "device index not a number",
            file("Main [ CameraReset [ User [ x DIK_A ] ] ]"),
        ),
        (
            "device index 256",
            file("Main [ CameraReset [ User [ 256 DIK_A ] ] ]"),
        ),
        (
            "unknown directive",
            file("Main [ !foo [ User [ 0 DIK_A ] ] ]"),
        ),
        (
            "four parts",
            file("Main [ CameraReset [ User [ 0 DIK_A ] ] ]"),
        ),
    ];
    for (what, bytes) in kept {
        let km = read(&bytes).unwrap_or_else(|e| panic!("{what}: {e}"));
        let n = km.binding_count();
        assert!(n <= 1, "{what}");
        if what != "four parts" {
            assert_eq!(n, 0, "{what}: the binding is dropped");
        }
    }
    // Toggle type 3 needs a down-type activation (§12.7).
    let km = read(&file(
        "EditControls [ CursorCharLeft [ User [ 0 DIK_LEFT ] 0 Up ] ]",
    ))
    .unwrap();
    assert_eq!(km.binding_count(), 0);
    let km = read(&file(
        "EditControls [ CursorCharLeft [ User [ 0 DIK_LEFT ] 0 DblClickDown ] ]",
    ))
    .unwrap();
    assert_eq!(km.binding_count(), 1);
    // Numeric names of existing IDs are accepted.
    let km = read(&file("10 [ 15 [ User [ 0 DIK_HOME ] ] ]")).unwrap();
    assert_eq!(
        (km.sections[0].input_map, km.sections[0].bindings[0].command),
        (10, 15)
    );
    // Conflicting bindings in one file: the first is kept.
    let km = read(&file(
        "Main [ CameraReset [ User [ 0 DIK_HOME ] ] EscapeKey [ User [ 0 DIK_HOME ] 0 Up ] ]",
    ))
    .unwrap();
    assert_eq!(km.sections[0].bindings.len(), 1);
    assert_eq!(km.sections[0].bindings[0].command, 15);
}

// ----- I9 -------------------------------------------------------------------

#[test]
fn i9_activations_wheel_and_copy() {
    let pt = |code| mc(code);
    let wheel = |sub| Control {
        device: 1,
        sub,
        code: 8,
    };
    let d = data(vec![
        Section {
            input_map: MAIN,
            bindings: vec![
                bind(23, pt(0x0C), 0, 0x04),    // TapLeft
                bind(22, pt(0x0D), 0, 0x60),    // SelectDblRight
                bind(20, pt(0x0E), 0, 0x06),    // SelectLeft Up/Tap
                bind(21, pt(0x0D), 0, 0x10),    // SelectRight DblClickUp only
                bind(24, pt(0x0E), 0x80, 0x04), // TapRight with bit 8
                bind(10, kc(0x11), 0, 0x06),    // MovementForward Up/Tap
            ],
        },
        Section {
            input_map: 2,
            bindings: vec![bind(30, wheel(1), 0, 3), bind(31, wheel(2), 0x1, 3)],
        },
        Section {
            input_map: 6,
            bindings: vec![bind(32, kc(0xD1), 0, 3)],
        },
    ]);
    let (doc, report) = import_key_map(None, &d, &ImportOptions::default());
    assert_eq!(entries(&doc, "pointer", "tap_left"), ["MouseLeft on=tap"]);
    assert_eq!(
        entries(&doc, "pointer", "select_double_right"),
        ["MouseRight on=double_nearby"]
    );
    assert_eq!(
        entries(&doc, "pointer", "select_left"),
        ["MouseMiddle on=release"]
    );
    assert!(
        doc.get("pointer", "select_right").is_none(),
        "0x10 only: dropped, left out"
    );
    assert!(doc.get("pointer", "tap_right").is_none());
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("activation on a chord")),
        "{report:?}"
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("activation on an axis action"))
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("without a Tessera form"))
    );
    assert_eq!(entries(&doc, "ui.scroll_mouse", "scroll_up"), ["WheelUp"]);
    assert_eq!(
        entries(&doc, "ui.scroll_mouse", "scroll_down"),
        ["Shift+WheelDown"]
    );
    assert_eq!(entries(&doc, "text_entry", "text_page_down"), ["PageDown"]);
    assert_eq!(entries(&doc, "ui.chat_log", "text_page_down"), ["PageDown"]);
    assert_eq!(report.imported, 6, "the copy counts once: {report:?}");
    assert_eq!(report.dropped, 3);
    // A wheel sub-control with 0x06 on a button action.
    let d = data(vec![Section {
        input_map: 2,
        bindings: vec![bind(30, wheel(1), 0, 6)],
    }]);
    let (_, report) = import_key_map(None, &d, &ImportOptions::default());
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("activation on a wheel click"))
    );
}

#[test]
fn rejected_file_imports_the_defaults() {
    let d = data(vec![Section {
        input_map: MAIN,
        bindings: vec![bind(37, kc(0x39), 0, 3)],
    }]);
    let (doc, report) = import_key_map(Some(b"broken ]"), &d, &ImportOptions::default());
    assert!(report.rejected.is_some() && !report.file_loaded);
    assert_eq!(entries(&doc, "movement", "jump"), ["Space", "PadSouth"]);
    let big = vec![b' '; super::MAX_SOURCE_SIZE as usize + 1];
    let (_, report) = import_key_map(Some(&big), &d, &ImportOptions::default());
    assert!(report.rejected.unwrap().contains("larger"));
    // An imported document compiles cleanly against the built-in registry.
    let (_, w) = crate::InputMapper::new(
        crate::default_registry(),
        &crate::default_keymap().overlay(&doc),
    );
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn pad_and_meta_translation() {
    let xin = |code, sub| Control {
        device: 4,
        sub,
        code,
    };
    let d = data(vec![Section {
        input_map: MAIN,
        bindings: vec![
            bind(37, xin(0x30, 0), 0, 3), // jump on A
            bind(37, xin(0x3A, 0), 0, 3), // jump on button 10 → PadMode, warning
            bind(37, xin(0x3B, 0), 0, 3), // button 11 → dropped
            bind(37, kc(0x39), 0x2, 3),   // Ctrl+Space
            bind(18, mc(0x0D), 0, 3),     // mouselook
            bind(36, mc(0x00), 0, 3),     // PointerX
        ],
    }]);
    let (doc, report) = import_key_map(
        None,
        &d,
        &ImportOptions {
            pad_deadzone: Some(0.25),
        },
    );
    assert_eq!(
        entries(&doc, "movement", "jump"),
        ["Ctrl+Space", "PadSouth", "PadMode"]
    );
    assert!(report.warnings.iter().any(|w| w.contains("PadMode")));
    assert_eq!(entries(&doc, "camera", "mouselook"), ["MouseRight"]);
    assert_eq!(entries(&doc, "movement", "mouselook"), ["MouseRight"]);
    // No pad binding of PointerX: Tessera's default pad entry stays.
    assert_eq!(
        entries(&doc, "camera", "look_x"),
        ["MouseX 1 0", "PadRightX 1 0.15"]
    );
    let _ = AnalogAxis::MouseX;
    let _ = Chord::parse("A").unwrap();
}

// ----- I6 -------------------------------------------------------------------

fn arb_binding() -> impl Strategy<Value = KeyBinding> {
    let control = prop_oneof![
        (0..3u16).prop_map(|i| kc([0x01u16, 0x39, 0xC7][usize::from(i)])),
        (0x0Cu16..0x14).prop_map(mc),
        (0u16..0x50).prop_map(|c| Control {
            device: 4,
            sub: 0,
            code: c
        }),
        (0u8..3).prop_map(|s| Control {
            device: 1,
            sub: s,
            code: 8
        }),
    ];
    (
        0..COMMANDS.len(),
        control,
        any::<u32>(),
        prop_oneof![
            Just(0u32),
            Just(3),
            Just(4),
            Just(6),
            Just(0x60),
            0u32..0x80
        ],
        any::<bool>(),
    )
        .prop_filter("joystick codes need a name", |(_, c, ..)| {
            c.device != 4 || super::names::control_name(3, c.code).is_some()
        })
        .prop_map(|(i, control, mask, activation, user)| KeyBinding {
            command: COMMANDS[i].1,
            control,
            mask,
            activation,
            user,
        })
}

proptest! {
    /// I6: writing a key map and reading it back gives the same sections,
    /// bindings and meta keys (controls named in their device's table,
    /// activations within 0x7F, no toggle-type drops).
    #[test]
    fn i6_write_read_round_trip(
        bindings in proptest::collection::vec(arb_binding(), 0..30),
        map in 0..MAPS.len(),
    ) {
        let mut v = vocab();
        v.toggle_types = None;
        let mut section = Section { input_map: MAPS[map].1, bindings: Vec::new() };
        for b in bindings {
            section.add(b);
        }
        let km = KeyMap { devices: standard(), meta_keys: default_meta(), sections: vec![section] };
        let text = write_key_map(&km, &v);
        let mut r = Vec::new();
        let nodes = parse_file(&text, &mut r).unwrap();
        let back = read_key_map(&nodes, &v, &mut r).unwrap();
        prop_assert_eq!(back, km);
    }

    /// Hostile files never panic, and whatever is accepted imports into a
    /// document that compiles.
    #[test]
    fn hostile_files(bytes in proptest::collection::vec(any::<u8>(), 0..400)) {
        let d = data(vec![]);
        let (doc, _) = import_key_map(Some(&bytes), &d, &ImportOptions::default());
        let (_, w) = crate::InputMapper::new(crate::default_registry(), &crate::default_keymap().overlay(&doc));
        prop_assert!(w.is_empty());
    }
}

proptest! {
    /// `Section::extend` gives exactly what adding one by one gives, also
    /// over a starting list that already holds conflicting bindings.
    #[test]
    fn extend_matches_add(
        start in proptest::collection::vec((0u16..3, 0u32..2, 0u32..8, 0u32..4), 0..12),
        items in proptest::collection::vec((0u16..3, 0u32..2, 0u32..8, 0u32..4), 0..40),
    ) {
        let b = |(code, mask, activation, command): (u16, u32, u32, u32)| {
            bind(command, kc(code), mask, activation)
        };
        let start: Vec<KeyBinding> = start.into_iter().map(b).collect();
        let mut one = Section { input_map: MAIN, bindings: start.clone() };
        let mut bulk = one.clone();
        for &i in &items {
            one.add(b(i));
        }
        bulk.extend(items.into_iter().map(b));
        prop_assert_eq!(bulk, one);
    }
}

/// The `Main [ ... ]` file of `count` bindings of command ID 10 written as a
/// number, on W with distinct masks (a hostile file near the 1 MiB limit).
fn many_bindings_file(count: usize) -> Vec<u8> {
    let mut s = String::from("Main [ ");
    for i in 1..=count {
        s.push_str(&format!("10[User[0 DIK_W]{i}]"));
    }
    s.push(']');
    file(&s)
}

#[test]
fn hostile_file_with_many_bindings_imports_in_bounded_time() {
    let bytes = many_bindings_file(40_000);
    assert!(bytes.len() as u64 <= super::MAX_SOURCE_SIZE);
    let d = data(vec![Section {
        input_map: MAIN,
        bindings: vec![bind(10, kc(0x11), 0, 3)],
    }]);
    let started = std::time::Instant::now();
    let mut reports = Vec::new();
    let nodes = parse_file(&bytes, &mut reports).unwrap();
    let km = read_key_map(&nodes, &vocab(), &mut reports).unwrap();
    assert_eq!(km.binding_count(), 40_000);
    // One "numeric command name" report per binding, capped.
    assert_eq!(reports.len(), super::text::MAX_REPORTS);
    let (doc, report) = import_key_map(Some(&bytes), &d, &ImportOptions::default());
    let took = started.elapsed();
    assert!(report.file_loaded, "{report:?}");
    assert!(report.warnings.len() <= crate::keymap::MAX_KEYMAP_WARNINGS + 1);
    // Masks of bits 1-8 translate (modifiers, mouse chords): cut to the limit.
    assert_eq!(
        doc.get("movement", "move_forward").unwrap().len(),
        crate::keymap::MAX_BINDINGS_PER_ACTION
    );
    // Quadratic merging took several seconds even in a release build; the
    // budget is generous for a loaded machine and a debug build (about 1 s
    // here, 25 s before).
    assert!(took.as_secs() < 10, "import took {took:?}");
}

#[test]
fn many_meta_keys_and_masked_bindings_import_in_bounded_time() {
    let mut meta = default_meta();
    meta.retain(|m| m.bit != 1);
    for code in 0..20_000u16 {
        meta.push(MetaKey {
            control: Control {
                device: 0,
                sub: 0,
                code: 0x1000 + code,
            },
            bit: 1,
        });
    }
    let mut d = data(vec![Section {
        input_map: MAIN,
        bindings: (0..17_000u32)
            .map(|i| bind(10, kc(0x11), 1 | (i << 1), 3))
            .collect(),
    }]);
    d.key_maps[0].key_map.meta_keys = meta;
    let started = std::time::Instant::now();
    let (_, report) = import_key_map(None, &d, &ImportOptions::default());
    let took = started.elapsed();
    assert_eq!(report.dropped, 17_000, "bit 1 has no Tessera form");
    assert!(report.warnings.iter().any(|w| w.contains("meta-key bit 1")));
    assert!(took.as_secs() < 10, "import took {took:?}");
}

#[test]
fn reader_reports_are_capped_and_clipped() {
    let long = "Q".repeat(3000);
    let mut s = String::from("Main [ ");
    for _ in 0..(super::text::MAX_REPORTS + 50) {
        s.push_str(&format!("{long} [ User [ 0 DIK_W ] ] "));
    }
    s.push(']');
    let mut reports = Vec::new();
    let nodes = parse_file(&file(&s), &mut reports).unwrap();
    read_key_map(&nodes, &vocab(), &mut reports).unwrap();
    assert_eq!(reports.len(), super::text::MAX_REPORTS);
    assert!(reports.iter().all(|r| r.len() < 200), "{:?}", reports[0]);
}

#[test]
fn incomplete_install_data_imports_nothing() {
    let mut d = InstallData::default();
    d.problems.push("no key-map record found".into());
    let (doc, report) = import_key_map(None, &d, &ImportOptions::default());
    assert_eq!(doc, crate::keymap::Keymap::new(), "Tessera's defaults stay");
    let why = report.install_incomplete.as_deref().unwrap();
    assert!(why.contains("key-map record") && why.contains("command names"));
    assert_eq!(report.warnings[0], "install data: no key-map record found");
    // Names but no records, and records but no names.
    let mut d = data(vec![]);
    d.key_maps.clear();
    let (doc, report) = import_key_map(None, &d, &ImportOptions::default());
    assert!(doc.iter().next().is_none());
    assert_eq!(
        report.install_incomplete.as_deref(),
        Some("no default key-map record")
    );
    let mut d = data(vec![]);
    d.vocabulary.input_maps = NameTable::default();
    let (_, report) = import_key_map(Some(&file("")), &d, &ImportOptions::default());
    assert_eq!(
        report.install_incomplete.as_deref(),
        Some("no input-map names")
    );
    // Complete data: no flag.
    let (_, report) = import_key_map(None, &data(vec![]), &ImportOptions::default());
    assert_eq!(report.install_incomplete, None);
}

#[cfg(unix)]
#[test]
fn source_fifo_is_refused_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("keys.cfg");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .is_ok_and(|s| s.success());
    if !made {
        eprintln!("mkfifo unavailable; skipping");
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let path = fifo.clone();
    std::thread::spawn(move || {
        // The open used after the type check, on the FIFO itself (the case
        // of a FIFO swapped in between): it must not block.
        let opened =
            crate::keymap::open_nonblocking(&path).map(|f| f.metadata().map(|m| m.is_file()));
        let read = super::read_source_file(&path)
            .map(|_| ())
            .map_err(|e| e.to_string());
        let _ = tx.send((opened.is_ok_and(|m| m.is_ok_and(|f| !f)), read));
    });
    let (opened_not_file, read) = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("blocked on a FIFO");
    assert!(opened_not_file);
    assert!(read.unwrap_err().contains("not a regular file"));
}
