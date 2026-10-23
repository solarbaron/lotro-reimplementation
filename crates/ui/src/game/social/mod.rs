// SPDX-License-Identifier: GPL-3.0-or-later
//! The social panels of P4 (`social-panels.md`): the Social panel P8 with the friends and
//! ignore lists, the fellowship display (member rows, shared vitals, the leader marker and
//! the presentation of leader change, removal and disbanding), the fellowship invitation
//! prompt and the trade window.
//!
//! Each part is a **data-driven view model** (pure, testable without a tree) and an element
//! binding that fills the install's layouts from it:
//!
//! | Module | Model | Binding | Spec |
//! |---|---|---|---|
//! | [`panel`] | [`panel::SocialPanelState`]: page toggles (§3.1 Tessera rule) | [`panel::SocialPanel`] | §3.1, §3.8, TR8 |
//! | [`friends`] | [`friends::FriendsModel`]: rows, sorting, selection, buttons, row menu | [`friends::FriendsPage`] | §3.2–§3.5, TR5 |
//! | [`ignore`] | [`ignore::IgnoreModel`] | [`ignore::IgnorePage`] | §3.6, TR6 |
//! | [`fellowship`] | [`fellowship::Fellowship`]: members, rows, leader, member data, derived notices | [`fellowship::FellowshipPanels`] | §4, TR2–TR4 |
//! | [`invite`] | [`invite::Invitation`] | [`invite::InviteWindow`] | §5, TR7 |
//! | [`trade`] | [`trade::Trade`]: sides, coin, ready states | [`trade::TradeWindow`] | §6, TR9, TR10 |
//!
//! `tessera-ui` links no session crate: every user action yields a typed
//! [`SocialIntent`] that the client maps to the scenario feed's command
//! (`social-panels.md` §8.1), and every session event reaches a model through a plain
//! method the client calls (§8.2). Strings are [`crate::game::chat::TextRef`]s rendered by
//! the client's [`TextSource`] (Tessera's own wording without an install). Lines for the
//! chat logs come back as [`Notice`]s.

pub mod fellowship;
pub mod friends;
pub mod ignore;
pub mod invite;
pub mod panel;
pub mod trade;
pub mod vocab;

pub use vocab::SocialVocab;

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::chat::notice::{Notice, TextRef, TextSource};
use super::{find_below, subtree};

/// A user action for the session (`social-panels.md` §8.1): the client maps each to the
/// feed's command (SF §9.4) or, for [`SocialIntent::StartTell`], to the chat entry.
/// Entity, item and window IDs are the client's own values, passed through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocialIntent {
    /// `AddFriend { name }`.
    AddFriend {
        /// The name.
        name: String,
    },
    /// `RemoveFriend { name }`.
    RemoveFriend {
        /// The name.
        name: String,
    },
    /// `Ignore { name }`.
    Ignore {
        /// The name.
        name: String,
    },
    /// `Unignore { name }`.
    Unignore {
        /// The name.
        name: String,
    },
    /// `InviteToFellowship { name }`.
    InviteToFellowship {
        /// The name.
        name: String,
    },
    /// `KickFromFellowship { name }`.
    KickFromFellowship {
        /// The name.
        name: String,
    },
    /// `SetFellowshipLeader { name }`.
    SetFellowshipLeader {
        /// The name.
        name: String,
    },
    /// `LeaveFellowship`.
    LeaveFellowship,
    /// `DisbandFellowship` (`scenario-feed.md` "P4 social": the leader's disband action,
    /// Q12).
    DisbandFellowship,
    /// `AnswerInvite { accept }`.
    AnswerInvite {
        /// Accepted.
        accept: bool,
    },
    /// `SelectTarget { target }`.
    SelectTarget {
        /// The entity, or none to clear the target.
        target: Option<u64>,
    },
    /// `OpenTrade { entity }` (the player menu's trade entry, §6.5).
    OpenTrade {
        /// The peer's entity.
        entity: u64,
    },
    /// `OfferTrade { window, item, quantity, coin }`.
    OfferTrade {
        /// The trade window.
        window: u64,
        /// The item offered, or none for a coin offer.
        item: Option<u64>,
        /// The quantity (0 without an item).
        quantity: u32,
        /// The coin offered, in the smallest unit.
        coin: u64,
    },
    /// `SetTradeReady { window, ready }`.
    SetTradeReady {
        /// The trade window.
        window: u64,
        /// Ready.
        ready: bool,
    },
    /// `CloseWindow { window }` of the trade window.
    CloseTrade {
        /// The trade window.
        window: u64,
    },
    /// No command: open the chat entry in the `Tell` mode with this target
    /// (`chat-window.md` §10 item 12; the typed line becomes `Chat` on `Tell`).
    StartTell {
        /// The recipient.
        name: String,
    },
}

/// Why a text-input dialog was asked for (`ui-behaviour.md` §5.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogPurpose {
    /// The friends page's add dialog (`social-panels.md` §3.5).
    AddFriend,
    /// The ignore page's add dialog (§3.6).
    AddIgnore,
}

impl DialogPurpose {
    /// The dialog's answer (§3.5, §3.6): nothing for an empty answer (the player gave
    /// none); the intent for a valid name; for any other answer (too long for the session
    /// layer's name bound, or with control characters) a type 1 line in Tessera's words,
    /// so the player learns why nothing happened.
    #[must_use]
    pub fn answer(&self, answer: &str) -> Option<SocialAction> {
        let name = answer.trim();
        if name.is_empty() {
            return None;
        }
        let intent = match self {
            Self::AddFriend => friends::FriendsModel::answer_add(name),
            Self::AddIgnore => ignore::IgnoreModel::answer_add(name),
        };
        Some(match intent {
            Some(i) => SocialAction::Intent(i),
            None => SocialAction::Post(Notice::own(
                format!(
                    "That is not a valid character name (at most {} bytes, no control \
                     characters).",
                    super::chat::notice::MAX_NAME_BYTES
                ),
                super::chat::notice::ERROR,
            )),
        })
    }
}

/// A text-input dialog the client opens (an engine dialog, UB §5.5). Its answer goes back
/// to [`DialogPurpose::answer`] with the same purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDialog {
    /// What the answer is for.
    pub purpose: DialogPurpose,
    /// The question.
    pub question: TextRef,
    /// The longest answer, in bytes of UTF-8 (the session layer's name bound,
    /// [`super::chat::notice::MAX_NAME_BYTES`]); a client dialog that counts characters
    /// keeps the answer within it, and a longer answer is refused by
    /// [`DialogPurpose::answer`] with a line.
    pub max_len: usize,
    /// The default answer.
    pub default: String,
}

/// One entry of a context menu the client shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    /// The label.
    pub label: TextRef,
    /// Whether it can be chosen (else shown ghosted).
    pub enabled: bool,
    /// What choosing it does (`None` for a heading or a ghosted entry without an action).
    pub intent: Option<SocialIntent>,
}

/// A context menu (a row menu of §3.5, §3.6 or the player menu's group submenu of §4.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextMenu {
    /// The heading, if any.
    pub heading: Option<TextRef>,
    /// The entries, in order.
    pub entries: Vec<MenuEntry>,
}

/// What a social widget asks of the client after a UI event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocialAction {
    /// Send this (or, for [`SocialIntent::StartTell`], open the tell entry).
    Intent(SocialIntent),
    /// Open a text-input dialog.
    Dialog(TextDialog),
    /// Open a context menu at the pointer.
    Menu(ContextMenu),
    /// Post a line to the chat logs.
    Post(Notice),
    /// An item was dropped on the trade window's own list (`social-panels.md` §6.6): the
    /// client checks the item and calls [`trade::Trade::drop_item`].
    TradeDrop {
        /// The dragged item (the drag payload's item reference).
        item: u64,
    },
}

/// The game-code message of an event, if it is one.
pub(crate) fn game_message(ev: &UiEvent) -> Option<&crate::events::Message> {
    match ev {
        UiEvent::Game { message } => Some(message),
        _ => None,
    }
}

/// Registers a game-code handler for `msg` on element ID `id` (non-zero).
pub(crate) fn listen_id(ui: &mut Ui, id: u32, msg: u32) {
    if id != 0 {
        ui.register_handler(id, msg);
    }
}

/// The element below `root` with ID `id`, or `root` itself when it has that ID.
pub(crate) fn find(ui: &Ui, root: ElementKey, id: u32) -> Option<ElementKey> {
    if ui.element_id(root) == Some(id) {
        return Some(root);
    }
    find_below(ui, root, id)
}

/// Ghosts (`true`) or un-ghosts a button (`social-panels.md` §2: request `ghosted` or
/// `normal`, UW §7.3).
pub(crate) fn ghost(ui: &mut Ui, k: Option<ElementKey>, on: bool) {
    if let Some(k) = k {
        let st = ui.vocab().states;
        ui.request_state(k, if on { st.ghosted } else { st.normal });
    }
}

/// Sets an element's text from a reference (or clears it).
pub(crate) fn set_ref(
    ui: &mut Ui,
    k: Option<ElementKey>,
    r: Option<&TextRef>,
    src: &mut dyn TextSource,
) {
    if let Some(k) = k {
        // No reference clears the text (empty), never falling back to the layout's own
        // design-time text (§5.2 "clears the warning", TR7 "no warning", §6.4 "cleared").
        ui.set_text(k, Some(r.map(|r| r.render(src)).unwrap_or_default()));
    }
}

/// The row of a list (an entry of `rows`) that `k` is or lies below.
pub(crate) fn row_of(ui: &Ui, rows: &[ElementKey], k: ElementKey) -> Option<usize> {
    let mut cur = Some(k);
    let mut n = 0;
    while let Some(c) = cur {
        if let Some(i) = rows.iter().position(|r| *r == c) {
            return Some(i);
        }
        n += 1;
        if n > crate::ui::MAX_TREE_DEPTH {
            return None;
        }
        cur = ui.parent(c);
    }
    None
}

/// Makes a list hold exactly `n` rows created from (`layout key`, `template`): extra rows
/// are removed from the end, missing rows created and appended. Returns the rows, in
/// list order (fewer when creation fails).
///
/// Existing rows are reused (callers only change their texts). Surplus rows are destroyed
/// in one batch ([`Ui::destroy_many`], one pass over the UI) and the list is laid out once;
/// missing rows of a list box are appended with one [`Ui::list_add_many`] and those of a
/// Tree (`FriendListBox`, §3.2) with one [`Ui::tree_add_many`] (each a top-level node with
/// the tree's `UICore_Tree_auto_expand` flag), so either list is laid out once.
pub(crate) fn ensure_rows(
    ui: &mut Ui,
    list: ElementKey,
    layout_key: u32,
    template: u32,
    n: usize,
) -> Vec<ElementKey> {
    let mut rows = ui.list_entries(list);
    if rows.len() > n {
        let surplus = rows.split_off(n);
        if let Some(l) = ui.el_mut(list).and_then(|e| e.widget.list.as_mut()) {
            l.entries.truncate(n);
            l.dirty = true;
        }
        ui.destroy_many(&surplus);
        ui.list_layout_if_dirty(list);
        return rows;
    }
    if rows.len() == n {
        return rows;
    }
    let mut new = Vec::with_capacity(n - rows.len());
    while rows.len() + new.len() < n {
        match ui.create_element_by_key(None, layout_key, template) {
            Ok(k) => new.push(k),
            Err(e) => {
                tracing::debug!("social row not created: {e}");
                break;
            }
        }
    }
    let is_tree = ui.el(list).is_some_and(|e| e.widget.tree.is_some());
    let added = if is_tree {
        // Top-level nodes with the tree's own expanded flag, as `list_add` gives them
        // (`ui-widgets.md` §9), added in one batch and laid out once.
        let expanded = ui.prop(list, ui.vocab().pids.tree_auto_expand).bool();
        let nodes: Vec<(ElementKey, Option<ElementKey>, bool)> =
            new.iter().map(|&k| (k, None, expanded)).collect();
        match ui.tree_add_many(list, &nodes) {
            Ok(()) => new.len(),
            Err((added, e)) => {
                tracing::debug!("social row not added: {e}");
                added
            }
        }
    } else {
        match ui.list_add_many(list, &new) {
            Ok(()) => new.len(),
            Err((added, e)) => {
                tracing::debug!("social row not added: {e}");
                added
            }
        }
    };
    if added < new.len() {
        ui.destroy_many(&new[added..]);
    }
    rows.extend_from_slice(&new[..added]);
    rows
}

/// Every Button below `root` (bounded).
pub(crate) fn buttons_below(ui: &Ui, root: ElementKey) -> Vec<ElementKey> {
    subtree(ui, root)
        .into_iter()
        .skip(1)
        .filter(|k| ui.el(*k).is_some_and(|e| e.class.is_button()))
        .collect()
}

/// The left-button and right-button state of a mouse message.
pub(crate) fn mouse_button(m: &crate::events::Message) -> u32 {
    m.arg.command.as_ref().map_or(0, |c| c.mouse_button())
}
