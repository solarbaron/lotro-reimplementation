// SPDX-License-Identifier: GPL-3.0-or-later
//! The P3 revision's hostile rows N-H1 … N-H11 (`docs/specs/scenario-feed.md` §14.5) and
//! further static errors of `action(…)`, `rewards`, `buyback`, the `buy-back` pattern and
//! input form: every file is refused with the expected kind and position, without a panic.
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

fn accepts(rest: &str) {
    let text = format!("{H}{rest}");
    if let Err(e) = Scenario::parse(text.as_bytes()) {
        panic!("refused {text:?}: {e}");
    }
}

/// The overlay check of §12.5.1 on a file that parses.
fn overlay(rest: &str) -> (K, u32, u32) {
    let text = format!("{H}{rest}");
    let s = Scenario::parse(text.as_bytes()).expect("parse");
    match s.check_overlay() {
        Ok(()) => panic!("allowed in an overlay run: {text:?}"),
        Err(e) => (e.kind, e.line, e.column),
    }
}

#[test]
fn rows_n_h1_to_n_h11() {
    // N-H1
    assert_eq!(
        after_header("page $p \"x\"\noption $p 1 action(Vendor)\n"),
        (K::UnexpectedToken, 3, 13)
    );
    // N-H2
    assert_eq!(after_header("at 0s rewards\n"), (K::MissingArgument, 2, 1));
    // N-H3
    assert_eq!(
        after_header("at 0s rewards gold:5\n"),
        (K::UnexpectedToken, 2, 15)
    );
    // N-H4
    assert_eq!(
        after_header("at 0s rewards coin:-5\n"),
        (K::OutOfRange, 2, 15)
    );
    // N-H5
    assert_eq!(
        after_header("at 0s rewards coin:1 coin:2\n"),
        (K::UnexpectedToken, 2, 22)
    );
    // N-H6
    let seventeen = vec!["rep:\"a\":1"; 17].join(" ");
    assert_eq!(
        after_header(&format!("at 0s rewards {seventeen}\n")),
        (K::LimitExceeded(Limit::Entries), 2, 175)
    );
    // N-H7
    assert_eq!(
        after_header("at 0s rewards trait-points:1000001\n"),
        (K::OutOfRange, 2, 15)
    );
    // N-H8
    assert_eq!(
        after_header("at 0s rewards rep:\"a\":1000000000001\n"),
        (K::OutOfRange, 2, 15)
    );
    // N-H9
    assert_eq!(
        after_header("at 0s buyback qty=2\n"),
        (K::MissingArgument, 2, 1)
    );
    // N-H10
    assert_eq!(
        after_header("input\nat 1s buy-back 0\nend\n"),
        (K::OutOfRange, 3, 16)
    );
    // N-H11
    assert_eq!(
        after_header("at 0s say player action(@Vendor)\n"),
        (K::UnexpectedToken, 2, 18)
    );
}

#[test]
fn action_arguments() {
    for (rest, want) in [
        ("page $p action()\n", (K::UnexpectedToken, 2, 9)),
        ("page $p action(@)\n", (K::UnexpectedToken, 2, 9)),
        ("page $p action(@1x)\n", (K::UnexpectedToken, 2, 9)),
        ("page $p action(\"x\")\n", (K::UnexpectedToken, 2, 9)),
        ("page $p action(-1)\n", (K::OutOfRange, 2, 9)),
        ("page $p action(4294967296)\n", (K::OutOfRange, 2, 9)),
        ("page $p action(0x10)\n", (K::BadNumber, 2, 9)),
        // Since the P6 progression revision `action(N,REF)` is a form; `2` is no REF.
        ("page $p action(1,2)\n", (K::UnexpectedToken, 2, 9)),
        ("page $p action(01)\n", (K::BadNumber, 2, 9)),
        ("page $p action(@a b)\n", (K::UnexpectedToken, 2, 9)),
    ] {
        assert_eq!(after_header(rest), want, "{rest:?}");
    }
    let long = format!("page $p action(@{})\n", "a".repeat(128));
    assert_eq!(after_header(&long), (K::TooLong, 2, 9));
    let longest = format!("page $p action(@{})\n", "a".repeat(127));
    accepts(&longest);
    let many = format!("page $p action({})\n", "1".repeat(33));
    assert_eq!(after_header(&many), (K::BadNumber, 2, 9));
    // Every TEXT position takes it; chat and names do not.
    accepts("page $p action(@Vendor)\noption $p 1 action(4294967295) goto=close\n");
    accepts(
        "on create-character\n+0s refuse-character \"A\" action(3)\nend\n\
         at 0s marker $m 0,0,0 action(@Trainer)\n",
    );
    assert_eq!(
        after_header("at 0s system action(1)\n"),
        (K::UnexpectedToken, 2, 14)
    );
    assert_eq!(
        after_header("actor $a kind=npc pos=0,0,0 name=action(1)\n"),
        (K::UnexpectedToken, 2, 29)
    );
}

#[test]
fn reward_tokens() {
    for (rest, want) in [
        ("at 0s rewards coin\n", (K::UnexpectedToken, 2, 15)),
        ("at 0s rewards coin:\n", (K::UnexpectedToken, 2, 15)),
        ("at 0s rewards coin=5\n", (K::UnexpectedToken, 2, 15)),
        ("at 0s rewards :5\n", (K::UnexpectedToken, 2, 15)),
        ("at 0s rewards coin:1:2\n", (K::UnexpectedToken, 2, 15)),
        ("at 0s rewards \"coin\":1\n", (K::UnexpectedToken, 2, 15)),
        ("at 0s rewards rep:\"a\"\n", (K::UnexpectedToken, 2, 15)),
        ("at 0s rewards rep::5\n", (K::UnexpectedToken, 2, 15)),
        ("at 0s rewards rep:5\n", (K::UnexpectedToken, 2, 15)),
        ("at 0s rewards reputation:5\n", (K::UnexpectedToken, 2, 15)),
        ("at 0s rewards coin:1.5\n", (K::BadNumber, 2, 15)),
        ("at 0s rewards coin:0x10\n", (K::BadNumber, 2, 15)),
        (
            "at 0s rewards coin:1000000000000001\n",
            (K::OutOfRange, 2, 15),
        ),
        ("at 0s rewards glory:-1\n", (K::OutOfRange, 2, 15)),
        ("at 0s rewards wallet:\"t\":-1\n", (K::OutOfRange, 2, 15)),
        (
            "at 0s rewards rep:\"a\":-1000000000001\n",
            (K::OutOfRange, 2, 15),
        ),
        (
            "at 0s rewards rep:$nope[0]:1\n",
            (K::UndefinedBinding, 2, 15),
        ),
        (
            "at 0s rewards trait-points:1 glory:2 trait-points:3\n",
            (K::UnexpectedToken, 2, 38),
        ),
    ] {
        assert_eq!(after_header(rest), want, "{rest:?}");
    }
    let long = format!("at 0s rewards coin:{}\n", "1".repeat(33));
    assert_eq!(after_header(&long), (K::BadNumber, 2, 15));
    // Sixteen tokens, the seven single kinds once each, the others any number of times.
    accepts(
        "at 0s rewards coin:0 glory:1 item-xp:2 craft-xp:3 mount-xp:4 virtue-xp:5 \
         trait-points:1000000 rep:\"a\":-1000000000000 rep:\"a\":1000000000000 \
         wallet:\"t\":1000000000000000 wallet:\"t\":0 rep:\"b\":1 rep:\"c\":1 rep:\"d\":1 \
         rep:\"e\":1 rep:\"f\":1\n",
    );
    accepts("select $f = did 0x70000001 all\nat 0s rewards rep:$f[0]:5 wallet:$f[3]:1\n");
    // `rep` and `wallet` refs keep a `:` inside their string.
    accepts("at 0s rewards rep:\"a:b\":5\n");
}

#[test]
fn buyback_arguments() {
    for (rest, want) in [
        ("at 0s buyback\n", (K::MissingArgument, 2, 1)),
        ("at 0s buyback items=\"a\" qty=0\n", (K::OutOfRange, 2, 25)),
        (
            "at 0s buyback items=\"a\" qty=1000000001\n",
            (K::OutOfRange, 2, 25),
        ),
        (
            "at 0s buyback items=\"a\" stock=5\n",
            (K::UnexpectedToken, 2, 25),
        ),
        (
            "at 0s buyback items=\"a\" npc=player\n",
            (K::UnexpectedToken, 2, 25),
        ),
        ("at 0s buyback \"a\"\n", (K::UnexpectedToken, 2, 15)),
        (
            "at 0s buyback items=\"a\" price=1000000000000001\n",
            (K::OutOfRange, 2, 25),
        ),
        (
            "at 0s buyback items=\"a\" barter=\"t\"\n",
            (K::UnexpectedToken, 2, 25),
        ),
        (
            "at 0s buyback items=\"a\" barter=\"t\":0\n",
            (K::OutOfRange, 2, 25),
        ),
        (
            "at 0s buyback items=\"a\" qty=1 qty=2\n",
            (K::UnexpectedToken, 2, 31),
        ),
        (
            "at 0s buyback items=\"a\",,\"b\"\n",
            (K::UnexpectedToken, 2, 15),
        ),
        (
            "item $i \"Ring\"\nat 0s buyback items=$i\n",
            (K::WrongBindingKind, 3, 15),
        ),
    ] {
        assert_eq!(after_header(rest), want, "{rest:?}");
    }
    accepts(
        "select $s = did 0x70000001 all\nat 0s buyback items=$s qty=2 price=1,2 barter=\"t\":3\n",
    );
}

#[test]
fn buy_back_pattern_and_input_form() {
    for (rest, want) in [
        (
            "on buy-back entry=0\n+0s system \"x\"\nend\n",
            (K::OutOfRange, 2, 13),
        ),
        (
            "on buy-back entry=1025\n+0s system \"x\"\nend\n",
            (K::OutOfRange, 2, 13),
        ),
        (
            "on buy-back 1\n+0s system \"x\"\nend\n",
            (K::UnexpectedToken, 2, 13),
        ),
        ("input\nat 1s buy-back\nend\n", (K::MissingArgument, 3, 1)),
        ("input\nat 1s buy-back 1025\nend\n", (K::OutOfRange, 3, 16)),
        (
            "input\nat 1s buy-back 1 2\nend\n",
            (K::UnexpectedToken, 3, 18),
        ),
        (
            "input\nat 1s buy-back entry=1\nend\n",
            (K::BadNumber, 3, 16),
        ),
    ] {
        assert_eq!(after_header(rest), want, "{rest:?}");
    }
    accepts(
        "on buy-back entry=1024 times=2\n+0s system \"x\"\nend\n\
         on buy-back\n+0s system \"y\"\nend\ninput\nat 1s buy-back 1024\nend\n",
    );
}

#[test]
fn p3_forms_are_outside_the_overlay_subset() {
    assert_eq!(
        overlay("at 0s rewards coin:1\n"),
        (K::NotAllowedInOverlay, 2, 1)
    );
    assert_eq!(
        overlay("at 0s buyback items=\"a\"\n"),
        (K::NotAllowedInOverlay, 2, 1)
    );
    assert_eq!(
        overlay("input\nat 1s buy-back 1\nend\n"),
        (K::NotAllowedInOverlay, 3, 1)
    );
    assert_eq!(
        overlay("page $p action(1)\n"),
        (K::NotAllowedInOverlay, 2, 1)
    );
}
