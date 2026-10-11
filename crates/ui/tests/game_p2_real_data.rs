// SPDX-License-Identifier: GPL-3.0-or-later
//! The P2 combat widgets against a user's own install (read-only): the vocabulary of
//! `combat-ui.md` resolves by label; the panels P4 (`AvatarPanel`), P5 (`OpponentPanel`),
//! E7 (`GamePlay_SelectionTarget`), E11 and E12 (the progress-bar drag boxes) open from
//! their layouts and every combat widget takes its install look (TR41: no fallback while
//! the install's elements exist); effect icons, floating texts, the selection display and
//! the defeat panel are created from `effects`, `floatytext` and `Dialog`. Only counts and
//! labels are printed; no game data leaves the install.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-ui --test game_p2_real_data -- --nocapture`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record, UiLayout};
use tessera_props::PropertyRegistry;
use tessera_ui::game::combat::*;
use tessera_ui::game::{CharVocab, GameVocab};
use tessera_ui::{Ui, UiEnv, Vocab};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;

struct InstallEnv {
    set: Arc<ArchiveSet>,
    dict: Arc<PropertyDictionary>,
    cache: Mutex<HashMap<u32, Option<Arc<UiLayout>>>>,
}

impl UiEnv for InstallEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        let mut c = self.cache.lock().ok()?;
        c.entry(did.0)
            .or_insert_with(|| {
                let payload = self.set.read(did.0).ok()?;
                UiLayout::parse_expecting(&payload, did.0, &*self.dict)
                    .ok()
                    .map(Arc::new)
            })
            .clone()
    }
}

fn did_map(set: &ArchiveSet, did: Did) -> Option<DataIdMap> {
    let payload = set.read(did.0).ok()?;
    DataIdMap::parse_expecting(&payload, did).ok()
}

#[test]
fn p2_widgets_on_the_install() {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return;
        }
    };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let dict = {
        let payload = set.read(PROPERTY_DICTIONARY).expect("read the dictionary");
        PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dictionary")
    };
    let registry = PropertyRegistry::from_dictionary(dict.clone());
    let dict = Arc::new(dict);
    let root = did_map(&set, Did(ROOT_DID_MAP)).expect("root DID map");
    let emapper = root
        .by_label("EMAPPER")
        .and_then(|d| did_map(&set, d))
        .expect("EMAPPER map");
    let labels = UiLabels::load(
        |label| emapper.by_label(label),
        |did| {
            let payload = set.read(did.0).ok()?;
            EnumMapper::parse_expecting(&payload, did).ok()
        },
    );
    let uilayout = root
        .by_label("UILAYOUT")
        .and_then(|d| did_map(&set, d))
        .expect("UILAYOUT map");
    let mut vocab = Vocab::default();
    vocab.resolve_with(
        &mut |n| registry.pid(n),
        &mut |n| labels.key(UiEnum::Message, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |_| None,
    );
    let mut cv = CombatVocab::default();
    cv.resolve_all(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| registry.pid(n),
    );
    let unresolved = |entries: Vec<(&'static str, u32)>| {
        entries
            .into_iter()
            .filter(|(_, v)| *v >= 0xFFFA_0000)
            .map(|(l, _)| l)
            .collect::<Vec<_>>()
    };
    let (ui_ids, ui_states, ui_pids) = (
        unresolved(cv.ids.entries()),
        unresolved(cv.states.entries()),
        unresolved(cv.pids.entries()),
    );
    println!(
        "combat vocabulary: {} element IDs ({} unresolved: {ui_ids:?}), {} states ({} unresolved: {ui_states:?}), {} properties ({} unresolved: {ui_pids:?})",
        cv.ids.entries().len(),
        ui_ids.len(),
        cv.states.entries().len(),
        ui_states.len(),
        cv.pids.entries().len(),
        ui_pids.len()
    );
    assert!(ui_ids.is_empty(), "element IDs do not resolve: {ui_ids:?}");
    // The text properties and the `PipDisplay` type resolve with the rest (no hand copy).
    let text = unresolved(cv.text.entries());
    let types = unresolved(cv.types.entries());
    assert!(text.is_empty(), "text properties do not resolve: {text:?}");
    assert!(types.is_empty(), "element types do not resolve: {types:?}");

    let env = InstallEnv {
        set: set.clone(),
        dict: dict.clone(),
        cache: Mutex::new(HashMap::new()),
    };
    let mut ui = Ui::new(Box::new(env), vocab, (1920, 1080));
    let mut gv = GameVocab::default();
    gv.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| registry.pid(n),
    );
    gv.register_classes(&mut ui);
    let mut chv = CharVocab::default();
    chv.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| registry.pid(n),
    );
    chv.register_classes(&mut ui);
    let layout = |l: &str| uilayout.by_label(l);
    let toolbar = layout("toolbar").expect("toolbar layout");
    let activatables = layout("gameplayactivatables").expect("gameplayactivatables layout");
    let open = |ui: &mut Ui, did: Did, id: u32, what: &str| match ui.open_panel(did, id) {
        Ok(k) => Some(k),
        Err(e) => {
            println!("{what} not opened: {e}");
            None
        }
    };
    let roots = CombatRoots {
        avatar_panel: open(&mut ui, toolbar, cv.ids.avatar_panel, "AvatarPanel"),
        opponent_panel: open(&mut ui, toolbar, cv.ids.opponent_panel, "OpponentPanel"),
        selection_target: open(
            &mut ui,
            activatables,
            cv.ids.selection_target,
            "GamePlay_SelectionTarget",
        ),
        player_progress: open(
            &mut ui,
            activatables,
            cv.ids.progress_dragbox,
            "progress drag box",
        ),
        opponent_progress: open(
            &mut ui,
            activatables,
            cv.ids.opponent_progress_dragbox,
            "opponent progress drag box",
        ),
        toolbar: open(&mut ui, toolbar, cv.ids.toolbar_field, "ToolbarField"),
    };
    let layouts = CombatLayouts {
        floatytext: layout("floatytext"),
        effects: layout("effects"),
        dialog: layout("Dialog"),
    };
    println!(
        "layouts: floatytext {}, effects {}, Dialog {}",
        layouts.floatytext.is_some(),
        layouts.effects.is_some(),
        layouts.dialog.is_some()
    );
    let mut c = CombatUi::new(&mut ui, cv, layouts, roots, FloatySettings::default());
    assert_eq!(c.pip_display_type, cv.types.pip_display);
    let looks = [
        (
            "player frame",
            c.player.as_ref().is_some_and(|f| !f.is_fallback()),
        ),
        (
            "target frame",
            c.target.as_ref().is_some_and(|f| !f.is_fallback()),
        ),
        (
            "selection's target",
            c.selection_target
                .as_ref()
                .is_some_and(|f| !f.is_fallback()),
        ),
        (
            "player cast bar",
            c.player_cast.as_ref().is_some_and(|b| !b.is_fallback()),
        ),
        (
            "target cast bar",
            c.target_cast.as_ref().is_some_and(|b| !b.is_fallback()),
        ),
        (
            "player effects",
            c.player_effects.as_ref().is_some_and(|b| !b.is_fallback()),
        ),
        (
            "target effects",
            c.target_effects.as_ref().is_some_and(|b| !b.is_fallback()),
        ),
        (
            "floating text",
            c.floaty.as_ref().is_some_and(|f| !f.is_fallback()),
        ),
        (
            "selection display",
            c.marker.as_ref().is_some_and(|m| !m.is_fallback()),
        ),
        (
            "defeat panel",
            c.defeat.as_ref().is_some_and(|d| !d.is_fallback()),
        ),
        (
            "combat state",
            c.combat_state.as_ref().is_some_and(|d| !d.is_fallback()),
        ),
    ];
    let fallback: Vec<&str> = looks
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(w, _)| *w)
        .collect();
    println!(
        "install looks: {} of {}; fallback: {fallback:?}",
        looks.len() - fallback.len(),
        looks.len()
    );
    assert!(
        fallback.is_empty(),
        "widgets in their fallback look: {fallback:?}"
    );

    // A short combat run (§17.1 shapes) creating the template instances.
    let player = FrameView {
        entity: 1,
        name: "P".into(),
        level: LevelShown::Level(7),
        has_vitals: true,
        health: Some(Vital::new(825.0, 900.0)),
        power: Some(Vital::new(400.0, 400.0)),
        ..FrameView::default()
    };
    c.set_player(&mut ui, &player);
    let boar = FrameView {
        entity: 2,
        name: "B".into(),
        has_vitals: true,
        name_colour: Some(NameColour::Con(ConRank::Tough)),
        health: Some(Vital::new(990.0, 1200.0)),
        level: LevelShown::Level(7),
        ..FrameView::default()
    };
    c.set_target(
        &mut ui,
        Some(&boar),
        Some(SelectionView {
            health: Some(Vital::new(3.0, 10.0)),
            power: Some(Vital::new(7.0, 10.0)),
            ..SelectionView::default()
        }),
    );
    let mp = ui.vocab().pids.meter_position;
    let pos = |ui: &Ui, k: Option<tessera_ui::ElementKey>| k.map(|k| ui.prop(k, mp).float());
    let block = c
        .marker
        .as_ref()
        .and_then(|m| m.elements().0)
        .expect("E3 block");
    let below =
        |ui: &Ui, root: tessera_ui::ElementKey, id: u32| tessera_ui::game::find_below(ui, root, id);
    // E3 mini vitals (TR8): health and power on their own meters, temporary meters at 0.
    let (mh, mpow, mt) = (
        pos(&ui, below(&ui, block, cv.ids.opponent_health_meter)),
        pos(&ui, below(&ui, block, cv.ids.opponent_power_meter)),
        pos(&ui, below(&ui, block, cv.ids.opponent_temp_health)),
    );
    println!("E3 meters: health {mh:?}, power {mpow:?}, temporary health {mt:?}");
    assert!(mh.is_some_and(|p| (p - 0.3).abs() < 1e-4));
    assert!(mpow.is_some_and(|p| (p - 0.7).abs() < 1e-4));
    assert!(mt.is_none_or(|p| p == 0.0));
    // E7 (TR18): the selection's target's health on `VitalCurrentHealthMeter`.
    let wolf = FrameView {
        entity: 3,
        name: "W".into(),
        has_vitals: true,
        health: Some(Vital::new(5.0, 10.0)),
        ..FrameView::default()
    };
    c.set_selection_target(&mut ui, Some(&wolf));
    let e7 = c
        .selection_target
        .as_ref()
        .map(UnitFrame::root)
        .expect("E7");
    let e7h = pos(&ui, below(&ui, e7, cv.ids.selection_target_health));
    println!("E7 health meter {e7h:?}");
    assert!(e7h.is_some_and(|p| (p - 0.5).abs() < 1e-4));
    // E20 (TR11): the P3 pip display of the resource's type is the one shown.
    if let Some(t) = roots.toolbar {
        let pips = tessera_ui::game::subtree(&ui, t)
            .into_iter()
            .filter(|k| ui.element_type(*k) == Some(cv.types.pip_display))
            .count();
        let mut with_pips = player.clone();
        with_pips.resource = Some(ClassResource::Pips {
            pip_type: 1,
            value: Vital::new(2.0, 5.0),
        });
        c.set_player(&mut ui, &with_pips);
        let shown = tessera_ui::game::subtree(&ui, t)
            .into_iter()
            .filter(|k| ui.element_type(*k) == Some(cv.types.pip_display) && ui.is_visible(*k))
            .count();
        println!("pip displays under P3: {pips}, shown for type 1: {shown}");
        assert!(pips > 0, "no PipDisplay under P3");
        assert_eq!(shown, 1);
        c.set_player(&mut ui, &player);
    }
    c.cast_started(
        &mut ui,
        CastOwner::Player,
        &CastView {
            label: "J".into(),
            interruptible: true,
            cancellable: true,
            duration: 0.8,
        },
        2.2,
    );
    c.effect_applied(
        EffectOwner::Player,
        EffectView {
            instance: 1,
            start: 3.4,
            duration: Some(20.0),
            ..EffectView::default()
        },
    );
    c.floating_text(2, FloatyText::new("-210", 0xFFF7_E4A9, true));
    c.set_in_combat(&mut ui, true);
    let anchor = |_e: EntityId| {
        Some(ScreenAnchor {
            x: 960.0,
            y: 500.0,
            distance: 8.0,
            on_screen: true,
            drawn: true,
        })
    };
    c.update(&mut ui, 3.5, &anchor);
    // E11 (TR27): only the plain bar is displayed (the player's cast still fills); the
    // mounted and music variants were hidden (after their 0.5 s auto-fade).
    ui.set_time(3.5);
    ui.frame_work();
    if let Some(r) = roots.player_progress {
        for (id, want) in [
            (cv.ids.progress_bar, true),
            (cv.ids.progress_bar_mounted, false),
            (cv.ids.progress_bar_music, false),
        ] {
            let k = below(&ui, r, id).expect("progress bar variant");
            assert_eq!(ui.is_visible(k), want, "progress bar variant {id:#x}");
        }
    }
    let icon = c.player_effects.as_ref().and_then(|b| b.icon(1));
    println!(
        "effect icon {}, floating texts shown {}, defeat panel {}",
        icon.is_some(),
        c.floaty.as_ref().map_or(0, |f| f.shown().len()),
        c.player_defeated(&mut ui)
    );
    assert!(icon.is_some(), "TimedEffectTemplate not created");
    assert_eq!(c.floaty.as_ref().map_or(0, |f| f.shown().len()), 1);
    assert!(c.defeat.as_ref().and_then(DefeatPanel::panel).is_some());
    // TR39: the player's defeat ended and hid E11.
    assert!(c.player_cast.as_ref().is_some_and(|b| b.cast().is_none()));

    // Widget work per frame (ARCHITECTURE §15, decision 0015 U1): a busy fight with 16
    // effects, a cast and a stream of floating texts. Printed always, asserted only with
    // `TESSERA_PERF=1` (docs/DEVELOPMENT.md).
    for i in 0..16u64 {
        let owner = if i % 2 == 0 {
            EffectOwner::Player
        } else {
            EffectOwner::Target
        };
        c.effect_applied(
            owner,
            EffectView {
                instance: 100 + i,
                harmful: i % 3 == 0,
                start: 3.5,
                duration: Some(10.0 + i as f64),
                ..EffectView::default()
            },
        );
    }
    let frames = 600;
    let start = std::time::Instant::now();
    for f in 0..frames {
        let now = 4.0 + f64::from(f) / 60.0;
        if f % 6 == 0 {
            c.floating_text(
                2,
                FloatyText::new(format!("-{f}"), 0xFFF7_E4A9, f % 30 == 0),
            );
        }
        if f % 90 == 0 {
            c.cast_started(
                &mut ui,
                CastOwner::Target,
                &CastView {
                    label: "G".into(),
                    interruptible: true,
                    cancellable: false,
                    duration: 1.5,
                },
                now,
            );
        }
        ui.set_time(now);
        c.update(&mut ui, now, &anchor);
    }
    let mean_ms = start.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
    println!("combat widget update: mean {mean_ms:.4} ms per frame over {frames} frames");
    if std::env::var_os("TESSERA_PERF").is_some_and(|v| v == "1") {
        assert!(mean_ms < 0.3, "mean {mean_ms} ms over the 0.3 ms U1 mean");
    }
}
