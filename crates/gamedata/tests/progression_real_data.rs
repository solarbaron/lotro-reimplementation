// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the progression model (decision 0037 §1 item 1, trait half; the
//! provisional census of `tessera_gamedata::progression`) against a user's own install,
//! read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Visits every
//! trait tree once and every placement once with its tree, builds each tree page and the
//! tooltip of every placed trait, and checks that every tree and placement lands in exactly
//! one category with no failure. Counts that differ from `combat-data.md` §8.5 (17 trees,
//! 1,216 placements) are printed as warnings. Prints counts and identifiers only, never game
//! text. Use `--release`.

use std::path::PathBuf;
use std::sync::Arc;

use tessera_gamedata::progression::{
    ProgressionPresenter, ProgressionState, TreeCategory, trait_tree_census,
};
use tessera_gamedata::quest::InstallPresentationData;
use tessera_gamedata::{Family, GameDataStore};
use tessera_text::Strings;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

#[test]
fn trait_tree_census_over_the_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the progression real-data test");
        return;
    };
    let started = std::time::Instant::now();
    let store = GameDataStore::open(&dir).expect("install opens");
    let strings = Arc::new(Strings::open_install(&dir, None).expect("strings open"));
    let data = InstallPresentationData::new(&store, strings).expect("data");
    let c = trait_tree_census(&store, &data).expect("census runs");
    eprintln!("trait-tree census ({:?}):", started.elapsed());
    eprintln!("  trees {}: {:?}", c.tree_total(), c.trees);
    eprintln!("  placements {}: {:?}", c.placement_total(), c.placements);
    for (did, cat) in c.listed_trees.iter().take(24) {
        eprintln!("    tree {did} {cat:?}");
    }
    for (did, i, cat) in c.listed_placements.iter().take(24) {
        eprintln!("    placement {did}[{i}] {cat:?}");
    }
    eprintln!(
        "  distinct traits {}, branches {}, tooltips {}, unplaced point-based traits {}, \
         failures {}",
        c.distinct_traits, c.branches, c.tooltips, c.unplaced_point_based, c.failure_count
    );
    for (did, i, why) in c.failures.iter().take(24) {
        eprintln!("    {did}[{i}]: {why}");
    }
    if c.tree_total() != 17 {
        eprintln!(
            "  WARNING (data change?): {} trees, combat-data.md §8.5 counts 17",
            c.tree_total()
        );
    }
    if c.placement_total() != 1_216 {
        eprintln!(
            "  WARNING (data change?): {} placements, combat-data.md §8.5 counts 1,216",
            c.placement_total()
        );
    }
    let trees = store.family(Family::TraitTree).expect("trait-tree family");
    assert_eq!(c.tree_total(), trees.len(), "every tree counted once");
    assert_eq!(
        c.tooltips,
        c.placement_total(),
        "every placement's tooltip built"
    );
    assert_eq!(c.failure_count, 0, "no placement left unbuilt");
    assert_eq!(
        c.trees.get(&TreeCategory::Unreadable).copied().unwrap_or(0),
        0
    );

    // Prerequisites are branch-scoped (`combat-data.md` §8.5 OPEN): whenever the dependent
    // placement's branch holds the cell, the prerequisite names that placement.
    let pp = ProgressionPresenter::new(&data);
    let state = ProgressionState::new();
    let (mut deps, mut same, mut cross, mut wrong) = (0usize, 0usize, 0usize, 0usize);
    let (mut shared_trees, mut shared_keys) = (0usize, 0usize);
    for did in store.index().expect("index").dids(Family::TraitTree) {
        let view = pp.tree_view(did, &state).expect("tree page");
        let mut at: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
        for k in view.cells.iter().filter_map(|c| c.cell) {
            *at.entry(k.key()).or_default() += 1;
        }
        let n = at.values().filter(|&&n| n > 1).count();
        shared_keys += n;
        shared_trees += usize::from(n > 0);
        for cell in &view.cells {
            for p in &cell.prerequisites {
                deps += 1;
                let own = view.cells.iter().position(|o| {
                    o.branch == cell.branch && o.cell.map(|k| k.key()) == Some(p.cell)
                });
                match (own, p.placement) {
                    (Some(o), Some(got)) if o == got => same += 1,
                    (None, Some(_)) => cross += 1,
                    _ => wrong += 1,
                }
            }
        }
    }
    eprintln!(
        "  prerequisites {deps}: same branch {same}, other branch (unique cell) {cross}, \
         wrong {wrong}; cell keys held by several placements: {shared_keys} in \
         {shared_trees} trees"
    );
    assert_eq!(wrong, 0, "a prerequisite names another branch's placement");
}
