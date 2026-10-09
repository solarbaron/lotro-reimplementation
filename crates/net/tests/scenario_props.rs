// SPDX-License-Identifier: GPL-3.0-or-later
//! Property tests and determinism of the scenario feed (`docs/specs/scenario-feed.md`
//! §10.1, §11): random scenario text never panics the parser or a bounded playback, the
//! output per poll stays within the per-tick caps, and the same input gives the same log.
#![cfg(feature = "scenario")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use proptest::prelude::*;
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{
    EmptyResolver, HeadlessDriver, LogContext, ResolvedScenario, Scenario, format_event,
};
use tessera_net::{Command, Event, Session};

/// Most events one poll can deliver: 8 ticks of 4,096 events, plus the sandbox's own
/// answers (world entry, a disconnect).
const MAX_EVENTS_PER_POLL: usize = 8 * 4096 + 16;

/// Plays `resolved` for at most `frames` frames of one tick each over a session, as the
/// headless driver does but bounded; checks the per-poll output.
fn bounded_playback(resolved: Arc<ResolvedScenario>, frames: u64) -> Vec<String> {
    let mut seen = Vec::new();
    let mut ctx = LogContext::new(&resolved);
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let Ok(backend) = SandboxBackend::new(config) else {
        return seen;
    };
    let mut session = Session::new(backend);
    let tick = Duration::from_millis(resolved.tick_ms());
    let mut now = Instant::now();
    let mut events = Vec::new();
    let _ = session.send(now, Command::Connect);
    session.poll_into(now, &mut events);
    let name = resolved
        .enter_character()
        .map(str::to_owned)
        .or_else(|| {
            events.iter().find_map(|e| match e {
                Event::Connected { characters } => characters.first().map(|c| c.name.clone()),
                _ => None,
            })
        })
        .unwrap_or_else(|| "Wanderer".to_owned());
    let _ = session.send(now, Command::EnterWorld { character: name });
    for _ in 0..frames {
        events.clear();
        session.poll_into(now, &mut events);
        assert!(
            events.len() <= MAX_EVENTS_PER_POLL,
            "{} events",
            events.len()
        );
        for e in &events {
            ctx.observe(e);
            if !matches!(e, Event::StaticSpawned { .. } | Event::StaticsCleared) {
                seen.push(format_event(e, &ctx));
            }
        }
        // Poke the feed with a few commands so triggers and generic answers run.
        let _ = session.send(now, Command::Combat(tessera_net::CombatCommand::CancelCast));
        let _ = session.send(
            now,
            Command::Chat {
                channel: tessera_net::ChatChannel::Say,
                text: "hi".into(),
            },
        );
        now += tick;
    }
    seen
}

fn line() -> impl Strategy<Value = String> {
    let n = 0u32..5000;
    prop_oneof![
        Just("player \"Testa\"".to_owned()),
        Just("character \"Bruni\" level=3".to_owned()),
        Just("tick 10ms".to_owned()),
        Just("end-at 2s".to_owned()),
        Just("defaults off".to_owned()),
        Just("actor $a kind=npc name=\"A\" pos=1,2,0".to_owned()),
        Just("actor $b kind=creature pos=place($s[0]) body=$s[1] level=4".to_owned()),
        Just("select $s = skill harmful=yes n=3 seed=9".to_owned()),
        Just("select $q = quest level=1..50 all".to_owned()),
        Just("item $i \"Bread\" qty=3".to_owned()),
        Just("page $p speaker=$a \"Hello\"".to_owned()),
        Just("option $p 1 \"Bye\" goto=close".to_owned()),
        Just("on use $a".to_owned()),
        Just("on cancel-cast times=3".to_owned()),
        Just("on chat".to_owned()),
        Just("input".to_owned()),
        Just("end".to_owned()),
        Just("+0s show $p".to_owned()),
        Just("+0.1s cast player \"Kindle\" 200ms".to_owned()),
        Just("+0s repeat 5 every 20ms".to_owned()),
        Just("+0s give $i".to_owned()),
        Just("+0s give $s to=vault".to_owned()),
        Just("+0s vendor items=\"A\",\"B\" price=1,2".to_owned()),
        Just("+0s loot items=$s qty=2".to_owned()),
        Just("+0s move $a 0,5,0 over=100ms".to_owned()),
        Just("+0s effect player \"Ward\" for=50ms".to_owned()),
        Just("+0s teleport 5,5,0".to_owned()),
        Just("+0s spawn $a".to_owned()),
        Just("at pre create-character \"Cora\"".to_owned()),
        Just("at 0.2s use $a".to_owned()),
        Just("at 0.1s chat \"x\" channel=tell:\"Bram\"".to_owned()),
        n.clone().prop_map(|t| format!("at {t}ms system \"t{t}\"")),
        n.clone()
            .prop_map(|t| format!("+{t}ms say $a \"line\" channel=emote")),
        n.prop_map(|t| format!("at {t}ms repeat 3 every 7ms")),
        "[ -~]{0,48}",
        "[a-z$@\" =.,0-9]{0,32}",
    ]
}

const VERBS: &[&str] = &[
    "spawn $a",
    "despawn $b",
    "spawn $b",
    "move $a 1,1,0 over=120ms",
    "move $b 0,1,0",
    "face $a 90",
    "face $b player",
    "level player 3",
    "bag 4",
    "give $i",
    "give $j slot=2",
    "give $s",
    "remove $i",
    "quantity $i 2",
    "equip $i slot=1",
    "unequip $i",
    "condition $j 1/2",
    "wallet coin 5",
    "target player $a",
    "vital $b health 5/10",
    "combat $a on",
    "quickslot 1 \"Kindle\"",
    "quickslot 2 $i",
    "cast player \"Kindle\" 300ms target=$b",
    "delay player 100ms",
    "interrupt $a",
    "cooldown \"Kindle\" 1s",
    "cooldown \"Kindle\" clear",
    "effect $b \"Ward\" for=200ms",
    "effect-end $b \"Ward\"",
    "damage player $b 5 left=3",
    "heal $a player 1",
    "defeat $b",
    "revive $b",
    "show $p",
    "offer \"Q\" from=$a",
    "quest \"Q\" active",
    "objective \"Q\" 1 1/2",
    "track \"Q\" on",
    "vendor items=\"A\",\"B\" npc=$a",
    "loot items=\"C\",\"D\" from=$b",
    "loot-taken 1",
    "close vendor",
    "say $a \"hi\"",
    "system \"x\"",
    "channel join \"L\"",
    "fellowship player \"Bram\"",
    "invite \"Bram\"",
    "trade $a",
    "trade-offer own items=$i",
    "trade-ready own on",
    "friends \"Bram\":on",
    "ignore \"X\"",
    "perform $a \"T\"",
    "perform-stop $a",
    "marker $m 1,1,0 \"M\"",
    "travel-list dest=\"N\"",
    "travel \"N\" 1s",
    "teleport 3,3,0",
    "mount player \"P\"",
    "instances \"C\"",
    "instance \"C\"",
    "xp 1/2",
    "learn $s",
    "forget \"S\"",
    "trainer skills=\"S\"",
    "trait-points 1/2",
    "trait 1 \"T\"",
    "profession \"P\" tier=1 proficiency=1/2 mastery=1/2",
    "recipe \"R\"",
    "craft-result \"R\" success",
    "reputation \"F\" 5",
    "title \"T\"",
];

const PATTERNS: &[&str] = &[
    "use $a",
    "use-skill",
    "use-item $i",
    "target any",
    "cancel-cast",
    "choose $p",
    "accept",
    "decline",
    "buy",
    "sell",
    "take-loot",
    "equip",
    "move-item",
    "split",
    "destroy",
    "chat",
    "close vendor",
    "open-trade",
    "offer-trade",
    "trade-ready",
    "dismount",
    "slot-trait",
    "craft",
];

const INPUTS: &[&str] = &[
    "target $a",
    "use-skill \"Kindle\"",
    "use-skill quickslot=1",
    "use-item $i",
    "use $a",
    "cancel-cast",
    "choose 1",
    "choose $p 2",
    "accept",
    "decline",
    "buy 1",
    "sell $i",
    "take-loot all",
    "equip $i slot=3",
    "unequip $i",
    "move-item $i to=backpack slot=4",
    "destroy $j",
    "split $i 1",
    "quickslot 3 \"K\"",
    "chat \"x\"",
    "chat \"x\" channel=tell:\"B\"",
    "join-channel \"L\"",
    "open-trade $a",
    "offer-trade coin=1",
    "trade-ready on",
    "choose-travel 1",
    "join-instance 1",
    "train 1",
    "slot-trait 1 none",
    "craft \"R\"",
    "close vendor",
    "dismount",
];

fn structured() -> impl Strategy<Value = String> {
    let step = (0u32..3000, 0..VERBS.len()).prop_map(|(t, v)| format!("at {t}ms {}", VERBS[v]));
    let body = prop::collection::vec((0u32..500, 0..VERBS.len()), 0..4);
    let trigger = (0..PATTERNS.len(), body, prop::option::of(1u32..4)).prop_map(|(p, b, times)| {
        let mut s = format!("on {}", PATTERNS[p]);
        if let Some(n) = times {
            s.push_str(&format!(" times={n}"));
        }
        s.push('\n');
        for (d, v) in b {
            s.push_str(&format!("+{d}ms {}\n", VERBS[v]));
        }
        s.push_str("end");
        s
    });
    let input = prop::collection::vec((0u32..3000, 0..INPUTS.len()), 0..12);
    (
        10u32..200,
        prop::collection::vec(step, 0..30),
        prop::collection::vec(trigger, 0..4),
        input,
    )
        .prop_map(|(tick, steps, triggers, input)| {
            let mut s = format!(
                "tessera-scenario 1\ntick {tick}ms\nplayer \"Testa\"\ncharacter \"Bruni\"\n\
                 select $s = skill n=3\nactor $a kind=npc name=\"A\" pos=1,2,0\n\
                 actor $b kind=creature pos=0,3,0\nitem $i \"Bread\" qty=3\nitem $j \"Ale\"\n\
                 page $p speaker=$a \"Hello\"\noption $p 1 \"Next\" goto=$p\n\
                 option $p 2 \"Bye\" goto=close\n"
            );
            for l in steps {
                s.push_str(&l);
                s.push('\n');
            }
            for t in triggers {
                s.push_str(&t);
                s.push('\n');
            }
            s.push_str("input\n");
            for (t, c) in input {
                s.push_str(&format!("at {t}ms {}\n", INPUTS[c]));
            }
            s.push_str("end\n");
            s
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// Random scenario text: the parser returns `Ok` or a typed error, and a valid scenario
    /// plays a bounded number of ticks without a panic and within the per-poll cap.
    #[test]
    fn random_scenarios_never_panic(lines in prop::collection::vec(line(), 0..40)) {
        let text = format!("tessera-scenario 1\n{}\n", lines.join("\n"));
        if let Ok(sc) = Scenario::parse(text.as_bytes())
            && let Ok(resolved) = sc.resolve(&mut EmptyResolver)
        {
            let _ = bounded_playback(Arc::new(resolved), 60);
        }
    }

    /// Structured random scenarios that mostly parse: bounded playback without a panic,
    /// within the per-poll cap, and the same events on a second run.
    #[test]
    fn structured_scenarios_play_deterministically(text in structured()) {
        let sc = Scenario::parse(text.as_bytes());
        if let Ok(sc) = sc
            && let Ok(resolved) = sc.resolve(&mut EmptyResolver)
        {
            let resolved = Arc::new(resolved);
            let a = bounded_playback(Arc::clone(&resolved), 80);
            let b = bounded_playback(resolved, 80);
            prop_assert_eq!(a, b);
        }
    }

    /// Arbitrary bytes never panic the parser; errors have positions from 1.
    #[test]
    fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
        let mut input = b"tessera-scenario 1\n".to_vec();
        input.extend_from_slice(&bytes);
        if let Err(e) = Scenario::parse(&input) {
            prop_assert!(e.line >= 1 && e.column >= 1);
        }
        if let Err(e) = Scenario::parse(&bytes) {
            prop_assert!(e.line >= 1 && e.column >= 1);
        }
    }
}

const KITCHEN_SINK: &str = r#"tessera-scenario 1
title "Sink \u{e9}"
tick 50ms
player "Testa"
character "Bruni"
actor $npc kind=npc name="Npc" pos=2,0,0
actor $foe kind=creature name="Foe" pos=0,4,0 scale=2 species=1 sex=2
item $sword "Sword"
item $gem "Gem"
page $p speaker=player "Hm."
at 0s spawn $npc
+0s spawn $foe
+0s level player 5
+0s bag 8 container=vault
+0s give $sword
+0s give $gem to=shared slot=3
+0s quantity $gem 9
+0s condition $sword 40/100
+0s wallet "Token" 12
+0s equip $sword slot=2
+0s unequip $sword slot=5
+0s remove $gem
+0s target player $foe
+0s vital player power 50/80
+0s vital $foe resource="Spark" 3/5
+0s combat $foe on
+0s quickslot 2 $sword
+0s heal none player 12.5 vital=power with="Mend" type=3 left=62.5
+0s defeat $foe by=player
+0s revive $foe
+0s show $p
+0s objective "Q" 1 2/3 done
+0s track "Q" on
+0s quest "Q" completed
+0s say "Herald" "Hear ye" channel=named:"Lore"
+0s say player "hi" channel=fellowship
+0s channel join "Lore"
+0s fellowship player $npc "Bram" leader=$npc
+0s invite "Bram"
+0s trade $npc
+0s trade-offer own items=$sword,"Ale" qty=2 coin=7
+0s trade-ready peer on
+0s friends "Bram":on "Cora":off
+0s ignore "Rude"
+0s perform $npc "Tune" delay=1s
+0s perform-stop $npc
+0s marker $m 1,1,0 "Here" quest="Q"
+0s marker-remove $m
+0s travel "North" 2s
+0s mount player "Pony"
+0s instances "Cave","Keep"
+0s instance none
+0s xp 10/100
+0s learn "Sing"
+0s forget "Sing"
+0s trainer skills="Sing","Dance" coin=5 level=1,2
+0s trait-points 1/2
+0s trait 3 "Brave" rank=2
+0s profession "Cook" tier=1 proficiency=5/10 mastery=0/10
+0s recipe "Stew"
+0s craft-result "Stew" critical item=$sword
+0s reputation "Guild" -50 tier=2
+0s title "Hero"
+0s face $npc 45
+0s close trade
"#;

fn log_of(text: &str) -> String {
    let resolved = Arc::new(
        Scenario::parse(text.as_bytes())
            .expect("parse")
            .resolve(&mut EmptyResolver)
            .expect("resolve"),
    );
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let mut d = HeadlessDriver::new(
        Session::new(SandboxBackend::new(config).expect("sandbox")),
        resolved,
        Some(Vec::new()),
    );
    let outcome = d.run();
    assert_eq!(outcome.exit_code(), 0);
    String::from_utf8(d.into_log().expect("log")).expect("utf-8")
}

/// Every step verb, logged in its canonical form; the same file gives the same log twice.
#[test]
fn kitchen_sink_is_deterministic_and_complete() {
    let a = log_of(KITCHEN_SINK);
    let b = log_of(KITCHEN_SINK);
    assert_eq!(a, b);
    for line in [
        "title \"Sink \\u{e9}\"",
        "0.000 EntityCreated entity=$foe kind=creature name=\"Foe\" class=none at=0:0.000,4.000,0.000 facing=180.00 body=character:1:2 scale=2 equipment=[] idle=none",
        "0.000 LevelChanged entity=player level=5",
        "0.000 ContainerSize container=vault slots=8",
        "0.000 ItemAdded item=$gem what=\"Gem\" at=shared:3 quantity=1",
        "0.000 ItemQuantity item=$gem quantity=9",
        "0.000 ItemCondition item=$sword current=40 max=100",
        "0.000 WalletChanged currency=item:\"Token\" amount=12",
        "0.000 ItemMoved item=$sword at=equipment:2",
        "0.000 ItemMoved item=$sword at=backpack:5",
        "0.000 ItemRemoved item=$gem",
        "0.000 TargetChanged entity=player target=$foe",
        "0.000 VitalChanged entity=$foe vital=resource:\"Spark\" current=3 max=5",
        "0.000 CombatState entity=$foe in_combat=yes",
        "0.000 QuickslotChanged slot=2 content=item:$sword",
        "0.000 VitalDelta source=none target=player vital=power kind=heal amount=12.5 result=normal what=\"Mend\" damage_type=3",
        "0.000 VitalChanged entity=player vital=power current=62.5 max=80",
        "0.000 Defeated entity=$foe by=player",
        "0.000 Revived entity=$foe",
        "0.000 DialogueShown window=1 speaker=player text=\"Hm.\" options=[]",
        "0.000 QuestObjective quest=\"Q\" objective=1 count=2 total=3 done=yes",
        "0.000 QuestTracked quest=\"Q\" tracked=yes",
        "0.000 QuestState quest=\"Q\" state=completed",
        "0.000 ChatMessage channel=named:\"Lore\" sender=\"Herald\" text=\"Hear ye\"",
        "0.000 ChatMessage channel=fellowship sender=\"Testa\" text=\"hi\"",
        "0.000 ChannelJoined channel=\"Lore\"",
        "0.000 FellowshipChanged members=[{name=\"Testa\" entity=player leader=no},{name=\"Npc\" entity=$npc leader=yes},{name=\"Bram\" entity=none leader=no}]",
        "0.000 FellowshipInvite from=\"Bram\"",
        "0.000 TradeShown window=2 peer=$npc",
        "0.000 TradeOffer window=2 side=own entries=[{what=\"Sword\" item=$sword quantity=2},{what=\"Ale\" item=none quantity=2}] coin=7",
        "0.000 TradeReady window=2 side=peer ready=yes",
        "0.000 FriendList entries=[{name=\"Bram\" online=yes},{name=\"Cora\" online=no}]",
        "0.000 IgnoreList names=[\"Rude\"]",
        "0.000 PerformanceStarted entity=$npc tune=\"Tune\" delay_ms=1000",
        "0.000 PerformanceStopped entity=$npc",
        "0.000 MarkerAdded marker=1 location=0:1.000,1.000,0.000 label=\"Here\" icon=none quest=\"Q\"",
        "0.000 MarkerRemoved marker=1",
        "0.000 TravelStarted destination=\"North\" duration_ms=2000",
        "0.000 MountChanged entity=player mount=\"Pony\"",
        "0.000 InstanceListShown window=3 entries=[{entry=1 instance=\"Cave\"},{entry=2 instance=\"Keep\"}]",
        "0.000 InstanceChanged instance=none",
        "0.000 ExperienceChanged current=10 needed=100",
        "0.000 SkillLearned what=\"Sing\"",
        "0.000 SkillForgotten what=\"Sing\"",
        "0.000 TrainerShown window=4 trainer=none entries=[{entry=1 what=\"Sing\" coin=5 level=1},{entry=2 what=\"Dance\" coin=5 level=2}]",
        "0.000 TraitPoints available=1 total=2",
        "0.000 TraitSlotted slot=3 what=\"Brave\" rank=2",
        "0.000 ProfessionChanged profession=\"Cook\" tier=1 proficiency=5 proficiency_max=10 mastery=0 mastery_max=10",
        "0.000 RecipeLearned what=\"Stew\"",
        "0.000 CraftResult recipe=\"Stew\" outcome=critical item=$sword",
        "0.000 ReputationChanged faction=\"Guild\" points=-50 tier=2",
        "0.000 TitleGranted what=\"Hero\"",
        "0.000 EntityMoved entity=$npc at=2.000,0.000,0.000 facing=45.00 velocity=0.000,0.000,0.000",
        "0.000 WindowClosed window=2",
        "0.000 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0",
    ] {
        assert!(a.lines().any(|l| l == line), "missing `{line}` in:\n{a}");
    }
}
