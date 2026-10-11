// SPDX-License-Identifier: GPL-3.0-or-later
//! The native panels on synthetic layouts (no install): opening and showing panels, the
//! Escape rule of `game-panels.md` §16 (GP15, GP16, the sequence of §13.7), Tessera's own
//! plugin manager window and the P12 layout view. (The chat window's lines go to the
//! client's command dispatcher, decision 0020: `tests/panels_review.rs` and the
//! dispatcher's own tests.)

use std::cell::RefCell;
use std::rc::Rc;

use tessera_client::panels::{
    NativePanels, NoMaps, PanelCatalog, PanelId, PanelRef, PluginControl, PluginRow, Services,
};
use tessera_formats::ui_layout::{ElementDesc, Rect};
use tessera_formats::{Did, TSize, UiLayout};
use tessera_ui::events::ControlEvent;
use tessera_ui::game::escape::EscapeOutcome;
use tessera_ui::game::{GameLayouts, GameVocab};
use tessera_ui::{ElementKey, MemoryEnv, Ui, UiEvent, Vocab};

const LAYOUT: Did = Did(0x2200_0001);
const FIELD: u32 = 14;
const TREE: u32 = 10;

fn element(id: u32, ty: u32, (x, y, w, h): (i32, i32, i32, i32)) -> ElementDesc {
    let mut d = ElementDesc {
        element_id: id,
        element_type: ty,
        ..ElementDesc::default()
    };
    d.block.rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    d.block.has_position = true;
    d
}

/// Panels: main menu (P6), quest log (P9), toolbar (P3, HUD), plugin manager (P12, with a
/// Tree), options (P7).
fn fixture() -> (Ui, NativePanels) {
    let mut pm = element(0x1000_0004, FIELD, (0, 0, 300, 200));
    pm.children
        .push(element(0x1000_0040, TREE, (10, 10, 200, 150)));
    let layout = UiLayout {
        did: LAYOUT,
        design_width: 1024,
        design_height: 768,
        elements_size: TSize::default(),
        elements: vec![
            element(0x1000_0001, FIELD, (400, 250, 230, 280)),
            element(0x1000_0002, FIELD, (10, 100, 700, 500)),
            element(0x1000_0003, FIELD, (0, 568, 1024, 200)),
            pm,
            element(0x1000_0005, FIELD, (200, 100, 500, 500)),
        ],
    };
    let mut env = MemoryEnv::default();
    env.add_layout(layout);
    let ui = Ui::new(Box::new(env), Vocab::default(), (1024, 768));
    let r = |e| PanelRef {
        layout: LAYOUT,
        element_id: e,
    };
    let catalog = PanelCatalog::from_refs([
        (PanelId::MainMenu, r(0x1000_0001)),
        (PanelId::QuestLog, r(0x1000_0002)),
        (PanelId::Toolbar, r(0x1000_0003)),
        (PanelId::PluginManager, r(0x1000_0004)),
        (PanelId::Options, r(0x1000_0005)),
    ]);
    let panels = NativePanels::new(catalog, GameVocab::default(), GameLayouts::default());
    (ui, panels)
}

/// A plugin host recording its calls.
#[derive(Clone, Default)]
struct FakePlugins {
    rows: Rc<RefCell<Vec<PluginRow>>>,
    calls: Rc<RefCell<Vec<String>>>,
}

impl FakePlugins {
    fn with(names: &[&str]) -> Self {
        let p = Self::default();
        for n in names {
            p.rows.borrow_mut().push(PluginRow {
                name: (*n).to_string(),
                version: "1.0".to_string(),
                author: "someone".to_string(),
                description: format!("{n} plugin"),
                apartment: format!("{n}Apt"),
                loaded: false,
            });
        }
        p
    }
}

impl PluginControl for FakePlugins {
    fn rows(&self) -> Vec<PluginRow> {
        self.rows.borrow().clone()
    }

    fn load(&mut self, name: &str) -> Result<(), String> {
        self.calls.borrow_mut().push(format!("load {name}"));
        let mut rows = self.rows.borrow_mut();
        let r = rows
            .iter_mut()
            .find(|r| r.name == name)
            .ok_or_else(|| "unknown".to_string())?;
        r.loaded = true;
        Ok(())
    }

    fn unload(&mut self, apartment: Option<&str>) -> Result<(), String> {
        self.calls
            .borrow_mut()
            .push(format!("unload {}", apartment.unwrap_or("*")));
        for r in self.rows.borrow_mut().iter_mut() {
            if apartment.is_none_or(|a| a == r.apartment) {
                r.loaded = false;
            }
        }
        Ok(())
    }

    fn refresh(&mut self) {
        self.calls.borrow_mut().push("refresh".to_string());
    }
}

macro_rules! services {
    ($maps:ident, $plugins:ident) => {
        Services {
            maps: &mut $maps,
            plugins: &mut $plugins,
        }
    };
}

#[test]
fn panels_open_show_and_hide() {
    let (mut ui, mut panels) = fixture();
    let (mut maps, mut plugins) = (NoMaps, FakePlugins::default());
    let mut sv = services!(maps, plugins);
    let k = panels.open(&mut ui, PanelId::QuestLog, &mut sv).unwrap();
    let b = ui.layout_box(k).unwrap();
    assert_eq!((b.l, b.t, b.w(), b.h()), (10, 100, 700, 500));
    assert!(panels.is_shown(&ui, PanelId::QuestLog));
    assert_eq!(
        ui.active_panel(),
        Some(k),
        "an on-demand panel becomes active"
    );
    // Opening again reuses the root.
    assert_eq!(panels.open(&mut ui, PanelId::QuestLog, &mut sv).unwrap(), k);
    assert!(!panels.toggle(&mut ui, PanelId::QuestLog, &mut sv).unwrap());
    assert!(!panels.is_shown(&ui, PanelId::QuestLog));
    assert!(panels.toggle(&mut ui, PanelId::QuestLog, &mut sv).unwrap());
    // Not in the catalogue.
    assert!(panels.open(&mut ui, PanelId::Social, &mut sv).is_err());
    // The chat window needs a setup.
    assert!(panels.open(&mut ui, PanelId::Chat, &mut sv).is_err());
    // HUD without chat: only the toolbar is in this catalogue.
    let hud = panels.open_hud(&mut ui, &mut sv);
    assert!(hud.iter().all(|(id, _)| *id != PanelId::Chat));
    assert!(
        hud.iter()
            .any(|(id, r)| *id == PanelId::Toolbar && r.is_ok())
    );
    panels.destroy(&mut ui, PanelId::QuestLog);
    assert_eq!(panels.root(PanelId::QuestLog), None);
    assert!(!ui.exists(k));
}

#[test]
fn escape_rule_gp15_gp16() {
    let (mut ui, mut panels) = fixture();
    let (mut maps, mut plugins) = (NoMaps, FakePlugins::default());
    let mut sv = services!(maps, plugins);
    let toolbar = panels.open(&mut ui, PanelId::Toolbar, &mut sv).unwrap();
    // §13.7 step 1: nothing open: Escape opens and activates P6.
    let o = panels.escape(&mut ui, true, false, &mut sv);
    let EscapeOutcome::ShowedMainMenu(menu) = o else {
        panic!("{o:?}");
    };
    assert_eq!(panels.root(PanelId::MainMenu), Some(menu));
    assert_eq!(ui.active_panel(), Some(menu));
    // Releases and auto-repeats do nothing.
    assert_eq!(
        panels.escape(&mut ui, false, false, &mut sv),
        EscapeOutcome::Ignored
    );
    assert_eq!(
        panels.escape(&mut ui, true, true, &mut sv),
        EscapeOutcome::Ignored
    );
    // Step 2: P6 active: hidden.
    let o = panels.escape(&mut ui, true, false, &mut sv);
    assert_eq!(o, EscapeOutcome::HidActive(menu));
    assert!(!panels.is_shown(&ui, PanelId::MainMenu));
    // Step 3: P9 open and active: hidden, the HUD untouched.
    let quest = panels.open(&mut ui, PanelId::QuestLog, &mut sv).unwrap();
    assert_eq!(
        panels.escape(&mut ui, true, false, &mut sv),
        EscapeOutcome::HidActive(quest)
    );
    assert!(ui.is_visible(toolbar));
    // The toolbar (HUD) is active: rule 2 shows P6 again.
    ui.activate(toolbar);
    let o = panels.escape(&mut ui, true, false, &mut sv);
    assert_eq!(o, EscapeOutcome::ShowedMainMenu(menu));
    assert!(ui.is_visible(toolbar));
    // A shown P6 that is not active is hidden by rule 2.
    ui.activate(toolbar);
    assert_eq!(
        panels.escape(&mut ui, true, false, &mut sv),
        EscapeOutcome::HidMainMenu(menu)
    );
    // GP15: no outcome is a quit; ten more presses keep alternating.
    for _ in 0..10 {
        let o = panels.escape(&mut ui, true, false, &mut sv);
        assert!(matches!(
            o,
            EscapeOutcome::ShowedMainMenu(_)
                | EscapeOutcome::HidActive(_)
                | EscapeOutcome::HidMainMenu(_)
        ));
    }
    assert!(ui.is_visible(toolbar));
}

/// `combat-ui.md` TR5 asks step 1 of the Escape rule: is the *active* panel a visible
/// on-demand panel (the main menu P6 included)?
#[test]
fn step_one_finds_only_an_active_on_demand_panel() {
    let (mut ui, mut panels) = fixture();
    let (mut maps, mut plugins) = (NoMaps, FakePlugins::default());
    let mut sv = services!(maps, plugins);
    let toolbar = panels.open(&mut ui, PanelId::Toolbar, &mut sv).unwrap();
    ui.activate(toolbar);
    assert!(!panels.active_on_demand(&ui), "a HUD panel is active");
    // P6 open and active: step 1 hides it, so the combat step must not run.
    let EscapeOutcome::ShowedMainMenu(menu) = panels.escape(&mut ui, true, false, &mut sv) else {
        panic!("P6 not shown");
    };
    assert_eq!(ui.active_panel(), Some(menu));
    assert!(panels.active_on_demand(&ui), "P6 counts as on-demand");
    // P6 shown but not active: step 1 finds nothing.
    ui.activate(toolbar);
    assert!(!panels.active_on_demand(&ui));
    // P9 shown but not active: still nothing; active: found.
    let quest = panels.open(&mut ui, PanelId::QuestLog, &mut sv).unwrap();
    ui.activate(toolbar);
    assert!(!panels.active_on_demand(&ui), "shown is not active");
    ui.activate(quest);
    assert!(panels.active_on_demand(&ui));
}

#[test]
fn plugin_manager_layout_view_fills_the_tree() {
    let (mut ui, mut panels) = fixture();
    let (mut maps, mut plugins) = (NoMaps, FakePlugins::with(&["Alpha", "Beta", "Gamma"]));
    let calls = Rc::clone(&plugins.calls);
    let mut sv = services!(maps, plugins);
    let root = panels.open_plugin_manager(&mut ui, &mut sv).unwrap();
    assert_eq!(panels.root(PanelId::PluginManager), Some(root));
    assert!(!panels.manager.is_own());
    assert_eq!(panels.manager.rows().count(), 3);
    let rows = panels.manager.row_elements();
    let tree = ui.parent(rows[0]).expect("rows in the tree");
    assert_eq!(ui.list_entries(tree), rows);
    // Click selects, double-click loads.
    let click = |k: ElementKey| UiEvent::Control {
        control: k,
        event: ControlEvent::MouseClick {
            button: 1,
            x: 1,
            y: 1,
        },
    };
    panels.handle_event(&mut ui, &click(rows[1]), &mut sv);
    assert_eq!(
        panels.manager.selected().map(|r| r.name.as_str()),
        Some("Beta")
    );
    let dbl = UiEvent::Control {
        control: rows[1],
        event: ControlEvent::MouseDoubleClick {
            button: 1,
            x: 1,
            y: 1,
        },
    };
    panels.handle_event(&mut ui, &dbl, &mut sv);
    assert_eq!(calls.borrow().as_slice(), ["load Beta"]);
    // Rows were rebuilt with the new state and the selection kept.
    assert!(panels.manager.selected().is_some_and(|r| r.loaded));
    assert_eq!(panels.manager.rows().count(), 3);
}

#[test]
fn plugin_manager_own_window() {
    let (mut ui, mut panels) = fixture();
    let mut plugins = FakePlugins::with(&["Alpha", "Beta"]);
    let w = panels.manager.open_own(&mut ui, &plugins).unwrap();
    assert!(panels.manager.is_own());
    assert_eq!(panels.manager.root(), Some(w));
    assert!(ui.is_visible(w));
    let rows = panels.manager.row_elements();
    assert_eq!(rows.len(), 2);
    let click = |k: ElementKey| UiEvent::Control {
        control: k,
        event: ControlEvent::MouseClick {
            button: 1,
            x: 1,
            y: 1,
        },
    };
    // The buttons are the window's last four children before nothing else: find them by
    // their text.
    let mut buttons = Vec::new();
    for c in ui.control_children(w) {
        let t = ui.text_of(c);
        if ["Load", "Unload", "Refresh", "Close"].contains(&t.as_str()) {
            buttons.push((t, c));
        }
    }
    let button = |n: &str| {
        buttons
            .iter()
            .find(|(t, _)| t == n)
            .map(|(_, k)| *k)
            .unwrap()
    };
    // Load with nothing selected does nothing.
    assert!(
        panels
            .manager
            .handle_event(&mut ui, &click(button("Load")), &mut plugins)
    );
    assert!(plugins.calls.borrow().is_empty());
    assert!(
        panels
            .manager
            .handle_event(&mut ui, &click(rows[0]), &mut plugins)
    );
    panels
        .manager
        .handle_event(&mut ui, &click(button("Load")), &mut plugins);
    panels
        .manager
        .handle_event(&mut ui, &click(button("Unload")), &mut plugins);
    panels
        .manager
        .handle_event(&mut ui, &click(button("Refresh")), &mut plugins);
    assert_eq!(
        plugins.calls.borrow().as_slice(),
        ["load Alpha", "unload AlphaApt", "refresh"]
    );
    panels
        .manager
        .handle_event(&mut ui, &click(button("Close")), &mut plugins);
    assert!(!ui.is_visible(w));
    // Reopening shows the same window.
    assert_eq!(panels.manager.open_own(&mut ui, &plugins).unwrap(), w);
    assert!(ui.is_visible(w));
    // Unrelated events are not the manager's.
    assert!(!panels.manager.handle_event(
        &mut ui,
        &UiEvent::ScreenResized {
            width: 1,
            height: 1
        },
        &mut plugins
    ));
}
