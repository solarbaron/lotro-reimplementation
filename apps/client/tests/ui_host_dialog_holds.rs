// SPDX-License-Identifier: GPL-3.0-or-later
//! A key held from before a modal dialog through the client's UI host (`ui-widgets.md`
//! §18.4, revision 5, answer 1): the dialog consumes only the releases of keys pressed while
//! it is shown, so a hold from before it stays held while the dialog is shown, over several
//! frames, until its own release, which reaches game code. Keys go through the host's
//! step 1 exactly as the client feeds them (raw events, the walker's input mapper, the UI's
//! dialog key handler).
//!
//! The review finding this covers: `ui_host_dialogs.rs` checked the release but not that
//! the hold stays held while the dialog is shown, so a host that ended the hold when the
//! dialog appeared would have passed. Everything synthetic (no install: the code-made
//! dialogs); no GPU.

mod ui_common;

use tessera_input::{KeyCode, RawEvent};
use tessera_ui::game::chat::notice::NoTables;
use tessera_ui::game::dialog::{DialogArt, DialogBoxes, DialogRefs, DialogRequest, DialogText};
use ui_common::Rig;

fn press(rig: &mut Rig, key: KeyCode, pressed: bool) {
    rig.raw(RawEvent::Key {
        key,
        pressed,
        repeat: false,
    });
}

#[test]
fn a_hold_from_before_a_modal_dialog_stays_held_while_it_is_shown() {
    let mut rig = Rig::new(|_| {});
    let forward = rig
        .mapper
        .registry()
        .action("move_forward")
        .expect("move_forward");
    press(&mut rig, KeyCode::W, true);
    rig.frame();
    assert!(rig.mapper.pressed(forward), "W moves");
    let mut boxes = DialogBoxes::new(DialogRefs::fallback(DialogArt::default()));
    let mut req = DialogRequest::confirmation(8, DialogText::plain("Sure?"));
    req.modal = true;
    {
        let mut ui = rig.host.ui_mut();
        boxes.request(&mut ui, req, &mut NoTables);
    }
    rig.frame();
    assert!(rig.host.modal_dialog_shown(), "a modal dialog is shown");
    assert!(
        rig.mapper.pressed(forward),
        "a hold from before the dialog stays held when the dialog appears"
    );
    for _ in 0..3 {
        rig.frame();
        assert!(rig.host.modal_dialog_shown());
        assert!(
            rig.mapper.pressed(forward),
            "a hold from before the dialog stays held while it is shown"
        );
    }
    press(&mut rig, KeyCode::W, false);
    rig.frame();
    assert!(
        !rig.mapper.pressed(forward),
        "the release of a key held from before the dialog reaches game code"
    );
}
