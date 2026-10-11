// SPDX-License-Identifier: GPL-3.0-or-later
//! The fellowship invitation prompt (`social-panels.md` §5, Tessera rule §9 TR7): the
//! `FellowInvite` window of layout `fellowinvite` with its title, invitation text, warning
//! and one row per member of the inviting group; accept answers yes, decline and close
//! answer no; no timeout.

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::chat::notice::{TextRef, TextSource, args as A, tables as T};
use super::vocab::SocialVocab;
use super::{SocialAction, SocialIntent, ensure_rows, find, game_message, listen_id, set_ref};

/// One member row of the invitation (§5.1: class icon, level, name, area).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InviteRow {
    /// The name.
    pub name: String,
    /// The level, if known.
    pub level: Option<i64>,
    /// The area name, if known (cleared when unknown).
    pub area: Option<String>,
}

/// The pending invitation (TR7).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Invitation {
    pending: Option<String>,
}

impl Invitation {
    /// `FellowshipInvite { from }` (TR7): shows the prompt; a second invitation while one
    /// is shown replaces its text and row.
    pub fn invite(&mut self, from: &str) {
        self.pending = Some(from.to_string());
    }

    /// Whether the prompt is shown.
    #[must_use]
    pub fn shown(&self) -> bool {
        self.pending.is_some()
    }

    /// The inviter.
    #[must_use]
    pub fn from(&self) -> Option<&str> {
        self.pending.as_deref()
    }

    /// The title (UI, 0x03045B45).
    #[must_use]
    pub fn title() -> TextRef {
        TextRef::new(T::UI, 0x0304_5B45)
    }

    /// The invitation text (FT, 0x089728CE) with the inviter's name.
    #[must_use]
    pub fn text(&self) -> Option<TextRef> {
        self.pending
            .as_ref()
            .map(|f| TextRef::new(T::FELLOWSHIP, 0x0897_28CE).text(A::NAME, f.clone()))
    }

    /// The rows: one, the inviter, without class, level or area (TR7).
    #[must_use]
    pub fn rows(&self) -> Vec<InviteRow> {
        self.pending
            .iter()
            .map(|f| InviteRow {
                name: f.clone(),
                level: None,
                area: None,
            })
            .collect()
    }

    /// Accept (yes) or decline and close (no): `AnswerInvite`, and the prompt hides.
    /// `None` when no invitation is pending.
    pub fn answer(&mut self, accept: bool) -> Option<SocialIntent> {
        self.pending
            .take()
            .map(|_| SocialIntent::AnswerInvite { accept })
    }
}

/// The invitation window on the element tree.
#[derive(Debug, Clone, PartialEq)]
pub struct InviteWindow {
    vocab: SocialVocab,
    /// `FellowInvite`.
    pub root: ElementKey,
    /// The rows, in list order.
    pub rows: Vec<ElementKey>,
}

impl InviteWindow {
    /// Opens `FellowInvite` of layout `fellowinvite`, hidden (TR1), and registers the
    /// button handlers.
    ///
    /// # Errors
    /// The panel could not be opened.
    pub fn open(ui: &mut Ui, vocab: &SocialVocab) -> Result<Self, crate::UiError> {
        let root = ui.open_panel_by_key(vocab.layouts.fellowinvite, vocab.ids.invite)?;
        ui.set_shown(root, false);
        Ok(Self::attach(ui, vocab, root))
    }

    /// Binds a window the client opened.
    pub fn attach(ui: &mut Ui, vocab: &SocialVocab, root: ElementKey) -> Self {
        let ids = vocab.ids;
        let click = ui.vocab().msgs.button_click;
        for id in [ids.invite_accept, ids.invite_decline, ids.invite_close] {
            listen_id(ui, id, click);
        }
        Self {
            vocab: vocab.clone(),
            root,
            rows: Vec::new(),
        }
    }

    /// Shows the model: hidden without an invitation; else the title, text, no warning,
    /// the rows, shown.
    pub fn refresh(&mut self, ui: &mut Ui, model: &Invitation, src: &mut dyn TextSource) {
        let ids = self.vocab.ids;
        if !model.shown() {
            ui.set_shown(self.root, false);
            return;
        }
        let f = |ui: &Ui, id: u32| find(ui, self.root, id);
        let title = f(ui, ids.invite_title);
        set_ref(ui, title, Some(&Invitation::title()), src);
        let text = f(ui, ids.invite_text);
        set_ref(ui, text, model.text().as_ref(), src);
        let warning = f(ui, ids.invite_warning);
        set_ref(ui, warning, None, src);
        if let Some(list) = f(ui, ids.invite_list) {
            let rows = model.rows();
            self.rows = ensure_rows(
                ui,
                list,
                self.vocab.layouts.fellowinvite,
                ids.invite_row,
                rows.len(),
            );
            for (k, r) in self.rows.clone().into_iter().zip(&rows) {
                if let Some(e) = find(ui, k, ids.invite_row_name) {
                    ui.set_text(e, Some(r.name.clone()));
                }
                if let Some(e) = find(ui, k, ids.invite_row_level) {
                    // TR7: no level or area is shown empty, never as the layout's own text.
                    ui.set_text(e, Some(r.level.map(|l| l.to_string()).unwrap_or_default()));
                }
                if let Some(e) = find(ui, k, ids.invite_row_area) {
                    ui.set_text(e, Some(r.area.clone().unwrap_or_default()));
                }
            }
        }
        ui.set_shown(self.root, true);
    }

    /// Handles a UI event: accept answers yes, decline and close answer no; the window
    /// hides (§5.2).
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        model: &mut Invitation,
    ) -> Vec<SocialAction> {
        let Some(m) = game_message(ev) else {
            return Vec::new();
        };
        if m.msg != ui.vocab().msgs.button_click {
            return Vec::new();
        }
        let ids = self.vocab.ids;
        let accept = if m.source_id == ids.invite_accept {
            true
        } else if m.source_id == ids.invite_decline || m.source_id == ids.invite_close {
            false
        } else {
            return Vec::new();
        };
        if !ui.is_ancestor_or_self(self.root, m.source) {
            return Vec::new();
        }
        let out: Vec<SocialAction> = model
            .answer(accept)
            .map(SocialAction::Intent)
            .into_iter()
            .collect();
        ui.set_shown(self.root, false);
        out
    }
}
