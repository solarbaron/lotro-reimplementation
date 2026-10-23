// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-gamedata` presenters over hostile property data: the item and combat tooltip
//! engine (`item-tooltips.md`, `skill-tooltips.md`), the quest journal presenter
//! (`quest-journal.md`), the crafting and progression presenters (`crafting-panels.md`,
//! `progression-panels.md`) and the map readers (`map-windows.md`: `GameMap::from_struct`,
//! `game_map_of`, `MapTree::from_value`, `NoteIcons::read`, `region_map_list`).
//!
//! The input builds an in-memory install: a few object records (property sets of any
//! shape, with any value types under any PID), dictionary defaults, root classes and
//! enum texts. Every property name maps to one of a few PIDs, so each presenter finds its
//! own fields filled with whatever the input put there (a struct where an integer was
//! expected, references to records that are missing or that refer back to themselves,
//! non-finite floats, deep nesting). The rest of the input picks the calls' arguments.
//! Nothing may panic, loop without end or allocate without bound.

#![no_main]

use std::collections::HashMap;
use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyStruct};
use tessera_formats::reader::Utf16String;
use tessera_formats::ui_layout::UiEnum;
use tessera_formats::{
    Did, PropertyNames, PropertySet, PropertyType, PropertyValue, StringInfo, StringSource, TSize,
    UiLayout,
};
use tessera_gamedata::crafting::{CraftingPresenter, CraftingState, PageControls, PlayerCraft};
use tessera_gamedata::map::{GameMap, MapPids, MapTree, NoteIcons, game_map_of, region_map_list};
use tessera_gamedata::progression::{
    ProgressionPresenter, ProgressionState, TrainerEntryIn, TrainerOffer,
};
use tessera_gamedata::quest::{
    DetailsMode, FeedState, ObjectiveReport, PlayerContext, PresentationData, QuestPresenter,
    RecordRef,
};
use tessera_gamedata::tooltip::{
    CombatSubject, Purpose, TextRef, TextResult, TooltipData, TooltipEngine, TooltipRequest, Viewer,
};
use tessera_text::{Message, RenderOptions, Value, Values};

/// PIDs every property name maps to (1..=PIDS).
const PIDS: u32 = 40;
/// Object records of one input.
const MAX_RECORDS: usize = 8;
/// First record DID (a quest/object-like prefix; the presenters do not check it).
const RECORD_BASE: u32 = 0x7000_0100;
/// Most entries of one property set or struct.
const MAX_ENTRIES: u8 = 24;
/// Nesting limit of generated values.
const MAX_DEPTH: usize = 12;
/// Values per input.
const MAX_NODES: usize = 4_000;

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
    nodes: usize,
}

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos = self.pos.saturating_add(1);
        b
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.u8(), self.u8()])
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    fn done(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn pid(&mut self) -> u32 {
        1 + u32::from(self.u8()) % PIDS
    }

    /// A DID that is usually one of the input's records (or just past them).
    fn did(&mut self) -> Did {
        let b = self.u8();
        if b < 0xF0 {
            Did(RECORD_BASE + u32::from(b) % (MAX_RECORDS as u32 + 2))
        } else {
            Did(self.u32())
        }
    }

    fn text(&mut self) -> String {
        let n = usize::from(self.u8() % 12);
        (0..n).map(|_| char::from(b' ' + self.u8() % 95)).collect()
    }

    fn info(&mut self) -> StringInfo {
        let source = if self.u8() & 1 == 0 {
            let t = self.text();
            StringSource::Literal(Utf16String(t.encode_utf16().collect()))
        } else {
            StringSource::Table {
                token: self.u32(),
                table: Did(0x2500_0000 + u32::from(self.u8() % 4)),
            }
        };
        StringInfo { source, args: None }
    }

    fn entries(&mut self, depth: usize) -> Vec<PropertyEntry> {
        let n = self.u8() % MAX_ENTRIES;
        let mut out: Vec<PropertyEntry> = Vec::new();
        for _ in 0..n {
            let pid = self.pid();
            let value = self.value(depth + 1);
            if !out.iter().any(|e| e.pid == pid) {
                out.push(PropertyEntry { pid, value });
            }
        }
        out
    }

    fn value(&mut self, depth: usize) -> PropertyValue {
        self.nodes += 1;
        let tag = self.u8();
        if depth > MAX_DEPTH || self.nodes > MAX_NODES || self.done() {
            return PropertyValue::Int(i32::from(tag));
        }
        match tag % 20 {
            0 | 1 => {
                let fields = self.entries(depth);
                PropertyValue::Struct(PropertyStruct {
                    size: TSize {
                        count: u32::try_from(fields.len()).unwrap_or(0),
                        ..TSize::default()
                    },
                    fields,
                })
            }
            2 | 3 => {
                let n = self.u8() % MAX_ENTRIES;
                let mut elements = Vec::new();
                for _ in 0..n {
                    let pid = self.pid();
                    let value = self.value(depth + 1);
                    elements.push(PropertyEntry { pid, value });
                }
                PropertyValue::Array(PropertyArray {
                    count: u32::try_from(elements.len()).unwrap_or(0),
                    elements,
                })
            }
            4 => PropertyValue::Int(self.u32() as i32),
            5 => PropertyValue::Enum(self.u32() % 16),
            6 => PropertyValue::Float(f32::from_bits(self.u32())),
            7 | 8 => PropertyValue::DidRef(self.did()),
            9 => PropertyValue::Bool(self.u8() & 1 == 1),
            10 => PropertyValue::Bitfield64(u64::from(self.u32()) << 32 | u64::from(self.u32())),
            11 => PropertyValue::Bitfield32(self.u32()),
            12 => PropertyValue::Int64(i64::from(self.u32() as i32) * i64::from(self.u16())),
            13 => PropertyValue::StringInfo(Box::new(self.info())),
            14 => PropertyValue::String(self.text()),
            15 => PropertyValue::PropertyId(self.pid()),
            16 => PropertyValue::Colour(self.u32().to_le_bytes()),
            17 => PropertyValue::Vector3([
                f32::from_bits(self.u32()),
                f32::from_bits(self.u32()),
                f32::from_bits(self.u32()),
            ]),
            18 => PropertyValue::StringToken(self.u32()),
            _ => PropertyValue::Int(i32::from(self.u8() % 8) - 1),
        }
    }
}

fn hash(s: &str) -> u32 {
    s.bytes().fold(0x811C_9DC5u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

/// The in-memory install of one input.
#[derive(Default)]
struct FuzzData {
    records: HashMap<Did, Arc<PropertySet>>,
    defaults: HashMap<u32, PropertyValue>,
    classes: HashMap<Did, u32>,
    enum_mask: u8,
}

impl FuzzData {
    fn record_did(&self, label: &str) -> Option<Did> {
        let k = hash(label) % (MAX_RECORDS as u32 + 2);
        let d = Did(RECORD_BASE + k);
        self.records.contains_key(&d).then_some(d)
    }

    fn plain(&self, info: &StringInfo) -> Result<String, String> {
        match &info.source {
            StringSource::Literal(s) => Ok(String::from_utf16_lossy(&s.0)),
            StringSource::Table { token, .. } if token % 7 == 0 => Err(format!("miss {token:#x}")),
            StringSource::Table { token, .. } => Ok(format!("t{token:x}")),
        }
    }
}

impl PropertyNames for FuzzData {
    fn pid_of(&self, name: &str) -> Option<u32> {
        self.pid(name)
    }
}

impl TooltipData for FuzzData {
    fn pid(&self, name: &str) -> Option<u32> {
        Some(1 + hash(name) % PIDS)
    }
    fn property_name(&self, pid: u32) -> Option<String> {
        (1..=PIDS).contains(&pid).then(|| format!("P{pid}"))
    }
    fn property_type(&self, _pid: u32) -> Option<PropertyType> {
        None
    }
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        self.defaults.get(&pid).cloned()
    }
    fn enum_text(&self, pid: u32, key: u32) -> Option<String> {
        ((pid ^ key) & u32::from(self.enum_mask) == 0).then(|| format!("e{key}"))
    }
    fn enum_key(&self, pid: u32, name: &str) -> Option<u32> {
        (pid & 1 == 0).then(|| hash(name) % 16)
    }
    fn record(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String> {
        if did.0 == RECORD_BASE + MAX_RECORDS as u32 + 1 {
            return Err("unreadable".into());
        }
        Ok(self.records.get(&did).cloned())
    }
    fn weenie(&self, label: &str) -> Option<Did> {
        self.record_did(label)
    }
    fn layout(&self, _did: Did) -> Result<Option<Arc<UiLayout>>, String> {
        Ok(None)
    }
    fn layout_by_label(&self, _label: &str) -> Option<Did> {
        None
    }
    fn ui_label(&self, _which: UiEnum, key: u32) -> Option<String> {
        key.is_multiple_of(3).then(|| format!("u{key}"))
    }
    fn ui_key(&self, _which: UiEnum, name: &str) -> Option<u32> {
        Some(hash(name) % 64)
    }
    fn text_value(&self, info: &StringInfo) -> Result<Value, String> {
        self.plain(info).map(Value::Text)
    }
    fn render(&self, text: TextRef<'_>, values: &Values) -> TextResult {
        let s = match text {
            TextRef::Info(info) => self.plain(info)?,
            TextRef::Token { table_key, token } => format!("k{table_key:x}.{token:x}"),
        };
        if s.is_empty() {
            return Ok(None);
        }
        let r = tessera_text::render(&Message::literal(&s), values, &RenderOptions::default());
        Ok(Some(r.text))
    }
    fn set_of(&self, item: Did) -> Option<Did> {
        self.classes
            .get(&item)
            .map(|c| Did(RECORD_BASE + c % MAX_RECORDS as u32))
    }
    fn root_class(&self, did: Did) -> Option<u32> {
        self.classes.get(&did).copied()
    }
    fn class_lineage(&self, class: u32) -> Vec<u32> {
        vec![class, class / 2, 0]
    }
    fn enum_label(&self, pid: u32, key: u32) -> Option<String> {
        (!pid.is_multiple_of(3)).then(|| format!("L{key}"))
    }
    fn unique(&self, label: &str) -> Option<Did> {
        self.record_did(label)
    }
    fn properties_with_prefix(&self, prefix: &str) -> Vec<(u32, String)> {
        (0..3)
            .map(|i| (1 + (hash(prefix) + i) % PIDS, format!("{prefix}{i}")))
            .collect()
    }
}

impl PresentationData for FuzzData {
    fn table(&self, label: &str) -> Option<Did> {
        Some(Did(0x2500_0000 + hash(label) % 4))
    }
    fn text_args(&self, text: TextRef<'_>) -> Result<Vec<u32>, String> {
        match text {
            TextRef::Info(info) => self
                .plain(info)
                .map(|s| s.bytes().take(3).map(u32::from).collect()),
            TextRef::Token { token, .. } => Ok(vec![token % 5]),
        }
    }
    fn enum_name(&self, pid: u32, key: u32) -> Option<String> {
        (pid & 2 == 0).then(|| format!("N{key}"))
    }
}

fn build(b: &mut Bytes<'_>) -> FuzzData {
    let mut d = FuzzData {
        enum_mask: b.u8() & 3,
        ..FuzzData::default()
    };
    let n = usize::from(b.u8()) % (MAX_RECORDS + 1);
    for i in 0..n {
        let entries = b.entries(0);
        let set = PropertySet {
            size: TSize {
                count: u32::try_from(entries.len()).unwrap_or(0),
                ..TSize::default()
            },
            entries,
        };
        let did = Did(RECORD_BASE + i as u32);
        d.records.insert(did, Arc::new(set));
        if b.u8() & 1 == 1 {
            d.classes.insert(did, u32::from(b.u16()));
        }
    }
    for _ in 0..b.u8() % 6 {
        let pid = b.pid();
        let v = b.value(MAX_DEPTH - 2);
        d.defaults.insert(pid, v);
    }
    d
}

fn reports(b: &mut Bytes<'_>) -> Vec<ObjectiveReport> {
    (0..b.u8() % 6)
        .map(|_| ObjectiveReport {
            objective: b.u16() % 8,
            count: b.u32(),
            total: b.u32(),
            done: b.u8() & 1 == 1,
        })
        .collect()
}

fn tooltips(data: &FuzzData, dids: &[Did], b: &mut Bytes<'_>) {
    let engine = TooltipEngine::new(data);
    let viewer = Viewer::default();
    for &did in dids {
        let _ = engine.classify(did, &viewer);
        let _ = engine.build(&TooltipRequest::hover(did), &viewer);
        let _ = engine.classify_combat(&CombatSubject::Skill(did));
        let _ = engine.classify_combat(&CombatSubject::Effect(did));
        if let Ok(Some(set)) = data.record(did) {
            let _ = engine.pair(&set, Purpose::Hover);
            let _ = engine.skill_item_level(&set);
            let _ = engine.effect_duration(&set, i32::from(b.u8()) - 8);
        }
    }
}

fn quests(data: &FuzzData, dids: &[Did], b: &mut Bytes<'_>) {
    let q = QuestPresenter::new(data);
    let context = PlayerContext {
        level: Some(i32::from(b.u8()) - 4),
        class: Some(u32::from(b.u8() % 8)),
        ..PlayerContext::default()
    };
    let player = q.player(context.clone());
    let reports = reports(b);
    let state = [
        None,
        Some(FeedState::Active),
        Some(FeedState::Completed),
        Some(FeedState::Failed),
        Some(FeedState::Removed),
    ];
    for &did in dids {
        let r = RecordRef::Record(did);
        let _ = q.log_entry(&r, &player);
        for mode in [
            DetailsMode::Underway,
            DetailsMode::Completed,
            DetailsMode::History,
        ] {
            let _ = q.details(&r, mode, &reports, b.u8() & 1 == 1, &player);
        }
        let s = state[usize::from(b.u8()) % state.len()];
        let _ = q.tracker_entry(&r, s, &reports, &player);
        let _ = q.abandon_confirmation(&r, &player);
        if let Ok(Some(set)) = data.record(did) {
            let _ = q.scope_bits(&set);
            let (level, _) = q.level(&set, &context);
            let _ = q.name(&set, &player);
            let _ = q.title(&set, level, &player);
            let _ = q.progress(&set, s, &reports, b.u8() & 1 == 1, &player);
            for index in q.objective_indices(&set).into_iter().take(8) {
                let _ = q.objective_progress(&set, index, &reports, true, &player);
                let _ = q.objective_description(&set, index, &player);
            }
            let _ = q.ready(&set, s, &reports);
        }
        let _ = q.arc_name(did, &player);
    }
    let _ = q.log_entry(&RecordRef::Literal(b.text()), &player);
    let _ = q.category_text(b.u32() % 64);
    let _ = q.difficulty_state(i32::from(b.u8()), Some(i32::from(b.u8())));
    let _ = q.count_text(usize::from(b.u8()), Some(i64::from(b.u8())));
}

fn crafting(data: &FuzzData, dids: &[Did], b: &mut Bytes<'_>) {
    let c = CraftingPresenter::new(data);
    let state = CraftingState::default();
    let mut player = PlayerCraft::default();
    for &did in dids.iter().take(4) {
        player.holdings.insert(did.0, u64::from(b.u16()));
    }
    player.tool_bits = u64::from(b.u32());
    for &did in dids {
        let r = RecordRef::Record(did);
        let _ = c.recipe(&r);
        let _ = c.profession(did);
        let page = c.page(&r, &state, &player);
        let _ = page.filtered(&b.text());
        let _ = page.recipe_rows();
        let _ = c.recipe_page(&r, b.u16() % 16, &state, &player, &PageControls::default());
        let _ = c.profession_name(&r);
        if let Ok(Some(set)) = data.record(did) {
            let _ = c.scroll_rows(&set);
            let _ = c.gathering_node(did, &set);
            let _ = c.hotspot_effects(&set);
            let _ = c.facility_hotspot(did, &set);
        }
    }
    let _ = c.tier_display(b.u16());
    let _ = c.category_display(b.u32());
    let _ = c.meter_text(b.u32(), b.u32());
    let _ = CraftingPresenter::duration_text(f32::from_bits(b.u32()));
}

fn progression(data: &FuzzData, dids: &[Did], b: &mut Bytes<'_>) {
    let p = ProgressionPresenter::new(data);
    let state = ProgressionState::default();
    let mut entries = Vec::new();
    for (k, &did) in dids.iter().enumerate() {
        let _ = p.trait_tooltip(did, b.u16() % 12);
        let _ = p.tree(did);
        let _ = p.tree_view(did, &state);
        entries.push(TrainerEntryIn {
            entry: u16::try_from(k + 1).unwrap_or(u16::MAX),
            what: RecordRef::Record(did),
            coin: u64::from(b.u32()),
            level: u32::from(b.u8()),
            rank: (b.u8() & 1 == 1).then(|| b.u16() % 12),
            trained: b.u8() & 1 == 1,
            unmet: b.u8() & 1 == 1,
        });
    }
    for offer in [
        TrainerOffer::Skills,
        TrainerOffer::Traits,
        TrainerOffer::Recipes,
    ] {
        let _ = p.trainer_rows(offer, &entries);
    }
}

fn maps(data: &FuzzData, dids: &[Did], b: &mut Bytes<'_>) {
    let pids = MapPids::resolve(data);
    for &did in dids {
        let Ok(Some(set)) = data.record(did) else {
            continue;
        };
        if let Some(m) = game_map_of(&*set, &pids)
            && let Some(g) = m.georef()
        {
            let _ = g.to_pixel(f32::from_bits(b.u32()), f32::from_bits(b.u32()));
            let _ = g.to_world(f64::from(b.u32() as i32), f64::from(b.u32() as i32));
        }
        let icons = NoteIcons::read(&set, &pids);
        let _ = icons.get(u64::from(b.u32()));
        let _ = icons.for_bit(b.u32() % 70);
        let _ = region_map_list(&set, &pids);
        for e in set.entries.iter().take(8) {
            if let PropertyValue::Struct(s) = &e.value {
                let _ = GameMap::from_struct(s, &pids).georef();
            }
            if let Ok(t) = MapTree::from_value(&e.value, &pids) {
                let _ = t.node_for_area(b.did());
                let _ = t.node_by_element(b.u32());
                let n = t.nodes.len();
                if n > 0 {
                    let a = usize::from(b.u8()) % n;
                    let c = usize::from(b.u8()) % n;
                    let _ = t.is_within(a, c);
                    let _ = t.nodes.get(a).map(|node| node.display_mask());
                }
            }
        }
    }
}

fuzz_target!(|input: &[u8]| {
    let mut b = Bytes {
        data: input,
        pos: 0,
        nodes: 0,
    };
    let data = build(&mut b);
    let mut dids: Vec<Did> = data.records.keys().copied().collect();
    dids.sort_unstable();
    // A missing record and an unreadable one.
    dids.push(Did(RECORD_BASE + MAX_RECORDS as u32));
    dids.push(Did(RECORD_BASE + MAX_RECORDS as u32 + 1));
    match b.u8() % 6 {
        0 => tooltips(&data, &dids, &mut b),
        1 => quests(&data, &dids, &mut b),
        2 => crafting(&data, &dids, &mut b),
        3 => progression(&data, &dids, &mut b),
        4 => maps(&data, &dids, &mut b),
        _ => {
            tooltips(&data, &dids, &mut b);
            quests(&data, &dids, &mut b);
            crafting(&data, &dids, &mut b);
            progression(&data, &dids, &mut b);
            maps(&data, &dids, &mut b);
        }
    }
});
