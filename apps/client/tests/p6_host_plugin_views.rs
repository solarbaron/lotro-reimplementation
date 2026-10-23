// SPDX-License-Identifier: GPL-3.0-or-later
//! The P6 progression views (`lua-plugin-api.md` §15.6) through the production wiring of the
//! game UI host: `UiHost::set_plugin_progression` before `UiHost::attach_plugins` wraps the
//! plugins' local player (after the wallet and mount views) so that a synthetic plugin reads
//! `GetVocation` and `GetProfessionInfo` from the session's model, and the model's events
//! reach it through the host's frame steps. Every name and number is invented; no install is
//! read.

mod ui_common;

use tessera_client::progression::{ApiKeys, P6Data, P6State, SessionProgression};
use tessera_net::domain::{ProgressionEvent, Ref};
use tessera_net::{DataId, EntityId, Event, Transform};
use ui_common::{Rig, write_plugin};

const READER: &str = r#"
local p = Turbine.Gameplay.LocalPlayer.GetInstance()
Attr = p:GetAttributes()
Log = {}
Attr.VocationChanged = function() Log[#Log + 1] = "vocation " .. Attr:GetVocation() end
"#;

fn eval(rig: &Rig, src: &str) -> String {
    rig.plugins
        .manager()
        .with_lua("Default", |lua| lua.load(src).eval::<String>())
        .expect("apartment exists")
        .expect("evaluates")
}

#[test]
fn the_host_hands_the_progression_views_to_the_plugins() {
    let mut rig = Rig::new(|plugins| write_plugin(plugins, "T", "Reader", None, READER));
    let model = SessionProgression::new();
    rig.host.set_plugin_progression(model.source());
    rig.host.set_character(&mut rig.plugins, Some("Cato"));
    rig.attach(&["Reader"]);
    rig.wait_loaded(1);
    assert_eq!(
        eval(&rig, "return tostring(T.Reader.Attr:GetVocation())"),
        "0"
    );

    let mut keys = ApiKeys::default();
    keys.professions.insert(0x7000_1000, 9);
    keys.vocations.insert(0x7000_2000, 3);
    let data = P6Data::fallback().with_keys(keys);
    let mut state = P6State::new();
    let rec = |d| Ref::Record(DataId(d));
    let mut changes = Vec::new();
    for ev in [
        Event::WorldEntered {
            player: EntityId(1),
            name: "Cato".into(),
            transform: Transform::default(),
        },
        Event::Progression(ProgressionEvent::ProfessionChanged {
            profession: rec(0x7000_1000),
            tier: 3,
            proficiency: 120,
            proficiency_max: 400,
            mastery: 60,
            mastery_max: 800,
            mastery_tier: Some(2),
        }),
        Event::Progression(ProgressionEvent::VocationChanged {
            vocation: Some(rec(0x7000_2000)),
            slots: Some(3),
        }),
    ] {
        state.apply(&ev, &mut changes);
    }
    model.apply(&changes, &state, &data, None, rig.plugins.manager_mut());
    let delivered = rig.frames_until(200, |r| {
        eval(r, "return table.concat(T.Reader.Log, '|')") == "vocation 3"
    });
    assert!(
        delivered,
        "{}",
        eval(&rig, "return table.concat(T.Reader.Log, '|')")
    );
    let read = eval(
        &rig,
        "local i = T.Reader.Attr:GetProfessionInfo(9)
         return table.concat({ T.Reader.Attr:GetVocation(), i:GetProficiencyLevel(),
             i:GetProficiencyExperience(), i:GetMasteryLevel() }, ' ')",
    );
    assert_eq!(read, "3 3 120 2");
    rig.shutdown();
}
