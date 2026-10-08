// SPDX-License-Identifier: GPL-3.0-or-later
//! Map-data decoders (`tessera_formats::gameplay::map`, `docs/specs/map-data.md`) on
//! arbitrary bytes: the named-places table (§3), the map-note table (§4) and the dungeon
//! quest-note table (§5), followed by the checks and views a reader runs on them (place
//! issues and lookups, validity, filing, the radar range test, kind recognition).
//!
//! Input: byte 0 picks the decoder (`% 3`); the rest is the payload. Properties are typed
//! with a total synthetic schema so that embedded sets and the `layers` property decode.
//!
//! Nothing may panic; a decoded table must re-find every key it holds, file every row
//! exactly once, keep the link block exactly on link rows and hold no embedded set with a
//! TSize hint above 22 (`property-system.md` §5.3 step 1).
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_gameplay_map -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::gameplay::map::{
    MAX_TSIZE_HINT, MapNotesTable, MapPids, PlacesTable, QuestNoteInfo, QuestNotesTable,
    classify_set, game_specific, position_flags_consistent, position_is_valid,
};
use tessera_formats::{Did, PropertyType};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

/// Every label of [`MapPids`] gets a small synthetic PID, so views find properties.
fn pids() -> MapPids {
    MapPids::resolve(&|label: &str| {
        MapPids::LABELS
            .iter()
            .position(|l| *l == label)
            .and_then(|i| u32::try_from(i + 1).ok())
    })
}

fuzz_target!(|input: &[u8]| {
    let [which, data @ ..] = input else {
        return;
    };
    let p = pids();
    match which % 3 {
        0 => {
            let Ok(t) = PlacesTable::parse(data) else {
                return;
            };
            let _ = t.issues();
            let _ = t.is_bucket_ordered();
            for (i, place) in t.iter().enumerate() {
                let found = t.find(place.name.units());
                assert!(found.is_some_and(|f| f <= i));
                let _ = position_is_valid(&place.position);
                let _ = position_flags_consistent(&place.position);
                let _ = place.zone();
            }
        }
        1 => {
            let Ok(t) = MapNotesTable::parse(data, &total) else {
                return;
            };
            let ix = t.index();
            let filed = ix.by_dungeon.values().map(Vec::len).sum::<usize>()
                + ix.by_landblock.values().map(Vec::len).sum::<usize>()
                + ix.ranged.len();
            assert_eq!(filed, t.len());
            for n in t.iter() {
                assert_eq!(n.link.is_some(), n.is_link());
                assert!(n.extra.size.hint <= MAX_TSIZE_HINT);
                let _ = n.validity(&|_: Did, token: u32| token.is_multiple_of(2));
                let _ = n.radar_shows(n.max_range * n.max_range);
                let _ = n.zone_link();
                let _ = n.base.layer_keys();
                let _ = n.type_keys().count();
                let _ = classify_set(&n.extra, &p);
                let _ = game_specific(&n.extra, &p);
            }
        }
        _ => {
            let Ok(t) = QuestNotesTable::parse(data, &total) else {
                return;
            };
            let groups = t.by_dungeon();
            assert!(groups.values().map(Vec::len).sum::<usize>() <= t.len());
            for n in &t.notes {
                assert!(n.quest_info.size.hint <= MAX_TSIZE_HINT);
                let _ = QuestNoteInfo::read(&n.quest_info, &p);
                let _ = classify_set(&n.quest_info, &p);
            }
        }
    }
});
