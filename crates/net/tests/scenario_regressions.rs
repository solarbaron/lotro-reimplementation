// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for review findings on the scenario feed (`docs/specs/scenario-feed.md`):
//! parser error kinds (§4.2, §4.4, §7.2, §11.1), generic answers and triggers on unknown
//! items and taken loot (§7.6), the driver's dialogue mirror (§7.7), label notes (§8.6),
//! run-time limits (§7.3, §7.4, §11.3) and the driver's frame and diagnostics API.
#![cfg(feature = "scenario")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_net::domain::{ItemId, Shortcut};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{
    Alt, Candidate, CandidateOutcome, CandidateStatus, EmptyResolver, FilterValue, HeadlessDriver,
    LabelContext, LogTime, ResolvedScenario, Scenario, ScenarioErrorKind as K, SelectorQuery,
    SelectorResolver,
};
use tessera_net::{
    ChatChannel, CombatCommand, CombatEvent, Command, DataId, Event, NetError, Session,
};

fn parse_err(text: &str) -> (K, u32, u32) {
    let e = Scenario::parse(text.as_bytes()).expect_err("must fail");
    (e.kind, e.line, e.column)
}

fn parses(text: &str) {
    if let Err(e) = Scenario::parse(text.as_bytes()) {
        panic!("{e} in:\n{text}");
    }
}

fn resolve(text: &str, resolver: &mut dyn SelectorResolver) -> ResolvedScenario {
    Scenario::parse(text.as_bytes())
        .expect("parse")
        .resolve(resolver)
        .expect("resolve")
}

fn run_resolved(resolved: ResolvedScenario) -> (String, u8) {
    let resolved = Arc::new(resolved);
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let session = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let mut driver = HeadlessDriver::new(session, resolved, Some(Vec::new()));
    let outcome = driver.run();
    let log = String::from_utf8(driver.into_log().expect("log")).expect("utf-8");
    (log, outcome.exit_code())
}

fn run(text: &str) -> String {
    run_resolved(resolve(text, &mut EmptyResolver)).0
}

fn has(log: &str, line: &str) {
    assert!(
        log.lines().any(|l| l == line),
        "missing `{line}` in:\n{log}"
    );
}

fn count(log: &str, needle: &str) -> usize {
    log.lines().filter(|l| l.contains(needle)).count()
}

/// A resolver with an install: `n` candidates per query; every label of a query is
/// reported unmatched, plus `extra` invented ones.
struct Labels {
    n: u32,
    extra: usize,
}

impl SelectorResolver for Labels {
    fn has_install(&self) -> bool {
        true
    }

    fn candidates(
        &mut self,
        query: &SelectorQuery<'_>,
        limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        let n = usize::try_from(self.n).unwrap_or(0).min(limit);
        out.extend((0..n).map(|i| Candidate {
            did: DataId(0x7000_0000 + u32::try_from(i).unwrap_or(0)),
            row: 0,
            location: None,
            anchor: None,
        }));
        let mut o = CandidateOutcome::new(CandidateStatus::Ready);
        for f in query.filters {
            if let FilterValue::Alternatives(alts) = &f.value {
                for a in alts {
                    if let Alt::Label(l) = a {
                        o.unmatched_labels.push((f.attribute, l.clone()));
                    }
                }
            }
        }
        if let Some(f) = query.filters.first() {
            for i in 0..self.extra {
                o.unmatched_labels
                    .push((f.attribute, format!("@Invented{i}")));
            }
        }
        o
    }

    fn enum_value(&mut self, _context: LabelContext, _label: &str) -> Option<u32> {
        None
    }

    fn display_name(&mut self, _did: DataId, _row: Option<u32>) -> Option<String> {
        None
    }
}

// --- Parser ---------------------------------------------------------------------------

/// §4.4: any block inside `input` is NotAllowedHere at the opener's keyword.
#[test]
fn repeat_inside_input_is_not_allowed_here() {
    assert_eq!(
        parse_err("tessera-scenario 1\ninput\nat 1s repeat 2 every 1s\nend\n"),
        (K::NotAllowedHere, 3, 7)
    );
    assert_eq!(
        parse_err("tessera-scenario 1\ninput\n+0s repeat 2 every 1s\nend\n"),
        (K::NotAllowedHere, 3, 5)
    );
    // Other `+` lines in input stay UnexpectedToken at the timing (§16 OPEN 8).
    assert_eq!(
        parse_err("tessera-scenario 1\ninput\n+0s cancel-cast\nend\n"),
        (K::UnexpectedToken, 3, 1)
    );
}

/// §7.2: steps in `on` bodies are checked at run time, not statically; a single
/// duration token is still bounded by the horizon.
#[test]
fn trigger_bodies_may_pass_the_horizon_and_are_dropped_at_run_time() {
    let text = "tessera-scenario 1\ntick 1000ms\nplayer \"Testa\"\non cancel-cast\n+59min system \"late\"\n+2min system \"too late\"\nend\non chat\n+30min repeat 3 every 20min\n+5min system \"r\"\nend\nend\ninput\nat 1s cancel-cast\nat 2s chat \"x\"\nend\n";
    parses(text);
    let log = run(text);
    has(
        &log,
        "3541.000 ChatMessage channel=system sender=none text=\"late\"",
    );
    assert_eq!(count(&log, "too late"), 0, "{log}");
    // The repeat inside the chat trigger: iterations at 2+30, 2+50, 2+70 min; the body's
    // +5 min gives 37 min and 57 min, the third (77 min) is dropped.
    has(
        &log,
        "2102.000 ChatMessage channel=system sender=none text=\"r\"",
    );
    has(
        &log,
        "3302.000 ChatMessage channel=system sender=none text=\"r\"",
    );
    assert!(
        log.lines()
            .last()
            .is_some_and(|l| l.contains("reason=complete") && l.contains("dropped=2")),
        "{log}"
    );
    // One token over the horizon stays OutOfRange.
    assert_eq!(
        parse_err("tessera-scenario 1\non cancel-cast\n+61min system \"x\"\nend\n"),
        (K::OutOfRange, 3, 1)
    );
    // Top-level times are still checked statically.
    assert_eq!(
        parse_err("tessera-scenario 1\nat 59min system \"a\"\n+2min system \"b\"\n"),
        (K::OutOfRange, 3, 1)
    );
}

/// §4.2: no form allows an empty string.
#[test]
fn empty_texts_are_refused() {
    let cases = [
        ("title \"\"\n", (K::UnexpectedToken, 2, 7)),
        ("page $p \"\"\n", (K::UnexpectedToken, 2, 9)),
        (
            "page $p \"Hi\"\noption $p 1 \"\"\n",
            (K::UnexpectedToken, 3, 13),
        ),
        ("at 0s marker $m 0,0,0 \"\"\n", (K::UnexpectedToken, 2, 23)),
        (
            "on create-character\n+0s refuse-character \"A\" \"\"\nend\n",
            (K::UnexpectedToken, 3, 26),
        ),
    ];
    for (body, want) in cases {
        assert_eq!(
            parse_err(&format!("tessera-scenario 1\n{body}")),
            want,
            "{body}"
        );
    }
    parses("tessera-scenario 1\ntitle \"T\"\npage $p \"Hi\"\noption $p 1 \"Ok\"\n");
}

/// §11.1: a keyword over 32 bytes is TooLong wherever a keyword stands.
#[test]
fn over_long_keywords_are_too_long_everywhere() {
    let long = "abcdefghijabcdefghijabcdefghijabc";
    assert_eq!(
        parse_err(&format!("tessera-scenario 1\nat 0s {long} player\n")),
        (K::TooLong, 2, 7)
    );
    assert_eq!(
        parse_err(&format!("tessera-scenario 1\non {long}\nend\n")),
        (K::TooLong, 2, 4)
    );
    assert_eq!(
        parse_err(&format!("tessera-scenario 1\ninput\nat 1s {long}\nend\n")),
        (K::TooLong, 3, 7)
    );
    assert_eq!(
        parse_err(&format!("tessera-scenario 1\n{long}\n")),
        (K::TooLong, 2, 1)
    );
    // At most 32 bytes: unknown, not too long.
    assert_eq!(
        parse_err("tessera-scenario 1\nat 0s abcdefghijabcdefghijabcdefghijab player\n"),
        (K::UnknownKeyword, 2, 7)
    );
}

// --- Resolution -----------------------------------------------------------------------

/// §8.2, §8.6: one label note per unknown label, bounded by the labels written.
#[test]
fn every_unmatched_label_gets_a_note() {
    let alts: Vec<String> = (0..100).map(|i| format!("@L{i}")).collect();
    let text = format!(
        "tessera-scenario 1\nselect $s = item quality={} all\n",
        alts.join("|")
    );
    let r = resolve(&text, &mut Labels { n: 0, extra: 0 });
    let labels = r
        .notes()
        .iter()
        .filter(|n| n.to_string().starts_with("label "))
        .count();
    assert_eq!(labels, 100);
    // A resolver inventing labels cannot inflate the notes.
    let r = resolve(
        "tessera-scenario 1\nselect $s = item quality=@A|@B all\n",
        &mut Labels {
            n: 0,
            extra: 10_000,
        },
    );
    let labels = r
        .notes()
        .iter()
        .filter(|n| n.to_string().starts_with("label "))
        .count();
    assert_eq!(labels, 2);
}

// --- Feed -----------------------------------------------------------------------------

/// A session over a sandbox playing `text`, in the world.
fn entered(text: &str, resolver: &mut dyn SelectorResolver) -> (Session, Instant) {
    let mut r = resolve(text, resolver);
    r.set_plays_input(false);
    let config = SandboxConfig {
        scenario: Some(Arc::new(r)),
        ..SandboxConfig::default()
    };
    let mut session = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let mut now = Instant::now();
    session.send(now, Command::Connect).expect("connect");
    let mut events = Vec::new();
    session.poll_into(now, &mut events);
    session
        .send(
            now,
            Command::EnterWorld {
                character: "Testa".into(),
            },
        )
        .expect("enter");
    for _ in 0..200 {
        events.clear();
        session.poll_into(now, &mut events);
        if events
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. }))
        {
            return (session, now);
        }
        now += Duration::from_millis(50);
    }
    panic!("no world entry");
}

fn poll_ticks(session: &mut Session, now: &mut Instant, ticks: u32) -> Vec<Event> {
    let mut all = Vec::new();
    for _ in 0..ticks {
        *now += Duration::from_millis(50);
        session.poll_into(*now, &mut all);
    }
    all
}

/// §7.6: `SetQuickslot` naming an unknown item gets no answer and leaves the quickslot
/// state alone, so `use-skill quickslot=N` triggers do not see it.
#[test]
fn set_quickslot_with_an_unknown_item_gets_no_answer() {
    let text = "tessera-scenario 1\nplayer \"Testa\"\nitem $bread \"Bread\"\n";
    let (mut session, mut now) = entered(text, &mut EmptyResolver);
    poll_ticks(&mut session, &mut now, 2);
    session
        .send(
            now,
            Command::Combat(CombatCommand::SetQuickslot {
                slot: 1,
                content: Some(Shortcut::Item(ItemId(999))),
            }),
        )
        .expect("send");
    let events = poll_ticks(&mut session, &mut now, 3);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Combat(CombatEvent::QuickslotChanged { .. }))),
        "{events:?}"
    );
    // A known item (the first declared item has id 1) and a skill still answer.
    session
        .send(
            now,
            Command::Combat(CombatCommand::SetQuickslot {
                slot: 2,
                content: Some(Shortcut::Item(ItemId(1))),
            }),
        )
        .expect("send");
    let events = poll_ticks(&mut session, &mut now, 3);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Combat(CombatEvent::QuickslotChanged { slot: 2, .. })
        )),
        "{events:?}"
    );
}

/// §7.6, §16 OPEN 6: an already taken loot entry is unknown: no trigger fires on it.
#[test]
fn take_loot_on_a_taken_entry_fires_no_trigger() {
    let log = run(
        "tessera-scenario 1\nplayer \"Testa\"\nat 0s loot items=\"Bread\",\"Ale\"\non take-loot\n+0s system \"fired\"\nend\ninput\nat 0.1s take-loot 1\nat 0.2s take-loot 1\nat 0.3s take-loot all\nend\n",
    );
    has(
        &log,
        "0.100 ChatMessage channel=system sender=none text=\"fired\"",
    );
    has(&log, "0.200 > TakeLoot window=1 entry=1");
    assert_eq!(
        count(&log, "0.200 ChatMessage"),
        0,
        "a taken entry must not fire:\n{log}"
    );
    // `all` names the window: it still matches (and takes the remaining entry).
    has(
        &log,
        "0.300 ChatMessage channel=system sender=none text=\"fired\"",
    );
    has(&log, "0.300 LootTaken window=1 entry=2");
}

/// §7.7: `choose $page N` is sent while the open window shows `$page`'s content, even
/// when another page renders identically (the event carries no page identity).
#[test]
fn choose_page_with_identical_pages_is_sent() {
    let log = run(
        "tessera-scenario 1\nplayer \"Testa\"\npage $c \"Next\"\npage $a \"Hi\"\noption $a 1 \"Ok\" goto=close\npage $b \"Hi\"\noption $b 1 \"Ok\" goto=$c\nat 0s show $b\ninput\nat 1s choose $b 1\nat 2s choose $a 1\nend\n",
    );
    has(&log, "1.000 > ChooseOption window=1 option=1");
    has(
        &log,
        "1.000 DialogueShown window=1 speaker=none text=\"Next\" options=[]",
    );
    // The window now shows $c, whose content differs from $a's: skipped.
    assert!(
        log.lines()
            .last()
            .is_some_and(|l| l.contains("input-skipped=1")),
        "{log}"
    );
}

/// The static-skip cache keeps the counts: a window step with a missing list element
/// skips (and counts) on every run.
#[test]
fn window_steps_with_missing_elements_skip_every_time() {
    let log = run(
        "tessera-scenario 1\nplayer \"Testa\"\nselect $s = item all\npage $p name($s[0])\noption $p 1 \"Ok\"\nat 0s repeat 5 every 1s\n+0s vendor items=\"x\",\"y\",$s[0]\n+0s loot items=$s[0],\"z\"\n+0s show $p\nend\n",
    );
    assert!(
        log.lines()
            .last()
            .is_some_and(|l| l.contains("skipped=15 ")),
        "{log}"
    );
    assert_eq!(count(&log, "VendorShown"), 0, "{log}");
}

/// §7.3: a run still going at the horizon ends with reason `horizon`; the pending cast
/// end is dropped.
#[test]
fn horizon_end_drops_pending_entries() {
    let log = run(
        "tessera-scenario 1\ntick 1000ms\nplayer \"Testa\"\nat 59min cast player \"Bolt\" 2min\n",
    );
    has(
        &log,
        "3600.000 end reason=horizon skipped=0 input-skipped=0 dropped=1 spilled=0",
    );
}

/// §7.4: the feed holds at most 1,024 unprocessed commands; the 1,025th is refused.
#[test]
fn feed_refuses_the_1025th_pending_command() {
    let (mut session, now) = entered("tessera-scenario 1\nplayer \"Testa\"\n", &mut EmptyResolver);
    for i in 0..1024 {
        session
            .send(
                now,
                Command::Chat {
                    channel: ChatChannel::Say,
                    text: format!("c{i}"),
                },
            )
            .unwrap_or_else(|e| panic!("command {i}: {e}"));
    }
    let r = session.send(
        now,
        Command::Chat {
            channel: ChatChannel::Say,
            text: "over".into(),
        },
    );
    assert!(matches!(r, Err(NetError::SendQueueFull)), "{r:?}");
}

/// §5.3, §11.3: more than 65,536 run-time items stop the feed with `ItemOverflow`.
#[test]
fn item_overflow_stops_the_feed() {
    let r = resolve(
        "tessera-scenario 1\nplayer \"Testa\"\nselect $x = item all\nat 0s give $x\n+1s give $x to=vault\n",
        &mut Labels {
            n: 70_000,
            extra: 0,
        },
    );
    let (log, code) = run_resolved(r);
    assert_eq!(code, 5, "{log}");
    assert!(
        log.lines()
            .last()
            .is_some_and(|l| l.contains("error=ItemOverflow line=5")),
        "{log}"
    );
}

/// §13 rule 2 guards: numeric enum values, the `did` family and the labels outside
/// selectors.
#[test]
fn committed_content_guards() {
    let num = |body: &str| {
        Scenario::parse(format!("tessera-scenario 1\n{body}").as_bytes())
            .expect("parse")
            .numeric_enum_values()
    };
    assert!(num("select $s = item category=5 all\n"));
    assert!(!num("select $s = item category=@Food all\n"));
    assert!(!num("select $s = place region=3 all\n"));
    assert!(num("player \"T\" class=23\n"));
    assert!(num("actor $a kind=npc pos=0,0,0 species=@Elf sex=2\n"));
    assert!(!num("actor $a kind=npc pos=0,0,0 species=@Elf sex=m\n"));
    assert!(num(
        "item $i \"Sword\"\ninput\nat 1s equip $i slot=3\nend\n"
    ));
    assert!(!num("at 0s system \"x\"\n"));

    let sc = Scenario::parse(b"tessera-scenario 1\nselect $d = did 0x0700_0001\n");
    // Hex lists have no separators: this is a BadNumber, not a did selector.
    assert!(sc.is_err());
    let sc =
        Scenario::parse(b"tessera-scenario 1\nselect $d = did 0x07000001 all\n").expect("parse");
    assert!(sc.uses_did_family());
    let sc = Scenario::parse(b"tessera-scenario 1\nselect $s = item all\n").expect("parse");
    assert!(!sc.uses_did_family());

    let sc =
        Scenario::parse(b"tessera-scenario 1\nplayer \"T\" class=@Burglar species=@Hobbit sex=f\n")
            .expect("parse");
    assert_eq!(
        sc.labels(),
        vec![
            (2, "class", "@Burglar"),
            (2, "species", "@Hobbit"),
            (2, "sex", "f"),
        ]
    );
}

// --- Driver API -----------------------------------------------------------------------

/// The frame callback sees the virtual clock, the session and the queued weight; the
/// resolve timeout comes from the sandbox; `with_max_ticks` lowers the horizon.
#[test]
fn driver_frames_carry_the_virtual_clock() {
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\nat 0s system \"a\"\nat 0.5s system \"b\"\n";
    let resolved = Arc::new(resolve(text, &mut EmptyResolver));
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        resolve_timeout: Duration::from_secs(3),
        ..SandboxConfig::default()
    };
    let backend = SandboxBackend::new(config).expect("sandbox");
    assert_eq!(backend.resolve_timeout(), Duration::from_secs(3));
    let session = Session::new(backend);
    let mut driver = HeadlessDriver::new(session, Arc::clone(&resolved), None::<Vec<u8>>);
    let mut frames: Vec<(Instant, LogTime, usize)> = Vec::new();
    let outcome = driver.run_frames(|f| {
        let w = f
            .session
            .backend::<SandboxBackend>()
            .and_then(SandboxBackend::scenario_queued_weight)
            .expect("weight");
        assert!(w <= 2_097_152);
        frames.push((f.now, f.time, f.events.len()));
    });
    assert_eq!(outcome.exit_code(), 0);
    let ticked: Vec<&(Instant, LogTime, usize)> = frames
        .iter()
        .filter(|(_, t, _)| matches!(t, LogTime::At(ms) if *ms > 0))
        .collect();
    assert!(ticked.len() >= 5, "{frames:?}");
    for w in ticked.windows(2) {
        let (LogTime::At(a), LogTime::At(b)) = (w[0].1, w[1].1) else {
            unreachable!()
        };
        assert_eq!(b - a, 100);
        assert_eq!(w[1].0 - w[0].0, Duration::from_millis(100));
    }

    // A lowered horizon ends the run early with reason `horizon`.
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\nat 10s system \"late\"\n";
    let resolved = Arc::new(resolve(text, &mut EmptyResolver));
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let session = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let mut driver = HeadlessDriver::new(session, resolved, Some(Vec::new())).with_max_ticks(20);
    let outcome = driver.run();
    assert_eq!(outcome.exit_code(), 0);
    let log = String::from_utf8(driver.into_log().expect("log")).expect("utf-8");
    assert!(!log.contains("late"), "{log}");
    assert!(
        log.lines()
            .last()
            .is_some_and(|l| l.starts_with("2.000 end reason=horizon")),
        "{log}"
    );
}

/// A log that reaches its size limit ends the run at once with `error=LogOverflow` (exit 5),
/// keeping every whole line written before it (`scenario-feed.md` §11.1 OPEN, the review's
/// recommended bound).
#[test]
fn a_log_at_its_size_limit_ends_with_log_overflow() {
    let text = "tessera-scenario 1\ntick 50ms\nplayer \"Testa\"\n\
                at 0s repeat 1000 every 50ms\n+0s system \"A line of the scenario log\"\nend\n";
    let resolved = Arc::new(resolve(text, &mut EmptyResolver));
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let session = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let mut driver =
        HeadlessDriver::new(session, Arc::clone(&resolved), Some(Vec::new())).with_log_limit(4_096);
    let outcome = driver.run();
    assert_eq!(outcome.exit_code(), 5);
    let log = String::from_utf8(driver.into_log().expect("log")).expect("utf-8");
    let (body, last) = log.trim_end_matches('\n').rsplit_once('\n').expect("lines");
    assert!(body.len() < 4_096, "{}", body.len());
    assert!(
        last.contains(" end reason=error ") && last.ends_with(" error=LogOverflow line=0"),
        "{last}"
    );
    // The run stopped at the poll that overflowed, long before the scenario's end.
    let LogTime::At(ms) = outcome.time else {
        panic!("{:?}", outcome.time)
    };
    assert!(ms < 10_000, "{ms}");
    // The same run without the lowered limit completes.
    assert_eq!(run_resolved(resolve(text, &mut EmptyResolver)).1, 0);
}

/// The public input player yields the input script as the headless driver plays it:
/// `at pre` lines first, timed lines by tick (time, then file order), and a line whose
/// window is not open builds no command (§7.7).
#[test]
fn the_input_player_yields_lines_by_tick() {
    use tessera_net::scenario::InputPlayer;
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
                input\nat 0.25s chat \"c\"\nat 0s chat \"a\"\nat pre delete-character \"X\"\n\
                at 0.1s accept\nat 0.1s chat \"b\"\nend\n";
    let mut resolved = resolve(text, &mut EmptyResolver);
    resolved.set_plays_input(false);
    let mut silent = InputPlayer::new(Arc::new(resolved.clone()));
    assert!(silent.is_done(), "nothing to play without plays_input");
    assert!(silent.next_pre().is_none() && silent.next_due(u64::MAX).is_none());
    resolved.set_plays_input(true);
    let mut p = InputPlayer::new(Arc::new(resolved));
    assert_eq!(p.tick_ms(), 100);
    let pre = p.next_pre().expect("pre line");
    assert_eq!(
        (pre.line, pre.command.map(|c| c.name())),
        (7, Some("DeleteCharacter"))
    );
    assert!(p.next_pre().is_none());
    // Tick 1 holds the lines at 0 s and 0.1 s: `a` (time 0), then `accept` and `b` (file
    // order at 0.1 s); `accept` has no quest-offer window.
    let names: Vec<(u32, Option<&str>)> = std::iter::from_fn(|| p.next_due(1))
        .map(|l| (l.line, l.command.map(|c| c.name())))
        .collect();
    assert_eq!(names, vec![(6, Some("Chat")), (8, None), (9, Some("Chat"))]);
    p.skip(8, "target does not exist");
    assert_eq!(p.skipped(), 1);
    assert!(p.next_due(2).is_none(), "0.25 s is due in tick 3");
    assert_eq!(p.next_due(3).map(|l| l.line), Some(5));
    assert!(p.is_done());
}
