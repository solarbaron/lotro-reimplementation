// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the key-map importer (`keymap.md` §12.17 I2–I5, I10)
//! and of the mapper fed with the install's command IDs and toggle types
//! (§2.2, §7.8.6). Skipped unless `TESSERA_GAME_DIR` is set.
//!
//! Archives are opened read-only through `tessera-dat`; nothing is written
//! anywhere (I8). Counts are observations of one install: they are printed,
//! and the tests fail only on structural differences (a record that no
//! longer parses, a missing source, a broken invariant of the format).

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_input::import::names::control_name;
use tessera_input::import::{
    ImportOptions, InstallData, KeyMap, effective_key_map, import_key_map, load_install_data,
};
use tessera_input::{
    Activation, BandRequests, BindingSpec, InputMapper, KeyCode, Keymap, OfferPhase, OfferedEvent,
    RawEvent, UiActions, Verdict, contexts, default_keymap, default_registry,
};

fn game_dir() -> Option<PathBuf> {
    std::env::var_os("TESSERA_GAME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

/// The install's importer data, or `None` (skipped) without an install.
fn install() -> Option<InstallData> {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return None;
    };
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open the archive set");
    let data = load_install_data(|did| set.read(did).ok());
    for p in &data.problems {
        eprintln!("importer source problem: {p}");
    }
    assert!(
        !data.key_maps.is_empty(),
        "no default key map could be read: {:?}",
        data.problems
    );
    assert!(
        !data.vocabulary.commands.is_empty(),
        "no InputActions names"
    );
    assert!(!data.vocabulary.input_maps.is_empty(), "no InputMap names");
    Some(data)
}

/// I3: every record parsed (the decoder refuses trailing bytes); devices,
/// controls, commands, sections, sub-controls and activations are as stated.
#[test]
fn i3_key_map_records() {
    let Some(data) = install() else { return };
    let vocab = &data.vocabulary;
    assert_eq!(vocab.standard_devices.len(), 8, "eight standard devices");
    let mut activations = HashSet::new();
    for rec in &data.key_maps {
        let km = &rec.key_map;
        eprintln!(
            "I3 {:#010x}: {} devices, {} meta keys, {} sections, {} bindings",
            rec.did,
            km.devices.len(),
            km.meta_keys.len(),
            km.sections.len(),
            km.binding_count()
        );
        assert_eq!(
            km.devices, vocab.standard_devices,
            "{:#010x}: device list is not the eight standard devices",
            rec.did
        );
        for mk in &km.meta_keys {
            assert!(
                (1..=32).contains(&mk.bit),
                "{:#010x}: meta-key bit {}",
                rec.did,
                mk.bit
            );
        }
        for s in &km.sections {
            assert!(
                vocab.input_maps.name(s.input_map).is_some(),
                "{:#010x}: input map {:#x} not in the InputMap chain",
                rec.did,
                s.input_map
            );
            for b in &s.bindings {
                assert!(
                    vocab.commands.name(b.command).is_some(),
                    "{:#010x}: command {:#x} not in the InputActions chain",
                    rec.did,
                    b.command
                );
                let dev = km
                    .devices
                    .get(usize::from(b.control.device))
                    .unwrap_or_else(|| panic!("{:#010x}: device index {b:?}", rec.did));
                assert!(
                    control_name(dev.kind, b.control.code).is_some(),
                    "{:#010x}: control {:#x} has no name for device type {}",
                    rec.did,
                    b.control.code,
                    dev.kind
                );
                assert!(b.control.sub <= 2, "{:#010x}: sub-control {b:?}", rec.did);
                activations.insert(b.activation);
            }
        }
    }
    let mut seen: Vec<u32> = activations.into_iter().collect();
    seen.sort_unstable();
    eprintln!("I3 activations used: {seen:#04x?}");
    for a in seen {
        assert!(
            [0x03, 0x04, 0x06, 0x60].contains(&a),
            "record activation {a:#04x} outside 0x03, 0x04, 0x06, 0x60"
        );
    }
}

/// I2: the effective key map from the records alone (§12.8).
#[test]
fn i2_effective_key_map() {
    let Some(data) = install() else { return };
    let defaults: Vec<KeyMap> = data.key_maps.iter().map(|r| r.key_map.clone()).collect();
    let total: usize = defaults.iter().map(KeyMap::binding_count).sum();
    let eff = effective_key_map(None, &defaults);
    let dropped = total - eff.binding_count().min(total);
    let names: Vec<String> = eff
        .sections
        .iter()
        .map(|s| {
            format!(
                "{} {}",
                data.vocabulary.input_maps.name(s.input_map).unwrap_or("?"),
                s.bindings.len()
            )
        })
        .collect();
    eprintln!(
        "I2: {} sections, {} bindings from {total} record bindings ({dropped} dropped), {} meta keys",
        eff.sections.len(),
        eff.binding_count(),
        eff.meta_keys.len()
    );
    eprintln!("I2 sections: {}", names.join(", "));
    if (
        eff.sections.len(),
        eff.binding_count(),
        dropped,
        eff.meta_keys.len(),
    ) != (12, 254, 2, 12)
    {
        eprintln!("I2: differs from the observation of keymap.md §12.8 (12, 254, 2, 12)");
    }
    assert!(eff.binding_count() <= total);
    assert!(!eff.sections.is_empty());
    // Section order: first seen, record by record.
    let mut expected = Vec::new();
    for km in &defaults {
        for s in &km.sections {
            if !expected.contains(&s.input_map) {
                expected.push(s.input_map);
            }
        }
    }
    let got: Vec<u32> = eff.sections.iter().map(|s| s.input_map).collect();
    assert_eq!(got, expected, "sections in the order first seen");
    // No two bindings of one effective section conflict.
    for s in &eff.sections {
        for (i, a) in s.bindings.iter().enumerate() {
            for b in &s.bindings[i + 1..] {
                assert!(!a.conflicts(b), "conflicting bindings kept: {a:?} {b:?}");
            }
        }
    }
}

/// I4: the default action map.
#[test]
fn i4_action_map() {
    let Some(data) = install() else { return };
    let am = data
        .action_map
        .as_ref()
        .expect("default action map (ACTIONMAP key 1)");
    let a: HashSet<u32> = am.toggle_types.iter().map(|(c, _)| *c).collect();
    let b: HashSet<u32> = am.classes.iter().map(|c| c.command).collect();
    eprintln!(
        "I4 {:#010x}: A {}, B {}, C {}",
        am.did,
        am.toggle_types.len(),
        am.classes.len(),
        am.tokens.len()
    );
    assert_eq!(a, b, "tables A and B have the same command set");
    for (c, t) in &am.toggle_types {
        assert!((1..=5).contains(t), "command {c:#x} toggle type {t}");
    }
    for c in &am.classes {
        assert!((3..=5).contains(&c.class), "{c:?}");
    }
    let repeat = am.auto_repeat_commands();
    eprintln!("I4: {} commands of toggle type 3", repeat.len());
    assert!(!repeat.is_empty());
}

/// I5: the InputActions names.
#[test]
fn i5_command_names() {
    let Some(data) = install() else { return };
    let n = data.vocabulary.commands.len();
    let ids: HashSet<u32> = data.vocabulary.commands.iter().map(|(id, _)| id).collect();
    eprintln!(
        "I5: {n} InputActions names ({} distinct IDs), {} InputMap names",
        ids.len(),
        data.vocabulary.input_maps.len()
    );
    if n != 471 {
        eprintln!("I5: differs from the observation of keymap.md §12.11 (471)");
    }
    assert_eq!(ids.len(), n, "distinct IDs");
}

/// Text of every entry, for comparing lists.
fn texts(list: Option<&[BindingSpec]>) -> Vec<String> {
    list.unwrap_or_default()
        .iter()
        .map(|b| match b {
            BindingSpec::Digital { chord, value } => {
                let mut s = chord.to_string();
                if chord.activation() != Activation::Press {
                    s += &format!(" on={}", chord.activation());
                }
                if *value != 1.0 {
                    s += &format!(" value={value}");
                }
                s
            }
            BindingSpec::Analog {
                axis,
                scale,
                deadzone,
            } => format!("{axis} scale={scale} deadzone={deadzone}"),
        })
        .collect()
}

/// I10: importing the records alone gives the §8 defaults of the pointer
/// and UI contexts (with the stated differences). Differences are reported;
/// only a rejected import or a missing context fails.
#[test]
fn i10_records_alone_give_the_ui_defaults() {
    let Some(data) = install() else { return };
    let (doc, report) = import_key_map(None, &data, &ImportOptions::default());
    assert!(report.rejected.is_none(), "{report:?}");
    assert!(report.install_incomplete.is_none(), "{report:?}");
    eprintln!(
        "I10: {} imported, {} dropped, {} warnings",
        report.imported,
        report.dropped,
        report.warnings.len()
    );
    for w in &report.warnings {
        eprintln!("  warning: {w}");
    }
    // The document reads back strictly (it is a valid keymap file).
    let text = doc.to_toml_string().expect("serialise the imported keymap");
    let back = Keymap::parse(&text).expect("strict read of the imported keymap");
    assert_eq!(texts_all(&back), texts_all(&doc));

    let defaults = default_keymap();
    let tessera_choice = |s: &String| s.starts_with("Ctrl+Shift+");
    let mut differences = 0;
    for ctx in [
        contexts::POINTER,
        contexts::UI_SCROLL_MOUSE,
        contexts::UI_DIALOG,
        contexts::TEXT_ENTRY,
        contexts::UI_CHAT_LOG,
        contexts::UI_CLIPBOARD,
    ] {
        let actions: Vec<&str> = defaults
            .iter()
            .filter(|(c, _, _)| *c == ctx)
            .map(|(_, a, _)| a)
            .collect();
        assert!(!actions.is_empty(), "{ctx} has defaults");
        for action in actions {
            let mut want = texts(defaults.get(ctx, action));
            if ctx == contexts::TEXT_ENTRY || ctx == contexts::UI_CHAT_LOG {
                want.retain(|s| !tessera_choice(s));
            }
            if ctx == contexts::UI_CLIPBOARD {
                let key = want
                    .first()
                    .and_then(|s| s.strip_prefix("Ctrl+"))
                    .unwrap_or_default()
                    .to_owned();
                want = vec![format!("Ctrl+{key}"), format!("Super+{key}")];
            }
            let got = texts(doc.get(ctx, action));
            if got != want {
                differences += 1;
                eprintln!("I10 difference: {ctx}.{action}: imported {got:?}, expected {want:?}");
            }
        }
    }
    eprintln!("I10: {differences} differences");
}

/// I11 (`keymap.md` §8.3, K50): importing the records alone gives the panel and use actions
/// the original's keys, and Tessera's defaults equal them except for the six bag toggles
/// (§8.3 Decision 1: unbound in Tessera, Ctrl + F1 … F6 in the original).
#[test]
fn i11_records_give_the_panel_and_use_keys() {
    let Some(data) = install() else { return };
    let (doc, report) = import_key_map(None, &data, &ImportOptions::default());
    assert!(report.rejected.is_none(), "{report:?}");
    let defaults = default_keymap();
    let mut rows: Vec<(String, Vec<String>)> = [
        ("toggle_character", "C"),
        ("toggle_bags", "I"),
        ("pin_tooltip", "H"),
        ("toggle_wallet", "B"),
        ("toggle_quest_log", "L"),
        ("use_selection", "U"),
        ("toggle_traits", "J"),
        ("toggle_skills", "K"),
        ("toggle_crafting", "T"),
    ]
    .into_iter()
    .map(|(a, k)| (a.to_owned(), vec![k.to_owned()]))
    .collect();
    for n in 1..=3 {
        rows.push((format!("toggle_crafting_{n}"), Vec::new()));
    }
    for (action, want) in &rows {
        let got = texts(doc.get(contexts::GLOBAL, action));
        assert_eq!(&got, want, "imported {action}");
        assert_eq!(
            &texts(defaults.get(contexts::GLOBAL, action)),
            want,
            "default {action}"
        );
    }
    for n in 1..=6 {
        let action = format!("toggle_bag_{n}");
        let got = texts(doc.get(contexts::GLOBAL, &action));
        assert_eq!(got, [format!("Ctrl+F{n}")], "imported {action}");
        assert!(
            texts(defaults.get(contexts::GLOBAL, &action)).is_empty(),
            "default {action}"
        );
    }
    eprintln!("I11: {} panel and use actions as K50", rows.len() + 6);
}

fn texts_all(k: &Keymap) -> Vec<(String, String, Vec<String>)> {
    k.iter()
        .map(|(c, a, l)| (c.to_owned(), a.to_owned(), texts(Some(l))))
        .collect()
}

/// §2.2 and §7.8.6 with the install's data: offered events carry the
/// command IDs read from the install, and OS repeats follow toggle type 3.
#[test]
fn mapper_with_install_command_data() {
    let Some(data) = install() else { return };
    let registry = default_registry();
    let ui = UiActions::new(&registry).expect("built-in UI ids");
    let (mut m, _) = InputMapper::new(registry, &default_keymap());
    m.set_command_ids(data.command_ids());
    m.set_repeat_commands(data.auto_repeat_commands());
    m.activate(ui.text_entry);

    let mut log: Vec<(OfferPhase, String, Option<u32>)> = Vec::new();
    let steps = [
        // Left: CursorCharLeft, toggle type 3 → a Repeat. Home:
        // CursorStartOfLine, type 2 → no Repeat (K43).
        (KeyCode::Left, true, false),
        (KeyCode::Left, true, true),
        (KeyCode::Left, false, false),
        (KeyCode::Home, true, false),
        (KeyCode::Home, true, true),
        (KeyCode::Home, false, false),
    ];
    for (i, (key, pressed, repeat)) in steps.into_iter().enumerate() {
        let mut sink = |e: &OfferedEvent, _: &mut BandRequests| {
            if let Some(a) = e.as_action() {
                log.push((a.phase, a.command.to_string(), a.command_id));
            }
            Verdict::Consumed
        };
        m.handle_with(
            &RawEvent::Key {
                key,
                pressed,
                repeat,
            },
            Duration::from_millis(10 * i as u64),
            &mut sink,
        );
    }
    eprintln!("offers: {log:?}");
    let phases: Vec<(OfferPhase, &str)> = log.iter().map(|(p, c, _)| (*p, c.as_str())).collect();
    assert_eq!(
        phases,
        [
            (OfferPhase::Start, "CursorCharLeft"),
            (OfferPhase::Repeat, "CursorCharLeft"),
            (OfferPhase::End, "CursorCharLeft"),
            (OfferPhase::Start, "CursorStartOfLine"),
            (OfferPhase::End, "CursorStartOfLine"),
        ]
    );
    assert!(
        log.iter().all(|(_, _, id)| id.is_some()),
        "every offered command has an ID from the install"
    );
}
