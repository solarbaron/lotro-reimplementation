// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI host's wiring of `tessera-ui` (`ui-scaling.md` §6.3–§6.7, §7.1, §7.5;
//! `render.md` §6.16 U2; `lua-plugin-api.md` §13.12.1, §18 check 76):
//!
//! - the `[UI]` art filter, text factor and text source reach the UI at start;
//! - the font ladders' enum agrees with the plugin API's `Turbine.UI.Lotro.Font`;
//! - the software cursor's texture follows *Se* × `ui_scale_cursor` (needs a GPU adapter;
//!   skipped, reported as passed, without one);
//! - the UI pass's GPU time reaches the frame's sample (U2);
//! - the action map's toggle types decide which mouse actions are Impulse (decision 0014
//!   D9): with a synthetic table and (with `TESSERA_GAME_DIR`) the install's, one left
//!   click gives a plugin control one `MouseClick`; without action-map data every type is 0,
//!   so the `TapLeft` pulse gives a second one (Known difference 12);
//! - the Interface window's text-size and art-filter actions reach the UI live;
//! - the image LRU and the glyph cache share one S6 total, split by the UI (whole MiB);
//! - a scale-factor change moves the software cursor's physical position.
//!
//! Every fixture is synthetic; the real-data test only reads the install and prints counts.

mod ui_common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use tessera_client::ui_host::fonts::FONT_ENUM;
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_settings::{ArtFilter, CursorMode, TextSource, UiScale};
use tessera_ui::text::UiTextSource;
use ui_common::{Rig, write_plugin};

#[test]
fn ui_settings_reach_the_ui_at_start() {
    let rig = Rig::with_host(
        |_| {},
        |o| {
            let mut o = o.clone();
            o.prefs.art_filter = ArtFilter::Smooth;
            o.prefs.text_scale = 1.2;
            o.prefs.text_source = TextSource::Tessera;
            UiHost::without_install(&o)
        },
    );
    let ui = rig.host.ui();
    assert_eq!(ui.art_filter(), tessera_ui::paint::ArtFilter::Smooth);
    assert!((ui.text_scale() - 1.2).abs() < 1e-9, "{}", ui.text_scale());
    assert_eq!(ui.text_source(), UiTextSource::Tessera);
    drop(ui);
    // The defaults are the identity (§7.1).
    let rig = Rig::new(|_| {});
    let ui = rig.host.ui();
    assert_eq!(ui.art_filter(), tessera_ui::paint::ArtFilter::Sharp);
    assert!((ui.text_scale() - 1.0).abs() < 1e-12);
    assert_eq!(ui.text_source(), UiTextSource::Game);
}

/// The client's copy of `lua-plugin-api.md` Appendix A.2 (the ladders' source, §6.4.2)
/// equals the enum the plugin API gives a plugin.
#[test]
fn font_enum_matches_the_plugin_api() {
    const DUMP: &str = r#"
local rows = {}
for k, v in pairs(Turbine.UI.Lotro.Font) do
    if type(v) == "number" then
        table.insert(rows, k .. "=" .. string.format("%d", v))
    end
end
table.sort(rows)
for _, r in ipairs(rows) do Turbine.Shell.WriteLine("font " .. r) end
Turbine.Shell.WriteLine("fonts done")
"#;
    let mut rig = Rig::new(|p| write_plugin(p, "Check", "Fonts", Some("Fonts"), DUMP));
    rig.attach(&["Fonts"]);
    rig.wait_loaded(1);
    assert!(rig.said("fonts done"), "{:?}", rig.lines());
    let mut got: Vec<String> = rig
        .lines()
        .into_iter()
        .filter_map(|l| l.strip_prefix("font ").map(str::to_owned))
        .collect();
    got.sort();
    let mut want: Vec<String> = FONT_ENUM.iter().map(|(n, d)| format!("{n}={d}")).collect();
    want.sort();
    assert_eq!(got, want);
    rig.shutdown();
}

/// `ui-scaling.md` §6.7: the cursor image is trunc(32 × *Se* × `ui_scale_cursor`) physical
/// pixels and is built again when *Se* changes.
#[test]
fn the_software_cursor_follows_the_scale() {
    let renderer =
        match tessera_render::Renderer::new_headless(&tessera_render::RendererConfig::default()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("no GPU adapter ({e}); skipping");
                return;
            }
        };
    let mut rig = Rig::with_host(
        |_| {},
        |o| {
            let mut o = o.clone();
            o.screen = (1024, 768);
            o.prefs.cursor = CursorMode::Software;
            o.prefs.ui_scale = UiScale::Fixed(1.0);
            o.prefs.scale_cursor = 1.5;
            UiHost::without_install(&o)
        },
    );
    rig.renderer = Some(renderer);
    rig.frame();
    assert_eq!(rig.host.cursor_image(), (48, (0, 0)), "32 × 1 × 1.5");
    // Twice the window at scale 2: Se = 2.
    rig.host.resize(2048, 1536);
    rig.host.apply_uiscale_request(
        &tessera_client::commands::uiscale::UiScaleRequest::Set(2.0),
        std::time::Instant::now(),
    );
    rig.frame();
    assert!((rig.host.scale().effective_scale() - 2.0).abs() < 1e-9);
    assert_eq!(rig.host.cursor_image(), (96, (0, 0)), "32 × 2 × 1.5");
    let ui = rig.host.ui();
    assert_eq!(ui.cursor_physical_size(), 96);
    assert_eq!(ui.cursor_size(), 48, "the tooltip offset stays in UI units");
    drop(ui);
    rig.shutdown();
}

/// render.md §6.16 U2: the renderer's `UiStats::gpu_ns` becomes the frame sample's
/// `ui_gpu_ms`, and the recorded frame carries it.
#[test]
fn the_ui_pass_gpu_time_reaches_the_sample() {
    let mut rig = Rig::new(|_| {});
    rig.host.step8(None, &mut rig.plugins);
    assert_eq!(rig.host.last_sample().ui_gpu_ms, None);
    let mut stats = tessera_render::FrameStats::default();
    stats.ui.gpu_ns = Some(250_000);
    rig.host.record_render_stats(&stats);
    assert_eq!(rig.host.last_sample().ui_gpu_ms, Some(0.25));
    rig.host.end_frame(&rig.plugins, 1.0);
    let rec = rig.host.recorder().expect("recording");
    assert_eq!(
        rec.frames.last().map(|(_, s)| s.ui_gpu_ms),
        Some(Some(0.25))
    );
    // A frame without a figure (no timestamp queries) carries none.
    rig.frame();
    assert_eq!(rig.host.last_sample().ui_gpu_ms, None);
}

/// A plugin control under the pointer that reports its clicks.
const CLICKS: &str = r#"
w = Turbine.UI.Window() w:SetSize(200, 200) w:SetPosition(100, 100) w:SetVisible(true)
c = Turbine.UI.Control() c:SetParent(w) c:SetSize(100, 100) c:SetPosition(0, 0)
c:SetVisible(true)
c.MouseClick = function(sender, args) Turbine.Shell.WriteLine("click " .. tostring(args.Button)) end
Turbine.Shell.WriteLine("clicks ready")
"#;

fn clicks(rig: &Rig) -> usize {
    rig.lines()
        .iter()
        .filter(|l| l.starts_with("click "))
        .count()
}

fn one_click_one_mouse_click(rig: &mut Rig) {
    rig.attach(&["Clicks"]);
    rig.wait_loaded(1);
    assert!(rig.said("clicks ready"), "{:?}", rig.lines());
    rig.click(150.0, 150.0);
    for _ in 0..3 {
        rig.frame();
    }
    assert_eq!(
        clicks(rig),
        1,
        "one click, one MouseClick: {:?}",
        rig.lines()
    );
    assert!(rig.said("click 1"), "left button: {:?}", rig.lines());
}

/// The nine mouse-button actions with their keys and toggle types in the install
/// (`lua-plugin-api.md` §13.12.1 table): a synthetic action map.
const MOUSE_ACTIONS: [(&str, u32, u8); 9] = [
    ("SelectLeft", 0x56, 1),
    ("SelectRight", 0x4C, 1),
    ("SelectMid", 0xA4, 1),
    ("SelectDblLeft", 0x33, 1),
    ("SelectDblRight", 0x2E, 1),
    ("SelectDblMid", 0x93, 1),
    ("TapLeft", 0x32, 2),
    ("TapRight", 0x73, 2),
    ("TapMid", 0x6E, 2),
];

/// Gives the rig's mapper the command IDs and its UI the toggle types of
/// [`MOUSE_ACTIONS`], as the host does from the install's records.
fn synthetic_action_map(rig: &mut Rig) {
    rig.mapper
        .set_command_ids(MOUSE_ACTIONS.iter().map(|(n, id, _)| (*n, *id)));
    rig.host.ui_mut().set_toggle_types(
        MOUSE_ACTIONS
            .iter()
            .map(|(_, id, t)| (*id, *t))
            .collect::<BTreeMap<u32, u8>>(),
    );
}

/// Decision 0014 D9 with synthetic action-map data: `TapLeft` is type 2 (Impulse), so the
/// click's tap pulse gives no button event and one click is one `MouseClick`.
#[test]
fn one_click_is_one_mouse_click_with_a_synthetic_action_map() {
    let mut rig = Rig::new(|p| write_plugin(p, "Check", "Clicks", Some("Clicks"), CLICKS));
    synthetic_action_map(&mut rig);
    one_click_one_mouse_click(&mut rig);
    rig.shutdown();
}

/// Without action-map data every toggle type is 0 (`lua-plugin-api.md` §13.12.1), so the
/// host hard-codes no Impulse action: the `TapLeft` pulse follows the Momentary rules and a
/// left click gives two `MouseClick` events (decision 0014 Known difference 12).
#[test]
fn without_action_map_data_a_tap_pulse_gives_button_events() {
    let mut rig = Rig::new(|p| write_plugin(p, "Check", "Clicks", Some("Clicks"), CLICKS));
    rig.attach(&["Clicks"]);
    rig.wait_loaded(1);
    assert!(rig.said("clicks ready"), "{:?}", rig.lines());
    rig.click(150.0, 150.0);
    for _ in 0..3 {
        rig.frame();
    }
    assert_eq!(clicks(&rig), 2, "SelectLeft and TapLeft: {:?}", rig.lines());
    assert!(
        rig.lines()
            .iter()
            .filter(|l| l.starts_with("click "))
            .all(|l| l == "click 1"),
        "both left: {:?}",
        rig.lines()
    );
    rig.shutdown();
}

/// `ui-scaling.md` §7.5, §7.6 live: the Interface window's text slider (driven through its
/// real scroll bar) and its art-filter action change the UI's text factor and art filter.
#[test]
fn interface_text_and_art_actions_reach_the_ui() {
    use tessera_ui::game::interface::{ArtChoice, InterfaceAction, stop_for_text};
    let mut rig = Rig::new(|_| {});
    rig.host.open_interface();
    let bar = rig
        .host
        .interface_window()
        .and_then(|w| w.text_bar())
        .expect("text slider");
    let stop = stop_for_text(1.25);
    let events = {
        let mut ui = rig.host.ui_mut();
        ui.scrollbar_select_stop(bar, i32::from(stop));
        ui.take_events()
    };
    rig.host.interface_frame(&events, std::time::Instant::now());
    assert!(rig.host.interface_open());
    assert!(
        (rig.host.ui().text_scale() - 1.25).abs() < 1e-9,
        "{}",
        rig.host.ui().text_scale()
    );
    assert!((rig.host.prefs().text_scale - 1.25).abs() < 1e-6);
    let applied = rig.host.apply_interface_action(
        InterfaceAction::SetArt(ArtChoice::Smooth),
        std::time::Instant::now(),
    );
    assert_eq!(applied, tessera_client::ui_host::InterfaceApplied::Other);
    assert_eq!(
        rig.host.ui().art_filter(),
        tessera_ui::paint::ArtFilter::Smooth
    );
    rig.host.apply_interface_action(
        InterfaceAction::SetArt(ArtChoice::Nearest),
        std::time::Instant::now(),
    );
    assert_eq!(
        rig.host.ui().art_filter(),
        tessera_ui::paint::ArtFilter::Nearest
    );
    rig.shutdown();
}

/// §8 S6 (revision 8 answer): the UI splits *T* = `image_lru_mib` × max(1, *Se*²) (glyphs
/// *T* ÷ 8 rounded down to whole MiB), and the host gives the images' share to the
/// environment's LRU at start and after every change of *Se*.
#[test]
fn image_lru_and_glyph_cache_share_the_s6_pool() {
    use tessera_ui::scaling::s6_budget;
    const MIB: u64 = 1 << 20;
    let mut rig = Rig::with_host(
        |_| {},
        |o| {
            let mut o = o.clone();
            o.screen = (1024, 768);
            o.prefs.ui_scale = UiScale::Fixed(1.0);
            // 100 MiB: T ÷ 8 = 12.5 MiB, so the glyph share is 12 whole MiB.
            o.prefs.image_lru_mib = 100;
            UiHost::without_install(&o)
        },
    );
    let lru_now = |rig: &Rig| rig.host.env().map(|h| h.borrow().lru_budget());
    let b = rig.host.ui().s6_budget();
    assert_eq!(b, s6_budget(100, 1.0));
    assert_eq!(b.glyphs, 12 * MIB, "rounded down to whole MiB");
    assert_eq!(b.images, 88 * MIB);
    assert_eq!(lru_now(&rig), Some(b.images));
    assert_eq!(rig.host.ui().glyph_memory().0 as u64, b.glyphs);
    rig.host.resize(2048, 1536);
    rig.host.apply_uiscale_request(
        &tessera_client::commands::uiscale::UiScaleRequest::Set(2.0),
        std::time::Instant::now(),
    );
    rig.frame();
    assert!((rig.host.scale().effective_scale() - 2.0).abs() < 1e-9);
    let b2 = rig.host.ui().s6_budget();
    assert_eq!(b2, s6_budget(100, 2.0));
    assert_eq!(b2.glyphs, 50 * MIB);
    assert_eq!(lru_now(&rig), Some(b2.images));
    // Back to Se = 1: both shares fall again.
    rig.host.resize(1024, 768);
    rig.host.apply_uiscale_request(
        &tessera_client::commands::uiscale::UiScaleRequest::Set(1.0),
        std::time::Instant::now(),
    );
    rig.frame();
    assert_eq!(lru_now(&rig), Some(88 * MIB));
    rig.shutdown();
}

/// A window scale-factor change (render.md §6.11, `keymap.md` §7.7) moves the software
/// cursor's physical position with the mapper's cursor, before the next pointer move.
#[test]
fn a_scale_factor_change_moves_the_software_cursor() {
    let mut rig = Rig::new(|_| {});
    rig.move_to(100.0, 50.0);
    rig.frame();
    assert_eq!(rig.host.pointer_physical(), Some((100.0, 50.0)));
    let timing = rig.mapper.timing();
    rig.host.rescale(&mut rig.mapper, timing, 2.0, (2048, 1536));
    assert_eq!(rig.host.pointer_physical(), Some((200.0, 100.0)));
    assert_eq!(rig.mapper.cursor(), Some((200.0, 100.0)));
    rig.shutdown();
}

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// With the install the action map's toggle types reach the UI (`keymap.md` §12.10, K43:
/// `TapLeft` is type 2), and one click is still one `MouseClick`. Prints counts only.
#[test]
fn one_click_is_one_mouse_click_with_the_install_action_map() {
    let Some(dir) = game_dir() else { return };
    let set = Arc::new(
        tessera_dat::ArchiveSet::open(&dir, tessera_dat::SetOptions::default())
            .expect("archive set"),
    );
    let booted = tessera_client::ui_host::boot::UiBoot::spawn(set, dir, None)
        .expect("spawn")
        .wait()
        .expect("UI data");
    let tap = booted
        .command_ids
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("TapLeft"))
        .map(|(_, id)| *id)
        .expect("TapLeft in the InputActions enum");
    eprintln!(
        "toggle types: {} rows, TapLeft type {:?}",
        booted.toggle_types.len(),
        booted.toggle_types.get(&tap)
    );
    assert!(!booted.toggle_types.is_empty());
    assert_eq!(booted.toggle_types.get(&tap), Some(&2), "K43");
    let booted = std::cell::Cell::new(Some(booted));
    let mut rig = Rig::with_host(
        |p| write_plugin(p, "Check", "Clicks", Some("Clicks"), CLICKS),
        |o: &UiOptions| {
            let o = UiOptions {
                hud: false,
                ..o.clone()
            };
            UiHost::from_boot(booted.take().expect("booted once"), &o)
        },
    );
    one_click_one_mouse_click(&mut rig);
    rig.shutdown();
}
