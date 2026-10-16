// SPDX-License-Identifier: GPL-3.0-or-later
//! Behaviour of the P6 progression widgets on Tessera's fallback windows with synthetic
//! views (Tessera rules while `progression-panels.md` is missing; decision 0037): the
//! experience bar's fill and texts, the level-up banner on a virtual clock, the trait
//! panel's tabs, branches, cells, prerequisites, points and slots with their requests, and
//! the skill-training window's rows, ghosting, prices and the train request.

mod p6_common;

use p6_common::*;
use tessera_ui::game::npc::coins::coin_counts;
use tessera_ui::game::p6vocab::own;
use tessera_ui::game::progression::fallback;
use tessera_ui::game::progression::xpbar::BannerPhase;
use tessera_ui::game::progression::{
    BranchView, DefaultProgressionTexts, ExperienceBar, ExperienceView, LevelUpBanner,
    ProgressionAction, SkillRowView, SkillTrainerView, SkillTrainerWindow, TraitCellView,
    TraitPanel, TraitPanelView, TraitSlotView,
};
use tessera_ui::game::{GameVocab, NpcVocab};
use tessera_ui::{ElementKey, Ui};

const T: DefaultProgressionTexts = DefaultProgressionTexts;
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
fn experience_bar_fill_and_texts() {
    let mut ui = ui(1024, 768);
    let (nv, p6) = vocabs();
    let root = fallback::experience_bar(&mut ui, &nv, &p6, &art()).unwrap();
    let mut bar = ExperienceBar::attach(&ui, &GameVocab::default(), root);
    assert!(bar.is_attached());
    assert!(bar.meter.is_none());
    bar.show(&mut ui, &xp(12, 1500, 4000));
    assert_eq!(bar.fill_width(&ui), Some(112));
    let l = part(&ui, root, own::XP_LEVEL);
    assert_eq!(ui.text_of(l), "Level 12");
    let t = part(&ui, root, own::XP_TEXT);
    assert_eq!(ui.text_of(t), "1500 / 4000");
    bar.show(&mut ui, &xp(13, 0, 5000));
    assert_eq!(bar.fill_width(&ui), Some(0));
    bar.show(&mut ui, &xp(13, 9000, 5000));
    assert_eq!(bar.fill_width(&ui), Some(300));
}

#[test]
fn levelup_banner_on_a_virtual_clock() {
    let mut ui = ui(1024, 768);
    let (nv, p6) = vocabs();
    let root = fallback::levelup_banner(&mut ui, &nv, &p6, &art(), &T).unwrap();
    let mut b = LevelUpBanner::attach(&ui, root);
    assert!(!ui.is_visible(root));
    b.start(&mut ui, 10.0, 13, "You have reached level 13");
    assert_eq!(b.level(), Some(13));
    assert_eq!(b.phase(10.25), BannerPhase::Shown(0.5));
    b.update(&mut ui, 12.0);
    assert!(ui.is_visible(root));
    let t = part(&ui, root, own::LEVELUP_TEXT);
    assert_eq!(ui.text_of(t), "You have reached level 13");
    b.update(&mut ui, 15.0);
    assert!(!ui.is_visible(root));
    assert_eq!(b.level(), None);
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
        icon: icon(20 + (id % 10) as u32),
        name: format!("Trait {id}"),
        rank,
        max_rank: max,
        rank_text: format!("{rank}/{max}"),
        prerequisites: pre,
        available: avail,
        tooltip: String::new(),
    }
}

/// The trait panel of decision 0037 §2: two tabs, two branches (the first chosen), cells
/// with ranks (spent, maxed, none), a met and an unmet prerequisite, points and a slotted
/// trait.
pub fn trait_view() -> TraitPanelView {
    TraitPanelView {
        title: String::new(),
        trees: vec!["The Blade".into(), "The Shield".into()],
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
        ],
        points_text: "Points: 4 available, 9 spent".into(),
        slots: vec![TraitSlotView {
            icon: icon(26),
            text: "Slot 1: Trait 1 (rank 3)".into(),
        }],
    }
}

fn traits(ui: &mut Ui) -> TraitPanel {
    let (nv, p6) = vocabs();
    let root = fallback::trait_panel(ui, &nv, &p6, &art(), &T).unwrap();
    TraitPanel::attach(ui, &nv, &p6, root, &art())
}

fn cell_el(p: &TraitPanel, id: u64) -> ElementKey {
    let i = p.view().cells.iter().position(|c| c.id == id).unwrap();
    p.cells.iter().find(|(_, j)| *j == i).unwrap().0
}

#[test]
fn trait_panel_cells_and_requests() {
    let mut ui = ui(1024, 768);
    let mut p = traits(&mut ui);
    p.show(&mut ui, &trait_view());
    assert_eq!(p.tabs.len(), 2);
    assert_eq!(p.branches.len(), 2);
    assert_eq!(p.cells.len(), 5);
    assert_eq!(p.slots.len(), 1);
    assert_eq!(p.dropped(), 0);
    // Left on an available cell below its highest rank: rank + 1.
    let e = left(&ui, cell_el(&p, 1));
    assert_eq!(
        p.handle_event(&mut ui, &e),
        Some(ProgressionAction::SetTraitRank {
            tree: 500,
            cell: 1,
            rank: 4
        })
    );
    // At its highest rank, or with an unmet prerequisite: nothing.
    let e = left(&ui, cell_el(&p, 2));
    assert_eq!(p.handle_event(&mut ui, &e), None);
    let e = left(&ui, cell_el(&p, 4));
    assert_eq!(p.handle_event(&mut ui, &e), None);
    // Right with a rank: rank − 1; without: nothing.
    let e = right(&ui, cell_el(&p, 2));
    assert_eq!(
        p.handle_event(&mut ui, &e),
        Some(ProgressionAction::SetTraitRank {
            tree: 500,
            cell: 2,
            rank: 4
        })
    );
    let e = right(&ui, cell_el(&p, 4));
    assert_eq!(p.handle_event(&mut ui, &e), None);
    // The locked overlay and the prerequisite marks.
    let lock =
        |p: &TraitPanel, ui: &Ui, id| ui.is_visible(part(ui, cell_el(p, id), own::TRAIT_CELL_LOCK));
    assert!(lock(&p, &ui, 4));
    assert!(!lock(&p, &ui, 3));
    let pre = |p: &TraitPanel, ui: &Ui, id| {
        ui.is_visible(part(ui, cell_el(p, id), own::TRAIT_CELL_PREREQ))
    };
    assert!(pre(&p, &ui, 3) && pre(&p, &ui, 4) && !pre(&p, &ui, 1));
    // Branches and tabs.
    let e = left(&ui, p.branches[0]);
    assert_eq!(p.handle_event(&mut ui, &e), None);
    let e = left(&ui, p.branches[1]);
    assert_eq!(
        p.handle_event(&mut ui, &e),
        Some(ProgressionAction::ChooseBranch {
            tree: 500,
            branch: 2
        })
    );
    let e = left(&ui, p.tabs[1]);
    assert_eq!(
        p.handle_event(&mut ui, &e),
        Some(ProgressionAction::ShowTree { index: 1 })
    );
    let close = p.close.unwrap();
    let e = button(&ui, close);
    assert_eq!(
        p.handle_event(&mut ui, &e),
        Some(ProgressionAction::CloseTraits)
    );
    let pts = p.points.unwrap();
    assert_eq!(ui.text_of(pts), "Points: 4 available, 9 spent");
}

#[test]
fn trait_cells_outside_the_grid_are_dropped() {
    let mut ui = ui(1024, 768);
    let mut p = traits(&mut ui);
    let mut v = trait_view();
    v.cells.push(cell(9, 40, 0, 0, 1, None, true));
    v.cells.push(cell(10, 0, 400, 0, 1, None, true));
    v.cells.push(cell(11, u16::MAX, u16::MAX, 0, 1, None, true));
    p.show(&mut ui, &v);
    assert_eq!(p.cells.len(), 5);
    assert_eq!(p.dropped(), 3);
    // A second show replaces every part.
    p.show(&mut ui, &trait_view());
    assert_eq!(p.cells.len(), 5);
    assert_eq!(p.tabs.len(), 2);
}

#[test]
fn trait_grid_holds_every_cell_key_in_either_orientation() {
    // combat-data.md §8.5: a = 1–8, b = 1–12 (0-based rows and columns up to 7 and 11).
    let mut ui = ui(1024, 768);
    let mut p = traits(&mut ui);
    let mut v = trait_view();
    v.cells.push(cell(20, 7, 11, 0, 1, None, true));
    v.cells.push(cell(21, 11, 7, 0, 1, None, true));
    v.cells.push(cell(22, 11, 11, 0, 1, None, true));
    p.show(&mut ui, &v);
    assert_eq!(p.dropped(), 0);
    assert_eq!(p.cells.len(), 8);
    settle(&mut ui);
    let grid = ui.parent(cell_el(&p, 22)).unwrap();
    let g = ui.displayed_box(grid);
    for id in [20, 21, 22] {
        let c = ui.displayed_box(cell_el(&p, id));
        assert!(
            c.l >= g.l && c.r <= g.r && c.t >= g.t && c.b <= g.b,
            "cell {id} inside"
        );
    }
    let root = ui.displayed_box(p.root);
    assert!(
        g.b <= root.b && root.h() <= 768,
        "the panel fits a 768-high screen"
    );
}

#[test]
fn trait_panel_closes_only_on_a_left_click() {
    let mut ui = ui(1024, 768);
    let mut p = traits(&mut ui);
    p.show(&mut ui, &trait_view());
    let close = p.close.unwrap();
    let e = right(&ui, close);
    assert_eq!(p.handle_event(&mut ui, &e), None);
    let e = button(&ui, close);
    assert_eq!(
        p.handle_event(&mut ui, &e),
        Some(ProgressionAction::CloseTraits)
    );
}

/// The skill-training window of decision 0037 §2: an affordable row, a row above the
/// player's level (requirements not met), a trained row, a too-expensive row.
pub fn trainer_view() -> SkillTrainerView {
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
    SkillTrainerView {
        window: 42,
        title: "Ada the Trainer".into(),
        rows: vec![
            row(1, "Sweeping Cut", 10, 1_250, false, false),
            row(2, "Blade Wall", 20, 4_000, false, true),
            row(3, "Quick Strike", 4, 300, true, false),
            row(4, "Heavy Blow", 12, 2_500_000, false, false),
        ],
        player_coin: Some(150_000),
        empty_text: "Nothing to train.".into(),
    }
}

fn trainer(ui: &mut Ui) -> SkillTrainerWindow {
    let (nv, p6) = vocabs();
    let root = fallback::skill_trainer(ui, &nv, &p6, &art(), &T).unwrap();
    SkillTrainerWindow::attach(ui, &nv, &p6, root, &art())
}

#[test]
fn skill_trainer_rows_and_train() {
    let mut ui = ui(1024, 768);
    let mut w = trainer(&mut ui);
    w.show(&mut ui, &trainer_view(), &T);
    assert_eq!(w.rows.len(), 4);
    let gp = ui.vocab().pids.button_ghosted;
    let train = w.train.unwrap();
    assert!(ui.prop(train, gp).bool());
    let e = button(&ui, train);
    assert_eq!(w.handle_event(&mut ui, &e), None);
    // Ghosted rows cannot be selected.
    let e = left(&ui, w.rows[2].0);
    w.handle_event(&mut ui, &e);
    assert_eq!(w.selected, None);
    let e = left(&ui, w.rows[1].0);
    w.handle_event(&mut ui, &e);
    assert_eq!(w.selected, None);
    assert_eq!(ui.state(w.rows[2].0), Some(ui.vocab().states.ghosted));
    // Select and train.
    let e = left(&ui, w.rows[0].0);
    assert_eq!(w.handle_event(&mut ui, &e), None);
    assert_eq!(w.selected, Some(0));
    assert!(!ui.prop(train, gp).bool());
    let e = button(&ui, train);
    assert_eq!(
        w.handle_event(&mut ui, &e),
        Some(ProgressionAction::TrainSkill {
            window: 42,
            entry: 1
        })
    );
    let e = double(&ui, w.rows[3].0);
    assert_eq!(
        w.handle_event(&mut ui, &e),
        Some(ProgressionAction::TrainSkill {
            window: 42,
            entry: 4
        })
    );
    // The selection survives a refresh of the same window, not a trained row.
    let mut v = trainer_view();
    v.rows[3].trained = true;
    w.show(&mut ui, &v, &T);
    assert_eq!(w.selected, None);
    let close = w.close.unwrap();
    let e = button(&ui, close);
    assert_eq!(
        w.handle_event(&mut ui, &e),
        Some(ProgressionAction::CloseTrainer { window: 42 })
    );
    // An empty list shows the empty text.
    let mut v = trainer_view();
    v.rows.clear();
    w.show(&mut ui, &v, &T);
    assert!(ui.is_visible(w.empty.unwrap()));
}

#[test]
fn skill_trainer_prices_too_expensive() {
    let mut ui = ui(1024, 768);
    let mut w = trainer(&mut ui);
    w.show(&mut ui, &trainer_view(), &T);
    let nv = NpcVocab::default();
    let price = |ui: &Ui, i: usize| part(ui, w.rows[i].0, own::TRAINER_ROW_PRICE);
    let states = |ui: &Ui, k: ElementKey| -> Vec<Option<u32>> {
        tessera_ui::game::subtree(ui, k)
            .into_iter()
            .filter(|e| ui.element_id(*e) == Some(nv.ids.coin_text) && ui.is_visible(*e))
            .map(|e| ui.state(e))
            .collect()
    };
    let cheap = states(&ui, price(&ui, 0));
    assert!(!cheap.is_empty());
    assert!(cheap.iter().all(|s| *s != Some(nv.states.too_expensive)));
    let dear = states(&ui, price(&ui, 3));
    assert!(dear.iter().all(|s| *s == Some(nv.states.too_expensive)));
}
