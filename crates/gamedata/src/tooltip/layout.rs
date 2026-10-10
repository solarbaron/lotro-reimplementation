// SPDX-License-Identifier: GPL-3.0-or-later
//! The tooltip element and its row templates, read from the layout data
//! (`item-tooltips.md` §4; base inheritance per `ui-layout.md` §7).
//!
//! Only what the model needs is resolved: an element's merged own properties, its merged
//! named states, its type (the end of its base chain), its design size and its children.
//! Media lists are not merged (the UI does that when it creates the panel).

use std::collections::HashSet;
use std::sync::Arc;

use tessera_formats::ui_layout::{MAX_BASE_CHAIN, UiEnum};
use tessera_formats::{Did, ElementDesc, PropertySet, PropertyValue, StringInfo};

use super::data::TooltipData;

/// Most entry templates read from one list box (hostile-input bound; the data has at most
/// 55).
pub const MAX_TEMPLATES: usize = 256;

/// Most nesting levels of row templates (pairs, nested lists, fields holding a widget).
pub const MAX_TEMPLATE_DEPTH: usize = 6;

/// Most template nodes one top-level template read builds: the template, its nested
/// children and the line elements of its lists (hostile-input bound on the work of a
/// self-referencing or very wide layout; the data's largest template has a handful).
pub const MAX_TEMPLATE_NODES: usize = 256;

/// Line elements `item-tooltips.md` §4.2 names for list widgets ("entries …"), looked up by
/// label in the row's layout when the row has no entry templates (§6.15 OPEN).
const LINE_ELEMENTS: [(&str, &str); 3] = [
    ("ExaminationUsageReqs", "UsageReqsListElement"),
    ("ExaminationEffectList", "EffectListElement"),
    ("ExaminationAllowedSlots", "AllowedSlotsElement"),
];

/// Most entry templates looked at when choosing a list widget's line element.
const MAX_LINE_CANDIDATES: usize = 8;

/// A state colour: (state ID, state label, ARGB).
pub type StateColour = (u32, Option<String>, u32);

/// The work left for one top-level template read, and the top-level templates being read
/// (a list entry naming one of them is a cycle).
#[derive(Debug)]
pub(crate) struct Budget {
    left: usize,
    ancestors: Vec<TemplateRef>,
}

impl Budget {
    /// A full budget of [`MAX_TEMPLATE_NODES`].
    pub(crate) fn new() -> Self {
        Self {
            left: MAX_TEMPLATE_NODES,
            ancestors: Vec::new(),
        }
    }

    /// Takes one node; `false` when the budget is spent.
    fn take(&mut self) -> bool {
        if self.left == 0 {
            return false;
        }
        self.left -= 1;
        true
    }
}

/// A (layout, element) pair naming a top-level element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TemplateRef {
    /// The layout record (0x22…).
    pub layout: Did,
    /// The element ID (`UIElementID`).
    pub element: u32,
}

macro_rules! ui_pids {
    ($(#[$m:meta])* $name:ident { $($field:ident = $label:literal,)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, Default)]
        pub(crate) struct $name {
            $(pub(crate) $field: Option<u32>,)*
        }

        impl $name {
            pub(crate) fn resolve(data: &dyn TooltipData) -> Self {
                Self { $($field: data.pid($label),)* }
            }
        }
    };
}

ui_pids! {
    /// UI properties the tooltip model reads (`item-tooltips.md` §3.3, §4, §6;
    /// `ui-widgets.md` §8).
    UiPids {
        entry_templates = "UICore_ListBox_entry_templates",
        entry_template_layout = "UICore_ListBox_entry_template_layout",
        entry_template_element = "UICore_ListBox_entry_template_element",
        autofill = "UICore_ListBox_entry_templates_autofill",
        horizontal = "UICore_ListBox_horizontal",
        max_width = "UICore_Element_max_width",
        font_colors = "UICore_Text_font_colors",
        font_color = "UICore_Text_font_color",
        text_entry = "UICore_Text_entry",
        parse_markup = "UICore_Text_parse_markup_tags",
        display_string = "UI_Examination_Tooltip_DisplayString",
        generic = "UI_Examination_Tooltip_GeneroProperty",
        server_data = "UI_Examination_Tooltip_GeneroPropertyServerData",
        display_default = "UI_Examination_Tooltip_DisplayDefault",
        as_duration = "UI_Examination_Tooltip_DisplayAsDuration",
        float_precision = "UI_Examination_Tooltip_FloatPrecision",
        comparison = "UI_Examination_Tooltip_ComparisonProperty",
        remove_when_hidden = "UI_Examination_Tooltip_RemoveWhenHidden",
        stretch_wide = "UI_Examination_Tooltip_Stretch_Wide",
        left_child = "UI_Examination_Tooltip_Row_Left_Child",
        right_child = "UI_Examination_Tooltip_Row_Right_Child",
        separator = "UI_Examination_Tooltip_Separator",
        unique_text = "UI_Examination_Tooltip_ItemUniqueText",
        indestructible_text = "UI_Examination_Tooltip_ItemIndestructibleText",
    }
}

fn get(set: &PropertySet, pid: Option<u32>) -> Option<&PropertyValue> {
    set.get(pid?)
}

fn as_bool(v: Option<&PropertyValue>) -> bool {
    matches!(v, Some(PropertyValue::Bool(true)))
}

fn as_key(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(e) | PropertyValue::PropertyId(e) | PropertyValue::Bitfield32(e) => {
            Some(*e)
        }
        PropertyValue::DidRef(d) => Some(d.0),
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        _ => None,
    }
}

fn as_int(v: Option<&PropertyValue>) -> Option<i32> {
    match v? {
        PropertyValue::Int(i) => Some(*i),
        PropertyValue::Int64(i) => i32::try_from(*i).ok(),
        PropertyValue::Enum(e) => i32::try_from(*e).ok(),
        _ => None,
    }
}

/// A colour value as `0xAARRGGBB` (bytes stored B, G, R, A; `property-system.md` §4.7).
fn as_argb(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Colour(c) => Some(u32::from_le_bytes(*c)),
        _ => None,
    }
}

/// The first text colour of a property set: the first element of `UICore_Text_font_colors`,
/// else `UICore_Text_font_color`.
fn first_colour(set: &PropertySet, p: &UiPids) -> Option<u32> {
    if let Some(PropertyValue::Array(a)) = get(set, p.font_colors)
        && let Some(c) = a.elements.iter().find_map(|e| as_argb(&e.value))
    {
        return Some(c);
    }
    get(set, p.font_color).and_then(as_argb)
}

// ---------------------------------------------------------------------------------------------
// Resolution (ui-layout.md §7)
// ---------------------------------------------------------------------------------------------

/// Why an element could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolveError {
    /// The layout record is missing.
    LayoutMissing(Did),
    /// The layout record does not decode.
    LayoutUnreadable(Did, String),
    /// The element is not a top-level element of the layout.
    ElementMissing(TemplateRef),
    /// A base chain failed (missing base, cycle, too long).
    Chain(String),
    /// A row template names itself through its entries, or is too large.
    Template(&'static str),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LayoutMissing(d) => write!(f, "layout {d} missing"),
            Self::LayoutUnreadable(d, r) => write!(f, "layout {d} unreadable: {r}"),
            Self::ElementMissing(t) => write!(
                f,
                "element {:#010x} not top-level in {}",
                t.element, t.layout
            ),
            Self::Chain(r) => write!(f, "base chain: {r}"),
            Self::Template(r) => write!(f, "row template: {r}"),
        }
    }
}

/// A resolved element: the merged descriptor (typed by the end of its chain) and its
/// immediate base element, if it had one.
#[derive(Debug, Clone)]
pub(crate) struct Resolved {
    pub(crate) desc: ElementDesc,
    pub(crate) base: Option<TemplateRef>,
}

/// Loads `layout` and clones its top-level element `element`.
fn top_level(data: &dyn TooltipData, t: TemplateRef) -> Result<ElementDesc, ResolveError> {
    let layout = match data.layout(t.layout) {
        Ok(Some(l)) => l,
        Ok(None) => return Err(ResolveError::LayoutMissing(t.layout)),
        Err(e) => return Err(ResolveError::LayoutUnreadable(t.layout, e)),
    };
    layout
        .top_level(t.element)
        .cloned()
        .ok_or(ResolveError::ElementMissing(t))
}

/// Resolves the top-level element `t` (`ui-layout.md` §7.1).
pub(crate) fn resolve_top(
    data: &dyn TooltipData,
    t: TemplateRef,
) -> Result<Resolved, ResolveError> {
    let desc = top_level(data, t)?;
    let base = desc.base().map(|b| TemplateRef {
        layout: b.layout,
        element: b.element,
    });
    let mut visited = HashSet::new();
    visited.insert(t);
    let desc = resolve_desc(data, desc, &mut visited)?;
    Ok(Resolved { desc, base })
}

/// Resolves a descriptor that may carry a base reference: merges it into its resolved
/// base (§7.1 step 4). `visited` holds the (layout, element) pairs of the chain so far.
pub(crate) fn resolve_desc(
    data: &dyn TooltipData,
    desc: ElementDesc,
    visited: &mut HashSet<TemplateRef>,
) -> Result<ElementDesc, ResolveError> {
    let Some(b) = desc.base() else {
        return Ok(desc);
    };
    let t = TemplateRef {
        layout: b.layout,
        element: b.element,
    };
    if visited.len() >= MAX_BASE_CHAIN {
        return Err(ResolveError::Chain("too long".into()));
    }
    if !visited.insert(t) {
        return Err(ResolveError::Chain("cycle".into()));
    }
    let base = top_level(data, t).map_err(|e| ResolveError::Chain(e.to_string()))?;
    let mut r = resolve_desc(data, base, visited)?;
    merge_desc(&mut r, &desc);
    Ok(r)
}

/// Merges property set `d` into `r`: every property of `d` replaces the one with the same
/// PID or is added (`ui-layout.md` §7.2 step 3).
fn merge_props(r: &mut PropertySet, d: &PropertySet) {
    for e in &d.entries {
        if let Some(slot) = r.entries.iter_mut().find(|x| x.pid == e.pid) {
            slot.value = e.value.clone();
        } else {
            r.entries.push(e.clone());
        }
    }
}

/// Merges descriptor `d` into `r` (`ui-layout.md` §7.3), without media and without the
/// design-time re-layout of step 8.
fn merge_desc(r: &mut ElementDesc, d: &ElementDesc) {
    r.block.flags = d.block.flags;
    r.block.rect = d.block.rect;
    r.block.z_level = d.block.z_level;
    r.block.has_position = d.block.has_position;
    merge_props(&mut r.block.properties, &d.block.properties);
    if d.element_id != 0 {
        r.element_id = d.element_id;
    }
    r.default_state = d.default_state;
    r.edges = d.edges;
    r.read_order = d.read_order;
    for s in &d.states {
        if let Some(rs) = r.states.iter_mut().find(|x| x.state_id == s.state_id) {
            rs.flags = s.flags;
            merge_props(&mut rs.properties, &s.properties);
        } else {
            r.states.push(s.clone());
        }
    }
    let c = u32::try_from(
        r.children
            .iter()
            .filter(|rc| !d.children.iter().any(|dc| dc.element_id == rc.element_id))
            .count(),
    )
    .unwrap_or(u32::MAX);
    for dc in &d.children {
        if let Some(rc) = r
            .children
            .iter_mut()
            .find(|x| x.element_id == dc.element_id)
        {
            merge_desc(rc, dc);
        } else {
            let mut added = dc.clone();
            added.read_order = added.read_order.saturating_add(c);
            r.children.push(added);
        }
    }
}

/// The children of a resolved element in authored order, each resolved.
fn resolved_children(
    data: &dyn TooltipData,
    desc: &ElementDesc,
) -> Vec<Result<ElementDesc, ResolveError>> {
    desc.children_in_read_order()
        .into_iter()
        .map(|c| resolve_desc(data, c.clone(), &mut HashSet::new()))
        .collect()
}

/// Label prefixes of the generic rows §6.20 leaves empty in P1: legendary
/// (`ItemAdvancement_*`) and decoration-house rows.
const DEFERRED_GENERIC: [&str; 3] = [
    "ItemAdvancement_",
    "ItemDecorationNotAllowedInKinshipHouse",
    "ItemDecorationOnlyAllowedForHouseOwner",
];

fn desc_label(data: &dyn TooltipData, desc: &ElementDesc) -> Option<String> {
    data.ui_label(UiEnum::ElementId, desc.element_id)
}

fn type_label(data: &dyn TooltipData, desc: &ElementDesc) -> Option<String> {
    data.ui_label(UiEnum::ElementType, desc.element_type)
}

// ---------------------------------------------------------------------------------------------
// §4.1 The tooltip element
// ---------------------------------------------------------------------------------------------

/// The list box of a tooltip element (`item-tooltips.md` §4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListBoxInfo {
    /// The list box's designed width.
    pub design_width: u32,
    /// `UICore_Element_max_width`, when set.
    pub max_width: Option<u32>,
    /// `UICore_ListBox_entry_templates_autofill`.
    pub autofill: bool,
    /// The entry templates in display order; an entry without both fields is `None`.
    pub templates: Vec<Option<TemplateRef>>,
}

impl ListBoxInfo {
    /// The width limit of §5 step 3 and §7: `max_width`, else the designed width (Tessera
    /// rule of the §4.1 OPEN).
    #[must_use]
    pub fn width_limit(&self) -> u32 {
        self.max_width.unwrap_or(self.design_width)
    }
}

/// A tooltip element (`item-tooltips.md` §4.1): a frame holding one examination list box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooltipFrame {
    /// Where it is.
    pub at: TemplateRef,
    /// Its `UIElementID` label.
    pub label: Option<String>,
    /// The label of its immediate base element (`Tooltip_New` in the data).
    pub base_label: Option<String>,
    /// How many children the element itself holds, not counting those it inherits from
    /// its base (one in the data, IV2).
    pub children: usize,
    /// The examination list box.
    pub list: ListBoxInfo,
}

/// Why a frame could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrameError {
    Resolve(ResolveError),
    NoListBox(String),
}

/// The entry templates of a list box's merged properties.
fn entry_templates(set: &PropertySet, p: &UiPids) -> Vec<Option<TemplateRef>> {
    let Some(PropertyValue::Array(a)) = get(set, p.entry_templates) else {
        return Vec::new();
    };
    a.elements
        .iter()
        .take(MAX_TEMPLATES)
        .map(|e| {
            let s = e.value.as_struct()?;
            let layout = match s.get(p.entry_template_layout?)? {
                PropertyValue::DidRef(d) if !d.is_null() => *d,
                _ => return None,
            };
            let element = as_key(s.get(p.entry_template_element?)?)?;
            Some(TemplateRef { layout, element })
        })
        .collect()
}

fn list_box_info(desc: &ElementDesc, p: &UiPids) -> ListBoxInfo {
    let props = &desc.block.properties;
    ListBoxInfo {
        design_width: u32::try_from(desc.block.rect.width).unwrap_or(0),
        max_width: as_int(get(props, p.max_width)).and_then(|w| u32::try_from(w).ok()),
        autofill: as_bool(get(props, p.autofill)),
        templates: entry_templates(props, p),
    }
}

/// Reads the tooltip element `at` (§4.1): resolves it and finds its examination list box.
pub(crate) fn read_frame(
    data: &dyn TooltipData,
    p: &UiPids,
    at: TemplateRef,
) -> Result<TooltipFrame, FrameError> {
    let own = top_level(data, at)
        .map_err(FrameError::Resolve)?
        .children
        .len();
    let r = resolve_top(data, at).map_err(FrameError::Resolve)?;
    let children = resolved_children(data, &r.desc);
    let count = children.len();
    let mut list = None;
    for c in children.into_iter().flatten() {
        if type_label(data, &c).as_deref() == Some("ExaminationListBox") {
            list = Some(list_box_info(&c, p));
            break;
        }
    }
    let list = list.ok_or_else(|| {
        FrameError::NoListBox(format!("{count} children, none an ExaminationListBox"))
    })?;
    Ok(TooltipFrame {
        at,
        label: data.ui_label(UiEnum::ElementId, at.element),
        base_label: r
            .base
            .and_then(|b| data.ui_label(UiEnum::ElementId, b.element)),
        children: own,
        list,
    })
}

// ---------------------------------------------------------------------------------------------
// §4.2–§4.4 Row templates
// ---------------------------------------------------------------------------------------------

/// The row behaviour selected by a row template's widget type (`item-tooltips.md` §4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Widget {
    /// `ExaminationName` (§6.2).
    Name,
    /// `ExaminationIcon` (§6.3).
    Icon,
    /// `ExaminationItemBinding` (§6.5).
    Binding,
    /// `ExaminationItemUniqueness` (§6.6).
    Uniqueness,
    /// `ExaminationGeneroProperty`, `ExaminationSkillGeneroProperty` (§6.4).
    Generic,
    /// `ExaminationItemLevel` (§6.7).
    ItemLevel,
    /// `ExaminationIndestructibility` (§6.8).
    Indestructible,
    /// `ExaminationArmor` (§6.9).
    Armour,
    /// `ExaminationDurability` (§6.10).
    Durability,
    /// `ExaminationValue` (§6.11).
    Worth,
    /// `ExaminationDamage` (§6.12).
    Damage,
    /// `ExaminationDPS` (§6.12).
    Dps,
    /// `ExaminationEquipmentCategory` (§6.13).
    EquipmentCategory,
    /// `ExaminationAllowedSlots` (§6.14).
    AllowedSlots,
    /// `ExaminationUsageReqs` (§6.15).
    Requirements,
    /// `UIExaminationLevelRangeRestriction` (§6.15).
    LevelRange,
    /// `ExaminationEffectList` (§6.16).
    EffectList,
    /// `ExaminationGameSetMemberList` (§6.17).
    SetMembers,
    /// `ExaminationGameSetBonusList` (§6.17).
    SetBonuses,
    /// `ExaminationUsageCooldown` (§6.18).
    Cooldown,
    /// `ExaminationItemDecay` (§6.18).
    Decay,
    /// `ExaminationQuestID` (§6.18).
    QuestId,
    /// `ExaminationRow`: two children on one line (§4.3).
    Pair,
    /// A list of rows from its own entry templates (the name-and-icon row, §4.3).
    List,
    /// A field with `UI_Examination_Tooltip_Separator` (§4.4).
    Separator,
    /// A plain `Text` element (`CurrentlyEquipped_Label`, §8.1).
    StaticText,
    /// Any other widget type: left empty in P1 (§6.20). Holds the type label.
    Deferred(String),
}

impl Widget {
    fn from_type_label(label: &str) -> Option<Self> {
        Some(match label {
            "ExaminationName" => Self::Name,
            "ExaminationIcon" => Self::Icon,
            "ExaminationItemBinding" => Self::Binding,
            "ExaminationItemUniqueness" => Self::Uniqueness,
            "ExaminationGeneroProperty" | "ExaminationSkillGeneroProperty" => Self::Generic,
            "ExaminationItemLevel" => Self::ItemLevel,
            "ExaminationIndestructibility" => Self::Indestructible,
            "ExaminationArmor" => Self::Armour,
            "ExaminationDurability" => Self::Durability,
            "ExaminationValue" => Self::Worth,
            "ExaminationDamage" => Self::Damage,
            "ExaminationDPS" => Self::Dps,
            "ExaminationEquipmentCategory" => Self::EquipmentCategory,
            "ExaminationAllowedSlots" => Self::AllowedSlots,
            "ExaminationUsageReqs" => Self::Requirements,
            "UIExaminationLevelRangeRestriction" => Self::LevelRange,
            "ExaminationEffectList" => Self::EffectList,
            "ExaminationGameSetMemberList" => Self::SetMembers,
            "ExaminationGameSetBonusList" => Self::SetBonuses,
            "ExaminationUsageCooldown" => Self::Cooldown,
            "ExaminationItemDecay" => Self::Decay,
            "ExaminationQuestID" => Self::QuestId,
            "ExaminationRow" => Self::Pair,
            "Text" => Self::StaticText,
            _ => return None,
        })
    }

    /// Whether the widget is one of the examination rows the model fills.
    #[must_use]
    pub fn is_filled(&self) -> bool {
        !matches!(self, Self::Deferred(_))
    }
}

/// A row template, resolved and read once (`item-tooltips.md` §4.2).
#[derive(Debug, Clone)]
pub struct RowTemplate {
    /// Where it is (for nested children: the top-level element or the parent's position).
    pub at: TemplateRef,
    /// Its `UIElementID` label.
    pub label: Option<String>,
    /// Its widget type label.
    pub type_label: Option<String>,
    /// The behaviour.
    pub widget: Widget,
    /// Designed width and height.
    pub size: (u32, u32),
    /// `UI_Examination_Tooltip_Stretch_Wide`.
    pub stretch_wide: bool,
    /// `UI_Examination_Tooltip_RemoveWhenHidden`.
    pub remove_when_hidden: bool,
    /// The row's format (`UI_Examination_Tooltip_DisplayString`).
    pub format: Option<StringInfo>,
    /// The uniqueness or indestructible text, or a `Text` element's entry.
    pub fixed_text: Option<StringInfo>,
    /// `UI_Examination_Tooltip_GeneroProperty`.
    pub generic: Option<u32>,
    /// `UI_Examination_Tooltip_GeneroPropertyServerData`.
    pub server_data: bool,
    /// `UI_Examination_Tooltip_DisplayDefault`.
    pub display_default: bool,
    /// `UI_Examination_Tooltip_DisplayAsDuration`.
    pub as_duration: bool,
    /// `UI_Examination_Tooltip_FloatPrecision` (default 1).
    pub float_precision: i32,
    /// `UI_Examination_Tooltip_ComparisonProperty`.
    pub comparison: Option<u32>,
    /// The row's own value of the comparison property, when it holds one (§6.4 step 5).
    pub comparison_value: Option<PropertyValue>,
    /// `UICore_ListBox_horizontal`: a list laid out sideways (the name-and-icon row, §4.3).
    pub horizontal: bool,
    /// `UICore_Text_parse_markup_tags`.
    pub parse_markup: bool,
    /// The base text colour (ARGB).
    pub colour: Option<u32>,
    /// State colours: (state ID, state label, ARGB).
    pub state_colours: Vec<(u32, Option<String>, u32)>,
    /// The base text colour of a list widget's line element (§4.2 "entries …"): the
    /// requirement, effect, set and allowed-slot lines.
    pub line_colour: Option<u32>,
    /// The state colours of that line element: (state ID, state label, ARGB).
    pub line_state_colours: Vec<(u32, Option<String>, u32)>,
    /// Children: the two sides of a pair, the entries of a list, or the inner widget of a
    /// field. `None` entries are children that failed to resolve.
    pub children: Vec<Option<Arc<RowTemplate>>>,
    /// Why a child or the template itself could not be read fully.
    pub problems: Vec<String>,
}

impl RowTemplate {
    /// The colour of state `label`, if the template has that state with a colour.
    #[must_use]
    pub fn state_colour(&self, label: &str) -> Option<u32> {
        self.state_colours
            .iter()
            .find(|(_, l, _)| l.as_deref() == Some(label))
            .map(|(_, _, c)| *c)
    }

    /// The colour of one line of a list widget in `state`: the line element's state colour,
    /// else its base colour, else the row's own state colour or base colour.
    #[must_use]
    pub fn line_colour(&self, state: Option<&str>) -> Option<u32> {
        let line_state = state.and_then(|s| {
            self.line_state_colours
                .iter()
                .find(|(_, l, _)| l.as_deref() == Some(s))
                .map(|(_, _, c)| *c)
        });
        line_state
            .or(self.line_colour)
            .or_else(|| state.and_then(|s| self.state_colour(s)))
            .or(self.colour)
    }

    /// The problems of this template and of its nested children, with the label of the
    /// template they belong to.
    #[must_use]
    pub fn all_problems(&self) -> Vec<(Option<String>, String)> {
        let mut out = Vec::new();
        let mut stack: Vec<&RowTemplate> = vec![self];
        while let Some(t) = stack.pop() {
            for pr in &t.problems {
                out.push((t.label.clone(), pr.clone()));
            }
            stack.extend(t.children.iter().flatten().map(|c| &**c));
        }
        out
    }
}

/// Builds the template of a top-level element within `budget` (one node for the element).
pub(crate) fn read_template(
    data: &dyn TooltipData,
    p: &UiPids,
    at: TemplateRef,
    depth: usize,
    budget: &mut Budget,
) -> Result<RowTemplate, ResolveError> {
    if budget.ancestors.contains(&at) {
        return Err(ResolveError::Template(
            "names itself through its entries (cycle)",
        ));
    }
    if !budget.take() {
        return Err(ResolveError::Template("too many nested templates"));
    }
    let r = resolve_top(data, at)?;
    budget.ancestors.push(at);
    let t = template_of(data, p, at, &r.desc, depth, budget);
    budget.ancestors.pop();
    Ok(t)
}

/// The line element of a list widget (§4.2 "entries …"): among the widget's entry
/// templates, the first whose element has state colours, else the first with a base
/// colour; without entry templates, the element §4.2 names for the widget type, looked up
/// by label in the row's layout (§6.15 OPEN). Returns (base colour, state colours).
fn line_element(
    data: &dyn TooltipData,
    p: &UiPids,
    at: TemplateRef,
    props: &PropertySet,
    type_label: &str,
    budget: &mut Budget,
) -> (Option<u32>, Vec<StateColour>) {
    let mut candidates: Vec<TemplateRef> = entry_templates(props, p)
        .into_iter()
        .flatten()
        .take(MAX_LINE_CANDIDATES)
        .collect();
    if candidates.is_empty()
        && let Some((_, label)) = LINE_ELEMENTS.iter().find(|(t, _)| *t == type_label)
        && let Some(element) = data.ui_key(UiEnum::ElementId, label)
    {
        candidates.push(TemplateRef {
            layout: at.layout,
            element,
        });
    }
    let mut first_base = None;
    for c in candidates {
        if !budget.take() {
            break;
        }
        let Ok(r) = resolve_top(data, c) else {
            continue;
        };
        let base = first_colour(&r.desc.block.properties, p);
        let states = state_colours(data, p, &r.desc);
        if !states.is_empty() {
            return (base, states);
        }
        if first_base.is_none() {
            first_base = base;
        }
    }
    (first_base, Vec::new())
}

fn state_colours(data: &dyn TooltipData, p: &UiPids, desc: &ElementDesc) -> Vec<StateColour> {
    desc.states
        .iter()
        .filter_map(|s| {
            let c = first_colour(&s.properties, p)?;
            Some((s.state_id, data.ui_label(UiEnum::StateId, s.state_id), c))
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn template_of(
    data: &dyn TooltipData,
    p: &UiPids,
    at: TemplateRef,
    desc: &ElementDesc,
    depth: usize,
    budget: &mut Budget,
) -> RowTemplate {
    let props = &desc.block.properties;
    let type_label = type_label(data, desc);
    let separator = as_bool(get(props, p.separator));
    let is_list_box = matches!(
        type_label.as_deref(),
        Some("AutoResizeListBox" | "ListBox" | "ExaminationListBox")
    );
    let is_field = matches!(type_label.as_deref(), None | Some("Field"));
    let mut widget = if separator {
        Widget::Separator
    } else if let Some(w) = type_label.as_deref().and_then(Widget::from_type_label) {
        w
    } else if is_list_box {
        Widget::List
    } else {
        Widget::Deferred(type_label.clone().unwrap_or_default())
    };
    // §6.20: generic rows Tessera leaves empty in P1 (legendary and decoration-house rows).
    if widget == Widget::Generic
        && desc_label(data, desc).is_some_and(|l| DEFERRED_GENERIC.iter().any(|d| l.starts_with(d)))
    {
        widget = Widget::Deferred(type_label.clone().unwrap_or_default());
    }
    let string = |pid: Option<u32>| match get(props, pid) {
        Some(PropertyValue::StringInfo(s)) => Some((**s).clone()),
        _ => None,
    };
    let fixed_text = match widget {
        Widget::Uniqueness => string(p.unique_text),
        Widget::Indestructible => string(p.indestructible_text),
        Widget::StaticText => string(p.text_entry),
        _ => None,
    };
    let state_colours = state_colours(data, p, desc);
    let comparison = get(props, p.comparison).and_then(as_key);
    let (line_colour, line_state_colours) = if matches!(
        widget,
        Widget::Requirements
            | Widget::EffectList
            | Widget::AllowedSlots
            | Widget::SetMembers
            | Widget::SetBonuses
    ) {
        line_element(
            data,
            p,
            at,
            props,
            type_label.as_deref().unwrap_or(""),
            budget,
        )
    } else {
        (None, Vec::new())
    };
    let mut t = RowTemplate {
        at,
        label: data.ui_label(UiEnum::ElementId, desc.element_id),
        type_label,
        widget: Widget::Separator,
        size: (
            u32::try_from(desc.block.rect.width).unwrap_or(0),
            u32::try_from(desc.block.rect.height).unwrap_or(0),
        ),
        stretch_wide: as_bool(get(props, p.stretch_wide)),
        remove_when_hidden: as_bool(get(props, p.remove_when_hidden)),
        format: string(p.display_string),
        fixed_text,
        generic: get(props, p.generic).and_then(as_key),
        server_data: as_bool(get(props, p.server_data)),
        display_default: as_bool(get(props, p.display_default)),
        as_duration: as_bool(get(props, p.as_duration)),
        float_precision: as_int(get(props, p.float_precision)).unwrap_or(1),
        comparison,
        comparison_value: comparison.and_then(|q| props.get(q)).cloned(),
        horizontal: as_bool(get(props, p.horizontal)),
        parse_markup: as_bool(get(props, p.parse_markup)),
        colour: first_colour(props, p),
        state_colours,
        line_colour,
        line_state_colours,
        children: Vec::new(),
        problems: Vec::new(),
    };
    if depth + 1 >= MAX_TEMPLATE_DEPTH {
        if matches!(widget, Widget::Pair | Widget::List) {
            t.problems.push("template nesting too deep".into());
        }
        t.widget = widget;
        return t;
    }
    match widget {
        Widget::Pair => {
            for side in [p.left_child, p.right_child] {
                let id = get(props, side).and_then(as_key).filter(|&k| k != 0);
                let child = id.and_then(|id| {
                    pair_child(data, p, at, desc, id, depth, budget)
                        .map_err(|e| t.problems.push(e.to_string()))
                        .ok()
                });
                t.children.push(child.map(Arc::new));
            }
        }
        Widget::List => {
            for e in entry_templates(props, p) {
                let Some(e) = e else {
                    t.problems
                        .push("entry template without layout or element".into());
                    t.children.push(None);
                    continue;
                };
                match read_template(data, p, e, depth + 1, budget) {
                    Ok(c) => t.children.push(Some(Arc::new(c))),
                    Err(err) => {
                        t.problems.push(err.to_string());
                        t.children.push(None);
                    }
                }
            }
            if t.children.is_empty() {
                // A list without templates: look at its own children instead.
                for c in desc.children_in_read_order() {
                    if !budget.take() {
                        t.problems.push("too many nested templates".into());
                        break;
                    }
                    if let Ok(c) = resolve_desc(data, c.clone(), &mut HashSet::new()) {
                        let inner = template_of(data, p, at, &c, depth + 1, budget);
                        t.children.push(Some(Arc::new(inner)));
                    }
                }
            }
        }
        Widget::Deferred(_) if is_field => {
            // A field holding examination widgets (the icon field, the name-and-icon field
            // of the legendary layouts, §4.2): the row shows its filled children.
            for c in desc.children_in_read_order() {
                if !budget.take() {
                    t.problems.push("too many nested templates".into());
                    break;
                }
                let Ok(c) = resolve_desc(data, c.clone(), &mut HashSet::new()) else {
                    continue;
                };
                let inner = template_of(data, p, at, &c, depth + 1, budget);
                if inner.widget.is_filled()
                    && !matches!(inner.widget, Widget::Separator | Widget::StaticText)
                {
                    t.children.push(Some(Arc::new(inner)));
                }
            }
            if !t.children.is_empty() {
                widget = Widget::List;
            }
        }
        _ => {}
    }
    t.widget = widget;
    t
}

/// The element a pair names as one side: a child of the row (searched in the resolved
/// children), else a top-level element of the row's layout.
fn pair_child(
    data: &dyn TooltipData,
    p: &UiPids,
    at: TemplateRef,
    desc: &ElementDesc,
    id: u32,
    depth: usize,
    budget: &mut Budget,
) -> Result<RowTemplate, ResolveError> {
    if let Some(c) = desc.child(id) {
        if !budget.take() {
            return Err(ResolveError::Template("too many nested templates"));
        }
        let c = resolve_desc(data, c.clone(), &mut HashSet::new())?;
        return Ok(template_of(data, p, at, &c, depth + 1, budget));
    }
    read_template(
        data,
        p,
        TemplateRef {
            layout: at.layout,
            element: id,
        },
        depth + 1,
        budget,
    )
}

/// The colour table of §6.2: every state of `ColorNameText` of the `text` layout, state ID
/// → (label, first colour).
pub(crate) fn colour_table(data: &dyn TooltipData, p: &UiPids) -> Vec<(u32, Option<String>, u32)> {
    let Some(layout) = data.layout_by_label("text") else {
        return Vec::new();
    };
    let Some(element) = data.ui_key(UiEnum::ElementId, "ColorNameText") else {
        return Vec::new();
    };
    let Ok(r) = resolve_top(data, TemplateRef { layout, element }) else {
        return Vec::new();
    };
    state_colours(data, p, &r.desc)
}
