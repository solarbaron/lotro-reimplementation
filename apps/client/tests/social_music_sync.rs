// SPDX-License-Identifier: GPL-3.0-or-later
//! P4 exit criterion 3 (decision 0024 P4): "a synchronised `/play` from a scenario starts
//! within the tolerance of `instruments.md`". The committed scenario
//! `tests/scenarios/music-sync.scenario` plays headless on its virtual clock; its events
//! reach the client's social state and player music exactly as the runtime hands them over
//! (`SocialState::take_music_events` → `PlayerMusic::social_event`), with the audio clock run
//! in step with the virtual clock (`StartClock::Virtual`). For every synchronised start the
//! test measures every performance's time-0 from the instruments' schedule log and checks
//! both rules of `instruments.md` §8.9 (0.0001 s) against the start time *S* of the feed:
//! 2.500 s for the scripted start (three parts: the player's, Brin's, Cael's replacement
//! part) and 4.200 s for the generic answer to the player's own `/play … sync` and
//! `/playstart`.
//!
//! A second test runs the player's `/play … sync` and `/playstart` through the command
//! dispatcher with the plain sandbox, whose answer to `StartPerformance` starts the part
//! (one start for one `/playstart`, `scenario-feed.md` §6.7).
//!
//! Synthetic: an instrument table built here (one sound for every note), the offline audio
//! engine, original test tunes in a temporary music folder. Needs no install.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser as _;
use tessera_audio::engine::MixerSettings;
use tessera_audio::instruments::{
    InstrumentTable, MusicScripts, MusicVocabulary, START_TOLERANCE, SyncStart, TableLimits,
};
use tessera_client::audio::backend::BackendChoice;
use tessera_client::audio::player_music::{
    MusicConfig, MusicData, MusicRequest, PlayerMusic, StartClock,
};
use tessera_client::cli::Args;
use tessera_client::scenario::InstallChoice;
use tessera_client::social::SocialState;
use tessera_formats::Did;
use tessera_net::scenario::{HeadlessDriver, LogTime};
use tessera_net::{Event, Session};

const INFO: u32 = 0x2A00_0001;
const PAYLOAD: u32 = 0x0A00_0001;
const Q_KEY: u32 = 0x1000_0BCD;
const N_KEY: u32 = 0x1000_0BCE;
const WAIT: Duration = Duration::from_secs(20);

/// Original test tunes (written for these tests). The reel lasts 0.75 s, so the player's
/// part of the first start (at 2.5 s) has ended before the player's own start at 4.2 s
/// replaces it (a start stops the performer's running performance, `instruments.md` §8.8.7).
/// The jig begins with a rest, so its first note is not its time-0.
const REEL: &str = "X:1\nT:Lantern Reel\nM:3/4\nL:1/8\nQ:240\nK:C\nC E G c B G|\n";
const JIG: &str = "X:1\nT:Harbour Jig\nM:6/8\nL:1/8\nQ:120\nK:G\nz G B d B G|A F D G3|\n";

/// A sound-info record (`instruments.md` §4.2).
fn sound_info_bytes(did: u32, payload: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(did.to_le_bytes());
    b.extend(0u32.to_le_bytes());
    b.extend((-1.0f32).to_le_bytes());
    b.extend((-1.0f32).to_le_bytes());
    b.extend(1.0f32.to_le_bytes());
    b.extend(3.0f32.to_le_bytes());
    b.extend(30.0f32.to_le_bytes());
    b.extend(payload.to_le_bytes());
    b.push(1);
    b.push(b'x');
    b
}

/// Both music scripts: one Sound node every note of every instrument reaches.
fn synthetic_table() -> InstrumentTable {
    use tessera_audio::instruments::ScriptDef;
    use tessera_formats::script_table::{BaseBlock, Node, NodeData, NodeKind, SoundNode};
    let sound = Node {
        kind: NodeKind::SOUND,
        data: NodeData::Sound(Box::new(SoundNode {
            sound: Did(INFO),
            fire_and_forget: true,
            ..SoundNode::default()
        })),
        base: BaseBlock {
            use_parent_intensity: true,
            intensity_multiplier: 1.0,
            probability: 1.0,
            node_id: 1,
            ..BaseBlock::default()
        },
        particle_flag: None,
        ports: Vec::new(),
        targets: Vec::new(),
    };
    let def = |key, roots| ScriptDef {
        key,
        table: None,
        roots,
        max_instances: 8,
    };
    let scripts = MusicScripts {
        quantized: def(Q_KEY, vec![sound.clone()]),
        normal: def(N_KEY, vec![sound]),
        defining_tables: Vec::new(),
    };
    let vocab = MusicVocabulary {
        quantized_script: Q_KEY,
        normal_script: N_KEY,
        instrument_labels: vec![(4, "Lute".into())],
        ..MusicVocabulary::default()
    };
    InstrumentTable::build(scripts, &vocab, TableLimits::default(), |d: Did| {
        Ok::<_, String>((d.0 == INFO).then(|| sound_info_bytes(INFO, PAYLOAD)))
    })
    .expect("synthetic table")
}

/// Player music over the synthetic table: instrument 4 equipped, the offline engine, the
/// schedule log on.
fn music(folder: &Path) -> PlayerMusic {
    music_with(folder, Some(4))
}

/// [`music`] with `equipped` as the equipped instrument.
fn music_with(folder: &Path, equipped: Option<u32>) -> PlayerMusic {
    let mut m = PlayerMusic::with_data(
        MusicConfig {
            backend: BackendChoice::Offline,
            folders: vec![folder.to_path_buf()],
            ..MusicConfig::default()
        },
        MusicData {
            set: None,
            table: Ok(Arc::new(synthetic_table())),
            messages: None,
            equipped,
            mixer: MixerSettings::default(),
            settings_music: None,
            secs: 0.0,
        },
    );
    m.set_schedule_log(1 << 16);
    m
}

fn music_folder() -> tempfile::TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    std::fs::write(d.path().join("Lantern Reel.abc"), REEL).expect("write");
    std::fs::write(d.path().join("Harbour Jig.abc"), JIG).expect("write");
    d
}

/// Every performance of `s` meets both rules of `instruments.md` §8.9 (every queued note
/// event measured).
fn check(s: &SyncStart, m: &PlayerMusic, want_s: f64, parts: usize) {
    assert!(
        (s.start_secs - want_s).abs() <= 1e-9,
        "S = {} (expected {want_s})",
        s.start_secs
    );
    assert_eq!(s.parts.len(), parts, "parts started: {:?}", s.characters);
    assert!(s.refused.is_empty());
    let log = m.scheduled_events();
    for t in s.measure(log) {
        assert!(t.measured > 0 && t.measured == t.expected, "{t:?}");
        for v in [t.earliest, t.latest] {
            assert!(
                (v - s.start_secs).abs() <= START_TOLERANCE,
                "time-0 {v} vs S {} ({t:?})",
                s.start_secs
            );
        }
        eprintln!(
            "part of {}: {} note events, time-0 {:.7} … {:.7} s, S {:.7} s",
            t.character, t.measured, t.earliest, t.latest, s.start_secs
        );
    }
    let c = s.check_tolerance(log);
    eprintln!(
        "start at {:.3} s: spread {:.2e} s, worst |time-0 − S| {:.2e} s, {} measured",
        s.start_secs, c.spread, c.worst_from_start, c.measured
    );
    assert!(c.passes(), "{c:?}");
}

#[test]
fn the_scenarios_synchronised_starts_are_within_the_tolerance() {
    let folder = music_folder();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/music-sync.scenario");
    let args = Args::try_parse_from([
        "tessera-client",
        "--headless",
        "--no-data",
        "--scenario",
        path.to_str().expect("utf-8"),
    ])
    .expect("args");
    let sc =
        Arc::new(tessera_client::scenario::prepare(&args, &InstallChoice::None).expect("prepares"));
    let config = tessera_client::scenario::headless_config(&args, Arc::clone(&sc)).expect("config");
    let backend = tessera_net::sandbox::SandboxBackend::new(config).expect("sandbox");
    let mut driver = HeadlessDriver::<Vec<u8>>::new(Session::new(backend), sc, None);

    let mut m = music(folder.path());
    m.set_session_starts(true);
    m.set_start_clock(StartClock::Virtual { origin: 0.0 });
    let mut social = SocialState::new(true);
    let mut rendered = 0.0f64;
    let mut started = false;
    let mut mirrored = false;
    let outcome = driver.run_frames(|f| {
        let LogTime::At(ms) = f.time else {
            // Before world entry: nothing for the music.
            for ev in f.events {
                let _ = social.apply(ev, &|_| None);
            }
            return;
        };
        if !started {
            // The audio clock starts with the virtual clock at world entry.
            assert!(m.start_engine(), "the offline engine starts");
            started = true;
        }
        // The audio clock catches up with the virtual clock before the tick's events.
        let t = ms as f64 / 1e3;
        if t > rendered {
            m.render_offline(t - rendered);
            rendered = m.audio_clock().unwrap_or(t);
        }
        for ev in f.events {
            let _ = social.apply(ev, &|_| None);
        }
        // The player's own `/play … sync` at 3 s (the input script sends its
        // `ReadyPerformance`; the client reads and registers the part).
        if ms >= 3000 && !mirrored {
            mirrored = true;
            m.command("/play \"Harbour Jig\" sync");
            assert!(m.wait_idle(WAIT));
            let requests = m.take_requests();
            assert_eq!(
                requests,
                vec![MusicRequest::Ready {
                    tune: "Harbour Jig".into()
                }]
            );
        }
        let player = social.player().to_owned();
        let entity = social.player_entity();
        for ev in social.take_music_events() {
            m.social_event(&ev, &player, entity);
        }
        assert!(m.wait_idle(WAIT));
    });
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
    // Every note of both starts is scheduled once the audio clock passes the tunes' ends.
    m.render_offline(8.0);
    let starts: Vec<SyncStart> = m.sync_starts().cloned().collect();
    assert_eq!(starts.len(), 2, "two synchronised starts");
    // The scripted start at 1.5 s with a 1 s lead: the player's part, Brin's, and Cael's
    // replacement (its first part was withdrawn).
    check(&starts[0], &m, 2.5, 3);
    // The generic answer to the player's `StartPerformance` at 3.5 s (lead 0.7 s).
    check(&starts[1], &m, 4.2, 1);
    let lines = m.take_lines();
    assert!(
        lines
            .iter()
            .filter(|l| l.contains("synchronised start"))
            .count()
            == 2,
        "one start notice per start: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("Brin is ready")),
        "{lines:?}"
    );
}

#[test]
fn the_players_play_sync_and_playstart_start_through_the_sandbox() {
    use tessera_client::commands::{Context, Dispatcher, Origin};
    use tessera_client::net::World;
    use tessera_client::walker::WalkerSession;
    let folder = music_folder();
    let mut m = music(folder.path());
    let now = Instant::now();
    let mut world = World::sandbox(now).expect("sandbox");
    let deadline = Instant::now() + WAIT;
    while world.state() != tessera_net::SessionState::InWorld {
        world.update(Instant::now());
        assert!(Instant::now() < deadline, "the sandbox enters the world");
        std::thread::sleep(Duration::from_millis(5));
    }
    // The plain sandbox answers `StartPerformance` (scenario-feed.md §9.8).
    m.set_session_starts(true);
    assert!(m.start_engine());
    let mut d = Dispatcher::new();
    let run = |d: &mut Dispatcher, m: &mut PlayerMusic, w: &mut World, line: &str| {
        let mut out = Vec::new();
        let mut ctx = Context {
            now: Instant::now(),
            origin: Origin::Chat,
            session: Some(w as &mut dyn WalkerSession),
            music: Some(m),
            plugins: None,
        };
        d.submit(line, &mut ctx, &mut out);
        out
    };
    run(&mut d, &mut m, &mut world, "/play \"Lantern Reel\" sync");
    assert!(m.wait_idle(WAIT));
    assert!(m.is_waiting());
    run(&mut d, &mut m, &mut world, "/playstart");
    assert!(m.wait_idle(WAIT));
    assert!(!m.is_performing(), "/playstart starts nothing locally");
    // The runtime's step: the requests go to the session, its events to the music.
    tessera_client::social::send_music_requests(&mut world, &mut m, Instant::now());
    assert!(m.wait_idle(WAIT));
    assert!(
        !m.is_performing(),
        "the sandbox's answer starts the part, not the client"
    );
    let mut starts = Vec::new();
    let deadline = Instant::now() + WAIT;
    while starts.is_empty() {
        world.update(Instant::now());
        let player = world.social().player().to_owned();
        let entity = world.social().player_entity();
        for ev in world.social_mut().take_music_events() {
            if matches!(
                ev,
                Event::Social(tessera_net::domain::SocialEvent::PerformanceSyncStart { .. })
            ) {
                starts.push(ev.clone());
            }
            m.social_event(&ev, &player, entity);
        }
        assert!(Instant::now() < deadline, "the sandbox's start arrives");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(m.wait_idle(WAIT));
    assert!(
        m.is_performing(),
        "the session's start started the waiting part"
    );
    let s = m.sync_starts().last().cloned().expect("one start");
    m.render_offline(6.0);
    let c = s.check_tolerance(m.scheduled_events());
    assert!(c.passes(), "{c:?}");
    assert_eq!(m.sync_starts().count(), 1, "one start for one /playstart");
}

/// The social events of a session as the runtime hands them to the music.
fn feed(m: &mut PlayerMusic, events: &[Event]) {
    for ev in events {
        m.social_event(ev, "Tam", Some(tessera_net::EntityId(1)));
    }
    assert!(m.wait_idle(WAIT));
}

fn fellowship(members: &[(&str, u64)]) -> Event {
    use tessera_net::domain::{FellowMember, SocialEvent};
    let mut list = vec![FellowMember {
        name: "Tam".into(),
        entity: Some(tessera_net::EntityId(1)),
        leader: true,
    }];
    list.extend(members.iter().map(|(n, e)| FellowMember {
        name: (*n).into(),
        entity: Some(tessera_net::EntityId(*e)),
        leader: false,
    }));
    Event::Social(SocialEvent::FellowshipChanged { members: list })
}

fn ready(member: &str, entity: Option<u64>, tune: &str) -> Event {
    Event::Social(tessera_net::domain::SocialEvent::PerformanceReady {
        member: member.into(),
        entity: entity.map(tessera_net::EntityId),
        tune: tune.into(),
    })
}

/// Regression (P4 review): `PerformanceStopped` withdraws a fed member's waiting part when
/// the member's `PerformanceReady` (without an entity) follows the fellowship list that
/// names the member's entity (`instruments.md` §12 P4 Decision 6).
#[test]
fn perform_stop_after_fellowship_and_ready_withdraws_the_part() {
    let folder = music_folder();
    let mut m = music(folder.path());
    assert!(m.start_engine());
    feed(&mut m, &[fellowship(&[("Brin", 2)])]);
    feed(&mut m, &[ready("Brin", None, "Lantern Reel")]);
    assert_eq!(m.waiting_parts(), 1, "{:?}", m.take_lines());
    feed(
        &mut m,
        &[Event::Social(
            tessera_net::domain::SocialEvent::PerformanceStopped {
                entity: tessera_net::EntityId(2),
            },
        )],
    );
    assert_eq!(m.waiting_parts(), 0, "the stop withdrew Brin's part");
}

/// Regression (P4 review): a disconnection (and a new world entry) ends the session's
/// fellowship for the music: the members' waiting parts go, and their characters are freed,
/// so the 64-member bound applies to performers at one time, not over the client's life.
#[test]
fn members_are_dropped_with_the_session_and_freed() {
    let folder = music_folder();
    let mut m = music(folder.path());
    assert!(m.start_engine());
    feed(&mut m, &[fellowship(&[("Brin", 2)])]);
    feed(&mut m, &[ready("Brin", Some(2), "Lantern Reel")]);
    assert_eq!(m.waiting_parts(), 1);
    feed(
        &mut m,
        &[Event::Disconnected {
            reason: tessera_net::DisconnectReason::TimedOut,
        }],
    );
    assert_eq!(m.waiting_parts(), 0, "the old session's part is gone");
    assert_eq!(m.member_characters(), 0, "Brin's character is freed");
    // More than 64 members over time, one at a time: none is refused.
    for i in 0..70u64 {
        let name = format!("Member{i}");
        feed(&mut m, &[fellowship(&[(&name, 100 + i)])]);
        feed(&mut m, &[ready(&name, None, "Lantern Reel")]);
        assert_eq!(m.waiting_parts(), 1, "member {i}: {:?}", m.take_lines());
        feed(&mut m, &[fellowship(&[])]);
        assert_eq!(m.waiting_parts(), 0);
    }
    assert!(m.member_characters() <= 1);
    assert!(
        !m.take_lines().iter().any(|l| l.contains("too many")),
        "no part refused"
    );
}

/// Regression (P4 review): a `PerformanceStarted` plays its tune solo-like at the
/// processing time plus its delay (`instruments.md` §12 P4 Decision 5), for another
/// performer and for the player.
#[test]
fn performance_started_plays_after_its_delay() {
    let folder = music_folder();
    let mut m = music(folder.path());
    assert!(m.start_engine());
    let started = |entity: u64| {
        Event::Social(tessera_net::domain::SocialEvent::PerformanceStarted {
            entity: tessera_net::EntityId(entity),
            tune: "Lantern Reel".into(),
            delay_ms: 400,
        })
    };
    feed(&mut m, &[started(7)]);
    let s = m
        .schedule_log()
        .last()
        .cloned()
        .expect("the performer's tune is scheduled");
    let now = m.audio_clock().expect("clock");
    assert!(
        s.start_secs > now + 0.3 && s.start_secs <= now + 0.4 + 1e-6,
        "S = t + 0.4 s: {s:?} at {now}"
    );
    assert!(
        !m.is_performing(),
        "another performer's tune, not the player's"
    );
    feed(&mut m, &[started(1)]);
    assert!(
        m.is_performing(),
        "the player's own PerformanceStarted plays"
    );
    assert_eq!(m.schedule_log().count(), 2);
}

/// Regression (P4 review): the player's own `/play` with no instrument equipped and none
/// chosen is refused as in M6; only other performers' parts fall back to the table's first
/// instrument.
#[test]
fn play_without_an_instrument_is_still_refused() {
    let folder = music_folder();
    let mut m = music_with(folder.path(), None);
    assert!(m.start_engine());
    m.command("/play \"Lantern Reel\"");
    assert!(m.wait_idle(WAIT));
    let lines = m.take_lines();
    assert!(
        lines.iter().any(|l| l.contains("no instrument equipped")),
        "{lines:?}"
    );
    assert!(!m.is_performing());
}

/// The runtime's request step (`send_music_requests`): a session that refuses
/// `StartPerformance` gets the local start, exactly once per `/playstart`.
#[test]
fn a_refused_start_starts_the_waiting_part_locally_once() {
    use tessera_client::net::World;
    let folder = music_folder();
    let mut m = music(folder.path());
    assert!(m.start_engine());
    m.set_session_starts(true);
    m.command("/play \"Lantern Reel\" sync");
    assert!(m.wait_idle(WAIT));
    assert!(m.is_waiting());
    m.command("/playstart");
    assert!(m.wait_idle(WAIT));
    assert!(!m.is_performing());
    // A session not in the world refuses every command.
    let mut world = World::sandbox(Instant::now()).expect("sandbox");
    tessera_client::social::send_music_requests(&mut world, &mut m, Instant::now());
    assert!(m.wait_idle(WAIT));
    assert!(
        m.is_performing(),
        "the refused start started the part locally"
    );
    assert_eq!(m.sync_starts().count(), 1, "one local start");
    assert!(m.take_requests().is_empty());
    tessera_client::social::send_music_requests(&mut world, &mut m, Instant::now());
    assert!(m.wait_idle(WAIT));
    assert_eq!(m.sync_starts().count(), 1, "nothing more without a request");
}
