// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0024 P2 exit criterion 2, client side, on the user's install (read-only;
//! `skill-tooltips.md` §14): every skill and every effect record's hover tooltip is
//! modelled (`tessera-gamedata`), converted by the client with its examination icon
//! (`combat::view::exam_tooltip`) and laid out headless in the game UI's examination
//! panel (`tessera-ui`), and each record ends in exactly one tooltip category and one icon
//! category of §14.1.
//!
//! A failure is a panic, an error escaping the categories, a tooltip whose model was built
//! but which the panel could not lay out (no frame, a template that could not be created,
//! an empty frame), a tooltip whose record has a decoding examination icon but whose icon
//! row shows none, or a rendered count other than the records with a valid hover pair.
//! Prints counts only, never game text.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Run with
//! `TESSERA_GAME_DIR=… cargo test --release -p tessera-client --test
//! combat_tooltips_real_data -- --nocapture`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_client::character::CharacterData;
use tessera_client::combat::view::exam_tooltip;
use tessera_client::ui_host::boot::UiBoot;
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::Did;
use tessera_formats::Record as _;
use tessera_formats::surface::Surface;
use tessera_gamedata::tooltip::{
    CombatCategory, CombatCensus, CombatRequest, CombatSubject, IconCategory, Purpose, SkillViewer,
    TooltipData as _,
};
use tessera_ui::game::examine::{ExamKey, ExamTooltips, RowContent};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// Whether a row (or a row nested in it, as `NameIcon_Row_Base` nests its icon) shows an
/// icon or image.
fn has_icon(r: &tessera_ui::game::examine::ExamRow) -> bool {
    match &r.content {
        RowContent::Image(_) | RowContent::Icon { image: Some(_), .. } => true,
        RowContent::Pair { left, right } => has_icon(left) || has_icon(right),
        RowContent::List(rows) => rows.iter().any(has_icon),
        _ => false,
    }
}

#[derive(Debug, Default)]
struct Layout {
    built: u64,
    failed: u64,
    template_failures: u64,
    with_icon: u64,
    icon_missing: u64,
    examples: Vec<String>,
}

impl Layout {
    fn example(&mut self, s: String) {
        if self.examples.len() < 16 {
            self.examples.push(s);
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn every_skill_and_effect_tooltip_is_laid_out_or_categorised() {
    let Some(dir) = game_dir() else { return };
    let t0 = Instant::now();
    let data = CharacterData::open(&dir).expect("character data");
    let skills = data.store().skills().expect("skills");
    let effects = data.store().effects().expect("effects");
    eprintln!(
        "{} skills, {} effects ({:?} to open and index)",
        skills.len(),
        effects.len(),
        t0.elapsed()
    );
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let empty = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(Arc::clone(&set), dir.clone(), Some(empty.path().into()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    let vocab = booted.char_vocab.clone();
    let host = UiHost::from_boot(
        booted,
        &UiOptions {
            screen: (1920, 1080),
            hud: false,
            ..UiOptions::default()
        },
    );
    let tips = ExamTooltips::new(vocab);

    let mut surfaces: HashMap<Did, bool> = HashMap::new();
    let mut decodes = |d: Did| -> bool {
        *surfaces.entry(d).or_insert_with(|| {
            set.read(d.0)
                .ok()
                .is_some_and(|b| Surface::parse(&b).is_ok_and(|s| s.to_rgba8().is_ok()))
        })
    };

    let engine = data.engine();
    let viewer = SkillViewer::default();
    let mut census = CombatCensus::default();
    let mut layout = Layout::default();
    let mut expected = 0u64;
    let subjects: Vec<CombatSubject> = skills
        .iter()
        .map(|&d| CombatSubject::Skill(d))
        .chain(effects.iter().map(|&d| CombatSubject::Effect(d)))
        .collect();
    let t1 = Instant::now();
    for (n, subject) in subjects.iter().enumerate() {
        let (did, is_skill) = match subject {
            CombatSubject::Skill(d) => (*d, true),
            CombatSubject::Effect(d) => (*d, false),
            _ => unreachable!("records only"),
        };
        let outcome = engine.classify_combat(subject);
        let icon = IconCategory::of(outcome.icon, &mut decodes);
        let quickslot = is_skill.then(|| IconCategory::of(outcome.quickslot_icon, &mut decodes));
        census.add(did, &outcome, icon, quickslot);
        // The independent count of records with a valid hover pair.
        if let Ok(Some(rec)) = engine.data().record(did)
            && let Some(p) = engine.pair(&rec, Purpose::Hover)
            && engine
                .data()
                .layout(p.layout)
                .ok()
                .flatten()
                .is_some_and(|l| l.top_level(p.element).is_some())
        {
            expected += 1;
        }
        if !matches!(
            outcome.category,
            Ok(CombatCategory::Rendered | CombatCategory::RenderedNoName)
        ) {
            continue;
        }
        let model = engine
            .build_combat(&CombatRequest::hover(subject.clone()), &viewer)
            .unwrap_or_else(|e| panic!("{did}: categorised as built but did not build: {e}"));
        let Some(input) = exam_tooltip(
            &data,
            &model,
            ExamKey {
                instance: 0,
                record: did.0,
            },
        ) else {
            layout.failed += 1;
            layout.example(format!("{did}: not converted"));
            continue;
        };
        // §11.1: the icon field shows the examination icon (an image row of the model).
        let has_icon_row = input.rows.iter().any(has_icon);
        if has_icon_row {
            layout.with_icon += 1;
        } else if icon == IconCategory::Rendered {
            layout.icon_missing += 1;
            layout.example(format!("{did}: laid out without its icon"));
        }
        let mut ui = host.ui_mut();
        match tips.build(&mut ui, &input) {
            Ok(b) => {
                layout.built += 1;
                if !b.failures.is_empty() || b.size.0 <= 0 || b.size.1 <= 0 {
                    layout.template_failures += 1;
                    layout.example(format!(
                        "{did}: size {:?}, {} template failures",
                        b.size,
                        b.failures.len()
                    ));
                }
                ui.destroy(b.frame);
            }
            Err(e) => {
                layout.failed += 1;
                layout.example(format!("{did}: {e}"));
            }
        }
        if n % 10_000 == 0 && n > 0 {
            eprintln!("  {n} records, {:?}", t1.elapsed());
        }
    }
    eprintln!("modelled and laid out in {:?}", t1.elapsed());
    for (k, v) in &census.tooltips {
        eprintln!("  {k:?}: {v}");
    }
    for (label, n) in &census.non_empty {
        eprintln!("  non-empty {label}: {n}");
    }
    for (k, v) in &census.icons {
        eprintln!("  icon {k:?}: {v}");
    }
    for (k, v) in &census.quickslot_icons {
        eprintln!("  quickslot icon {k:?}: {v}");
    }
    eprintln!(
        "  escaped: {}, row failures: {}, no name (missing, empty): {:?}, deferred rows: {}",
        census.escaped, census.row_failures, census.no_name, census.deferred
    );
    eprintln!(
        "  laid out: {} (with an icon {}), not laid out: {}, template failures: {}, \
         without their icon: {}",
        layout.built,
        layout.with_icon,
        layout.failed,
        layout.template_failures,
        layout.icon_missing
    );
    for e in census.examples.iter().chain(&layout.examples) {
        eprintln!("  example: {e}");
    }

    let total = subjects.len() as u64;
    let categorised: u64 = census.tooltips.values().sum::<u64>() + census.escaped;
    assert_eq!(categorised, total, "every record is categorised once");
    assert_eq!(census.escaped, 0, "errors escaped the categories");
    assert_eq!(
        census.icons.values().sum::<u64>(),
        total,
        "every record has one icon category"
    );
    assert_eq!(
        census.count(CombatCategory::LayoutUnreadable),
        0,
        "unreadable layouts"
    );
    assert_eq!(census.count(CombatCategory::ElementMissing), 0);
    assert_eq!(census.row_failures, 0, "row failures");
    let rendered =
        census.count(CombatCategory::Rendered) + census.count(CombatCategory::RenderedNoName);
    assert_eq!(
        rendered, expected,
        "rendered = records with a valid hover pair"
    );
    assert_eq!(layout.built, rendered, "every built tooltip is laid out");
    assert_eq!(layout.failed, 0, "tooltips not laid out");
    assert_eq!(layout.template_failures, 0, "template failures");
    assert_eq!(layout.icon_missing, 0, "tooltips without their icon");
}
