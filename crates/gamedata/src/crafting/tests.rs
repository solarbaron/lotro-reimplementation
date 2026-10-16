// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the crafting model (`crafting-panels.md` §15 worked examples and the
//! rules of §4–§9) over the in-memory install of the quest tests: our own records, labels
//! and texts, with invented IDs and wording.

use std::sync::Arc;

use tessera_formats::property::PropertyEntry;
use tessera_formats::{Did, PropertyValue};

use super::quantity;
use super::rules::{self, Meter, TierMark};
use super::*;
use crate::quest::tests::{Mem, arr, literal, si, st};

const PROF: Did = Did(0x7000_0100);
const RECIPE: Did = Did(0x7000_0200);
const COOL: Did = Did(0x7000_0201);
const CONTAINER: Did = Did(0x7000_0202);
const ORE: Did = Did(0x7000_0300);
const COAL: Did = Did(0x7000_0301);
const GEM: Did = Did(0x7000_0302);
const BAR: Did = Did(0x7000_0303);
const FINE_BAR: Did = Did(0x7000_0304);
const PACK: Did = Did(0x7000_0305);
const CHEST: Did = Did(0x7000_0306);
const FORGE: Did = Did(0x7000_0400);
const INDUCTION: Did = Did(0x7000_0500);
const CHANNEL: Did = Did(0x7000_0501);
const CONTROL: Did = Did(0x7000_0600);
const COOLDOWNS: Did = Did(0x7000_0601);
const VOCATION: Did = Did(0x7000_0700);
const GUILD: Did = Did(0x7000_0701);
const FACTION: Did = Did(0x7000_0702);
const SCROLL: Did = Did(0x7000_0800);
const NODE: Did = Did(0x7000_0900);

const CRIT_MOD_PID: u32 = 9_001;

fn name(m: &mut Mem, did: Did, text: &str) {
    let e = m.e("Name", si(literal(text)));
    m.put(did, vec![e]);
}

fn ingredient(m: &mut Mem, item: Did, q: i32, bonus: Option<f32>) -> PropertyEntry {
    let mut fields = vec![
        m.e("CraftRecipe_Ingredient", PropertyValue::DidRef(item)),
        m.e("CraftRecipe_IngredientQuantity", PropertyValue::Int(q)),
    ];
    if let Some(b) = bonus {
        fields.push(m.e("CraftRecipe_IngredientCritBonus", PropertyValue::Float(b)));
    }
    m.e("CraftRecipe_IngredientEntry", st(fields))
}

/// The in-memory install: one profession with a tool, a plain recipe with two ingredients,
/// a pack, two optional ingredients and a critical result, a one-use cooldown recipe, a
/// recipe with a non-item result, texts, a craft control and a cooldown control.
fn install() -> Mem {
    let mut m = Mem::default();
    m.text(UI_CRAFTING_TABLE, tokens::TITLE, "Crafting");
    m.text(UI_CRAFTING_TABLE, tokens::ROW, "${#05697E45}");
    m.text(
        UI_CRAFTING_TABLE,
        tokens::ROW_COUNT,
        "${#05697E45} (${#00484A34})",
    );
    m.text(UI_CRAFTING_TABLE, tokens::PACK_COUNT, "${#00484A34}");
    m.text(
        UI_CRAFTING_TABLE,
        tokens::METER_TEXT,
        "${#08A76A74}/${#00005168}",
    );
    m.text(
        UI_CRAFTING_TABLE,
        tokens::EXPERIENCE,
        "${#04967744} xp at ${#00058DA2}",
    );
    m.text(UI_CRAFTING_TABLE, tokens::COOLDOWN, "every ${#00058E15}");
    m.text(UI_CRAFTING_TABLE, tokens::TOOL, "tool: ${#05B9DC5C}");
    m.text(UI_CRAFTING_TABLE, tokens::FACILITY, "at: ${#057E0A39}");
    m.text(CRAFT_SYSTEM_TABLE, tokens::NONE, "none");
    m.text(
        UI_CRAFTING_TABLE,
        tokens::INGREDIENT_INFO,
        "${#00484A34}/${#0059485C} ${#00052615}",
    );
    m.text(UI_CRAFTING_TABLE, tokens::PACK_CHECKBOX, "use ${#069C3CF4}");
    m.text(UI_CRAFTING_TABLE, tokens::PACKS_LABEL, "packs ${#069C3CF4}");
    m.text(UI_CRAFTING_TABLE, tokens::OPTIONAL_BONUS, "+${#04A67A64}%");
    m.text(UI_CRAFTING_TABLE, tokens::CRITICAL_CHANCE, "${#04A67A64}%");
    m.text(UI_CRAFTING_TABLE, tokens::RESULT_SUCCESS, "Made");
    m.text(UI_CRAFTING_TABLE, tokens::RESULT_FAILED, "Failed");
    m.text(
        UI_CRAFTING_TABLE,
        tokens::TRAINER_NAME,
        "${#00005343} teaches",
    );
    m.text(UI_CRAFTING_TABLE, tokens::LOSE_PROFESSION, "lose one");
    m.text(UI_CRAFTING_TABLE, tokens::VOCATION_CONFIRM, "sure?");
    m.text(EXAMINATION_TABLE, tokens::SCROLL_ONE_SHOT, "once");
    m.text(
        EXAMINATION_TABLE,
        tokens::SCROLL_COOLDOWN,
        "wait ${#00058E15}",
    );

    // Texts of the enum keys.
    let tier = m.p("CraftRecipe_Tier");
    let category = m.p("CraftRecipe_UICategory");
    let tool = m.p("CraftProfession_RequiredTool");
    for t in 1..=3 {
        m.enum_texts.insert((tier, t), format!("Tier {t}"));
    }
    m.enum_texts.insert((category, 7), "Swords".into());
    m.enum_texts.insert((category, 8), "axes".into());
    m.enum_texts.insert((tool, 4), "Hammer".into());

    for (d, n) in [
        (ORE, "Ore"),
        (COAL, "Coal"),
        (GEM, "Gem"),
        (BAR, "Bar"),
        (FINE_BAR, "Fine bar"),
        (PACK, "Ore pack"),
        (CHEST, "Chest"),
        (FORGE, "Forge"),
    ] {
        name(&mut m, d, n);
    }

    // Profession: tool bit 3 (key 4), critical modifier property.
    let tiers = (1..=3)
        .map(|t| {
            let f = vec![
                m.e("CraftProfession_Tier", PropertyValue::Enum(t)),
                m.e("CraftProfession_ProficiencyXPCap", PropertyValue::Int(200)),
            ];
            m.e("CraftProfession_TierEntry", st(f))
        })
        .collect();
    let prof = vec![
        m.e("CraftProfession_TierArray", arr(tiers)),
        m.e(
            "CraftProfession_RequiredTool",
            PropertyValue::Bitfield32(0x8),
        ),
        m.e(
            "CraftProfession_CriticalChanceAddModifier_PropertyName",
            PropertyValue::PropertyId(CRIT_MOD_PID),
        ),
        m.e("CraftProfession_Name", si(literal("Smith"))),
        m.e("CraftProfession_LearnString", si(literal("learn smithing"))),
        m.e(
            "CraftProfession_ChangeString",
            si(literal("change to smithing")),
        ),
    ];
    m.put(PROF, prof);

    // Plain recipe: tier 2, category 7; ore ×2, coal ×5; pack ×3; two optional
    // ingredients; a critical result; facility; induction; xp key 3.
    let ings = vec![
        ingredient(&mut m, ORE, 2, None),
        ingredient(&mut m, COAL, 5, None),
    ];
    let opts = vec![
        ingredient(&mut m, GEM, 1, Some(0.25)),
        ingredient(&mut m, COAL, 1, Some(0.35)),
    ];
    let recipe = vec![
        m.e("CraftRecipe_Profession", PropertyValue::DidRef(PROF)),
        m.e("CraftRecipe_Tier", PropertyValue::Enum(2)),
        m.e("CraftRecipe_UICategory", PropertyValue::Enum(7)),
        m.e("CraftRecipe_IngredientList", arr(ings)),
        m.e("CraftRecipe_OptionalIngredientList", arr(opts)),
        m.e("CraftRecipe_IngredientPack", PropertyValue::DidRef(PACK)),
        m.e("CraftRecipe_IngredientPackQuantity", PropertyValue::Int(3)),
        m.e("CraftRecipe_ResultItem", PropertyValue::DidRef(BAR)),
        m.e("CraftRecipe_ResultItemQuantity", PropertyValue::Int(2)),
        m.e(
            "CraftRecipe_CriticalResultItem",
            PropertyValue::DidRef(FINE_BAR),
        ),
        m.e(
            "CraftRecipe_CriticalSuccessChance",
            PropertyValue::Float(0.05),
        ),
        m.e(
            "CraftRecipe_RequiredFacilityEffect",
            PropertyValue::DidRef(FORGE),
        ),
        m.e(
            "CraftRecipe_InductionAction",
            PropertyValue::DidRef(INDUCTION),
        ),
        m.e("CraftRecipe_XPReward", PropertyValue::Enum(3)),
    ];
    m.put(RECIPE, recipe);

    // Cooldown recipe, tier 1, category 8, named.
    let ings = vec![ingredient(&mut m, ORE, 1, None)];
    let cool = vec![
        m.e("CraftRecipe_Profession", PropertyValue::DidRef(PROF)),
        m.e("CraftRecipe_Tier", PropertyValue::Enum(1)),
        m.e("CraftRecipe_UICategory", PropertyValue::Enum(8)),
        m.e("CraftRecipe_IngredientList", arr(ings)),
        m.e("CraftRecipe_ResultItem", PropertyValue::DidRef(BAR)),
        m.e("CraftRecipe_CooldownDuration", PropertyValue::Enum(35)),
        m.e("CraftRecipe_Name", si(literal("Daily bar"))),
    ];
    m.put(COOL, cool);

    // A recipe whose result is a container (not an item) with an icon; two outputs.
    let ings = vec![ingredient(&mut m, ORE, 1, None)];
    let outs = vec![
        {
            let f = vec![
                m.e("CraftRecipe_Ingredient", PropertyValue::DidRef(ORE)),
                m.e("CraftRecipe_ResultItem", PropertyValue::DidRef(BAR)),
            ];
            m.e("CraftRecipe_MultiOutputEntry", st(f))
        },
        {
            let f = vec![
                m.e("CraftRecipe_Ingredient", PropertyValue::DidRef(ORE)),
                m.e("CraftRecipe_ResultItem", PropertyValue::DidRef(FINE_BAR)),
            ];
            m.e("CraftRecipe_MultiOutputEntry", st(f))
        },
    ];
    let container = vec![
        m.e("CraftRecipe_Profession", PropertyValue::DidRef(PROF)),
        m.e("CraftRecipe_Tier", PropertyValue::Enum(2)),
        m.e("CraftRecipe_UICategory", PropertyValue::Enum(8)),
        m.e("CraftRecipe_IngredientList", arr(ings)),
        m.e("CraftRecipe_ResultItem", PropertyValue::DidRef(CHEST)),
        m.e("CraftRecipe_MultiOutputArray", arr(outs)),
        m.e(
            "CraftRecipe_Field_ResultIcon",
            PropertyValue::DidRef(Did(0x4100_0001)),
        ),
    ];
    m.put(CONTAINER, container);

    // Induction with a title; a channelling state that bars crafting (bit key 4).
    let ind = vec![
        m.e("Induction_TitleString", si(literal("Smelting"))),
        m.e("Induction_Interruptable", PropertyValue::Bool(true)),
    ];
    m.put(INDUCTION, ind);
    let barred = m.p("Channeling_BarredActions");
    m.enum_names.insert((barred, 4), "Crafting".into());
    let ch = vec![m.e("Channeling_BarredActions", PropertyValue::Bitfield32(0x8))];
    m.put(CHANNEL, ch);

    // Craft control: key 3 → 40 xp. Cooldown control: key 35 → 1 day.
    let row = {
        let f = vec![
            m.e("CraftControl_XPRewardEnum", PropertyValue::Enum(3)),
            m.e("CraftControl_XPRewardValue", PropertyValue::Int(40)),
        ];
        m.e("CraftControl_XPRewardEntry", st(f))
    };
    let control = vec![m.e("CraftControl_XPRewardArray", arr(vec![row]))];
    m.put(CONTROL, control);
    m.weenies.insert("CraftControl".into(), CONTROL);
    let row = {
        let f = vec![
            m.e("CooldownControl_DurationType", PropertyValue::Enum(35)),
            m.e(
                "CooldownControl_DurationValue",
                PropertyValue::Float(86_400.0),
            ),
        ];
        m.e("CooldownControl_DurationMapListEntry", st(f))
    };
    let cds = vec![m.e("CooldownControl_DurationMapList", arr(vec![row]))];
    m.put(COOLDOWNS, cds);
    m.weenies.insert("CooldownControl".into(), COOLDOWNS);

    // Vocation, guild, faction, scroll, node.
    let member = m.e("CraftVocation_Profession", PropertyValue::DidRef(PROF));
    let voc = vec![
        m.e("CraftVocation_ProfessionArray", arr(vec![member])),
        m.e("CraftVocation_Name", si(literal("Armourer"))),
        m.e("CraftVocation_LearnString", si(literal("learn armourer"))),
        m.e("CraftVocation_ChangeString", si(literal("become armourer"))),
    ];
    m.put(VOCATION, voc);
    let fac = vec![m.e("Reputation_Faction_Name", si(literal("Smiths' guild")))];
    m.put(FACTION, fac);
    let guild = vec![
        m.e("CraftGuild_Profession", PropertyValue::DidRef(PROF)),
        m.e("CraftGuild_Faction", PropertyValue::DidRef(FACTION)),
        m.e("CraftGuild_LearnString", si(literal("join the guild"))),
    ];
    m.put(GUILD, guild);
    let scroll = vec![m.e("RecipeItem_Recipe", PropertyValue::DidRef(COOL))];
    m.put(SCROLL, scroll);
    let node = vec![
        m.e("Name", si(literal("Copper vein"))),
        m.e("Usage_RequiredCraftProfession", PropertyValue::DidRef(PROF)),
        m.e("Usage_RequiredCraftTool", PropertyValue::Bitfield32(0x8)),
        m.e("Usage_RequiredCraftTier", PropertyValue::Enum(1)),
        m.e("PhysObj", PropertyValue::DidRef(Did(0x4700_0001))),
        m.e("Induction_ActionDID", PropertyValue::DidRef(INDUCTION)),
        m.e("Usage_MinUsageDist", PropertyValue::Float(3.5)),
        m.e("Render_AdjustableScale", PropertyValue::Float(0.5)),
    ];
    m.put(NODE, node);
    m
}

fn presenter(m: &Mem) -> CraftingPresenter<'_> {
    CraftingPresenter::new(m).with_item_test(Arc::new(|d| d != CHEST))
}

fn learned(state: &mut CraftingState, did: Did) {
    state.apply(&CraftEvent::RecipeLearned(LearnedRecipe::new(
        RecordRef::Record(did),
    )));
}

fn report(tier: u16, prof: (u32, u32), mastery: (u32, u32), mt: Option<u16>) -> ProfessionReport {
    ProfessionReport {
        tier,
        proficiency: prof.0,
        proficiency_max: prof.1,
        mastery: mastery.0,
        mastery_max: mastery.1,
        mastery_tier: mt,
    }
}

fn holdings(items: &[(Did, u64)]) -> PlayerCraft {
    let mut p = PlayerCraft::default();
    for (d, q) in items {
        p.add(&RecordRef::Record(*d), *q);
    }
    p
}

// -------------------------------------------------------------------------------------------
// §15 worked examples
// -------------------------------------------------------------------------------------------

#[test]
fn makeable_count_example_15_1() {
    // A ×2, B ×5; held 7 and 11 → 2.
    assert_eq!(rules::makeable(&[(7, 2), (11, 5)], false, false, false), 2);
    // B held 4 → 0.
    assert_eq!(rules::makeable(&[(7, 2), (4, 5)], false, false, false), 0);
    // One-use → 1.
    assert_eq!(rules::makeable(&[(7, 2), (11, 5)], true, false, false), 1);
    // Cooldown running → 0; ready → 1.
    assert_eq!(rules::makeable(&[(7, 2), (11, 5)], false, true, true), 0);
    assert_eq!(rules::makeable(&[(7, 2), (11, 5)], false, true, false), 1);
    // Hostile: a needed quantity of 0 counts as 1; no ingredients count as 1.
    assert_eq!(rules::makeable(&[(3, 0)], false, false, false), 3);
    assert_eq!(rules::makeable(&[], false, false, false), 1);
    assert_eq!(rules::pack_count(7, 3), 2);
    assert_eq!(rules::pack_count(7, 0), 7);
    assert_eq!(rules::per_craft(Some(-4)), 1);
}

#[test]
fn tier_rows_example_15_2() {
    let r = report(3, (120, 400), (60, 800), Some(2));
    assert_eq!(completed_tiers(&r), (2, 1));
    assert_eq!(
        tier_marks(1, &r),
        vec![TierMark::ProficiencyIcon, TierMark::MasteryIcon]
    );
    assert_eq!(
        tier_marks(2, &r),
        vec![
            TierMark::ProficiencyIcon,
            TierMark::MasteryMeter(Meter::new(60, 800))
        ]
    );
    let TierMark::MasteryMeter(m) = &tier_marks(2, &r)[1] else {
        panic!("mastery meter")
    };
    assert!((m.position - 0.075).abs() < 1e-6);
    let marks = tier_marks(3, &r);
    let [TierMark::ProficiencyMeter(p)] = marks.as_slice() else {
        panic!("proficiency meter: {marks:?}")
    };
    assert!((p.position - 0.3).abs() < 1e-6);
    assert!(tier_marks(4, &r).is_empty());
    // A completed proficiency tier: a = T; b never above a.
    let done = report(2, (400, 400), (0, 0), Some(5));
    assert_eq!(completed_tiers(&done), (2, 2));
    // A cap of 0 shows no proficiency meter; position 0 for a meter with cap 0.
    assert!(tier_marks(1, &report(1, (5, 0), (0, 0), None)).is_empty());
    assert_eq!(Meter::new(5, 0).position, 0.0);
    assert_eq!(Meter::new(500, 10).position, 1.0);
}

#[test]
fn quantity_example_15_3() {
    let m = 4;
    assert_eq!(quantity::read("", m), 1);
    assert_eq!(quantity::after_typing("9", m), 4);
    assert_eq!(quantity::read("007", m), 7);
    assert_eq!(quantity::after_typing("007", m), 4);
    assert_eq!(quantity::decrease(1, m), 1);
    assert_eq!(quantity::increase(4, m), 4);
    assert_eq!(quantity::increase(2, m), 3);
    assert_eq!(quantity::increase(9, m), 4);
    assert_eq!(quantity::decrease(3, m), 2);
    // M = 0: the entry shows 1.
    assert_eq!(quantity::shown("", 0), 1);
    assert_eq!(quantity::after_typing("5", 0), 0);
    // Make All with M = 1,200 → 999.
    assert_eq!(quantity::make_all(1_200), 999);
    assert_eq!(quantity::to_send(0), None);
    assert_eq!(quantity::to_send(5_000), Some(999));
    // Hostile texts.
    assert_eq!(quantity::read("x12", m), 0);
    assert_eq!(quantity::read("99999999999999999999999", m), u64::MAX);
    assert_eq!(quantity::limit_text("12345"), "123");
}

#[test]
fn critical_chance_example_15_5() {
    assert_eq!(rules::critical_percent(0.05, 0.02, [0.25]), 32);
    assert_eq!(rules::critical_percent(0.05, 0.02, [0.25, 0.35]), 67);
    assert_eq!(rules::critical_percent(0.05, 0.02, [1.0]), 100);
    assert_eq!(rules::critical_percent(-3.0, 0.0, []), 0);
    assert_eq!(rules::critical_percent(f32::NAN, 0.0, []), 0);
    assert_eq!(rules::bonus_percent(0.35), 35);
    assert_eq!(rules::round_half_away(-2.5), -3);
    assert_eq!(rules::round_half_away(2.5), 3);
}

#[test]
fn search_words_and_rows() {
    let w = rules::search_words("  Iron  BAR ");
    assert_eq!(w, vec!["iron".to_owned(), "bar".to_owned()]);
    assert!(rules::row_passes("Iron Bar (2)", &w));
    assert!(!rules::row_passes("Iron Ingot", &w));
    assert!(rules::row_passes("anything", &[]));
    assert!(rules::search_words(&"x".repeat(10_000)).len() <= 1);
}

// -------------------------------------------------------------------------------------------
// §4 Panel
// -------------------------------------------------------------------------------------------

#[test]
fn state_follows_the_feed() {
    let mut s = CraftingState::new();
    let p = RecordRef::Record(PROF);
    assert!(s.apply(&CraftEvent::Profession {
        what: p.clone(),
        report: report(1, (0, 200), (0, 400), None),
    }));
    assert!(!s.apply(&CraftEvent::Profession {
        what: p.clone(),
        report: report(1, (0, 200), (0, 400), None),
    }));
    learned(&mut s, RECIPE);
    learned(&mut s, COOL);
    learned(&mut s, RECIPE);
    assert_eq!(s.recipes().len(), 2);
    assert!(s.apply(&CraftEvent::RecipeForgotten(RecordRef::Record(COOL))));
    assert_eq!(s.recipes().len(), 1);
    assert!(!s.has_vocation() && !s.shows_train_more() && !s.slots_full());
    s.apply(&CraftEvent::Vocation {
        vocation: Some(RecordRef::Record(VOCATION)),
        slots: Some(3),
    });
    assert!(s.has_vocation() && s.shows_train_more() && !s.slots_full());
    s.apply(&CraftEvent::Vocation {
        vocation: None,
        slots: Some(1),
    });
    assert!(!s.has_vocation() && s.slots_full());
}

#[test]
fn page_groups_orders_and_counts() {
    let m = install();
    let cp = presenter(&m);
    let mut s = CraftingState::new();
    let p = RecordRef::Record(PROF);
    s.apply(&CraftEvent::Profession {
        what: p.clone(),
        report: report(2, (120, 400), (0, 400), Some(1)),
    });
    learned(&mut s, RECIPE);
    learned(&mut s, COOL);
    learned(&mut s, CONTAINER);
    // A recipe of another profession is not listed.
    s.apply(&CraftEvent::RecipeLearned(LearnedRecipe {
        recipe: RecordRef::Literal("Stew".into()),
        profession: Some(RecordRef::Literal("Cook".into())),
        tier: Some(1),
        category: None,
    }));
    let player = holdings(&[(ORE, 7), (COAL, 11), (PACK, 7)]);
    let page = cp.page(&p, &s, &player);
    assert_eq!(page.name.as_deref(), Some("Smith"));
    assert_eq!(cp.title().as_deref(), Some("Crafting"));
    let tiers: Vec<u16> = page.tiers.iter().map(|t| t.tier).collect();
    assert_eq!(tiers, vec![1, 2]);
    assert_eq!(page.recipe_rows(), 3);
    // Tier 2: categories ordered by text with case folded: "axes" before "Swords".
    let t2 = &page.tiers[1];
    assert_eq!(t2.text.as_deref(), Some("Tier 2"));
    let cats: Vec<Option<u32>> = t2.categories.iter().map(|c| c.category).collect();
    assert_eq!(cats, vec![Some(8), Some(7)]);
    let row = page.find(&RecordRef::Record(RECIPE), 2).expect("row");
    // Name: the result item's name; n = min(7/2, 11/5) = 2; p = 7/3 = 2.
    assert_eq!(row.name.as_deref(), Some("Bar"));
    assert_eq!(row.text.as_deref(), Some("Bar (2)"));
    assert_eq!((row.makeable, row.packs), (2, 2));
    assert_eq!(row.pack_text.as_deref(), Some("2"));
    assert_eq!(row.template, RowTemplate::Plain);
    assert!(row.craftable && t2.active && t2.categories[1].active);
    // Tier marks of tier 2 (a = 1): the proficiency meter with its text.
    let [TierMark::ProficiencyMeter(meter)] = t2.marks.as_slice() else {
        panic!("{:?}", t2.marks)
    };
    assert_eq!(meter.text.as_deref(), Some("120/400"));
    // Cooldown one-use recipe: template Cooldown; makeable 1 while ready, 0 while running.
    let cool = page.find(&RecordRef::Record(COOL), 1).expect("cool row");
    assert_eq!(cool.template, RowTemplate::Cooldown);
    assert_eq!(cool.text.as_deref(), Some("Daily bar (1)"));
    let mut running = player.clone();
    running.cooldowns.insert(RecordRef::Record(COOL));
    let page2 = cp.page(&p, &s, &running);
    let cool = page2.find(&RecordRef::Record(COOL), 1).expect("cool row");
    assert!(cool.cooldown_running && !cool.craftable);
    assert_eq!(cool.text.as_deref(), Some("Daily bar"));
    // Multi-output template.
    let c = page
        .find(&RecordRef::Record(CONTAINER), 2)
        .expect("container");
    assert_eq!(c.template, RowTemplate::MultiOutput);
    // Nothing held: no active states.
    let empty = cp.page(&p, &s, &PlayerCraft::default());
    assert!(empty.tiers.iter().all(|t| !t.active));
    // Search keeps only matching recipes and drops empty parents.
    let f = page.filtered("daily");
    assert_eq!(f.recipe_rows(), 1);
    assert_eq!(f.tiers.len(), 1);
    assert_eq!(page.filtered("").recipe_rows(), 3);
    assert_eq!(page.filtered("nothing").tiers.len(), 0);
}

#[test]
fn recipe_page_parts_and_request() {
    let m = install();
    let cp = presenter(&m);
    let mut s = CraftingState::new();
    let p = RecordRef::Record(PROF);
    // Tier 2 mastered (mastery tier 3 in progress).
    s.apply(&CraftEvent::Profession {
        what: p.clone(),
        report: report(3, (0, 400), (0, 800), Some(3)),
    });
    learned(&mut s, RECIPE);
    let mut player = holdings(&[(ORE, 7), (COAL, 11), (GEM, 1)]);
    player.stats.insert(CRIT_MOD_PID, 0.02);
    let r = RecordRef::Record(RECIPE);
    let controls = PageControls::default();
    let page = cp.recipe_page(&r, 2, &s, &player, &controls);
    assert_eq!(page.name.as_deref(), Some("Bar"));
    assert_eq!(
        page.result,
        ResultDisplay::Item {
            item: RecordRef::Record(BAR),
            quantity: 2
        }
    );
    assert_eq!(page.experience.as_deref(), Some("40 xp at Tier 2"));
    assert_eq!(page.cooldown, None);
    assert_eq!(page.tool.text.as_deref(), Some("tool: Hammer"));
    assert!(!page.tool.met, "no tool bits");
    assert_eq!(page.facility.text.as_deref(), Some("at: Forge"));
    assert!(!page.facility.met);
    assert_eq!(page.ingredients.len(), 2);
    assert_eq!(page.ingredients[0].info.as_deref(), Some("7/2 Ore"));
    assert!(page.ingredients.iter().all(|i| i.enough));
    let pack = page.pack.as_ref().expect("pack");
    assert_eq!(pack.checkbox_label.as_deref(), Some("use Ore pack"));
    assert!(!pack.slot.enough);
    let master = page.master.as_ref().expect("master page");
    assert!(!master.overlay, "tier 2 is mastered");
    assert_eq!(master.optional.len(), 2);
    assert!(master.optional[0].enabled && master.optional[0].visible);
    assert_eq!(master.optional[1].bonus_text.as_deref(), Some("+35%"));
    // 0.05 + 0.02 = 7 %.
    assert_eq!(master.chance_percent, 7);
    assert_eq!(master.chance_text.as_deref(), Some("7%"));
    assert!(!page.make_enabled, "tool and facility missing");
    assert_eq!(page.limit, 2);
    assert_eq!(page.quantity, 1);

    // With the tool, the facility and both optional slots checked.
    player.tool_bits = 0x8 | 0x1;
    player.effects.insert(FORGE.0);
    let checked = PageControls {
        optional: 0b11,
        ..PageControls::default()
    };
    let page = cp.recipe_page(&r, 2, &s, &player, &checked);
    assert!(page.tool.met && page.facility.met && page.make_enabled);
    let master = page.master.as_ref().expect("master");
    // 0.05 + 0.02 + 0.25 + 0.35 = 67 %.
    assert_eq!(master.chance_percent, 67);
    let req = page.request(2, false).expect("make");
    assert_eq!(
        req,
        CraftRequest {
            recipe: r.clone(),
            quantity: 2,
            tier: Some(2),
            output: 0,
            optional: 0b11,
            pack: false
        }
    );
    let all = page.request(0, true).expect("make all");
    assert_eq!(all.quantity, 2);
    assert_eq!(page.request(0, false), None, "nothing sent for 0");
    // Busy: Make disabled (CI5).
    let busy = PageControls {
        busy: true,
        ..checked.clone()
    };
    assert!(!cp.recipe_page(&r, 2, &s, &player, &busy).make_enabled);
    // A channelling state that bars crafting disables Make.
    let mut barred = player.clone();
    barred.channel = Some(CHANNEL);
    assert!(!cp.recipe_page(&r, 2, &s, &barred, &checked).make_enabled);
    // An unmastered tier: overlay, optional slots cleared and disabled.
    let page3 = cp.recipe_page(&r, 3, &s, &player, &checked);
    let m3 = page3.master.expect("master");
    assert!(m3.overlay && m3.optional.iter().all(|o| !o.checked && !o.enabled));
    // Pack mode: M = pack count; the pack is not held.
    let pack_mode = PageControls {
        pack: true,
        ..PageControls::default()
    };
    let pp = cp.recipe_page(&r, 2, &s, &player, &pack_mode);
    assert!(!pp.make_enabled && pp.limit == 0);
}

#[test]
fn non_item_result_outputs_and_cooldown_lines() {
    let m = install();
    let cp = presenter(&m);
    let mut s = CraftingState::new();
    learned(&mut s, CONTAINER);
    learned(&mut s, COOL);
    let r = RecordRef::Record(CONTAINER);
    let player = PlayerCraft::default();
    let page = cp.recipe_page(&r, 2, &s, &player, &PageControls::default());
    assert_eq!(
        page.result,
        ResultDisplay::NonItem {
            icon: Some(Did(0x4100_0001))
        }
    );
    assert_eq!(
        page.outputs,
        Some(OutputSelector {
            pages: 3,
            current: 0
        })
    );
    let second = PageControls {
        output: 2,
        ..PageControls::default()
    };
    let p2 = cp.recipe_page(&r, 2, &s, &player, &second);
    assert_eq!(p2.name.as_deref(), Some("Fine bar"));
    let far = PageControls {
        output: 60,
        ..PageControls::default()
    };
    assert_eq!(
        cp.recipe_page(&r, 2, &s, &player, &far)
            .outputs
            .map(|o| o.current),
        Some(2)
    );
    let cool = cp.recipe_page(
        &RecordRef::Record(COOL),
        1,
        &s,
        &player,
        &PageControls::default(),
    );
    assert!(!cool.one_use);
    assert_eq!(cool.cooldown.as_deref(), Some("every 1d"));
    assert_eq!(cool.facility.text.as_deref(), Some("at: none"));
}

#[test]
fn literal_recipe_scenario_15_4() {
    let m = install();
    let mut cp = presenter(&m);
    let prof = RecordRef::Literal("P".into());
    let mut def = RecipeDef {
        profession: Some(prof.clone()),
        tiers: vec![1],
        category: Some(7),
        ingredients: vec![Part::new(RecordRef::Literal("A".into()), 2)],
        result: Some(Part::new(RecordRef::Literal("Cup".into()), 1)),
        critical: Some(Part::new(RecordRef::Literal("Fine cup".into()), 1)),
        ..RecipeDef::default()
    };
    def.name = Some("R".into());
    cp.define("R", def);
    let mut s = CraftingState::new();
    s.apply(&CraftEvent::Profession {
        what: prof.clone(),
        report: report(1, (0, 200), (0, 400), None),
    });
    s.apply(&CraftEvent::RecipeLearned(LearnedRecipe::new(
        RecordRef::Literal("R".into()),
    )));
    let mut player = PlayerCraft::default();
    player.add(&RecordRef::Literal("A".into()), 4);
    let page = cp.page(&prof, &s, &player);
    let r = RecordRef::Literal("R".into());
    let row = page.find(&r, 1).expect("row");
    assert_eq!(row.makeable, 2);
    assert_eq!(row.text.as_deref(), Some("R (2)"));
    assert!(page.tiers[0].active);
    let rp = cp.recipe_page(&r, 1, &s, &player, &PageControls::default());
    assert!(rp.make_enabled);
    let req = rp.request(2, false).expect("craft R qty=2");
    assert_eq!(req.quantity, 2);
    // After the crafts: A used, proficiency 40/200.
    s.apply(&CraftEvent::Profession {
        what: prof.clone(),
        report: report(1, (40, 200), (0, 400), None),
    });
    let page = cp.page(&prof, &s, &PlayerCraft::default());
    let [TierMark::ProficiencyMeter(meter)] = page.tiers[0].marks.as_slice() else {
        panic!("{:?}", page.tiers[0].marks)
    };
    assert!((meter.position - 0.2).abs() < 1e-6);
    assert_eq!(page.find(&r, 1).map(|r| r.makeable), Some(0));
    assert!(!page.tiers[0].active);
    // An undefined literal is listed by its ref with no ingredients.
    // A one-use literal: template OneShot, makeable at most 1.
    cp.define(
        "Once",
        RecipeDef {
            profession: Some(prof.clone()),
            tiers: vec![1],
            one_use: true,
            ingredients: vec![Part::new(RecordRef::Literal("A".into()), 1)],
            ..RecipeDef::default()
        },
    );
    s.apply(&CraftEvent::RecipeLearned(LearnedRecipe::new(
        RecordRef::Literal("Once".into()),
    )));
    let mut five = PlayerCraft::default();
    five.add(&RecordRef::Literal("A".into()), 5);
    let page = cp.page(&prof, &s, &five);
    let once = page
        .find(&RecordRef::Literal("Once".into()), 1)
        .expect("one-use row");
    assert_eq!((once.template, once.makeable), (RowTemplate::OneShot, 1));
    let other = cp.recipe(&RecordRef::Literal("Q".into())).expect("literal");
    assert_eq!(other.name.as_deref(), Some("Q"));
    assert!(other.ingredients.is_empty());
    // Bar and result lines.
    let bar = cp.craft_bar(&r);
    assert_eq!(bar.label.as_deref(), Some("R"));
    let line = cp.result_line(&r, CraftOutcome::Critical, None);
    assert_eq!(line.lead, ResultLead::CriticalSuccess);
    assert_eq!(line.subject.as_deref(), Some("R"));
}

#[test]
fn bar_results_scroll_rows_and_world_views() {
    let m = install();
    let cp = presenter(&m);
    let bar = cp.craft_bar(&RecordRef::Record(RECIPE));
    assert_eq!(bar.label.as_deref(), Some("Smelting"));
    assert!(bar.interruptible);
    let line = cp.result_line(
        &RecordRef::Record(RECIPE),
        CraftOutcome::Success,
        Some((&RecordRef::Record(BAR), 2)),
    );
    assert_eq!(line.lead, ResultLead::Text("Made".into()));
    assert_eq!(line.subject.as_deref(), Some("Bar"));
    assert_eq!(line.quantity, Some(2));
    let failed = cp.result_line(&RecordRef::Record(RECIPE), CraftOutcome::Failed, None);
    assert_eq!(failed.lead, ResultLead::Text("Failed".into()));
    assert_eq!(failed.subject.as_deref(), Some("Bar"));

    let scroll = m.records.get(&SCROLL).expect("scroll").clone();
    let rows = cp.scroll_rows(&scroll).expect("rows");
    assert_eq!(rows.recipe, COOL);
    assert_eq!(rows.one_shot, None);
    assert_eq!(rows.expected, (false, true));
    assert_eq!(rows.cooldown.as_deref(), Some("wait 1d"));
    let plain = m.records.get(&ORE).expect("ore").clone();
    assert!(cp.scroll_rows(&plain).is_none());

    let node_set = m.records.get(&NODE).expect("node").clone();
    let node = cp.gathering_node(NODE, &node_set).expect("node");
    assert_eq!(node.name.as_deref(), Some("Copper vein"));
    assert_eq!(node.induction, Some(INDUCTION));
    assert_eq!(node.tool, Some(0x8));
    assert_eq!(node.tier, Some(1));
    assert_eq!(node.use_distance, Some(3.5));
    assert!(cp.gathering_node(ORE, &plain).is_none());
}

#[test]
fn trainer_rows_and_windows() {
    let m = install();
    let cp = presenter(&m);
    let row = cp.trainer_option(TrainerKind::Vocation, &RecordRef::Record(VOCATION));
    assert_eq!(row.text.as_deref(), Some("Armourer"));
    assert_eq!(row.description.as_deref(), Some("Smith"));
    let g = cp.trainer_option(TrainerKind::Guild, &RecordRef::Record(GUILD));
    assert_eq!(g.text.as_deref(), Some("Smiths' guild"));
    let p = cp.trainer_option(TrainerKind::Profession, &RecordRef::Record(PROF));
    assert_eq!(p.text.as_deref(), Some("Smith"));

    let mut s = CraftingState::new();
    let w = cp.trainer_window(
        TrainerKind::Vocation,
        &RecordRef::Record(VOCATION),
        Some("Tom"),
        &s,
    );
    assert_eq!(w.name_text.as_deref(), Some("Tom teaches"));
    assert_eq!(w.display_text.as_deref(), Some("learn armourer"));
    assert!(!w.confirm_train);
    s.apply(&CraftEvent::Vocation {
        vocation: Some(RecordRef::Record(VOCATION)),
        slots: Some(1),
    });
    let w = cp.trainer_window(
        TrainerKind::Vocation,
        &RecordRef::Record(VOCATION),
        None,
        &s,
    );
    assert_eq!(w.display_text.as_deref(), Some("become armourer"));
    assert!(w.confirm_train && w.confirm_text.as_deref() == Some("sure?"));
    // Profession trainer: selector only with every slot taken.
    let w = cp.trainer_window(TrainerKind::Profession, &RecordRef::Record(PROF), None, &s);
    assert!(w.selector.is_none());
    assert_eq!(w.display_text.as_deref(), Some("learn smithing"));
    s.apply(&CraftEvent::Profession {
        what: RecordRef::Record(PROF),
        report: ProfessionReport::default(),
    });
    let w = cp.trainer_window(TrainerKind::Profession, &RecordRef::Record(PROF), None, &s);
    let sel = w.selector.expect("selector");
    assert_eq!(sel, vec![(RecordRef::Record(PROF), Some("Smith".into()))]);
    assert_eq!(w.display_text.as_deref(), Some("change to smithing"));
    assert_eq!(w.lose_label.as_deref(), Some("lose one"));
    let guild = cp.trainer_window(TrainerKind::Guild, &RecordRef::Record(GUILD), None, &s);
    assert_eq!(guild.display_text.as_deref(), Some("join the guild"));
}

#[test]
fn hostile_records_never_panic() {
    let mut m = install();
    // A recipe with wrong types, huge counts, absurd tiers, no profession.
    let bad = Did(0x7000_0FFF);
    let ings: Vec<PropertyEntry> = (0..40)
        .map(|i| {
            ingredient(
                &mut m,
                Did(0x7000_1000 + i),
                i32::MIN + i as i32,
                Some(f32::NAN),
            )
        })
        .collect();
    let entries = vec![
        m.e(
            "CraftRecipe_Profession",
            PropertyValue::DidRef(Did(0x7000_0BAD)),
        ),
        m.e("CraftRecipe_Tier", PropertyValue::Enum(u32::MAX)),
        m.e("CraftRecipe_IngredientList", arr(ings)),
        m.e("CraftRecipe_ResultItem", PropertyValue::DidRef(Did(0))),
        m.e(
            "CraftRecipe_ResultItemQuantity",
            PropertyValue::Int(i32::MIN),
        ),
    ];
    m.put(bad, entries);
    let wrong = Did(0x7000_0FFE);
    let e = vec![m.e("CraftRecipe_IngredientList", PropertyValue::Int(3))];
    m.put(wrong, e);
    let cp = presenter(&m);
    let mut s = CraftingState::new();
    for d in [bad, wrong, Did(0x7000_7777)] {
        learned(&mut s, d);
        let r = RecordRef::Record(d);
        let mut controls = PageControls {
            output: u16::MAX,
            optional: u8::MAX,
            optional_page: usize::MAX,
            quantity_text: "9".repeat(100),
            ..PageControls::default()
        };
        for tier in [0, 1, u16::MAX] {
            let page = cp.recipe_page(&r, tier, &s, &PlayerCraft::default(), &controls);
            assert!(page.ingredients.len() <= MAIN_SLOTS);
            let _ = page.request(u64::MAX, true);
            controls.pack = !controls.pack;
        }
        let _ = cp.craft_bar(&r);
        let _ = cp.result_line(&r, CraftOutcome::Failed, Some((&r, u32::MAX)));
    }
    assert!(cp.recipe(&RecordRef::Record(wrong)).is_err());
    let page = cp.page(
        &RecordRef::Record(Did(0x7000_0BAD)),
        &s,
        &PlayerCraft::default(),
    );
    assert!(page.recipe_rows() <= 1);
    let _ = cp.trainer_window(
        TrainerKind::Profession,
        &RecordRef::Record(bad),
        Some(""),
        &s,
    );
}

// -------------------------------------------------------------------------------------------
// Review fixes
// -------------------------------------------------------------------------------------------

/// A player who holds `items`, has the profession's tool and the forge effect.
fn equipped(items: &[(Did, u64)]) -> PlayerCraft {
    let mut p = holdings(items);
    p.tool_bits = 0x8;
    p.effects.insert(FORGE.0);
    p
}

/// Regression: the request carries the output the page shows (§5.2 Tessera rule), never
/// an out-of-range control value.
#[test]
fn request_carries_the_shown_output() {
    let m = install();
    let cp = presenter(&m);
    let mut s = CraftingState::new();
    learned(&mut s, CONTAINER);
    let player = equipped(&[(ORE, 3)]);
    let r = RecordRef::Record(CONTAINER);
    for (asked, shown) in [(0u16, 0u16), (1, 1), (9, 2), (40_000, 2)] {
        let mut controls = PageControls {
            output: asked,
            ..PageControls::default()
        };
        let page = cp.recipe_page(&r, 2, &s, &player, &controls);
        assert!(page.make_enabled);
        assert_eq!(page.outputs.map(|o| o.current), Some(shown));
        assert_eq!(page.request(1, false).map(|q| q.output), Some(shown));
        page.settle(&mut controls);
        assert_eq!(controls.output, shown);
    }
    // A recipe without outputs sends 0 and settles to 0.
    learned(&mut s, RECIPE);
    let mut controls = PageControls {
        output: 7,
        ..PageControls::default()
    };
    let page = cp.recipe_page(
        &RecordRef::Record(RECIPE),
        2,
        &s,
        &equipped(&[(ORE, 2), (COAL, 5)]),
        &controls,
    );
    assert_eq!(page.request(1, false).map(|q| q.output), Some(0));
    page.settle(&mut controls);
    assert_eq!(controls.output, 0);
}

/// Regression: Make sends at most *M* when *M* > 0 (CI6), whatever the entry still holds.
#[test]
fn make_quantity_is_at_most_the_limit() {
    let m = install();
    let cp = presenter(&m);
    let mut s = CraftingState::new();
    learned(&mut s, RECIPE);
    let r = RecordRef::Record(RECIPE);
    // n = min(4/2, 10/5) = 2.
    let page = cp.recipe_page(
        &r,
        2,
        &s,
        &equipped(&[(ORE, 4), (COAL, 10)]),
        &PageControls {
            quantity_text: "5".into(),
            ..PageControls::default()
        },
    );
    assert_eq!(page.limit, 2);
    assert!(page.make_enabled);
    assert_eq!(page.request(5, false).map(|q| q.quantity), Some(2));
    assert_eq!(page.request(1, false).map(|q| q.quantity), Some(1));
    assert_eq!(page.request(500, true).map(|q| q.quantity), Some(2));
}

/// Regression: an optional box that gets disabled is cleared in the controls the caller
/// keeps (§5.5), so it is not checked again when it is enabled again.
#[test]
fn disabled_optional_boxes_stay_cleared() {
    let m = install();
    let cp = presenter(&m);
    let mut s = CraftingState::new();
    s.apply(&CraftEvent::Profession {
        what: RecordRef::Record(PROF),
        report: report(3, (0, 400), (0, 800), Some(3)),
    });
    learned(&mut s, RECIPE);
    let r = RecordRef::Record(RECIPE);
    let with_gem = equipped(&[(ORE, 2), (COAL, 6), (GEM, 1)]);
    let without_gem = equipped(&[(ORE, 2), (COAL, 6)]);
    let mut controls = PageControls {
        optional: 0b1,
        ..PageControls::default()
    };
    // Checked while the gem is held.
    let page = cp.recipe_page(&r, 2, &s, &with_gem, &controls);
    assert!(page.master.as_ref().expect("master").optional[0].checked);
    page.settle(&mut controls);
    assert_eq!(controls.optional, 0b1);
    // The gem is traded away: the box is disabled and cleared.
    let page = cp.recipe_page(&r, 2, &s, &without_gem, &controls);
    let slot = &page.master.as_ref().expect("master").optional[0];
    assert!(!slot.enabled && !slot.checked);
    page.settle(&mut controls);
    assert_eq!(controls.optional, 0);
    // Received again: enabled but not checked.
    let page = cp.recipe_page(&r, 2, &s, &with_gem, &controls);
    let slot = &page.master.as_ref().expect("master").optional[0];
    assert!(slot.enabled && !slot.checked);
    assert_eq!(page.request(1, false).map(|q| q.optional), Some(0));
    // Unused bits and the pager are settled too.
    let mut stray = PageControls {
        optional: 0xFF,
        optional_page: 99,
        ..PageControls::default()
    };
    let page = cp.recipe_page(&r, 2, &s, &with_gem, &stray);
    page.settle(&mut stray);
    assert_eq!(stray.optional & !0b11, 0, "only the recipe's two slots");
    assert_eq!(stray.optional_page, 1);
    // A recipe without a pack or critical result clears the pack box and the bits.
    learned(&mut s, COOL);
    controls.pack = true;
    controls.optional = 0b1;
    let page = cp.recipe_page(&RecordRef::Record(COOL), 1, &s, &with_gem, &controls);
    page.settle(&mut controls);
    assert!(!controls.pack && controls.optional == 0 && controls.optional_page == 0);
}

/// Regression: a required tool whose name does not resolve leaves the tool line without
/// text; the "none" text is only for a profession that needs no tool (§5.3).
#[test]
fn unnamed_required_tool_is_not_none() {
    let mut m = install();
    let tool = m.p("CraftProfession_RequiredTool");
    m.enum_texts.remove(&(tool, 4));
    let cp = presenter(&m);
    let mut s = CraftingState::new();
    learned(&mut s, RECIPE);
    let page = cp.recipe_page(
        &RecordRef::Record(RECIPE),
        2,
        &s,
        &PlayerCraft::default(),
        &PageControls::default(),
    );
    assert_eq!(page.tool.text, None);
    assert!(!page.tool.met);
    // A literal recipe (no profession record): no tool needed, the none text.
    let lit = RecordRef::Literal("Plain".into());
    let page = cp.recipe_page(
        &lit,
        1,
        &s,
        &PlayerCraft::default(),
        &PageControls::default(),
    );
    assert_eq!(page.tool.text.as_deref(), Some("tool: none"));
    assert!(page.tool.met);
}

/// Every recipe category of §10.1 (with the Tessera `Unreadable` and `TooManyTiers`), one
/// synthetic record each.
#[test]
fn census_recipe_categories() {
    const BASE: u32 = 0x7000_0A00;
    let mut m = install();
    let tier = m.p("CraftRecipe_Tier");
    for t in 4..=70 {
        m.enum_texts.insert((tier, t), format!("Tier {t}"));
    }
    let recipe = |m: &mut Mem, did: u32, f: &dyn Fn(&mut Mem, &mut Vec<PropertyEntry>)| {
        let ings = vec![ingredient(m, ORE, 1, None)];
        let mut e = vec![
            m.e("CraftRecipe_Profession", PropertyValue::DidRef(PROF)),
            m.e("CraftRecipe_Tier", PropertyValue::Enum(2)),
            m.e("CraftRecipe_UICategory", PropertyValue::Enum(7)),
            m.e("CraftRecipe_IngredientList", arr(ings)),
            m.e("CraftRecipe_ResultItem", PropertyValue::DidRef(BAR)),
            m.e("CraftRecipe_Name", si(literal("Thing"))),
        ];
        f(m, &mut e);
        m.put(Did(did), e);
    };
    let set = |label: &'static str, v: PropertyValue| {
        move |m: &mut Mem, e: &mut Vec<PropertyEntry>| {
            let pid = m.p(label);
            e.retain(|x| x.pid != pid);
            e.push(m.e(label, v.clone()));
        }
    };
    recipe(&mut m, BASE + 1, &|_, _| {});
    recipe(
        &mut m,
        BASE + 2,
        &set("CraftRecipe_IngredientList", PropertyValue::Int(3)),
    );
    recipe(
        &mut m,
        BASE + 3,
        &set("CraftRecipe_Profession", PropertyValue::DidRef(ORE)),
    );
    recipe(
        &mut m,
        BASE + 4,
        &set("CraftRecipe_Tier", PropertyValue::Enum(99)),
    );
    recipe(
        &mut m,
        BASE + 5,
        &set("CraftRecipe_UICategory", PropertyValue::Enum(99)),
    );
    recipe(&mut m, BASE + 6, &|m, e| {
        let pid = m.p("CraftRecipe_Name");
        e.retain(|x| x.pid != pid);
        let pid = m.p("CraftRecipe_ResultItem");
        e.retain(|x| x.pid != pid);
        e.push(m.e(
            "CraftRecipe_ResultItem",
            PropertyValue::DidRef(Did(BASE + 0x50)),
        ));
    });
    recipe(&mut m, BASE + 7, &|m, e| {
        let ings = vec![ingredient(m, Did(BASE + 0x51), 1, None)];
        let pid = m.p("CraftRecipe_IngredientList");
        e.retain(|x| x.pid != pid);
        e.push(m.e("CraftRecipe_IngredientList", arr(ings)));
    });
    recipe(&mut m, BASE + 8, &|m, e| {
        // 70 tiers, two repeats: 68 distinct, 4 past the limit of 64.
        let mut tiers: Vec<PropertyEntry> = (1..=68)
            .map(|t| m.e("CraftRecipe_Tier", PropertyValue::Enum(t)))
            .collect();
        tiers.push(m.e("CraftRecipe_Tier", PropertyValue::Enum(1)));
        tiers.push(m.e("CraftRecipe_Tier", PropertyValue::Enum(2)));
        e.push(m.e("CraftRecipe_TierArray", arr(tiers)));
    });
    recipe(&mut m, BASE + 9, &|m, e| {
        let ings = (0..7).map(|_| ingredient(m, ORE, 1, None)).collect();
        let pid = m.p("CraftRecipe_IngredientList");
        e.retain(|x| x.pid != pid);
        e.push(m.e("CraftRecipe_IngredientList", arr(ings)));
    });
    recipe(
        &mut m,
        BASE + 10,
        &set("CraftRecipe_ResultItem", PropertyValue::DidRef(CHEST)),
    );
    let cp = presenter(&m);
    let scrolls = std::collections::BTreeMap::new();
    let expect = [
        (BASE + 1, RecipeCategory::Shown),
        (BASE + 2, RecipeCategory::Unreadable),
        (BASE + 0xFF, RecipeCategory::Unreadable),
        (BASE + 3, RecipeCategory::ProfessionMissing),
        (BASE + 4, RecipeCategory::TierTextMissing),
        (BASE + 5, RecipeCategory::CategoryTextMissing),
        (BASE + 6, RecipeCategory::NameMissing),
        (BASE + 7, RecipeCategory::IngredientMissing),
        (BASE + 8, RecipeCategory::TooManyTiers),
        (BASE + 9, RecipeCategory::TooManySlots),
        (BASE + 10, RecipeCategory::ResultImageMissing),
        (RECIPE.0, RecipeCategory::Shown),
        (CONTAINER.0, RecipeCategory::Shown),
        (COOL.0, RecipeCategory::Shown),
    ];
    for (did, want) in expect {
        let o = census::recipe_outcome(&cp, Did(did), &scrolls);
        assert_eq!((o.did, o.category), (Did(did), want), "{did:#x}");
        assert!(o.failures.is_empty(), "{did:#x}: {:?}", o.failures);
        assert_eq!(o.info.is_none(), want == RecipeCategory::Unreadable);
    }
    // The tiers are deduplicated and capped.
    let info = cp.recipe(&RecordRef::Record(Did(BASE + 8))).expect("info");
    assert_eq!(info.tiers.len(), MAX_RECIPE_TIERS);
    assert_eq!(info.tiers_dropped, 4);
    assert_eq!(info.tiers[..3], [1, 2, 3]);
}

/// Literal definitions are deduplicated and capped the same way.
#[test]
fn literal_tiers_are_distinct() {
    let m = install();
    let mut cp = presenter(&m);
    cp.define(
        "Twice",
        RecipeDef {
            tiers: vec![1, 1, 2, 1],
            ..RecipeDef::default()
        },
    );
    let info = cp
        .recipe(&RecordRef::Literal("Twice".into()))
        .expect("defined");
    assert_eq!((info.tiers.clone(), info.tiers_dropped), (vec![1, 2], 0));
}

/// Learning and forgetting at the state's bound: learn order kept, positions follow
/// removals, and no linear scan per event.
#[test]
fn learned_recipes_bulk() {
    let mut s = CraftingState::new();
    let started = std::time::Instant::now();
    for i in 0..=u32::try_from(MAX_RECIPES).expect("fits") {
        learned(&mut s, Did(i));
    }
    assert_eq!(s.recipes().len(), MAX_RECIPES);
    assert!(!s.apply(&CraftEvent::RecipeForgotten(RecordRef::Literal("x".into()))));
    assert!(s.apply(&CraftEvent::RecipeForgotten(RecordRef::Record(Did(3)))));
    assert!(!s.apply(&CraftEvent::RecipeForgotten(RecordRef::Record(Did(3)))));
    assert_eq!(s.recipes()[3].recipe, RecordRef::Record(Did(4)));
    // A re-learn after a removal finds the shifted position.
    let again = LearnedRecipe {
        tier: Some(9),
        ..LearnedRecipe::new(RecordRef::Record(Did(40_000)))
    };
    assert!(s.apply(&CraftEvent::RecipeLearned(again.clone())));
    assert_eq!(s.recipes()[39_999], again);
    assert_eq!(s.learned(&RecordRef::Record(Did(40_000))), Some(&again));
    assert_eq!(s.learned(&RecordRef::Record(Did(3))), None);
    learned(&mut s, Did(3));
    assert_eq!(
        s.recipes().last().map(|l| &l.recipe),
        Some(&RecordRef::Record(Did(3)))
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
}
