// SPDX-License-Identifier: GPL-3.0-or-later
//! Engine dialogs through the client's UI host (`ui-widgets.md` §18.4–§18.6): keys go
//! through the host's step 1 exactly as the client feeds them (raw events, the walker's
//! input mapper, the UI's dialog key handler).
//!
//! - A social panel's text-input dialog answered by Enter: the core's `DialogResult` reaches
//!   the social panels through `UiHost::social_frame`, so the entered name becomes the
//!   panel's command (§18.5, §18.6 Tessera rule).
//! - A modal dialog (§18.4): while it is shown no key reaches game code (a movement key is
//!   not pressed in the mapper's snapshot), its own keys still answer it, the release of
//!   the key that closed it is consumed too, and once it is gone keys reach game code
//!   again.
//! - Bindings that are never offered to the UI (no command name: `toggle_mount` on Ctrl+M)
//!   do not fire under a modal dialog either, and a release-class binding of a key pressed
//!   under the dialog does not fire when the key is released after the dialog closed.
//! - A modal dialog shown with `StartInactive` leaves the keyboard focus where it was (a
//!   plugin text box here, standing in for the chat entry): typed text and editing keys do
//!   not reach it while the dialog is shown.
//!
//! Everything synthetic (no install: the code-made dialogs); no GPU.

mod ui_common;

use tessera_client::social::SocialState;
use tessera_client::social::ui::SocialUi;
use tessera_input::{Activation, BindingSpec, Chord, KeyCode, RawEvent};
use tessera_net::EntityId;
use tessera_ui::UiEvent;
use tessera_ui::game::CharVocab;
use tessera_ui::game::chat::notice::NoTables;
use tessera_ui::game::social::SocialVocab;
use tessera_ui::game::social::friends::FriendsModel;
use tessera_ui::widgets::dialog::{DialogArt, DialogBoxes, DialogRefs, DialogRequest, DialogText};
use ui_common::{Rig, write_plugin};

fn entered_state() -> SocialState {
    let mut st = SocialState::new(true);
    let _ = st.apply(
        &tessera_net::Event::WorldEntered {
            player: EntityId(1),
            name: "Tam".into(),
            transform: tessera_net::Transform::new(tessera_net::Vec3::ZERO, 0.0),
        },
        &|_| None,
    );
    st
}

fn press(rig: &mut Rig, key: KeyCode, pressed: bool) {
    rig.raw(RawEvent::Key {
        key,
        pressed,
        repeat: false,
    });
}

#[test]
fn a_dialog_answered_by_enter_reaches_the_social_panels() {
    let mut rig = Rig::new(|_| {});
    let mut social = SocialUi::new(SocialVocab::default(), CharVocab::default());
    let mut state = entered_state();
    {
        let mut ui = rig.host.ui_mut();
        social.open_dialog(&mut ui, FriendsModel::add_dialog(), &mut state);
    }
    rig.host.set_social(social);
    rig.frame();
    assert!(
        rig.host
            .social()
            .and_then(|s| s.dialogs().shown())
            .is_some(),
        "the add dialog is shown"
    );
    rig.type_text("Kim");
    let _ = rig.host.take_game_events();
    rig.key(KeyCode::Enter);
    let events = rig.host.take_game_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, UiEvent::DialogResult { yes: true, .. })),
        "Enter gives the dialog's result: {events:?}"
    );
    let commands = rig.host.social_frame(&mut state, None, None, &events);
    assert_eq!(commands.len(), 1, "{commands:?}");
    assert!(format!("{:?}", commands[0]).contains("Kim"), "{commands:?}");
    assert!(
        rig.host
            .social()
            .and_then(|s| s.dialogs().shown())
            .is_none(),
        "the dialog closed with its result"
    );
}

#[test]
fn a_modal_dialog_keeps_keys_from_game_code() {
    let mut rig = Rig::new(|_| {});
    let forward = rig
        .mapper
        .registry()
        .action("move_forward")
        .expect("move_forward");
    // Control: without a dialog W reaches the walker's movement.
    press(&mut rig, KeyCode::W, true);
    rig.frame();
    assert!(rig.mapper.pressed(forward), "W moves without a dialog");
    press(&mut rig, KeyCode::W, false);
    rig.frame();
    assert!(!rig.mapper.pressed(forward));

    let mut boxes = DialogBoxes::new(DialogRefs::fallback(DialogArt::default()));
    let mut req = DialogRequest::confirmation(7, DialogText::plain("Sure?"));
    req.modal = true;
    {
        let mut ui = rig.host.ui_mut();
        boxes.request(&mut ui, req, &mut NoTables);
    }
    rig.frame();
    assert!(rig.host.modal_dialog_shown(), "a modal dialog is shown");
    // §18.4: no key reaches game code below it.
    press(&mut rig, KeyCode::W, true);
    rig.frame();
    assert!(
        !rig.mapper.pressed(forward),
        "W does not reach game code under a modal dialog"
    );
    press(&mut rig, KeyCode::W, false);
    rig.frame();
    // Its own keys still answer it: Enter is the yes result, given at its press; the
    // release comes after the result closed the dialog and is consumed too.
    let open_chat = rig
        .mapper
        .registry()
        .action("open_chat")
        .expect("open_chat");
    let _ = rig.host.take_game_events();
    press(&mut rig, KeyCode::Enter, true);
    rig.frame();
    let events = rig.host.take_game_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, UiEvent::DialogResult { yes: true, .. })),
        "the dialog's own key answers it: {events:?}"
    );
    {
        let mut ui = rig.host.ui_mut();
        for ev in &events {
            let _ = boxes.handle_event(&mut ui, ev, &mut NoTables);
        }
        boxes.update(&mut ui, &mut NoTables);
    }
    let results = boxes.take_results();
    assert_eq!(results.len(), 1, "one result");
    rig.frame();
    assert!(!rig.host.modal_dialog_shown(), "the dialog is gone");
    let _ = rig.host.take_game_events();
    press(&mut rig, KeyCode::Enter, false);
    rig.frame();
    assert_eq!(
        rig.mapper.state(open_chat).releases,
        0,
        "the release of the closing Enter does not reach game code"
    );
    assert!(!rig.mapper.pressed(open_chat));
    let events = rig.host.take_game_events();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, UiEvent::KeyBroadcast { .. })),
        "nor the key broadcast: {events:?}"
    );
    // Keys reach game code again.
    press(&mut rig, KeyCode::W, true);
    rig.frame();
    assert!(rig.mapper.pressed(forward), "W moves again");
    press(&mut rig, KeyCode::W, false);
    rig.frame();
}

/// A key held from before a modal dialog is not left pressed (§18.4 consumes only the keys
/// pressed while the dialog is shown): the movement it started stops. Tessera reading: as
/// when a text entry takes the keyboard, its game-band hold ends when the dialog appears
/// (the End reaches game code), so its later release has nothing left to end.
#[test]
fn a_key_held_from_before_a_modal_dialog_is_released() {
    let mut rig = Rig::new(|_| {});
    let forward = rig
        .mapper
        .registry()
        .action("move_forward")
        .expect("move_forward");
    press(&mut rig, KeyCode::W, true);
    rig.frame();
    assert!(rig.mapper.pressed(forward));
    let mut boxes = DialogBoxes::new(DialogRefs::fallback(DialogArt::default()));
    let mut req = DialogRequest::confirmation(8, DialogText::plain("Sure?"));
    req.modal = true;
    {
        let mut ui = rig.host.ui_mut();
        boxes.request(&mut ui, req, &mut NoTables);
    }
    rig.frame();
    assert!(rig.host.modal_dialog_shown());
    press(&mut rig, KeyCode::W, false);
    rig.frame();
    assert!(
        !rig.mapper.pressed(forward),
        "the release of a key held from before the dialog reaches game code"
    );
}

/// Shows a modal confirmation dialog through `boxes` and runs a frame.
fn show_modal(rig: &mut Rig, boxes: &mut DialogBoxes, id: u64, start_inactive: bool) {
    let mut req = DialogRequest::confirmation(id, DialogText::plain("Sure?"));
    req.modal = true;
    req.start_inactive = start_inactive;
    {
        let mut ui = rig.host.ui_mut();
        boxes.request(&mut ui, req, &mut NoTables);
    }
    rig.frame();
    assert!(rig.host.modal_dialog_shown(), "a modal dialog is shown");
}

/// Hands the frame's UI events to `boxes` and takes its results.
fn answer(rig: &mut Rig, boxes: &mut DialogBoxes) -> usize {
    let events = rig.host.take_game_events();
    let mut ui = rig.host.ui_mut();
    for ev in &events {
        let _ = boxes.handle_event(&mut ui, ev, &mut NoTables);
    }
    boxes.update(&mut ui, &mut NoTables);
    drop(ui);
    boxes.take_results().len()
}

/// `toggle_mount` (Ctrl+M) has no command name, so the mapper never offers it to the UI;
/// under a modal dialog it still does not fire (§18.4).
#[test]
fn a_binding_without_a_command_does_not_fire_under_a_modal_dialog() {
    let mut rig = Rig::new(|_| {});
    let mount = rig
        .mapper
        .registry()
        .action("toggle_mount")
        .expect("toggle_mount");
    let ctrl_m = |rig: &mut Rig, pressed: bool| {
        if pressed {
            press(rig, KeyCode::ControlLeft, true);
            press(rig, KeyCode::M, true);
        } else {
            press(rig, KeyCode::M, false);
            press(rig, KeyCode::ControlLeft, false);
        }
    };
    // Control: without a dialog Ctrl+M toggles the mount.
    ctrl_m(&mut rig, true);
    rig.frame();
    assert!(
        rig.mapper.state(mount).presses > 0,
        "Ctrl+M without a dialog"
    );
    ctrl_m(&mut rig, false);
    rig.frame();
    let mut boxes = DialogBoxes::new(DialogRefs::fallback(DialogArt::default()));
    show_modal(&mut rig, &mut boxes, 9, false);
    ctrl_m(&mut rig, true);
    rig.frame();
    assert_eq!(
        rig.mapper.state(mount).presses,
        0,
        "Ctrl+M does not reach game code under a modal dialog"
    );
    assert!(!rig.mapper.pressed(mount));
    ctrl_m(&mut rig, false);
    rig.frame();
    // The dialog is answered by Enter and goes; then Ctrl+M works again.
    rig.key(KeyCode::Enter);
    assert_eq!(answer(&mut rig, &mut boxes), 1, "one result");
    rig.frame();
    assert!(!rig.host.modal_dialog_shown());
    ctrl_m(&mut rig, true);
    rig.frame();
    assert!(
        rig.mapper.state(mount).presses > 0,
        "Ctrl+M after the dialog"
    );
    ctrl_m(&mut rig, false);
    rig.frame();
}

/// A release-class binding (a user's or an imported key map) fires at the release; a key
/// (F9: no default binding, so no press-class hold of the key marks it first) pressed
/// under a modal dialog and released after the dialog closed does not fire it
/// (§18.4: the release of every key pressed while the dialog was shown is consumed).
#[test]
fn a_release_binding_of_a_key_pressed_under_a_modal_dialog_does_not_fire() {
    let mut rig = Rig::new(|_| {});
    let mut keymap = rig.mapper.keymap().clone();
    keymap
        .set(
            "movement",
            "toggle_mount",
            vec![BindingSpec::chord(
                Chord::parse("F9")
                    .and_then(|c| c.with_activation(Activation::Release))
                    .expect("binding"),
            )],
        )
        .expect("set");
    let warnings = rig.mapper.set_keymap(&keymap);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mount = rig
        .mapper
        .registry()
        .action("toggle_mount")
        .expect("toggle_mount");
    // Control: without a dialog the release of Q fires it.
    press(&mut rig, KeyCode::F9, true);
    rig.frame();
    press(&mut rig, KeyCode::F9, false);
    rig.frame();
    assert!(rig.mapper.state(mount).presses > 0, "F9's release fires it");
    rig.frame();
    let mut boxes = DialogBoxes::new(DialogRefs::fallback(DialogArt::default()));
    show_modal(&mut rig, &mut boxes, 10, false);
    press(&mut rig, KeyCode::F9, true);
    rig.frame();
    // Enter answers the dialog while F9 is still held.
    rig.key(KeyCode::Enter);
    assert_eq!(answer(&mut rig, &mut boxes), 1, "one result");
    rig.frame();
    assert!(!rig.host.modal_dialog_shown(), "the dialog is gone");
    press(&mut rig, KeyCode::F9, false);
    rig.frame();
    assert_eq!(
        rig.mapper.state(mount).presses,
        0,
        "the release of a key pressed under the dialog is consumed"
    );
    // A new press and release fire it again.
    press(&mut rig, KeyCode::F9, true);
    rig.frame();
    press(&mut rig, KeyCode::F9, false);
    rig.frame();
    assert!(rig.mapper.state(mount).presses > 0, "F9 works again");
}

/// A text box that reports its text on every change.
const TYPED: &str = r#"
local w = Turbine.UI.Window()
w:SetPosition(100, 400)
w:SetSize(300, 60)
w:SetVisible(true)
local t = Turbine.UI.TextBox()
t:SetParent(w)
t:SetPosition(10, 10)
t:SetSize(200, 20)
t:SetVisible(true)
t.TextChanged = function(sender, args)
  Turbine.Shell.WriteLine("typed: [" .. t:GetText() .. "]")
end
"#;

/// A modal dialog with `StartInactive` leaves the keyboard focus in the element below it
/// (the chat entry in the client; a plugin text box here): typed text and the editing keys
/// do not reach that element while the dialog is shown (§18.4), and do again once it is
/// gone.
#[test]
fn typed_text_does_not_reach_an_entry_below_a_start_inactive_modal_dialog() {
    let mut rig = Rig::new(|p| write_plugin(p, "Suite", "Typed", Some("Typed"), TYPED));
    rig.attach(&["Typed"]);
    rig.wait_loaded(1);
    rig.click(150.0, 420.0);
    rig.type_text("a");
    assert!(rig.said("typed: [a]"), "{:?}", rig.lines());
    let mut boxes = DialogBoxes::new(DialogRefs::fallback(DialogArt::default()));
    show_modal(&mut rig, &mut boxes, 11, true);
    assert!(
        rig.host.keyboard_focused(),
        "the StartInactive dialog left the focus in the text box"
    );
    rig.type_text("b");
    rig.key(KeyCode::Backspace);
    rig.frame();
    assert!(
        !rig.lines()
            .iter()
            .any(|l| l == "typed: [ab]" || l == "typed: []"),
        "nothing typed under the modal dialog reached the text box: {:?}",
        rig.lines()
    );
    // The dialog goes (its owner dismisses it); typing reaches the text box again.
    {
        let mut ui = rig.host.ui_mut();
        boxes.dismiss(&mut ui, &mut NoTables);
        boxes.update(&mut ui, &mut NoTables);
    }
    rig.frame();
    assert!(!rig.host.modal_dialog_shown(), "the dialog is gone");
    rig.type_text("c");
    assert!(rig.said("typed: [ac]"), "{:?}", rig.lines());
}
