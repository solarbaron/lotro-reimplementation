// SPDX-License-Identifier: GPL-3.0-or-later
//! Committed synthetic goldens of the P6 windows (decisions 0031 tier 1 and 0037 §2): the
//! experience bar part-filled and just after a level-up with the level-up banner frozen at a
//! fixed virtual time, the trait panel, the skill-training window, the crafting-trainer
//! window and the crafting panel in its two states of decision 0037 §2 ((a) the content of
//! `crafting-panels.md` §15.4, (b) the tier rows of §15.2), each built with the fallback
//! windows, filled with literal values and generated art, painted by the CPU painter and
//! compared with `tests/golden/p6_<name>.png` (one step per channel, no outliers).
//! `TESSERA_BLESS=1` rewrites the images. No install file is opened.
//!
//! These are **widget goldens**: the windows are filled from hand-built views, not from a
//! scenario run (`tessera-ui` links no session layer); the scenario-driven state of
//! decision 0037 §3 item 5 belongs to `apps/client`, which builds the same views.

mod p6_common;

use p6_common::*;
use tessera_ui::game::GameVocab;
use tessera_ui::game::crafting::{
    CraftPanel, CraftTemplates, CraftTrainerView, CraftTrainerWindow, DefaultCraftTexts,
    TrainerKind, fallback as craft_fallback,
};
use tessera_ui::game::npc::coins::coin_counts;
use tessera_ui::game::progression::fallback;
use tessera_ui::game::progression::{
    BranchView, DefaultProgressionTexts, ExperienceBar, ExperienceView, LevelUpBanner,
    SkillRowView, SkillTrainerView, SkillTrainerWindow, TraitCellView, TraitPanel, TraitPanelView,
    TraitSlotView,
};

const PT: DefaultProgressionTexts = DefaultProgressionTexts;
const CT: DefaultCraftTexts = DefaultCraftTexts;
const VALUES: [u64; 3] = [100_000, 100, 1];

fn xp(level: u32, current: u64, needed: u64) -> ExperienceView {
    ExperienceView {
        level,
        level_text: format!("Level {level}"),
        current,
        needed,
        text: format!("{current} / {needed}"),
        tooltip: String::new(),
    }
}

#[test]
fn experience_bar() {
    let mut ui = ui(1024, 768);
    let (nv, p6) = vocabs();
    let root = fallback::experience_bar(&mut ui, &nv, &p6, &art()).unwrap();
    let mut bar = ExperienceBar::attach(&ui, &GameVocab::default(), root);
    bar.show(&mut ui, &xp(12, 1500, 4000));
    golden("xp_bar", &capture(&mut ui, root));
}

#[test]
fn experience_bar_after_level_up() {
    let mut ui = ui(1024, 768);
    let (nv, p6) = vocabs();
    let root = fallback::experience_bar(&mut ui, &nv, &p6, &art()).unwrap();
    let mut bar = ExperienceBar::attach(&ui, &GameVocab::default(), root);
    bar.show(&mut ui, &xp(12, 3900, 4000));
    // The feed's level-up to 13 at virtual time 30 s and the new bar; the banner is frozen
    // at 32 s (fully shown).
    bar.show(&mut ui, &xp(13, 120, 5200));
    let b = fallback::levelup_banner(&mut ui, &nv, &p6, &art(), &PT).unwrap();
    let mut banner = LevelUpBanner::attach(&ui, b);
    banner.start(&mut ui, 30.0, 13, "You have reached level 13");
    banner.update(&mut ui, 32.0);
    golden("xp_levelup", &capture(&mut ui, root));
    golden("levelup_banner", &capture(&mut ui, b));
}

fn cell(
    id: u64,
    row: u16,
    col: u16,
    rank: u16,
    max: u16,
    pre: Option<bool>,
    avail: bool,
) -> TraitCellView {
    TraitCellView {
        id,
        row,
        col,
        icon: icon(19 + (id % 12) as u32),
        name: format!("Trait {id}"),
        rank,
        max_rank: max,
        rank_text: format!("{rank}/{max}"),
        prerequisites: pre,
        available: avail,
        tooltip: String::new(),
    }
}

#[test]
fn trait_panel() {
    let mut ui = ui(1024, 768);
    let (nv, p6) = vocabs();
    let root = fallback::trait_panel(&mut ui, &nv, &p6, &art(), &PT).unwrap();
    let mut p = TraitPanel::attach(&mut ui, &nv, &p6, root, &art());
    p.show(
        &mut ui,
        &TraitPanelView {
            title: String::new(),
            trees: vec!["The Blade".into(), "The Shield".into(), "The Banner".into()],
            current: 0,
            tree: 500,
            branches: vec![
                BranchView {
                    key: 1,
                    text: "Blade-dancer".into(),
                    chosen: true,
                },
                BranchView {
                    key: 2,
                    text: "Defender".into(),
                    chosen: false,
                },
            ],
            cells: vec![
                cell(1, 0, 0, 3, 5, None, true),
                cell(2, 0, 1, 5, 5, None, false),
                cell(3, 1, 0, 1, 3, Some(true), true),
                cell(4, 1, 1, 0, 3, Some(false), false),
                cell(5, 2, 2, 0, 1, None, true),
                cell(6, 3, 4, 0, 2, Some(false), false),
            ],
            points_text: "Points: 4 available, 9 spent".into(),
            slots: vec![
                TraitSlotView {
                    icon: icon(20),
                    text: "Slot 1: Trait 1 (rank 3)".into(),
                },
                TraitSlotView {
                    icon: icon(22),
                    text: "Slot 2: Trait 3 (rank 1)".into(),
                },
            ],
        },
    );
    golden("trait_panel", &capture(&mut ui, root));
}

#[test]
fn skill_trainer() {
    let mut ui = ui(1024, 768);
    let (nv, p6) = vocabs();
    let root = fallback::skill_trainer(&mut ui, &nv, &p6, &art(), &PT).unwrap();
    let mut w = SkillTrainerWindow::attach(&mut ui, &nv, &p6, root, &art());
    let row =
        |entry: u16, name: &str, level: u32, coin: u64, trained: bool, unmet: bool| SkillRowView {
            entry,
            icon: icon(25 + u32::from(entry)),
            name: name.into(),
            level_text: format!("Level {level}"),
            coin,
            coins: coin_counts(coin, &VALUES),
            trained,
            unmet,
        };
    w.show(
        &mut ui,
        &SkillTrainerView {
            window: 42,
            title: "Ada the Trainer".into(),
            rows: vec![
                row(1, "Sweeping Cut", 10, 1_250, false, false),
                row(2, "Blade Wall", 20, 4_000, false, true),
                row(3, "Quick Strike", 4, 300, true, false),
                row(4, "Heavy Blow", 12, 2_500_000, false, false),
            ],
            player_coin: Some(150_000),
            empty_text: String::new(),
        },
        &PT,
    );
    w.select(&mut ui, Some(0));
    golden("skill_trainer", &capture(&mut ui, root));
}

#[test]
fn craft_trainer() {
    let mut ui = ui(1024, 768);
    let (nv, p6) = vocabs();
    let root =
        craft_fallback::trainer(&mut ui, &nv, &p6, &art(), TrainerKind::Profession, &CT).unwrap();
    let mut w = CraftTrainerWindow::attach(
        &mut ui,
        &nv,
        &p6,
        root,
        TrainerKind::Profession,
        &art(),
        &CT,
    );
    w.show(
        &mut ui,
        &CraftTrainerView {
            window: 7,
            kind: TrainerKind::Profession,
            name: "Ada the Smith".into(),
            text: "Smiths forge weapons and armour from metal. Learning this profession \
                   replaces one of yours."
                .into(),
            replace: Some(vec!["Cooking".into(), "Farming".into(), "Tailoring".into()]),
            confirm: None,
        },
        &CT,
    );
    golden("craft_trainer", &capture(&mut ui, root));
}

fn panel(ui: &mut tessera_ui::Ui) -> CraftPanel {
    let (nv, p6) = vocabs();
    let root = craft_fallback::panel(ui, &nv, &p6, &art(), &CT).unwrap();
    CraftPanel::attach(ui, &nv, &p6, root, CraftTemplates::default(), &art())
}

#[test]
fn crafting_panel_state_a() {
    let mut ui = ui(1024, 768);
    let mut p = panel(&mut ui);
    p.show(&mut ui, &panel_a(), &CT);
    p.select(&mut ui, Some((1, 1)));
    p.show_recipe(&mut ui, Some(&page_a()), &CT);
    golden("crafting_a", &capture(&mut ui, p.root));
}

#[test]
fn crafting_panel_state_b() {
    let mut ui = ui(1024, 768);
    let mut p = panel(&mut ui);
    p.show(&mut ui, &panel_b(), &CT);
    p.select(&mut ui, Some((10, 1)));
    p.show_recipe(&mut ui, Some(&page_master(true)), &CT);
    golden("crafting_b", &capture(&mut ui, p.root));
}
