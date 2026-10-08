// SPDX-License-Identifier: GPL-3.0-or-later
//! Channels, channel flags and volume settings (`docs/specs/sound.md` §7).

use std::fmt;

/// Number of channels: the nine values of the "SoundChannel" enum mapper (`sound.md` §7.1).
pub const CHANNEL_COUNT: usize = 9;

/// A sound channel, the `SoundScript_Channel` value 0–8 of a request (`sound.md` §7.1).
///
/// The labels below are the observed labels of the channel mapper. Setting names
/// (`Sound.<label>Volume`) are built from the labels the settings layer reads from the
/// mapper at run time (`sound.md` §7.1), not from [`Channel::label`]; the engine only needs
/// the numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Channel(u8);

impl Channel {
    /// 0, Default. Plays at the Default volume; filed under SFX (8) for channel
    /// bookkeeping (`sound.md` §7.2).
    pub const DEFAULT: Self = Self(0);
    /// 1, Combat.
    pub const COMBAT: Self = Self(1);
    /// 2, Music (flags 0x4 | 0x8).
    pub const MUSIC: Self = Self(2);
    /// 3, UI.
    pub const UI: Self = Self(3);
    /// 4, VO (flags 0x4 | 0x10: exclusive).
    pub const VO: Self = Self(4);
    /// 5, QuantizedPlayerMusic (flag 0x2).
    pub const QUANTIZED_PLAYER_MUSIC: Self = Self(5);
    /// 6, PlayerMusic.
    pub const PLAYER_MUSIC: Self = Self(6);
    /// 7, Ambient (flag 0x8).
    pub const AMBIENT: Self = Self(7);
    /// 8, SFX.
    pub const SFX: Self = Self(8);

    /// All channels in value order.
    pub const ALL: [Self; CHANNEL_COUNT] = [
        Self::DEFAULT,
        Self::COMBAT,
        Self::MUSIC,
        Self::UI,
        Self::VO,
        Self::QUANTIZED_PLAYER_MUSIC,
        Self::PLAYER_MUSIC,
        Self::AMBIENT,
        Self::SFX,
    ];

    /// The channel of enum value `value`, `None` outside 0–8.
    #[must_use]
    pub fn new(value: u32) -> Option<Self> {
        u8::try_from(value)
            .ok()
            .filter(|v| usize::from(*v) < CHANNEL_COUNT)
            .map(Self)
    }

    /// The channel of enum value `value`; values outside 0–8 (never in the data, `sound.md`
    /// §13 ND2) fall back to [`Channel::DEFAULT`] (Tessera's choice).
    #[must_use]
    pub fn new_lossy(value: u32) -> Self {
        Self::new(value).unwrap_or(Self::DEFAULT)
    }

    /// The enum value 0–8.
    #[must_use]
    pub fn value(self) -> u32 {
        u32::from(self.0)
    }

    /// Index into per-channel arrays.
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    /// The channel used for bookkeeping (exclusivity, suspension): channel 0 is filed
    /// under SFX (8), every other channel under itself (`sound.md` §7.2).
    #[must_use]
    pub fn bookkeeping(self) -> Self {
        if self == Self::DEFAULT {
            Self::SFX
        } else {
            self
        }
    }

    /// A fallback label for logs and [`fmt::Display`] (the observed mapper label). Setting
    /// names come from the mapper at run time (`sound.md` §7.1), never from this.
    #[must_use]
    pub fn label(self) -> &'static str {
        [
            "Default",
            "Combat",
            "Music",
            "UI",
            "VO",
            "QuantizedPlayerMusic",
            "PlayerMusic",
            "Ambient",
            "SFX",
        ][self.index()]
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.0, self.label())
    }
}

/// Channel flags (`sound.md` §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChannelFlags(pub u8);

impl ChannelFlags {
    /// 0x1: muted; gain 0, voices kept.
    pub const MUTE: u8 = 0x01;
    /// 0x2: started through a separate pool (Inferred: player-music synchronisation).
    /// Tessera plays such voices like any other (`instruments.md` §10.4 Q9, Decision).
    pub const POOL: u8 = 0x02;
    /// 0x4: read by the voice-limit pass (exact effect OPEN, `sound.md` §7.2); Tessera
    /// ignores it (§6.6 Decision).
    pub const LIMIT: u8 = 0x04;
    /// 0x8: paused and kept instead of stopped while sound is suspended.
    pub const PERSIST: u8 = 0x08;
    /// 0x10: exclusive, one voice at a time (§6.8 item 9).
    pub const EXCLUSIVE: u8 = 0x10;

    /// The flags the client sets at start-up for `channel` (`sound.md` §7.2).
    #[must_use]
    pub fn startup(channel: Channel) -> Self {
        Self(match channel {
            Channel::MUSIC => Self::LIMIT | Self::PERSIST,
            Channel::VO => Self::LIMIT | Self::EXCLUSIVE,
            Channel::AMBIENT => Self::PERSIST,
            Channel::QUANTIZED_PLAYER_MUSIC => Self::POOL,
            _ => 0,
        })
    }

    /// Whether every bit of `bits` is set.
    #[must_use]
    pub fn has(self, bits: u8) -> bool {
        self.0 & bits == bits
    }
}

/// Volume and limit settings (`sound.md` §6.6, §7). Volumes are linear factors in [0, 1]
/// (the engine clamps other values, NaN to 0); the defaults are 1.0 (master and every
/// channel) and 32 playing sounds.
#[derive(Debug, Clone, PartialEq)]
pub struct MixerSettings {
    /// "Sound.MasterVolume", *M* of §6.1.
    pub master_volume: f32,
    /// `Sound.<label>Volume` per channel, *V*(*c*) of §6.1.
    pub channel_volumes: [f32; CHANNEL_COUNT],
    /// "Sound.MaxPlayingSounds" (§6.6).
    pub max_playing_sounds: u32,
    /// "Sound.EAXEnabled": whether the reverb of §8 is applied.
    pub reverb_enabled: bool,
    /// "Sound.SoundDisabled": new requests get the code *disabled* (§6.8 item 7).
    pub sound_disabled: bool,
    /// "Sound.MusicDisabled": mutes channel 2 (§7.2 OPEN, Decision).
    pub music_disabled: bool,
}

impl MixerSettings {
    /// Default of "Sound.MaxPlayingSounds" (`sound.md` §6.6).
    pub const DEFAULT_MAX_PLAYING_SOUNDS: u32 = 32;
}

impl Default for MixerSettings {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            channel_volumes: [1.0; CHANNEL_COUNT],
            max_playing_sounds: Self::DEFAULT_MAX_PLAYING_SOUNDS,
            reverb_enabled: true,
            sound_disabled: false,
            music_disabled: false,
        }
    }
}

/// A volume setting made safe: NaN → 0, clamped to [0, 1] ("Volume settings are linear
/// factors in [0, 1]", `sound.md` §6.1). A value above 1 would flatten the distance law,
/// since min(*v*, *v* × *a* × *M* × *V*) reaches *v* before the source is close.
pub(crate) fn sane_volume(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_values() {
        assert_eq!(Channel::new(8), Some(Channel::SFX));
        assert_eq!(Channel::new(9), None);
        assert_eq!(Channel::new(u32::MAX), None);
        assert_eq!(Channel::new_lossy(300), Channel::DEFAULT);
        assert_eq!(Channel::DEFAULT.bookkeeping(), Channel::SFX);
        assert_eq!(Channel::VO.bookkeeping(), Channel::VO);
        assert_eq!(Channel::ALL.len(), CHANNEL_COUNT);
        assert_eq!(Channel::PLAYER_MUSIC.label(), "PlayerMusic");
    }

    #[test]
    fn volume_settings_are_clamped_to_unit_range() {
        assert_eq!(sane_volume(4.0), 1.0);
        assert_eq!(sane_volume(1.5), 1.0);
        assert_eq!(sane_volume(-1.0), 0.0);
        assert_eq!(sane_volume(f32::NAN), 0.0);
        assert_eq!(sane_volume(f32::INFINITY), 1.0);
        assert_eq!(sane_volume(0.3), 0.3);
    }

    #[test]
    fn startup_flags() {
        assert!(ChannelFlags::startup(Channel::VO).has(ChannelFlags::EXCLUSIVE));
        assert!(ChannelFlags::startup(Channel::MUSIC).has(ChannelFlags::PERSIST));
        assert!(ChannelFlags::startup(Channel::AMBIENT).has(ChannelFlags::PERSIST));
        assert!(!ChannelFlags::startup(Channel::AMBIENT).has(ChannelFlags::LIMIT));
        assert_eq!(ChannelFlags::startup(Channel::SFX), ChannelFlags(0));
    }
}
