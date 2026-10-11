// SPDX-License-Identifier: GPL-3.0-or-later
//! A text cut to fit (`fonts.md` §7.6) with `UICore_Text_auto_tooltip_truncated_text`
//! gets its truncated-text tooltip (`ui-behaviour.md` §10.2) while its display list is
//! built; the paint step then settles on the next frame instead of staying unsettled for
//! good (a regression: the change made during the paint jobs was dropped). Synthetic tree,
//! invented values.

mod ui_common;

use std::sync::Arc;
use std::time::Duration;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_ui::env::PreparedText;
use tessera_ui::paint::Unlimited;
use ui_common::*;

const LAYOUT: u32 = 0x2200_0001;

fn settles_with(truncated: bool) -> (Vec<bool>, Option<PropertyValue>) {
    let vv = v();
    let pp = vv.pids;
    let text = prop(
        typed(0x2, vv.types.text, 0, 0, 100, 20),
        pp.text_auto_tooltip,
        b(true),
    );
    let root = child(field(0x1, 0, 0, 200, 200), text);
    let mut env = env_with(vec![layout(LAYOUT, 1024, 768, vec![root])]);
    env.texts.insert(
        "a long name".into(),
        Arc::new(PreparedText {
            truncated,
            ..PreparedText::default()
        }),
    );
    let mut ui = ui_with(env, 1024, 768);
    let rk = ui.open_panel(Did(LAYOUT), 0x1).unwrap();
    let tk = ui.find_child(rk, 0x2).unwrap();
    ui.set_text(tk, Some("a long name".into()));
    let mut budget = Unlimited;
    let settled = (0..4u64)
        .map(|t| {
            ui.paint(
                Duration::from_secs(10) + Duration::from_millis(16 * t),
                &mut budget,
            );
            ui.paint_settled()
        })
        .collect();
    (settled, ui.property(tk, pp.tooltips_on).cloned())
}

#[test]
fn a_cut_text_settles_after_its_tooltip_is_set() {
    let (settled, tips) = settles_with(true);
    assert!(settled[1..].iter().all(|s| *s), "{settled:?}");
    assert_eq!(
        tips,
        Some(PropertyValue::Bool(true)),
        "the truncated-text tooltip is on"
    );
    let (settled, _) = settles_with(false);
    assert!(settled.iter().all(|s| *s), "{settled:?}");
}
