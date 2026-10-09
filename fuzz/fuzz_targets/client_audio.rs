// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's world-audio inputs from untrusted data (`apps/client/src/audio/clock.rs`,
//! `location.rs` and `props.rs`, included by path so this crate does not pull in the
//! client), and the read-only import of the original's `[Sound]` section
//! (`tessera_settings`).
//!
//! Byte 0 selects the mode:
//! - 0: the game-time record decoder (`environment.md` §6); a decoded record's clock is
//!   run at a few times, which must stay in [0, 1).
//! - 1: an INI text (`install-layout.md` §8.3) through `tessera_install::Ini` and
//!   `AudioSettings::import_sound_section`: volumes stay in [0, 1], counts bounded.
//! - 2: the place rules (`sound.md` §10.3, §10.8) from arbitrary words: monitor volumes
//!   (§10.3.4) with arbitrary floats, two position reports (one from a walker position and
//!   cell, one raw) through the event tracker (§10.8.3), the cell source with an arbitrary
//!   cell set (§10.8.2), the landblock source from block-map pairs and a landblock-info set,
//!   the per-property resolution (§10.8.1), then the player properties and their writes.
//!   Checked: the event and `Volume_IsOutside` tables, the per-property precedence, the
//!   source caps, and that the location writes turn the old view into the new one.
//!
//! Nothing may panic.

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../apps/client/src/audio/clock.rs"]
mod clock;
#[allow(dead_code)]
#[path = "../../apps/client/src/audio/location.rs"]
mod location;
#[allow(dead_code)]
#[path = "../../apps/client/src/audio/props.rs"]
mod props;

use location::{
    EventTracker, LocPos, LocationSet, MAX_SOURCE_PROPERTIES, Sources, cell_contributes,
    cell_source, landblock_source, monitor_source, outside_value,
};
use props::{AreaMusic, AudioPids, MonitorVolume, PlaceInputs, PlayerAudioProps};
use tessera_formats::PropertyValue;
use tessera_formats::property::PropertyType;
use tessera_world::{LandblockKey, WorldPos};

/// PIDs are drawn from 0..PID_SPACE so that every source and the game-logic PIDs collide.
const PID_SPACE: u32 = 16;

/// Types handed to `diff_writes` (`typed_value` covers each).
const TYPES: [Option<PropertyType>; 9] = [
    None,
    Some(PropertyType::Bool),
    Some(PropertyType::TriState),
    Some(PropertyType::Int),
    Some(PropertyType::Int64),
    Some(PropertyType::Float),
    Some(PropertyType::Bitfield32),
    Some(PropertyType::Bitfield64),
    Some(PropertyType::DidRef),
];

/// The input as little-endian words; reads past the end give 0.
struct Words(Vec<u32>);

impl Words {
    fn u(&self, i: usize) -> u32 {
        self.0.get(i).copied().unwrap_or(0)
    }

    fn f(&self, i: usize) -> f32 {
        f32::from_bits(self.u(i))
    }

    /// `n` (PID, value) pairs from words `at..at + 2n`.
    fn pairs(&self, at: usize, n: usize) -> impl Iterator<Item = (u32, PropertyValue)> + '_ {
        (0..n).map(move |k| {
            let i = at + 2 * k;
            (self.u(i) % PID_SPACE, PropertyValue::Enum(self.u(i + 1)))
        })
    }

    fn set(&self, at: usize, n: usize) -> LocationSet {
        self.pairs(at, n).collect()
    }
}

// Word layout of mode 2.
const MONITORS: usize = 0; // 4 × 16: origin 3, rotation 4, scale 3, min 3, max 3
const POINT: usize = 64; // 3: the walker's position
const STEPS: usize = 67; // 2 × 2: (region | x << 8 | y << 16 | instance << 24, cell)
const MONITOR_SETS: usize = 71; // 4 × 2 pairs
const CELL_SET: usize = 87; // 4 pairs
const BLOCK_MAP: usize = 95; // 2 × 4 pairs, value None when the PID word's bit 31 is set
const INFO_SET: usize = 111; // 2 × 2 pairs
const GAME: usize = 119; // area flag, area value, dungeon, landmark, night, types, flags

fn monitor(w: &Words, k: usize) -> MonitorVolume {
    let o = MONITORS + k * 16;
    let v = |i: usize| w.f(o + i);
    MonitorVolume {
        origin: [f64::from(v(0)), f64::from(v(1)), f64::from(v(2))],
        rotation: [v(3), v(4), v(5), v(6)],
        scale: [v(7), v(8), v(9)],
        min: [v(10), v(11), v(12)],
        max: [v(13), v(14), v(15)],
        offset: [v(1), v(2), v(3)],
        scale_with_entity: v(4) > 0.0,
        values: Arc::new(w.set(MONITOR_SETS + k * 4, 2)),
    }
}

/// The position of step `s`: step 0 from the walker's position and cell
/// (`LocPos::from_walker`), step 1 raw.
fn step_pos(w: &Words, s: usize, at: [f64; 3]) -> LocPos {
    let a = w.u(STEPS + 2 * s);
    let [region, x, y, inst] = a.to_le_bytes();
    let cell = w.u(STEPS + 2 * s + 1) as u16;
    if s == 0 {
        let pos = WorldPos::new(region & 0x0F, at[0], at[1], at[2]);
        let cell = LandblockKey::new(region & 0x0F, x, y)
            .ok()
            .filter(|_| inst & 2 != 0)
            .map(|k| (k, cell));
        LocPos::from_walker((inst & 1 != 0).then_some(&pos), cell)
    } else {
        LocPos {
            region: region & 0x1F,
            x,
            y,
            cell,
            instance: u32::from(inst & 1),
        }
    }
}

fn place_rules(rest: &[u8]) {
    let w = Words(
        rest.as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect(),
    );
    let monitors: Vec<MonitorVolume> = (0..4).map(|k| monitor(&w, k)).collect();
    let pids = AudioPids {
        area_music: Some(1),
        ambient_music_type: Some(2),
        world_is_night: Some(3),
        volume_is_outside: Some(4),
        combat_violent: Some(5),
        is_the_player: Some(6),
        ..AudioPids::default()
    };
    let game_logic: Vec<u32> = pids.game_logic().into_iter().flatten().collect();
    let type_salt = w.u(GAME + 5);
    let types = |pid: u32| {
        let i = (pid ^ type_salt) as usize % TYPES.len();
        TYPES.get(i).copied().flatten()
    };
    let flags = w.u(GAME + 6);

    let mut tracker = EventTracker::default();
    let mut sources = Sources::default();
    let mut prev: Option<PlayerAudioProps> = None;
    for s in 0..2 {
        // Step 0 stands at the given point, step 1 near a monitor's origin.
        let at = if s == 0 {
            [w.f(POINT), w.f(POINT + 1), w.f(POINT + 2)].map(f64::from)
        } else {
            let m = monitors
                .get((flags >> 8) as usize % 4)
                .map_or([0.0; 3], |m| m.origin);
            [m[0] + 0.5, m[1] - 0.25, m[2] + f64::from(w.f(POINT))]
        };
        let pos = step_pos(&w, s, at);

        // §10.8.3: the first report fires both events; a landblock event is a cell event.
        let ev = tracker.report(pos);
        if s == 0 {
            assert!(ev.landblock && ev.cell);
        }
        assert!(!ev.landblock || ev.cell);
        assert_eq!(tracker.last(), Some(pos));

        if ev.landblock {
            let block_map = (0..4).map(|k| {
                let i = BLOCK_MAP + 8 * s + 2 * k;
                let pid = w.u(i);
                let v = (pid & 0x8000_0000 == 0).then(|| PropertyValue::Enum(w.u(i + 1)));
                (pid % PID_SPACE, v)
            });
            let lb = landblock_source(block_map, w.pairs(INFO_SET + 4 * s, 2));
            assert!(lb.len() <= MAX_SOURCE_PROPERTIES);
            sources.landblock = Arc::new(lb);
        }
        if ev.cell {
            let cell_props = w.set(CELL_SET, 4);
            let loaded = (flags >> (1 + s)) & 1 != 0;
            let c = cell_source(
                &pos,
                pids.volume_is_outside,
                PropertyValue::TriState,
                loaded.then_some(&cell_props),
            );
            // §10.8.2: the computed value always, the cell's set only inside a cell ≠ 1.
            let out = outside_value(&pos);
            assert!(matches!(out, 0 | 1 | 255));
            assert_eq!(out == 0, pos.in_cell());
            assert_eq!(c.get(&4), Some(&PropertyValue::TriState(out)));
            if !cell_contributes(&pos) || !loaded {
                assert_eq!(c.len(), 1);
            }
            sources.cell = Arc::new(c);
        }
        let mon = monitor_source(props::monitors_containing(&monitors, at));
        assert!(mon.len() <= MAX_SOURCE_PROPERTIES);
        sources.monitor = Arc::new(mon);

        // §10.8.1 rule 1: per property, the highest source that has it.
        let resolved = sources.resolve();
        let order = [
            &sources.monitor,
            &sources.load,
            &sources.cell,
            &sources.landblock,
            &sources.world,
        ];
        for (pid, v) in &resolved {
            let top = order.iter().find_map(|src| src.get(pid));
            assert_eq!(top, Some(v));
        }
        assert!(
            order
                .iter()
                .all(|src| src.keys().all(|k| resolved.contains_key(k)))
        );
        let outside_from_cell = !sources.monitor.contains_key(&4);

        let inputs = PlaceInputs {
            location: resolved,
            area: if w.u(GAME) & 1 != 0 {
                AreaMusic::Known((w.u(GAME) & 2 != 0).then_some(w.u(GAME + 1)))
            } else {
                AreaMusic::Pending
            },
            dungeon_music: (flags & 8 != 0).then_some(w.u(GAME + 2)),
            landmark_music: (flags & 16 != 0).then_some(w.u(GAME + 3)),
            night: ((flags >> (5 + s)) & 1 != 0).then_some(w.u(GAME + 4) & 1 != 0),
        };
        let a = props::player_props(&pids, inputs, prev.as_ref().and_then(|p| p.area_music));
        assert!(game_logic.iter().all(|p| !a.location.contains_key(p)));
        assert!(a.is_the_player && !a.combat_violent);
        if outside_from_cell {
            assert_eq!(a.outside(&pids), Some(outside_value(&pos)));
        }

        let writes = props::diff_writes(&pids, prev.as_ref(), &a, types);
        let old = prev
            .as_ref()
            .map(|p| p.location.clone())
            .unwrap_or_default();
        assert!(writes.len() <= old.len() + a.location.len() + game_logic.len());
        // The location writes turn the old view into the new one (rules 2 and 3).
        let mut view = old;
        for (pid, v) in writes.iter().filter(|(p, _)| !game_logic.contains(p)) {
            match v {
                Some(v) => {
                    view.insert(*pid, v.clone());
                }
                None => {
                    view.remove(pid);
                }
            }
        }
        assert_eq!(view, a.location);
        assert!(props::diff_writes(&pids, Some(&a), &a, types).is_empty());
        prev = Some(a);
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    match mode % 3 {
        0 => {
            let expected = rest.get(..4).map_or(clock::GAME_TIME_DID, |b| {
                u32::from_le_bytes([b[0], b[1], b[2], b[3]])
            });
            if let Ok(g) = clock::GameTime::parse(rest, expected) {
                for x in [0.0, -1.0e9, 1.0e12, 12_345.678, f64::MAX, f64::MIN] {
                    if let Some(t) = g.time_of_day(x) {
                        assert!((0.0..1.0).contains(&t), "{t}");
                        let _ = g.is_night(t);
                    }
                }
            }
        }
        1 => {
            let ini = tessera_install::Ini::from_bytes(rest);
            let pairs: Vec<(&str, &str)> = ini
                .sections
                .iter()
                .filter(|s| s.name.eq_ignore_ascii_case("Sound"))
                .flat_map(|s| s.entries.iter().map(|e| (e.key.as_str(), e.value.as_str())))
                .collect();
            let (a, r) = tessera_settings::AudioSettings::import_sound_section(pairs);
            assert!(a.volumes.len() <= tessera_settings::MAX_CHANNEL_VOLUMES);
            assert!(r.rejected.len() <= 16);
            for v in a.volumes.values().chain(a.master_volume.as_ref()) {
                assert!((0.0..=1.0).contains(v));
            }
            let mut s = tessera_settings::Settings::default();
            a.store(&mut s);
            let (back, _) = tessera_settings::AudioSettings::from_settings(&s);
            assert_eq!(back.volumes.len(), a.volumes.len());
        }
        _ => place_rules(rest),
    }
});
