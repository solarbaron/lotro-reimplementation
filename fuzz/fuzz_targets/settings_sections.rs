// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-settings` sections that no other target reads from hostile documents: `[Map]`
//! (`MapPrefs`, `map-windows.md` §3.1, §3.4, §6.9, §8) and `[audio]` (`AudioSettings::
//! from_settings`; `client_audio` only reads back what the `[Sound]` importer stored). The
//! settings file is the user's, so every value is hostile.
//!
//! The input is (lossy) UTF-8 text read as a settings document. When byte 0 is odd, a
//! second document is built from the rest of the input: a `[Map]` and an `[audio]` section
//! with one value per key drawn from valid and invalid forms of every TOML type, so most
//! runs reach the value rules.
//!
//! Nothing may panic. What a reader keeps is in its documented range and bound (a kept
//! channel label is 1–`MAX_CHANNEL_LABEL_LEN` bytes of `[A-Za-z0-9_-]`); what it
//! stores reads back to the same preferences, in memory and through the written file text.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_settings::{
    AudioSettings, DISTANT_FILTER_KEY, MAP_COORDINATES_KEY, MAP_DISCOVERY_KEY, MAP_FULLSCREEN_KEY,
    MAP_SECTION, MAX_CHANNEL_LABEL_LEN, MAX_CHANNEL_VOLUMES, MAX_PLAYING_SOUNDS_LIMIT, MapPrefs,
    NEAR_FILTER_KEY, RADAR_FILTER_KEY, SECTION_AUDIO, Settings, audio_keys,
};

/// Values the synthetic sections draw from.
const VALUES: &[&str] = &[
    "true",
    "false",
    "\"on\"",
    "\"OFF\"",
    "\" yes \"",
    "\"maybe\"",
    "\"all\"",
    "\"None\"",
    "0",
    "-1",
    "1",
    "4096",
    "4097",
    "9223372036854775807",
    "0.5",
    "1.5",
    "-0.0",
    "nan",
    "inf",
    "\"0x00000000ffffffff\"",
    "\"0XFFFFFFFFFFFFFFFF\"",
    "\"0x10000000000000000\"",
    "\"0x\"",
    "[1, 2]",
    "{ Music = 0.5, Ambient = 2.0, \"bad label\" = 0.1, x = \"y\" }",
    "\"\"",
];

/// The `[Map]` keys (the crate's own names, so a renamed key is still reached) and one
/// unknown key.
const MAP_KEYS: &[&str] = &[
    MAP_FULLSCREEN_KEY,
    MAP_COORDINATES_KEY,
    MAP_DISCOVERY_KEY,
    NEAR_FILTER_KEY,
    DISTANT_FILTER_KEY,
    RADAR_FILTER_KEY,
    "unknown_key",
];

/// The `[audio]` keys (`audio_keys()`, the crate's own list) and one unknown key.
fn audio_section_keys() -> Vec<&'static str> {
    let mut keys = audio_keys().to_vec();
    keys.push("other");
    keys
}

/// A kept per-channel label: 1–`MAX_CHANNEL_LABEL_LEN` bytes of `[A-Za-z0-9_-]`.
fn label_ok(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= MAX_CHANNEL_LABEL_LEN
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A `[section]` with one value per key, chosen by the input bytes (bit 7 leaves it out).
fn section(name: &str, keys: &[&str], bytes: &mut impl Iterator<Item = u8>) -> String {
    let mut out = format!("[{name}]\n");
    for key in keys {
        let Some(b) = bytes.next() else { break };
        if b & 0x80 != 0 {
            continue;
        }
        let v = VALUES[usize::from(b) % VALUES.len()];
        out.push_str(&format!("{key} = {v}\n"));
    }
    out
}

fn audio_in_range(a: &AudioSettings) {
    assert!(
        a.volumes.len() <= MAX_CHANNEL_VOLUMES,
        "{} volumes",
        a.volumes.len()
    );
    for label in a.volumes.keys() {
        assert!(label_ok(label), "channel label {label:?} kept");
    }
    for v in a.volumes.values().chain(a.master_volume.as_ref()) {
        assert!((0.0..=1.0).contains(v), "volume {v}");
    }
    if let Some(n) = a.max_playing_sounds {
        assert!(
            (1..=MAX_PLAYING_SOUNDS_LIMIT).contains(&n),
            "max playing {n}"
        );
    }
}

fn check(text: &str) {
    let mut settings = Settings::parse_lenient(text).settings;

    let (map, _) = MapPrefs::from_settings(&settings);
    let (audio, _) = AudioSettings::from_settings(&settings);
    audio_in_range(&audio);

    map.store(&mut settings);
    audio.store(&mut settings);
    // Set keys are written valid; an unset key keeps whatever the file had, which reads as
    // unset again (with its warning).
    let (map_again, _) = MapPrefs::from_settings(&settings);
    assert_eq!(
        map_again, map,
        "the [Map] round trip changed the preferences"
    );
    let (audio_again, _) = AudioSettings::from_settings(&settings);
    assert_eq!(
        audio_again, audio,
        "the [audio] round trip changed the settings"
    );

    if let Ok(written) = settings.to_toml_string() {
        let back = Settings::parse_lenient(&written).settings;
        let (map_file, _) = MapPrefs::from_settings(&back);
        assert_eq!(map_file, map, "the [Map] section changed through the file");
        let (audio_file, _) = AudioSettings::from_settings(&back);
        assert_eq!(
            audio_file, audio,
            "the [audio] section changed through the file"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    check(&String::from_utf8_lossy(data));
    if let Some((&mode, rest)) = data.split_first()
        && mode & 1 == 1
    {
        let mut bytes = rest.iter().copied();
        let mut doc = section(MAP_SECTION, MAP_KEYS, &mut bytes);
        doc.push_str(&section(SECTION_AUDIO, &audio_section_keys(), &mut bytes));
        check(&doc);
    }
});
