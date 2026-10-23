// SPDX-License-Identifier: GPL-3.0-or-later
//! The bridge between the UI's saved placements and `[UI.panels]` in the client's UI host
//! (`ui-scaling.md` §5.8 implementer OPEN, §7.1 "Reset panel positions", invariant I8,
//! §10.4): a user move of a saved panel is recorded, handed to the settings writer and written
//! off the frame thread; a host rebuilt from the written settings places the panel at the
//! same anchored offset, also after a scale change; a reset empties the in-memory store with
//! `[UI.panels]` and puts the panel back at its layout position. A synthetic layout, no
//! install; the settings file is a temporary one.

use std::time::Instant;

use tessera_client::commands::uiscale::UiScaleRequest;
use tessera_client::ui_host::settings_writer::{SettingsJob, SettingsWriter};
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_formats::Did;
use tessera_formats::property::{PropertyEntry, PropertyValue};
use tessera_formats::ui_layout::{EdgeMode, ElementDesc, UiLayout};
use tessera_settings::{AnchorX, AnchorY, SafeAspect, SettingsStore, UiPrefs, UiScale};
use tessera_ui::{ElementKey, MemoryEnv, Vocab};

const LAYOUT: u32 = 0x22FF_6000;
const ROOT: u32 = 0x1000_6001;
const KEY: &str = "0x22FF6000/0x10006001";

/// A 200 × 100 window at (100, 50) of a 1024 × 768 design area, edges KEEP, that saves its
/// location.
fn env() -> MemoryEnv {
    let v = Vocab::default();
    let mut d = ElementDesc {
        element_id: ROOT,
        element_type: v.types.field,
        ..ElementDesc::default()
    };
    d.block.rect.x = 100;
    d.block.rect.y = 50;
    d.block.rect.width = 200;
    d.block.rect.height = 100;
    d.block.has_position = true;
    for (i, e) in [0u32; 4].iter().enumerate() {
        let m = EdgeMode(*e);
        match i {
            0 => d.edges.left = m,
            1 => d.edges.top = m,
            2 => d.edges.right = m,
            _ => d.edges.bottom = m,
        }
    }
    d.block.properties.entries.push(PropertyEntry {
        pid: v.pids.save_loc,
        value: PropertyValue::Bool(true),
    });
    d.block.properties.size.count = 1;
    let mut l = UiLayout {
        did: Did(LAYOUT),
        design_width: 1024,
        design_height: 768,
        ..UiLayout::default()
    };
    l.elements = vec![d];
    l.elements_size.count = 1;
    let mut env = MemoryEnv::default();
    env.add_layout(l);
    env
}

fn host(prefs: UiPrefs) -> (UiHost, ElementKey) {
    let options = UiOptions {
        screen: (1920, 1080),
        prefs,
        hud: false,
        panels: Vec::new(),
        record: false,
    };
    let mut h = UiHost::new(Box::new(env()), None, Vocab::default(), &options);
    h.set_persist_placements(true);
    let k = h
        .ui_mut()
        .open_panel(Did(LAYOUT), ROOT)
        .expect("synthetic panel");
    // The frame's placement pass (`UiHost::step8` runs it before painting).
    h.apply_saved_placements(false);
    (h, k)
}

fn prefs() -> UiPrefs {
    UiPrefs {
        ui_scale: UiScale::Fixed(1.0),
        safe_aspect: SafeAspect::R21_9,
        ..UiPrefs::default()
    }
}

/// The distance of the panel's displayed right edge from the safe area's right edge, and of
/// its top from the safe area's top, in UI units.
fn offsets(h: &UiHost, k: ElementKey) -> (f64, f64) {
    let ui = h.ui();
    let a = ui.safe_area_box();
    let d = ui.displayed_box(k);
    (a.l + a.w - f64::from(d.r + 1), f64::from(d.t) - a.t)
}

#[test]
fn a_moved_panel_comes_back_at_its_anchored_offset_and_resets() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("tessera.toml");

    // A user move near the right edge at F = 1920 × 1080 (Se 1).
    let (mut h, k) = host(prefs());
    assert!(h.ui().is_saved_element(k), "the root saves its location");
    h.ui_mut().move_to(k, 1700, 40);
    h.record_placement(k);
    let panels = h.take_placement_persist().expect("a placement to persist");
    let p = &panels[KEY];
    assert_eq!((p.ax, p.ay), (AnchorX::Right, AnchorY::Top));
    assert!(
        (p.dx - 20.0).abs() < 0.01 && (p.dy - 40.0).abs() < 0.01,
        "{p:?}"
    );
    assert!(h.take_placement_persist().is_none(), "once per change");

    // Written by the settings writer's thread.
    let mut w = SettingsWriter::new(Some(SettingsStore::new(path.clone())));
    w.submit(SettingsJob::Panels(panels));
    w.flush();
    assert_ne!(w.thread_id(), Some(std::thread::current().id()));
    let settings = SettingsStore::new(path.clone()).load().settings;
    let (read, _) = UiPrefs::from_settings(&settings);
    assert!(read.panels.contains_key(KEY), "{:?}", read.panels);

    // A host rebuilt from the written settings places the panel at the same offset.
    let mut p2 = prefs();
    p2.panels = read.panels.clone();
    let (mut h2, k2) = host(p2);
    let (dx, dy) = offsets(&h2, k2);
    assert!(
        (dx - 20.0).abs() <= 1.0 && (dy - 40.0).abs() <= 1.0,
        "{dx} {dy}"
    );
    assert_eq!(h2.ui().displayed_box(k2).l, 1700);

    // A scale change (F shrinks): the panel keeps its distance from the right edge (I8).
    h2.apply_uiscale_request(&UiScaleRequest::Set(1.5), Instant::now());
    let f = h2.ui().screen_size();
    assert!(f.0 < 1920, "F changed: {f:?}");
    let (dx, dy) = offsets(&h2, k2);
    assert!(
        (dx - 20.0).abs() <= 1.0 && (dy - 40.0).abs() <= 1.0,
        "{dx} {dy}"
    );

    // "Reset panel positions": the store is emptied with `[UI.panels]` and the panel is back
    // at its layout position.
    h2.reset_panel_positions();
    assert!(h2.ui().saved_store().placements.is_empty());
    assert!(h2.prefs().panels.is_empty());
    let d = h2.ui().displayed_box(k2);
    assert_eq!((d.l, d.t), (100, 50), "{d:?}");
    let extra = h2.take_interface_persist().expect("the reset is persisted");
    assert!(extra.clear_panels);
    // A later frame does not move it back (nothing is stored any more).
    h2.apply_saved_placements(true);
    let d = h2.ui().displayed_box(k2);
    assert_eq!((d.l, d.t), (100, 50));
}

#[test]
fn scripted_runs_do_not_persist_moves() {
    let (mut h, k) = host(prefs());
    h.set_persist_placements(false);
    h.ui_mut().move_to(k, 600, 300);
    h.record_placement(k);
    assert!(h.take_placement_persist().is_none());
}
