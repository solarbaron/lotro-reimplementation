// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest log's details pane `SelectedQuestDisplay` and the history window's list
//! (`quest-journal.md` §4.7, §4.9): a vertical list of sections built from the templates of
//! layout `questdisplay`, each sized to the pane's width.

use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::examine::ExamTemplate;
use super::super::npc::kit::{
    FNode, FallbackArt, clear_rows, colours, commit_rows, find, fit_height, make_row, put_text,
    text_part,
};
use super::super::npc::offer::{GroupType, RewardRow, RewardRows, RowTemplates};
use super::super::npcvocab::NpcVocab;
use super::super::show;
use super::QuestTexts;

/// Most sections a pane shows (Tessera bound).
pub const MAX_SECTIONS: usize = 512;

/// Most height one text section takes (Tessera bound; the pane scrolls).
pub const MAX_TEXT_HEIGHT: i32 = 4096;

/// One section of the details pane, in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailsSection {
    /// `QuestDisplay_QuestHeaderField` (§4.7 item 1).
    Header {
        /// The title (§4.4 format).
        title: String,
        /// The arc name (`None`: hidden, the category moves up).
        arc: Option<String>,
        /// The category text (display text only).
        category: String,
        /// The header icon state.
        group: GroupType,
    },
    /// The objective block (item 2): one bulleted entry per progress line (§5.2 step 3).
    Progress(Vec<String>),
    /// A label row (`QuestDisplay_QuestDescriptionLabel`, the bestowal and objective
    /// labels).
    Label(String),
    /// A text row (objective text, description, bestowal dialogue, repeat note).
    Text(String),
    /// The reward block (item 6, the rows of `npc-windows.md` §6.3).
    Rewards(Vec<RewardRow>),
}

/// The selected quest's details, rendered by the client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestDetails {
    /// The sections in display order ([`underway_sections`], [`completed_sections`]).
    pub sections: Vec<DetailsSection>,
}

/// The parts of an underway quest's pane (§4.7 items 1–6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnderwayParts {
    /// Title, arc, category and group of the header.
    pub title: String,
    /// The arc name.
    pub arc: Option<String>,
    /// The category text.
    pub category: String,
    /// The group type (header icon).
    pub group: GroupType,
    /// The progress lines of the current objective (§5.2).
    pub progress: Vec<String>,
    /// The current objective's `Quest_ObjectiveDescription` (left out when absent).
    pub objective_text: Option<String>,
    /// The background label and `Quest_Description` (left out without a description).
    pub description: Option<(String, String)>,
    /// The repeat note.
    pub repeat_note: Option<String>,
    /// The reward rows (treasure-record rows only, §4.7 Tessera rule).
    pub rewards: Vec<RewardRow>,
}

/// The sections of an underway quest in the order of §4.7: header, objective block,
/// objective text, background, repeat note, rewards (the filter row is hidden).
#[must_use]
pub fn underway_sections(p: &UnderwayParts) -> Vec<DetailsSection> {
    let mut v = vec![DetailsSection::Header {
        title: p.title.clone(),
        arc: p.arc.clone(),
        category: p.category.clone(),
        group: p.group,
    }];
    if !p.progress.is_empty() {
        v.push(DetailsSection::Progress(p.progress.clone()));
    }
    if let Some(t) = p.objective_text.as_ref().filter(|t| !t.is_empty()) {
        v.push(DetailsSection::Text(t.clone()));
    }
    if let Some((l, d)) = &p.description {
        v.push(DetailsSection::Label(l.clone()));
        v.push(DetailsSection::Text(d.clone()));
    }
    if let Some(r) = &p.repeat_note {
        v.push(DetailsSection::Text(r.clone()));
    }
    if !p.rewards.is_empty() {
        v.push(DetailsSection::Rewards(p.rewards.clone()));
    }
    v
}

/// The sections of a completed quest or the history window (§4.7, §4.9): header; the
/// bestowal label and dialogue; background; each objective's label and description;
/// rewards (left out in the history window, which passes none).
#[must_use]
pub fn completed_sections(
    header: DetailsSection,
    bestowal: Option<(String, String)>,
    description: Option<(String, String)>,
    objectives: &[(String, String)],
    rewards: &[RewardRow],
) -> Vec<DetailsSection> {
    let mut v = vec![header];
    for (l, t) in bestowal.iter().chain(description.iter()) {
        v.push(DetailsSection::Label(l.clone()));
        v.push(DetailsSection::Text(t.clone()));
    }
    for (l, t) in objectives.iter().take(MAX_SECTIONS) {
        v.push(DetailsSection::Label(l.clone()));
        v.push(DetailsSection::Text(t.clone()));
    }
    if !rewards.is_empty() {
        v.push(DetailsSection::Rewards(rewards.to_vec()));
    }
    v.truncate(MAX_SECTIONS);
    v
}

/// The section templates of layout `questdisplay` (§4.7). `None` uses a fallback row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DetailsTemplates {
    /// `QuestDisplay_QuestHeaderField`.
    pub header: Option<ExamTemplate>,
    /// `QuestDisplay_ObjectivesField`'s entry (`QuestObjectives_Objective`).
    pub objective: Option<ExamTemplate>,
    /// A label row (`QuestDisplay_QuestDescriptionLabel`).
    pub label: Option<ExamTemplate>,
    /// A text row (`QuestDisplay_QuestDescription`).
    pub text: Option<ExamTemplate>,
    /// The reward rows.
    pub rewards: RowTemplates,
}

/// The details pane.
#[derive(Debug, Clone)]
pub struct DetailsPane {
    /// The pane (`SelectedQuestDisplay`).
    pub pane: ElementKey,
    /// The section rows.
    pub rows: Vec<ElementKey>,
    /// The reward rows.
    pub rewards: Option<RewardRows>,
    /// The text rows with their text element, fitted to their text (again after a font
    /// change, [`Self::refit`]).
    fitted: Vec<(ElementKey, ElementKey)>,
    templates: DetailsTemplates,
    art: FallbackArt,
    nv: NpcVocab,
}

impl DetailsPane {
    /// A pane on `pane`.
    pub fn new(
        ui: &mut Ui,
        nv: &NpcVocab,
        pane: ElementKey,
        templates: DetailsTemplates,
        art: &FallbackArt,
    ) -> Self {
        let _ = ui;
        Self {
            pane,
            rows: Vec::new(),
            rewards: None,
            fitted: Vec::new(),
            templates,
            art: art.clone(),
            nv: nv.clone(),
        }
    }

    fn width(&self, ui: &Ui) -> i32 {
        ui.layout_box(self.pane)
            .map_or(400, |b| b.w())
            .clamp(120, 2048)
    }

    /// Shows the sections (or nothing).
    pub fn show(&mut self, ui: &mut Ui, details: Option<&QuestDetails>, texts: &dyn QuestTexts) {
        if let Some(r) = self.rewards.as_mut() {
            r.clear(ui);
        }
        self.rewards = None;
        self.fitted.clear();
        clear_rows(ui, self.pane, &mut self.rows);
        let Some(d) = details else { return };
        let w = self.width(ui);
        let ids = self.nv.ids;
        let cp = self.nv.pids.text_font_color;
        let markup = self.nv.pids.parse_markup;
        let st = self.nv.states;
        let art = self.art.clone();
        let mut committed = 0usize;
        for s in d.sections.iter().take(MAX_SECTIONS) {
            match s {
                DetailsSection::Header {
                    title,
                    arc,
                    category,
                    group,
                } => {
                    let Some(k) = make_row(ui, self.pane, self.templates.header, || {
                        FNode::field(ids.header_field, 0, 0, w, 64)
                            .image(&art.panel)
                            .child(
                                FNode::field(ids.header_icon, 12, 12, 20, 20)
                                    .state(st.quest_solo, art.solo.as_ref(), vec![])
                                    .state(
                                        st.quest_small_fellowship,
                                        art.small_fellowship.as_ref(),
                                        vec![],
                                    )
                                    .state(st.quest_fellowship, art.fellowship.as_ref(), vec![])
                                    .state(st.quest_raid, art.raid.as_ref(), vec![]),
                            )
                            .child(FNode::text(
                                ids.header_title,
                                46,
                                4,
                                w - 52,
                                18,
                                cp,
                                colours::YELLOW,
                                "",
                            ))
                            .child(FNode::text(
                                ids.header_arc,
                                46,
                                24,
                                w - 52,
                                16,
                                cp,
                                colours::TITLE,
                                "",
                            ))
                            .child(FNode::text(
                                ids.header_category,
                                46,
                                42,
                                w - 52,
                                16,
                                cp,
                                colours::BODY,
                                "",
                            ))
                            .child(FNode::field(ids.header_xp_tier, 0, 0, 1, 1).hidden())
                    }) else {
                        break;
                    };
                    size_to_width(ui, k, w);
                    let f = |ui: &Ui, id: u32| find(ui, k, id);
                    put_text(ui, f(ui, ids.header_title), title);
                    let arc_el = f(ui, ids.header_arc);
                    let cat_el = f(ui, ids.header_category);
                    put_text(ui, cat_el, category);
                    match arc {
                        Some(a) => {
                            put_text(ui, arc_el, a);
                            show(ui, arc_el, true);
                        }
                        None => {
                            // §4.7: the arc name hidden, the category moves up to the mean
                            // of the two elements' tops.
                            show(ui, arc_el, false);
                            if let (Some(a), Some(c)) = (arc_el, cat_el)
                                && let (Some(ab), Some(cb)) = (ui.layout_box(a), ui.layout_box(c))
                            {
                                ui.move_to(c, cb.l, (ab.t + cb.t) / 2);
                            }
                        }
                    }
                    show(ui, f(ui, ids.header_xp_tier), false);
                    if let Some(i) = f(ui, ids.header_icon) {
                        let s = match group {
                            GroupType::Raid => st.quest_raid,
                            GroupType::Fellowship => st.quest_fellowship,
                            GroupType::SmallFellowship => st.quest_small_fellowship,
                            GroupType::Solo => st.quest_solo,
                        };
                        ui.switch_state_if_defined(i, s);
                    }
                    self.rows.push(k);
                }
                DetailsSection::Progress(lines) => {
                    for line in lines.iter().take(MAX_SECTIONS) {
                        let Some(k) = make_row(ui, self.pane, self.templates.objective, || {
                            FNode::text(ids.objective, 0, 0, w, 16, cp, colours::YELLOW, "")
                                .prop(markup, tessera_formats::property::PropertyValue::Bool(true))
                        }) else {
                            break;
                        };
                        self.text_row(ui, k, Some(ids.objective), line);
                    }
                }
                DetailsSection::Label(t) => {
                    let Some(k) = make_row(ui, self.pane, self.templates.label, || {
                        FNode::text(
                            ids.details_description_label,
                            0,
                            0,
                            w,
                            18,
                            cp,
                            colours::TITLE,
                            "",
                        )
                    }) else {
                        break;
                    };
                    self.text_row(ui, k, None, t);
                }
                DetailsSection::Text(t) => {
                    let Some(k) = make_row(ui, self.pane, self.templates.text, || {
                        FNode::text(ids.details_description, 0, 0, w, 16, cp, colours::BODY, "")
                            .prop(markup, tessera_formats::property::PropertyValue::Bool(true))
                    }) else {
                        break;
                    };
                    self.text_row(ui, k, None, t);
                }
                DetailsSection::Rewards(rows) => {
                    self.commit(ui, &mut committed);
                    let mut r =
                        RewardRows::new(self.pane, &self.nv, self.templates.rewards, &self.art);
                    r.rewards(ui, rows, texts);
                    self.rewards = Some(r);
                }
            }
        }
        self.commit(ui, &mut committed);
    }

    /// Makes the rows made since `committed` entries of the pane (one layout).
    fn commit(&mut self, ui: &mut Ui, committed: &mut usize) {
        let new: Vec<ElementKey> = self.rows[(*committed).min(self.rows.len())..].to_vec();
        let kept = commit_rows(ui, self.pane, &new);
        self.rows.truncate(*committed);
        self.rows.extend(kept);
        *committed = self.rows.len();
    }

    fn text_row(&mut self, ui: &mut Ui, k: ElementKey, id: Option<u32>, text: &str) {
        let w = self.width(ui);
        size_to_width(ui, k, w);
        let t = text_part(ui, k, id).unwrap_or(k);
        put_text(ui, Some(t), text);
        fit_row(ui, k, t);
        self.fitted.push((k, t));
        self.rows.push(k);
    }

    /// Fits the text rows again (after their font changed: the font-size arrows, or the
    /// arrows' index applied to rows made at the default font) and lays the pane out again.
    pub fn refit(&mut self, ui: &mut Ui) {
        self.fitted.retain(|(k, t)| ui.exists(*k) && ui.exists(*t));
        for (k, t) in self.fitted.clone() {
            fit_row(ui, k, t);
        }
        if ui.class(self.pane).is_some_and(crate::WidgetClass::is_list) {
            ui.list_layout(self.pane);
        }
    }

    /// Nothing to release (the pane registers no handlers); kept for symmetry.
    pub fn detach(&mut self, ui: &mut Ui) {
        let _ = ui;
    }
}

/// Fits text row `k` (text element `t`, possibly `k` itself) to its wrapped text. A template
/// row whose text is a child grows or shrinks with its text, so that the next section starts
/// below it (§4.7: sections are stacked, never overlapped).
fn fit_row(ui: &mut Ui, k: ElementKey, t: ElementKey) {
    if t == k {
        fit_height(ui, k, 16, MAX_TEXT_HEIGHT);
        return;
    }
    let before = ui.layout_box(t).map_or(0, |b| b.h());
    let (h, _) = fit_height(ui, t, 16, MAX_TEXT_HEIGHT);
    let grow = h - before;
    if grow != 0
        && let Some(rb) = ui.layout_box(k)
    {
        ui.resize(k, rb.w(), rb.h().saturating_add(grow).max(1));
        // A text anchored to the row's top and bottom followed the row; one anchored only
        // at the top keeps the fitted height.
        if ui.layout_box(t).is_some_and(|b| b.h() != h) {
            let tw = ui.layout_box(t).map_or(0, |b| b.w());
            ui.resize(t, tw, h);
        }
    }
}

/// Sizes a section row to the pane's width (§4.7: each element of the pane is sized to the
/// pane's width), keeping its height; its children follow by their own layout rules.
fn size_to_width(ui: &mut Ui, k: ElementKey, w: i32) {
    let Some(b) = ui.layout_box(k) else { return };
    if b.w() != w {
        ui.resize(k, w, b.h());
    }
}
