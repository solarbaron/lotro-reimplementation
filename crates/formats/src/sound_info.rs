// SPDX-License-Identifier: GPL-3.0-or-later
//! Sound-info records, DID prefix 0x2A (`sound.md` §4, §12.1).
//!
//! A sound info names a payload record (0x0A, [`crate::sound`]) and gives its playback
//! parameters: flags, priority and the distance range. Script nodes and properties always
//! reference sound infos, never payloads directly.

use std::fmt;

use crate::did::{Did, DidKind};
use crate::error::Result;
use crate::reader::Reader;
use crate::record::{Record, read_prologue};

/// Flags word of a sound info (`sound.md` §4.3). Unknown bits are kept.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SoundInfoFlags(pub u32);

impl SoundInfoFlags {
    /// 0x01: the voice loops (`sound.md` §6.4).
    pub const LOOP: u32 = 0x01;
    /// 0x02: selects voice mode 4 instead of 2 when the request does not choose a mode
    /// (effect beyond the voice limit OPEN).
    pub const VOICE_MODE_4: u32 = 0x02;
    /// 0x04: positional: distance law, panning from the source position and a reverb send.
    pub const POSITIONAL: u32 = 0x04;
    /// 0x10: not read by the playback code examined (meaning OPEN; short effects).
    pub const UNREAD_0X10: u32 = 0x10;
    /// 0x40: distance-attenuated without panning: distance law, centred, no reverb send.
    pub const ATTENUATED_CENTRED: u32 = 0x40;
    /// Every bit `sound.md` §4.3 names (invariant SI3).
    pub const KNOWN: u32 = Self::LOOP
        | Self::VOICE_MODE_4
        | Self::POSITIONAL
        | Self::UNREAD_0X10
        | Self::ATTENUATED_CENTRED;

    /// The stored word.
    #[must_use]
    pub fn bits(self) -> u32 {
        self.0
    }

    /// Whether every bit of `mask` is set.
    #[must_use]
    pub fn contains(self, mask: u32) -> bool {
        self.0 & mask == mask
    }

    /// 0x01: the voice loops.
    #[must_use]
    pub fn is_loop(self) -> bool {
        self.contains(Self::LOOP)
    }

    /// 0x02: voice mode 4 is selected when the request names no mode.
    #[must_use]
    pub fn selects_voice_mode_4(self) -> bool {
        self.contains(Self::VOICE_MODE_4)
    }

    /// 0x04: positional (distance law, panning, reverb send).
    #[must_use]
    pub fn is_positional(self) -> bool {
        self.contains(Self::POSITIONAL)
    }

    /// 0x10: the bit no playback code reads (kept, meaning OPEN).
    #[must_use]
    pub fn has_unread_0x10(self) -> bool {
        self.contains(Self::UNREAD_0X10)
    }

    /// 0x40: distance law without panning and without a reverb send.
    #[must_use]
    pub fn is_attenuated_centred(self) -> bool {
        self.contains(Self::ATTENUATED_CENTRED)
    }

    /// Whether the distance law of `sound.md` §6.2 applies (0x04 or 0x40).
    #[must_use]
    pub fn uses_distance_law(self) -> bool {
        self.0 & (Self::POSITIONAL | Self::ATTENUATED_CENTRED) != 0
    }

    /// Neither 0x04 nor 0x40: a 2D sound that ignores distance and plays centred.
    #[must_use]
    pub fn is_non_positional(self) -> bool {
        !self.uses_distance_law()
    }

    /// Bits outside [`SoundInfoFlags::KNOWN`] (none in the reference install, SI3).
    #[must_use]
    pub fn unknown_bits(self) -> u32 {
        self.0 & !Self::KNOWN
    }
}

impl fmt::Debug for SoundInfoFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SoundInfoFlags({:#04X})", self.0)
    }
}

/// A violation of the sound-info invariants SI1 (payload prefix), SI2 and SI3
/// (`sound.md` §13), reported by [`SoundInfo::validate`].
///
/// These are data anomalies for validation tools, not decode errors: [`SoundInfo::parse`]
/// accepts such records and keeps the stored values. Float fields hold the stored value.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum SoundInfoIssue {
    /// SI1: the payload DID does not have the payload prefix 0x0A.
    #[error("payload {0} is not a sound payload (0x0A) DID")]
    PayloadNotSound(Did),
    /// SI2: an unused float is not −1.0.
    #[error("unused floats are {unused_a} and {unused_b}, not -1 and -1")]
    UnusedFloats {
        /// The stored `unused_a`.
        unused_a: f32,
        /// The stored `unused_b`.
        unused_b: f32,
    },
    /// SI2: the priority is not in 0–1 (or is NaN).
    #[error("priority {0} is not in 0..=1")]
    Priority(f32),
    /// SI2: the maximum distance is not greater than 0 (or is NaN).
    #[error("maximum distance {0} is not greater than 0")]
    MaxDistance(f32),
    /// SI3: flag bits outside [`SoundInfoFlags::KNOWN`] are set (the unknown bits).
    #[error("unknown flag bits {0:#X}")]
    UnknownFlags(u32),
}

/// A sound-info record (prefix 0x2A; `sound.md` §4.1–§4.3).
///
/// Layout: DID, u32 flags, five f32 (`unused_a`, `unused_b`, `priority`, `min_distance`,
/// `max_distance`), the payload DID and a Pascal-string name; nothing follows. Stored values
/// are kept as they are: `min_distance` may exceed `max_distance` (17 records), and the
/// playback rules (`sound.md` §6) handle that.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundInfo {
    /// Record DID.
    pub did: Did,
    /// Flags (§4.3).
    pub flags: SoundInfoFlags,
    /// First unused float: read and kept, never used by playback (−1.0 in every record).
    pub unused_a: f32,
    /// Second unused float, as `unused_a` (−1.0 in every record).
    pub unused_b: f32,
    /// Voice-limit priority, 0–1: a positional voice scores (2 − `priority`) × distance, a
    /// non-positional one −1 (§4.2, §6.6).
    pub priority: f32,
    /// Metres: a sound under the distance law plays at full volume within this distance.
    pub min_distance: f32,
    /// Metres: a sound under the distance law is silent beyond this distance; a positional
    /// one-shot that would start beyond it is not started.
    pub max_distance: f32,
    /// The payload record to play (a 0x0A DID).
    pub payload: Did,
    /// Asset name: a tool label, Latin-1 (never shown to players).
    pub name: String,
}

impl SoundInfo {
    /// Bytes before the name: DID, flags, five f32, payload DID.
    pub const FIXED_SIZE: usize = 32;

    /// Defaults of a sound info that was never loaded (`sound.md` §4.1 constructor
    /// defaults): flags 0, unused floats −1, priority 0, distances 1 and 50, no payload, no
    /// name. They never occur in records.
    #[must_use]
    pub fn unloaded(did: Did) -> Self {
        Self {
            did,
            flags: SoundInfoFlags(0),
            unused_a: -1.0,
            unused_b: -1.0,
            priority: 0.0,
            min_distance: 1.0,
            max_distance: 50.0,
            payload: Did(0),
            name: String::new(),
        }
    }

    /// Flag 0x01: the voice loops.
    #[must_use]
    pub fn is_loop(&self) -> bool {
        self.flags.is_loop()
    }

    /// Flag 0x04: positional.
    #[must_use]
    pub fn is_positional(&self) -> bool {
        self.flags.is_positional()
    }

    /// Flags bits that `sound.md` §4.3 does not name (kept as stored).
    #[must_use]
    pub fn unknown_flags(&self) -> u32 {
        self.flags.unknown_bits()
    }

    /// Whether the payload DID has the payload prefix (0x0A).
    #[must_use]
    pub fn payload_is_sound(&self) -> bool {
        self.payload.kind() == DidKind::Sound
    }

    /// Checks the record-local invariants of `sound.md` §13 and returns the first
    /// violation, in this order: SI1's payload prefix (0x0A), SI2 (`unused_a` = `unused_b`
    /// = −1.0, 0 ≤ `priority` ≤ 1, `max_distance` > 0) and SI3 (no flag bits outside
    /// [`SoundInfoFlags::KNOWN`]).
    ///
    /// The rest of SI1 needs the archive and is the caller's: the exact decode is enforced
    /// by [`SoundInfo::parse`], and that the payload record exists and is audio (not
    /// DirectMusic) must be checked against the payload records.
    pub fn validate(&self) -> Result<(), SoundInfoIssue> {
        if !self.payload_is_sound() {
            return Err(SoundInfoIssue::PayloadNotSound(self.payload));
        }
        if self.unused_a != -1.0 || self.unused_b != -1.0 {
            return Err(SoundInfoIssue::UnusedFloats {
                unused_a: self.unused_a,
                unused_b: self.unused_b,
            });
        }
        if !(0.0..=1.0).contains(&self.priority) {
            return Err(SoundInfoIssue::Priority(self.priority));
        }
        if self.max_distance.is_nan() || self.max_distance <= 0.0 {
            return Err(SoundInfoIssue::MaxDistance(self.max_distance));
        }
        if self.unknown_flags() != 0 {
            return Err(SoundInfoIssue::UnknownFlags(self.unknown_flags()));
        }
        Ok(())
    }
}

impl Record<'_> for SoundInfo {
    const NAME: &'static str = "sound info";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::SoundInfo)?;
        let flags = SoundInfoFlags(r.u32()?);
        let unused_a = r.f32()?;
        let unused_b = r.f32()?;
        let priority = r.f32()?;
        let min_distance = r.f32()?;
        let max_distance = r.f32()?;
        let payload = r.did()?;
        let name = r.pascal_string()?;
        r.expect_end(Self::NAME)?;
        let info = Self {
            did,
            flags,
            unused_a,
            unused_b,
            priority,
            min_distance,
            max_distance,
            payload,
            name,
        };
        if info.unknown_flags() != 0 {
            tracing::debug!(
                did = %info.did,
                bits = info.unknown_flags(),
                "sound info with unknown flag bits"
            );
        }
        Ok(info)
    }
}
