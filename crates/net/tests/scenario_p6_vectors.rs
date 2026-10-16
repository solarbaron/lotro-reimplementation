// SPDX-License-Identifier: GPL-3.0-or-later
//! The "P6 progression" revision of the scenario feed (`docs/specs/scenario-feed.md`
//! §18.P6; levels, training, traits and crafting): Vectors P6-V1 and P6-V2 and resolver case
//! P6-V3 (§14.P6a–§14.P6c) byte for byte, and the run-time rules of the new steps, options,
//! patterns and input forms beyond the vectors.
#![cfg(feature = "scenario")]

use std::sync::Arc;

use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{
    Candidate, CandidateOutcome, CandidateStatus, EmptyResolver, Family, FilterValue,
    HeadlessDriver, LabelContext, ResolvedScenario, Scenario, SelectorQuery, SelectorResolver,
};
use tessera_net::{DataId, Session};

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

fn run_with(text: &str, resolver: &mut dyn SelectorResolver) -> String {
    let scenario = Scenario::parse(text.as_bytes()).expect("parse");
    run_resolved(scenario.resolve(resolver).expect("resolve"))
}

fn run(text: &str) -> String {
    run_with(text, &mut EmptyResolver)
}

fn same(log: &str, expected: &str) {
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

const VECTOR_P6_V1: &str = r##"tessera-scenario 1
title "Vector P6-V1"
tick 100ms
player "Testa"
actor $coach kind=npc name="Coach" pos=0,2,0
at 0s spawn $coach
+0s level player 9 quiet
+0s untrained skills="Kick","Sweep" coin=50,80 level=10,12 rank=0,1
on use $coach
+0s trainer skills="Kick","Sweep","Lunge" npc=$coach coin=50,80 level=10,12,14 rank=0,1 trained=3 unmet=2
end
on train entry=1
+0s learn "Kick"
+0s untrained skills="Sweep" coin=80 level=12 rank=1
+0s level player 10
end
on trait-rank tree="Valour"
+0s trait-rank "Valour" "Steady" 2
+0s trait-points 3/8 tree="Valour"
end
on trait-branch
+0s trait-tree "Valour" branch=4
end
input
at 0.1s use $coach
at 0.2s train 1
at 0.3s trait-rank "Valour" "Steady" 2
at 0.4s trait-branch "Valour" 4
end
"##;

const LOG_P6_V1: &str = r##"tessera-scenario-log 1
install none
tick 100
title "Vector P6-V1"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$coach kind=non-player name="Coach" at=0.000,2.000,0.000 facing=180.00
0.000 LevelChanged entity=player level=9 quiet=yes
0.000 UntrainedSkills entries=[{entry=1 what="Kick" coin=50 level=10 rank=0},{entry=2 what="Sweep" coin=80 level=12 rank=1}]
0.100 > UseEntity entity=$coach
0.100 TrainerShown window=1 trainer=$coach entries=[{entry=1 what="Kick" coin=50 level=10 rank=0},{entry=2 what="Sweep" coin=80 level=12 rank=1 flags=unmet},{entry=3 what="Lunge" coin=80 level=14 rank=1 flags=trained}]
0.200 > TrainSkill window=1 entry=1
0.200 SkillLearned what="Kick"
0.200 UntrainedSkills entries=[{entry=1 what="Sweep" coin=80 level=12 rank=1}]
0.200 LevelChanged entity=player level=10
0.300 > SetTraitRank tree="Valour" what="Steady" rank=2
0.300 TraitRank tree="Valour" what="Steady" rank=2
0.300 TraitPoints available=3 total=8 tree="Valour"
0.400 > ChooseTraitBranch tree="Valour" branch=4
0.400 TraitTreeChanged tree="Valour" branch=4
0.400 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
"##;

const VECTOR_P6_V2: &str = r##"tessera-scenario 1
title "Vector P6-V2"
tick 100ms
player "Testa"
actor $smith kind=npc name="Smith" pos=2,0,0 facing=270
item $cup "Cup"
item $mug "Mug"
page $hello speaker=$smith "Lessons?"
option $hello 1 action(38,"Pottery") goto=close
at 0s spawn $smith
+0s vocation "Potter" slots=3
+0s profession "Pottery" tier=2 proficiency=40/200 mastery=10/100 mastery-tier=1
+0s recipe "Cup" profession="Pottery" tier=2 category=7
+0s recipe "Mug" profession="Pottery" tier=1
at 3s trainer skills="Bowl" offers=recipes coin=25
on use $smith
+0s show $hello
end
on choose $hello 1
+0s craft-trainer profession "Pottery" npc=$smith
end
on train-craft
+0s close craft-trainer
+0s system "Already known."
end
on craft "Cup"
+0s cast player "Cup" 1s kind=craft
+1s give $cup
+0s craft-result "Cup" success item=$cup batch=1/2
+0s cast player "Cup" 1s kind=craft
+1s give $mug
+0s craft-result "Cup" critical item=$mug batch=2/2
+0s forget-recipe "Mug"
end
input
at 0.1s use $smith
at 0.2s choose $hello 1
at 0.3s train-craft replace="Pottery"
at 0.5s craft "Cup" qty=2 tier=2 output=1 optional=3 pack
end
"##;

const LOG_P6_V2: &str = r##"tessera-scenario-log 1
install none
tick 100
title "Vector P6-V2"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$smith kind=non-player name="Smith" at=2.000,0.000,0.000 facing=270.00
0.000 VocationChanged vocation="Potter" slots=3
0.000 ProfessionChanged profession="Pottery" tier=2 proficiency=40 proficiency_max=200 mastery=10 mastery_max=100 mastery_tier=1
0.000 RecipeLearned what="Cup" profession="Pottery" tier=2 category=7
0.000 RecipeLearned what="Mug" profession="Pottery" tier=1
0.100 > UseEntity entity=$smith
0.100 DialogueShown window=1 speaker=$smith text="Lessons?" options=[{option=1 text=action(38,"Pottery")}]
0.200 > ChooseOption window=1 option=1
0.200 WindowClosed window=1
0.200 CraftTrainerShown window=2 kind=profession what="Pottery" npc=$smith
0.300 > TrainCraft window=2 replace="Pottery"
0.300 WindowClosed window=2
0.300 ChatMessage channel=system sender=none text="Already known."
0.500 > Craft recipe="Cup" quantity=2 tier=2 output=1 optional=3 pack=yes
0.500 CastStarted entity=player cast=craft what="Cup" target=none duration_ms=1000
1.500 CastEnded entity=player outcome=completed
1.500 ItemAdded item=$cup what="Cup" at=backpack:0 quantity=1
1.500 CraftResult recipe="Cup" outcome=success item=$cup batch=1/2
1.500 CastStarted entity=player cast=craft what="Cup" target=none duration_ms=1000
2.500 CastEnded entity=player outcome=completed
2.500 ItemAdded item=$mug what="Mug" at=backpack:1 quantity=1
2.500 CraftResult recipe="Cup" outcome=critical item=$mug batch=2/2
2.500 RecipeForgotten what="Mug"
3.000 TrainerShown window=3 trainer=none entries=[{entry=1 what="Bowl" coin=25 level=0}] offers=recipes
3.000 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
"##;

const VECTOR_P6_V3: &str = r##"tessera-scenario 1
title "Vector P6-V3"
tick 100ms
player "Testa"
select $nodes = creature kind=container gathering=yes n=1
select $gather = induction of=$nodes all
select $forge = effect facility=yes n=1
actor $node kind=object name="Node" body=$nodes[0] pos=0,3,0
at 0s spawn $node
+0s effect player $forge[0]
on use $node times=1
+0s cast player $gather[0] 7s kind=induction
+7s loot items="Ore" from=$node qty=3
+0s despawn $node
end
input
at 0.2s use $node
at 7.4s take-loot all
end
"##;

const LOG_P6_V3: &str = r##"tessera-scenario-log 1
install present
tick 100
title "Vector P6-V3"
resolve $nodes line=5 family=creature requested=1 candidates=2 got=1
resolve $gather line=6 family=induction requested=all candidates=1 got=1
resolve $forge line=7 family=effect requested=1 candidates=1 got=1
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntityCreated entity=$node kind=object name="Node" class=0x70001000 at=0:0.000,3.000,0.000 facing=180.00 body=object:0x70001000 scale=1 equipment=[] idle=none look=none
0.000 EffectApplied entity=player effect=1 what=0x70003000 duration_ms=none stacks=1 source=none
0.200 > UseEntity entity=$node
0.200 CastStarted entity=player cast=induction what=0x70002000 target=none duration_ms=7000
7.200 CastEnded entity=player outcome=completed
7.200 LootShown window=1 source=$node entries=[{entry=1 what="Ore" quantity=3}] coin=0
7.200 EntityDespawned entity=$node
7.400 > TakeLoot window=1 entry=none
7.400 LootTaken window=1 entry=1
7.400 ItemAdded item=item:1 what="Ore" at=backpack:0 quantity=3
7.400 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
"##;

#[test]
fn vector_p6_v1() {
    same(&run(VECTOR_P6_V1), LOG_P6_V1);
}

#[test]
fn vector_p6_v2() {
    same(&run(VECTOR_P6_V2), LOG_P6_V2);
}

/// The resolver of case P6-V3 (§14.P6c).
struct Gathering;

impl SelectorResolver for Gathering {
    fn has_install(&self) -> bool {
        true
    }

    fn candidates(
        &mut self,
        query: &SelectorQuery<'_>,
        _limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        let c = |did: u32| Candidate {
            did: DataId(did),
            row: 0,
            location: None,
            anchor: None,
        };
        match query.family {
            Family::Creature => {
                out.push(c(0x7000_1001));
                out.push(c(0x7000_1000));
            }
            Family::Induction => {
                // `of=$nodes` names the resolved list [0x70001000].
                let FilterValue::Binding(b) = &query.filters[0].value else {
                    panic!("induction of= is a binding");
                };
                let nodes: Vec<u32> = query.referenced(*b).iter().map(|r| r.did.0).collect();
                assert_eq!(nodes, [0x7000_1000]);
                out.push(c(0x7000_2000));
            }
            Family::Effect => out.push(c(0x7000_3000)),
            other => panic!("unexpected family {other:?}"),
        }
        CandidateOutcome::new(CandidateStatus::Ready)
    }

    fn enum_value(&mut self, _context: LabelContext, _label: &str) -> Option<u32> {
        None
    }

    fn display_name(&mut self, _did: DataId, _row: Option<u32>) -> Option<String> {
        None
    }
}

#[test]
fn case_p6_v3() {
    same(&run_with(VECTOR_P6_V3, &mut Gathering), LOG_P6_V3);
}
