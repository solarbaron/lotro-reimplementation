// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the progression widgets: the experience bar on an install-like meter (a
//! `LevelProgressMeter` element of the engine Meter class) and the level-up phases.

use super::super::GameVocab;
use super::super::npc::kit::{FNode, open_root};
use super::xpbar::{BannerPhase, FADE_IN, LEVELUP_SECONDS, banner_phase};
use super::*;
use crate::{MemoryEnv, Ui, Vocab};

#[test]
fn experience_bar_on_the_toolbar_meter() {
    let mut ui = Ui::new(
        Box::new(MemoryEnv::default()),
        Vocab::default(),
        (1024, 768),
    );
    let gv = GameVocab::default();
    gv.register_classes(&mut ui);
    let node = FNode::field(0, 0, 0, 400, 40).child(
        FNode::new(crate::WidgetClass::Meter, 0x1000_0001, 10, 10, 300, 10)
            .typed(gv.types.level_progress_meter),
    );
    let root = open_root(&mut ui, &node).unwrap();
    let mut bar = ExperienceBar::attach(&ui, &gv, root);
    assert!(bar.is_attached());
    let m = bar.meter.unwrap();
    bar.show(
        &mut ui,
        &ExperienceView {
            level: 5,
            level_text: "Level 5".into(),
            current: 250,
            needed: 1000,
            text: "250 / 1000".into(),
            tooltip: String::new(),
        },
    );
    let pid = ui.vocab().pids.meter_position;
    assert_eq!(ui.prop(m, pid).float(), 0.25);
    assert_eq!(bar.fill_width(&ui), None);
}

#[test]
fn experience_position() {
    let v = |c, n| ExperienceView {
        current: c,
        needed: n,
        ..ExperienceView::default()
    };
    assert_eq!(v(5, 0).position(), 0.0);
    assert_eq!(v(50, 100).position(), 0.5);
    assert_eq!(v(500, 100).position(), 1.0);
}

#[test]
fn banner_phases() {
    assert_eq!(banner_phase(-0.1), BannerPhase::Hidden);
    assert_eq!(banner_phase(f64::NAN), BannerPhase::Hidden);
    assert_eq!(banner_phase(FADE_IN / 2.0), BannerPhase::Shown(0.5));
    assert_eq!(banner_phase(2.0), BannerPhase::Shown(1.0));
    assert_eq!(banner_phase(LEVELUP_SECONDS - 0.5), BannerPhase::Shown(0.5));
    assert_eq!(banner_phase(LEVELUP_SECONDS), BannerPhase::Hidden);
}

#[test]
fn skill_row_ghosting() {
    let r = SkillRowView {
        trained: true,
        ..SkillRowView::default()
    };
    assert!(r.ghosted());
    assert!(
        SkillRowView {
            unmet: true,
            ..SkillRowView::default()
        }
        .ghosted()
    );
    assert!(!SkillRowView::default().ghosted());
}
