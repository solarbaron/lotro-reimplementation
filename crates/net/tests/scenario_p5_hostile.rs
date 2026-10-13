// SPDX-License-Identifier: GPL-3.0-or-later
//! The P5 travel revision's hostile rows T-H1 … T-H18 (`docs/specs/scenario-feed.md`
//! §14.5) and further static errors of marker sets, the `markers`, `travel-end`,
//! `instance-queue` and `mount-vital` steps, the travel, finder and mount options, the
//! `leave-queue` pattern and the `join-instance` input options: every file is refused with
//! the expected kind and position, without a panic.
#![cfg(feature = "scenario")]

use tessera_net::scenario::{Limit, MAX_QUEUE_SECONDS, Scenario, ScenarioErrorKind as K};

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

#[test]
fn rows_t_h1_to_t_h18() {
    // T-H1
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" min-level=10001\n"),
        (K::OutOfRange, 2, 28)
    );
    // T-H2
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" undiscovered=0\n"),
        (K::OutOfRange, 2, 28)
    );
    // T-H3
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" swift=1 slow=1\n"),
        (K::UnexpectedToken, 2, 36)
    );
    // T-H4
    assert_eq!(
        after_header("at 0s travel \"a\" 1s route=fast\n"),
        (K::UnexpectedToken, 2, 21)
    );
    // T-H5
    assert_eq!(
        after_header("at 0s instances \"a\" size=1,@Small,x\n"),
        (K::UnexpectedToken, 2, 21)
    );
    // T-H6
    assert_eq!(
        after_header("at 0s instances \"a\" locked=1,1\n"),
        (K::UnexpectedToken, 2, 21)
    );
    // T-H7
    assert_eq!(
        after_header("at 0s instance-queue waiting\n"),
        (K::UnexpectedToken, 2, 22)
    );
    // T-H8
    assert_eq!(
        after_header("at 0s instance-queue queued seconds=86401\n"),
        (K::OutOfRange, 2, 29)
    );
    // T-H9
    assert_eq!(
        after_header("at 0s mount player none kind=combat\n"),
        (K::UnexpectedToken, 2, 25)
    );
    // T-H10
    assert_eq!(
        after_header("at 0s mount-vital player morale 5/4\n"),
        (K::OutOfRange, 2, 33)
    );
    // T-H11
    assert_eq!(
        after_header("at 0s mount-vital player stamina 1/1\n"),
        (K::UnexpectedToken, 2, 26)
    );
    // T-H12
    assert_eq!(
        after_header("at 0s marker $m 0,0,0 \"x\" note-type=65\n"),
        (K::OutOfRange, 2, 27)
    );
    // T-H13
    assert_eq!(
        after_header(
            "select $s = map-region all\nat 0s marker $m 0,0,0 \"x\"\nat 1s markers $m $s named\n"
        ),
        (K::WrongBindingKind, 4, 15)
    );
    // T-H14
    assert_eq!(
        after_header("select $s = map-region kind=area all\n"),
        (K::UnexpectedToken, 2, 24)
    );
    // T-H15
    assert_eq!(
        after_header("input\nat 1s join-instance 1 level=0\nend\n"),
        (K::OutOfRange, 3, 23)
    );
    // T-H16
    assert_eq!(
        after_header("at 0s travel-end now\n"),
        (K::UnexpectedToken, 2, 18)
    );
    // T-H17
    assert_eq!(
        after_header("select $s = map-region all\nat 0s markers $ms $s\n"),
        (K::MissingArgument, 3, 1)
    );
    // T-H18
    let items = vec!["1"; 1025].join(",");
    assert_eq!(
        after_header(&format!("at 0s travel-list dest=\"a\" min-level={items}\n")),
        (K::LimitExceeded(Limit::Entries), 2, 28)
    );
}

#[test]
fn marker_set_binding_kinds() {
    // A set named by `marker`.
    assert_eq!(
        after_header(
            "select $s = map-region all\nat 0s markers $m $s \"x\"\n+0s marker $m 0,0,0 \"x\"\n"
        ),
        (K::WrongBindingKind, 4, 12)
    );
    // `markers` over something that is not a selector binding.
    assert_eq!(
        after_header("actor $a kind=npc pos=0,0,0\nat 0s markers $m $a \"x\"\n"),
        (K::WrongBindingKind, 3, 18)
    );
    assert_eq!(
        after_header("at 0s markers $m $nope \"x\"\n"),
        (K::UndefinedBinding, 2, 18)
    );
    // An element where the binding is needed.
    assert_eq!(
        after_header("select $s = map-region all\nat 0s markers $m $s[0] \"x\"\n"),
        (K::WrongBindingKind, 3, 18)
    );
    // `marker-remove` takes both kinds.
    accepts(
        "select $s = map-region all\nat 0s markers $m $s named\n+0s marker $n 0,0,0 \"x\"\n\
         +0s marker-remove $m\n+0s marker-remove $n\n",
    );
    // An extra argument, an empty label, options.
    assert_eq!(
        after_header("select $s = map-region all\nat 0s markers $m $s named x\n"),
        (K::UnexpectedToken, 3, 27)
    );
    assert_eq!(
        after_header("select $s = map-region all\nat 0s markers $m $s \"\"\n"),
        (K::UnexpectedToken, 3, 21)
    );
    assert_eq!(
        after_header("select $s = map-region all\nat 0s markers $m $s named note-type=0\n"),
        (K::OutOfRange, 3, 27)
    );
    assert_eq!(
        after_header("select $s = map-region all\nat 0s markers $m $s named icon=$s\n"),
        (K::UnexpectedToken, 3, 27)
    );
    accepts(
        "select $s = map-region all\nat 0s markers $m $s name($s[0]) icon=$s[1] \
         quest=\"Q\" note-type=@Camp\n",
    );
}

#[test]
fn marker_bindings_of_both_kinds_share_one_limit() {
    let mut text = String::from("select $s = map-region all\n");
    for i in 0..512 {
        text.push_str(&format!("at 0s marker $m{i} 0,0,0 \"x\"\n"));
    }
    for i in 0..512 {
        text.push_str(&format!("at 0s markers $s{i} $s \"x\"\n"));
    }
    accepts(&text);
    let mut over = text.clone();
    over.push_str("at 0s markers $last $s \"x\"\n");
    assert_eq!(
        after_header(&over),
        (K::LimitExceeded(Limit::Markers), 1027, 15)
    );
    let mut over = text;
    over.push_str("at 0s marker $last 0,0,0 \"x\"\n");
    assert_eq!(
        after_header(&over),
        (K::LimitExceeded(Limit::Markers), 1027, 14)
    );
}

#[test]
fn travel_list_option_forms() {
    // Entry lists: limit, range, shape, repeats.
    let items: Vec<String> = (1..=1025).map(|i| i.to_string()).collect();
    assert_eq!(
        after_header(&format!(
            "at 0s travel-list dest=\"a\" unmet={}\n",
            items.join(",")
        )),
        (K::LimitExceeded(Limit::Entries), 2, 28)
    );
    accepts(&format!(
        "at 0s travel-list dest=\"a\" unmet={}\n",
        items[..1024].join(",")
    ));
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" unusable=1025\n"),
        (K::OutOfRange, 2, 28)
    );
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" unusable=x\n"),
        (K::BadNumber, 2, 28)
    );
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" undiscovered=1,\n"),
        (K::BadNumber, 2, 28)
    );
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" swift=2,2\n"),
        (K::UnexpectedToken, 2, 28)
    );
    // The overlap is reported at the later key whichever comes first.
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" slow=3 swift=1,3\n"),
        (K::UnexpectedToken, 2, 35)
    );
    accepts("at 0s travel-list dest=\"a\" slow=1 swift=2 unmet=1 unusable=1 undiscovered=1\n");
    // Value lists: range, shape.
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" min-level=-1\n"),
        (K::OutOfRange, 2, 28)
    );
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" min-level=x\n"),
        (K::UnexpectedToken, 2, 28)
    );
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" min-level=1.5\n"),
        (K::BadNumber, 2, 28)
    );
    accepts("at 0s travel-list dest=\"a\" min-level=0,10000\n");
    // Areas: refs only. 1,025 refs cannot fit in one 4,096-byte line, so the area limit
    // is reached only by the line limit; a long list within it is taken.
    let refs = vec!["\"v\""; 1025].join(",");
    assert_eq!(
        after_header(&format!("at 0s travel-list dest=\"a\" area={refs}\n")),
        (K::LineTooLong, 2, 1)
    );
    accepts(&format!(
        "at 0s travel-list dest=\"a\" area={}\n",
        vec!["\"v\""; 1000].join(",")
    ));
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" area=\"v\",,\"w\"\n"),
        (K::UnexpectedToken, 2, 28)
    );
    assert_eq!(
        after_header("select $s = area all\nat 0s travel-list dest=\"a\" area=$s\n"),
        (K::UnexpectedToken, 3, 28)
    );
    accepts("select $s = area all\nat 0s travel-list dest=\"a\" area=$s[0],\"v\"\n");
    // A repeated option key.
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" swift=1 swift=2\n"),
        (K::UnexpectedToken, 2, 36)
    );
    // `route` belongs to `travel`, not to `travel-list`.
    assert_eq!(
        after_header("at 0s travel-list dest=\"a\" route=swift\n"),
        (K::UnexpectedToken, 2, 28)
    );
}

#[test]
fn travel_mount_and_instances_option_forms() {
    accepts("at 0s travel \"a\" 1s route=slow\n");
    assert_eq!(
        after_header("at 0s travel \"a\" 1s route=swift route=slow\n"),
        (K::UnexpectedToken, 2, 33)
    );
    assert_eq!(
        after_header("at 0s mount player \"Pony\" kind=war\n"),
        (K::UnexpectedToken, 2, 27)
    );
    accepts("at 0s mount player \"Pony\" kind=basic\n+0s mount player none\n");
    assert_eq!(
        after_header("at 0s instances \"a\" level=0\n"),
        (K::OutOfRange, 2, 21)
    );
    assert_eq!(
        after_header("at 0s instances \"a\" level=10001\n"),
        (K::OutOfRange, 2, 21)
    );
    assert_eq!(
        after_header("at 0s instances \"a\" tier=4294967296\n"),
        (K::OutOfRange, 2, 21)
    );
    assert_eq!(
        after_header("at 0s instances \"a\" size=@\n"),
        (K::UnexpectedToken, 2, 21)
    );
    assert_eq!(
        after_header(&format!(
            "at 0s instances \"a\" size=@{}\n",
            "L".repeat(200)
        )),
        (K::TooLong, 2, 21)
    );
    assert_eq!(
        after_header("at 0s instances \"a\" from=$nobody\n"),
        (K::UndefinedBinding, 2, 21)
    );
    assert_eq!(
        after_header("at 0s instances \"a\" featured=none\n"),
        (K::UnexpectedToken, 2, 21)
    );
    accepts(
        "actor $w kind=npc pos=0,0,0\nat 0s instances \"a\",\"b\" from=$w featured=\"b\" \
         level=1,10000 size=0,4294967295 tier=@T locked=1,2\n",
    );
}

#[test]
fn queue_and_vital_step_forms() {
    assert_eq!(
        after_header("at 0s instance-queue\n"),
        (K::MissingArgument, 2, 1)
    );
    assert_eq!(
        after_header("at 0s instance-queue queued \"a\" \"b\"\n"),
        (K::UnexpectedToken, 2, 33)
    );
    assert_eq!(
        after_header("at 0s instance-queue queued none\n"),
        (K::UnexpectedToken, 2, 29)
    );
    assert_eq!(
        after_header("at 0s instance-queue queued seconds=-1\n"),
        (K::OutOfRange, 2, 29)
    );
    assert_eq!(MAX_QUEUE_SECONDS, 86_400);
    accepts("at 0s instance-queue world-join-queue seconds=86400\n+0s instance-queue none\n");
    assert_eq!(
        after_header("at 0s mount-vital player morale\n"),
        (K::MissingArgument, 2, 1)
    );
    assert_eq!(
        after_header("at 0s mount-vital player morale 1/2000000000\n"),
        (K::OutOfRange, 2, 33)
    );
    assert_eq!(
        after_header("at 0s mount-vital player morale 12\n"),
        (K::BadFraction, 2, 33)
    );
    assert_eq!(
        after_header("at 0s mount-vital $a morale 1/1\n"),
        (K::UndefinedBinding, 2, 19)
    );
    assert_eq!(
        after_header("at 0s mount-vital player morale 1/1 x\n"),
        (K::UnexpectedToken, 2, 37)
    );
}

#[test]
fn leave_queue_pattern_and_input_forms() {
    accepts("on leave-queue times=2\n+0s travel-end\nend\ninput\nat 1s leave-queue\nend\n");
    assert_eq!(
        after_header("on leave-queue now\nend\n"),
        (K::UnexpectedToken, 2, 16)
    );
    assert_eq!(
        after_header("input\nat 1s leave-queue 1\nend\n"),
        (K::UnexpectedToken, 3, 19)
    );
    assert_eq!(
        after_header("input\nat 1s join-instance 1 level=10001\nend\n"),
        (K::OutOfRange, 3, 23)
    );
    assert_eq!(
        after_header("input\nat 1s join-instance 1 size=x\nend\n"),
        (K::BadNumber, 3, 23)
    );
    assert_eq!(
        after_header("input\nat 1s join-instance 1 tier=1,2\nend\n"),
        (K::BadNumber, 3, 23)
    );
    assert_eq!(
        after_header("input\nat 1s join-instance 1 route=slow\nend\n"),
        (K::UnexpectedToken, 3, 23)
    );
    accepts("input\nat 1s join-instance 1 size=@S tier=2 level=10000\nend\n");
    // The `join-instance` pattern keeps its `entry=` filter only.
    assert_eq!(
        after_header("on join-instance size=1\nend\n"),
        (K::UnexpectedToken, 2, 18)
    );
}

#[test]
fn selector_attributes_of_the_revision() {
    assert_eq!(
        after_header("select $s = map-region region=x all\n"),
        (K::BadNumber, 2, 24)
    );
    assert_eq!(
        after_header("select $s = map-region level=1 all\n"),
        (K::UnknownAttribute, 2, 24)
    );
    assert_eq!(
        after_header("select $s = effect war-steed=maybe all\n"),
        (K::UnexpectedToken, 2, 20)
    );
    assert_eq!(
        after_header("select $s = item war-steed=yes all\n"),
        (K::UnknownAttribute, 2, 18)
    );
    accepts("select $s = effect mount-type=@Horse|3 war-steed=yes n=2\n");
    accepts("select $s = map-region kind=node region=2 all\n");
}

#[test]
fn hostile_p5_lines_never_panic() {
    // Arbitrary option values on every new form: typed errors only.
    let values = [
        "",
        "=",
        ",",
        ",,",
        "@",
        "@@",
        "-",
        "0x10",
        "99999999999999999999999999999999999",
        "\"s\"",
        "$x",
        "$x[0]",
        "none",
        "1,@a,\"s\"",
        "1/0",
        "nan",
        "inf",
    ];
    let forms = [
        "at 0s travel-list dest=\"a\" min-level=V",
        "at 0s travel-list dest=\"a\" swift=V slow=V",
        "at 0s travel-list dest=\"a\" area=V",
        "at 0s travel \"a\" 1s route=V",
        "at 0s instances \"a\" level=V size=V tier=V locked=V",
        "at 0s instances \"a\" from=V featured=V",
        "at 0s mount player V kind=V",
        "at 0s instance-queue V V seconds=V",
        "at 0s mount-vital player V V",
        "at 0s marker $m 0,0,0 \"x\" note-type=V",
        "select $s = map-region all\nat 0s markers V V V note-type=V icon=V quest=V",
        "input\nat 1s join-instance V size=V tier=V level=V\nend",
        "select $s = map-region kind=V region=V all",
        "select $s = effect mount-type=V war-steed=V all",
    ];
    for f in forms {
        for v in values {
            let text = format!("{H}{}\n", f.replace('V', v));
            let _ = Scenario::parse(text.as_bytes());
        }
    }
}
