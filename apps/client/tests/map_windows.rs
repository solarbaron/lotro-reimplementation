// SPDX-License-Identifier: GPL-3.0-or-later
//! The map window's feed markers and the P5 client built-ins without an install (decision
//! 0024 P5; `map-windows.md` §6.7, §7.3; `walker.md` §16.3; `travel-windows.md` §6.6): the
//! committed `map` scenario plays headless and the client's [`MapUi`] (Tessera's fallback
//! window) keeps every feed marker of the session, replaces a moved one and drops removed
//! ones; the dispatcher registers `ride`, `findplace` and `leaveinstance` under Tessera's
//! names and queues their requests.

mod travel_common;
mod ui_common;

use std::sync::Arc;
use std::time::Instant;

use tessera_client::commands::p5::P5Request;
use tessera_client::commands::{Context, Dispatcher, LineKind, Origin};
use tessera_client::map::{MapSettings, MapUi};
use tessera_client::travel::P5Refs;
use tessera_ui::game::mapfallback::P5Art;
use tessera_ui::game::mapmarks::DefaultMapTexts;
use travel_common::play;
use ui_common::Rig;

#[test]
fn feed_markers_follow_the_session() {
    let mut rig = Rig::new(|_| {});
    rig.frame();
    let mut map = MapUi::new(
        Arc::new(P5Refs::fallback()),
        P5Art::default(),
        MapSettings::default(),
    );
    {
        let mut ui = rig.host.ui_mut();
        map.open(&mut ui, None, None, &DefaultMapTexts)
            .expect("the fallback map window");
    }
    let mut max = 0usize;
    let mut moved = false;
    let (outcome, _) = play("map", None, |p| {
        rig.frame();
        {
            let mut ui = rig.host.ui_mut();
            map.frame(&mut ui, p.p5, None, None, &[], &DefaultMapTexts);
        }
        let view = map.view().expect("view");
        let on: Vec<u32> = view.overlays.markers().keys().copied().collect();
        let want: Vec<u32> = p.p5.markers().keys().copied().collect();
        assert_eq!(
            on, want,
            "the window holds the session's markers (§6.7 item 7)"
        );
        max = max.max(on.len());
        if let Some(m) = view.overlays.markers().get(&1)
            && m.label == "Camp, moved"
        {
            moved = true;
            assert!((m.point.global.0 - 12.0).abs() < 1e-3);
        }
        if let Some(m) = view.overlays.markers().get(&2) {
            assert!(m.quest, "a quest marker keeps its kind");
        }
    });
    assert_eq!(outcome.exit_code(), 0);
    assert_eq!(max, 3, "three markers without an install");
    assert!(moved, "a later MarkerAdded replaces the marker");
    assert!(map.view().expect("view").overlays.markers().len() <= 1);
    let mut ui = rig.host.ui_mut();
    let lines = map.lookup(&mut ui, None, "Bree", None, &DefaultMapTexts);
    assert_eq!(lines.len(), 1, "a line, not a result, without an install");
    assert!(map.failures().is_empty(), "{:?}", map.failures());
}

#[test]
fn p5_builtins_queue_their_requests() {
    let mut d = Dispatcher::new();
    for name in ["ride", "findplace", "leaveinstance"] {
        assert!(d.registry().is_command(name), "{name} is registered");
    }
    let mut out = Vec::new();
    let mut ctx = Context {
        now: Instant::now(),
        origin: Origin::Chat,
        session: None,
        music: None,
        plugins: None,
    };
    for line in [
        "/ride",
        "/ride steed:medium@lock=canter",
        "/ride off",
        "/ride list 3",
        "/ride dir:99999",
        "/findplace 12.3S, 45.6W",
        "/findplace",
        "/leaveinstance",
    ] {
        d.submit(line, &mut ctx, &mut out);
    }
    let got = d.p5_queue().take();
    assert_eq!(got.len(), 7, "{got:?}");
    assert!(matches!(got[0], P5Request::Ride(_)));
    assert_eq!(got[4], P5Request::FindPlace("12.3S, 45.6W".into()));
    assert_eq!(got[5], P5Request::FindPlace(String::new()));
    assert_eq!(got[6], P5Request::LeaveInstance);
    assert_eq!(
        out.len(),
        1,
        "the bad /ride argument posts one line: {out:?}"
    );
    assert_eq!(out[0].kind, LineKind::Error);
    out.clear();
    d.submit("/help ride", &mut ctx, &mut out);
    assert!(
        out.iter().any(|l| l.text.contains("list [PAGE]")),
        "{out:?}"
    );
}

/// `map-windows.md` §6.7 item 7 on the minimap (§6.8): a later `MarkerAdded` of a plain
/// marker replaces it there too (moved and relabelled), and removed ones go.
#[test]
fn minimap_markers_follow_replacements() {
    use tessera_ui::game::radar::RadarInput;
    let mut rig = Rig::new(|_| {});
    rig.frame();
    let mut map = MapUi::new(
        Arc::new(P5Refs::fallback()),
        P5Art::default(),
        MapSettings::default(),
    );
    let input = RadarInput::default();
    let mut moved = false;
    let (outcome, _) = play("map", None, |p| {
        {
            let mut ui = rig.host.ui_mut();
            map.update_radars(&mut ui, &[], &input, p.p5, None, &DefaultMapTexts);
        }
        let plain: Vec<u32> =
            p.p5.markers()
                .iter()
                .filter(|(_, m)| m.quest.is_none())
                .map(|(id, _)| *id)
                .collect();
        let on: Vec<u32> = map.radar_overlays().markers().keys().copied().collect();
        assert_eq!(on, plain, "the minimaps hold the plain markers");
        if let Some(m) = map.radar_overlays().markers().get(&1)
            && m.label == "Camp, moved"
        {
            moved = true;
            assert!((m.point.global.0 - 12.0).abs() < 1e-3);
        }
    });
    assert_eq!(outcome.exit_code(), 0);
    assert!(moved, "a later MarkerAdded replaces the minimap marker");
}
