// SPDX-License-Identifier: GPL-3.0-or-later
//! The social panels of `docs/specs/social-panels.md` (P4) on synthetic data: the worked
//! examples §12.1–§12.3, the Tessera rules TR2–TR10 and the invariants SPI6–SPI8, on the
//! view models and on synthetic element trees. Names are invented; element IDs, states and
//! properties are the placeholders of `SocialVocab::default()`.

mod ui_common;

use tessera_formats::ui_layout::ElementDesc;
use tessera_ui::events::{Command, Message, MsgArg};
use tessera_ui::game::CharVocab;
use tessera_ui::game::chat::notice::{
    Arg, NoTables, Notice, TextRef, TextSource, args as A, tables as T,
};
use tessera_ui::game::npc::CoinCounts;
use tessera_ui::game::social::fellowship::{Fellowship, FellowshipPanels, Member, VitalKind};
use tessera_ui::game::social::friends::{
    FriendColumn, FriendEntry, FriendsModel, FriendsPage, GroupState,
};
use tessera_ui::game::social::ignore::{IgnoreColumn, IgnoreModel};
use tessera_ui::game::social::invite::{Invitation, InviteWindow};
use tessera_ui::game::social::panel::{SocialPage, SocialPanel, SocialPanelState, Toggle};
use tessera_ui::game::social::trade::{
    CoinField, CoinValues, Side, Trade, TradeEntry, TradeWindow, coin_counts, name_state,
};
use tessera_ui::game::social::{DialogPurpose, SocialAction, SocialIntent, SocialVocab};
use tessera_ui::{ElementKey, Ui, UiEvent};
use ui_common::*;

fn sv() -> SocialVocab {
    SocialVocab::default()
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_string()).collect()
}

fn ft(token: u32) -> (u32, u32) {
    (T::FELLOWSHIP, token)
}

fn tok(n: &Notice) -> (u32, u32) {
    let r = n.text_ref().unwrap();
    (r.table, r.token)
}

fn name_arg(n: &Notice) -> String {
    match n.text_ref().and_then(|r| r.arg(A::NAME)) {
        Some(Arg::Text(t)) => t.clone(),
        other => panic!("{other:?}"),
    }
}

fn values() -> CoinValues {
    // §12.2: 100,000 / 100 / 1 (EC §7, read from the install by the client).
    CoinValues {
        gold: 100_000,
        silver: 100,
        copper: 1,
    }
}

// ----- §12.3 friends sort, TR5 ------------------------------------------------------------

fn friends(v: &[(&str, bool)]) -> Vec<FriendEntry> {
    v.iter()
        .map(|(n, o)| FriendEntry {
            name: (*n).into(),
            online: *o,
        })
        .collect()
}

fn order(m: &FriendsModel) -> Vec<String> {
    m.sorted().into_iter().map(|e| e.name.clone()).collect()
}

#[test]
fn example_12_3_friends_sort() {
    let mut m = FriendsModel::default();
    m.set_list(&friends(&[("bo", false), ("Al", true), ("cy", true)]));
    assert_eq!(m.sort(), (FriendColumn::Member, false));
    assert_eq!(order(&m), s(&["Al", "bo", "cy"]));
    m.click_header(FriendColumn::LastOn);
    assert_eq!(order(&m), s(&["bo", "Al", "cy"]));
    m.click_header(FriendColumn::LastOn);
    assert_eq!(order(&m), s(&["cy", "Al", "bo"]));
    m.click_header(FriendColumn::Member);
    assert_eq!(
        m.sort(),
        (FriendColumn::Member, false),
        "a new column is ascending"
    );
    // Grouped: not grouped before grouped.
    m.set_fellows(&s(&["CY"]));
    m.click_header(FriendColumn::Grouped);
    assert_eq!(order(&m), s(&["Al", "bo", "cy"]));
    m.set_fellows(&s(&["al"]));
    assert_eq!(order(&m), s(&["bo", "cy", "Al"]));
}

#[test]
fn friends_rows_buttons_and_menu_tr5() {
    let mut m = FriendsModel::default();
    m.set_list(&friends(&[("bo", false), ("Al", true)]));
    m.set_fellows(&s(&["Al"]));
    let rows = m.rows();
    assert_eq!(rows[0].name, "Al");
    assert!(rows[0].grouped && !rows[1].grouped);
    assert_eq!(
        (rows[0].level.table, rows[0].level.token),
        (T::UI_FRIENDS, 0x0A82_90DF)
    );
    assert_eq!(rows[0].last_on.token, 0x0FE8_45CC);
    assert_eq!(rows[1].last_on.token, 0x0A82_90DF, "offline: unknown");
    // No selection: only Add.
    let b = m.buttons();
    assert!(b.add && !b.remove && !b.note && !b.tell && !b.invite);
    assert!(m.remove().is_none() && m.tell().is_none());
    // Al online and grouped: tell yes, invite no.
    m.select(Some("Al"));
    let b = m.buttons();
    assert!(b.remove && b.tell && !b.invite && !b.note);
    assert_eq!(
        m.tell(),
        Some(SocialIntent::StartTell { name: "Al".into() })
    );
    // bo offline: no tell, no invite.
    m.select(Some("bo"));
    let b = m.buttons();
    assert!(b.remove && !b.tell && !b.invite);
    assert_eq!(
        m.remove(),
        Some(SocialIntent::RemoveFriend { name: "bo".into() })
    );
    // The selection is kept by name across a replacement.
    m.set_list(&friends(&[("bo", true), ("Zed", false)]));
    assert_eq!(m.selected(), Some("bo"));
    assert_eq!(
        m.invite(),
        Some(SocialIntent::InviteToFellowship { name: "bo".into() })
    );
    m.set_list(&friends(&[("Zed", false)]));
    assert_eq!(m.selected(), None);
    // The add dialog and its answer.
    let d = FriendsModel::add_dialog();
    assert_eq!(d.purpose, DialogPurpose::AddFriend);
    assert_eq!(d.question.token, 0x0BC2_5DB4);
    assert_eq!(
        FriendsModel::answer_add("  Kim "),
        Some(SocialIntent::AddFriend { name: "Kim".into() })
    );
    assert_eq!(FriendsModel::answer_add("   "), None);
    // The row menu.
    let menu = m.menu("Zed", GroupState::default()).unwrap();
    // §3.5, SP33: the heading's argument is the note question's 0xA2D94C5.
    let heading = menu.heading.clone().unwrap();
    assert_eq!((heading.table, heading.token), (T::UI_FRIENDS, 0x0193_53F5));
    assert!(matches!(heading.arg(A::NOTE_NAME), Some(Arg::Text(t)) if t == "Zed"));
    assert!(heading.arg(A::NAME).is_none());
    assert_eq!(heading.render(&mut NoTables), "Zed");
    assert_eq!(menu.entries.len(), 6);
    assert!(!menu.entries[0].enabled, "tell: offline");
    assert!(!menu.entries[1].enabled, "invite: offline");
    assert_eq!(
        menu.entries[3].intent,
        Some(SocialIntent::Ignore { name: "Zed".into() })
    );
    assert!(!menu.entries[4].enabled && !menu.entries[5].enabled, "TR5");
    m.set_list(&friends(&[("Zed", true)]));
    let full = GroupState {
        in_group: true,
        leads: true,
        full: true,
    };
    assert!(!m.menu("Zed", full).unwrap().entries[1].enabled);
    let member = GroupState {
        in_group: true,
        leads: false,
        full: false,
    };
    assert!(!m.menu("Zed", member).unwrap().entries[1].enabled);
    assert!(m.menu("Zed", GroupState::default()).unwrap().entries[1].enabled);
}

#[test]
fn ignore_model_tr6() {
    let mut m = IgnoreModel::default();
    m.set_list(&s(&["cy", "Al", "bo"]));
    assert_eq!(m.sort(), (IgnoreColumn::Member, false));
    assert_eq!(m.sorted(), s(&["Al", "bo", "cy"]));
    m.click_header(IgnoreColumn::Member);
    assert_eq!(m.sorted(), s(&["cy", "bo", "Al"]));
    assert_eq!(m.buttons(), (false, false));
    m.select(Some("bo"));
    assert_eq!(m.buttons(), (true, false));
    assert_eq!(
        m.remove(),
        Some(SocialIntent::Unignore { name: "bo".into() })
    );
    let menu = m.menu("bo").unwrap();
    assert_eq!(menu.heading.unwrap().token, 0x0B60_7755);
    assert!(menu.entries[0].enabled && !menu.entries[1].enabled);
    m.set_list(&s(&["BO"]));
    assert_eq!(m.selected(), Some("BO"), "kept by name-equality");
    assert_eq!(
        IgnoreModel::answer_add("Kim"),
        Some(SocialIntent::Ignore { name: "Kim".into() })
    );
}

// ----- §12.1 fellowship, TR2–TR4, SPI6, SPI7 ----------------------------------------------

const P: u64 = 1;
const A_: u64 = 2;
const B_: u64 = 3;

fn mem(name: &str, entity: Option<u64>, leader: bool) -> Member {
    Member {
        name: name.into(),
        entity,
        leader,
    }
}

fn row_names(f: &Fellowship) -> Vec<String> {
    f.rows().into_iter().map(|r| r.name).collect()
}

#[test]
fn example_12_1_formation_leader_removal_disband() {
    let mut f = Fellowship::new("Tam", Some(P));
    // Formation, the player leading.
    let l = f.changed(&[mem("Tam", Some(P), true), mem("Brin", Some(A_), false)]);
    assert_eq!(row_names(&f), s(&["Brin"]));
    assert!(f.player_leads(), "PlayerLeaderIcon");
    assert_eq!(l.len(), 1);
    assert_eq!((tok(&l[0]), l[0].mask), (ft(0x0245_83E7), 0x400));
    // Cael joins.
    let l = f.changed(&[
        mem("Tam", Some(P), true),
        mem("Brin", Some(A_), false),
        mem("Cael", Some(B_), false),
    ]);
    assert_eq!(row_names(&f), s(&["Brin", "Cael"]));
    assert_eq!(l.len(), 1);
    assert_eq!(tok(&l[0]), ft(0x047E_6917));
    assert_eq!(name_arg(&l[0]), "Cael");
    // Leader Brin.
    let l = f.changed(&[
        mem("Tam", Some(P), false),
        mem("Brin", Some(A_), true),
        mem("Cael", Some(B_), false),
    ]);
    assert!(!f.player_leads());
    assert!(f.rows()[0].leader && !f.rows()[1].leader);
    assert_eq!(l.len(), 1);
    assert_eq!(tok(&l[0]), ft(0x03FD_5682));
    assert_eq!(name_arg(&l[0]), "Brin");
    // The player kicks Cael (as sent), then the list without Cael.
    f.note_sent(&SocialIntent::KickFromFellowship {
        name: "Cael".into(),
    });
    let l = f.changed(&[mem("Tam", Some(P), false), mem("Brin", Some(A_), true)]);
    assert_eq!(row_names(&f), s(&["Brin"]));
    assert_eq!(l.len(), 1);
    assert_eq!(tok(&l[0]), ft(0x0928_4BE7));
    assert_eq!(name_arg(&l[0]), "Cael");
    // Disband.
    let l = f.changed(&[]);
    assert!(!f.exists() && f.rows().is_empty());
    assert_eq!(l.len(), 1);
    assert_eq!(tok(&l[0]), ft(0x018C_CD74));
}

#[test]
fn derived_lines_other_cases_and_spi7() {
    let mut f = Fellowship::new("Tam", Some(P));
    // Joining someone else's fellowship.
    let l = f.changed(&[mem("Brin", Some(A_), true), mem("Tam", Some(P), false)]);
    assert_eq!(tok(&l[0]), ft(0x0CDF_FC77));
    // SPI7: the same names, leader and order derive nothing.
    assert!(
        f.changed(&[mem("Brin", None, true), mem("Tam", Some(P), false)])
            .is_empty()
    );
    // A member leaves without a kick.
    f.changed(&[
        mem("Brin", Some(A_), true),
        mem("Tam", Some(P), false),
        mem("Cael", None, false),
    ]);
    let l = f.changed(&[mem("Brin", Some(A_), true), mem("Tam", Some(P), false)]);
    assert_eq!(tok(&l[0]), ft(0x0C75_F607));
    // The player becomes leader.
    let l = f.changed(&[mem("Brin", Some(A_), false), mem("Tam", Some(P), true)]);
    assert_eq!(tok(&l[0]), ft(0x043F_25A6));
    // The player leaves.
    f.note_sent(&SocialIntent::LeaveFellowship);
    let l = f.changed(&[]);
    assert_eq!(tok(&l[0]), ft(0x0307_3954));
    // The sent commands are forgotten after an event.
    f.changed(&[mem("Tam", Some(P), true), mem("Brin", None, false)]);
    let l = f.changed(&[]);
    assert_eq!(tok(&l[0]), ft(0x018C_CD74));
}

#[test]
fn members_tr2_player_detection_limit_and_spi6() {
    // The player found by name when no member has its entity; a list without the player
    // gets it appended.
    let mut f = Fellowship::new("Tam", Some(P));
    f.changed(&[mem("Tam", None, false), mem("Brin", None, true)]);
    assert_eq!(row_names(&f), s(&["Brin"]));
    let mut f = Fellowship::new("Tam", Some(P));
    f.changed(&[mem("Brin", None, true)]);
    assert_eq!(f.members().len(), 2);
    assert_eq!(f.members()[1].name, "Tam");
    assert_eq!(row_names(&f), s(&["Brin"]));
    // At most five rows; the player never gets one (SPI6).
    let many: Vec<Member> = (0..8)
        .map(|i| mem(&format!("M{i}"), None, i == 3))
        .chain([mem("Tam", Some(P), false)])
        .collect();
    f.changed(&many);
    let rows = f.rows();
    assert_eq!(rows.len(), 5);
    assert!(rows.iter().all(|r| r.name != "Tam"));
    assert!(rows[3].leader);
    // No leader flagged: no marker anywhere.
    f.changed(&[mem("Tam", Some(P), false), mem("Brin", None, false)]);
    assert!(f.leader().is_none() && !f.player_leads());
    assert!(!f.rows()[0].leader);
}

#[test]
fn member_data_tr3_targeting_and_assist() {
    let mut f = Fellowship::new("Tam", Some(P));
    f.changed(&[
        mem("Tam", Some(P), true),
        mem("Brin", Some(A_), false),
        mem("Cael", None, false),
    ]);
    f.level(A_, 42);
    f.vital(A_, VitalKind::Morale, 50.0, 200.0);
    f.fellow_vital("brin", VitalKind::Morale, 150.0, 200.0);
    f.fellow_vital("Cael", VitalKind::Power, 50.0, 100.0);
    f.defeated(A_, true);
    f.effect(A_, 9, true);
    let rows = f.rows();
    assert_eq!(rows[0].level, Some(42));
    assert_eq!(
        rows[0].morale.unwrap().fill(),
        0.75,
        "the latest report wins"
    );
    assert!(rows[0].defeated);
    assert_eq!(rows[0].effects, vec![9]);
    assert_eq!(
        rows[1].power.unwrap().fill(),
        0.5,
        "by name without an entity"
    );
    assert!(rows[1].level.is_none() && rows[1].morale.is_none());
    f.vital(A_, VitalKind::Morale, 20.0, 200.0);
    assert_eq!(f.rows()[0].morale.unwrap().fill(), 0.1);
    f.defeated(A_, false);
    assert!(!f.rows()[0].defeated);
    // Targeting (§8.1, TR3).
    assert_eq!(
        f.click_row(0),
        Some(SocialAction::Intent(SocialIntent::SelectTarget {
            target: Some(A_)
        }))
    );
    let SocialAction::Post(n) = f.click_row(1).unwrap() else {
        panic!("no notice")
    };
    assert_eq!((tok(&n), n.mask), (ft(0x0571_E4C5), 0x1));
    f.presence(A_, false);
    assert!(matches!(f.click_row(0), Some(SocialAction::Post(_))));
    f.presence(A_, true);
    // SelectFellowN: 1 the player, N ≥ 2 the (N − 1)-th row.
    assert_eq!(
        f.select_fellow(1),
        Some(SocialAction::Intent(SocialIntent::SelectTarget {
            target: Some(P)
        }))
    );
    assert_eq!(f.select_fellow(2), f.click_row(0));
    assert_eq!(f.select_fellow(6), None);
    // Assist: the member's last target, if any.
    assert_eq!(f.assist_row(0), None);
    f.target_changed(A_, Some(77));
    assert_eq!(
        f.assist_fellow(2),
        Some(SocialAction::Intent(SocialIntent::SelectTarget {
            target: Some(77)
        }))
    );
    // The group menu.
    let m = f.row_menu(0).unwrap();
    assert_eq!(
        m.entries
            .iter()
            .map(|e| e.intent.clone())
            .collect::<Vec<_>>(),
        vec![
            Some(SocialIntent::KickFromFellowship {
                name: "Brin".into()
            }),
            Some(SocialIntent::SetFellowshipLeader {
                name: "Brin".into()
            }),
        ]
    );
    assert_eq!(
        f.self_menu().unwrap().entries[0].intent,
        Some(SocialIntent::LeaveFellowship)
    );
}

// ----- TR7 invitation ---------------------------------------------------------------------

#[test]
fn invitation_tr7() {
    let mut i = Invitation::default();
    assert!(!i.shown());
    assert_eq!(i.answer(true), None);
    i.invite("Brin");
    i.invite("Cael");
    assert_eq!(
        i.from(),
        Some("Cael"),
        "a second invitation replaces the first"
    );
    let t = i.text().unwrap();
    assert_eq!((t.table, t.token), ft(0x0897_28CE));
    assert_eq!(i.rows().len(), 1);
    assert_eq!(Invitation::title().token, 0x0304_5B45);
    assert_eq!(
        i.answer(false),
        Some(SocialIntent::AnswerInvite { accept: false })
    );
    assert!(!i.shown());
}

// ----- §12.2 trade, TR9, TR10, SPI8 -------------------------------------------------------

/// An entry: an item instance with `item`, a literal ref (`scenario-feed.md` §5.2) without.
fn entry(name: &str, item: Option<u64>) -> TradeEntry {
    TradeEntry {
        name: name.into(),
        item,
        quantity: 1,
        literal: item.is_none(),
        ..TradeEntry::default()
    }
}

#[test]
fn example_12_2_trade() {
    let mut t = Trade::new(values());
    t.open(1, Some("Dara"));
    let l = t.peer_label().unwrap();
    assert_eq!(l.token, 0x0CFA_21C2);
    assert!(!t.offer(Side::Own).ready && !t.offer(Side::Peer).ready);
    assert!(t.button_ghosted(), "both offers empty");
    t.set_offer(1, Side::Peer, vec![entry("Rope", None)], 1_234_567);
    assert_eq!(values().split(1_234_567), [12, 345, 67]);
    assert_eq!(
        coin_counts(values(), 1_234_567),
        CoinCounts {
            gold: 12,
            silver: 345,
            copper: 67
        }
    );
    assert!(!t.button_ghosted());
    assert_eq!(
        t.click_button(true),
        Some(SocialIntent::SetTradeReady {
            window: 1,
            ready: true
        })
    );
    assert!(t.button_on(), "optimistic");
    t.set_ready(1, Side::Own, true);
    assert!(t.offer(Side::Own).ready);
    t.set_ready(1, Side::Peer, true);
    assert!(t.offer(Side::Peer).ready);
    t.set_offer(
        1,
        Side::Peer,
        vec![entry("Rope", None), entry("Lamp", None)],
        0,
    );
    assert!(
        !t.offer(Side::Own).ready && !t.offer(Side::Peer).ready,
        "SPI8"
    );
    assert!(!t.button_on());
    t.closed(1);
    assert!(!t.shown());
    // Events of another window are ignored.
    t.open(2, None);
    assert!(t.peer_label().is_none());
    t.set_offer(9, Side::Peer, vec![entry("x", None)], 5);
    assert!(t.offer(Side::Peer).entries.is_empty());
}

#[test]
fn trade_coin_drop_cancel() {
    let mut t = Trade::new(values());
    t.set_wallet(250_000);
    t.open(3, Some("Dara"));
    // §6.3: a valid commit offers the total.
    t.type_coin(CoinField::Gold, "2");
    t.type_coin(CoinField::Copper, "5");
    assert_eq!(
        t.commit_coin(CoinField::Copper),
        vec![SocialAction::Intent(SocialIntent::OfferTrade {
            window: 3,
            item: None,
            quantity: 0,
            coin: 200_005
        })]
    );
    // Not a number: the line, the entry cleared.
    t.type_coin(CoinField::Silver, "1x");
    let out = t.commit_coin(CoinField::Silver);
    assert!(matches!(&out[0], SocialAction::Post(n) if n.text_ref().unwrap().token == 0x085F_80B9));
    assert_eq!(t.typed()[1], "");
    // Too much: the line, the committed entry cleared.
    t.type_coin(CoinField::Gold, "3");
    let out = t.commit_coin(CoinField::Gold);
    assert!(matches!(&out[0], SocialAction::Post(n) if n.text_ref().unwrap().token == 0x0619_5A09));
    assert_eq!(
        out.last(),
        Some(&SocialAction::Intent(SocialIntent::OfferTrade {
            window: 3,
            item: None,
            quantity: 0,
            coin: 5
        }))
    );
    // Entries hold at most 5 characters.
    t.type_coin(CoinField::Gold, "1234567");
    assert_eq!(t.typed()[0], "12345");
    t.type_coin(CoinField::Gold, "");
    // Drops (§6.6, TR9).
    assert_eq!(
        t.drop_item(55, 4, true),
        SocialAction::Intent(SocialIntent::OfferTrade {
            window: 3,
            item: Some(55),
            quantity: 4,
            coin: 5
        })
    );
    assert!(matches!(t.drop_item(55, 4, false), SocialAction::Post(n) if n.mask == 0x1));
    // The own coin of a TradeOffer is written into the entries.
    t.set_offer(3, Side::Own, vec![], 1_234_567);
    assert_eq!(
        t.typed(),
        &s(&["12", "345", "67"]).try_into().unwrap() as &[String; 3]
    );
    // Cancel sends CloseWindow; then nothing is open.
    assert_eq!(t.cancel(), Some(SocialIntent::CloseTrade { window: 3 }));
    assert_eq!(t.cancel(), None);
}

// ----- §3.1 toggle rule -------------------------------------------------------------------

#[test]
fn social_panel_toggle_rule() {
    let st = SocialPanelState::default();
    assert_eq!(st.toggle_panel(false), Toggle::Show(SocialPage::Grouping));
    assert_eq!(st.toggle_panel(true), Toggle::Hide);
    let st = SocialPanelState {
        last: Some(SocialPage::Friends),
    };
    assert_eq!(st.toggle_panel(false), Toggle::Show(SocialPage::Friends));
    assert_eq!(
        st.toggle_page(SocialPage::Raid, Some(SocialPage::Friends)),
        Toggle::Show(SocialPage::Raid)
    );
    assert_eq!(
        st.toggle_page(SocialPage::Friends, Some(SocialPage::Friends)),
        Toggle::Hide
    );
    assert_eq!(
        st.toggle_page(SocialPage::Friends, None),
        Toggle::Show(SocialPage::Friends)
    );
    assert_eq!(
        SocialPage::of_command("ToggleSocialGroupStagePanel"),
        Some(SocialPage::RaidLock)
    );
    assert_eq!(SocialPage::of_command("ToggleSocialPanel"), None);
}

// ----- the element trees ------------------------------------------------------------------

const L: u32 = 0x22FF_5000;

fn ui_social(elements: Vec<ElementDesc>) -> Ui {
    let mut env = env_with(vec![layout(L, 800, 600, elements)]);
    let k = sv().layouts;
    for key in [
        k.social,
        k.friends,
        k.ignore,
        k.minifellow,
        k.fellowinvite,
        k.securetrade,
        k.itemdisplay,
    ] {
        env.keys.insert(key, L);
    }
    ui_with(env, 1024, 768)
}

fn button(id: u32, x: i32, y: i32) -> ElementDesc {
    let st = v().states;
    let mut d = typed(id, v().types.button, x, y, 90, 20);
    for s in [st.normal, st.highlight, st.ghosted] {
        d = state(d, s, 0, vec![], vec![]);
    }
    d
}

fn text(id: u32) -> ElementDesc {
    typed(id, v().types.text, 0, 0, 100, 20)
}

/// A text element with the layout's own design-time text, as the install's rows carry.
fn text_with_default(id: u32, default: &str) -> ElementDesc {
    prop(
        text(id),
        v().pids.text_entry,
        tessera_formats::property::PropertyValue::StringInfo(Box::new(
            tessera_formats::string_info::StringInfo {
                source: tessera_formats::string_info::StringSource::Literal(default.into()),
                args: None,
            },
        )),
    )
}

fn ghosted(ui: &Ui, k: ElementKey) -> bool {
    ui.property(k, v().pids.button_ghosted) == Some(&b(true))
}

fn msg(ui: &Ui, source: ElementKey, msg: u32, command: Option<&str>) -> UiEvent {
    UiEvent::Game {
        message: Message {
            seq: 0,
            source,
            source_id: ui.element_id(source).unwrap_or(0),
            msg,
            arg: MsgArg {
                command: command.map(Command::named),
                ..MsgArg::default()
            },
        },
    }
}

fn social_layout() -> Vec<ElementDesc> {
    let i = sv().ids;
    let st = sv().states;
    let pp = sv().pids;
    let vp = v().pids;
    let pages = array(
        0x77,
        [
            (i.grouping_tab, i.grouping_page, true),
            (i.friends_tab, i.friends_page_outer, false),
            (i.raid_tab, i.raid_page, false),
        ]
        .into_iter()
        .map(|(t, p, o)| {
            strukt(vec![
                (vp.panel_tab, en(t)),
                (vp.panel_page, en(p)),
                (vp.panel_page_open, b(o)),
            ])
        })
        .collect(),
    );
    let mut fpage = field(i.friends_page, 0, 0, 565, 470);
    fpage = child(
        fpage,
        typed(i.friend_list, v().types.tree, 10, 58, 540, 371),
    );
    for (n, id) in [
        i.friend_header_member,
        i.friend_header_last_on,
        i.friend_add,
        i.friend_remove,
        i.friend_note,
        i.friend_invite,
        i.friend_tell,
    ]
    .into_iter()
    .enumerate()
    {
        fpage = child(fpage, button(id, 100 * n as i32, 439));
    }
    let mut panel = prop(
        typed(i.social_panel, v().types.panel, 0, 0, 587, 577),
        vp.panel_pages,
        pages,
    );
    for t in [i.grouping_tab, i.friends_tab, i.raid_tab] {
        panel = child(panel, button(t, 0, 2));
    }
    panel = child(
        panel,
        child(
            field(i.grouping_page, 0, 30, 565, 500),
            button(0x1000_5999, 5, 5),
        ),
    );
    panel = child(
        panel,
        child(field(i.friends_page_outer, 0, 30, 565, 500), fpage),
    );
    panel = child(panel, field(i.raid_page, 0, 30, 565, 500));
    let root = child(
        child(field(i.social_field, 0, 0, 587, 577), panel),
        button(i.social_close, 560, 5),
    );
    // The row template.
    let mut grouped = field(i.friend_row_grouped, 8, 6, 16, 16);
    for s in [st.grouped_unknown, st.grouped_none, st.grouped_yes] {
        grouped = state(grouped, s, 0, vec![], vec![]);
    }
    let name = prop(
        prop(
            text(i.friend_row_name),
            pp.online_colour,
            PropertyValue_colour(0xFF00_FF00),
        ),
        pp.offline_colour,
        PropertyValue_colour(0xFFDB_DB00),
    );
    let mut row = button(i.friend_template, 0, 0);
    for c in [
        grouped,
        name,
        text(i.friend_row_class),
        text(i.friend_row_level),
        text(i.friend_row_area),
        text(i.friend_row_last_on),
        text(i.friend_row_note),
    ] {
        row = child(row, c);
    }
    vec![root, row]
}

#[allow(non_snake_case)]
fn PropertyValue_colour(argb: u32) -> tessera_formats::property::PropertyValue {
    tessera_formats::property::PropertyValue::Colour(argb.to_le_bytes())
}

struct Words;

impl TextSource for Words {
    fn render(&mut self, table: u32, token: u32, args: &[(u32, String)]) -> Option<String> {
        let _ = args;
        Some(format!("{table:x}/{token:x}"))
    }
}

#[test]
fn social_panel_and_friends_page_on_the_tree() {
    let mut ui = ui_social(social_layout());
    let vocab = sv();
    let i = vocab.ids;
    let mut panel = SocialPanel::open(&mut ui, &vocab).unwrap();
    assert!(!ui.is_visible(panel.root), "TR1: hidden at start");
    // TR8: the other pages' buttons are ghosted.
    let other = tessera_ui::game::find_below(&ui, panel.root, 0x1000_5999).unwrap();
    assert!(ghosted(&ui, other));
    // ToggleSocialPanel: the set-up page; a page command; the same page command hides.
    assert!(panel.command(&mut ui, "ToggleSocialPanel"));
    assert_eq!(panel.current(&ui), Some(SocialPage::Grouping));
    panel.command(&mut ui, "ToggleSocialFriendsPanel");
    assert_eq!(panel.current(&ui), Some(SocialPage::Friends));
    panel.command(&mut ui, "ToggleSocialFriendsPanel");
    assert!(!ui.is_visible(panel.root));
    panel.command(&mut ui, "ToggleSocialPanel");
    assert_eq!(
        panel.current(&ui),
        Some(SocialPage::Friends),
        "the last page"
    );
    assert!(!panel.command(&mut ui, "SomethingElse"));
    // The friends page.
    let mut page = FriendsPage::bind(&mut ui, &vocab, panel.root).unwrap();
    let mut m = FriendsModel::default();
    m.set_list(&friends(&[("bo", false), ("Al", true), ("cy", true)]));
    m.set_fellows(&s(&["cy"]));
    page.refresh(&mut ui, &m, &mut NoTables);
    assert_eq!(page.rows.len(), 3);
    let row_text = |ui: &mut Ui, r: ElementKey, id: u32| {
        let k = tessera_ui::game::find_below(ui, r, id).unwrap();
        ui.text_of(k)
    };
    let names: Vec<String> = page
        .rows
        .clone()
        .into_iter()
        .map(|r| row_text(&mut ui, r, i.friend_row_name))
        .collect();
    assert_eq!(names, s(&["Al", "bo", "cy"]));
    let al_name = tessera_ui::game::find_below(&ui, page.rows[0], i.friend_row_name).unwrap();
    assert_eq!(
        ui.property(al_name, vocab.pids.text_font_color),
        Some(&PropertyValue_colour(0xFF00_FF00))
    );
    let bo_name = tessera_ui::game::find_below(&ui, page.rows[1], i.friend_row_name).unwrap();
    assert_eq!(
        ui.property(bo_name, vocab.pids.text_font_color),
        Some(&PropertyValue_colour(0xFFDB_DB00))
    );
    let cy_g = tessera_ui::game::find_below(&ui, page.rows[2], i.friend_row_grouped).unwrap();
    assert_eq!(ui.state(cy_g), Some(vocab.states.grouped_yes));
    let al_g = tessera_ui::game::find_below(&ui, page.rows[0], i.friend_row_grouped).unwrap();
    assert_eq!(ui.state(al_g), Some(vocab.states.grouped_unknown));
    assert_eq!(row_text(&mut ui, page.rows[1], i.friend_row_level), "?");
    let mut words = Words;
    page.refresh(&mut ui, &m, &mut words);
    assert_eq!(
        row_text(&mut ui, page.rows[1], i.friend_row_level),
        format!("{:x}/{:x}", T::UI_FRIENDS, 0x0A82_90DFu32)
    );
    // Buttons without a selection.
    let f = |ui: &Ui, id| tessera_ui::game::find_below(ui, panel.root, id).unwrap();
    let (add, remove, note, tell, invite) = (
        f(&ui, i.friend_add),
        f(&ui, i.friend_remove),
        f(&ui, i.friend_note),
        f(&ui, i.friend_tell),
        f(&ui, i.friend_invite),
    );
    assert!(!ghosted(&ui, add) && ghosted(&ui, remove) && ghosted(&ui, note));
    // A right click on Al's row selects it and asks for the menu.
    let ms = ui.vocab().msgs;
    let ev = msg(&ui, page.rows[0], ms.mouse_click, Some("SelectRight"));
    let out = page.handle_event(&mut ui, &ev, &mut m, GroupState::default(), &mut NoTables);
    assert!(matches!(out.as_slice(), [SocialAction::Menu(_)]));
    assert_eq!(m.selected(), Some("Al"));
    assert!(!ghosted(&ui, remove) && !ghosted(&ui, tell) && !ghosted(&ui, invite));
    // Tell and invite buttons.
    let ev = msg(&ui, tell, ms.button_click, Some("SelectLeft"));
    assert_eq!(
        page.handle_event(&mut ui, &ev, &mut m, GroupState::default(), &mut NoTables),
        vec![SocialAction::Intent(SocialIntent::StartTell {
            name: "Al".into()
        })]
    );
    let ev = msg(&ui, add, ms.button_click, Some("SelectLeft"));
    assert!(matches!(
        page.handle_event(&mut ui, &ev, &mut m, GroupState::default(), &mut NoTables)
            .as_slice(),
        [SocialAction::Dialog(_)]
    ));
    // A double click on bo (offline) selects it and sends nothing.
    let ev = msg(&ui, page.rows[1], ms.mouse_dbl_click, Some("SelectDblLeft"));
    assert!(
        page.handle_event(&mut ui, &ev, &mut m, GroupState::default(), &mut NoTables)
            .is_empty()
    );
    assert_eq!(m.selected(), Some("bo"));
    assert!(ghosted(&ui, tell));
    // A header click re-sorts the rows (Last online: offline first).
    let h = f(&ui, i.friend_header_last_on);
    let ev = msg(&ui, h, ms.button_click, Some("SelectLeft"));
    page.handle_event(&mut ui, &ev, &mut m, GroupState::default(), &mut NoTables);
    let names: Vec<String> = page
        .rows
        .clone()
        .into_iter()
        .map(|r| row_text(&mut ui, r, i.friend_row_name))
        .collect();
    assert_eq!(names, s(&["bo", "Al", "cy"]));
    // The selection follows its name.
    let list = page.list.unwrap();
    assert_eq!(ui.list_selected(list), Some(page.rows[0]));
    // A shorter list removes rows.
    m.set_list(&friends(&[("Al", true)]));
    page.refresh(&mut ui, &m, &mut NoTables);
    assert_eq!(page.rows.len(), 1);
    assert_eq!(ui.list_entries(list).len(), 1);
}

/// A meter below the morale display that `UI_VitalDisplay_Array` does not name.
const BACKGROUND_METER: u32 = 0x1000_5A01;
/// The synthetic player-vitals panel's root.
const P4_ROOT: u32 = 0x1000_5A10;

/// A text source that renders the level templates of SP28 with a marker, so a test sees
/// which entry a level text came from.
struct Levels;

impl TextSource for Levels {
    fn render(&mut self, table: u32, token: u32, args: &[(u32, String)]) -> Option<String> {
        let arg = |id: u32| args.iter().find(|(a, _)| *a == id).map(|(_, v)| v.clone());
        match (table, token) {
            (T::UI, 0x070F_9F3C) => Some(format!("Lv {}", arg(A::FELLOW_LEVEL)?)),
            (T::UI, 0x038B_9CDC) => Some(format!("Level {}", arg(A::LEVEL)?)),
            _ => None,
        }
    }
}

fn fellowship_layout() -> Vec<ElementDesc> {
    let i = sv().ids;
    let st = sv().states;
    let mut out = Vec::new();
    for id in [
        i.fellow_panel_1,
        i.fellow_panel_2,
        i.fellow_panel_3,
        i.fellow_panel_4,
    ] {
        out.push(child(
            field(id, 14, 121, 221, 452),
            typed(i.fellow_list, v().types.list_box, 0, 20, 209, 432),
        ));
    }
    let mut leader = field(i.fellow_leader, 39, 34, 16, 16);
    for s in [st.leader_fellowship, st.leader_raid, st.leader_monster] {
        leader = state(leader, s, 0, vec![], vec![]);
    }
    let meter = |id: u32| typed(id, v().types.meter, 0, 0, 99, 6);
    // §4.2: the displays with the meters `UI_VitalDisplay_Array` names, a background meter
    // the array does not name, the hidden texts and the wrath field.
    let mut health = field(i.vital_health, 52, 25, 99, 6);
    for c in [
        meter(BACKGROUND_METER),
        meter(i.vital_health_meter),
        meter(i.vital_temp_health),
        meter(i.vital_dread),
        text(i.vital_health_text),
    ] {
        health = child(health, c);
    }
    let mut inner_power = field(i.vital_power, 0, 0, 99, 6);
    for c in [
        meter(i.vital_power_meter),
        meter(i.vital_temp_power),
        text(i.vital_power_text),
    ] {
        inner_power = child(inner_power, c);
    }
    let power = child(
        child(field(i.vital_power, 51, 32, 99, 6), inner_power),
        child(field(i.vital_rage_field, 0, 0, 99, 6), meter(0x1000_5A02)),
    );
    let mut row = field(i.fellow_template, 0, 0, 221, 75);
    for c in [
        text(i.fellow_name),
        text_with_default(i.fellow_level, "99"),
        leader,
        field(i.fellow_defeated, 55, 20, 112, 16),
        field(i.fellow_link_dead, 55, 20, 112, 16),
        child(child(field(i.fellow_vitals, 3, -3, 170, 80), health), power),
        button(i.fellow_assist, 153, 21),
        field(i.fellow_effects, 54, 37, 150, 37),
        // §4.2: the voice and status elements, none hidden by the layout (SPI9).
        button(i.fellow_voice, 38, 0),
        field(i.fellow_voice_squelched, 38, 0, 20, 20),
        field(i.fellow_sending, 38, 0, 20, 20),
        field(i.fellow_volume, 0, 0, 20, 60),
        field(i.fellow_pet_vitals, 0, 0, 20, 20),
    ] {
        row = child(row, c);
    }
    out.push(row);
    // The player-vitals panel P4's elements (§4.3).
    let mut p4 = field(P4_ROOT, 0, 0, 200, 80);
    for c in [
        field(i.player_leader, 0, 0, 16, 16),
        field(i.player_voice, 71, 11, 20, 20),
        field(i.player_voice_squelched, 71, 11, 20, 20),
        field(i.player_sending, 71, 11, 20, 20),
        field(i.player_assistant, 0, 0, 16, 16),
    ] {
        p4 = child(p4, c);
    }
    out.push(p4);
    out
}

#[test]
fn fellowship_panels_on_the_tree() {
    let vocab = sv();
    let i = vocab.ids;
    let mut ui = ui_social(fellowship_layout());
    let mut panels = FellowshipPanels::open(&mut ui, &vocab);
    assert_eq!(panels.panels.len(), 4);
    assert!(panels.panels.iter().all(|p| !ui.is_visible(*p)));
    let p4 = ui
        .open_panel(tessera_formats::did::Did(L), P4_ROOT)
        .unwrap();
    let pl = tessera_ui::game::find_below(&ui, p4, i.player_leader).unwrap();
    let mut f = Fellowship::new("Tam", Some(P));
    // TR14: the portrait's voice, squelched, sending-data and assistant elements are
    // shown by the layout (SPI9) and hidden when the panel is bound.
    let p4_voice = [
        i.player_voice,
        i.player_voice_squelched,
        i.player_sending,
        i.player_assistant,
    ];
    assert!(p4_voice.iter().all(|id| {
        let k = tessera_ui::game::find_below(&ui, p4, *id).unwrap();
        ui.is_visible(k)
    }));
    panels.attach_player_panel(&mut ui, p4, &f);
    assert!(!ui.is_visible(pl));
    for id in p4_voice {
        let k = tessera_ui::game::find_below(&ui, p4, id).unwrap();
        assert!(!ui.is_visible(k), "TR14: P4 element {id:#x} hidden");
    }
    f.changed(&[
        mem("Tam", Some(P), true),
        mem("Brin", Some(A_), false),
        mem("Cael", None, false),
    ]);
    f.level(A_, 12);
    f.vital(A_, VitalKind::Morale, 30.0, 60.0);
    f.vital(A_, VitalKind::Power, 10.0, 40.0);
    f.defeated(A_, true);
    panels.refresh(&mut ui, &f, &mut Levels);
    assert!(ui.is_visible(panels.panels[0]));
    assert!(panels.panels[1..].iter().all(|p| !ui.is_visible(*p)));
    assert!(ui.is_visible(pl), "PlayerLeaderIcon while the player leads");
    assert_eq!(panels.rows.len(), 2);
    let get = |ui: &Ui, r: ElementKey, id: u32| tessera_ui::game::find_below(ui, r, id).unwrap();
    let r0 = panels.rows[0];
    let name = get(&ui, r0, i.fellow_name);
    assert_eq!(ui.text_of(name), "Brin");
    let lvl = get(&ui, r0, i.fellow_level);
    assert_eq!(
        ui.text_of(lvl),
        "Lv 12",
        "SP28: (UI, 0x070F9F3C) with 0x7C83B5"
    );
    // TR3: a member without an entity has an empty level, not the layout's own text.
    let lvl1 = get(&ui, panels.rows[1], i.fellow_level);
    assert_eq!(ui.text_of(lvl1), "");
    assert!(!ui.is_visible(get(&ui, r0, i.fellow_leader)));
    assert!(ui.is_visible(get(&ui, r0, i.fellow_defeated)));
    assert!(!ui.is_visible(get(&ui, r0, i.fellow_link_dead)));
    // TR15: only the current meters take the value; the temporary and dread meters are
    // at 0; the background meter keeps the layout's (no position); texts and the wrath
    // field hidden, the power fields shown.
    let pos = |ui: &Ui, id: u32| {
        ui.property(get(ui, r0, id), v().pids.meter_position)
            .cloned()
    };
    assert_eq!(pos(&ui, i.vital_health_meter), Some(f32_value(0.5)));
    assert_eq!(pos(&ui, i.vital_power_meter), Some(f32_value(0.25)));
    for id in [i.vital_temp_health, i.vital_dread, i.vital_temp_power] {
        assert_eq!(pos(&ui, id), Some(f32_value(0.0)), "{id:#x} at 0");
    }
    assert_eq!(
        pos(&ui, BACKGROUND_METER),
        None,
        "left as the layout has it"
    );
    for id in [i.vital_health_text, i.vital_power_text, i.vital_rage_field] {
        assert!(!ui.is_visible(get(&ui, r0, id)), "{id:#x} hidden");
    }
    assert!(ui.is_visible(get(&ui, r0, i.vital_power)));
    // TR14 (SPI13): no voice, squelched, sending-data or volume element on a row.
    for r in &panels.rows {
        for id in [
            i.fellow_voice,
            i.fellow_voice_squelched,
            i.fellow_sending,
            i.fellow_volume,
            i.fellow_pet_vitals,
        ] {
            assert!(!ui.is_visible(get(&ui, *r, id)), "TR14: {id:#x} hidden");
        }
    }
    // TR16: each row's effect list goes to the combat widgets with the member's entity.
    let lists: Vec<(ElementKey, Option<u64>)> = panels
        .rows
        .iter()
        .map(|r| (get(&ui, *r, i.fellow_effects), None))
        .collect();
    assert_eq!(
        panels.effect_lists(),
        &[(lists[0].0, Some(A_)), (lists[1].0, None)]
    );
    // Leader change to Brin: the marker moves to Brin's row.
    f.changed(&[
        mem("Tam", Some(P), false),
        mem("Brin", Some(A_), true),
        mem("Cael", None, false),
    ]);
    panels.refresh(&mut ui, &f, &mut Levels);
    assert!(!ui.is_visible(pl));
    let leader = get(&ui, panels.rows[0], i.fellow_leader);
    assert!(ui.is_visible(leader));
    assert_eq!(ui.state(leader), Some(vocab.states.leader_fellowship));
    // A click on Cael's row (no entity) posts the out-of-range line.
    let ms = ui.vocab().msgs;
    let ev = msg(&ui, panels.rows[1], ms.mouse_click, Some("SelectLeft"));
    let out = panels.handle_event(&ui, &ev, &f);
    assert!(matches!(out.as_slice(), [SocialAction::Post(_)]));
    let ev = msg(&ui, panels.rows[0], ms.mouse_click, Some("SelectLeft"));
    assert_eq!(
        panels.handle_event(&ui, &ev, &f),
        vec![SocialAction::Intent(SocialIntent::SelectTarget {
            target: Some(A_)
        })]
    );
    // Disband: the rows go and panel 1 hides.
    f.changed(&[]);
    panels.refresh(&mut ui, &f, &mut Levels);
    assert!(panels.rows.is_empty());
    assert!(panels.effect_lists().is_empty(), "TR16: no rows, no lists");
    assert!(!ui.is_visible(panels.panels[0]));
}

fn f32_value(x: f32) -> tessera_formats::property::PropertyValue {
    tessera_formats::property::PropertyValue::Float(x)
}

#[test]
fn invite_window_on_the_tree() {
    let vocab = sv();
    let i = vocab.ids;
    let mut win = field(i.invite, 62, 86, 354, 431);
    for c in [
        text(i.invite_title),
        text(i.invite_text),
        text(i.invite_warning),
        typed(i.invite_list, v().types.list_box, 21, 104, 313, 220),
        button(i.invite_accept, 67, 401),
        button(i.invite_decline, 187, 401),
        button(i.invite_close, 334, 25),
    ] {
        win = child(win, c);
    }
    let mut row = field(i.invite_row, 0, 0, 296, 39);
    for c in [
        text(i.invite_row_name),
        text_with_default(i.invite_row_level, "123"),
        text_with_default(i.invite_row_area, "Menu option 2"),
    ] {
        row = child(row, c);
    }
    let mut ui = ui_social(vec![win, row]);
    let mut w = InviteWindow::open(&mut ui, &vocab).unwrap();
    let mut m = Invitation::default();
    w.refresh(&mut ui, &m, &mut NoTables);
    assert!(!ui.is_visible(w.root));
    m.invite("Brin");
    w.refresh(&mut ui, &m, &mut NoTables);
    assert!(ui.is_visible(w.root));
    let root = w.root;
    let get = |ui: &Ui, id: u32| tessera_ui::game::find_below(ui, root, id).unwrap();
    let text_el = get(&ui, i.invite_text);
    assert!(ui.text_of(text_el).contains("Brin"));
    assert_eq!(w.rows.len(), 1);
    let rn = tessera_ui::game::find_below(&ui, w.rows[0], i.invite_row_name).unwrap();
    assert_eq!(ui.text_of(rn), "Brin");
    // TR7: no level or area: both empty, never the layout's own texts.
    for id in [i.invite_row_level, i.invite_row_area] {
        let e = tessera_ui::game::find_below(&ui, w.rows[0], id).unwrap();
        assert_eq!(ui.text_of(e), "");
    }
    let accept = get(&ui, i.invite_accept);
    let ms = ui.vocab().msgs;
    let ev = msg(&ui, accept, ms.button_click, Some("SelectLeft"));
    assert_eq!(
        w.handle_event(&mut ui, &ev, &mut m),
        vec![SocialAction::Intent(SocialIntent::AnswerInvite {
            accept: true
        })]
    );
    assert!(!ui.is_visible(w.root));
    m.invite("Cael");
    w.refresh(&mut ui, &m, &mut NoTables);
    let close = get(&ui, i.invite_close);
    let ev = msg(&ui, close, ms.button_click, Some("SelectLeft"));
    assert_eq!(
        w.handle_event(&mut ui, &ev, &mut m),
        vec![SocialAction::Intent(SocialIntent::AnswerInvite {
            accept: false
        })]
    );
}

/// The synthetic `securetrade` window and medium item display.
fn trade_ui() -> Ui {
    let vocab = sv();
    let cv = CharVocab::default();
    let i = vocab.ids;
    let st = v().states;
    let side_field = |id: u32, list: u32| {
        let mut f = field(id, 0, 48, 252, 264);
        for s in [st.normal, st.highlight] {
            f = state(f, s, 0, vec![], vec![]);
        }
        child(f, typed(list, v().types.list_box, 0, 0, 226, 222))
    };
    // §6.1: each coin field holds a label (a Field) and its editable entry, by ID.
    let coin = |id: u32, entry: u32| {
        child(
            child(field(id, 0, 0, 50, 20), text(0x1000_5B10 + id % 16)),
            prop(text(entry), v().pids.text_editable, b(true)),
        )
    };
    // §6.1, SP31: the two money displays, of the money-display types, without children.
    let nv = &vocab.npc;
    let money = |id: u32, ty: u32, suppress: bool| {
        let d = typed(id, ty, 83, 5, 160, 22);
        let d = prop(d, nv.pids.force_show_all, b(true));
        prop(d, nv.pids.suppress_leading_zeroes, b(suppress))
    };
    let mut trade_button = button(i.trade_button, 31, 316);
    trade_button = prop(trade_button, v().pids.button_boolean, b(true));
    let mut win = field(i.trade, 254, 118, 518, 388);
    for c in [
        side_field(i.trade_own_field, i.trade_own_list),
        side_field(i.trade_peer_field, i.trade_peer_list),
        trade_button,
        button(i.trade_cancel, 130, 316),
        button(i.trade_close, 494, 23),
        text(i.trade_peer_label),
        money(i.trade_wallet, nv.types.player_money_display, false),
        money(i.trade_peer_money, nv.types.money_display, true),
        field(i.trade_wait, 4, 85, 508, 266),
        coin(i.coin_gold, i.coin_gold_entry),
        coin(i.coin_silver, i.coin_silver_entry),
        coin(i.coin_copper, i.coin_copper_entry),
    ] {
        win = child(win, c);
    }
    // §6.2, SP30: the icon holds the (hidden) quantity overlay, a Text that comes before
    // the name Text in tree order.
    let mut name = typed(i.item_name, v().types.text, 35, 0, 165, 32);
    for s in [
        vocab.states.object_name,
        vocab.states.quality_rare,
        vocab.states.quality_common,
    ] {
        name = state(name, s, 0, vec![], vec![]);
    }
    let quantity = prop(
        typed(cv.ids.quantity, v().types.text, 0, 0, 32, 32),
        v().pids.hide,
        b(true),
    );
    let medium = child(
        child(
            field(i.item_medium, 0, 0, 200, 32),
            child(
                field(0x1000_5C00, 0, 0, 32, 32),
                child(
                    typed(i.item_icon, cv.types.item_icon_display, 0, 0, 32, 32),
                    quantity,
                ),
            ),
        ),
        name,
    );
    ui_social(vec![win, medium])
}

/// The coin entry's editable text of coin field `id` (SP31: by its entry ID).
fn coin_entry(ui: &Ui, root: ElementKey, id: u32) -> ElementKey {
    let ids = sv().ids;
    let entry = if id == ids.coin_gold {
        ids.coin_gold_entry
    } else if id == ids.coin_silver {
        ids.coin_silver_entry
    } else {
        ids.coin_copper_entry
    };
    let f = tessera_ui::game::find_below(ui, root, id).unwrap();
    tessera_ui::game::find_below(ui, f, entry).unwrap()
}

#[test]
fn trade_window_on_the_tree() {
    let vocab = sv();
    let cv = CharVocab::default();
    let i = vocab.ids;
    let st = v().states;
    let mut ui = trade_ui();
    let mut w = TradeWindow::open(&mut ui, &vocab, cv.clone()).unwrap();
    let mut t = Trade::new(values());
    t.set_wallet(300_000);
    w.refresh(&mut ui, &t, &mut NoTables);
    assert!(!ui.is_visible(w.root));
    t.open(1, Some("Dara"));
    t.set_offer(
        1,
        Side::Peer,
        vec![entry("Rope", Some(70)), entry("Note", None)],
        1_234_567,
    );
    w.refresh(&mut ui, &t, &mut NoTables);
    assert!(ui.is_visible(w.root));
    let root = w.root;
    let get = |ui: &Ui, id: u32| tessera_ui::game::find_below(ui, root, id).unwrap();
    let label = get(&ui, i.trade_peer_label);
    assert!(ui.text_of(label).contains("Dara"));
    // §6.1, TR9: the money displays are coin elements, not texts.
    let nv = &vocab.npc;
    let coin_texts = |d: &tessera_ui::game::npc::MoneyDisplay, ui: &mut Ui| -> Vec<String> {
        let shown: Vec<_> = d
            .coins
            .iter()
            .flatten()
            .filter(|c| ui.is_visible(c.element))
            .filter_map(|c| c.text)
            .collect();
        shown.into_iter().map(|t| ui.text_of(t)).collect()
    };
    let peer_money = w.peer_money.clone().expect("the peer's money display");
    assert_eq!(
        peer_money.counts(),
        Some(CoinCounts {
            gold: 12,
            silver: 345,
            copper: 67
        })
    );
    assert_eq!(coin_texts(&peer_money, &mut ui), ["12", "345", "67"]);
    let money_el = get(&ui, i.trade_peer_money);
    assert_eq!(ui.list_entries(money_el).len(), 3, "three coin elements");
    assert_eq!(
        ui.text_of(money_el),
        "",
        "the display has no text of its own"
    );
    let wallet = w.wallet.clone().expect("the wallet display");
    // 300,000 with ForceShowAll and no SuppressLeadingZeroes: 3 gold, 0 silver, 0 copper.
    assert_eq!(coin_texts(&wallet, &mut ui), ["3", "0", "0"]);
    let _ = nv;
    assert_eq!(w.entries.1.len(), 2);
    assert_eq!(w.entries.0.len(), 0);
    // §6.2: the name goes into `ItemDisplay_Name`, not the quantity overlay before it.
    let part =
        |ui: &Ui, row: ElementKey, id: u32| tessera_ui::game::find_below(ui, row, id).unwrap();
    let name = part(&ui, w.entries.1[0], i.item_name);
    assert_eq!(ui.text_of(name), "Rope");
    assert_eq!(ui.state(name), Some(vocab.states.object_name), "no quality");
    let quantity = part(&ui, w.entries.1[0], cv.ids.quantity);
    assert_eq!(ui.text_of(quantity), "", "quantity 1 is not shown");
    let icon = part(&ui, w.entries.1[0], i.item_icon);
    assert!(ui.examines(icon), "the examination tooltip of the item");
    let name2 = part(&ui, w.entries.1[1], i.item_name);
    assert_eq!(ui.text_of(name2), "Note");
    assert_eq!(ui.state(name2), Some(vocab.states.object_name), "TR9");
    let icon2 = part(&ui, w.entries.1[1], i.item_icon);
    assert!(!ui.examines(icon2), "a literal ref has nothing to examine");
    // The name colour follows `Item_Quality` (§6.2 Tessera rule).
    let rare = TradeEntry {
        quality: Some(2),
        quantity: 5,
        ..entry("Lamp", Some(71))
    };
    assert_eq!(name_state(&vocab, &rare), vocab.states.quality_rare);
    t.set_offer(1, Side::Peer, vec![rare, entry("Note", None)], 1_234_567);
    w.refresh(&mut ui, &t, &mut NoTables);
    let name = part(&ui, w.entries.1[0], i.item_name);
    assert_eq!(ui.text_of(name), "Lamp");
    assert_eq!(ui.state(name), Some(vocab.states.quality_rare));
    let quantity = part(&ui, w.entries.1[0], cv.ids.quantity);
    assert_eq!(ui.text_of(quantity), "5");
    let button_el = get(&ui, i.trade_button);
    assert!(!ghosted(&ui, button_el));
    // The trade button: on → SetTradeReady (optimistic).
    ui.set_property(button_el, v().pids.button_highlighted, b(true));
    let ms = ui.vocab().msgs;
    let ev = msg(&ui, button_el, ms.button_click, Some("SelectLeft"));
    assert_eq!(
        w.handle_event(&mut ui, &ev, &mut t),
        vec![SocialAction::Intent(SocialIntent::SetTradeReady {
            window: 1,
            ready: true
        })]
    );
    t.set_ready(1, Side::Peer, true);
    w.refresh(&mut ui, &t, &mut NoTables);
    let peer_field = get(&ui, i.trade_peer_field);
    assert_eq!(ui.state(peer_field), Some(st.highlight));
    // A content change resets both (SPI8).
    t.set_offer(1, Side::Peer, vec![], 0);
    w.refresh(&mut ui, &t, &mut NoTables);
    assert_eq!(ui.state(peer_field), Some(st.normal));
    assert_eq!(
        ui.property(button_el, v().pids.button_highlighted),
        Some(&b(false))
    );
    assert!(ghosted(&ui, button_el), "both offers empty");
    assert!(w.entries.1.is_empty());
    // A coin entry losing focus commits (§6.3).
    let gold = tessera_ui::game::find_below(&ui, get(&ui, i.coin_gold), i.coin_gold_entry).unwrap();
    ui.set_text(gold, Some("2".into()));
    let ev = UiEvent::Game {
        message: Message {
            seq: 0,
            source: gold,
            source_id: ui.element_id(gold).unwrap(),
            msg: ms.set_focus,
            arg: MsgArg::value(0),
        },
    };
    assert_eq!(
        w.handle_event(&mut ui, &ev, &mut t),
        vec![SocialAction::Intent(SocialIntent::OfferTrade {
            window: 1,
            item: None,
            quantity: 0,
            coin: 200_000
        })]
    );
    // Cancel: CloseWindow, hidden.
    let cancel = get(&ui, i.trade_cancel);
    let ev = msg(&ui, cancel, ms.button_click, Some("SelectLeft"));
    assert_eq!(
        w.handle_event(&mut ui, &ev, &mut t),
        vec![SocialAction::Intent(SocialIntent::CloseTrade { window: 1 })]
    );
    assert!(!ui.is_visible(w.root));
}

#[test]
fn text_refs_build_with_arguments() {
    let r = TextRef::new(T::UI_SECURE_TRADE, 0x0CFA_21C2).text(A::NAME, "Dara");
    assert!(r.render(&mut NoTables).contains("Dara"));
}

// ----- review fixes -------------------------------------------------------------------------

#[test]
fn typed_coin_survives_a_refresh_for_another_event() {
    let vocab = sv();
    let mut ui = trade_ui();
    let mut w = TradeWindow::open(&mut ui, &vocab, CharVocab::default()).unwrap();
    let root = w.root;
    let i = vocab.ids;
    let mut t = Trade::new(values());
    t.set_wallet(10_000_000);
    t.open(1, Some("Dara"));
    w.refresh(&mut ui, &t, &mut NoTables);
    let gold = coin_entry(&ui, root, i.coin_gold);
    let button_el = tessera_ui::game::find_below(&ui, root, i.trade_button).unwrap();
    assert!(ghosted(&ui, button_el));
    // Typed, not committed; then a peer offer, a ready change and a wallet change.
    ui.set_text(gold, Some("2".into()));
    t.set_offer(1, Side::Peer, vec![], 0);
    w.refresh(&mut ui, &t, &mut NoTables);
    assert_eq!(ui.text_of(gold), "2", "a peer offer keeps the typed coin");
    assert!(!ghosted(&ui, button_el), "§6.4: coin typed on screen");
    t.set_ready(1, Side::Peer, true);
    t.set_wallet(9_000_000);
    w.refresh(&mut ui, &t, &mut NoTables);
    assert_eq!(ui.text_of(gold), "2");
    // An own TradeOffer writes the entries (TR9).
    t.set_offer(1, Side::Own, vec![], 300_000);
    w.refresh(&mut ui, &t, &mut NoTables);
    assert_eq!(ui.text_of(gold), "3");
    // A new trade clears them.
    t.closed(1);
    w.refresh(&mut ui, &t, &mut NoTables);
    t.open(2, None);
    w.refresh(&mut ui, &t, &mut NoTables);
    assert_eq!(ui.text_of(gold), "");
}

#[test]
fn feed_coin_longer_than_an_entry_is_not_cut_on_commit() {
    let vocab = sv();
    let mut ui = trade_ui();
    let mut w = TradeWindow::open(&mut ui, &vocab, CharVocab::default()).unwrap();
    let root = w.root;
    let i = vocab.ids;
    let mut t = Trade::new(values());
    t.set_wallet(u64::MAX);
    t.open(1, None);
    t.set_offer(1, Side::Own, vec![], 123_456 * 100_000);
    w.refresh(&mut ui, &t, &mut NoTables);
    let gold = coin_entry(&ui, root, i.coin_gold);
    assert_eq!(ui.text_of(gold), "123456");
    let copper = coin_entry(&ui, root, i.coin_copper);
    ui.set_text(copper, Some("7".into()));
    let ms = ui.vocab().msgs;
    let ev = UiEvent::Game {
        message: Message {
            seq: 0,
            source: copper,
            source_id: ui.element_id(copper).unwrap(),
            msg: ms.set_focus,
            arg: MsgArg::value(0),
        },
    };
    assert_eq!(
        w.handle_event(&mut ui, &ev, &mut t),
        vec![SocialAction::Intent(SocialIntent::OfferTrade {
            window: 1,
            item: None,
            quantity: 0,
            coin: 12_345_600_007
        })]
    );
    // An edited entry is still cut to five characters.
    ui.set_text(gold, Some("9876543".into()));
    let out = w.handle_event(&mut ui, &ev, &mut t);
    assert_eq!(
        out,
        vec![SocialAction::Intent(SocialIntent::OfferTrade {
            window: 1,
            item: None,
            quantity: 0,
            coin: 98_765 * 100_000 + 7
        })]
    );
}

#[test]
fn over_wallet_commit_clears_only_that_entry() {
    // §6.3: "clears the entry", then the amounts present are sent.
    let mut t = Trade::new(values());
    t.set_wallet(150);
    t.open(1, None);
    t.type_coin(CoinField::Gold, "0");
    t.type_coin(CoinField::Silver, "2");
    let out = t.commit_coin(CoinField::Copper);
    assert!(matches!(&out[0], SocialAction::Post(n) if n.text_ref().unwrap().token == 0x0619_5A09));
    assert_eq!(
        t.typed(),
        &s(&["0", "2", ""]).try_into().unwrap() as &[String; 3]
    );
    assert_eq!(
        out.last(),
        Some(&SocialAction::Intent(SocialIntent::OfferTrade {
            window: 1,
            item: None,
            quantity: 0,
            coin: 200
        }))
    );
    // After the trade ended on the peer's side, cancel only hides.
    t.closed(1);
    assert_eq!(t.cancel(), None);
}

#[test]
fn commit_tests_and_clears_only_the_committed_entry() {
    // §6.3, SP32: only the committed entry is tested; an entry that is not a number
    // elsewhere is not cleared and counts as 0 in the amounts sent.
    let mut t = Trade::new(values());
    t.set_wallet(1_000_000);
    t.open(1, None);
    t.type_coin(CoinField::Gold, "1");
    t.type_coin(CoinField::Silver, "x");
    let out = t.commit_coin(CoinField::Gold);
    assert_eq!(
        out,
        vec![SocialAction::Intent(SocialIntent::OfferTrade {
            window: 1,
            item: None,
            quantity: 0,
            coin: 100_000
        })]
    );
    assert_eq!(t.typed()[1], "x", "another entry is not tested");
    // Committing the silver entry tests it: not a number, cleared, then sent.
    let out = t.commit_coin(CoinField::Silver);
    assert!(matches!(&out[0], SocialAction::Post(n) if n.text_ref().unwrap().token == 0x085F_80B9));
    assert_eq!(t.typed()[1], "");
    assert!(matches!(
        out.last(),
        Some(SocialAction::Intent(SocialIntent::OfferTrade {
            coin: 100_000,
            ..
        }))
    ));
}

#[test]
fn level_templates_sp28() {
    use tessera_ui::game::social::{fellowship, invite};
    let f = fellowship::level_text(42);
    assert_eq!((f.table, f.token), (T::UI, 0x070F_9F3C));
    assert_eq!(f.arg(A::FELLOW_LEVEL), Some(&Arg::Int(42)));
    let i = invite::level_text(7);
    assert_eq!((i.table, i.token), (T::UI, 0x038B_9CDC));
    assert_eq!(i.arg(A::LEVEL), Some(&Arg::Int(7)));
    // Without the install's tables the level is the plain number.
    assert_eq!(f.render(&mut NoTables), "42");
    assert_eq!(i.render(&mut Levels), "Level 7");
}

#[test]
fn trade_entries_are_not_drag_sources() {
    let vocab = sv();
    let mut ui = trade_ui();
    let mut w = TradeWindow::open(&mut ui, &vocab, CharVocab::default()).unwrap();
    let mut t = Trade::new(values());
    t.open(1, None);
    t.set_offer(1, Side::Peer, vec![entry("Rope", Some(70))], 0);
    t.set_offer(1, Side::Own, vec![entry("Lamp", Some(71))], 0);
    w.refresh(&mut ui, &t, &mut NoTables);
    let dragable = v().pids.dragable;
    for e in [w.entries.0[0], w.entries.1[0]] {
        let icon = tessera_ui::game::find_below(&ui, e, vocab.ids.item_icon).unwrap();
        assert_eq!(ui.property(icon, dragable), Some(&b(false)));
        assert!(ui.examines(icon), "an item entry keeps its examination");
    }
}

#[test]
fn trade_lists_shrink_and_grow_in_batches() {
    let vocab = sv();
    let mut ui = trade_ui();
    let mut w = TradeWindow::open(&mut ui, &vocab, CharVocab::default()).unwrap();
    let mut t = Trade::new(values());
    t.open(1, None);
    let many: Vec<TradeEntry> = (0..32).map(|n| entry(&format!("e{n}"), Some(n))).collect();
    t.set_offer(1, Side::Peer, many.clone(), 0);
    w.refresh(&mut ui, &t, &mut NoTables);
    assert_eq!(w.entries.1.len(), 32);
    let first = w.entries.1[..3].to_vec();
    t.set_offer(1, Side::Peer, many[..3].to_vec(), 0);
    w.refresh(&mut ui, &t, &mut NoTables);
    assert_eq!(w.entries.1, first, "the first rows are kept");
    let list = tessera_ui::game::find_below(&ui, w.root, vocab.ids.trade_peer_list).unwrap();
    assert_eq!(ui.list_entries(list), first);
    t.set_offer(1, Side::Peer, many, 0);
    w.refresh(&mut ui, &t, &mut NoTables);
    assert_eq!(ui.list_entries(list), w.entries.1);
    assert_eq!(w.entries.1.len(), 32);
    let name = tessera_ui::game::find_below(&ui, w.entries.1[31], vocab.ids.item_name).unwrap();
    assert_eq!(ui.text_of(name), "e31");
}

#[test]
fn friends_rows_shrink_and_grow_in_batches() {
    let mut ui = ui_social(social_layout());
    let vocab = sv();
    let panel = SocialPanel::open(&mut ui, &vocab).unwrap();
    let mut page = FriendsPage::bind(&mut ui, &vocab, panel.root).unwrap();
    let list = page.list.unwrap();
    let mut m = FriendsModel::default();
    let names: Vec<String> = (0..300).map(|n| format!("f{n:03}")).collect();
    let all: Vec<FriendEntry> = names
        .iter()
        .map(|n| FriendEntry {
            name: n.clone(),
            online: true,
        })
        .collect();
    m.set_list(&all);
    page.refresh(&mut ui, &m, &mut NoTables);
    assert_eq!(page.rows.len(), 300);
    m.select(Some("f299"));
    page.refresh(&mut ui, &m, &mut NoTables);
    m.set_list(&all[..10]);
    page.refresh(&mut ui, &m, &mut NoTables);
    assert_eq!(page.rows.len(), 10);
    assert_eq!(ui.list_entries(list), page.rows);
    assert_eq!(ui.list_selected(list), None, "the selected row went");
    m.set_list(&[]);
    page.refresh(&mut ui, &m, &mut NoTables);
    assert!(ui.list_entries(list).is_empty());
    m.set_list(&all);
    page.refresh(&mut ui, &m, &mut NoTables);
    assert_eq!(ui.list_entries(list), page.rows);
    let last =
        tessera_ui::game::find_below(&ui, page.rows[299], vocab.ids.friend_row_name).unwrap();
    assert_eq!(ui.text_of(last), "f299");
}

#[test]
fn social_close_button_and_last_page() {
    let mut ui = ui_social(social_layout());
    let vocab = sv();
    let i = vocab.ids;
    let mut panel = SocialPanel::open(&mut ui, &vocab).unwrap();
    assert!(panel.command(&mut ui, "ToggleSocialPanel"));
    assert_eq!(panel.current(&ui), Some(SocialPage::Grouping));
    // A tab click, then a hide that is not a command.
    let pages = panel.pages.unwrap();
    ui.panel_open(pages, i.friends_page_outer, i.friends_tab);
    ui.set_shown(panel.root, false);
    panel.command(&mut ui, "ToggleSocialPanel");
    assert_eq!(
        panel.current(&ui),
        Some(SocialPage::Friends),
        "§3.1: the last page"
    );
    // The close button hides the panel and the page is remembered.
    ui.panel_open(pages, i.raid_page, i.raid_tab);
    let close = tessera_ui::game::find_below(&ui, panel.root, i.social_close).unwrap();
    let ms = ui.vocab().msgs;
    let ev = msg(&ui, close, ms.button_click, Some("SelectLeft"));
    assert!(panel.handle_event(&mut ui, &ev));
    assert!(!ui.is_visible(panel.root));
    assert_eq!(panel.state.last, Some(SocialPage::Raid));
    panel.command(&mut ui, "ToggleSocialPanel");
    assert_eq!(panel.current(&ui), Some(SocialPage::Raid));
    // Other events are not the panel's.
    let ev = msg(&ui, pages, ms.button_click, Some("SelectLeft"));
    assert!(!panel.handle_event(&mut ui, &ev));
}

#[test]
fn select_fellow_one_without_the_player_entity_does_nothing() {
    let mut f = Fellowship::new("Tam", None);
    f.changed(&[mem("Tam", None, true), mem("Brin", Some(A_), false)]);
    assert_eq!(f.select_fellow(1), None);
    let f = Fellowship::new("Tam", Some(P));
    assert_eq!(
        f.select_fellow(1),
        Some(SocialAction::Intent(SocialIntent::SelectTarget {
            target: Some(P)
        }))
    );
}

#[test]
fn member_data_is_evicted_not_refused() {
    use tessera_ui::game::social::fellowship::MAX_TRACKED;
    let mut f = Fellowship::new("Tam", Some(P));
    f.changed(&[mem("Tam", Some(P), true), mem("Brin", Some(A_), false)]);
    f.level(A_, 7);
    f.presence(A_, false);
    // Many other entities fill the store.
    for e in 1000..1000 + MAX_TRACKED as u64 + 10 {
        f.vital(e, VitalKind::Morale, 1.0, 2.0);
        f.presence(e, false);
    }
    // The member's data survived; a new member's entity is still tracked.
    assert_eq!(f.rows()[0].level, Some(7));
    assert!(
        matches!(f.click_row(0), Some(SocialAction::Post(_))),
        "despawned"
    );
    let fresh = 900_000;
    f.changed(&[
        mem("Tam", Some(P), true),
        mem("Brin", Some(A_), false),
        mem("Cael", Some(fresh), false),
    ]);
    f.level(fresh, 30);
    f.presence(fresh, false);
    assert_eq!(f.rows()[1].level, Some(30));
    assert!(
        matches!(f.click_row(1), Some(SocialAction::Post(_))),
        "a despawn after the store filled is recorded"
    );
    f.presence(fresh, true);
    assert!(matches!(f.click_row(1), Some(SocialAction::Intent(_))));
    // Vitals by name are evicted the same way.
    f.fellow_vital("Cael", VitalKind::Power, 5.0, 10.0);
    for n in 0..MAX_TRACKED + 10 {
        f.fellow_vital(&format!("x{n}"), VitalKind::Morale, 1.0, 1.0);
    }
    f.fellow_vital("Dun", VitalKind::Morale, 1.0, 4.0);
    assert!(f.rows()[1].power.is_some());
}

#[test]
fn dialog_answers_post_a_line_for_an_invalid_name() {
    assert_eq!(DialogPurpose::AddFriend.answer("   "), None);
    assert_eq!(
        DialogPurpose::AddIgnore.answer(" Kim "),
        Some(SocialAction::Intent(SocialIntent::Ignore {
            name: "Kim".into()
        }))
    );
    // 40 characters of a two-byte script: within a 64-character dialog, over 64 bytes.
    let long: String = std::iter::repeat_n('é', 40).collect();
    assert!(matches!(
        DialogPurpose::AddFriend.answer(&long),
        Some(SocialAction::Post(n)) if n.mask == 0x1
    ));
}

#[test]
fn grouped_sort_with_many_fellows() {
    let mut m = FriendsModel::default();
    m.set_list(&friends(&[
        ("bo", true),
        ("Al", true),
        ("cy", false),
        ("Di", true),
    ]));
    let fellows: Vec<String> = (0..2000)
        .map(|n| format!("x{n}"))
        .chain(["BO".into()])
        .collect();
    m.set_fellows(&fellows);
    m.click_header(FriendColumn::Grouped);
    // Only the first MAX_MEMBERS names are kept, so "BO" is not a fellow here.
    assert_eq!(order(&m), s(&["Al", "bo", "cy", "Di"]));
    m.set_fellows(&s(&["BO", "cy"]));
    assert_eq!(
        order(&m),
        s(&["Al", "cy", "Di", "bo"]),
        "cy is offline: not grouped"
    );
    m.click_header(FriendColumn::Grouped);
    assert_eq!(order(&m), s(&["bo", "Di", "cy", "Al"]));
}
