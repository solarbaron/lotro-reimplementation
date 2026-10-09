// SPDX-License-Identifier: GPL-3.0-or-later
//! Player music in the client against the user's own install, read-only (ROADMAP M6;
//! `abc-dialect.md` §3.1, §8.5, §10.3, §11; `instruments.md` §4.5, §12).
//!
//! 1. Start-up builds the instrument table from the install and finds the message table
//!    (`UI_Music` key of the named-string-table map, resolved at run time).
//! 2. `/play` of an original test tune from a scratch music folder on the offline backend:
//!    the scheduled note-on count and times are logged and compared with the reader's
//!    events; the performance starts 0.7 s after the command was processed; rendering it
//!    plays every note with no missing sample and nothing late.
//! 3. `/play … sync` waits; `/playstart` starts it 0.7 s later.
//! 4. A refused tune shows the user's table entry, not Tessera's own text.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` names an install. Prints counts
//! and times only, never game text:
//!
//! ```text
//! export TESSERA_GAME_DIR="$(cargo run -q -p tessera-dattool -- detect --path)"
//! cargo test -p tessera-client --release --test player_music_real_data -- --nocapture
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use tessera_audio::abc::{self, AbcError, AbcErrorKind, PartRequest, Position, StartMode};
use tessera_client::audio::backend::BackendChoice;
use tessera_client::audio::player_music::{MusicConfig, PlayerMusic, error_text, fallback_error};
use tessera_client::wait::{LoadWait, ProgressWait};
use tessera_dat::{ArchiveSet, SetOptions};

/// How long the player-music waits last: as long as start-up, jobs and queued commands
/// progress. A wait gives up after this many polls (about 2 ms apart, longer on a loaded
/// machine) in a row in which nothing finished, or after this many polls in all
/// ([`PlayerMusic::wait_settled`]). Start-up and each command job are single steps, so the
/// stall limit is at least as long as the longest of them may take.
const MUSIC_WAIT: LoadWait = if cfg!(debug_assertions) {
    LoadWait::Progress(ProgressWait::new(900_000, 7_200_000))
} else {
    LoadWait::Progress(ProgressWait::new(150_000, 1_800_000))
};

/// An original test tune: a scale with a chord and a tie, two parts.
const TUNE: &str = "X:1\nT:Tessera Test Air\nM:3/4\nL:1/8\nQ:1/4=100\nK:D\n\
                    D2 F2 A2|d4 A2|[D2F2A2] B2 c2-|c4 z2|\n\
                    X:2\nT:Tessera Test Round\nM:4/4\nL:1/4\nQ:120\nK:C\nC E G c|c G E C|\n";

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os(tessera_install::GAME_DIR_ENV) {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

#[test]
fn play_an_original_tune_on_the_offline_backend() {
    let Some(dir) = game_dir() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archives"));
    let music = tempfile::tempdir().expect("scratch music folder");
    std::fs::write(music.path().join("tessera-test.abc"), TUNE).expect("write tune");
    let mut m = PlayerMusic::new(MusicConfig {
        backend: BackendChoice::Offline,
        game_dir: Some(dir.clone()),
        folders: vec![music.path().to_path_buf()],
        ..MusicConfig::default()
    });
    m.attach_data(Arc::clone(&set), &dir);

    // 1. Start-up (at the first command).
    assert!(m.command("/instrument"));
    assert!(m.wait_settled(MUSIC_WAIT), "start-up finished");
    let data = m.data().expect("start-up data");
    let table = data.table.as_ref().expect("instrument table").clone();
    let msg_table = data
        .messages
        .as_ref()
        .and_then(tessera_client::audio::player_music::Messages::table);
    eprintln!(
        "start-up {:.2} s: {} instrument types, {} sound nodes, message table {}",
        data.secs,
        table.instruments().len(),
        table.sounds().len(),
        msg_table.map_or_else(|| "missing".into(), |t| format!("{t:#010x}")),
    );
    assert!(
        msg_table.is_some(),
        "abc-dialect.md §10.3: the message table exists"
    );
    let lines = m.take_lines();
    assert_eq!(lines.len(), 2, "current instrument and the list");

    // A refused tune: the user's entry, rendered with line and column. (Assertions here
    // print no game text.)
    const BROKEN: &str = "X:1\nK:C\nC D [CEGBdfa]|\n";
    let e = abc::parse(BROKEN.as_bytes(), PartRequest::Unspecified).expect_err("refused");
    assert_eq!(e.kind, AbcErrorKind::TooManyNotesInChord);
    let pos = e.position.expect("position");
    let messages = m.data().and_then(|d| d.messages.as_ref());
    let text = error_text(messages, &e);
    assert!(
        text != fallback_error(&e),
        "§10.3: the table's entry is used"
    );
    assert!(
        text.contains(&pos.line.to_string()) && text.contains(&pos.column.to_string()),
        "line and column rendered"
    );
    let synthetic = AbcError {
        kind: AbcErrorKind::NoteTooLow,
        position: Some(Position {
            line: 77,
            column: 91,
        }),
        path: None,
    };
    let low = error_text(messages, &synthetic);
    assert!(low != fallback_error(&synthetic) && low.contains("77") && low.contains("91"));
    std::fs::write(music.path().join("broken.abc"), BROKEN).expect("write");
    assert!(m.command("/play broken"));
    assert!(m.wait_settled(MUSIC_WAIT));
    let lines = m.take_lines();
    assert!(
        lines == vec![text.clone()],
        "the refusal shows one message ({} lines)",
        lines.len()
    );
    eprintln!(
        "refusal at line {} column {}: {} characters from the user's table",
        pos.line,
        pos.column,
        text.chars().count()
    );

    // 2. The debug instrument choice (no item model in the sandbox): the first named type
    // other than 0.
    let instrument = *table
        .instruments()
        .iter()
        .find(|&&k| k != 0)
        .expect("an instrument type");
    assert!(m.command(&format!("/instrument {instrument}")));
    m.take_lines();
    assert_eq!(m.instrument(), Some(instrument));

    let expected = abc::parse(TUNE.as_bytes(), PartRequest::Unspecified).expect("tune reads");
    let expected_ons: Vec<f64> = expected.note_ons().map(|e| e.time).collect();
    assert!(m.command("/play tessera-test"));
    assert!(m.wait_settled(MUSIC_WAIT));
    let lines = m.take_lines();
    assert_eq!(
        lines.len(),
        1,
        "the now-playing notice: {} lines",
        lines.len()
    );
    let engine_now = m.engine().map(|e| e.clock().as_secs(e.sample_rate()));
    let solo = m.schedule_log().last().expect("scheduled").clone();
    assert_eq!(solo.mode, StartMode::Solo);
    assert_eq!(solo.note_on_times, expected_ons, "every note-on scheduled");
    assert!(
        (solo.start_secs - engine_now.unwrap_or(0.0) - abc::SOLO_START_OFFSET).abs() < 1e-9,
        "§8.5: 0.7 s after the command was processed"
    );
    eprintln!(
        "solo: instrument {instrument}, start {:.3} s on the audio clock, {} note-ons at {:?} s, end {:.3} s",
        solo.start_secs,
        solo.note_on_times.len(),
        solo.note_on_times
            .iter()
            .map(|t| (t * 1000.0).round() / 1000.0)
            .collect::<Vec<_>>(),
        solo.end_time
    );
    let (frames, peak) = m
        .render_offline(solo.start_secs + solo.end_time + 1.0)
        .expect("offline engine");
    let st = m.player_stats().expect("player");
    eprintln!(
        "rendered {:.2} s: {} note-ons, {} note-offs, {} voices accepted, peak {peak:.3}; {:?}",
        frames as f64 / 48_000.0,
        st.note_ons,
        st.note_offs,
        st.voices_accepted,
        m.stats()
    );
    assert_eq!(
        st.note_ons,
        expected_ons.len() as u64,
        "every note-on processed"
    );
    assert_eq!(st.late_dropped, 0);
    assert_eq!(st.missing_sample, 0);
    assert_eq!(st.missing_sound_info, 0);
    assert!(st.voices_accepted > 0 && peak > 0.0, "the tune is audible");

    // 3. Synced: waits for /playstart, then starts 0.7 s after it.
    assert!(m.command("/play tessera-test 2 sync"));
    assert!(m.wait_settled(MUSIC_WAIT));
    assert!(m.take_lines().is_empty(), "a synced part waits silently");
    let before = m.schedule_log().count();
    m.render_offline(0.5);
    assert_eq!(m.schedule_log().count(), before, "still waiting");
    assert!(m.command("/playstart"));
    let lines = m.take_lines();
    assert_eq!(lines.len(), 1, "the synced-start notice");
    let synced = m.schedule_log().last().expect("synced").clone();
    assert_eq!(synced.mode, StartMode::Synced);
    assert_eq!(synced.note_on_times.len(), 8);
    let now = m
        .engine()
        .map(|e| e.clock().as_secs(e.sample_rate()))
        .unwrap_or(0.0);
    assert!((synced.start_secs - now - abc::SOLO_START_OFFSET).abs() < 1e-9);
    m.render_offline(synced.end_time + 1.5);
    let st2 = m.player_stats().expect("player");
    eprintln!(
        "synced: start {:.3} s, {} note-ons at {:?} s; total note-ons {}",
        synced.start_secs,
        synced.note_on_times.len(),
        synced.note_on_times,
        st2.note_ons
    );
    assert_eq!(st2.note_ons, st.note_ons + 8);
    assert_eq!(st2.late_dropped, 0);
    assert_eq!(st2.missing_sample, 0);

    // The stop command: a long performance stops at once.
    assert!(m.command("/play tessera-test"));
    assert!(m.wait_settled(MUSIC_WAIT));
    m.render_offline(1.0);
    assert!(m.command("/playstop"));
    let ons = m.player_stats().expect("player").note_ons;
    m.render_offline(10.0);
    assert_eq!(
        m.player_stats().expect("player").note_ons,
        ons,
        "nothing after the stop"
    );
}

/// `instruments.md` §4.5 on real items: the first instrument item found in the install
/// (scanning object property sets, no DID named here) equipped through `--avatar` gives
/// the player its `Item_Music_Instrument_Type`.
#[test]
fn equipped_instrument_item_sets_the_type() {
    use tessera_client::audio::player_music::ItemFacts;
    use tessera_formats::{PropertyDictionary, PropertySetRecord, Record};

    let Some(dir) = game_dir() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archives"));
    let dict_did = 0x3400_0000;
    let dict = PropertyDictionary::parse_expecting(&set.read(dict_did).expect("dict"), dict_did)
        .expect("dict");
    let t0 = std::time::Instant::now();
    let (mut scanned, mut found) = (0usize, None);
    'scan: for member in set.members() {
        for entry in member.archive.entries_range(0x7900_0000..0x7A00_0000) {
            let Ok(entry) = entry else { continue };
            scanned += 1;
            if scanned > 400_000 {
                break 'scan;
            }
            let Ok(bytes) = member.archive.read_entry(&entry) else {
                continue;
            };
            let Ok(rec) = PropertySetRecord::parse_expecting(&bytes, entry.did, &dict) else {
                continue;
            };
            let facts = ItemFacts::from_properties(&rec.set, &dict);
            if facts.sets_instrument() && facts.phys_obj && facts.instrument_type.is_some() {
                found = Some((entry.did, facts));
                break 'scan;
            }
        }
    }
    let Some((item, facts)) = found else {
        panic!("no instrument item among {scanned} property sets");
    };
    eprintln!(
        "first instrument item after {scanned} property sets ({:.2?}): type {:?}",
        t0.elapsed(),
        facts.instrument_type
    );
    let mut m = PlayerMusic::new(MusicConfig {
        backend: BackendChoice::Offline,
        game_dir: Some(dir.clone()),
        items: vec![item],
        ..MusicConfig::default()
    });
    m.attach_data(Arc::clone(&set), &dir);
    assert!(m.command("/instrument"));
    assert!(m.wait_settled(MUSIC_WAIT));
    assert_eq!(m.data().and_then(|d| d.equipped), facts.instrument_type);
    assert_eq!(m.instrument(), facts.instrument_type);
    // A debug choice overrides the item; `item` goes back to it.
    assert!(m.command("/instrument 0"));
    assert_eq!(m.instrument(), Some(0));
    assert!(m.command("/instrument item"));
    assert_eq!(m.instrument(), facts.instrument_type);
}
