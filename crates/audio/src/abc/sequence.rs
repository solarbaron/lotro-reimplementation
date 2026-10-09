// SPDX-License-Identifier: GPL-3.0-or-later
//! The sequencer: durations and limits, the time line, note events, ordering and
//! re-attacks (`abc-dialect.md` §8–§9).

use super::chars::Position;
use super::error::{AbcError, AbcErrorKind as K};
use super::reader::{Item, NoteRec, Song};
use super::{MAX_DURATION, MIN_DURATION, ORDER_TOLERANCE};

/// Lowest pitch the reader accepts: ABC `C,` (§7.3).
pub const PITCH_MIN: u8 = 48;
/// Highest pitch the reader accepts: ABC `c'` (§7.3).
pub const PITCH_MAX: u8 = 84;

/// The client's fixed pitch-slot table for pitches 48–84 (`abc-dialect.md` §9.5). Each slot
/// is the key of the note label `Note_<name><octave>` in the note enum mapper
/// (`instruments.md` §3.2, §7.1).
pub const PITCH_SLOTS: [u8; 37] = [
    28, 12, 2, 1, 21, 31, 30, 5, 20, 9, 17, 16, // 48–59
    29, 14, 4, 3, 25, 34, 33, 8, 24, 11, 19, 18, // 60–71
    32, 15, 7, 6, 27, 37, 36, 10, 26, 13, 23, 22, // 72–83
    35, // 84
];

/// The pitch slot of a pitch (§9.5), or `None` outside 48–84.
pub fn pitch_slot(pitch: u8) -> Option<u8> {
    let i = pitch.checked_sub(PITCH_MIN)?;
    PITCH_SLOTS.get(usize::from(i)).copied()
}

/// The note label of a pitch, `Note_` + name + octave (§9.5; `instruments.md` §7.1), for
/// looking the slot up in the user's note enum mapper at run time; `None` outside 48–84.
pub fn note_label(pitch: u8) -> Option<String> {
    const NAMES: [&str; 12] = [
        "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
    ];
    if !(PITCH_MIN..=PITCH_MAX).contains(&pitch) {
        return None;
    }
    let i = usize::from(pitch - PITCH_MIN);
    Some(format!("Note_{}{}", NAMES[i % 12], 2 + i / 12))
}

/// Packs a slot and a volume into the word handed to the sound system (§9.4;
/// `instruments.md` §5.1): slot in bits 0–23, volume in bits 24–31 when it is at most 127
/// (otherwise bits 24–31 stay 0).
pub fn pack_note(slot: u8, volume: u8) -> u32 {
    let mut w = u32::from(slot) & 0x00FF_FFFF;
    if volume <= 127 {
        w |= u32::from(volume) << 24;
    }
    w
}

/// The duration in seconds of a length under the part-wide timing values, in exactly the
/// operation order of §8.2: *den* × *L*, × *len*, × 60, ÷ *bpm*, each step rounded to a
/// double.
pub fn duration_seconds(meter_den: u32, unit: f64, len: f64, tempo: u32) -> f64 {
    let a = f64::from(meter_den) * unit;
    let b = a * len;
    let c = b * 60.0;
    c / f64::from(tempo)
}

/// One note or rest of the part with its start and duration (§8.4), in reading order.
/// Chord entries dropped as duplicates are not listed; tie continuations are listed with
/// their own start and duration.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledNote {
    /// Start on the time line, seconds (may be negative after an empty chord).
    pub start: f64,
    /// Duration, seconds (§8.2).
    pub duration: f64,
    /// Pitch 48–84, or `None` for a rest.
    pub pitch: Option<u8>,
    /// Volume value (§7.11).
    pub volume: u8,
    /// Tied to a later note of the same pitch (§7.6).
    pub tied: bool,
    /// Part of a chord.
    pub in_chord: bool,
    /// Index of the part event (single note, rest or chord, §7.12) this note belongs to,
    /// counting every event of the part including empty chords.
    pub event: usize,
    /// Position of the note's letter.
    pub position: Position,
}

/// Kind of a note event (§9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteEventKind {
    /// Start a note.
    On,
    /// Stop a note.
    Off,
    /// End of the performance (one, last, delta 0).
    End,
}

/// One event of the performance (§9.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteEvent {
    /// Time on the time line, seconds from the performance start (before the start offset
    /// of §8.5). Can be negative after an empty chord.
    pub time: f64,
    /// Seconds since the previous event (the first counts from 0); 0 for the end event.
    pub delta: f64,
    /// On, off or end.
    pub kind: NoteEventKind,
    /// Pitch 48–84 (0 for the end event).
    pub pitch: u8,
    /// Pitch slot 1–37 (§9.5; 0 for the end event).
    pub slot: u8,
    /// Volume value 30–127 (§7.11; 0 for the end event).
    pub volume: u8,
}

impl NoteEvent {
    /// The packed slot/volume word of this event ([`pack_note`]).
    pub fn packed(&self) -> u32 {
        pack_note(self.slot, self.volume)
    }
}

#[derive(Debug, Clone, Copy)]
struct RawEvent {
    time: f64,
    seq: usize,
    on: bool,
    pitch: u8,
    volume: u8,
}

/// A pending tie per pitch: the index of the pending note-off and the tied note's position.
#[derive(Debug, Clone, Copy)]
struct PendingTie {
    event: usize,
    pos: Position,
}

/// Durations, checks, time line and events for a read part.
pub(crate) fn sequence(song: &Song) -> Result<(Vec<ScheduledNote>, Vec<NoteEvent>), AbcError> {
    let dur = |len: f64| duration_seconds(song.meter_den, song.unit, len, song.tempo);
    let check = |d: f64, pos: Position| -> Result<(), AbcError> {
        if d.is_nan() || d > MAX_DURATION {
            Err(AbcError::at(K::NoteTooLong, pos))
        } else if d < MIN_DURATION {
            Err(AbcError::at(K::NoteTooShort, pos))
        } else {
            Ok(())
        }
    };
    let mut notes = Vec::new();
    let mut raw: Vec<RawEvent> = Vec::new();
    let mut ties: [Option<PendingTie>; 37] = [None; 37];
    let mut t = 0.0f64;
    let mut sound = |n: &NoteRec, start: f64, d: f64, raw: &mut Vec<RawEvent>| {
        let Some(pitch) = n.pitch else { return };
        let Some(slot_i) = pitch.checked_sub(PITCH_MIN).map(usize::from) else {
            return;
        };
        let Some(tie) = ties.get_mut(slot_i) else {
            return;
        };
        match *tie {
            Some(p) => {
                if let Some(e) = raw.get_mut(p.event) {
                    e.time += d;
                }
                *tie = if n.tied {
                    Some(PendingTie {
                        event: p.event,
                        pos: n.pos,
                    })
                } else {
                    None
                };
            }
            None => {
                let seq = raw.len();
                raw.push(RawEvent {
                    time: start,
                    seq,
                    on: true,
                    pitch,
                    volume: n.volume,
                });
                raw.push(RawEvent {
                    time: start + d,
                    seq: seq + 1,
                    on: false,
                    pitch,
                    volume: n.volume,
                });
                if n.tied {
                    *tie = Some(PendingTie {
                        event: seq + 1,
                        pos: n.pos,
                    });
                }
            }
        }
    };
    for (event, item) in song.items.iter().enumerate() {
        match item {
            Item::Single(n) => {
                let d = dur(n.len);
                check(d, n.pos)?;
                sound(n, t, d, &mut raw);
                notes.push(scheduled(n, t, d, false, event));
                t += d;
            }
            Item::Chord(entries) => {
                let mut shortest = f64::INFINITY;
                for n in entries {
                    let d = dur(n.len);
                    check(d, n.pos)?;
                    sound(n, t, d, &mut raw);
                    notes.push(scheduled(n, t, d, true, event));
                    shortest = shortest.min(d);
                }
                if shortest.is_finite() {
                    t += shortest;
                }
            }
            Item::Empty => t -= 1.0,
        }
    }
    if let Some(p) = ties.iter().flatten().min_by_key(|p| p.event) {
        return Err(AbcError::at(K::UnfinishedTie, p.pos));
    }
    Ok((notes, order_and_reattack(raw)))
}

fn scheduled(
    n: &NoteRec,
    start: f64,
    duration: f64,
    in_chord: bool,
    event: usize,
) -> ScheduledNote {
    ScheduledNote {
        start,
        duration,
        pitch: n.pitch,
        volume: n.volume,
        tied: n.tied,
        in_chord,
        event,
        position: n.pos,
    }
}

/// Sorting (§9.2) and the re-attack pass (§9.3), then the output list (§9.4).
///
/// Times closer than 0.0001 s count as equal and keep creation order. Because that
/// equality is not transitive, Tessera sorts by exact time first and then puts each run of
/// events within 0.0001 s of the run's first event into creation order.
fn order_and_reattack(mut raw: Vec<RawEvent>) -> Vec<NoteEvent> {
    raw.sort_by(|a, b| a.time.total_cmp(&b.time).then(a.seq.cmp(&b.seq)));
    let mut i = 0;
    while i < raw.len() {
        let anchor = raw[i].time;
        let mut j = i + 1;
        while j < raw.len() && raw[j].time - anchor < ORDER_TOLERANCE {
            j += 1;
        }
        if j - i > 1 {
            raw[i..j].sort_by_key(|e| e.seq);
        }
        i = j;
    }
    let mut sounding = [false; 37];
    let mut out: Vec<NoteEvent> = Vec::with_capacity(raw.len() + 1);
    let mut last = 0.0f64;
    let mut emit = |out: &mut Vec<NoteEvent>, time: f64, kind, pitch: u8, volume| {
        out.push(NoteEvent {
            time,
            delta: time - last,
            kind,
            pitch,
            slot: pitch_slot(pitch).unwrap_or(0),
            volume,
        });
        last = time;
    };
    for e in raw {
        let Some(idx) = e.pitch.checked_sub(PITCH_MIN).map(usize::from) else {
            continue;
        };
        let Some(s) = sounding.get_mut(idx) else {
            continue;
        };
        if e.on {
            if *s {
                emit(&mut out, e.time, NoteEventKind::Off, e.pitch, e.volume);
            }
            *s = true;
            emit(&mut out, e.time, NoteEventKind::On, e.pitch, e.volume);
        } else if *s {
            *s = false;
            emit(&mut out, e.time, NoteEventKind::Off, e.pitch, e.volume);
        }
    }
    out.push(NoteEvent {
        time: last,
        delta: 0.0,
        kind: NoteEventKind::End,
        pitch: 0,
        slot: 0,
        volume: 0,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_match_labels_rule() {
        assert_eq!(pitch_slot(48), Some(28));
        assert_eq!(pitch_slot(60), Some(29));
        assert_eq!(pitch_slot(72), Some(32));
        assert_eq!(pitch_slot(84), Some(35));
        assert_eq!(pitch_slot(47), None);
        assert_eq!(pitch_slot(85), None);
        assert_eq!(note_label(48).as_deref(), Some("Note_C2"));
        assert_eq!(note_label(69).as_deref(), Some("Note_A3"));
        assert_eq!(note_label(84).as_deref(), Some("Note_C5"));
        let mut seen = PITCH_SLOTS.to_vec();
        seen.sort_unstable();
        assert_eq!(seen, (1..=37).collect::<Vec<u8>>());
    }

    #[test]
    fn packing() {
        // instruments.md §14.2: pitch 69 → slot 11, velocity 105 → 0x6900000B.
        assert_eq!(pack_note(11, 105), 0x6900_000B);
        assert_eq!(pack_note(11, 128), 0x0000_000B);
    }

    #[test]
    fn boundary_examples() {
        // abc-dialect.md §13.2.
        assert_eq!(duration_seconds(4, 0.25, 10.0 / 1000.0, 10), 0.06);
        assert!(duration_seconds(4, 0.25, 9.0 / 1000.0, 9) < 0.06);
    }
}
