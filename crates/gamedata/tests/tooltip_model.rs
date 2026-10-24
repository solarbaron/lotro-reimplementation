// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the item tooltip model (`item-tooltips.md` §3–§8, §11, §14) over an
//! in-memory [`TooltipData`]: our own layouts, records and texts, with invented labels,
//! IDs and wording. No game data.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyStruct};
use tessera_formats::reader::Utf16String;
use tessera_formats::ui_layout::{Rect, StateBlock, UiEnum};
use tessera_formats::{
    Did, ElementDesc, PropertySet, PropertyType, PropertyValue, StringInfo, StringSource, UiLayout,
};
use tessera_gamedata::tooltip::rules::tokens;
use tessera_gamedata::tooltip::{
    Category, CompareKind, Condition, Equipped, Instance, Purpose, RowContent, Subject,
    TemplateRef, TextRef, TextResult, TooltipData, TooltipEngine, TooltipError, TooltipRequest,
    Viewer, Widget,
};
use tessera_text::{Rgba, RichText, Style, Value, Values};

// Layouts, tables and records of the synthetic install.
const L_EXAM: Did = Did(0x2200_0000);
const L_BASE: Did = Did(0x2200_0001);
const L_TEXT: Did = Did(0x2200_0002);
const L_MISSING: Did = Did(0x2200_0FFF);
const T_FORMATS: Did = Did(0x2500_0001);

const ITEM: Did = Did(0x7000_0001);
const MAIN_HAND: Did = Did(0x7000_0002);
const OFF_HAND: Did = Did(0x7000_0003);
const NO_PAIR: Did = Did(0x7000_0004);
const BAD_ELEMENT: Did = Did(0x7000_0005);
const BAD_LAYOUT: Did = Did(0x7000_0006);
const CURRENCY_DIR: Did = Did(0x7000_0100);
const GOLD: Did = Did(0x7000_0101);
const SILVER: Did = Did(0x7000_0102);
const COPPER: Did = Did(0x7000_0103);

// Element IDs.
const E_TOOLTIP: u32 = 0x1000_0001;
const E_TOOLTIP_NEW: u32 = 0x1000_0002;
const E_LISTBOX: u32 = 0x1000_0003;
const E_NAME: u32 = 0x1000_0010;
const E_BINDING_ROW: u32 = 0x1000_0011;
const E_BINDING: u32 = 0x1000_0012;
const E_UNIQUE: u32 = 0x1000_0013;
const E_SEPARATOR: u32 = 0x1000_0014;
const E_ARMOUR: u32 = 0x1000_0015;
const E_DURABILITY: u32 = 0x1000_0016;
const E_DESCRIPTION: u32 = 0x1000_0017;
const E_WORTH: u32 = 0x1000_0018;
const E_BARTER: u32 = 0x1000_0019;
const E_CATEGORY_TEXT: u32 = 0x1000_001A;
const E_TARGETS: u32 = 0x1000_001B;
const E_EQUIPPED_LABEL: u32 = 0x1000_001C;
const E_COMPARE: u32 = 0x1000_001D;
const E_COMPARE_LIST: u32 = 0x1000_001E;
const E_COLOUR_TABLE: u32 = 0x1000_0020;
const E_SKILL_THING: u32 = 0x1000_0021;

// Widget types.
const TYPES: &[(u32, &str)] = &[
    (8, "Frame"),
    (14, "Field"),
    (16, "Text"),
    (0x1000_0001, "ExaminationListBox"),
    (0x1000_0002, "ExaminationName"),
    (0x1000_0003, "ExaminationRow"),
    (0x1000_0004, "ExaminationItemBinding"),
    (0x1000_0005, "ExaminationItemUniqueness"),
    (0x1000_0006, "ExaminationArmor"),
    (0x1000_0007, "ExaminationDurability"),
    (0x1000_0008, "ExaminationGeneroProperty"),
    (0x1000_0009, "ExaminationValue"),
    (0x1000_000A, "ExaminationSkillDamage"),
    (0x1000_000B, "ListBox"),
    (0x1000_000C, "AutoResizeListBox"),
    (0x1000_000D, "ExaminationIcon"),
    (0x1000_000E, "ExaminationItemLevel"),
    (0x1000_000F, "ExaminationUsageReqs"),
    (0x1000_0010, "ExaminationAllowedSlots"),
    (0x1000_0011, "ExaminationGameSetMemberList"),
    (0x1000_0012, "ExaminationGameSetBonusList"),
    (0x1000_0013, "ExaminationEffectList"),
];

// States.
const S_BROKEN: u32 = 0x1000_0001;
const S_INCOMPARABLE: u32 = 0x1000_0002;
const S_OBJECT: u32 = 0x1000_0003;

const INCOMPARABLE_COLOUR: u32 = 0xFF00_FDFF;
const OBJECT_COLOUR: u32 = 0xFFFF_FFFF;
const BROKEN_COLOUR: u32 = 0xFFFF_0000;

fn ty(label: &str) -> u32 {
    TYPES
        .iter()
        .find(|(_, l)| *l == label)
        .map(|(k, _)| *k)
        .expect("type")
}

/// The in-memory install.
#[derive(Default)]
struct Mem {
    names: Vec<(String, PropertyType)>,
    defaults: HashMap<u32, PropertyValue>,
    enum_texts: HashMap<(u32, u32), String>,
    enum_keys: HashMap<(u32, String), u32>,
    records: HashMap<Did, Arc<PropertySet>>,
    weenies: HashMap<String, Did>,
    layouts: HashMap<Did, Arc<UiLayout>>,
    layout_labels: HashMap<String, Did>,
    ui: Vec<(UiEnum, u32, String)>,
    /// (table key or table DID, token) → template with `{0x…}` slots.
    texts: HashMap<(u32, u32), String>,
    /// (table key, token) → a colour the whole rendered text carries (markup colour).
    styled: HashMap<(u32, u32), u32>,
    /// Item → its set (`set_of`).
    sets: HashMap<Did, Did>,
}

impl Mem {
    fn p(&mut self, name: &str, ty: PropertyType) -> u32 {
        if let Some(i) = self.names.iter().position(|(n, _)| n == name) {
            return i as u32 + 1;
        }
        self.names.push((name.to_owned(), ty));
        self.names.len() as u32
    }

    fn known(&self, name: &str) -> u32 {
        self.pid(name).unwrap_or_else(|| panic!("unknown {name}"))
    }

    fn add_ui_label(&mut self, which: UiEnum, key: u32, label: &str) {
        self.ui.push((which, key, label.to_owned()));
    }

    /// Substitutes `{0x…}` slots; `None` when a slot has no value (G25).
    fn fill(template: &str, values: &Values) -> Option<String> {
        let mut out = String::new();
        let mut rest = template;
        while let Some(i) = rest.find("{0x") {
            out.push_str(&rest[..i]);
            let end = rest[i..].find('}').expect("closed slot") + i;
            let id = u32::from_str_radix(&rest[i + 3..end], 16).expect("hex");
            match values.get(id)? {
                Value::Text(s) => out.push_str(s),
                Value::Integer(n) => out.push_str(&n.to_string()),
                Value::Float(x) => out.push_str(&x.to_string()),
                _ => return None,
            }
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        Some(out)
    }
}

fn literal(s: &str) -> StringInfo {
    StringInfo {
        source: StringSource::Literal(Utf16String(s.encode_utf16().collect())),
        args: None,
    }
}

fn table_ref(token: u32) -> StringInfo {
    StringInfo {
        source: StringSource::Table {
            token,
            table: T_FORMATS,
        },
        args: None,
    }
}

impl TooltipData for Mem {
    fn pid(&self, name: &str) -> Option<u32> {
        self.names
            .iter()
            .position(|(n, _)| n == name)
            .map(|i| i as u32 + 1)
    }
    fn property_name(&self, pid: u32) -> Option<String> {
        self.names
            .get((pid as usize).wrapping_sub(1))
            .map(|(n, _)| n.clone())
    }
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.names
            .get((pid as usize).wrapping_sub(1))
            .map(|(_, t)| *t)
    }
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        self.defaults.get(&pid).cloned()
    }
    fn enum_text(&self, pid: u32, key: u32) -> Option<String> {
        self.enum_texts.get(&(pid, key)).cloned()
    }
    fn enum_key(&self, pid: u32, name: &str) -> Option<u32> {
        self.enum_keys.get(&(pid, name.to_owned())).copied()
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
            StringSource::Table { .. } => Err("tables hold formats only".into()),
        }
    }
    fn render(&self, text: TextRef<'_>, values: &Values) -> TextResult {
        let template = match text {
            TextRef::Info(info) => match &info.source {
                StringSource::Literal(s) => String::from_utf16_lossy(&s.0),
                StringSource::Table { token, table } => self
                    .texts
                    .get(&(table.0, *token))
                    .cloned()
                    .ok_or_else(|| format!("miss {token:#x}"))?,
            },
            TextRef::Token { table_key, token } => self
                .texts
                .get(&(table_key, token))
                .cloned()
                .ok_or_else(|| format!("miss {token:#x}"))?,
        };
        let colour = match text {
            TextRef::Token { table_key, token } => self.styled.get(&(table_key, token)).copied(),
            TextRef::Info(_) => None,
        };
        Ok(Self::fill(&template, values)
            .filter(|t| !t.trim().is_empty())
            .map(|t| match colour {
                Some(c) => {
                    let [b, g, r, a] = c.to_le_bytes();
                    let mut rt = RichText::new();
                    let style = Style {
                        color: Some(Rgba { r, g, b, a }),
                        ..Style::default()
                    };
                    rt.push_str(&t, &style);
                    rt
                }
                None => RichText::plain(&t),
            }))
    }
    fn set_of(&self, item: Did) -> Option<Did> {
        self.sets.get(&item).copied()
    }
}

fn colour(argb: u32) -> PropertyValue {
    PropertyValue::Colour(argb.to_le_bytes())
}

fn el(id: u32, type_label: &str, w: i32, h: i32, props: Vec<(u32, PropertyValue)>) -> ElementDesc {
    ElementDesc {
        block: StateBlock {
            rect: Rect {
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            properties: set(props),
            ..StateBlock::default()
        },
        element_id: id,
        element_type: ty(type_label),
        ..ElementDesc::default()
    }
}

fn set(props: Vec<(u32, PropertyValue)>) -> PropertySet {
    PropertySet {
        entries: props
            .into_iter()
            .map(|(pid, value)| PropertyEntry { pid, value })
            .collect(),
        ..PropertySet::default()
    }
}

fn info(i: StringInfo) -> PropertyValue {
    PropertyValue::StringInfo(Box::new(i))
}

fn state(id: u32, colours_pid: u32, colour_pid: u32, argb: u32) -> StateBlock {
    StateBlock {
        state_id: id,
        properties: set(vec![(
            colours_pid,
            PropertyValue::Array(PropertyArray {
                count: 1,
                elements: vec![PropertyEntry {
                    pid: colour_pid,
                    value: colour(argb),
                }],
            }),
        )]),
        ..StateBlock::default()
    }
}

/// Builds the synthetic install.
#[allow(clippy::too_many_lines)]
fn install() -> Mem {
    use PropertyType as T;
    let mut m = Mem::default();
    // UI properties.
    let templates = m.p("UICore_ListBox_entry_templates", T::Array);
    let t_layout = m.p("UICore_ListBox_entry_template_layout", T::DidRef);
    let t_element = m.p("UICore_ListBox_entry_template_element", T::Enum);
    let autofill = m.p("UICore_ListBox_entry_templates_autofill", T::Bool);
    let max_width = m.p("UICore_Element_max_width", T::Int);
    let colours = m.p("UICore_Text_font_colors", T::Array);
    let font_colour = m.p("UICore_Text_font_color", T::Colour);
    let text_entry = m.p("UICore_Text_entry", T::StringInfo);
    let display = m.p("UI_Examination_Tooltip_DisplayString", T::StringInfo);
    let generic = m.p("UI_Examination_Tooltip_GeneroProperty", T::PropertyId);
    let stretch = m.p("UI_Examination_Tooltip_Stretch_Wide", T::Bool);
    let left = m.p("UI_Examination_Tooltip_Row_Left_Child", T::Enum);
    let right = m.p("UI_Examination_Tooltip_Row_Right_Child", T::Enum);
    let separator = m.p("UI_Examination_Tooltip_Separator", T::Bool);
    let unique_text = m.p("UI_Examination_Tooltip_ItemUniqueText", T::StringInfo);
    // Item properties.
    let hover_l = m.p("UI_Examination_Tooltip_0Click_Layout", T::DidRef);
    let hover_e = m.p("UI_Examination_Tooltip_0Click_Element", T::Enum);
    let cmp_l = m.p("UI_Examination_Tooltip_Compare_Layout", T::DidRef);
    let cmp_e = m.p("UI_Examination_Tooltip_Compare_Element", T::Enum);
    let cmp_type = m.p("UI_Examination_Tooltip_Compare_Type", T::Enum);
    let name = m.p("Name", T::StringInfo);
    let description = m.p("Description", T::StringInfo);
    let quality = m.p("Item_Quality", T::Enum);
    let on_acquire = m.p("Inventory_BindOnAcquire", T::Bool);
    let to_account = m.p("Inventory_BindToAccount", T::Bool);
    let _bound = m.p("Inventory_BoundToID", T::InstanceId);
    let unique = m.p("Inventory_Unique", T::Bool);
    let value = m.p("Item_Value", T::Int);
    let cur = m.p("Item_CurStructurePoints", T::Int);
    let max = m.p("Item_MaxStructurePoints", T::Int);
    let _wear = m.p("Item_WearState", T::Enum);
    let armour = m.p("Item_Armor_Value", T::Int);
    let barter = m.p("Barter_IsBarterItem", T::Bool);
    let category = m.p("Item_Decoration_Category", T::Bitfield64);
    let targets = m.p("Usage_ValidTargetTypes", T::Bitfield32);
    let slots = m.p("Inventory_CompatibleSlot", T::Bitfield32);
    let currency_list = m.p("Currency_CurrencyList", T::Array);
    let currency = m.p("Currency_Currency", T::DidRef);
    let currency_value = m.p("Currency_Value", T::Int);
    m.defaults.insert(barter, PropertyValue::Bool(false));
    m.defaults.insert(armour, PropertyValue::Int(0));

    // Enum texts and keys.
    m.enum_keys.insert((cmp_type, "Equipment".into()), 2);
    m.enum_keys.insert((cmp_type, "Recipe".into()), 4);
    m.enum_texts.insert((category, 1), "Floor".into());
    m.enum_texts.insert((category, 3), "Wall".into());
    m.enum_texts.insert((targets, 1), "Self".into());
    // Key 2 (bit 1) of the targets mapper has no display text.

    // UI labels.
    for (k, l) in TYPES {
        m.add_ui_label(UiEnum::ElementType, *k, l);
    }
    for (id, l) in [
        (E_TOOLTIP, "Thing_ExamineLayout"),
        (E_TOOLTIP_NEW, "Tooltip_New"),
        (E_LISTBOX, "Examination_AutoResizeListbox"),
        (E_NAME, "Name_Base"),
        (E_BINDING_ROW, "ItemBinding_Row_Base"),
        (E_BINDING, "ItemBinding_Element"),
        (E_UNIQUE, "ItemUniqueness_Element"),
        (E_SEPARATOR, "Separator_Base"),
        (E_ARMOUR, "ArmorValue_Base"),
        (E_DURABILITY, "Durability_Base"),
        (E_DESCRIPTION, "Description_Base"),
        (E_WORTH, "Value_Base"),
        (E_BARTER, "ItemBarter_Base"),
        (E_CATEGORY_TEXT, "ItemDecorationCategory_Base"),
        (E_TARGETS, "UsageTargetType_Base"),
        (E_EQUIPPED_LABEL, "CurrentlyEquipped_Label"),
        (E_COMPARE, "ThingCompare_ExamineLayout"),
        (E_COMPARE_LIST, "Examination_AutoResizeListbox_Compare"),
        (E_COLOUR_TABLE, "ColorNameText"),
        (E_SKILL_THING, "SkillItem_Damage_Base"),
    ] {
        m.add_ui_label(UiEnum::ElementId, id, l);
    }
    m.add_ui_label(UiEnum::StateId, S_BROKEN, "broken");
    m.add_ui_label(
        UiEnum::StateId,
        S_INCOMPARABLE,
        "ItemQuality_Incomparable_ColorName",
    );
    m.add_ui_label(UiEnum::StateId, S_OBJECT, "Object_ColorName");

    // Texts.
    let exam = tokens::EXAMINATION_TABLE_KEY;
    let utils = tokens::UTILS_TABLE_KEY;
    for (key, token, text) in [
        (T_FORMATS.0, 1, "{0x52615}"),
        (T_FORMATS.0, 2, "{0x5a6195} armour"),
        (T_FORMATS.0, 3, "Durability {0xf1e3399}/{0x4015ba9}"),
        (T_FORMATS.0, 4, "\"{0x5a6195}\""),
        (T_FORMATS.0, 5, "Barter item"),
        (T_FORMATS.0, 6, "Placement: {0x5a6195}"),
        (T_FORMATS.0, 7, "Targets: {0x5a6195}"),
        (
            exam,
            tokens::BIND_ON_ACQUIRE_ACCOUNT,
            "Binds to account on acquire",
        ),
        (exam, tokens::BOUND_ACCOUNT, "Bound to account"),
        (exam, tokens::WORTH_LABEL, "Worth:"),
        (utils, tokens::UTILS_LIST, "{0x5425}, {0x59bf}"),
        (tokens::NUMBER_TABLE_KEY, tokens::COIN_COUNT, "{0x5242a9}"),
    ] {
        m.texts.insert((key, token), text.to_owned());
    }

    // The base layout: Tooltip_New, a frame with an inherited child of its own (the
    // background), so IV2 counts only the derived element's children.
    let mut tooltip_new = el(E_TOOLTIP_NEW, "Frame", 239, 100, vec![]);
    tooltip_new
        .children
        .push(el(0x1000_00FF, "Field", 239, 100, vec![]));
    m.layouts.insert(
        L_BASE,
        Arc::new(UiLayout {
            did: L_BASE,
            elements: vec![tooltip_new],
            ..UiLayout::default()
        }),
    );

    // The colour table of the `text` layout.
    let mut table = el(E_COLOUR_TABLE, "Text", 100, 18, vec![]);
    table.states = vec![
        state(S_INCOMPARABLE, colours, font_colour, INCOMPARABLE_COLOUR),
        state(S_OBJECT, colours, font_colour, OBJECT_COLOUR),
    ];
    m.layouts.insert(
        L_TEXT,
        Arc::new(UiLayout {
            did: L_TEXT,
            elements: vec![table],
            ..UiLayout::default()
        }),
    );
    m.layout_labels.insert("text".into(), L_TEXT);

    // The examination layout.
    let entry = |element: u32| PropertyEntry {
        pid: 0x7777,
        value: PropertyValue::Struct(PropertyStruct {
            fields: vec![
                PropertyEntry {
                    pid: t_layout,
                    value: PropertyValue::DidRef(L_EXAM),
                },
                PropertyEntry {
                    pid: t_element,
                    value: PropertyValue::Enum(element),
                },
            ],
            ..PropertyStruct::default()
        }),
    };
    let list = |id: u32, rows: &[u32]| {
        el(
            id,
            "ExaminationListBox",
            229,
            100,
            vec![
                (
                    templates,
                    PropertyValue::Array(PropertyArray {
                        count: rows.len() as u32,
                        elements: rows.iter().map(|&r| entry(r)).collect(),
                    }),
                ),
                (autofill, PropertyValue::Bool(true)),
                (max_width, PropertyValue::Int(229)),
            ],
        )
    };
    let frame = |id: u32, rows: &[u32], list_id: u32| {
        let mut f = ElementDesc {
            element_id: id,
            base_element: E_TOOLTIP_NEW,
            base_layout: L_BASE,
            ..ElementDesc::default()
        };
        f.children.push(list(list_id, rows));
        f
    };
    // §14.1 order: name, binding, sep, armour, sep, sep, durability, sep, description, sep,
    // worth, sep; then the generic rows and a P2 row.
    let rows = [
        E_NAME,
        E_BINDING_ROW,
        E_SEPARATOR,
        E_ARMOUR,
        E_SEPARATOR,
        E_SEPARATOR,
        E_DURABILITY,
        E_SEPARATOR,
        E_DESCRIPTION,
        E_SEPARATOR,
        E_WORTH,
        E_SEPARATOR,
        E_BARTER,
        E_CATEGORY_TEXT,
        E_TARGETS,
        E_SKILL_THING,
    ];
    let mut binding_row = el(
        E_BINDING_ROW,
        "ExaminationRow",
        200,
        18,
        vec![
            (left, PropertyValue::Enum(E_BINDING)),
            (right, PropertyValue::Enum(E_UNIQUE)),
            (stretch, PropertyValue::Bool(true)),
        ],
    );
    binding_row
        .children
        .push(el(E_BINDING, "ExaminationItemBinding", 146, 18, vec![]));
    binding_row.children.push(el(
        E_UNIQUE,
        "ExaminationItemUniqueness",
        100,
        18,
        vec![(unique_text, info(literal("Unique")))],
    ));
    let mut durability = el(
        E_DURABILITY,
        "ExaminationDurability",
        146,
        18,
        vec![(display, info(table_ref(3)))],
    );
    durability.states = vec![state(S_BROKEN, colours, font_colour, BROKEN_COLOUR)];
    let generic_row = |id: u32, pid: u32, token: u32| {
        el(
            id,
            "ExaminationGeneroProperty",
            220,
            18,
            vec![
                (generic, PropertyValue::PropertyId(pid)),
                (display, info(table_ref(token))),
            ],
        )
    };
    let elements = vec![
        frame(E_TOOLTIP, &rows, E_LISTBOX),
        frame(E_COMPARE, &[E_EQUIPPED_LABEL, E_NAME], E_COMPARE_LIST),
        el(
            E_NAME,
            "ExaminationName",
            180,
            48,
            vec![(display, info(table_ref(1)))],
        ),
        binding_row,
        el(
            E_SEPARATOR,
            "Field",
            170,
            10,
            vec![(separator, PropertyValue::Bool(true))],
        ),
        el(
            E_ARMOUR,
            "ExaminationArmor",
            220,
            18,
            vec![(display, info(table_ref(2)))],
        ),
        durability,
        generic_row(E_DESCRIPTION, description, 4),
        el(E_WORTH, "ExaminationValue", 204, 22, vec![]),
        generic_row(E_BARTER, barter, 5),
        generic_row(E_CATEGORY_TEXT, category, 6),
        generic_row(E_TARGETS, targets, 7),
        el(
            E_EQUIPPED_LABEL,
            "Text",
            220,
            18,
            vec![(text_entry, info(literal("Currently equipped")))],
        ),
        el(E_SKILL_THING, "ExaminationSkillDamage", 220, 18, vec![]),
    ];
    m.layouts.insert(
        L_EXAM,
        Arc::new(UiLayout {
            did: L_EXAM,
            design_width: 1024,
            design_height: 768,
            elements,
            ..UiLayout::default()
        }),
    );

    // Currencies (gold, silver, copper).
    m.weenies.insert("CurrencyDirectory".into(), CURRENCY_DIR);
    m.records.insert(
        CURRENCY_DIR,
        Arc::new(set(vec![(
            currency_list,
            PropertyValue::Array(PropertyArray {
                count: 3,
                elements: [GOLD, SILVER, COPPER]
                    .iter()
                    .map(|&d| PropertyEntry {
                        pid: currency,
                        value: PropertyValue::DidRef(d),
                    })
                    .collect(),
            }),
        )])),
    );
    for (d, v) in [(GOLD, 100_000), (SILVER, 100), (COPPER, 1)] {
        m.records.insert(
            d,
            Arc::new(set(vec![(currency_value, PropertyValue::Int(v))])),
        );
    }

    // Items.
    let pair = |l: Did, e: u32| {
        vec![
            (hover_l, PropertyValue::DidRef(l)),
            (hover_e, PropertyValue::Enum(e)),
        ]
    };
    let mut item = pair(L_EXAM, E_TOOLTIP);
    item.extend([
        (name, info(literal("Copper Sword"))),
        (quality, PropertyValue::Enum(3)),
        (on_acquire, PropertyValue::Bool(true)),
        (to_account, PropertyValue::Bool(true)),
        (unique, PropertyValue::Bool(true)),
        (value, PropertyValue::Int(123_405)),
        (cur, PropertyValue::Int(30)),
        (max, PropertyValue::Int(30)),
        (armour, PropertyValue::Int(0)),
        (barter, PropertyValue::Bool(false)),
        (category, PropertyValue::Bitfield64(0b101)),
        (targets, PropertyValue::Bitfield32(0b11)),
        (slots, PropertyValue::Bitfield32(0x2001_0000)),
        (cmp_type, PropertyValue::Enum(2)),
    ]);
    m.records.insert(ITEM, Arc::new(set(item)));
    for (d, n) in [(MAIN_HAND, "Old Blade"), (OFF_HAND, "Old Dagger")] {
        let mut v = pair(L_EXAM, E_TOOLTIP);
        v.extend([
            (cmp_l, PropertyValue::DidRef(L_EXAM)),
            (cmp_e, PropertyValue::Enum(E_COMPARE)),
            (name, info(literal(n))),
        ]);
        m.records.insert(d, Arc::new(set(v)));
    }
    m.records
        .insert(NO_PAIR, Arc::new(set(vec![(name, info(literal("Thing")))])));
    m.records
        .insert(BAD_ELEMENT, Arc::new(set(pair(L_EXAM, 0x1000_0999))));
    m.records
        .insert(BAD_LAYOUT, Arc::new(set(pair(L_MISSING, E_TOOLTIP))));
    let _ = (m.known("Name"), m.known("Item_Value"));
    p1_rows(&mut m);
    m
}

fn engine() -> TooltipEngine<Mem> {
    TooltipEngine::new(install())
}

fn row_text(m: &tessera_gamedata::tooltip::TooltipModel, label: &str) -> Option<String> {
    m.rows
        .iter()
        .find(|r| r.label.as_deref() == Some(label))
        .map(|r| r.plain_text())
}

#[test]
fn frame_structure_iv2_iv3() {
    let e = engine();
    let f = e
        .frame(TemplateRef {
            layout: L_EXAM,
            element: E_TOOLTIP,
        })
        .expect("frame");
    assert_eq!(f.base_label.as_deref(), Some("Tooltip_New"));
    assert_eq!(f.children, 1);
    assert!(f.list.autofill);
    assert_eq!(f.list.max_width, Some(229));
    assert_eq!(f.list.templates.len(), 16);
    for t in f.list.templates.iter().flatten() {
        assert!(e.template(*t).is_ok(), "{t:?}");
    }
}

#[test]
fn example_14_1_end_to_end_assembly() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(ITEM), &Viewer::default())
        .expect("tooltip");
    assert!(m.failures.is_empty(), "{:?}", m.failures);
    let shown: Vec<_> = m
        .shown_rows()
        .map(|r| r.label.clone().unwrap_or_default())
        .collect();
    assert_eq!(
        shown,
        [
            "Name_Base",
            "ItemBinding_Row_Base",
            "Separator_Base",
            "Durability_Base",
            "Separator_Base",
            "Value_Base",
            "Separator_Base",
            "ItemDecorationCategory_Base",
        ]
    );
    // The P2 row is built empty and removed (§6.20).
    let skill = m
        .rows
        .iter()
        .find(|r| r.label.as_deref() == Some("SkillItem_Damage_Base"))
        .expect("row");
    assert!(matches!(skill.widget, Widget::Deferred(_)) && skill.is_empty());
    assert!(m.assembly.stretched.contains(&1));
}

#[test]
fn example_14_2_binding_and_uniqueness() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(ITEM), &Viewer::default())
        .expect("tooltip");
    assert_eq!(
        row_text(&m, "ItemBinding_Row_Base").as_deref(),
        Some("Binds to account on acquire  Unique")
    );
    // The same record as a bound instance: rule 3.
    let bound = e.data().pid("Inventory_BoundToID").expect("pid");
    let req = TooltipRequest {
        instance: Instance {
            properties: vec![PropertyEntry {
                pid: bound,
                value: PropertyValue::InstanceId(0x55),
            }],
            ..Instance::default()
        },
        ..TooltipRequest::hover(ITEM)
    };
    let m = e.build(&req, &Viewer::default()).expect("tooltip");
    assert_eq!(
        row_text(&m, "ItemBinding_Row_Base").as_deref(),
        Some("Bound to account  Unique")
    );
}

#[test]
fn example_14_3_name_colour() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(ITEM), &Viewer::default())
        .expect("tooltip");
    let name = m
        .rows
        .iter()
        .find(|r| r.widget == Widget::Name)
        .expect("name");
    assert_eq!(name.plain_text(), "Copper Sword");
    assert_eq!(
        name.state.as_deref(),
        Some("ItemQuality_Incomparable_ColorName")
    );
    assert_eq!(name.colour, Some(INCOMPARABLE_COLOUR));
    assert!(m.has_name());
}

#[test]
fn example_14_4_worth_coins() {
    let e = engine();
    let req = TooltipRequest {
        instance: Instance {
            quantity: Some(2),
            ..Instance::default()
        },
        ..TooltipRequest::hover(ITEM)
    };
    let m = e.build(&req, &Viewer::default()).expect("tooltip");
    let worth = m
        .rows
        .iter()
        .find(|r| r.widget == Widget::Worth)
        .expect("worth");
    let RowContent::Money { label, coins } = &worth.content else {
        panic!("{:?}", worth.content);
    };
    assert_eq!(
        label.as_ref().map(RichText::plain_text).as_deref(),
        Some("Worth:")
    );
    let c: Vec<_> = coins
        .iter()
        .map(|c| (c.currency, c.count, c.text.as_str()))
        .collect();
    assert_eq!(
        c,
        [(GOLD, 2, "2"), (SILVER, 468, "468"), (COPPER, 10, "10")]
    );
}

#[test]
fn worth_coins_sort_a_property_set_currency_list_largest_first() {
    // §6.11 order caveat (`npc-windows.md` §9.3): the currency directory's property set
    // stores silver, gold, copper; a reader of it sorts by `Currency_Value`, largest first,
    // so the breakdown of example 14.4 is unchanged.
    let mut m = install();
    let list_pid = m.known("Currency_CurrencyList");
    let currency = m.known("Currency_Currency");
    m.records.insert(
        CURRENCY_DIR,
        Arc::new(set(vec![(
            list_pid,
            PropertyValue::Array(PropertyArray {
                count: 3,
                elements: [SILVER, GOLD, COPPER]
                    .iter()
                    .map(|&d| PropertyEntry {
                        pid: currency,
                        value: PropertyValue::DidRef(d),
                    })
                    .collect(),
            }),
        )])),
    );
    let e = TooltipEngine::new(m);
    let req = TooltipRequest {
        instance: Instance {
            quantity: Some(2),
            ..Instance::default()
        },
        ..TooltipRequest::hover(ITEM)
    };
    let m = e.build(&req, &Viewer::default()).expect("tooltip");
    let worth = m
        .rows
        .iter()
        .find(|r| r.widget == Widget::Worth)
        .expect("worth");
    let RowContent::Money { coins, .. } = &worth.content else {
        panic!("{:?}", worth.content);
    };
    let c: Vec<_> = coins
        .iter()
        .map(|c| (c.currency, c.index, c.count))
        .collect();
    assert_eq!(c, [(GOLD, 0, 2), (SILVER, 1, 468), (COPPER, 2, 10)]);
}

#[test]
fn generic_rows_follow_6_4() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(ITEM), &Viewer::default())
        .expect("tooltip");
    // Value equal to the dictionary default: empty.
    assert_eq!(row_text(&m, "ItemBarter_Base").as_deref(), Some(""));
    // Absent description: empty.
    assert_eq!(row_text(&m, "Description_Base").as_deref(), Some(""));
    // 64-bit bitfield: texts of keys 1 and 3 joined by the list format.
    assert_eq!(
        row_text(&m, "ItemDecorationCategory_Base").as_deref(),
        Some("Placement: Floor, Wall")
    );
    // A set bit without a display text leaves the variable unset: G25, empty row.
    assert_eq!(row_text(&m, "UsageTargetType_Base").as_deref(), Some(""));
    // A true flag shows the format's fixed text.
    let barter = e.data().pid("Barter_IsBarterItem").expect("pid");
    let req = TooltipRequest {
        instance: Instance {
            properties: vec![PropertyEntry {
                pid: barter,
                value: PropertyValue::Bool(true),
            }],
            ..Instance::default()
        },
        ..TooltipRequest::hover(ITEM)
    };
    let m = e.build(&req, &Viewer::default()).expect("tooltip");
    assert_eq!(
        row_text(&m, "ItemBarter_Base").as_deref(),
        Some("Barter item")
    );
    // A barter item is not sellable: no worth row.
    assert_eq!(row_text(&m, "Value_Base").as_deref(), Some(""));
}

#[test]
fn condition_sets_the_durability_state() {
    let e = engine();
    let req = TooltipRequest {
        instance: Instance {
            condition: Some(Condition {
                current: 0,
                max: 30,
            }),
            ..Instance::default()
        },
        ..TooltipRequest::hover(ITEM)
    };
    let m = e.build(&req, &Viewer::default()).expect("tooltip");
    let d = m
        .rows
        .iter()
        .find(|r| r.widget == Widget::Durability)
        .expect("durability");
    assert_eq!(d.plain_text(), "Durability 0/30");
    assert_eq!(d.state.as_deref(), Some("broken"));
    assert_eq!(d.colour, Some(BROKEN_COLOUR));
}

fn equipped(slot: u32, did: Did) -> Equipped {
    Equipped {
        slot,
        did,
        instance: Instance::default(),
    }
}

#[test]
fn example_14_5_comparison() {
    let e = engine();
    let viewer = Viewer {
        equipped: vec![equipped(0x20000, OFF_HAND), equipped(0x10000, MAIN_HAND)],
        ..Viewer::default()
    };
    let m = e
        .build(&TooltipRequest::hover(ITEM), &viewer)
        .expect("tooltip");
    assert_eq!(m.comparison.kind, CompareKind::Equipment);
    let order: Vec<_> = m
        .comparison
        .tooltips
        .iter()
        .map(|(s, t)| (*s, t.subject.clone()))
        .collect();
    assert_eq!(
        order,
        [
            (0x10000, Subject::Record(MAIN_HAND)),
            (0x20000, Subject::Record(OFF_HAND))
        ]
    );
    for (_, t) in &m.comparison.tooltips {
        assert_eq!(t.purpose, Purpose::Comparison);
        assert_eq!(
            t.text_lines().first().map(String::as_str),
            Some("Currently equipped")
        );
    }
    // Equipped items are not compared; the option off disables comparison.
    let req = TooltipRequest {
        instance: Instance {
            equipped: true,
            ..Instance::default()
        },
        ..TooltipRequest::hover(ITEM)
    };
    assert!(
        e.build(&req, &viewer)
            .expect("tooltip")
            .comparison
            .tooltips
            .is_empty()
    );
    let off = Viewer {
        compare_equipped: false,
        ..viewer
    };
    assert!(
        e.build(&TooltipRequest::hover(ITEM), &off)
            .expect("tooltip")
            .comparison
            .tooltips
            .is_empty()
    );
}

#[test]
fn literal_items_get_a_name_only() {
    let e = engine();
    let req = TooltipRequest {
        subject: Subject::Literal("A pebble".into()),
        ..TooltipRequest::hover(ITEM)
    };
    let m = e.build(&req, &Viewer::default()).expect("tooltip");
    assert!(m.frame.is_none());
    assert_eq!(m.text_lines(), ["A pebble"]);
    assert_eq!(m.rows[0].colour, Some(OBJECT_COLOUR));
    assert!(m.has_name());
}

#[test]
fn categories_of_12() {
    let e = engine();
    let v = Viewer::default();
    assert_eq!(e.classify(ITEM, &v).category, Ok(Category::Rendered));
    assert_eq!(e.classify(NO_PAIR, &v).category, Ok(Category::NoTooltip));
    assert_eq!(
        e.classify(BAD_ELEMENT, &v).category,
        Ok(Category::ElementMissing)
    );
    assert_eq!(
        e.classify(BAD_LAYOUT, &v).category,
        Ok(Category::LayoutUnreadable)
    );
    assert_eq!(
        e.classify(Did(0x7000_0999), &v).category,
        Err(TooltipError::RecordMissing(Did(0x7000_0999)))
    );
    assert!(matches!(
        e.build(&TooltipRequest::hover(NO_PAIR), &v),
        Err(TooltipError::NoTooltip)
    ));
    // An override pair takes precedence (§3.3 step 1).
    let req = TooltipRequest {
        override_pair: Some(TemplateRef {
            layout: L_EXAM,
            element: E_TOOLTIP,
        }),
        ..TooltipRequest::hover(NO_PAIR)
    };
    assert!(e.build(&req, &v).expect("tooltip").has_name());
}

// ---------------------------------------------------------------------------------------------
// A second examination layout: name-and-icon list, item level, requirements, allowed slots,
// sets, effects, a comparison-property row; a ring, a set, a recipe and hostile templates.
// ---------------------------------------------------------------------------------------------

const L_EXAM2: Did = Did(0x2200_0003);

const E2_TOOLTIP: u32 = 0x1000_0040;
const E2_LIST: u32 = 0x1000_0041;
const E2_NAMEICON: u32 = 0x1000_0042;
const E2_ICON_FIELD: u32 = 0x1000_0043;
const E2_ICON: u32 = 0x1000_0044;
const E2_NAME: u32 = 0x1000_0045;
const E2_LEVEL: u32 = 0x1000_0046;
const E2_REQS: u32 = 0x1000_0047;
const E2_REQS_LINE: u32 = 0x1000_0048;
const E2_SLOTS: u32 = 0x1000_0049;
const E2_SET_MEMBERS: u32 = 0x1000_004A;
const E2_SET_BONUS: u32 = 0x1000_004B;
const E2_SET_HEADER: u32 = 0x1000_004C;
const E2_SET_LINE: u32 = 0x1000_004D;
const E2_TIER: u32 = 0x1000_004E;
const E2_EFFECTS: u32 = 0x1000_004F;
const E2_EFFECT_LINE: u32 = 0x1000_0050;
const E_BOMB_TOOLTIP: u32 = 0x1000_0060;
const E_BOMB_LIST: u32 = 0x1000_0061;
const E_SELF: u32 = 0x1000_0062;
const E_FAN_A: u32 = 0x1000_0063;
const E_FAN_B: u32 = 0x1000_0064;
const E_FAN_C: u32 = 0x1000_0065;

const S_CANTUSE: u32 = 0x1000_0004;
const S_ACTIVE: u32 = 0x1000_0005;
const S_INACTIVE: u32 = 0x1000_0006;

const LINE_COLOUR: u32 = 0xFFE5_E5E5;
const CANTUSE_COLOUR: u32 = 0xFFFF_0000;
const ACTIVE_COLOUR: u32 = 0xFF00_E500;
const INACTIVE_COLOUR: u32 = 0xFF7F_7F7F;
const HEADER_COLOUR: u32 = 0xFF12_3456;
const SLOTS_COLOUR: u32 = 0xFFAB_CDEF;
const EFFECT_COLOUR: u32 = 0xFFCC_CC7F;
const MARKER_COLOUR: u32 = 0xFF00_FF00;

const RING: Did = Did(0x7000_0010);
const SET_B: Did = Did(0x7000_0011);
const SET_C: Did = Did(0x7000_0012);
const SET: Did = Did(0x7000_0020);
const PROG: Did = Did(0x7000_0021);
const RECIPE_ITEM: Did = Did(0x7000_0030);
const RECIPE: Did = Did(0x7000_0031);
const BOMB: Did = Did(0x7000_0040);

fn entry_to(m: &mut Mem, layout: Did, element: u32) -> PropertyEntry {
    let t_layout = m.p("UICore_ListBox_entry_template_layout", PropertyType::DidRef);
    let t_element = m.p("UICore_ListBox_entry_template_element", PropertyType::Enum);
    PropertyEntry {
        pid: 0x7777,
        value: PropertyValue::Struct(PropertyStruct {
            fields: vec![
                PropertyEntry {
                    pid: t_layout,
                    value: PropertyValue::DidRef(layout),
                },
                PropertyEntry {
                    pid: t_element,
                    value: PropertyValue::Enum(element),
                },
            ],
            ..PropertyStruct::default()
        }),
    }
}

fn entries(m: &mut Mem, layout: Did, elements: &[u32]) -> (u32, PropertyValue) {
    let templates = m.p("UICore_ListBox_entry_templates", PropertyType::Array);
    let list: Vec<PropertyEntry> = elements.iter().map(|&e| entry_to(m, layout, e)).collect();
    (
        templates,
        PropertyValue::Array(PropertyArray {
            count: list.len() as u32,
            elements: list,
        }),
    )
}

fn base_colour(m: &mut Mem, argb: u32) -> (u32, PropertyValue) {
    let colours = m.p("UICore_Text_font_colors", PropertyType::Array);
    let font_colour = m.p("UICore_Text_font_color", PropertyType::Colour);
    (
        colours,
        PropertyValue::Array(PropertyArray {
            count: 1,
            elements: vec![PropertyEntry {
                pid: font_colour,
                value: colour(argb),
            }],
        }),
    )
}

fn structs(pid: u32, items: Vec<Vec<(u32, PropertyValue)>>) -> PropertyValue {
    PropertyValue::Array(PropertyArray {
        count: items.len() as u32,
        elements: items
            .into_iter()
            .map(|fields| PropertyEntry {
                pid,
                value: PropertyValue::Struct(PropertyStruct {
                    fields: fields
                        .into_iter()
                        .map(|(pid, value)| PropertyEntry { pid, value })
                        .collect(),
                    ..PropertyStruct::default()
                }),
            })
            .collect(),
    })
}

/// Adds the second layout and its records to the synthetic install.
#[allow(clippy::too_many_lines)]
fn p1_rows(m: &mut Mem) {
    use PropertyType as T;
    let colours = m.p("UICore_Text_font_colors", T::Array);
    let font_colour = m.p("UICore_Text_font_color", T::Colour);
    let autofill = m.p("UICore_ListBox_entry_templates_autofill", T::Bool);
    let horizontal = m.p("UICore_ListBox_horizontal", T::Bool);
    let display = m.p("UI_Examination_Tooltip_DisplayString", T::StringInfo);
    let generic = m.p("UI_Examination_Tooltip_GeneroProperty", T::PropertyId);
    let comparison = m.p("UI_Examination_Tooltip_ComparisonProperty", T::PropertyId);
    let hover_l = m.p("UI_Examination_Tooltip_0Click_Layout", T::DidRef);
    let hover_e = m.p("UI_Examination_Tooltip_0Click_Element", T::Enum);
    let cmp_l = m.p("UI_Examination_Tooltip_Compare_Layout", T::DidRef);
    let cmp_e = m.p("UI_Examination_Tooltip_Compare_Element", T::Enum);
    let cmp_type = m.p("UI_Examination_Tooltip_Compare_Type", T::Enum);
    let name = m.p("Name", T::StringInfo);
    let slots = m.p("Inventory_CompatibleSlot", T::Bitfield32);
    let level = m.p("Item_Level", T::Int);
    let pending = m.p("Item_Level_Bonus_Pending", T::Bool);
    let min_level = m.p("Usage_MinLevel", T::Int);
    let max_level = m.p("Usage_MaxLevel", T::Int);
    let tier = m.p("Item_Tier", T::Int);
    let might = m.p("Stat_Might", T::Int);
    let mod_array = m.p("Mod_Array", T::Array);
    let mod_entry = m.p("Mod_Entry", T::Struct);
    let modified = m.p("Mod_Modified", T::PropertyId);
    let mod_op = m.p("Mod_Op", T::Enum);
    let meta_list = m.p("PropertyMetaData_PropertyMetaDataList", T::Array);
    let meta_property = m.p("PropertyMetaData_Property", T::PropertyId);
    let meta_name = m.p("PropertyMetaData_Name", T::StringInfo);
    let mod_progression = m.p("Mod_Progression", T::DidRef);
    let set_name = m.p("Set_Name", T::StringInfo);
    let set_level = m.p("Set_Level", T::Int);
    let member_list = m.p("Set_MemberList", T::Array);
    let member = m.p("Set_MemberDID", T::DidRef);
    let tiers = m.p("Set_ActiveCountDataList", T::Array);
    let tier_data = m.p("Set_ActiveCountData", T::Struct);
    let active_count = m.p("Set_ActiveCount", T::Int);
    let linear = m.p("LinearInterpolatingProgression_Array", T::Array);
    let point = m.p("LinearInterpolatingProgression_ControlPoint", T::Struct);
    let key = m.p("LinearInterpolatingProgression_Key", T::Int);
    let value = m.p("LinearInterpolatingProgression_Value", T::Float);
    let recipe = m.p("RecipeItem_Recipe", T::DidRef);
    let result = m.p("CraftRecipe_ResultItem", T::DidRef);
    let quantity = m.p("CraftRecipe_ResultItemQuantity", T::Int);

    for (k, l) in [
        (S_CANTUSE, "cantuse"),
        (S_ACTIVE, "gameset_active"),
        (S_INACTIVE, "gameset_inactive"),
    ] {
        m.add_ui_label(UiEnum::StateId, k, l);
    }
    for (id, l) in [
        (E2_TOOLTIP, "Ring_ExamineLayout"),
        (E2_LIST, "Examination_AutoResizeListbox_Ring"),
        (E2_NAMEICON, "NameIcon_Row_Base"),
        (E2_ICON_FIELD, "Icon_Base"),
        (E2_ICON, "Icon_Element"),
        (E2_NAME, "Name_Ring"),
        (E2_LEVEL, "ExaminationItemLevel_Base"),
        (E2_REQS, "UsageReqsDisplay_Base"),
        (E2_REQS_LINE, "UsageReqsListElement"),
        (E2_SLOTS, "AllowedSlots_Base"),
        (E2_SET_MEMBERS, "GameSetMemberListDisplayBase"),
        (E2_SET_BONUS, "GameSetBonusListDisplayBase"),
        (E2_SET_HEADER, "Set_Header"),
        (E2_SET_LINE, "Set_Line"),
        (E2_TIER, "ItemTier_Base"),
        (E2_EFFECTS, "EffectDisplay_Base"),
        (E2_EFFECT_LINE, "EffectListElement"),
        (E_SELF, "Self_List"),
        (E_FAN_A, "Fan_A"),
        (E_FAN_B, "Fan_B"),
        (E_FAN_C, "Fan_C"),
    ] {
        m.add_ui_label(UiEnum::ElementId, id, l);
    }
    m.enum_texts.insert((slots, 12), "Left ring".into());
    m.enum_texts.insert((slots, 13), "Right ring".into());
    m.enum_texts.insert((slots, 30), "Equipment".into());
    m.enum_keys.insert((slots, "Equipment".into()), 30);

    let exam = tokens::EXAMINATION_TABLE_KEY;
    for (key, token, text) in [
        (exam, tokens::ITEM_LEVEL, "Level {0x50aa9c}"),
        (exam, tokens::LEVEL_BONUS_PENDING, "(+)"),
        (
            tokens::UTILS_TABLE_KEY,
            tokens::UTILS_PAIR,
            "{0x5425} {0x59bf}",
        ),
        (T_FORMATS.0, 8, "Tier {0x5a6195}"),
    ] {
        m.texts.insert((key, token), text.to_owned());
    }
    m.styled
        .insert((exam, tokens::LEVEL_BONUS_PENDING), MARKER_COLOUR);

    // The layout.
    let frame = |m: &mut Mem, id: u32, list_id: u32, rows: &[u32]| {
        let mut f = ElementDesc {
            element_id: id,
            base_element: E_TOOLTIP_NEW,
            base_layout: L_BASE,
            ..ElementDesc::default()
        };
        let templates = entries(m, L_EXAM2, rows);
        f.children.push(el(
            list_id,
            "ExaminationListBox",
            229,
            100,
            vec![templates, (autofill, PropertyValue::Bool(true))],
        ));
        f
    };
    let tooltip = frame(
        m,
        E2_TOOLTIP,
        E2_LIST,
        &[
            E2_NAMEICON,
            E2_LEVEL,
            E2_REQS,
            E2_SLOTS,
            E2_SET_MEMBERS,
            E2_SET_BONUS,
            E2_TIER,
            E2_EFFECTS,
        ],
    );
    let bomb = frame(m, E_BOMB_TOOLTIP, E_BOMB_LIST, &[E_SELF, E_FAN_A]);
    let name_icon_entries = entries(m, L_EXAM2, &[E2_ICON_FIELD, E2_NAME]);
    let name_icon = el(
        E2_NAMEICON,
        "AutoResizeListBox",
        180,
        48,
        vec![
            (horizontal, PropertyValue::Bool(true)),
            (autofill, PropertyValue::Bool(true)),
            name_icon_entries,
        ],
    );
    let mut icon_field = el(E2_ICON_FIELD, "Field", 40, 48, vec![]);
    icon_field
        .children
        .push(el(E2_ICON, "ExaminationIcon", 32, 32, vec![]));
    let reqs_line_colour = base_colour(m, LINE_COLOUR);
    let mut reqs_line = el(E2_REQS_LINE, "Text", 220, 40, vec![reqs_line_colour]);
    reqs_line.states = vec![state(S_CANTUSE, colours, font_colour, CANTUSE_COLOUR)];
    let slots_colour = base_colour(m, SLOTS_COLOUR);
    let member_entries = entries(m, L_EXAM2, &[E2_SET_HEADER, E2_SET_LINE]);
    let bonus_entries = entries(m, L_EXAM2, &[E2_SET_LINE]);
    let header_colour = base_colour(m, HEADER_COLOUR);
    let mut set_line = el(E2_SET_LINE, "Text", 215, 40, vec![]);
    set_line.states = vec![
        state(S_ACTIVE, colours, font_colour, ACTIVE_COLOUR),
        state(S_INACTIVE, colours, font_colour, INACTIVE_COLOUR),
    ];
    let effect_colour = base_colour(m, EFFECT_COLOUR);
    let self_entries = entries(m, L_EXAM2, &[E_SELF; 256]);
    let fan_a = entries(m, L_EXAM2, &[E_FAN_B; 256]);
    let fan_b = entries(m, L_EXAM2, &[E_FAN_C; 256]);
    let fan_c = entries(m, L_EXAM2, &[E2_NAME; 256]);
    let elements = vec![
        tooltip,
        bomb,
        name_icon,
        icon_field,
        el(
            E2_NAME,
            "ExaminationName",
            180,
            48,
            vec![(display, info(table_ref(1)))],
        ),
        el(E2_LEVEL, "ExaminationItemLevel", 220, 18, vec![]),
        el(E2_REQS, "ExaminationUsageReqs", 220, 1, vec![]),
        reqs_line,
        el(
            E2_SLOTS,
            "ExaminationAllowedSlots",
            220,
            22,
            vec![slots_colour],
        ),
        el(
            E2_SET_MEMBERS,
            "ExaminationGameSetMemberList",
            220,
            0,
            vec![member_entries],
        ),
        el(
            E2_SET_BONUS,
            "ExaminationGameSetBonusList",
            220,
            0,
            vec![bonus_entries],
        ),
        el(E2_SET_HEADER, "Text", 215, 40, vec![header_colour]),
        set_line,
        el(
            E2_TIER,
            "ExaminationGeneroProperty",
            220,
            18,
            vec![
                (generic, PropertyValue::PropertyId(tier)),
                (comparison, PropertyValue::PropertyId(tier)),
                (tier, PropertyValue::Int(2)),
                (display, info(table_ref(8))),
            ],
        ),
        el(E2_EFFECTS, "ExaminationEffectList", 220, 0, vec![]),
        el(E2_EFFECT_LINE, "Text", 220, 40, vec![effect_colour]),
        el(E_SELF, "ListBox", 220, 18, vec![self_entries]),
        el(E_FAN_A, "ListBox", 220, 18, vec![fan_a]),
        el(E_FAN_B, "ListBox", 220, 18, vec![fan_b]),
        el(E_FAN_C, "ListBox", 220, 18, vec![fan_c]),
    ];
    m.layouts.insert(
        L_EXAM2,
        Arc::new(UiLayout {
            did: L_EXAM2,
            elements,
            ..UiLayout::default()
        }),
    );

    // Records.
    let hover = |l: Did, e: u32| {
        vec![
            (hover_l, PropertyValue::DidRef(l)),
            (hover_e, PropertyValue::Enum(e)),
        ]
    };
    let mut ring = hover(L_EXAM2, E2_TOOLTIP);
    ring.extend([
        (name, info(literal("Copper Ring"))),
        (slots, PropertyValue::Bitfield32(0x2000_1800)),
        (cmp_type, PropertyValue::Enum(2)),
        (level, PropertyValue::Int(30)),
        (pending, PropertyValue::Bool(true)),
        (min_level, PropertyValue::Int(10)),
        (max_level, PropertyValue::Int(80)),
        (tier, PropertyValue::Int(2)),
        (
            mod_array,
            structs(
                mod_entry,
                vec![vec![
                    (modified, PropertyValue::PropertyId(might)),
                    (mod_op, PropertyValue::Enum(7)),
                    (might, PropertyValue::Int(7)),
                ]],
            ),
        ),
    ]);
    m.records.insert(RING, Arc::new(set(ring)));
    // `skill-tooltips.md` §9.4: a modifier line needs the property's metadata object.
    let control = Did(0x7000_0050);
    let might_meta = Did(0x7000_0051);
    m.weenies.insert("PropertyMetaDataControl".into(), control);
    m.records.insert(
        control,
        Arc::new(set(vec![(
            meta_list,
            PropertyValue::Array(PropertyArray {
                count: 1,
                elements: vec![PropertyEntry {
                    pid: 0x7778,
                    value: PropertyValue::DidRef(might_meta),
                }],
            }),
        )])),
    );
    m.records.insert(
        might_meta,
        Arc::new(set(vec![
            (meta_property, PropertyValue::PropertyId(might)),
            (meta_name, info(literal("Might"))),
        ])),
    );
    for (token, text) in [
        (0x0E44_7774, "+{0x5a6195} {0x744a2c9}"),
        (0x0F5F_0564, "{0x5a6195}"),
    ] {
        m.texts.insert((exam, token), text.to_owned());
    }
    for (d, n) in [(SET_B, "Copper Band"), (SET_C, "Copper Torc")] {
        let mut v = hover(L_EXAM, E_TOOLTIP);
        v.extend([
            (cmp_l, PropertyValue::DidRef(L_EXAM)),
            (cmp_e, PropertyValue::Enum(E_COMPARE)),
            (name, info(literal(n))),
            (level, PropertyValue::Int(40)),
        ]);
        m.records.insert(d, Arc::new(set(v)));
    }
    for d in [RING, SET_B, SET_C] {
        m.sets.insert(d, SET);
    }
    m.records.insert(
        SET,
        Arc::new(set(vec![
            (set_name, info(literal("Copper Set"))),
            (set_level, PropertyValue::Int(50)),
            (
                member_list,
                PropertyValue::Array(PropertyArray {
                    count: 3,
                    elements: [RING, SET_B, SET_C]
                        .iter()
                        .map(|&d| PropertyEntry {
                            pid: member,
                            value: PropertyValue::DidRef(d),
                        })
                        .collect(),
                }),
            ),
            (
                tiers,
                structs(
                    tier_data,
                    vec![
                        vec![
                            (active_count, PropertyValue::Int(2)),
                            (
                                mod_array,
                                structs(
                                    mod_entry,
                                    vec![vec![
                                        (modified, PropertyValue::PropertyId(might)),
                                        (mod_progression, PropertyValue::DidRef(PROG)),
                                    ]],
                                ),
                            ),
                        ],
                        vec![
                            (active_count, PropertyValue::Int(3)),
                            (
                                mod_array,
                                structs(
                                    mod_entry,
                                    vec![vec![
                                        (modified, PropertyValue::PropertyId(might)),
                                        (might, PropertyValue::Int(9)),
                                    ]],
                                ),
                            ),
                        ],
                    ],
                ),
            ),
        ])),
    );
    m.records.insert(
        PROG,
        Arc::new(set(vec![(
            linear,
            structs(
                point,
                vec![
                    vec![
                        (key, PropertyValue::Int(30)),
                        (value, PropertyValue::Float(300.0)),
                    ],
                    vec![
                        (key, PropertyValue::Int(50)),
                        (value, PropertyValue::Float(500.0)),
                    ],
                ],
            ),
        )])),
    );
    let mut recipe_item = hover(L_EXAM, E_TOOLTIP);
    recipe_item.extend([
        (name, info(literal("Recipe: Copper Sword"))),
        (cmp_type, PropertyValue::Enum(4)),
        (recipe, PropertyValue::DidRef(RECIPE)),
    ]);
    m.records.insert(RECIPE_ITEM, Arc::new(set(recipe_item)));
    m.records.insert(
        RECIPE,
        Arc::new(set(vec![
            (result, PropertyValue::DidRef(ITEM)),
            (quantity, PropertyValue::Int(3)),
        ])),
    );
    m.records
        .insert(BOMB, Arc::new(set(hover(L_EXAM2, E_BOMB_TOOLTIP))));
}

fn row<'a>(
    m: &'a tessera_gamedata::tooltip::TooltipModel,
    label: &str,
) -> &'a tessera_gamedata::tooltip::Row {
    m.rows
        .iter()
        .find(|r| r.label.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no row {label}"))
}

/// (text, state, colour) of a lines row.
fn lines_of(r: &tessera_gamedata::tooltip::Row) -> Vec<(String, Option<String>, Option<u32>)> {
    match &r.content {
        RowContent::Lines { lines, .. } => lines
            .iter()
            .map(|l| (l.text.plain_text(), l.state.clone(), l.colour))
            .collect(),
        RowContent::Empty => Vec::new(),
        other => panic!("not a lines row: {other:?}"),
    }
}

fn ring_viewer() -> Viewer {
    Viewer {
        level: Some(5),
        equipped: vec![equipped(0x800, SET_B), equipped(0x1000, SET_C)],
        ..Viewer::default()
    }
}

#[test]
fn name_and_icon_row_is_a_horizontal_list() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(RING), &ring_viewer())
        .expect("tooltip");
    assert!(m.failures.is_empty(), "{:?}", m.failures);
    let r = row(&m, "NameIcon_Row_Base");
    assert!(r.horizontal);
    let RowContent::List(entries) = &r.content else {
        panic!("{:?}", r.content);
    };
    assert_eq!(entries.len(), 2);
    assert!(matches!(
        entries[0].content,
        RowContent::List(ref icon) if matches!(icon[0].content, RowContent::Icon { item, .. } if item == RING)
    ));
    assert_eq!(entries[1].plain_text(), "Copper Ring");
    assert!(m.has_name());
    // §4.3: the row grows sideways to fit both: 40 + 180 wide, 48 high.
    let a = r.assembly_row();
    assert_eq!((a.width, a.height), (220, 48));
    // So the list is not shrunk below it (it would have been 180 as a vertical list).
    assert_eq!(m.assembly.width, 220);
}

#[test]
fn item_level_keeps_the_marker_markup_6_7() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(RING), &ring_viewer())
        .expect("tooltip");
    let r = row(&m, "ExaminationItemLevel_Base");
    let RowContent::Text(t) = &r.content else {
        panic!("{:?}", r.content);
    };
    assert_eq!(t.plain_text(), "Level 30 (+)");
    let last = t.spans().last().expect("span");
    assert_eq!(last.text, "(+)");
    assert_eq!(
        last.style.color,
        Some(Rgba {
            r: 0,
            g: 255,
            b: 0,
            a: 255
        })
    );
    assert_eq!(t.spans()[0].style.color, None);
}

#[test]
fn requirement_lines_take_the_entry_element_colours_6_15() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(RING), &ring_viewer())
        .expect("tooltip");
    assert_eq!(
        lines_of(row(&m, "UsageReqsDisplay_Base")),
        [
            (
                "Minimum level 10".to_owned(),
                Some("cantuse".to_owned()),
                Some(CANTUSE_COLOUR)
            ),
            ("Maximum level 80".to_owned(), None, Some(LINE_COLOUR)),
        ]
    );
    let t = e
        .template(TemplateRef {
            layout: L_EXAM2,
            element: E2_REQS,
        })
        .expect("template");
    // The state lives only on the entry element.
    assert!(t.state_colours.is_empty());
    assert_eq!(t.line_colour(Some("cantuse")), Some(CANTUSE_COLOUR));
}

#[test]
fn allowed_slots_skip_the_equipment_bit_6_14() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(RING), &ring_viewer())
        .expect("tooltip");
    assert_eq!(
        lines_of(row(&m, "AllowedSlots_Base")),
        [
            ("Left ring".to_owned(), None, Some(SLOTS_COLOUR)),
            ("Right ring".to_owned(), None, Some(SLOTS_COLOUR)),
        ]
    );
}

#[test]
fn effect_lines_use_the_line_element_6_16() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(RING), &ring_viewer())
        .expect("tooltip");
    // P2 revision (`item-tooltips.md` §6.16, `skill-tooltips.md` §9.4): the operator's
    // template with the metadata name, state `buffed` (no such state on the line element, so
    // its base colour).
    assert_eq!(
        lines_of(row(&m, "EffectDisplay_Base")),
        [(
            "+7 Might".to_owned(),
            Some("buffed".to_owned()),
            Some(EFFECT_COLOUR)
        )]
    );
}

#[test]
fn set_lines_states_colours_and_set_level_6_17() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(RING), &ring_viewer())
        .expect("tooltip");
    assert!(m.failures.is_empty(), "{:?}", m.failures);
    let members = row(&m, "GameSetMemberListDisplayBase");
    let RowContent::Lines { header, .. } = &members.content else {
        panic!("{:?}", members.content);
    };
    assert_eq!(
        header.as_ref().map(|h| h.text.plain_text()).as_deref(),
        Some("Copper Set")
    );
    let active = || (Some("gameset_active".to_owned()), Some(ACTIVE_COLOUR));
    let inactive = || (Some("gameset_inactive".to_owned()), Some(INACTIVE_COLOUR));
    let with = |t: &str, (s, c): (Option<String>, Option<u32>)| (t.to_owned(), s, c);
    assert_eq!(
        lines_of(members),
        [
            with("Copper Ring", inactive()),
            with("Copper Band", active()),
            with("Copper Torc", active()),
        ]
    );
    // Two of three worn: tier 2 active, tier 3 not. Tier 2's progression is read at the
    // set level 50 (`items.md` §7.1), not the ring's item level 30 (which gives 300).
    assert_eq!(
        lines_of(row(&m, "GameSetBonusListDisplayBase")),
        [
            with("2: Stat_Might: 500", active()),
            with("3: Stat_Might: 9", inactive()),
        ]
    );
}

#[test]
fn comparison_property_row_needs_an_equal_value_6_4_step_5() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(RING), &ring_viewer())
        .expect("tooltip");
    assert_eq!(row(&m, "ItemTier_Base").plain_text(), "Tier 2");
    let tier = e.data().pid("Item_Tier").expect("pid");
    let req = TooltipRequest {
        instance: Instance {
            properties: vec![PropertyEntry {
                pid: tier,
                value: PropertyValue::Int(3),
            }],
            ..Instance::default()
        },
        ..TooltipRequest::hover(RING)
    };
    let m = e.build(&req, &ring_viewer()).expect("tooltip");
    assert!(row(&m, "ItemTier_Base").is_empty());
}

#[test]
fn two_copies_of_one_ring_give_two_comparisons_8_2() {
    let e = engine();
    let viewer = Viewer {
        equipped: vec![equipped(0x800, MAIN_HAND), equipped(0x1000, MAIN_HAND)],
        ..Viewer::default()
    };
    let m = e
        .build(&TooltipRequest::hover(RING), &viewer)
        .expect("tooltip");
    assert_eq!(m.comparison.kind, CompareKind::Equipment);
    let slots: Vec<u32> = m.comparison.tooltips.iter().map(|(s, _)| *s).collect();
    assert_eq!(slots, [0x800, 0x1000]);
}

#[test]
fn recipe_comparison_carries_the_result_quantity_8_3() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(RECIPE_ITEM), &Viewer::default())
        .expect("tooltip");
    assert_eq!(m.comparison.kind, CompareKind::Recipe);
    let [(_, result)] = m.comparison.tooltips.as_slice() else {
        panic!("{:?}", m.comparison.tooltips.len());
    };
    assert_eq!(result.subject, Subject::Record(ITEM));
    // 3 × 123,405 copper = 3 gold, 702 silver, 15 copper.
    let worth = result
        .rows
        .iter()
        .find(|r| r.widget == Widget::Worth)
        .expect("worth");
    let RowContent::Money { coins, .. } = &worth.content else {
        panic!("{:?}", worth.content);
    };
    let counts: Vec<i64> = coins.iter().map(|c| c.count).collect();
    assert_eq!(counts, [3, 702, 15]);
}

#[test]
fn hostile_self_referencing_and_fanning_templates_are_bounded() {
    let e = engine();
    let t0 = std::time::Instant::now();
    let m = e
        .build(&TooltipRequest::hover(BOMB), &Viewer::default())
        .expect("tooltip");
    assert!(
        t0.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        t0.elapsed()
    );
    // The cycle and the spent node budget are row failures.
    assert!(m.failures.iter().any(|f| f.reason.contains("cycle")));
    assert!(m.failures.iter().any(|f| f.reason.contains("too many")));
    let nodes = |t: &tessera_gamedata::tooltip::RowTemplate| {
        let mut n = 0;
        let mut stack = vec![t];
        while let Some(t) = stack.pop() {
            n += 1;
            stack.extend(t.children.iter().flatten().map(|c| &**c));
        }
        n
    };
    for el in [E_SELF, E_FAN_A] {
        let t = e
            .template(TemplateRef {
                layout: L_EXAM2,
                element: el,
            })
            .expect("template");
        assert!(nodes(&t) <= tessera_gamedata::tooltip::layout::MAX_TEMPLATE_NODES);
    }
}

#[test]
fn bitfield_list_stops_at_max_lines() {
    let mut m = install();
    let category = m.known("Item_Decoration_Category");
    for k in 1..=128 {
        m.enum_texts.insert((category, k), format!("k{k}"));
    }
    let e = TooltipEngine::new(m);
    let req = TooltipRequest {
        instance: Instance {
            properties: vec![PropertyEntry {
                pid: category,
                value: PropertyValue::BitfieldAny(tessera_formats::property::BitfieldAny {
                    bits: 128,
                    bytes: vec![0xFF; 16],
                }),
            }],
            ..Instance::default()
        },
        ..TooltipRequest::hover(ITEM)
    };
    let model = e.build(&req, &Viewer::default()).expect("tooltip");
    let text = row_text(&model, "ItemDecorationCategory_Base").expect("row");
    let parts = text.trim_start_matches("Placement: ").split(", ").count();
    assert_eq!(parts, tessera_gamedata::tooltip::MAX_LINES);
}

#[test]
fn a_failed_coin_format_is_a_row_failure_with_digits() {
    let mut m = install();
    m.texts
        .remove(&(tokens::NUMBER_TABLE_KEY, tokens::COIN_COUNT));
    let e = TooltipEngine::new(m);
    let model = e
        .build(&TooltipRequest::hover(ITEM), &Viewer::default())
        .expect("tooltip");
    assert_eq!(
        row_text(&model, "Value_Base").as_deref(),
        Some("Worth: 1 234 5")
    );
    assert!(
        model
            .failures
            .iter()
            .any(|f| f.template == "Value_Base" && f.reason.contains("miss")),
        "{:?}",
        model.failures
    );
}

#[test]
fn data_unavailable_has_no_census_category() {
    assert_eq!(TooltipError::DataUnavailable("x".into()).category(), None);
}
