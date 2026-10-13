// SPDX-License-Identifier: GPL-3.0-or-later
//! The travel window's presentation model (decision 0024 P5): rows built from the feed's
//! `TravelShown` entries with their display names, route mode, cost display, minimum level,
//! marks, territory and area texts, search text and grouping; the selection display and the
//! go button; the travel presentation of `TravelStarted`; and the census of every travel
//! destination and station row.
//!
//! Spec: `docs/specs/travel-windows.md` §4.2–§4.10, §5.2, §8.1 (`map-data.md` §8 for the
//! records, `npc-windows.md` §9.2–§9.3 for coins). The client computes no list, cost,
//! level, usability or eligibility (decision 0024 §2 principle 4): those are the feed's.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`TravelPresenter`], [`TravelEntryIn`], [`RouteMode`], [`TravelFlags`] | Builds the window from feed entries | §4.10 |
//! | [`TravelWindow`], [`TravelRow`], [`RowMarks`], [`TravelGroup`], [`LevelText`], [`TravelWindow::filter`] | Rows, marks, grouping, search | §4.4–§4.6, §4.10 rules 1–5, 7 |
//! | [`SelectionDisplay`], [`RouteStatus`], [`GoButton`], [`TravelWindow::selection`] | The selection display and the coin go button | §4.7, §4.8, §4.10 rule 4 |
//! | [`TravelPresentation`], [`travel_presentation`] | Loading screen or end-travel box for `TravelStarted` | §5.2 |
//! | [`travel_census`], [`TravelCensus`], [`DestinationCategory`] | Census of every destination and station row | §8.1 |
//! | [`tokens`] | String-table tokens the spec names | §4.2, §4.7, §4.8 |
//!
//! # Hostile input
//!
//! Nothing here parses bytes. A window holds at most [`MAX_ROWS`] rows; search texts are
//! compared on at most [`MAX_SEARCH_CHARS`] characters.

mod census;

#[cfg(test)]
mod tests;

pub use census::{DestinationCategory, StationRows, TravelCensus, travel_census};

use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::gameplay::map::{MapPids, TravelRoute};
use tessera_formats::{Did, StringInfo};
use tessera_text::{Value, Values};

use crate::npc::{CoinDisplay, CoinOptions, NpcPresenter};
use crate::quest::{PresentationData, RecordRef, token_info};
use crate::tooltip::TextRef;

/// Most rows of one travel window (`scenario-feed.md` §6.3: 1,024 entries per list).
pub const MAX_ROWS: usize = 1_024;
/// Most characters of a search filter or search text compared (the search field holds 80;
/// a Tessera bound).
pub const MAX_SEARCH_CHARS: usize = 1_024;

/// The `Travel` string table's label (`travel-windows.md` §4.1, TW2).
pub const TRAVEL_TABLE: &str = "Travel";
/// The `UI` string table's label.
pub const UI_TABLE: &str = "UI";

/// String-table tokens of `travel-windows.md` (this install's values, named by the spec).
pub mod tokens {
    /// `Travel`: the window title; variable [`VAR_TITLE_NAME`] = the entity's name (TW2).
    pub const TITLE: u32 = 0x0269_CC25;
    /// The title's name variable.
    pub const VAR_TITLE_NAME: u32 = 0x5343;
    /// `Travel`: the free text (§4.2 `Travel_Currency_DestinationCost_Free`).
    pub const FREE: u32 = 0x08FB_7675;
    /// `Travel`: the swift label (§4.2 `Travel_SwiftTravel`).
    pub const SWIFT: u32 = 0x0F4A_924C;
    /// `Travel`: the swift icon's tooltip (§4.2 `Travel_ListItem_SwiftTravel`).
    pub const SWIFT_TOOLTIP: u32 = 0x0925_F034;
    /// `UI`: the minimum-level text; variable [`VAR_MIN_LEVEL`] (§4.7).
    pub const MIN_LEVEL: u32 = 0x0682_DDAC;
    /// The minimum-level variable.
    pub const VAR_MIN_LEVEL: u32 = 0x0050_AA9C;
    /// `UI`: the fixed text used as the level below 2 (§4.7).
    pub const LEVEL_BELOW_TWO: u32 = 0x0B8C_7995;
    /// `Travel`: route status texts: no route selected, discovered, undiscovered (§4.7).
    pub const ROUTE_STATUS: [u32; 3] = [0x07F5_4814, 0x0250_F084, 0x0320_D014];
    /// `Travel`: go button without selection (§4.8).
    pub const GO_NO_SELECTION: u32 = 0x0481_FBC4;
    /// `Travel`: go button, destination undiscovered (§4.8).
    pub const GO_UNDISCOVERED: u32 = 0x0740_F624;
    /// `Travel`: go button, usable (§4.8).
    pub const GO_USABLE: u32 = 0x01D0_CE24;
    /// `UI`: the empty-list label (§4.2 `Travel_NoDestination_Label`).
    pub const NO_DESTINATION: u32 = 0x0416_3FDC;
}

tessera_formats::property_ids! {
    /// Properties the travel model reads besides the travel network's.
    pub(crate) struct TravelPids {
        area_name = "Area_Name",
        dungeon_name = "Dungeon_Name",
        area_territory = "Area_Territory",
        min_level = "Usage_MinLevel",
    }
}

/// How a destination is reached (`scenario-feed.md` §9.1 `RouteMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteMode {
    /// A swift destination (`TravelDestinationIsSwiftTravel` true).
    Swift,
    /// A slow destination.
    Slow,
}

/// `TravelFlags` of a feed entry (`scenario-feed.md` §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TravelFlags(pub u8);

impl TravelFlags {
    /// Bit 0: not discovered.
    pub const NOT_DISCOVERED: u8 = 1;
    /// Bit 1: requirements not met.
    pub const NOT_MET: u8 = 2;
    /// Bit 2: not usable.
    pub const NOT_USABLE: u8 = 4;

    /// Discovered (bit 0 clear).
    #[must_use]
    pub fn discovered(self) -> bool {
        self.0 & Self::NOT_DISCOVERED == 0
    }

    /// Requirements met (bit 1 clear).
    #[must_use]
    pub fn met(self) -> bool {
        self.0 & Self::NOT_MET == 0
    }

    /// Usable (bit 2 clear).
    #[must_use]
    pub fn usable(self) -> bool {
        self.0 & Self::NOT_USABLE == 0
    }
}

/// One `TravelShown` entry as the client half receives it (`scenario-feed.md` §9.1
/// `TravelEntry`; the client's adapter copies the feed's values).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TravelEntryIn {
    /// The entry number.
    pub entry: u16,
    /// A destination or route record, or a literal.
    pub destination: RecordRef,
    /// The cost in coin units (never computed).
    pub coin: u64,
    /// The minimum level, when the feed gives one.
    pub min_level: Option<u16>,
    /// The flags.
    pub flags: TravelFlags,
    /// The route mode, when the feed gives one.
    pub route: Option<RouteMode>,
    /// The area, when the feed gives one.
    pub area: Option<RecordRef>,
}

/// The minimum-level display (§4.7): the level, or the fixed text below 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LevelText {
    /// The level is below 2: the variable is the fixed text of [`tokens::LEVEL_BELOW_TWO`].
    BelowTwo,
    /// The level itself.
    Level(u32),
}

/// A row's marks and state (§4.4, §4.10 rule 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowMarks {
    /// `Travel_ListItem_DestinationIcons_FailedRequirements_Icon`: level above the
    /// player's, or requirements not met.
    pub failed_requirements: bool,
    /// `…_UnknownDestination_Icon`: not discovered.
    pub unknown: bool,
    /// `Travel_ListItem_SwiftTravel`: swift.
    pub swift: bool,
    /// State `ghosted` and `Travel_GhostedOverlay`: not usable.
    pub ghosted: bool,
}

/// What a row's destination resolved to (census of the rows of one window).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RowSource {
    /// A destination record.
    Destination,
    /// A route record (its `TravelRoute_Destination`).
    Route,
    /// A literal ref.
    Literal,
    /// A record that is neither (shown with no name).
    Unknown,
}

/// One row of the travel window.
#[derive(Debug, Clone, PartialEq)]
pub struct TravelRow {
    /// The feed entry.
    pub entry: u16,
    /// The destination record, when the entry names one (directly or through a route).
    pub destination: Option<Did>,
    /// The route record, when the entry names one.
    pub route: Option<Did>,
    /// What the entry resolved to.
    pub source: RowSource,
    /// Row text: the destination's rendered `TravelDisplayName`, or the literal.
    pub name: Option<String>,
    /// Swift (feed route first, else the destination's `TravelDestinationIsSwiftTravel`).
    pub swift: bool,
    /// The cost (the entry's `coin`).
    pub cost: u64,
    /// The minimum level (feed, else the route's `Usage_MinLevel`, else 0).
    pub min_level: u32,
    /// Discovered, requirements met, usable (feed flags).
    pub flags: TravelFlags,
    /// Territory text (the area's territory's `Area_Name`); `None` puts the row at the top
    /// level.
    pub territory: Option<String>,
    /// Area text (the area's `Area_Name`, or a literal area).
    pub area: Option<String>,
    /// Marks.
    pub marks: RowMarks,
    /// Search text: name, area, territory, swift label (§4.4 rule 2).
    pub search_text: String,
}

/// A group of rows: the top level (no header) or one territory header (§4.5, §4.10 rule
/// 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TravelGroup {
    /// The header text; `None` for the top-level rows (inserted before every header).
    pub territory: Option<String>,
    /// Row indices, in entry order.
    pub rows: Vec<usize>,
}

/// The route-status state (§4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteStatus {
    /// `Travel_NoRouteSelected`.
    NoRouteSelected,
    /// `Travel_DestinationDiscovered`.
    Discovered,
    /// `Travel_DestinationUndiscovered`.
    Undiscovered,
}

impl RouteStatus {
    /// The state's label in the layout.
    #[must_use]
    pub fn state_label(self) -> &'static str {
        match self {
            Self::NoRouteSelected => "Travel_NoRouteSelected",
            Self::Discovered => "Travel_DestinationDiscovered",
            Self::Undiscovered => "Travel_DestinationUndiscovered",
        }
    }

    /// Its text token of the `Travel` table.
    #[must_use]
    pub fn token(self) -> u32 {
        match self {
            Self::NoRouteSelected => tokens::ROUTE_STATUS[0],
            Self::Discovered => tokens::ROUTE_STATUS[1],
            Self::Undiscovered => tokens::ROUTE_STATUS[2],
        }
    }
}

/// The coin go button (§4.8, §4.10 rule 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoButton {
    /// State `normal` (otherwise `ghosted`).
    pub normal: bool,
    /// Its label token of the `Travel` table.
    pub label: u32,
    /// `Travel_CurrencyButtonLabel_NoRouteSelected` shown (no selection).
    pub no_selection_label: bool,
}

/// What the window shows for the selection (§4.7, invariants TI2, TI3).
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionDisplay {
    /// `Travel_Currency_DestinationCost` shown with this amount (cost ≠ 0).
    pub cost: Option<u64>,
    /// `Travel_Currency_DestinationCost_Free` shown (cost 0).
    pub free: bool,
    /// `Travel_Currency_MinLevel`, when shown.
    pub min_level: Option<LevelText>,
    /// The rendered minimum-level text, when shown and renderable.
    pub min_level_text: Option<String>,
    /// `Travel_RouteStatus`.
    pub route_status: RouteStatus,
    /// Territory and area texts (fields shown with a selection).
    pub territory_area: Option<(Option<String>, Option<String>)>,
    /// `Travel_SwiftTravel` shown.
    pub swift_label: bool,
    /// The coin go button.
    pub go: GoButton,
}

/// The travel window built from one `TravelShown` (`travel-windows.md` §4.10).
#[derive(Debug, Clone, PartialEq)]
pub struct TravelWindow {
    /// The rendered title (rule 1).
    pub title: Option<String>,
    /// Rows in entry order (at most [`MAX_ROWS`]).
    pub rows: Vec<TravelRow>,
    /// Groups: the top-level rows first (when any), then the headers in the order of their
    /// first row.
    pub groups: Vec<TravelGroup>,
}

impl TravelWindow {
    /// Whether `Travel_NoDestinationField` is shown (rule 7).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The rows the search keeps (§4.6, Tessera rule: case-insensitive "contains" over
    /// lower-cased texts): `true` for a kept row. An empty filter keeps every row.
    #[must_use]
    pub fn filter(&self, text: &str) -> Vec<bool> {
        let needle: String = text
            .chars()
            .take(MAX_SEARCH_CHARS)
            .collect::<String>()
            .to_lowercase();
        self.rows
            .iter()
            .map(|r| needle.is_empty() || r.search_text.to_lowercase().contains(&needle))
            .collect()
    }

    /// Whether a group's header shows under a filter result (`auto_filter_parent_with_filtered_children`).
    #[must_use]
    pub fn header_visible(&self, group: usize, kept: &[bool]) -> bool {
        self.groups.get(group).is_some_and(|g| {
            g.rows
                .iter()
                .any(|&i| kept.get(i).copied().unwrap_or(false))
        })
    }

    /// The selection display for row `selected` (`None`: no selection) (§4.7, §4.8,
    /// §4.10 rule 4).
    #[must_use]
    pub fn selection(&self, selected: Option<usize>, tp: &TravelPresenter<'_>) -> SelectionDisplay {
        let Some(r) = selected.and_then(|i| self.rows.get(i)) else {
            return SelectionDisplay {
                cost: None,
                free: false,
                min_level: None,
                min_level_text: None,
                route_status: RouteStatus::NoRouteSelected,
                territory_area: None,
                swift_label: false,
                go: GoButton {
                    normal: false,
                    label: tokens::GO_NO_SELECTION,
                    no_selection_label: true,
                },
            };
        };
        let level = if r.min_level < 2 {
            LevelText::BelowTwo
        } else {
            LevelText::Level(r.min_level)
        };
        let normal = r.flags.discovered() && r.flags.usable();
        SelectionDisplay {
            cost: (r.cost != 0).then_some(r.cost),
            free: r.cost == 0,
            min_level_text: tp.level_text(&level),
            min_level: Some(level),
            route_status: if r.flags.discovered() {
                RouteStatus::Discovered
            } else {
                RouteStatus::Undiscovered
            },
            territory_area: Some((r.territory.clone(), r.area.clone())),
            swift_label: r.swift,
            go: GoButton {
                normal,
                label: if normal {
                    tokens::GO_USABLE
                } else if !r.flags.discovered() {
                    tokens::GO_UNDISCOVERED
                } else {
                    tokens::GO_NO_SELECTION
                },
                no_selection_label: false,
            },
        }
    }
}

/// What a `TravelStarted` shows (`travel-windows.md` §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TravelPresentation {
    /// Duration 0: nothing (rule 3).
    Nothing,
    /// The loading screen, its meter filling over the duration (rule 1).
    LoadingScreen {
        /// Milliseconds.
        duration_ms: u32,
    },
    /// The end-travel box for the duration (rule 2).
    EndTravelBox {
        /// Milliseconds.
        duration_ms: u32,
    },
}

/// The presentation of `TravelStarted { destination, duration_ms, route }` (§5.2):
/// `swift` is the event's route mode when given, else the destination's (`None` for a
/// literal: slow).
#[must_use]
pub fn travel_presentation(swift: Option<bool>, duration_ms: u32) -> TravelPresentation {
    if duration_ms == 0 {
        TravelPresentation::Nothing
    } else if swift == Some(true) {
        TravelPresentation::LoadingScreen { duration_ms }
    } else {
        TravelPresentation::EndTravelBox { duration_ms }
    }
}

/// The loading-screen meter value at `elapsed_ms` of a presentation of `duration_ms`
/// (§5.2 rule 1: linear from 0 to 1).
#[must_use]
pub fn meter_value(elapsed_ms: u32, duration_ms: u32) -> f32 {
    if duration_ms == 0 {
        return 1.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let v = elapsed_ms.min(duration_ms) as f32 / duration_ms as f32;
    v.clamp(0.0, 1.0)
}

/// The travel model over a [`PresentationData`].
pub struct TravelPresenter<'d> {
    data: &'d dyn PresentationData,
    map: MapPids,
    pids: TravelPids,
    travel_table: Option<Did>,
    ui_table: Option<Did>,
}

impl std::fmt::Debug for TravelPresenter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TravelPresenter")
            .field("travel_table", &self.travel_table)
            .finish_non_exhaustive()
    }
}

impl<'d> TravelPresenter<'d> {
    /// Resolves labels and tables once.
    #[must_use]
    pub fn new(data: &'d dyn PresentationData) -> Self {
        let names = |l: &str| data.pid(l);
        Self {
            data,
            map: MapPids::resolve(&names),
            pids: TravelPids::resolve(&names),
            travel_table: data.table(TRAVEL_TABLE),
            ui_table: data.table(UI_TABLE),
        }
    }

    fn render(&self, info: &StringInfo, values: &Values) -> Option<String> {
        self.data
            .render(TextRef::Info(info), values)
            .ok()
            .flatten()
            .map(|t| t.plain_text())
    }

    fn token(&self, table: Option<Did>, token: u32, values: &Values) -> Option<String> {
        self.render(&token_info(table?, token), values)
    }

    /// A text of the `Travel` table.
    #[must_use]
    pub fn travel_text(&self, token: u32) -> Option<String> {
        self.token(self.travel_table, token, &Values::new())
    }

    /// The minimum-level text (§4.7).
    #[must_use]
    pub fn level_text(&self, level: &LevelText) -> Option<String> {
        let mut v = Values::new();
        match level {
            LevelText::BelowTwo => {
                let fixed = self.token(self.ui_table, tokens::LEVEL_BELOW_TWO, &Values::new())?;
                v.insert(tokens::VAR_MIN_LEVEL, Value::Text(fixed));
            }
            LevelText::Level(n) => {
                v.insert(tokens::VAR_MIN_LEVEL, *n);
            }
        }
        self.token(self.ui_table, tokens::MIN_LEVEL, &v)
    }

    /// The title (§4.10 rule 1): the `Travel` title text with the source's name, or with
    /// the variable rendered empty.
    #[must_use]
    pub fn title(&self, source_name: Option<&str>) -> Option<String> {
        let mut v = Values::new();
        v.insert(
            tokens::VAR_TITLE_NAME,
            Value::Text(source_name.unwrap_or("").to_owned()),
        );
        self.token(self.travel_table, tokens::TITLE, &v)
    }

    /// The coin display of a cost (`npc-windows.md` §9.2–§9.3).
    #[must_use]
    pub fn cost_display(&self, npc: &NpcPresenter<'_>, cost: u64) -> CoinDisplay {
        npc.coins(cost, CoinOptions::default())
    }

    fn set(&self, did: Did) -> Option<std::sync::Arc<tessera_formats::PropertySet>> {
        self.data.record(did).ok().flatten()
    }

    /// The destination, route and source of a feed ref (§4.10 rule 2).
    fn resolve(&self, r: &RecordRef) -> (Option<Did>, Option<Did>, RowSource) {
        let RecordRef::Record(did) = r else {
            return (None, None, RowSource::Literal);
        };
        let Some(set) = self.set(*did) else {
            return (None, None, RowSource::Unknown);
        };
        if set.has(self.map.display_name) || set.has(self.map.is_swift) {
            return (Some(*did), None, RowSource::Destination);
        }
        if let Ok(route) = TravelRoute::read(&*set, &self.map) {
            return (Some(route.destination), Some(*did), RowSource::Route);
        }
        (None, None, RowSource::Unknown)
    }

    /// The area and territory texts of an `area` ref (§4.10 rule 3): an area record's name
    /// (`Area_Name`, or `Dungeon_Name` for a dungeon, which the `area` selector family
    /// includes, `scenario-feed.md` §8.3) and its territory's name; a literal is the area
    /// text.
    #[must_use]
    pub fn area_texts(&self, area: Option<&RecordRef>) -> (Option<String>, Option<String>) {
        match area {
            None => (None, None),
            Some(RecordRef::Literal(s)) => (None, Some(s.clone())),
            Some(RecordRef::Record(d)) => {
                let Some(set) = self.set(*d) else {
                    return (None, None);
                };
                let name = |s: &tessera_formats::PropertySet| {
                    s.string_info(self.pids.area_name)
                        .or_else(|| s.string_info(self.pids.dungeon_name))
                        .and_then(|i| self.render(i, &Values::new()))
                };
                let area = name(&set);
                let territory = set
                    .did_nonnull(self.pids.area_territory)
                    .and_then(|t| self.set(t))
                    .and_then(|t| name(&t));
                (territory, area)
            }
        }
    }

    /// One row (§4.10 rules 2–4, §4.4 rules 1–4).
    #[must_use]
    pub fn row(&self, e: &TravelEntryIn, player_level: u32) -> TravelRow {
        let (destination, route, source) = self.resolve(&e.destination);
        let dest_set = destination.and_then(|d| self.set(d));
        let name = match &e.destination {
            RecordRef::Literal(s) => Some(s.clone()),
            RecordRef::Record(_) => dest_set
                .as_ref()
                .and_then(|s| s.string_info(self.map.display_name).cloned())
                .and_then(|i| self.render(&i, &Values::new())),
        };
        let record_swift = dest_set.as_ref().and_then(|s| s.boolean(self.map.is_swift));
        let swift = match e.route {
            Some(m) => m == RouteMode::Swift,
            None => record_swift.unwrap_or(false),
        };
        let min_level = match e.min_level {
            Some(l) => u32::from(l),
            None => route
                .and_then(|r| self.set(r))
                .and_then(|s| s.int(self.pids.min_level))
                .and_then(|l| u32::try_from(l).ok())
                .unwrap_or(0),
        };
        let (territory, area) = self.area_texts(e.area.as_ref());
        let marks = RowMarks {
            failed_requirements: min_level > player_level || !e.flags.met(),
            unknown: !e.flags.discovered(),
            swift,
            ghosted: !e.flags.usable(),
        };
        let mut search_text = String::new();
        for part in [name.as_deref(), area.as_deref(), territory.as_deref()]
            .into_iter()
            .flatten()
        {
            search_text.push_str(part);
        }
        if swift && let Some(label) = self.travel_text(tokens::SWIFT) {
            search_text.push_str(&label);
        }
        let search_text: String = search_text.chars().take(MAX_SEARCH_CHARS).collect();
        TravelRow {
            entry: e.entry,
            destination,
            route,
            source,
            name,
            swift,
            cost: e.coin,
            min_level,
            flags: e.flags,
            territory,
            area,
            marks,
            search_text,
        }
    }

    /// The window of one `TravelShown` (§4.10): rows in entry order (at most
    /// [`MAX_ROWS`]), rows with an empty territory at the top level first, then one header
    /// per territory text in the order of its first row.
    #[must_use]
    pub fn window(
        &self,
        source_name: Option<&str>,
        entries: &[TravelEntryIn],
        player_level: u32,
    ) -> TravelWindow {
        let rows: Vec<TravelRow> = entries
            .iter()
            .take(MAX_ROWS)
            .map(|e| self.row(e, player_level))
            .collect();
        TravelWindow {
            title: self.title(source_name),
            groups: group_rows(&rows),
            rows,
        }
    }
}

/// Groups rows by territory text (§4.5, §4.10 rule 5).
#[must_use]
pub fn group_rows(rows: &[TravelRow]) -> Vec<TravelGroup> {
    let mut top = TravelGroup {
        territory: None,
        rows: Vec::new(),
    };
    let mut headers: Vec<TravelGroup> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        match r.territory.as_deref().filter(|t| !t.is_empty()) {
            None => top.rows.push(i),
            Some(t) => match headers
                .iter_mut()
                .find(|g| g.territory.as_deref() == Some(t))
            {
                Some(g) => g.rows.push(i),
                None => headers.push(TravelGroup {
                    territory: Some(t.to_owned()),
                    rows: vec![i],
                }),
            },
        }
    }
    let mut out = Vec::with_capacity(headers.len() + 1);
    if !top.rows.is_empty() {
        out.push(top);
    }
    out.extend(headers);
    out
}
