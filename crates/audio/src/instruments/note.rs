// SPDX-License-Identifier: GPL-3.0-or-later
//! The packed note word (`instruments.md` §5.1, §5.2, §14.2) and the music event that
//! carries it (§5.1).

/// Divisor of the velocity: `Music_Note_Volume` = velocity / 127 (`instruments.md` §5.2).
pub const VELOCITY_DIVISOR: f32 = 127.0;

/// Highest velocity that is packed and played; a larger value counts as 0 (§5.1, §5.2).
pub const MAX_VELOCITY: u32 = 127;

/// Velocity of a live note (the live-play handler's value, `instruments.md` §5.1 I41, §12
/// sandbox Decision).
pub const LIVE_VELOCITY: u32 = 90;

/// Mask of the note key as the packers write it: 24 bits (§5.1).
pub const KEY_WRITE_MASK: u32 = 0x00FF_FFFF;

/// Packs a note key and a velocity the way both packers of the client do (§5.1 *Writing*):
/// the key masked to bits 0–23 and, if the velocity is at most 127, the velocity in bits
/// 24–31. A larger velocity is dropped (the word is the key alone, §14.2).
#[must_use]
pub fn pack_note(key: u32, velocity: u32) -> u32 {
    let mut w = key & KEY_WRITE_MASK;
    if velocity <= MAX_VELOCITY {
        w |= velocity << 24;
    }
    w
}

/// The note key the playback module reads from a packed word: bits 0–7 only, at note-on
/// (parameter and registry key) and at note-off (§5.1 *Reading*, §8.4, IN16).
#[must_use]
pub fn note_key(packed: u32) -> u8 {
    (packed & 0xFF) as u8
}

/// The velocity: bits 24–31 (§5.1).
#[must_use]
pub fn velocity(packed: u32) -> u32 {
    packed >> 24
}

/// `Music_Note_Volume`: velocity / 127, where a velocity above 127 counts as 0 (§5.2).
#[must_use]
pub fn note_volume(packed: u32) -> f32 {
    let v = velocity(packed);
    if v > MAX_VELOCITY {
        0.0
    } else {
        v as f32 / VELOCITY_DIVISOR
    }
}

/// Kind of a [`MusicEvent`] (§5.1: 1 note-on, 2 note-off), plus the end of a performance
/// (`abc-dialect.md` §9.4), which ends the character's performance (§8.5 Decision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MusicEventKind {
    /// Start a note.
    NoteOn,
    /// Stop a note.
    NoteOff,
    /// The performance of the character ends: its music is cancelled (§8.5 Decision).
    End,
}

/// An entity that plays music: the client's own id for a character (the player module
/// keeps no other identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CharacterId(pub u64);

impl std::fmt::Display for CharacterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "character#{}", self.0)
    }
}

/// A note event (`instruments.md` §5.1) on the audio clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MusicEvent {
    /// When the event is due: seconds on the audio clock (`AudioTime` / sample rate).
    pub time: f64,
    /// Creation order, used to order events with equal times (§8.2). The player assigns
    /// it; [`super::NoteQueue`] keeps what it is given.
    pub seq: u64,
    /// On, off or end.
    pub kind: MusicEventKind,
    /// The character that plays.
    pub character: CharacterId,
    /// The instrument type (enum `Music_Instrument_Type`, §3.1).
    pub instrument: u32,
    /// The packed note (§5.1).
    pub packed: u32,
    /// The quantized flag (§5.1): chooses the script (§5.3).
    pub quantized: bool,
    /// The from-song flag (§5.1): set for ABC performances.
    pub from_song: bool,
}

impl MusicEvent {
    /// The note key read from the packed word (bits 0–7).
    #[must_use]
    pub fn key(&self) -> u8 {
        note_key(self.packed)
    }

    /// `Music_Note_Volume` of the packed word.
    #[must_use]
    pub fn volume(&self) -> f32 {
        note_volume(self.packed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_14_2() {
        assert_eq!(pack_note(11, 105), 0x6900_000B);
        assert!((note_volume(0x6900_000B) - 105.0 / 127.0).abs() < 1e-6);
        assert_eq!(note_volume(0x8000_000B), 0.0);
        assert_eq!(pack_note(11, 128), 0x0000_000B);
        assert_eq!(note_volume(pack_note(11, 128)), 0.0);
        assert_eq!(note_key(0x6900_170B), 0x0B);
        assert_eq!(velocity(0x6900_170B), 105);
        assert_eq!(pack_note(0xFFFF_FFFF, 0), 0x00FF_FFFF);
        assert_eq!(note_key(pack_note(0x0123_4567, 90)), 0x67);
    }
}
