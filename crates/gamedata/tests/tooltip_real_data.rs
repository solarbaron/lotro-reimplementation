// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the item examination tooltip model (`item-tooltips.md` §12, decision
//! 0024 P1 exit criterion 2) against a user's own install, read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Builds the
//! hover tooltip model of every item record and sorts each record into the categories of
//! §12; checks IV1–IV5 over the tooltip elements item records name. Prints counts and
//! layout identifiers only, never game text. Values are read from the install at run time;
//! nothing here names a record. Use `--release`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use tessera_formats::PropertyValue;
use tessera_gamedata::tooltip::{
    Category, Census, InstallTooltipData, Purpose, RowTemplate, TemplateRef, TextRef, TooltipData,
    TooltipEngine, Viewer, Widget,
};
use tessera_gamedata::{Family, GameDataStore};
use tessera_text::Strings;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn has_name_widget(t: &RowTemplate) -> bool {
    t.widget == Widget::Name || t.children.iter().flatten().any(|c| has_name_widget(c))
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 8)
}

#[test]
fn every_item_record_gets_a_tooltip_model() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the tooltip real-data test");
        return;
    };
    let t0 = Instant::now();
    let store = GameDataStore::open(&dir).expect("install opens");
    let strings = Arc::new(Strings::open_install(&dir, None).expect("strings open"));
    let data = InstallTooltipData::new(&store, strings).expect("tooltip data");
    let engine = TooltipEngine::new(data);
    let items = store.family(Family::Item).expect("item family");
    eprintln!("{} item records ({:?} to index)", items.len(), t0.elapsed());

    // §12: every record in exactly one category.
    let t1 = Instant::now();
    let next = AtomicUsize::new(0);
    let viewer = Viewer::default();
    let census = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads())
            .map(|_| {
                s.spawn(|| {
                    let mut c = Census::default();
                    let mut unnamed = Vec::new();
                    loop {
                        let i = next.fetch_add(64, Ordering::Relaxed);
                        if i >= items.len() {
                            break;
                        }
                        for &did in &items[i..(i + 64).min(items.len())] {
                            let o = engine.classify(did, &viewer);
                            if o.category == Ok(Category::Rendered) && !o.named {
                                unnamed.push(did);
                            }
                            c.add(did, &o);
                        }
                    }
                    (c, unnamed)
                })
            })
            .collect();
        let mut all = Census::default();
        let mut unnamed = Vec::new();
        for w in workers {
            let (c, u) = w.join().expect("worker");
            all.merge(c);
            unnamed.extend(u);
        }
        (all, unnamed)
    });
    let (census, unnamed) = census;
    eprintln!("classified in {:?}", t1.elapsed());
    for (k, v) in &census.categories {
        eprintln!("  {k:?}: {v}");
    }
    eprintln!(
        "  escaped: {}, unnamed: {}, deferred rows: {}",
        census.escaped, census.unnamed, census.deferred
    );
    for (label, n) in &census.non_empty {
        eprintln!("  non-empty {label}: {n}");
    }
    for e in &census.examples {
        eprintln!("  example: {e}");
    }

    // The expected number of rendered tooltips: records whose hover pair names a
    // top-level element of its layout (IV1), counted independently of the engine's build.
    let mut expected = 0u64;
    let mut pairs: BTreeSet<TemplateRef> = BTreeSet::new();
    let mut compare_pairs: BTreeSet<TemplateRef> = BTreeSet::new();
    let data = engine.data();
    for &did in &items {
        let Ok(Some(set)) = data.record(did) else {
            continue;
        };
        for purpose in [Purpose::Hover, Purpose::Comparison] {
            if let Some(p) = engine.pair(&set, purpose) {
                pairs.insert(p);
                if purpose == Purpose::Comparison {
                    compare_pairs.insert(p);
                }
            }
        }
        // One-click pair (Tessera uses the hover pair; IV2 still covers its elements).
        if let (Some(lp), Some(ep)) = (
            data.pid("UI_Examination_Tooltip_1Click_Layout"),
            data.pid("UI_Examination_Tooltip_1Click_Element"),
        ) && let (Some(PropertyValue::DidRef(l)), Some(PropertyValue::Enum(e))) =
            (set.get(lp), set.get(ep))
            && !l.is_null()
            && *e != 0
        {
            pairs.insert(TemplateRef {
                layout: *l,
                element: *e,
            });
        }
        if let Some(p) = engine.pair(&set, Purpose::Hover)
            && data
                .layout(p.layout)
                .ok()
                .flatten()
                .is_some_and(|l| l.top_level(p.element).is_some())
        {
            expected += 1;
        }
    }

    // `skill-tooltips.md` §9.8 (ST24): the skill levels of skill items. Counts only.
    let (mut naming, mut stored, mut calculators, mut level_one) = (0u64, 0u64, 0u64, 0u64);
    let mut distinct = BTreeSet::new();
    for &did in &items {
        let Ok(Some(set)) = data.record(did) else {
            continue;
        };
        let l = engine.skill_item_level(&set);
        if l.calculator.is_some() {
            calculators += 1;
        }
        if l.skill.is_none() {
            continue;
        }
        naming += 1;
        if let Some(v) = l.stored {
            stored += 1;
            distinct.insert(v);
            if v == 1 {
                level_one += 1;
            }
        }
    }
    eprintln!(
        "skill items: {naming} name a skill, {stored} store Usage_SkillLevel ({} distinct, \
         {level_one} of them 1, range {:?}..={:?}); {calculators} name a SkillLevelCalculator \
         (not evaluated, Tessera rule)",
        distinct.len(),
        distinct.first(),
        distinct.last()
    );
    assert!(stored <= naming);

    // IV2–IV4 over the tooltip elements item records name.
    let mut iv2 = 0;
    let mut iv3 = (0, 0);
    let mut iv4 = (0, 0);
    let mut bad: BTreeMap<String, String> = BTreeMap::new();
    for &p in &pairs {
        let name = data
            .ui_label(tessera_formats::ui_layout::UiEnum::ElementId, p.element)
            .unwrap_or_else(|| format!("{:#010x}", p.element));
        match engine.frame(p) {
            Ok(f) => {
                let ok = f.base_label.as_deref() == Some("Tooltip_New")
                    && f.children == 1
                    && f.list.autofill;
                if ok {
                    iv2 += 1;
                } else {
                    bad.insert(
                        name.clone(),
                        format!("IV2: {f:?}").chars().take(200).collect(),
                    );
                }
                for t in f.list.templates.iter().flatten() {
                    iv3.1 += 1;
                    if data
                        .layout(t.layout)
                        .ok()
                        .flatten()
                        .is_some_and(|l| l.top_level(t.element).is_some())
                    {
                        iv3.0 += 1;
                    }
                }
                if name.ends_with("Compare_ExamineLayout") {
                    iv4.1 += 1;
                    let first = f.list.templates.first().copied().flatten().and_then(|t| {
                        data.ui_label(tessera_formats::ui_layout::UiEnum::ElementId, t.element)
                    });
                    if first.as_deref() == Some("CurrentlyEquipped_Label") {
                        iv4.0 += 1;
                    }
                }
            }
            Err(e) => {
                bad.insert(name, e.to_string());
            }
        }
    }
    eprintln!(
        "tooltip elements: {} ({} compare pairs); IV2 {iv2}; IV3 {}/{}; IV4 {}/{}",
        pairs.len(),
        compare_pairs.len(),
        iv3.0,
        iv3.1,
        iv4.0,
        iv4.1
    );
    for (k, v) in &bad {
        eprintln!("  element {k}: {v}");
    }

    // IV5: the colour table holds the five item-quality states and the object state.
    let colours = [
        "ItemQuality_Legendary_ColorName",
        "ItemQuality_Rare_ColorName",
        "ItemQuality_Incomparable_ColorName",
        "ItemQuality_Uncommon_ColorName",
        "ItemQuality_Common_ColorName",
        "Object_ColorName",
    ];
    for state in colours {
        assert!(
            engine.name_colour(state).is_some(),
            "IV5: no colour for {state}"
        );
    }

    assert_eq!(census.escaped, 0, "errors escaped the categories");
    assert_eq!(census.count(Category::LayoutUnreadable), 0);
    assert_eq!(census.count(Category::ElementMissing), 0);
    assert_eq!(census.count(Category::RowFailure), 0, "row failures");
    // A rendered tooltip without a name is allowed only when its layout has no name row
    // or the item's name renders empty (§12 OPEN).
    let name_pid = data.pid("Name");
    let mut no_name_row = 0;
    let mut empty_name = 0;
    let mut missing = Vec::new();
    for &did in &unnamed {
        let set = data.record(did).ok().flatten().expect("record");
        let frame = engine
            .pair(&set, Purpose::Hover)
            .and_then(|p| engine.frame(p).ok())
            .expect("frame");
        let has_row = frame
            .list
            .templates
            .iter()
            .flatten()
            .filter_map(|t| engine.template(*t).ok())
            .any(|t| has_name_widget(&t));
        let renders = match name_pid.and_then(|p| set.get(p)) {
            Some(PropertyValue::StringInfo(i)) => matches!(
                data.render(TextRef::Info(i), &tessera_text::Values::new()),
                Ok(Some(_))
            ),
            _ => false,
        };
        if !has_row {
            no_name_row += 1;
        } else if !renders {
            empty_name += 1;
        } else {
            missing.push(did);
        }
    }
    eprintln!(
        "unnamed: {} ({no_name_row} layouts without a name row, {empty_name} empty names)",
        unnamed.len()
    );
    assert!(
        missing.is_empty(),
        "name rows left empty: {:?}",
        &missing[..missing.len().min(8)]
    );
    assert_eq!(
        census.count(Category::Rendered),
        expected,
        "Rendered differs from the records with a valid hover pair"
    );
    assert_eq!(
        census.count(Category::Rendered) + census.count(Category::NoTooltip),
        items.len() as u64
    );
    assert!(bad.is_empty(), "tooltip elements break IV2");
    assert_eq!(iv3.0, iv3.1, "IV3");
    assert_eq!(iv4.0, iv4.1, "IV4");
}
