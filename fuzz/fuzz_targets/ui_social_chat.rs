// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui`'s social chat model (`game::chat::social::SocialChat`, `chat-window.md`
//! §15, CT1–CT9) on hostile typed commands and hostile session lines: the tell, reply,
//! retell, fellowship, join, part, channel-say and slot built-ins take what the user types
//! (as the dispatcher's tokens or §7.4 arguments), and `session_line`, `channel_joined`,
//! `channel_left`, `set_ignore_list` and `friend_list` take what the server sends. Also the
//! vendor quantity entry (`game::npc::quantity::read_number`, `npc-windows.md` §7.5 rule 4),
//! and the NPC windows' `--open-panel` names (`NpcWindowKind::from_panel_name`), the other
//! text the P3/P4 windows read from the user.
//!
//! Input layout: (lossy) UTF-8 text, one operation per line: the first character picks
//! the operation, the rest of the line is its text (split at white space for token forms).
//!
//! Nothing may panic. At most eight channel slots are held and no two hold name-equal
//! names (CW-I21), the recent-name lists hold at most `RECENT_NAMES` names, a join sent
//! names a channel of at most 50 UTF-16 units and `MAX_NAME_BYTES` UTF-8 bytes (CT5), a
//! leave sent names exactly a channel a slot held before the call (CT5: the slot's
//! spelling, which only the protocol bounds), an accepted `--open-panel` name is exactly
//! one of the fixed panel names (`npc-windows.md` §4), a message to send is at most
//! `MAX_TEXT_BYTES` bytes, and a quantity read is the number its leading digits spell
//! (saturating).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_ui::game::chat::notice::{MAX_NAME_BYTES, MAX_TEXT_BYTES, name_equal};
use tessera_ui::game::chat::social::{
    Channel, ChatOutcome, ChatSend, MAX_CHANNEL_NAME_UNITS, RECENT_NAMES, SLOTS, SocialChat,
};
use tessera_ui::game::npc::NpcWindowKind;
use tessera_ui::game::npc::quantity::read_number;

/// The quantity rule as a model: leading digits, saturating at `u64::MAX`.
fn model_number(text: &str) -> u64 {
    if !text.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return 0;
    }
    let v: u128 = text
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0u128, |v, d| {
            (v * 10 + u128::from(d - b'0')).min(u128::from(u64::MAX) + 1)
        });
    u64::try_from(v).unwrap_or(u64::MAX)
}

/// `held_before` is the slot table as it stood before the operation ran.
fn check_outcome(o: &ChatOutcome, held_before: &[Option<String>; SLOTS]) {
    if let Some(s) = &o.send {
        let text = match s {
            ChatSend::Tell { text, .. }
            | ChatSend::Fellowship { text }
            | ChatSend::Named { text, .. } => text.as_str(),
            ChatSend::Join { name } => {
                // CT5's join bound applies to the name the user types.
                assert!(name.encode_utf16().count() <= MAX_CHANNEL_NAME_UNITS);
                assert!(name.len() <= MAX_NAME_BYTES, "{} bytes joined", name.len());
                ""
            }
            ChatSend::Leave { name } => {
                // CT5: `part` sends the slot's name exactly as the server spelled it; the
                // join bound does not apply to it.
                assert!(
                    held_before.iter().flatten().any(|h| h == name),
                    "leave of {name:?}, not a held slot name"
                );
                ""
            }
        };
        assert!(text.len() <= MAX_TEXT_BYTES, "{} bytes sent", text.len());
    }
}

fn check(chat: &SocialChat) {
    // CW-I21: at most eight slots, no two held names name-equal. (A slot's name comes from
    // the server's `ChannelJoined`; its length is the protocol's bound, not CT5's.)
    assert_eq!(chat.slots().len(), SLOTS);
    let held: Vec<&String> = chat.slots().iter().flatten().collect();
    for (i, a) in held.iter().enumerate() {
        for b in &held[i + 1..] {
            assert!(!name_equal(a, b), "slots hold {a:?} and {b:?}");
        }
    }
    let t = chat.targets();
    assert!(t.senders.names().len() <= RECENT_NAMES);
    assert!(t.recipients.names().len() <= RECENT_NAMES);
}

fn tokens(s: &str) -> Vec<String> {
    s.split_whitespace().take(16).map(str::to_owned).collect()
}

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let mut lines = text.split('\n');
    let player = lines.next().unwrap_or("Player");
    let mut chat = SocialChat::new(player, data.first().is_some_and(|b| b & 1 == 1));
    for line in lines.take(128) {
        let mut chars = line.chars();
        let Some(op) = chars.next() else { continue };
        let rest = chars.as_str();
        let (head, tail) = rest.split_once(' ').unwrap_or((rest, ""));
        let held_before = chat.slots().clone();
        let o = match op {
            't' => Some(chat.tell(&tokens(rest))),
            'r' => Some(chat.reply(rest)),
            'R' => Some(chat.retell(rest)),
            'f' => Some(chat.fellowship(rest)),
            'j' => Some(chat.join(&tokens(rest))),
            'p' => Some(chat.part(&tokens(rest))),
            'c' => Some(chat.chsay(&tokens(rest))),
            's' => {
                let k = head.bytes().next().map_or(0, |b| usize::from(b % 10));
                Some(chat.slot_say(k, tail))
            }
            'T' => {
                let _ = chat.session_line(Channel::Tell { peer: head }, None, tail);
                None
            }
            'S' => {
                let _ = chat.session_line(Channel::Say, Some(head), tail);
                None
            }
            'F' => {
                let _ = chat.session_line(Channel::Fellowship, Some(head), tail);
                None
            }
            'N' => {
                let (sender, line) = tail.split_once(' ').unwrap_or((tail, ""));
                let _ = chat.session_line(Channel::Named { name: head }, Some(sender), line);
                None
            }
            'J' => {
                let _ = chat.channel_joined(rest);
                None
            }
            'L' => {
                let _ = chat.channel_left(rest);
                None
            }
            'i' => {
                chat.set_ignore_list(&tokens(rest));
                None
            }
            'w' => {
                let entries: Vec<(String, bool)> = tokens(rest)
                    .into_iter()
                    .map(|n| {
                        let on = n.len() % 2 == 0;
                        (n, on)
                    })
                    .collect();
                let _ = chat.friend_list(&entries);
                None
            }
            'e' => {
                chat.world_entered(rest);
                None
            }
            'x' => {
                chat.disconnected();
                None
            }
            'g' => {
                chat.set_fellowship(rest.len() % 2 == 1);
                None
            }
            'q' => {
                assert_eq!(read_number(rest), model_number(rest), "{rest:?}");
                None
            }
            'o' => {
                // The `--open-panel` names of the NPC windows (`npc-windows.md` §4).
                if let Some(k) = NpcWindowKind::from_panel_name(rest) {
                    assert_eq!(k.panel_name(), rest);
                    assert_eq!(NpcWindowKind::from_panel_name(k.panel_name()), Some(k));
                }
                None
            }
            _ => {
                let _ = chat.chat_mode_reply();
                let _ = chat.channels();
                None
            }
        };
        if let Some(o) = &o {
            check_outcome(o, &held_before);
        }
        check(&chat);
    }
});
