// SPDX-License-Identifier: GPL-3.0-or-later
//! Fallback journal windows (Tessera rule, decision 0031 tier 1): the quest log P9 and the
//! quest tracker box P11 built from code-made elements with the install's element IDs
//! ([`NpcVocab`]), at the design sizes of `quest-journal.md` §4.1 and §6.1.

use tessera_formats::property::PropertyValue;

use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::super::GameError;
use super::super::npc::kit::{FNode, FallbackArt, colours, open_root};
use super::super::npcvocab::NpcVocab;
use super::log::{
    SCOPE_EPIC, SCOPE_FELLOWSHIP, SCOPE_INSTANCE, SCOPE_RAID, SCOPE_REPEATABLE,
    SCOPE_SMALL_FELLOWSHIP, SCOPE_TASK,
};
use super::{QuestLabel, QuestTexts, quest_label_text};

/// The `Solo` scope bit (§4.5).
const SCOPE_SOLO: u32 = 0x20;

/// The `Shared` scope bit (§4.5).
const SCOPE_SHARED: u32 = 0x200;

/// The fallback filter buttons: (mask, short text). Tessera's own short texts.
const FILTERS: [(u32, &str); 9] = [
    (SCOPE_SOLO, "So"),
    (SCOPE_SMALL_FELLOWSHIP, "Sf"),
    (SCOPE_FELLOWSHIP, "Fe"),
    (SCOPE_INSTANCE, "In"),
    (SCOPE_EPIC, "Ep"),
    (SCOPE_RAID, "Ra"),
    (SCOPE_REPEATABLE, "Re"),
    (SCOPE_SHARED, "Sh"),
    (SCOPE_TASK, "Ta"),
];

/// The quest log window (`QuestPanel`, 770 × 550, §4.1).
///
/// # Errors
/// The element cap or an attachment error.
pub fn quest_log(
    ui: &mut Ui,
    nv: &NpcVocab,
    art: &FallbackArt,
    texts: &dyn QuestTexts,
) -> Result<ElementKey, GameError> {
    let v = ui.vocab().clone();
    let ids = nv.ids;
    let cp = nv.pids.text_font_color;
    let (w, h) = (770, 550);
    let label = |l: QuestLabel| quest_label_text(texts, l);
    let button = |id: u32, x: i32, y: i32, bw: i32, bh: i32, t: &str| {
        FNode::button(
            id,
            x,
            y,
            bw,
            bh,
            (&art.button, &art.button_ghosted),
            cp,
            v.states.ghosted,
            t,
        )
    };
    let mut filters = FNode::field(ids.filters, 10, 30, 356, 40);
    for (i, (mask, t)) in FILTERS.iter().enumerate() {
        let x = i32::try_from(i).unwrap_or(0) * 34;
        filters = filters.child(
            button(0, x, 2, 32, 22, t)
                .prop(nv.pids.filter_scope, PropertyValue::Bitfield32(*mask))
                .state(v.states.highlight, art.tab_selected.as_ref(), vec![]),
        );
    }
    let root = FNode::field(ids.quest_panel, 9, 118, w, h)
        .image(&art.window)
        .child(
            FNode::field(0, 0, 0, w, 24)
                .image(&art.title_bar)
                .child(FNode::text(
                    0,
                    8,
                    4,
                    300,
                    16,
                    cp,
                    colours::TITLE,
                    &label(QuestLabel::LogTitle),
                )),
        )
        .child(FNode::button(
            ids.quest_panel_close,
            w - 24,
            4,
            16,
            16,
            (&art.close, &art.close),
            cp,
            v.states.ghosted,
            "",
        ))
        .child(filters)
        .child(button(
            ids.show_completed,
            330,
            32,
            100,
            22,
            &label(QuestLabel::ShowCompleted),
        ))
        .child(
            FNode::text(ids.filter_text, 440, 32, 160, 20, cp, colours::WHITE, "")
                .image(&art.entry)
                .prop(v.pids.text_editable, PropertyValue::Bool(true)),
        )
        .child(FNode::text(
            ids.count_text,
            620,
            34,
            140,
            16,
            cp,
            colours::TITLE,
            "",
        ))
        .child(button(
            ids.expand_all,
            12,
            79,
            16,
            16,
            &label(QuestLabel::ExpandAll),
        ))
        .child(button(
            ids.collapse_all,
            32,
            79,
            16,
            16,
            &label(QuestLabel::CollapseAll),
        ))
        .child(FNode::text(
            ids.shown_label,
            56,
            79,
            276,
            16,
            cp,
            colours::BODY,
            "",
        ))
        .child(FNode::new(WidgetClass::ListBox, ids.quest_tree, 11, 99, 321, 384).image(&art.panel))
        .child(
            FNode::new(
                WidgetClass::ListBox,
                ids.selected_display,
                344,
                80,
                410,
                401,
            )
            .image(&art.panel),
        )
        .child(button(
            ids.track_button,
            344,
            500,
            90,
            24,
            &label(QuestLabel::Track),
        ))
        .child(button(
            ids.share_button,
            438,
            500,
            70,
            24,
            &label(QuestLabel::Share),
        ))
        .child(button(
            ids.abandon_button,
            512,
            500,
            90,
            24,
            &label(QuestLabel::Abandon),
        ))
        .child(button(
            ids.history_button,
            606,
            500,
            90,
            24,
            &label(QuestLabel::History),
        ))
        .child(button(ids.find_fellowship_button, 700, 500, 54, 24, ""))
        .child(button(ids.quest_font_down, 12, 500, 28, 24, "A-"))
        .child(button(ids.quest_font_up, 44, 500, 28, 24, "A+"))
        .child(FNode::text(
            ids.quest_font_example,
            78,
            504,
            40,
            16,
            cp,
            colours::BODY,
            "Aa",
        ))
        .child(
            button(
                ids.show_tracker,
                620,
                54,
                140,
                20,
                &label(QuestLabel::ShowTracker),
            )
            .state(v.states.highlight, art.tab_selected.as_ref(), vec![]),
        )
        .child(FNode::text(
            ids.task_count_text,
            440,
            56,
            160,
            16,
            cp,
            colours::TITLE,
            "",
        ))
        .child(FNode::field(ids.quest_slot_store, 0, 0, 1, 1).hidden());
    open_root(ui, &root)
}

/// The quest tracker box (`GamePlay_QuestTrackerBox`, 340 × 550, §6.1) with its list
/// `GamePlay_QuestTracker`; returns the root and the list.
///
/// # Errors
/// The element cap or an attachment error.
pub fn tracker(
    ui: &mut Ui,
    nv: &NpcVocab,
    art: &FallbackArt,
    texts: &dyn QuestTexts,
) -> Result<(ElementKey, Option<ElementKey>), GameError> {
    let ids = nv.ids;
    let cp = nv.pids.text_font_color;
    let title = quest_label_text(texts, QuestLabel::TrackerTitle);
    let root = FNode::field(ids.tracker_box, 670, 223, 340, 550)
        .image(&art.panel)
        .child(FNode::text(0, 4, 2, 330, 16, cp, colours::TITLE, &title))
        .child(FNode::new(
            WidgetClass::ListBox,
            ids.tracker,
            0,
            20,
            340,
            530,
        ));
    let k = open_root(ui, &root)?;
    let list = super::super::npc::kit::find(ui, k, ids.tracker);
    Ok((k, list))
}
