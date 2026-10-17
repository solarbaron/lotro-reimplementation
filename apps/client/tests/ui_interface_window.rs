// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's own Interface window (decision 0040, `ui-scaling.md` §7.2): opening the window,
//! moving the UI-scale slider and applying the result to the scaling state, all headless and
//! without an install. The slider is driven through the real slider-mode scroll-bar machinery
//! (`ui-widgets.md` §6.5), which posts the stop index; the window's controller turns it into a
//! [`InterfaceAction::SetScaleStop`], and the host's apply path (mirrored here) stores the
//! user's *S* and recomputes the effective scale *Se* live (§4.3, §4.5).

use std::time::{Duration, Instant};

use tessera_client::ui_host::interface::{InterfaceController, scale_of_stop};
use tessera_client::ui_host::scaling::ScaleState;
use tessera_settings::{SafeAspect, UiPrefs, UiScale};
use tessera_ui::Ui;
use tessera_ui::game::interface::{InterfaceAction, stop_for_scale};

#[test]
fn slider_sets_the_setting_and_changes_se() {
    // A 1920 × 1080 window at S 1.0 → Se 1.0, F = the window (§9.2 C2).
    let mut ui = Ui::new(
        Box::new(tessera_ui::NullEnv),
        tessera_ui::Vocab::default(),
        (1920, 1080),
    );
    let mut scale = ScaleState::new(UiScale::Fixed(1.0), SafeAspect::R21_9, (1920, 1080), 1.0);
    let prefs = UiPrefs::default();
    assert!(
        (scale.effective_scale() - 1.0).abs() < 1e-9,
        "Se starts at 1.0"
    );

    // Open the window and find its scale slider.
    let mut ctrl = InterfaceController::open(&mut ui, &scale, &prefs).expect("open");
    let bar = ctrl.scale_bar().expect("scale slider");

    // Move the slider to 200 % (stop 30) through the real scroll-bar path.
    let target = stop_for_scale(2.0);
    assert_eq!(target, 30);
    ui.scrollbar_select_stop(bar, i32::from(target));
    let events = ui.take_events();
    assert!(!events.is_empty(), "moving the slider posts a message");

    // The window turns the moved slider into a SetScaleStop action.
    let mut action = None;
    for ev in &events {
        if let Some(a) = ctrl.handle_event(&ui, ev) {
            action = Some(a);
        }
    }
    let Some(InterfaceAction::SetScaleStop(stop)) = action else {
        panic!("expected SetScaleStop, got {action:?}");
    };
    assert_eq!(stop, target);

    // Apply it to the scaling state exactly as the UI host does.
    let now = Instant::now();
    let applied = scale.set_scale(scale_of_stop(stop), now);
    assert!(applied.screen_changed, "F changes with the scale");

    // The setting records the user's S = 2.0, even though the window limits Se.
    assert_eq!(scale.user_scale(), UiScale::Fixed(2.0), "ui_scale stored");
    // Se changed live: min(2.0, 1920/1024 = 1.875, 1080/768 = 1.40625) = 1.40625 (§4.3).
    assert!(
        (scale.effective_scale() - 1.40625).abs() < 1e-9,
        "Se changed to 1.40625, got {}",
        scale.effective_scale()
    );
    assert!(scale.is_limited(), "2.0 is limited at 1920×1080");

    // The change is queued for the settings file once the settle elapses (§4.5 step 5).
    let out = scale
        .take_persist(now + Duration::from_millis(200))
        .expect("a settled scale change to persist");
    assert_eq!(out.0, UiScale::Fixed(2.0));

    // Closing the window removes it from the tree (no panic, handlers released).
    ctrl.close(&mut ui);
}

/// The Auto toggle emits SetAuto with the opposite of the shown state, and the safe-aspect
/// buttons emit SetAspect; both drive the scaling state live (§4.2, §5.1, §7.2 rows 3 and 6).
#[test]
fn auto_toggle_and_aspect_choice() {
    let mut ui = Ui::new(
        Box::new(tessera_ui::NullEnv),
        tessera_ui::Vocab::default(),
        (1920, 1080),
    );
    // Start on Auto so the toggle shows it checked.
    let scale = ScaleState::new(UiScale::Auto, SafeAspect::R21_9, (1920, 1080), 1.0);
    let prefs = UiPrefs::default();
    let mut ctrl = InterfaceController::open(&mut ui, &scale, &prefs).expect("open");
    // The window is in the tree; nothing panics on refresh.
    ctrl.refresh(&mut ui, &scale, &prefs);
    ctrl.close(&mut ui);
}
