// SPDX-License-Identifier: GPL-3.0-or-later
//! The travel window of `travel-windows.md` §4 with the Tessera rules of §4.10 (rows from
//! the scenario feed's `TravelShown`), the end-travel box of §5.1 and the travel
//! presentation of §5.2.
//!
//! The client renders the rows (names, territory and area texts, coin counts) from the
//! feed and the install into [`TravelView`]; the window computes no list, test, price or
//! level (TI5). Its controls return [`TravelAction`]s: the World commands the client sends
//! (`ChooseTravel`, `Dismount`) and the close request. The window never closes itself on a
//! go; the client hides it when the feed answers (`WindowClosed` or `TravelStarted`,
//! §4.10 rule 6).
//!
//! The window works on the install's `travel` layout (rows from its `Travel_ListItem` and
//! `Travel_ListItem_Header` templates) or on the fallback window of
//! [`super::travelfallback`]; both carry the same element IDs.

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::examine::ExamTemplate;
use super::mapfallback::P5Art;
use super::npc::coins::{CoinCounts, MoneyDisplay};
use super::npc::kit::{
    ClickKind, FNode, Listen, TreeRow, clear_rows, colours, commit_tree_rows, find, make_row,
    mark_row, put_text, row_of, text_part,
};
use super::npc::{NpcTexts, WindowId};
use super::npcvocab::NpcVocab;
use super::p5vocab::P5Vocab;
use super::popup::click;
use super::{show, switch_if_defined};

/// Most rows a travel window lists (Tessera bound; the feed sends at most 1,024 entries).
pub const MAX_ROWS: usize = 1024;

/// Size of a fallback row (`Travel_ListItem`, 400 × 22).
pub const ROW_SIZE: (i32, i32) = (400, 22);

/// A user action in the travel window or the end-travel box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TravelAction {
    /// `ChooseTravel { window, entry }` (§4.10 rule 6).
    Choose {
        /// The travel window.
        window: WindowId,
        /// The entry number.
        entry: u16,
    },
    /// The close button (§4.9): the client sends `CloseWindow` for the feed window.
    Close {
        /// The travel window.
        window: WindowId,
    },
    /// The end-travel button (§5.2 rule 4): `Dismount`.
    Dismount,
}

/// One travel row, rendered by the client (§4.10 rule 3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TravelRow {
    /// The feed's entry number.
    pub entry: u16,
    /// The destination's name.
    pub name: String,
    /// The territory text (empty: the row is listed at the top level, rule 5).
    pub territory: String,
    /// The area text.
    pub area: String,
    /// The cost in coin (the feed's `coin`; 0 = free).
    pub coin: u64,
    /// The cost split into coins (§9.3 of `npc-windows.md`).
    pub coins: CoinCounts,
    /// The minimum level shown (0 when none).
    pub min_level: u16,
    /// A swift destination.
    pub swift: bool,
    /// Discovered (flag bit 0 clear).
    pub discovered: bool,
    /// Requirements met (flag bit 1 clear).
    pub met: bool,
    /// Usable (flag bit 2 clear).
    pub usable: bool,
}

/// A `TravelShown`, rendered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TravelView {
    /// The feed window.
    pub window: WindowId,
    /// The title (TW2's text with the source's name, or with the variable empty, §4.10
    /// rule 1), rendered by the client.
    pub title: String,
    /// The local player's level (for the failed-requirements mark).
    pub player_level: u32,
    /// The rows, in entry order.
    pub rows: Vec<TravelRow>,
}

/// A fixed label of the travel window (`travel-windows.md` §4.2, §4.7, §4.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TravelLabel {
    /// The go button without a selection, or ghosted while discovered (`Travel` token
    /// 0x0481FBC4).
    GoNoSelection,
    /// The go button for an undiscovered destination (0x0740F624).
    GoUndiscovered,
    /// The go button for a usable destination (0x01D0CE24).
    GoUsable,
    /// The free text (0x08FB7675).
    Free,
    /// `Travel_RouteStatus` without a selection (0x07F54814).
    NoRouteSelected,
    /// `Travel_RouteStatus` for a discovered destination (0x0250F084).
    Discovered,
    /// `Travel_RouteStatus` for an undiscovered destination (0x0320D014).
    Undiscovered,
    /// The swift label (0x0F4A924C; also appended to a swift row's search text).
    Swift,
    /// `Travel_NoDestination_Label`.
    NoDestination,
    /// `Travel_Territory_Label`.
    Territory,
    /// `Travel_Area_Label`.
    Area,
    /// The search label.
    Search,
    /// The end-travel box's text (`UI` 0x00D287E4).
    EndTravelText,
    /// The end-travel button (`UI` 0x0B961B54).
    EndTravelButton,
}

impl TravelLabel {
    /// Tessera's own short text.
    #[must_use]
    pub fn fallback(self) -> &'static str {
        match self {
            Self::GoNoSelection => "Select a destination",
            Self::GoUndiscovered => "Not discovered",
            Self::GoUsable => "Travel",
            Self::Free => "Free",
            Self::NoRouteSelected => "No route selected",
            Self::Discovered => "Discovered",
            Self::Undiscovered => "Undiscovered",
            Self::Swift => "Swift travel",
            Self::NoDestination => "No destinations",
            Self::Territory => "Territory:",
            Self::Area => "Area:",
            Self::Search => "Search",
            Self::EndTravelText => "Travelling",
            Self::EndTravelButton => "End travel",
        }
    }
}

/// Texts the travel window asks the client for (rendered from the install's string
/// tables); the defaults are Tessera's own.
pub trait TravelTexts: NpcTexts {
    /// A fixed label; `None` keeps the layout's own text (and gives the fallback widgets
    /// [`TravelLabel::fallback`]).
    fn travel_label(&self, label: TravelLabel) -> Option<String> {
        let _ = label;
        None
    }

    /// `Travel_Currency_MinLevel`'s text (§4.7: `UI` token 0x0682DDAC with the level, or
    /// with token 0x0B8C7995's text below level 2); default Tessera's own.
    fn min_level(&self, level: u16) -> String {
        if level < 2 {
            "Any level".to_string()
        } else {
            format!("Level {level}")
        }
    }
}

/// [`TravelTexts`] with every default.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultTravelTexts;

impl NpcTexts for DefaultTravelTexts {}
impl TravelTexts for DefaultTravelTexts {}

/// The label's text: the client's, else Tessera's own.
#[must_use]
pub fn travel_label(texts: &dyn TravelTexts, l: TravelLabel) -> String {
    texts
        .travel_label(l)
        .unwrap_or_else(|| l.fallback().to_string())
}

/// A group of rows: a territory header (`None`: rows at the top level).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TravelGroup {
    /// The territory text of the header.
    pub territory: Option<String>,
    /// The rows (indexes into the model's rows), in entry order.
    pub rows: Vec<usize>,
}

/// The marks of one row (§4.4 rules 1, 3, 6 with the feed flags, §4.10 rule 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowMarks {
    /// The failed-requirements icon: the player's level below the minimum level, or the
    /// requirements not met.
    pub failed: bool,
    /// The unknown-destination icon: not discovered.
    pub unknown: bool,
    /// Ghosted (state `ghosted` and the overlay): not usable.
    pub ghosted: bool,
    /// The swift icon.
    pub swift: bool,
}

/// What `Travel_Currency_DestinationCost` and `…_Free` show (§4.7; TI2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostShown {
    /// No selection: both hidden.
    Nothing,
    /// The money display with this cost.
    Coin(CoinCounts),
    /// The free text.
    Free,
}

/// `Travel_RouteStatus`'s state (§4.7; TI3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteStatus {
    /// `Travel_NoRouteSelected`.
    NoRouteSelected,
    /// `Travel_DestinationDiscovered`.
    Discovered,
    /// `Travel_DestinationUndiscovered`.
    Undiscovered,
}

/// The selection display (§4.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionDisplay {
    /// The cost.
    pub cost: CostShown,
    /// The minimum level text's level (`None`: hidden).
    pub min_level: Option<u16>,
    /// The route status.
    pub status: RouteStatus,
    /// The territory and area texts (`None`: their fields hidden).
    pub place: Option<(String, String)>,
    /// The swift label shown.
    pub swift: bool,
}

/// The coin go button (§4.8, §4.10 rule 4; TI4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoButton {
    /// Ghosted (no selection, undiscovered or unusable).
    pub ghosted: bool,
    /// The button's text.
    pub label: TravelLabel,
    /// `Travel_CurrencyButtonLabel_NoRouteSelected` shown (no selection).
    pub no_route_label: bool,
}

/// The travel window's model: rows, search filter and selection (§4.4–§4.8, §4.10).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TravelModel {
    /// The rows, in entry order.
    pub rows: Vec<TravelRow>,
    /// The local player's level.
    pub player_level: u32,
    /// The selected row.
    pub selected: Option<usize>,
    /// The search filter (as typed).
    pub filter: String,
}

/// Tessera's lower-casing of the search (`travel-windows.md` §4.6 Tessera rule: ASCII and
/// the Unicode simple lower-case mapping).
#[must_use]
pub fn fold(s: &str) -> String {
    s.chars().flat_map(char::to_lowercase).collect()
}

impl TravelModel {
    /// The model of a view (at most [`MAX_ROWS`] rows). The selection is kept when the row
    /// with the same entry is listed again and not ghosted (§4.4 rule 7).
    pub fn show(&mut self, view: &TravelView) {
        let before = self
            .selected
            .and_then(|i| self.rows.get(i))
            .map(|r| r.entry);
        self.rows = view.rows.iter().take(MAX_ROWS).cloned().collect();
        self.player_level = view.player_level;
        self.selected =
            before.and_then(|e| self.rows.iter().position(|r| r.entry == e && r.usable));
    }

    /// The search text of row `i` (§4.4 rule 2): name, area, territory, then the swift
    /// label's text when swift.
    #[must_use]
    pub fn search_text(&self, i: usize, swift_label: &str) -> String {
        let Some(r) = self.rows.get(i) else {
            return String::new();
        };
        let mut s = format!("{}{}{}", r.name, r.area, r.territory);
        if r.swift {
            s.push_str(swift_label);
        }
        s
    }

    /// Whether row `i` passes the search filter (§4.6 and its Tessera rule).
    #[must_use]
    pub fn matches(&self, i: usize, swift_label: &str) -> bool {
        self.filter.is_empty()
            || fold(&self.search_text(i, swift_label)).contains(&fold(&self.filter))
    }

    /// The groups of the rows that pass the filter (§4.5 and §4.10 rule 5; TI1): rows with
    /// an empty territory at the top level before any header; then one header per
    /// territory text in the order of its first row, rows in entry order; a header whose
    /// rows are all filtered out is left out (`auto_filter_parent_with_filtered_children`).
    #[must_use]
    pub fn groups(&self, swift_label: &str) -> Vec<TravelGroup> {
        let mut top = Vec::new();
        let mut groups: Vec<TravelGroup> = Vec::new();
        for (i, r) in self.rows.iter().enumerate() {
            let pass = self.matches(i, swift_label);
            if r.territory.is_empty() {
                if pass {
                    top.push(i);
                }
                continue;
            }
            let g = match groups
                .iter()
                .position(|g| g.territory.as_deref() == Some(r.territory.as_str()))
            {
                Some(g) => g,
                None => {
                    groups.push(TravelGroup {
                        territory: Some(r.territory.clone()),
                        rows: Vec::new(),
                    });
                    groups.len() - 1
                }
            };
            if pass {
                groups[g].rows.push(i);
            }
        }
        let mut out = Vec::new();
        if !top.is_empty() {
            out.push(TravelGroup {
                territory: None,
                rows: top,
            });
        }
        out.extend(groups.into_iter().filter(|g| !g.rows.is_empty()));
        out
    }

    /// The marks of row `i`.
    #[must_use]
    pub fn marks(&self, i: usize) -> RowMarks {
        let Some(r) = self.rows.get(i) else {
            return RowMarks::default();
        };
        RowMarks {
            failed: self.player_level < u32::from(r.min_level) || !r.met,
            unknown: !r.discovered,
            ghosted: !r.usable,
            swift: r.swift,
        }
    }

    /// The selection display (§4.7).
    #[must_use]
    pub fn selection(&self) -> SelectionDisplay {
        match self.selected.and_then(|i| self.rows.get(i)) {
            None => SelectionDisplay {
                cost: CostShown::Nothing,
                min_level: None,
                status: RouteStatus::NoRouteSelected,
                place: None,
                swift: false,
            },
            Some(r) => SelectionDisplay {
                cost: if r.coin == 0 {
                    CostShown::Free
                } else {
                    CostShown::Coin(r.coins)
                },
                min_level: Some(r.min_level),
                status: if r.discovered {
                    RouteStatus::Discovered
                } else {
                    RouteStatus::Undiscovered
                },
                place: Some((r.territory.clone(), r.area.clone())),
                swift: r.swift,
            },
        }
    }

    /// The coin go button (§4.8 with §4.10 rule 4): `normal` only with a selected row that
    /// is discovered and usable; a ghosted button keeps the undiscovered text when not
    /// discovered and the no-selection text otherwise.
    #[must_use]
    pub fn go_button(&self) -> GoButton {
        match self.selected.and_then(|i| self.rows.get(i)) {
            None => GoButton {
                ghosted: true,
                label: TravelLabel::GoNoSelection,
                no_route_label: true,
            },
            Some(r) if r.discovered && r.usable => GoButton {
                ghosted: false,
                label: TravelLabel::GoUsable,
                no_route_label: false,
            },
            Some(r) => GoButton {
                ghosted: true,
                label: if r.discovered {
                    TravelLabel::GoNoSelection
                } else {
                    TravelLabel::GoUndiscovered
                },
                no_route_label: false,
            },
        }
    }

    /// The go command (§4.10 rule 6): `ChooseTravel` for the selected row while the go
    /// button is `normal`.
    #[must_use]
    pub fn go(&self, window: WindowId) -> Option<TravelAction> {
        let r = self.selected.and_then(|i| self.rows.get(i))?;
        (!self.go_button().ghosted).then_some(TravelAction::Choose {
            window,
            entry: r.entry,
        })
    }
}

/// The travel window's templates in the install's `travel` layout: `Travel_ListItem` and
/// `Travel_ListItem_Header` (§4.2). `None` uses a fallback row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TravelTemplates {
    /// The row template.
    pub row: Option<ExamTemplate>,
    /// The header template.
    pub header: Option<ExamTemplate>,
}

/// The parts of the travel window (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TravelParts {
    /// `Travel_NameText`.
    pub title: Option<ElementKey>,
    /// `Travel_CloseButton`.
    pub close: Option<ElementKey>,
    /// `TravelListBox`.
    pub tree: Option<ElementKey>,
    /// `TravelList_SearchText`.
    pub search: Option<ElementKey>,
    /// `Travel_Expand_All_Button`.
    pub expand_all: Option<ElementKey>,
    /// `Travel_Collapse_All_Button`.
    pub collapse_all: Option<ElementKey>,
    /// `Travel_NoDestinationField`.
    pub no_destination: Option<ElementKey>,
    /// `Travel_Currency_MinLevel`.
    pub min_level: Option<ElementKey>,
    /// `Travel_RouteStatus`.
    pub route_status: Option<ElementKey>,
    /// `Travel_SwiftTravel`.
    pub swift_label: Option<ElementKey>,
    /// `Travel_Territory_Field`.
    pub territory_field: Option<ElementKey>,
    /// `Travel_Territory`.
    pub territory: Option<ElementKey>,
    /// `Travel_Area_Field`.
    pub area_field: Option<ElementKey>,
    /// `Travel_Area`.
    pub area: Option<ElementKey>,
    /// `Travel_Currency_GoButton`.
    pub go: Option<ElementKey>,
    /// `Travel_Currency_DestinationCost`.
    pub cost: Option<ElementKey>,
    /// `Travel_Currency_DestinationCost_Free`.
    pub free: Option<ElementKey>,
    /// `Travel_CurrencyButtonLabel_NoRouteSelected`.
    pub no_route_label: Option<ElementKey>,
}

/// A travel window instance.
#[derive(Debug, Clone)]
pub struct TravelWindow {
    /// The feed window.
    pub window: WindowId,
    /// The root (`Travel_Window`).
    pub root: ElementKey,
    /// The parts.
    pub parts: TravelParts,
    /// The model.
    pub model: TravelModel,
    /// The row elements with their row indexes.
    pub rows: Vec<(ElementKey, usize)>,
    /// The header elements.
    pub headers: Vec<ElementKey>,
    cost_display: Option<MoneyDisplay>,
    swift_text: String,
    templates: TravelTemplates,
    art: P5Art,
    nv: NpcVocab,
    pv: P5Vocab,
    listen: Listen,
}

impl TravelWindow {
    /// Finds the window's parts below `root` (§4.2), hides the paper-item and store parts
    /// (§4.8 Tessera rule) and builds the cost display.
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        pv: &P5Vocab,
        root: ElementKey,
        templates: TravelTemplates,
        art: &P5Art,
    ) -> Self {
        let ids = pv.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        for id in [
            ids.travel_paper_go,
            ids.travel_buy_now,
            ids.travel_paper_balance,
        ] {
            show(ui, f(ui, id), false);
        }
        let parts = TravelParts {
            title: f(ui, ids.travel_title),
            close: f(ui, ids.travel_close),
            tree: f(ui, ids.travel_tree),
            search: f(ui, ids.travel_search),
            expand_all: f(ui, ids.travel_expand_all),
            collapse_all: f(ui, ids.travel_collapse_all),
            no_destination: f(ui, ids.travel_no_destination),
            min_level: f(ui, ids.travel_min_level),
            route_status: f(ui, ids.travel_route_status),
            swift_label: f(ui, ids.travel_swift_label),
            territory_field: f(ui, ids.travel_territory_field),
            territory: f(ui, ids.travel_territory),
            area_field: f(ui, ids.travel_area_field),
            area: f(ui, ids.travel_area),
            go: f(ui, ids.travel_go),
            cost: f(ui, ids.travel_cost),
            free: f(ui, ids.travel_free),
            no_route_label: f(ui, ids.travel_no_route_label),
        };
        let cost_display = parts
            .cost
            .and_then(|c| MoneyDisplay::find(ui, nv, c))
            .map(|m| MoneyDisplay::attach(ui, nv, m, &art.base));
        let swift_text = parts
            .swift_label
            .and_then(|k| text_part(ui, k, None))
            .map(|k| ui.text_of(k))
            .unwrap_or_default();
        let mut w = Self {
            window: 0,
            root,
            parts,
            model: TravelModel::default(),
            rows: Vec::new(),
            headers: Vec::new(),
            cost_display,
            swift_text,
            templates,
            art: art.clone(),
            nv: nv.clone(),
            pv: pv.clone(),
            listen: Listen::default(),
        };
        for k in [parts.close, parts.go, parts.expand_all, parts.collapse_all] {
            w.listen.clicks(ui, k);
        }
        let ms = ui.vocab().msgs;
        for m in [ms.character_input, ms.key_press, ms.key_release] {
            w.listen.on(ui, parts.search, m);
        }
        w
    }

    /// The swift label's text (the layout's own, else the client's, else Tessera's).
    fn swift_label(&self, texts: &dyn TravelTexts) -> String {
        if self.swift_text.is_empty() {
            travel_label(texts, TravelLabel::Swift)
        } else {
            self.swift_text.clone()
        }
    }

    /// Shows a `TravelShown` (§4.10): the title, the rows grouped under territory headers
    /// with their marks, `Travel_NoDestinationField` while there is no row, and the
    /// selection display.
    ///
    /// The selection is carried over only within the same feed window (§4.4 rule 7); a
    /// `TravelShown` for another window starts without one (§4.9: closing clears it).
    pub fn show(&mut self, ui: &mut Ui, view: &TravelView, texts: &dyn TravelTexts) {
        if view.window != self.window {
            self.model.selected = None;
        }
        self.window = view.window;
        put_text(ui, self.parts.title, &view.title);
        self.model.show(view);
        if self.swift_text.is_empty()
            && let Some(k) = self.parts.swift_label
        {
            put_text(ui, Some(k), &travel_label(texts, TravelLabel::Swift));
        }
        self.fill(ui, texts);
        self.refresh_selection(ui, texts);
    }

    /// Rebuilds the tree from the model's groups (§4.4–§4.6).
    fn fill(&mut self, ui: &mut Ui, texts: &dyn TravelTexts) {
        let Some(tree) = self.parts.tree else { return };
        let mut old: Vec<ElementKey> = self.rows.drain(..).map(|(k, _)| k).collect();
        old.append(&mut self.headers);
        clear_rows(ui, tree, &mut old);
        let swift = self.swift_label(texts);
        let groups = self.model.groups(&swift);
        let ids = self.pv.ids;
        let cp = self.nv.pids.text_font_color;
        let art = self.art.clone();
        let st = ui.vocab().states;
        let mut commit: Vec<TreeRow> = Vec::new();
        for g in &groups {
            let parent = match &g.territory {
                Some(t) => {
                    let h = make_row(ui, tree, self.templates.header, || {
                        FNode::text(
                            ids.travel_header,
                            0,
                            0,
                            ROW_SIZE.0,
                            26,
                            cp,
                            colours::WHITE,
                            "",
                        )
                    });
                    let Some(h) = h else { break };
                    let el = text_part(ui, h, None).unwrap_or(h);
                    put_text(ui, Some(el), t);
                    self.headers.push(h);
                    commit.push(TreeRow {
                        row: h,
                        parent: None,
                        expanded: true,
                    });
                    Some(h)
                }
                None => None,
            };
            for &i in &g.rows {
                let row = make_row(ui, tree, self.templates.row, || {
                    fallback_row(&ids, cp, &art, st.normal, st.highlight, st.ghosted)
                });
                let Some(r) = row else { break };
                self.fill_row(ui, r, i);
                super::popup::listen_row(ui, &mut self.listen, r);
                self.rows.push((r, i));
                commit.push(TreeRow {
                    row: r,
                    parent,
                    expanded: true,
                });
            }
        }
        let kept = commit_tree_rows(ui, tree, &commit);
        self.rows.retain(|(k, _)| kept.contains(k));
        self.headers.retain(|k| kept.contains(k));
        // §4.10 rule 7, §4.3: shown while the window has no row.
        show(ui, self.parts.no_destination, self.model.rows.is_empty());
        if let Some(nd) = self.parts.no_destination
            && let Some(t) = text_part(ui, nd, None)
            && ui.text_of(t).is_empty()
        {
            put_text(
                ui,
                Some(t),
                &travel_label(texts, TravelLabel::NoDestination),
            );
        }
        self.mark_selected(ui);
    }

    /// Fills one row element (§4.4 rules 1, 3, 4, 6).
    fn fill_row(&self, ui: &mut Ui, r: ElementKey, i: usize) {
        let ids = self.pv.ids;
        let Some(row) = self.model.rows.get(i) else {
            return;
        };
        let m = self.model.marks(i);
        let text = text_part(ui, r, Some(ids.travel_row_text));
        put_text(ui, text, &row.name);
        show(ui, find(ui, r, ids.travel_row_failed), m.failed);
        show(ui, find(ui, r, ids.travel_row_unknown), m.unknown);
        show(ui, find(ui, r, ids.travel_row_swift), m.swift);
        show(ui, find(ui, r, ids.travel_row_ghosted), m.ghosted);
        let st = ui.vocab().states;
        switch_if_defined(ui, r, if m.ghosted { st.ghosted } else { st.normal });
    }

    /// Marks the selected row (`highlight`) and the others (`normal`, or `ghosted`).
    fn mark_selected(&self, ui: &mut Ui) {
        let st = ui.vocab().states;
        for (k, i) in &self.rows {
            if self.model.marks(*i).ghosted {
                switch_if_defined(ui, *k, st.ghosted);
            } else {
                mark_row(ui, *k, self.model.selected == Some(*i));
            }
        }
    }

    /// Fills the selection display and the go button (§4.7, §4.8, §4.10 rule 4).
    fn refresh_selection(&mut self, ui: &mut Ui, texts: &dyn TravelTexts) {
        let s = self.model.selection();
        let p = self.parts;
        match s.cost {
            CostShown::Nothing => {
                show(ui, p.cost, false);
                show(ui, p.free, false);
            }
            CostShown::Free => {
                show(ui, p.cost, false);
                show(ui, p.free, true);
                if let Some(f) = p.free
                    && let Some(t) = text_part(ui, f, None)
                    && ui.text_of(t).is_empty()
                {
                    put_text(ui, Some(t), &travel_label(texts, TravelLabel::Free));
                }
            }
            CostShown::Coin(c) => {
                show(ui, p.cost, true);
                show(ui, p.free, false);
                if let Some(d) = self.cost_display.as_mut() {
                    d.set(ui, c, false, texts);
                }
            }
        }
        match s.min_level {
            Some(l) => {
                show(ui, p.min_level, true);
                put_text(
                    ui,
                    p.min_level.and_then(|k| text_part(ui, k, None)),
                    &texts.min_level(l),
                );
            }
            None => show(ui, p.min_level, false),
        }
        if let Some(rs) = p.route_status {
            let (state, label) = match s.status {
                RouteStatus::NoRouteSelected => (
                    self.pv.states.no_route_selected,
                    TravelLabel::NoRouteSelected,
                ),
                RouteStatus::Discovered => (
                    self.pv.states.destination_discovered,
                    TravelLabel::Discovered,
                ),
                RouteStatus::Undiscovered => (
                    self.pv.states.destination_undiscovered,
                    TravelLabel::Undiscovered,
                ),
            };
            show(ui, Some(rs), true);
            if super::defines_state(ui, rs, state) {
                ui.switch_state(rs, state);
            } else {
                let el = text_part(ui, rs, None).unwrap_or(rs);
                put_text(ui, Some(el), &travel_label(texts, label));
            }
        }
        match &s.place {
            Some((t, a)) => {
                show(ui, p.territory_field, true);
                show(ui, p.area_field, true);
                put_text(ui, p.territory, t);
                put_text(ui, p.area, a);
            }
            None => {
                show(ui, p.territory_field, false);
                show(ui, p.area_field, false);
            }
        }
        show(ui, p.swift_label, s.swift);
        let go = self.model.go_button();
        if let Some(g) = p.go {
            let gp = ui.vocab().pids.button_ghosted;
            ui.set_property(
                g,
                gp,
                tessera_formats::property::PropertyValue::Bool(go.ghosted),
            );
            if ui.class(g) != Some(WidgetClass::Button) {
                let st = ui.vocab().states;
                switch_if_defined(ui, g, if go.ghosted { st.ghosted } else { st.normal });
            }
            // The go button carries its own text (`Button_General_Use`, §4.2); its child
            // texts are the cost parts.
            put_text(ui, Some(g), &travel_label(texts, go.label));
        }
        show(ui, p.no_route_label, go.no_route_label);
        self.mark_selected(ui);
    }

    /// Closing (§4.9): forgets the shown feed window and clears the selection. The client
    /// calls it when the window closes (`WindowClosed`, a travel request, the close button).
    pub fn close(&mut self, ui: &mut Ui, texts: &dyn TravelTexts) {
        self.window = 0;
        self.model.selected = None;
        self.refresh_selection(ui, texts);
    }

    /// Selects row `i` (or none) and refreshes the display.
    pub fn select(&mut self, ui: &mut Ui, i: Option<usize>, texts: &dyn TravelTexts) {
        self.model.selected = i.filter(|i| *i < self.model.rows.len());
        self.refresh_selection(ui, texts);
    }

    /// Sets the search filter (§4.6) and rebuilds the tree.
    pub fn set_filter(&mut self, ui: &mut Ui, filter: &str, texts: &dyn TravelTexts) {
        if self.model.filter == filter {
            return;
        }
        self.model.filter = super::npc::kit::cut(filter);
        self.fill(ui, texts);
    }

    /// The command for a UI event: row selection and double-click go, the go button, the
    /// close button, expand and collapse all, and typing in the search field (§4.6–§4.9).
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        texts: &dyn TravelTexts,
    ) -> Option<TravelAction> {
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
                return Some(TravelAction::Close {
                    window: self.window,
                });
            }
            if hit(self.parts.go) {
                return self.model.go(self.window);
            }
            for (b, on) in [
                (self.parts.expand_all, true),
                (self.parts.collapse_all, false),
            ] {
                if hit(b) {
                    if let Some(tree) = self.parts.tree {
                        for h in self.headers.clone() {
                            ui.tree_set_expanded(tree, h, on);
                        }
                    }
                    return None;
                }
            }
        }
        let ks: Vec<ElementKey> = self.rows.iter().map(|(k, _)| *k).collect();
        let r = row_of(ui, &ks, src)?;
        let i = self.rows[r].1;
        match kind {
            ClickKind::Left | ClickKind::Button => {
                self.select(ui, Some(i), texts);
                None
            }
            ClickKind::Double => {
                self.select(ui, Some(i), texts);
                self.model.go(self.window)
            }
            ClickKind::Right => None,
        }
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}

/// A fallback `Travel_ListItem` (§4.2 row template): 400 × 22 with the two overlays
/// (hidden), the ghosted overlay (hidden), the swift icon at (9, 3) and the name at (31, 4).
pub(crate) fn fallback_row(
    ids: &super::p5vocab::P5Ids,
    cp: u32,
    art: &P5Art,
    normal: u32,
    highlight: u32,
    ghosted: u32,
) -> FNode {
    use super::npc::kit::colour_value;
    FNode::field(ids.travel_row, 0, 0, ROW_SIZE.0, ROW_SIZE.1)
        .image(&art.base.row)
        .state(normal, art.base.row.as_ref(), vec![])
        .state(highlight, art.base.row_highlight.as_ref(), vec![])
        .state(ghosted, art.base.row.as_ref(), vec![])
        .children([
            FNode::field(ids.travel_row_unknown, 0, 0, ROW_SIZE.0, ROW_SIZE.1)
                .image(&art.unknown)
                .hidden(),
            FNode::field(ids.travel_row_failed, 0, 0, ROW_SIZE.0, ROW_SIZE.1)
                .image(&art.failed)
                .hidden(),
            FNode::field(ids.travel_row_ghosted, 0, 0, ROW_SIZE.0, ROW_SIZE.1)
                .image(&art.ghosted)
                .hidden(),
            FNode::field(ids.travel_row_swift, 9, 3, 16, 16)
                .image(&art.swift)
                .hidden(),
            FNode::text(ids.travel_row_text, 31, 3, 367, 16, cp, 0xFFCC_9933, "")
                .state(normal, None, vec![(cp, colour_value(0xFFCC_9933))])
                .state(highlight, None, vec![(cp, colour_value(colours::WHITE))]),
        ])
}

// ----- the end-travel box and the travel presentation (§5) ----------------------------

/// The end-travel box (`GamePlay_EndTravel`, §5.1, §5.2 rule 2): shown for a slow travel's
/// duration; its button sends `Dismount` (§5.2 rule 4).
#[derive(Debug, Clone)]
pub struct EndTravelBox {
    /// The box (`GamePlay_EndTravel`).
    pub root: ElementKey,
    /// `EndTravel_Button`.
    pub button: Option<ElementKey>,
    listen: Listen,
}

impl EndTravelBox {
    /// Finds the box at or below `k` and its button, fills the texts when the layout gives
    /// none, and hides it.
    #[must_use]
    pub fn attach(ui: &mut Ui, pv: &P5Vocab, k: ElementKey, texts: &dyn TravelTexts) -> Self {
        let ids = pv.ids;
        let root = find(ui, k, ids.end_travel).unwrap_or(k);
        let button = find(ui, root, ids.end_travel_button);
        for (id, l) in [
            (ids.end_travel_text, TravelLabel::EndTravelText),
            (ids.end_travel_button, TravelLabel::EndTravelButton),
        ] {
            if let Some(e) = find(ui, root, id)
                && let Some(t) = text_part(ui, e, None)
                && ui.text_of(t).is_empty()
            {
                put_text(ui, Some(t), &travel_label(texts, l));
            }
        }
        ui.set_shown(root, false);
        let mut b = Self {
            root,
            button,
            listen: Listen::default(),
        };
        b.listen.clicks(ui, button);
        b
    }

    /// Shows or hides the box.
    pub fn set_shown(&self, ui: &mut Ui, on: bool) {
        ui.set_shown(self.root, on);
    }

    /// `Dismount` for a click on the button.
    pub fn handle_event(&self, ui: &Ui, ev: &UiEvent) -> Option<TravelAction> {
        let (src, kind) = click(ui, ev)?;
        (matches!(kind, ClickKind::Button | ClickKind::Left)
            && self.button.is_some_and(|b| ui.is_ancestor_or_self(b, src)))
        .then_some(TravelAction::Dismount)
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}

/// How a started travel is presented (§5.2 rules 1–3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TravelShow {
    /// Nothing (duration 0).
    Nothing,
    /// The loading screen with its meter (a swift destination).
    LoadingScreen,
    /// The end-travel box (a slow destination).
    EndTravelBox,
}

/// The travel presentation of a `TravelStarted` (§5.2), on the client's clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TravelPresentation {
    /// What is shown.
    pub show: TravelShow,
    /// When it started (seconds).
    pub start: f64,
    /// Its duration (seconds).
    pub duration: f64,
    /// Ended early (a teleport or `TravelEnded`).
    pub ended: bool,
}

impl TravelPresentation {
    /// A `TravelStarted` at time `now` (seconds) with the destination's swift flag and the
    /// duration in milliseconds (rules 1–3; a new one replaces a running one, rule 7).
    #[must_use]
    pub fn start(now: f64, swift: bool, duration_ms: u32) -> Self {
        let show = if duration_ms == 0 {
            TravelShow::Nothing
        } else if swift {
            TravelShow::LoadingScreen
        } else {
            TravelShow::EndTravelBox
        };
        Self {
            show,
            start: now,
            duration: f64::from(duration_ms) / 1000.0,
            ended: false,
        }
    }

    /// A teleport or `TravelEnded` ends it (rules 4–5).
    pub fn end(&mut self) {
        self.ended = true;
    }

    /// Whether the presentation is over at `now`.
    #[must_use]
    pub fn over(&self, now: f64) -> bool {
        self.ended || self.show == TravelShow::Nothing || now >= self.start + self.duration
    }

    /// What is shown at `now` ([`TravelShow::Nothing`] once over).
    #[must_use]
    pub fn shown(&self, now: f64) -> TravelShow {
        if self.over(now) {
            TravelShow::Nothing
        } else {
            self.show
        }
    }

    /// The loading screen meter at `now` (rule 1: linear from 0 to 1 over the duration).
    #[must_use]
    pub fn meter(&self, now: f64) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        #[allow(clippy::cast_possible_truncation)]
        let f = ((now - self.start) / self.duration).clamp(0.0, 1.0) as f32;
        f
    }
}
