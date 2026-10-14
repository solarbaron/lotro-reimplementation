// SPDX-License-Identifier: GPL-3.0-or-later
//! The instance finder of `travel-windows.md` §6 (the fellowship maker's launch-now page)
//! with the Tessera rules of §6.8 (entries from the scenario feed's `InstanceListShown`):
//! the entry tree of kind and category headers, template and group rows (§6.3), the
//! featured field, the size and tier menus and the level field (§6.4), the join button,
//! the queue display of the confirm page (§6.5) and its alert (§7.2), the listings board
//! shown unavailable (§6.7), and the instance-travel window (§4.11).
//!
//! The client renders every text (template names, the mapper display texts of kinds,
//! categories, groups, sizes and tiers) into [`FinderView`]; the finder runs no
//! eligibility test (§6.8 rule 5). Its controls return [`FinderAction`]s: the World
//! commands the client sends (`JoinInstance`, `LeaveQueue`) and requests to close or open
//! a window.

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::examine::ExamTemplate;
use super::mapfallback::P5Art;
use super::npc::kit::{
    ClickKind, FNode, Listen, TreeRow, clear_rows, colour_value, colours, commit_tree_rows, cut,
    find, fit_label_height, ghost, make_row, mark_row, put_text, row_of, text_part,
};
use super::npc::{NpcTexts, WindowId};
use super::npcvocab::NpcVocab;
use super::p5vocab::P5Vocab;
use super::popup::{Popup, PopupItem, click};
use super::{show, subtree};

/// Most finder entries listed (Tessera bound; the feed sends at most 1,024).
pub const MAX_ENTRIES: usize = 1024;

/// Most size or tier items of one row (Tessera bound).
pub const MAX_CHOICES: usize = 64;

/// Size of a fallback row (`FellowshipMaker_EncounterDisplayBase`, 314 × 22).
pub const ROW_SIZE: (i32, i32) = (314, 22);

/// The kind of an entry without a kind bit (`travel-windows.md` §6.3 rule 2).
pub const NO_KIND: u32 = 8;

/// The mask of the six kind bits of the fellowship-maker flag word (§6.3 rule 2).
pub const KIND_MASK: u32 = 0x18_000F;

/// The kind *k* of a flag word (§6.3 rule 2): the word & [`KIND_MASK`], 8 when no kind bit
/// is set.
#[must_use]
pub fn kind_of(flags: u32) -> u32 {
    match flags & KIND_MASK {
        0 => NO_KIND,
        k => k,
    }
}

/// A user action in the finder, its queue page or alert, or the instance-travel window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinderAction {
    /// `JoinInstance { window, entry, size, tier, level }` (§6.8 rule 5; §4.11 Tessera
    /// rule: teleport-now sends entry 1).
    Join {
        /// The instance window.
        window: WindowId,
        /// The entry number.
        entry: u16,
        /// The chosen group-size key.
        size: Option<u32>,
        /// The chosen difficulty-tier key.
        tier: Option<u32>,
        /// The chosen level.
        level: Option<u16>,
    },
    /// `LeaveQueue` (§6.8 rule 6).
    LeaveQueue,
    /// A close control: the client sends `CloseWindow` for the feed window.
    Close {
        /// The window.
        window: WindowId,
    },
    /// The queue alert was clicked (§7.2 Tessera rule): the client opens the finder.
    OpenFinder,
}

/// One finder entry, rendered by the client (§6.8 rule 1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FinderEntry {
    /// The feed's entry number.
    pub entry: u16,
    /// The template's DID (0 for a literal ref).
    pub template: u32,
    /// The row text (`PrivateEncounterTemplate_Name`, or the literal).
    pub name: String,
    /// The kind *k* ([`kind_of`]).
    pub kind: u32,
    /// The kind header's text.
    pub kind_label: String,
    /// The category (`WorldJoin_EncounterCategory`; 0 when absent).
    pub category: u32,
    /// The category header's text (the fixed text for category 0).
    pub category_label: String,
    /// The instance group (key and the `WJInstanceGroup` display text), when the template
    /// belongs to one.
    pub group: Option<(u32, String)>,
    /// `Skirmish_Template_GroupSizeArray`: (key, display text) in stored order.
    pub sizes: Vec<(u32, String)>,
    /// `Skirmish_Template_DifficultyTierArray`: (key, display text) in stored order.
    pub tiers: Vec<(u32, String)>,
    /// `Skirmish_Template_MinLevelScale` … `MaxLevelScale` (`None` when absent).
    pub level_range: Option<(u16, u16)>,
    /// The template's `Usage_MinLevel` (the featured field's ghosting, §6.3 rule 6).
    pub min_level: u16,
    /// The feed's level shown (`InstanceEntry.level`).
    pub level: Option<u16>,
    /// The feed's size key shown (`InstanceEntry.size`).
    pub size: Option<u32>,
    /// The feed's tier key shown (`InstanceEntry.tier`).
    pub tier: Option<u32>,
    /// Drawn in its error look (`InstanceEntry.locked`).
    pub locked: bool,
}

/// An `InstanceListShown`, rendered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FinderView {
    /// The feed window.
    pub window: WindowId,
    /// The entries, in entry order.
    pub entries: Vec<FinderEntry>,
    /// The featured entry (`featured`), when the feed names one listed.
    pub featured: Option<u16>,
    /// The local player's level.
    pub player_level: u16,
}

/// One row of the finder tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinderRow {
    /// The row text (the template's name, or the group's text).
    pub text: String,
    /// The entries the row stands for (indexes into the view's entries, list order).
    pub entries: Vec<usize>,
    /// The group key of a group row.
    pub group: Option<u32>,
    /// The category header the row is under (index into [`FinderTree::categories`]).
    pub category: usize,
    /// Drawn in its error look.
    pub locked: bool,
}

/// The finder tree (§6.3; TI6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FinderTree {
    /// Kind headers: (*k*, text), in order of first use.
    pub kinds: Vec<(u32, String)>,
    /// Category headers: (kind header index, category, text), in order of first use.
    pub categories: Vec<(usize, u32, String)>,
    /// Rows, in order of first use.
    pub rows: Vec<FinderRow>,
}

impl FinderTree {
    /// Builds the tree from the entries (§6.3 rules 1–4, §6.8 rules 1–2): one kind header
    /// per *k* and one category header per (*k*, *c*), created at first use; a template of
    /// an instance group is shown through one group row per group (with the kind and
    /// category of the group's first entry); headers and rows in entry order.
    #[must_use]
    pub fn build(entries: &[FinderEntry]) -> Self {
        let mut t = Self::default();
        for (i, e) in entries.iter().enumerate().take(MAX_ENTRIES) {
            if let Some((g, _)) = &e.group
                && let Some(r) = t.rows.iter_mut().find(|r| r.group == Some(*g))
            {
                r.entries.push(i);
                r.locked |= e.locked;
                continue;
            }
            let kind = match t.kinds.iter().position(|(k, _)| *k == e.kind) {
                Some(k) => k,
                None => {
                    t.kinds.push((e.kind, e.kind_label.clone()));
                    t.kinds.len() - 1
                }
            };
            let cat = match t
                .categories
                .iter()
                .position(|(k, c, _)| *k == kind && *c == e.category)
            {
                Some(c) => c,
                None => {
                    t.categories
                        .push((kind, e.category, e.category_label.clone()));
                    t.categories.len() - 1
                }
            };
            t.rows.push(FinderRow {
                text: e
                    .group
                    .as_ref()
                    .map_or_else(|| e.name.clone(), |(_, l)| l.clone()),
                entries: vec![i],
                group: e.group.as_ref().map(|(g, _)| *g),
                category: cat,
                locked: e.locked,
            });
        }
        t
    }
}

/// The finder's model: tree, selection, choices and queue state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FinderModel {
    /// The entries.
    pub entries: Vec<FinderEntry>,
    /// The tree.
    pub tree: FinderTree,
    /// The selected row.
    pub selected: Option<usize>,
    /// The chosen size item of the selected row.
    pub size: usize,
    /// The chosen tier item.
    pub tier: usize,
    /// The level shown.
    pub level: Option<u16>,
    /// The featured entry.
    pub featured: Option<u16>,
    /// The player's level.
    pub player_level: u16,
    /// The client is in an instance (§6.6 rule 1).
    pub in_instance: bool,
    /// The search filter.
    pub filter: String,
}

/// The join button's label (`travel-windows.md` §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FinderLabel {
    /// No template resolved (`UI_FellowshipMaker` 0x07F08470).
    JoinNone,
    /// Joinable (0x028AC480).
    Join,
    /// Not joinable (0x042040B0; while in an instance, §6.6 rule 1).
    JoinUnavailable,
    /// The featured field's label (0x06C30B6C).
    Featured,
    /// The page heading (0x026A8B0C).
    Heading,
    /// The size label (0x049C28EC).
    Size,
    /// The tier label (0x088C340C).
    Tier,
    /// The level label (0x0BF6303C).
    Level,
    /// The window title.
    Title,
    /// The confirm page's leave-queue button.
    LeaveQueue,
    /// The confirm page's confirm button.
    Confirm,
    /// The instance-travel window's teleport-now button (0x02212BCC of 0x25033EC8).
    TeleportNow,
    /// Its teleport-later button (0x0576EF5C).
    TeleportLater,
    /// The listings board while unavailable (§6.7 Tessera rule).
    BoardUnavailable,
}

impl FinderLabel {
    /// Tessera's own short text.
    #[must_use]
    pub fn fallback(self) -> &'static str {
        match self {
            Self::JoinNone => "Select an instance",
            Self::Join => "Join",
            Self::JoinUnavailable => "Unavailable",
            Self::Featured => "Featured",
            Self::Heading => "Instances",
            Self::Size => "Size",
            Self::Tier => "Tier",
            Self::Level => "Level",
            Self::Title => "Instance finder",
            Self::LeaveQueue => "Leave queue",
            Self::Confirm => "Confirm",
            Self::TeleportNow => "Go now",
            Self::TeleportLater => "Later",
            Self::BoardUnavailable => "Not available",
        }
    }
}

/// Texts the finder asks the client for; the defaults are Tessera's own.
pub trait FinderTexts: NpcTexts {
    /// A fixed label; `None` keeps the layout's own text (and gives the fallback widgets
    /// [`FinderLabel::fallback`]).
    fn finder_label(&self, label: FinderLabel) -> Option<String> {
        let _ = label;
        None
    }

    /// The text of a queue state (§7.2: `FellowshipMakerUI_AlertStateStringArray`, keys
    /// 1–7); default Tessera's own.
    fn queue_state(&self, state: u8) -> String {
        match state {
            1 => "Confirm to enter".to_string(),
            2 | 7 => "Waiting in queue".to_string(),
            3 | 6 => "Waiting for others".to_string(),
            4 => "Joining".to_string(),
            5 => "Confirm to queue".to_string(),
            _ => String::new(),
        }
    }

    /// A timer's text for `seconds` (Tessera's own "m:ss").
    fn timer(&self, seconds: u64) -> String {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

/// [`FinderTexts`] with every default.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultFinderTexts;

impl NpcTexts for DefaultFinderTexts {}
impl FinderTexts for DefaultFinderTexts {}

/// The label's text: the client's, else Tessera's own.
#[must_use]
pub fn finder_label(texts: &dyn FinderTexts, l: FinderLabel) -> String {
    texts
        .finder_label(l)
        .unwrap_or_else(|| l.fallback().to_string())
}

impl FinderModel {
    /// The model of a view; the selection is cleared.
    pub fn show(&mut self, view: &FinderView) {
        self.entries = view.entries.iter().take(MAX_ENTRIES).cloned().collect();
        self.tree = FinderTree::build(&self.entries);
        self.selected = None;
        self.size = 0;
        self.tier = 0;
        self.level = None;
        self.featured = view
            .featured
            .filter(|f| self.entries.iter().any(|e| e.entry == *f));
        self.player_level = view.player_level;
    }

    /// The size items of row `r` (§6.8 rule 4: the template's sizes, for a group row the
    /// union over the group's templates in list order).
    #[must_use]
    pub fn sizes(&self, r: usize) -> Vec<(u32, String)> {
        let mut out: Vec<(u32, String)> = Vec::new();
        for i in self.tree.rows.get(r).map_or(&[][..], |x| &x.entries[..]) {
            for s in self.entries.get(*i).map_or(&[][..], |e| &e.sizes[..]) {
                if !out.iter().any(|(k, _)| *k == s.0) && out.len() < MAX_CHOICES {
                    out.push(s.clone());
                }
            }
        }
        out
    }

    /// The entry the selected row and size resolve to (§6.4, §6.8 rule 2): for a group row
    /// the first of the group's listed templates whose sizes hold the chosen size, else the
    /// group's first listed entry.
    #[must_use]
    pub fn resolved(&self) -> Option<usize> {
        let row = self.tree.rows.get(self.selected?)?;
        if row.group.is_none() {
            return row.entries.first().copied();
        }
        let size = self.sizes(self.selected?).get(self.size).map(|(k, _)| *k);
        size.and_then(|s| {
            row.entries.iter().copied().find(|i| {
                self.entries
                    .get(*i)
                    .is_some_and(|e| e.sizes.iter().any(|(k, _)| *k == s))
            })
        })
        .or_else(|| row.entries.first().copied())
    }

    /// The tier items of the resolved template.
    #[must_use]
    pub fn tiers(&self) -> Vec<(u32, String)> {
        self.resolved()
            .and_then(|i| self.entries.get(i))
            .map(|e| e.tiers.iter().take(MAX_CHOICES).cloned().collect())
            .unwrap_or_default()
    }

    /// The level field (§6.8 rule 4): the feed's level when given, else the player's level
    /// clamped into the template's scaling range, else the player's level; editable only
    /// with a scaling range (min < max). `None` without a resolved template.
    #[must_use]
    pub fn level_field(&self) -> Option<(u16, bool)> {
        let e = self.entries.get(self.resolved()?)?;
        let editable = e.level_range.is_some_and(|(a, b)| a < b);
        let level = e.level.unwrap_or_else(|| match e.level_range {
            Some((a, b)) if a <= b => self.player_level.clamp(a, b),
            _ => self.player_level,
        });
        Some((level, editable))
    }

    /// Selects row `r` (§6.3 last paragraph): the menus are refilled with their first item
    /// chosen, or the feed's shown key when listed, and the level field is filled.
    pub fn select(&mut self, r: Option<usize>) {
        self.selected = r.filter(|r| *r < self.tree.rows.len());
        self.size = 0;
        self.tier = 0;
        if let Some(e) = self.resolved().and_then(|i| self.entries.get(i)).cloned() {
            if let Some(s) = e.size
                && let Some(p) = self
                    .sizes(self.selected.unwrap_or(0))
                    .iter()
                    .position(|(k, _)| *k == s)
            {
                self.size = p;
            }
            if let Some(t) = e.tier
                && let Some(p) = self.tiers().iter().position(|(k, _)| *k == t)
            {
                self.tier = p;
            }
        }
        self.level = self.level_field().map(|(l, _)| l);
    }

    /// The join button (§6.4, §6.8 rule 5, §6.6 rule 1): (ghosted, label).
    #[must_use]
    pub fn join_button(&self) -> (bool, FinderLabel) {
        if self.resolved().is_none() {
            (true, FinderLabel::JoinNone)
        } else if self.in_instance {
            (true, FinderLabel::JoinUnavailable)
        } else {
            (false, FinderLabel::Join)
        }
    }

    /// The join command (§6.8 rule 5).
    #[must_use]
    pub fn join(&self, window: WindowId) -> Option<FinderAction> {
        if self.join_button().0 {
            return None;
        }
        let e = self.entries.get(self.resolved()?)?;
        let size = self.sizes(self.selected?).get(self.size).map(|(k, _)| *k);
        let tier = self.tiers().get(self.tier).map(|(k, _)| *k);
        Some(FinderAction::Join {
            window,
            entry: e.entry,
            size,
            tier,
            level: self.level,
        })
    }

    /// The row of the featured entry.
    #[must_use]
    pub fn featured_row(&self) -> Option<usize> {
        let f = self.featured?;
        let i = self.entries.iter().position(|e| e.entry == f)?;
        self.tree.rows.iter().position(|r| r.entries.contains(&i))
    }

    /// Whether the featured field is ghosted (§6.3 rule 6, §6.8 rule 3): no featured
    /// template, or its minimum level above the player's.
    #[must_use]
    pub fn featured_ghosted(&self) -> bool {
        let Some(f) = self.featured else { return true };
        self.entries
            .iter()
            .find(|e| e.entry == f)
            .is_none_or(|e| e.min_level > self.player_level)
    }

    /// The search text of row `r` (§6.3 rule 7: "as §4.6 over the row and header
    /// texts"; as the travel row's text holds its territory header's text): the row text,
    /// then its category header's text, then its kind header's text.
    #[must_use]
    pub fn search_text(&self, r: usize) -> String {
        let Some(row) = self.tree.rows.get(r) else {
            return String::new();
        };
        let cat = self.tree.categories.get(row.category);
        let kind = cat.and_then(|(k, _, _)| self.tree.kinds.get(*k));
        format!(
            "{}{}{}",
            row.text,
            cat.map_or("", |(_, _, t)| t.as_str()),
            kind.map_or("", |(_, t)| t.as_str())
        )
    }

    /// Whether row `r` passes the search filter (§6.3 rule 7, §4.6 and its Tessera rule):
    /// its [search text](Self::search_text) contains the filter, case folded.
    #[must_use]
    pub fn matches(&self, r: usize) -> bool {
        self.filter.is_empty()
            || (r < self.tree.rows.len()
                && super::travel::fold(&self.search_text(r))
                    .contains(&super::travel::fold(&self.filter)))
    }
}

/// The finder's templates in the install's `fellowshipmaker` layout (§6.2): the row and
/// the two header templates. `None` uses fallback rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FinderTemplates {
    /// `FellowshipMaker_EncounterDisplayBase`.
    pub row: Option<ExamTemplate>,
    /// `FellowshipMaker_EncounterDisplayHeader` (kinds).
    pub header: Option<ExamTemplate>,
    /// `FellowshipMaker_EncounterDisplayHeader_Tier2` (categories).
    pub header2: Option<ExamTemplate>,
    /// `FellowshipMakerConfirmPage` (§6.5): the confirm page's content, created inside the
    /// panel's `FellowshipMaker_ConfirmPage` when the layout leaves that page empty.
    pub confirm: Option<ExamTemplate>,
}

/// The parts of the finder (§6.2, §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FinderParts {
    /// `FellowshipMaker_WindowLabel`.
    pub title: Option<ElementKey>,
    /// `FellowshipMaker_CloseButton`.
    pub close: Option<ElementKey>,
    /// `FellowshipMaker_Panel`.
    pub panel: Option<ElementKey>,
    /// `FellowshipMaker_LaunchNowPage`.
    pub launch_page: Option<ElementKey>,
    /// `FellowshipMaker_ConfirmPage`.
    pub confirm_page: Option<ElementKey>,
    /// `FellowshipMakerLaunchNow_EncounterTree`.
    pub tree: Option<ElementKey>,
    /// The tree's search field.
    pub search: Option<ElementKey>,
    /// `FellowshipMakerLaunchNow_SizeMenu`.
    pub size_menu: Option<ElementKey>,
    /// `FellowshipMakerLaunchNow_TierMenu`.
    pub tier_menu: Option<ElementKey>,
    /// `FellowshipMakerLaunchNow_LevelField`.
    pub level: Option<ElementKey>,
    /// `FellowshipMakerLaunchNow_JoinButton`.
    pub join: Option<ElementKey>,
    /// `FellowshipMakerLaunchNow_JoinButtonLabel`.
    pub join_label: Option<ElementKey>,
    /// `FellowshipMakerLaunchNow_FeaturedInstance_EncounterDisplay`.
    pub featured: Option<ElementKey>,
    /// `FellowshipMakerLaunchNow_FeaturedInstance_ResetButton`.
    pub featured_reset: Option<ElementKey>,
    /// `FellowshipMakerLaunchNow_LFInstanceFinder`.
    pub board: Option<ElementKey>,
    /// `FellowshipMakerConfirmPage_WaitingField`.
    pub waiting: Option<ElementKey>,
    /// `FellowshipMakerConfirmPage_WaitingField_Timer_Timer`.
    pub waiting_timer: Option<ElementKey>,
    /// `FellowshipMakerConfirmPage_ConfirmTimer_Field`.
    pub confirm_timer: Option<ElementKey>,
    /// `FellowshipMakerConfirmPage_LeaveQueueButton`.
    pub leave_queue: Option<ElementKey>,
}

/// Which menu popup is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuKind {
    Size,
    Tier,
}

/// The queue display (§6.5, §6.8 rule 6): the state and its timer base.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct QueueState {
    /// The displayed player state (0 none, 1–7 the `FellowshipMakerUI_PlayerState` keys).
    pub state: u8,
    /// The scripted seconds (time in queue, or time left to confirm).
    pub seconds: Option<u32>,
    /// The client time (seconds) when the state arrived.
    pub since: f64,
}

/// The instance finder window.
#[derive(Debug, Clone)]
pub struct FinderWindow {
    /// The feed window.
    pub window: WindowId,
    /// The root (`FellowshipMaker_Window`).
    pub root: ElementKey,
    /// The parts.
    pub parts: FinderParts,
    /// The model.
    pub model: FinderModel,
    /// The queue display.
    pub queue: QueueState,
    /// The row elements with their row indexes.
    pub rows: Vec<(ElementKey, usize)>,
    /// The header elements.
    pub headers: Vec<ElementKey>,
    popup: Option<(MenuKind, Popup)>,
    templates: FinderTemplates,
    art: P5Art,
    nv: NpcVocab,
    pv: P5Vocab,
    listen: Listen,
    /// The join label's box as the layout placed it (relative to the join button), when the
    /// label is an element of its own (§6.4 OPEN, Tessera rule).
    join_label_box: Option<crate::geom::IBox>,
    /// The join label was fitted while its text could not be measured (fonts loading).
    join_label_pending: bool,
}

impl FinderWindow {
    /// Finds the finder's parts below `root` (§6.2), shows the launch-now page, the
    /// listings board unavailable with every button ghosted (§6.7 Tessera rule) and the
    /// reset button hidden.
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        pv: &P5Vocab,
        root: ElementKey,
        templates: FinderTemplates,
        art: &P5Art,
        texts: &dyn FinderTexts,
    ) -> Self {
        let ids = pv.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        // §6.2: `FellowshipMaker_Instance_ListBox` names the featured field and the tree
        // field as its entry templates; when the layout left them uncreated, the page's
        // list gets one entry of each (Tessera rule).
        if let Some(list) = f(ui, ids.finder_list)
            && ui.class(list).is_some_and(WidgetClass::is_list)
            && f(ui, ids.encounter_tree).is_none()
        {
            for i in 0..2 {
                if let Err(e) = ui.list_add_template(list, i) {
                    tracing::debug!("finder: list template {i} not created: {e}");
                }
            }
        }
        // §6.5: the panel's `FellowshipMaker_ConfirmPage` can be an empty field; the
        // confirm page's content (`FellowshipMakerConfirmPage`, with the waiting field, the
        // timers and the two buttons) is then created inside it (Tessera rule, as the
        // launch-now page's content is its base element).
        if let (Some(page), Some(t)) = (f(ui, ids.fm_confirm_page), templates.confirm)
            && f(ui, ids.confirm_waiting).is_none()
            && f(ui, ids.leave_queue_button).is_none()
            && let Err(e) = ui.create_element(Some(page), tessera_formats::Did(t.layout), t.element)
        {
            tracing::debug!("finder: confirm page content not created: {e}");
        }
        let parts = FinderParts {
            title: f(ui, ids.fm_title),
            close: f(ui, ids.fm_close),
            panel: f(ui, ids.fm_panel),
            launch_page: f(ui, ids.fm_launch_page),
            confirm_page: f(ui, ids.fm_confirm_page),
            tree: f(ui, ids.encounter_tree),
            search: f(ui, ids.finder_search),
            size_menu: f(ui, ids.finder_size_menu),
            tier_menu: f(ui, ids.finder_tier_menu),
            level: f(ui, ids.finder_level_field),
            join: f(ui, ids.finder_join),
            join_label: f(ui, ids.finder_join_label),
            featured: f(ui, ids.featured_display),
            featured_reset: f(ui, ids.featured_reset),
            board: f(ui, ids.finder_board),
            waiting: f(ui, ids.confirm_waiting),
            waiting_timer: f(ui, ids.confirm_waiting_timer),
            confirm_timer: f(ui, ids.confirm_timer),
            leave_queue: f(ui, ids.leave_queue_button),
        };
        if let Some(b) = parts.board {
            show(ui, find(ui, b, ids.board_overlay), true);
            for k in subtree(ui, b) {
                if ui.class(k).is_some_and(WidgetClass::is_button) {
                    ghost(ui, Some(k), true);
                }
            }
        }
        show(ui, parts.featured_reset, false);
        if let Some(t) = parts.title
            && ui.text_of(t).is_empty()
        {
            put_text(ui, Some(t), &finder_label(texts, FinderLabel::Title));
        }
        let mut w = Self {
            window: 0,
            root,
            parts,
            model: FinderModel::default(),
            queue: QueueState::default(),
            rows: Vec::new(),
            headers: Vec::new(),
            popup: None,
            templates,
            art: art.clone(),
            nv: nv.clone(),
            pv: pv.clone(),
            listen: Listen::default(),
            join_label_box: parts
                .join_label
                .filter(|l| parts.join.is_some_and(|j| ui.parent(*l) == Some(j)))
                .and_then(|l| ui.layout_box(l)),
            join_label_pending: false,
        };
        for k in [
            parts.close,
            parts.join,
            parts.size_menu,
            parts.tier_menu,
            parts.featured,
            parts.leave_queue,
        ] {
            w.listen.clicks(ui, k);
        }
        let ms = ui.vocab().msgs;
        for m in [ms.character_input, ms.key_press, ms.key_release] {
            w.listen.on(ui, parts.search, m);
        }
        w.open_page(ui, false);
        w
    }

    /// Opens the launch-now page (`false`) or the confirm page (`true`) of the panel.
    fn open_page(&self, ui: &mut Ui, confirm: bool) {
        let ids = self.pv.ids;
        let page = if confirm {
            ids.fm_confirm_page
        } else {
            ids.fm_launch_page
        };
        let tab = self.parts.panel.and_then(|p| {
            ui.el(p)?
                .widget
                .panel
                .as_ref()?
                .tab_page
                .iter()
                .find(|(_, pg)| *pg == page)
                .map(|(t, _)| *t)
        });
        if let (Some(p), Some(t)) = (self.parts.panel, tab) {
            ui.panel_open(p, page, t);
        }
        // The two pages are shown and hidden explicitly as well: a page the panel lists
        // no tab for is not switched by `panel_open`, and the panel's remembered page
        // would then keep the other one shown.
        show(ui, self.parts.launch_page, !confirm);
        show(ui, self.parts.confirm_page, confirm);
    }

    /// Shows an `InstanceListShown` (§6.8): the tree, the featured field, no selection.
    pub fn show(&mut self, ui: &mut Ui, view: &FinderView, texts: &dyn FinderTexts) {
        self.window = view.window;
        self.close_popup(ui);
        self.model.show(view);
        self.fill(ui);
        self.refresh(ui, texts);
    }

    /// Sets whether the client is in an instance (§6.6 rule 1: `InstanceChanged`).
    pub fn set_in_instance(&mut self, ui: &mut Ui, on: bool, texts: &dyn FinderTexts) {
        self.model.in_instance = on;
        self.refresh(ui, texts);
    }

    /// Rebuilds the tree (§6.3): kind headers, category headers, rows.
    fn fill(&mut self, ui: &mut Ui) {
        let Some(tree) = self.parts.tree else { return };
        let mut old: Vec<ElementKey> = self.rows.drain(..).map(|(k, _)| k).collect();
        old.append(&mut self.headers);
        clear_rows(ui, tree, &mut old);
        let ids = self.pv.ids;
        let cp = self.nv.pids.text_font_color;
        let art = self.art.clone();
        let st = ui.vocab().states;
        let mut commit = Vec::new();
        let mut kind_rows: Vec<Option<ElementKey>> = vec![None; self.model.tree.kinds.len()];
        let mut cat_rows: Vec<Option<ElementKey>> = vec![None; self.model.tree.categories.len()];
        let model = self.model.clone();
        for (r, row) in model.tree.rows.iter().enumerate() {
            if !model.matches(r) {
                continue;
            }
            let Some((kind, _, ctext)) = model.tree.categories.get(row.category) else {
                continue;
            };
            let Some(text) = model.tree.kinds.get(*kind).map(|(_, t)| t) else {
                continue;
            };
            if kind_rows[*kind].is_none() {
                let h = make_row(ui, tree, self.templates.header, || {
                    FNode::text(
                        ids.encounter_header,
                        0,
                        0,
                        ROW_SIZE.0,
                        24,
                        cp,
                        colours::TITLE,
                        "",
                    )
                });
                let Some(h) = h else { break };
                put_text(
                    ui,
                    text_part(ui, h, Some(ids.encounter_text)).or(Some(h)),
                    text,
                );
                self.headers.push(h);
                commit.push(TreeRow {
                    row: h,
                    parent: None,
                    expanded: true,
                });
                kind_rows[*kind] = Some(h);
            }
            if cat_rows[row.category].is_none() {
                let h = make_row(ui, tree, self.templates.header2, || {
                    FNode::text(
                        ids.encounter_header2,
                        12,
                        0,
                        ROW_SIZE.0 - 12,
                        22,
                        cp,
                        colours::YELLOW,
                        "",
                    )
                });
                let Some(h) = h else { break };
                put_text(
                    ui,
                    text_part(ui, h, Some(ids.encounter_text)).or(Some(h)),
                    ctext,
                );
                self.headers.push(h);
                commit.push(TreeRow {
                    row: h,
                    parent: kind_rows[*kind],
                    expanded: true,
                });
                cat_rows[row.category] = Some(h);
            }
            let k = make_row(ui, tree, self.templates.row, || {
                fallback_row(&ids, cp, &art, st.normal, st.highlight, st.ghosted)
            });
            let Some(k) = k else { break };
            put_text(ui, text_part(ui, k, Some(ids.encounter_text)), &row.text);
            show(ui, find(ui, k, ids.encounter_check), false);
            if row.locked {
                // Tessera rule (§6.3 rule 5: the error look is a state change, not
                // traced): the row's `ghosted` state when it defines one.
                // The fallback row (no install template) also draws its text red
                // (implementer OPEN (b) of §6.8).
                ui.switch_state_if_defined(k, st.ghosted);
                if self.templates.row.is_none()
                    && let Some(t) = text_part(ui, k, Some(ids.encounter_text))
                {
                    ui.set_property(t, cp, colour_value(colours::RED));
                }
            }
            super::popup::listen_row(ui, &mut self.listen, k);
            self.rows.push((k, r));
            commit.push(TreeRow {
                row: k,
                parent: cat_rows[row.category],
                expanded: true,
            });
        }
        let kept = commit_tree_rows(ui, tree, &commit);
        self.rows.retain(|(k, _)| kept.contains(k));
        self.headers.retain(|k| kept.contains(k));
    }

    /// Fills the menus' displays, the level field, the join button and the featured field.
    fn refresh(&mut self, ui: &mut Ui, texts: &dyn FinderTexts) {
        let st = ui.vocab().states;
        let p = self.parts;
        for (k, r) in &self.rows {
            if self.model.tree.rows.get(*r).is_some_and(|x| !x.locked) {
                mark_row(ui, *k, self.model.selected == Some(*r));
            }
        }
        let sizes = self
            .model
            .selected
            .map(|r| self.model.sizes(r))
            .unwrap_or_default();
        let tiers = self.model.tiers();
        let menu_text = |items: &[(u32, String)], i: usize| {
            items.get(i).map(|(_, t)| t.clone()).unwrap_or_default()
        };
        for (menu, text) in [
            (p.size_menu, menu_text(&sizes, self.model.size)),
            (p.tier_menu, menu_text(&tiers, self.model.tier)),
        ] {
            if let Some(m) = menu {
                let el = text_part(ui, m, None).unwrap_or(m);
                put_text(ui, Some(el), &text);
            }
        }
        match self.model.level_field() {
            Some((l, editable)) => {
                put_text(ui, p.level, &l.to_string());
                if let Some(k) = p.level {
                    let ep = ui.vocab().pids.text_editable;
                    ui.set_property(
                        k,
                        ep,
                        tessera_formats::property::PropertyValue::Bool(editable),
                    );
                    ui.switch_state_if_defined(k, if editable { st.normal } else { st.ghosted });
                }
            }
            None => put_text(ui, p.level, ""),
        }
        let (ghosted, label) = self.model.join_button();
        ghost(ui, p.join, ghosted);
        let el = p
            .join_label
            .or_else(|| p.join.and_then(|j| text_part(ui, j, None)));
        put_text(ui, el, &finder_label(texts, label));
        self.fit_join_label(ui);
        if let Some(f) = p.featured {
            let text = self
                .model
                .featured_row()
                .and_then(|r| self.model.tree.rows.get(r))
                .map(|x| x.text.clone())
                .unwrap_or_default();
            let el = text_part(ui, f, Some(self.pv.ids.encounter_text)).unwrap_or(f);
            put_text(ui, Some(el), &text);
            let selected =
                self.model.selected.is_some() && self.model.selected == self.model.featured_row();
            let s = if self.model.featured_ghosted() {
                st.ghosted
            } else if selected {
                st.highlight
            } else {
                st.normal
            };
            ui.switch_state_if_defined(f, s);
        }
    }

    /// Selects row `r` (or none).
    pub fn select(&mut self, ui: &mut Ui, r: Option<usize>, texts: &dyn FinderTexts) {
        self.close_popup(ui);
        self.model.select(r);
        self.refresh(ui, texts);
    }

    /// Sets the search filter and rebuilds the tree.
    pub fn set_filter(&mut self, ui: &mut Ui, filter: &str, texts: &dyn FinderTexts) {
        if self.model.filter == filter {
            return;
        }
        self.model.filter = cut(filter);
        self.fill(ui);
        self.refresh(ui, texts);
    }

    /// Chooses size item `i` (the template of a group row is resolved again, §6.4).
    pub fn choose_size(&mut self, ui: &mut Ui, i: usize, texts: &dyn FinderTexts) {
        self.model.size = i;
        self.model.tier = 0;
        self.model.level = self.model.level_field().map(|(l, _)| l);
        self.refresh(ui, texts);
    }

    /// Chooses tier item `i`.
    pub fn choose_tier(&mut self, ui: &mut Ui, i: usize, texts: &dyn FinderTexts) {
        self.model.tier = i;
        self.refresh(ui, texts);
    }

    /// Sets the level typed into an editable level field (clamped into the template's
    /// scaling range; ignored when the field is not editable).
    pub fn set_level(&mut self, ui: &mut Ui, level: u16, texts: &dyn FinderTexts) {
        let Some(i) = self.model.resolved() else {
            return;
        };
        if let Some((a, b)) = self
            .model
            .entries
            .get(i)
            .and_then(|e| e.level_range)
            .filter(|(a, b)| a < b)
        {
            self.model.level = Some(level.clamp(a, b));
            self.refresh(ui, texts);
        }
    }

    fn close_popup(&mut self, ui: &mut Ui) {
        if let Some((_, p)) = self.popup.take() {
            p.close(ui);
        }
    }

    fn toggle_menu(&mut self, ui: &mut Ui, kind: MenuKind) {
        let open = self.popup.as_ref().map(|(k, _)| *k);
        self.close_popup(ui);
        if open == Some(kind) {
            return;
        }
        let (anchor, items) = match kind {
            MenuKind::Size => (
                self.parts.size_menu,
                self.model
                    .selected
                    .map(|r| self.model.sizes(r))
                    .unwrap_or_default(),
            ),
            MenuKind::Tier => (self.parts.tier_menu, self.model.tiers()),
        };
        let (Some(anchor), false) = (anchor, items.is_empty()) else {
            return;
        };
        let items = items
            .into_iter()
            .map(|(_, t)| PopupItem::command(&t))
            .collect();
        match Popup::open(ui, &self.nv, &self.art.base, None, anchor, 120, items) {
            Ok(p) => self.popup = Some((kind, p)),
            Err(e) => tracing::debug!("finder: menu not opened: {e}"),
        }
    }

    /// Sets the displayed queue state (§6.5, §6.8 rule 6) at client time `now`: states 2
    /// and 7 show the waiting field with its count-up timer (hidden at 0 s), state 1 the
    /// confirm timer counting down, others hide the waiting field. While the state is not 0
    /// the confirm page is open, else the launch-now page (Tessera reading of §6.5's "drives
    /// the confirm page").
    pub fn set_queue(
        &mut self,
        ui: &mut Ui,
        state: u8,
        seconds: Option<u32>,
        now: f64,
        texts: &dyn FinderTexts,
    ) {
        self.queue = QueueState {
            state: state.min(7),
            seconds,
            since: now,
        };
        self.open_page(ui, self.queue.state != 0);
        self.tick(ui, now, texts);
    }

    /// Whether a text fit waits for fonts that are still loading (the join label, §6.4
    /// OPEN); [`Self::tick`] makes it once they have arrived.
    #[must_use]
    pub fn fit_pending(&self) -> bool {
        self.join_label_pending
    }

    /// Fits the join label to its text (§6.4 OPEN, Tessera rule: the label grows to its
    /// wrapped text, at most to the button's height); fitted again by [`Self::tick`] when
    /// its fonts were still loading.
    fn fit_join_label(&mut self, ui: &mut Ui) {
        if let (Some(k), Some(b)) = (self.parts.join_label, self.join_label_box) {
            self.join_label_pending = !fit_label_height(ui, k, b);
        }
    }

    /// Updates the queue timers at client time `now`, and fits the join label again once
    /// fonts that were loading when its text was set have arrived.
    pub fn tick(&mut self, ui: &mut Ui, now: f64, texts: &dyn FinderTexts) {
        if self.join_label_pending {
            self.fit_join_label(ui);
        }
        let q = self.queue;
        let p = self.parts;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let elapsed = (now - q.since).max(0.0) as u64;
        let base = u64::from(q.seconds.unwrap_or(0));
        let waiting = matches!(q.state, 2 | 7);
        show(ui, p.waiting, waiting);
        if waiting {
            let up = base.saturating_add(elapsed);
            show(ui, p.waiting_timer, base > 0);
            put_text(ui, p.waiting_timer, &texts.timer(up));
        }
        if let Some(c) = p.confirm_timer {
            if q.state == 1 {
                ui.set_shown(c, true);
                ui.switch_state_if_defined(c, ui.vocab().states.normal);
                let el = text_part(ui, c, None).unwrap_or(c);
                put_text(ui, Some(el), &texts.timer(base.saturating_sub(elapsed)));
            } else {
                ui.set_shown(c, false);
            }
        }
    }

    /// The command for a UI event: row selection and double-click join, the join button,
    /// the featured field (selects the featured row and clears the search), the menus, the
    /// leave-queue and close buttons, typing in the search field.
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        texts: &dyn FinderTexts,
    ) -> Option<FinderAction> {
        if let Some(i) = self
            .popup
            .as_mut()
            .and_then(|(_, p)| p.handle_event(ui, ev))
        {
            let kind = self.popup.as_ref().map(|(k, _)| *k);
            self.close_popup(ui);
            match kind {
                Some(MenuKind::Size) => self.choose_size(ui, i, texts),
                Some(MenuKind::Tier) => self.choose_tier(ui, i, texts),
                None => {}
            }
            return None;
        }
        if let UiEvent::Game { message } = ev
            && self.parts.search.is_some_and(|s| s == message.source)
        {
            let ms = ui.vocab().msgs;
            if [ms.character_input, ms.key_press, ms.key_release].contains(&message.msg) {
                let t = self.parts.search.map(|s| ui.text_of(s)).unwrap_or_default();
                self.set_filter(ui, &t, texts);
                return None;
            }
        }
        let (src, kind) = click(ui, ev)?;
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.root, src) {
            return None;
        }
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        if matches!(kind, ClickKind::Button | ClickKind::Left) {
            if hit(self.parts.close) {
                self.close_popup(ui);
                return Some(FinderAction::Close {
                    window: self.window,
                });
            }
            if hit(self.parts.join) {
                return self.model.join(self.window);
            }
            if hit(self.parts.leave_queue) {
                return Some(FinderAction::LeaveQueue);
            }
            if hit(self.parts.size_menu) {
                self.toggle_menu(ui, MenuKind::Size);
                return None;
            }
            if hit(self.parts.tier_menu) {
                self.toggle_menu(ui, MenuKind::Tier);
                return None;
            }
        }
        if hit(self.parts.featured) {
            if self.model.featured_ghosted() {
                return None;
            }
            if !self.model.filter.is_empty() {
                if let Some(s) = self.parts.search {
                    ui.set_text(s, Some(String::new()));
                }
                self.set_filter(ui, "", texts);
            }
            let r = self.model.featured_row();
            self.select(ui, r, texts);
            return (kind == ClickKind::Double)
                .then(|| self.model.join(self.window))
                .flatten();
        }
        let ks: Vec<ElementKey> = self.rows.iter().map(|(k, _)| *k).collect();
        let r = row_of(ui, &ks, src)?;
        let row = self.rows.get(r)?.1;
        match kind {
            ClickKind::Left | ClickKind::Button => {
                self.select(ui, Some(row), texts);
                None
            }
            ClickKind::Double => {
                self.select(ui, Some(row), texts);
                self.model.join(self.window)
            }
            ClickKind::Right => None,
        }
    }

    /// Whether a menu popup is open.
    #[must_use]
    pub fn popup_root(&self) -> Option<ElementKey> {
        self.popup.as_ref().map(|(_, p)| p.root)
    }

    /// Removes the game-code handlers and an open popup.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.close_popup(ui);
        self.listen.release(ui);
    }
}

/// A fallback `FellowshipMaker_EncounterDisplayBase` (§6.2: 314 × 22 with the text and the
/// check mark).
pub(crate) fn fallback_row(
    ids: &super::p5vocab::P5Ids,
    cp: u32,
    art: &P5Art,
    normal: u32,
    highlight: u32,
    ghosted: u32,
) -> FNode {
    FNode::field(ids.encounter_row, 0, 0, ROW_SIZE.0, ROW_SIZE.1)
        .image(&art.base.row)
        .state(normal, art.base.row.as_ref(), vec![])
        .state(highlight, art.base.row_highlight.as_ref(), vec![])
        .state(ghosted, art.base.row.as_ref(), vec![])
        .children([
            FNode::text(
                ids.encounter_text,
                24,
                3,
                ROW_SIZE.0 - 50,
                16,
                cp,
                colours::WHITE,
                "",
            ),
            FNode::field(ids.encounter_check, ROW_SIZE.0 - 22, 3, 16, 16)
                .image(&art.check)
                .hidden(),
        ])
}

/// The queue alert (`FellowshipMakerWaitingInQueueAlertField`, §7.2 Tessera rule): shown
/// while the displayed queue state is not 0, with the state's text as tooltip; a click
/// opens the finder.
#[derive(Debug, Clone)]
pub struct QueueAlert {
    /// The alert element.
    pub root: ElementKey,
    listen: Listen,
}

impl QueueAlert {
    /// Attaches to the alert at or below `k` (the client opens the `alert` layout's field,
    /// or Tessera's fallback, [`super::travelfallback::queue_alert`]); hidden.
    #[must_use]
    pub fn attach(ui: &mut Ui, pv: &P5Vocab, k: ElementKey) -> Self {
        let root = find(ui, k, pv.ids.queue_alert).unwrap_or(k);
        ui.set_shown(root, false);
        let mut a = Self {
            root,
            listen: Listen::default(),
        };
        a.listen.clicks(ui, Some(root));
        a
    }

    /// Shows the alert for `state` (hidden for 0).
    pub fn set_state(&self, ui: &mut Ui, state: u8, texts: &dyn FinderTexts) {
        ui.set_shown(self.root, state != 0);
        if state != 0 {
            ui.set_tooltip_text(self.root, Some(cut(&texts.queue_state(state))));
            let p = ui.vocab().pids.tooltips_on;
            ui.set_property(
                self.root,
                p,
                tessera_formats::property::PropertyValue::Bool(true),
            );
            // The click registration made it mouse-visible; the tooltip writes above
            // recompute the flag from the type rule before `tooltips_on` is set, which
            // would leave it off, so a real click would not reach the alert.
            ui.set_mouse_visible_flag(self.root, true);
        }
    }

    /// `OpenFinder` for a click on the alert.
    pub fn handle_event(&self, ui: &Ui, ev: &UiEvent) -> Option<FinderAction> {
        let (src, kind) = click(ui, ev)?;
        (matches!(
            kind,
            ClickKind::Button | ClickKind::Left | ClickKind::Double
        ) && ui.is_ancestor_or_self(self.root, src))
        .then_some(FinderAction::OpenFinder)
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}

/// The instance-travel window (`travel-windows.md` §4.11 with its Tessera rule): title =
/// the template's name, the intro text, teleport-now sends `JoinInstance { entry: 1 }`,
/// teleport-later and close only close.
#[derive(Debug, Clone)]
pub struct InstanceTravelWindow {
    /// The feed window.
    pub window: WindowId,
    /// The root (`RootPETravelField`).
    pub root: ElementKey,
    now: Option<ElementKey>,
    later: Option<ElementKey>,
    close: Option<ElementKey>,
    title: Option<ElementKey>,
    intro: Option<ElementKey>,
    listen: Listen,
}

impl InstanceTravelWindow {
    /// Finds the parts below `root` and fills the buttons' labels when the layout gives
    /// none.
    pub fn attach(ui: &mut Ui, pv: &P5Vocab, root: ElementKey, texts: &dyn FinderTexts) -> Self {
        let ids = pv.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        let mut w = Self {
            window: 0,
            root,
            now: f(ui, ids.pe_now),
            later: f(ui, ids.pe_later),
            close: f(ui, ids.pe_close),
            title: f(ui, ids.pe_title),
            intro: f(ui, ids.pe_intro),
            listen: Listen::default(),
        };
        for (k, l) in [
            (w.now, FinderLabel::TeleportNow),
            (w.later, FinderLabel::TeleportLater),
        ] {
            if let Some(b) = k
                && let Some(t) = text_part(ui, b, None)
                && ui.text_of(t).is_empty()
            {
                put_text(ui, Some(t), &finder_label(texts, l));
            }
        }
        for k in [w.now, w.later, w.close] {
            w.listen.clicks(ui, k);
        }
        w
    }

    /// Shows the window for one template: its name as the title and the intro text (the
    /// template's description, §4.11 Tessera rule).
    pub fn show(&mut self, ui: &mut Ui, window: WindowId, name: &str, intro: &str) {
        self.window = window;
        put_text(ui, self.title, name);
        put_text(ui, self.intro, intro);
    }

    /// The command for a UI event.
    pub fn handle_event(&self, ui: &Ui, ev: &UiEvent) -> Option<FinderAction> {
        let (src, kind) = click(ui, ev)?;
        if !matches!(kind, ClickKind::Button | ClickKind::Left) {
            return None;
        }
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        if hit(self.now) {
            return Some(FinderAction::Join {
                window: self.window,
                entry: 1,
                size: None,
                tier: None,
                level: None,
            });
        }
        (hit(self.later) || hit(self.close)).then_some(FinderAction::Close {
            window: self.window,
        })
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}
