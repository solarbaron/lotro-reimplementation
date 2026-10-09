// SPDX-License-Identifier: GPL-3.0-or-later
//! Conformance vectors of `docs/specs/scenario-feed.md` §14: each scenario runs headless
//! over the plain sandbox with no install and must give the expected canonical log byte
//! for byte.
#![cfg(feature = "scenario")]

use std::sync::Arc;

use tessera_net::DataId;
use tessera_net::Session;
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{
    Candidate, CandidateOutcome, CandidateStatus, EmptyResolver, HeadlessDriver, LabelContext,
    ResolvedScenario, Scenario, SelectorQuery, SelectorResolver,
};

fn run_resolved(resolved: ResolvedScenario) -> String {
    let resolved = Arc::new(resolved);
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let session = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let mut driver = HeadlessDriver::new(session, resolved, Some(Vec::new()));
    let _ = driver.run();
    String::from_utf8(driver.into_log().expect("log")).expect("utf-8")
}

/// Runs `text` headless with no install and returns the canonical log.
fn run(text: &str) -> String {
    let scenario = Scenario::parse(text.as_bytes()).expect("parse");
    run_resolved(scenario.resolve(&mut EmptyResolver).expect("resolve"))
}

fn check(text: &str, expected: &str) {
    let log = run(text);
    if log != expected {
        let diff: Vec<String> = log
            .lines()
            .zip(expected.lines().chain(std::iter::repeat("<missing>")))
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .take(5)
            .map(|(i, (a, b))| format!("line {}:\n  got:  {a}\n  want: {b}", i + 1))
            .collect();
        panic!("log differs:\n{}\n--- full log ---\n{log}", diff.join("\n"));
    }
}

const VECTOR_A: &str = r#"tessera-scenario 1
title "Vector A"
tick 50ms
player "Testa"
actor $guide kind=npc name="Guide" pos=2,0,0 facing=270
at 0s spawn $guide
+0s vital player health 1000/1000
+0.5s say $guide "Welcome, traveller."
at 1s despawn $guide
"#;

const LOG_A: &str = r#"tessera-scenario-log 1
install none
tick 50
title "Vector A"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$guide kind=non-player name="Guide" at=2.000,0.000,0.000 facing=270.00
0.000 VitalChanged entity=player vital=health current=1000 max=1000
0.500 ChatMessage channel=say sender="Guide" text="Welcome, traveller."
1.000 EntityDespawned entity=$guide
1.000 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
"#;

#[test]
fn vector_a_actor_vital_chat() {
    check(VECTOR_A, LOG_A);
}

const VECTOR_B: &str = r#"tessera-scenario 1
title "Vector B"
tick 100ms
player "Testa"
select $skills = skill harmful=yes n=2 seed=7
actor $dummy kind=creature name="Practice post" pos=0,3,0 facing=180
at 0s spawn $dummy
+0s vital $dummy health 500/500
+0s quickslot 1 "Kindle"
+0s quickslot 2 $skills[0]
on use-skill quickslot=1 target=$dummy
+0s cast player "Kindle" 1s target=$dummy
+1s damage player $dummy 120 result=critical left=380
+0s effect $dummy "Smoulder" for=2s
end
at 2s repeat 3 every 500ms
+0s say $dummy "creaks" channel=emote
end
input
at 0.2s target $dummy
at 0.3s use-skill quickslot=1
end
"#;

const LOG_B: &str = r#"tessera-scenario-log 1
install none
tick 100
title "Vector B"
resolve $skills line=5 family=skill requested=2 candidates=0 got=0 note=no-install
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$dummy kind=creature name="Practice post" at=0.000,3.000,0.000 facing=180.00
0.000 VitalChanged entity=$dummy vital=health current=500 max=500
0.000 QuickslotChanged slot=1 content=skill:"Kindle"
0.200 > SelectTarget target=$dummy
0.200 TargetChanged entity=player target=$dummy
0.300 > UseSkill what="Kindle" target=$dummy
0.300 CastStarted entity=player cast=cast what="Kindle" target=$dummy duration_ms=1000
1.300 CastEnded entity=player outcome=completed
1.300 VitalDelta source=player target=$dummy vital=health kind=damage amount=120 result=critical what=none damage_type=none
1.300 VitalChanged entity=$dummy vital=health current=380 max=500
1.300 EffectApplied entity=$dummy effect=1 what="Smoulder" duration_ms=2000 stacks=1 source=none
2.000 ChatMessage channel=emote sender="Practice post" text="creaks"
2.500 ChatMessage channel=emote sender="Practice post" text="creaks"
3.000 ChatMessage channel=emote sender="Practice post" text="creaks"
3.300 EffectRemoved entity=$dummy effect=1
3.300 end reason=complete skipped=1 input-skipped=0 dropped=0 spilled=0
"#;

#[test]
fn vector_b_trigger_repeat_input_empty_selector() {
    check(VECTOR_B, LOG_B);
}

const VECTOR_D: &str = r#"tessera-scenario 1
title "Vector D"
tick 100ms
player "Testa"
actor $elder kind=npc name="Elder" pos=-2,0,0 facing=90
page $thanks speaker=$elder "Thank you."
page $hello speaker=$elder "Good day. Will you help?"
option $hello 1 "Gladly." goto=$thanks
option $hello 2 "Not now." goto=close
at 0s spawn $elder
on use $elder
+0s show $hello
end
on choose $hello 1
+0.5s offer "Fetch water" from=$elder
end
input
at 0.1s use $elder
at 0.4s choose $hello 1
at 1s accept
end
"#;

const LOG_D: &str = r#"tessera-scenario-log 1
install none
tick 100
title "Vector D"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$elder kind=non-player name="Elder" at=-2.000,0.000,0.000 facing=90.00
0.100 > UseEntity entity=$elder
0.100 DialogueShown window=1 speaker=$elder text="Good day. Will you help?" options=[{option=1 text="Gladly."},{option=2 text="Not now."}]
0.400 > ChooseOption window=1 option=1
0.400 DialogueShown window=1 speaker=$elder text="Thank you." options=[]
0.900 QuestOffered window=2 quest="Fetch water" giver=$elder
1.000 > AnswerQuest window=2 accept=yes
1.000 QuestState quest="Fetch water" state=active
1.000 WindowClosed window=2
1.000 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
"#;

#[test]
fn vector_d_dialogue_quest_offer_generic_answers() {
    check(VECTOR_D, LOG_D);
}

const VECTOR_E: &str = r#"tessera-scenario 1
title "Vector E"
tick 100ms
end-at 1s
player "Testa"
actor $cart kind=object name="Cart" pos=0,0,0 facing=0
item $bread "Bread" qty=5
item $ale "Ale"
at 0s spawn $cart
+0s bag 4
+0s give $bread
+0s give $ale slot=2
+0s wallet coin 1234
+0.1s move $cart 0,3,0 over=300ms
+0s cooldown "Rest" 2s
at 0.7s vendor items="Bread","Ale" npc=$cart price=10,25
input
at 0.5s split $bread 2
at 0.6s move-item $ale to=backpack slot=0
end
"#;

const LOG_E: &str = r#"tessera-scenario-log 1
install none
tick 100
title "Vector E"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$cart kind=object name="Cart" at=0.000,0.000,0.000 facing=0.00
0.000 ContainerSize container=backpack slots=4
0.000 ItemAdded item=$bread what="Bread" at=backpack:0 quantity=5
0.000 ItemAdded item=$ale what="Ale" at=backpack:2 quantity=1
0.000 WalletChanged currency=coin amount=1234
0.100 CooldownStarted what="Rest" duration_ms=2000
0.200 EntityMoved entity=$cart at=0.000,1.000,0.000 facing=0.00 velocity=0.000,10.000,0.000
0.300 EntityMoved entity=$cart at=0.000,2.000,0.000 facing=0.00 velocity=0.000,10.000,0.000
0.400 EntityMoved entity=$cart at=0.000,3.000,0.000 facing=0.00 velocity=0.000,0.000,0.000
0.500 > SplitStack item=$bread quantity=2
0.500 ItemQuantity item=$bread quantity=3
0.500 ItemAdded item=item:3 what="Bread" at=backpack:1 quantity=2
0.600 > MoveItem item=$ale to=backpack:0
0.600 ItemMoved item=$ale at=backpack:0
0.600 ItemMoved item=$bread at=backpack:2
0.700 VendorShown window=1 vendor=$cart entries=[{entry=1 what="Bread" price=coin:10 stock=none},{entry=2 what="Ale" price=coin:25 stock=none}]
1.000 end reason=complete skipped=0 input-skipped=0 dropped=1 spilled=0
"#;

#[test]
fn vector_e_inventory_move_over_entries_end_at() {
    check(VECTOR_E, LOG_E);
}

const VECTOR_F: &str = r#"tessera-scenario 1
title "Vector F"
tick 100ms
player "Testa" level=5
character "Bruni" class=@Example_Class
actor $post kind=object name="Post" pos=0,1,0 facing=359.999
item $apple "Apple"
item $pear "Pear"
at 0s spawn $post
+0s give $apple slot=1
+0s give $pear slot=1
+0s give $apple
on create-character name="Cora"
+0s accept-character "Cora" level=1
end
on create-character name="Dora"
+0s refuse-character "Dora" "That name is taken."
end
on chat
+0.05s cast player "Ember" 30ms
end
input
at pre create-character "Cora"
at pre create-character "Dora"
at 0s chat "hi" channel=tell:"Bruni"
end
"#;

const LOG_F: &str = r#"tessera-scenario-log 1
install none
tick 100
title "Vector F"
pre > Connect
pre Connected characters=["Testa","Bruni"]
pre CharacterInfo name="Testa" body=none class=none level=5
pre > CreateCharacter name="Cora" appearance=none class=none
pre CharacterInfo name="Cora" body=none class=none level=1
pre > CreateCharacter name="Dora" appearance=none class=none
pre CharacterRefused name="Dora" reason="That name is taken."
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$post kind=object name="Post" at=0.000,1.000,0.000 facing=0.00
0.000 ItemAdded item=$apple what="Apple" at=backpack:1 quantity=1
0.100 > Chat channel=tell:"Bruni" text="hi"
0.100 ChatMessage channel=system sender=none text="Bruni is not in the sandbox."
0.200 CastStarted entity=player cast=cast what="Ember" target=none duration_ms=30
0.300 CastEnded entity=player outcome=completed
0.300 end reason=complete skipped=2 input-skipped=0 dropped=0 spilled=0
"#;

#[test]
fn vector_f_pre_world_character_triggers_chat_answer() {
    check(VECTOR_F, LOG_F);
}

#[test]
fn every_vector_is_deterministic() {
    for text in [VECTOR_A, VECTOR_B, VECTOR_D, VECTOR_E, VECTOR_F] {
        assert_eq!(run(text), run(text));
    }
}

/// A resolver that offers a fixed candidate list for every query, in a scrambled order.
#[derive(Debug)]
struct Fixed(Vec<Candidate>);

impl SelectorResolver for Fixed {
    fn has_install(&self) -> bool {
        true
    }

    fn candidates(
        &mut self,
        _query: &SelectorQuery<'_>,
        _limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        out.extend(self.0.iter().rev().copied());
        CandidateOutcome::new(CandidateStatus::Ready)
    }

    fn enum_value(&mut self, _context: LabelContext, _label: &str) -> Option<u32> {
        None
    }

    fn display_name(&mut self, _did: DataId, _row: Option<u32>) -> Option<String> {
        Some("Named".to_owned())
    }
}

fn ten() -> Vec<Candidate> {
    let mut v: Vec<Candidate> = (1..=10)
        .map(|i| Candidate {
            did: DataId(0x7000_0000 + i),
            row: 0,
            location: None,
            anchor: None,
        })
        .collect();
    v.swap(0, 7);
    v.swap(3, 9);
    v
}

/// Vector C through the public resolution path (§14.3).
#[test]
fn vector_c_sampling_through_resolve() {
    let cases: [(&str, &[u32], Option<&str>); 7] = [
        ("n=3 seed=7", &[1, 5, 8], None),
        ("n=3 seed=0", &[2, 6, 10], None),
        ("n=5 seed=1", &[2, 4, 6, 8, 9], None),
        ("n=1 seed=7", &[8], None),
        ("n=10 seed=7", &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], None),
        (
            "n=12 seed=7",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            Some("short"),
        ),
        ("n=3", &[1, 2, 3], None),
    ];
    for (count, want, note) in cases {
        let text = format!("tessera-scenario 1\nselect $s = skill {count}\n");
        let sc = Scenario::parse(text.as_bytes()).expect("parse");
        let r = sc.resolve(&mut Fixed(ten())).expect("resolve");
        let got: Vec<u32> = r
            .binding("$s")
            .expect("binding")
            .iter()
            .map(|c| c.did.0 - 0x7000_0000)
            .collect();
        assert_eq!(got, want, "{count}");
        let line = r.notes()[0].to_string();
        match note {
            Some(n) => assert!(line.ends_with(&format!("note={n}")), "{line}"),
            None => assert!(!line.contains("note="), "{line}"),
        }
        assert!(r.has_install());
    }
}

/// With an install, elements resolve and drive steps; the log carries DIDs.
#[test]
fn resolved_elements_drive_steps() {
    let text = r#"tessera-scenario 1
tick 100ms
player "Testa"
select $skills = skill n=3
actor $npc kind=npc name=name($skills[1]) pos=1,0,0 body=$skills[2]
at 0s spawn $npc
+0s learn $skills
+0s quickslot 1 $skills[0]
"#;
    let sc = Scenario::parse(text.as_bytes()).expect("parse");
    let r = sc.resolve(&mut Fixed(ten())).expect("resolve");
    let log = run_resolved(r);
    assert!(log.contains("install present"), "{log}");
    assert!(
        log.contains(
            "0.000 EntityCreated entity=$npc kind=non-player name=\"Named\" class=0x70000003 at=0:1.000,0.000,0.000"
        ),
        "{log}"
    );
    assert!(log.contains("0.000 SkillLearned what=0x70000001"), "{log}");
    assert!(log.contains("0.000 SkillLearned what=0x70000003"), "{log}");
    assert!(
        log.contains("0.000 QuickslotChanged slot=1 content=skill:0x70000001"),
        "{log}"
    );
    assert!(log.contains("end reason=complete skipped=0"), "{log}");
}
