// SPDX-License-Identifier: GPL-3.0-or-later
//! The P4 social built-ins of the dispatcher (`chat-window.md` §15.10 CT2–CT7;
//! `social-panels.md` §9 TR11): `tell`, `reply`, `retell`, `fellowship`, the user-channel
//! commands `join`, `part`, `channels`, `chsay` and `ch1` … `ch8`, and the list and
//! fellowship commands `friend`, `ignore`, `invite`, `leave`, `dismiss`, `promote` and
//! `disband`.
//!
//! Each runs on the world session's [`crate::social::SocialState`]: the pure models of
//! `tessera-ui` decide (usage, refusals, slots, targets), the lines they return are rendered
//! from the install's tables with their masks (CT12; usage lines are the dispatcher's own
//! usage text, D20 item 5), and the command they ask for goes to the session. A refusal of
//! the session stays a type 1 line in Tessera's words (CT2). Without a session (headless
//! tools) the built-ins say so and send nothing; a start-up line of a channel command (`tell`,
//! `fellowship`, a slot row, `join`, `part`, `chsay`) is dropped (D20 item 7).

use tessera_ui::game::chat::ChatOutcome;
use tessera_ui::game::chat::notice::{self as n, Notice, TextRef, is_valid_name, tables as T};
use tessera_ui::game::social::SocialIntent;

use super::line::Invocation;
use super::registry::{Channel, SocialWord};
use super::{Context, Dispatcher, LocalLine, NOT_CONNECTED, Origin};
use crate::social::SocialState;
use crate::walker::WalkerSession;

/// The line of a command the session refused (CT2: Tessera's words, type 1).
pub const REFUSED: &str = "That was refused: ";
/// The line of a list or fellowship command without its name (TR11, type 1).
pub const NAME_MISSING: &str = "Give a character name.";

/// `Commands`-free fallback sub-words of `friend` and `ignore` (TR11).
const ADD: &str = "add";
const REMOVE: &str = "remove";
const LIST: &str = "list";

/// A friend or ignore sub-word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sub {
    Add,
    Remove,
    List,
}

fn sub_word(d: &Dispatcher, word: &str) -> Option<Sub> {
    let eq = |a: &str, b: &str| a.to_lowercase() == b.to_lowercase();
    let t = d.text();
    if eq(word, ADD) || t.sub_add.iter().any(|w| eq(w, word)) {
        Some(Sub::Add)
    } else if eq(word, REMOVE) || t.sub_remove.iter().any(|w| eq(w, word)) {
        Some(Sub::Remove)
    } else if eq(word, LIST) || t.sub_list.iter().any(|w| eq(w, word)) {
        Some(Sub::List)
    } else {
        None
    }
}

/// The session and its social state, or the not-connected line.
fn social_session<'s>(
    ctx: &'s mut Context<'_>,
    out: &mut Vec<LocalLine>,
) -> Option<&'s mut (dyn WalkerSession + 's)> {
    let Some(session) = ctx.session.as_deref_mut() else {
        out.push(LocalLine::error(NOT_CONNECTED));
        return None;
    };
    if session.social_mut().is_none() {
        out.push(LocalLine::error(NOT_CONNECTED));
        return None;
    }
    Some(session)
}

/// Whether a start-up line (D20 item 7) must not chat: it is logged and dropped.
fn startup_drop(
    d: &Dispatcher,
    inv: &Invocation,
    ctx: &Context<'_>,
    out: &mut Vec<LocalLine>,
) -> bool {
    if ctx.origin != Origin::Startup {
        return false;
    }
    tracing::warn!(word = %inv.word, "start-up command line is a chat line; dropped, not sent");
    out.push(LocalLine::log(format!(
        "Start-up line not sent (chat): {}{}",
        d.prefix(),
        inv.word
    )));
    true
}

/// Posts a built-in's lines (rendered with their masks) and sends its command.
fn finish(
    d: &Dispatcher,
    outcome: ChatOutcome,
    session: &mut dyn WalkerSession,
    now: std::time::Instant,
    out: &mut Vec<LocalLine>,
) {
    if let Some(social) = session.social_mut() {
        for notice in &outcome.lines {
            post(d, social, notice, out);
        }
    }
    if let Some(send) = outcome.send {
        let command = SocialState::chat_command(&send);
        if let Err(e) = session.send_command(now, command) {
            tracing::info!("chat built-in not sent: {e}");
            out.push(LocalLine::error(format!("{REFUSED}{e}")));
        }
    }
}

/// Renders one line of a built-in into `out`.
fn post(d: &Dispatcher, social: &mut SocialState, notice: &Notice, out: &mut Vec<LocalLine>) {
    let usage = |c: &str| d.usage(c);
    let text = social.render(notice, &usage);
    out.push(LocalLine::masked(notice.mask, text));
}

/// `tell`, `reply`, `retell`, `fellowship` and the slot rows (CT4, CT5, CT7).
pub(super) fn run_channel(
    d: &Dispatcher,
    ch: Channel,
    inv: &Invocation,
    ctx: &mut Context<'_>,
    out: &mut Vec<LocalLine>,
) {
    if startup_drop(d, inv, ctx, out) {
        return;
    }
    let now = ctx.now;
    let Some(session) = social_session(ctx, out) else {
        return;
    };
    let Some(social) = session.social_mut() else {
        return;
    };
    let c = social.chat_mut();
    let outcome = match ch {
        Channel::Tell => c.tell(inv.args_tokens()),
        Channel::Reply => c.reply(&inv.arguments),
        Channel::Retell => c.retell(&inv.arguments),
        Channel::Fellowship => c.fellowship(&inv.arguments),
        Channel::Slot(k) => c.slot_say(usize::from(k), &inv.arguments),
        _ => ChatOutcome::default(),
    };
    finish(d, outcome, session, now, out);
}

/// A P4 social word (CT5, TR11).
pub(super) fn run_word(
    d: &Dispatcher,
    w: SocialWord,
    inv: &Invocation,
    ctx: &mut Context<'_>,
    out: &mut Vec<LocalLine>,
) {
    // Joining, leaving and speaking on channels are channel commands: a start-up line of
    // them is dropped (D20 item 7). The channel list, the friend and ignore lists and the
    // fellowship commands are not channel commands and run from start-up lines too.
    if matches!(w, SocialWord::Join | SocialWord::Part | SocialWord::ChSay)
        && startup_drop(d, inv, ctx, out)
    {
        return;
    }
    let now = ctx.now;
    let Some(session) = social_session(ctx, out) else {
        return;
    };
    let tokens = inv.args_tokens();
    let outcome = {
        let Some(social) = session.social_mut() else {
            return;
        };
        match w {
            SocialWord::Join => social.chat_mut().join(tokens),
            SocialWord::Part => social.chat_mut().part(tokens),
            SocialWord::ChSay => social.chat_mut().chsay(tokens),
            SocialWord::Channels => ChatOutcome {
                lines: social.chat().channels(),
                send: None,
            },
            _ => return list_or_fellowship(d, w, tokens, session, now, out),
        }
    };
    finish(d, outcome, session, now, out);
}

/// The list and fellowship words of TR11.
fn list_or_fellowship(
    d: &Dispatcher,
    w: SocialWord,
    tokens: &[String],
    session: &mut dyn WalkerSession,
    now: std::time::Instant,
    out: &mut Vec<LocalLine>,
) {
    let name_arg = |i: usize, out: &mut Vec<LocalLine>| -> Option<String> {
        match tokens.get(i) {
            Some(t) if is_valid_name(t) => Some(t.clone()),
            Some(_) => {
                out.push(LocalLine::error("That is not a valid character name."));
                None
            }
            None => {
                out.push(LocalLine::error(NAME_MISSING));
                None
            }
        }
    };
    let intent = match w {
        SocialWord::Friend | SocialWord::Ignore => {
            let Some(sub) = tokens.first().and_then(|t| sub_word(d, t)) else {
                // TR11: every line of these commands is type 8 or type 1.
                out.push(LocalLine::masked(n::LOG, d.usage(w.name())));
                return;
            };
            let friend = w == SocialWord::Friend;
            match sub {
                Sub::List => {
                    if let Some(social) = session.social_mut() {
                        list(social, friend, out);
                    }
                    return;
                }
                Sub::Add => {
                    let Some(name) = name_arg(1, out) else { return };
                    if friend {
                        SocialIntent::AddFriend { name }
                    } else {
                        SocialIntent::Ignore { name }
                    }
                }
                Sub::Remove => {
                    let Some(name) = name_arg(1, out) else { return };
                    if friend {
                        SocialIntent::RemoveFriend { name }
                    } else {
                        SocialIntent::Unignore { name }
                    }
                }
            }
        }
        SocialWord::Invite => {
            let Some(name) = name_arg(0, out) else { return };
            SocialIntent::InviteToFellowship { name }
        }
        SocialWord::Dismiss => {
            let Some(name) = name_arg(0, out) else { return };
            SocialIntent::KickFromFellowship { name }
        }
        SocialWord::Promote => {
            let Some(name) = name_arg(0, out) else { return };
            SocialIntent::SetFellowshipLeader { name }
        }
        SocialWord::Leave => SocialIntent::LeaveFellowship,
        // Decision 0035 D4 adds `DisbandFellowship` (social-panels.md Q12), answered by
        // the feed's triggers only.
        SocialWord::Disband => SocialIntent::DisbandFellowship,
        _ => return,
    };
    let Some(command) = session.social_mut().and_then(|s| s.intent(&intent)) else {
        return;
    };
    if let Err(e) = session.send_command(now, command) {
        tracing::info!("social command not sent: {e}");
        out.push(LocalLine::error(format!("{REFUSED}{e}")));
    }
}

/// Tessera's own wording of the list lines when the install's tables have none (CT12:
/// Tessera's wording with the same mask).
const FRIENDS_EMPTY: &str = "Your friends list is empty.";
const IGNORE_EMPTY: &str = "Your ignore list is empty.";
const IGNORE_HEADER: &str = "Ignored characters:";
const ONLINE: &str = "online";
const OFFLINE: &str = "offline";

/// A list line from the install's table entry `token` of `table`, else `own`.
fn list_text(social: &mut SocialState, table: u32, token: u32, own: &str) -> String {
    let t = social.render_ref(&TextRef::new(table, token));
    if t.trim().is_empty() {
        own.to_owned()
    } else {
        t
    }
}

/// `friend list` and `ignore list` (TR11): type 8 lines.
fn list(social: &mut SocialState, friends: bool, out: &mut Vec<LocalLine>) {
    if friends {
        let rows: Vec<(String, bool)> = social
            .friends()
            .sorted()
            .into_iter()
            .map(|f| (f.name.clone(), f.online))
            .collect();
        if rows.is_empty() {
            let t = list_text(social, T::CHAT, 0x0657_7433, FRIENDS_EMPTY);
            out.push(LocalLine::masked(n::LOG, t));
            return;
        }
        let online = list_text(social, T::UI_SOCIAL, 0x0BE6_09B3, ONLINE);
        let offline = list_text(social, T::UI_SOCIAL, 0x0F90_2393, OFFLINE);
        for (name, on) in rows {
            let w = if on { &online } else { &offline };
            out.push(LocalLine::masked(n::LOG, format!("{name} {w}")));
        }
        return;
    }
    let names = social.ignore().sorted();
    if names.is_empty() {
        let t = list_text(social, T::CHAT, 0x0178_5864, IGNORE_EMPTY);
        out.push(LocalLine::masked(n::LOG, t));
        return;
    }
    let header = list_text(social, T::CHAT, 0x043D_716F, IGNORE_HEADER);
    out.push(LocalLine::masked(n::LOG, header));
    for name in names {
        out.push(LocalLine::masked(n::LOG, name));
    }
}
