// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the combat panel and visual data (`combat-ui.md`, `combat-visuals.md`,
//! `game-controls.md` §5.8, §7.1, §7.5, `interaction.md` §3.8, §3.9, §8) over an in-memory
//! install with invented records and wording. No game data.

mod mem;

use mem::{Mem, array, info, literal, structs};
use tessera_formats::ui_layout::UiEnum;
use tessera_formats::{Did, PropertyType as T, PropertyValue as V};
use tessera_gamedata::combat::{
    Avoidance, CombatData, EffectRow, LevelDisplay, MAX_CONSIDERATION_TABLES, PLAIN_FRAME_STATE,
    TargetKind,
};
use tessera_gamedata::tooltip::SkillIconSize;

const CONTROL: Did = Did(0x7000_0001);
const TABLE_A: Did = Did(0x7000_0002);
const VDO_CONTROL: Did = Did(0x7000_0003);
const VDO_1: Did = Did(0x7000_0004);
const BODY: Did = Did(0x7000_0005);
const EFFECT: Did = Did(0x7000_0006);
const SKILL: Did = Did(0x7000_0007);
const INDUCTION: Did = Did(0x7000_0008);
const CHANNEL: Did = Did(0x7000_0009);
const DEFEAT: Did = Did(0x7000_000A);
const DEFEAT_PROG: Did = Did(0x7000_000B);
const DEFEAT_EFFECT: Did = Did(0x7000_000C);
const STATES: Did = Did(0x7000_000D);
const FLOATY: Did = Did(0x7800_0004);

const S_TOUGH: u32 = 0x1000_0001;
const S_VDO: u32 = 0x1000_0002;

#[allow(clippy::too_many_lines)]
fn install() -> Mem {
    let mut m = Mem::default();
    m.ui_label(UiEnum::StateId, S_TOUGH, "ConTough_ColorName");
    m.ui_label(UiEnum::StateId, S_VDO, "vdo_gold");

    // Consideration (`game-controls.md` §7.5).
    let rank_to_state = m.p("ConsiderationControl_RankToUIState", T::Array);
    let rank = m.p("ConsiderationControl_ConsiderRank", T::Enum);
    let state = m.p("ConsiderationControl_UIStateID", T::Enum);
    let tables = m.p("ConsiderationControl_CombatTableData", T::Array);
    let consider_type = m.p("ConsiderationControl_ConsiderType", T::Enum);
    let list = m.p("ConsiderationControl_CombatTableList", T::Array);
    let ranks = m.p("ConsiderationTable_RankPerDifferentialArray", T::Array);
    m.enum_labels.insert((consider_type, 2), "Monster".into());
    m.weenies.insert("ConsiderationControl".into(), CONTROL);
    m.record(
        CONTROL,
        vec![
            (
                rank_to_state,
                structs(
                    1,
                    vec![vec![(rank, V::Enum(60)), (state, V::Enum(S_TOUGH))]],
                ),
            ),
            (
                tables,
                structs(
                    1,
                    vec![vec![
                        (consider_type, V::Enum(2)),
                        (list, array(2, vec![V::DidRef(TABLE_A); 3])),
                    ]],
                ),
            ),
        ],
    );
    let mut r = vec![V::Enum(50); 16];
    r.extend(vec![V::Enum(60); 15]);
    m.record(TABLE_A, vec![(ranks, array(3, r))]);

    // Vital-display overrides (`game-controls.md` §7.1).
    let vdo_array = m.p("VitalDisplayOverride_Array", T::Array);
    let vdo_key = m.p("VitalDisplayOverride_RequiredEnum", T::Enum);
    let vdo_state = m.p("VitalDisplayOverride_UIState", T::Enum);
    let vdo_freep = m.p("VitalDisplayOverride_IsForFreepCharacters", T::Bool);
    m.weenies
        .insert("gmVitalDisplayOverrideControl".into(), VDO_CONTROL);
    m.record(
        VDO_CONTROL,
        vec![(vdo_array, array(4, vec![V::DidRef(VDO_1)]))],
    );
    m.record(
        VDO_1,
        vec![
            (vdo_key, V::Enum(3)),
            (vdo_state, V::Enum(S_VDO)),
            (vdo_freep, V::Bool(true)),
        ],
    );

    // A target body (`combat-ui.md` §5.2).
    let decoration = m.p("MonsterLevel_ExaminationModStatType", T::Enum);
    let hide = m.p("NPC_HideLevelInDisplay", T::Bool);
    m.enum_labels.insert((decoration, 7), "Elite".into());
    m.record(BODY, vec![(decoration, V::Enum(7)), (hide, V::Bool(true))]);

    // An effect (`combat-ui.md` §8.1; `combat-visuals.md` §5).
    let effect_name = m.p("Effect_Name", T::StringInfo);
    let icon = m.p("Effect_Icon", T::DidRef);
    let harmful = m.p("Effect_Harmful", T::Bool);
    let resist = m.p("Effect_Resist_Category", T::Bitfield32);
    let combat_only = m.p("Effect_Duration_CombatOnly", T::Bool);
    let fx = m.p("Effect_FX", T::Enum);
    let induced = m.p("Effect_InduceCombatState_StateToInduce", T::Bitfield32);
    m.enum_labels.insert((resist, 5), "Elemental".into());
    m.enum_labels.insert((resist, 6), "Fear".into());
    m.record(
        EFFECT,
        vec![
            (effect_name, info(literal("Dread"))),
            (icon, V::DidRef(Did(0x4100_0009))),
            (harmful, V::Bool(true)),
            (resist, V::Bitfield32(0x20)),
            (combat_only, V::Bool(true)),
            (fx, V::Enum(0x1000_0042)),
            (induced, V::Bitfield32(0x4)),
        ],
    );

    // Combat states (`game-controls.md` §5.8).
    let state_list = m.p("CombatState_Control_StateDataList", T::Array);
    let state_bits = m.p("CombatState_StateData_State", T::Bitfield32);
    let state_script = m.p("CombatState_StateData_ScriptID", T::Enum);
    m.weenies.insert("CombatStateControl".into(), STATES);
    m.record(
        STATES,
        vec![(
            state_list,
            structs(
                1,
                vec![
                    vec![
                        (state_bits, V::Bitfield32(0x2)),
                        (state_script, V::Enum(70)),
                    ],
                    vec![
                        (state_bits, V::Bitfield32(0x4)),
                        (state_script, V::Enum(77)),
                    ],
                ],
            ),
        )],
    );

    // A skill with an induction, behaviours and hooks (`combat-visuals.md` §2.2, §2.3).
    let skill_name = m.p("Skill_Name", T::StringInfo);
    let small_icon = m.p("Skill_SmallIcon", T::DidRef);
    let induction = m.p("Skill_InductionAction", T::DidRef);
    let self_list = m.p("Skill_BehaviorSelfList", T::Array);
    let other_list = m.p("Skill_BehaviorOtherList", T::Array);
    let behaviour = m.p("Skill_Behavior", T::Enum);
    let hooks = m.p("Skill_AttackHookList", T::Array);
    let hook_index = m.p("Skill_AttackHook_AnimationHookIndex", T::Int);
    let fx_target = m.p("Skill_AttackHook_fxTargetHit", T::Enum);
    let induction_duration = m.p("Induction_Duration", T::Float);
    let induction_title = m.p("Induction_TitleString", T::StringInfo);
    let induction_fx = m.p("Induction_FXID", T::Enum);
    let induction_interruptible = m.p("Induction_Interruptable", T::Bool);
    m.record(
        SKILL,
        vec![
            (skill_name, info(literal("Strike"))),
            (small_icon, V::DidRef(Did(0x4100_0001))),
            (induction, V::DidRef(INDUCTION)),
            (
                self_list,
                structs(
                    1,
                    vec![
                        vec![(behaviour, V::Enum(11))],
                        vec![(behaviour, V::Enum(12))],
                    ],
                ),
            ),
            (other_list, structs(1, vec![vec![(behaviour, V::Enum(21))]])),
            (
                hooks,
                structs(
                    1,
                    vec![
                        vec![(hook_index, V::Int(1)), (fx_target, V::Enum(31))],
                        vec![(hook_index, V::Int(2)), (fx_target, V::Enum(32))],
                    ],
                ),
            ),
        ],
    );
    m.record(
        INDUCTION,
        vec![
            (induction_duration, V::Float(2.0)),
            (induction_title, info(literal("Charging"))),
            (induction_fx, V::Enum(41)),
            (induction_interruptible, V::Bool(false)),
        ],
    );

    // A channel with FX properties found by name (`combat-visuals.md` §2.1).
    let channel_duration = m.p("Channeling_Duration", T::Float);
    let channel_start = m.p("Channeling_StartScriptID", T::Enum);
    let start_fx = m.p("Channeling_StartScript_FXID", T::Enum);
    let target_start_fx = m.p("Channeling_TargetStartScript_FXID", T::Enum);
    let success_fx = m.p("Channeling_Success_Script_FXID", T::Enum);
    let channel_title = m.p("Channeling_TitleString", T::StringInfo);
    let channel_interruptible = m.p("Channeling_Interruptable", T::Bool);
    m.record(
        CHANNEL,
        vec![
            (channel_duration, V::Float(9.0)),
            (channel_start, V::Enum(51)),
            (start_fx, V::Enum(52)),
            (target_start_fx, V::Enum(53)),
            (success_fx, V::Enum(54)),
            (channel_title, info(literal("Singing"))),
            (channel_interruptible, V::Bool(true)),
        ],
    );

    // A defeat record (`interaction.md` §8.1).
    let rally = m.p("Death_RallyPoint", T::String);
    let defeat_prog = m.p("Death_DefeatEffectProgression", T::DidRef);
    let prog_array = m.p("PropertyProgression_Array", T::Array);
    let min_index = m.p("Progression_MinimumIndexValue", T::Int);
    let defeat_effect = m.p("Death_DefeatEffect", T::DidRef);
    m.record(
        DEFEAT,
        vec![
            (rally, V::String("Village".into())),
            (defeat_prog, V::DidRef(DEFEAT_PROG)),
        ],
    );
    m.record(
        DEFEAT_PROG,
        vec![
            (min_index, V::Int(1)),
            (
                prog_array,
                array(
                    defeat_effect,
                    vec![V::DidRef(Did(0)), V::DidRef(DEFEAT_EFFECT)],
                ),
            ),
        ],
    );

    // Floating-text settings (`combat-ui.md` §6.2).
    let far = m.p("UI_FloatyText_FarRange", T::Float);
    let check = m.p("UI_FloatyText_CheckDistance", T::Bool);
    let columns = m.p("UI_FloatyText_DamageColumns", T::Int);
    m.uniques.insert("UI_FloatyText_Properties".into(), FLOATY);
    m.record(
        FLOATY,
        vec![
            (far, V::Float(40.0)),
            (check, V::Bool(false)),
            (columns, V::Float(2.0)),
        ],
    );
    m.text(
        Avoidance::TABLE_KEY,
        Avoidance::Parry.short_token(),
        "Parried",
    );
    m
}

#[test]
fn consideration_and_name_colours() {
    let c = CombatData::new(install());
    let con = c.consideration().expect("control");
    // Level 10 viewer, level 11 target: index 16 → rank 60 → Tough.
    assert_eq!(con.state("Monster", 10, 11), Some("ConTough_ColorName"));
    assert_eq!(
        c.name_colour_state(TargetKind::Creature, false, Some((10, 11))),
        "ConTough_ColorName"
    );
    // Rank 50 has no state: no colour from the table, and TR15 falls back to even.
    assert_eq!(con.state("Monster", 10, 9), None);
    assert_eq!(
        c.name_colour_state(TargetKind::Creature, false, Some((10, 9))),
        "ConEven_ColorName"
    );
    assert_eq!(
        c.name_colour_state(TargetKind::Creature, true, Some((10, 11))),
        "Corpse_ColorName"
    );
}

#[test]
fn overrides_and_target_frames() {
    let c = CombatData::new(install());
    let v = c.vital_display_overrides();
    assert_eq!(v.state(3), Some("vdo_gold"));
    assert!(v.entries[0].freep);
    let f = c.target_frame(Some(BODY), TargetKind::Creature);
    assert_eq!(f.decoration, "monster_elite");
    assert_eq!(f.level, LevelDisplay::HiddenWithSkull);
    let f = c.target_frame(None, TargetKind::Creature);
    assert_eq!(f.decoration, "monster_normal");
    assert_eq!(f.level, LevelDisplay::Shown);
    assert_eq!(
        c.target_frame(Some(BODY), TargetKind::Player).decoration,
        PLAIN_FRAME_STATE
    );
}

#[test]
fn effect_icons_tr29() {
    let c = CombatData::new(install());
    let e = c.effect_icon(EFFECT).expect("icon");
    assert_eq!(e.icon, Some(Did(0x4100_0009)));
    assert_eq!(e.row, EffectRow::Harmful);
    assert_eq!(e.border.as_deref(), Some("Fear"));
    assert!(e.combat_only);
    assert!(c.effect_icon(Did(0x7000_0FFF)).is_none());
    assert_eq!(
        c.skill_icon(SKILL, SkillIconSize::Small),
        Some(Did(0x4100_0001))
    );
}

#[test]
fn cast_bars_tr25() {
    let c = CombatData::new(install());
    let s = c.cast_bar(SKILL);
    assert_eq!(s.label.as_deref(), Some("Strike"));
    assert!(!s.interruptible);
    let i = c.cast_bar(INDUCTION);
    assert_eq!(i.label.as_deref(), Some("Charging"));
    let ch = c.cast_bar(CHANNEL);
    assert_eq!(ch.label.as_deref(), Some("Singing"));
    assert!(ch.interruptible);
    let none = c.cast_bar(Did(0x7000_0FFF));
    assert_eq!(none.label, None);
    assert!(none.interruptible);
}

#[test]
fn channelled_skills_without_an_induction_are_interruptible_tr25() {
    let mut m = install();
    let skill_name = m.p("Skill_Name", T::StringInfo);
    let channelling = m.p("Skill_Toggle_ChannelingState", T::DidRef);
    let channel_duration = m.p("Channeling_Duration", T::Float);
    let channel_interruptible = m.p("Channeling_Interruptable", T::Bool);
    let skill = Did(0x7000_0020);
    let channel = Did(0x7000_0021);
    m.record(
        channel,
        vec![
            (channel_duration, V::Float(5.0)),
            (channel_interruptible, V::Bool(false)),
        ],
    );
    m.record(
        skill,
        vec![
            (skill_name, info(literal("Hum"))),
            (channelling, V::DidRef(channel)),
        ],
    );
    let c = CombatData::new(m);
    // TR25 follows only `Skill_InductionAction`: the channel's flag does not apply.
    let bar = c.cast_bar(skill);
    assert_eq!(bar.label.as_deref(), Some("Hum"));
    assert!(bar.interruptible);
    // The channel itself, as `what`, keeps its own flag.
    assert!(!c.cast_bar(channel).interruptible);
}

#[test]
fn defeat_records_and_floaty_settings() {
    let c = CombatData::new(install());
    let d = c.defeat_record(DEFEAT).expect("record");
    assert_eq!(d.rally_point.as_deref(), Some("Village"));
    assert_eq!(c.defeat_effect(&d, 2), Some(DEFEAT_EFFECT));
    assert_eq!(c.defeat_effect(&d, 1), None);
    assert!(c.defeat_record(SKILL).is_none());
    let f = c.floaty_settings(false);
    assert_eq!(f.far_range, 40.0);
    assert!(!f.check_distance);
    // A mistyped property keeps the built-in value (§6.2).
    assert_eq!(f.damage_columns, 3);
    // No distant-battle set: the built-ins.
    assert_eq!(c.floaty_settings(true).far_range, 30.0);
    assert_eq!(
        c.avoidance_text(Avoidance::Parry).as_deref(),
        Some("Parried")
    );
    assert_eq!(c.avoidance_text(Avoidance::Miss), None);
}

#[test]
fn skill_and_effect_visuals() {
    let c = CombatData::new(install());
    let v = c.skill_visuals(SKILL).expect("skill");
    assert_eq!(v.self_list, [11, 12]);
    assert_eq!(v.choose(false, true, |n| n - 1), Some(12));
    assert_eq!(v.choose(false, false, |_| 0), Some(21));
    assert_eq!(v.choose(true, true, |_| 0), None);
    assert_eq!(v.hook(2).map(|h| h.fx_target_hit), Some(32));
    assert_eq!(v.induction, Some(INDUCTION));
    let i = c.induction_visuals(INDUCTION).expect("induction");
    assert_eq!(i.fx, 41);
    assert!(!i.interruptible);
    let ch = c.channel_visuals(CHANNEL).expect("channel");
    assert_eq!(ch.start.caster, 51);
    assert_eq!(ch.start.caster_fx, 52);
    assert_eq!(ch.start.target_fx, 53);
    assert_eq!(ch.success.caster_fx, 54);
    assert_eq!(ch.failure.caster_fx, 0);
    let e = c.effect_visuals(EFFECT).expect("effect");
    assert_eq!(e.fx, 0x1000_0042);
    assert_eq!(e.induced_state, Some(0x4));
    assert_eq!(c.induced_state_script(EFFECT), Some(77));
    assert_eq!(c.combat_states().len(), 2);
    assert!(c.skill_visuals(EFFECT).is_none());
    assert!(c.channel_visuals(SKILL).is_none());
}

#[test]
fn hostile_consideration_controls_read_a_bounded_number_of_tables() {
    let mut m = install();
    let tables = m.p("ConsiderationControl_CombatTableData", T::Array);
    let consider_type = m.p("ConsiderationControl_ConsiderType", T::Enum);
    let list = m.p("ConsiderationControl_CombatTableList", T::Array);
    let ranks = m.p("ConsiderationTable_RankPerDifferentialArray", T::Array);
    let dids: Vec<Did> = (0..200u32).map(|i| Did(0x7100_0000 + i)).collect();
    for &d in &dids {
        m.record(d, vec![(ranks, array(3, vec![V::Enum(50); 31]))]);
    }
    let entry = vec![
        (consider_type, V::Enum(2)),
        (list, array(2, dids.iter().map(|&d| V::DidRef(d)).collect())),
    ];
    m.record(CONTROL, vec![(tables, structs(1, vec![entry; 300]))]);
    let c = CombatData::new(m);
    let con = c.consideration().expect("control");
    assert_eq!(con.lists.len(), MAX_CONSIDERATION_TABLES);
    assert_eq!(con.tables.len(), MAX_CONSIDERATION_TABLES);
}
