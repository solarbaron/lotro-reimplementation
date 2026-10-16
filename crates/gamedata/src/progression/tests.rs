// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the progression model over the in-memory install of the quest tests
//! (invented records, labels and texts).

use tessera_formats::gameplay::combat::TraitCell;
use tessera_formats::{Did, PropertyValue};

use super::*;
use crate::quest::tests::{Mem, arr, literal, si, st};

const TREE: Did = Did(0x7000_0100);
const A: Did = Did(0x7000_0201);
const B: Did = Did(0x7000_0202);
const C: Did = Did(0x7000_0203);
const SKILL: Did = Did(0x7000_0301);

fn cell(a: u32, b: u32) -> u32 {
    (a << 16) | b
}

fn install() -> Mem {
    let mut m = Mem::default();
    let nature = m.p("Trait_Nature");
    let branch = m.p("Trait_TraitTree_Branch");
    let ttype = m.p("Trait_TraitTree_TreeType");
    m.enum_texts.insert((nature, 3), "Class".into());
    m.enum_texts.insert((branch, 5), "Blue line".into());
    m.enum_texts.insert((branch, 6), "Red line".into());
    m.enum_texts.insert((ttype, 1), "Burglar".into());
    for (d, n) in [(A, "Quick"), (B, "Sly"), (C, "Bold")] {
        let e = vec![
            m.e("Trait_Name", si(literal(n))),
            m.e("Trait_Description", si(literal("does things"))),
            m.e("Trait_Nature", PropertyValue::Enum(3)),
            m.e("Trait_Tier", PropertyValue::Int(1)),
            m.e("Trait_Virtue_Maximum_Rank", PropertyValue::Int(5)),
            m.e(
                "Trait_PointBasedTrait_PointCostProgression",
                PropertyValue::DidRef(Did(0x7000_0E00)),
            ),
            m.e("Trait_Icon", PropertyValue::DidRef(Did(0x4100_0010))),
        ];
        m.put(d, e);
    }
    // A at (1,1) in branch 5; B at (2,1) needs A rank 2; C at (1,2) in branch 6 needs a
    // cell nobody occupies.
    let dep = |m: &mut Mem, k: u32, r: i32| {
        let f = vec![
            m.e("Trait_TraitTree_TraitDependency", PropertyValue::Enum(k)),
            m.e("Trait_TraitTree_TraitDependencyRank", PropertyValue::Int(r)),
        ];
        m.e("Trait_TraitTree_TraitDependency", st(f))
    };
    let place = |m: &mut Mem, t: Did, b: u32, k: u32, deps: Vec<_>| {
        let mut f = vec![
            m.e("Trait_TraitTree_Trait", PropertyValue::DidRef(t)),
            m.e("Trait_TraitTree_Branch", PropertyValue::Enum(b)),
            m.e("Trait_TraitTree_TraitLocation", PropertyValue::Enum(k)),
        ];
        if !deps.is_empty() {
            f.push(m.e("Trait_TraitTree_TraitDependencyArray", arr(deps)));
        }
        m.e("Trait_TraitTree_TraitStruct", st(f))
    };
    let d1 = dep(&mut m, cell(1, 1), 2);
    let d2 = dep(&mut m, cell(8, 12), 1);
    let p1 = place(&mut m, A, 5, cell(1, 1), vec![]);
    let p2 = place(&mut m, B, 5, cell(2, 1), vec![d1]);
    let p3 = place(&mut m, C, 6, cell(1, 2), vec![d2]);
    let tree = vec![
        m.e("Trait_TraitTree_TreeType", PropertyValue::Enum(1)),
        m.e("Trait_TraitTree_TraitArray", arr(vec![p1, p2, p3])),
    ];
    m.put(TREE, tree);
    let skill = vec![
        m.e("Skill_Name", si(literal("Strike"))),
        m.e("Skill_Icon", PropertyValue::DidRef(Did(0x4100_0020))),
    ];
    m.put(SKILL, skill);
    m
}

#[test]
fn level_up_rule_and_experience() {
    let mut s = ProgressionState::new();
    // The first level of a session is silent.
    assert_eq!(
        s.apply(&ProgressionEvent::Level {
            level: 9,
            quiet: false
        }),
        None
    );
    assert_eq!(
        s.apply(&ProgressionEvent::Level {
            level: 10,
            quiet: false
        }),
        Some(LevelUp { from: 9, to: 10 })
    );
    // Quiet and non-raising reports are silent.
    assert_eq!(
        s.apply(&ProgressionEvent::Level {
            level: 12,
            quiet: true
        }),
        None
    );
    assert_eq!(
        s.apply(&ProgressionEvent::Level {
            level: 11,
            quiet: false
        }),
        None
    );
    assert_eq!(s.level(), Some(11));
    s.apply(&ProgressionEvent::Experience {
        current: 250,
        needed: 1_000,
    });
    let bar = s.experience().expect("bar");
    assert!((bar.fraction - 0.25).abs() < 1e-6);
    assert_eq!(ExperienceBar::new(5, 0).fraction, 0.0);
    assert_eq!(ExperienceBar::new(u64::MAX, 1).fraction, 1.0);
}

#[test]
fn tree_view_cells_prerequisites_and_branches() {
    let m = install();
    let pp = ProgressionPresenter::new(&m);
    let mut s = ProgressionState::new();
    let tree = RecordRef::Record(TREE);
    let v = pp.tree_view(TREE, &s).expect("view");
    assert_eq!(v.type_text.as_deref(), Some("Burglar"));
    assert_eq!(v.cells.len(), 3);
    assert_eq!(v.cells[0].cell, Some(TraitCell::Grid { a: 1, b: 1 }));
    assert!(v.cells[0].available);
    assert_eq!(v.cells[1].prerequisites[0].placement, Some(0));
    assert_eq!(v.cells[1].prerequisites[0].rank, 2);
    assert!(!v.cells[1].available);
    assert_eq!(v.cells[2].prerequisites[0].placement, None);
    let blue = &v.branches[0];
    assert_eq!(
        (blue.key, blue.cells, blue.text.as_deref()),
        (5, 2, Some("Blue line"))
    );
    assert!(!v.active && !blue.chosen);
    let tip = &v.cells[0].tooltip;
    assert_eq!(tip.name.as_deref(), Some("Quick"));
    assert_eq!(tip.nature.as_deref(), Some("Class"));
    assert_eq!(
        (tip.max_rank, tip.rank, tip.point_based),
        (Some(5), 0, true)
    );

    // Feed ranks, points and the active tree.
    s.apply(&ProgressionEvent::TraitRank {
        tree: tree.clone(),
        what: RecordRef::Record(A),
        rank: 2,
    });
    s.apply(&ProgressionEvent::TraitPoints {
        available: 3,
        total: 10,
        tree: Some(tree.clone()),
    });
    s.apply(&ProgressionEvent::TraitTree {
        tree: Some(tree.clone()),
        branch: Some(5),
    });
    let v = pp.tree_view(TREE, &s).expect("view");
    assert!(v.cells[1].available && v.cells[1].prerequisites[0].met);
    assert_eq!(v.cells[0].tooltip.rank, 2);
    assert_eq!(v.points, Some((3, 10)));
    assert!(v.active && v.branches[0].chosen && !v.branches[1].chosen);
    assert!(pp.tree_view(Did(0x7000_0999), &s).is_none());
}

#[test]
fn trainer_rows_from_feed_entries() {
    let m = install();
    let pp = ProgressionPresenter::new(&m);
    let entries = vec![
        TrainerEntryIn {
            entry: 1,
            what: RecordRef::Record(SKILL),
            coin: 120,
            level: 5,
            rank: None,
            trained: false,
            unmet: false,
        },
        TrainerEntryIn {
            entry: 2,
            what: RecordRef::Literal("Kick".into()),
            coin: 0,
            level: 20,
            rank: Some(2),
            trained: false,
            unmet: false,
        },
        TrainerEntryIn {
            entry: 3,
            what: RecordRef::Record(A),
            coin: 7,
            level: 1,
            rank: None,
            trained: true,
            unmet: false,
        },
    ];
    let rows = pp.trainer_rows(TrainerOffer::Skills, &entries);
    assert_eq!(rows[0].name.as_deref(), Some("Strike"));
    assert_eq!(rows[0].icon, Some(Did(0x4100_0020)));
    assert!(!rows[0].unmet && !rows[0].ghosted);
    assert_eq!(rows[1].name.as_deref(), Some("Kick"));
    assert!(
        !rows[1].unmet,
        "only the feed's flag marks a row unmet, never the level"
    );
    assert!(rows[2].ghosted);
    let traits = pp.trainer_rows(TrainerOffer::Traits, &entries[2..]);
    assert_eq!(traits[0].name.as_deref(), Some("Quick"));
}

#[test]
fn state_bounds_and_hostile_values() {
    let mut s = ProgressionState::new();
    let t = RecordRef::Literal("T".into());
    for i in 0..10u16 {
        s.apply(&ProgressionEvent::TraitSlotted {
            slot: i,
            what: Some(RecordRef::Literal(format!("x{i}"))),
            rank: u16::MAX,
        });
    }
    s.apply(&ProgressionEvent::TraitSlotted {
        slot: 3,
        what: None,
        rank: 0,
    });
    assert_eq!(s.slots().len(), 9);
    s.apply(&ProgressionEvent::SkillLearned(t.clone()));
    s.apply(&ProgressionEvent::SkillLearned(t.clone()));
    assert_eq!(s.learned().len(), 1);
    s.apply(&ProgressionEvent::SkillForgotten(t.clone()));
    assert!(s.learned().is_empty());
    s.apply(&ProgressionEvent::TraitTree {
        tree: None,
        branch: Some(4),
    });
    assert_eq!(s.active_tree(), (None, None));
    assert_eq!(s.rank(&t, &t), 0);
    assert_eq!(s.points(None), None);
    // A tree record of the wrong shape gives a page with no cells, never a panic.
    let mut m = install();
    let bad = vec![m.e("Trait_TraitTree_TraitArray", PropertyValue::Int(4))];
    m.put(Did(0x7000_0777), bad);
    let pp = ProgressionPresenter::new(&m);
    let v = pp.tree_view(Did(0x7000_0777), &s).expect("view");
    assert!(v.cells.is_empty() && v.branches.is_empty());
}

/// Builds a dependency struct on `k` with `rank` (`None`: no rank field).
fn dependency(m: &mut Mem, k: u32, rank: Option<i32>) -> tessera_formats::property::PropertyEntry {
    let mut f = vec![m.e("Trait_TraitTree_TraitDependency", PropertyValue::Enum(k))];
    if let Some(r) = rank {
        f.push(m.e("Trait_TraitTree_TraitDependencyRank", PropertyValue::Int(r)));
    }
    m.e("Trait_TraitTree_TraitDependency", st(f))
}

/// Builds a placement of `t` in `branch` at `k` (`None`: no location).
fn placement(
    m: &mut Mem,
    t: Did,
    branch: u32,
    k: Option<u32>,
    deps: Vec<tessera_formats::property::PropertyEntry>,
) -> tessera_formats::property::PropertyEntry {
    let mut f = vec![
        m.e("Trait_TraitTree_Trait", PropertyValue::DidRef(t)),
        m.e("Trait_TraitTree_Branch", PropertyValue::Enum(branch)),
    ];
    if let Some(k) = k {
        f.push(m.e("Trait_TraitTree_TraitLocation", PropertyValue::Enum(k)));
    }
    if !deps.is_empty() {
        f.push(m.e("Trait_TraitTree_TraitDependencyArray", arr(deps)));
    }
    m.e("Trait_TraitTree_TraitStruct", st(f))
}

fn put_tree(m: &mut Mem, did: Did, placements: Vec<tessera_formats::property::PropertyEntry>) {
    let e = vec![
        m.e("Trait_TraitTree_TreeType", PropertyValue::Enum(1)),
        m.e("Trait_TraitTree_TraitArray", arr(placements)),
    ];
    m.put(did, e);
}

/// Regression: cell keys repeat across branches; a dependency names the placement of its
/// own branch at that cell, else the tree's only placement there, else none.
#[test]
fn dependencies_are_branch_scoped() {
    const SHARED: Did = Did(0x7000_0110);
    let mut m = install();
    // Branch 5: A at (1,1). Branch 6: B at (1,1), C at (2,1) needs (1,1) rank 3.
    // Branch 7: A at (3,3) needs (1,1) (two other branches hold it: ambiguous) and (2,1)
    // (only branch 6 holds it: unique).
    let d_own = dependency(&mut m, cell(1, 1), Some(3));
    let d_amb = dependency(&mut m, cell(1, 1), None);
    let d_uni = dependency(&mut m, cell(2, 1), None);
    let p = vec![
        placement(&mut m, A, 5, Some(cell(1, 1)), vec![]),
        placement(&mut m, B, 6, Some(cell(1, 1)), vec![]),
        placement(&mut m, C, 6, Some(cell(2, 1)), vec![d_own]),
        placement(&mut m, A, 7, Some(cell(3, 3)), vec![d_amb, d_uni]),
    ];
    put_tree(&mut m, SHARED, p);
    let pp = ProgressionPresenter::new(&m);
    let mut s = ProgressionState::new();
    let tree = RecordRef::Record(SHARED);
    // The feed ranks B (branch 6) at 3 and A (branch 5) at 0.
    s.apply(&ProgressionEvent::TraitRank {
        tree: tree.clone(),
        what: RecordRef::Record(B),
        rank: 3,
    });
    let v = pp.tree_view(SHARED, &s).expect("view");
    let own = &v.cells[2].prerequisites[0];
    assert_eq!(
        own.placement,
        Some(1),
        "same branch, not the first tree-wide"
    );
    assert_eq!(own.rank, 3);
    assert!(own.met && v.cells[2].available);
    let amb = &v.cells[3].prerequisites[0];
    assert_eq!(amb.placement, None, "ambiguous across other branches");
    assert!(!amb.met);
    assert_eq!(
        v.cells[3].prerequisites[1].placement,
        Some(2),
        "unique cell"
    );
    // The census uses the same rule.
    let c = census::census_of(&m, [SHARED], &|_| true, &[]);
    assert_eq!(
        c.listed_placements,
        vec![(SHARED, 3, PlacementCategory::DependencyMissing)]
    );
    assert_eq!(c.failure_count, 0);
}

/// Regression: a stored rank out of 1 … 65,535 is clamped, never read as the absent rank.
#[test]
fn dependency_ranks_are_clamped() {
    use super::present::dependency_rank;
    assert_eq!(dependency_rank(None), 1);
    assert_eq!(dependency_rank(Some(3)), 3);
    assert_eq!(dependency_rank(Some(70_000)), u16::MAX);
    assert_eq!(dependency_rank(Some(i64::MAX)), u16::MAX);
    assert_eq!(dependency_rank(Some(0)), 1);
    assert_eq!(dependency_rank(Some(-5)), 1);
    // Through a tree: a huge rank is not met at rank 1.
    const HUGE: Did = Did(0x7000_0111);
    let mut m = install();
    let d = dependency(&mut m, cell(1, 1), Some(70_000));
    let p = vec![
        placement(&mut m, A, 5, Some(cell(1, 1)), vec![]),
        placement(&mut m, B, 5, Some(cell(2, 1)), vec![d]),
    ];
    put_tree(&mut m, HUGE, p);
    let pp = ProgressionPresenter::new(&m);
    let mut s = ProgressionState::new();
    s.apply(&ProgressionEvent::TraitRank {
        tree: RecordRef::Record(HUGE),
        what: RecordRef::Record(A),
        rank: 1,
    });
    let v = pp.tree_view(HUGE, &s).expect("view");
    assert_eq!(v.cells[1].prerequisites[0].rank, u16::MAX);
    assert!(!v.cells[1].available);
}

/// The census over synthetic trees: every placement and tree category, counts, listings
/// and no failure.
#[test]
fn census_categories() {
    const NAMELESS: Did = Did(0x7000_0204);
    const NOT_TRAIT: Did = Did(0x7000_0205);
    const ABSENT: Did = Did(0x7000_0120);
    const EMPTY: Did = Did(0x7000_0121);
    const MIXED: Did = Did(0x7000_0122);
    const BIG: Did = Did(0x7000_0123);
    let mut m = install();
    let e = vec![m.e("Trait_Tier", PropertyValue::Int(1))];
    m.put(NAMELESS, e);
    let e = vec![m.e("Trait_Name", si(literal("Stray")))];
    m.put(NOT_TRAIT, e);
    put_tree(&mut m, EMPTY, vec![]);
    let dangling = dependency(&mut m, cell(8, 12), None);
    let p = vec![
        placement(&mut m, A, 5, Some(cell(1, 1)), vec![]),
        placement(&mut m, NOT_TRAIT, 5, Some(cell(1, 2)), vec![]),
        placement(&mut m, NAMELESS, 5, Some(cell(1, 3)), vec![]),
        placement(&mut m, B, 5, None, vec![]),
        placement(&mut m, B, 5, Some(0), vec![]),
        placement(&mut m, C, 5, Some(cell(2, 2)), vec![dangling]),
    ];
    put_tree(&mut m, MIXED, p);
    let big: Vec<_> = (0..=MAX_CELLS)
        .map(|_| placement(&mut m, A, 5, Some(cell(1, 1)), vec![]))
        .collect();
    put_tree(&mut m, BIG, big);
    let is_trait = |d: Did| [A, B, C, NAMELESS].contains(&d);
    let c = census::census_of(
        &m,
        [TREE, ABSENT, EMPTY, MIXED, BIG],
        &is_trait,
        &[A, B, C, NAMELESS],
    );
    let count = |k: TreeCategory| c.trees.get(&k).copied().unwrap_or(0);
    assert_eq!(c.tree_total(), 5);
    assert_eq!(count(TreeCategory::Unreadable), 1);
    assert_eq!(count(TreeCategory::Empty), 1);
    assert_eq!(count(TreeCategory::TraitMissing), 1, "MIXED");
    assert_eq!(count(TreeCategory::TooManyCells), 1, "BIG");
    // TREE: its third placement depends on a cell nobody holds.
    assert_eq!(count(TreeCategory::DependencyMissing), 1);
    let placements = |k: PlacementCategory| c.placements.get(&k).copied().unwrap_or(0);
    assert_eq!(c.placement_total(), 3 + 6 + MAX_CELLS + 1);
    assert_eq!(placements(PlacementCategory::TooManyCells), 1);
    assert_eq!(placements(PlacementCategory::TraitMissing), 1);
    assert_eq!(placements(PlacementCategory::NameMissing), 1);
    assert_eq!(placements(PlacementCategory::CellMissing), 2);
    assert_eq!(placements(PlacementCategory::DependencyMissing), 2);
    assert_eq!(
        placements(PlacementCategory::Shown),
        2 + 1 + MAX_CELLS,
        "TREE's A and B, MIXED's A, BIG's first MAX_CELLS"
    );
    assert!(
        c.listed_placements
            .contains(&(BIG, MAX_CELLS, PlacementCategory::TooManyCells))
    );
    assert!(
        c.listed_placements
            .contains(&(MIXED, 2, PlacementCategory::NameMissing))
    );
    assert_eq!(c.listed_trees.len(), 5);
    assert_eq!(c.tooltips, c.placement_total() - 1);
    assert_eq!(c.failure_count, 0);
    assert_eq!(c.distinct_traits, 5);
    assert_eq!(c.unplaced_point_based, 0);
}

/// Learning and forgetting at the state's bounds stays fast (no linear scan per event).
#[test]
fn learned_skills_bulk() {
    let mut s = ProgressionState::new();
    let started = std::time::Instant::now();
    for i in 0..=MAX_SKILLS as u32 {
        s.apply(&ProgressionEvent::SkillLearned(RecordRef::Record(Did(i))));
    }
    s.apply(&ProgressionEvent::SkillLearned(RecordRef::Record(Did(5))));
    assert_eq!(s.learned().len(), MAX_SKILLS);
    s.apply(&ProgressionEvent::SkillForgotten(RecordRef::Record(Did(5))));
    s.apply(&ProgressionEvent::SkillForgotten(RecordRef::Record(Did(5))));
    assert_eq!(s.learned().len(), MAX_SKILLS - 1);
    assert_eq!(
        s.learned()[5],
        RecordRef::Record(Did(6)),
        "learn order kept"
    );
    s.apply(&ProgressionEvent::SkillLearned(RecordRef::Record(Did(5))));
    assert_eq!(s.learned().last(), Some(&RecordRef::Record(Did(5))));
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
}
