// SPDX-License-Identifier: GPL-3.0-or-later
//! Player-music instruments (`docs/specs/instruments.md`; ROADMAP M6): the instrument
//! table, note → sample, the playback queue, sustain and release, the instance limit and
//! synchronised start.
//!
//! | Step | Spec | Here |
//! |---|---|---|
//! | Property IDs, enum keys and labels from the data | §3, §7.1 | [`MusicVocabulary`] |
//! | The two music scripts through the avatar's table closure | §4.1, §5.3 | [`MusicScripts::locate`] |
//! | Tree evaluation and the flattened instrument table | §5.5–§5.8, §12 | [`InstrumentTable`] |
//! | Packed note, velocity, note key | §5.1, §5.2, §7.1 | [`pack_note`], [`note_key`], [`note_volume`] |
//! | Queue order, late drop | §8.2, §8.3 | [`NoteQueue`] |
//! | Registry, cancel, instance limit, start and release, gates, loudness, position | §8.4–§10 | [`MusicPlayer`] |
//! | `/play … sync` and `/playstart` | `abc-dialect.md` §8.5, §11 | [`SyncGroup`] |
//! | Synchronised group start, late-start catch-up, withdrawal | §8.7 step 4, §8.8, §12 P4 Decision | [`SyncGroup::start_at`], [`head_skip`] |
//! | Start tolerance and its measurement | §8.9 | [`START_TOLERANCE`], [`MusicPlayer::set_schedule_log`], [`SyncStart::check_tolerance`] |
//!
//! Nothing hard-codes a DID, PID, enum key or offset of the data: names and labels are
//! looked up at load ([`MusicVocabulary::from_data`]), the scripts are found through the
//! avatar's closure and walked by the rules of §5.6, and every instrument fact (channel,
//! fire-and-forget, ramp, intensity, sound info, looping, distances) is read from the
//! records.
//!
//! # Time
//!
//! Every event time is seconds on the engine's audio clock ([`MusicPlayer::now`]). The
//! player hands each note-on and note-off to the engine with its exact frame
//! ([`crate::engine::SoundRequest::start_at`], [`crate::engine::AudioEngine::stop_at`]),
//! so notes start and stop with sample accuracy rather than on a frame tick (§12 Decision
//! 2). Pitch is the recorded sample's (one sample per note, no transposition, §7.2); the
//! playback-rate factor of §7.3 is applied by the engine's resampler. The release is the
//! engine's linear ramp over `RampOutTime` (§9.2, §12 Decision 3).
//!
//! ```
//! use tessera_audio::instruments::{note_key, note_volume, pack_note};
//!
//! // instruments.md §14.2: key 11, velocity 105 (`+f+`).
//! let w = pack_note(11, 105);
//! assert_eq!(w, 0x6900_000B);
//! assert_eq!(note_key(0x6900_170B), 11);
//! assert!((note_volume(w) - 105.0 / 127.0).abs() < 1e-6);
//! assert_eq!(note_volume(pack_note(11, 128)), 0.0);
//! ```

mod error;
mod note;
mod player;
mod queue;
mod sync;
mod table;
mod vocab;

pub use error::InstrumentError;
pub use note::{
    CharacterId, KEY_WRITE_MASK, LIVE_VELOCITY, MAX_VELOCITY, MusicEvent, MusicEventKind,
    VELOCITY_DIVISOR, note_key, note_volume, pack_note, velocity,
};
pub use player::{
    MAX_SCHEDULE_LOG, MusicPlayer, PlayerConfig, PlayerStats, SampleBank, SampleProvider,
    ScheduledEvent,
};
pub use queue::{Due, LATE_DROP_SECS, NoteQueue, TIE_TOLERANCE};
pub use sync::{
    CATCH_UP_SECS, MAX_WAITING, PartOrigin, PartTiming, QueueLimit, RefusedPart, START_TOLERANCE,
    SYNC_LEAD_SECS, StartedPart, SyncGroup, SyncStart, ToleranceCheck, WaitingPerformance,
    head_skip,
};
pub use table::{
    InstrumentTable, MusicScript, MusicScripts, NoteDraws, NoteSound, PathFactor, ScriptDef,
    TableIssue, TableLimits,
};
pub use vocab::{MusicVocabulary, NORMAL_SCRIPT_LABEL, PITCH_COUNT, QUANTIZED_SCRIPT_LABEL};
