// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's world-audio inputs from untrusted data (`apps/client/src/audio/clock.rs`
//! and `props.rs`, included by path so this crate does not pull in the client), and the
//! read-only import of the original's `[Sound]` section (`tessera_settings`).
//!
//! Byte 0 selects the mode:
//! - 0: the game-time record decoder (`environment.md` §6); a decoded record's clock is
//!   run at a few times, which must stay in [0, 1).
//! - 1: an INI text (`install-layout.md` §8.3) through `tessera_install::Ini` and
//!   `AudioSettings::import_sound_section`: volumes stay in [0, 1], counts bounded.
//! - 2: monitor volumes and the place rules (`sound.md` §10.3) from arbitrary floats.
//!
//! Nothing may panic.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../apps/client/src/audio/clock.rs"]
mod clock;
#[allow(dead_code)]
#[path = "../../apps/client/src/audio/props.rs"]
mod props;

use props::{AreaMusic, AudioPids, MonitorVolume, PlaceValues};
use tessera_formats::PropertyValue;

fn f32s(b: &[u8]) -> impl Iterator<Item = f32> + '_ {
    b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c))
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
        _ => {
            let f: Vec<f32> = f32s(rest).collect();
            let get = |i: usize| f.get(i).copied().unwrap_or(0.0);
            let values = PlaceValues {
                music_type: Some(PropertyValue::Enum(get(0).to_bits())),
                ..PlaceValues::default()
            };
            let monitors: Vec<MonitorVolume> = (0..4)
                .map(|k| {
                    let o = k * 16;
                    MonitorVolume {
                        origin: [
                            f64::from(get(o)),
                            f64::from(get(o + 1)),
                            f64::from(get(o + 2)),
                        ],
                        rotation: [get(o + 3), get(o + 4), get(o + 5), get(o + 6)],
                        scale: [get(o + 7), get(o + 8), get(o + 9)],
                        min: [get(o + 10), get(o + 11), get(o + 12)],
                        max: [get(o + 13), get(o + 14), get(o + 15)],
                        offset: [get(o + 1), get(o + 2), get(o + 3)],
                        scale_with_entity: get(o + 4) > 0.0,
                        values: values.clone(),
                    }
                })
                .collect();
            let p = [f64::from(get(64)), f64::from(get(65)), f64::from(get(66))];
            let m = props::monitor_at(&monitors, p).map(|m| m.values.clone());
            let inputs = props::PlaceInputs {
                block: PlaceValues::default(),
                cell: (get(67) > 0.0).then(PlaceValues::default),
                monitor: m,
                area: if get(68) > 0.0 {
                    AreaMusic::Known(Some(get(69).to_bits()))
                } else {
                    AreaMusic::Pending
                },
                dungeon_music: Some(get(70).to_bits()),
                landmark_music: None,
                night: Some(get(71) > 0.0),
            };
            let a = props::player_props(&inputs, None);
            let pids = AudioPids {
                area_music: Some(1),
                ambient_music_type: Some(2),
                world_is_night: Some(3),
                volume_is_outside: Some(4),
                ..AudioPids::default()
            };
            let w = props::diff_writes(&pids, None, &a, |_| None);
            assert!(w.len() <= 8);
            assert!(props::diff_writes(&pids, Some(&a), &a, |_| None).is_empty());
        }
    }
});
