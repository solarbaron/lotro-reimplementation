// SPDX-License-Identifier: GPL-3.0-or-later
//! The social panels on the game UI (`social-panels.md` §3–§7, Tessera rules §9): the
//! Social panel P8 with the friends and ignore pages, the four fellowship panels with
//! `PlayerLeaderIcon`, the fellowship invitation window and the trade window, built from the
//! install's layouts (TR1: they exist without data and start hidden) and drawn from the
//! world session's [`SocialState`].
//!
//! [`SocialUi`] binds `tessera-ui`'s views (`game::social`) to the models of the state:
//!
//! - the UI host queues the UI events of a frame and the panel input commands the UI did
//!   not take (`ToggleSocialPanel`, the five page commands, `SelectFellowOne` … `Six`,
//!   `AssistFellowTwo` … `Six`; [`is_social_command`]);
//! - after step 7 [`SocialUi::frame`] runs them with the state: the views turn events into
//!   [`SocialAction`]s, the actions become session commands ([`SocialState::intent`],
//!   `social-panels.md` §8.1), lines for the chat, the tell target for the chat entry, or a
//!   trade offer of a dropped item; then every view is refreshed when the state changed;
//! - `--open-panel social|friends|fellowship|trade` asks for a panel by name
//!   ([`SocialUi::request_open`]).
//!
//! The client has no engine text-input dialog or context menu yet: an add dialog posts the
//! command that does the same (`/friend add <name>`, `/ignore add <name>`), and a row menu
//! is kept ([`SocialUi::menu`]) for [`SocialUi::choose_menu`] (tests and drivers); the
//! fellowship commands (`/dismiss`, `/promote`, `/leave`) are the player's way meanwhile.
//! Trade entries are item slots with the install's icons; hovering one shows its examination
//! tooltip (the player's own item with its instance state, a peer's entry by its record).

use tessera_net::Command;
use tessera_ui::game::CharVocab;
use tessera_ui::game::chat::notice::{self as n, Notice};
use tessera_ui::game::examine::ExamTooltips;
use tessera_ui::game::items::SlotPlace;
use tessera_ui::game::social::fellowship::FellowshipPanels;
use tessera_ui::game::social::friends::{FriendsPage, GroupState};
use tessera_ui::game::social::ignore::IgnorePage;
use tessera_ui::game::social::invite::InviteWindow;
use tessera_ui::game::social::panel::{SocialPage, SocialPanel};
use tessera_ui::game::social::trade::{OWN_CONTAINER, PEER_CONTAINER, TradeWindow};
use tessera_ui::game::social::{ContextMenu, SocialAction, SocialIntent, SocialVocab};
use tessera_ui::{ElementKey, Ui, UiEvent};

use super::state::SocialState;
use crate::character::{CharacterData, PlayerState};

/// Most UI events and commands queued for the social panels in one frame.
pub const MAX_QUEUED: usize = 4096;
/// Most lines the panels post in one frame.
pub const MAX_POSTS: usize = 256;

/// Whether an input command is one of the social panels' (§7.2, TR12).
#[must_use]
pub fn is_social_command(name: &str) -> bool {
    name == "ToggleSocialPanel"
        || SocialPage::of_command(name).is_some()
        || fellow_number(name).is_some()
}

/// `SelectFellowOne` … `Six` → (N, false), `AssistFellowTwo` … `Six` → (N, true).
fn fellow_number(name: &str) -> Option<(usize, bool)> {
    let (rest, assist) = match name.strip_prefix("SelectFellow") {
        Some(r) => (r, false),
        None => (name.strip_prefix("AssistFellow")?, true),
    };
    let n = ["One", "Two", "Three", "Four", "Five", "Six"]
        .iter()
        .position(|w| *w == rest)?
        + 1;
    (!assist || n >= 2).then_some((n, assist))
}

/// A panel asked for by name (`--open-panel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialOpen {
    /// The Social panel at the page it last showed (first time: its set-up page).
    Social,
    /// The Social panel at the friends page.
    Friends,
    /// The fellowship display: the Social panel at its fellowship (grouping) page, and the
    /// fellowship panels, shown while a fellowship exists (TR2).
    Fellowship,
    /// The trade window, shown while a trade is open (TR9).
    Trade,
}

/// Parses a social panel name of `--open-panel`: `social`, `friends`, `fellowship`,
/// `trade` (and the root labels `Social_Field`, `SecureTradeField`), ASCII case ignored.
#[must_use]
pub fn parse_open(name: &str) -> Option<SocialOpen> {
    match name.trim().to_ascii_lowercase().as_str() {
        "social" | "social_field" | "p8" => Some(SocialOpen::Social),
        "friends" | "friend" => Some(SocialOpen::Friends),
        "fellowship" | "fellows" | "group" => Some(SocialOpen::Fellowship),
        "trade" | "securetradefield" => Some(SocialOpen::Trade),
        _ => None,
    }
}

/// What a [`SocialUi::frame`] produced besides the refreshed panels.
#[derive(Debug, Default)]
pub struct SocialFrame {
    /// Commands for the world session, in order.
    pub commands: Vec<Command>,
    /// Lines for the chat (mask, rendered text).
    pub lines: Vec<(u64, String)>,
    /// A tell to start: the chat entry opens in the `Tell` mode with this target
    /// (`chat-window.md` §10 item 12, `social-panels.md` §8.1).
    pub tell: Option<String>,
}

/// The social panels (module docs).
pub struct SocialUi {
    vocab: SocialVocab,
    chars: CharVocab,
    opened: bool,
    panel: Option<SocialPanel>,
    friends: Option<FriendsPage>,
    ignore: Option<IgnorePage>,
    fellowship: Option<FellowshipPanels>,
    invite: Option<InviteWindow>,
    trade: Option<TradeWindow>,
    tips: ExamTooltips,
    seen: Option<u64>,
    events: Vec<UiEvent>,
    commands: Vec<String>,
    open: Vec<SocialOpen>,
    menu: Option<ContextMenu>,
    leader_icon: bool,
}

impl std::fmt::Debug for SocialUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SocialUi")
            .field("opened", &self.opened)
            .field("panel", &self.panel.is_some())
            .field("fellowship", &self.fellowship.is_some())
            .field("trade", &self.trade.is_some())
            .finish_non_exhaustive()
    }
}

impl SocialUi {
    /// The panels over the install's vocabulary (nothing is built until the first frame).
    #[must_use]
    pub fn new(vocab: SocialVocab, chars: CharVocab) -> Self {
        let tips = ExamTooltips::new(chars.clone());
        Self {
            vocab,
            chars,
            opened: false,
            panel: None,
            friends: None,
            ignore: None,
            fellowship: None,
            invite: None,
            trade: None,
            tips,
            seen: None,
            events: Vec::new(),
            commands: Vec::new(),
            open: Vec::new(),
            menu: None,
            leader_icon: false,
        }
    }

    /// The vocabulary.
    #[must_use]
    pub fn vocab(&self) -> &SocialVocab {
        &self.vocab
    }

    /// The Social panel, once built.
    #[must_use]
    pub fn panel(&self) -> Option<&SocialPanel> {
        self.panel.as_ref()
    }

    /// The friends page, once built.
    #[must_use]
    pub fn friends(&self) -> Option<&FriendsPage> {
        self.friends.as_ref()
    }

    /// The ignore page, once built.
    #[must_use]
    pub fn ignore(&self) -> Option<&IgnorePage> {
        self.ignore.as_ref()
    }

    /// The fellowship panels, once built.
    #[must_use]
    pub fn fellowship(&self) -> Option<&FellowshipPanels> {
        self.fellowship.as_ref()
    }

    /// The invitation window, once built.
    #[must_use]
    pub fn invite(&self) -> Option<&InviteWindow> {
        self.invite.as_ref()
    }

    /// The trade window, once built.
    #[must_use]
    pub fn trade(&self) -> Option<&TradeWindow> {
        self.trade.as_ref()
    }

    /// The row menu the player last asked for (right click), until chosen or replaced.
    #[must_use]
    pub fn menu(&self) -> Option<&ContextMenu> {
        self.menu.as_ref()
    }

    /// UI events and commands waiting for the next [`Self::frame`].
    #[must_use]
    pub fn pending(&self) -> (usize, usize) {
        (self.events.len(), self.commands.len())
    }

    /// Queues a UI event for the next [`Self::frame`] (bounded).
    pub fn queue_event(&mut self, ev: &UiEvent) {
        if self.events.len() < MAX_QUEUED {
            self.events.push(ev.clone());
        }
    }

    /// Queues a panel input command; returns whether it is one of the social panels'.
    pub fn queue_command(&mut self, name: &str) -> bool {
        if !is_social_command(name) {
            return false;
        }
        if self.commands.len() < MAX_QUEUED {
            self.commands.push(name.to_owned());
        }
        true
    }

    /// Asks for a panel by name; it opens in the next [`Self::frame`].
    pub fn request_open(&mut self, what: SocialOpen) {
        if !self.open.contains(&what) {
            self.open.push(what);
        }
    }

    /// Builds every panel from the install's layouts, hidden (TR1). A panel that cannot be
    /// built is left out (logged).
    pub fn ensure_open(&mut self, ui: &mut Ui) {
        if self.opened {
            return;
        }
        self.opened = true;
        self.vocab.register_classes(ui);
        match SocialPanel::open(ui, &self.vocab) {
            Ok(p) => {
                self.friends = FriendsPage::bind(ui, &self.vocab, p.root);
                self.ignore = IgnorePage::bind(ui, &self.vocab, p.root);
                self.panel = Some(p);
            }
            Err(e) => tracing::warn!("the Social panel did not open: {e}"),
        }
        let f = FellowshipPanels::open(ui, &self.vocab);
        if f.panels.is_empty() {
            tracing::warn!("the fellowship panels did not open");
        }
        self.fellowship = Some(f);
        match InviteWindow::open(ui, &self.vocab) {
            Ok(w) => self.invite = Some(w),
            Err(e) => tracing::warn!("the invitation window did not open: {e}"),
        }
        match TradeWindow::open(ui, &self.vocab, self.chars.clone()) {
            Ok(w) => self.trade = Some(w),
            Err(e) => tracing::warn!("the trade window did not open: {e}"),
        }
        tracing::info!(
            social = self.panel.is_some(),
            friends = self.friends.is_some(),
            ignore = self.ignore.is_some(),
            fellowship = self.fellowship.as_ref().map_or(0, |f| f.panels.len()),
            invite = self.invite.is_some(),
            trade = self.trade.is_some(),
            "social panels built (hidden until used)"
        );
    }

    /// Hides the Social panel (Escape, `game-panels.md` §16). Returns its root when it was
    /// shown.
    pub fn escape(&mut self, ui: &mut Ui) -> Option<ElementKey> {
        match self.panel.as_mut() {
            Some(p) if ui.is_visible(p.root) => {
                p.hide(ui);
                Some(p.root)
            }
            _ => None,
        }
    }

    /// Sets `PlayerLeaderIcon` of the player-vitals panel P4 (`root`), once.
    pub fn attach_player_vitals(&mut self, ui: &mut Ui, root: ElementKey, state: &SocialState) {
        if self.leader_icon {
            return;
        }
        let Some(f) = self.fellowship.as_mut() else {
            return;
        };
        let k = tessera_ui::game::find_below(ui, root, self.vocab.ids.player_leader);
        if k.is_some() {
            f.set_player_leader(ui, k, state.fellowship());
            self.leader_icon = true;
        }
    }

    /// Chooses entry `i` of the kept row menu (as a click on it would): its action runs in
    /// the next [`Self::frame`]. Returns whether the entry exists and is enabled.
    pub fn choose_menu(&mut self, i: usize, state: &mut SocialState) -> Option<Command> {
        let menu = self.menu.take()?;
        let e = menu.entries.get(i)?;
        if !e.enabled {
            return None;
        }
        state.intent(e.intent.as_ref()?)
    }

    /// The frame's work (module docs). `data` and `player` are the install's item facts and
    /// the player's state, for the trade window's tooltips and drops.
    pub fn frame(
        &mut self,
        ui: &mut Ui,
        state: &mut SocialState,
        data: Option<&CharacterData>,
        player: Option<&PlayerState>,
    ) -> SocialFrame {
        let mut out = SocialFrame::default();
        self.ensure_open(ui);
        let mut actions: Vec<SocialAction> = Vec::new();
        for name in std::mem::take(&mut self.commands) {
            if let Some(p) = self.panel.as_mut()
                && p.command(ui, &name)
            {
                continue;
            }
            if let Some((n, assist)) = fellow_number(&name) {
                let f = state.fellowship();
                let a = if assist {
                    f.assist_fellow(n)
                } else {
                    f.select_fellow(n)
                };
                actions.extend(a);
            }
        }
        for what in std::mem::take(&mut self.open) {
            self.open_now(ui, what);
        }
        for ev in std::mem::take(&mut self.events) {
            tracing::trace!(?ev, "social panels: UI event");
            actions.extend(self.event(ui, &ev, state, data, player));
        }
        self.run_actions(actions, state, player, &mut out);
        if self.seen != Some(state.generation()) || !out.commands.is_empty() {
            self.refresh(ui, state);
        }
        self.tips.prune(ui);
        out
    }

    fn open_now(&mut self, ui: &mut Ui, what: SocialOpen) {
        let Some(p) = self.panel.as_mut() else {
            return;
        };
        match what {
            SocialOpen::Social => {
                let page = p.state.last.unwrap_or(SocialPage::Grouping);
                p.show_page(ui, page);
            }
            SocialOpen::Friends => p.show_page(ui, SocialPage::Friends),
            SocialOpen::Fellowship => p.show_page(ui, SocialPage::Grouping),
            // The trade window shows itself while a trade is open (TR9).
            SocialOpen::Trade => {}
        }
    }

    fn event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        state: &mut SocialState,
        data: Option<&CharacterData>,
        player: Option<&PlayerState>,
    ) -> Vec<SocialAction> {
        if self.tips.handle_event(ui, ev) {
            return Vec::new();
        }
        if let UiEvent::ExamineHover { element, pointer } = ev {
            self.trade_tooltip(ui, *element, *pointer, state, data, player);
            return Vec::new();
        }
        if let Some(p) = self.panel.as_mut()
            && p.handle_event(ui, ev)
        {
            return Vec::new();
        }
        let group = GroupState {
            in_group: state.fellowship().exists(),
            leads: state.fellowship().player_leads(),
            full: state.fellowship().members().len() >= 6,
        };
        let touches = self.touches(ui, ev);
        let mut out = Vec::new();
        let parts = state.parts();
        if let Some(f) = self.friends.as_mut() {
            out.extend(f.handle_event(ui, ev, parts.friends, group, parts.text));
        }
        if let Some(i) = self.ignore.as_mut() {
            out.extend(i.handle_event(ui, ev, parts.ignore));
        }
        if let Some(f) = self.fellowship.as_ref() {
            out.extend(f.handle_event(ui, ev, parts.fellowship));
        }
        if let Some(w) = self.invite.as_mut() {
            out.extend(w.handle_event(ui, ev, parts.invite));
        }
        if let Some(t) = self.trade.as_mut() {
            out.extend(t.handle_event(ui, ev, parts.trade));
        }
        // Only an event of a social element (a click, a selection, a drop on the trade
        // window) may have changed a model: other panels' events refresh nothing.
        if touches || !out.is_empty() {
            state.mark_changed();
        }
        out
    }

    /// Whether `ev` concerns an element of a social window (its source, or a drag's
    /// target). Events of other kinds count as touching (conservative).
    fn touches(&self, ui: &Ui, ev: &UiEvent) -> bool {
        let mut roots: Vec<ElementKey> = Vec::new();
        roots.extend(self.panel.as_ref().map(|p| p.root));
        if let Some(f) = self.fellowship.as_ref() {
            roots.extend(f.panels.iter().copied());
        }
        roots.extend(self.invite.as_ref().map(|w| w.root));
        roots.extend(self.trade.as_ref().map(|w| w.root));
        let below = |k: ElementKey| {
            let mut at = Some(k);
            // Bounded walk (element trees are shallow; a cycle cannot loop forever).
            for _ in 0..256 {
                let Some(e) = at else { return false };
                if roots.contains(&e) {
                    return true;
                }
                at = ui.parent(e);
            }
            false
        };
        match ev {
            UiEvent::Game { message } => below(message.source),
            UiEvent::DragEnded { source, target, .. } => {
                below(*source) || target.is_some_and(below)
            }
            _ => true,
        }
    }

    fn run_actions(
        &mut self,
        actions: Vec<SocialAction>,
        state: &mut SocialState,
        player: Option<&PlayerState>,
        out: &mut SocialFrame,
    ) {
        let mut queue: std::collections::VecDeque<SocialAction> = actions.into();
        let mut guard = 0usize;
        while let Some(a) = queue.pop_front() {
            guard += 1;
            if guard > MAX_QUEUED {
                break;
            }
            match a {
                SocialAction::Intent(SocialIntent::StartTell { name }) => out.tell = Some(name),
                SocialAction::Intent(i) => {
                    if let Some(c) = state.intent(&i) {
                        out.commands.push(c);
                    }
                }
                SocialAction::Post(notice) => post(state, &notice, out),
                SocialAction::Menu(m) => self.menu = Some(m),
                SocialAction::Dialog(d) => {
                    // No engine text-input dialog in the client yet (module docs).
                    let cmd = match d.purpose {
                        tessera_ui::game::social::DialogPurpose::AddFriend => "/friend add",
                        tessera_ui::game::social::DialogPurpose::AddIgnore => "/ignore add",
                    };
                    let notice = Notice::own(
                        format!("Type {cmd} <name> in the chat to add a name."),
                        n::LOG,
                    );
                    post(state, &notice, out);
                }
                SocialAction::TradeDrop { item } => {
                    let (in_backpack, quantity) = backpack_item(player, item);
                    queue.push_back(state.trade_mut().drop_item(item, quantity, in_backpack));
                }
            }
        }
    }

    /// The examination tooltip of a trade entry under the pointer (`social-panels.md` §6.2:
    /// an item display shows its item's tooltip, `item-tooltips.md`).
    fn trade_tooltip(
        &mut self,
        ui: &mut Ui,
        element: ElementKey,
        pointer: (i32, i32),
        state: &SocialState,
        data: Option<&CharacterData>,
        player: Option<&PlayerState>,
    ) {
        let (Some(t), Some(data)) = (self.trade.as_ref(), data) else {
            return;
        };
        let Some(slot) = t.slots.slot_of(ui, element) else {
            return;
        };
        let SlotPlace::Other { container, index } = slot.place else {
            return;
        };
        let own = match container {
            OWN_CONTAINER => true,
            PEER_CONTAINER => false,
            _ => return,
        };
        let Some(entry) = state.trade_entry(own, index as usize) else {
            return;
        };
        let purpose = tessera_gamedata::tooltip::Purpose::Hover;
        let input = match (entry.item, player) {
            (Some(id), Some(p)) if p.inventory().item(id.0).is_some() => {
                crate::character::tooltip::exam_tooltip_of(data, p, id.0, purpose, false).ok()
            }
            _ => record_tooltip(data, &entry.what),
        };
        if let Some(input) = input
            && let Err(e) = self.tips.show(ui, &input, pointer)
        {
            tracing::debug!("trade tooltip not shown: {e}");
        }
    }

    /// Refreshes every view from the state.
    pub fn refresh(&mut self, ui: &mut Ui, state: &mut SocialState) {
        let parts = state.parts();
        if let Some(f) = self.friends.as_mut() {
            f.refresh(ui, parts.friends, parts.text);
        }
        if let Some(i) = self.ignore.as_mut() {
            i.refresh(ui, parts.ignore);
        }
        if let Some(f) = self.fellowship.as_mut() {
            f.refresh(ui, parts.fellowship);
        }
        if let Some(w) = self.invite.as_mut() {
            w.refresh(ui, parts.invite, parts.text);
        }
        if let Some(t) = self.trade.as_mut() {
            t.refresh(ui, parts.trade, parts.text);
        }
        self.seen = Some(state.generation());
    }

    /// Whether a social window is shown (the Social panel, a fellowship panel, the
    /// invitation or the trade window).
    #[must_use]
    pub fn any_shown(&self, ui: &Ui) -> bool {
        self.panel.as_ref().is_some_and(|p| ui.is_visible(p.root))
            || self
                .fellowship
                .as_ref()
                .is_some_and(|f| f.panels.iter().any(|k| ui.is_visible(*k)))
            || self.invite.as_ref().is_some_and(|w| ui.is_visible(w.root))
            || self.trade.as_ref().is_some_and(|w| ui.is_visible(w.root))
    }
}

/// Renders a panel's line into the frame's lines (bounded).
fn post(state: &mut SocialState, notice: &Notice, out: &mut SocialFrame) {
    if out.lines.len() >= MAX_POSTS {
        return;
    }
    let usage = |c: &str| format!("Usage: /{c}");
    let text = state.render(notice, &usage);
    out.lines.push((notice.mask, text));
}

/// Whether `item` is one of the player's own bag items, and its stack's quantity (TR9).
fn backpack_item(player: Option<&PlayerState>, item: u64) -> (bool, u32) {
    let Some(p) = player else {
        return (false, 0);
    };
    match p.inventory().item(item) {
        Some(it) if it.place.container == tessera_gamedata::inventory::Container::Backpack => {
            (true, it.quantity.max(1))
        }
        _ => (false, 0),
    }
}

/// The examination tooltip of a record (a peer's trade entry): the hover tooltip of the
/// record without instance state; a literal ref has none.
fn record_tooltip(
    data: &CharacterData,
    what: &tessera_net::domain::Ref,
) -> Option<tessera_ui::game::examine::ExamTooltip> {
    let tessera_net::domain::Ref::Record(did) = what else {
        return None;
    };
    let did = tessera_formats::Did(did.0);
    let req = tessera_gamedata::tooltip::TooltipRequest::hover(did);
    let viewer = tessera_gamedata::tooltip::Viewer::default();
    let model = data.engine().build(&req, &viewer).ok()?;
    let key = tessera_ui::game::examine::ExamKey {
        instance: 0,
        record: did.0,
    };
    crate::character::tooltip::convert(data, &model, key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::EntityId;

    #[test]
    fn social_commands_and_names() {
        for c in [
            "ToggleSocialPanel",
            "ToggleSocialFriendsPanel",
            "ToggleSocialFellowingPanel",
            "ToggleSocialKinshipPanel",
            "ToggleSocialRaidPanel",
            "ToggleSocialGroupStagePanel",
            "SelectFellowOne",
            "SelectFellowSix",
            "AssistFellowTwo",
            "AssistFellowSix",
        ] {
            assert!(is_social_command(c), "{c}");
        }
        for c in [
            "AssistFellowOne",
            "SelectFellowSeven",
            "ToggleJournalPanel",
            "",
        ] {
            assert!(!is_social_command(c), "{c}");
        }
        assert_eq!(fellow_number("SelectFellowThree"), Some((3, false)));
        assert_eq!(fellow_number("AssistFellowFive"), Some((5, true)));
        assert_eq!(parse_open("Social"), Some(SocialOpen::Social));
        assert_eq!(parse_open(" FRIENDS "), Some(SocialOpen::Friends));
        assert_eq!(parse_open("fellowship"), Some(SocialOpen::Fellowship));
        assert_eq!(parse_open("SecureTradeField"), Some(SocialOpen::Trade));
        assert_eq!(parse_open("bags"), None);
    }

    /// Without an install the panels cannot be built: the frame still turns the commands of
    /// the fellowship into session commands, and nothing panics.
    #[test]
    fn frame_without_layouts() {
        let mut ui = Ui::new(
            Box::new(tessera_ui::NullEnv),
            tessera_ui::Vocab::default(),
            (800, 600),
        );
        let mut s = SocialUi::new(SocialVocab::default(), CharVocab::default());
        let mut st = SocialState::new(true);
        let _ = st.apply(
            &tessera_net::Event::WorldEntered {
                player: EntityId(1),
                name: "Tam".into(),
                transform: tessera_net::Transform::new(tessera_net::Vec3::ZERO, 0.0),
            },
            &|_| None,
        );
        assert!(s.queue_command("SelectFellowOne"));
        assert!(!s.queue_command("Jump"));
        s.request_open(SocialOpen::Social);
        let out = s.frame(&mut ui, &mut st, None, None);
        assert_eq!(
            out.commands,
            vec![Command::Combat(
                tessera_net::domain::CombatCommand::SelectTarget {
                    target: Some(EntityId(1))
                }
            )]
        );
        assert!(!s.any_shown(&ui));
        assert!(s.escape(&mut ui).is_none());
    }

    /// Regression (P4 review): a UI event of another panel (a bag drag, a click elsewhere)
    /// does not count as a change of the social models, so nothing refreshes and the
    /// plugins' party views are not rebuilt.
    #[test]
    fn other_panels_events_change_nothing() {
        let mut ui = Ui::new(
            Box::new(tessera_ui::NullEnv),
            tessera_ui::Vocab::default(),
            (800, 600),
        );
        let mut s = SocialUi::new(SocialVocab::default(), CharVocab::default());
        let mut st = SocialState::new(true);
        let _ = s.frame(&mut ui, &mut st, None, None);
        let generation = st.generation();
        let other = ui.screen_root();
        s.queue_event(&UiEvent::Game {
            message: tessera_ui::Message {
                seq: 1,
                source: other,
                source_id: 0,
                msg: 0,
                arg: tessera_ui::MsgArg::default(),
            },
        });
        s.queue_event(&UiEvent::DragEnded {
            source: other,
            target: None,
            accepted: false,
        });
        let _ = s.frame(&mut ui, &mut st, None, None);
        assert_eq!(st.generation(), generation);
        // Reading the models for a view is not a change either.
        let _ = st.parts();
        assert_eq!(st.generation(), generation);
        st.mark_changed();
        assert_ne!(st.generation(), generation);
    }
}
