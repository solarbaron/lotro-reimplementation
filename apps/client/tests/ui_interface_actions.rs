// SPDX-License-Identifier: GPL-3.0-or-later
//! The Interface window's text-size and art-filter rows (`ui-scaling.md` §7.2 rows 8 and 9,
//! §7.5, §7.6, §6.3) driven through the client's UI host, as the frame loop drives them:
//!
//! - the text slider moved on its scroll bar and the art buttons clicked with the pointer
//!   (raw events, step 1), the UI's events taken from the host's one per-frame list
//!   (`UiHost::take_game_events`) and handed to `UiHost::interface_frame`;
//! - the result checked in the UI (text factor, art filter), in the host's preferences, in
//!   the window's model, in the optional-row persist handed to the settings writer and in
//!   the `[UI]` keys that writer stores in a temporary settings file;
//! - the scaling state (*S*, *Se*, *F*, *A*, the safe aspect, the §4.5 settle and the scale
//!   persist) is untouched by both rows: they are not scale changes.
//!
//! Synthetic: no install, Tessera's neutral fallback art; no game data.

mod ui_common;

use std::time::{Duration, Instant};

use tessera_client::ui_host::interface::{ExtraPersist, model_of};
use tessera_client::ui_host::settings_writer::{SettingsJob, SettingsWriter};
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_settings::{ArtFilter, SettingsStore, UiPrefs, UiScale};
use tessera_ui::ElementKey;
use tessera_ui::game::interface::{ArtChoice, stop_for_text, text_for_stop};
use ui_common::Rig;

/// A physical screen at twice the design size of the rig's, at a fixed UI scale of 2, so
/// that UI units and physical pixels differ (the clicks go through the §3.6 mapping).
const SCREEN: (u32, u32) = (3200, 1800);
/// The fixed UI scale *S*.
const SCALE: f32 = 2.0;
/// The element ID of the first art-filter button of `tessera_ui::game::interface` (its
/// buttons follow in `ArtChoice::ALL` order). Tessera's own ID range, not data.
///
/// A copy of the private `ID_ART` (`ID_BASE + 0x20`) of `crates/ui/src/game/interface.rs`:
/// renumbering that ID block breaks this test with "art button ... not found". Follow-up:
/// look the buttons up through a public `InterfaceWindow` accessor (like `text_bar()`) once
/// `tessera-ui` has one.
const ID_ART: u32 = 0x7EFF_FD20;

fn host(o: &UiOptions) -> UiHost {
    let mut o = o.clone();
    o.screen = SCREEN;
    o.prefs.ui_scale = UiScale::Fixed(SCALE);
    UiHost::without_install(&o)
}

/// What the scaling state holds; equal before and after a text-size or art change.
#[derive(Debug, Clone, PartialEq)]
struct ScaleSnapshot {
    user: UiScale,
    safe_aspect: tessera_settings::SafeAspect,
    se_bits: u64,
    ui_size: (i32, i32),
    safe_area: String,
    limited: bool,
    ui_screen: (i32, i32),
}

fn snapshot(h: &UiHost) -> ScaleSnapshot {
    let s = h.scale();
    ScaleSnapshot {
        user: s.user_scale(),
        safe_aspect: s.safe_aspect(),
        se_bits: s.effective_scale().to_bits(),
        ui_size: s.ui_size(),
        safe_area: format!("{:?}", s.safe_area()),
        limited: s.is_limited(),
        ui_screen: h.ui().screen_size(),
    }
}

/// The element with `id` below the screen root (depth-first, bounded).
fn find_id(rig: &Rig, id: u32) -> Option<ElementKey> {
    let ui = rig.host.ui();
    let mut stack = vec![ui.screen_root()];
    let mut seen = 0usize;
    while let Some(k) = stack.pop() {
        seen += 1;
        if seen > 1 << 16 {
            return None;
        }
        if ui.element_id(k) == Some(id) {
            return Some(k);
        }
        stack.extend(ui.children(k));
    }
    None
}

/// Clicks the middle of `k` with the pointer: the UI box mapped to physical pixels with the
/// host's UI screen (`ui-scaling.md` §3.6), then raw events and one frame.
fn click_element(rig: &mut Rig, k: ElementKey) {
    let (x, y) = {
        let ui = rig.host.ui();
        let b = ui.displayed_box(k);
        assert!(b.w() > 0 && b.h() > 0, "the button is laid out: {b:?}");
        let s = rig.host.scale().screen();
        let x = (s.edge_x(f64::from(b.l)) + s.edge_x(f64::from(b.r) + 1.0)) / 2;
        let y = (s.edge_y(f64::from(b.t)) + s.edge_y(f64::from(b.b) + 1.0)) / 2;
        (x, y)
    };
    rig.click(f64::from(x), f64::from(y));
}

/// Hands the frame's UI events to the Interface window, as the client's per-frame window
/// pass does (`window_frames`).
fn window_pass(rig: &mut Rig) {
    let events = rig.host.take_game_events();
    rig.host.interface_frame(&events, Instant::now());
}

/// Opens the window on a settled UI and drains any persist from the start.
fn open(rig: &mut Rig) {
    assert!(
        rig.frames_until(500, |r| r.host.settled()),
        "the UI settles first"
    );
    let _ = rig.host.take_interface_persist();
    rig.host.open_interface();
    assert!(rig.host.interface_open());
    rig.frame();
    window_pass(rig);
    assert!(rig.host.interface_open(), "opening it posts no action");
    assert_eq!(rig.host.take_interface_persist(), None);
}

/// §7.5: the text slider sets `ui_text_scale` live (the UI's text factor), shows the new
/// stop, persists the optional row and leaves the scaling state alone.
#[test]
fn the_text_size_slider_sets_the_text_factor_not_the_scale() {
    let mut rig = Rig::with_host(|_| {}, host);
    open(&mut rig);
    let before = snapshot(&rig.host);
    assert!(
        (rig.host.scale().effective_scale() - f64::from(SCALE)).abs() < 1e-9,
        "Se = S at this screen: {before:?}"
    );
    let bar = rig
        .host
        .interface_window()
        .and_then(|w| w.text_bar())
        .expect("text slider");
    let stop = stop_for_text(1.25);
    assert!((text_for_stop(stop) - 1.25).abs() < 1e-6);
    rig.host
        .ui_mut()
        .scrollbar_select_stop(bar, i32::from(stop));
    rig.frame();
    window_pass(&mut rig);

    assert!(rig.host.interface_open());
    assert!(
        (rig.host.ui().text_scale() - 1.25).abs() < 1e-9,
        "{}",
        rig.host.ui().text_scale()
    );
    assert!((rig.host.prefs().text_scale - 1.25).abs() < 1e-6);
    let model = model_of(rig.host.scale(), rig.host.prefs());
    assert_eq!(model.text_stop, stop);
    assert_eq!(model.text_label, "125 %");
    assert_eq!(model.art, ArtChoice::Sharp, "the art row is unchanged");

    // Not a scale change: S, Se, F, A and the safe aspect are the same; no §4.5 settle
    // waits and no scale persist is pending.
    assert_eq!(snapshot(&rig.host), before);
    assert!(!rig.host.scale_repaint_pending());
    assert_eq!(
        rig.host
            .take_scale_persist(Instant::now() + Duration::from_secs(10)),
        None
    );
    let persist = rig
        .host
        .take_interface_persist()
        .expect("optional row persist");
    assert_eq!(
        persist,
        ExtraPersist {
            art: ArtFilter::Sharp,
            text_scale: text_for_stop(stop),
            clear_panels: false,
            clear_groups: false,
        }
    );
    assert_eq!(rig.host.take_interface_persist(), None, "taken once");
    // The texts reflow and the UI settles again at the same scale.
    assert!(rig.frames_until(500, |r| r.host.settled()));
    assert_eq!(snapshot(&rig.host), before);
    rig.shutdown();
}

/// §7.6 and §6.3: the art buttons, clicked with the pointer, set `ui_art_filter` live (the
/// UI's art filter) and persist it; the scaling state stays as it was.
#[test]
fn the_art_buttons_set_the_art_filter_not_the_scale() {
    let mut rig = Rig::with_host(|_| {}, host);
    open(&mut rig);
    let before = snapshot(&rig.host);
    for (i, (choice, filter, ui_filter)) in [
        (
            ArtChoice::Smooth,
            ArtFilter::Smooth,
            tessera_ui::paint::ArtFilter::Smooth,
        ),
        (
            ArtChoice::Nearest,
            ArtFilter::Nearest,
            tessera_ui::paint::ArtFilter::Nearest,
        ),
        (
            ArtChoice::Sharp,
            ArtFilter::Sharp,
            tessera_ui::paint::ArtFilter::Sharp,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let index = ArtChoice::ALL
            .iter()
            .position(|c| *c == choice)
            .expect("a choice");
        let button = find_id(&rig, ID_ART + u32::try_from(index).expect("small"))
            .unwrap_or_else(|| panic!("art button {choice:?}"));
        click_element(&mut rig, button);
        window_pass(&mut rig);
        assert!(rig.host.interface_open(), "step {i}: still open");
        assert_eq!(rig.host.ui().art_filter(), ui_filter, "step {i}");
        assert_eq!(rig.host.prefs().art_filter, filter, "step {i}");
        assert_eq!(
            model_of(rig.host.scale(), rig.host.prefs()).art,
            choice,
            "step {i}"
        );
        let persist = rig.host.take_interface_persist().expect("persist");
        assert_eq!(persist.art, filter, "step {i}");
        assert!(
            (persist.text_scale - 1.0).abs() < 1e-6,
            "text row unchanged"
        );
        assert!(!persist.clear_panels && !persist.clear_groups);
        assert_eq!(snapshot(&rig.host), before, "step {i}: not a scale change");
        assert!(!rig.host.scale_repaint_pending(), "step {i}");
        // Every surface repaints with the new filter; the UI settles again.
        assert!(rig.frames_until(500, |r| r.host.settled()), "step {i}");
    }
    assert_eq!(
        rig.host
            .take_scale_persist(Instant::now() + Duration::from_secs(10)),
        None
    );
    rig.shutdown();
}

/// The optional rows reach the `[UI]` keys through the client's settings writer (off the
/// frame thread, `ui-scaling.md` §4.4, §7.1), and only them: `ui_scale` keeps what the file
/// had.
#[test]
fn the_rows_reach_the_settings_file() {
    use tessera_ui::game::interface::InterfaceAction;
    let mut rig = Rig::with_host(|_| {}, host);
    open(&mut rig);
    let bar = rig
        .host
        .interface_window()
        .and_then(|w| w.text_bar())
        .expect("text slider");
    let stop = stop_for_text(1.5);
    rig.host
        .ui_mut()
        .scrollbar_select_stop(bar, i32::from(stop));
    rig.frame();
    window_pass(&mut rig);
    rig.host
        .apply_interface_action(InterfaceAction::SetArt(ArtChoice::Nearest), Instant::now());
    let extra = rig.host.take_interface_persist().expect("persist");
    assert_eq!(extra.art, ArtFilter::Nearest);
    assert!((extra.text_scale - text_for_stop(stop)).abs() < 1e-6);

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("tessera.toml");
    let seed = UiPrefs {
        ui_scale: UiScale::Fixed(SCALE),
        ..UiPrefs::default()
    };
    SettingsStore::new(path.clone())
        .update(|s| seed.store(s))
        .expect("seed the file");
    let mut writer = SettingsWriter::new(Some(SettingsStore::new(path.clone())));
    writer.submit(SettingsJob::UiExtra(extra));
    writer.flush();
    assert_ne!(writer.thread_id(), Some(std::thread::current().id()));
    let s = SettingsStore::new(path).update(|_| {}).expect("read back");
    let (prefs, _) = UiPrefs::from_settings(&s);
    assert_eq!(prefs.art_filter, ArtFilter::Nearest);
    assert!((prefs.text_scale - text_for_stop(stop)).abs() < 1e-6);
    assert_eq!(
        prefs.ui_scale,
        UiScale::Fixed(SCALE),
        "the scale row is untouched"
    );
    assert!((rig.host.ui().text_scale() - f64::from(text_for_stop(stop))).abs() < 1e-6);
    rig.shutdown();
}
