// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera_wstate::character` (`docs/specs/player-character.md`): the typed views of the
//! player-character records on arbitrary property sets, and the phonetic-shift, name,
//! pip and level rules on arbitrary values. Nothing may panic.
//!
//! The first input byte selects what the rest is:
//!
//! - even: an embedded property set, decoded with a synthetic dictionary that defines
//!   every property the views read (`testkit::CHARACTER_PROPERTIES`); every view runs on
//!   it, and the generator view (if it reads) resolves its literal texts and generates
//!   names with a deterministic random source;
//! - odd: text. The bytes are split at invalid UTF-8 bytes (for example 0xFF) into shift entries of four strings each
//!   (end match, start match, two replacements); the compiled table is applied to the
//!   following pieces, and the remaining bytes feed an experience table and a pip.

#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use tessera_formats::{Did, StringInfo, StringSource};
use tessera_props::{PropertyRegistry, PropertySet, Value};
use tessera_wstate::character::{
    Allegiance, AllegianceControl, BankRepository, BankUpgradeSchedule, ClassCreationInfo,
    ClassPriority, ExperienceTable, LevelTableDirectory, NameGenerator, NameRuleControl,
    NationalityDirectory, NationalityRecord, Pip, PipControl, RaceDirectory, RaceRecord,
    RestedExperienceControl, ShiftEntry, ShiftTable, XpGlobals, allegiance_problems,
    default_character_properties, generate_name, mirror_base,
};
use tessera_wstate::testkit::character_dictionary;

fn registry() -> &'static PropertyRegistry {
    static R: OnceLock<PropertyRegistry> = OnceLock::new();
    R.get_or_init(|| PropertyRegistry::parse(&character_dictionary()).expect("dictionary"))
}

fn literal(info: &StringInfo) -> Option<String> {
    match &info.source {
        StringSource::Literal(u) => Some(u.to_string_lossy()),
        StringSource::Table { token, .. } => (token % 2 == 0).then(String::new),
    }
}

fn views(set: &PropertySet, reg: &PropertyRegistry) {
    let _ = RaceDirectory::from_properties(set, reg);
    if let Ok(r) = RaceRecord::from_properties(set, reg) {
        let _ = r.gender(0x1000);
    }
    let _ = NationalityDirectory::from_properties(set, reg);
    if let Ok(n) = NationalityRecord::from_properties(set, reg) {
        let _ = n.naming_sexes();
    }
    let _ = LevelTableDirectory::from_properties(set, reg);
    if let Ok(c) = ClassCreationInfo::from_properties(set, reg) {
        let _ = c.starting_items_for(1).count();
    }
    let _ = ClassCreationInfo::kit_inventory(set, reg);
    if let Ok(c) = ClassPriority::from_properties(set, reg) {
        let _ = c.display_order();
    }
    let _ = NameRuleControl::from_properties(set, reg);
    let _ = XpGlobals::from_properties(set, reg);
    let _ = RestedExperienceControl::from_properties(set, reg);
    let _ = PipControl::from_properties(set, reg);
    if let Ok(p) = Pip::from_properties(set, reg) {
        let v = Value::Int(-3);
        let _ = p.effective_max(|_| Some(&v));
        if let Some(t) = &p.threshold {
            let _ = t.state_position(p.home.unwrap_or(0));
            let _ = t.bands_are_consistent(p.max.unwrap_or(0));
        }
    }
    if let Ok(b) = BankRepository::from_properties(set, reg) {
        let _ = b.schedules();
    }
    if let Ok(s) = BankUpgradeSchedule::from_properties(set, reg) {
        let _ = s.is_consistent();
        let _ = s.final_capacity();
    }
    if let (Ok(c), Ok(a)) = (
        AllegianceControl::from_properties(set, reg),
        Allegiance::from_properties(set, reg),
    ) {
        let _ = allegiance_problems(&c, &[(Did(0x7000_0001), a)]);
    }
    let _ = default_character_properties(set, reg, 0x2000);
    for p in set.iter() {
        if let Some(n) = reg.name(p.pid) {
            let _ = mirror_base(n);
        }
    }
    if let Ok(g) = NameGenerator::from_properties(set, reg)
        && let Ok(text) = g.resolve(literal)
    {
        for t in text.shifts.values() {
            let _ = t.is_well_formed();
            let _ = t.canonical_entries();
        }
        let mut state = 7u32;
        for e in g.race_categories.iter().take(8) {
            let _ = generate_name(
                &g,
                &text,
                e.species,
                e.sex,
                |n| {
                    state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    state % n.max(1)
                },
                |s| s.len() > 3,
            );
        }
    }
}

fn text_mode(data: &[u8]) {
    let text = String::from_utf8_lossy(data);
    let pieces: Vec<&str> = text.split('\u{FFFD}').collect();
    let mut entries = Vec::new();
    for [e, s, l, r] in pieces.as_chunks::<4>().0.iter().take(64) {
        entries.push(ShiftEntry {
            end: (*e).to_owned(),
            start: (*s).to_owned(),
            left: (*l).to_owned(),
            right: (*r).to_owned(),
        });
    }
    let table = ShiftTable::compile(&entries);
    let _ = table.is_well_formed();
    assert_eq!(table.len(), entries.len());
    for w in pieces.windows(2).take(64) {
        if let [a, b] = w {
            let _ = table.join(a, b);
        }
    }
    let totals: Vec<u64> = data
        .as_chunks::<8>()
        .0
        .iter()
        .take(512)
        .map(|c| u64::from_le_bytes(*c))
        .collect();
    let xp = ExperienceTable { totals };
    let _ = xp.is_valid();
    let _ = xp.max_level();
    for c in data.as_chunks::<8>().0.iter().take(16) {
        let _ = xp.level_for(u64::from_le_bytes(*c));
    }
    let word = |i: usize| -> i32 {
        data.get(i..i + 4)
            .and_then(|b| b.try_into().ok())
            .map_or(0, i32::from_le_bytes)
    };
    let pip = Pip {
        min: Some(word(0)),
        max: Some(word(4)),
        max_modifier: Some(word(8).unsigned_abs()),
        ..Pip::default()
    };
    let v = Value::Int(word(12));
    let _ = pip.effective_max(|_| Some(&v));
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    if mode & 1 == 0 {
        let reg = registry();
        if let Ok(set) = PropertySet::parse_embedded(rest, Did(0x7900_0001), reg) {
            views(&set, reg);
        }
    } else {
        text_mode(rest);
    }
});
