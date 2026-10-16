// SPDX-License-Identifier: GPL-3.0-or-later
//! The "P6 progression" revision's hostile rows P6-H1 … P6-H18
//! (`docs/specs/scenario-feed.md` §14.5) and further static errors of the new steps,
//! options, patterns, input forms, selector attributes and the text function
//! `action(VALUE,REF)`: every file is refused with the expected kind and position, without
//! a panic.
#![cfg(feature = "scenario")]

use tessera_net::scenario::{Limit, Scenario, ScenarioErrorKind as K};

const H: &str = "tessera-scenario 1\n";

fn after_header(rest: &str) -> (K, u32, u32) {
    let text = format!("{H}{rest}");
    match Scenario::parse(text.as_bytes()) {
        Ok(_) => panic!("accepted: {text:?}"),
        Err(e) => (e.kind, e.line, e.column),
    }
}

fn accepts(rest: &str) -> Scenario {
    let text = format!("{H}{rest}");
    match Scenario::parse(text.as_bytes()) {
        Ok(s) => s,
        Err(e) => panic!("refused {text:?}: {e}"),
    }
}

#[test]
fn rows_p6_h1_to_p6_h18() {
    let rows: [(&str, (K, u32, u32)); 18] = [
        // P6-H1
        ("at 0s level player 9 loud\n", (K::UnexpectedToken, 2, 22)),
        // P6-H2
        (
            "at 0s trait-rank \"T\" \"A\" 1001\n",
            (K::OutOfRange, 2, 26),
        ),
        // P6-H3
        (
            "at 0s trait-tree none branch=4\n",
            (K::UnexpectedToken, 2, 23),
        ),
        // P6-H4
        (
            "at 0s trainer skills=\"a\" rank=1001\n",
            (K::OutOfRange, 2, 26),
        ),
        // P6-H5
        (
            "at 0s trainer skills=\"a\" offers=spells\n",
            (K::UnexpectedToken, 2, 26),
        ),
        // P6-H6
        ("at 0s untrained\n", (K::MissingArgument, 2, 1)),
        // P6-H7
        (
            "at 0s craft-result \"r\" success batch=3/2\n",
            (K::OutOfRange, 2, 32),
        ),
        // P6-H8
        ("at 0s vocation \"v\" slots=33\n", (K::OutOfRange, 2, 20)),
        // P6-H9
        (
            "at 0s craft-trainer smith \"p\"\n",
            (K::UnexpectedToken, 2, 21),
        ),
        // P6-H10
        (
            "input\nat 1s craft \"r\" optional=256\nend\n",
            (K::OutOfRange, 3, 17),
        ),
        // P6-H11
        (
            "input\nat 1s craft \"r\" output=65\nend\n",
            (K::OutOfRange, 3, 17),
        ),
        // P6-H12
        (
            "input\nat 1s trait-rank \"T\" \"A\"\nend\n",
            (K::MissingArgument, 3, 1),
        ),
        // P6-H13
        ("at 0s recipe \"r\" tier=1001\n", (K::OutOfRange, 2, 18)),
        // P6-H14
        (
            "select $s = recipe one-use=maybe all\n",
            (K::UnexpectedToken, 2, 20),
        ),
        // P6-H15
        (
            "at 0s trainer skills=\"a\" trained=1 trained=2\n",
            (K::UnexpectedToken, 2, 36),
        ),
        // P6-H16
        ("at 0s untrained none coin=5\n", (K::UnexpectedToken, 2, 22)),
        // P6-H17
        (
            "at 0s trait-points 1/2 tree=$t[0]\n",
            (K::UndefinedBinding, 2, 24),
        ),
        // P6-H18
        (
            "at 0s craft-result \"r\" success batch=0/2\n",
            (K::OutOfRange, 2, 32),
        ),
    ];
    for (i, (rest, want)) in rows.iter().enumerate() {
        assert_eq!(after_header(rest), *want, "P6-H{}: {rest:?}", i + 1);
    }
}

#[test]
fn step_forms_beyond_the_rows() {
    for (rest, want) in [
        // `quiet` once, nothing after it.
        (
            "at 0s level player 9 quiet quiet\n",
            (K::UnexpectedToken, 2, 28),
        ),
        // `untrained` lists.
        (
            "at 0s untrained skills=\"a\" coin=1000000000000001\n",
            (K::OutOfRange, 2, 28),
        ),
        (
            "at 0s untrained skills=\"a\" level=10001\n",
            (K::OutOfRange, 2, 28),
        ),
        (
            "at 0s untrained skills=\"a\" rank=1001\n",
            (K::OutOfRange, 2, 28),
        ),
        (
            "at 0s untrained none skills=\"a\"\n",
            (K::UnexpectedToken, 2, 22),
        ),
        ("at 0s untrained none extra\n", (K::UnexpectedToken, 2, 22)),
        ("at 0s untrained \"a\"\n", (K::UnexpectedToken, 2, 17)),
        (
            "at 0s untrained skills=$nope\n",
            (K::UndefinedBinding, 2, 17),
        ),
        // `trainer` entry lists as the P5 ones.
        (
            "at 0s trainer skills=\"a\" trained=0\n",
            (K::OutOfRange, 2, 26),
        ),
        (
            "at 0s trainer skills=\"a\" unmet=1,1\n",
            (K::UnexpectedToken, 2, 26),
        ),
        // `trait-rank`, `trait-tree`, `trait-points`.
        (
            "at 0s trait-rank \"T\" \"A\" 1 2\n",
            (K::UnexpectedToken, 2, 28),
        ),
        ("at 0s trait-rank \"T\"\n", (K::MissingArgument, 2, 1)),
        (
            "at 0s trait-tree \"T\" branch=4294967296\n",
            (K::OutOfRange, 2, 22),
        ),
        (
            "at 0s trait-tree \"T\" branch=@\n",
            (K::UnexpectedToken, 2, 22),
        ),
        ("at 0s trait-tree\n", (K::MissingArgument, 2, 1)),
        (
            "at 0s trait-points 1/2 tree=none\n",
            (K::UnexpectedToken, 2, 24),
        ),
        // `forget-recipe`, `vocation`, `craft-trainer`.
        ("at 0s forget-recipe\n", (K::MissingArgument, 2, 1)),
        ("at 0s forget-recipe none\n", (K::UnexpectedToken, 2, 21)),
        ("at 0s vocation none slots=-1\n", (K::OutOfRange, 2, 21)),
        ("at 0s craft-trainer guild\n", (K::MissingArgument, 2, 1)),
        (
            "at 0s craft-trainer guild \"g\" npc=$who\n",
            (K::UndefinedBinding, 2, 31),
        ),
        // `profession … mastery-tier=`, `recipe` options, `craft-result … batch=`.
        (
            "at 0s profession \"p\" tier=1 proficiency=1/2 mastery=1/2 mastery-tier=1001\n",
            (K::OutOfRange, 2, 57),
        ),
        (
            "at 0s recipe \"r\" category=4294967296\n",
            (K::OutOfRange, 2, 18),
        ),
        (
            "at 0s recipe \"r\" profession=none\n",
            (K::UnexpectedToken, 2, 18),
        ),
        ("at 0s learn \"r\" tier=1\n", (K::UnexpectedToken, 2, 17)),
        (
            "at 0s craft-result \"r\" success batch=1/65536\n",
            (K::OutOfRange, 2, 32),
        ),
        (
            "at 0s craft-result \"r\" success batch=1.5/2\n",
            (K::BadNumber, 2, 32),
        ),
        (
            "at 0s craft-result \"r\" success batch=2\n",
            (K::BadFraction, 2, 32),
        ),
    ] {
        assert_eq!(after_header(rest), want, "{rest:?}");
    }
    let many = vec!["1"; 1025].join(",");
    assert_eq!(
        after_header(&format!("at 0s untrained skills=\"a\" rank={many}\n")),
        (K::LimitExceeded(Limit::Entries), 2, 28)
    );
    // 1,025 literal refs do not fit a 4,096-byte line; the line limit refuses them first.
    let refs = vec!["\"a\""; 1025].join(",");
    assert_eq!(
        after_header(&format!("at 0s untrained skills={refs}\n")),
        (K::LineTooLong, 2, 1)
    );
}

#[test]
fn patterns_and_input_forms() {
    for (rest, want) in [
        // Patterns (§7.5).
        (
            "on train-craft replace=$p[0]\nend\n",
            (K::UndefinedBinding, 2, 16),
        ),
        ("on train-craft extra\nend\n", (K::UnexpectedToken, 2, 16)),
        (
            "on trait-rank \"A\" \"B\"\nend\n",
            (K::UnexpectedToken, 2, 19),
        ),
        ("on trait-branch \"T\"\nend\n", (K::UnexpectedToken, 2, 17)),
        // Input forms (§7.7).
        (
            "input\nat 1s craft \"r\" tier=1001\nend\n",
            (K::OutOfRange, 3, 17),
        ),
        (
            "input\nat 1s craft \"r\" pack pack\nend\n",
            (K::UnexpectedToken, 3, 22),
        ),
        (
            "input\nat 1s craft \"r\" packed\nend\n",
            (K::UnexpectedToken, 3, 17),
        ),
        (
            "input\nat 1s train-craft replace=none\nend\n",
            (K::UnexpectedToken, 3, 19),
        ),
        (
            "input\nat 1s trait-rank \"T\" \"A\" 1001\nend\n",
            (K::OutOfRange, 3, 26),
        ),
        (
            "input\nat 1s trait-branch \"T\"\nend\n",
            (K::MissingArgument, 3, 1),
        ),
        (
            "input\nat 1s trait-branch \"T\" -1\nend\n",
            (K::OutOfRange, 3, 24),
        ),
        // `close craft-trainer` is a window kind like the others; another word is not.
        ("at 0s close crafting\n", (K::UnexpectedToken, 2, 13)),
    ] {
        assert_eq!(after_header(rest), want, "{rest:?}");
    }
    accepts(
        "at 0s close craft-trainer\non close craft-trainer\nend\n\
         on train-craft replace=any\nend\non train-craft replace=none\nend\n\
         on train-craft replace=\"Pottery\"\nend\non trait-rank \"A\" tree=\"T\"\nend\n\
         on trait-branch tree=\"T\"\nend\n\
         input\nat 1s close craft-trainer\nat 1s train-craft\nat 1s trait-branch \"T\" @Left\n\
         at 1s craft \"r\" qty=2 tier=0 output=64 optional=255 pack\nend\n",
    );
}

#[test]
fn selector_attributes() {
    for (rest, want) in [
        (
            "select $s = trait point-based=sometimes all\n",
            (K::UnexpectedToken, 2, 19),
        ),
        (
            "select $s = trait in-tree=$nope all\n",
            (K::UndefinedBinding, 2, 19),
        ),
        (
            "select $r = recipe all\nselect $s = trait in-tree=$r all\n",
            (K::WrongBindingKind, 3, 19),
        ),
        (
            "select $s = vocation kind=npc all\n",
            (K::UnknownAttribute, 2, 22),
        ),
        (
            "select $s = craft-guild profession=$v all\n",
            (K::UndefinedBinding, 2, 25),
        ),
        (
            "select $q = quest all\nselect $s = induction of=$q all\n",
            (K::WrongBindingKind, 3, 23),
        ),
        (
            "select $s = hotspot selectable=1 all\n",
            (K::UnexpectedToken, 2, 21),
        ),
    ] {
        assert_eq!(after_header(rest), want, "{rest:?}");
    }
    accepts(
        "select $t = trait-tree type=@Class branch=1|@Left all\n\
         select $c = trait point-based=yes tier=1..3 in-tree=$t all\n\
         select $v = vocation all\nselect $p = profession of=$v all\n\
         select $g = craft-guild profession=$p all\n\
         select $r = recipe profession=$p tier=2 category=@Food one-use=yes cooldown=no \
         critical=yes facility=no all\n\
         select $n = creature kind=container gathering=yes craft-profession=$p all\n\
         select $f = effect facility=yes all\nselect $h = hotspot selectable=yes facility=yes all\n\
         select $i = induction of=$n all\nselect $j = induction of=$h all\n\
         select $k = induction of=$r all\nselect $s = skill all\nselect $l = induction of=$s all\n",
    );
}

#[test]
fn action_for_text() {
    for (rest, want) in [
        ("page $p action(38,\"\")\n", (K::UnexpectedToken, 2, 9)),
        (
            &format!("page $p action(38,\"{}\")\n", "a".repeat(65)),
            (K::TooLong, 2, 9),
        ),
        ("page $p action(38,\"a\")x\n", (K::UnexpectedToken, 2, 9)),
        ("page $p action(38,$nope[0])\n", (K::UndefinedBinding, 2, 9)),
        ("page $p action(@,\"a\")\n", (K::UnexpectedToken, 2, 9)),
        ("page $p action(-1,\"a\")\n", (K::OutOfRange, 2, 9)),
        ("page $p action(\"a\",\"b\")\n", (K::UnexpectedToken, 2, 9)),
        ("page $p action(1,)\n", (K::UnexpectedToken, 2, 9)),
    ] {
        assert_eq!(after_header(rest), want, "{rest:?}");
    }
    let s = accepts(&format!(
        "select $p = profession all\npage $a action(@Train,$p[0])\n\
         option $a 1 action(@Train,\"{}\") goto=close\n",
        "a".repeat(64)
    ));
    // Both labels are recorded for resolution, in token order, under `action`.
    let labels: Vec<(u32, &str, &str)> = s.labels();
    assert_eq!(labels, [(3, "action", "@Train"), (4, "action", "@Train")]);
    assert!(!s.numeric_enum_values());
    let n = accepts("page $a action(38,\"Pottery\")\n");
    assert!(
        n.numeric_enum_values(),
        "action(N,REF) is a numeric key (§13 rule 7)"
    );
}

#[test]
fn rule_7_enum_positions_and_labels() {
    let s = accepts(
        "at 0s recipe \"r\" category=@Food\nat 0s trait-tree \"T\" branch=@Left\n\
         input\nat 1s trait-branch \"T\" @Right\nend\n",
    );
    assert!(!s.numeric_enum_values());
    assert_eq!(
        s.labels(),
        [
            (2, "category", "@Food"),
            (3, "branch", "@Left"),
            (5, "branch", "@Right")
        ]
    );
    for numeric in [
        "at 0s recipe \"r\" category=7\n",
        "at 0s trait-tree \"T\" branch=4\n",
        "input\nat 1s trait-branch \"T\" 4\nend\n",
        "select $t = trait-tree branch=4 all\n",
        "select $r = recipe category=7 all\n",
    ] {
        assert!(
            accepts(numeric).numeric_enum_values(),
            "{numeric:?} is a numeric enum position"
        );
    }
    // Tiers, ranks, slot counts, batches, outputs and entry lists are plain numbers.
    assert!(
        !accepts(
            "at 0s profession \"p\" tier=2 proficiency=1/2 mastery=1/2 mastery-tier=1\n\
             at 0s trait-rank \"T\" \"A\" 3\nat 0s vocation none slots=3\n\
             at 0s trainer skills=\"a\" rank=1 trained=1 offers=traits\n\
             at 0s craft-result \"r\" failed batch=1/1\n\
             input\nat 1s craft \"r\" tier=1 output=2 optional=3 pack\nend\n"
        )
        .numeric_enum_values()
    );
}

#[test]
fn the_overlay_subset_takes_only_quiet_player_levels() {
    accepts("at 0s level player 9 quiet\n")
        .check_overlay()
        .expect("level player N quiet is in the overlay subset");
    for (rest, col) in [
        ("at 0s untrained none\n", 1),
        ("at 0s trait-rank \"T\" \"A\" 1\n", 1),
        ("at 0s craft-trainer guild \"g\"\n", 1),
        ("on train-craft\nend\n", 1),
        ("input\nat 1s trait-rank \"T\" \"A\" 1\nend\n", 1),
    ] {
        let e = accepts(rest).check_overlay().expect_err(rest);
        assert_eq!(e.kind, K::NotAllowedInOverlay, "{rest:?}");
        assert_eq!(e.column, col, "{rest:?}");
    }
}
