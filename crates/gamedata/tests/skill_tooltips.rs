// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the skill and effect tooltip model (`skill-tooltips.md` §3–§16) and of
//! the P2 revision of item tooltips (`item-tooltips.md` §6.16, §6.20, §8.3) over an in-memory
//! install: our own layouts, records and wording. No game data.

mod mem;

use std::sync::Arc;

use mem::{Mem, T_FORMATS, array, colour, info, literal, set, structs, table_ref};
use tessera_formats::property::{PropertyArray, PropertyEntry};
use tessera_formats::ui_layout::{Rect, StateBlock, UiEnum};
use tessera_formats::{Did, ElementDesc, PropertyType as T, PropertyValue as V, UiLayout};
use tessera_gamedata::tooltip::duration::{
    NUMBER_TOKEN, SHORT_TEMPLATE, TENTHS_TOKEN, UI_TABLE_KEY,
};
use tessera_gamedata::tooltip::rules::tokens;
use tessera_gamedata::tooltip::{
    CombatCategory, CombatCensus, CombatRequest, CombatSubject, CompareKind, EffectMode,
    IconCategory, LineState, LiveEffect, NameState, Row, RowContent, SkillIconSize, SkillViewer,
    TimerKind, TooltipEngine, TooltipModel, TooltipRequest, VITAL_TYPE_HEALTH, VITAL_TYPE_POWER,
    Viewer, Widget, line_tokens as lt, skill_tokens as st,
};

const L_EXAM: Did = Did(0x2200_0000);
const L_TEXT: Did = Did(0x2200_0002);

const SKILL: Did = Did(0x7000_0100);
const INDUCTION: Did = Did(0x7000_0110);
const TRAIT: Did = Did(0x7000_0111);
const EFFECT_A: Did = Did(0x7000_0120);
const EFFECT_B: Did = Did(0x7000_0121);
const EFFECT_MISSING: Did = Did(0x7000_0122);
const EFFECT_QUIET: Did = Did(0x7000_0123);
const MONSTER: Did = Did(0x7000_0130);
const ITEM: Did = Did(0x7000_0140);
const GRANTER: Did = Did(0x7000_0141);
const META_CONTROL: Did = Did(0x7000_0150);
const META_X: Did = Did(0x7000_0151);
const META_Y: Did = Did(0x7000_0152);
const SKILL_COST: Did = Did(0x7000_0160);
const SKILL_TOGGLE: Did = Did(0x7000_0161);
const CHANNEL: Did = Did(0x7000_0162);
const SKILL_IGNORES: Did = Did(0x7000_0163);
const SKILL_CRIT: Did = Did(0x7000_0164);
const COST_PROG: Did = Did(0x7000_0165);
const GRANTER_COST: Did = Did(0x7000_0166);
const EFFECT_C: Did = Did(0x7000_0124);
const EFFECT_D: Did = Did(0x7000_0125);
const EFFECT_OVERRIDE: Did = Did(0x7000_0126);
const EFFECT_PULSED: Did = Did(0x7000_0127);
const EFFECT_LEVELLED: Did = Did(0x7000_0128);
const SKILL_LEVELLED: Did = Did(0x7000_0167);
const ITEM_L50: Did = Did(0x7000_0168);
const ITEM_LNONE: Did = Did(0x7000_0169);
const ITEM_L0: Did = Did(0x7000_016A);
const ITEM_CALC: Did = Did(0x7000_016B);
const CALCULATOR: Did = Did(0x7000_016C);
const ICON: Did = Did(0x4100_0001);
const EFFECT_ICON: Did = Did(0x4100_0002);

const E_SKILL_TT: u32 = 0x1000_0001;
const E_EFFECT_TT: u32 = 0x1000_0002;
const E_ITEM_TT: u32 = 0x1000_0003;
const E_LIST: u32 = 0x1000_0004;
const E_NAME: u32 = 0x1000_0010;
const E_ICON: u32 = 0x1000_0011;
const E_SEP: u32 = 0x1000_0012;
const E_QR: u32 = 0x1000_0013;
const E_QUAL: u32 = 0x1000_0014;
const E_RANGE: u32 = 0x1000_0015;
const E_TARGETS: u32 = 0x1000_0016;
const E_CAST: u32 = 0x1000_0017;
const E_DESC: u32 = 0x1000_0018;
const E_DAMAGE: u32 = 0x1000_0019;
const E_EFFECTS: u32 = 0x1000_001A;
const E_USER: u32 = 0x1000_001B;
const E_LINE: u32 = 0x1000_001C;
const E_COST: u32 = 0x1000_001D;
const E_TRAIT: u32 = 0x1000_001E;
const E_RECOVERY: u32 = 0x1000_001F;
const E_RESIST: u32 = 0x1000_0020;
const E_EDESC: u32 = 0x1000_0021;
const E_TIME: u32 = 0x1000_0022;
const E_SI_RANGE: u32 = 0x1000_0023;
const E_SI_EFFECTS: u32 = 0x1000_0024;
const E_ITEM_EFFECTS: u32 = 0x1000_0025;
const E_COLOURS: u32 = 0x1000_0026;
const E_SPECIAL: u32 = 0x1000_0027;
const E_CRIT5: u32 = 0x1000_0028;
const E_CRIT6: u32 = 0x1000_0029;
const E_CRIT7: u32 = 0x1000_002A;
const E_SI_PLAIN: u32 = 0x1000_002B;
const E_SI_FLOOR: u32 = 0x1000_002C;

const S_BUFFED: u32 = 0x2000_0001;
const S_DEBUFFED: u32 = 0x2000_0002;
const S_DURATION: u32 = 0x2000_0003;
const S_CANTUSE: u32 = 0x2000_0004;
const S_SKILL_NAME: u32 = 0x2000_0005;
const S_EFFECT_NAME: u32 = 0x2000_0006;

const SKILL_NAME_COLOUR: u32 = 0xFFC3_B585;
const LINE_COLOUR: u32 = 0xFFCC_CC7F;
const BUFFED: u32 = 0xFF00_E500;
const DEBUFFED: u32 = 0xFFFF_0000;
const DURATION: u32 = 0xFF00_9900;
const CANTUSE: u32 = 0xFFFF_0001;

const TYPES: &[(u32, &str)] = &[
    (14, "Field"),
    (16, "Text"),
    (0x3000_0001, "ExaminationListBox"),
    (0x3000_0002, "ExaminationName"),
    (0x3000_0003, "ExaminationRow"),
    (0x3000_0004, "ExaminationIcon"),
    (0x3000_0005, "ExaminationSkillQualifier"),
    (0x3000_0006, "ExaminationSkillMaxRange"),
    (0x3000_0007, "ExaminationSkillMaxTargets"),
    (0x3000_0008, "ExaminationSkillCastingTime"),
    (0x3000_0009, "ExaminationGeneroProperty"),
    (0x3000_000A, "ExaminationSkillDamage"),
    (0x3000_000B, "ExaminationEffectList"),
    (0x3000_000C, "ExaminationSkillVitalCost"),
    (0x3000_000D, "ExaminationSkillTrait"),
    (0x3000_000E, "ExaminationSkillRecoveryTime"),
    (0x3000_000F, "ExaminationEffectResistance"),
    (0x3000_0010, "ExaminationEffectDescription"),
    (0x3000_0011, "ExaminationEffectTimeDisplay"),
    (0x3000_0012, "ExaminationSkillSpecialClass"),
    (0x3000_0013, "ExaminationSkillEffectList"),
];

fn ty(label: &str) -> u32 {
    TYPES
        .iter()
        .find(|(_, l)| *l == label)
        .map(|(k, _)| *k)
        .expect("type")
}

fn el(id: u32, type_label: &str, w: i32, h: i32, props: Vec<(u32, V)>) -> ElementDesc {
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

fn state(m: &mut Mem, id: u32, argb: u32) -> StateBlock {
    let colours = m.p("UICore_Text_font_colors", T::Array);
    let font_colour = m.p("UICore_Text_font_color", T::Colour);
    StateBlock {
        state_id: id,
        properties: set(vec![(
            colours,
            V::Array(PropertyArray {
                count: 1,
                elements: vec![PropertyEntry {
                    pid: font_colour,
                    value: colour(argb),
                }],
            }),
        )]),
        ..StateBlock::default()
    }
}

fn entries(m: &mut Mem, elements: &[u32]) -> (u32, V) {
    let templates = m.p("UICore_ListBox_entry_templates", T::Array);
    let t_layout = m.p("UICore_ListBox_entry_template_layout", T::DidRef);
    let t_element = m.p("UICore_ListBox_entry_template_element", T::Enum);
    (
        templates,
        structs(
            0x7777,
            elements
                .iter()
                .map(|&e| vec![(t_layout, V::DidRef(L_EXAM)), (t_element, V::Enum(e))])
                .collect(),
        ),
    )
}

#[allow(clippy::too_many_lines)]
fn install() -> Mem {
    let mut m = Mem::default();
    for (k, l) in TYPES {
        m.ui_label(UiEnum::ElementType, *k, l);
    }
    for (id, l) in [
        (E_SKILL_TT, "Skill_ExamineLayout"),
        (E_EFFECT_TT, "Effect_ExamineLayout"),
        (E_ITEM_TT, "SkillItem_ExamineLayout"),
        (E_NAME, "Name_Base"),
        (E_ICON, "Icon_Base"),
        (E_QR, "Skill_QualifierRange_Row_Base"),
        (E_QUAL, "Skill_Qualifier_Base"),
        (E_RANGE, "Skill_MaxRange_Base"),
        (E_TARGETS, "Skill_MaxTargets_Base"),
        (E_CAST, "Skill_CastingTime_Base"),
        (E_DESC, "Skill_Description_Base"),
        (E_DAMAGE, "Skill_Damage_Base"),
        (E_EFFECTS, "EffectDisplay_Base"),
        (E_USER, "UserEffectDisplay_Base"),
        (E_LINE, "EffectListElement"),
        (E_COST, "Skill_VitalCost_Base"),
        (E_TRAIT, "Skill_Trait_Base"),
        (E_RECOVERY, "Skill_RecoveryTime_Base"),
        (E_RESIST, "Effect_ResistanceCategory_Base"),
        (E_EDESC, "Effect_Description_Base"),
        (E_TIME, "Effect_TimeDisplay_Base"),
        (E_SI_RANGE, "SkillItem_MaxRange_Base"),
        (E_SI_EFFECTS, "SkillItem_EffectDisplay_Base"),
        (E_ITEM_EFFECTS, "ItemEffectDisplay_Base"),
        (E_COLOURS, "ColorNameText"),
        (E_SPECIAL, "Skill_SpecialClass_Base"),
        (E_CRIT5, "CriticalEffectDisplay_Base"),
        (E_CRIT6, "SuperCriticalEffectDisplay_Base"),
        (E_CRIT7, "BothCriticalEffectDisplay_Base"),
        (E_SI_PLAIN, "SkillItem_ItemEffectDisplay_Base"),
        (E_SI_FLOOR, "SkillItem_FloorEffectDisplay_Base"),
    ] {
        m.ui_label(UiEnum::ElementId, id, l);
    }
    for (id, l) in [
        (S_BUFFED, "buffed"),
        (S_DEBUFFED, "debuffed"),
        (S_DURATION, "duration"),
        (S_CANTUSE, "cantuse"),
        (S_SKILL_NAME, "Skill_ColorName"),
        (S_EFFECT_NAME, "Effect_ColorName"),
    ] {
        m.ui_label(UiEnum::StateId, id, l);
    }

    // UI properties.
    let autofill = m.p("UICore_ListBox_entry_templates_autofill", T::Bool);
    let display = m.p("UI_Examination_Tooltip_DisplayString", T::StringInfo);
    let generic = m.p("UI_Examination_Tooltip_GeneroProperty", T::PropertyId);
    let separator = m.p("UI_Examination_Tooltip_Separator", T::Bool);
    let left = m.p("UI_Examination_Tooltip_Row_Left_Child", T::Enum);
    let right = m.p("UI_Examination_Tooltip_Row_Right_Child", T::Enum);
    let for_skill_item = m.p("UI_Examination_Tooltip_For_SkillItem", T::Bool);
    let target = m.p("UI_Examination_Tooltip_SkillEffectTarget", T::Enum);
    let category = m.p("UI_Examination_Tooltip_EffectCategory", T::Bitfield32);
    let min_level = m.p("UI_SkillDisplay_SkillMinLevel", T::Int);
    let two_place = m.p("UIComponent_Timer_ShowTwoPlaceFormat", T::Bool);
    let full = m.p("UIComponent_Timer_ShowFullFormat", T::Bool);
    let colours = m.p("UICore_Text_font_colors", T::Array);
    let font_colour = m.p("UICore_Text_font_color", T::Colour);
    let hover_l = m.p("UI_Examination_Tooltip_0Click_Layout", T::DidRef);
    let hover_e = m.p("UI_Examination_Tooltip_0Click_Element", T::Enum);
    let cmp_type = m.p("UI_Examination_Tooltip_Compare_Type", T::Enum);

    // Record properties.
    let name = m.p("Name", T::StringInfo);
    let skill_name = m.p("Skill_Name", T::StringInfo);
    let small_icon = m.p("Skill_SmallIcon", T::DidRef);
    let max_range = m.p("Skill_MaxRange", T::Float);
    let max_targets = m.p("Skill_AreaEffectMaxTargets", T::Int);
    let hooks = m.p("Skill_AttackHookList", T::Array);
    let hook_info = m.p("Skill_AttackHookInfo", T::Struct);
    let qualifier = m.p("Skill_AttackHook_DamageQualifier", T::Enum);
    let hook_targets = m.p("Skill_AttackHook_TargetEffectList", T::Array);
    let user_effects = m.p("Skill_UserEffectList", T::Array);
    let effect_data = m.p("Skill_EffectData", T::Struct);
    let skill_effect = m.p("Skill_Effect", T::DidRef);
    let skill_spellcraft = m.p("Skill_EffectSpellcraft", T::Float);
    let desc = m.p("Skill_Desc", T::StringInfo);
    let recovery = m.p("Skill_SkillRecoveryTime", T::Float);
    let costs = m.p("Skill_VitalCostList", T::Array);
    let vital_info = m.p("Skill_VitalInfo", T::Struct);
    let vital_type = m.p("Skill_Vital_Type", T::Enum);
    let vital_points = m.p("Skill_Vital_Points", T::Float);
    let required_trait = m.p("Skill_RequiredTrait", T::DidRef);
    let trait_name = m.p("Trait_Name", T::StringInfo);
    let induction = m.p("Skill_InductionAction", T::DidRef);
    let induction_duration = m.p("Induction_Duration", T::Float);
    let monster = m.p("Skill_IsMonsterSkill", T::Bool);
    let effect_name = m.p("Effect_Name", T::StringInfo);
    let effect_icon = m.p("Effect_Icon", T::DidRef);
    let harmful = m.p("Effect_Harmful", T::Bool);
    let def_desc = m.p("Effect_Definition_Description", T::StringInfo);
    let applied_desc = m.p("Effect_Applied_Description", T::StringInfo);
    let procedural = m.p(
        "Effect_Display_Procedurally_Generated_Examination_Information",
        T::Bool,
    );
    let probability = m.p("Effect_ConstantApplicationProbability", T::Float);
    let interval = m.p("Effect_Duration_ConstantInterval", T::Float);
    let combat_only = m.p("Effect_Duration_CombatOnly", T::Bool);
    let resist = m.p("Effect_Resist_Category", T::Bitfield32);
    let mod_array = m.p("Mod_Array", T::Array);
    let mod_entry = m.p("Mod_Entry", T::Struct);
    let modified = m.p("Mod_Modified", T::PropertyId);
    let mod_op = m.p("Mod_Op", T::Enum);
    let stat_x = m.p("Stat_X", T::Int);
    let stat_y = m.p("Stat_Y", T::Float);
    let meta_list = m.p("PropertyMetaData_PropertyMetaDataList", T::Array);
    let meta_property = m.p("PropertyMetaData_Property", T::PropertyId);
    let meta_name = m.p("PropertyMetaData_Name", T::StringInfo);
    let meta_pct = m.p("PropertyMetaData_DisplayAsPercentage", T::Bool);
    let meta_hide = m.p("PropertyMetaData_HideIdentityExaminationMod", T::Bool);
    let usage_skill = m.p("Usage_SkillToExecute", T::DidRef);
    let granted = m.p("Item_GrantedSkill", T::DidRef);
    let item_level = m.p("Item_Level", T::Int);

    // Texts.
    let exam = tokens::EXAMINATION_TABLE_KEY;
    for (token, t) in [
        (lt::ADD, "+{0x5a6195} {0x744a2c9}"),
        (lt::SUBTRACT, "-{0x5a6195} {0x744a2c9}"),
        (lt::MULTIPLY_DECREASE, "-{0x5a6195} {0x744a2c9}"),
        (lt::VALUE_PLAIN, "{0x5a6195}"),
        (lt::VALUE_PERCENT, "{0x5a6195}%"),
        (lt::CHANCE, "{0x4a67a64}% chance"),
        (lt::COMBAT_ONLY, "Ends {0x48a069}s after combat"),
        (lt::COMBAT_ONLY_OTHER, "Combat only"),
        (lt::DURATION, "Duration: {0xa668aae}"),
        (st::CASTING_INDUCTION, "Induction: {0x58e15}"),
        (st::RECOVERY_TOTAL, "Cooldown: {0x58e15}"),
        (st::RECOVERY_REMAINING, "Ready in {0x58e15}"),
        (st::VITAL_COST, "{0x48484} {0x5ae85c}"),
        (st::MAX_TARGETS, "{0x5a6195} targets"),
        (st::VITAL_COST_PER_SECOND, "{0x48484} {0x5ae85c}/s"),
        (st::COST_JOIN, "{0x603e6f1}, {0x603e6f2}"),
        (st::CASTING_CHANNEL, "Channel: {0x58e15}"),
        (st::SPECIAL_CHANNELLED, "Channelled toggle"),
        (st::SPECIAL_TOGGLE, "Toggle"),
        (st::QUALIFIER_IMMEDIATE, "Immediate"),
        (st::QUALIFIER_IGNORES_RESET, "Ignores reset"),
        (st::QUALIFIED_IMMEDIATE, "{0x50e84}, immediate"),
        (st::QUALIFIED_IGNORES_RESET, "{0x50e84}, ignores reset"),
    ] {
        m.text(exam, token, t);
    }
    m.text(UI_TABLE_KEY, NUMBER_TOKEN, "{0x5a6195}");
    m.text(UI_TABLE_KEY, TENTHS_TOKEN, "{0x79842c3}.{0x58a38d3}");
    m.text(
        UI_TABLE_KEY,
        SHORT_TEMPLATE,
        "[{0x52438d3}|{0x486e3}|{0x4d4a73}|{0x1e3a8f3}|{0x79842c3}]",
    );
    for (token, t) in [
        (1, "{0x52615}"),
        (2, "{0x5a6195}m"),
        (3, "{0x5a6195}"),
        (4, "{0x5a6195}"),
        (5, "On you:"),
        (6, "{0x8603e64}"),
        (7, "Requires {0x5965e4}"),
        (8, "{0x689c009} {0x96a2908}"),
        (9, "{0x480683}"),
    ] {
        m.text(T_FORMATS.0, token, t);
    }
    m.enum_texts.insert((qualifier, 1), "Melee".into());
    m.enum_texts.insert((vital_type, 1), "Morale".into());
    m.enum_texts.insert((vital_type, 2), "Power".into());
    m.enum_texts.insert((resist, 5), "Fear".into());
    m.enum_keys
        .insert((cmp_type, "SkillGrantingItem".into()), 7);

    // The layout.
    let frame = |m: &mut Mem, id: u32, rows: &[u32]| {
        let templates = entries(m, rows);
        let mut f = ElementDesc {
            element_id: id,
            ..ElementDesc::default()
        };
        f.children.push(el(
            E_LIST,
            "ExaminationListBox",
            229,
            100,
            vec![templates, (autofill, V::Bool(true))],
        ));
        f
    };
    let skill_tt = frame(
        &mut m,
        E_SKILL_TT,
        &[
            E_NAME, E_ICON, E_SEP, E_QR, E_TARGETS, E_CAST, E_SEP, E_DESC, E_SEP, E_DAMAGE, E_SEP,
            E_EFFECTS, E_USER, E_SEP, E_COST, E_TRAIT, E_SEP, E_RECOVERY, E_SPECIAL, E_CRIT5,
            E_CRIT6, E_CRIT7,
        ],
    );
    let effect_tt = frame(&mut m, E_EFFECT_TT, &[E_NAME, E_RESIST, E_EDESC, E_TIME]);
    let item_tt = frame(
        &mut m,
        E_ITEM_TT,
        &[
            E_NAME,
            E_SI_RANGE,
            E_SI_EFFECTS,
            E_ITEM_EFFECTS,
            E_SI_PLAIN,
            E_SI_FLOOR,
        ],
    );
    let line_entries = entries(&mut m, &[E_LINE]);
    let line_colour = (
        colours,
        V::Array(PropertyArray {
            count: 1,
            elements: vec![PropertyEntry {
                pid: font_colour,
                value: colour(LINE_COLOUR),
            }],
        }),
    );
    let mut line = el(E_LINE, "Text", 220, 40, vec![line_colour]);
    line.states = vec![
        state(&mut m, S_BUFFED, BUFFED),
        state(&mut m, S_DEBUFFED, DEBUFFED),
        state(&mut m, S_DURATION, DURATION),
    ];
    let mut trait_row = el(
        E_TRAIT,
        "ExaminationSkillTrait",
        229,
        18,
        vec![(display, info(table_ref(7)))],
    );
    trait_row.states = vec![state(&mut m, S_CANTUSE, CANTUSE)];
    let elements = vec![
        skill_tt,
        effect_tt,
        item_tt,
        el(
            E_NAME,
            "ExaminationName",
            180,
            22,
            vec![(display, info(table_ref(1)))],
        ),
        el(E_ICON, "ExaminationIcon", 32, 32, vec![]),
        el(E_SEP, "Field", 220, 6, vec![(separator, V::Bool(true))]),
        el(
            E_QR,
            "ExaminationRow",
            200,
            18,
            vec![(left, V::Enum(E_QUAL)), (right, V::Enum(E_RANGE))],
        ),
        el(E_QUAL, "ExaminationSkillQualifier", 229, 18, vec![]),
        el(
            E_RANGE,
            "ExaminationSkillMaxRange",
            229,
            18,
            vec![(display, info(table_ref(2)))],
        ),
        el(
            E_TARGETS,
            "ExaminationSkillMaxTargets",
            229,
            18,
            vec![(display, info(table_ref(3)))],
        ),
        el(E_CAST, "ExaminationSkillCastingTime", 229, 18, vec![]),
        el(
            E_DESC,
            "ExaminationGeneroProperty",
            229,
            18,
            vec![
                (generic, V::PropertyId(desc)),
                (display, info(table_ref(4))),
            ],
        ),
        el(E_DAMAGE, "ExaminationSkillDamage", 229, 18, vec![]),
        el(
            E_EFFECTS,
            "ExaminationEffectList",
            220,
            0,
            vec![(target, V::Enum(3)), line_entries.clone()],
        ),
        el(
            E_USER,
            "ExaminationEffectList",
            220,
            0,
            vec![
                (target, V::Enum(1)),
                (display, info(table_ref(5))),
                line_entries.clone(),
            ],
        ),
        line,
        el(
            E_COST,
            "ExaminationSkillVitalCost",
            229,
            18,
            vec![(display, info(table_ref(6)))],
        ),
        trait_row,
        el(
            E_RECOVERY,
            "ExaminationSkillRecoveryTime",
            229,
            18,
            vec![(two_place, V::Bool(true))],
        ),
        el(
            E_RESIST,
            "ExaminationEffectResistance",
            220,
            18,
            vec![(display, info(table_ref(8)))],
        ),
        el(
            E_EDESC,
            "ExaminationEffectDescription",
            220,
            40,
            vec![line_entries.clone()],
        ),
        el(
            E_TIME,
            "ExaminationEffectTimeDisplay",
            220,
            40,
            vec![(full, V::Bool(true))],
        ),
        el(
            E_SPECIAL,
            "ExaminationSkillSpecialClass",
            229,
            18,
            vec![(display, info(table_ref(9)))],
        ),
        el(
            E_CRIT5,
            "ExaminationEffectList",
            220,
            0,
            vec![(target, V::Enum(5)), line_entries.clone()],
        ),
        el(
            E_CRIT6,
            "ExaminationEffectList",
            220,
            0,
            vec![(target, V::Enum(6)), line_entries.clone()],
        ),
        el(
            E_CRIT7,
            "ExaminationEffectList",
            220,
            0,
            vec![(target, V::Enum(7)), line_entries.clone()],
        ),
        el(
            E_SI_RANGE,
            "ExaminationSkillMaxRange",
            229,
            18,
            vec![
                (display, info(table_ref(2))),
                (for_skill_item, V::Bool(true)),
            ],
        ),
        el(
            E_SI_EFFECTS,
            "ExaminationSkillEffectList",
            220,
            0,
            vec![
                (target, V::Enum(3)),
                (for_skill_item, V::Bool(true)),
                line_entries.clone(),
            ],
        ),
        // §6.1 (ST23): a class-1062 row with the skill-item flag still examines the item.
        el(
            E_SI_PLAIN,
            "ExaminationEffectList",
            220,
            0,
            vec![
                (category, V::Bitfield32(0x10)),
                (for_skill_item, V::Bool(true)),
                line_entries.clone(),
            ],
        ),
        // §9.8 *V*₀: a skill-item list whose row set carries a minimum level.
        el(
            E_SI_FLOOR,
            "ExaminationSkillEffectList",
            220,
            0,
            vec![
                (target, V::Enum(3)),
                (for_skill_item, V::Bool(true)),
                (min_level, V::Int(50)),
                line_entries.clone(),
            ],
        ),
        el(
            E_ITEM_EFFECTS,
            "ExaminationEffectList",
            220,
            0,
            vec![(category, V::Bitfield32(0x10)), line_entries],
        ),
    ];
    m.layouts.insert(
        L_EXAM,
        Arc::new(UiLayout {
            did: L_EXAM,
            elements,
            ..UiLayout::default()
        }),
    );
    // The name colour table (`item-tooltips.md` §6.2).
    let mut colour_el = el(E_COLOURS, "Text", 100, 20, vec![]);
    colour_el.states = vec![
        state(&mut m, S_SKILL_NAME, SKILL_NAME_COLOUR),
        state(&mut m, S_EFFECT_NAME, 0xFFC3_B586),
    ];
    m.layouts.insert(
        L_TEXT,
        Arc::new(UiLayout {
            did: L_TEXT,
            elements: vec![colour_el],
            ..UiLayout::default()
        }),
    );
    m.layout_labels.insert("text".into(), L_TEXT);

    // Records.
    let hover = |e: u32| vec![(hover_l, V::DidRef(L_EXAM)), (hover_e, V::Enum(e))];
    let effect_ref = |d: Did| {
        vec![
            (skill_effect, V::DidRef(d)),
            (skill_spellcraft, V::Float(-1.0)),
        ]
    };
    let mut skill = hover(E_SKILL_TT);
    skill.extend([
        (skill_name, info(literal("Strike"))),
        (small_icon, V::DidRef(ICON)),
        (max_range, V::Float(25.0)),
        (max_targets, V::Int(1)),
        (
            hooks,
            structs(
                hook_info,
                vec![vec![
                    (qualifier, V::Enum(1)),
                    (
                        hook_targets,
                        structs(effect_data, vec![effect_ref(EFFECT_A)]),
                    ),
                ]],
            ),
        ),
        (
            user_effects,
            structs(effect_data, vec![effect_ref(EFFECT_B)]),
        ),
        (desc, info(literal("Hits hard."))),
        (recovery, V::Float(30.0)),
        (
            costs,
            structs(
                vital_info,
                vec![vec![
                    (vital_type, V::Enum(2)),
                    (vital_points, V::Float(12.4)),
                ]],
            ),
        ),
        (required_trait, V::DidRef(TRAIT)),
        (induction, V::DidRef(INDUCTION)),
    ]);
    m.record(SKILL, skill);
    // 1.56 rather than 1.55: a stored 1.55 is 1.5499999523 in single precision and gives
    // t = 5 by §10.1 step 1 (§16.3).
    m.record(INDUCTION, vec![(induction_duration, V::Float(1.56))]);
    m.record(TRAIT, vec![(trait_name, info(literal("Valour")))]);
    let mods = |x: i32| {
        structs(
            mod_entry,
            vec![
                vec![
                    (modified, V::PropertyId(stat_x)),
                    (mod_op, V::Enum(7)),
                    (stat_x, V::Int(x)),
                ],
                vec![
                    (modified, V::PropertyId(stat_y)),
                    (mod_op, V::Enum(8)),
                    (stat_y, V::Float(0.9)),
                ],
            ],
        )
    };
    let mut a = hover(E_EFFECT_TT);
    a.extend([
        (effect_name, info(literal("Weakness"))),
        (effect_icon, V::DidRef(EFFECT_ICON)),
        (harmful, V::Bool(true)),
        (def_desc, info(literal(""))),
        (applied_desc, info(literal("Weakened."))),
        (mod_array, mods(-15)),
        (probability, V::Float(0.25)),
        (interval, V::Float(95.0)),
        (combat_only, V::Bool(true)),
        (resist, V::Bitfield32(0x50)),
    ]);
    m.record(EFFECT_A, a);
    m.classes.insert(EFFECT_A, 734);
    m.record(
        EFFECT_B,
        vec![
            (effect_name, info(literal("Mending"))),
            (harmful, V::Bool(false)),
            (def_desc, info(literal("Heals you."))),
        ],
    );
    m.classes.insert(EFFECT_B, 755);
    let mut missing = hover(E_EFFECT_TT);
    missing.push((effect_name, info(table_ref(0x999))));
    m.record(EFFECT_MISSING, missing);
    m.record(
        EFFECT_QUIET,
        vec![
            (effect_name, info(literal("Quiet"))),
            (procedural, V::Bool(false)),
            (mod_array, mods(3)),
        ],
    );
    m.classes.insert(EFFECT_QUIET, 734);
    m.record(
        MONSTER,
        vec![
            (skill_name, info(literal("Bite"))),
            (monster, V::Bool(true)),
        ],
    );
    let mut item = hover(E_ITEM_TT);
    item.extend([
        (name, info(literal("Charm"))),
        (usage_skill, V::DidRef(SKILL)),
        (item_level, V::Int(10)),
        (
            mod_array,
            structs(
                mod_entry,
                vec![vec![
                    (modified, V::PropertyId(stat_x)),
                    (mod_op, V::Enum(7)),
                    (stat_x, V::Int(7)),
                ]],
            ),
        ),
    ]);
    m.record(ITEM, item);
    let mut granter = hover(E_ITEM_TT);
    granter.extend([
        (name, info(literal("Scroll"))),
        (cmp_type, V::Enum(7)),
        (granted, V::DidRef(SKILL)),
    ]);
    m.record(GRANTER, granter);
    // Property metadata (`misc-records.md` §13.1).
    m.weenies
        .insert("PropertyMetaDataControl".into(), META_CONTROL);
    m.record(
        META_CONTROL,
        vec![(
            meta_list,
            array(0x7779, vec![V::DidRef(META_X), V::DidRef(META_Y)]),
        )],
    );
    m.record(
        META_X,
        vec![
            (meta_property, V::PropertyId(stat_x)),
            (meta_name, info(literal("Might"))),
            (meta_hide, V::Bool(true)),
        ],
    );
    m.record(
        META_Y,
        vec![
            (meta_property, V::PropertyId(stat_y)),
            (meta_name, info(literal("Armour"))),
            (meta_pct, V::Bool(true)),
            (meta_hide, V::Bool(true)),
        ],
    );
    more_records(&mut m, stat_y);
    m
}

/// Records for the rules the first install leaves out: progression, percent and per-second
/// costs (§7.10), toggles and channels (§7.6, §7.11), qualifier tokens (§7.2), the critical
/// lists (§9.6), the override line (§9.4 step 5, §16.2 (c)), the suppressed chance (§9.5), and
/// permanent and pulsed durations (§9.5).
#[allow(clippy::too_many_lines)]
fn more_records(m: &mut Mem, stat_y: u32) {
    let hover_l = m.p("UI_Examination_Tooltip_0Click_Layout", T::DidRef);
    let hover_e = m.p("UI_Examination_Tooltip_0Click_Element", T::Enum);
    let hover = |e: u32| vec![(hover_l, V::DidRef(L_EXAM)), (hover_e, V::Enum(e))];
    let skill_name = m.p("Skill_Name", T::StringInfo);
    let costs = m.p("Skill_VitalCostList", T::Array);
    let per_second = m.p("Skill_Toggle_VitalCostPerSecondList", T::Array);
    let vital_info = m.p("Skill_VitalInfo", T::Struct);
    let vital_type = m.p("Skill_Vital_Type", T::Enum);
    let vital_points = m.p("Skill_Vital_Points", T::Float);
    let vital_prog = m.p("Skill_Vital_Points_Progression", T::DidRef);
    let vital_percent = m.p("Skill_Vital_Percent", T::Float);
    let consumes_all = m.p("Skill_Vital_Consumes_All", T::Bool);
    let toggle_hook = m.p("Skill_Toggle_Hook_Number", T::Int);
    let channelling = m.p("Skill_Toggle_ChannelingState", T::DidRef);
    let channel_duration = m.p("Channeling_Duration", T::Float);
    let immediate = m.p("Skill_Immediate", T::Bool);
    let ignores_reset = m.p("Skill_IgnoresResetTime", T::Bool);
    let hooks = m.p("Skill_AttackHookList", T::Array);
    let hook_info = m.p("Skill_AttackHookInfo", T::Struct);
    let qualifier = m.p("Skill_AttackHook_DamageQualifier", T::Enum);
    let crit = m.p("Skill_AttackHook_CriticalTargetEffectList", T::Array);
    let super_crit = m.p("Skill_AttackHook_SuperCriticalTargetEffectList", T::Array);
    let effect_data = m.p("Skill_EffectData", T::Struct);
    let skill_effect = m.p("Skill_Effect", T::DidRef);
    let linear = m.p("LinearInterpolatingProgression_Array", T::Array);
    let point = m.p("LinearInterpolatingProgression_ControlPoint", T::Struct);
    let key = m.p("LinearInterpolatingProgression_Key", T::Int);
    let value = m.p("LinearInterpolatingProgression_Value", T::Float);
    let effect_name = m.p("Effect_Name", T::StringInfo);
    let harmful = m.p("Effect_Harmful", T::Bool);
    let def_desc = m.p("Effect_Definition_Description", T::StringInfo);
    let probability = m.p("Effect_ConstantApplicationProbability", T::Float);
    let suppress = m.p("Effect_SuppressApplicationProbabilityExamination", T::Bool);
    let interval = m.p("Effect_Duration_ConstantInterval", T::Float);
    let pulses = m.p("Effect_Duration_ConstantPulseCount", T::Int);
    let permanent = m.p("Effect_Duration_Permanent", T::Bool);
    let mod_array = m.p("Mod_Array", T::Array);
    let mod_entry = m.p("Mod_Entry", T::Struct);
    let modified = m.p("Mod_Modified", T::PropertyId);
    let mod_op = m.p("Mod_Op", T::Enum);
    let override_text = m.p("Mod_DescriptionOverride", T::StringInfo);
    let granted = m.p("Item_GrantedSkill", T::DidRef);
    let name = m.p("Name", T::StringInfo);
    let cmp_type = m.p("UI_Examination_Tooltip_Compare_Type", T::Enum);

    // §7.10: Morale 10 % of the maximum, Power from a progression beside a stored 0 (as the
    // install stores them), a consumes-all entry, and a per-second Power cost.
    m.record(
        COST_PROG,
        vec![(
            linear,
            structs(
                point,
                vec![
                    vec![(key, V::Int(1)), (value, V::Float(10.0))],
                    vec![(key, V::Int(50)), (value, V::Float(500.0))],
                ],
            ),
        )],
    );
    let mut cost_skill = hover(E_SKILL_TT);
    cost_skill.extend([
        (skill_name, info(literal("Drain"))),
        (
            costs,
            structs(
                vital_info,
                vec![
                    vec![
                        (vital_type, V::Enum(2)),
                        (vital_points, V::Float(0.0)),
                        (vital_prog, V::DidRef(COST_PROG)),
                    ],
                    vec![(vital_type, V::Enum(1)), (vital_percent, V::Float(0.1))],
                    vec![
                        (vital_type, V::Enum(3)),
                        (vital_points, V::Float(50.0)),
                        (consumes_all, V::Bool(true)),
                    ],
                ],
            ),
        ),
        (
            per_second,
            structs(
                vital_info,
                vec![vec![
                    (vital_type, V::Enum(2)),
                    (vital_points, V::Float(2.0)),
                ]],
            ),
        ),
    ]);
    m.record(SKILL_COST, cost_skill);
    let mut granter = hover(E_ITEM_TT);
    granter.extend([
        (name, info(literal("Tome"))),
        (cmp_type, V::Enum(7)),
        (granted, V::DidRef(SKILL_COST)),
    ]);
    m.record(GRANTER_COST, granter);

    // §7.6 step 2, §7.11, §7.2: a channelled, immediate toggle without hooks.
    m.record(CHANNEL, vec![(channel_duration, V::Float(4.0))]);
    let mut toggle = hover(E_SKILL_TT);
    toggle.extend([
        (skill_name, info(literal("Hum"))),
        (toggle_hook, V::Int(1)),
        (channelling, V::DidRef(CHANNEL)),
        (immediate, V::Bool(true)),
    ]);
    m.record(SKILL_TOGGLE, toggle);
    // A plain toggle with a qualified hook that ignores the reset time.
    let mut ignores = hover(E_SKILL_TT);
    ignores.extend([
        (skill_name, info(literal("Guard"))),
        (toggle_hook, V::Int(1)),
        (ignores_reset, V::Bool(true)),
        (
            hooks,
            structs(hook_info, vec![vec![(qualifier, V::Enum(1))]]),
        ),
    ]);
    m.record(SKILL_IGNORES, ignores);

    // §9.6 targets 5–7: B only critical, D only devastating, C both; a placeholder skipped.
    let refs = |ds: &[Did]| {
        structs(
            effect_data,
            ds.iter()
                .map(|&d| vec![(skill_effect, V::DidRef(d))])
                .collect(),
        )
    };
    let mut crit_skill = hover(E_SKILL_TT);
    crit_skill.extend([
        (skill_name, info(literal("Slash"))),
        (
            hooks,
            structs(
                hook_info,
                vec![vec![
                    (crit, refs(&[EFFECT_B, Did(0), EFFECT_C])),
                    (super_crit, refs(&[EFFECT_C, EFFECT_D])),
                ]],
            ),
        ),
    ]);
    m.record(SKILL_CRIT, crit_skill);
    for (d, n, text) in [(EFFECT_C, "Burn", "Burns."), (EFFECT_D, "Stun", "Stuns.")] {
        m.record(
            d,
            vec![
                (effect_name, info(literal(n))),
                (harmful, V::Bool(false)),
                (def_desc, info(literal(text))),
            ],
        );
        m.classes.insert(d, 755);
    }

    // §9.4 step 5 (§16.2 (c)): an override on a percentage property, value 0.05; the chance
    // is suppressed and the effect is permanent (no chance, no duration line).
    m.record(
        EFFECT_OVERRIDE,
        vec![
            (effect_name, info(literal("Thrift"))),
            (harmful, V::Bool(false)),
            (
                mod_array,
                structs(
                    mod_entry,
                    vec![vec![
                        (modified, V::PropertyId(stat_y)),
                        (mod_op, V::Enum(7)),
                        (stat_y, V::Float(0.05)),
                        (override_text, info(literal("Saves {0x6d04635} power"))),
                    ]],
                ),
            ),
            (probability, V::Float(0.25)),
            (suppress, V::Bool(true)),
            (interval, V::Float(95.0)),
            (permanent, V::Bool(true)),
        ],
    );
    m.classes.insert(EFFECT_OVERRIDE, 734);
    // A pulsed effect: 5 pulses of 2 s.
    m.record(
        EFFECT_PULSED,
        vec![
            (effect_name, info(literal("Bleed"))),
            (harmful, V::Bool(false)),
            (def_desc, info(literal("Ticks."))),
            (interval, V::Float(2.0)),
            (pulses, V::Int(5)),
        ],
    );
    m.classes.insert(EFFECT_PULSED, 755);

    // §9.8 (ST24): a skill whose target effect lasts by a progression (10 s at level 1, 500 s
    // at level 50), and skill items storing levels 50, none and 0, and one naming a
    // calculator beside a stored 50.
    let interval_prog = m.p("Effect_Duration_ProgressionInterval", T::DidRef);
    let hook_targets = m.p("Skill_AttackHook_TargetEffectList", T::Array);
    let spellcraft = m.p("Skill_EffectSpellcraft", T::Float);
    let usage_skill = m.p("Usage_SkillToExecute", T::DidRef);
    let usage_level = m.p("Usage_SkillLevel", T::Int);
    let calculator = m.p("SkillLevelCalculator", T::DidRef);
    m.record(
        EFFECT_LEVELLED,
        vec![
            (effect_name, info(literal("Ward"))),
            (harmful, V::Bool(false)),
            (def_desc, info(literal("Wards."))),
            (interval_prog, V::DidRef(COST_PROG)),
        ],
    );
    m.classes.insert(EFFECT_LEVELLED, 755);
    let mut levelled = hover(E_SKILL_TT);
    levelled.extend([
        (skill_name, info(literal("Ward"))),
        (
            hooks,
            structs(
                hook_info,
                vec![vec![(
                    hook_targets,
                    structs(
                        effect_data,
                        vec![vec![
                            (skill_effect, V::DidRef(EFFECT_LEVELLED)),
                            (spellcraft, V::Float(-1.0)),
                        ]],
                    ),
                )]],
            ),
        ),
    ]);
    m.record(SKILL_LEVELLED, levelled);
    for (did, level, calc) in [
        (ITEM_L50, Some(50), None),
        (ITEM_LNONE, None, None),
        (ITEM_L0, Some(0), None),
        (ITEM_CALC, Some(50), Some(CALCULATOR)),
    ] {
        let mut item = hover(E_ITEM_TT);
        item.extend([
            (name, info(literal("Rune"))),
            (usage_skill, V::DidRef(SKILL_LEVELLED)),
        ]);
        if let Some(l) = level {
            item.push((usage_level, V::Int(l)));
        }
        if let Some(c) = calc {
            item.push((calculator, V::DidRef(c)));
        }
        m.record(did, item);
    }
}

fn engine() -> TooltipEngine<Mem> {
    TooltipEngine::new(install())
}

fn row<'a>(m: &'a TooltipModel, label: &str) -> &'a Row {
    m.rows
        .iter()
        .find(|r| r.label.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no row {label}"))
}

fn lines(r: &Row) -> Vec<(String, Option<String>, Option<u32>)> {
    match &r.content {
        RowContent::Lines { lines, .. } => lines
            .iter()
            .map(|l| (l.text.plain_text(), l.state.clone(), l.colour))
            .collect(),
        RowContent::Empty => Vec::new(),
        other => panic!("not a lines row: {other:?}"),
    }
}

fn l(t: &str, s: Option<&str>, c: u32) -> (String, Option<String>, Option<u32>) {
    (t.to_owned(), s.map(str::to_owned), Some(c))
}

fn skill_tooltip(viewer: &SkillViewer) -> TooltipModel {
    engine()
        .build_combat(&CombatRequest::hover(CombatSubject::Skill(SKILL)), viewer)
        .expect("skill tooltip")
}

#[test]
fn skill_rows_7() {
    let m = skill_tooltip(&SkillViewer::default());
    assert!(m.failures.is_empty(), "{:?}", m.failures);
    let name = row(&m, "Name_Base");
    assert_eq!(name.plain_text(), "Strike");
    assert_eq!(name.state.as_deref(), Some("Skill_ColorName"));
    assert_eq!(name.colour, Some(SKILL_NAME_COLOUR));
    assert_eq!(row(&m, "Icon_Base").content, RowContent::Image(ICON));
    assert_eq!(
        row(&m, "Skill_QualifierRange_Row_Base").plain_text(),
        "Melee  25m"
    );
    // §7.4: one target → empty.
    assert!(row(&m, "Skill_MaxTargets_Base").is_empty());
    // §7.6, §16.3: 1.56 s with tenths.
    assert_eq!(
        row(&m, "Skill_CastingTime_Base").plain_text(),
        "Induction: [||||1.6]"
    );
    assert_eq!(row(&m, "Skill_Description_Base").plain_text(), "Hits hard.");
    // §7.9: the damage row is left empty in P2.
    let damage = row(&m, "Skill_Damage_Base");
    assert!(matches!(damage.widget, Widget::Deferred(_)));
    assert!(damage.is_empty());
    // §7.10: round(0 × percent + 12.4).
    assert_eq!(row(&m, "Skill_VitalCost_Base").plain_text(), "12 Power");
    // §7.13: `cantuse` (the sandbox reports no traits).
    let tr = row(&m, "Skill_Trait_Base");
    assert_eq!(tr.plain_text(), "Requires Valour");
    assert_eq!(tr.state.as_deref(), Some("cantuse"));
    assert_eq!(tr.colour, Some(CANTUSE));
    // §7.15: the total, statically.
    assert_eq!(
        row(&m, "Skill_RecoveryTime_Base").plain_text(),
        "Cooldown: [||||30]"
    );
    assert_eq!(m.timers.len(), 1);
    let t = &m.timers[0];
    assert_eq!(t.kind, TimerKind::Recovery);
    assert!(!t.counting);
    assert_eq!(
        m.rows[t.row].label.as_deref(),
        Some("Skill_RecoveryTime_Base")
    );
}

#[test]
fn skill_effect_lists_9_6() {
    let m = skill_tooltip(&SkillViewer::default());
    // Target list: the class-734 composition of a harmful effect (§9.2, §9.4, §9.5, §16.2).
    assert_eq!(
        lines(row(&m, "EffectDisplay_Base")),
        [
            l("-15 Might", Some("debuffed"), DEBUFFED),
            l("-10% Armour", Some("debuffed"), DEBUFFED),
            l("25% chance", Some("debuffed"), DEBUFFED),
            l("Ends 9s after combat", Some("duration"), DURATION),
            l("Duration: [|||1|35]", Some("duration"), DURATION),
        ]
    );
    // User list: the base composition with a header (§9.6 Tessera rule).
    let user = row(&m, "UserEffectDisplay_Base");
    assert_eq!(lines(user), [l("Heals you.", None, LINE_COLOUR)]);
    let RowContent::Lines { header, .. } = &user.content else {
        panic!("lines");
    };
    assert_eq!(
        header.as_ref().map(|h| h.text.plain_text()).as_deref(),
        Some("On you:")
    );
}

#[test]
fn recovery_counts_a_running_cooldown_7_15() {
    let viewer = SkillViewer {
        cooldowns: vec![(SKILL, 12.5)],
        ..SkillViewer::default()
    };
    let e = engine();
    let m = e
        .build_combat(&CombatRequest::hover(CombatSubject::Skill(SKILL)), &viewer)
        .expect("tooltip");
    assert_eq!(
        row(&m, "Skill_RecoveryTime_Base").plain_text(),
        "Ready in [||||12]"
    );
    let t = &m.timers[0];
    assert!(t.counting);
    assert_eq!(
        e.timer_text(t, 5.0).map(|x| x.plain_text()).as_deref(),
        Some("Ready in [||||7]")
    );
    assert_eq!(t.seconds_after(20.0), 0.0);
}

#[test]
fn effect_tooltip_with_a_live_effect_8() {
    let e = engine();
    let live = LiveEffect {
        effect_id: 5,
        holder: 9,
        elapsed: 10.0,
        duration: Some(3735.0),
        spellcraft: Some(50.0),
        caster_is_viewer: false,
    };
    let req = CombatRequest {
        live: Some(live),
        ..CombatRequest::hover(CombatSubject::Effect(EFFECT_A))
    };
    let m = e
        .build_combat(&req, &SkillViewer::default())
        .expect("tooltip");
    assert!(m.failures.is_empty(), "{:?}", m.failures);
    assert_eq!(row(&m, "Name_Base").plain_text(), "Weakness");
    assert_eq!(row(&m, "Name_Base").colour, Some(0xFFC3_B586));
    // §8.2, §16.4: the lowest set bit (key 5), spellcraft 50.
    assert_eq!(
        row(&m, "Effect_ResistanceCategory_Base").plain_text(),
        "Fear 50"
    );
    // §8.3: applied mode: description, modifiers, no chance, another caster's combat line.
    assert_eq!(
        lines(row(&m, "Effect_Description_Base")),
        [
            l("Weakened.", None, LINE_COLOUR),
            l("-15 Might", Some("debuffed"), DEBUFFED),
            l("-10% Armour", Some("debuffed"), DEBUFFED),
            l("Combat only", Some("duration"), DURATION),
            l("Duration: [|||1|35]", Some("duration"), DURATION),
        ]
    );
    // §8.4: the full format of 3,725 s.
    assert_eq!(row(&m, "Effect_TimeDisplay_Base").plain_text(), "[||1|2|5]");
    assert_eq!(m.timers[0].kind, TimerKind::EffectTime);
    assert!(m.timers[0].counting);
    // Without a live effect, the resistance and time rows are empty.
    let m = e
        .build_combat(
            &CombatRequest::hover(CombatSubject::Effect(EFFECT_A)),
            &SkillViewer::default(),
        )
        .expect("tooltip");
    assert!(row(&m, "Effect_ResistanceCategory_Base").is_empty());
    assert!(row(&m, "Effect_TimeDisplay_Base").is_empty());
    assert_eq!(
        lines(row(&m, "Effect_Description_Base"))[3],
        l("Ends 9s after combat", Some("duration"), DURATION)
    );
}

#[test]
fn compositions_9_2() {
    let e = engine();
    // A property-modification effect with the procedural flag false shows no line.
    let q = e.effect_lines(EFFECT_QUIET, EffectMode::Definition, 1, true);
    assert!(q.lines.is_empty());
    // The base composition in applied mode without an applied description: nothing.
    let b = e.effect_lines(EFFECT_B, EffectMode::Applied, 1, true);
    assert!(b.lines.is_empty());
    let b = e.effect_lines(EFFECT_B, EffectMode::Definition, 1, true);
    assert_eq!(b.lines.len(), 1);
    assert_eq!(b.lines[0].state, LineState::Plain);
}

#[test]
fn skill_item_rows_and_item_stat_lines_6_16_6_20() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(ITEM), &Viewer::default())
        .expect("item tooltip");
    assert!(m.failures.is_empty(), "{:?}", m.failures);
    assert_eq!(row(&m, "Name_Base").plain_text(), "Charm");
    assert_eq!(row(&m, "SkillItem_MaxRange_Base").plain_text(), "25m");
    assert_eq!(lines(row(&m, "SkillItem_EffectDisplay_Base")).len(), 5);
    assert_eq!(
        lines(row(&m, "ItemEffectDisplay_Base")),
        [l("+7 Might", Some("buffed"), BUFFED)]
    );
    // §6.1 (ST23): the effect-list provider 1062 is no child of 1079, so its row examines the
    // item even with the skill-item flag.
    assert_eq!(
        lines(row(&m, "SkillItem_ItemEffectDisplay_Base")),
        [l("+7 Might", Some("buffed"), BUFFED)]
    );
}

/// The duration line of a skill-item effect row (the effect of `SKILL_LEVELLED`).
fn duration_line(m: &TooltipModel, label: &str) -> Option<String> {
    lines(row(m, label))
        .into_iter()
        .map(|(t, _, _)| t)
        .find(|t| t.starts_with("Duration"))
}

#[test]
fn skill_item_effect_lines_use_the_item_skill_level_9_8() {
    let e = engine();
    let build = |did: Did, level: Option<i32>| {
        let viewer = Viewer {
            level,
            ..Viewer::default()
        };
        let m = e
            .build(&TooltipRequest::hover(did), &viewer)
            .expect("item tooltip");
        assert!(m.failures.is_empty(), "{:?}", m.failures);
        m
    };
    let at_50 = Some("Duration: [|||8|20]".to_owned());
    let at_1 = Some("Duration: [||||10]".to_owned());
    // A stored Usage_SkillLevel wins over the viewer's level.
    let m = build(ITEM_L50, Some(1));
    assert_eq!(duration_line(&m, "SkillItem_EffectDisplay_Base"), at_50);
    // ... and over the row set's minimum level (it replaces *V*₀).
    assert_eq!(
        duration_line(&m, "SkillItem_FloorEffectDisplay_Base"),
        at_50
    );
    // None stored: the viewer's level (1 when unknown), raised to the row's minimum.
    let m = build(ITEM_LNONE, None);
    assert_eq!(duration_line(&m, "SkillItem_EffectDisplay_Base"), at_1);
    assert_eq!(
        duration_line(&m, "SkillItem_FloorEffectDisplay_Base"),
        at_50
    );
    let m = build(ITEM_LNONE, Some(50));
    assert_eq!(duration_line(&m, "SkillItem_EffectDisplay_Base"), at_50);
    // A row level of 0 leaves the row empty.
    let m = build(ITEM_L0, Some(50));
    assert!(row(&m, "SkillItem_EffectDisplay_Base").is_empty());
    assert!(row(&m, "SkillItem_FloorEffectDisplay_Base").is_empty());
    // Tessera rule: a calculator is not evaluated; the item reads as storing no level.
    let m = build(ITEM_CALC, Some(1));
    assert_eq!(duration_line(&m, "SkillItem_EffectDisplay_Base"), at_1);

    // The census view of the same items (ST24).
    let level_of = |did: Did| {
        let set = tessera_gamedata::tooltip::TooltipData::record(e.data(), did)
            .ok()
            .flatten()
            .expect("item");
        e.skill_item_level(&set)
    };
    let l50 = level_of(ITEM_L50);
    assert_eq!(
        (l50.skill, l50.stored, l50.calculator, l50.level()),
        (Some(SKILL_LEVELLED), Some(50), None, 50)
    );
    assert_eq!(level_of(ITEM_LNONE).level(), -1);
    assert_eq!(level_of(ITEM_LNONE).row_level(7), 7);
    assert_eq!(level_of(ITEM_L0).row_level(7), 0);
    let calc = level_of(ITEM_CALC);
    assert_eq!((calc.calculator, calc.level()), (Some(CALCULATOR), -1));
    assert_eq!(level_of(ITEM).skill, Some(SKILL));
    assert_eq!(level_of(GRANTER).skill, None);

    // The skill tooltip itself uses the list level *V* (the viewer's, at least 1).
    let skill = |level| {
        e.build_combat(
            &CombatRequest::hover(CombatSubject::Skill(SKILL_LEVELLED)),
            &SkillViewer {
                level,
                ..SkillViewer::default()
            },
        )
        .expect("skill tooltip")
    };
    assert_eq!(duration_line(&skill(Some(50)), "EffectDisplay_Base"), at_50);
    assert_eq!(duration_line(&skill(Some(0)), "EffectDisplay_Base"), at_1);
}

#[test]
fn skill_granting_items_attach_the_skill_8_3() {
    let e = engine();
    let m = e
        .build(&TooltipRequest::hover(GRANTER), &Viewer::default())
        .expect("item tooltip");
    assert_eq!(m.comparison.kind, CompareKind::SkillGrantingItem);
    assert_eq!(m.comparison.tooltips.len(), 1);
    assert_eq!(
        row(&m.comparison.tooltips[0].1, "Name_Base").plain_text(),
        "Strike"
    );
    // The granter itself names no skill to execute: its skill-item rows are empty.
    assert!(row(&m, "SkillItem_MaxRange_Base").is_empty());
}

#[test]
fn literals_13() {
    let e = engine();
    let m = e
        .build_combat(
            &CombatRequest::hover(CombatSubject::SkillLiteral("Zap".into())),
            &SkillViewer::default(),
        )
        .expect("literal");
    assert_eq!(m.text_lines(), ["Zap"]);
    assert_eq!(m.rows[0].colour, Some(SKILL_NAME_COLOUR));
}

#[test]
fn census_categories_14() {
    let e = engine();
    let mut census = CombatCensus::default();
    let decodes = |d: Did| d == ICON;
    for (s, quick) in [
        (CombatSubject::Skill(SKILL), true),
        (CombatSubject::Skill(MONSTER), true),
        (CombatSubject::Effect(EFFECT_A), false),
        (CombatSubject::Effect(EFFECT_MISSING), false),
        (CombatSubject::Effect(EFFECT_B), false),
    ] {
        let o = e.classify_combat(&s);
        let did = match s {
            CombatSubject::Skill(d) | CombatSubject::Effect(d) => d,
            _ => unreachable!(),
        };
        census.add(
            did,
            &o,
            IconCategory::of(o.icon, decodes),
            quick.then(|| IconCategory::of(o.quickslot_icon, decodes)),
        );
        match did {
            SKILL => {
                assert_eq!(o.category, Ok(CombatCategory::Rendered));
                assert_eq!(o.deferred, 1);
                assert_eq!(o.icon, Some(ICON));
                assert!(o.non_empty.iter().any(|l| l == "Skill_RecoveryTime_Base"));
            }
            MONSTER => assert_eq!(o.category, Ok(CombatCategory::NoTooltip)),
            EFFECT_MISSING => {
                assert_eq!(o.category, Ok(CombatCategory::RenderedNoName));
                assert_eq!(o.name, NameState::Missing);
                assert!(o.lookup_misses > 0);
            }
            _ => {}
        }
    }
    assert_eq!(census.count(CombatCategory::Rendered), 2);
    assert_eq!(census.count(CombatCategory::RenderedNoName), 1);
    assert_eq!(census.no_name, (1, 0));
    assert_eq!(census.count(CombatCategory::NoTooltip), 2);
    assert_eq!(census.escaped, 0);
    assert_eq!(census.icons.get(&IconCategory::Rendered), Some(&1));
    assert_eq!(census.icons.get(&IconCategory::Unreadable), Some(&1));
    assert_eq!(census.icons.get(&IconCategory::Absent), Some(&3));
    assert_eq!(e.skill_icon(SKILL, SkillIconSize::Small), Some(ICON));
    assert_eq!(e.skill_icon(SKILL, SkillIconSize::Large), None);
}

#[test]
fn hostile_records_do_not_panic() {
    let mut m = install();
    let hooks = m.p("Skill_AttackHookList", T::Array);
    let costs = m.p("Skill_VitalCostList", T::Array);
    let range = m.p("Skill_MaxRange", T::Float);
    let recovery = m.p("Skill_SkillRecoveryTime", T::Float);
    let hover_l = m.p("UI_Examination_Tooltip_0Click_Layout", T::DidRef);
    let hover_e = m.p("UI_Examination_Tooltip_0Click_Element", T::Enum);
    let bad = Did(0x7000_0199);
    m.record(
        bad,
        vec![
            (hover_l, V::DidRef(L_EXAM)),
            (hover_e, V::Enum(E_SKILL_TT)),
            (hooks, V::Int(3)),
            (costs, array(1, vec![V::Float(f32::NAN); 300])),
            (range, V::Float(f32::INFINITY)),
            (recovery, V::Float(f32::NAN)),
        ],
    );
    let e = TooltipEngine::new(m);
    let model = e
        .build_combat(
            &CombatRequest::hover(CombatSubject::Skill(bad)),
            &SkillViewer {
                level: Some(i32::MIN),
                max_vitals: vec![(VITAL_TYPE_POWER, f32::MAX)],
                ..SkillViewer::default()
            },
        )
        .expect("tooltip");
    assert!(!model.has_name());
    let o = e.classify_combat(&CombatSubject::Effect(Did(0x7000_0FFF)));
    assert!(o.category.is_err());
}

fn tooltip_of(e: &TooltipEngine<Mem>, did: Did, viewer: &SkillViewer) -> TooltipModel {
    e.build_combat(&CombatRequest::hover(CombatSubject::Skill(did)), viewer)
        .expect("skill tooltip")
}

#[test]
fn vital_costs_progression_percent_per_second_7_10() {
    let e = engine();
    let viewer = SkillViewer {
        level: Some(50),
        max_vitals: vec![(VITAL_TYPE_HEALTH, 12_345.0)],
        ..SkillViewer::default()
    };
    let m = tooltip_of(&e, SKILL_COST, &viewer);
    // Morale: round(12,345 × 0.1) = 1,235, grouped (string-table.md §9.4.1 step 3); Power:
    // the progression at level 50 (the stored 0 is not used); the consumes-all entry shows
    // nothing; then the per-second list.
    assert_eq!(
        row(&m, "Skill_VitalCost_Base").plain_text(),
        "1,235 Morale, 500 Power, 2 Power/s"
    );
    // Level 1, no vitals: the progression's first point; the percent cost is 0 → no line.
    let m = tooltip_of(&e, SKILL_COST, &SkillViewer::default());
    assert_eq!(
        row(&m, "Skill_VitalCost_Base").plain_text(),
        "10 Power, 2 Power/s"
    );
}

#[test]
fn item_tooltips_carry_the_combat_viewer_13() {
    let e = engine();
    let viewer = Viewer {
        level: Some(50),
        ..Viewer::default()
    };
    // Without combat state: the level only.
    let m = e
        .build(&TooltipRequest::hover(GRANTER_COST), &viewer)
        .expect("item tooltip");
    assert_eq!(
        row(&m.comparison.tooltips[0].1, "Skill_VitalCost_Base").plain_text(),
        "500 Power, 2 Power/s"
    );
    // With the feed's vitals the attached skill tooltip has its percent cost.
    let combat = SkillViewer {
        max_vitals: vec![(1, 12_345.0)],
        ..SkillViewer::default()
    };
    let m = e
        .build_with_combat(&TooltipRequest::hover(GRANTER_COST), &viewer, &combat)
        .expect("item tooltip");
    assert_eq!(
        row(&m.comparison.tooltips[0].1, "Skill_VitalCost_Base").plain_text(),
        "1,235 Morale, 500 Power, 2 Power/s"
    );
}

#[test]
fn toggles_channels_and_qualifier_tokens_7_2_7_6_7_11() {
    let e = engine();
    let m = tooltip_of(&e, SKILL_TOGGLE, &SkillViewer::default());
    assert!(m.failures.is_empty(), "{:?}", m.failures);
    assert_eq!(
        row(&m, "Skill_CastingTime_Base").plain_text(),
        "Channel: [||||4]"
    );
    assert_eq!(
        row(&m, "Skill_SpecialClass_Base").plain_text(),
        "Channelled toggle"
    );
    // No hooks, immediate: token 0x0A9A9C99 alone.
    assert!(
        row(&m, "Skill_QualifierRange_Row_Base")
            .plain_text()
            .starts_with("Immediate"),
        "{:?}",
        row(&m, "Skill_QualifierRange_Row_Base").plain_text()
    );

    let m = tooltip_of(&e, SKILL_IGNORES, &SkillViewer::default());
    assert!(row(&m, "Skill_CastingTime_Base").is_empty());
    assert_eq!(row(&m, "Skill_SpecialClass_Base").plain_text(), "Toggle");
    assert!(
        row(&m, "Skill_QualifierRange_Row_Base")
            .plain_text()
            .starts_with("Melee, ignores reset"),
        "{:?}",
        row(&m, "Skill_QualifierRange_Row_Base").plain_text()
    );
    // Not a toggle: no special-class text.
    let m = tooltip_of(&e, SKILL, &SkillViewer::default());
    assert!(row(&m, "Skill_SpecialClass_Base").is_empty());
}

#[test]
fn critical_list_merges_9_6() {
    let e = engine();
    let m = tooltip_of(&e, SKILL_CRIT, &SkillViewer::default());
    assert_eq!(
        lines(row(&m, "CriticalEffectDisplay_Base")),
        [l("Heals you.", None, LINE_COLOUR)]
    );
    assert_eq!(
        lines(row(&m, "SuperCriticalEffectDisplay_Base")),
        [l("Stuns.", None, LINE_COLOUR)]
    );
    assert_eq!(
        lines(row(&m, "BothCriticalEffectDisplay_Base")),
        [l("Burns.", None, LINE_COLOUR)]
    );
}

#[test]
fn override_line_suppressed_chance_and_durations_9_4_9_5() {
    let e = engine();
    // §16.2 (c): the override with 0x06D04635 = the percentage value text of 0.05; the chance
    // line is suppressed and a permanent effect has no duration line.
    let o = e.effect_lines(EFFECT_OVERRIDE, EffectMode::Definition, 1, true);
    let texts: Vec<String> = o.lines.iter().map(|x| x.text.plain_text()).collect();
    assert_eq!(texts, ["Saves 5% power"]);
    assert_eq!(o.lines[0].state, LineState::Buffed);
    // Pulsed: 5 × 2 s.
    let p = e.effect_lines(EFFECT_PULSED, EffectMode::Definition, 1, true);
    let texts: Vec<String> = p.lines.iter().map(|x| x.text.plain_text()).collect();
    assert_eq!(texts, ["Ticks.", "Duration: [||||10]"]);
}

#[test]
fn short_recoveries_show_the_wrap_string_7_15() {
    let mut m = install();
    let recovery = m.p("Skill_SkillRecoveryTime", T::Float);
    let skill_name = m.p("Skill_Name", T::StringInfo);
    let hover_l = m.p("UI_Examination_Tooltip_0Click_Layout", T::DidRef);
    let hover_e = m.p("UI_Examination_Tooltip_0Click_Element", T::Enum);
    let quick = Did(0x7000_0170);
    let none = Did(0x7000_0171);
    m.record(
        quick,
        vec![
            (hover_l, V::DidRef(L_EXAM)),
            (hover_e, V::Enum(E_SKILL_TT)),
            (skill_name, info(literal("Jab"))),
            (recovery, V::Float(0.5)),
        ],
    );
    m.record(
        none,
        vec![
            (hover_l, V::DidRef(L_EXAM)),
            (hover_e, V::Enum(E_SKILL_TT)),
            (skill_name, info(literal("Poke"))),
            (recovery, V::Float(0.0)),
        ],
    );
    let e = TooltipEngine::new(m);
    // 0 < T < 1: the wrap string with an empty timer text (revision 2, "Below one second").
    let t = tooltip_of(&e, quick, &SkillViewer::default());
    assert_eq!(
        row(&t, "Skill_RecoveryTime_Base").plain_text().trim_end(),
        "Cooldown:"
    );
    assert_eq!(t.timers.len(), 1, "{:?}", t.timers);
    assert!(!t.timers[0].counting);
    // T ≤ 0 still empties the row.
    let t = tooltip_of(&e, none, &SkillViewer::default());
    assert!(row(&t, "Skill_RecoveryTime_Base").is_empty());
    assert!(t.timers.is_empty(), "{:?}", t.timers);
}

#[test]
fn a_counting_recovery_keeps_its_wrap_string_below_one_second_7_15() {
    let viewer = SkillViewer {
        cooldowns: vec![(SKILL, 12.5)],
        ..SkillViewer::default()
    };
    let e = engine();
    let m = e
        .build_combat(&CombatRequest::hover(CombatSubject::Skill(SKILL)), &viewer)
        .expect("tooltip");
    let t = &m.timers[0];
    assert_eq!(
        e.timer_text(t, 12.0)
            .map(|x| x.plain_text().trim_end().to_owned())
            .as_deref(),
        Some("Ready in")
    );
}

/// A skill requiring a trait of the given nature (`None`: no nature).
fn trait_skill(m: &mut Mem, skill: Did, tr: Did, nature: Option<u32>) {
    let skill_name = m.p("Skill_Name", T::StringInfo);
    let hover_l = m.p("UI_Examination_Tooltip_0Click_Layout", T::DidRef);
    let hover_e = m.p("UI_Examination_Tooltip_0Click_Element", T::Enum);
    let required = m.p("Skill_RequiredTrait", T::DidRef);
    let trait_name = m.p("Trait_Name", T::StringInfo);
    let trait_nature = m.p("Trait_Nature", T::Enum);
    m.record(
        skill,
        vec![
            (hover_l, V::DidRef(L_EXAM)),
            (hover_e, V::Enum(E_SKILL_TT)),
            (skill_name, info(literal("Stance"))),
            (required, V::DidRef(tr)),
        ],
    );
    let mut props = vec![(trait_name, info(literal("Mastery")))];
    if let Some(n) = nature {
        props.push((trait_nature, V::Enum(n)));
    }
    m.record(tr, props);
}

#[test]
fn passive_skill_traits_use_their_token_7_13() {
    // Natures by the spec's keys (the synthetic mapper names none of them).
    let cases = [
        (Some(1), true),   // Characteristic
        (Some(13), true),  // PvMP
        (Some(14), true),  // MountedCombat
        (Some(19), true),  // Class_Burglar
        (Some(28), true),  // Class_Specialization
        (Some(33), true),  // Class_Corsair
        (Some(2), false),  // a nature outside the rule
        (Some(18), false), // just below the range
        (Some(34), false), // just above it
        (Some(39), false),
        (None, false), // a trait without a nature
    ];
    for (i, (nature, passive)) in cases.into_iter().enumerate() {
        let mut m = install();
        let skill = Did(0x7000_0180 + i as u32);
        let tr = Did(0x7000_0190 + i as u32);
        trait_skill(&mut m, skill, tr, nature);
        let e = TooltipEngine::new(m);
        let t = tooltip_of(&e, skill, &SkillViewer::default());
        let r = row(&t, "Skill_Trait_Base");
        assert_eq!(r.state.as_deref(), Some("cantuse"), "{nature:?}");
        if passive {
            // Token 0x0E102ECC is absent here (an empty piece in the English table): the
            // row is empty and left out.
            assert!(r.is_empty(), "{nature:?}: {:?}", r.content);
        } else {
            assert_eq!(r.plain_text(), "Requires Mastery", "{nature:?}");
        }
    }
}

#[test]
fn passive_skill_trait_token_text_and_viewer_trait_7_13() {
    let mut m = install();
    m.text(tokens::EXAMINATION_TABLE_KEY, st::PASSIVE_TRAIT, "Passive");
    let skill = Did(0x7000_01A0);
    let tr = Did(0x7000_01A1);
    trait_skill(&mut m, skill, tr, Some(1));
    let e = TooltipEngine::new(m);
    let t = tooltip_of(&e, skill, &SkillViewer::default());
    let r = row(&t, "Skill_Trait_Base");
    assert_eq!(r.plain_text(), "Passive");
    assert_eq!(r.state.as_deref(), Some("cantuse"));
    // A viewer who has the trait: not `cantuse`, so the row's format.
    let viewer = SkillViewer {
        traits: vec![tr],
        ..SkillViewer::default()
    };
    let t = tooltip_of(&e, skill, &viewer);
    let r = row(&t, "Skill_Trait_Base");
    assert_eq!(r.plain_text(), "Requires Mastery");
    assert_eq!(r.state, None);
}

#[test]
fn passive_natures_are_read_from_the_mapper_by_label_7_13() {
    let mut m = install();
    let trait_nature = m.p("Trait_Nature", T::Enum);
    m.enum_keys
        .insert((trait_nature, "Characteristic".to_owned()), 100);
    let passive = Did(0x7000_01B0);
    let moved = Did(0x7000_01B2);
    trait_skill(&mut m, passive, Did(0x7000_01B1), Some(100));
    trait_skill(&mut m, moved, Did(0x7000_01B3), Some(1));
    let e = TooltipEngine::new(m);
    let t = tooltip_of(&e, passive, &SkillViewer::default());
    assert!(row(&t, "Skill_Trait_Base").is_empty());
    let t = tooltip_of(&e, moved, &SkillViewer::default());
    assert_eq!(row(&t, "Skill_Trait_Base").plain_text(), "Requires Mastery");
}
