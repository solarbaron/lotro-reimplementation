// SPDX-License-Identifier: GPL-3.0-or-later
//! M4 assembly semantics on arbitrary bytes (`docs/specs/appearance.md`, `setup.md` part 2):
//!
//! - an appearance record (0x20): block choice for modifiers and dye codes, selection
//!   counts, group priorities and the slot table fed with every block;
//! - a setup (0x04): typed attach map, holding locations, viewport frames, bone checks;
//! - a property set (with a total synthetic schema) read as appearance lists, an item
//!   (worn/held look, holding rule), NPC props and bone-slider definitions;
//! - the race table and an avatar body through a fetch function serving the input as every
//!   record.
//!
//! Nothing may panic; derived values must stay consistent with their sources.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_appearance_m4 -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::convert::Infallible;

use libfuzzer_sys::fuzz_target;
use tessera_formats::appearance::{AppearanceListPids, SlotTable};
use tessera_formats::avatar::{
    AvatarPids, BoneSliderDefinition, BoneSliderPids, RacePids, RaceTable, parse_slider_values,
    resolve_avatar_body, slider_positions, slider_value_pairs,
};
use tessera_formats::item::{
    HoldingRule, ItemPids, ItemVisual, WardrobeFallbacks, npc_cosmetic_inventory_visible,
    npc_held_items,
};
use tessera_formats::{AppearanceRecord, Did, PropertySet, PropertyType, Record, Setup};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

/// Names map to small PIDs whose types (`pid % 24`) suit them.
fn names(n: &str) -> Option<u32> {
    let h = n
        .bytes()
        .fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(u32::from(b)));
    Some(0x1000_0000 | (h & 0xFFFF))
}

fuzz_target!(|data: &[u8]| {
    if let Ok(a) = AppearanceRecord::parse(data) {
        let _ = a.first_unusual_field();
        let _ = a.key_table_matches_selection_counts();
        let mut slots = SlotTable::new();
        for (i, e) in a.entries.iter().enumerate() {
            let _ = e.selection_count();
            for m in [0.0f32, 0.01, 0.5, 1.0, -1.0, f32::NAN] {
                if let Some(b) = e.block_for_modifier(m) {
                    assert!(e.blocks.iter().any(|x| std::ptr::eq(x, b)));
                }
                let _ = e.dye_block(m);
            }
            for b in &e.blocks {
                let n = b.groups_with_priority().count();
                assert_eq!(n, b.groups.len());
                for g in 0..=b.groups.len() {
                    assert_eq!(b.group_priority(g).is_some(), g < b.groups.len());
                }
                let _ = b.groups_end();
                slots.offer_block(b, i as u32);
            }
        }
        assert!(
            slots.len()
                <= a.entries
                    .iter()
                    .flat_map(|e| &e.blocks)
                    .map(|b| b.parts.len())
                    .sum::<usize>()
        );
    }
    if let Ok(s) = Setup::parse(data) {
        let _ = s.first_unusual_field();
        for n in [0usize, 1, 62, 89, 256] {
            let _ = s.bone_issue(n);
        }
        assert!(s.holding_locations().count() <= s.bone_frames.len());
        for f in &s.bone_frames {
            let _ = s.holding_location(f.key);
        }
        for a in s.attachments() {
            assert!(s.bone_for(a.bone_type).is_some());
        }
        for f in &s.named_frames {
            assert!(s.viewport_frame(f.key).is_some());
        }
    }
    if let Ok(set) = PropertySet::parse(data, &total) {
        if let Ok(p) = AppearanceListPids::from_names(&names) {
            for e in p.read_lists(&set) {
                let _ = e.file_in(&set);
            }
        }
        if let Ok(p) = ItemPids::from_names(&names) {
            let item = ItemVisual::from_property_set(&set, &p);
            let fb = WardrobeFallbacks::from_species_labels(|l| names(l).map(|k| k & 0xFF));
            let _ = item.look(0x17, 0x1000, &fb);
            let rule = HoldingRule {
                slot_locations: vec![(0x1_0000, 1), (u64::MAX, 2)],
            };
            let _ = rule.location(&item, true);
            let _ = rule.location(&item, false);
        }
        let _ = npc_held_items(&set, &names);
        let _ = npc_cosmetic_inventory_visible(&set, &names);
        if let Ok(p) = BoneSliderPids::from_names(&names) {
            let defs = BoneSliderDefinition::read_all(&set, &p);
            for d in &defs {
                let _ = d.value_for(0x80);
                let _ = d.channel_bits();
            }
            assert_eq!(
                slider_positions(defs.len(), &[0x10, 0x20]).len(),
                defs.len()
            );
        }
    }
    if let Ok(s) = std::str::from_utf8(data) {
        if let Some(b) = parse_slider_values(s) {
            assert_eq!(b.len() * 2, s.len());
        }
        assert!(slider_value_pairs(s).len() <= s.len() / 2);
    }
    // Every DID serves the input: the walks must stop on cycles, limits and bad records.
    let fetch = |_: Did| Ok::<_, Infallible>(Some(data.to_vec()));
    if let Ok(p) = RacePids::from_names(&names) {
        let _ = RaceTable::read(Did(0x7000_0001), &total, &p, fetch);
    }
    if let Ok(p) = AvatarPids::from_names(&names) {
        let _ = resolve_avatar_body(Did(0x7000_0010), &total, &p, fetch);
        if let Ok(body) = resolve_avatar_body(Did(0x4700_0010), &total, &p, fetch) {
            let all = body.appearance_files();
            assert_eq!(
                all.len(),
                body.template_appearance.len() + body.class_list.len() + body.instance_list.len()
            );
            // An instance list of the placed figure replaces the entity's own.
            let own = body.class_list.clone();
            let placed = body.appearance_files_with_instance(Some(&own));
            assert_eq!(
                placed.len(),
                body.template_appearance.len() + 2 * body.class_list.len()
            );
        }
    }
});
