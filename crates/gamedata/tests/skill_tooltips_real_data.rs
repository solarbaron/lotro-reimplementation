// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data census of the skill and effect tooltips (`skill-tooltips.md` §14, decision 0024
//! P2 exit criterion 2, data side) and a read of the combat data (`combat-ui.md`,
//! `combat-visuals.md`) against a user's own install, read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Builds the
//! hover tooltip of every skill and every effect record with the Tessera rules (viewer of
//! level 1, no vitals, no live effect), its examination icon and, for skills, the quickslot
//! icon; every record must end in exactly one tooltip category and one icon category, and the
//! rendered tooltips must be exactly the records with a usable hover pair. Prints counts only,
//! never game text. Use `--release`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tessera_formats::Did;
use tessera_formats::Record;
use tessera_formats::surface::Surface;
use tessera_gamedata::GameDataStore;
use tessera_gamedata::combat::CombatData;
use tessera_gamedata::tooltip::{
    CombatCategory, CombatCensus, CombatSubject, IconCategory, InstallTooltipData, Purpose,
    TooltipData, TooltipEngine,
};
use tessera_text::Strings;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 8)
}

#[test]
#[allow(clippy::too_many_lines)]
fn every_skill_and_effect_record_is_categorised() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the skill tooltip real-data test");
        return;
    };
    let t0 = Instant::now();
    let store = GameDataStore::open(&dir).expect("install opens");
    let strings = Arc::new(Strings::open_install(&dir, None).expect("strings open"));
    let data = InstallTooltipData::new(&store, strings).expect("tooltip data");
    let engine = TooltipEngine::new(&data);
    let skills = store.skills().expect("skills");
    let effects = store.effects().expect("effects");
    eprintln!(
        "{} skills, {} effects ({:?} to index)",
        skills.len(),
        effects.len(),
        t0.elapsed()
    );
    assert!(!skills.is_empty() && !effects.is_empty());

    // Surfaces decode once each.
    let surfaces: Mutex<HashMap<Did, bool>> = Mutex::new(HashMap::new());
    let decodes = |d: Did| -> bool {
        if let Some(&ok) = surfaces.lock().expect("lock").get(&d) {
            return ok;
        }
        let ok = matches!(store.object_store().game_data().read(d), Ok(Some(bytes))
            if Surface::parse(&bytes).is_ok_and(|s| s.to_rgba8().is_ok()));
        surfaces.lock().expect("lock").insert(d, ok);
        ok
    };

    let subjects: Vec<CombatSubject> = skills
        .iter()
        .map(|&d| CombatSubject::Skill(d))
        .chain(effects.iter().map(|&d| CombatSubject::Effect(d)))
        .collect();
    let t1 = Instant::now();
    let next = AtomicUsize::new(0);
    let (skill_census, effect_census) = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads())
            .map(|_| {
                s.spawn(|| {
                    let mut sc = CombatCensus::default();
                    let mut ec = CombatCensus::default();
                    loop {
                        let i = next.fetch_add(64, Ordering::Relaxed);
                        if i >= subjects.len() {
                            break;
                        }
                        for subject in &subjects[i..(i + 64).min(subjects.len())] {
                            let o = engine.classify_combat(subject);
                            let icon = IconCategory::of(o.icon, decodes);
                            match subject {
                                CombatSubject::Skill(d) => {
                                    let q = IconCategory::of(o.quickslot_icon, decodes);
                                    sc.add(*d, &o, icon, Some(q));
                                }
                                CombatSubject::Effect(d) => ec.add(*d, &o, icon, None),
                                _ => {}
                            }
                        }
                    }
                    (sc, ec)
                })
            })
            .collect();
        let mut sc = CombatCensus::default();
        let mut ec = CombatCensus::default();
        for w in workers {
            let (a, b) = w.join().expect("worker");
            sc.merge(a);
            ec.merge(b);
        }
        (sc, ec)
    });
    eprintln!("classified in {:?}", t1.elapsed());

    // The expected number of built tooltips: records whose hover pair names a top-level
    // element of its layout, counted independently of the build.
    let expected = |dids: &[Did]| -> u64 {
        let mut n = 0;
        for &d in dids {
            let Ok(Some(set)) = data.record(d) else {
                continue;
            };
            if let Some(p) = engine.pair(&set, Purpose::Hover)
                && data
                    .layout(p.layout)
                    .ok()
                    .flatten()
                    .is_some_and(|l| l.top_level(p.element).is_some())
            {
                n += 1;
            }
        }
        n
    };
    for (what, census, dids) in [
        ("skills", &skill_census, &skills),
        ("effects", &effect_census, &effects),
    ] {
        eprintln!("{what}:");
        for (k, v) in &census.tooltips {
            eprintln!("  {k:?}: {v}");
        }
        eprintln!(
            "  no name (missing, empty): {:?}; row failures: {}; escaped: {}; deferred rows: {}; lookup misses: {}",
            census.no_name,
            census.row_failures,
            census.escaped,
            census.deferred,
            census.lookup_misses
        );
        for (k, v) in &census.icons {
            eprintln!("  icon {k:?}: {v}");
        }
        for (k, v) in &census.quickslot_icons {
            eprintln!("  quickslot icon {k:?}: {v}");
        }
        for (label, n) in &census.non_empty {
            eprintln!("  non-empty {label}: {n}");
        }
        for e in &census.examples {
            eprintln!("  example: {e}");
        }
        let total: u64 = census.tooltips.values().sum();
        assert_eq!(
            total + census.escaped,
            dids.len() as u64,
            "{what}: every record once"
        );
        assert_eq!(census.escaped, 0, "{what}: errors escaped the categories");
        let built =
            census.count(CombatCategory::Rendered) + census.count(CombatCategory::RenderedNoName);
        assert_eq!(
            built,
            expected(dids),
            "{what}: built tooltips = usable hover pairs"
        );
        assert_eq!(census.count(CombatCategory::LayoutUnreadable), 0, "{what}");
        assert_eq!(census.count(CombatCategory::ElementMissing), 0, "{what}");
        assert_eq!(census.row_failures, 0, "{what}: row failures");
        let icons: u64 = census.icons.values().sum();
        assert_eq!(icons, dids.len() as u64, "{what}: every icon once");
    }
    // ST7, SV4: every skill names an examination and a quickslot icon that decodes.
    assert_eq!(
        skill_census.icons.get(&IconCategory::Rendered).copied(),
        Some(skills.len() as u64)
    );
    assert_eq!(
        skill_census
            .quickslot_icons
            .get(&IconCategory::Rendered)
            .copied(),
        Some(skills.len() as u64)
    );
    assert_eq!(skill_census.count(CombatCategory::RenderedNoName), 0);
    // §14.1: no effect icon is unreadable either.
    assert_eq!(
        effect_census
            .icons
            .get(&IconCategory::Unreadable)
            .copied()
            .unwrap_or(0),
        0
    );
    // §7.10: cost entries with a progression store `Skill_Vital_Points` = 0; the progression
    // is evaluated (level 1 here), so the vital-cost row is shown for well over a few skills
    // (§14.3 informative: 2,230 skills with a vital-cost source).
    let cost_rows = skill_census
        .non_empty
        .get("Skill_VitalCost_Base")
        .copied()
        .unwrap_or(0);
    eprintln!("vital-cost rows shown at level 1: {cost_rows}");
    assert!(cost_rows > 1000, "vital-cost rows: {cost_rows}");

    // Typed records: a skill is a skill, not an effect; a surface DID is no record (Ok(None)).
    let first = skills[0];
    assert!(store.skill_record(first).expect("skill").is_some());
    assert!(store.effect_record(first).expect("not an effect").is_none());
    assert!(
        store
            .skill_record(Did(0x4100_0001))
            .expect("non-object DID")
            .is_none()
    );
    assert!(store.effect_record(effects[0]).expect("effect").is_some());

    // Combat data (`combat-ui.md`, `combat-visuals.md`): read every skill's and effect's
    // visual data and the singletons; nothing may panic.
    let t2 = Instant::now();
    let combat = CombatData::new(&data);
    let mut no_behaviour = 0usize;
    for &d in &skills {
        let v = combat.skill_visuals(d).expect("skill visuals");
        if v.self_list.iter().all(|&k| k == 0) || v.other_list.iter().all(|&k| k == 0) {
            no_behaviour += 1;
        }
    }
    let mut with_fx = 0usize;
    for &d in &effects {
        if combat.effect_visuals(d).is_some_and(|v| v.fx != 0) {
            with_fx += 1;
        }
        let _ = combat.effect_icon(d);
    }
    let con = combat.consideration();
    eprintln!(
        "combat data in {:?}: skills without a behaviour {no_behaviour}; effects with Effect_FX {with_fx}; \
         consideration lists {}; vital display overrides {}; combat states {}; floaty far ranges {} / {}",
        t2.elapsed(),
        con.map_or(0, |c| c.lists.len()),
        combat.vital_display_overrides().entries.len(),
        combat.combat_states().len(),
        combat.floaty_settings(false).far_range,
        combat.floaty_settings(true).far_range,
    );
    // CV2: every skill has non-empty Self and Other lists.
    assert_eq!(no_behaviour, 0);
    assert!(con.is_some_and(|c| !c.lists.is_empty() && !c.rank_states.is_empty()));
    assert!(!combat.vital_display_overrides().entries.is_empty());
    assert!(!combat.combat_states().is_empty());
}
