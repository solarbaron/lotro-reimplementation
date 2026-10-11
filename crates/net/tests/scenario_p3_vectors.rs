// SPDX-License-Identifier: GPL-3.0-or-later
//! The P3 revision of the scenario feed (`docs/specs/scenario-feed.md` §18.P3; NPC windows,
//! `docs/specs/npc-windows.md` §10): Vectors N1 and N2 (§14.N1, §14.N2) byte for byte, with
//! and without an install, and the run-time rules of `action(…)`, `rewards`, `buyback` and
//! `buy-back` beyond the vectors.
#![cfg(feature = "scenario")]

use std::sync::Arc;

use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{
    Candidate, CandidateOutcome, CandidateStatus, EmptyResolver, HeadlessDriver, LabelContext,
    ResolvedScenario, Scenario, SelectorQuery, SelectorResolver,
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

/// An install with one NPC action label, `@Vendor` = 2, and the selector `$x` (if any)
/// resolving to records 0x70000001 and 0x70000002.
struct Install {
    vendor_key: Option<u32>,
    contexts: Vec<(LabelContext, String)>,
}

impl Install {
    fn new(vendor_key: Option<u32>) -> Self {
        Self {
            vendor_key,
            contexts: Vec::new(),
        }
    }
}

impl SelectorResolver for Install {
    fn has_install(&self) -> bool {
        true
    }

    fn candidates(
        &mut self,
        _query: &SelectorQuery<'_>,
        _limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        for did in [0x7000_0001, 0x7000_0002] {
            out.push(Candidate {
                did: DataId(did),
                row: 0,
                location: None,
                anchor: None,
            });
        }
        CandidateOutcome::new(CandidateStatus::Ready)
    }

    fn enum_value(&mut self, context: LabelContext, label: &str) -> Option<u32> {
        self.contexts.push((context, label.to_owned()));
        match (context, label) {
            (LabelContext::NpcAction, "@Vendor") => self.vendor_key,
            _ => None,
        }
    }

    fn display_name(&mut self, _did: DataId, _row: Option<u32>) -> Option<String> {
        None
    }
}

const VECTOR_N1: &str = r#"tessera-scenario 1
title "Vector N1"
tick 100ms
player "Testa"
actor $smith kind=npc name="Smith" pos=0,2,0 facing=180
page $greet speaker=$smith "Well met."
option $greet 1 action(@Vendor)
option $greet 2 "Farewell." goto=close
at 0s spawn $smith
on use $smith
+0s show $greet
end
on choose $greet 1
+0s offer "Iron ore" from=$smith
+0s rewards coin:1250 rep:"Smiths":300
end
input
at 0.1s use $smith
at 0.2s choose 1
at 0.3s accept
end
"#;

const LOG_N1: &str = r#"tessera-scenario-log 1
install none
tick 100
title "Vector N1"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$smith kind=non-player name="Smith" at=0.000,2.000,0.000 facing=180.00
0.100 > UseEntity entity=$smith
0.100 DialogueShown window=1 speaker=$smith text="Well met." options=[{option=1 text="Vendor"},{option=2 text="Farewell."}]
0.200 > ChooseOption window=1 option=1
0.200 QuestOffered window=2 quest="Iron ore" giver=$smith
0.200 QuestRewards window=2 entries=[{kind=coin amount=1250 what=none},{kind=reputation amount=300 what="Smiths"}]
0.300 > AnswerQuest window=2 accept=yes
0.300 QuestState quest="Iron ore" state=active
0.300 WindowClosed window=2
0.300 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
"#;

const VECTOR_N2: &str = r#"tessera-scenario 1
title "Vector N2"
tick 100ms
player "Testa"
actor $trader kind=npc name="Trader" pos=3,0,0 facing=270
item $knife "Knife"
at 0s spawn $trader
+0s give $knife
on use $trader
+0s vendor items="Bread","Rope" npc=$trader price=12,250 stock=5
+0s buyback items="Lamp" price=40
end
on buy entry=2
+0s wallet coin 750
end
on sell $knife
+0s remove $knife
+0s wallet coin 1750
end
on buy-back entry=1
+0s wallet coin 1710
end
input
at 0.1s use $trader
at 0.2s buy 2 qty=3
at 0.3s sell $knife
at 0.4s buy-back 1
at 0.5s close vendor
end
"#;

const LOG_N2: &str = r#"tessera-scenario-log 1
install none
tick 100
title "Vector N2"
pre > Connect
pre Connected characters=["Testa"]
pre > EnterWorld character="Testa"
0.000 WorldEntered entity=player name="Testa" at=0.000,0.000,0.000 facing=90.00
0.000 LocationReport entity=player at=0:0.000,0.000,0.000 facing=90.00 anchor=none cause=entered
0.000 EntitySpawned entity=$trader kind=non-player name="Trader" at=3.000,0.000,0.000 facing=270.00
0.000 ItemAdded item=$knife what="Knife" at=backpack:0 quantity=1
0.100 > UseEntity entity=$trader
0.100 VendorShown window=1 vendor=$trader entries=[{entry=1 what="Bread" price=coin:12 stock=5},{entry=2 what="Rope" price=coin:250 stock=5}]
0.100 BuybackShown window=1 entries=[{entry=1 what="Lamp" quantity=1 price=coin:40}]
0.200 > Buy window=1 entry=2 quantity=3
0.200 WalletChanged currency=coin amount=750
0.300 > Sell window=1 item=$knife quantity=1
0.300 ItemRemoved item=$knife
0.300 WalletChanged currency=coin amount=1750
0.400 > BuyBack window=1 entry=1
0.400 WalletChanged currency=coin amount=1710
0.500 > CloseWindow window=1
0.500 WindowClosed window=1
0.500 end reason=complete skipped=0 input-skipped=0 dropped=0 spilled=0
"#;

#[test]
fn vector_n1_action_label_and_rewards() {
    same(&run(VECTOR_N1), LOG_N1);
}

#[test]
fn vector_n1_with_an_install_resolves_the_action_label() {
    let mut install = Install::new(Some(2));
    let log = run_with(VECTOR_N1, &mut install);
    let expected = LOG_N1
        .replace("install none", "install present")
        .replace("{option=1 text=\"Vendor\"}", "{option=1 text=action(2)}");
    same(&log, &expected);
    assert_eq!(
        install.contexts,
        vec![(LabelContext::NpcAction, "@Vendor".to_owned())]
    );
}

#[test]
fn vector_n1_with_an_unknown_action_label_writes_a_label_note() {
    let log = run_with(VECTOR_N1, &mut Install::new(None));
    let expected = LOG_N1.replace(
        "install none\ntick 100\ntitle \"Vector N1\"\n",
        "install present\ntick 100\ntitle \"Vector N1\"\nlabel line=7 attribute=action label=@Vendor\n",
    );
    same(&log, &expected);
}

#[test]
fn vector_n2_vendor_buyback_sell() {
    same(&run(VECTOR_N2), LOG_N2);
}

/// The end line of a log.
fn end_line(log: &str) -> &str {
    log.lines().last().unwrap_or_default()
}

#[test]
fn numeric_action_texts_and_action_in_every_text_position() {
    let log = run(r#"tessera-scenario 1
tick 100ms
player "Testa"
page $p action(4294967295)
option $p 1 action(0)
option $p 16 action(@Trainer_Long_Label_9)
at 0s show $p
+0s marker $m 1,2,3 action(7)
"#);
    assert!(
        log.contains(
            "DialogueShown window=1 speaker=none text=action(4294967295) \
             options=[{option=1 text=action(0)},{option=16 text=\"Trainer_Long_Label_9\"}]"
        ),
        "{log}"
    );
    assert!(log.contains("label=action(7)"), "{log}");
    assert!(end_line(&log).contains("skipped=0"), "{log}");
}

#[test]
fn every_reward_kind_in_written_order() {
    let log = run(r#"tessera-scenario 1
tick 100ms
player "Testa"
at 0s offer "Q"
+0s rewards trait-points:1000000 wallet:"Token":5 coin:0 rep:"Bad":-1000000000000 glory:1 item-xp:2 craft-xp:3 mount-xp:4 virtue-xp:1000000000000000 rep:"Good":1000000000000 wallet:"Token":6
"#);
    let want = "0.000 QuestRewards window=1 entries=[\
        {kind=trait-points amount=1000000 what=none},\
        {kind=wallet amount=5 what=\"Token\"},\
        {kind=coin amount=0 what=none},\
        {kind=reputation amount=-1000000000000 what=\"Bad\"},\
        {kind=glory amount=1 what=none},\
        {kind=item-xp amount=2 what=none},\
        {kind=craft-xp amount=3 what=none},\
        {kind=mount-xp amount=4 what=none},\
        {kind=virtue-xp amount=1000000000000000 what=none},\
        {kind=reputation amount=1000000000000 what=\"Good\"},\
        {kind=wallet amount=6 what=\"Token\"}]";
    assert!(log.contains(want), "{log}");
}

#[test]
fn rewards_and_buyback_skip_without_their_window() {
    let log = run(r#"tessera-scenario 1
tick 100ms
player "Testa"
at 0s rewards coin:5
+0s buyback items="Lamp"
+0s vendor items="Bread"
+0s close vendor
+0s buyback items="Lamp"
+0s offer "Q"
+0s close quest-offer
+0s rewards coin:5
"#);
    assert!(!log.contains("QuestRewards"), "{log}");
    assert!(!log.contains("BuybackShown"), "{log}");
    assert!(end_line(&log).contains("skipped=4"), "{log}");
}

#[test]
fn rewards_follow_the_open_offer_and_missing_elements_skip() {
    let log = run_with(
        r#"tessera-scenario 1
tick 100ms
player "Testa"
select $f = did 0x70000001,0x70000002 all
at 0s offer "Q"
+0s rewards rep:$f[1]:-5 wallet:$f[0]:7
+0s rewards rep:$f[2]:1
+0s offer "R"
+0s rewards coin:9
"#,
        &mut Install::new(None),
    );
    assert!(
        log.contains(
            "0.000 QuestRewards window=1 entries=[{kind=reputation amount=-5 what=0x70000002},\
             {kind=wallet amount=7 what=0x70000001}]"
        ),
        "{log}"
    );
    // A second offer reuses the open window.
    assert!(
        log.contains("0.000 QuestOffered window=1 quest=\"R\""),
        "{log}"
    );
    assert!(
        log.contains("0.000 QuestRewards window=1 entries=[{kind=coin amount=9 what=none}]"),
        "{log}"
    );
    assert!(end_line(&log).contains("skipped=1"), "{log}");
}

#[test]
fn buyback_entries_prices_barter_and_bindings() {
    let log = run_with(
        r#"tessera-scenario 1
tick 100ms
player "Testa"
select $x = did 0x70000001,0x70000002 all
at 0s vendor items="Bread"
+0s buyback items=$x qty=3 price=5 barter="Token":2
+0s buyback items="A","B","C" price=1,2
+0s buyback items=$x[0],"Z"
"#,
        &mut Install::new(None),
    );
    for want in [
        "0.000 BuybackShown window=1 entries=[{entry=1 what=0x70000001 quantity=3 price=coin:5+\"Token\"*2},\
         {entry=2 what=0x70000002 quantity=3 price=coin:5+\"Token\"*2}]",
        "0.000 BuybackShown window=1 entries=[{entry=1 what=\"A\" quantity=1 price=coin:1},\
         {entry=2 what=\"B\" quantity=1 price=coin:2},{entry=3 what=\"C\" quantity=1 price=coin:2}]",
        "0.000 BuybackShown window=1 entries=[{entry=1 what=0x70000001 quantity=1 price=coin:0},\
         {entry=2 what=\"Z\" quantity=1 price=coin:0}]",
    ] {
        assert!(log.contains(want), "{want}\n--- log ---\n{log}");
    }
}

#[test]
fn buy_back_patterns_check_window_and_entry() {
    let log = run(r#"tessera-scenario 1
tick 100ms
player "Testa"
at 0s vendor items="Bread"
on buy-back entry=2
+0s system "two"
end
on buy-back
+0s system "any"
end
input
at 0.1s buy-back 1
at 0.2s buy-back 2
at 0.3s buy-back 3
end
"#);
    // No buy-back page yet: no entry is known, nothing fires.
    assert!(!log.contains("ChatMessage"), "{log}");
    assert!(log.contains("0.100 > BuyBack window=1 entry=1"), "{log}");
    assert!(log.contains("0.300 > BuyBack window=1 entry=3"), "{log}");

    let log = run(r#"tessera-scenario 1
tick 100ms
player "Testa"
at 0s vendor items="Bread"
+0s buyback items="A","B"
on buy-back entry=2 times=1
+0s system "two"
end
on buy-back
+0s system "any"
end
input
at 0.1s buy-back 1
at 0.2s buy-back 2
at 0.3s buy-back 2
at 0.4s buy-back 3
at 0.5s close vendor
at 0.6s buy-back 1
end
"#);
    for want in [
        "0.100 ChatMessage channel=system sender=none text=\"any\"",
        "0.200 ChatMessage channel=system sender=none text=\"two\"",
        "0.300 ChatMessage channel=system sender=none text=\"any\"",
    ] {
        assert!(log.contains(want), "{want}\n--- log ---\n{log}");
    }
    assert!(!log.contains("0.400 ChatMessage"), "{log}");
    // After the window closed the input line has no window: it is skipped.
    assert!(!log.contains("0.600 >"), "{log}");
    assert!(end_line(&log).contains("input-skipped=1"), "{log}");
}

#[test]
fn a_new_vendor_window_forgets_the_old_buyback_page() {
    let log = run(r#"tessera-scenario 1
tick 100ms
player "Testa"
at 0s vendor items="Bread"
+0s buyback items="A"
+0s close vendor
+0s vendor items="Bread"
on buy-back
+0s system "any"
end
input
at 0.1s buy-back 1
end
"#);
    assert!(log.contains("0.100 > BuyBack window=2 entry=1"), "{log}");
    assert!(!log.contains("ChatMessage"), "{log}");
}

/// §13 rule 2: `action(N)` is a numeric NPC-action enum key, so the committed-content check
/// reports it in every text position; `action(@Label)` is a label and is listed by `labels`.
#[test]
fn numeric_action_keys_are_numeric_enum_values() {
    let parsed = |body: &str| {
        Scenario::parse(format!("tessera-scenario 1\n{body}").as_bytes()).expect("parse")
    };
    for body in [
        "page $p action(1)\n",
        "page $p \"x\"\noption $p 1 action(2)\n",
        "page $p \"x\"\noption $p 1 action(4294967295)\n",
        "player \"T\"\nat 0s marker $m 1,2,3 action(7)\n",
    ] {
        assert!(parsed(body).numeric_enum_values(), "{body}");
    }
    let sc = parsed("page $p \"x\"\noption $p 1 action(@Vendor)\n");
    assert!(!sc.numeric_enum_values());
    assert_eq!(sc.labels(), vec![(3, "action", "@Vendor")]);
}

/// `vendor` and `buyback` share their `price=` and `barter=` rules (§6.3): coin up to
/// 10^15, a barter cost `REF:N`.
#[test]
fn vendor_and_buyback_share_price_rules() {
    let parse = |step: &str| {
        Scenario::parse(format!("tessera-scenario 1\nplayer \"T\"\nat 0s {step}\n").as_bytes())
    };
    for verb in ["vendor", "buyback"] {
        assert!(
            parse(&format!("{verb} items=\"A\" price=1000000000000000")).is_ok(),
            "{verb}"
        );
        assert!(
            parse(&format!("{verb} items=\"A\" price=1000000000000001")).is_err(),
            "{verb}"
        );
        assert!(
            parse(&format!("{verb} items=\"A\" barter=\"T\":2")).is_ok(),
            "{verb}"
        );
        assert!(
            parse(&format!("{verb} items=\"A\" barter=\"T\"")).is_err(),
            "{verb}"
        );
        assert!(
            parse(&format!("{verb} items=\"A\" barter=\"T\":0")).is_err(),
            "{verb}"
        );
    }
}
