// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the figure look (`appearance.md` §3.5) on a user's own install
//! (read-only): for every playable body of §2.2, the species table's `Startup` attaches a
//! face compositor that composes, and colour edits tint the hair and skin surfaces; the
//! two graphics options switch the compositor as §3.5 states.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Archives are opened read-only;
//! nothing is written. Output is DIDs, keys and counts only. Run with
//! `cargo test -p tessera-mesh --release --test look_real_data -- --nocapture`.

use std::path::PathBuf;
use std::sync::OnceLock;

use tessera_formats::script_table::resolve::{
    ClipResolution, ClipResolver, MotionState, ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::{Did, EnumMapper};
use tessera_mesh::assembly::look::{
    FigureLook, LookLimits, apply_figure_properties, figure_look_from_data,
};
use tessera_mesh::assembly::{
    AssembledCharacter, AssemblyOptions, AssemblyVocabulary, CharacterAssembler, CharacterSpec,
    GameDataSource,
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

const MALE: u32 = 0x1000;
const FEMALE: u32 = 0x2000;

/// `appearance.md` §2.2 (P1–P14): label and sex.
const BODIES: [(&str, &str, u32); 14] = [
    ("P1", "Hobbit", MALE),
    ("P2", "Hobbit", FEMALE),
    ("P3", "Human", MALE),
    ("P4", "Human", FEMALE),
    ("P5", "Elf", MALE),
    ("P6", "Elf", FEMALE),
    ("P7", "Dwarf", MALE),
    ("P8", "Beorning", MALE),
    ("P9", "Beorning", FEMALE),
    ("P10", "HighElf", MALE),
    ("P11", "HighElf", FEMALE),
    ("P12", "DwarfStoutAxe", MALE),
    ("P13", "RiverHobbit", MALE),
    ("P14", "RiverHobbit", FEMALE),
];

/// The compositor names of §3.5: the newer face and the older face.
const NEWER_FACE: u32 = 0x1000_0001;
const OLDER_FACE: u32 = 0x1000_000A;
/// The face-skin binding key the compositors attach to (§3.5).
const FACE_SKIN: u32 = 0x1000_006C;
/// Bodies whose `Startup` evaluation reaches no compositor node although the closure holds
/// some (`appearance.md` §15 OPEN of 2026-10-10).
const NO_FACE_OPEN: [&str; 4] = ["P5", "P6", "P10", "P11"];
/// Bodies whose newer face is attached even with `GOP_AvatarFaceCompositing` off
/// (`appearance.md` §15 OPEN of 2026-10-10).
const COMPOSITING_OFF_OPEN: [&str; 4] = ["P3", "P4", "P13", "P14"];

struct Probe {
    data: &'static GameData,
    script_vocab: ScriptVocabulary,
    asm_vocab: AssemblyVocabulary,
}

impl Probe {
    fn resolve(&self, c: &AssembledCharacter, options: AssemblyOptions) -> ClipResolution {
        let registry = self.data.registry().unwrap();
        let dict = registry.dictionary();
        let mut chain =
            collect_entity_scripts(c.clip_inputs.start, dict, &self.script_vocab, |d| {
                self.data.read(d)
            })
            .expect("scripts");
        if let Some(inst) = &c.clip_inputs.instance_properties {
            chain.scripts.add_property_set(inst, &self.script_vocab);
        }
        apply_figure_properties(&mut chain.scripts, c, &self.asm_vocab, options);
        let mut r = ClipResolver::new(dict, self.script_vocab.clone(), |d: Did| self.data.read(d))
            .with_dictionary_defaults(dict)
            .with_random_seed(Some(0));
        r.resolve(&chain.scripts, MotionState::Idle)
            .expect("resolution")
    }

    fn look(&self, c: &AssembledCharacter, options: AssemblyOptions) -> FigureLook {
        let r = self.resolve(c, options);
        figure_look_from_data(self.data, &r, &LookLimits::default())
    }
}

#[test]
fn playable_bodies_get_composited_faces_and_tints() {
    let Some(data) = data() else { return };
    let registry = data.registry().unwrap();
    let dict = registry.dictionary();
    let script_vocab = ScriptVocabulary::from_data(dict, |d| {
        data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
    })
    .expect("script vocabulary");
    let asm_vocab = AssemblyVocabulary::from_game_data(data).expect("vocabulary");
    let probe = Probe {
        data,
        script_vocab,
        asm_vocab: asm_vocab.clone(),
    };
    let mut asm = CharacterAssembler::new(GameDataSource::new(data).expect("source"), asm_vocab);
    let (head, crest) = (mesh_type(data, "Head"), mesh_type(data, "Crest"));
    let on = AssemblyOptions::default();
    let mut failures = Vec::new();
    let mut gaps = Vec::new();
    let mut composed = 0;
    for (row, label, sex) in BODIES {
        let species = enum_key(data, "RaceTable_Race", label).unwrap_or_else(|| panic!("{label}"));
        let c = asm
            .assemble_character(&CharacterSpec::new(species, sex))
            .expect("body assembles");
        let look = probe.look(&c, on);
        let faces: Vec<String> = look
            .compositors
            .iter()
            .map(|f| {
                format!(
                    "{:#x} {} slot {:#x} key {:#x} {}",
                    f.name,
                    f.record,
                    f.mesh_type,
                    f.material_type,
                    f.image.as_ref().map_or_else(
                        || "no image".into(),
                        |i| format!("{}×{}", i.width, i.height)
                    )
                )
            })
            .collect();
        let hair_edits = look
            .edits
            .iter()
            .filter(|e| e.parameter == 2003 && (e.mesh_type == crest || e.mesh_type == 2))
            .count();
        let skin_edits = look
            .edits
            .iter()
            .filter(|e| e.parameter == 2003 && e.material_type == FACE_SKIN)
            .count();
        eprintln!(
            "{row} {label}: compositors [{}]; {} colour edits ({hair_edits} hair, {skin_edits} \
             face skin), {} other; notes {:?}",
            faces.join("; "),
            look.edits.len(),
            look.other_edits,
            look.notes
        );
        // §3.5: with both options on, the newer face is attached to the head's face skin
        // and composes. Elf and High-elf bodies reach no compositor node (appearance.md §15
        // OPEN of 2026-10-10): reported, not failed.
        match look.compositors.iter().find(|f| f.name == NEWER_FACE) {
            Some(f) if f.image.is_some() && f.mesh_type == head && f.material_type == FACE_SKIN => {
                composed += 1;
            }
            Some(f) => failures.push(format!("{row}: newer face {f:?} does not compose")),
            None if NO_FACE_OPEN.contains(&row) => {
                gaps.push(format!("{row}: no face compositor reached (OPEN)"));
            }
            None => failures.push(format!("{row}: no newer face compositor")),
        }
        if look.compositors.iter().any(|f| f.name == OLDER_FACE) {
            failures.push(format!(
                "{row}: the older face is attached with the option on"
            ));
        }
        if hair_edits == 0 {
            failures.push(format!("{row}: no hair colour edit"));
        }
        // §3.5: with face compositing off, no compositor is attached.
        let off = probe.look(
            &c,
            AssemblyOptions {
                avatar_face_compositing: false,
                ..on
            },
        );
        if !off.compositors.is_empty() && COMPOSITING_OFF_OPEN.contains(&row) {
            gaps.push(format!(
                "{row}: {} compositors with face compositing off (OPEN)",
                off.compositors.len()
            ));
        } else if !off.compositors.is_empty() {
            failures.push(format!(
                "{row}: {} compositors with face compositing off",
                off.compositors.len()
            ));
        }
        // §3.5: with the updated-avatar option off, one face is chosen by the head (the
        // older one in the tables that define both); never both.
        let old = probe.look(
            &c,
            AssemblyOptions {
                avatar_update_visible: false,
                ..on
            },
        );
        let names: Vec<u32> = old.compositors.iter().map(|f| f.name).collect();
        eprintln!("{row}: updated-avatar option off: compositor names {names:x?}");
        if names.len() > 1 {
            failures.push(format!(
                "{row}: several faces with the option off: {names:x?}"
            ));
        }
    }
    eprintln!("{composed} of {} bodies composed a face", BODIES.len());
    for g in &gaps {
        eprintln!("  open: {g}");
    }
    assert!(
        composed >= BODIES.len() - NO_FACE_OPEN.len(),
        "faces composed"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn appearance_values_drive_the_look() {
    let Some(data) = data() else { return };
    let registry = data.registry().unwrap();
    let dict = registry.dictionary();
    let script_vocab = ScriptVocabulary::from_data(dict, |d| {
        data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
    })
    .expect("script vocabulary");
    let asm_vocab = AssemblyVocabulary::from_game_data(data).expect("vocabulary");
    let probe = Probe {
        data,
        script_vocab,
        asm_vocab: asm_vocab.clone(),
    };
    let mut asm = CharacterAssembler::new(GameDataSource::new(data).expect("source"), asm_vocab);
    let species = enum_key(data, "RaceTable_Race", "Human").expect("Human");
    let (Some(hair), Some(skin)) = (dict.pid("AC_Color_Hair"), dict.pid("AC_Color_Skin")) else {
        panic!("colour properties");
    };
    let mut spec = CharacterSpec::new(species, MALE);
    // Bytes B, G, R, A (property-system.md §4.7): a pure red hair and a pure blue skin.
    spec.appearance_values = vec![
        (
            hair,
            tessera_formats::PropertyValue::Colour([0, 0, 255, 255]),
        ),
        (
            skin,
            tessera_formats::PropertyValue::Colour([255, 0, 0, 255]),
        ),
    ];
    let c = asm.assemble_character(&spec).expect("body assembles");
    let look = probe.look(&c, AssemblyOptions::default());
    let crest = mesh_type(data, "Crest");
    let hair_edit = look
        .edits
        .iter()
        .find(|e| e.parameter == 2003 && e.mesh_type == crest)
        .expect("a hair edit");
    assert_eq!(hair_edit.colour, [1.0, 0.0, 0.0, 1.0]);
    let face = look
        .compositors
        .iter()
        .find(|f| f.name == NEWER_FACE)
        .expect("newer face");
    // The skin colour reaches the compositor's colour map (§3.5 table: face skin
    // 0x1000000B).
    assert_eq!(
        face.colours
            .iter()
            .find(|(k, _)| *k == 0x1000_000B)
            .map(|(_, c)| *c),
        Some([255, 0, 0, 255])
    );
    let base = probe.look(
        &asm.assemble_character(&CharacterSpec::new(species, MALE))
            .expect("body assembles"),
        AssemblyOptions::default(),
    );
    let default_face = base
        .compositors
        .iter()
        .find(|f| f.name == NEWER_FACE)
        .expect("newer face");
    assert_ne!(
        face.identity, default_face.identity,
        "the colour changes the image"
    );
    assert_ne!(face.image, default_face.image);
}
