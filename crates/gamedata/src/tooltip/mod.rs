// SPDX-License-Identifier: GPL-3.0-or-later
//! Examination tooltips: the tooltip model of an item record or scenario item (decision 0024
//! P1) and of a skill or effect record (P2), from the choice of tooltip layout to the content
//! of each row, comparison and the per-record categories of the real-data censuses.
//!
//! Specs: `docs/specs/item-tooltips.md` §3–§8, §11–§14 with its P2 revision of §6.16, §6.20
//! and §8.3; `docs/specs/skill-tooltips.md` §3–§14 (placement, pinning and the pixel layout of
//! the rows belong to `tessera-ui`).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`TooltipEngine`] | Builds [`TooltipModel`]s from a [`TooltipRequest`] and a [`Viewer`]; classifies records for the census | §3–§8, §12 |
//! | [`TooltipData`], [`InstallTooltipData`] | What the model reads, and its implementation over an install | §3, §4, §6 |
//! | [`Subject`], [`Instance`], [`Condition`], [`Viewer`], [`Equipped`], [`Purpose`] | The scenario inputs: a record or a literal, instance state, the viewer | §11 |
//! | [`TooltipFrame`], [`ListBoxInfo`], [`RowTemplate`], [`Widget`] | The tooltip element and its row templates from the layout data | §4 |
//! | [`Row`], [`RowContent`], [`Line`], [`Coin`] | Row content | §6, §7 |
//! | [`assemble`], [`AssemblyRow`], [`Assembled`] | Row-list assembly as data | §5 |
//! | [`Comparison`], [`CompareKind`] | Comparison tooltips and the slot query | §8 |
//! | [`Category`], [`RecordOutcome`], [`Census`] | Census categories | §12 |
//! | [`rules`] | The data-free rules (binding, worth, coins, slots, durations, …) | §6, §8.2, §11 |
//! | [`TooltipEngine::build_combat`], [`CombatRequest`], [`CombatSubject`], [`SkillViewer`], [`LiveEffect`] | Skill and effect tooltips | `skill-tooltips.md` §3–§8, §13 |
//! | [`TooltipEngine::effect_lines`], [`EffectLines`], [`EffectLine`], [`EffectMode`], [`LineState`], [`line_tokens`] | Effect lines of effects, skills and items | `skill-tooltips.md` §9 |
//! | [`duration`], [`TooltipEngine::duration_text`], [`TimerRow`], [`TooltipEngine::timer_text`] | Durations and timer rows | `skill-tooltips.md` §7.15, §8.4, §10 |
//! | [`numbers`] | Numbers as message values | `string-table.md` §9.4 |
//! | [`TooltipEngine::examination_icon`], [`TooltipEngine::skill_icon`], [`SkillIconSize`] | Skill and effect icons | `skill-tooltips.md` §11 |
//! | [`TooltipEngine::classify_combat`], [`CombatCategory`], [`NameState`], [`IconCategory`], [`CombatOutcome`], [`CombatCensus`] | Census of skills and effects | `skill-tooltips.md` §14 |
//!
//! # Tessera rules applied
//!
//! All row data is available when the tooltip is built (§5: nothing pending); the
//! `enchanted`, `ghosted` and `cantuse` (category row) states are never set (§6.1, §6.3,
//! §6.13); the viewer-effective item level equals the item level (§6.7); one-click requests
//! use the hover pair (§3.3); zero coins are hidden (§6.11); list boxes without
//! `max_width` use their designed width (§4.1); requirement, effect, set, allowed-slot and
//! duration texts use Tessera's own wording (§6.14–§6.17 except the effect lines, §6.18); the
//! comparison option is on by default (§8.3). Rows of §6.20 are built empty and counted, except
//! the skill-item rows, which show the item's skill (P2), and the recipe one-use and cooldown
//! rows of recipe scrolls, which follow `crafting-panels.md` §7 (P6). The skill and effect rules are listed
//! in the `skill` module's documentation (`skill-tooltips.md` ST21).
//!
//! # Hostile input
//!
//! Nothing here parses bytes; records, layouts and strings are decoded by
//! `tessera-formats`, `tessera-props` and `tessera-text`. The model bounds its own work:
//! [`layout::MAX_TEMPLATES`] entry templates per list, [`layout::MAX_TEMPLATE_DEPTH`]
//! nested template levels, [`layout::MAX_TEMPLATE_NODES`] nodes per row template (a list
//! entry naming a template being read is a cycle and is left out), base chains of
//! `MAX_BASE_CHAIN` links, [`MAX_LINES`] lines per list row, [`MAX_SET_MEMBERS`] set
//! members, one level of attached tooltips, and [`MAX_CACHED`] entries per cache.

mod assemble;
mod data;
pub mod duration;
mod effects;
mod error;
pub mod layout;
pub mod numbers;
mod rows;
pub mod rules;
mod skill;

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use tessera_formats::gameplay::combat::EffectRefPids;
use tessera_formats::gameplay::items::{DurationMap, ItemSchema};
use tessera_formats::gameplay::misc::MetadataPids;
use tessera_formats::property::PropertyEntry;
use tessera_formats::{Did, PropertySet, PropertyValue};
use tessera_text::RichText;

pub use assemble::{Assembled, AssemblyRow, MAX_LIST_DEPTH, assemble, satisfies_iv6};
pub use data::{
    InstallTooltipData, MAX_CACHED, MAX_PREFIX_MATCHES, TextRef, TextResult, TooltipData,
};
pub use effects::{
    COMBAT_ONLY_BASE_DELAY, EffectLine, EffectLines, EffectMode, LineState, MAX_LIST_EFFECTS,
    PROPERTY_MOD_EFFECT_LABEL, line_tokens,
};
pub use error::TooltipError;
pub use layout::{
    ListBoxInfo, RowTemplate, SKILL_PROVIDER_TYPES, TemplateRef, TooltipFrame, Widget,
};
pub use skill::{
    CombatCategory, CombatCensus, CombatOutcome, CombatRequest, CombatSubject, IconCategory,
    LiveEffect, NameState, SkillIconSize, SkillItemLevel, SkillViewer, TimerKind, TimerRow,
    VITAL_TYPE_HEALTH, VITAL_TYPE_POWER, skill_tokens,
};

use layout::Budget;
pub use rules::{CoinCount, CurrencyUnit};

use layout::{FrameError, ResolveError, UiPids};

/// Most lines one list row holds (requirements, effects, set lines, allowed slots).
pub const MAX_LINES: usize = 64;
/// Most set members listed.
pub const MAX_SET_MEMBERS: usize = 64;
/// Most attached tooltips of one comparison (the slot query yields at most 25 slots).
pub const MAX_COMPARED: usize = 25;
/// Most property-metadata objects read (`misc-records.md` §13.1; the data has 5,211).
pub const MAX_METADATA: usize = 65_536;

// ---------------------------------------------------------------------------------------------
// Inputs (§11)
// ---------------------------------------------------------------------------------------------

/// What is examined: an item record or a literal scenario item (§11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// An item record of the install.
    Record(Did),
    /// A literal item (`Ref::Literal`): a minimal tooltip with its text as the name.
    Literal(String),
}

/// The purpose of a tooltip (§3.3 step 2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// Hovering an item slot.
    #[default]
    Hover,
    /// A pinned copy of the hover tooltip (§10).
    Pinned,
    /// A one-click tooltip (Tessera rule: the hover pair).
    OneClick,
    /// The equipped side of a comparison (§8).
    Comparison,
}

/// A reported item condition (`ItemCondition`, §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Condition {
    /// Current structure points.
    pub current: u32,
    /// Maximum structure points.
    pub max: u32,
}

/// The state of one item instance as a scenario reports it (§11).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Instance {
    /// `ItemAdded`/`ItemQuantity` quantity; `None` for a record (reads 1).
    pub quantity: Option<i64>,
    /// `ItemCondition`, replacing the record's structure points.
    pub condition: Option<Condition>,
    /// Whether the item is in the `Equipment` container (§8.1 step 2).
    pub equipped: bool,
    /// Instance properties (`ItemProperty`, decision 0025 D5), read before the record's.
    /// A later entry with the same PID replaces an earlier one.
    pub properties: Vec<PropertyEntry>,
}

/// One equipped item of the viewer (§8.2, §6.17).
#[derive(Debug, Clone, PartialEq)]
pub struct Equipped {
    /// The slot bit it is equipped in (`Inventory_CompatibleSlot` bit values).
    pub slot: u32,
    /// The item record.
    pub did: Did,
    /// Its instance state.
    pub instance: Instance,
}

/// The local character looking at the tooltip (§2 "Viewer", §11).
#[derive(Debug, Clone, PartialEq)]
pub struct Viewer {
    /// Class key (`Usage_ClassType` mapper), when known.
    pub class: Option<u32>,
    /// Race key (`Usage_RequiredRaceEntry` mapper), when known.
    pub race: Option<u32>,
    /// Level, when known.
    pub level: Option<i32>,
    /// The option `GOP_EnableCrossCompareItems` (Tessera rule: on by default; settings key
    /// `ui.compare_equipped_items`).
    pub compare_equipped: bool,
    /// Equipped items.
    pub equipped: Vec<Equipped>,
}

impl Default for Viewer {
    fn default() -> Self {
        Self {
            class: None,
            race: None,
            level: None,
            compare_equipped: true,
            equipped: Vec::new(),
        }
    }
}

/// A tooltip request (§3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct TooltipRequest {
    /// What is examined.
    pub subject: Subject,
    /// Its instance state.
    pub instance: Instance,
    /// The purpose.
    pub purpose: Purpose,
    /// The hovered element's override pair (§3.3 step 1), when both values are non-zero.
    pub override_pair: Option<TemplateRef>,
    /// The hovered element's `UI_Examination_Tooltip_Override_Compare_Type` (§8.1).
    pub override_compare_kind: Option<u32>,
}

impl TooltipRequest {
    /// The hover tooltip of a record, as a record (no instance state).
    #[must_use]
    pub fn hover(did: Did) -> Self {
        Self {
            subject: Subject::Record(did),
            instance: Instance::default(),
            purpose: Purpose::Hover,
            override_pair: None,
            override_compare_kind: None,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Outputs
// ---------------------------------------------------------------------------------------------

/// One line of text with its colour state.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The text (markup already interpreted into styled spans).
    pub text: RichText,
    /// The state the line is in (`cantuse`, `gameset_active`, `gameset_inactive`, …).
    pub state: Option<String>,
    /// The colour of that state from the row template, when it has one.
    pub colour: Option<u32>,
}

impl Line {
    /// A line without a state.
    #[must_use]
    pub fn plain(text: RichText) -> Self {
        Self {
            text,
            state: None,
            colour: None,
        }
    }
}

/// One coin of the worth row (§6.11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coin {
    /// The currency record.
    pub currency: Did,
    /// Its index in the currency directory.
    pub index: usize,
    /// The count (above 0).
    pub count: i64,
    /// The count text.
    pub text: String,
}

/// What a row shows (§6).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RowContent {
    /// Nothing: removed by §5 step 1.
    Empty,
    /// A separator (§4.4).
    Separator,
    /// One text.
    Text(RichText),
    /// A DID value shown as an image (§6.4 step 6).
    Image(Did),
    /// The composed icon of an item (§6.3).
    Icon {
        /// The item whose icon is composed (`icon-layers.md` §3).
        item: Did,
        /// The `ghosted` overlay (Tessera rule: never in P1).
        ghosted: bool,
    },
    /// The worth row: label and coins (§6.11).
    Money {
        /// The "worth" label.
        label: Option<RichText>,
        /// Coins in currency order.
        coins: Vec<Coin>,
    },
    /// A list of lines with an optional header (requirements, effects, sets, slots).
    Lines {
        /// Header line.
        header: Option<Line>,
        /// The lines.
        lines: Vec<Line>,
    },
    /// A two-column row (§4.3).
    Pair {
        /// Left child.
        left: Option<Box<Row>>,
        /// Right child.
        right: Option<Box<Row>>,
    },
    /// A nested list of rows (§4.3).
    List(Vec<Row>),
}

/// A filled row.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Its template's label.
    pub label: Option<String>,
    /// The behaviour.
    pub widget: Widget,
    /// Designed size.
    pub size: (u32, u32),
    /// `UI_Examination_Tooltip_Stretch_Wide`.
    pub stretch_wide: bool,
    /// A list laid out sideways (`UICore_ListBox_horizontal`, the name-and-icon row §4.3).
    pub horizontal: bool,
    /// What it shows.
    pub content: RowContent,
    /// The state the row is in (`broken`, the name's quality state, …).
    pub state: Option<String>,
    /// The text colour (state colour, colour-table colour or the template's colour).
    pub colour: Option<u32>,
}

impl Row {
    /// Whether the row shows nothing (§5 step 1).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match &self.content {
            RowContent::Empty => true,
            RowContent::Separator
            | RowContent::Text(_)
            | RowContent::Image(_)
            | RowContent::Icon { .. }
            | RowContent::Money { .. } => false,
            RowContent::Lines { lines, .. } => lines.is_empty(),
            RowContent::Pair { left, right } => {
                left.as_ref().is_none_or(|r| r.is_empty())
                    && right.as_ref().is_none_or(|r| r.is_empty())
            }
            RowContent::List(rows) => rows.iter().all(Row::is_empty),
        }
    }

    /// The row as §5 sees it. Model-level sizes: a non-empty row has its designed size
    /// (at least 1 × 1); a pair is as wide as its non-empty sides; the UI measures text.
    #[must_use]
    pub fn assembly_row(&self) -> AssemblyRow {
        let (w, h) = (self.size.0.max(1), self.size.1.max(1));
        match &self.content {
            RowContent::Separator => AssemblyRow::separator(self.size.0, self.size.1),
            // A horizontal list grows sideways to fit its entries (§4.3): as wide as the sum
            // of its non-empty entries, as high as the highest.
            RowContent::List(rows) if self.horizontal => {
                let (width, height) = rows
                    .iter()
                    .filter(|r| !r.is_empty())
                    .map(Row::assembly_row)
                    .fold((0u32, 0u32), |(w, h), a| {
                        (w.saturating_add(a.width), h.max(a.height))
                    });
                AssemblyRow {
                    width,
                    height,
                    stretch_wide: self.stretch_wide,
                    ..AssemblyRow::default()
                }
            }
            RowContent::List(rows) => AssemblyRow {
                width: w,
                height: h,
                stretch_wide: self.stretch_wide,
                list: Some(rows.iter().map(Row::assembly_row).collect()),
                ..AssemblyRow::default()
            },
            RowContent::Pair { left, right } => {
                let side = |r: &Option<Box<Row>>| {
                    r.as_ref()
                        .filter(|r| !r.is_empty())
                        .map_or((0, 0), |r| (r.size.0.max(1), r.size.1.max(1)))
                };
                let (l, r) = (side(left), side(right));
                AssemblyRow {
                    width: l.0.saturating_add(r.0),
                    height: l.1.max(r.1),
                    stretch_wide: self.stretch_wide,
                    ..AssemblyRow::default()
                }
            }
            _ if self.is_empty() => AssemblyRow {
                stretch_wide: self.stretch_wide,
                ..AssemblyRow::default()
            },
            _ => AssemblyRow {
                width: w,
                height: h,
                stretch_wide: self.stretch_wide,
                ..AssemblyRow::default()
            },
        }
    }

    /// The row's text without styles, lines joined by newlines (tests and previews).
    #[must_use]
    pub fn plain_text(&self) -> String {
        match &self.content {
            RowContent::Text(t) => t.plain_text(),
            RowContent::Money { label, coins } => {
                let mut s = label.as_ref().map(RichText::plain_text).unwrap_or_default();
                for c in coins {
                    if !s.is_empty() {
                        s.push(' ');
                    }
                    s.push_str(&c.text);
                }
                s
            }
            RowContent::Lines { header, lines } => header
                .iter()
                .chain(lines)
                .map(|l| l.text.plain_text())
                .collect::<Vec<_>>()
                .join("\n"),
            RowContent::Pair { left, right } => [left, right]
                .into_iter()
                .flatten()
                .map(|r| r.plain_text())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join("  "),
            RowContent::List(rows) => rows
                .iter()
                .map(Row::plain_text)
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join("  "),
            RowContent::Empty
            | RowContent::Separator
            | RowContent::Image(_)
            | RowContent::Icon { .. } => String::new(),
        }
    }
}

/// A row rule that failed on malformed data (§12 `RowFailure`): the row is left empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowFailure {
    /// The template's label (or position when unlabelled).
    pub template: String,
    /// What failed.
    pub reason: String,
}

/// The comparison kind of a hover tooltip (§8.1, §8.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompareKind {
    /// No kind, or kind 0.
    #[default]
    None,
    /// `Equipment`: compared with the viewer's equipped items (§8.1).
    Equipment,
    /// `Recipe`: the result item's tooltip is attached.
    Recipe,
    /// `SkillGrantingItem`: the hover tooltip of the item record's `Item_GrantedSkill` is
    /// attached (`skill-tooltips.md` §12.4).
    SkillGrantingItem,
    /// `TraitItem`: the trait's hover tooltip would be attached (`skill-tooltips.md` §12.4);
    /// trait tooltips are P6, so nothing is attached yet.
    TraitItem,
    /// Any other kind (the mount comparison is P5): its key.
    Other(u32),
}

/// The tooltips attached to a hover tooltip (§8).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Comparison {
    /// The kind read from the request or the item.
    pub kind: CompareKind,
    /// Attached tooltips in the order of §8.2 (for equipment: with their slot bits).
    pub tooltips: Vec<(u32, TooltipModel)>,
}

/// An item examination tooltip (§3–§8): the frame, the filled rows in template order, the
/// assembly of §5 and the attached comparison tooltips.
#[derive(Debug, Clone, PartialEq)]
pub struct TooltipModel {
    /// What is examined.
    pub subject: Subject,
    /// The purpose it was built for.
    pub purpose: Purpose,
    /// The tooltip element; `None` for a literal item (§11).
    pub frame: Option<Arc<TooltipFrame>>,
    /// Rows in template order, filled.
    pub rows: Vec<Row>,
    /// The assembly of §5 over `rows`.
    pub assembly: Assembled,
    /// Attached tooltips (§8).
    pub comparison: Comparison,
    /// Row rules that failed.
    pub failures: Vec<RowFailure>,
    /// Timer rows (`skill-tooltips.md` §7.15, §8.4): their text changes while shown.
    pub timers: Vec<TimerRow>,
    /// Texts the record names that could not be found (lookup misses; reported, not
    /// failures, `skill-tooltips.md` §14.1). Counted by the skill and effect rows.
    pub lookup_misses: u32,
}

impl TooltipModel {
    /// The rows that survive assembly, in order.
    pub fn shown_rows(&self) -> impl Iterator<Item = &Row> + '_ {
        self.assembly.kept.iter().filter_map(|&i| self.rows.get(i))
    }

    /// Whether the name row (or the literal name) is shown.
    #[must_use]
    pub fn has_name(&self) -> bool {
        fn named(r: &Row) -> bool {
            match &r.content {
                RowContent::Text(_) => r.widget == Widget::Name,
                RowContent::List(rows) => rows.iter().any(named),
                RowContent::Pair { left, right } => {
                    [left, right].into_iter().flatten().any(|r| named(r))
                }
                _ => false,
            }
        }
        self.shown_rows().any(named)
    }

    /// The shown rows as plain text lines (tests and text previews).
    #[must_use]
    pub fn text_lines(&self) -> Vec<String> {
        self.shown_rows()
            .map(|r| match r.content {
                RowContent::Separator => "—".to_owned(),
                _ => r.plain_text(),
            })
            .filter(|t| !t.is_empty())
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Census (§12)
// ---------------------------------------------------------------------------------------------

/// The per-record categories of the real-data test (§12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    /// The tooltip element was built (§12 OPEN: a name row is not required; tooltips
    /// without one are counted in [`Census::unnamed`]).
    Rendered,
    /// No hover pair: the original shows none.
    NoTooltip,
    /// The named layout is missing or fails to decode (or lacks the §4.1 structure).
    LayoutUnreadable,
    /// The element is not a top-level element of the layout.
    ElementMissing,
    /// A row's rule failed (the tooltip still renders and also counts as `Rendered`).
    RowFailure,
}

/// The outcome of one record (§12).
#[derive(Debug, Clone, PartialEq)]
pub struct RecordOutcome {
    /// The category (`Rendered`, `NoTooltip`, `LayoutUnreadable`, `ElementMissing`), or the
    /// error that escaped the categories.
    pub category: Result<Category, TooltipError>,
    /// Row failures (non-empty means the record also counts as `RowFailure`).
    pub row_failures: Vec<RowFailure>,
    /// Labels of the row templates that were non-empty.
    pub non_empty: Vec<String>,
    /// Rows left empty by §6.20 (widget types without a P1 rule).
    pub deferred: usize,
    /// Whether a name row is shown; a `Rendered` record without one is counted in
    /// [`Census::unnamed`].
    pub named: bool,
}

/// Counts over many records (§12 report).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Census {
    /// Records per category (`RowFailure` counted in addition to `Rendered`).
    pub categories: BTreeMap<Category, u64>,
    /// Errors that escaped the categories, with a few examples.
    pub escaped: u64,
    /// Examples of escaped errors and failures (bounded).
    pub examples: Vec<String>,
    /// Per row-template label: tooltips with it non-empty.
    pub non_empty: BTreeMap<String, u64>,
    /// Rows left empty by §6.20.
    pub deferred: u64,
    /// Rendered tooltips without a name row.
    pub unnamed: u64,
}

/// Most examples a [`Census`] keeps.
pub const MAX_CENSUS_EXAMPLES: usize = 32;

impl Census {
    /// Adds one record's outcome.
    pub fn add(&mut self, did: Did, o: &RecordOutcome) {
        match &o.category {
            Ok(c) => *self.categories.entry(*c).or_default() += 1,
            Err(e) => {
                self.escaped += 1;
                self.example(format!("{did}: {e}"));
            }
        }
        if !o.row_failures.is_empty() {
            *self.categories.entry(Category::RowFailure).or_default() += 1;
            if let Some(f) = o.row_failures.first() {
                self.example(format!("{did}: row {}: {}", f.template, f.reason));
            }
        }
        if o.category == Ok(Category::Rendered) && !o.named {
            self.unnamed += 1;
        }
        for l in &o.non_empty {
            *self.non_empty.entry(l.clone()).or_default() += 1;
        }
        self.deferred += o.deferred as u64;
    }

    /// Adds another census.
    pub fn merge(&mut self, other: Census) {
        for (k, v) in other.categories {
            *self.categories.entry(k).or_default() += v;
        }
        self.escaped += other.escaped;
        for e in other.examples {
            self.example(e);
        }
        for (k, v) in other.non_empty {
            *self.non_empty.entry(k).or_default() += v;
        }
        self.deferred += other.deferred;
        self.unnamed += other.unnamed;
    }

    /// The count of one category.
    #[must_use]
    pub fn count(&self, c: Category) -> u64 {
        self.categories.get(&c).copied().unwrap_or(0)
    }

    fn example(&mut self, s: String) {
        if self.examples.len() < MAX_CENSUS_EXAMPLES {
            self.examples.push(s);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The engine
// ---------------------------------------------------------------------------------------------

macro_rules! pids {
    ($(#[$m:meta])* $name:ident { $($field:ident = $label:literal,)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, Default)]
        struct $name {
            $($field: Option<u32>,)*
        }

        impl $name {
            fn resolve(data: &dyn TooltipData) -> Self {
                Self { $($field: data.pid($label),)* }
            }
        }
    };
}

pids! {
    /// Item and record properties the rows read by name (§3, §6, §8).
    Pids {
        hover_layout = "UI_Examination_Tooltip_0Click_Layout",
        hover_element = "UI_Examination_Tooltip_0Click_Element",
        compare_layout = "UI_Examination_Tooltip_Compare_Layout",
        compare_element = "UI_Examination_Tooltip_Compare_Element",
        compare_type = "UI_Examination_Tooltip_Compare_Type",
        bound_to_name = "UI_Examination_Tooltip_BoundToCharacterName",
        bound_to_id = "Inventory_BoundToID",
        bind_on_raid = "Inventory_BindOnRaid",
        bind_to_account = "Inventory_BindToAccount",
        bind_on_equip = "Inventory_BindOnEquip",
        bind_on_acquire = "Inventory_BindOnAcquire",
        unique = "Inventory_Unique",
        indestructible = "Inventory_Indestructible",
        compatible_slot = "Inventory_CompatibleSlot",
        name = "Name",
        quality = "Item_Quality",
        level = "Item_Level",
        level_bonus_pending = "Item_Level_Bonus_Pending",
        armour = "Item_Armor_Value",
        cur_points = "Item_CurStructurePoints",
        max_points = "Item_MaxStructurePoints",
        wear_state = "Item_WearState",
        value = "Item_Value",
        force_not_sellable = "Item_ForceNotSellable",
        quest_item = "Usage_IsQuestItem",
        barter = "Barter_IsBarterItem",
        legendary = "ItemAdvancement_Item",
        store_item = "Item_IsPurchasedWebStoreItem",
        equipment_category = "Item_EquipmentCategory",
        damage_type = "Combat_DamageType",
        class_type = "Usage_ClassType",
        race_entry = "Usage_RequiredRaceEntry",
        quest = "Quest_DataID",
        quest_name = "Quest_Name",
        currency_list = "Currency_CurrencyList",
        currency_value = "Currency_Value",
        recipe = "RecipeItem_Recipe",
        recipe_one_time = "CraftRecipe_OneTimeRecipe",
        recipe_cooldown = "CraftRecipe_CooldownDuration",
        recipe_result = "CraftRecipe_ResultItem",
        recipe_result_quantity = "CraftRecipe_ResultItemQuantity",
        effect_name = "Effect_Name",
        item_property_set = "Item_PropertySet",
    }
}

pids! {
    /// Skill, effect and related properties the skill and effect rows read
    /// (`skill-tooltips.md` §6–§9).
    CombatTipPids {
        skill_name = "Skill_Name",
        skill_icon = "Skill_Icon",
        skill_large_icon = "Skill_LargeIcon",
        skill_small_icon = "Skill_SmallIcon",
        skill_max_range = "Skill_MaxRange",
        skill_max_targets = "Skill_AreaEffectMaxTargets",
        skill_induction = "Skill_InductionAction",
        skill_channelling = "Skill_Toggle_ChannelingState",
        skill_channelling_override = "Skill_Toggle_ChannelingDurationOverride",
        skill_toggle_hook = "Skill_Toggle_Hook_Number",
        skill_ignores_reset = "Skill_IgnoresResetTime",
        skill_immediate = "Skill_Immediate",
        skill_vital_costs = "Skill_VitalCostList",
        skill_toggle_costs = "Skill_Toggle_VitalCostPerSecondList",
        vital_type = "Skill_Vital_Type",
        vital_points = "Skill_Vital_Points",
        vital_points_progression = "Skill_Vital_Points_Progression",
        vital_percent = "Skill_Vital_Percent",
        vital_consumes_all = "Skill_Vital_Consumes_All",
        skill_required_trait = "Skill_RequiredTrait",
        trait_name = "Trait_Name",
        trait_nature = "Trait_Nature",
        skill_recovery = "Skill_SkillRecoveryTime",
        skill_user_effects = "Skill_UserEffectList",
        skill_critical_effects = "Skill_CriticalEffectList",
        hook_list = "Skill_AttackHookList",
        hook_qualifier = "Skill_AttackHook_DamageQualifier",
        hook_target_effects = "Skill_AttackHook_TargetEffectList",
        hook_positional_effects = "Skill_AttackHook_PositionalTargetEffectList",
        hook_critical_effects = "Skill_AttackHook_CriticalTargetEffectList",
        hook_super_critical_effects = "Skill_AttackHook_SuperCriticalTargetEffectList",
        usage_skill = "Usage_SkillToExecute",
        usage_skill_level = "Usage_SkillLevel",
        skill_level_calculator = "SkillLevelCalculator",
        granted_skill = "Item_GrantedSkill",
        induction_duration = "Induction_Duration",
        induction_tooltip_override = "Induction_SkillTooltipOverride",
        channelling_duration = "Channeling_Duration",
        effect_name = "Effect_Name",
        effect_icon = "Effect_Icon",
        effect_harmful = "Effect_Harmful",
        effect_definition_description = "Effect_Definition_Description",
        effect_applied_description = "Effect_Applied_Description",
        effect_procedural = "Effect_Display_Procedurally_Generated_Examination_Information",
        effect_suppress_probability = "Effect_SuppressApplicationProbabilityExamination",
        effect_probability = "Effect_ConstantApplicationProbability",
        effect_combat_only = "Effect_Duration_CombatOnly",
        effect_interval = "Effect_Duration_ConstantInterval",
        effect_pulse_count = "Effect_Duration_ConstantPulseCount",
        effect_permanent = "Effect_Duration_Permanent",
        effect_interval_progression = "Effect_Duration_ProgressionInterval",
        effect_pulse_progression = "Effect_Duration_ProgressionPulseCount",
        effect_resist_category = "Effect_Resist_Category",
    }
}

type FrameSlot = Result<Arc<TooltipFrame>, FrameError>;
type TemplateSlot = Result<Arc<RowTemplate>, String>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Builds item examination tooltips over a [`TooltipData`] (§3–§8). Layout reads are cached
/// per (layout, element), at most [`MAX_CACHED`] of each (the cache is cleared when full);
/// the engine is `Sync` when the data is, so a census can run on several threads.
pub struct TooltipEngine<D: TooltipData> {
    data: D,
    ui: UiPids,
    p: Pids,
    c: CombatTipPids,
    meta_pids: MetadataPids,
    effect_ref_pids: EffectRefPids,
    metadata: OnceLock<HashMap<u32, effects::Meta>>,
    schema: ItemSchema,
    frames: Mutex<HashMap<TemplateRef, FrameSlot>>,
    templates: Mutex<HashMap<TemplateRef, TemplateSlot>>,
    colours: OnceLock<Vec<(u32, Option<String>, u32)>>,
    currencies: OnceLock<Vec<CurrencyUnit>>,
    cooldowns: OnceLock<Option<DurationMap>>,
    decays: OnceLock<Option<DurationMap>>,
    slot_exclusions: OnceLock<Vec<u32>>,
    passive_natures: OnceLock<skill::PassiveNatures>,
}

impl<D: TooltipData> std::fmt::Debug for TooltipEngine<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TooltipEngine")
            .field("frames", &lock(&self.frames).len())
            .field("templates", &lock(&self.templates).len())
            .finish_non_exhaustive()
    }
}

/// Names a [`TooltipData`] as a [`tessera_formats::PropertyNames`] for the item schema.
struct Names<'a>(&'a dyn TooltipData);

impl tessera_formats::PropertyNames for Names<'_> {
    fn pid_of(&self, name: &str) -> Option<u32> {
        self.0.pid(name)
    }
}

/// What kind of record a build examines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Item,
    Skill,
    Effect,
}

/// The skill and effect context of one build (`skill-tooltips.md` §13): the viewer's combat
/// state, a live effect, and what the rows report back (timers, lookup misses).
#[derive(Debug, Default)]
pub(crate) struct Ctx {
    pub(crate) viewer: SkillViewer,
    pub(crate) live: Option<LiveEffect>,
    pub(crate) misses: Cell<u32>,
    pub(crate) timer: RefCell<Option<TimerRow>>,
}

impl Ctx {
    pub(crate) fn miss(&self, n: u32) {
        self.misses.set(self.misses.get().saturating_add(n));
    }
}

/// The examined object of one build: the record's properties overlaid by the instance's.
pub(crate) struct Examined<'a> {
    pub(crate) did: Option<Did>,
    pub(crate) set: Cow<'a, PropertySet>,
    pub(crate) instance: &'a Instance,
    pub(crate) kind: Kind,
    pub(crate) ctx: &'a Ctx,
    /// The row level a skill-item effect list writes as `UI_Examination_SkillEffectLevel`
    /// (`skill-tooltips.md` §9.8, §12.3); `None` outside skill-item rows.
    pub(crate) row_level: Option<i32>,
}

impl<D: TooltipData> TooltipEngine<D> {
    /// An engine over `data`; property IDs are looked up by name once here.
    pub fn new(data: D) -> Self {
        let ui = UiPids::resolve(&data);
        let p = Pids::resolve(&data);
        let c = CombatTipPids::resolve(&data);
        let schema = ItemSchema::resolve(&Names(&data));
        let meta_pids = MetadataPids::resolve(&Names(&data));
        let effect_ref_pids = EffectRefPids::resolve(&Names(&data));
        Self {
            data,
            ui,
            p,
            c,
            meta_pids,
            effect_ref_pids,
            metadata: OnceLock::new(),
            schema,
            frames: Mutex::new(HashMap::new()),
            templates: Mutex::new(HashMap::new()),
            colours: OnceLock::new(),
            currencies: OnceLock::new(),
            cooldowns: OnceLock::new(),
            decays: OnceLock::new(),
            slot_exclusions: OnceLock::new(),
            passive_natures: OnceLock::new(),
        }
    }

    /// The data the engine reads.
    pub fn data(&self) -> &D {
        &self.data
    }

    /// The tooltip element `at` (§4.1), read once.
    ///
    /// # Errors
    ///
    /// [`TooltipError::LayoutUnreadable`] or [`TooltipError::ElementMissing`].
    pub fn frame(&self, at: TemplateRef) -> Result<Arc<TooltipFrame>, TooltipError> {
        if let Some(f) = lock(&self.frames).get(&at) {
            return f.clone().map_err(|e| frame_error(at, e));
        }
        let f = layout::read_frame(&self.data, &self.ui, at).map(Arc::new);
        let mut cache = lock(&self.frames);
        if cache.len() >= MAX_CACHED {
            cache.clear();
        }
        cache.insert(at, f.clone());
        drop(cache);
        f.map_err(|e| frame_error(at, e))
    }

    /// The row template `at` (§4.2), read once.
    ///
    /// # Errors
    ///
    /// The reason it could not be resolved.
    pub fn template(&self, at: TemplateRef) -> Result<Arc<RowTemplate>, String> {
        if let Some(t) = lock(&self.templates).get(&at) {
            return t.clone();
        }
        let t = layout::read_template(&self.data, &self.ui, at, 0, &mut Budget::new())
            .map(Arc::new)
            .map_err(|e| e.to_string());
        let mut cache = lock(&self.templates);
        if cache.len() >= MAX_CACHED {
            cache.clear();
        }
        cache.insert(at, t.clone());
        drop(cache);
        t
    }

    /// The tooltip pair of a property set for a purpose (§3.3 step 2); `None` when either
    /// value is missing or zero (step 3).
    #[must_use]
    pub fn pair(&self, set: &PropertySet, purpose: Purpose) -> Option<TemplateRef> {
        let (l, e) = match purpose {
            Purpose::Comparison => (self.p.compare_layout, self.p.compare_element),
            Purpose::Hover | Purpose::Pinned | Purpose::OneClick => {
                (self.p.hover_layout, self.p.hover_element)
            }
        };
        let layout = match set.get(l?)? {
            PropertyValue::DidRef(d) if !d.is_null() => *d,
            _ => return None,
        };
        let element = match set.get(e?)? {
            PropertyValue::Enum(k) if *k != 0 => *k,
            PropertyValue::Int(k) if *k != 0 => u32::try_from(*k).ok()?,
            _ => return None,
        };
        Some(TemplateRef { layout, element })
    }

    /// Builds the tooltip of `request` for `viewer` (§3–§8).
    ///
    /// # Errors
    ///
    /// [`TooltipError::NoTooltip`] when the item names no tooltip, the layout and element
    /// errors of §12, and the record errors.
    pub fn build(
        &self,
        request: &TooltipRequest,
        viewer: &Viewer,
    ) -> Result<TooltipModel, TooltipError> {
        self.build_at(request, viewer, &SkillViewer::default(), 0)
    }

    /// [`Self::build`] with the viewer's combat state (`skill-tooltips.md` §13): the maximum
    /// vitals and running cooldowns the skill-item rows (§6.16) and an attached
    /// `SkillGrantingItem` skill tooltip (§12.4) read. Its `level` is used when set, else
    /// `viewer.level`.
    ///
    /// # Errors
    ///
    /// As [`Self::build`].
    pub fn build_with_combat(
        &self,
        request: &TooltipRequest,
        viewer: &Viewer,
        combat: &SkillViewer,
    ) -> Result<TooltipModel, TooltipError> {
        self.build_at(request, viewer, combat, 0)
    }

    fn build_at(
        &self,
        request: &TooltipRequest,
        viewer: &Viewer,
        combat: &SkillViewer,
        depth: usize,
    ) -> Result<TooltipModel, TooltipError> {
        let did = match &request.subject {
            Subject::Literal(text) => return Ok(self.literal(text, request.purpose)),
            Subject::Record(did) => *did,
        };
        let record = match self.data.record(did) {
            Ok(Some(r)) => r,
            Ok(None) => return Err(TooltipError::RecordMissing(did)),
            Err(reason) => return Err(TooltipError::RecordUnreadable { did, reason }),
        };
        let set = overlay(&record, &request.instance.properties);
        let at = match request.override_pair {
            Some(p) if !p.layout.is_null() && p.element != 0 => p,
            _ => self
                .pair(&set, request.purpose)
                .ok_or(TooltipError::NoTooltip)?,
        };
        let frame = self.frame(at)?;
        let ctx = Ctx {
            viewer: SkillViewer {
                level: combat.level.or(viewer.level),
                ..combat.clone()
            },
            ..Ctx::default()
        };
        let ex = Examined {
            did: Some(did),
            set,
            instance: &request.instance,
            kind: Kind::Item,
            ctx: &ctx,
            row_level: None,
        };
        let (rows, failures, timers) = self.fill_rows(&frame, &ex, viewer);
        let assembly = assemble(
            &rows.iter().map(Row::assembly_row).collect::<Vec<_>>(),
            frame.list.design_width,
            Some(frame.list.width_limit()),
        );
        let comparison = if depth == 0 {
            self.compare(request, &ex, viewer, depth)
        } else {
            Comparison::default()
        };
        Ok(TooltipModel {
            subject: request.subject.clone(),
            purpose: request.purpose,
            frame: Some(frame),
            rows,
            assembly,
            comparison,
            failures,
            timers,
            lookup_misses: ctx.misses.get(),
        })
    }

    /// Fills every row template of `frame` (§4.2) for `ex`.
    pub(crate) fn fill_rows(
        &self,
        frame: &TooltipFrame,
        ex: &Examined<'_>,
        viewer: &Viewer,
    ) -> (Vec<Row>, Vec<RowFailure>, Vec<TimerRow>) {
        let mut failures = Vec::new();
        let mut timers = Vec::new();
        let mut rows = Vec::with_capacity(frame.list.templates.len());
        for (i, t) in frame.list.templates.iter().enumerate() {
            let Some(t) = t else {
                failures.push(RowFailure {
                    template: format!("#{i}"),
                    reason: "entry template without layout or element".into(),
                });
                continue;
            };
            match self.template(*t) {
                Ok(t) => {
                    // Parts of the template that could not be read (§12 RowFailure).
                    for (label, reason) in t.all_problems() {
                        failures.push(RowFailure {
                            template: label.unwrap_or_else(|| format!("{:#010x}", t.at.element)),
                            reason,
                        });
                    }
                    rows.push(self.fill(&t, ex, viewer, &mut failures));
                    if let Some(mut timer) = ex.ctx.timer.borrow_mut().take() {
                        timer.row = rows.len() - 1;
                        timers.push(timer);
                    }
                }
                Err(reason) => failures.push(RowFailure {
                    template: format!("{:#010x}", t.element),
                    reason,
                }),
            }
        }
        (rows, failures, timers)
    }

    /// The minimal tooltip of a literal item (§11, Tessera rule).
    fn literal(&self, text: &str, purpose: Purpose) -> TooltipModel {
        let colour = self.name_colour(rules::OBJECT_COLOR_STATE);
        let row = Row {
            label: None,
            widget: Widget::Name,
            size: (180, 22),
            stretch_wide: false,
            horizontal: false,
            content: if text.trim().is_empty() {
                RowContent::Empty
            } else {
                RowContent::Text(RichText::plain(text))
            },
            state: Some(rules::OBJECT_COLOR_STATE.to_owned()),
            colour,
        };
        let assembly = assemble(&[row.assembly_row()], 229, Some(229));
        TooltipModel {
            subject: Subject::Literal(text.to_owned()),
            purpose,
            frame: None,
            rows: vec![row],
            assembly,
            comparison: Comparison::default(),
            failures: Vec::new(),
            timers: Vec::new(),
            lookup_misses: 0,
        }
    }

    /// The comparison of §8 for a hover tooltip.
    fn compare(
        &self,
        request: &TooltipRequest,
        ex: &Examined<'_>,
        viewer: &Viewer,
        depth: usize,
    ) -> Comparison {
        if request.purpose != Purpose::Hover {
            return Comparison::default();
        }
        let key = request.override_compare_kind.or_else(|| {
            ex.set.get(self.p.compare_type?).and_then(|v| match v {
                PropertyValue::Enum(k) => Some(*k),
                _ => None,
            })
        });
        let kind = match key {
            None | Some(0) => CompareKind::None,
            Some(k) => {
                let pid = self.p.compare_type;
                let named = |n: &str| pid.and_then(|p| self.data.enum_key(p, n)) == Some(k);
                if named("Equipment") {
                    CompareKind::Equipment
                } else if named("Recipe") {
                    CompareKind::Recipe
                } else if named("SkillGrantingItem") {
                    CompareKind::SkillGrantingItem
                } else if named("TraitItem") {
                    CompareKind::TraitItem
                } else {
                    CompareKind::Other(k)
                }
            }
        };
        let mut out = Comparison {
            kind,
            tooltips: Vec::new(),
        };
        match kind {
            CompareKind::Equipment => {
                if !viewer.compare_equipped || request.instance.equipped {
                    return out;
                }
                let s = self.slot_bits(&ex.set);
                // Each equipped item is added once (§8.2 step 3): items, not records, so two
                // copies of one record in a pair of slots give two tooltips.
                let mut added: Vec<usize> = Vec::new();
                for slot in rules::comparison_slots(s) {
                    let Some((i, e)) = viewer
                        .equipped
                        .iter()
                        .enumerate()
                        .find(|(_, e)| e.slot == slot)
                    else {
                        continue;
                    };
                    if added.contains(&i) {
                        continue;
                    }
                    added.push(i);
                    let req = TooltipRequest {
                        subject: Subject::Record(e.did),
                        instance: Instance {
                            equipped: true,
                            ..e.instance.clone()
                        },
                        purpose: Purpose::Comparison,
                        override_pair: None,
                        override_compare_kind: None,
                    };
                    if let Ok(m) = self.build_at(&req, viewer, &ex.ctx.viewer, depth + 1) {
                        out.tooltips.push((slot, m));
                    }
                    if out.tooltips.len() >= MAX_COMPARED {
                        break;
                    }
                }
            }
            CompareKind::Recipe => {
                // The result item and its quantity (§8.3; `economy.md` §5.1: default 1).
                let result = (|| {
                    let recipe = did_value(ex.set.get(self.p.recipe?)?)?;
                    let r = self.data.record(recipe).ok()??;
                    let item = did_value(r.get(self.p.recipe_result?)?)?;
                    let quantity = match self.p.recipe_result_quantity.and_then(|p| r.get(p)) {
                        Some(PropertyValue::Int(q)) if *q > 0 => i64::from(*q),
                        _ => 1,
                    };
                    Some((item, quantity))
                })();
                if let Some((result, quantity)) = result {
                    let req = TooltipRequest {
                        instance: Instance {
                            quantity: Some(quantity),
                            ..Instance::default()
                        },
                        ..TooltipRequest::hover(result)
                    };
                    if let Ok(m) = self.build_at(&req, viewer, &ex.ctx.viewer, depth + 1) {
                        out.tooltips.push((0, m));
                    }
                }
            }
            CompareKind::SkillGrantingItem => {
                // `skill-tooltips.md` §12.4: the record's `Item_GrantedSkill` (an instance:
                // its record), the skill's hover tooltip, no option gate.
                let skill = ex.did.and_then(|d| {
                    let r = self.data.record(d).ok()??;
                    did_value(r.get(self.c.granted_skill?)?)
                });
                if let Some(skill) = skill {
                    let req = CombatRequest::hover(CombatSubject::Skill(skill));
                    if let Ok(m) = self.build_combat(&req, &ex.ctx.viewer) {
                        out.tooltips.push((0, m));
                    }
                }
            }
            // Trait tooltips are P6 (`skill-tooltips.md` §12.4): nothing attached yet.
            CompareKind::TraitItem | CompareKind::None | CompareKind::Other(_) => {}
        }
        out
    }

    fn slot_bits(&self, set: &PropertySet) -> u32 {
        self.p
            .compatible_slot
            .and_then(|p| set.get(p))
            .and_then(|v| match v {
                PropertyValue::Bitfield32(b) => Some(*b),
                PropertyValue::Bitfield64(b) => u32::try_from(*b & 0xFFFF_FFFF).ok(),
                _ => None,
            })
            .unwrap_or(0)
    }

    /// Classifies one record for the census of §12 (hover tooltip, record only).
    pub fn classify(&self, did: Did, viewer: &Viewer) -> RecordOutcome {
        let req = TooltipRequest::hover(did);
        match self.build(&req, viewer) {
            Ok(m) => {
                let mut non_empty = Vec::new();
                let mut deferred = 0;
                for r in &m.rows {
                    if matches!(r.widget, Widget::Deferred(_)) {
                        deferred += 1;
                    } else if !r.is_empty() && !matches!(r.content, RowContent::Separator) {
                        non_empty.push(r.label.clone().unwrap_or_else(|| "(unlabelled)".into()));
                    }
                }
                let named = m.has_name();
                RecordOutcome {
                    category: Ok(Category::Rendered),
                    row_failures: m.failures,
                    non_empty,
                    deferred,
                    named,
                }
            }
            Err(e) => RecordOutcome {
                category: e.category().ok_or(e),
                row_failures: Vec::new(),
                non_empty: Vec::new(),
                deferred: 0,
                named: false,
            },
        }
    }

    /// The colour of a state of the name-colour table (§6.2: the states of `ColorNameText`
    /// of the `text` layout), read once.
    #[must_use]
    pub fn name_colour(&self, label: &str) -> Option<u32> {
        self.colours
            .get_or_init(|| layout::colour_table(&self.data, &self.ui))
            .iter()
            .find(|(_, l, _)| l.as_deref() == Some(label))
            .map(|(_, _, c)| *c)
    }
}

fn did_value(v: &PropertyValue) -> Option<Did> {
    match v {
        PropertyValue::DidRef(d) if !d.is_null() => Some(*d),
        _ => None,
    }
}

/// The record's properties overlaid by the instance's (§6.4 provider step 1).
fn overlay<'a>(record: &'a Arc<PropertySet>, instance: &[PropertyEntry]) -> Cow<'a, PropertySet> {
    if instance.is_empty() {
        return Cow::Borrowed(record.as_ref());
    }
    let mut set = (**record).clone();
    for e in instance {
        if let Some(slot) = set.entries.iter_mut().find(|x| x.pid == e.pid) {
            slot.value = e.value.clone();
        } else {
            set.entries.push(e.clone());
        }
    }
    Cow::Owned(set)
}

fn frame_error(at: TemplateRef, e: FrameError) -> TooltipError {
    match e {
        FrameError::Resolve(ResolveError::ElementMissing(t)) if t == at => {
            TooltipError::ElementMissing {
                layout: at.layout,
                element: at.element,
            }
        }
        FrameError::Resolve(ResolveError::LayoutMissing(d)) => TooltipError::LayoutUnreadable {
            layout: d,
            reason: "layout record missing".into(),
        },
        FrameError::Resolve(other) => TooltipError::LayoutUnreadable {
            layout: at.layout,
            reason: other.to_string(),
        },
        FrameError::NoListBox(reason) => TooltipError::LayoutUnreadable {
            layout: at.layout,
            reason,
        },
    }
}
