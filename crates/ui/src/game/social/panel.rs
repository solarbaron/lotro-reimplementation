// SPDX-License-Identifier: GPL-3.0-or-later
//! The Social panel P8 (`social-panels.md` §3.1, §3.8, Tessera rules TR8 and §3.1's toggle
//! rule): `Social_Field` of layout `social`, its five pages, the input commands that open
//! it, and the other pages shown with empty lists and ghosted buttons.

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::vocab::SocialVocab;
use super::{buttons_below, find, game_message, ghost, listen_id};

/// A page of the Social panel (§3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialPage {
    /// `GroupingPage` (open at set-up).
    Grouping,
    /// `Friends_Page`.
    Friends,
    /// `Kinship_Page`.
    Kinship,
    /// `Raid_Page`.
    Raid,
    /// `RaidLock_Page`.
    RaidLock,
}

impl SocialPage {
    /// The page of an input command (§3.1, §7.2): `ToggleSocialFellowingPanel` →
    /// grouping, `…FriendsPanel` → friends, `…KinshipPanel` → kinship, `…RaidPanel` →
    /// raid, `…GroupStagePanel` → raid locks. `None` for any other command.
    #[must_use]
    pub fn of_command(name: &str) -> Option<Self> {
        Some(match name {
            "ToggleSocialFellowingPanel" => Self::Grouping,
            "ToggleSocialFriendsPanel" => Self::Friends,
            "ToggleSocialKinshipPanel" => Self::Kinship,
            "ToggleSocialRaidPanel" => Self::Raid,
            "ToggleSocialGroupStagePanel" => Self::RaidLock,
            _ => return None,
        })
    }

    /// (tab, page) element IDs.
    #[must_use]
    pub fn ids(self, v: &SocialVocab) -> (u32, u32) {
        let i = v.ids;
        match self {
            Self::Grouping => (i.grouping_tab, i.grouping_page),
            Self::Friends => (i.friends_tab, i.friends_page_outer),
            Self::Kinship => (i.kinship_tab, i.kinship_page),
            Self::Raid => (i.raid_tab, i.raid_page),
            Self::RaidLock => (i.raid_lock_tab, i.raid_lock_page),
        }
    }
}

/// What a toggle command does (§3.1 Tessera rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    /// Show the panel at this page.
    Show(SocialPage),
    /// Hide the panel.
    Hide,
}

/// The panel's toggle state (§3.1 Tessera rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SocialPanelState {
    /// The page the panel last showed (none before the first showing: the set-up page).
    pub last: Option<SocialPage>,
}

impl SocialPanelState {
    /// `ToggleSocialPanel`: hides a shown panel, else shows it at the page it last showed
    /// (first time: the grouping page, its set-up page).
    #[must_use]
    pub fn toggle_panel(&self, shown: bool) -> Toggle {
        if shown {
            Toggle::Hide
        } else {
            Toggle::Show(self.last.unwrap_or(SocialPage::Grouping))
        }
    }

    /// A page command: shows the panel at its page, and hides it only when it is shown at
    /// that page already. `current` is the page shown, if the panel is shown.
    #[must_use]
    pub fn toggle_page(&self, page: SocialPage, current: Option<SocialPage>) -> Toggle {
        if current == Some(page) {
            Toggle::Hide
        } else {
            Toggle::Show(page)
        }
    }
}

/// The Social panel on the element tree.
#[derive(Debug, Clone, PartialEq)]
pub struct SocialPanel {
    vocab: SocialVocab,
    /// `Social_Field`.
    pub root: ElementKey,
    /// `Social_Panel`.
    pub pages: Option<ElementKey>,
    /// The toggle state.
    pub state: SocialPanelState,
}

impl SocialPanel {
    /// Opens `Social_Field` of layout `social`, hidden, with the other pages' buttons
    /// ghosted (TR8).
    ///
    /// # Errors
    /// The panel could not be opened.
    pub fn open(ui: &mut Ui, vocab: &SocialVocab) -> Result<Self, crate::UiError> {
        let root = ui.open_panel_by_key(vocab.layouts.social, vocab.ids.social_field)?;
        ui.set_shown(root, false);
        Ok(Self::attach(ui, vocab, root))
    }

    /// Binds a panel the client opened (TR8: the grouping, kinship, raid and raid-lock
    /// pages' buttons are ghosted; their lists stay empty) and registers the handler of
    /// `Social_CloseButton` ([`SocialPanel::handle_event`]).
    pub fn attach(ui: &mut Ui, vocab: &SocialVocab, root: ElementKey) -> Self {
        let ids = vocab.ids;
        let click = ui.vocab().msgs.button_click;
        listen_id(ui, ids.social_close, click);
        for id in [
            ids.grouping_page,
            ids.kinship_page,
            ids.raid_page,
            ids.raid_lock_page,
        ] {
            if let Some(p) = find(ui, root, id) {
                for b in buttons_below(ui, p) {
                    ghost(ui, Some(b), true);
                }
            }
        }
        Self {
            vocab: vocab.clone(),
            root,
            pages: find(ui, root, ids.social_panel),
            state: SocialPanelState::default(),
        }
    }

    /// The page the `Social_Panel` widget has open, whether or not the panel is shown (the
    /// Panel keeps its page while hidden).
    fn open_page(&self, ui: &Ui) -> Option<SocialPage> {
        let (page, _) = ui.panel_current(self.pages?)?;
        [
            SocialPage::Grouping,
            SocialPage::Friends,
            SocialPage::Kinship,
            SocialPage::Raid,
            SocialPage::RaidLock,
        ]
        .into_iter()
        .find(|p| p.ids(&self.vocab).1 == page)
    }

    /// The page shown, when the panel is shown.
    #[must_use]
    pub fn current(&self, ui: &Ui) -> Option<SocialPage> {
        if !ui.is_visible(self.root) {
            return None;
        }
        self.open_page(ui)
    }

    /// Runs an input command (§3.1, TR12): `ToggleSocialPanel` or a page command. Returns
    /// whether the command was one of them.
    ///
    /// The page last shown is read from the `Social_Panel` widget, so a tab click followed
    /// by a hide that is not a command (the close button, the client) is remembered (§3.1
    /// Tessera rule).
    pub fn command(&mut self, ui: &mut Ui, name: &str) -> bool {
        let shown = ui.is_visible(self.root);
        let current = self.current(ui);
        if let Some(p) = self.open_page(ui) {
            self.state.last = Some(p);
        }
        let t = if name == "ToggleSocialPanel" {
            self.state.toggle_panel(shown)
        } else if let Some(p) = SocialPage::of_command(name) {
            self.state.toggle_page(p, current)
        } else {
            return false;
        };
        match t {
            Toggle::Hide => ui.set_shown(self.root, false),
            Toggle::Show(p) => self.show_page(ui, p),
        }
        true
    }

    /// Shows the panel at `page`.
    pub fn show_page(&mut self, ui: &mut Ui, page: SocialPage) {
        ui.set_shown(self.root, true);
        let (tab, pg) = page.ids(&self.vocab);
        if let Some(p) = self.pages {
            ui.panel_open(p, pg, tab);
        }
        self.state.last = Some(page);
    }

    /// Hides the panel, remembering the page it showed.
    pub fn hide(&mut self, ui: &mut Ui) {
        if let Some(p) = self.open_page(ui) {
            self.state.last = Some(p);
        }
        ui.set_shown(self.root, false);
    }

    /// Handles a UI event (§3.1): a click on `Social_CloseButton` hides the panel. Returns
    /// whether the event was the panel's.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> bool {
        let Some(m) = game_message(ev) else {
            return false;
        };
        let close = self.vocab.ids.social_close;
        if m.msg == ui.vocab().msgs.button_click
            && close != 0
            && m.source_id == close
            && ui.is_ancestor_or_self(self.root, m.source)
        {
            self.hide(ui);
            return true;
        }
        false
    }
}
