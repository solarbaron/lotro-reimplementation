// SPDX-License-Identifier: GPL-3.0-or-later
//! The hostile-input table of `docs/specs/scenario-feed.md` §14.5, plus further static
//! errors: every file is refused with the expected kind and position, without a panic.
#![cfg(feature = "scenario")]

use tessera_net::scenario::{Limit, Scenario, ScenarioErrorKind as K};

const H: &str = "tessera-scenario 1\n";

fn err(bytes: &[u8]) -> (K, u32, u32) {
    match Scenario::parse(bytes) {
        Ok(_) => panic!("accepted: {:?}", String::from_utf8_lossy(bytes)),
        Err(e) => (e.kind, e.line, e.column),
    }
}

fn after_header(rest: &str) -> (K, u32, u32) {
    err(format!("{H}{rest}").as_bytes())
}

#[test]
fn table_of_section_14_5() {
    assert_eq!(err(b""), (K::BadHeader, 1, 1));
    assert_eq!(err(b"tessera-scenario 2\n"), (K::UnsupportedVersion, 1, 18));
    assert_eq!(err(&vec![b'a'; 1_048_577]), (K::FileTooLarge, 1, 1));
    let mut enc = H.as_bytes().to_vec();
    enc.extend_from_slice(b"title \"a\xFF\"\n");
    assert_eq!(err(&enc), (K::Encoding, 2, 9));
    assert_eq!(
        after_header("title \"a\u{0}\"\n"),
        (K::ControlCharacter, 2, 9)
    );
    assert_eq!(
        after_header(&format!("{}\n", "#".repeat(5000))),
        (K::LineTooLong, 2, 1)
    );
    assert_eq!(after_header("title \"abc\n"), (K::UnterminatedString, 2, 7));
    assert_eq!(after_header("title \"\\q\"\n"), (K::BadEscape, 2, 8));
    assert_eq!(after_header("title \"\\u{7}\"\n"), (K::BadEscape, 2, 8));
    assert_eq!(after_header("tick 5ms\n"), (K::OutOfRange, 2, 6));
    assert_eq!(after_header("at 2h system \"x\"\n"), (K::BadDuration, 2, 4));
    assert_eq!(
        after_header("at 61min system \"x\"\n"),
        (K::OutOfRange, 2, 4)
    );
    assert_eq!(
        after_header("at 0.0005s system \"x\"\n"),
        (K::BadDuration, 2, 4)
    );
    assert_eq!(
        after_header("at 1s spawn $ghost\n"),
        (K::UndefinedBinding, 2, 13)
    );
    assert_eq!(
        after_header("select $a = item n=70000\n"),
        (K::LimitExceeded(Limit::Selector), 2, 18)
    );
    assert_eq!(
        after_header("select $a = item kind=weapon\n"),
        (K::MissingCount, 2, 1)
    );
    assert_eq!(
        after_header("select $a = item colour=@Red all\n"),
        (K::UnknownAttribute, 2, 18)
    );
    assert_eq!(
        after_header("actor $a kind=npc pos=1,2\n"),
        (K::BadPosition, 2, 19)
    );
    assert_eq!(
        after_header("actor $a kind=npc pos=0,0,0\nactor $a kind=npc pos=0,0,0\n"),
        (K::DuplicateBinding, 3, 7)
    );
    assert_eq!(
        after_header("actor $a kind=npc pos=0,0,0\nat 0s spawn $a[0]\n"),
        (K::WrongBindingKind, 3, 13)
    );
    assert_eq!(after_header("on use player\n"), (K::UnclosedBlock, 2, 1));
    assert_eq!(after_header("end\n"), (K::UnexpectedEnd, 2, 1));
    assert_eq!(
        after_header("on cancel-cast\n+0s repeat 2 every 1s\n+0s repeat 2 every 1s\n"),
        (K::NestingTooDeep, 4, 5)
    );
    assert_eq!(
        after_header("at 0s repeat 20000 every 1ms\n"),
        (K::LimitExceeded(Limit::Repeat), 2, 14)
    );
    assert_eq!(
        after_header("on cancel-cast\nat 1s system \"x\"\n"),
        (K::UnexpectedToken, 3, 1)
    );
    assert_eq!(
        after_header("at 0s system \"x\"\ntick 100ms\n"),
        (K::SettingAfterStep, 3, 1)
    );
    assert_eq!(
        after_header("input\non cancel-cast\n"),
        (K::NotAllowedHere, 3, 1)
    );
    let actors: String = (0..1025)
        .map(|i| format!("actor $a{i} kind=npc pos=0,0,0\n"))
        .collect();
    assert_eq!(
        after_header(&actors),
        (K::LimitExceeded(Limit::Actors), 1026, 1)
    );
    let vector_d_moved = r#"title "Vector D"
tick 100ms
player "Testa"
actor $elder kind=npc name="Elder" pos=-2,0,0 facing=90
page $hello speaker=$elder "Good day. Will you help?"
option $hello 1 "Gladly." goto=$thanks
option $hello 2 "Not now." goto=close
page $thanks speaker=$elder "Thank you."
"#;
    assert_eq!(after_header(vector_d_moved), (K::UndefinedBinding, 7, 27));
    assert_eq!(after_header("title \"a\"b\n"), (K::UnexpectedToken, 2, 7));
    assert_eq!(
        after_header("select $a = item all seed=1\n"),
        (K::UnexpectedToken, 2, 22)
    );
    assert_eq!(
        after_header("player \"Testa\"\nenter \"Nobody\"\n"),
        (K::UnknownCharacter, 3, 7)
    );
    assert_eq!(
        after_header("player \"A\"\nplayer \"B\"\n"),
        (K::DuplicateBinding, 3, 1)
    );
    assert_eq!(
        after_header("player \"A\"\ncharacter \"A\"\n"),
        (K::DuplicateBinding, 3, 11)
    );
    assert_eq!(
        after_header("player \"A\"\nenter \"B\"\non cancel-cast\n"),
        (K::UnclosedBlock, 4, 1)
    );
    assert_eq!(
        after_header("at 0s repeat 2 every 1s\n+0s repeat 2 every 1s\n"),
        (K::NotAllowedHere, 3, 5)
    );
    assert_eq!(
        after_header("on cancel-cast\non cancel-cast\n"),
        (K::NotAllowedHere, 3, 1)
    );
    assert_eq!(
        after_header("input\nend\ninput\n"),
        (K::NotAllowedHere, 4, 1)
    );
    assert_eq!(
        after_header("on create-character\n+0s system \"x\"\n"),
        (K::NotAllowedHere, 3, 5)
    );
    let long = format!("actor $a kind=npc pos=0,0,1{}.5\n", "0".repeat(30));
    assert_eq!(after_header(&long), (K::BadNumber, 2, 19));
    assert!(
        Scenario::parse(
            format!(
                "{H}actor $a kind=npc pos=-99999.999999999,-99999.999999999,-99999.999999999\n"
            )
            .as_bytes()
        )
        .is_ok()
    );
    let refs = vec!["\"a\""; 33].join(",");
    assert_eq!(
        after_header(&format!("at 0s trade-offer own items={refs}\n")),
        (K::LimitExceeded(Limit::Entries), 2, 23)
    );
    assert_eq!(
        after_header("input\nat 1s choose 17\nend\n"),
        (K::OutOfRange, 3, 14)
    );
    assert_eq!(
        after_header("input\nat 1s craft \"Stew\" qty=70000\nend\n"),
        (K::OutOfRange, 3, 20)
    );
    assert_eq!(
        after_header("end-at 0s\ninput\nat 0s cancel-cast\nend\n"),
        (K::OutOfRange, 4, 4)
    );
}

#[test]
fn more_static_errors() {
    assert_eq!(err(b"\n# only a comment\n"), (K::BadHeader, 1, 1));
    assert_eq!(err(b"hello 1\n"), (K::BadHeader, 1, 1));
    assert_eq!(err(b"tessera-scenario 1 extra\n"), (K::BadHeader, 1, 20));
    assert_eq!(
        err("\u{FEFF}tessera-scenario 1\n".as_bytes()),
        (K::Encoding, 1, 1)
    );
    assert_eq!(after_header("frobnicate\n"), (K::UnknownKeyword, 2, 1));
    assert_eq!(
        after_header("tick 50ms\ntick 50ms\n"),
        (K::DuplicateSetting, 3, 1)
    );
    assert_eq!(
        after_header("select $a = gizmo all\n"),
        (K::UnknownFamily, 2, 13)
    );
    assert_eq!(after_header("at 0s spawn\n"), (K::MissingArgument, 2, 1));
    assert_eq!(after_header("at 0s blink\n"), (K::UnknownKeyword, 2, 7));
    assert_eq!(
        after_header(&format!("title \"{}\"\n", "x".repeat(129))),
        (K::TooLong, 2, 7)
    );
    assert_eq!(
        after_header(&format!("player \"{}\"\n", "x".repeat(65))),
        (K::TooLong, 2, 8)
    );
    let toks = vec!["x"; 65].join(" ");
    assert_eq!(
        after_header(&format!("{toks}\n")),
        (K::LimitExceeded(Limit::Tokens), 2, 129)
    );
    assert_eq!(
        after_header("at 0s level player 0\n"),
        (K::OutOfRange, 2, 20)
    );
    assert_eq!(
        after_header("at 0s vital player health 5/4\n"),
        (K::OutOfRange, 2, 27)
    );
    assert_eq!(
        after_header("at 0s vital player health 5\n"),
        (K::BadFraction, 2, 27)
    );
    assert_eq!(
        after_header("at 0s cast player \"x\" 1s kind=spell\n"),
        (K::UnexpectedToken, 2, 26)
    );
    assert_eq!(
        after_header("select $d = did 0x1,0x2 all\nselect $d2 = did 0x123456789 all\n"),
        (K::OutOfRange, 3, 18)
    );
    let dids = (0..257)
        .map(|i| format!("0x{i:X}"))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        after_header(&format!("select $d = did {dids} all\n")),
        (K::LimitExceeded(Limit::Did), 2, 17)
    );
    assert_eq!(
        after_header("on use player times=0\nend\n"),
        (K::OutOfRange, 2, 15)
    );
    assert_eq!(
        after_header("select $q = quest all\nselect $r = recipe profession=$q all\n"),
        (K::WrongBindingKind, 3, 20)
    );
    assert_eq!(
        after_header("page $p \"x\"\noption $p 1 \"a\"\noption $p 1 \"b\"\n"),
        (K::UnexpectedToken, 4, 11)
    );
    assert_eq!(
        after_header("on create-character\n+1s accept-character \"A\"\nend\n"),
        (K::OutOfRange, 3, 1)
    );
    let pre: String = (0..17).map(|_| "+0s accept-character \"A\"\n").collect();
    assert_eq!(
        after_header(&format!("on create-character\n{pre}end\n")),
        (K::LimitExceeded(Limit::Steps), 19, 1)
    );
    assert_eq!(
        after_header("at 0s accept-character \"A\"\n"),
        (K::NotAllowedHere, 2, 7)
    );
    assert_eq!(
        after_header("input\nplayer \"A\"\nend\n"),
        (K::NotAllowedHere, 3, 1)
    );
    assert_eq!(
        after_header("input\n+1s cancel-cast\nend\n"),
        (K::UnexpectedToken, 3, 1)
    );
    assert_eq!(
        after_header("on use player\nend extra\n"),
        (K::UnexpectedToken, 3, 5)
    );
    assert_eq!(
        after_header("at 0s say player \"hi\" channel=tell:\"X\"\n"),
        (K::UnexpectedToken, 2, 23)
    );
    // A repeat whose iterations pass the horizon.
    assert_eq!(
        after_header("at 59min repeat 3 every 1min\n+0s system \"x\"\nend\n"),
        (K::OutOfRange, 2, 25)
    );
    // The expansion budget: 2 × 10,000 iterations × 10 lines = 200,000 is fine, one more
    // line is not.
    let body: String = (0..10).map(|_| "+0s system \"x\"\n").collect();
    let ok = format!(
        "at 0s repeat 10000 every 1ms\n{body}end\nat 0s repeat 10000 every 1ms\n{body}end\n"
    );
    assert!(Scenario::parse(format!("{H}{ok}").as_bytes()).is_ok());
    let over = format!("{ok}at 0s repeat 1 every 1ms\n+0s system \"x\"\nend\n");
    assert_eq!(
        after_header(&over),
        (K::LimitExceeded(Limit::Repeat), 27, 1)
    );
}

#[test]
fn errors_display_with_file() {
    let e = Scenario::parse(format!("{H}tick 5ms\n").as_bytes()).unwrap_err();
    assert_eq!(
        e.with_file("x.scenario").to_string(),
        "x.scenario:2:6: OutOfRange"
    );
}

/// The overlay check of §12.5.1 on a file that parses.
fn overlay_err(rest: &str) -> (K, u32, u32) {
    let s = Scenario::parse(format!("{H}{rest}").as_bytes()).expect("parses");
    match s.check_overlay() {
        Ok(()) => panic!("accepted in an overlay run: {rest:?}"),
        Err(e) => (e.kind, e.line, e.column),
    }
}

/// The 15 rows revision 4 added to the table of §14.5.
#[test]
fn table_of_section_14_5_revision_4() {
    assert_eq!(
        after_header("at 0s stat player pid:0x7FFF0001:int=1.5\n"),
        (K::BadValue, 2, 19)
    );
    assert_eq!(
        after_header("at 0s stat player pid:0x7FFF0001:int=2147483648\n"),
        (K::OutOfRange, 2, 19)
    );
    assert_eq!(
        after_header("at 0s stat player pid:0:int=1\n"),
        (K::OutOfRange, 2, 19)
    );
    assert_eq!(
        after_header("at 0s stat player pid:0x7FFF0001:text=1\n"),
        (K::UnexpectedToken, 2, 19)
    );
    assert_eq!(
        after_header("at 0s stat player pid:0x7FFF0001:colour=rgba:256,0,0,0\n"),
        (K::OutOfRange, 2, 19)
    );
    assert_eq!(
        after_header("look $l pid:0x7FFF0001:int=none\n"),
        (K::BadValue, 2, 9)
    );
    assert_eq!(
        after_header("look $l pid:0x7FFF0001:string=\"a b\"\n"),
        (K::BadValue, 2, 9)
    );
    assert_eq!(
        after_header("look $l pid:0x7FFF0001:instance=1\n"),
        (K::BadValue, 2, 9)
    );
    let many: Vec<&str> = std::iter::repeat_n("pid:0x7FFF0001:int=1", 49).collect();
    assert_eq!(
        after_header(&format!("look $l {}\n", many.join(" "))),
        (K::LimitExceeded(Limit::Look), 2, 1017)
    );
    assert_eq!(
        after_header("at 0s remove-character \"A\"\n"),
        (K::NotAllowedHere, 2, 7)
    );
    assert_eq!(
        after_header("on delete-character\n+0s accept-character \"A\"\nend\n"),
        (K::NotAllowedHere, 3, 5)
    );
    assert_eq!(
        after_header("on create-character\n+0s stat player @Example_Stat=1\nend\n"),
        (K::NotAllowedHere, 3, 5)
    );
    assert_eq!(
        overlay_err("actor $a kind=npc pos=0,0,0\n"),
        (K::NotAllowedInOverlay, 2, 1)
    );
    assert_eq!(
        overlay_err("on chat\nend\n"),
        (K::NotAllowedInOverlay, 2, 1)
    );
    assert_eq!(
        overlay_err("item $i \"Ring\"\ninput\nat pre destroy $i\nend\n"),
        (K::NotAllowedInOverlay, 4, 1)
    );
}

#[test]
fn property_values_and_looks_more_static_checks() {
    // Kinds and forms with `pid:`.
    for ok in [
        "at 0s stat player pid:0x7FFF0001:bool=yes\n",
        "at 0s stat player pid:5:int64=-9223372036854775808\n",
        "at 0s stat player pid:5:float=-1000000000\n",
        "at 0s stat player pid:5:enum=4294967295\n",
        "at 0s stat player pid:5:bitfield64=0xFFFFFFFFFFFFFFFF\n",
        "at 0s stat player pid:5:did=0x70000001\n",
        "at 0s stat player pid:5:instance=18446744073709551615\n",
        "at 0s stat player pid:5:colour=rgba:0,0,0,255\n",
        "at 0s stat player pid:5:string=\"a b = c\"\n",
        "at 0s stat player pid:5:int=none\n",
        "look $l pid:5:string=\"\" pid:6:bool=no\n",
    ] {
        assert!(
            Scenario::parse(format!("{H}{ok}").as_bytes()).is_ok(),
            "{ok}"
        );
    }
    for (bad, kind) in [
        (
            "at 0s stat player pid:5:float=1000000000.5\n",
            K::OutOfRange,
        ),
        ("at 0s stat player pid:5:did=12\n", K::BadValue),
        ("at 0s stat player pid:5:bool=1\n", K::BadValue),
        ("at 0s stat player pid:5:enum=-1\n", K::OutOfRange),
        ("at 0s stat player pid:4294967296:int=1\n", K::OutOfRange),
        ("at 0s stat player pid:5:colour=rgba:1,2,3\n", K::BadNumber),
        ("at 0s stat player 5=1\n", K::UnexpectedToken),
    ] {
        assert_eq!(after_header(bad).0, kind, "{bad}");
    }
    // Look bindings: 64 at most, and `look=` names a look.
    let looks: String = (0..65)
        .map(|i| format!("look $l{i} pid:5:int=1\n"))
        .collect();
    assert_eq!(
        after_header(&looks),
        (K::LimitExceeded(Limit::Looks), 66, 1)
    );
    assert_eq!(
        after_header("item $i \"x\"\nplayer \"A\" look=$i\n"),
        (K::WrongBindingKind, 3, 12)
    );
    // `look=asked` only on `accept-character`.
    assert_eq!(
        after_header("player \"A\" look=asked\n"),
        (K::UnexpectedToken, 2, 12)
    );
    // `accept-character … look=asked` is fine in an `on create-character` body.
    assert!(
        Scenario::parse(
            format!("{H}on create-character\n+0s accept-character \"A\" look=asked\nend\n")
                .as_bytes()
        )
        .is_ok()
    );
    // A well-formed overlay file passes.
    let ok = "tick 100ms\nselect $g = did 0x70001234 all\nitem $s $g[0]\nat 0s give $s\n+0s equip $s slot=17\n+0s stat player pid:5:int=1\non equip\n+0s wallet coin 5\nend\ninput\nat 1s unequip $s\nend\n";
    let s = Scenario::parse(format!("{H}{ok}").as_bytes()).unwrap();
    assert!(s.check_overlay().is_ok());
    assert!(s.uses_pid_forms());
    // `level` with another entity is refused at the entity token.
    assert_eq!(overlay_err("actor $a kind=npc pos=0,0,0\n").1, 2);
    let s = Scenario::parse(
        format!("{H}select $g = did 0x1 all\nitem $s $g[0]\nat 0s stat player pid:5:int=1\n")
            .as_bytes(),
    )
    .unwrap();
    assert!(s.check_overlay().is_ok());
}
