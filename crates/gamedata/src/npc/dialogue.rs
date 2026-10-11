// SPDX-License-Identifier: GPL-3.0-or-later
//! Dialogue (`npc-windows.md` §5): which native window shows a page, the text of each feed
//! `Text` form with the player variables, the comment window and the choice page with the
//! action entries of the choice window's layout.

use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::gameplay::quests::{NpcComment, QuestRecord};
use tessera_formats::{Did, PropertyValue, StringInfo};
use tessera_text::{RichText, parse_markup};

use super::offer::role_line;
use super::{MAX_ACTION_ENTRIES, NpcPresenter};
use crate::quest::{
    PlayerVars, element_property, find_element, is_player_variable, layout_element,
};
use crate::tooltip::TextRef;

/// `UILAYOUT` label of the choice window (§4).
const ACTION_PICKER_LAYOUT: &str = "npc_actionpicker";
/// The choice window's root (§4).
const ACTION_PICKER_ROOT: &str = "RootActionPickerField";

/// A feed `Text` (`scenario-feed.md` §5.2, §9.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogueText {
    /// A literal string.
    Literal(String),
    /// A record's display name.
    Name(Did),
    /// A record's description.
    Description(Did),
    /// The success or failure line of the *N*-th role (0-based, stored order) of a quest's
    /// `Quest_RoleArray`.
    QuestRole {
        /// The quest.
        quest: Did,
        /// The role index.
        role: u16,
        /// The failure line.
        failure: bool,
    },
    /// `NPCComment_Content` of a comment record.
    Comment(Did),
    /// `Quest_ObjectiveDescription` of the objective with this index.
    Objective {
        /// The quest.
        quest: Did,
        /// The objective index.
        objective: u16,
    },
    /// An NPC action's text (§5.3 rule 3).
    Action(u32),
}

/// The census category of a dialogue line (§11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LineCategory {
    /// Renders a non-empty text.
    Shown,
    /// The text or its StringInfo cannot be found.
    TextMissing,
    /// An argument outside the player variables of §5.5.
    VariableUnsupplied,
    /// Renders to an empty text.
    Empty,
}

/// A resolved dialogue line.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogueLine {
    /// The text (`None`: the element shows an empty text).
    pub text: Option<RichText>,
    /// The census category.
    pub category: LineCategory,
}

impl DialogueLine {
    fn missing() -> Self {
        Self {
            text: None,
            category: LineCategory::TextMissing,
        }
    }
}

/// Which native window shows a dialogue page (§5.1 Tessera rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogueWindow {
    /// The NPC comment window.
    Comment,
    /// The conversation window with only the Continue button.
    Conversation,
    /// The choice window.
    Choice,
}

/// The native window of a `DialogueShown` (§5.1).
#[must_use]
pub fn dialogue_window(text: &DialogueText, options: usize) -> DialogueWindow {
    match (text, options) {
        (DialogueText::Comment(_), 0) => DialogueWindow::Comment,
        (_, 0) => DialogueWindow::Conversation,
        _ => DialogueWindow::Choice,
    }
}

/// The NPC comment window's texts (§5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct CommentWindow {
    /// `NPCComment_Title`.
    pub title: DialogueLine,
    /// `NPCComment_WindowLabel`.
    pub window_label: DialogueLine,
    /// `NPCComment_Content`.
    pub content: DialogueLine,
    /// `NPCComment_CloseButtonText`.
    pub close: DialogueLine,
}

/// One entry of `ActionPickerUI_ActionEntryArray` (§5.3).
#[derive(Debug, Clone, PartialEq)]
pub struct ActionEntry {
    /// `ActionPickerUI_Action` (a key of `NPCActionPickerType`).
    pub action: u32,
    /// `ActionPickerUI_ListEntryTemplate` (an element ID); `None`: the entry uses
    /// `ActionPickerMiscTemplate`.
    pub template: Option<u32>,
    /// `ActionPickerUI_EntryTextProperty`: a property of the NPC that may hold its text.
    pub text_property: Option<u32>,
    /// `ActionPickerUI_DefaultTextEntry`.
    pub default_text: Option<StringInfo>,
}

/// One option row of a choice page.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceRow {
    /// The option number (not drawn; sent with `ChooseOption`).
    pub option: u8,
    /// The text.
    pub text: DialogueLine,
    /// The row template's element ID (`None`: `ActionPickerMiscTemplate`).
    pub template: Option<u32>,
}

/// A choice page (§5.3 Tessera rule): the greeting row, then one row per option in
/// ascending option number.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoicePage {
    /// The page text (an `ActionPickerGreetingTemplateText` row).
    pub greeting: DialogueLine,
    /// The options.
    pub options: Vec<ChoiceRow>,
}

fn element_id(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) | PropertyValue::PropertyId(k) => {
            Some(*k)
        }
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        _ => None,
    }
}

impl NpcPresenter<'_> {
    /// Renders a StringInfo as a dialogue line (§5.5, census categories of §11).
    #[must_use]
    pub fn line(&self, info: Option<&StringInfo>, player: &PlayerVars) -> DialogueLine {
        let Some(info) = info else {
            return DialogueLine::missing();
        };
        let args = match self.data().text_args(TextRef::Info(info)) {
            Ok(a) => a,
            Err(e) => {
                tracing::debug!(error = %e, "dialogue text missing");
                return DialogueLine::missing();
            }
        };
        let unsupplied = args.iter().any(|&id| !is_player_variable(id));
        match self.data().render(TextRef::Info(info), &player.values) {
            Err(_) => DialogueLine::missing(),
            Ok(_) if unsupplied => DialogueLine {
                text: None,
                category: LineCategory::VariableUnsupplied,
            },
            Ok(Some(t)) if !t.plain_text().trim().is_empty() => DialogueLine {
                text: Some(t),
                category: LineCategory::Shown,
            },
            Ok(_) => DialogueLine {
                text: None,
                category: LineCategory::Empty,
            },
        }
    }

    fn record_info(
        &self,
        did: Did,
        pids: &[tessera_formats::gameplay::common::Pid],
    ) -> Option<StringInfo> {
        let set = self.data().record(did).ok()??;
        pids.iter().find_map(|&p| set.string_info(p)).cloned()
    }

    /// The text of a feed `Text` (§5.6). `speaker_body` is the speaker's body record
    /// (for `Action`).
    #[must_use]
    pub fn dialogue_text(
        &self,
        text: &DialogueText,
        speaker_body: Option<Did>,
        player: &PlayerVars,
    ) -> DialogueLine {
        let p = self.pids();
        let qp = self.quest();
        let q = &qp.labels().quest;
        match text {
            DialogueText::Literal(s) => {
                let t = parse_markup(s);
                let empty = t.plain_text().trim().is_empty();
                DialogueLine {
                    text: (!empty).then_some(t),
                    category: if empty {
                        LineCategory::Empty
                    } else {
                        LineCategory::Shown
                    },
                }
            }
            DialogueText::Name(did) => {
                let info = self.record_info(
                    *did,
                    &[
                        q.name,
                        p.name,
                        p.skill_name,
                        p.effect_name,
                        p.trait_name,
                        p.travel_name,
                        p.area_name,
                        p.dungeon_name,
                        p.faction_name,
                        p.recipe_name,
                        p.profession_name,
                        p.instance_name,
                        p.title_string,
                    ],
                );
                self.line(info.as_ref(), player)
            }
            DialogueText::Description(did) => {
                let info = self.record_info(*did, &[q.description, p.description]);
                self.line(info.as_ref(), player)
            }
            DialogueText::QuestRole {
                quest,
                role,
                failure,
            } => {
                let Ok(set) = qp.record(*quest) else {
                    return DialogueLine::missing();
                };
                let rec = QuestRecord::new_unchecked(qp.labels(), &set);
                let roles = rec.roles();
                let info = roles
                    .get(usize::from(*role))
                    .and_then(|r| role_line(r, *failure, player.context.species));
                self.line(info, player)
            }
            DialogueText::Comment(did) => {
                let info = self.record_info(*did, &[p.comment_content]);
                self.line(info.as_ref(), player)
            }
            DialogueText::Objective { quest, objective } => {
                let Ok(set) = qp.record(*quest) else {
                    return DialogueLine::missing();
                };
                let rec = QuestRecord::new_unchecked(qp.labels(), &set);
                let objectives = rec.objectives();
                let info = objectives
                    .iter()
                    .find(|o| o.index() == Some(i32::from(*objective)))
                    .and_then(|o| o.description());
                self.line(info, player)
            }
            DialogueText::Action(a) => self.action_text(*a, speaker_body, player).0,
        }
    }

    /// The comment window of a comment record (§5.4).
    #[must_use]
    pub fn comment_window(&self, comment: Did, player: &PlayerVars) -> CommentWindow {
        let set = self.data().record(comment).ok().flatten();
        let view = set
            .as_ref()
            .map(|s| NpcComment::new(self.quest().labels(), s));
        CommentWindow {
            title: self.line(view.and_then(|v| v.title()), player),
            window_label: self.line(view.and_then(|v| v.window_label()), player),
            content: self.line(view.and_then(|v| v.content()), player),
            close: self.line(view.and_then(|v| v.close_button()), player),
        }
    }

    /// The action entries of the choice window's root (§5.3), read once.
    #[must_use]
    pub fn action_entries(&self) -> &[ActionEntry] {
        self.actions.get_or_init(|| {
            let p = self.pids();
            let Ok((layout, id)) =
                layout_element(self.data(), ACTION_PICKER_LAYOUT, ACTION_PICKER_ROOT)
            else {
                return Vec::new();
            };
            let Some(root) = find_element(&layout, id) else {
                return Vec::new();
            };
            let Some(PropertyValue::Array(a)) = p
                .action_entries
                .id()
                .and_then(|pid| element_property(root, pid))
            else {
                return Vec::new();
            };
            a.elements
                .iter()
                .take(MAX_ACTION_ENTRIES)
                .filter_map(|e| {
                    let PropertyValue::Struct(s) = &e.value else {
                        return None;
                    };
                    Some(ActionEntry {
                        action: s.enum_key(p.action)?,
                        template: s.get_pid(p.action_template).and_then(element_id),
                        text_property: s.property_ref(p.action_text_property),
                        default_text: s.string_info(p.action_default_text).cloned(),
                    })
                })
                .collect()
        })
    }

    /// The text and template of NPC action `action` (§5.3 rule 3): the speaker body
    /// record's value of the entry's text property when it has one, else the entry's
    /// default text.
    #[must_use]
    pub fn action_text(
        &self,
        action: u32,
        speaker_body: Option<Did>,
        player: &PlayerVars,
    ) -> (DialogueLine, Option<u32>) {
        let Some(entry) = self.action_entries().iter().find(|e| e.action == action) else {
            return (DialogueLine::missing(), None);
        };
        let own = entry
            .text_property
            .zip(speaker_body)
            .and_then(|(pid, body)| {
                let set = self.data().record(body).ok()??;
                match set.get(pid)? {
                    PropertyValue::StringInfo(i) => Some((**i).clone()),
                    _ => None,
                }
            });
        let info = own.or_else(|| entry.default_text.clone());
        (self.line(info.as_ref(), player), entry.template)
    }

    /// The choice page of a `DialogueShown` with options (§5.3 Tessera rule).
    #[must_use]
    pub fn choice_page(
        &self,
        text: &DialogueText,
        options: &[(u8, DialogueText)],
        speaker_body: Option<Did>,
        player: &PlayerVars,
    ) -> ChoicePage {
        let mut opts: Vec<&(u8, DialogueText)> = options.iter().take(256).collect();
        opts.sort_by_key(|o| o.0);
        ChoicePage {
            greeting: self.dialogue_text(text, speaker_body, player),
            options: opts
                .into_iter()
                .map(|(n, t)| {
                    let (text, template) = match t {
                        DialogueText::Action(a) => self.action_text(*a, speaker_body, player),
                        other => (self.dialogue_text(other, speaker_body, player), None),
                    };
                    ChoiceRow {
                        option: *n,
                        text,
                        template,
                    }
                })
                .collect(),
        }
    }
}
