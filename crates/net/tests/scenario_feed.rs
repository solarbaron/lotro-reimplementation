// SPDX-License-Identifier: GPL-3.0-or-later
//! Behaviour of the scenario feed in the sandbox backend (`docs/specs/scenario-feed.md`
//! §5–§7, §11): triggers on matching commands, generic answers, limits and run-time
//! errors, resolution notes, and the windowed rules after a run finished.
#![cfg(feature = "scenario")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{
    Attribute, Candidate, CandidateOutcome, CandidateStatus, EmptyResolver, FinishReason,
    HeadlessDriver, LabelContext, ResolveError, ResolvedScenario, Scenario, ScenarioStatus,
    SelectorQuery, SelectorResolver,
};
use tessera_net::{ChatChannel, CombatCommand, Command, DataId, Event, NpcCommand, Session};

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

fn lacks(log: &str, needle: &str) {
    assert!(!log.contains(needle), "unexpected `{needle}` in:\n{log}");
}

/// A resolver with an install that offers `n` candidates for every query, knows the
/// labels `@Known` (value 7) and `m`, and reports `@Missing` as unmatched.
#[derive(Debug)]
struct Synthetic {
    n: u32,
    status: CandidateStatus,
}

impl SelectorResolver for Synthetic {
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
        out.extend((0..n).rev().map(|i| Candidate {
            did: DataId(0x7000_0000 + u32::try_from(i).unwrap_or(0)),
            row: 0,
            location: None,
            anchor: None,
        }));
        let mut o = CandidateOutcome::new(self.status.clone());
        for f in query.filters {
            if let tessera_net::scenario::FilterValue::Alternatives(alts) = &f.value {
                for a in alts {
                    if let tessera_net::scenario::Alt::Label(l) = a
                        && l == "@Missing"
                    {
                        o.unmatched_labels.push((f.attribute, l.clone()));
                    }
                }
            }
        }
        o
    }

    fn enum_value(&mut self, context: LabelContext, label: &str) -> Option<u32> {
        match (context, label) {
            (_, "@Known") => Some(7),
            (LabelContext::Sex, "m") => Some(1),
            _ => None,
        }
    }

    fn display_name(&mut self, _did: DataId, _row: Option<u32>) -> Option<String> {
        None
    }
}

#[test]
fn triggers_fire_on_matching_commands() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa"
actor $npc kind=npc name="Npc" pos=1,0,0
item $sword "Sword"
item $bread "Bread" qty=4
at 0s spawn $npc
+0s give $sword
+0s give $bread
+0s vendor items="Ale","Mead" npc=$npc
+0s loot items="Coin","Gem" from=$npc
+0s trade $npc
+0s travel-list dest="North","South"
+0s instances "Cave"
+0s trainer skills="Sing"
on use-item $sword target=none
+0s system "use-item"
end
on target any
+0s system "target"
end
on buy entry=2
+0s system "buy 2"
end
on sell $bread
+0s system "sell"
end
on take-loot entry=all
+0s system "take all"
end
on equip $sword
+0s system "equip"
end
on split $bread
+0s system "split"
end
on chat channel=named:"Lore" text="hello"
+0s system "named chat"
end
on chat channel=tell
+0s system "any tell"
end
on join-channel "Lore"
+0s system "joined"
end
on invite "Bram"
+0s system "invited"
end
on answer-invite decline
+0s system "declined"
end
on open-trade $npc
+0s system "open trade"
end
on offer-trade
+0s system "offered"
end
on trade-ready
+0s system "ready"
end
on friend "Bram"
+0s system "friend"
end
on choose-travel entry=1
+0s system "travel"
end
on join-instance
+0s system "instance"
end
on train entry=1
+0s system "train"
end
on slot-trait 3
+0s system "trait"
end
on craft "Stew"
+0s system "craft"
end
on close vendor
+0s system "closed vendor"
end
on dismount
+0s system "dismount"
end
input
at 0.1s use-item $sword target=none
at 0.1s target $npc
at 0.2s buy 2
at 0.2s buy 1
at 0.2s sell $bread
at 0.3s take-loot all
at 0.3s equip $sword slot=5
at 0.3s split $bread 1
at 0.4s chat "hello" channel=named:"Lore"
at 0.4s chat "psst" channel=tell:"Bram"
at 0.5s join-channel "Lore"
at 0.5s invite "Bram"
at 0.5s answer-invite decline
at 0.6s open-trade $npc
at 0.6s offer-trade coin=5
at 0.6s trade-ready on
at 0.6s friend "Bram"
at 0.7s choose-travel 1
at 0.7s join-instance 1
at 0.7s train 1
at 0.7s slot-trait 3 "Brave"
at 0.7s craft "Stew"
at 0.8s close vendor
at 0.8s dismount
end
"#;
    let log = run(text);
    for msg in [
        "use-item",
        "target",
        "buy 2",
        "sell",
        "take all",
        "equip",
        "split",
        "named chat",
        "any tell",
        "joined",
        "invited",
        "declined",
        "open trade",
        "offered",
        "ready",
        "friend",
        "travel",
        "instance",
        "train",
        "trait",
        "craft",
        "closed vendor",
        "dismount",
    ] {
        assert!(
            log.contains(&format!(
                "ChatMessage channel=system sender=none text=\"{msg}\""
            )),
            "trigger `{msg}` did not fire:\n{log}"
        );
    }
    // Only entry 2 fired the buy trigger.
    assert_eq!(log.matches("text=\"buy 2\"").count(), 1, "{log}");
    // Generic answers ran too.
    has(&log, "0.100 TargetChanged entity=player target=$npc");
    has(&log, "0.300 LootTaken window=2 entry=1");
    has(&log, "0.300 ItemMoved item=$sword at=equipment:5");
    has(&log, "0.300 ItemQuantity item=$bread quantity=3");
    has(
        &log,
        "0.400 ChatMessage channel=named:\"Lore\" sender=\"Testa\" text=\"hello\"",
    );
    has(
        &log,
        "0.400 ChatMessage channel=system sender=none text=\"Bram is not in the sandbox.\"",
    );
    has(&log, "0.800 WindowClosed window=1");
    has(
        &log,
        "0.800 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0",
    );
}

#[test]
fn first_matching_trigger_wins_and_times_limits_firings() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa"
on cancel-cast times=2
+0s system "first"
end
on cancel-cast
+0s system "second"
end
input
at 0.1s cancel-cast
at 0.2s cancel-cast
at 0.3s cancel-cast
end
"#;
    let log = run(text);
    has(
        &log,
        "0.100 ChatMessage channel=system sender=none text=\"first\"",
    );
    has(
        &log,
        "0.200 ChatMessage channel=system sender=none text=\"first\"",
    );
    has(
        &log,
        "0.300 ChatMessage channel=system sender=none text=\"second\"",
    );
    assert_eq!(log.matches("\"first\"").count(), 2);
}

#[test]
fn non_matching_commands_fire_nothing() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa"
actor $a kind=npc name="A" pos=1,0,0
actor $b kind=npc name="B" pos=2,0,0
on use $a
+0s system "a"
end
on use-skill "Kindle" target=none
+0s system "kindle"
end
on chat channel=say text="exact"
+0s system "exact"
end
input
at 0.1s use $b
at 0.1s use-skill "Kindle" target=$a
at 0.1s use-skill "Other" target=none
at 0.1s chat "not exact"
at 0.1s chat "exact" channel=yell
end
"#;
    let log = run(text);
    lacks(&log, "text=\"a\"");
    lacks(&log, "text=\"kindle\"");
    assert!(!log.contains("sender=none text=\"exact\""), "{log}");
}

#[test]
fn defaults_off_gives_no_generic_answers() {
    let text = r#"tessera-scenario 1
tick 100ms
defaults off
player "Testa"
actor $a kind=npc name="A" pos=1,0,0
input
at 0.1s target $a
at 0.1s chat "hello"
at 0.1s quickslot 1 "Kindle"
end
"#;
    let log = run(text);
    has(&log, "0.100 > SelectTarget target=$a");
    lacks(&log, "TargetChanged");
    lacks(&log, "ChatMessage");
    lacks(&log, "QuickslotChanged");
}

#[test]
fn delete_character_trigger_and_pre_world_limits() {
    let text = r#"tessera-scenario 1
player "Testa"
on delete-character "Old"
end
on create-character
+0s accept-character "New" species=@Known sex=m class=7
end
input
at pre delete-character "Old"
at pre create-character "New"
end
"#;
    let (log, code) = run_resolved(resolve(
        text,
        &mut Synthetic {
            n: 0,
            status: CandidateStatus::Ready,
        },
    ));
    assert_eq!(code, 0);
    has(&log, "pre > DeleteCharacter name=\"Old\"");
    has(
        &log,
        "pre CharacterInfo name=\"New\" body=character:7:1 class=7 level=none",
    );
}

#[test]
fn spilled_events_are_counted_and_drained() {
    let mut text = String::from("tessera-scenario 1\nplayer \"Testa\"\n");
    for _ in 0..5000 {
        text.push_str("+0s system \"x\"\n");
    }
    let log = run(&text);
    assert_eq!(log.matches("0.000 ChatMessage").count(), 4096);
    assert_eq!(log.matches("0.050 ChatMessage").count(), 904);
    has(
        &log,
        "0.050 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=904",
    );
}

#[test]
fn weight_budget_limits_large_events_per_tick() {
    let big = "y".repeat(1024);
    let mut text = String::from(
        "tessera-scenario 1\ntick 1s\nplayer \"Testa\"\nat 0.001s repeat 300 every 1ms\n",
    );
    for _ in 0..13 {
        text.push_str(&format!("+0s system \"{big}\"\n"));
    }
    text.push_str("end\n");
    let log = run(&text);
    // 3,900 lines in tick 1, each of weight 1 + 16 = 17: 65,536 / 17 = 3,855 per tick.
    assert_eq!(log.matches("1.000 ChatMessage").count(), 3855);
    assert_eq!(log.matches("2.000 ChatMessage").count(), 45);
    has(
        &log,
        "2.000 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=45",
    );
}

#[test]
fn agenda_overflow_stops_the_feed() {
    let body: String = (0..10).map(|_| "+0s system \"x\"\n").collect();
    let text = format!(
        "tessera-scenario 1\ntick 1s\nplayer \"Testa\"\n\
         at 30min repeat 10000 every 10ms\n{body}end\n\
         on cancel-cast\n+1min repeat 10000 every 10ms\n{body}end\nend\n\
         input\nat 1s cancel-cast\nat 1s cancel-cast\nend\n"
    );
    let (log, code) = run_resolved(resolve(&text, &mut EmptyResolver));
    assert_eq!(code, 5);
    has(
        &log,
        "1.000 Disconnected reason=server-closed:\"scenario error: AgendaOverflow at line 16\"",
    );
    assert!(
        log.lines()
            .last()
            .is_some_and(|l| l.starts_with("1.000 end reason=error")
                && l.ends_with("error=AgendaOverflow line=16")),
        "{log}"
    );
}

#[test]
fn queue_overflow_stops_the_feed() {
    let text = "tessera-scenario 1\ntick 1s\nplayer \"Testa\"\nselect $s = skill all\n\
                at 0.001s repeat 20 every 1ms\n+0s learn $s\nend\n";
    let resolved = resolve(
        text,
        &mut Synthetic {
            n: 65_536,
            status: CandidateStatus::Ready,
        },
    );
    let (log, code) = run_resolved(resolved);
    assert_eq!(code, 5);
    assert!(
        log.contains("scenario error: QueueOverflow at line 6"),
        "{}",
        log.lines().rev().take(3).collect::<Vec<_>>().join("\n")
    );
    // Nothing of the failing tick was delivered.
    lacks(&log, "SkillLearned");
}

#[test]
fn resolution_notes_and_errors() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa" class=@Missing
select $a = skill class=@Missing|@Known n=5
select $b = item all
at 0s give $b
+0s vendor items=$b
+0s trade-offer own items=$b
"#;
    let r = resolve(
        text,
        &mut Synthetic {
            n: 70_000,
            status: CandidateStatus::Ready,
        },
    );
    let notes: Vec<String> = r.notes().iter().map(ToString::to_string).collect();
    assert_eq!(
        notes,
        [
            "resolve $a line=4 family=skill requested=5 candidates=70000 got=5",
            "resolve $b line=5 family=item requested=all candidates=70000 got=70000",
            "label line=3 attribute=class label=@Missing",
            "label line=4 attribute=class label=@Missing",
            "fanout line=6 cut=4464",
            "window line=7 cut=68976",
            "window line=8 cut=69968",
        ]
        .map(String::from)
    );
    let r = resolve(
        "tessera-scenario 1\nselect $a = skill n=3\n",
        &mut Synthetic {
            n: 2,
            status: CandidateStatus::Truncated,
        },
    );
    assert_eq!(
        r.notes()[0].to_string(),
        "resolve $a line=2 family=skill requested=3 candidates=2 got=2 note=truncated"
    );
    let sc = Scenario::parse(b"tessera-scenario 1\nselect $a = skill all\n").unwrap();
    assert!(matches!(
        sc.resolve(&mut Synthetic {
            n: 1,
            status: CandidateStatus::Failed("broken".into()),
        }),
        Err(ResolveError::InstallUnreadable { line: 2, .. })
    ));
    let sc = Scenario::parse(b"tessera-scenario 1\nrequires install\n").unwrap();
    assert_eq!(
        sc.resolve(&mut EmptyResolver).unwrap_err(),
        ResolveError::InstallRequired
    );
    let many: String = (0..17)
        .map(|i| format!("select $s{i} = skill n=65536\n"))
        .collect();
    let sc = Scenario::parse(format!("tessera-scenario 1\n{many}").as_bytes()).unwrap();
    assert_eq!(
        sc.resolve(&mut Synthetic {
            n: 65_536,
            status: CandidateStatus::Ready,
        })
        .unwrap_err(),
        ResolveError::ResolveLimit { line: 18 }
    );
    let _ = Attribute::Class;
}

#[test]
fn teleport_step_moves_the_player_for_the_sandbox() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa"
actor $a kind=npc name="A" pos=0,5,0
at 0s teleport 10,0,0 facing=0
+0s spawn $a
+0s face $a player
"#;
    let log = run(text);
    has(
        &log,
        "0.000 LocationReport entity=player at=0:10.000,0.000,0.000 facing=0.00 anchor=none cause=teleported",
    );
    has(
        &log,
        "0.000 EntityMoved entity=player at=10.000,0.000,0.000 facing=0.00 velocity=0.000,0.000,0.000",
    );
    // Facing the player at (10, 0) from (0, 5): south-east.
    assert!(
        log.contains("EntityMoved entity=$a at=0.000,5.000,0.000 facing=116.57"),
        "{log}"
    );
}

#[test]
fn inventory_and_combat_edge_cases() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa"
actor $foe kind=creature name="Foe" pos=3,0,0 level=7
item $helm "Helm"
at 0s spawn $foe
+0s give $helm to=equipment slot=4
+0s cast $foe "Bolt" 1s target=player
+0.2s delay $foe 500ms
+0s effect player "Ward" for=1s
+0s effect player "Ward"
+0.1s effect-end player "Ward"
+0s interrupt player
+0s cooldown "Bolt" 1s
+0.5s cooldown "Bolt" clear
+0s close loot
+0s loot-taken 1
"#;
    let log = run(text);
    has(&log, "0.000 LevelChanged entity=$foe level=7");
    has(
        &log,
        "0.000 ItemAdded item=$helm what=\"Helm\" at=equipment:4 quantity=1",
    );
    has(&log, "0.200 CastDelayed entity=$foe added_ms=500");
    has(&log, "0.300 EffectRemoved entity=player effect=2");
    has(&log, "0.800 CooldownEnded what=\"Bolt\"");
    // The delayed cast ends at 1.5; the first Ward expires at 1.2.
    has(&log, "1.200 EffectRemoved entity=player effect=1");
    has(&log, "1.500 CastEnded entity=$foe outcome=completed");
    // interrupt (no player cast), close loot, loot-taken: three skips.
    assert!(
        log.ends_with("1.500 end reason=complete skipped=3 input-skipped=0 dropped=0 spilled=0\n"),
        "{log}"
    );
    assert_eq!(log.matches("CooldownEnded").count(), 1, "{log}");
}

#[test]
fn input_lines_that_cannot_be_sent_are_skipped() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa"
input
at 0.1s accept
at 0.1s buy 1
at 0.1s use-skill quickslot=3
at pre cancel-cast
end
"#;
    let log = run(text);
    has(
        &log,
        "0.100 end reason=complete skipped=0 input-skipped=4 dropped=0 spilled=0",
    );
}

fn poll_until(session: &mut Session, now: &mut Instant, mut f: impl FnMut(&Event) -> bool) -> bool {
    let mut events = Vec::new();
    for _ in 0..200 {
        session.poll_into(*now, &mut events);
        if events.drain(..).any(|e| f(&e)) {
            return true;
        }
        *now += Duration::from_millis(50);
    }
    false
}

fn status(session: &Session) -> Option<ScenarioStatus> {
    session
        .backend::<SandboxBackend>()
        .and_then(SandboxBackend::scenario_status)
}

/// Windowed rules: after `Complete` without `end-at`, triggers still fire; after
/// `end-at`, the sandbox's own chat echo returns and the feed answers nothing.
#[test]
fn windowed_runs_after_finishing() {
    let text = r#"tessera-scenario 1
tick 50ms
player "Testa"
actor $a kind=npc name="A" pos=1,0,0
on use $a
+0.1s system "used"
end
"#;
    let mut r = resolve(text, &mut EmptyResolver);
    r.set_plays_input(false);
    let r = Arc::new(r);
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&r)),
        ..SandboxConfig::default()
    };
    let mut session = Session::new(SandboxBackend::new(config).unwrap());
    let mut now = Instant::now();
    session.send(now, Command::Connect).unwrap();
    let _ = session.poll(now);
    session
        .send(
            now,
            Command::EnterWorld {
                character: "Testa".into(),
            },
        )
        .unwrap();
    let _ = session.poll(now);
    assert!(matches!(
        status(&session),
        Some(ScenarioStatus::Finished {
            reason: FinishReason::Complete,
            ..
        })
    ));
    let actor = tessera_net::EntityId(tessera_net::scenario::ACTOR_ID_BASE + 1);
    session
        .send(now, Command::Npc(NpcCommand::UseEntity { entity: actor }))
        .unwrap();
    assert!(poll_until(&mut session, &mut now, |e| matches!(
        e,
        Event::ChatMessage { text, .. } if text == "used"
    )));
    assert!(matches!(
        status(&session),
        Some(ScenarioStatus::Finished { .. })
    ));

    // end-at: terminal.
    let text = "tessera-scenario 1\ntick 50ms\nend-at 100ms\nplayer \"Testa\"\n";
    let r = Arc::new(resolve(text, &mut EmptyResolver));
    let config = SandboxConfig {
        scenario: Some(r),
        ..SandboxConfig::default()
    };
    let mut session = Session::new(SandboxBackend::new(config).unwrap());
    let mut now = Instant::now();
    session.send(now, Command::Connect).unwrap();
    let _ = session.poll(now);
    session
        .send(
            now,
            Command::EnterWorld {
                character: "Testa".into(),
            },
        )
        .unwrap();
    for _ in 0..4 {
        let _ = session.poll(now);
        now += Duration::from_millis(50);
    }
    assert!(matches!(
        status(&session),
        Some(ScenarioStatus::Finished {
            time_ms: 100,
            reason: FinishReason::Complete
        })
    ));
    session
        .send(
            now,
            Command::Chat {
                channel: ChatChannel::Say,
                text: "echo".into(),
            },
        )
        .unwrap();
    session
        .send(now, Command::Combat(CombatCommand::CancelCast))
        .unwrap();
    let events = session.poll(now);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ChatMessage { channel: ChatChannel::Say, text, .. } if text == "echo"
    )));
    assert_eq!(events.len(), 1, "{events:?}");
}

#[test]
fn sandbox_without_scenario_ignores_new_commands() {
    let mut session = Session::new(SandboxBackend::new(SandboxConfig::default()).unwrap());
    let now = Instant::now();
    session.send(now, Command::Connect).unwrap();
    let _ = session.poll(now);
    session
        .send(
            now,
            Command::EnterWorld {
                character: "Wanderer".into(),
            },
        )
        .unwrap();
    let _ = session.poll(now);
    session
        .send(now, Command::Combat(CombatCommand::CancelCast))
        .unwrap();
    let events = session.poll(now);
    assert!(!events.iter().any(|e| matches!(e, Event::Combat(_))));
    assert!(status(&session).is_none());
}

#[test]
fn session_validates_new_commands() {
    let mut session = Session::new(SandboxBackend::new(SandboxConfig::default()).unwrap());
    let now = Instant::now();
    // Character commands only while connected.
    let create = Command::Character(tessera_net::CharacterCommand::CreateCharacter {
        name: "A".into(),
        appearance: None,
        class: None,
    });
    assert!(session.send(now, create.clone()).is_err());
    session.send(now, Command::Connect).unwrap();
    let _ = session.poll(now);
    session.send(now, create).unwrap();
    let bad = Command::Character(tessera_net::CharacterCommand::CreateCharacter {
        name: String::new(),
        appearance: None,
        class: None,
    });
    assert!(session.send(now, bad).is_err());
    // In-world commands are refused before world entry.
    assert!(
        session
            .send(now, Command::Combat(CombatCommand::CancelCast))
            .is_err()
    );
    session
        .send(
            now,
            Command::EnterWorld {
                character: "Wanderer".into(),
            },
        )
        .unwrap();
    let _ = session.poll(now);
    let named = Command::Chat {
        channel: ChatChannel::Named {
            name: String::new(),
        },
        text: "x".into(),
    };
    assert!(session.send(now, named).is_err());
    let long_ref = Command::Combat(CombatCommand::UseSkill {
        what: tessera_net::domain::Ref::Literal("x".repeat(65)),
        target: None,
    });
    assert!(session.send(now, long_ref).is_err());
}
