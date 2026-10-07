// SPDX-License-Identifier: GPL-3.0-or-later
//! The `[audio]` section of Tessera's settings file, and a read-only importer of the
//! original client's `[Sound]` volumes.
//!
//! This is Tessera's own format (like the rest of this crate, the rustdoc is its
//! definition). The section lives in [`Settings::extra`] under the key [`SECTION_AUDIO`],
//! so it is additive: an older Tessera keeps it as an unknown table, and keys of the
//! section that this version does not know are preserved by [`AudioSettings::store`].
//!
//! ```toml
//! [audio]
//! master_volume = 0.8          # 0..=1, the "Sound.MasterVolume" of sound.md §6.1
//! max_playing_sounds = 32      # 1..=4096 (sound.md §6.6)
//! reverb = true                # apply reverb environments (sound.md §8)
//! sound_disabled = false
//! music_disabled = false       # mutes the Music channel (sound.md §7.2 Decision)
//! import_original = true       # fill unset values from the original's [Sound] section
//!
//! [audio.volumes]              # per channel, keyed by the channel's label in the data
//! Music = 0.5                  # (sound.md §7.1: "Default", "Combat", "Music", ...)
//! Ambient = 1.0
//! ```
//!
//! Every value is optional: an unset value means "the original's value if imported, else
//! the default" (1.0 for volumes, 32 playing sounds, reverb on, nothing disabled). Labels
//! are compared ASCII case-insensitively, because third-party launchers lower-case the
//! original's keys (`install-layout.md` §8.3).
//!
//! The importer ([`AudioSettings::import_sound_section`]) reads `key=value` pairs of the
//! original's `UserPreferences.ini` `[Sound]` section (`install-layout.md` §8.3: keys
//! `MasterVolume`, `<Label>Volume`, `MaxPlayingSounds`, `EAXEnabled`, `SoundDisabled`,
//! `MusicDisabled`). It never writes anything; Tessera never writes the original's files.

use std::collections::BTreeMap;

use crate::error::LoadWarning;
use crate::model::{MAX_WARNINGS_PER_SECTION, Settings};

/// Key of the section in the settings document.
pub const SECTION_AUDIO: &str = "audio";
/// Most per-channel volumes kept (the data defines nine channels, `sound.md` §7.1).
pub const MAX_CHANNEL_VOLUMES: usize = 64;
/// Longest channel label accepted, in bytes.
pub const MAX_CHANNEL_LABEL_LEN: usize = 64;
/// Largest `max_playing_sounds` accepted (the engine's voice-slot bound).
pub const MAX_PLAYING_SOUNDS_LIMIT: u32 = 4096;

const KEY_MASTER: &str = "master_volume";
const KEY_VOLUMES: &str = "volumes";
const KEY_MAX_PLAYING: &str = "max_playing_sounds";
const KEY_REVERB: &str = "reverb";
const KEY_SOUND_DISABLED: &str = "sound_disabled";
const KEY_MUSIC_DISABLED: &str = "music_disabled";
const KEY_IMPORT: &str = "import_original";
const AUDIO_KEYS: [&str; 7] = [
    KEY_MASTER,
    KEY_VOLUMES,
    KEY_MAX_PLAYING,
    KEY_REVERB,
    KEY_SOUND_DISABLED,
    KEY_MUSIC_DISABLED,
    KEY_IMPORT,
];

/// Tessera's audio settings (module docs). `None` means "not set".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioSettings {
    /// Master volume, 0..=1.
    pub master_volume: Option<f32>,
    /// Channel volumes by channel label (0..=1). At most [`MAX_CHANNEL_VOLUMES`] entries.
    pub volumes: BTreeMap<String, f32>,
    /// Most playing sounds, 1..=[`MAX_PLAYING_SOUNDS_LIMIT`].
    pub max_playing_sounds: Option<u32>,
    /// Whether reverb environments are applied.
    pub reverb: Option<bool>,
    /// Sound off.
    pub sound_disabled: Option<bool>,
    /// Music off.
    pub music_disabled: Option<bool>,
    /// Fill unset values from the original's `[Sound]` section.
    pub import_original: Option<bool>,
}

/// What [`AudioSettings::import_sound_section`] found besides the values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoundImport {
    /// Keys whose values could not be used (key and reason), at most
    /// [`MAX_WARNINGS_PER_SECTION`].
    pub rejected: Vec<String>,
    /// Keys read and used.
    pub used: usize,
    /// Keys of the section that are not volumes or switches Tessera uses (for example
    /// `FocusBehavior`, `SoundProvider`).
    pub ignored: usize,
}

/// A volume made safe: finite, clamped to 0..=1.
fn volume(v: f64) -> Option<f32> {
    v.is_finite().then(|| v.clamp(0.0, 1.0) as f32)
}

/// A volume as a TOML float with the shortest decimal that reads back as the same `f32`
/// (`0.8`, not `0.800000011920929`), so that the hand-edited file stays as typed.
fn float_value(v: f32) -> toml::Value {
    toml::Value::Float(v.to_string().parse::<f64>().unwrap_or(f64::from(v)))
}

/// Whether `label` can be a channel label key.
fn label_ok(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= MAX_CHANNEL_LABEL_LEN
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn number(v: &toml::Value) -> Option<f64> {
    match v {
        toml::Value::Float(f) => Some(*f),
        toml::Value::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

struct Warn<'a> {
    out: &'a mut Vec<LoadWarning>,
    count: usize,
}

impl Warn<'_> {
    fn entry(&mut self, message: impl FnOnce() -> String) {
        self.count += 1;
        if self.count <= MAX_WARNINGS_PER_SECTION {
            self.out.push(LoadWarning::Entry {
                section: SECTION_AUDIO.to_owned(),
                message: message(),
            });
        }
    }

    fn finish(self) {
        if self.count > MAX_WARNINGS_PER_SECTION {
            self.out.push(LoadWarning::Suppressed {
                section: SECTION_AUDIO.to_owned(),
                count: self.count - MAX_WARNINGS_PER_SECTION,
            });
        }
    }
}

fn short(key: &str) -> String {
    key.chars().filter(|c| !c.is_control()).take(64).collect()
}

impl AudioSettings {
    /// Reads the `[audio]` section of `settings` leniently: a section that is not a table
    /// is ignored, a bad value is ignored, and each problem is reported.
    #[must_use]
    pub fn from_settings(settings: &Settings) -> (Self, Vec<LoadWarning>) {
        let mut warnings = Vec::new();
        let mut out = Self::default();
        let Some(v) = settings.extra.get(SECTION_AUDIO) else {
            return (out, warnings);
        };
        let toml::Value::Table(t) = v else {
            warnings.push(LoadWarning::Section {
                section: SECTION_AUDIO.to_owned(),
                message: format!("expected a table, found {}", v.type_str()),
            });
            return (out, warnings);
        };
        let mut w = Warn {
            out: &mut warnings,
            count: 0,
        };
        if let Some(v) = t.get(KEY_MASTER) {
            match number(v).and_then(volume) {
                Some(x) => out.master_volume = Some(x),
                None => w.entry(|| format!("{KEY_MASTER}: expected a number in 0..=1")),
            }
        }
        if let Some(v) = t.get(KEY_MAX_PLAYING) {
            match v {
                toml::Value::Integer(i)
                    if (1..=i64::from(MAX_PLAYING_SOUNDS_LIMIT)).contains(i) =>
                {
                    out.max_playing_sounds = u32::try_from(*i).ok();
                }
                _ => w.entry(|| {
                    format!(
                        "{KEY_MAX_PLAYING}: expected an integer in 1..={MAX_PLAYING_SOUNDS_LIMIT}"
                    )
                }),
            }
        }
        for (key, slot) in [
            (KEY_REVERB, &mut out.reverb),
            (KEY_SOUND_DISABLED, &mut out.sound_disabled),
            (KEY_MUSIC_DISABLED, &mut out.music_disabled),
            (KEY_IMPORT, &mut out.import_original),
        ] {
            if let Some(v) = t.get(key) {
                match v {
                    toml::Value::Boolean(b) => *slot = Some(*b),
                    other => {
                        w.entry(|| format!("{key}: expected a boolean, found {}", other.type_str()))
                    }
                }
            }
        }
        if let Some(v) = t.get(KEY_VOLUMES) {
            match v {
                toml::Value::Table(vols) => {
                    for (label, v) in vols {
                        if !label_ok(label) {
                            w.entry(|| {
                                format!("{KEY_VOLUMES}.{}: not a channel label", short(label))
                            });
                            continue;
                        }
                        let Some(x) = number(v).and_then(volume) else {
                            w.entry(|| {
                                format!("{KEY_VOLUMES}.{label}: expected a number in 0..=1")
                            });
                            continue;
                        };
                        if out.volumes.len() >= MAX_CHANNEL_VOLUMES
                            && !out.volumes.contains_key(label)
                        {
                            w.entry(|| {
                                format!("{KEY_VOLUMES}: more than {MAX_CHANNEL_VOLUMES} labels")
                            });
                            break;
                        }
                        out.volumes.insert(label.clone(), x);
                    }
                }
                other => w.entry(|| {
                    format!(
                        "{KEY_VOLUMES}: expected a table, found {}",
                        other.type_str()
                    )
                }),
            }
        }
        w.finish();
        (out, warnings)
    }

    /// Writes these values into `settings`' `[audio]` table: the keys this version owns
    /// are set (or removed when unset), every other key of the table is kept. A
    /// `[audio]` value that is not a table is replaced.
    pub fn store(&self, settings: &mut Settings) {
        let mut table = match settings.extra.remove(SECTION_AUDIO) {
            Some(toml::Value::Table(t)) => t,
            _ => toml::Table::new(),
        };
        let mut put = |key: &str, v: Option<toml::Value>| match v {
            Some(v) => {
                table.insert(key.to_owned(), v);
            }
            None => {
                table.remove(key);
            }
        };
        put(
            KEY_MASTER,
            self.master_volume
                .and_then(|v| volume(f64::from(v)))
                .map(float_value),
        );
        put(
            KEY_MAX_PLAYING,
            self.max_playing_sounds
                .filter(|n| (1..=MAX_PLAYING_SOUNDS_LIMIT).contains(n))
                .map(|n| toml::Value::Integer(i64::from(n))),
        );
        put(KEY_REVERB, self.reverb.map(toml::Value::Boolean));
        put(
            KEY_SOUND_DISABLED,
            self.sound_disabled.map(toml::Value::Boolean),
        );
        put(
            KEY_MUSIC_DISABLED,
            self.music_disabled.map(toml::Value::Boolean),
        );
        put(KEY_IMPORT, self.import_original.map(toml::Value::Boolean));
        let vols: toml::Table = self
            .volumes
            .iter()
            .filter(|(l, _)| label_ok(l))
            .filter_map(|(l, v)| volume(f64::from(*v)).map(|v| (l.clone(), float_value(v))))
            .take(MAX_CHANNEL_VOLUMES)
            .collect();
        put(
            KEY_VOLUMES,
            (!vols.is_empty()).then_some(toml::Value::Table(vols)),
        );
        settings
            .extra
            .insert(SECTION_AUDIO.to_owned(), toml::Value::Table(table));
    }

    /// The volume set for `label` (ASCII case-insensitive).
    #[must_use]
    pub fn volume_for(&self, label: &str) -> Option<f32> {
        self.volumes
            .iter()
            .find(|(l, _)| l.eq_ignore_ascii_case(label))
            .map(|(_, v)| *v)
    }

    /// These values, with every unset one taken from `fallback` (per channel label for the
    /// volumes, compared case-insensitively).
    #[must_use]
    pub fn or(&self, fallback: &Self) -> Self {
        let mut volumes = self.volumes.clone();
        for (l, v) in &fallback.volumes {
            if volumes.len() >= MAX_CHANNEL_VOLUMES {
                break;
            }
            if !volumes.keys().any(|k| k.eq_ignore_ascii_case(l)) {
                volumes.insert(l.clone(), *v);
            }
        }
        Self {
            master_volume: self.master_volume.or(fallback.master_volume),
            volumes,
            max_playing_sounds: self.max_playing_sounds.or(fallback.max_playing_sounds),
            reverb: self.reverb.or(fallback.reverb),
            sound_disabled: self.sound_disabled.or(fallback.sound_disabled),
            music_disabled: self.music_disabled.or(fallback.music_disabled),
            import_original: self.import_original.or(fallback.import_original),
        }
    }

    /// Reads the original client's `[Sound]` section, given as `(key, value)` pairs in file
    /// order (a repeated key: the last wins; keys compared ASCII case-insensitively,
    /// `install-layout.md` §8.3). Read-only: the caller reads the file, nothing is written.
    ///
    /// - `MasterVolume` and every other `<Label>Volume` key: decimals, clamped to 0..=1.
    /// - `MaxPlayingSounds`: an integer in 1..=[`MAX_PLAYING_SOUNDS_LIMIT`].
    /// - `EAXEnabled` → [`Self::reverb`], `SoundDisabled`, `MusicDisabled`: `True`/`False`
    ///   (any case) or `1`/`0`.
    ///
    /// Other keys are counted as ignored; unusable values are listed in
    /// [`SoundImport::rejected`]. [`Self::import_original`] stays unset.
    pub fn import_sound_section<'a>(
        entries: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> (Self, SoundImport) {
        let mut out = Self::default();
        let mut report = SoundImport::default();
        let reject = |report: &mut SoundImport, key: &str, why: &str| {
            if report.rejected.len() < MAX_WARNINGS_PER_SECTION {
                report.rejected.push(format!("{}: {why}", short(key)));
            }
        };
        let parse_bool = |v: &str| -> Option<bool> {
            let v = v.trim();
            if v.eq_ignore_ascii_case("true") || v == "1" {
                Some(true)
            } else if v.eq_ignore_ascii_case("false") || v == "0" {
                Some(false)
            } else {
                None
            }
        };
        for (key, value) in entries {
            let key = key.trim();
            let value = value.trim();
            let is = |k: &str| key.eq_ignore_ascii_case(k);
            if is("MaxPlayingSounds") {
                match value.parse::<u32>() {
                    Ok(n) if (1..=MAX_PLAYING_SOUNDS_LIMIT).contains(&n) => {
                        out.max_playing_sounds = Some(n);
                        report.used += 1;
                    }
                    _ => reject(&mut report, key, "not an integer in range"),
                }
            } else if is("EAXEnabled") || is("SoundDisabled") || is("MusicDisabled") {
                match parse_bool(value) {
                    Some(b) => {
                        let slot = if is("EAXEnabled") {
                            &mut out.reverb
                        } else if is("SoundDisabled") {
                            &mut out.sound_disabled
                        } else {
                            &mut out.music_disabled
                        };
                        *slot = Some(b);
                        report.used += 1;
                    }
                    None => reject(&mut report, key, "not True or False"),
                }
            } else if key.len() > "Volume".len()
                && key
                    .get(key.len() - "Volume".len()..)
                    .is_some_and(|s| s.eq_ignore_ascii_case("Volume"))
            {
                let label = &key[..key.len() - "Volume".len()];
                let Some(v) = value.parse::<f64>().ok().and_then(volume) else {
                    reject(&mut report, key, "not a decimal");
                    continue;
                };
                if label.eq_ignore_ascii_case("Master") {
                    out.master_volume = Some(v);
                    report.used += 1;
                } else if !label_ok(label) {
                    reject(&mut report, key, "not a channel label");
                } else {
                    let existing = out
                        .volumes
                        .keys()
                        .find(|k| k.eq_ignore_ascii_case(label))
                        .cloned();
                    match existing {
                        Some(k) => {
                            out.volumes.insert(k, v);
                            report.used += 1;
                        }
                        None if out.volumes.len() < MAX_CHANNEL_VOLUMES => {
                            out.volumes.insert(label.to_owned(), v);
                            report.used += 1;
                        }
                        None => reject(&mut report, key, "too many channel volumes"),
                    }
                }
            } else {
                report.ignored += 1;
            }
        }
        (out, report)
    }
}

/// Keys of the `[audio]` table this version reads and writes.
#[must_use]
pub fn audio_keys() -> &'static [&'static str] {
    &AUDIO_KEYS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_unknown_keys() {
        let text = "[audio]\nmaster_volume = 0.5\nfuture_key = \"x\"\n[audio.volumes]\nMusic = 0.25\nAmbient = 2\n";
        let s = Settings::parse_lenient(text).settings;
        let (a, w) = AudioSettings::from_settings(&s);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(a.master_volume, Some(0.5));
        assert_eq!(a.volume_for("music"), Some(0.25));
        assert_eq!(a.volume_for("Ambient"), Some(1.0));
        let mut s2 = s.clone();
        let mut b = a.clone();
        b.reverb = Some(false);
        b.store(&mut s2);
        let text2 = s2.to_toml_string().unwrap();
        assert!(text2.contains("future_key"), "{text2}");
        let back = Settings::parse_lenient(&text2).settings;
        assert_eq!(AudioSettings::from_settings(&back).0, b);
    }

    #[test]
    fn bad_values_are_reported_and_ignored() {
        let text = "[audio]\nmaster_volume = \"loud\"\nmax_playing_sounds = 0\nreverb = 3\n[audio.volumes]\n\"bad label\" = 0.5\nMusic = nan\n";
        let s = Settings::parse_lenient(text).settings;
        let (a, w) = AudioSettings::from_settings(&s);
        assert_eq!(a, AudioSettings::default());
        assert_eq!(w.len(), 5, "{w:?}");
        let s = Settings::parse_lenient("audio = 5\n").settings;
        let (a, w) = AudioSettings::from_settings(&s);
        assert_eq!(a, AudioSettings::default());
        assert!(matches!(w.as_slice(), [LoadWarning::Section { .. }]));
    }

    #[test]
    fn import_reads_the_sound_section() {
        let entries = [
            ("MasterVolume", "0.30"),
            ("musicvolume", "0.50"),
            ("AmbientVolume", "1.70"),
            ("MaxPlayingSounds", "48"),
            ("EAXEnabled", "False"),
            ("SoundDisabled", "false"),
            ("FocusBehavior", "Mute Sounds"),
            ("SFXVolume", "x"),
            ("MusicVolume", "0.40"),
        ];
        let (a, r) = AudioSettings::import_sound_section(entries);
        assert_eq!(a.master_volume, Some(0.3));
        assert_eq!(a.volume_for("Music"), Some(0.4));
        assert_eq!(a.volumes.len(), 2);
        assert_eq!(a.volume_for("ambient"), Some(1.0));
        assert_eq!(a.max_playing_sounds, Some(48));
        assert_eq!(a.reverb, Some(false));
        assert_eq!(a.sound_disabled, Some(false));
        assert_eq!(r.ignored, 1);
        assert_eq!(r.rejected.len(), 1);
        let mine = AudioSettings {
            master_volume: Some(1.0),
            ..AudioSettings::default()
        };
        let merged = mine.or(&a);
        assert_eq!(merged.master_volume, Some(1.0));
        assert_eq!(merged.volume_for("MUSIC"), Some(0.4));
    }
}
