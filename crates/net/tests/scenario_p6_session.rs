// SPDX-License-Identifier: GPL-3.0-or-later
//! Session and run-time rules of the "P6 progression" revision
//! (`docs/specs/scenario-feed.md` §6.6, §7.5, §7.7, §8.6, §9.5, §18.P6): the new
//! Progression events follow the entity rule, invalid P6 values are dropped and literal refs
//! cleaned, the three new commands need `InWorld` and are range-checked, the plain sandbox
//! ignores them, the test-server backend refuses them, and the
//! feed plays the new steps, labels, patterns and input forms as scripted.
#![cfg(feature = "scenario")]

use std::sync::Arc;
use std::time::Instant;

use tessera_net::domain::{
    Batch, CharacterEvent, CraftOutcome, CraftTrainerKind, MAX_LITERAL_REF_LEN, Ref, Text,
    TrainerEntry, TrainerFlags, TrainerOffer, UntrainedEntry, WindowId,
};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{
    Candidate, CandidateOutcome, CandidateStatus, EmptyResolver, Family, HeadlessDriver,
    LabelContext, Scenario, SelectorQuery, SelectorResolver,
};
use tessera_net::{
    Backend, Command, DataId, EntityId, Event, NetError, ProgressionCommand, ProgressionEvent,
    Session, Transform,
};

/// A backend that replays a script, to check what `Session` lets through.
#[derive(Debug)]
struct Scripted(Vec<Vec<Event>>);

impl Backend for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }
    fn submit(&mut self, _now: Instant, _command: Command) -> Result<(), NetError> {
        Ok(())
    }
    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        if !self.0.is_empty() {
            out.extend(self.0.remove(0));
        }
    }
}

const P: EntityId = EntityId(1);

fn lit(s: &str) -> Ref {
    Ref::Literal(s.to_owned())
}

fn prog(e: ProgressionEvent) -> Event {
    Event::Progression(e)
}

fn trainer(flags: u8, rank: Option<u16>) -> Event {
    prog(ProgressionEvent::TrainerShown {
        window: WindowId(1),
        trainer: None,
        entries: vec![TrainerEntry {
            entry: 1,
            what: lit("Kick"),
            coin: 5,
            level: 2,
            rank,
            flags: TrainerFlags(flags),
        }],
        offers: TrainerOffer::Traits,
    })
}

fn untrained(n: usize, level: u32, what: Ref) -> Event {
    prog(ProgressionEvent::UntrainedSkills {
        entries: (0..n)
            .map(|i| UntrainedEntry {
                entry: u16::try_from(i + 1).unwrap_or(u16::MAX),
                what: what.clone(),
                coin: 0,
                level,
                rank: None,
            })
            .collect(),
    })
}

fn result(batch: Option<Batch>) -> Event {
    prog(ProgressionEvent::CraftResult {
        recipe: lit("Cup"),
        outcome: CraftOutcome::Success,
        item: None,
        batch,
    })
}

fn names(ev: &[Event]) -> Vec<&'static str> {
    ev.iter()
        .filter(|e| matches!(e, Event::Progression(_)))
        .map(Event::name)
        .collect()
}

#[test]
fn session_applies_the_p6_event_rules() {
    let long = "x".repeat(100);
    let script = vec![
        vec![Event::Connected { characters: vec![] }],
        vec![
            // Before world entry the entity rule drops them.
            untrained(1, 1, lit("Kick")),
            prog(ProgressionEvent::RecipeForgotten { what: lit("Cup") }),
            Event::WorldEntered {
                player: P,
                name: "P".into(),
                transform: Transform::default(),
            },
            // Valid.
            untrained(0, 0, lit("Kick")),
            untrained(1024, 10_000, lit("Kick")),
            trainer(0b11, Some(1000)),
            prog(ProgressionEvent::TraitRank {
                tree: lit("T"),
                what: lit("A"),
                rank: 3,
            }),
            prog(ProgressionEvent::TraitTreeChanged {
                tree: None,
                branch: None,
            }),
            prog(ProgressionEvent::VocationChanged {
                vocation: Some(lit("Potter")),
                slots: Some(32),
            }),
            prog(ProgressionEvent::CraftTrainerShown {
                window: WindowId(2),
                kind: CraftTrainerKind::Guild,
                what: lit("Guild"),
                npc: None,
            }),
            result(Some(Batch { index: 2, count: 2 })),
            // Cleaned: literal refs cut to 64 bytes.
            untrained(1, 1, lit(&long)),
            prog(ProgressionEvent::RecipeLearned {
                what: lit("Cup"),
                profession: Some(lit(&long)),
                tier: Some(1000),
                category: Some(7),
            }),
            // Invalid: dropped.
            trainer(0b100, None),
            trainer(0, Some(1001)),
            untrained(1025, 0, lit("Kick")),
            untrained(1, 10_001, lit("Kick")),
            prog(ProgressionEvent::VocationChanged {
                vocation: None,
                slots: Some(33),
            }),
            result(Some(Batch { index: 0, count: 2 })),
            result(Some(Batch { index: 3, count: 2 })),
            prog(ProgressionEvent::ProfessionChanged {
                profession: lit("Pottery"),
                tier: 1,
                proficiency: 0,
                proficiency_max: 1,
                mastery: 0,
                mastery_max: 1,
                mastery_tier: Some(1001),
            }),
            prog(ProgressionEvent::RecipeLearned {
                what: lit("Cup"),
                profession: None,
                tier: Some(1001),
                category: None,
            }),
            // `LevelChanged … quiet` keeps the Character-group rule.
            Event::Character(CharacterEvent::LevelChanged {
                entity: P,
                level: 9,
                quiet: true,
            }),
        ],
    ];
    let mut s = Session::new(Scripted(script));
    let t = Instant::now();
    s.send(t, Command::Connect).unwrap();
    assert_eq!(s.poll(t).len(), 1);
    s.send(
        t,
        Command::EnterWorld {
            character: "P".into(),
        },
    )
    .unwrap();
    let ev = s.poll(t);
    assert_eq!(
        names(&ev),
        [
            "UntrainedSkills",
            "UntrainedSkills",
            "TrainerShown",
            "TraitRank",
            "TraitTreeChanged",
            "VocationChanged",
            "CraftTrainerShown",
            "CraftResult",
            "UntrainedSkills",
            "RecipeLearned",
        ],
        "{ev:#?}"
    );
    let cleaned: Vec<&ProgressionEvent> = ev
        .iter()
        .filter_map(|e| match e {
            Event::Progression(p) => Some(p),
            _ => None,
        })
        .collect();
    let ProgressionEvent::UntrainedSkills { entries } = cleaned[8] else {
        panic!("{:?}", cleaned[8]);
    };
    assert!(matches!(&entries[0].what, Ref::Literal(a) if a.len() == MAX_LITERAL_REF_LEN));
    let ProgressionEvent::RecipeLearned { profession, .. } = cleaned[9] else {
        panic!("{:?}", cleaned[9]);
    };
    assert!(matches!(profession, Some(Ref::Literal(a)) if a.len() == MAX_LITERAL_REF_LEN));
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Character(CharacterEvent::LevelChanged { quiet: true, .. })
    )));
}

fn craft(tier: Option<u16>, output: u16) -> Command {
    Command::Progression(ProgressionCommand::Craft {
        recipe: lit("Cup"),
        quantity: 2,
        tier,
        output,
        optional: 0xFF,
        pack: true,
    })
}

fn set_rank(rank: u16, tree: Ref) -> Command {
    Command::Progression(ProgressionCommand::SetTraitRank {
        tree,
        what: lit("A"),
        rank,
    })
}

fn new_commands() -> [Command; 3] {
    [
        Command::Progression(ProgressionCommand::TrainCraft {
            window: WindowId(1),
            replace: Some(lit("Pottery")),
        }),
        set_rank(3, lit("T")),
        Command::Progression(ProgressionCommand::ChooseTraitBranch {
            tree: lit("T"),
            branch: 4,
        }),
    ]
}

#[test]
fn p6_commands_need_the_world_and_are_checked() {
    let cmd_names: Vec<&str> = new_commands().iter().map(Command::name).collect();
    assert_eq!(
        cmd_names,
        ["TrainCraft", "SetTraitRank", "ChooseTraitBranch"]
    );
    let mut s = Session::new(SandboxBackend::new(SandboxConfig::default()).expect("sandbox"));
    let t = Instant::now();
    for c in new_commands() {
        assert!(matches!(s.send(t, c), Err(NetError::InvalidState { .. })));
    }
    s.send(t, Command::Connect).unwrap();
    let character = s
        .poll(t)
        .iter()
        .find_map(|e| match e {
            Event::Connected { characters } => characters.first().map(|c| c.name.clone()),
            _ => None,
        })
        .expect("a character");
    s.send(t, Command::EnterWorld { character }).unwrap();
    let _ = s.poll(t);
    for bad in [
        set_rank(1001, lit("T")),
        set_rank(1, lit("")),
        set_rank(1, lit(&"x".repeat(65))),
        craft(Some(1001), 0),
        craft(None, 65),
        Command::Progression(ProgressionCommand::TrainCraft {
            window: WindowId(1),
            replace: Some(lit("a\nb")),
        }),
        Command::Progression(ProgressionCommand::ChooseTraitBranch {
            tree: lit(""),
            branch: 0,
        }),
    ] {
        assert!(
            matches!(s.send(t, bad.clone()), Err(NetError::InvalidCommand(_))),
            "{bad:?}"
        );
    }
    // The sandbox without a scenario accepts and ignores them (§9.5).
    for c in new_commands() {
        s.send(t, c).unwrap();
    }
    s.send(t, craft(Some(1000), 64)).unwrap();
    s.send(t, set_rank(1000, Ref::Record(DataId(5)))).unwrap();
    assert!(names(&s.poll(t)).is_empty());
}

#[cfg(feature = "remote")]
mod remote_support;

/// The test-server backend refuses the three new commands like every other gameplay
/// command (§9.5) and never produces the P6 events.
#[cfg(feature = "remote")]
#[test]
fn the_test_server_backend_refuses_the_p6_commands() {
    use remote_support::{PeerOptions, connect, pump, spawn};
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        let _ = p.closed().await;
    });
    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    s.send(
        Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    pump(&mut s, |e| matches!(e, Event::WorldEntered { .. }));
    for c in new_commands().into_iter().chain([craft(Some(2), 1)]) {
        assert!(
            matches!(
                s.send(Instant::now(), c.clone()),
                Err(NetError::InvalidCommand(_))
            ),
            "{c:?}"
        );
    }
    s.send(Instant::now(), Command::Disconnect).unwrap();
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    peer.join();
}

// --- The feed beyond the vectors --------------------------------------------------------

/// A resolver with an install: `skill` answers `skills` candidates; labels resolve from a
/// fixed table.
struct Install {
    skills: u32,
}

impl SelectorResolver for Install {
    fn has_install(&self) -> bool {
        true
    }

    fn candidates(
        &mut self,
        query: &SelectorQuery<'_>,
        _limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        let n = match query.family {
            Family::Skill => self.skills,
            Family::Recipe | Family::Vocation | Family::CraftGuild => 2,
            _ => 0,
        };
        for i in 0..n {
            out.push(Candidate {
                did: DataId(0x7000_0000 + i),
                row: 0,
                location: None,
                anchor: None,
            });
        }
        CandidateOutcome::new(CandidateStatus::Ready)
    }

    fn enum_value(&mut self, context: LabelContext, label: &str) -> Option<u32> {
        match (context, label) {
            (LabelContext::TraitBranch, "@Left") => Some(4),
            (LabelContext::RecipeCategory, "@Food") => Some(7),
            (LabelContext::NpcAction, "@Train") => Some(38),
            _ => None,
        }
    }

    fn display_name(&mut self, _did: DataId, _row: Option<u32>) -> Option<String> {
        None
    }
}

fn play(text: &str, resolver: &mut dyn SelectorResolver) -> String {
    let scenario = Scenario::parse(text.as_bytes()).expect("parse");
    let resolved = Arc::new(scenario.resolve(resolver).expect("resolve"));
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let session = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let mut driver = HeadlessDriver::new(session, resolved, Some(Vec::new()));
    let _ = driver.run();
    String::from_utf8(driver.into_log().expect("log")).expect("utf-8")
}

fn has(log: &str, line: &str) {
    assert!(log.lines().any(|l| l == line), "no line {line:?} in\n{log}");
}

#[test]
fn untrained_lists_are_cut_at_1024_with_a_window_note() {
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
                select $s = skill all\nat 0s untrained skills=$s coin=5 rank=0,1\n";
    let log = play(text, &mut Install { skills: 1100 });
    has(&log, "window line=5 cut=76");
    let line = log
        .lines()
        .find(|l| l.starts_with("0.000 UntrainedSkills "))
        .expect("UntrainedSkills");
    assert_eq!(line.matches("{entry=").count(), 1024);
    assert!(line.contains("{entry=1 what=0x70000000 coin=5 level=0 rank=0}"));
    assert!(line.contains("{entry=1024 what=0x700003FF coin=5 level=0 rank=1}"));
}

#[test]
fn labels_resolve_or_leave_notes() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa"
select $r = recipe all
page $p "Lessons?"
option $p 1 action(@Train,"Pottery")
option $p 2 action(@Teach,$r[1])
at 0s recipe $r profession="Pottery" tier=3 category=@Food
+0s recipe "Mug" category=@Drink
+0s trait-tree "Valour" branch=@Left
+0s trait-tree "Valour" branch=@Right
+0s show $p
on trait-branch tree="Valour"
+0s trait-tree "Valour" branch=9
end
input
at 0.1s trait-branch "Valour" @Left
at 0.2s trait-branch "Valour" @Right
end
"#;
    let log = play(text, &mut Install { skills: 0 });
    has(&log, "label line=7 attribute=action label=@Teach");
    has(&log, "label line=9 attribute=category label=@Drink");
    has(&log, "label line=11 attribute=branch label=@Right");
    has(&log, "label line=18 attribute=branch label=@Right");
    has(
        &log,
        "0.000 RecipeLearned what=0x70000000 profession=\"Pottery\" tier=3 category=7",
    );
    has(
        &log,
        "0.000 RecipeLearned what=0x70000001 profession=\"Pottery\" tier=3 category=7",
    );
    has(&log, "0.000 RecipeLearned what=\"Mug\"");
    has(&log, "0.000 TraitTreeChanged tree=\"Valour\" branch=4");
    has(&log, "0.000 TraitTreeChanged tree=\"Valour\" branch=none");
    has(
        &log,
        "0.000 DialogueShown window=1 speaker=none text=\"Lessons?\" options=[{option=1 text=action(38,\"Pottery\")},{option=2 text=\"Teach\"}]",
    );
    has(&log, "0.100 > ChooseTraitBranch tree=\"Valour\" branch=4");
    has(&log, "0.100 TraitTreeChanged tree=\"Valour\" branch=9");
    // The unresolved label skips the input line.
    assert!(
        log.ends_with("input-skipped=1 dropped=0 spilled=0\n"),
        "{log}"
    );
}

#[test]
fn patterns_filter_and_steps_play_as_scripted() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa"
actor $smith kind=npc name="Smith" pos=2,0,0
at 0s spawn $smith
+0s untrained none
+0s vocation none
+0s trait-points 3/8
+0s trainer skills="Bowl","Jug" offers=traits rank=5 unmet=1,2 trained=2
+0s craft-trainer vocation "Potter" npc=$smith
+0s craft-trainer guild "Guild"
+0s level player 3
on train-craft replace=none
+0s system "none"
end
on train-craft replace="Pottery"
+0s system "pottery"
end
on train-craft
+0s system "other"
end
on trait-rank "Steady" tree="Valour"
+0s system "steady"
end
on trait-rank
+0s system "any rank"
end
on close craft-trainer
+0s forget-recipe "Mug"
end
input
at 0.1s train-craft
at 0.2s train-craft replace="Pottery"
at 0.3s train-craft replace="Woodwork"
at 0.4s trait-rank "Valour" "Steady" 1
at 0.5s trait-rank "Other" "Steady" 1
at 0.6s craft "Cup"
at 0.7s close craft-trainer
at 0.8s train-craft
end
"#;
    let log = play(text, &mut EmptyResolver);
    for line in [
        "0.000 UntrainedSkills entries=[]",
        "0.000 VocationChanged vocation=none slots=none",
        "0.000 TraitPoints available=3 total=8",
        "0.000 TrainerShown window=1 trainer=none entries=[{entry=1 what=\"Bowl\" coin=0 level=0 rank=5 flags=unmet},{entry=2 what=\"Jug\" coin=0 level=0 rank=5 flags=trained+unmet}] offers=traits",
        "0.000 CraftTrainerShown window=2 kind=vocation what=\"Potter\" npc=$smith",
        // A second step replaces the open window's content and keeps its id (§5.6).
        "0.000 CraftTrainerShown window=2 kind=guild what=\"Guild\" npc=none",
        "0.000 LevelChanged entity=player level=3",
        "0.100 > TrainCraft window=2 replace=none",
        "0.100 ChatMessage channel=system sender=none text=\"none\"",
        "0.200 > TrainCraft window=2 replace=\"Pottery\"",
        "0.200 ChatMessage channel=system sender=none text=\"pottery\"",
        "0.300 ChatMessage channel=system sender=none text=\"other\"",
        "0.400 > SetTraitRank tree=\"Valour\" what=\"Steady\" rank=1",
        "0.400 ChatMessage channel=system sender=none text=\"steady\"",
        "0.500 ChatMessage channel=system sender=none text=\"any rank\"",
        "0.600 > Craft recipe=\"Cup\" quantity=1",
        "0.700 > CloseWindow window=2",
        "0.700 WindowClosed window=2",
        "0.700 RecipeForgotten what=\"Mug\"",
    ] {
        has(&log, line);
    }
    // With the window closed the input form has no window: skipped, nothing sent.
    assert!(!log.contains("0.800 >"), "{log}");
    assert!(
        log.ends_with("input-skipped=1 dropped=0 spilled=0\n"),
        "{log}"
    );
}

#[test]
fn missing_elements_skip_the_step() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa"
select $s = skill n=1
page $p "Hi"
option $p 1 action(5,$s[0])
at 0s untrained skills=$s[0],"Kick"
+0s trait-rank $s[0] "A" 1
+0s craft-trainer profession $s[0]
+0s recipe "Cup" profession=$s[0]
+0s show $p
+0s untrained skills=$s
"#;
    let log = play(text, &mut EmptyResolver);
    assert!(!log.contains("TraitRank"), "{log}");
    assert!(!log.contains("CraftTrainerShown"), "{log}");
    assert!(!log.contains("RecipeLearned"), "{log}");
    assert!(!log.contains("DialogueShown"), "{log}");
    // An empty binding gives an empty list, not a skip.
    has(&log, "0.000 UntrainedSkills entries=[]");
    assert!(
        log.ends_with("skipped=5 input-skipped=0 dropped=0 spilled=0\n"),
        "{log}"
    );
}

/// The reading recorded as item (6) of the implementer OPEN in `scenario-feed.md` §16: for
/// `action(@Label,REF)` the REF element is checked before the label, so a missing element
/// makes the text missing (the step is skipped) even when the label is unresolved too. A
/// literal REF with an unresolved label still gives the label as a literal.
#[test]
fn action_for_with_a_missing_element_and_an_unresolved_label_is_missing() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa"
select $s = skill n=1
page $p "Hi"
option $p 1 action(@Train,$s[0])
page $q "Ho"
option $q 1 action(@Train,"Pottery")
at 0s show $p
+0s show $q
"#;
    let log = play(text, &mut EmptyResolver);
    assert!(!log.contains("text=\"Hi\""), "{log}");
    has(
        &log,
        "0.000 DialogueShown window=1 speaker=none text=\"Ho\" options=[{option=1 text=\"Train\"}]",
    );
    assert!(
        log.ends_with("skipped=1 input-skipped=0 dropped=0 spilled=0\n"),
        "{log}"
    );
}

#[test]
fn action_for_texts_are_cleaned() {
    use tessera_net::domain::{DialogueOption, NpcEvent};
    let long = "y".repeat(100);
    let script = vec![
        vec![Event::Connected { characters: vec![] }],
        vec![
            Event::WorldEntered {
                player: P,
                name: "P".into(),
                transform: Transform::default(),
            },
            Event::Npc(NpcEvent::DialogueShown {
                window: WindowId(1),
                speaker: None,
                text: Text::ActionFor {
                    action: 38,
                    what: lit(&long),
                },
                options: vec![DialogueOption {
                    option: 1,
                    text: Text::ActionFor {
                        action: 38,
                        what: lit("a\u{7}b"),
                    },
                }],
            }),
        ],
    ];
    let mut s = Session::new(Scripted(script));
    let t = Instant::now();
    s.send(t, Command::Connect).unwrap();
    let _ = s.poll(t);
    s.send(
        t,
        Command::EnterWorld {
            character: "P".into(),
        },
    )
    .unwrap();
    let ev = s.poll(t);
    let Some(Event::Npc(NpcEvent::DialogueShown { text, options, .. })) =
        ev.iter().find(|e| matches!(e, Event::Npc(_)))
    else {
        panic!("{ev:#?}");
    };
    assert!(matches!(
        text,
        Text::ActionFor { action: 38, what: Ref::Literal(a) } if a.len() == MAX_LITERAL_REF_LEN
    ));
    assert_eq!(
        options[0].text,
        Text::ActionFor {
            action: 38,
            what: lit("a b")
        }
    );
}
