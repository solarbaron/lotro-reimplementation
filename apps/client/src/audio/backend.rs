// SPDX-License-Identifier: GPL-3.0-or-later
//! Audio start-up: the backend chosen on the command line (decision 0019: the device for
//! windowed runs, falling back to the null backend; the null backend for `--headless` and
//! `--smoke`), and the volume settings: Tessera's own `[audio]` section, over an optional
//! read-only import of the original client's `[Sound]` section (`install-layout.md` §8.3),
//! over the defaults (`sound.md` §6.1, §6.6, §7).

use std::path::{Path, PathBuf};

use tessera_audio::engine::{
    AudioEngine, Backend, CHANNEL_COUNT, Channel, DeviceOptions, EngineConfig, MixerSettings,
};
use tessera_settings::AudioSettings;

/// Which audio backend to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendChoice {
    /// No audio at all (every request refused, `sound.md` §6.8 item 1).
    #[default]
    Off,
    /// Mixed against the wall clock and discarded (`--headless`, `--smoke`).
    Null,
    /// Mixed into memory by the client in real time (no device needed; for measurements).
    Offline,
    /// The default output device; fails when there is none.
    Device,
    /// The device if it opens, else the null backend (windowed default).
    Auto,
}

impl BackendChoice {
    /// Parses a `--audio` value.
    ///
    /// # Errors
    ///
    /// For an unknown name.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" | "none" => Ok(Self::Off),
            "null" => Ok(Self::Null),
            "offline" => Ok(Self::Offline),
            "device" => Ok(Self::Device),
            "auto" => Ok(Self::Auto),
            other => Err(format!(
                "unknown audio backend {other:?} (auto, device, null, offline, off)"
            )),
        }
    }

    /// The name, as `--audio` takes it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Null => "null",
            Self::Offline => "offline",
            Self::Device => "device",
            Self::Auto => "auto",
        }
    }
}

/// Where to import the original's `[Sound]` volumes from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PrefsChoice {
    /// Only if Tessera's `[audio] import_original = true`, from the install's settings
    /// folder.
    #[default]
    IfEnabled,
    /// From the install's settings folder (`install-layout.md` §8.1).
    Auto,
    /// From this file.
    File(PathBuf),
}

impl PrefsChoice {
    /// Parses a `--audio-prefs` value: `auto` or a file path.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        if s.trim().eq_ignore_ascii_case("auto") {
            Self::Auto
        } else {
            Self::File(PathBuf::from(s))
        }
    }
}

/// Where Tessera's own `[audio]` values come from.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SettingsChoice {
    /// The user's settings file (`tessera_settings::SettingsStore::open_default`:
    /// `$TESSERA_SETTINGS_FILE` or the platform's config folder). Windowed default.
    #[default]
    User,
    /// No settings file: every value unset (the defaults, or the imported original's).
    /// Headless default, so that a run does not depend on the user's settings.
    Defaults,
    /// These values (tests and tools).
    Values(AudioSettings),
    /// This settings file.
    File(PathBuf),
}

impl SettingsChoice {
    /// Parses an `--audio-settings` value: `user`, `default` or a settings-file path.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "user" => Self::User,
            "default" | "defaults" => Self::Defaults,
            _ => Self::File(PathBuf::from(s)),
        }
    }
}

/// Output rate of the null and offline backends.
pub const SOFT_RATE: u32 = 48_000;

/// The engine configuration of a backend choice.
#[must_use]
pub fn engine_config(choice: BackendChoice) -> EngineConfig {
    let backend = match choice {
        BackendChoice::Off => Backend::Disabled,
        BackendChoice::Null => Backend::Null {
            sample_rate: SOFT_RATE,
        },
        BackendChoice::Offline => Backend::Offline {
            sample_rate: SOFT_RATE,
        },
        BackendChoice::Device => Backend::Device(DeviceOptions::default()),
        BackendChoice::Auto => Backend::Auto(DeviceOptions::default()),
    };
    EngineConfig {
        backend,
        ..EngineConfig::default()
    }
}

/// Starts the engine for `choice`. A device that cannot be opened is logged and gives the
/// disabled backend (an explicit `device` choice); `auto` falls back to null by itself.
#[must_use]
pub fn start_engine(choice: BackendChoice) -> AudioEngine {
    match AudioEngine::new(engine_config(choice)) {
        Ok(e) => {
            tracing::info!(
                requested = choice.name(),
                backend = ?e.backend(),
                rate = e.sample_rate(),
                device = e.device_name().unwrap_or("-"),
                "audio engine started"
            );
            e
        }
        Err(err) => {
            tracing::error!("audio backend {}: {err}; audio is off", choice.name());
            AudioEngine::new(engine_config(BackendChoice::Off))
                .expect("the disabled backend needs no device")
        }
    }
}

/// Reads the `[Sound]` section of the original's preferences file at `path` (read-only).
///
/// # Errors
///
/// When the file cannot be read.
pub fn import_preferences(path: &Path) -> Result<AudioSettings, String> {
    let ini = tessera_install::Ini::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pairs: Vec<(&str, &str)> = ini
        .sections
        .iter()
        .filter(|s| s.name.eq_ignore_ascii_case("Sound"))
        .flat_map(|s| s.entries.iter().map(|e| (e.key.as_str(), e.value.as_str())))
        .collect();
    let (a, report) = AudioSettings::import_sound_section(pairs);
    for r in &report.rejected {
        tracing::warn!("audio preferences import: {r}");
    }
    tracing::info!(
        file = %path.display(),
        used = report.used,
        ignored = report.ignored,
        "imported the original's [Sound] values (read-only)"
    );
    Ok(a)
}

/// Most install-directory entries looked at for a launcher config.
const MAX_DIR_ENTRIES: usize = 4096;

/// The launcher config's `Product.DocumentFolder` for the install at `game_dir`
/// (`install-layout.md` §6, §8.1), read-only. The config is looked up in the order of P14:
/// `lotro.launcherconfig`, any other `*.launcherconfig` (by name), then
/// `TurbineLauncher.exe.config`, names compared case-insensitively; the first one that
/// reads decides. `None` when there is none or it has no such key.
#[must_use]
pub fn document_folder(game_dir: &Path) -> Option<String> {
    let mut names: Vec<(u8, String, PathBuf)> = std::fs::read_dir(game_dir)
        .ok()?
        .take(MAX_DIR_ENTRIES)
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            let lower = name.to_ascii_lowercase();
            let rank = if lower == "lotro.launcherconfig" {
                0
            } else if lower.len() > ".launcherconfig".len() && lower.ends_with(".launcherconfig") {
                1
            } else if lower == "turbinelauncher.exe.config" {
                2
            } else {
                return None;
            };
            Some((rank, name, e.path()))
        })
        .collect();
    names.sort();
    for (_, _, path) in names {
        match tessera_install::launcherconfig::read_launcher_config(&path) {
            Ok(cfg) => return cfg.document_folder().map(str::to_owned),
            Err(e) => tracing::debug!("audio: launcher config {}: {e}", path.display()),
        }
    }
    None
}

/// The original's preferences file for the install at `game_dir` (`install-layout.md`
/// §8.1, with the launcher config's `Product.DocumentFolder`), if one exists.
#[must_use]
pub fn find_preferences(game_dir: &Path) -> Option<PathBuf> {
    let install = tessera_install::GameInstall::open(game_dir).ok()?;
    let folder = document_folder(game_dir);
    let dir = install.settings_dir(
        &tessera_install::SettingsQuery {
            prefs_file: None,
            document_folder: folder.as_deref(),
        },
        &tessera_install::SearchEnv::from_process(),
    );
    dir.has_preferences.then_some(dir.preferences)
}

/// Tessera's `[audio]` settings from the user's settings file (never fails; problems are
/// logged). Reading may copy a damaged file to a backup next to it
/// (`tessera_settings::SettingsStore::load`); it never writes the game's files.
#[must_use]
pub fn tessera_audio_settings() -> AudioSettings {
    match tessera_settings::SettingsStore::open_default() {
        Ok(store) => audio_settings_from(&store),
        Err(_) => AudioSettings::default(),
    }
}

fn audio_settings_from(store: &tessera_settings::SettingsStore) -> AudioSettings {
    let loaded = store.load();
    let (a, warnings) = AudioSettings::from_settings(&loaded.settings);
    for w in warnings.iter().chain(&loaded.warnings) {
        tracing::warn!("settings: {w}");
    }
    a
}

/// Tessera's own `[audio]` values per `choice`.
#[must_use]
pub fn own_settings(choice: &SettingsChoice) -> AudioSettings {
    match choice {
        SettingsChoice::User => tessera_audio_settings(),
        SettingsChoice::Defaults => AudioSettings::default(),
        SettingsChoice::Values(v) => v.clone(),
        SettingsChoice::File(p) => {
            audio_settings_from(&tessera_settings::SettingsStore::new(p.clone()))
        }
    }
}

/// The settings in effect: Tessera's values `own`, then the imported ones per `prefs`.
#[must_use]
pub fn resolve_settings(
    own: AudioSettings,
    prefs: &PrefsChoice,
    game_dir: Option<&Path>,
) -> AudioSettings {
    let path = match prefs {
        PrefsChoice::File(p) => Some(p.clone()),
        PrefsChoice::Auto => game_dir.and_then(find_preferences),
        PrefsChoice::IfEnabled if own.import_original == Some(true) => {
            game_dir.and_then(find_preferences)
        }
        PrefsChoice::IfEnabled => None,
    };
    match path.map(|p| import_preferences(&p)) {
        Some(Ok(imported)) => own.or(&imported),
        Some(Err(e)) => {
            tracing::warn!("audio preferences import: {e}");
            own
        }
        None => own,
    }
}

/// The mixer settings for `settings`, with the channel labels of the data (`sound.md`
/// §7.1: "Implementations read the labels at run time and name their own settings after
/// them"); `labels[v]` is the label of channel value `v`.
#[must_use]
pub fn mixer_settings(settings: &AudioSettings, labels: &[String; CHANNEL_COUNT]) -> MixerSettings {
    let mut m = MixerSettings::default();
    if let Some(v) = settings.master_volume {
        m.master_volume = v;
    }
    for (i, label) in labels.iter().enumerate() {
        if let Some(v) = settings.volume_for(label) {
            m.channel_volumes[i] = v;
        }
    }
    if let Some(n) = settings.max_playing_sounds {
        m.max_playing_sounds = n;
    }
    if let Some(r) = settings.reverb {
        m.reverb_enabled = r;
    }
    if let Some(d) = settings.sound_disabled {
        m.sound_disabled = d;
    }
    if let Some(d) = settings.music_disabled {
        m.music_disabled = d;
    }
    m
}

/// The engine's own channel labels (`sound.md` §7.1), used when the data has none.
#[must_use]
pub fn default_labels() -> [String; CHANNEL_COUNT] {
    Channel::ALL.map(|c| c.label().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_parse() {
        assert_eq!(BackendChoice::parse("Device"), Ok(BackendChoice::Device));
        assert_eq!(BackendChoice::parse("none"), Ok(BackendChoice::Off));
        assert!(BackendChoice::parse("alsa").is_err());
        for c in [
            BackendChoice::Off,
            BackendChoice::Null,
            BackendChoice::Offline,
            BackendChoice::Device,
            BackendChoice::Auto,
        ] {
            assert_eq!(BackendChoice::parse(c.name()), Ok(c));
        }
        assert_eq!(PrefsChoice::parse("AUTO"), PrefsChoice::Auto);
        assert_eq!(
            PrefsChoice::parse("/x/UserPreferences.ini"),
            PrefsChoice::File("/x/UserPreferences.ini".into())
        );
    }

    #[test]
    fn soft_backends_start_without_a_device() {
        for c in [
            BackendChoice::Off,
            BackendChoice::Null,
            BackendChoice::Offline,
        ] {
            let e = start_engine(c);
            assert_eq!(e.stats().underruns, 0);
        }
    }

    #[test]
    fn settings_map_to_channels_by_label() {
        let mut s = AudioSettings {
            master_volume: Some(0.5),
            max_playing_sounds: Some(12),
            music_disabled: Some(true),
            ..AudioSettings::default()
        };
        s.volumes.insert("music".into(), 0.25);
        s.volumes.insert("Nonexistent".into(), 0.0);
        let m = mixer_settings(&s, &default_labels());
        assert_eq!(m.master_volume, 0.5);
        assert_eq!(m.channel_volumes[Channel::MUSIC.index()], 0.25);
        assert_eq!(m.channel_volumes[Channel::AMBIENT.index()], 1.0);
        assert_eq!(m.max_playing_sounds, 12);
        assert!(m.music_disabled && m.reverb_enabled && !m.sound_disabled);
    }

    #[test]
    fn preferences_import_reads_only_the_sound_section() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("UserPreferences.ini");
        std::fs::write(
            &p,
            "[Render]\r\nMusicVolume=0.10\r\n[Sound]\r\nMasterVolume=0.30\r\nMusicVolume=0.50\r\n",
        )
        .unwrap();
        let before = std::fs::read(&p).unwrap();
        let a = import_preferences(&p).unwrap();
        assert_eq!(a.master_volume, Some(0.3));
        assert_eq!(a.volume_for("Music"), Some(0.5));
        // Read-only: the file is unchanged.
        assert_eq!(std::fs::read(&p).unwrap(), before);
        assert!(import_preferences(&dir.path().join("missing.ini")).is_err());
    }

    fn launcher_config(folder: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<configuration><appSettings>\
             <add key=\"DataCenter.GameName\" value=\"LOTRO\" />\
             <add key=\"Product.DocumentFolder\" value=\"{folder}\" />\
             </appSettings></configuration>"
        )
    }

    #[test]
    fn the_document_folder_comes_from_the_launcher_config() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(document_folder(dir.path()), None);
        std::fs::write(
            dir.path().join("TurbineLauncher.exe.config"),
            launcher_config("Old Folder"),
        )
        .unwrap();
        assert_eq!(document_folder(dir.path()).as_deref(), Some("Old Folder"));
        std::fs::write(
            dir.path().join("zz.launcherconfig"),
            launcher_config("Other Folder"),
        )
        .unwrap();
        assert_eq!(document_folder(dir.path()).as_deref(), Some("Other Folder"));
        // `lotro.launcherconfig` first, its name compared case-insensitively.
        std::fs::write(
            dir.path().join("LOTRO.launcherconfig"),
            launcher_config("Test Server Folder"),
        )
        .unwrap();
        assert_eq!(
            document_folder(dir.path()).as_deref(),
            Some("Test Server Folder")
        );
        // An unreadable first candidate gives way to the next.
        std::fs::write(dir.path().join("LOTRO.launcherconfig"), "<configuration>").unwrap();
        assert_eq!(document_folder(dir.path()).as_deref(), Some("Other Folder"));
        assert_eq!(document_folder(&dir.path().join("missing")), None);
    }

    #[test]
    fn settings_choices() {
        assert_eq!(SettingsChoice::parse("user"), SettingsChoice::User);
        assert_eq!(SettingsChoice::parse("Default"), SettingsChoice::Defaults);
        assert_eq!(
            SettingsChoice::parse("/x/settings.toml"),
            SettingsChoice::File("/x/settings.toml".into())
        );
        assert_eq!(
            own_settings(&SettingsChoice::Defaults),
            AudioSettings::default()
        );
        let pinned = AudioSettings {
            master_volume: Some(0.25),
            ..AudioSettings::default()
        };
        assert_eq!(
            own_settings(&SettingsChoice::Values(pinned.clone())),
            pinned
        );
        // A settings file of its own: read, never the user's.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.toml");
        std::fs::write(&p, "[audio]\nmaster_volume = 0.5\nsound_disabled = true\n").unwrap();
        let a = own_settings(&SettingsChoice::File(p));
        assert_eq!(a.master_volume, Some(0.5));
        assert_eq!(a.sound_disabled, Some(true));
        // Pinned values with no import stay as they are.
        assert_eq!(
            resolve_settings(pinned.clone(), &PrefsChoice::IfEnabled, None),
            pinned
        );
    }
}
