// SPDX-License-Identifier: GPL-3.0-or-later
//! The install loaders of `tessera-gamedata` over a **damaged synthetic install**:
//! `chargen::load_chargen`, `load_names`, `load_name_filter` (`player-character.md` §8,
//! `misc-records.md` §9), `inventory::load_inventory_control` and `load_paperdoll`
//! (`character-panels.md`, `ui-layout.md` §3.3 labels through `UiLabels::load`),
//! `chargen::read_controls`, the map-region list (`map::MapRegions::load` through
//! `InstallMapData`, `map-windows.md`) and the selector vocabulary that
//! `GameDataStore::object` and the record index load.
//!
//! The base install is the synthetic one of the crate's own tests
//! (`crates/gamedata/tests/common/mod.rs`: invented labels, PIDs, classes and DIDs, no
//! game data): its dictionary, class library and enum mappers, plus a race directory
//! with two race tables. The input adds up to eight objects whose property sets use any
//! label of that dictionary with a value of the label's type (structs and arrays nested
//! and bounded), names them with the labels the loaders look up, supplies the
//! name-filter table and the equipment-page layout as raw payloads, and starts with an
//! edit plan (`common/mutate.rs`) applied to the records in the order they are read:
//! damaged, cut, removed, replaced or failing once.
//!
//! Nothing may panic, loop without end or allocate without bound.

#![no_main]

#[path = "common/mutate.rs"]
mod mutate;

#[path = "../../crates/gamedata/tests/common/mod.rs"]
mod synth;

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use mutate::{InjectedFailure, Mutator, Read};
use synth::{
    C_AVATAR, C_CLASS_TABLE, C_ITEM_BASE, C_MOB, C_NPC, C_OTHER, C_SKILL, C_TRAIT, C_TRAIT_TREE,
    PROPS, Synth, enum_mapper, pid, v_array, v_bool, v_dids, v_struct, v_text, v_u32, v_u64,
};
use tessera_formats::Did;
use tessera_formats::ui_layout::UiEnum;
use tessera_gamedata::chargen::{
    AppearancePids, CreationChoice, load_chargen, load_name_filter, load_names, read_controls,
};
use tessera_gamedata::inventory::{
    INVENTORY_CONTROL_LABEL, load_inventory_control, load_paperdoll,
};
use tessera_gamedata::map::{InstallMapData, MapRegions, REGION_CONTROL_LABEL};
use tessera_gamedata::{Family, GameDataStore, IndexOptions};
use tessera_props::{MemorySource, RecordSource, SourceError};
use tessera_wstate::ObjectStore;
use tessera_wstate::testkit::did_map;

/// Object DIDs the input can fill (prefix 0x70, an object record).
const POOL: u32 = 0x7000_0E00;
const POOL_LEN: u32 = 8;
/// The base race directory and its tables.
const DIRECTORY: u32 = 0x7000_0C01;
const RACE_A: u32 = 0x7000_0C02;
const RACE_B: u32 = 0x7000_0C03;
/// Raw tables supplied by the input.
const NAME_FILTER: u32 = 0x7900_0001;
const PAPERDOLL_LAYOUT: u32 = 0x2200_0001;
/// UI enum mappers of the `EMAPPER` map (the first two form a base cycle).
const UI_MAPPER: u32 = 0x2300_0100;
const UI_MAPPER_BASE: u32 = 0x2300_0101;
/// DID maps of the synthetic root map.
const ROOT: u32 = 0x2800_0000;
const WEENIE_MAP: u32 = 0x2800_0001;
const UNIQUE_MAP: u32 = 0x2800_0002;
const EMAPPER_MAP: u32 = 0x2800_0003;
const UILAYOUT_MAP: u32 = 0x2800_0004;

/// WEENIECONTENT labels the loaders look up (interoperability names read from an install;
/// here they name synthetic records).
const WEENIE_LABELS: [&str; 7] = [
    "RaceTableDirectory",
    "NationalityTableDirectory",
    "LevelTableDirectory",
    "ChargenClassPriorityControl",
    "RandomNameGenerationDirectory",
    INVENTORY_CONTROL_LABEL,
    REGION_CONTROL_LABEL,
];

const CLASSES: [u32; 10] = [
    C_OTHER,
    C_ITEM_BASE,
    C_MOB,
    C_NPC,
    C_AVATAR,
    C_SKILL,
    C_CLASS_TABLE,
    C_TRAIT,
    C_TRAIT_TREE,
    0xFFFF,
];

/// Nesting limit of generated values.
const MAX_DEPTH: usize = 4;
/// Values per input.
const MAX_NODES: usize = 600;
/// Entries per generated set, struct or array.
const MAX_ENTRIES: u8 = 10;

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
    nodes: usize,
}

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos = self.pos.saturating_add(1);
        b
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    fn done(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn take(&mut self, n: usize) -> Vec<u8> {
        let start = self.pos.min(self.data.len());
        let end = start.saturating_add(n).min(self.data.len());
        self.pos = self.pos.saturating_add(n);
        self.data.get(start..end).unwrap_or_default().to_vec()
    }

    /// A DID: usually one of the pool or the base records.
    fn did(&mut self) -> u32 {
        match self.u8() % 4 {
            0 | 1 => POOL + u32::from(self.u8()) % (POOL_LEN + 1),
            2 => [DIRECTORY, RACE_A, RACE_B, NAME_FILTER, UI_MAPPER][usize::from(self.u8() % 5)],
            _ => self.u32(),
        }
    }

    fn label(&mut self) -> usize {
        usize::from(self.u8()) % PROPS.len()
    }

    /// A value of type `code` (`property-system.md` §4).
    fn value(&mut self, code: u32, depth: usize) -> Vec<u8> {
        self.nodes += 1;
        let deep = depth >= MAX_DEPTH || self.nodes > MAX_NODES || self.done();
        match code {
            11 if !deep => v_struct(&self.entries(depth + 1)),
            11 => v_struct(&[]),
            12 if !deep => {
                let n = self.u8() % MAX_ENTRIES;
                let mut elements = Vec::new();
                for _ in 0..n {
                    let (label, ty, _) = PROPS[self.label()];
                    elements.push((pid(label), self.value(ty, depth + 1)));
                }
                v_array(&elements)
            }
            12 => v_array(&[]),
            13 => {
                let n = usize::from(self.u8() % 8);
                let t: String = (0..n).map(|_| char::from(b'a' + self.u8() % 26)).collect();
                v_text(&t)
            }
            14 => v_u64(u64::from(self.u32()) << 32 | u64::from(self.u32())),
            20 => v_u32(self.did()),
            21 => v_bool(self.u8() & 1 == 1),
            8 | 18 => v_u32(u32::from(self.u8() % 32)),
            _ => v_u32(self.u32()),
        }
    }

    fn entries(&mut self, depth: usize) -> Vec<(u32, Vec<u8>)> {
        let n = self.u8() % MAX_ENTRIES;
        let mut out: Vec<(u32, Vec<u8>)> = Vec::new();
        for _ in 0..n {
            let (label, ty, _) = PROPS[self.label()];
            let p = pid(label);
            let v = self.value(ty, depth);
            if !out.iter().any(|(q, _)| *q == p) {
                out.push((p, v));
            }
        }
        out
    }
}

/// The fixed part of the install: the crate tests' base (dictionary, class library, enum
/// mappers), the race directory with its two tables, the UI mapper's base and the DID maps
/// that do not depend on the input. Built once per process (it is the same for every run)
/// and cloned per run, so a run only pays for what its input adds.
fn base() -> &'static (MemorySource, Vec<Did>) {
    static BASE: OnceLock<(MemorySource, Vec<Did>)> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut s = Synth::new();
        let gender = |sex: u32| v_struct(&[(pid("RaceTable_GenderType"), v_u32(sex))]);
        let genders = |sexes: &[u32]| {
            v_array(
                &sexes
                    .iter()
                    .map(|x| (pid("RaceTable_GenderStruct"), gender(*x)))
                    .collect::<Vec<_>>(),
            )
        };
        s.object(
            DIRECTORY,
            C_OTHER,
            Some(&[(
                "RaceTable_RaceTableList",
                v_dids("Vendor_Inventory", &[RACE_A, RACE_B, RACE_A, POOL]),
            )]),
        );
        s.object(
            RACE_A,
            C_OTHER,
            Some(&[
                ("RaceTable_Race", v_u32(23)),
                ("RaceTable_GenderList", genders(&[0x1000, 0x2000])),
            ]),
        );
        s.object(
            RACE_B,
            C_OTHER,
            Some(&[
                ("RaceTable_Race", v_u32(81)),
                ("RaceTable_GenderList", genders(&[0x2000])),
            ]),
        );
        s.raw(
            UI_MAPPER_BASE,
            enum_mapper(UI_MAPPER_BASE, UI_MAPPER, &[(4, "Back")]),
            false,
        );
        let emapper: Vec<(u32, u32, &str)> = UiEnum::ALL
            .iter()
            .enumerate()
            .map(|(i, e)| (i as u32 + 1, UI_MAPPER, e.emapper_label()))
            .collect();
        let mut src = s.src.clone();
        src.insert(
            UNIQUE_MAP,
            did_map(UNIQUE_MAP, &[(1, NAME_FILTER, "NameFilterTable")]),
        );
        src.insert(EMAPPER_MAP, did_map(EMAPPER_MAP, &emapper));
        src.insert(
            UILAYOUT_MAP,
            did_map(
                UILAYOUT_MAP,
                &[(1, PAPERDOLL_LAYOUT, "characterequipmentpage")],
            ),
        );
        src.insert(
            ROOT,
            did_map(
                ROOT,
                &[
                    (0x0A, WEENIE_MAP, "WEENIECONTENT"),
                    (0x0F, UNIQUE_MAP, "UNIQUEDB"),
                    (0x10, EMAPPER_MAP, "EMAPPER"),
                    (0x11, UILAYOUT_MAP, "UILAYOUT"),
                ],
            ),
        );
        (src, s.objects.clone())
    })
}

/// The base install plus the input's objects and tables.
fn install(b: &mut Bytes<'_>) -> (MemorySource, Vec<Did>) {
    let (base_src, base_objects) = base();
    let mut src = base_src.clone();
    let mut objects = base_objects.clone();

    // The input's objects.
    let mut weenie: Vec<(u32, u32, String)> = vec![(1, DIRECTORY, WEENIE_LABELS[0].to_string())];
    let count = usize::from(b.u8()) % (POOL_LEN as usize + 1);
    for i in 0..count {
        let did = POOL + i as u32;
        let class = CLASSES[usize::from(b.u8()) % CLASSES.len()];
        let entries = b.entries(0);
        let mut root = did.to_le_bytes().to_vec();
        root.extend_from_slice(&class.to_le_bytes());
        src.insert(did, root);
        objects.push(Did(did));
        let mut set = (did + 0x0900_0000).to_le_bytes().to_vec();
        set.extend_from_slice(&v_struct(&entries));
        src.insert(did + 0x0900_0000, set);
        let named = b.u8();
        if named & 0x80 != 0 {
            let label = WEENIE_LABELS[usize::from(named) % WEENIE_LABELS.len()];
            weenie.retain(|(_, _, l)| l != label);
            weenie.push((10 + i as u32, did, label.to_string()));
        }
    }

    // Raw tables: the name filter (UNIQUEDB) and the equipment-page layout (UILAYOUT).
    let filter_len = usize::from(b.u8());
    let mut filter = NAME_FILTER.to_le_bytes().to_vec();
    filter.extend(b.take(filter_len));
    src.insert(NAME_FILTER, filter);
    let layout_len = usize::from(b.u8()) * 2;
    let mut layout = PAPERDOLL_LAYOUT.to_le_bytes().to_vec();
    layout.extend(b.take(layout_len));
    src.insert(PAPERDOLL_LAYOUT, layout);
    // The UI element-ID mapper names the slot field; its base chain may loop.
    let base = if b.u8() & 1 == 0 { UI_MAPPER_BASE } else { 0 };
    src.insert(
        UI_MAPPER,
        enum_mapper(
            UI_MAPPER,
            base,
            &[
                (1, "CharacterEquipmentPage_SlotField"),
                (2, "Head"),
                (3, "Chest"),
            ],
        ),
    );

    let w: Vec<(u32, u32, &str)> = weenie
        .iter()
        .map(|(k, d, l)| (*k, *d, l.as_str()))
        .collect();
    src.insert(WEENIE_MAP, did_map(WEENIE_MAP, &w));
    (src, objects)
}

struct Damaged {
    base: MemorySource,
    mutator: Mutator,
}

impl RecordSource for Damaged {
    fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, SourceError> {
        let base = || self.base.read_record(did).ok().flatten().map(|b| (b, None));
        match self.mutator.read(did.0, base) {
            Read::Ok(r) => Ok(r.map(|(b, _)| b)),
            Read::Fail => Err(Box::new(InjectedFailure)),
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let (plan, rest) = mutate::plan(data);
    let mut b = Bytes {
        data: rest,
        pos: 0,
        nodes: 0,
    };
    let (base, objects) = install(&mut b);
    let pick = (b.u8(), b.u8(), b.u8());
    let store = GameDataStore::new(
        ObjectStore::new(Damaged {
            base,
            mutator: Mutator::new(plan),
        }),
        objects.clone(),
    )
    .with_options({
        let mut o = IndexOptions::default();
        o.threads = 1;
        o
    });
    let os = store.object_store();

    if let Ok(chargen) = load_chargen(os) {
        let _ = chargen.problems();
        for species in [23, 81, u32::from(pick.0)] {
            let _ = chargen.race(species);
            let _ = chargen.class_order(species);
            let _ = chargen.classes_for(species);
            let _ = chargen.nationalities_for(species);
            let _ = chargen.check(&CreationChoice {
                species,
                sex: [0x1000, 0x2000, u32::from(pick.1)][usize::from(pick.2 % 3)],
                nationality: u32::from(pick.1 % 4),
                class: u32::from(pick.2 % 32),
            });
        }
    }
    if let Ok(names) = load_names(os, |info| {
        (pick.0 & 1 == 0).then(|| format!("{info:?}").chars().take(16).collect())
    }) {
        let n = names.random_name(23, 0x1000, u64::from(pick.1));
        let _ = names.check(&n.name);
    }
    let _ = load_name_filter(os);
    let _ = load_inventory_control(os);
    let _ = load_paperdoll(os);

    if let Ok(registry) = os.game_data().registry() {
        let pids = AppearancePids::resolve(registry);
        for did in &objects {
            let _ = store.object(*did);
            if let Ok(Some(set)) = os.properties(*did) {
                let _ = read_controls(&set, &pids);
            }
        }
    }
    if let Ok(data) = InstallMapData::new(&store) {
        let _ = MapRegions::load(&data);
    }
    let _ = store.map_regions();
    if let Ok(index) = store.index() {
        let _ = index.len(Family::Item);
        let _ = index.failures_total();
    }
});
