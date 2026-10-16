// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the crafting model (`crafting-panels.md` §10; decision 0037 §1) against
//! a user's own install, read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Runs the
//! census over every recipe, profession, vocation, guild, crafting trainer, gathering node,
//! facility and recipe scroll: every record in exactly one category, no failure; for every
//! recipe the tree rows and the recipe page of each of its tiers are built, and the recipe
//! rows of every scroll that names it. Counts that differ from the spec's (this install,
//! 2026-10-16) are printed as warnings, never failures (§10.4 Tessera rule). Prints counts
//! and identifiers only, never game text. Use `--release`.

use std::path::PathBuf;
use std::sync::Arc;

use tessera_gamedata::crafting::{CraftingPresenter, RecipeCategory, craft_census};
use tessera_gamedata::quest::InstallPresentationData;
use tessera_gamedata::tooltip::{
    InstallTooltipData, Row, RowContent, TooltipEngine, TooltipRequest, Viewer, Widget,
};
use tessera_gamedata::{Family, GameDataStore};
use tessera_text::Strings;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

/// Whether a row of `widget` (at any depth) is in the tree and whether it shows text.
fn find(rows: &[Row], widget: &Widget) -> (bool, bool) {
    let mut present = false;
    let mut shown = false;
    for r in rows {
        if &r.widget == widget {
            present = true;
            shown |= !r.is_empty();
        }
        if let RowContent::List(children) = &r.content {
            let (p, s) = find(children, widget);
            present |= p;
            shown |= s;
        }
    }
    (present, shown)
}

fn expect(what: &str, got: usize, spec: usize) {
    if got != spec {
        eprintln!("  WARNING (data change?): {what}: {got}, the spec counts {spec}");
    }
}

#[test]
fn crafting_census_over_the_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the crafting real-data test");
        return;
    };
    let started = std::time::Instant::now();
    let store = GameDataStore::open(&dir).expect("install opens");
    let strings = Arc::new(Strings::open_install(&dir, None).expect("strings open"));
    let data = InstallPresentationData::new(&store, strings).expect("data");
    let c = craft_census(&store, &data).expect("census runs");
    eprintln!("crafting census ({:?}):", started.elapsed());
    eprintln!("  recipes {}: {:?}", c.recipes.total(), c.recipes.counts);
    for (did, cat) in c.recipes.listed.iter().take(24) {
        eprintln!("    {did} {cat:?}");
    }
    let f = &c.flags;
    eprintln!(
        "  one-use {}, cooldown {}, multi-output {}, critical {}, optional {}, pack {}, \
         guild {}, non-item result {} ({} with icon), rows {}, categories {} ({} shared)",
        f.one_use,
        f.cooldown,
        f.multi_output,
        f.critical,
        f.optional,
        f.pack,
        f.guild,
        f.non_item_result,
        f.non_item_with_icon,
        f.rows,
        f.categories,
        f.shared_categories
    );
    let mut per_prof: Vec<usize> = f.per_profession.values().copied().collect();
    per_prof.sort_unstable_by(|a, b| b.cmp(a));
    eprintln!("  per profession {per_prof:?}");
    eprintln!("  per tier {:?}", f.per_tier.values().collect::<Vec<_>>());
    eprintln!("  professions {:?}", c.professions.counts);
    eprintln!("  vocations {:?}", c.vocations.counts);
    eprintln!("  guilds {:?}", c.guilds.counts);
    eprintln!("  trainers {:?}", c.trainers.counts);
    eprintln!("  gathering nodes {:?}", c.nodes.counts);
    eprintln!("  facility effects {:?}", c.facilities.counts);
    eprintln!("  facility hotspots {:?}", c.hotspots.counts);
    eprintln!("  recipe scrolls {:?}", c.scrolls.counts);
    eprintln!(
        "  failures {}, unreadable records {}",
        c.failure_count, c.unreadable
    );
    for (did, why) in c.failures.iter().take(24) {
        eprintln!("    {did}: {why}");
    }

    // Spec counts (§10, 2026-10-16): warnings only.
    expect("recipes", c.recipes.total(), 7_791);
    expect(
        "Shown recipes",
        c.recipes.count(RecipeCategory::Shown),
        7_782,
    );
    expect(
        "ResultImageMissing",
        c.recipes.count(RecipeCategory::ResultImageMissing),
        9,
    );
    expect("one-use", f.one_use, 1_339);
    expect("cooldown", f.cooldown, 2_262);
    expect("multi-output", f.multi_output, 2_414);
    expect("critical", f.critical, 4_461);
    expect("optional", f.optional, 4_012);
    expect("pack", f.pack, 4_520);
    expect("guild", f.guild, 2_184);
    expect("non-item result", f.non_item_result, 79);
    expect("categories", f.categories, 491);
    expect("professions", c.professions.total(), 10);
    expect("vocations", c.vocations.total(), 7);
    expect("guilds", c.guilds.total(), 7);
    expect("trainers", c.trainers.total(), 32);
    expect("gathering nodes", c.nodes.total(), 335);
    expect("facility effects", c.facilities.total(), 13);
    expect("facility hotspots", c.hotspots.total(), 26);
    expect("recipe scrolls", c.scrolls.total(), 6_055);

    // The recipe rows of every scroll's tooltip (§7): laid out by the item tooltip model,
    // shown exactly when the recipe is one-use or has a cooldown.
    let tip_strings = Arc::new(Strings::open_install(&dir, None).expect("strings open"));
    let engine = TooltipEngine::new(InstallTooltipData::new(&store, tip_strings).expect("tips"));
    let presenter = CraftingPresenter::new(&data);
    let viewer = Viewer::default();
    let (mut one_shot, mut cooldown, mut missing_rows, mut wrong) =
        (0usize, 0usize, 0usize, 0usize);
    for &(scroll, _) in &c.scroll_list {
        let set = store
            .object(scroll)
            .expect("scroll")
            .expect("present")
            .properties;
        let rows = presenter.scroll_rows(&set).expect("scroll rows");
        let tip = engine
            .build(&TooltipRequest::hover(scroll), &viewer)
            .expect("scroll tooltip");
        let (p1, s1) = find(&tip.rows, &Widget::RecipeOneShot);
        let (p2, s2) = find(&tip.rows, &Widget::RecipeCooldown);
        missing_rows += usize::from(!p1 || !p2);
        one_shot += usize::from(s1);
        cooldown += usize::from(s2);
        wrong += usize::from((p1 && s1 != rows.expected.0) || (p2 && s2 != rows.expected.1));
    }
    eprintln!(
        "  scroll tooltips {}: one-use rows {one_shot}, cooldown rows {cooldown}, \
         layouts without the rows {missing_rows}, rows that disagree {wrong}",
        c.scroll_list.len()
    );
    expect("one-use scroll rows", one_shot, 1_339);
    expect("cooldown scroll rows", cooldown, 2_238);
    assert_eq!(wrong, 0, "scroll recipe rows follow §7");

    // The rules (decision 0037 §1 item 4): every recipe exactly once, no failure.
    let recipes = store.family(Family::Recipe).expect("recipe family");
    assert_eq!(
        c.recipes.total(),
        recipes.len(),
        "every recipe record counted exactly once"
    );
    // CI9 for the selector families too: each record exactly once.
    for (family, total, what) in [
        (Family::Profession, c.professions.total(), "profession"),
        (Family::Vocation, c.vocations.total(), "vocation"),
        (Family::CraftGuild, c.guilds.total(), "guild"),
    ] {
        let n = store.family(family).expect("family").len();
        assert_eq!(total, n, "every {what} record counted exactly once");
    }
    assert_eq!(c.failure_count, 0, "no recipe leaves a part unbuilt");
    assert!(c.recipes.count(RecipeCategory::Shown) > 0);
}
