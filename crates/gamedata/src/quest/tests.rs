// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the journal model (`quest-journal.md` §12 worked examples and the
//! rules of §4–§8) over an in-memory [`PresentationData`]: our own records, labels and
//! texts (format strings rendered by `tessera-text`), with invented IDs and wording.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyStruct};
use tessera_formats::reader::Utf16String;
use tessera_formats::ui_layout::UiEnum;
use tessera_formats::{
    Did, PropertySet, PropertyType, PropertyValue, StringArgValue, StringInfo, StringSource,
    UiLayout,
};
use tessera_text::{Message, RenderOptions, Value, Values};

use super::*;
use crate::tooltip::{TextRef, TextResult, TooltipData};

// ---------------------------------------------------------------------------------------------
// The in-memory install
// ---------------------------------------------------------------------------------------------

/// An in-memory [`PresentationData`] (shared with the NPC-window tests).
#[derive(Default)]
pub(crate) struct Mem {
    names: Vec<String>,
    pub defaults: HashMap<u32, PropertyValue>,
    pub enum_texts: HashMap<(u32, u32), String>,
    pub enum_names: HashMap<(u32, u32), String>,
    pub records: HashMap<Did, Arc<PropertySet>>,
    pub weenies: HashMap<String, Did>,
    pub layouts: HashMap<Did, Arc<UiLayout>>,
    pub layout_labels: HashMap<String, Did>,
    pub ui: Vec<(UiEnum, u32, String)>,
    tables: HashMap<String, Did>,
    texts: HashMap<(Did, u32), String>,
}

impl Mem {
    /// The PID of `name`, defining it on first use.
    pub fn p(&mut self, name: &str) -> u32 {
        if let Some(i) = self.names.iter().position(|n| n == name) {
            return i as u32 + 1;
        }
        self.names.push(name.to_owned());
        self.names.len() as u32
    }

    fn table_did(&mut self, table: &str) -> Did {
        let n = self.tables.len() as u32;
        *self
            .tables
            .entry(table.to_owned())
            .or_insert(Did(0x2500_0001 + n))
    }

    /// Defines `token` of `table` as a format string (`${#XXXXXXXX}` slots).
    pub fn text(&mut self, table: &str, token: u32, format: &str) {
        let t = self.table_did(table);
        self.texts.insert((t, token), format.to_owned());
    }

    /// A StringInfo naming `token` of `table`, defined as `format`.
    pub fn info(&mut self, table: &str, token: u32, format: &str) -> StringInfo {
        self.text(table, token, format);
        StringInfo {
            source: StringSource::Table {
                token,
                table: self.table_did(table),
            },
            args: None,
        }
    }

    /// A property entry by name.
    pub fn e(&mut self, name: &str, value: PropertyValue) -> PropertyEntry {
        PropertyEntry {
            pid: self.p(name),
            value,
        }
    }

    /// Stores a record.
    pub fn put(&mut self, did: Did, entries: Vec<PropertyEntry>) {
        self.records.insert(did, Arc::new(set(entries)));
    }

    fn message(&self, info: &StringInfo) -> Result<Message, String> {
        match &info.source {
            StringSource::Literal(s) => Ok(Message::literal(&String::from_utf16_lossy(&s.0))),
            StringSource::Table { token, table } => self
                .texts
                .get(&(*table, *token))
                .map(|f| Message::from_format_string(f))
                .ok_or_else(|| format!("miss {token:#010x}")),
        }
    }

    fn own_values(&self, info: &StringInfo) -> Values {
        let mut v = Values::new();
        if let Some(a) = &info.args {
            for x in &a.values {
                let val = match &x.value {
                    StringArgValue::Integer(n) => Value::Integer(i64::from(*n)),
                    StringArgValue::Float(f) => Value::from(*f),
                    StringArgValue::Nested(n) => match self.text_value(n) {
                        Ok(v) => v,
                        Err(_) => continue,
                    },
                };
                v.insert(x.var_id, val);
            }
        }
        v
    }
}

pub(crate) fn set(entries: Vec<PropertyEntry>) -> PropertySet {
    PropertySet {
        entries,
        ..PropertySet::default()
    }
}

pub(crate) fn st(fields: Vec<PropertyEntry>) -> PropertyValue {
    PropertyValue::Struct(PropertyStruct {
        fields,
        ..PropertyStruct::default()
    })
}

pub(crate) fn arr(elements: Vec<PropertyEntry>) -> PropertyValue {
    PropertyValue::Array(PropertyArray {
        count: elements.len() as u32,
        elements,
    })
}

pub(crate) fn si(info: StringInfo) -> PropertyValue {
    PropertyValue::StringInfo(Box::new(info))
}

pub(crate) fn literal(s: &str) -> StringInfo {
    StringInfo {
        source: StringSource::Literal(Utf16String(s.encode_utf16().collect())),
        args: None,
    }
}

impl TooltipData for Mem {
    fn pid(&self, name: &str) -> Option<u32> {
        self.names
            .iter()
            .position(|n| n == name)
            .map(|i| i as u32 + 1)
    }
    fn property_name(&self, pid: u32) -> Option<String> {
        self.names.get((pid as usize).wrapping_sub(1)).cloned()
    }
    fn property_type(&self, _pid: u32) -> Option<PropertyType> {
        None
    }
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        self.defaults.get(&pid).cloned()
    }
    fn enum_text(&self, pid: u32, key: u32) -> Option<String> {
        self.enum_texts.get(&(pid, key)).cloned()
    }
    fn enum_key(&self, pid: u32, name: &str) -> Option<u32> {
        self.enum_names
            .iter()
            .find(|((p, _), n)| *p == pid && n.as_str() == name)
            .map(|((_, k), _)| *k)
    }
    fn record(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String> {
        Ok(self.records.get(&did).cloned())
    }
    fn weenie(&self, label: &str) -> Option<Did> {
        self.weenies.get(label).copied()
    }
    fn layout(&self, did: Did) -> Result<Option<Arc<UiLayout>>, String> {
        Ok(self.layouts.get(&did).cloned())
    }
    fn layout_by_label(&self, label: &str) -> Option<Did> {
        self.layout_labels.get(label).copied()
    }
    fn ui_label(&self, which: UiEnum, key: u32) -> Option<String> {
        self.ui
            .iter()
            .find(|(w, k, _)| *w == which && *k == key)
            .map(|(_, _, l)| l.clone())
    }
    fn ui_key(&self, which: UiEnum, name: &str) -> Option<u32> {
        self.ui
            .iter()
            .find(|(w, _, l)| *w == which && l == name)
            .map(|(_, k, _)| *k)
    }
    fn text_value(&self, info: &StringInfo) -> Result<Value, String> {
        match &info.source {
            StringSource::Literal(s) => Ok(Value::Text(String::from_utf16_lossy(&s.0))),
            StringSource::Table { .. } => {
                Ok(Value::message(self.message(info)?, self.own_values(info)))
            }
        }
    }
    fn render(&self, text: TextRef<'_>, values: &Values) -> TextResult {
        let TextRef::Info(info) = text else {
            return Err("tokens by key are not used".into());
        };
        let msg = self.message(info)?;
        let mut all = self.own_values(info);
        for (id, v) in values.iter() {
            if all.get(id).is_none() {
                all.insert(id, v.clone());
            }
        }
        if msg.arg_ids().iter().any(|id| all.get(*id).is_none()) {
            return Ok(None);
        }
        let r = tessera_text::render(&msg, &all, &RenderOptions::default());
        Ok((!r.text.plain_text().trim().is_empty()).then_some(r.text))
    }
    fn set_of(&self, _item: Did) -> Option<Did> {
        None
    }
}

impl PresentationData for Mem {
    fn table(&self, label: &str) -> Option<Did> {
        self.tables.get(label).copied()
    }
    fn text_args(&self, text: TextRef<'_>) -> Result<Vec<u32>, String> {
        let TextRef::Info(info) = text else {
            return Err("tokens by key are not used".into());
        };
        let own = self.own_values(info);
        let mut out = Vec::new();
        for id in self.message(info)?.arg_ids() {
            if own.get(id).is_none() && !out.contains(&id) {
                out.push(id);
            }
        }
        Ok(out)
    }
    fn enum_name(&self, pid: u32, key: u32) -> Option<String> {
        self.enum_names.get(&(pid, key)).cloned()
    }
}

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

const QA: Did = Did(0x7000_0001);
const QB: Did = Did(0x7000_0002);
const QC: Did = Did(0x7000_0003);
const QP: Did = Did(0x7000_0010);
const DEED: Did = Did(0x7000_0020);
const NPC_R: Did = Did(0x7000_0030);
const NPC_N: Did = Did(0x7000_0031);
const RING_Q: Did = Did(0x7000_0040);

/// The journal's tables and the property names the presenter resolves.
pub(crate) fn base() -> Mem {
    let mut m = Mem::default();
    for n in [
        "Quest_Name",
        "Quest_Description",
        "Quest_IsAccomplishment",
        "Quest_ObjectiveArray",
        "Quest_RoleArray",
        "Quest_Category",
        "Quest_Scope",
        "Quest_ChallengeLevel",
        "Quest_ChallengeLevelOverrideProperty",
        "Quest_HideChallengeLevel",
        "Quest_MaxTimesCompletable",
        "Quest_CanBeCancelled",
        "Quest_IsInstanceQuest",
        "Quest_IsSessionQuest",
        "Quest_IsRaidQuest",
        "Quest_IsFellowshipRecommended",
        "Quest_IsSmallFellowshipRecommended",
        "Quest_ShowRaidInJournal",
        "Quest_ExpTier",
        "Quest_IsHidden",
        "Quest_CompletionObjective",
        "Quest_ObjectiveIndex",
        "Quest_CompletionConditionArray",
        "Quest_CompletionCondition",
        "Quest_ObjectiveDescription",
        "Quest_ObjectiveProgressOverride",
        "Quest_ObjectiveBillboardOverride",
        "QuestEvent_Entry",
        "QuestEvent_ID",
        "QuestEvent_Number",
        "QuestEvent_ProgressOverride",
        "QuestEvent_BillboardProgressOverride",
        "QuestEvent_ShowProgressText",
        "QuestEvent_ShowBillboardText",
        "QuestEvent_NPCTalk",
        "QuestDispenser_Action",
        "QuestDispenser_NPC",
        "QuestDispenser_RoleSuccessText",
        "QuestDispenser_TextArray",
        "QuestDispenser_Species",
        "Quest_QuestTreasureDID",
        "Name",
        "Advancement_Level",
        "Agent_Species",
        "Agent_Class",
        "Sex",
    ] {
        m.p(n);
    }
    m.text(tokens::QUEST_TABLE, tokens::CATEGORY_FALLBACK, "(other)");
    m.text(
        tokens::QUEST_TABLE,
        tokens::TITLE_WITH_LEVEL,
        "[${#0050AA9C}] ${#0058E905}",
    );
    m.text(
        tokens::QUEST_TABLE,
        tokens::TITLE_WITHOUT_LEVEL,
        "${#0058E905}",
    );
    m.text(tokens::QUEST_TABLE, tokens::COMPLETED_WORD, "Done");
    m.text(tokens::QUEST_TABLE, tokens::FAILED_WORD, "Lost");
    m.text(
        tokens::QUEST_TABLE,
        tokens::ALTERNATIVE_JOIN,
        "${#0896E2F1}\n~ or ~\n${#0896E2F2}",
    );
    m.text(
        tokens::QUEST_TABLE,
        tokens::DEED_COUNTER,
        "${#0896E2F1} (${#053A16A2}/${#0059485C})",
    );
    m.text(
        tokens::QUEST_TABLE,
        tokens::MSG_NEW_QUEST,
        "Begun: ${#00569A84}",
    );
    m.text(
        tokens::QUEST_TABLE,
        tokens::MSG_NEW_DEED,
        "Deed: ${#00569A84}",
    );
    m.text(
        tokens::QUEST_TABLE,
        tokens::MSG_COMPLETED,
        "Ended: ${#00569A84}",
    );
    m.text(
        tokens::QUEST_TABLE,
        tokens::MSG_FAILED,
        "Lost: ${#00569A84}",
    );
    m.text(
        tokens::QUEST_TABLE,
        tokens::ABANDON_DEFAULT,
        "Drop ${#00569A84}?",
    );
    m.text(
        tokens::QUEST_TABLE,
        tokens::ABANDON_INSTANCE,
        "Leave ${#00569A84}?",
    );
    m.text(
        tokens::UI_QUEST_TABLE,
        tokens::OBJECTIVE_LABEL,
        "Step ${#053A16A2}",
    );
    m.text(
        tokens::UI_QUEST_TABLE,
        tokens::SHOWN_LABEL,
        "${#00484A34} of ${#0059485C} (${#00059E45})",
    );
    m.text(tokens::UI_QUEST_TABLE, tokens::MODE_ACTIVE, "underway");
    m.text(tokens::UI_QUEST_TABLE, tokens::MODE_COMPLETED, "finished");
    m.text(
        tokens::UI_QUEST_TABLE,
        tokens::COUNT_PAIR,
        "${#08A76A74}/${#00005168}",
    );
    m.text(
        tokens::UI_QUEST_TABLE,
        tokens::REPEAT_NEVER_DONE,
        "Never done",
    );
    m.text(
        tokens::UI_QUEST_TABLE,
        tokens::BACKGROUND_LABEL,
        "Background",
    );
    m
}

/// A simple quest: name, category, level, optional scope, one objective with a
/// description.
fn simple(m: &mut Mem, did: Did, name: &str, category: u32, level: i32, scope: Option<u32>) {
    let name_info = literal(name);
    let mut entries = vec![
        m.e("Quest_Name", si(name_info)),
        m.e("Quest_Category", PropertyValue::Enum(category)),
        m.e("Quest_ChallengeLevel", PropertyValue::Int(level)),
        m.e("Quest_RoleArray", arr(vec![])),
    ];
    let o = objective(m, 1, Some("Do it"), vec![]);
    entries.push(m.e("Quest_ObjectiveArray", arr(vec![o])));
    if let Some(s) = scope {
        entries.push(m.e("Quest_Scope", PropertyValue::Enum(s)));
    }
    m.put(did, entries);
}

/// An objective struct with completion conditions (each a list of event structs).
pub(crate) fn objective(
    m: &mut Mem,
    index: i32,
    description: Option<&str>,
    conditions: Vec<Vec<PropertyValue>>,
) -> PropertyEntry {
    let mut fields = vec![m.e("Quest_ObjectiveIndex", PropertyValue::Int(index))];
    if let Some(d) = description {
        fields.push(m.e("Quest_ObjectiveDescription", si(literal(d))));
    }
    let conds: Vec<PropertyEntry> = conditions
        .into_iter()
        .map(|events| {
            let evs = events
                .into_iter()
                .map(|e| m.e("QuestEvent_Entry", e))
                .collect();
            m.e("Quest_CompletionCondition", arr(evs))
        })
        .collect();
    fields.push(m.e("Quest_CompletionConditionArray", arr(conds)));
    m.e("Quest_CompletionObjective", st(fields))
}

fn event(m: &mut Mem, kind: u32, number: i32, progress: Option<StringInfo>) -> PropertyValue {
    let mut f = vec![
        m.e("QuestEvent_ID", PropertyValue::Enum(kind)),
        m.e("QuestEvent_Number", PropertyValue::Int(number)),
    ];
    if let Some(p) = progress {
        f.push(m.e("QuestEvent_ProgressOverride", si(p)));
    }
    st(f)
}

fn player(m: &Mem) -> PlayerVars {
    PlayerVars::new(
        m,
        PlayerContext {
            name: Some("Wren".into()),
            level: Some(10),
            ..PlayerContext::default()
        },
    )
}

fn plain(t: &Option<tessera_text::RichText>) -> String {
    t.as_ref().map(|t| t.plain_text()).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// §12.1 Log order, titles, filter, counts
// ---------------------------------------------------------------------------------------------

#[test]
fn log_order_titles_and_filter() {
    let mut m = base();
    let cat = m.p("Quest_Category");
    m.enum_texts.insert((cat, 5), "Beta".into());
    m.enum_texts.insert((cat, 7), "alpha".into());
    simple(&mut m, QA, "Wolves", 5, 12, None);
    simple(&mut m, QB, "apples", 5, 12, Some(1));
    simple(&mut m, QC, "Cart", 7, 3, None);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let entries: Vec<LogEntry> = [QA, QB, QC]
        .iter()
        .map(|d| p.log_entry(&RecordRef::Record(*d), &pl).expect("entry"))
        .collect();
    let with_state: Vec<(&LogEntry, Option<FeedState>)> = entries
        .iter()
        .map(|e| (e, Some(FeedState::Active)))
        .collect();
    let view = LogView::build(&with_state, LogMode::Active, &LogFilter::default());
    let cats: Vec<&str> = view.categories.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(cats, ["alpha", "Beta"]);
    assert_eq!(view.order(), [2, 1, 0]);
    assert_eq!(plain(&entries[1].title), "[12] apples");
    assert_eq!(plain(&entries[0].title), "[12] Wolves");
    assert_eq!(plain(&entries[2].title), "[3] Cart");
    assert_eq!((view.shown_rows, view.total_rows), (3, 3));
    let shown = p
        .shown_text(view.shown_rows, view.total_rows, LogMode::Active)
        .expect("renders");
    assert_eq!(plain(&shown), "3 of 3 (underway)");
    assert_eq!(view.first_shown(), Some(2));

    // Task filter alone: solo-only quests hide; B is a task (0x08 | 0x20).
    assert_eq!(entries[0].scope_bits, 0x20);
    assert_eq!(entries[1].scope_bits, 0x28);
    let task = LogFilter {
        mask: 0x08,
        name: String::new(),
    };
    let view = LogView::build(&with_state, LogMode::Active, &task);
    assert_eq!(view.shown_rows, 1);
    assert_eq!(view.first_shown(), Some(1));
    assert!(!view.categories[0].shown, "alpha has no shown row");

    // Name filter: lower-cased substring of the title.
    let named = LogFilter {
        mask: FULL_FILTER_MASK,
        name: "WOL".into(),
    };
    assert_eq!(
        LogView::build(&with_state, LogMode::Active, &named).shown_rows,
        1
    );

    // Completed mode lists only completed quests; deeds never.
    let view = LogView::build(&with_state, LogMode::Completed, &LogFilter::default());
    assert_eq!(view.total_rows, 0);
}

#[test]
fn category_fallback_and_counts() {
    let mut m = base();
    simple(&mut m, QA, "Lonely", 99, 0, None);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let e = p.log_entry(&RecordRef::Record(QA), &pl).expect("entry");
    assert_eq!(e.category_text, "(other)");
    assert_eq!(e.category_display, None);
    assert_eq!(e.level_source, LevelSource::None);
    assert_eq!(plain(&e.title), "Lonely", "level < 1: name alone");
    assert_eq!(plain(&p.count_text(4, Some(25)).expect("ok")), "4/25");
    assert_eq!(plain(&p.count_text(4, None).expect("ok")), "4");
    let lit = p
        .log_entry(&RecordRef::Literal("A literal".into()), &pl)
        .expect("literal");
    assert_eq!(lit.category_text, "(other)");
    assert!(lit.objectives.is_empty());
}

#[test]
fn level_from_player_override_and_hidden() {
    let mut m = base();
    simple(&mut m, QA, "Scaled", 1, 0, None);
    let ovr = m.p("Quest_ChallengeLevelOverrideProperty");
    let adv = m.p("Advancement_Level");
    let mut s = (*m.records[&QA]).clone();
    s.entries.push(PropertyEntry {
        pid: ovr,
        value: PropertyValue::PropertyId(adv),
    });
    m.records.insert(QA, Arc::new(s));
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let e = p.log_entry(&RecordRef::Record(QA), &pl).expect("entry");
    assert_eq!(e.level, 10, "Advancement_Level from the reported level");
    assert_eq!(e.level_source, LevelSource::Override { known: true });
    let unknown = PlayerVars::new(&m, PlayerContext::default());
    let e = p
        .log_entry(&RecordRef::Record(QA), &unknown)
        .expect("entry");
    assert_eq!(e.level_source, LevelSource::Override { known: false });
}

// ---------------------------------------------------------------------------------------------
// §12.2 Progress text and counters; §12.3 deed suffix
// ---------------------------------------------------------------------------------------------

fn bats_quest(m: &mut Mem) {
    let orla = m.e("Name", si(literal("Orla")));
    m.put(NPC_R, vec![orla]);
    let bats = m.info("Q", 0x100, "Defeat bats (${#053A16A2}/${#0059485C})");
    let talk = m.info("Q", 0x101, "Talk to ${#00005343}");
    let x = event(m, 22, 6, Some(bats));
    let mut y_fields = match event(m, 11, 1, Some(talk)) {
        PropertyValue::Struct(s) => s.fields,
        _ => unreachable!(),
    };
    y_fields.push(m.e("QuestEvent_NPCTalk", PropertyValue::DidRef(NPC_R)));
    let y = st(y_fields);
    let o1 = objective(m, 1, Some("First"), vec![]);
    let o2 = objective(m, 2, Some("Second"), vec![vec![x], vec![y]]);
    let entries = vec![
        m.e("Quest_Name", si(literal("Bats"))),
        m.e("Quest_RoleArray", arr(vec![])),
        m.e("Quest_ObjectiveArray", arr(vec![o2, o1])),
    ];
    m.put(QP, entries);
}

#[test]
fn progress_text_with_counters_and_alternatives() {
    let mut m = base();
    bats_quest(&mut m);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let set = p.record(QP).expect("quest");
    let reports = [ObjectiveReport {
        objective: 2,
        count: 4,
        total: 6,
        done: false,
    }];
    let t = p.progress(&set, Some(FeedState::Active), &reports, false, &pl);
    assert_eq!(t.objective, Some(2));
    assert_eq!(t.source, ProgressSource::Events);
    assert_eq!(t.plain(), "Defeat bats (4/6)\n~ or ~\nTalk to Orla");
    assert!(matches!(t.lines[1], ProgressLine::Alternative(_)));
    // Without reports: the lowest objective (no lines → its description).
    let t = p.progress(&set, Some(FeedState::Active), &[], false, &pl);
    assert_eq!(
        (t.objective, t.source),
        (Some(1), ProgressSource::Description)
    );
    assert_eq!(t.plain(), "First");
    // Without report for objective 2: count 0, total = QuestEvent_Number.
    let t = p.objective_progress(&set, 2, &[], false, &pl);
    assert!(t.plain().starts_with("Defeat bats (0/6)"));
    // Completed and failed words.
    assert_eq!(
        p.progress(&set, Some(FeedState::Completed), &reports, false, &pl)
            .plain(),
        "Done"
    );
    assert_eq!(
        p.progress(&set, Some(FeedState::Failed), &reports, false, &pl)
            .plain(),
        "Lost"
    );
    // Tracker budget with three entries: ⌊20 ÷ 3⌋ × 12.
    let metrics = TrackerMetrics {
        total_max_objectives: 20,
        size_per_objective: 12,
    };
    assert_eq!(tracker_line_budget(3, metrics), 72);
    assert_eq!(tracker_line_budget(1, metrics), 240);
    assert_eq!(tracker_line_budget(10, metrics), 24);
}

#[test]
fn objective_override_and_number_fallback() {
    let mut m = base();
    let ovr = m.info("Q", 0x200, "Gather ${#000C09A3} of ${#063E25B3}");
    let ev = event(&mut m, 22, 3, None);
    let ev2 = event(&mut m, 22, 2, None);
    let mut o = objective(&mut m, 1, None, vec![vec![ev, ev2]]);
    if let PropertyValue::Struct(s) = &mut o.value {
        s.fields.push(PropertyEntry {
            pid: m.p("Quest_ObjectiveProgressOverride"),
            value: si(ovr),
        });
    }
    let bare = objective(&mut m, 2, None, vec![]);
    let entries = vec![
        m.e("Quest_Name", si(literal("Gather"))),
        m.e("Quest_RoleArray", arr(vec![])),
        m.e("Quest_ObjectiveArray", arr(vec![o, bare])),
    ];
    m.put(QA, entries);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let set = p.record(QA).expect("quest");
    let t = p.objective_progress(&set, 1, &[], false, &pl);
    assert_eq!(t.source, ProgressSource::ObjectiveOverride);
    assert_eq!(
        t.plain(),
        "Gather 0 of 5",
        "total: sum of QuestEvent_Number"
    );
    let r = [ObjectiveReport {
        objective: 1,
        count: 2,
        total: 9,
        done: false,
    }];
    assert_eq!(
        p.objective_progress(&set, 1, &r, false, &pl).plain(),
        "Gather 2 of 9"
    );
    let t = p.objective_progress(&set, 2, &[], false, &pl);
    assert_eq!(
        (t.source, t.plain()),
        (ProgressSource::ObjectiveNumber, "Step 2".into())
    );
}

#[test]
fn deed_counter_suffix() {
    let mut m = base();
    let hills = m.info("Q", 0x300, "Explore the hills");
    let ev = event(&mut m, 22, 10, Some(hills));
    let o = objective(&mut m, 1, None, vec![vec![ev]]);
    let entries = vec![
        m.e("Quest_Name", si(literal("Hills"))),
        m.e("Quest_IsAccomplishment", PropertyValue::Bool(true)),
        m.e("Quest_RoleArray", arr(vec![])),
        m.e("Quest_ObjectiveArray", arr(vec![o])),
    ];
    m.put(DEED, entries);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let r = [ObjectiveReport {
        objective: 1,
        count: 3,
        total: 10,
        done: false,
    }];
    let t = p
        .tracker_entry(&RecordRef::Record(DEED), Some(FeedState::Active), &r, &pl)
        .expect("entry");
    assert_eq!(t.progress.plain(), "Explore the hills (3/10)");
    assert!(t.deed && !t.ready);
    let set = p.record(DEED).expect("deed");
    // The log's details pane does not ask for the suffix.
    assert_eq!(
        p.progress(&set, Some(FeedState::Active), &r, false, &pl)
            .plain(),
        "Explore the hills"
    );
    // A deed is never listed in the log.
    let e = p.log_entry(&RecordRef::Record(DEED), &pl).expect("entry");
    let view = LogView::build(
        &[(&e, Some(FeedState::Active))],
        LogMode::Active,
        &LogFilter::default(),
    );
    assert_eq!(view.total_rows, 0);
}

// ---------------------------------------------------------------------------------------------
// §5.1 Current objective, §6.4 ready
// ---------------------------------------------------------------------------------------------

#[test]
fn current_objective_rules() {
    let r = |objective, done| ObjectiveReport {
        objective,
        count: 0,
        total: 1,
        done,
    };
    let idx = [1, 2, 3];
    assert_eq!(current_objective(&idx, &[]), Some(1));
    assert_eq!(current_objective(&idx, &[r(1, true)]), Some(2));
    assert_eq!(current_objective(&idx, &[r(1, true), r(3, true)]), Some(3));
    assert_eq!(
        current_objective(&idx, &[r(1, false), r(2, false)]),
        Some(2)
    );
    assert_eq!(
        current_objective(&idx, &[r(9, false)]),
        Some(1),
        "unknown index ignored"
    );
    assert_eq!(current_objective(&[], &[]), None);
}

// ---------------------------------------------------------------------------------------------
// §8.2 Rings (§12.4)
// ---------------------------------------------------------------------------------------------

#[test]
fn ring_keys_follow_state_and_talk_roles() {
    let mut m = base();
    let role = vec![
        m.e("QuestDispenser_Action", PropertyValue::Enum(2)),
        m.e("QuestDispenser_NPC", PropertyValue::DidRef(NPC_N)),
        m.e("Quest_ObjectiveIndex", PropertyValue::Int(2)),
    ];
    let role = m.e("Quest_Role", st(role));
    let o1 = objective(&mut m, 1, Some("a"), vec![]);
    let o2 = objective(&mut m, 2, Some("b"), vec![]);
    let entries = vec![
        m.e("Quest_Name", si(literal("Ring"))),
        m.e("Quest_ExpTier", PropertyValue::Enum(2)),
        m.e("Quest_RoleArray", arr(vec![role])),
        m.e("Quest_ObjectiveArray", arr(vec![o1, o2])),
    ];
    m.put(RING_Q, entries);
    let p = QuestPresenter::new(&m);
    assert_eq!(
        p.ring_key(RING_Q, None, &[], Some(NPC_N)),
        Some("Quest_Available")
    );
    let done1 = [ObjectiveReport {
        objective: 1,
        count: 1,
        total: 1,
        done: true,
    }];
    assert_eq!(
        p.ring_key(RING_Q, Some(FeedState::Active), &done1, Some(NPC_N)),
        Some("Quest_Advance")
    );
    assert_eq!(
        p.ring_key(RING_Q, Some(FeedState::Active), &[], Some(NPC_N)),
        Some("Quest_InProgress"),
        "objective 1 is current; the talk role is for 2"
    );
    assert_eq!(
        p.ring_key(RING_Q, Some(FeedState::Completed), &done1, Some(NPC_N)),
        None
    );
    assert_eq!(
        p.ring_key(NPC_N, None, &[], None),
        None,
        "not a quest record"
    );
    // Variants.
    let set = p.record(RING_Q).expect("q");
    let mut hard = (*set).clone();
    let tier = m.p("Quest_ExpTier");
    let scope = m.p("Quest_Scope");
    hard.entries.retain(|e| e.pid != tier);
    hard.entries.push(PropertyEntry {
        pid: tier,
        value: PropertyValue::Enum(5),
    });
    hard.entries.push(PropertyEntry {
        pid: scope,
        value: PropertyValue::Enum(3),
    });
    m.records.insert(RING_Q, Arc::new(hard));
    let p = QuestPresenter::new(&m);
    assert_eq!(
        p.ring_key(RING_Q, None, &[], None),
        Some("Quest_Available_Epic_Hard")
    );
}

// ---------------------------------------------------------------------------------------------
// §7 Messages (§12.5)
// ---------------------------------------------------------------------------------------------

#[test]
fn state_and_objective_messages() {
    let mut m = base();
    let bb = m.info("Q", 0x400, "Bats: ${#000C09A3}/${#063E25B3}");
    let mut o1 = objective(&mut m, 1, Some("a"), vec![]);
    if let PropertyValue::Struct(s) = &mut o1.value {
        s.fields.push(PropertyEntry {
            pid: m.p("Quest_ObjectiveBillboardOverride"),
            value: si(bb),
        });
    }
    let entries = vec![
        m.e("Quest_Name", si(literal("Msg"))),
        m.e("Quest_RoleArray", arr(vec![])),
        m.e("Quest_ObjectiveArray", arr(vec![o1])),
    ];
    m.put(QA, entries);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let msg = p
        .state_message(&RecordRef::Record(QA), None, FeedState::Active, &pl)
        .expect("ok")
        .expect("posted");
    assert_eq!(msg.text.plain_text(), "Begun: Msg");
    assert_eq!((msg.chat_type, msg.mask), (21, 0x10_0000));
    assert!(
        p.state_message(
            &RecordRef::Record(QA),
            Some(FeedState::Active),
            FeedState::Active,
            &pl
        )
        .expect("ok")
        .is_none()
    );
    assert!(
        p.state_message(&RecordRef::Record(QA), None, FeedState::Removed, &pl)
            .expect("ok")
            .is_none()
    );
    let done = p
        .state_message(
            &RecordRef::Record(QA),
            Some(FeedState::Active),
            FeedState::Completed,
            &pl,
        )
        .expect("ok")
        .expect("posted");
    assert_eq!(done.text.plain_text(), "Ended: Msg");
    let r = [ObjectiveReport {
        objective: 1,
        count: 2,
        total: 5,
        done: false,
    }];
    let line = p
        .objective_message(QA, 1, &r, &pl)
        .expect("ok")
        .expect("posted");
    assert_eq!(line.text.plain_text(), "Bats: 2/5");
    let set = p.record(QA).expect("q");
    assert!(p.posts_objective_line(&set, 1, &r));
}

#[test]
fn hidden_deed_posts_nothing() {
    let mut m = base();
    let o = objective(&mut m, 1, None, vec![]);
    let entries = vec![
        m.e("Quest_Name", si(literal("Secret"))),
        m.e("Quest_IsAccomplishment", PropertyValue::Bool(true)),
        m.e("Quest_IsHidden", PropertyValue::Bool(true)),
        m.e("Quest_RoleArray", arr(vec![])),
        m.e("Quest_ObjectiveArray", arr(vec![o])),
    ];
    m.put(DEED, entries);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    assert!(
        p.state_message(&RecordRef::Record(DEED), None, FeedState::Active, &pl)
            .expect("ok")
            .is_none()
    );
}

// ---------------------------------------------------------------------------------------------
// §3 Journal state, §4.8 buttons, §6.2 tracker capacity
// ---------------------------------------------------------------------------------------------

#[test]
fn journal_state_tracking_and_buttons() {
    let mut j = QuestJournal::new();
    let q = |n: u32| RecordRef::Record(Did(0x7000_1000 + n));
    for n in 0..11 {
        let c = j.apply(&JournalEvent::State {
            quest: q(n),
            state: FeedState::Active,
        });
        assert_eq!(c.state, Some((None, FeedState::Active)));
        j.apply(&JournalEvent::Tracked {
            quest: q(n),
            tracked: true,
        });
    }
    assert_eq!(j.tracked_count(), 11);
    assert_eq!(j.tracked().len(), TRACKER_CAPACITY);
    assert_eq!(j.tracked()[0].quest, q(0));
    // Removed untracks and drops reports; the 11th is now shown.
    j.apply(&JournalEvent::Objective {
        quest: q(0),
        report: ObjectiveReport {
            objective: 1,
            count: 1,
            total: 2,
            done: false,
        },
    });
    let c = j.apply(&JournalEvent::State {
        quest: q(0),
        state: FeedState::Removed,
    });
    assert_eq!(c.state, Some((Some(FeedState::Active), FeedState::Removed)));
    assert!(
        j.get(&q(0))
            .is_some_and(|x| x.reports.is_empty() && !x.tracked)
    );
    assert_eq!(j.tracked().len(), 10);
    assert_eq!(j.tracked()[9].quest, q(10));
    // Objective reports: an unchanged count reports no change.
    let rep = |count| JournalEvent::Objective {
        quest: q(1),
        report: ObjectiveReport {
            objective: 2,
            count,
            total: 5,
            done: false,
        },
    };
    assert_eq!(j.apply(&rep(1)).objective, Some(2));
    assert_eq!(j.apply(&rep(1)).objective, None);
    assert_eq!(j.apply(&rep(2)).objective, Some(2));
    let bad = JournalEvent::Objective {
        quest: q(1),
        report: ObjectiveReport {
            objective: 2000,
            count: 0,
            total: 0,
            done: false,
        },
    };
    assert!(j.apply(&bad).ignored);
    // Completed marks completed_before.
    j.apply(&JournalEvent::State {
        quest: q(1),
        state: FeedState::Completed,
    });
    assert!(j.get(&q(1)).is_some_and(|x| x.completed_before));

    // Buttons.
    let mut m = base();
    simple(&mut m, QA, "Btn", 1, 5, None);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let e = p.log_entry(&RecordRef::Record(QA), &pl).expect("entry");
    let b = TrackButton::of(Some((&e, Some(FeedState::Active), false)), 3);
    assert_eq!(
        (b.label, b.ghosted, b.sends),
        (tokens::TRACK.0, false, true)
    );
    let b = TrackButton::of(Some((&e, Some(FeedState::Active), true)), 10);
    assert_eq!(
        (b.label, b.ghosted, b.sends),
        (tokens::UNTRACK.0, false, false)
    );
    assert!(TrackButton::of(Some((&e, Some(FeedState::Active), false)), 10).ghosted);
    assert!(TrackButton::of(Some((&e, Some(FeedState::Completed), false)), 0).ghosted);
    assert!(TrackButton::of(None, 0).ghosted);
    assert!(!AbandonButton::of(Some((&e, Some(FeedState::Active)))).ghosted);
    assert!(AbandonButton::of(Some((&e, Some(FeedState::Completed)))).ghosted);
    let conf = p
        .abandon_confirmation(&RecordRef::Record(QA), &pl)
        .expect("ok");
    assert_eq!(plain(&conf), "Drop Btn?");
    assert_eq!(marker_image(None), MarkerImage::QuestNote);
    assert_eq!(
        marker_image(Some(Did(0x4100_0001))),
        MarkerImage::Given(Did(0x4100_0001))
    );
}

// ---------------------------------------------------------------------------------------------
// §4.7 Details pane, §9 census, player variables
// ---------------------------------------------------------------------------------------------

#[test]
fn details_pane_sections_in_order() {
    let mut m = base();
    bats_quest(&mut m);
    let mut s = (*m.records[&QP]).clone();
    s.entries.push(PropertyEntry {
        pid: m.p("Quest_Description"),
        value: si(literal("Long ago")),
    });
    s.entries.push(PropertyEntry {
        pid: m.p("Quest_MaxTimesCompletable"),
        value: PropertyValue::Int(-1),
    });
    m.records.insert(QP, Arc::new(s));
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let d = p
        .details(
            &RecordRef::Record(QP),
            DetailsMode::Underway,
            &[],
            false,
            &pl,
        )
        .expect("details");
    let kinds: Vec<&str> = d
        .iter()
        .map(|s| match s {
            DetailsSection::Header(_) => "header",
            DetailsSection::Progress(_) => "progress",
            DetailsSection::ObjectiveText(_) => "objective",
            DetailsSection::Background { .. } => "background",
            DetailsSection::RepeatNote(_) => "repeat",
            DetailsSection::Rewards(_) => "rewards",
            DetailsSection::Bestowal { .. } => "bestowal",
            DetailsSection::PastObjective { .. } => "past",
        })
        .collect();
    // No treasure record: no reward row, so no rewards section (§4.7 item 6).
    assert_eq!(
        kinds,
        ["header", "progress", "objective", "background", "repeat"]
    );
    let d = p
        .details(
            &RecordRef::Record(QP),
            DetailsMode::Completed,
            &[],
            true,
            &pl,
        )
        .expect("details");
    let past = d
        .iter()
        .filter(|s| matches!(s, DetailsSection::PastObjective { .. }))
        .count();
    assert_eq!(past, 2);
    assert!(
        d.iter()
            .any(|s| matches!(s, DetailsSection::Bestowal { .. })),
        "the description is the bestowal line (rule 3)"
    );
}

#[test]
fn census_categories_and_flags() {
    let mut m = base();
    simple(&mut m, QA, "Fine", 1, 5, None);
    bats_quest(&mut m);
    let o = objective(&mut m, 1, None, vec![]);
    let deed = vec![
        m.e("Quest_Name", si(literal("D"))),
        m.e("Quest_IsAccomplishment", PropertyValue::Bool(true)),
        m.e("Quest_RoleArray", arr(vec![])),
        m.e("Quest_ObjectiveArray", arr(vec![o])),
    ];
    m.put(DEED, deed);
    let bad = vec![m.e("Name", si(literal("not a quest")))];
    m.put(QC, bad);
    let unnamed = vec![
        m.e("Quest_Name", si(literal(" "))),
        m.e("Quest_RoleArray", arr(vec![])),
        m.e("Quest_ObjectiveArray", arr(vec![])),
    ];
    m.put(QB, unnamed);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let c = census(&p, &[QA, QP, DEED, QC, QB, Did(0x7000_FFFF)], &pl);
    assert_eq!(c.records, 6);
    assert_eq!(c.count(QuestCategory::Shown), 2);
    assert_eq!(c.count(QuestCategory::Deed), 1);
    assert_eq!(c.count(QuestCategory::NoName), 1);
    assert_eq!(c.count(QuestCategory::Unreadable), 2);
    // QA: category without display text, no description, objective without lines.
    assert_eq!(c.flags.get(&QuestFlag::CategoryFallback), Some(&2));
    assert_eq!(c.flags.get(&QuestFlag::NoDescription), Some(&2));
    assert_eq!(c.flags.get(&QuestFlag::NoLevel), Some(&1));
    assert_eq!(c.flags.get(&QuestFlag::ObjectiveWithoutLines), Some(&2));
    assert_eq!(c.flags.get(&QuestFlag::UnsuppliedArgument), None);
    assert_eq!(c.flags.get(&QuestFlag::TextFailure), None);
    assert_eq!(c.objectives, 3);
}

#[test]
fn unsupplied_arguments_are_flagged() {
    let mut m = base();
    let odd = m.info("Q", 0x500, "Odd ${#01234567}");
    let o = objective(&mut m, 1, None, vec![]);
    let entries = vec![
        m.e("Quest_Name", si(literal("Odd"))),
        m.e("Quest_Description", si(odd)),
        m.e("Quest_RoleArray", arr(vec![])),
        m.e("Quest_ObjectiveArray", arr(vec![o])),
    ];
    m.put(QA, entries);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let out = classify(&p, QA, &pl);
    assert_eq!(out.category, QuestCategory::Shown);
    assert!(out.flags.contains(&QuestFlag::UnsuppliedArgument));
    assert!(
        out.flags.contains(&QuestFlag::NoDescription),
        "it does not render"
    );
}

#[test]
fn player_variables_fill_texts() {
    let mut m = base();
    let species = m.p("Agent_Species");
    let class = m.p("Agent_Class");
    m.enum_texts.insert((species, 65), "Hobbit-folk".into());
    m.enum_texts.insert((class, 3), "Minstrel".into());
    let pl = PlayerVars::new(
        &m,
        PlayerContext {
            name: Some("Wren".into()),
            species: Some(65),
            class: Some(3),
            level: Some(7),
            ..PlayerContext::default()
        },
    );
    let info = m.info(
        "Q",
        0x600,
        "${#03EC29B5} the ${#03EC68D5} ${#0EB8DCB3} (${#0050AA9C})${#08A72645}",
    );
    let r = m.render(TextRef::Info(&info), &pl.values).expect("ok");
    assert_eq!(plain(&r), "Wren the Hobbit-folk Minstrel (7)");
    for id in ids::PLAYER_NAME {
        assert!(is_player_variable(id));
    }
    assert!(is_counter(ids::EVENT_LEFT) && !is_player_variable(ids::EVENT_LEFT));
}

#[test]
fn difficulty_states_from_consideration_tables() {
    let mut m = base();
    let rank = m.p("ConsiderationControl_ConsiderRank");
    let state = m.p("ConsiderationControl_UIStateID");
    let rts = m.p("ConsiderationControl_RankToUIState");
    let list = m.p("ConsiderationControl_QuestTableList");
    let ranks = m.p("ConsiderationTable_RankPerDifferentialArray");
    let table = Did(0x7000_5000);
    let control = Did(0x7000_5001);
    // 31 ranks: 10 for d < 0, 50 for d = 0, 90 for d > 0.
    let ranks_v: Vec<PropertyEntry> = (0..31)
        .map(|i| PropertyEntry {
            pid: rank,
            value: PropertyValue::Enum(match i {
                0..15 => 10,
                15 => 50,
                _ => 90,
            }),
        })
        .collect();
    m.put(
        table,
        vec![PropertyEntry {
            pid: ranks,
            value: arr(ranks_v),
        }],
    );
    let map = |r: u32, s: u32| PropertyEntry {
        pid: rts,
        value: st(vec![
            PropertyEntry {
                pid: rank,
                value: PropertyValue::Enum(r),
            },
            PropertyEntry {
                pid: state,
                value: PropertyValue::Enum(s),
            },
        ]),
    };
    let tables: Vec<PropertyEntry> = (0..5)
        .map(|_| PropertyEntry {
            pid: list,
            value: PropertyValue::DidRef(table),
        })
        .collect();
    m.put(
        control,
        vec![
            PropertyEntry {
                pid: rts,
                value: arr(vec![map(10, 1), map(50, 2), map(90, 3)]),
            },
            PropertyEntry {
                pid: list,
                value: arr(tables),
            },
        ],
    );
    m.weenies.insert("ConsiderationControl".into(), control);
    m.ui.push((UiEnum::StateId, 1, "ConTrivial_ColorName".into()));
    m.ui.push((UiEnum::StateId, 2, "ConEven_ColorName".into()));
    m.ui.push((UiEnum::StateId, 3, "ConImpossible_ColorName".into()));
    let p = QuestPresenter::new(&m);
    assert_eq!(p.difficulty_state(5, Some(3)), "ConImpossible_ColorName");
    assert_eq!(p.difficulty_state(1, Some(3)), "ConTrivial_ColorName");
    assert_eq!(p.difficulty_state(0, Some(3)), "ConEven_ColorName");
    assert_eq!(
        p.difficulty_state(90, Some(200)),
        "ConTrivial_ColorName",
        "clamped"
    );
    assert_eq!(p.difficulty_state(5, None), EVEN_STATE);
}

// ---------------------------------------------------------------------------------------------
// Review fixes: billboard values, journal changes, literals, bounds
// ---------------------------------------------------------------------------------------------

/// An event struct with a billboard override and no progress override.
fn billboard_event(m: &mut Mem, kind: u32, text: StringInfo, npc: Option<Did>) -> PropertyValue {
    let mut f = vec![
        m.e("QuestEvent_ID", PropertyValue::Enum(kind)),
        m.e("QuestEvent_Number", PropertyValue::Int(10)),
        m.e("QuestEvent_BillboardProgressOverride", si(text)),
    ];
    if let Some(n) = npc {
        f.push(m.e("QuestEvent_NPCTalk", PropertyValue::DidRef(n)));
    }
    st(f)
}

#[test]
fn event_billboard_lines_carry_world_value_and_npc_name() {
    let mut m = base();
    let orla = m.e("Name", si(literal("Orla")));
    m.put(NPC_R, vec![orla]);
    let world = m.info("Q", 0x500, "Lanterns lit: ${#005A6195}");
    let talk = m.info("Q", 0x501, "Speak with ${#00005343}");
    let e34 = billboard_event(&mut m, 34, world, None);
    let e11 = billboard_event(&mut m, 11, talk, Some(NPC_R));
    let o1 = objective(&mut m, 1, None, vec![vec![e34]]);
    let o2 = objective(&mut m, 2, None, vec![vec![e11]]);
    let entries = vec![
        m.e("Quest_Name", si(literal("Lights"))),
        m.e("Quest_RoleArray", arr(vec![])),
        m.e("Quest_ObjectiveArray", arr(vec![o1, o2])),
    ];
    m.put(QA, entries);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let r = [ObjectiveReport {
        objective: 1,
        count: 7,
        total: 10,
        done: false,
    }];
    let line = p
        .objective_message(QA, 1, &r, &pl)
        .expect("ok")
        .expect("type 34 billboard posts");
    assert_eq!(line.text.plain_text(), "Lanterns lit: 7");
    let r = [
        ObjectiveReport {
            objective: 1,
            count: 10,
            total: 10,
            done: true,
        },
        ObjectiveReport {
            objective: 2,
            count: 0,
            total: 1,
            done: false,
        },
    ];
    let line = p
        .objective_message(QA, 2, &r, &pl)
        .expect("ok")
        .expect("type 11 billboard posts");
    assert_eq!(line.text.plain_text(), "Speak with Orla");
    // A report naming an index the record lacks posts nothing.
    let set = p.record(QA).expect("q");
    assert!(!p.posts_objective_line(&set, 9, &r));
    assert!(p.posts_objective_line(&set, 2, &r));
}

#[test]
fn journal_first_reports_removal_and_report_bound() {
    let mut j = QuestJournal::new();
    let q = RecordRef::Record(QA);
    let rep = |objective, count, done| JournalEvent::Objective {
        quest: q.clone(),
        report: ObjectiveReport {
            objective,
            count,
            total: 5,
            done,
        },
    };
    // A first report equal to "no report" (0, not done) changes nothing (§7.2).
    assert_eq!(j.apply(&rep(1, 0, false)).objective, None);
    assert_eq!(j.apply(&rep(1, 1, false)).objective, Some(1));
    assert_eq!(j.apply(&rep(2, 0, true)).objective, Some(2));
    // Removed of a tracked quest reports the untracking.
    j.apply(&JournalEvent::Tracked {
        quest: q.clone(),
        tracked: true,
    });
    let c = j.apply(&JournalEvent::State {
        quest: q.clone(),
        state: FeedState::Removed,
    });
    assert_eq!(c.tracked, Some(false));
    assert_eq!(j.tracked_count(), 0);
    // Removed of an untracked quest reports no tracking change.
    let c = j.apply(&JournalEvent::State {
        quest: q.clone(),
        state: FeedState::Removed,
    });
    assert_eq!(c, JournalChange::default());

    // The total report bound.
    let mut j = QuestJournal::new();
    let per = usize::from(MAX_OBJECTIVE_INDEX) + 1;
    let quests = MAX_JOURNAL_REPORTS / per + 1;
    let mut ignored = 0usize;
    for n in 0..quests {
        let quest = RecordRef::Record(Did(0x7001_0000 + u32::try_from(n).expect("small")));
        for objective in 0..=MAX_OBJECTIVE_INDEX {
            let c = j.apply(&JournalEvent::Objective {
                quest: quest.clone(),
                report: ObjectiveReport {
                    objective,
                    count: 1,
                    total: 1,
                    done: false,
                },
            });
            ignored += usize::from(c.ignored);
        }
    }
    let kept: usize = j.quests().iter().map(|x| x.reports.len()).sum();
    assert_eq!(kept, MAX_JOURNAL_REPORTS);
    assert_eq!(ignored, quests * per - MAX_JOURNAL_REPORTS);
    assert_eq!(j.ignored_events(), u64::try_from(ignored).expect("fits"));
    // Removing a quest frees its reports for others.
    let first = j.quests()[0].quest.clone();
    j.apply(&JournalEvent::State {
        quest: first,
        state: FeedState::Removed,
    });
    let last = j.quests().last().expect("quests").quest.clone();
    let c = j.apply(&JournalEvent::Objective {
        quest: last,
        report: ObjectiveReport {
            objective: MAX_OBJECTIVE_INDEX,
            count: 1,
            total: 1,
            done: false,
        },
    });
    assert!(!c.ignored);
}

#[test]
fn literal_quests_have_words_messages_and_confirmation() {
    let m = base();
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let lit = RecordRef::Literal("An errand".into());
    let t = p
        .tracker_entry(&lit, Some(FeedState::Completed), &[], &pl)
        .expect("literal");
    assert_eq!(
        (t.progress.plain(), t.progress.source),
        ("Done".to_owned(), ProgressSource::Completed)
    );
    let t = p
        .tracker_entry(&lit, Some(FeedState::Failed), &[], &pl)
        .expect("literal");
    assert_eq!(t.progress.plain(), "Lost");
    let t = p
        .tracker_entry(&lit, Some(FeedState::Active), &[], &pl)
        .expect("literal");
    assert!(t.progress.lines.is_empty());
    let msg = |prev, state| {
        p.state_message(&lit, prev, state, &pl)
            .expect("ok")
            .map(|m| m.text.plain_text())
    };
    assert_eq!(
        msg(None, FeedState::Active),
        Some("Begun: An errand".into())
    );
    assert_eq!(
        msg(Some(FeedState::Active), FeedState::Completed),
        Some("Ended: An errand".into())
    );
    assert_eq!(
        msg(Some(FeedState::Active), FeedState::Failed),
        Some("Lost: An errand".into())
    );
    assert_eq!(msg(Some(FeedState::Active), FeedState::Removed), None);
    assert_eq!(msg(Some(FeedState::Active), FeedState::Active), None);
    let conf = p.abandon_confirmation(&lit, &pl).expect("ok");
    assert_eq!(plain(&conf), "Drop An errand?");
}

#[test]
fn lower_uses_the_simple_mapping() {
    assert_eq!(lower("\u{130}STANBUL"), "istanbul");
    assert_eq!(lower("\u{130}"), "i");
    assert_eq!(lower("ÄBC ẞ"), "äbc ß");
}

#[test]
fn progress_text_stays_within_the_line_bound() {
    let mut m = base();
    let line = m.info("Q", 0x600, "Find a stone");
    let conditions: Vec<Vec<PropertyValue>> = (0..300)
        .map(|_| vec![event(&mut m, 22, 1, Some(line.clone()))])
        .collect();
    let o = objective(&mut m, 1, None, conditions);
    let entries = vec![
        m.e("Quest_Name", si(literal("Stones"))),
        m.e("Quest_RoleArray", arr(vec![])),
        m.e("Quest_ObjectiveArray", arr(vec![o])),
    ];
    m.put(QA, entries);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let set = p.record(QA).expect("q");
    let t = p.objective_progress(&set, 1, &[], false, &pl);
    assert_eq!(t.source, ProgressSource::Events);
    assert!(t.lines.len() <= MAX_PROGRESS_LINES, "{}", t.lines.len());
    assert!(matches!(t.lines.last(), Some(ProgressLine::Text(_))));
    assert!(matches!(t.lines[1], ProgressLine::Alternative(_)));
}

#[test]
fn header_counts_and_task_limiter() {
    let mut m = base();
    let max = m.p("Quest_MaxActiveQuests");
    let limit_pid = m.p("Quest_TaskLimit");
    simple(&mut m, QA, "One", 112, 5, None);
    simple(&mut m, QB, "Two", 112, 5, None);
    simple(&mut m, QC, "Three", 7, 5, None);
    let p = QuestPresenter::new(&m);
    let pl = player(&m);
    let e: Vec<LogEntry> = [QA, QB, QC]
        .iter()
        .map(|&d| p.log_entry(&RecordRef::Record(d), &pl).expect("entry"))
        .collect();
    let with = [
        (&e[0], Some(FeedState::Active)),
        (&e[1], Some(FeedState::Completed)),
        (&e[2], Some(FeedState::Active)),
    ];
    assert_eq!(quests_underway(&with), 2);
    let limiter = TaskLimiter {
        category: 112,
        property: Some(limit_pid),
        default_limit: Some(5),
    };
    assert_eq!(limiter.count(&with), 1);
    let mut ctx = PlayerContext::default();
    assert_eq!(limiter.limit(&ctx), Some(5));
    assert_eq!(p.quest_limit(&ctx), None);
    ctx.properties.insert(limit_pid, 8);
    ctx.properties.insert(max, 25);
    assert_eq!(limiter.limit(&ctx), Some(8));
    assert_eq!(p.quest_limit(&ctx), Some(25));
    assert_eq!(
        plain(&p.count_text(1, limiter.limit(&ctx)).expect("ok")),
        "1/8"
    );
}

#[test]
fn layout_errors_name_their_category() {
    let m = base();
    assert_eq!(
        layout_element(&m, "nowhere", "Thing").err(),
        Some(LayoutError::LayoutMissing {
            layout: "nowhere".into()
        })
    );
    let e: String = LayoutError::ElementMissing {
        layout: "quest".into(),
        element: "QuestTree".into(),
        detail: "",
    }
    .into();
    assert_eq!(e, "ElementMissing: quest/QuestTree");
}
