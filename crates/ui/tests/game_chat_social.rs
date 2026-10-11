// SPDX-License-Identifier: GPL-3.0-or-later
//! The P4 social chat of `docs/specs/chat-window.md` revision 4 (§15, Tessera rules §15.10
//! CT1–CT9): worked examples §11.9–§11.14 and the behaviour invariants CW-I21–CW-I30 on
//! synthetic data. Names and command words are invented; tokens are the spec's.

mod ui_common;

use proptest::prelude::*;
use tessera_formats::did::Did;
use tessera_formats::string_info::{StringInfo, StringSource};
use tessera_ui::Ui;
use tessera_ui::game::GameVocab;
use tessera_ui::game::chat::im::{ImDelivery, ImPids, ImSessions};
use tessera_ui::game::chat::notice::{
    self as n, Arg, NoTables, Notice, NoticeText, TextRef, TextSource, args as A, fallback_text,
    name_equal, tables as T,
};
use tessera_ui::game::chat::social::{
    Channel, ChatOutcome, ChatSend, ImLine, SLOTS, SessionLine, SocialChat, slot_mask,
};
use tessera_ui::game::chat::{
    ChannelModes, Chat, ChatConfig, CommandOutcome, ModeKind, NoHistory, TabConfig, WindowConfig,
};
use ui_common::*;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_string()).collect()
}

/// The (table, token) of a notice.
fn tok(n: &Notice) -> (u32, u32) {
    let r = n.text_ref().expect("a table line");
    (r.table, r.token)
}

/// A text argument's value.
fn arg(n: &Notice, id: u32) -> String {
    match n.text_ref().and_then(|r| r.arg(id)) {
        Some(Arg::Text(t)) => t.clone(),
        Some(Arg::Int(i)) => i.to_string(),
        other => panic!("argument {id:#x}: {other:?}"),
    }
}

fn line(o: SessionLine) -> (Notice, Option<ImLine>) {
    match o {
        SessionLine::Line { notice, im } => (notice, im),
        other => panic!("not a line: {other:?}"),
    }
}

fn one(o: &ChatOutcome) -> &Notice {
    assert_eq!(o.lines.len(), 1, "{o:?}");
    assert!(o.send.is_none(), "{o:?}");
    &o.lines[0]
}

fn chat_ref(token: u32) -> (u32, u32) {
    (T::CHAT, token)
}

// ----- §11.9 a tell, a reply, the instant-message tab ------------------------------------

#[test]
fn example_11_9_tell_reply_and_own_echo() {
    let mut sc = SocialChat::new("Ana", true);
    // 1. An incoming tell.
    let (l, im) = line(sc.session_line(Channel::Tell { peer: "Bram" }, Some("Bram"), "hi"));
    assert_eq!(tok(&l), chat_ref(0x0BCA_09CC));
    assert_eq!(l.mask, 0x20);
    assert_eq!(arg(&l, A::NAME), "Bram");
    assert_eq!(arg(&l, A::TEXT), "hi");
    let im = im.unwrap();
    assert_eq!((im.partner.as_str(), im.speaker.as_str()), ("Bram", "Bram"));
    assert_eq!(sc.targets().reply.as_deref(), Some("Bram"));
    // 2. `T␠Bram␠thanks` from the `Tell` mode with target R.
    let o = sc.tell(&s(&["Bram", "thanks"]));
    assert_eq!(
        o.send,
        Some(ChatSend::Tell {
            peer: "Bram".into(),
            text: "thanks".into()
        })
    );
    assert_eq!(sc.targets().retell.as_deref(), Some("Bram"));
    // Through `reply` the same command, T unchanged.
    let mut sc2 = sc.clone();
    sc2.retell(""); // usage only
    let o = sc2.reply("thanks");
    assert_eq!(
        o.send,
        Some(ChatSend::Tell {
            peer: "Bram".into(),
            text: "thanks".into()
        })
    );
    // 3. The echo of the player's own tell (CW-I24: own never changes R).
    let (l, im) = line(sc.session_line(Channel::Tell { peer: "Bram" }, Some("Ana"), "thanks"));
    assert_eq!(tok(&l), chat_ref(0x0AD7_4A23));
    assert_eq!(arg(&l, A::RECIPIENT), "Bram");
    assert_eq!(l.mask, 0x20);
    let im = im.unwrap();
    assert_eq!((im.partner.as_str(), im.speaker.as_str()), ("Bram", "Ana"));
    assert_eq!(sc.targets().reply.as_deref(), Some("Bram"));
    // 4. The plain `Tell` mode: `T␠␠Cara hello` tokenizes to `Cara`, `hello`.
    let o = sc.tell(&s(&["Cara", "hello"]));
    assert_eq!(
        o.send,
        Some(ChatSend::Tell {
            peer: "Cara".into(),
            text: "hello".into()
        })
    );
    // 5. A `System` line is unchanged (§10 item 2).
    assert_eq!(
        sc.session_line(Channel::System, None, "no such player"),
        SessionLine::Unchanged
    );
    // A tell to oneself on the test server comes back as incoming (CT1 known difference).
    let (l, _) = line(sc.session_line(Channel::Tell { peer: "Ana" }, Some("Ana"), "me"));
    assert_eq!(tok(&l), chat_ref(0x0BCA_09CC));
}

#[test]
fn example_11_10_tell_refusals() {
    let mut sc = SocialChat::new("Ana", true);
    let o = sc.tell(&s(&["Bram"]));
    assert_eq!(one(&o).text, NoticeText::Usage("tell".into()));
    assert_eq!(one(&o).mask, 0x8);
    let o = sc.reply("ok");
    assert_eq!(tok(one(&o)), chat_ref(0x00CC_89CC));
    assert_eq!(one(&o).mask, 0x21);
    let o = sc.retell("ok");
    assert_eq!(tok(one(&o)), chat_ref(0x0811_EFD4));
    assert_eq!(one(&o).mask, 0x21);
    let e = sc.chat_mode_reply().unwrap_err();
    assert_eq!((tok(&e), e.mask), (chat_ref(0x00CC_89CC), 0x21));
    // An invalid recipient and over-long text are refused with type 1 lines.
    let o = sc.tell(&s(&["Br\u{7}am", "x"]));
    assert_eq!(one(&o).mask, n::ERROR);
    let o = sc.tell(&["Bram".to_string(), "x".repeat(1025)]);
    assert_eq!(one(&o).mask, n::ERROR);
    assert!(
        sc.targets().retell.is_none(),
        "CW-I25: T only on a sent tell"
    );
    // Empty reply text is the usage.
    assert_eq!(one(&sc.reply("")).text, NoticeText::Usage("reply".into()));
}

// ----- §11.11 user channels ---------------------------------------------------------------

#[test]
fn example_11_11_user_channels() {
    let mut sc = SocialChat::new("Ana", true);
    assert_eq!(
        sc.join(&s(&["Lore"])).send,
        Some(ChatSend::Join {
            name: "Lore".into()
        })
    );
    assert!(
        sc.slots().iter().all(Option::is_none),
        "nothing until ChannelJoined"
    );
    // 1.
    let j = sc.channel_joined("Lore").unwrap();
    assert_eq!((tok(&j), j.mask), (chat_ref(0x09A9_9A08), 0x8));
    assert_eq!(arg(&j, A::SLOT), "1");
    // 2.
    assert_eq!(
        sc.slot_say(1, "hello").send,
        Some(ChatSend::Named {
            name: "Lore".into(),
            text: "hello".into()
        })
    );
    let (l, im) = line(sc.session_line(Channel::Named { name: "Lore" }, Some("Ana"), "hello"));
    assert!(im.is_none());
    assert_eq!((tok(&l), l.mask), (chat_ref(0x009D_15CC), 0x0800_0000));
    assert_eq!(arg(&l, A::CHANNEL_ECHO), "Lore");
    let (l, _) = line(sc.session_line(Channel::Named { name: "lore" }, Some("Bram"), "yo"));
    assert_eq!((tok(&l), l.mask), (chat_ref(0x063C_924C), 0x0800_0000));
    assert_eq!(arg(&l, A::CHANNEL), "Lore", "the slot's spelling");
    assert_eq!(arg(&l, A::SPEAKER), "Bram");
    // A channel in no slot is dropped.
    assert_eq!(
        sc.session_line(Channel::Named { name: "Other" }, Some("Bram"), "x"),
        SessionLine::Dropped
    );
    // 3.
    let o = sc.join(&s(&["lore"]));
    assert_eq!(tok(one(&o)), chat_ref(0x0754_8D0C));
    assert_eq!(arg(one(&o), A::CHANNEL_NAME), "lore");
    // 4.
    assert_eq!(
        tok(one(&sc.join(&s(&["Lore", "pw"])))),
        chat_ref(0x08DC_4764)
    );
    // 5.
    let o = sc.slot_say(3, "x");
    assert_eq!((tok(one(&o)), one(&o).mask), (chat_ref(0x0E2A_7CE8), 0x8));
    assert_eq!(arg(one(&o), A::COUNT), "3");
    // 6.
    let list = sc.channels();
    assert_eq!(list.len(), 9);
    assert_eq!((tok(&list[0]), list[0].mask), (chat_ref(0x09AA_EAC2), 0x8));
    assert_eq!(
        (tok(&list[1]), list[1].mask),
        (chat_ref(0x000E_E4F4), 0x0800_0000)
    );
    match list[1].text_ref().unwrap().arg(A::PASSWORD_PART) {
        Some(Arg::Ref(r)) => assert_eq!((r.table, r.token), chat_ref(0x02E8_19D4)),
        other => panic!("{other:?}"),
    }
    assert_eq!(tok(&list[2]), chat_ref(0x0EBE_4334));
    assert_eq!(list[5].mask, 0x40_0000_0000, "slot 5 is type 39");
    for (k, l) in list.iter().enumerate().skip(1) {
        assert_eq!(l.mask, slot_mask(k), "CW-I21");
    }
    // 7.
    for i in 2..=SLOTS {
        sc.channel_joined(&format!("C{i}")).unwrap();
    }
    let o = sc.join(&s(&["Nine"]));
    assert_eq!(tok(one(&o)), chat_ref(0x002A_AA94));
    assert_eq!(arg(one(&o), A::COUNT), "8");
    // 8.
    assert_eq!(
        sc.part(&s(&["Lore"])).send,
        Some(ChatSend::Leave {
            name: "Lore".into()
        })
    );
    assert_eq!(sc.slot_of("Lore"), Some(1), "held until ChannelLeft");
    let l = sc.channel_left("Lore").unwrap();
    assert_eq!((tok(&l), l.mask), (chat_ref(0x03B0_95F8), 0x8));
    assert_eq!(tok(one(&sc.part(&s(&["Lore"])))), chat_ref(0x0DF4_808C));
    // 9. Without user chat (the test server before minor 2).
    sc.set_user_chat(false);
    let o = sc.join(&s(&["Lore"]));
    assert_eq!((tok(one(&o)), one(&o).mask), (chat_ref(0x0406_8445), 0x8));
    let o = sc.slot_say(2, "x");
    assert_eq!((tok(one(&o)), one(&o).mask), (chat_ref(0x0406_8445), 0x1));
}

#[test]
fn join_checks_and_chsay() {
    let mut sc = SocialChat::new("Ana", true);
    sc.set_channel_command_names(s(&["sayword"]));
    assert_eq!(one(&sc.join(&[])).text, NoticeText::Usage("join".into()));
    let long = "x".repeat(51);
    let o = sc.join(&[long]);
    assert_eq!(tok(one(&o)), chat_ref(0x0283_2AE7));
    assert_eq!(arg(one(&o), A::LENGTH), "51");
    assert_eq!(arg(one(&o), A::LIMIT), "50");
    assert_eq!(tok(one(&sc.join(&s(&["a\u{1}b"])))), chat_ref(0x0AC8_421C));
    assert_eq!(tok(one(&sc.join(&s(&["SAYWORD"])))), chat_ref(0x0191_9744));
    assert_eq!(tok(one(&sc.join(&s(&["Ch3"])))), chat_ref(0x0191_9744));
    assert_eq!(tok(one(&sc.part(&s(&["tell"])))), chat_ref(0x03C0_43D4));
    // chsay.
    assert_eq!(one(&sc.chsay(&[])).text, NoticeText::Usage("chsay".into()));
    assert_eq!(
        tok(one(&sc.chsay(&s(&["Lore", "x"])))),
        chat_ref(0x02E3_2555)
    );
    sc.channel_joined("Lore");
    assert_eq!(
        one(&sc.chsay(&s(&["lore"]))).text,
        NoticeText::Usage("chsay".into())
    );
    assert_eq!(
        sc.chsay(&s(&["lore", "a", "b"])).send,
        Some(ChatSend::Named {
            name: "Lore".into(),
            text: "a b".into()
        })
    );
    // A second ChannelJoined of a held name changes nothing.
    assert!(sc.channel_joined("LORE").is_none());
    // Disconnected frees the slots without lines.
    sc.disconnected();
    assert!(sc.slots().iter().all(Option::is_none));
}

// ----- §11.12 fellowship ------------------------------------------------------------------

#[test]
fn example_11_12_fellowship() {
    let mut sc = SocialChat::new("Ana", true);
    let o = sc.fellowship("hi");
    assert_eq!(o.lines[0].text_ref().unwrap().table, T::FELLOWSHIP);
    assert_eq!(o.lines[0].text_ref().unwrap().token, 0x0635_8717);
    assert_eq!(o.lines[0].mask, 0x1);
    assert!(o.send.is_none(), "CW-I29");
    sc.set_fellowship(true);
    assert_eq!(
        sc.fellowship("hi").send,
        Some(ChatSend::Fellowship { text: "hi".into() })
    );
    let (l, _) = line(sc.session_line(Channel::Fellowship, Some("Ana"), "hi"));
    assert_eq!((tok(&l), l.mask), (chat_ref(0x04A3_E744), 0x400));
    let (l, _) = line(sc.session_line(Channel::Fellowship, Some("Bram"), "yo"));
    assert_eq!((tok(&l), l.mask), (chat_ref(0x05D2_0D84), 0x400));
    assert_eq!(arg(&l, A::SPEAKER), "Bram");
    // Without a sender: the text as a type 8 line.
    let (l, _) = line(sc.session_line(Channel::Fellowship, None, "notice"));
    assert_eq!(
        (l.text.clone(), l.mask),
        (NoticeText::Own("notice".into()), 0x80)
    );
    assert_eq!(
        one(&sc.fellowship("")).text,
        NoticeText::Usage("fellowship".into())
    );
}

// ----- §11.13 ignore filter ---------------------------------------------------------------

#[test]
fn example_11_13_ignore_filter() {
    let mut sc = SocialChat::new("Ana", true);
    sc.channel_joined("Lore");
    sc.set_ignore_list(&s(&["Cara"]));
    assert_eq!(
        sc.session_line(Channel::Say, Some("cara"), "x"),
        SessionLine::Dropped
    );
    assert_eq!(
        sc.session_line(Channel::Tell { peer: "Cara" }, Some("Cara"), "x"),
        SessionLine::Dropped
    );
    assert!(sc.targets().reply.is_none(), "CW-I23: R unchanged");
    assert!(sc.targets().senders.names().is_empty());
    assert_eq!(
        sc.session_line(Channel::Named { name: "Lore" }, Some("CARA"), "x"),
        SessionLine::Dropped
    );
    assert_eq!(
        sc.session_line(Channel::System, None, "x"),
        SessionLine::Unchanged
    );
    // The player's own tell to Cara goes out and its echo shows.
    assert!(sc.tell(&s(&["Cara", "x"])).send.is_some());
    let (l, _) = line(sc.session_line(Channel::Tell { peer: "Cara" }, Some("Ana"), "x"));
    assert_eq!(tok(&l), chat_ref(0x0AD7_4A23));
    // A tell without a sender is spoken by its peer.
    assert_eq!(
        sc.session_line(Channel::Tell { peer: "cara" }, None, "x"),
        SessionLine::Dropped
    );
    // The ignore list is cleared on Disconnected.
    sc.disconnected();
    assert!(sc.ignored().is_empty());
    assert!(!sc.is_ignored("cara"), "the filter's lookup is cleared too");
    assert_eq!(
        sc.session_line(Channel::Say, Some("Cara"), "x"),
        SessionLine::Unchanged
    );
}

// ----- §11.14 friend notices --------------------------------------------------------------

#[test]
fn example_11_14_friend_notices() {
    let mut sc = SocialChat::new("Ana", true);
    let f = |v: &[(&str, bool)]| -> Vec<(String, bool)> {
        v.iter().map(|(a, b)| ((*a).to_string(), *b)).collect()
    };
    assert!(
        sc.friend_list(&f(&[("Bram", false), ("Cara", true)]))
            .is_empty()
    );
    let l = sc.friend_list(&f(&[("Bram", true), ("Cara", true), ("Dain", true)]));
    assert_eq!(l.len(), 1);
    assert_eq!((tok(&l[0]), l[0].mask), (chat_ref(0x0A29_AFC5), 0x8));
    assert_eq!(arg(&l[0], A::NAME), "Bram");
    let l = sc.friend_list(&f(&[("cara", false)]));
    assert_eq!(l.len(), 1);
    assert_eq!(tok(&l[0]), chat_ref(0x0105_38E5));
    assert_eq!(arg(&l[0], A::NAME), "cara", "the new list's spelling");
    // CW-I28: at most one notice per name.
    let l = sc.friend_list(&f(&[("Cara", true), ("CARA", true)]));
    assert_eq!(l.len(), 1);
    // The first list after a world entry posts none.
    sc.world_entered("Ana");
    assert!(sc.friend_list(&f(&[("Cara", false)])).is_empty());
}

// ----- CT3 recent names -------------------------------------------------------------------

#[test]
fn recent_names_are_distinct_newest_first_and_bounded() {
    let mut sc = SocialChat::new("Ana", true);
    for i in 0..12 {
        sc.session_line(
            Channel::Tell {
                peer: &format!("P{i}"),
            },
            None,
            "x",
        );
    }
    sc.session_line(Channel::Tell { peer: "p5" }, None, "x");
    let names = sc.targets().senders.names();
    assert_eq!(names.len(), 10);
    assert_eq!(names[0], "p5");
    assert_eq!(names.iter().filter(|x| name_equal(x, "P5")).count(), 1);
    assert_eq!(sc.targets().reply.as_deref(), Some("p5"));
}

// ----- CT12 rendering ---------------------------------------------------------------------

struct Tables;

impl TextSource for Tables {
    fn render(&mut self, table: u32, token: u32, args: &[(u32, String)]) -> Option<String> {
        (table == T::CHAT && token == 0x0BCA_09CC).then(|| {
            let get = |id| args.iter().find(|(a, _)| *a == id).unwrap().1.clone();
            format!("<{}> {}", get(A::NAME), get(A::TEXT))
        })
    }
}

#[test]
fn notices_render_from_tables_or_tessera_wording() {
    let r = TextRef::new(T::CHAT, 0x0BCA_09CC)
        .text(A::NAME, "Bram")
        .text(A::TEXT, "<b>hi</b>");
    assert_eq!(r.render(&mut Tables), "<Bram> <b>hi</b>");
    let own = r.render(&mut NoTables);
    assert!(own.contains("Bram") && own.contains("<b>hi</b>"), "{own}");
    // Nested references render first.
    let list = TextRef::new(T::CHAT, 0x000E_E4F4)
        .int(A::SLOT, 1)
        .text(A::CHANNEL_NAME, "Lore")
        .nested(A::PASSWORD_PART, TextRef::new(T::CHAT, 0x02E8_19D4));
    assert!(list.render(&mut NoTables).contains("Lore"));
    // Unknown entries show their arguments.
    assert_eq!(
        fallback_text(1, 2, &[(3, "a".into()), (4, "b".into())]),
        "a b"
    );
    let u = Notice::usage("tell");
    assert_eq!(
        u.render(&mut NoTables, &|c| format!("usage of {c}")),
        "usage of tell"
    );
}

// ----- CT6 the instant-message session model ----------------------------------------------

fn im(partner: &str, speaker: &str) -> ImLine {
    ImLine {
        partner: partner.into(),
        speaker: speaker.into(),
        text: "t".into(),
    }
}

fn names(m: &ImSessions) -> Vec<Option<String>> {
    m.rows().iter().map(|r| r.name.clone()).collect()
}

#[test]
fn im_sessions_order_rename_limits_cw_i26_i27() {
    let mut m = ImSessions::new(4, 2, true);
    assert_eq!(
        m.deliver(&im("Bram", "Bram"), "1").0,
        ImDelivery::Row {
            index: 1,
            created: true
        }
    );
    m.deliver(&im("Ana2", "Ana2"), "2");
    m.deliver(&im("Cara", "Ana"), "3");
    assert_eq!(
        names(&m),
        vec![
            None,
            Some("Ana2".into()),
            Some("Bram".into()),
            Some("Cara".into())
        ]
    );
    assert_eq!(m.shown(), 0, "auto-created rows are not shown");
    assert!(m.rows()[1].unread);
    // Full (generic included): the line is not shown in this tab.
    assert_eq!(m.deliver(&im("Dain", "Dain"), "4").0, ImDelivery::NotShown);
    // A case-only difference renames the row and keeps the order (CW-I26).
    let (d, renamed) = m.deliver(&im("bram", "bram"), "5");
    assert_eq!(
        d,
        ImDelivery::Row {
            index: 3,
            created: false
        }
    );
    assert!(renamed.is_none(), "not the shown row");
    assert_eq!(
        names(&m),
        vec![
            None,
            Some("Ana2".into()),
            Some("Cara".into()),
            Some("bram".into())
        ]
    );
    // Lines per session are capped.
    m.deliver(&im("bram", "Ana"), "6");
    assert_eq!(m.rows()[3].lines.len(), 2);
    assert_eq!(m.rows()[3].lines[1].speaker, "Ana");
    // Showing a row gives the Tell target and clears its unread mark.
    assert_eq!(m.show(3).as_deref(), Some("bram"));
    assert!(!m.rows()[3].unread);
    assert_eq!(m.show(0).as_deref(), Some(""));
    // Close and start.
    assert!(!m.close(0), "never the generic row");
    assert!(m.close(2));
    assert_eq!(
        m.start("").unwrap_err().text_ref().unwrap().token,
        0x0EA6_8129
    );
    assert_eq!(m.start("a\u{1}").unwrap_err().mask, n::ERROR);
    let i = m.start("Dain").unwrap();
    assert_eq!(m.shown(), i);
    assert_eq!(m.rows()[i].name.as_deref(), Some("Dain"));
    assert!(!m.can_start());
    assert!(m.start("Eve").is_err());
    // Auto-creation off: the generic row takes the line.
    let mut g = ImSessions::new(4, 8, false);
    assert_eq!(
        g.deliver(&im("Bram", "Bram"), "1").0,
        ImDelivery::Row {
            index: 0,
            created: false
        }
    );
}

// ----- the window: mode words, ChatModeReply, browsing, the IM tab ------------------------

const CHAT: u32 = 0x22FF_4400;
const W1: u32 = 0x1000_4901;
const TAB_T: u32 = 0x1000_4910;
const LOG_FIELD: u32 = 0x1000_4911;
const ENTRY: u32 = 0x1000_4912;
const OUT: u32 = 0x1000_4913;
const LINE: u32 = 0x1000_4914;
const TABS: u32 = 0x1000_4915;
const IM_TAB_T: u32 = 0x1000_4920;
const IM_LOG: u32 = 0x1000_4921;
const IM_LIST: u32 = 0x1000_4922;
const IM_PARENT: u32 = 0x1000_4923;
const IM_ROW: u32 = 0x1000_4924;
const IM_ROW_LABEL: u32 = 0x1000_4925;
const IM_SLOG: u32 = 0x1000_4926;

fn chat_ui() -> (Ui, Chat) {
    chat_ui_with(IM_SLOG)
}

/// The synthetic chat window; `row_log` is the element ID the session row names for its
/// log (an absent ID makes every session row fail).
fn chat_ui_with(row_log: u32) -> (Ui, Chat) {
    let g = GameVocab::default();
    let ip = ImPids::default();
    let (p, pp, vv, st) = (g.pids, v().pids, v().types, v().states);
    let mut w = typed(W1, g.types.chat_window, 0, 0, 300, 200);
    w = prop(w, p.chat_window_id, int(1));
    w = prop(w, p.chat_primary, b(true));
    w = prop(w, p.chat_tab_element, en(TAB_T));
    w = prop(w, p.chat_im_tab_element, en(IM_TAB_T));
    w = prop(w, p.chat_log_parent, en(LOG_FIELD));
    w = prop(w, p.chat_text_entry_element, en(ENTRY));
    w = prop(w, p.chat_output_text, en(OUT));
    w = child(
        w,
        prop(
            typed(TABS, g.types.chat_tab_list, 0, 0, 300, 23),
            pp.lb_horizontal,
            b(true),
        ),
    );
    w = child(w, field(LOG_FIELD, 0, 23, 300, 157));
    w = child(
        w,
        prop(
            typed(ENTRY, g.types.chat_text_entry, 100, 180, 200, 20),
            pp.text_editable,
            b(true),
        ),
    );
    w = child(w, typed(OUT, vv.text, 18, 180, 81, 20));
    let tab_states = |mut d: tessera_formats::ui_layout::ElementDesc| {
        for s in [st.normal, st.highlight] {
            d = state(d, s, 0, vec![], vec![]);
        }
        d
    };
    let tab = tab_states(child(
        typed(TAB_T, g.types.chat_tab, 0, 0, 100, 23),
        field(g.ids.unread_indicator, 0, 0, 8, 8),
    ));
    let im_tab = tab_states(prop(
        child(
            typed(IM_TAB_T, g.types.im_tab, 0, 0, 100, 23),
            field(g.ids.unread_indicator, 0, 0, 8, 8),
        ),
        ip.tab_im_log,
        en(IM_LOG),
    ));
    let list = |id: u32, rows: i32| {
        prop(
            typed(id, vv.list_box, 0, 0, 300, 64),
            pp.lb_max_rows,
            int(rows),
        )
    };
    let log = child(
        prop(
            typed(g.ids.chat_log_element, g.types.chat_log, 0, 0, 300, 64),
            p.chat_log_text_element,
            en(LINE),
        ),
        list(g.ids.chat_log_listbox, 128),
    );
    let mut im_log = typed(IM_LOG, g.types.chat_log, 0, 0, 300, 150);
    im_log = prop(im_log, ip.log_im_listbox, en(IM_LIST));
    im_log = prop(im_log, ip.log_im_parent, en(IM_PARENT));
    im_log = child(
        im_log,
        prop(
            prop(list(IM_LIST, 3), pp.lb_click_select, b(true)),
            ip.log_im_text,
            en(IM_ROW),
        ),
    );
    im_log = child(im_log, field(IM_PARENT, 0, 0, 200, 150));
    let im_row = prop(
        child(
            child(
                field(IM_ROW, 0, 0, 100, 19),
                typed(IM_ROW_LABEL, vv.text, 0, 0, 100, 19),
            ),
            field(g.ids.unread_indicator, 0, 0, 8, 8),
        ),
        ip.tab_log,
        en(row_log),
    );
    let im_slog = child(
        prop(
            typed(IM_SLOG, g.types.chat_log, 0, 0, 200, 150),
            p.chat_log_text_element,
            en(LINE),
        ),
        list(g.ids.chat_log_listbox, 4),
    );
    let line = typed(LINE, vv.text, 0, 0, 300, 16);
    let mut ui = ui_one(
        CHAT,
        300,
        200,
        vec![w, tab, im_tab, log, im_log, im_row, im_slog, line],
        1024,
        768,
    );
    g.register_classes(&mut ui);
    let mut chat = Chat::new(g, Did(CHAT));
    chat.modes = ChannelModes::new(
        &[
            ("S".into(), 5, ModeKind::Plain),
            ("T".into(), 6, ModeKind::Plain),
            ("R".into(), 6, ModeKind::Reply),
            ("RT".into(), 6, ModeKind::TellAgain),
            ("F".into(), 11, ModeKind::Plain),
        ],
        &chat.setup,
    );
    chat.texts.no_reply_target = "no reply".into();
    let cfg = ChatConfig {
        windows: vec![WindowConfig {
            id: 1,
            visible: true,
            tabs: vec![TabConfig {
                name: Some(StringInfo {
                    source: StringSource::Literal("General".into()),
                    args: None,
                }),
                mask: 0x0000_7FF9_F8FE_FFFD,
            }],
        }],
    };
    chat.open(&mut ui, &cfg).unwrap();
    (ui, chat)
}

fn type_text(ui: &mut Ui, chat: &mut Chat, t: &str) {
    for ch in t.chars() {
        chat.text_input(ui, ch, 0.0);
    }
}

#[test]
fn mode_words_take_reply_and_retell_targets() {
    let (mut ui, mut chat) = chat_ui();
    let mut sc = SocialChat::new("Ana", true);
    sc.session_line(Channel::Tell { peer: "Bram" }, Some("Bram"), "hi");
    sc.tell(&s(&["Cara", "x"]));
    chat.set_tell_targets(sc.targets());
    let mut h = NoHistory;
    let entry = chat.windows[0].entry.unwrap();
    chat.command(&mut ui, "AcceptInput", false, false, &mut h);
    // The reply word: `Tell` with R.
    type_text(&mut ui, &mut chat, "/R ");
    assert_eq!((chat.mode, chat.target.as_str()), (6, "Bram"));
    assert_eq!(ui.text_of(entry), "");
    type_text(&mut ui, &mut chat, "thanks");
    let CommandOutcome::Submitted(l) = chat.command(&mut ui, "AcceptInput", false, false, &mut h)
    else {
        panic!("no submit")
    };
    assert_eq!(
        (l.word.as_str(), l.target.as_str(), l.text.as_str()),
        ("T", "Bram", "thanks")
    );
    // The retell word: `Tell` with T.
    chat.command(&mut ui, "AcceptInput", false, false, &mut h);
    type_text(&mut ui, &mut chat, "/RT ");
    assert_eq!(chat.target, "Cara");
    // The plain tell word: an empty target (CW44).
    chat.command(&mut ui, "EscapeKey", false, false, &mut h);
    chat.command(&mut ui, "AcceptInput", false, false, &mut h);
    type_text(&mut ui, &mut chat, "/T ");
    assert_eq!((chat.mode, chat.target.as_str()), (6, ""));
}

#[test]
fn chat_mode_reply_and_browsing() {
    let (mut ui, mut chat) = chat_ui();
    let mut h = NoHistory;
    // Without R: the no-reply line once in tab 1 (mask 0x21).
    assert_eq!(
        chat.command(&mut ui, "ChatModeReply", false, false, &mut h),
        CommandOutcome::Handled
    );
    let list = chat.windows[0].tabs[0].list.unwrap();
    assert_eq!(ui.list_entries(list).len(), 1);
    // With R: the entry opens in the `Tell` mode with R.
    let mut sc = SocialChat::new("Ana", true);
    for p in ["Bram", "Cara", "Dain"] {
        sc.session_line(Channel::Tell { peer: p }, None, "x");
    }
    sc.tell(&s(&["Eve", "x"]));
    chat.set_tell_targets(sc.targets());
    chat.command(&mut ui, "ChatModeReply", false, false, &mut h);
    assert!(chat.entry_focused(&ui));
    assert_eq!((chat.mode, chat.target.as_str()), (6, "Dain"));
    assert_eq!(ui.list_entries(list).len(), 1, "no line");
    // Shift + line up walks the senders (newest first), clamped.
    for want in ["Dain", "Cara", "Bram", "Bram"] {
        chat.command(&mut ui, "CursorPreviousLine", true, false, &mut h);
        assert_eq!(chat.target, want);
    }
    chat.command(&mut ui, "CursorNextLine", true, false, &mut h);
    assert_eq!(chat.target, "Cara");
    // Ctrl browses the recipients.
    chat.command(&mut ui, "CursorPreviousLine", false, true, &mut h);
    assert_eq!(chat.target, "Eve");
    // A changed list resets the index.
    sc.session_line(Channel::Tell { peer: "Finn" }, None, "x");
    chat.set_tell_targets(sc.targets());
    chat.command(&mut ui, "CursorPreviousLine", true, false, &mut h);
    assert_eq!(chat.target, "Finn");
}

#[test]
fn instant_message_tab_sessions_on_the_tree() {
    let (mut ui, mut chat) = chat_ui();
    chat.open_im_tab(&mut ui, "Tells", "Everyone", true)
        .unwrap();
    let im = chat.im.clone().unwrap();
    assert_eq!(ui.list_entries(chat.windows[0].tab_list.unwrap()).len(), 2);
    assert!(!ui.is_visible(im.log), "hidden until selected");
    assert_eq!(im.rows.len(), 1);
    assert_eq!(ui.text_of(im.rows[0].label), "Everyone");
    // Opening again does nothing.
    chat.open_im_tab(&mut ui, "Tells", "Everyone", true)
        .unwrap();
    assert_eq!(chat.im.as_ref().unwrap().rows.len(), 1);
    // A tell line: a new row in name order, unread, the line in its own log only.
    let mut sc = SocialChat::new("Ana", true);
    let (notice, line) = line(sc.session_line(Channel::Tell { peer: "Bram" }, None, "hi"));
    let text = notice.render(&mut NoTables, &|c| c.to_string());
    chat.post_line(&mut ui, notice.mask, &text);
    let d = chat
        .deliver_im(&mut ui, line.as_ref().unwrap(), "14:02:07", &mut NoTables)
        .unwrap();
    assert_eq!(
        d,
        ImDelivery::Row {
            index: 1,
            created: true
        }
    );
    let im = chat.im.clone().unwrap();
    assert_eq!(im.rows.len(), 2);
    assert_eq!(ui.text_of(im.rows[1].label), "Bram");
    let slog = im.rows[1].list.unwrap();
    assert_eq!(ui.list_entries(slog).len(), 1);
    let shown = tessera_ui::game::chat::window::line_text(&ui, ui.list_entries(slog)[0]);
    assert!(
        shown.contains("14:02:07") && shown.contains("hi"),
        "{shown}"
    );
    assert!(ui.is_visible(im.rows[1].unread.unwrap()));
    assert!(ui.is_visible(im.unread.unwrap()), "the tab's unread marker");
    assert_eq!(
        ui.list_entries(chat.windows[0].tabs[0].list.unwrap()).len(),
        1,
        "the normal tab got the CT1 line once"
    );
    // A second partner sorts before Bram.
    chat.deliver_im(&mut ui, &im_line("Aldo"), "t", &mut NoTables);
    let im = chat.im.clone().unwrap();
    let order: Vec<String> = ui
        .list_entries(im.sessions_list)
        .iter()
        .map(|k| {
            let r = im.rows.iter().find(|r| r.row == *k).unwrap();
            ui.text_of(r.label)
        })
        .collect();
    assert_eq!(order, s(&["Everyone", "Aldo", "Bram"]));
    // Selecting the tab shows its log; showing a row sets the Tell mode.
    chat.select_im_tab(&mut ui);
    assert!(ui.is_visible(im.log));
    assert!(!ui.is_visible(chat.windows[0].tabs[0].log));
    assert_eq!(chat.im_show(&mut ui, 2).as_deref(), Some("Bram"));
    assert_eq!((chat.mode, chat.target.as_str()), (6, "Bram"));
    let im = chat.im.clone().unwrap();
    assert!(ui.is_visible(im.rows[2].log) && !ui.is_visible(im.rows[0].log));
    assert!(!ui.is_visible(im.rows[2].unread.unwrap()));
    // Full list (3 rows): a further partner is not shown in this tab.
    assert_eq!(
        chat.deliver_im(&mut ui, &im_line("Cara"), "t", &mut NoTables),
        Some(ImDelivery::NotShown)
    );
    // Close the shown row: the generic row is shown.
    assert!(chat.im_close(&mut ui, 2));
    let im = chat.im.clone().unwrap();
    assert_eq!(im.rows.len(), 2);
    assert!(ui.is_visible(im.rows[0].log));
    // Start a session from the menu's prompt.
    let i = chat.im_start(&mut ui, "Cara").unwrap();
    assert_eq!(chat.target, "Cara");
    assert_eq!(chat.im.as_ref().unwrap().model.shown(), i);
    assert!(chat.im_start(&mut ui, "").is_err());
    // Selecting a normal tab hides the IM log again.
    chat.select_tab(&mut ui, 0, 0);
    assert!(!ui.is_visible(chat.im.as_ref().unwrap().log));
}

fn im_line(p: &str) -> ImLine {
    ImLine {
        partner: p.into(),
        speaker: p.into(),
        text: "x".into(),
    }
}

// ----- CW-I21, CW-I22: slots under any sequence of joins and leaves ------------------------

#[derive(Debug, Clone)]
enum Op {
    Join(String),
    Joined(String),
    Part(String),
    Left(String),
    Say(usize),
}

fn op() -> impl Strategy<Value = Op> {
    let name = prop::sample::select(vec!["a", "A", "b", "c", "d", "e", "f", "g", "h", "i", "J"])
        .prop_map(String::from);
    prop_oneof![
        name.clone().prop_map(Op::Join),
        name.clone().prop_map(Op::Joined),
        name.clone().prop_map(Op::Part),
        name.prop_map(Op::Left),
        (0usize..10).prop_map(Op::Say),
    ]
}

proptest! {
    #[test]
    fn slots_invariants(ops in prop::collection::vec(op(), 0..80)) {
        let mut sc = SocialChat::new("Ana", true);
        for o in ops {
            let before: Vec<Option<String>> = sc.slots().to_vec();
            let held = before.iter().flatten().count();
            match o {
                Op::Join(n) => {
                    if let Some(ChatSend::Join { name }) = sc.join(&[n]).send {
                        prop_assert!(held < SLOTS);
                        prop_assert!(sc.slot_of(&name).is_none());
                    }
                    prop_assert_eq!(sc.slots().to_vec(), before);
                }
                Op::Part(n) => {
                    if let Some(ChatSend::Leave { name }) = sc.part(&[n]).send {
                        prop_assert!(sc.slot_of(&name).is_some());
                    }
                    prop_assert_eq!(sc.slots().to_vec(), before);
                }
                Op::Joined(n) => { sc.channel_joined(&n); }
                Op::Left(n) => { sc.channel_left(&n); }
                Op::Say(k) => {
                    let out = sc.slot_say(k, "x");
                    if let Some(ChatSend::Named { name, .. }) = out.send {
                        prop_assert!(sc.slot_of(&name).is_some());
                    }
                    for l in &out.lines {
                        prop_assert!(l.mask == 0x8 || l.mask == 0x1);
                    }
                }
            }
            let held: Vec<&String> = sc.slots().iter().flatten().collect();
            prop_assert!(held.len() <= SLOTS);
            for (i, a) in held.iter().enumerate() {
                for b in &held[i + 1..] {
                    prop_assert!(!name_equal(a, b));
                }
            }
        }
    }

    #[test]
    fn session_lines_never_panic(
        sender in proptest::option::of("[a-zA-Z\\u{0}-\\u{20}]{0,8}"),
        peer in "[a-zA-Z]{0,8}",
        text in ".{0,40}",
        kind in 0u8..7,
    ) {
        let mut sc = SocialChat::new("Ana", true);
        sc.channel_joined("Lore");
        sc.set_ignore_list(&["x".to_string()]);
        let ch = match kind {
            0 => Channel::Say,
            1 => Channel::Yell,
            2 => Channel::Emote,
            3 => Channel::System,
            4 => Channel::Tell { peer: &peer },
            5 => Channel::Fellowship,
            _ => Channel::Named { name: &peer },
        };
        if let SessionLine::Line { notice, .. } = sc.session_line(ch, sender.as_deref(), &text) {
            prop_assert!(notice.mask != 0, "CW-I30");
        }
    }
}

#[test]
fn im_tab_without_its_generic_row_is_not_opened() {
    let (mut ui, mut chat) = chat_ui_with(0x1000_49FF);
    let tabs = chat.windows[0].tab_list.unwrap();
    let before = ui.list_entries(tabs).len();
    assert!(
        chat.open_im_tab(&mut ui, "Tells", "Everyone", true)
            .is_err()
    );
    assert!(chat.im.is_none());
    assert_eq!(
        ui.list_entries(tabs).len(),
        before,
        "nothing is left behind"
    );
    let root = chat.windows[0].root;
    assert!(tessera_ui::game::find_below(&ui, root, IM_LOG).is_none());
}

#[test]
fn im_rows_stay_aligned_with_the_model() {
    let (mut ui, mut chat) = chat_ui();
    chat.open_im_tab(&mut ui, "Tells", "Everyone", true)
        .unwrap();
    let mut sc = SocialChat::new("Ana", true);
    for p in ["Bram", "Cid"] {
        let (_, l) = line(sc.session_line(Channel::Tell { peer: p }, None, "hi"));
        chat.deliver_im(&mut ui, l.as_ref().unwrap(), "t", &mut NoTables)
            .unwrap();
    }
    let im = chat.im.as_ref().unwrap();
    assert_eq!(im.rows.len(), im.model.rows().len());
    assert_eq!(im.generic_label, "Everyone");
    assert_eq!(ui.text_of(im.rows[0].label), "Everyone");
}
