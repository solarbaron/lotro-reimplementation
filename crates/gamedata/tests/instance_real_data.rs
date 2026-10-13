// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the instance-finder model (`travel-windows.md` §6.3, §6.8, §8.2;
//! `instances.md` §15.1 B1–B3) against a user's own install, read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Runs the
//! template census over every private-encounter template (every template in exactly one
//! category), builds the finder tree over every finder template, resolves every instance
//! group for each of its sizes (B3) and the featured instance for each driver value (B1).
//! Prints counts and identifiers only, never game text. Use `--release`.

use std::path::PathBuf;
use std::sync::Arc;

use tessera_formats::property::PropertyValue;
use tessera_gamedata::GameDataStore;
use tessera_gamedata::instance::{
    FinderEntryIn, FinderPresenter, RowSubject, TemplateCategory, template_census,
};
use tessera_gamedata::quest::{InstallPresentationData, RecordRef};
use tessera_text::Strings;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

#[test]
fn finder_and_template_census_over_the_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the instance real-data test");
        return;
    };
    let started = std::time::Instant::now();
    let store = GameDataStore::open(&dir).expect("install opens");
    let strings = Arc::new(Strings::open_install(&dir, None).expect("strings open"));
    let data = InstallPresentationData::new(&store, strings).expect("data");
    let fp = FinderPresenter::new(&data);
    let control = fp.control().expect("world-join control").clone();
    eprintln!(
        "finder: {} available, {} groups, {} featured, index entries {}, kind conflicts {}",
        control.available.len(),
        control.groups.len(),
        control.featured.len(),
        fp.matching_index().entries.len(),
        fp.matching_index().kind_conflicts.len()
    );
    let c = template_census(&store, &fp).expect("census");
    eprintln!("template census ({:?}):", started.elapsed());
    for (cat, n) in &c.categories {
        eprintln!("  {cat:?}: {n}");
    }
    eprintln!(
        "  featured {}, groups {}, travel holders {} ({} references, {} empty arrays), \
         in two lists {}, names (table, literal, none) {:?}, level scaling {}, unreadable records {}",
        c.featured,
        c.groups,
        c.travel_holders,
        c.travel_references,
        c.empty_travel_arrays,
        c.in_two_lists,
        c.names,
        c.level_scaling,
        c.unreadable_records
    );
    eprintln!("  finder kinds {:?}", c.finder_kinds);
    eprintln!("  unlisted {:?}", c.listed);
    assert_eq!(c.count(TemplateCategory::RecordUnreadable), 0);
    assert_eq!(c.in_two_lists, 0, "no template in two lists");
    assert!(c.count(TemplateCategory::FinderRow) > 0);

    // The finder over every listed template (§6.8).
    let order = control.finder_order();
    let entries: Vec<FinderEntryIn> = order
        .iter()
        .enumerate()
        .take(1_024)
        .map(|(i, &d)| FinderEntryIn {
            entry: u16::try_from(i + 1).unwrap_or(u16::MAX),
            instance: RecordRef::Record(d),
            level: None,
            size: None,
            tier: None,
            locked: i % 5 == 0,
        })
        .collect();
    let featured = control
        .featured
        .first()
        .copied()
        .flatten()
        .map(RecordRef::Record);
    let view = fp.view(&entries, featured.as_ref(), 60);
    let groups = view
        .rows
        .iter()
        .filter(|r| matches!(r.subject, RowSubject::Group { .. }))
        .count();
    let texts = view.rows.iter().filter(|r| r.text.is_some()).count();
    let headers: usize = view.headers.iter().map(|h| h.categories.len()).sum();
    let header_texts = view.headers.iter().filter(|h| h.text.is_some()).count()
        + view
            .headers
            .iter()
            .flat_map(|h| &h.categories)
            .filter(|c| c.text.is_some())
            .count();
    eprintln!(
        "finder view: {} rows ({groups} group rows, {texts} with text), {} kind headers, \
         {headers} category headers, {header_texts} header texts, featured row {:?}",
        view.rows.len(),
        view.headers.len(),
        view.featured_row
    );
    // TI6: each row under exactly one category header under one kind header.
    let placed: usize = view
        .headers
        .iter()
        .flat_map(|h| &h.categories)
        .map(|c| c.rows.len())
        .sum();
    assert_eq!(placed, view.rows.len(), "TI6");
    assert_eq!(texts, view.rows.len(), "every finder row has a text");
    assert_eq!(
        header_texts,
        view.headers.len() + headers,
        "every header has a text"
    );
    assert!(featured.is_none() || view.featured_row.is_some());
    let sized = view.rows.iter().filter(|r| !r.sizes.is_empty()).count();
    let sizes_texts = view
        .rows
        .iter()
        .flat_map(|r| r.sizes.iter().chain(&r.tiers))
        .filter(|m| m.text.is_none())
        .count();
    eprintln!("  rows with sizes {sized}; menu items without text {sizes_texts}");

    // B3 over every group and size; B1 over every driver value.
    for (label, group) in control.group_table() {
        for &t in &group.templates {
            for size in fp.template(t).map(|t| t.group_sizes).unwrap_or_default() {
                let r = fp.group_template(label, size);
                assert!(r.is_some(), "group {label} size {size} resolves");
            }
        }
    }
    for n in 0..=control.featured.len() + 1 {
        let v = PropertyValue::Int(i32::try_from(n).unwrap_or(0));
        let f = fp.featured_instance(Some(&v));
        assert_eq!(f.is_some(), n >= 1 && n <= control.featured.len());
    }
    let travel = fp.instance_travel(order[0]);
    assert!(travel.title.is_some());
    eprintln!("instance real-data test took {:?}", started.elapsed());
}
