// SPDX-License-Identifier: GPL-3.0-or-later
//! Test support of the P6 tests: the P5 test environment (synthetic layouts, generated art,
//! Tessera's bundled font, click events, painting a window into an RGBA image), the P6
//! fallback art, synthetic crafting and progression views, and the committed-golden check
//! of decision 0031 tier 1 for `tests/golden/p6_<name>.png`. All values are invented; no
//! install file is opened.
#![allow(dead_code)]

#[path = "../p5_common/mod.rs"]
pub mod p5;

#[allow(unused_imports)]
pub use p5::{
    Rgba, art_ref, button, capture, double, left, message, paint_frames, part, real_click, right,
    settle, ui, ui_layouts,
};
use tessera_ui::game::IconImage;
use tessera_ui::game::crafting::{
    CategoryRowView, CraftPanelView, IngredientView, MasterView, MeterView, OptionalView, P6Art,
    PackView, ProfessionPageView, RecipePageView, RecipeRowView, RequirementView, ResultView,
    RowKind, TierMarkView, TierRowView,
};
use tessera_ui::game::{NpcVocab, P6Vocab};

/// The P6 fallback art over the generated images of the P5 environment (check boxes, the
/// proficiency and mastery icons and the row icons reuse generated images).
pub fn art() -> P6Art {
    let a = |i| Some(art_ref(i));
    P6Art {
        base: p5::art().base,
        check_off: a(11),
        check_on: a(19),
        proficiency: a(20),
        mastery: a(25),
        one_use: a(26),
        multi_output: a(28),
        cooldown: a(29),
        cooldown_ready: a(19),
        pack: a(32),
    }
}

/// The vocabularies (placeholders).
pub fn vocabs() -> (NpcVocab, P6Vocab) {
    (NpcVocab::default(), P6Vocab::default())
}

/// An item icon of generated art `i`.
pub fn icon(i: u32) -> Option<IconImage> {
    Some(IconImage::Composed(art_ref(i)))
}

/// A recipe row.
pub fn row(recipe: u64, tier: u16, kind: RowKind, text: &str, craftable: bool) -> RecipeRowView {
    RecipeRowView {
        recipe,
        tier,
        kind,
        text: text.into(),
        packs: None,
        craftable,
        cooldown: None,
    }
}

/// A meter view of `current` / `cap`.
pub fn meter(current: u64, cap: u64) -> MeterView {
    MeterView {
        current,
        cap,
        text: format!("{current}/{cap}"),
        tooltip: format!("{current} of {cap}"),
    }
}

/// The panel of `crafting-panels.md` §15.4 after the set-up (state (a) of decision 0037):
/// profession P at tier 1 with proficiency 0/200, recipe R (handle 1) under "Tools" with
/// count 2.
pub fn panel_a() -> CraftPanelView {
    CraftPanelView {
        title: String::new(),
        professions: vec![ProfessionPageView {
            name: "Smithing".into(),
            tiers: vec![TierRowView {
                tier: 1,
                text: "Apprentice".into(),
                marks: vec![TierMarkView::ProficiencyMeter(meter(0, 200))],
                categories: vec![CategoryRowView {
                    text: "Tools".into(),
                    recipes: vec![row(1, 1, RowKind::Plain, "Iron Tongs (2)", true)],
                }],
            }],
        }],
        train_more: None,
    }
}

/// The recipe page of R in state (a): ingredient A ×2 with 4 held, the tool met, makeable 2.
pub fn page_a() -> RecipePageView {
    RecipePageView {
        recipe: 1,
        tier: 1,
        output: 0,
        name: "Iron Tongs".into(),
        name_colour: None,
        result: ResultView::Item {
            icon: icon(12),
            quantity: 1,
            name: "Iron Tongs".into(),
        },
        outputs: None,
        one_use: false,
        experience: Some("Apprentice experience: 20".into()),
        cooldown_line: None,
        cooldown_timer: None,
        tool: RequirementView {
            text: "Requires: Smithing tools".into(),
            met: true,
        },
        facility: RequirementView {
            text: "Requires: no facility".into(),
            met: true,
        },
        ingredients: vec![IngredientView {
            icon: icon(13),
            needed: 2,
            held: 4,
            info: "4/2 Iron Ingot".into(),
        }],
        pack: None,
        master: None,
        makeable: 2,
        pack_count: 0,
        channel_blocked: false,
    }
}

/// The panel of `crafting-panels.md` §15.2 (state (b) of decision 0037): `profession P
/// tier=3 proficiency=120/400 mastery=60/800 mastery-tier=2` with recipes at tiers 1–3.
pub fn panel_b() -> CraftPanelView {
    let tier =
        |t: u16, text: &str, marks: Vec<TierMarkView>, recipe: (u64, &str, bool, RowKind)| {
            TierRowView {
                tier: t,
                text: text.into(),
                marks,
                categories: vec![CategoryRowView {
                    text: "Weapons".into(),
                    recipes: vec![row(recipe.0, t, recipe.3, recipe.1, recipe.2)],
                }],
            }
        };
    let mut cooldown = row(13, 3, RowKind::Cooldown, "Battle Axe", false);
    cooldown.cooldown = Some("2h 10m".into());
    let mut packs = row(14, 3, RowKind::OneUse, "Warhammer (1)", true);
    packs.packs = Some("3".into());
    let mut t3 = tier(
        3,
        "Journeyman",
        vec![TierMarkView::ProficiencyMeter(meter(120, 400))],
        (12, "Long Sword", false, RowKind::MultiOutput),
    );
    t3.categories[0].recipes.push(cooldown);
    t3.categories[0].recipes.push(packs);
    CraftPanelView {
        title: String::new(),
        professions: vec![
            ProfessionPageView {
                name: "Smithing".into(),
                tiers: vec![
                    tier(
                        1,
                        "Apprentice",
                        vec![
                            TierMarkView::ProficiencyIcon("Proficient".into()),
                            TierMarkView::MasteryIcon("Mastered".into()),
                        ],
                        (10, "Dagger (2)", true, RowKind::Plain),
                    ),
                    tier(
                        2,
                        "Novice",
                        vec![
                            TierMarkView::ProficiencyIcon("Proficient".into()),
                            TierMarkView::MasteryMeter(meter(60, 800)),
                        ],
                        (11, "Short Sword", false, RowKind::Plain),
                    ),
                    t3,
                ],
            },
            ProfessionPageView {
                name: "Tailoring".into(),
                tiers: Vec::new(),
            },
        ],
        train_more: Some("Visit a trainer to learn another profession.".into()),
    }
}

/// A recipe page with a master page: base chance 0.07, optional slots with bonuses 0.25 and
/// 0.35 (both held), a pack (`pack_count` 1,200), a critical item.
pub fn page_master(mastered: bool) -> RecipePageView {
    let mut p = page_a();
    p.recipe = 10;
    p.name = "Dagger".into();
    p.master = Some(MasterView {
        mastered,
        optional: vec![
            OptionalView {
                slot: IngredientView {
                    icon: icon(14),
                    needed: 1,
                    held: 2,
                    info: "2/1 Fine Oil".into(),
                },
                bonus: 0.25,
                bonus_text: "+25%".into(),
            },
            OptionalView {
                slot: IngredientView {
                    icon: icon(29),
                    needed: 1,
                    held: 1,
                    info: "1/1 Rare Gem".into(),
                },
                bonus: 0.35,
                bonus_text: "+35%".into(),
            },
        ],
        critical: ResultView::Item {
            icon: icon(30),
            quantity: 2,
            name: "Fine Dagger".into(),
        },
        base_chance: 0.07,
    });
    p.pack = Some(PackView {
        slot: IngredientView {
            icon: icon(32),
            needed: 1,
            held: 1200,
            info: "1200/1 Dagger Kit".into(),
        },
        checkbox_label: "Use Dagger Kit".into(),
        packs_label: "Dagger Kit".into(),
    });
    p.pack_count = 1200;
    p
}

/// Decision 0031 tier 1: compares `img` with `tests/golden/p6_<name>.png` (the P5 check,
/// [`p5::golden_file`]).
pub fn golden(name: &str, img: &Rgba) {
    p5::golden_file(&format!("p6_{name}"), img);
}
