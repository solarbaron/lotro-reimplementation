// SPDX-License-Identifier: GPL-3.0-or-later
//! The P5 travel revision of the scenario feed (`docs/specs/scenario-feed.md` §18.P5; map,
//! travel, mounts and instances): Vectors T1 and T2 and resolver case T3 (§14.T1–§14.T3)
//! byte for byte, and the run-time rules of marker sets, the travel and finder options,
//! the queue, mount class and mount vitals beyond the vectors.
#![cfg(feature = "scenario")]

use std::sync::Arc;

use tessera_net::model::{Location, RegionId};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{
    Candidate, CandidateOutcome, CandidateStatus, EmptyResolver, Family, HeadlessDriver,
    LabelContext, MAX_RUNTIME_MARKERS, ResolvedScenario, Scenario, SelectorQuery, SelectorResolver,
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

fn end_line(log: &str) -> &str {
    log.lines().last().unwrap_or_default()
}

fn loc(region: u16, x: f64, y: f64) -> Option<Location> {
    Some(Location::new(RegionId(region), x, y, 0.0))
}

/// The resolver of case T3: `map-region` answers three candidates (one without a
/// location), each with a display name; labels resolve from a fixed table.
#[derive(Default)]
struct Regions {
    /// (context, label) → key.
    keys: Vec<(LabelContext, &'static str, u32)>,
    /// Every `display_name` call, in order.
    names_asked: Vec<(DataId, Option<u32>)>,
    /// Extra located candidates for the cap test (0 for case T3).
    many: u32,
    /// Every `enum_value` call, in order.
    labels_asked: Vec<(LabelContext, String)>,
}

impl SelectorResolver for Regions {
    fn has_install(&self) -> bool {
        true
    }

    fn candidates(
        &mut self,
        query: &SelectorQuery<'_>,
        _limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        if query.family != Family::MapRegion {
            return CandidateOutcome::new(CandidateStatus::Ready);
        }
        if self.many > 0 {
            for i in 0..self.many {
                out.push(Candidate {
                    did: DataId(0x7800_0000 + i),
                    row: 0,
                    location: loc(1, f64::from(i % 100), 0.0),
                    anchor: None,
                });
            }
            return CandidateOutcome::new(CandidateStatus::Ready);
        }
        out.push(Candidate {
            did: DataId(0x7800_0010),
            row: 0,
            location: loc(2, 100.0, 200.0),
            anchor: None,
        });
        out.push(Candidate {
            did: DataId(0x1000_0500),
            row: 3,
            location: loc(1, 5.0, 6.0),
            anchor: None,
        });
        out.push(Candidate {
            did: DataId(0x1000_0400),
            row: 1,
            location: None,
            anchor: None,
        });
        CandidateOutcome::new(CandidateStatus::Ready)
    }

    fn enum_value(&mut self, context: LabelContext, label: &str) -> Option<u32> {
        self.labels_asked.push((context, label.to_owned()));
        self.keys
            .iter()
            .find(|(c, l, _)| *c == context && *l == label)
            .map(|(_, _, k)| *k)
    }

    fn display_name(&mut self, did: DataId, row: Option<u32>) -> Option<String> {
        self.names_asked.push((did, row));
        match did.0 {
            0x7800_0010 => Some("Pit".into()),
            0x1000_0500 => Some("Vale".into()),
            0x1000_0400 => Some("North".into()),
            _ => None,
        }
    }
}

const VECTOR_T1: &str = r#"tessera-scenario 1
title "Vector T1"
tick 100ms
player "Testa"
actor $groom kind=npc name="Groom" pos=0,3,0 facing=180
select $regions = map-region all
at 0s spawn $groom
+0s marker $camp 4,5,0 "Camp" note-type=7
+0s markers $maps $regions named
on use $groom
+0s travel-list dest="Low road","High pass","Ferry" from=$groom coin=0,250 min-level=1,40 undiscovered=3 unusable=3 swift=2 area="Vale"
end
on choose-travel entry=2
+0s close travel
+0s travel "High pass" 2s route=swift
end
on dismount
+0s travel-end
end
input
at 0.1s use $groom
at 0.2s choose-travel 2
at 0.5s dismount
end
"#;

const LOG_T1: &str = r#"tessera-scenario-log 1
install none
tick 100
title "Vector T1"
resolve $regions line=6 family=map-region requested=all candidates=0 got=0 note=no-install
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$groom kind=non-player name="Groom" at=0.000,3.000,0.000 facing=180.00
0.000 MarkerAdded marker=1 location=0:4.000,5.000,0.000 label="Camp" icon=none quest=none note_type=7
0.100 > UseEntity entity=$groom
0.100 TravelShown window=1 source=$groom entries=[{entry=1 destination="Low road" coin=0 min_level=1 area="Vale"},{entry=2 destination="High pass" coin=250 min_level=40 route=swift area="Vale"},{entry=3 destination="Ferry" coin=250 min_level=40 flags=undiscovered+unusable area="Vale"}]
0.200 > ChooseTravel window=1 entry=2
0.200 WindowClosed window=1
0.200 TravelStarted destination="High pass" duration_ms=2000 route=swift
0.500 > Dismount
0.500 TravelEnded
0.500 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
"#;

const VECTOR_T2: &str = r#"tessera-scenario 1
title "Vector T2"
tick 100ms
player "Testa"
actor $warden kind=npc name="Warden" pos=2,0,0 facing=270
at 0s spawn $warden
+0s instances "Old mine","Deep vault" from=$warden featured="Deep vault" level=20,45 size=3,6 tier=1 locked=2
on join-instance entry=1
+0s instance-queue queued "Old mine" seconds=0
end
on leave-queue
+0s instance-queue none
+0s close instances
end
at 1s mount player "Grey steed" kind=combat
+0s mount-vital player morale 800/1000
+0.5s mount player none
input
at 0.1s join-instance 1 size=3 tier=1 level=20
at 0.3s leave-queue
end
"#;

const LOG_T2: &str = r#"tessera-scenario-log 1
install none
tick 100
title "Vector T2"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$warden kind=non-player name="Warden" at=2.000,0.000,0.000 facing=270.00
0.000 InstanceListShown window=1 entries=[{entry=1 instance="Old mine" level=20 size=3 tier=1},{entry=2 instance="Deep vault" level=45 size=6 tier=1 locked=yes}] source=$warden featured="Deep vault"
0.100 > JoinInstance window=1 entry=1 size=3 tier=1 level=20
0.100 InstanceQueueChanged state=queued instance="Old mine" seconds=0
0.300 > LeaveQueue
0.300 InstanceQueueChanged state=none instance=none seconds=none
0.300 WindowClosed window=1
1.000 MountChanged entity=player mount="Grey steed" combat=yes
1.000 MountVitalChanged entity=player vital=morale current=800 max=1000
1.500 MountChanged entity=player mount=none
1.500 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
"#;

const VECTOR_T3: &str = r#"tessera-scenario 1
title "Vector T3"
tick 100ms
player "Testa"
select $regions = map-region all
at 0s marker $camp 4,5,0 "Camp"
+0s markers $maps $regions named note-type=7
+0.5s markers $maps $regions "Again"
+0.5s marker-remove $maps
"#;

const LOG_T3: &str = r#"tessera-scenario-log 1
install present
tick 100
title "Vector T3"
resolve $regions line=5 family=map-region requested=all candidates=3 got=3
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 MarkerAdded marker=1 location=0:4.000,5.000,0.000 label="Camp" icon=none quest=none
0.000 MarkerAdded marker=2 location=1:5.000,6.000,0.000 label="Vale" icon=none quest=none note_type=7
0.000 MarkerAdded marker=3 location=2:100.000,200.000,0.000 label="Pit" icon=none quest=none note_type=7
0.500 MarkerRemoved marker=2
0.500 MarkerRemoved marker=3
0.500 MarkerAdded marker=4 location=1:5.000,6.000,0.000 label="Again" icon=none quest=none
0.500 MarkerAdded marker=5 location=2:100.000,200.000,0.000 label="Again" icon=none quest=none
1.000 MarkerRemoved marker=4
1.000 MarkerRemoved marker=5
1.000 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
"#;

#[test]
fn vector_t1_travel_rows_route_travel_end_note_type() {
    same(&run(VECTOR_T1), LOG_T1);
}

#[test]
fn vector_t2_finder_rows_join_options_queue_mount() {
    same(&run(VECTOR_T2), LOG_T2);
}

#[test]
fn case_t3_marker_sets_over_map_regions() {
    let mut r = Regions::default();
    same(&run_with(VECTOR_T3, &mut r), LOG_T3);
    // Names are asked with the row, only for elements with a location, once per binding.
    assert_eq!(
        r.names_asked,
        [
            (DataId(0x1000_0500), Some(3)),
            (DataId(0x7800_0010), Some(0))
        ]
    );
}

#[test]
fn vectors_are_deterministic() {
    assert_eq!(run(VECTOR_T1), run(VECTOR_T1));
    assert_eq!(run(VECTOR_T2), run(VECTOR_T2));
    let a = run_with(VECTOR_T3, &mut Regions::default());
    let b = run_with(VECTOR_T3, &mut Regions::default());
    assert_eq!(a, b);
}

#[test]
fn note_type_labels_resolve_and_keys_outside_1_to_64_are_unresolved() {
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
                at 0s marker $a 1,0,0 \"A\" note-type=@Camp\n\
                +0s marker $b 2,0,0 \"B\" note-type=@Huge\n\
                +0s marker $c 3,0,0 \"C\" note-type=@Unknown\n";
    let mut r = Regions {
        keys: vec![
            (LabelContext::MapNoteType, "@Camp", 12),
            (LabelContext::MapNoteType, "@Huge", 65),
        ],
        ..Regions::default()
    };
    let log = run_with(text, &mut r);
    assert!(
        log.contains("label line=5 attribute=note-type label=@Huge\n"),
        "{log}"
    );
    assert!(
        log.contains("label line=6 attribute=note-type label=@Unknown\n"),
        "{log}"
    );
    assert!(
        log.contains("label=\"A\" icon=none quest=none note_type=12\n"),
        "{log}"
    );
    assert!(log.contains("label=\"B\" icon=none quest=none\n"), "{log}");
    assert!(log.contains("label=\"C\" icon=none quest=none\n"), "{log}");
    // Without an install a label gives none and no note.
    let log = run(text);
    assert!(!log.contains("label line="), "{log}");
    assert!(!log.contains("note_type="), "{log}");
}

#[test]
fn size_and_tier_labels_repeat_and_unresolved_ones_give_none() {
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
                at 0s instances \"A\",\"B\",\"C\" size=@Small,@Odd tier=@Hard level=5\n";
    let mut r = Regions {
        keys: vec![
            (LabelContext::GroupSize, "@Small", 3),
            (LabelContext::DifficultyTier, "@Hard", 2),
        ],
        ..Regions::default()
    };
    let log = run_with(text, &mut r);
    assert!(
        log.contains("label line=4 attribute=size label=@Odd\n"),
        "{log}"
    );
    assert!(
        log.contains(
            "InstanceListShown window=1 entries=[{entry=1 instance=\"A\" level=5 size=3 tier=2},\
             {entry=2 instance=\"B\" level=5 tier=2},{entry=3 instance=\"C\" level=5 tier=2}]\n"
        ),
        "{log}"
    );
}

#[test]
fn size_and_tier_labels_resolve_in_token_order() {
    // §8.6 item 6 (P5 travel): line order, token order and, within a list, item order,
    // whichever of `size=` and `tier=` is written first.
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
                at 0s instances \"A\" tier=@T1,@T2 size=@S1\n\
                +0s instances \"A\" size=@S2 tier=@T3\n\
                input\n\
                at 1s join-instance 1 tier=@T4 size=@S3\n\
                at 2s join-instance 1 size=@S4 tier=@T5\n\
                end\n";
    let scenario = Scenario::parse(text.as_bytes()).expect("parse");
    let listed: Vec<(u32, &str, &str)> = scenario.labels();
    let order = [
        (4, "tier", "@T1"),
        (4, "tier", "@T2"),
        (4, "size", "@S1"),
        (5, "size", "@S2"),
        (5, "tier", "@T3"),
        (7, "tier", "@T4"),
        (7, "size", "@S3"),
        (8, "size", "@S4"),
        (8, "tier", "@T5"),
    ];
    assert_eq!(listed, order);
    let mut r = Regions::default();
    let _ = scenario.resolve(&mut r).expect("resolve");
    let asked: Vec<(LabelContext, &str)> = r
        .labels_asked
        .iter()
        .map(|(c, l)| (*c, l.as_str()))
        .collect();
    let want: Vec<(LabelContext, &str)> = order
        .iter()
        .map(|(_, a, l)| {
            let c = if *a == "size" {
                LabelContext::GroupSize
            } else {
                LabelContext::DifficultyTier
            };
            (c, *l)
        })
        .collect();
    assert_eq!(asked, want);
}

#[test]
fn join_instance_with_an_unresolved_label_is_input_skipped() {
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
                at 0s instances \"A\"\ninput\n\
                at 0.1s join-instance 1 size=@Small\n\
                at 0.2s join-instance 1 tier=7\n\
                end\n";
    let log = run(text);
    assert!(!log.contains("size="), "{log}");
    assert!(
        log.contains("0.200 > JoinInstance window=1 entry=1 tier=7\n"),
        "{log}"
    );
    assert!(end_line(&log).contains("input-skipped=1"), "{log}");
    let mut r = Regions {
        keys: vec![(LabelContext::GroupSize, "@Small", 6)],
        ..Regions::default()
    };
    let log = run_with(text, &mut r);
    assert!(
        log.contains("0.100 > JoinInstance window=1 entry=1 size=6\n"),
        "{log}"
    );
    assert!(end_line(&log).contains("input-skipped=0"), "{log}");
}

#[test]
fn travel_options_without_values_keep_the_old_rows() {
    // Entry numbers beyond the list name nothing; `slow` and `unmet` work like the others.
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
                at 0s travel-list dest=\"A\",\"B\" unmet=1,9 slow=2 swift=7\n\
                +0s travel \"B\" 1s\n\
                +0s travel \"B\" 1s route=slow\n";
    let log = run(text);
    assert!(
        log.contains(
            "entries=[{entry=1 destination=\"A\" coin=0 flags=unmet},\
             {entry=2 destination=\"B\" coin=0 route=slow}]\n"
        ),
        "{log}"
    );
    assert!(
        log.contains("TravelStarted destination=\"B\" duration_ms=1000\n"),
        "{log}"
    );
    assert!(
        log.contains("TravelStarted destination=\"B\" duration_ms=1000 route=slow\n"),
        "{log}"
    );
}

#[test]
fn mount_kind_basic_and_absent() {
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
                at 0s mount player \"Pony\" kind=basic\n\
                +0s mount player \"Pony\"\n\
                +0s mount-vital player fury 0/0\n\
                +0s mount-vital player durability 1.5/2\n";
    let log = run(text);
    assert!(
        log.contains("MountChanged entity=player mount=\"Pony\" combat=no\n"),
        "{log}"
    );
    assert!(
        log.contains("MountChanged entity=player mount=\"Pony\"\n"),
        "{log}"
    );
    assert!(
        log.contains("MountVitalChanged entity=player vital=fury current=0 max=0\n"),
        "{log}"
    );
    assert!(
        log.contains("MountVitalChanged entity=player vital=durability current=1.5 max=2\n"),
        "{log}"
    );
}

#[test]
fn every_queue_state_keyword_is_logged() {
    let states = [
        "none",
        "confirm",
        "queued",
        "others-confirm",
        "join-pending",
        "confirm-enter-queue",
        "others-confirm-enter-queue",
        "world-join-queue",
    ];
    let mut text = String::from("tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n");
    for (i, s) in states.iter().enumerate() {
        text.push_str(&format!("at {i}s instance-queue {s} \"Q\" seconds={i}\n"));
    }
    let log = run(&text);
    for (i, s) in states.iter().enumerate() {
        assert!(
            log.contains(&format!(
                "InstanceQueueChanged state={s} instance=\"Q\" seconds={i}\n"
            )),
            "{log}"
        );
    }
}

#[test]
fn marker_sets_and_single_markers_number_apart() {
    // Single markers take 1 … M in file order of their first `marker` line, even when a
    // set's first line comes earlier; run-time ids follow in execution order.
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
                select $r = map-region all\n\
                at 0s markers $s $r \"S\" quest=\"Q\"\n\
                +0s marker $a 1,0,0 \"A\"\n\
                +0s marker $b 2,0,0 \"B\"\n\
                +0s marker-remove $b\n\
                +1s marker-remove $s\n\
                +0s marker-remove $s\n";
    let log = run_with(text, &mut Regions::default());
    let ours: Vec<&str> = log
        .lines()
        .filter(|l| l.contains("Marker"))
        .map(|l| l.split_once(' ').map_or(l, |(_, r)| r))
        .collect();
    assert_eq!(
        ours,
        [
            "MarkerAdded marker=3 location=1:5.000,6.000,0.000 label=\"S\" icon=none quest=\"Q\"",
            "MarkerAdded marker=4 location=2:100.000,200.000,0.000 label=\"S\" icon=none quest=\"Q\"",
            "MarkerAdded marker=1 location=0:1.000,0.000,0.000 label=\"A\" icon=none quest=none",
            "MarkerAdded marker=2 location=0:2.000,0.000,0.000 label=\"B\" icon=none quest=none",
            "MarkerRemoved marker=2",
            "MarkerRemoved marker=3",
            "MarkerRemoved marker=4",
        ],
        "{log}"
    );
    assert!(end_line(&log).contains("skipped=0"), "{log}");
}

#[test]
fn markers_past_the_run_time_cap_are_a_skip() {
    // 40,000 located regions: the first step takes ids 1 … 40,000, the second would pass
    // 65,536 and emits nothing, removals included.
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
                select $r = map-region all\n\
                at 0s markers $s $r \"S\"\n\
                +1s markers $s $r \"T\"\n\
                +1s marker-remove $s\n";
    // One step fits under the cap, two do not.
    const { assert!(40_000 <= MAX_RUNTIME_MARKERS && 80_000 > MAX_RUNTIME_MARKERS) };
    let mut r = Regions {
        many: 40_000,
        ..Regions::default()
    };
    let log = run_with(text, &mut r);
    let added = log.lines().filter(|l| l.contains("MarkerAdded")).count();
    let removed = log.lines().filter(|l| l.contains("MarkerRemoved")).count();
    assert_eq!(added, 40_000);
    assert_eq!(removed, 40_000);
    assert!(!log.contains("label=\"T\""));
    assert!(end_line(&log).contains("skipped=1"), "{}", end_line(&log));
}

#[test]
fn a_missing_icon_element_skips_the_markers_step() {
    let text = "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
                select $r = map-region all\n\
                select $i = item all\n\
                at 0s markers $s $r named icon=$i[0]\n";
    let log = run_with(text, &mut Regions::default());
    assert!(!log.contains("MarkerAdded"), "{log}");
    assert!(end_line(&log).contains("skipped=1"), "{log}");
}

#[test]
fn p5_numeric_keys_are_numeric_enum_values() {
    let parse = |rest: &str| {
        Scenario::parse(format!("tessera-scenario 1\n{rest}").as_bytes()).expect("parse")
    };
    for rest in [
        "at 0s marker $m 0,0,0 \"x\" note-type=3\n",
        "select $r = map-region all\nat 0s markers $m $r named note-type=3\n",
        "at 0s instances \"a\" size=@S,2\n",
        "at 0s instances \"a\" tier=1\n",
        "input\nat 1s join-instance 1 size=2\nend\n",
        "input\nat 1s join-instance 1 tier=2\nend\n",
        "select $e = effect mount-type=4 all\n",
    ] {
        assert!(parse(rest).numeric_enum_values(), "{rest}");
    }
    for rest in [
        "at 0s marker $m 0,0,0 \"x\" note-type=@Camp\n",
        "at 0s instances \"a\" size=@S tier=@T level=3 locked=1\n",
        "input\nat 1s join-instance 1 size=@S tier=@T level=4\nend\n",
        "select $e = effect role=mount war-steed=yes mount-type=@Horse all\n",
        "at 0s travel-list dest=\"a\" min-level=3 swift=1\n",
    ] {
        let s = parse(rest);
        assert!(!s.numeric_enum_values(), "{rest}");
    }
    let s =
        parse("at 0s instances \"a\" size=@S tier=@T\nat 1s marker $m 0,0,0 \"x\" note-type=@N\n");
    assert_eq!(
        s.labels(),
        [(2, "size", "@S"), (2, "tier", "@T"), (3, "note-type", "@N")]
    );
}

#[test]
fn map_region_and_mount_filters_reach_the_resolver() {
    struct Seen(Vec<(Family, Vec<&'static str>)>);
    impl SelectorResolver for Seen {
        fn has_install(&self) -> bool {
            true
        }
        fn candidates(
            &mut self,
            q: &SelectorQuery<'_>,
            _limit: usize,
            _out: &mut Vec<Candidate>,
        ) -> CandidateOutcome {
            self.0.push((
                q.family,
                q.filters.iter().map(|f| f.attribute.name()).collect(),
            ));
            CandidateOutcome::new(CandidateStatus::Unsupported)
        }
        fn enum_value(&mut self, _c: LabelContext, _l: &str) -> Option<u32> {
            None
        }
        fn display_name(&mut self, _d: DataId, _r: Option<u32>) -> Option<String> {
            None
        }
    }
    let text = "tessera-scenario 1\nplayer \"Testa\"\n\
                select $r = map-region kind=node|dungeon region=1|2 all\n\
                select $m = effect role=mount war-steed=no mount-type=@Horse n=3\n";
    let s = Scenario::parse(text.as_bytes()).expect("parse");
    let mut seen = Seen(Vec::new());
    let resolved = s.resolve(&mut seen).expect("resolve");
    assert_eq!(
        seen.0,
        [
            (Family::MapRegion, vec!["kind", "region"]),
            (Family::Effect, vec!["role", "war-steed", "mount-type"]),
        ]
    );
    let header = resolved.log_header().join("\n");
    assert!(
        header.contains(
            "resolve $r line=3 family=map-region requested=all candidates=0 got=0 note=unsupported"
        ),
        "{header}"
    );
}
