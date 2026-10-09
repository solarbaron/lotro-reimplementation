// SPDX-License-Identifier: GPL-3.0-or-later
//! The `[audio]` section through the settings store, and the read-only import of the
//! original client's `[Sound]` values (synthetic files only).

use tessera_settings::{
    AudioSettings, LoadStatus, MAX_CHANNEL_VOLUMES, SECTION_AUDIO, Settings, SettingsStore,
};

#[test]
fn audio_section_survives_save_and_load_with_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    std::fs::write(
        &path,
        "schema_version = 1\n[audio]\nmaster_volume = 0.75\nnew_in_a_later_version = [1, 2]\n\
         [audio.volumes]\nMusic = 0.5\n[audio.extra_table]\nx = 1\n",
    )
    .unwrap();
    let store = SettingsStore::new(&path);
    let loaded = store.load();
    assert_eq!(loaded.status, LoadStatus::Clean);
    let (mut audio, warnings) = AudioSettings::from_settings(&loaded.settings);
    assert!(warnings.is_empty());
    assert_eq!(audio.master_volume, Some(0.75));
    audio.volumes.insert("Ambient".into(), 0.25);
    audio.max_playing_sounds = Some(40);
    let mut settings = loaded.settings;
    audio.store(&mut settings);
    store.save(&settings).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("new_in_a_later_version"), "{text}");
    assert!(text.contains("extra_table"), "{text}");
    let again = store.load();
    let (back, _) = AudioSettings::from_settings(&again.settings);
    assert_eq!(back, audio);
    // Removing a value removes its key and keeps the rest.
    let mut cleared = back.clone();
    cleared.master_volume = None;
    let mut s = again.settings;
    cleared.store(&mut s);
    let table = s
        .extra
        .get(SECTION_AUDIO)
        .and_then(|v| v.as_table())
        .unwrap();
    assert!(!table.contains_key("master_volume"));
    assert!(table.contains_key("new_in_a_later_version"));
}

#[test]
fn a_settings_file_without_audio_gives_unset_values() {
    let (a, w) = AudioSettings::from_settings(&Settings::default());
    assert!(w.is_empty());
    assert_eq!(a, AudioSettings::default());
}

#[test]
fn too_many_labels_are_bounded() {
    let mut text = String::from("[audio.volumes]\n");
    for i in 0..(MAX_CHANNEL_VOLUMES + 10) {
        text.push_str(&format!("L{i} = 0.5\n"));
    }
    let s = Settings::parse_lenient(&text).settings;
    let (a, w) = AudioSettings::from_settings(&s);
    assert_eq!(a.volumes.len(), MAX_CHANNEL_VOLUMES);
    assert_eq!(w.len(), 1);
}

#[test]
fn import_of_a_preferences_like_section() {
    // As in `install-layout.md` §8.3: CRLF files, decimals with two places, True/False,
    // keys in any case (the caller hands over the [Sound] pairs in file order).
    let pairs = vec![
        ("AmbientVolume", "0.80"),
        ("CombatVolume", "1.00"),
        ("DefaultVolume", "1.00"),
        ("EAXEnabled", "True"),
        ("FocusBehavior", "Mute Sounds"),
        ("MasterVolume", "0.30"),
        ("MaxPlayingSounds", "32"),
        ("MusicDisabled", "False"),
        ("MusicVolume", "0.45"),
        ("PersistentSound", "True"),
        ("PlayerMusicVolume", "1.00"),
        ("QuantizedPlayerMusicVolume", "1.00"),
        ("SFXVolume", "1.00"),
        ("SoundDisabled", "False"),
        ("SoundFeatures", "x y z"),
        ("SoundProvider", "OpenAL"),
        ("UIVolume", "0.50"),
        ("VOVolume", "1.00"),
        ("vovolume", "0.90"),
    ];
    let (a, r) = AudioSettings::import_sound_section(pairs);
    assert_eq!(a.master_volume, Some(0.3));
    assert_eq!(a.volumes.len(), 9);
    assert_eq!(a.volume_for("VO"), Some(0.9));
    assert_eq!(a.volume_for("QuantizedPlayerMusic"), Some(1.0));
    assert_eq!(a.reverb, Some(true));
    assert_eq!(a.music_disabled, Some(false));
    assert_eq!(a.max_playing_sounds, Some(32));
    assert_eq!(r.ignored, 4);
    assert!(r.rejected.is_empty());
    assert_eq!(a.import_original, None);
    // Hostile values never panic and stay in range.
    let (b, r) = AudioSettings::import_sound_section([
        ("MasterVolume", "inf"),
        ("Volume", "0.5"),
        ("X Y Volume", "0.5"),
        ("MaxPlayingSounds", "99999999999"),
        ("EAXEnabled", "maybe"),
        ("MusicVolume", "-3"),
    ]);
    assert_eq!(b.master_volume, None);
    assert_eq!(b.volume_for("Music"), Some(0.0));
    assert_eq!(b.max_playing_sounds, None);
    assert_eq!(r.rejected.len(), 4);
}

#[test]
fn stored_volumes_are_written_as_typed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    let store = SettingsStore::new(&path);
    let mut audio = AudioSettings {
        master_volume: Some(0.8),
        ..AudioSettings::default()
    };
    audio.volumes.insert("Music".into(), 0.3);
    audio.volumes.insert("Ambient".into(), 1.0);
    let mut settings = Settings::default();
    audio.store(&mut settings);
    store.save(&settings).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("master_volume = 0.8\n"), "{text}");
    assert!(text.contains("Music = 0.3\n"), "{text}");
    assert!(text.contains("Ambient = 1.0\n"), "{text}");
    assert!(!text.contains("0.800000"), "{text}");
    let (back, _) = AudioSettings::from_settings(&store.load().settings);
    assert_eq!(back, audio);
}
