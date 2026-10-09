// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data assembly checks on a user's own install (read-only): every playable body of
//! `appearance.md` §2.2, every item row of `docs/checklists/m4-visual.md` (worn, layering
//! and held rows: M4 exit criterion 3, data side), the static NPC spots N1–N6 and all
//! weenie objects placed by the landblocks (`appearance.md` §7.1), with counts per
//! diagnostic category.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Archives are opened read-only;
//! nothing is written. Output is DIDs, keys and counts only. Run with
//! `cargo test -p tessera-mesh --release --test assembly_real_data -- --nocapture`. The
//! landblock scan covers every landblock-info record in release builds and every 23rd in
//! debug builds (`TESSERA_FULL=1` forces all).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::ArchiveSet;
use tessera_formats::landblock::{InstanceId, StaticEntityType};
use tessera_formats::script_table::resolve::{
    ClipResolver, MotionState, ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::setup::Anchor;
use tessera_formats::{Did, EnumMapper, LandblockInfo, PropertySet};
use tessera_mesh::assembly::{
    AssembledCharacter, AssemblyError, AssemblyVocabulary, CharacterAssembler, CharacterSpec,
    DiagnosticCategory as Cat, EquippedItem, GameDataSource, RigKind, ScriptAppearance, SourceRef,
    WieldState,
};
use tessera_props::{EnumSource, GameData};

fn data() -> Option<&'static GameData> {
    static DATA: OnceLock<Option<GameData>> = OnceLock::new();
    DATA.get_or_init(|| {
        let dir = match std::env::var_os("TESSERA_GAME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                eprintln!("TESSERA_GAME_DIR not set; skipping");
                return None;
            }
        };
        Some(GameData::open(&dir).expect("open the install read-only"))
    })
    .as_ref()
}

fn exhaustive() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

/// The key of `label` in the mapper that `property` is labelled by.
fn enum_key(data: &GameData, property: &str, label: &str) -> Option<u32> {
    let registry = data.registry().ok()?;
    let EnumSource::Mapper(m) = registry.enum_source(registry.pid(property)?)? else {
        return None;
    };
    EnumMapper::key_in_chain_from(m, |d| data.enum_mapper(d).ok().flatten(), label)
}

fn mesh_type(data: &GameData, label: &str) -> u32 {
    let m = data
        .lookup_path(&["EMAPPER", "MeshTypeID"])
        .unwrap()
        .expect("MeshTypeID mapper");
    EnumMapper::key_in_chain_from(m, |d| data.enum_mapper(d).ok().flatten(), label)
        .unwrap_or_else(|| panic!("MeshTypeID {label}"))
}

fn assembler(data: &'static GameData) -> CharacterAssembler<GameDataSource<'static, ArchiveSet>> {
    let vocab = AssemblyVocabulary::from_game_data(data).expect("vocabulary");
    CharacterAssembler::new(GameDataSource::new(data).expect("source"), vocab)
}

const MALE: u32 = 0x1000;
const FEMALE: u32 = 0x2000;

/// `appearance.md` §2.2 (P1–P14): label, sex, avatar entity, setup, model scale.
const BODIES: [(&str, &str, u32, u32, u32, f32); 14] = [
    ("P1", "Hobbit", MALE, 0x4700_0606, 0x0400_0551, 1.0),
    ("P2", "Hobbit", FEMALE, 0x4700_0607, 0x0400_0552, 1.0),
    ("P3", "Human", MALE, 0x4700_060A, 0x0400_0553, 1.0),
    ("P4", "Human", FEMALE, 0x4700_060B, 0x0400_0554, 1.0),
    ("P5", "Elf", MALE, 0x4700_060E, 0x0400_0555, 1.03),
    ("P6", "Elf", FEMALE, 0x4700_060F, 0x0400_0556, 1.02),
    ("P7", "Dwarf", MALE, 0x4700_0611, 0x0400_0557, 1.0),
    ("P8", "Beorning", MALE, 0x4700_7004, 0x0400_0553, 1.03),
    ("P9", "Beorning", FEMALE, 0x4700_7005, 0x0400_0554, 1.03),
    ("P10", "HighElf", MALE, 0x4700_806C, 0x0400_5EC4, 1.05),
    (
        "P11",
        "HighElf",
        FEMALE,
        0x4700_8071,
        0x0400_5EC5,
        1.03 * 1.02,
    ),
    ("P12", "DwarfStoutAxe", MALE, 0x4700_979A, 0x0400_0557, 1.0),
    ("P13", "RiverHobbit", MALE, 0x4700_CEA3, 0x0400_772B, 0.7),
    ("P14", "RiverHobbit", FEMALE, 0x4700_CEA4, 0x0400_772A, 0.7),
];

/// What a slot's winner must be.
#[derive(Debug, Clone, Copy)]
enum Want {
    /// A real (non-stub) mesh at this priority.
    Real(f32),
    /// A real mesh at any priority.
    RealAny,
    /// A stub at this priority (the slot is hidden).
    Stub(f32),
    /// This mesh at this priority.
    Mesh(u32, f32),
}

/// (slot label, which item of the row, expectation).
type Expect = (&'static str, usize, Want);

/// Worn rows W1–W22 (one item each, Human M unless a body is given): the slots the item
/// wins, exactly.
const WORN: [(&str, u32, &[Expect]); 22] = [
    (
        "W1",
        0x7900_06C3,
        &[
            ("Helmet", 0, Want::Real(4.0)),
            ("Crest", 0, Want::Stub(10.0)),
            ("Ears", 0, Want::Mesh(0x0600_E55A, 10.0)),
        ],
    ),
    (
        "W2",
        0x7900_05AF,
        &[
            ("Helmet", 0, Want::Real(4.0)),
            ("Crest", 0, Want::Real(10.0)),
        ],
    ),
    ("W3", 0x7900_A91E, &[("Helmet", 0, Want::Real(4.0))]),
    ("W4", 0x7900_B58F, &[("Helmet", 0, Want::Real(3.0))]),
    (
        "W5",
        0x7905_64CF,
        &[
            ("Helmet", 0, Want::Real(20.0)),
            ("Head", 0, Want::Stub(15.0)),
            ("Crest", 0, Want::Stub(10.0)),
            ("Beard", 0, Want::Stub(10.0)),
            ("Ears", 0, Want::Real(10.0)),
        ],
    ),
    ("W6", 0x7900_0670, &[("Shoulder", 0, Want::Real(1.0))]),
    ("W7", 0x7900_0420, &[("UpperBody", 0, Want::Real(1.0))]),
    (
        "W8",
        0x7900_0960,
        &[
            ("UpperBody", 0, Want::Real(1.0)),
            ("LowerBody", 0, Want::Stub(10.0)),
        ],
    ),
    (
        "W9",
        0x7900_DA3B,
        &[
            ("UpperBody", 0, Want::RealAny),
            ("LowerBody", 0, Want::Stub(10.0)),
            ("Feet", 0, Want::Stub(10.0)),
        ],
    ),
    (
        "W10",
        0x7903_4B00,
        &[
            ("UpperBody", 0, Want::RealAny),
            ("Hands", 0, Want::Mesh(0x0600_D4B0, 10.0)),
        ],
    ),
    (
        "W11",
        0x7900_D693,
        &[
            ("UpperBody", 0, Want::RealAny),
            ("LowerBody", 0, Want::Stub(10.0)),
            ("Hands", 0, Want::Mesh(0x0600_D4B0, 10.0)),
            ("Feet", 0, Want::Real(3.0)),
        ],
    ),
    (
        "W12",
        0x7900_D695,
        &[
            ("UpperBody", 0, Want::RealAny),
            ("LowerBody", 0, Want::Real(10.0)),
            ("Feet", 0, Want::Stub(10.0)),
            ("Hands", 0, Want::Mesh(0x0600_D4B0, 10.0)),
        ],
    ),
    ("W13", 0x7900_0412, &[("Cape", 0, Want::Real(1.0))]),
    (
        "W14",
        0x7902_A135,
        &[
            ("Cape", 0, Want::Real(1.0)),
            ("Crest", 0, Want::Stub(15.0)),
            ("Helmet", 0, Want::Stub(15.0)),
            ("Ears", 0, Want::Real(10.0)),
        ],
    ),
    ("W15", 0x7900_0411, &[("Hands", 0, Want::Real(1.0))]),
    ("W16", 0x7902_146C, &[("Hands", 0, Want::Real(6.0))]),
    ("W17", 0x7900_06BA, &[("LowerBody", 0, Want::Real(1.0))]),
    (
        "W18",
        0x7903_61BB,
        &[
            ("LowerBody", 0, Want::Real(1.0)),
            ("Feet", 0, Want::Real(7.0)),
        ],
    ),
    ("W19", 0x7900_0446, &[("Feet", 0, Want::Real(2.0))]),
    ("W20", 0x7900_0780, &[("Feet", 0, Want::Real(5.0))]),
    ("W21", 0x7901_852B, &[("Satchel", 0, Want::Real(3.0))]),
    ("W22", 0x7905_8E21, &[("Hands", 0, Want::Real(5.0))]),
];

/// The Hobbit M differences the checklist notes (P1).
const WORN_P1: [(&str, u32, &[Expect]); 5] = [
    (
        "W1 Hobbit",
        0x7900_06C3,
        &[
            ("Helmet", 0, Want::Real(4.0)),
            ("Crest", 0, Want::Stub(10.0)),
        ],
    ),
    ("W4 Hobbit", 0x7900_B58F, &[("Helmet", 0, Want::Real(4.0))]),
    (
        "W10 Hobbit",
        0x7903_4B00,
        &[
            ("UpperBody", 0, Want::RealAny),
            ("Hands", 0, Want::Stub(10.0)),
        ],
    ),
    (
        "W18 Hobbit",
        0x7903_61BB,
        &[("LowerBody", 0, Want::Real(1.0))],
    ),
    ("W19 Hobbit", 0x7900_0446, &[("Feet", 0, Want::Real(1.0))]),
];

/// Layering rows L1–L8 (Human M): the winners of the listed slots, by item index.
const LAYERED: [(&str, &[u32], &[Expect]); 9] = [
    (
        "L1",
        &[0x7900_0960, 0x7900_06BA],
        &[
            ("UpperBody", 0, Want::RealAny),
            ("LowerBody", 0, Want::Stub(10.0)),
        ],
    ),
    (
        "L2",
        &[0x7900_D693, 0x7900_0446],
        &[("Feet", 0, Want::Real(3.0))],
    ),
    (
        "L3",
        &[0x7900_D693, 0x7900_0780],
        &[("Feet", 1, Want::Real(5.0))],
    ),
    (
        "L4",
        &[0x7903_4B00, 0x7900_0411],
        &[("Hands", 0, Want::Mesh(0x0600_D4B0, 10.0))],
    ),
    (
        "L5",
        &[0x7902_A135, 0x7900_06C3],
        &[
            ("Helmet", 0, Want::Stub(15.0)),
            ("Crest", 0, Want::Stub(15.0)),
        ],
    ),
    (
        "L6",
        &[0x7902_A135, 0x7905_64CF],
        &[
            ("Helmet", 1, Want::Real(20.0)),
            ("Head", 1, Want::Stub(15.0)),
            ("Crest", 0, Want::Stub(15.0)),
            ("Beard", 1, Want::Stub(10.0)),
        ],
    ),
    (
        "L7",
        &[0x7903_61BB, 0x7900_0446],
        &[("Feet", 0, Want::Real(7.0))],
    ),
    (
        "L8a",
        &[0x7900_F859, 0x7905_8E21],
        &[("Hands", 1, Want::Real(5.0))],
    ),
    (
        "L8b",
        &[0x7900_F859, 0x7902_146C],
        &[("Hands", 1, Want::Real(6.0))],
    ),
];

/// Held rows H1–H25 (Human M setup 0x04000553), **wielded** (`appearance.md` §5.6
/// "Wielded placements"): every attachment's location label and bone; empty for "not
/// drawn". Changes against the M4 checklist (which followed the §5.3 paperdoll rule and
/// the item's parenting location): H15 (orb) is attached at `Wpn_RHand` and `Wpn_LHand`,
/// H17 (bow) at `Wpn_LHand`, H18 (crossbow) at `Wpn_RHand`, H23 at `Wpn_Forearm` (the
/// item's `Forearm` override takes no part), H24 (instrument) is not drawn.
/// (row, item, expected (location label, bone) per attachment).
type HeldRow = (&'static str, u32, &'static [(&'static str, u32)]);

const RHAND: &[(&str, u32)] = &[("Wpn_RHand", 79)];
const NONE: &[(&str, u32)] = &[];

const HELD: [HeldRow; 25] = [
    ("H1", 0x7900_066F, RHAND),
    ("H2", 0x7900_06D1, RHAND),
    ("H3", 0x7902_722C, RHAND),
    ("H4", 0x7901_C852, RHAND),
    ("H5", 0x7903_9DD5, RHAND),
    ("H6", 0x7903_83A4, RHAND),
    ("H7", 0x7906_8E0D, &[("Wpn_LOffHand", 65)]),
    ("H8", 0x7900_06CB, RHAND),
    ("H9", 0x7901_4A77, RHAND),
    ("H10", 0x7900_06EF, RHAND),
    ("H11", 0x7900_0831, RHAND),
    ("H12", 0x7900_ABD8, RHAND),
    ("H13", 0x7901_D6D7, RHAND),
    ("H14", 0x7901_198F, RHAND),
    ("H15", 0x7901_3A37, &[("Wpn_RHand", 79), ("Wpn_LHand", 65)]),
    ("H16", 0x7900_AF50, RHAND),
    ("H17", 0x7900_1F43, &[("Wpn_LHand", 65)]),
    ("H18", 0x7902_8A8E, RHAND),
    ("H19", 0x7901_09BA, &[("Wpn_ROffhand", 79)]),
    ("H20", 0x7901_D692, &[("Wpn_Forearm", 54)]),
    ("H21", 0x7900_F164, &[("Wpn_Forearm", 54)]),
    ("H22", 0x7900_06CF, &[("Wpn_Forearm", 54)]),
    ("H23", 0x7900_6BF8, &[("Wpn_Forearm", 54)]),
    ("H24", 0x7900_1298, NONE),
    ("H25", 0x7901_5F4E, NONE),
];

/// The H rows **stowed** (`appearance.md` §5.6 "Stowed placements") on Human M: one-handed
/// weapons and implements in the primary slot at the left hip, in the secondary slot at the
/// right hip; two-handed swords at `Sheath_Back_2H_Sword`; other two-handed weapons,
/// staves, spears and halberds at `Sheath_Back_2H`; bows and javelins at
/// `Sheath_Back_Bow`, crossbows at `Sheath_Back_Crossbow`; shields stay on the forearm
/// (skill mode not ranged, the Decision of §5.6 "Shields out of combat"); a ranged-slot
/// instrument is attached by the sheathed-instrument script (H24, revised 2026-10-09: the
/// Human M `Lute` node at `Sheath_Back_2H_Sword`, §5.6 "Stowed instruments"); orbs and class
/// items are not drawn.
const STOWED: [HeldRow; 25] = [
    ("H1", 0x7900_066F, &[("Sheath_LHip", 3)]),
    ("H2", 0x7900_06D1, &[("Sheath_LHip", 3)]),
    ("H3", 0x7902_722C, &[("Sheath_LHip", 3)]),
    ("H4", 0x7901_C852, &[("Sheath_LHip", 3)]),
    ("H5", 0x7903_9DD5, &[("Sheath_LHip", 3)]),
    ("H6", 0x7903_83A4, &[("Sheath_LHip", 3)]),
    ("H7", 0x7906_8E0D, &[("Sheath_RHip", 3)]),
    ("H8", 0x7900_06CB, &[("Sheath_Back_2H_Sword", 16)]),
    ("H9", 0x7901_4A77, &[("Sheath_Back_2H", 16)]),
    ("H10", 0x7900_06EF, &[("Sheath_Back_2H", 16)]),
    ("H11", 0x7900_0831, &[("Sheath_Back_2H", 16)]),
    ("H12", 0x7900_ABD8, &[("Sheath_Back_2H", 16)]),
    ("H13", 0x7901_D6D7, &[("Sheath_Back_2H", 16)]),
    ("H14", 0x7901_198F, &[("Sheath_Back_2H", 16)]),
    ("H15", 0x7901_3A37, NONE),
    ("H16", 0x7900_AF50, &[("Sheath_LHip", 3)]),
    ("H17", 0x7900_1F43, &[("Sheath_Back_Bow", 16)]),
    ("H18", 0x7902_8A8E, &[("Sheath_Back_Crossbow", 16)]),
    ("H19", 0x7901_09BA, &[("Sheath_Back_Bow", 16)]),
    ("H20", 0x7901_D692, &[("Wpn_Forearm", 54)]),
    ("H21", 0x7900_F164, &[("Wpn_Forearm", 54)]),
    ("H22", 0x7900_06CF, &[("Wpn_Forearm", 54)]),
    ("H23", 0x7900_6BF8, &[("Wpn_Forearm", 54)]),
    ("H24", 0x7900_1298, &[("Sheath_Back_2H_Sword", 16)]),
    ("H25", 0x7901_5F4E, NONE),
];

fn check_winners(
    data: &GameData,
    row: &str,
    c: &AssembledCharacter,
    expect: &[Expect],
    exact: bool,
    failures: &mut Vec<String>,
) {
    for &(label, item, want) in expect {
        let slot = mesh_type(data, label);
        let Some(p) = c.part_in(slot) else {
            failures.push(format!("{row}: no part in {label}"));
            continue;
        };
        let from = p.source();
        let ok_source = from == Some(SourceRef::Item(item as u32));
        let ok = match want {
            Want::Real(pr) => !p.hidden && p.priority == pr,
            Want::RealAny => !p.hidden,
            Want::Stub(pr) => p.hidden && p.priority == pr,
            Want::Mesh(m, pr) => p.mesh == Did(m) && p.priority == pr && !p.hidden,
        };
        if !ok || !ok_source {
            failures.push(format!(
                "{row}: {label} expected {want:?} from item {item}, found mesh {} priority {} \
                 hidden {} from {from:?}",
                p.mesh, p.priority, p.hidden
            ));
        }
    }
    if exact {
        let want: BTreeSet<u32> = expect.iter().map(|e| mesh_type(data, e.0)).collect();
        let got: BTreeSet<u32> = c
            .parts
            .iter()
            .filter(|p| matches!(p.source(), Some(SourceRef::Item(_))))
            .map(|p| p.slot)
            .collect();
        if want != got {
            failures.push(format!(
                "{row}: the item wins slots {got:x?}, expected exactly {want:x?}"
            ));
        }
    }
    let bad: Vec<_> = c
        .diagnostics
        .iter()
        // DyeInexact is informational: 156 items carry a dye code their table lacks
        // (`appearance.md` §4.9), and the block rule still picks a dye.
        .filter(|d| matches!(d.source, SourceRef::Item(_)) && d.category != Cat::DyeInexact)
        .collect();
    if !bad.is_empty() {
        failures.push(format!("{row}: item diagnostics {bad:?}"));
    }
}

fn diagnostic_summary(c: &AssembledCharacter) -> String {
    let mut counts: BTreeMap<Cat, usize> = BTreeMap::new();
    for d in &c.diagnostics {
        *counts.entry(d.category).or_default() += 1;
    }
    counts
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
fn bodies_and_checklist_items_assemble() {
    let Some(data) = data() else { return };
    let mut asm = assembler(data);
    let species = |l: &str| enum_key(data, "RaceTable_Race", l).unwrap_or_else(|| panic!("{l}"));
    let mut failures = Vec::new();

    // Clip selection from the clip inputs (script-table.md §11.3).
    let registry = data.registry().unwrap();
    let dict = registry.dictionary();
    let script_vocab = ScriptVocabulary::from_data(dict, |d| {
        data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
    })
    .expect("script vocabulary");
    let fetch = || |d: Did| data.read(d);
    let mut clips =
        ClipResolver::new(dict, script_vocab.clone(), fetch()).with_dictionary_defaults(dict);

    // B: every playable body, bare and with a basic outfit (W7, W17, W19).
    let outfit = [0x7900_0420u32, 0x7900_06BA, 0x7900_0446];
    let (upper, lower, feet) = (
        mesh_type(data, "UpperBody"),
        mesh_type(data, "LowerBody"),
        mesh_type(data, "Feet"),
    );
    for (row, label, sex, entity, setup, scale) in BODIES {
        let spec = CharacterSpec::new(species(label), sex);
        let c = asm.assemble_character(&spec).expect("body assembles");
        if c.entity != Did(entity) || c.skeleton.setup != Did(setup) {
            failures.push(format!(
                "{row}: entity {} setup {}, expected {entity:#x} {setup:#x}",
                c.entity, c.skeleton.setup
            ));
        }
        let ms = c.model_scale();
        if ms.iter().any(|s| (s - scale).abs() > 1e-3) {
            failures.push(format!("{row}: scale {ms:?} expected {scale}"));
        }
        assert_eq!(c.rig, RigKind::Humanoid, "{row}");
        assert!(c.skeleton.tagfile.len() > 8, "{row}: tagfile");
        assert!(
            c.parts.iter().any(|p| p.slot == upper && !p.hidden),
            "{row}"
        );
        let mut clip_dids = Vec::new();
        let chain = collect_entity_scripts(c.clip_inputs.start, dict, &script_vocab, fetch())
            .expect("avatar scripts");
        for motion in [MotionState::Idle, MotionState::Walk, MotionState::Run] {
            let r = clips
                .resolve(&chain.scripts, motion)
                .expect("clip resolution");
            let all = r.clips();
            if all.is_empty() {
                failures.push(format!("{row}: no clip for {motion:?}"));
            }
            clip_dids.push(format!(
                "{motion:?} {} clips, first {:?}",
                all.len(),
                all.first()
            ));
        }
        eprintln!(
            "{row} {label} {sex:#06x}: entity {} setup {} scale {:.4} parts {} (hidden {}) \
             sliders {:?}/{} clips [{}] diagnostics [{}]",
            c.entity,
            c.skeleton.setup,
            c.model_scale()[2],
            c.parts.len(),
            c.hidden_slots().count(),
            c.sliders.definitions,
            c.sliders.values.len(),
            clip_dids.join(", "),
            diagnostic_summary(&c)
        );
        let dressed = outfit
            .iter()
            .fold(spec, |s, &i| s.wearing(EquippedItem::new(i)));
        let c = asm.assemble_character(&dressed).expect("dressed body");
        for slot in [upper, lower, feet] {
            if !c
                .part_in(slot)
                .is_some_and(|p| matches!(p.source(), Some(SourceRef::Item(_))))
            {
                failures.push(format!("{row}: outfit does not fill slot {slot:#x}"));
            }
        }
        if c.diagnostics
            .iter()
            .any(|d| matches!(d.source, SourceRef::Item(_)) && d.category != Cat::DyeInexact)
        {
            failures.push(format!("{row}: outfit diagnostics {:?}", c.diagnostics));
        }
    }

    // W, L: worn and layered items on Human M (and the Hobbit M notes).
    let human_m = CharacterSpec::new(species("Human"), MALE);
    let p1_male = CharacterSpec::new(species("Hobbit"), MALE);
    let mut items_checked = BTreeSet::new();
    for (body, rows) in [(&human_m, &WORN[..]), (&p1_male, &WORN_P1[..])] {
        for &(row, item, expect) in rows {
            items_checked.insert(item);
            let spec = body.clone().wearing(EquippedItem::new(item));
            let c = asm.assemble_character(&spec).expect("worn row");
            check_winners(data, row, &c, expect, true, &mut failures);
        }
    }
    for (row, items, expect) in LAYERED {
        items_checked.extend(items.iter().copied());
        let spec = items
            .iter()
            .fold(human_m.clone(), |s, &i| s.wearing(EquippedItem::new(i)));
        let c = asm.assemble_character(&spec).expect("layered row");
        check_winners(data, row, &c, expect, false, &mut failures);
    }

    // H: held items on Human M; H20 also on Hobbit M (Lulna 17) and Dwarf M (Lulna 21).
    let location =
        |l: &str| enum_key(data, "Inventory_ParentingLocation", l).unwrap_or_else(|| panic!("{l}"));
    let dwarf_m = CharacterSpec::new(species("Dwarf"), MALE);
    let held_rows = HELD
        .iter()
        .map(|&(r, i, w)| (r.to_string(), &human_m, i, w))
        .chain([
            (
                "H20 Hobbit".into(),
                &p1_male,
                0x7901_D692,
                &[("Wpn_Forearm", 17)][..],
            ),
            (
                "H20 Dwarf".into(),
                &dwarf_m,
                0x7901_D692,
                &[("Wpn_Forearm", 21)][..],
            ),
        ]);
    let stowed_rows = STOWED
        .iter()
        .map(|&(r, i, w)| (format!("{r} stowed"), &human_m, i, w, WieldState::Sheathed));
    let held_rows = held_rows
        .map(|(r, b, i, w)| (r, b, i, w, WieldState::Wielded))
        .chain(stowed_rows);
    for (row, body, item, want, state) in held_rows {
        items_checked.insert(item);
        let spec = body
            .clone()
            .wearing(EquippedItem::new(item).with_state(state));
        let c = asm.assemble_character(&spec).expect("held row");
        if want.is_empty() {
            if !c.held.is_empty() || c.count(Cat::NoPlacement) != 1 {
                failures.push(format!(
                    "{row}: expected not drawn, held {:?}, diagnostics {:?}",
                    c.held, c.diagnostics
                ));
            }
        } else {
            let got: Vec<(u32, Anchor)> = c.held.iter().map(|h| (h.location, h.anchor)).collect();
            let expect: Vec<(u32, Anchor)> = want
                .iter()
                .map(|&(l, b)| (location(l), Anchor::Bone(b)))
                .collect();
            if got != expect {
                failures.push(format!(
                    "{row}: held {got:x?}, expected {want:?}, diagnostics {:?}",
                    c.diagnostics
                ));
            }
            if c.held.iter().any(|h| h.parts.is_empty()) {
                failures.push(format!("{row}: an item setup has no parts"));
            }
        }
        if c.parts
            .iter()
            .any(|p| !p.is_default() && matches!(p.source(), Some(SourceRef::Item(_))))
        {
            failures.push(format!("{row}: a held item was worn"));
        }
        // H24 stowed (m4-visual.md, revised 2026-10-09): offset (0, −0.1, 0.02) m, yaw 180,
        // pitch 20, roll 0, read from the sheathed-instrument script's node.
        if row == "H24 stowed"
            && let Some(h) = c.held.first()
        {
            let close =
                |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4);
            if !close(h.placement.offset, [0.0, -0.1, 0.02])
                || !close(h.placement.rotation, [180.0, 20.0, 0.0])
            {
                failures.push(format!("{row}: placement {:?}", h.placement));
            }
        }
    }
    // Dual wielding: H1 in the primary slot and H2 in the secondary slot.
    let secondary = enum_key(data, "Inventory_DefaultSlot", "Weapon_Secondary")
        .and_then(|k| k.checked_sub(1))
        .map(|b| 1u64 << b)
        .expect("Weapon_Secondary");
    for (state, want) in [
        (
            WieldState::Wielded,
            [("Wpn_RHand", 79), ("Wpn_LOffHand", 65)],
        ),
        (
            WieldState::Sheathed,
            [("Sheath_LHip", 3), ("Sheath_RHip", 3)],
        ),
    ] {
        let spec = human_m
            .clone()
            .wearing(EquippedItem::new(0x7900_066F).with_state(state))
            .wearing(
                EquippedItem::new(0x7900_06D1)
                    .in_slot(secondary)
                    .with_state(state),
            );
        let c = asm.assemble_character(&spec).expect("dual wield");
        let got: Vec<(u32, Anchor)> = c.held.iter().map(|h| (h.location, h.anchor)).collect();
        let expect: Vec<(u32, Anchor)> = want
            .iter()
            .map(|&(l, b)| (location(l), Anchor::Bone(b)))
            .collect();
        if got != expect {
            failures.push(format!("dual wield {state:?}: {got:?}, expected {want:?}"));
        }
    }
    eprintln!(
        "checklist: {} bodies, {} distinct items over {} worn, {} layered, {} held and {} \
         stowed rows",
        BODIES.len(),
        items_checked.len(),
        WORN.len() + WORN_P1.len(),
        LAYERED.len(),
        HELD.len() + 2,
        STOWED.len()
    );
    assert!(
        items_checked.len() >= 30,
        "exit criterion 3: at least 30 items"
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// One placed entity of interest from the landblock scan.
#[derive(Debug, Clone)]
struct Placed {
    landblock: u32,
    iid: InstanceId,
    object: Did,
    instance: Option<PropertySet>,
}

/// Weenie SEDs (first placement of each object) and the checklist's N rows.
fn scan_landblocks(
    data: &'static GameData,
    wanted: &[(u32, u64)],
) -> (Vec<Placed>, Vec<Placed>, usize) {
    let set = data.source();
    let registry = data.registry().unwrap();
    let mut work = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        work.extend(
            m.archive
                .entries_range(0x8020_0000..=0x802F_FFFF)
                .map(|e| (i, e.expect("directory walk"))),
        );
    }
    let full = exhaustive();
    let work: Vec<_> = work
        .into_iter()
        .enumerate()
        .filter(|(n, (_, e))| full || n % 23 == 0 || wanted.iter().any(|w| w.0 == e.did))
        .map(|(_, w)| w)
        .collect();
    let next = AtomicUsize::new(0);
    let out = Mutex::new((Vec::new(), Vec::new(), 0usize));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let (mut weenies, mut rows, mut decoded) = (Vec::new(), Vec::new(), 0);
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((m, e)) = work.get(i) else { break };
                    let Ok(bytes) = set.member(*m).archive.read_entry(e) else {
                        continue;
                    };
                    let Ok(info) = LandblockInfo::parse_expecting(
                        &bytes,
                        e.did,
                        Some(e.stamp),
                        registry.as_ref(),
                    ) else {
                        continue;
                    };
                    decoded += 1;
                    let instance = |iid: InstanceId| {
                        info.weenies
                            .items
                            .iter()
                            .find(|w| w.iid == iid)
                            .map(|w| w.properties.clone())
                    };
                    for sed in &info.entities {
                        let (Some(iid), Some(object)) = (sed.iid, sed.object) else {
                            continue;
                        };
                        let row = wanted.iter().any(|w| w.0 == e.did && w.1 == iid.0);
                        let weenie = sed.kind() == Some(StaticEntityType::Weenie);
                        if row || weenie {
                            let p = Placed {
                                landblock: e.did,
                                iid,
                                object,
                                instance: instance(iid),
                            };
                            if row {
                                rows.push(p.clone());
                            }
                            if weenie {
                                weenies.push(p);
                            }
                        }
                    }
                }
                let mut o = out.lock().unwrap();
                o.0.extend(weenies);
                o.1.extend(rows);
                o.2 += decoded;
            });
        }
    });
    let (mut weenies, rows, decoded) = out.into_inner().unwrap();
    weenies.sort_by_key(|p| (p.object, p.landblock, p.iid));
    weenies.dedup_by_key(|p| p.object);
    (weenies, rows, decoded)
}

#[test]
fn weenie_objects_and_npc_spots_assemble() {
    let Some(data) = data() else { return };
    // N rows of the checklist: (landblock info, IID, object, setup, idle enum, props
    // (entity, location label)).
    type NRow = (
        &'static str,
        u32,
        u64,
        u32,
        u32,
        u32,
        &'static [(u32, &'static str)],
    );
    const N_ROWS: [NRow; 6] = [
        (
            "N1",
            0x8021_1B8F,
            0x0111_B8F0_0000_002C,
            0x4700_4E6F,
            0x0400_003E,
            0x7,
            &[(0x4700_0EB5, "LHand"), (0x4700_2912, "RHand")],
        ),
        (
            "N2",
            0x8021_678A,
            0x0116_78A0_0000_00A5,
            0x4700_4E6F,
            0x0400_003E,
            0x59,
            &[(0x4700_06D9, "RHand"), (0x4700_0161, "LHand")],
        ),
        (
            "N3",
            0x8021_3FAA,
            0x0113_FAA0_0000_00C8,
            0x4700_4E75,
            0x0400_0004,
            0x5E,
            &[],
        ),
        (
            "N4",
            0x8021_3B85,
            0x0113_B850_0000_0019,
            0x4700_4FB6,
            0x0400_0152,
            0x29,
            &[(0x4700_0650, "Wpn_RHand")],
        ),
        (
            "N5",
            0x8021_4F79,
            0x0114_F790_0000_0076,
            0x4700_4E74,
            0x0400_0004,
            0x5A,
            &[(0x4700_0E36, "RHand")],
        ),
        (
            "N6",
            0x8025_3E07,
            0x0453_E070_0000_001B,
            0x7006_986E,
            0x0400_003E,
            0x41,
            &[(0x4700_012D, "Wpn_LHand")],
        ),
    ];
    let wanted: Vec<(u32, u64)> = N_ROWS.iter().map(|r| (r.1, r.2)).collect();
    let (weenies, rows, decoded) = scan_landblocks(data, &wanted);
    eprintln!(
        "landblock scan: {decoded} records decoded, {} distinct weenie objects, {} N rows found",
        weenies.len(),
        rows.len()
    );
    let mut asm = assembler(data);
    let location = |l: &str| enum_key(data, "NPC_Cosmetic_Inventory_HoldingLocation1", l);
    let mut failures = Vec::new();

    // N rows.
    for (row, lb, iid, object, setup, idle, props) in N_ROWS {
        let Some(p) = rows.iter().find(|p| p.landblock == lb && p.iid.0 == iid) else {
            failures.push(format!("{row}: SED not found"));
            continue;
        };
        if p.object != Did(object) {
            failures.push(format!("{row}: object {} expected {object:#x}", p.object));
        }
        let c = match asm.assemble_object(p.object, p.instance.as_ref()) {
            Ok(c) => c,
            Err(e) => {
                failures.push(format!("{row}: {e}"));
                continue;
            }
        };
        if c.skeleton.setup != Did(setup) {
            failures.push(format!(
                "{row}: setup {} expected {setup:#x}",
                c.skeleton.setup
            ));
        }
        if idle != 0 && c.clip_inputs.starting_idle_animation != Some(idle) {
            failures.push(format!(
                "{row}: idle {:?} expected {idle:#x}",
                c.clip_inputs.starting_idle_animation
            ));
        }
        for &(prop, label) in props {
            let found = c
                .held
                .iter()
                .any(|h| h.entity == Did(prop) && Some(h.location) == location(label));
            if !found {
                failures.push(format!(
                    "{row}: prop {prop:#x} not at {label}: {:?}",
                    c.held
                ));
            }
        }
        // N3 (corrected 2026-10-09): two pieces from the instance list, 0x2000002D and
        // 0x2000002E; the `Worn` element for 0x2000080A names a key that record lacks.
        if row == "N3" {
            let records: BTreeSet<u32> = c
                .parts
                .iter()
                .filter_map(|q| match q.origin {
                    tessera_mesh::assembly::PartOrigin::Appearance {
                        source: SourceRef::InstanceList(_),
                        record,
                        ..
                    } if !q.hidden => Some(record.0),
                    _ => None,
                })
                .collect();
            let want: BTreeSet<u32> = [0x2000_002D, 0x2000_002E].into_iter().collect();
            if records != want {
                failures.push(format!("N3: pieces from {records:x?}, expected {want:x?}"));
            }
        }
        if std::env::var_os("TESSERA_VERBOSE").is_some() {
            for part in &c.parts {
                eprintln!(
                    "    slot {:#x} mesh {} prio {} hidden {} origin {:?} layers {}",
                    part.slot,
                    part.mesh,
                    part.priority,
                    part.hidden,
                    part.origin,
                    part.materials.len()
                );
            }
            for d in &c.diagnostics {
                eprintln!("    {d:?}");
            }
        }
        eprintln!(
            "{row}: object {} setup {} rig {:?} scale {:.3} list {:?} parts {} held {} random {:?} \
             diagnostics [{}]",
            p.object,
            c.skeleton.setup,
            c.rig,
            c.model_scale()[2],
            c.instance_list_origin,
            c.parts.len(),
            c.held.len(),
            c.random_appearance,
            diagnostic_summary(&c)
        );
    }

    // N5 and N6 are random-appearance figures (m4-visual.md, corrected 2026-10-09): their
    // outfit comes from the Appearance nodes of their start-up scripts (§7.4 Rule), picked
    // with a generator seeded from the instance ID (§7.4 Decision). The picks must be
    // deterministic and must dress the rig.
    for (row, lb) in [("N5", 0x8021_4F79u32), ("N6", 0x8025_3E07)] {
        let Some(p) = rows.iter().find(|p| p.landblock == lb) else {
            continue;
        };
        let registry = data.registry().unwrap();
        let dict = registry.dictionary();
        let script_vocab = ScriptVocabulary::from_data(dict, |d| {
            data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
        })
        .expect("script vocabulary");
        let base = asm
            .assemble_object(p.object, p.instance.as_ref())
            .expect("random figure assembles");
        let picks = |seed: u64| {
            let mut chain =
                collect_entity_scripts(base.clip_inputs.start, dict, &script_vocab, |d: Did| {
                    data.read(d)
                })
                .expect("random figure scripts");
            if let Some(inst) = &base.clip_inputs.instance_properties {
                chain.scripts.add_property_set(inst, &script_vocab);
            }
            let mut resolver = ClipResolver::new(dict, script_vocab.clone(), |d: Did| data.read(d))
                .with_dictionary_defaults(dict)
                .with_random_seed(Some(seed));
            let r = resolver
                .resolve(&chain.scripts, MotionState::Idle)
                .expect("random figure resolution");
            r.appearances
                .iter()
                .filter(|a| a.picked)
                .map(|a| ScriptAppearance {
                    record: a.appearance,
                    key: a.key,
                    modifier: a.modifier,
                })
                .collect::<Vec<_>>()
        };
        let scripts = picks(p.iid.0);
        assert_eq!(
            scripts,
            picks(p.iid.0),
            "{row}: the picks are deterministic"
        );
        let c = asm
            .assemble_object_with(p.object, p.instance.as_ref(), &scripts)
            .expect("random figure dressed");
        let dressed = c
            .parts
            .iter()
            .filter(|q| matches!(q.source(), Some(SourceRef::Script(_))))
            .count();
        eprintln!(
            "{row}: {} script elements picked, {dressed} parts from them, diagnostics [{}]",
            scripts.len(),
            diagnostic_summary(&c)
        );
        if scripts.is_empty() || dressed == 0 || c.count(Cat::RandomAppearance) != 0 {
            failures.push(format!(
                "{row}: random appearance not dressed ({} elements, {dressed} parts)",
                scripts.len()
            ));
        }
    }

    // All weenie objects, each with the instance set of its first placement.
    let mut categories: BTreeMap<Cat, usize> = BTreeMap::new();
    let mut errors: BTreeMap<String, Vec<Did>> = BTreeMap::new();
    let mut rigs: HashMap<String, usize> = HashMap::new();
    let mut setups: BTreeMap<Did, usize> = BTreeMap::new();
    let mut origins: BTreeMap<String, usize> = BTreeMap::new();
    let (mut with_instance, mut held, mut idle) = (0, 0, 0);
    let mut assembled = 0;
    for p in &weenies {
        match asm.assemble_object(p.object, p.instance.as_ref()) {
            Ok(c) => {
                assembled += 1;
                *rigs.entry(format!("{:?}", c.rig)).or_default() += 1;
                *setups.entry(c.skeleton.setup).or_default() += 1;
                *origins
                    .entry(format!("{:?}", c.instance_list_origin))
                    .or_default() += 1;
                with_instance += usize::from(p.instance.is_some());
                held += c.held.len();
                idle += usize::from(c.clip_inputs.starting_idle_animation.is_some());
                if c.rig != RigKind::Static && std::env::var_os("TESSERA_VERBOSE").is_some() {
                    eprintln!(
                        "  {:?} weenie {}: entity {} setup {} parts {}",
                        c.rig,
                        p.object,
                        c.entity,
                        c.skeleton.setup,
                        c.parts.len()
                    );
                }
                for d in &c.diagnostics {
                    *categories.entry(d.category).or_default() += 1;
                }
                if c.random_appearance.is_some() {
                    eprintln!(
                        "  random-appearance weenie {}: setup {} rig {:?} type {:#x?}",
                        p.object, c.skeleton.setup, c.rig, c.random_appearance
                    );
                }
            }
            Err(e) => {
                let kind = match e {
                    AssemblyError::Body { .. } => "body",
                    AssemblyError::Setup { .. } => "setup",
                    AssemblyError::Source(_) => "source",
                    _ => "other",
                };
                errors.entry(kind.into()).or_default().push(p.object);
            }
        }
    }
    eprintln!(
        "weenie objects: {} placed, {assembled} assembled; rigs {rigs:?}; {} distinct setups",
        weenies.len(),
        setups.len()
    );
    eprintln!(
        "  with an instance set {with_instance}; instance list from {origins:?}; held parts \
         {held}; idle animations {idle}"
    );
    eprintln!("  errors: {errors:?}");
    for cat in Cat::ALL {
        if let Some(n) = categories.get(&cat) {
            eprintln!("  {cat}: {n}");
        }
    }
    assert!(!weenies.is_empty());
    assert!(
        errors.is_empty(),
        "every weenie object resolves to a body (§7.1)"
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
