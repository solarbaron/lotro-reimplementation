// SPDX-License-Identifier: GPL-3.0-or-later
//! Saved placements through the host's production path (`ui-scaling.md` §5.8 "Saving" and
//! "Applying"): a user drag of a saved panel's drag bar, fed as raw pointer events through
//! frame-loop step 1, ends in `UiEvent::LayoutChanged`, which the host records for
//! `[UI.panels]`; showing and hiding a panel records nothing; and a panel that saves its
//! visibility, hidden at attachment by a stored `shown = false`, stays shown once the user
//! shows it again, across a later scale change (and a closed one stays closed). A synthetic
//! layout, no install.

mod ui_common;

use std::time::Instant;

use tessera_client::commands::uiscale::UiScaleRequest;
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_formats::Did;
use tessera_formats::property::{PropertyEntry, PropertyValue};
use tessera_formats::ui_layout::{ElementDesc, UiLayout};
use tessera_input::{MouseButton, RawEvent};
use tessera_settings::{AnchorX, AnchorY, PanelPlacement, PanelRef, SafeAspect, UiPrefs, UiScale};
use tessera_ui::{ElementKey, MemoryEnv, Vocab};
use ui_common::Rig;

const LAYOUT: u32 = 0x22FF_6200;
const ROOT: u32 = 0x1000_6201;
const BAR: u32 = 0x1000_6202;
const KEY: &str = "0x22FF6200/0x10006201";

fn element(id: u32, ty: u32, (x, y, w, h): (i32, i32, i32, i32)) -> ElementDesc {
    let mut d = ElementDesc {
        element_id: id,
        element_type: ty,
        ..ElementDesc::default()
    };
    d.block.rect.x = x;
    d.block.rect.y = y;
    d.block.rect.width = w;
    d.block.rect.height = h;
    d.block.has_position = true;
    d
}

/// A 200 × 100 window at (100, 50) saving its location (and, with `visibility`, its
/// visibility), with a drag bar over its top 20 units.
fn env(visibility: bool) -> MemoryEnv {
    let v = Vocab::default();
    let mut root = element(ROOT, v.types.field, (100, 50, 200, 100));
    let mut flags = vec![v.pids.save_loc];
    if visibility {
        flags.push(v.pids.save_visibility);
    }
    for pid in flags {
        root.block.properties.entries.push(PropertyEntry {
            pid,
            value: PropertyValue::Bool(true),
        });
    }
    root.block.properties.size.count =
        u32::try_from(root.block.properties.entries.len()).expect("few properties");
    root.children
        .push(element(BAR, v.types.dragbar, (0, 0, 200, 20)));
    let mut l = UiLayout {
        did: Did(LAYOUT),
        design_width: 1024,
        design_height: 768,
        ..UiLayout::default()
    };
    l.elements = vec![root];
    l.elements_size.count = 1;
    let mut env = MemoryEnv::default();
    env.add_layout(l);
    env
}

fn rig(visibility: bool, saved: Option<PanelPlacement>) -> (Rig, ElementKey) {
    let mut rig = Rig::with_host(
        |_| {},
        |o| {
            let mut prefs = UiPrefs {
                ui_scale: UiScale::Fixed(1.0),
                safe_aspect: SafeAspect::R21_9,
                ..UiPrefs::default()
            };
            if let Some(p) = saved {
                prefs.panels.insert(KEY.to_owned(), p);
            }
            let options = UiOptions { prefs, ..o.clone() };
            UiHost::new(Box::new(env(visibility)), None, Vocab::default(), &options)
        },
    );
    rig.host.set_persist_placements(true);
    let k = rig
        .host
        .ui_mut()
        .open_panel(Did(LAYOUT), ROOT)
        .expect("synthetic panel");
    rig.frame();
    (rig, k)
}

fn button(rig: &mut Rig, pressed: bool) {
    rig.raw(RawEvent::MouseButton {
        button: MouseButton::Left,
        pressed,
    });
}

#[test]
fn a_drag_of_the_bar_records_the_placement() {
    let (mut rig, k) = rig(false, None);
    assert!(rig.host.take_placement_persist().is_none(), "nothing moved");
    let before = rig.host.ui().displayed_box(k);
    rig.move_to(150.0, 60.0);
    rig.frame();
    button(&mut rig, true);
    rig.frame();
    rig.move_to(450.0, 260.0);
    rig.frame();
    button(&mut rig, false);
    rig.frame();
    let after = rig.host.ui().displayed_box(k);
    assert_ne!(
        (after.l, after.t),
        (before.l, before.t),
        "the drag moved it"
    );
    let panels = rig
        .host
        .take_placement_persist()
        .expect("the end of the move is recorded");
    assert!(panels.contains_key(KEY), "{panels:?}");
    rig.shutdown();
}

#[test]
fn showing_a_visibility_saving_panel_again_survives_a_scale_change() {
    let hidden = PanelPlacement {
        reference: PanelRef::Safe,
        ax: AnchorX::Left,
        dx: 100.0,
        ay: AnchorY::Top,
        dy: 50.0,
        w: None,
        h: None,
        shown: Some(false),
    };
    let (mut rig, k) = rig(true, Some(hidden));
    assert!(
        !rig.host.ui().is_visible(k),
        "attached hidden by its placement"
    );
    rig.host.ui_mut().set_shown(k, true);
    rig.frame();
    rig.host.ui_mut().set_shown(k, false);
    rig.frame();
    rig.host.ui_mut().set_shown(k, true);
    rig.frame();
    assert!(
        rig.host.take_placement_persist().is_none(),
        "showing and hiding record nothing (§5.8: moves and resizes only)"
    );
    rig.host
        .apply_uiscale_request(&UiScaleRequest::Set(1.5), Instant::now());
    rig.frame();
    assert!(rig.host.ui().is_visible(k), "the shown panel stays shown");
    // A closed panel stays closed across a scale change too.
    rig.host.ui_mut().set_shown(k, false);
    rig.host
        .apply_uiscale_request(&UiScaleRequest::Set(1.0), Instant::now());
    rig.frame();
    assert!(!rig.host.ui().is_visible(k));
    rig.shutdown();
}
