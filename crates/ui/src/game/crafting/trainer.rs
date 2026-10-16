// SPDX-License-Identifier: GPL-3.0-or-later
//! The crafting-trainer windows (`crafting-panels.md` §8.3): the vocation, profession and
//! craft-guild trainers. The window shows the client's name and display texts, the
//! profession trainer's replace label and selector (shown only with a replace list), and
//! returns [`TrainerAction`]s for Train (after Tessera's confirmation when the view asks for
//! one, §8.3 Tessera rule) and for Cancel and close.
//!
//! The selector (`ProfessionTrainer_ProfessionSelector`, a `Menu_01` in the install) is
//! driven by Tessera: each click on it moves the selection to the next profession,
//! wrapping, and its text shows the selected name (Tessera rule while the menu widget's
//! popup is not wired to game code; the first entry is selected when it is shown, §8.3 step
//! 3).

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::super::npc::kit::{ClickKind, Listen, find, instantiate, put_text, text_part};
use super::super::npcvocab::NpcVocab;
use super::super::p6vocab::{P6Vocab, own};
use super::super::popup::click;
use super::super::{show, subtree};
use super::P6Art;
use super::fallback::Kit;
use super::{CraftLabel, CraftTexts, CraftTrainerView, TrainerAction, TrainerKind, craft_label};

/// Most entries of the replace selector (Tessera bound; a character has at most 3
/// professions).
pub const MAX_REPLACE: usize = 64;

/// A trainer window's parts (§8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CraftTrainerParts {
    /// The name text (`…_NameText`).
    pub name: Option<ElementKey>,
    /// The display text (`…_DisplayText`).
    pub text: Option<ElementKey>,
    /// `…_TrainButton`.
    pub train: Option<ElementKey>,
    /// `…_CancelButton`.
    pub cancel: Option<ElementKey>,
    /// The close button: `<prefix>_CloseButton` (the install's, label inferred, see
    /// [`super::super::p6vocab`]), else Tessera's close button of the fallback window.
    pub close: Option<ElementKey>,
    /// `ProfessionTrainer_LoseProfessionLabel`.
    pub lose_label: Option<ElementKey>,
    /// `ProfessionTrainer_ProfessionSelector`.
    pub selector: Option<ElementKey>,
    /// Tessera's confirmation field (the fallback's; none in the install: Train then asks
    /// for no confirmation and the client shows its own).
    pub confirm: Option<ElementKey>,
    /// The confirmation's text.
    pub confirm_text: Option<ElementKey>,
    /// The confirmation's yes button.
    pub confirm_yes: Option<ElementKey>,
    /// The confirmation's no button.
    pub confirm_no: Option<ElementKey>,
}

/// A crafting-trainer window instance.
#[derive(Debug, Clone)]
pub struct CraftTrainerWindow {
    /// The root.
    pub root: ElementKey,
    /// The window kind.
    pub kind: TrainerKind,
    /// The parts.
    pub parts: CraftTrainerParts,
    /// The view shown.
    pub view: Option<CraftTrainerView>,
    /// The selected entry of the replace selector.
    pub selected: usize,
    /// Whether the confirmation is shown.
    pub confirming: bool,
    listen: Listen,
}

impl CraftTrainerWindow {
    /// Finds the parts below `root`. A name or display text that does not resolve falls
    /// back to the first and second texts below the root; a train or cancel button to the
    /// first and second buttons (see [`super::super::p6vocab`] on the elided labels). A
    /// window without Tessera's confirmation (the install's) gets it as a code-made child
    /// (§8.3 Tessera rule).
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        p6: &P6Vocab,
        root: ElementKey,
        kind: TrainerKind,
        art: &P6Art,
        texts: &dyn CraftTexts,
    ) -> Self {
        if find(ui, root, own::CRAFT_CONFIRM).is_none() {
            let k = Kit::new(ui, nv, p6, art);
            let node = super::fallback::confirm(&k, texts);
            let _ = instantiate(ui, root, &node);
        }
        let ids = p6.ids;
        let (name, text, train, cancel, close) = match kind {
            TrainerKind::Vocation => (
                ids.vocation_name,
                ids.vocation_text,
                ids.vocation_train,
                ids.vocation_cancel,
                ids.vocation_close,
            ),
            TrainerKind::Profession => (
                ids.profession_name,
                ids.profession_text,
                ids.profession_train,
                ids.profession_cancel,
                ids.profession_close,
            ),
            TrainerKind::Guild => (
                ids.guild_name,
                ids.guild_text,
                ids.guild_train,
                ids.guild_cancel,
                ids.guild_close,
            ),
        };
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        let texts: Vec<ElementKey> = subtree(ui, root)
            .into_iter()
            .filter(|e| ui.class(*e) == Some(WidgetClass::Text))
            .collect();
        let buttons: Vec<ElementKey> = subtree(ui, root)
            .into_iter()
            .filter(|e| ui.class(*e) == Some(WidgetClass::Button))
            .collect();
        let selector = if kind == TrainerKind::Profession {
            f(ui, ids.profession_selector)
        } else {
            None
        };
        let parts = CraftTrainerParts {
            name: f(ui, name).or_else(|| texts.first().copied()),
            text: f(ui, text).or_else(|| texts.get(1).copied()),
            train: f(ui, train).or_else(|| buttons.first().copied()),
            cancel: f(ui, cancel).or_else(|| buttons.get(1).copied()),
            close: f(ui, close).or_else(|| f(ui, own::CRAFT_TRAINER_CLOSE)),
            lose_label: if kind == TrainerKind::Profession {
                f(ui, ids.lose_profession)
            } else {
                None
            },
            selector,
            confirm: f(ui, own::CRAFT_CONFIRM),
            confirm_text: f(ui, own::CRAFT_CONFIRM_TEXT),
            confirm_yes: f(ui, own::CRAFT_CONFIRM_YES),
            confirm_no: f(ui, own::CRAFT_CONFIRM_NO),
        };
        let mut w = Self {
            root,
            kind,
            parts,
            view: None,
            selected: 0,
            confirming: false,
            listen: Listen::default(),
        };
        for k in [
            parts.train,
            parts.cancel,
            parts.close,
            parts.selector,
            parts.confirm_yes,
            parts.confirm_no,
        ] {
            w.listen.clicks(ui, k);
        }
        show(ui, parts.confirm, false);
        w
    }

    /// Shows a trainer view (§8.3 steps 1–3): the name and display texts, and for the
    /// profession trainer the label and the selector (first entry selected) when the view
    /// has a replace list; the confirmation is hidden.
    pub fn show(&mut self, ui: &mut Ui, view: &CraftTrainerView, texts: &dyn CraftTexts) {
        put_text(ui, self.parts.name, &view.name);
        put_text(ui, self.parts.text, &view.text);
        let rep = view.replace.as_ref().filter(|r| !r.is_empty());
        show(ui, self.parts.lose_label, rep.is_some());
        show(ui, self.parts.selector, rep.is_some());
        self.selected = 0;
        self.confirming = false;
        show(ui, self.parts.confirm, false);
        let mut v = view.clone();
        if let Some(r) = v.replace.as_mut() {
            r.truncate(MAX_REPLACE);
        }
        self.view = Some(v);
        self.show_selection(ui);
        for (k, l) in [
            (self.parts.train, CraftLabel::Train),
            (self.parts.cancel, CraftLabel::Cancel),
        ] {
            if let Some(k) = k
                && ui.text_of(k).is_empty()
                && text_part(ui, k, None).is_none_or(|t| ui.text_of(t).is_empty())
            {
                put_text(ui, Some(k), &craft_label(texts, l));
            }
        }
    }

    fn show_selection(&self, ui: &mut Ui) {
        let Some(sel) = self.parts.selector else {
            return;
        };
        let name = self
            .view
            .as_ref()
            .and_then(|v| v.replace.as_ref())
            .and_then(|r| r.get(self.selected))
            .cloned()
            .unwrap_or_default();
        let el = if ui.class(sel) == Some(WidgetClass::Button) {
            Some(sel)
        } else {
            text_part(ui, sel, None).or(Some(sel))
        };
        put_text(ui, el, &name);
    }

    /// The replace entry Train sends: the selected entry while the selector is shown.
    #[must_use]
    pub fn replace(&self) -> Option<usize> {
        self.view
            .as_ref()
            .and_then(|v| v.replace.as_ref())
            .filter(|r| !r.is_empty())
            .map(|r| self.selected.min(r.len() - 1))
    }

    /// The command for a UI event: Train (with Tessera's confirmation first when the view
    /// asks for one), Cancel and close, the selector.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<TrainerAction> {
        let (src, kind) = click(ui, ev)?;
        if !matches!(kind, ClickKind::Button | ClickKind::Left) {
            return None;
        }
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.root, src) {
            return None;
        }
        let view = self.view.clone()?;
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        let window = view.window;
        if self.confirming {
            if hit(self.parts.confirm_yes) {
                self.confirming = false;
                show(ui, self.parts.confirm, false);
                return Some(TrainerAction::Train {
                    window,
                    replace: self.replace(),
                });
            }
            if hit(self.parts.confirm_no) {
                self.confirming = false;
                show(ui, self.parts.confirm, false);
            }
            return None;
        }
        if hit(self.parts.cancel) || hit(self.parts.close) {
            return Some(TrainerAction::Close { window });
        }
        if hit(self.parts.selector) {
            if let Some(r) = view.replace.as_ref().filter(|r| !r.is_empty()) {
                self.selected = (self.selected + 1) % r.len();
                self.show_selection(ui);
            }
            return None;
        }
        if hit(self.parts.train) {
            if let Some(text) = &view.confirm
                && self.parts.confirm.is_some()
            {
                self.confirming = true;
                put_text(ui, self.parts.confirm_text, text);
                show(ui, self.parts.confirm, true);
                return None;
            }
            return Some(TrainerAction::Train {
                window,
                replace: self.replace(),
            });
        }
        None
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}
