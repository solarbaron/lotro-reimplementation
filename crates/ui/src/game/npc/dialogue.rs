// SPDX-License-Identifier: GPL-3.0-or-later
//! Dialogue (`npc-windows.md` §5): which native window shows a `DialogueShown` page
//! (§5.1), the conversation window with only Continue (§5.2), the choice window with the
//! greeting row and one row per option (§5.3), the NPC comment window (§5.4), and closing
//! (§5.7).

use crate::env::ImageRef;
use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::examine::ExamTemplate;
use super::super::npcvocab::NpcVocab;
use super::super::set_image_ref;
use super::kit::{
    ClickKind, FNode, FallbackArt, FontLadder, Listen, OPTION_LABEL_ID, click_of, colours,
    commit_rows, cut, find, fit_height, icon_part, make_row, put_text, row_of, text_part,
};
use super::{NpcAction, NpcLabel, NpcTexts, WindowId, label_text};

/// Most option rows a choice page shows (Tessera bound; the feed allows 16).
pub const MAX_OPTIONS: usize = 64;

/// The native window of a dialogue page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogueKind {
    /// The NPC comment window (§5.4).
    Comment,
    /// The conversation window with only Continue (§5.2).
    Conversation,
    /// The choice window (§5.3).
    Choice,
}

/// §5.1 **Tessera rule**: a comment text without options → the comment window; no options
/// → the conversation window; otherwise the choice window.
#[must_use]
pub fn window_for(text_is_comment: bool, options: usize) -> DialogueKind {
    match (text_is_comment, options) {
        (true, 0) => DialogueKind::Comment,
        (false, 0) => DialogueKind::Conversation,
        _ => DialogueKind::Choice,
    }
}

/// One option of a page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DialogueOption {
    /// The option number (1 … 16 in the feed).
    pub option: u8,
    /// The rendered option text (or the action entry's text, §5.3 rule 3).
    pub text: String,
    /// The action entry's icon (§5.3 rule 3), when the row uses one.
    pub icon: Option<ImageRef>,
}

/// The comment record's texts (§5.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommentTexts {
    /// `NPCComment_Title`.
    pub title: String,
    /// `NPCComment_WindowLabel`.
    pub window_label: String,
    /// `NPCComment_CloseButtonText`.
    pub close_text: String,
}

/// A `DialogueShown` page, rendered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DialoguePage {
    /// The feed window.
    pub window: WindowId,
    /// The speaker's name (`None`: no speaker, an empty title).
    pub speaker: Option<String>,
    /// The page text (markup allowed, §5.5).
    pub text: String,
    /// The options (any order; shown in ascending option number).
    pub options: Vec<DialogueOption>,
    /// The comment record's texts, for the comment window.
    pub comment: Option<CommentTexts>,
}

impl DialoguePage {
    /// The window kind of §5.1 for this page.
    #[must_use]
    pub fn kind(&self) -> DialogueKind {
        window_for(self.comment.is_some(), self.options.len())
    }

    /// The options in display order: ascending option number, one per number (the first
    /// given wins), at most [`MAX_OPTIONS`].
    #[must_use]
    pub fn ordered_options(&self) -> Vec<&DialogueOption> {
        let mut v: Vec<&DialogueOption> = self.options.iter().collect();
        v.sort_by_key(|o| o.option);
        v.dedup_by_key(|o| o.option);
        v.truncate(MAX_OPTIONS);
        v
    }
}

/// The row templates of the choice list (§5.3): `ActionPickerGreetingTemplateText` and
/// `ActionPickerMiscTemplate` of layout `npc_actionpicker`. `None` uses a fallback row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChoiceTemplates {
    /// The greeting row.
    pub greeting: Option<ExamTemplate>,
    /// An action row.
    pub row: Option<ExamTemplate>,
}

/// Width of the choice list's rows (§5.3: 375).
pub const ROW_WIDTH: i32 = 375;

/// Height of an action row (§5.3: 33).
pub const ROW_HEIGHT: i32 = 33;

/// Most height a greeting row takes (Tessera bound; the list scrolls).
pub const MAX_GREETING_HEIGHT: i32 = 4096;

/// A dialogue window instance.
#[derive(Debug, Clone)]
pub struct DialogueWindow {
    /// The feed window shown.
    pub window: WindowId,
    /// The native window kind.
    pub kind: DialogueKind,
    /// The window's root.
    pub root: ElementKey,
    /// The title text (`QuestDispenser_Label`, `Panel_Label_Title`,
    /// `NPCComment_TitleTextField`).
    pub title: Option<ElementKey>,
    /// The comment window's title-bar label (`NPCComment_WindowLabel`).
    pub window_label: Option<ElementKey>,
    /// The page text (`QuestDispenserIntroText`, `NPCComment_ContentText`; the greeting
    /// row of a choice page).
    pub text: Option<ElementKey>,
    /// The comment window's text button (`NPCComment_CloseTextButton`).
    pub close_text: Option<ElementKey>,
    /// Elements whose click sends `CloseWindow`.
    pub closers: Vec<ElementKey>,
    /// The choice list (`ActionPickerListBox`).
    pub list: Option<ElementKey>,
    /// The option rows with their option numbers.
    pub rows: Vec<(ElementKey, u8)>,
    /// The conversation window's font-size arrows (§5.2 Tessera rule).
    fonts: FontLadder,
    templates: ChoiceTemplates,
    art: FallbackArt,
    nv: NpcVocab,
    listen: Listen,
}

impl DialogueWindow {
    /// Finds the parts of a dialogue window of `kind` below `root` and prepares it: for the
    /// conversation window the quick-info, mission and objective parts are hidden and of
    /// the button fields only `ContinueField` is shown (§5.2); close buttons and Continue
    /// send `CloseWindow`.
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        root: ElementKey,
        kind: DialogueKind,
        templates: ChoiceTemplates,
        art: &FallbackArt,
    ) -> Self {
        let ids = nv.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        let mut w = Self {
            window: 0,
            kind,
            root,
            title: None,
            window_label: None,
            text: None,
            close_text: None,
            closers: Vec::new(),
            list: None,
            rows: Vec::new(),
            fonts: FontLadder::default(),
            templates,
            art: art.clone(),
            nv: nv.clone(),
            listen: Listen::default(),
        };
        match kind {
            DialogueKind::Conversation => {
                w.title = f(ui, ids.dispenser_label);
                w.text = f(ui, ids.intro_text);
                for id in [
                    ids.quick_info,
                    ids.mission_pane,
                    ids.twist_pane,
                    ids.objectives_rewards,
                    ids.accept_decline_field,
                    ids.now_later_field,
                    ids.teleport_field,
                    ids.comment_field,
                    ids.open_ui_menu_field,
                    ids.open_instance_field,
                ] {
                    super::super::show(ui, f(ui, id), false);
                }
                super::super::show(ui, f(ui, ids.continue_field), true);
                w.closers = [ids.quest_close, ids.continue_button]
                    .into_iter()
                    .filter_map(|id| f(ui, id))
                    .collect();
                // §5.2 Tessera rule (font size): the arrows step the intro text, the
                // mission text and the sample through their font lists.
                let (up, down) = (f(ui, ids.bestow_font_up), f(ui, ids.bestow_font_down));
                w.fonts = FontLadder::new(ui, &mut w.listen, up, down);
                for id in [ids.intro_text, ids.mission_text, ids.bestow_font_example] {
                    let k = f(ui, id);
                    w.fonts.add(ui, nv, k);
                }
            }
            DialogueKind::Choice => {
                w.title = f(ui, ids.panel_title);
                w.list = f(ui, ids.action_list);
                w.closers = f(ui, ids.panel_close).into_iter().collect();
            }
            DialogueKind::Comment => {
                w.title = f(ui, ids.comment_title);
                w.window_label = f(ui, ids.comment_window_label);
                w.text = f(ui, ids.comment_content);
                w.close_text = f(ui, ids.comment_close_text);
                w.closers = [ids.comment_close_text, ids.comment_close]
                    .into_iter()
                    .filter_map(|id| f(ui, id))
                    .collect();
            }
        }
        for c in w.closers.clone() {
            w.listen.clicks(ui, Some(c));
        }
        w
    }

    /// Shows `page` (§5.2–§5.4). For the choice window the list is rebuilt: the greeting
    /// row with the page text, then one row per option in ascending option number (§5.3
    /// Tessera rule (choice page)); option numbers are not drawn.
    pub fn show(&mut self, ui: &mut Ui, page: &DialoguePage, texts: &dyn NpcTexts) {
        self.window = page.window;
        let speaker = page.speaker.clone().unwrap_or_default();
        match self.kind {
            DialogueKind::Conversation => {
                put_text(ui, self.title, &speaker);
                put_text(ui, self.text, &page.text);
            }
            DialogueKind::Comment => {
                let c = page.comment.clone().unwrap_or_default();
                put_text(ui, self.title, &c.title);
                put_text(ui, self.window_label, &c.window_label);
                put_text(ui, self.text, &page.text);
                let close = if c.close_text.is_empty() {
                    label_text(texts, NpcLabel::Close)
                } else {
                    c.close_text
                };
                put_text(ui, self.close_text, &close);
            }
            DialogueKind::Choice => {
                put_text(ui, self.title, &speaker);
                self.fill_choices(ui, page);
            }
        }
    }

    fn fill_choices(&mut self, ui: &mut Ui, page: &DialoguePage) {
        let Some(list) = self.list else { return };
        let mut old: Vec<ElementKey> = self.rows.drain(..).map(|(k, _)| k).collect();
        old.extend(self.text.take());
        super::kit::clear_rows(ui, list, &mut old);
        let cp = self.nv.pids.text_font_color;
        let markup = self.nv.pids.parse_markup;
        // The greeting row.
        let greeting_id = self.nv.ids.greeting_template;
        let greeting = make_row(ui, list, self.templates.greeting, || {
            FNode::text(greeting_id, 0, 0, ROW_WIDTH, 16, cp, colours::BODY, "")
                .prop(markup, tessera_formats::property::PropertyValue::Bool(true))
        });
        if let Some(g) = greeting {
            let t = text_part(ui, g, None).unwrap_or(g);
            put_text(ui, Some(t), &page.text);
            if t == g {
                fit_height(ui, g, 16, MAX_GREETING_HEIGHT);
            }
            self.text = Some(g);
        }
        let ids = self.nv.ids;
        let art = self.art.clone();
        let ghosted = ui.vocab().states.ghosted;
        for o in page.ordered_options() {
            let row = make_row(ui, list, self.templates.row, || {
                FNode::button(
                    ids.misc_template,
                    0,
                    0,
                    ROW_WIDTH,
                    ROW_HEIGHT,
                    (&art.row, &art.row),
                    cp,
                    ghosted,
                    "",
                )
                .child(FNode::field(ids.action_icon, 7, 6, 26, 24))
                .child(FNode::text(
                    OPTION_LABEL_ID,
                    38,
                    8,
                    ROW_WIDTH - 42,
                    18,
                    cp,
                    colours::TITLE,
                    "",
                ))
            });
            let Some(row) = row else { break };
            // A fallback row's text is its Text child; a template row (a Button) shows its
            // own text.
            let label = text_part(ui, row, None);
            put_text(ui, label, &cut(&o.text));
            if let Some(img) = &o.icon
                && let Some(ic) = icon_part(ui, row, Some(ids.action_icon))
            {
                set_image_ref(ui, ic, img, super::kit::ART_MODE);
            }
            self.listen.clicks(ui, Some(row));
            if let Some(t) = label.filter(|t| *t != row) {
                self.listen.clicks(ui, Some(t));
            }
            self.rows.push((row, o.option));
        }
        let all: Vec<ElementKey> = self
            .text
            .into_iter()
            .chain(self.rows.iter().map(|(k, _)| *k))
            .collect();
        let kept = commit_rows(ui, list, &all);
        self.rows.retain(|(k, _)| kept.contains(k));
    }

    /// The command for a UI event (§5.3 rule 4, §5.7): a click on a close button, Continue
    /// or the comment's close button sends `CloseWindow`; a left click on an option row
    /// sends `ChooseOption`; a font-size arrow that changes the font index gives
    /// [`NpcAction::FontSize`] (a setting the client keeps, §5.2 Tessera rule).
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<NpcAction> {
        let (src, kind) = click_of(ui, ev)?;
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.root, src) {
            return None;
        }
        if matches!(kind, ClickKind::Button | ClickKind::Left)
            && let Some(changed) = self.fonts.click(ui, &self.nv, src)
        {
            return changed.map(|i| NpcAction::FontSize {
                index: u32::try_from(i).unwrap_or(u32::MAX),
            });
        }
        if matches!(kind, ClickKind::Button | ClickKind::Left)
            && self.closers.iter().any(|c| ui.is_ancestor_or_self(*c, src))
        {
            return Some(self.close_action());
        }
        if matches!(kind, ClickKind::Button | ClickKind::Left) {
            let rows: Vec<ElementKey> = self.rows.iter().map(|(k, _)| *k).collect();
            if let Some(i) = row_of(ui, &rows, src) {
                return Some(NpcAction::ChooseOption {
                    window: self.window,
                    option: self.rows[i].1,
                });
            }
        }
        None
    }

    /// Restores the conversation pane's font index from Tessera's settings (`fonts.md`
    /// §9.2: kept only when one of the texts' font lists is long enough). Returns whether
    /// it was taken.
    pub fn set_font_index(&mut self, ui: &mut Ui, index: u32) -> bool {
        let i = usize::try_from(index).unwrap_or(usize::MAX);
        self.fonts.set(ui, &self.nv, i)
    }

    /// The conversation pane's font index (0 at first).
    #[must_use]
    pub fn font_index(&self) -> u32 {
        u32::try_from(self.fonts.index()).unwrap_or(u32::MAX)
    }

    /// `CloseWindow` for this window (close buttons, `EscapeKey`, §5.7).
    #[must_use]
    pub fn close_action(&self) -> NpcAction {
        NpcAction::CloseWindow {
            window: self.window,
        }
    }

    /// Removes the game-code handlers (before the window is destroyed or reused).
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}

/// Moves `k` so that its centre lies on `other`'s centre (§5.1: a page that needs another
/// native window shows it at the old one's position, centre to centre).
pub fn centre_on(ui: &mut Ui, k: ElementKey, other: ElementKey) {
    let (Some(a), Some(b)) = (ui.layout_box(k), ui.layout_box(other)) else {
        return;
    };
    let cx = b.l.saturating_add(b.w() / 2);
    let cy = b.t.saturating_add(b.h() / 2);
    ui.move_to(
        k,
        cx.saturating_sub(a.w() / 2),
        cy.saturating_sub(a.h() / 2),
    );
}

/// Element IDs of the dialogue windows' parts, for tests and the client.
#[must_use]
pub fn part(ui: &Ui, root: ElementKey, id: u32) -> Option<ElementKey> {
    find(ui, root, id)
}
