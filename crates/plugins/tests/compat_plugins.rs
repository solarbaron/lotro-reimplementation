// SPDX-License-Identifier: GPL-3.0-or-later
//! M5 exit criterion 4 with synthetic plugins (`plugin-api-usage.md` §6, §7, §9).
//!
//! The owner has no `Plugins/` folder yet, so the 14 distributions of
//! `plugin-api-usage.md` §9.2 cannot be run here; criterion 4 waits for the owner's
//! install (§9.5). These tests run a suite of UI-only plugins written for Tessera
//! (`tests/fixtures/compat/`, never copied from a published plugin) that reproduce the
//! usage patterns of §6 and the shapes of the §9 set, and check that each loads, creates
//! its windows and responds to synthetic input.
//!
//! | Synthetic plugin | Shape (§9.2) | Exercises |
//! |---|---|---|
//! | `AtlasMain`, `AtlasSearch` | #1 | two apartments, `LoadPlugin` from a button, stretch 1 and 2 with `.tga`/`.jpg` files, label markup, tree view, context menu with a sub-menu, alias quickslot, `Update` timer, key broadcast, chat listener, slash command, account data |
//! | `RefA`, `RefB` | #6, #7 | one shared apartment, options panels, `UIScaleChanged`, character/server data with completion handlers, list boxes, drag-and-drop info, a shared handler table on `Turbine.Chat.Received` |
//! | `Tunes` | #3 | default apartment, list selection, mouse wheel, game-styled scroll bar, `DragLeave`/`DragDrop`, alias quickslot, account/character data |
//! | `Watch`, `WatchReloader` | #4 | stretch 1–3 with image DIDs, tree with a sort method, context menu with check items, reloader helper (`UnloadScriptState`/`LoadPlugin` from `Update`), gameplay view guarded |
//! | `Ledger` | #5 | label markup, fonts, colours, stretch 1, alias quickslot running the plugin's own command, account/server data |
//! | `Stable`, `Waypoint` | #8 | two authors in one archive, `SetRotation`, stretch 2, aliases running a command the other plugin registers |
//! | `Herd1`–`Herd5` | #11 | five descriptors in the default apartment, `GoldWindow`, `Lotro.DragDropInfo` |
//! | `Needle` | #12 | computed imports, `SetRotation` about x and z, alias quickslot, a plugin-defined handler-table helper, class-level `Turbine.Plugin.Unload`, `.tga` chosen by language |
//! | `Digest` | #13 | all three data scopes, label markup, image DIDs, `.tga` and `.jpg`, tree view, double clicks |
//! | `Viewer` | #14 | a grid of image DIDs in a list box, stretch 1 and 2, an import done on demand, a large data table |
//! | `Classy` | class-helper users (§6.5) | `class()` subclasses of game-styled classes, `import "Turbine.UI.Extensions"`, `Closing` cancel |
//!
//! # Incompatibilities found (Tessera, 2026-10-08)
//!
//! None of the synthetic plugins fails to load or to respond. The differences below are
//! what a plugin can observe or a user can see; impl-client-host copies this table into
//! the ROADMAP status.
//!
//! | # | Area | Difference | Plugins affected | Spec |
//! |---|---|---|---|---|
//! | 1 | Game-styled controls | Built from the install's layout (`UILAYOUT` key 0x100000C8): `Lotro.Window`, `GoldWindow`, `Lotro.Button`, `GoldButton`, `CheckBox` and `TextBox` from their base elements, `Lotro.ScrollBar` with the layout's scroll bar as a part filling the control (rebuilt when the orientation changes). Not built: the scaled-window variants (marking a window does not rebuild it). Tessera's neutral art (frame, thumb) only when the layout is missing; then the other classes have no art | every plugin using `Turbine.UI.Lotro` controls | LPA §14.2.1, §14.2.6 OPEN |
//! | 2 | Title layout | The caption width is the laid-out caption's width on one line, 0 while the fonts are not loaded (the title is not laid out again when they arrive); the title text's four margins are not set to 0 | game-styled windows | LPA §14.2.3, §14.2.6 OPEN |
//! | 3 | Check boxes | The layout's check mark (0x100025B0) shows the state through its highlighted state; with the neutral fallback no box or mark is drawn | `Lotro.CheckBox` users | LPA §14.2.2, §14.2.6 OPEN |
//! | 4 | Quickslots | The shortcut's icon is not drawn; only `Alias` shortcuts execute (as chat commands through the client); other types are stored and returned | all quickslot users | PU §7.1, LPA §14.6 |
//! | 5 | Text | Markup, fonts, colours, alignment and wrapping are written as element properties; how they look depends on the client's text shaper; a `Label` bound to a scroll bar does not scroll its text | label users | LPA §13.4, §13.7 |
//! | 6 | List boxes | Layout follows `ui-widgets.md` §8.3 with `Orientation`/`MaxItemsPerLine` mapped by a Tessera reading (OPEN in LPA §13.7); default orientation vertical; items are selected by a left press | list users | LPA §13.7 |
//! | 7 | Trees | One column, 20-pixel indent by default, a press selects and toggles a node, filter and sort run on `Refresh`/`SetFilter`/`SetSortMethod` only | tree users | LPA §13.7 |
//! | 8 | Context menus | Drawn with neutral art, rows 20 pixels high, width estimated from the text length; sub-menus open on a click | menu users | LPA §13.10 |
//! | 9 | Event delivery | Events of another apartment caused during a synchronous delivery (§9.7) wait for the client's next delivery | plugins whose actions activate or deactivate another apartment's window | LPA §9.7 |
//! | 10 | `Color` | Fields written by plugin code are stored as Lua numbers, not rounded to single precision | plugins comparing written colour fields | LPA §13.9 |
//! | 11 | Fonts | `SetFont` accepts any DID with the font prefix 0x42 (records are not loaded to check) | font users | LPA §13.4 |
//! | 12 | Gameplay | `LocalPlayer.GetInstance()` is nil without a local-player source, so plugins must guard it (the gameplay layer's rule, not this module's) | #4, #5, #6, #10, #12, #13 shapes | LPA §15 |
//! | 13 | Input | The neutral frame's drag bar and resize handle take the mouse in their area; `MouseHover` repeats every tick; wheel and key rules of decision 0014 apply | all | LPA §13.12, 0014 |
//! | 14 | Title bar | The title bar and drag bar sit at top −8 (§14.2.3) but are drawn into the window's own surface, which covers only the window's rectangle, so their top 8 rows are cut off (`tessera-ui` has no canvas over children above the top edge) | game-styled windows | LPA §14.2.3, §14.2.6 OPEN, decision 0015 |
//! | 15 | Text boxes | `tessera-ui` reports no caret or selection: the layer estimates the caret as the end of each user edit, so `GetSelectionStart`/`InsertText` follow typing but not caret moves without an edit, and `SetSelection*` moves no visible caret or highlight; `SetReadOnly(true)` takes user edits back after the UI has applied them (no `TextChanged`) | text box users | LPA §13.4 OPEN, §13.5 |
//! | 16 | Control lists | `ControlList:Insert(i, c)` and `Set(i, c)` re-parent `c` as the last child, so `Get(i)` is not `c` afterwards | plugins that reorder children through `GetControls()` | LPA §13.7 OPEN |
//! | 17 | Game-object controls | The tier-2 members (`SetQuantity`, `SetEquipmentSlot`, `SetSelectionEnabled`, `SetItem`, …) ignore their arguments and the getters return fixed defaults | tier-2 users | LPA §14.5, §17.1 |
//! | 18 | Lifetime | A hidden window that plugin code dropped is kept until unload when one of its descendants' tables refers back to it (typically a handler closure that captures the window): Lua 5.1 has no ephemeron tables, and the parent keeps its children through a weak-keyed table | plugins that create a fresh dialog per use and drop it | LPA §8.2 OPEN |

#[path = "fixtures/harness.rs"]
mod harness;

use harness::Fx;

/// A fixture with the whole synthetic suite and its generated images and data.
fn suite() -> Fx {
    let mut fx = Fx::new();
    fx.copy_fixture("compat");
    fx.file("Tess/Atlas/Resources/map.tga", harness::tga(256, 256));
    fx.file("Tess/Atlas/Resources/tile.jpg", harness::jpeg(64, 64));
    fx.file("Tess/Refs/Art/row.tga", harness::tga(32, 32));
    fx.file("Tess/Ledger/back.tga", harness::tga(32, 32));
    fx.file("Tess/Stable/compass.tga", harness::tga(32, 32));
    fx.file("Tess/Needle/Images/dial_en.tga", harness::tga(128, 128));
    fx.file("Tess/Digest/header.jpg", harness::jpeg(390, 40));
    fx.file("Tess/Digest/row.tga", harness::tga(8, 8));
    // A large data table (`plugin-api-usage.md` §6.12 shape), generated here.
    let mut data = String::from("-- SPDX-License-Identifier: GPL-3.0-or-later\nImageIds = {\n");
    for i in 0..100_000u32 {
        data.push_str(&format!("0x{:08X},\n", 0x4100_0000 + i));
    }
    data.push_str("}\n");
    fx.file("Tess/Viewer/Data.lua", data);
    fx.refresh();
    fx
}

fn visible_windows(fx: &Fx, apartment: &str) -> usize {
    let ui = fx.pui.ui().borrow();
    fx.pui
        .windows(apartment)
        .into_iter()
        .filter(|k| ui.is_visible(*k))
        .count()
}

#[test]
fn map_style_plugin_and_its_helper() {
    let mut fx = suite();
    fx.namespace("Atlas", "Tess.Atlas");
    fx.load("AtlasMain");
    assert_eq!(visible_windows(&fx, "Atlas"), 1);
    assert!(fx.chat().iter().any(|l| l.contains("loaded")));
    // Update timer and the key broadcast.
    fx.frame(0.016);
    fx.frame(0.016);
    let ticks: f64 = fx.eval("Atlas", "return state.ticks");
    assert!(ticks >= 2.0);
    fx.key("ToggleBags", 0x1000_0094);
    assert_eq!(visible_windows(&fx, "Atlas"), 0);
    fx.key("ToggleBags", 0x1000_0094);
    assert_eq!(visible_windows(&fx, "Atlas"), 1);
    // Right click on the stretched map opens the menu; the sub-menu item fires.
    let (mx, my) = fx.centre("Atlas", "map");
    fx.click(mx, my, 2);
    fx.click(mx + 10, my + 30, 1); // row 2: "Zoom" (sub-menu)
    fx.click(mx + 130, my + 50, 1); // its row 2 (the sub-menu opens beside row 2): "x2"
    let picks: String = fx.eval(
        "Atlas",
        "return table.concat(state.menu, ',') .. ' ' .. settings.zoom",
    );
    assert_eq!(picks, "x2 2");
    // The alias quickslot runs the plugin's own slash command.
    let (qx, qy) = fx.centre("Atlas", "slot");
    fx.click(qx, qy, 1);
    for line in fx.pui.take_chat_commands() {
        assert!(fx.manager.execute_command(&line), "{line}");
    }
    let centered: f64 = fx.eval("Atlas", "return state.centered");
    assert_eq!(centered, 1.0);
    // A chat line reaches the listener.
    fx.manager
        .dispatch_chat("Someone", "hello", tessera_plugins::CHAT_TYPE_STANDARD);
    fx.manager.pump();
    let chat: f64 = fx.eval("Atlas", "return state.chat");
    assert_eq!(chat, 1.0);
    // The Search button loads the helper into its own apartment (deferred work).
    let (bx, by) = fx.centre("Atlas", "searchButton");
    fx.click(bx, by, 1);
    fx.frame(0.016);
    fx.namespace("AtlasSearch", "Tess.Atlas");
    assert_eq!(visible_windows(&fx, "AtlasSearch"), 1);
    let (gx, gy) = fx.centre("AtlasSearch", "go");
    fx.click(gx, gy, 1);
    for _ in 0..8 {
        fx.frame(0.016);
    }
    let found: f64 = fx.eval("AtlasSearch", "return found");
    assert_eq!(found, 5.0);
    // Settings are saved on unload.
    fx.manager.unload_all();
    fx.assert_no_errors();
}

#[test]
fn shared_apartment_reference_windows() {
    let mut fx = suite();
    fx.namespace("Refs", "Tess.Refs");
    fx.load("RefA");
    fx.load("RefB");
    fx.manager.run_deferred_until_idle(10);
    fx.deliver();
    assert_eq!(visible_windows(&fx, "Refs"), 2);
    let r: (f64, f64, bool) = fx.eval(
        "Refs",
        "return refA.window.list:GetItemCount(), refB.window.list:GetItemCount(), refA.loaded == false",
    );
    assert_eq!(r, (12.0, 0.0, true));
    // Options panels of both plugins (§6.5).
    for name in ["RefA", "RefB"] {
        assert!(fx.manager.options_panel(name).is_some(), "{name}");
    }
    // UIScaleChanged reaches the marked window through its own group (group 72 after
    // `RegisterForGlobalScaling`). The `Global` group (60) is fixed at 1.0 for plugins and
    // fires nothing (decision 0040, `ui-scaling.md` §7.7).
    let ev = fx.pui.scale_changed(&fx.manager, 60, 1.25);
    fx.client.extend(ev);
    let unchanged: Option<f64> = fx.eval("Refs", "return refA.scale");
    assert_eq!(unchanged, None, "no event, so refA.scale is still nil");
    let ev = fx.pui.scale_changed(&fx.manager, 72, 1.25);
    fx.client.extend(ev);
    let s: f64 = fx.eval("Refs", "return refA.scale");
    assert_eq!(s, 1.25);
    // The filter box takes typing.
    let (tx, ty) = fx.centre("Refs", "refB.filter");
    // A click gives the text box focus (LPA §13.1 Tessera rule); no `Focus()` call.
    fx.click(tx, ty, 1);
    fx.type_char('a');
    let t: String = fx.eval("Refs", "return refB.filter:GetText()");
    assert_eq!(t, "a");
    fx.manager.dispatch_chat("", "system line", 0);
    fx.manager.pump();
    let lines: f64 = fx.eval("Refs", "return refB.lines");
    assert_eq!(lines, 1.0);
    fx.manager.unload_all();
    fx.assert_no_errors();
}

#[test]
fn song_list_in_the_default_apartment() {
    let mut fx = suite();
    fx.namespace("Default", "Tess.Tunes");
    fx.load("Tunes");
    assert_eq!(visible_windows(&fx, "Default"), 1);
    // Select the second row and run its alias.
    let (lx, ly, _, _) = fx.screen_box("Default", "tunes.list");
    fx.click(lx + 20, ly + 45, 1);
    let sel: f64 = fx.eval("Default", "return tunes.selected");
    assert_eq!(sel, 2.0);
    let (qx, qy) = fx.centre("Default", "tunes.slot");
    fx.click(qx, qy, 1);
    for line in fx.pui.take_chat_commands() {
        fx.manager.execute_command(&line);
    }
    let played: String = fx.eval("Default", "return table.concat(tunes.played, ',')");
    assert_eq!(played, "Rain");
    // The wheel over a row fires its handler and scrolls the list.
    fx.wheel(lx + 20, ly + 10, false);
    let w: f64 = fx.eval("Default", "return tunes.wheel");
    assert_eq!(w, -1.0);
    fx.assert_no_errors();
}

#[test]
fn alerts_with_reloader() {
    let mut fx = suite();
    fx.namespace("Watch", "Tess.Watch");
    fx.load("Watch");
    assert_eq!(visible_windows(&fx, "Watch"), 1);
    // The tree is sorted: "Chat" before "Combat".
    let first: String = fx.eval("Watch", "return watch.tree:GetNodes():Get(1).group");
    assert_eq!(first, "Chat");
    // Click the first node: selected and expanded.
    let (nx, ny) = fx.centre("Watch", "watch.tree:GetNodes():Get(1)");
    fx.click(nx, ny, 1);
    let r: (bool, bool) = fx.eval(
        "Watch",
        "local n = watch.tree:GetNodes():Get(1) return watch.selected == n, n:IsExpanded()",
    );
    assert_eq!(r, (true, true));
    // Menu: Options > Sound toggles the check mark.
    let (wx, wy, _, _) = fx.screen_box("Watch", "watch.window");
    // Below the tree, on the window's own area: the menu opens at the pointer.
    fx.click(wx + 150, wy + 250, 2);
    fx.click(wx + 160, wy + 260, 1); // row 1: Options (sub-menu)
    fx.click(wx + 150 + 130, wy + 260, 1); // its row 1: Sound
    let sound: bool = fx.eval("Watch", "return watch.sound == false");
    assert!(sound, "the sound item was clicked and unchecked");
    // Menu: Reload runs the reloader helper, which unloads and reloads Watch.
    fx.click(wx + 150, wy + 250, 2);
    fx.click(wx + 160, wy + 280, 1); // row 2: Reload
    for _ in 0..10 {
        fx.frame(0.016);
    }
    assert!(fx.manager.apartment_names().iter().any(|a| a == "Watch"));
    let idle: bool = fx
        .manager
        .with_lua("WatchReloader", |lua| {
            lua.load("return Tess.Watch.reloader:GetWantsUpdates() == false")
                .eval::<bool>()
        })
        .unwrap()
        .unwrap();
    assert!(idle, "the reloader finished its three steps");
    assert_eq!(
        visible_windows(&fx, "Watch"),
        1,
        "the reloaded plugin made a new window"
    );
    fx.assert_no_errors();
}

#[test]
fn ledger_and_its_alias() {
    let mut fx = suite();
    fx.namespace("Ledger", "Tess.Ledger");
    fx.load("Ledger");
    let (qx, qy) = fx.centre("Ledger", "ledger.slot");
    fx.click(qx, qy, 1);
    fx.click(qx, qy, 1);
    for line in fx.pui.take_chat_commands() {
        fx.manager.execute_command(&line);
    }
    let r: (f64, String) = fx.eval("Ledger", "return ledger.entries, ledger.log:GetText()");
    assert_eq!(r.0, 2.0);
    assert!(r.1.contains("<rgb=#FFD700>2</rgb>"));
    let (cx, cy) = fx.centre("Ledger", "ledger.clear");
    fx.click(cx, cy, 1);
    let r: (f64, String) = fx.eval("Ledger", "return ledger.entries, ledger.log:GetText()");
    assert_eq!(r, (0.0, String::new()));
    fx.manager.unload_all();
    fx.assert_no_errors();
}

#[test]
fn travel_aliases_reach_another_plugins_command() {
    let mut fx = suite();
    fx.namespace("Stable", "Tess.Stable");
    fx.namespace("Waypoint", "Other.Waypoint");
    fx.load("Waypoint");
    fx.load("Stable");
    assert_eq!(visible_windows(&fx, "Stable"), 1);
    let rot: f64 = fx.eval("Stable", "return stable.window:GetRotation().z");
    assert_eq!(rot, 45.0);
    let (qx, qy) = fx.centre("Stable", "stable.slots[2]");
    fx.click(qx, qy, 1);
    for line in fx.pui.take_chat_commands() {
        assert!(fx.manager.execute_command(&line));
    }
    let v: String = fx.eval("Waypoint", "return table.concat(waypoint.visits, ',')");
    assert_eq!(v, "mill");
    fx.assert_no_errors();
}

#[test]
fn five_descriptors_in_the_default_apartment() {
    let mut fx = suite();
    fx.namespace("Default", "Tess.Herd");
    for i in 1..=5 {
        fx.load(&format!("Herd{i}"));
    }
    assert_eq!(visible_windows(&fx, "Default"), 5);
    let n: f64 = fx.eval("Default", "return #herd.windows");
    assert_eq!(n, 5.0);
    // Clicking a lower window raises it above the others.
    let (x, y) = fx.centre("Default", "herd.windows[1]");
    fx.click(x - 60, y, 1);
    fx.assert_no_errors();
}

#[test]
fn heading_dial() {
    let mut fx = suite();
    fx.namespace("Default", "Tess.Needle");
    fx.load("Needle");
    fx.frame(0.016);
    let r: (f64, f64) = fx.eval("Default", "return needle.updates, needle.callbacks");
    assert_eq!(r, (1.0, 1.0), "both entries of the handler table ran");
    let (qx, qy) = fx.centre("Default", "needle.slot");
    fx.click(qx, qy, 1);
    for line in fx.pui.take_chat_commands() {
        fx.manager.execute_command(&line);
    }
    let h: (f64, f64) = fx.eval(
        "Default",
        "return needle.heading, needle.window:GetRotation().z",
    );
    assert_eq!(h, (90.0, 90.0));
    fx.manager.unload_all();
    fx.assert_no_errors();
}

#[test]
fn digest_with_three_scopes() {
    let mut fx = suite();
    fx.namespace("Default", "Tess.Digest");
    fx.load("Digest");
    assert_eq!(visible_windows(&fx, "Default"), 1);
    let (nx, ny) = fx.centre("Default", "digest.tree:GetNodes():Get(1)");
    fx.click(nx, ny, 1); // expands
    let (lx, ly) = fx.centre(
        "Default",
        "digest.tree:GetNodes():Get(1):GetChildNodes():Get(1)",
    );
    fx.move_to(lx, ly);
    let h = fx.press(lx, ly);
    fx.release(h, lx, ly);
    fx.manager.unload_all();
    // A second session reads the character value back.
    fx.load("Digest");
    let seen: f64 = fx.eval("Default", "return digest.character.seen");
    assert_eq!(seen, 2.0);
    fx.assert_no_errors();
}

#[test]
fn image_browser_with_a_large_table() {
    let mut fx = suite();
    fx.namespace("Viewer", "Tess.Viewer");
    fx.load("Viewer");
    let n: f64 = fx.eval("Viewer", "return viewer.grid:GetItemCount()");
    assert_eq!(n, 24.0);
    let (bx, by) = fx.centre("Viewer", "viewer.next");
    fx.click(bx, by, 1);
    let r: (f64, bool, f64) = fx.eval(
        "Viewer",
        "return viewer.page, ExtraLoaded == true, viewer.grid:GetItem(1).did",
    );
    assert_eq!(r, (1.0, true, f64::from(0x4100_0000u32 + 24)));
    // Pick the 8th cell (second row of six).
    let (gx, gy, _, _) = fx.screen_box("Viewer", "viewer.grid");
    fx.click(gx + 64 + 10, gy + 64 + 10, 1);
    let picked: f64 = fx.eval("Viewer", "return viewer.picked");
    assert_eq!(picked, f64::from(0x4100_0000u32 + 24 + 7));
    fx.assert_no_errors();
}

#[test]
fn class_helper_users() {
    let mut fx = suite();
    fx.namespace("Classy", "Tess.Classy");
    fx.load("Classy");
    assert_eq!(visible_windows(&fx, "Classy"), 1);
    let (bx, by) = fx.centre("Classy", "classy.panel.button");
    fx.click(bx, by, 1);
    fx.click(bx, by, 1);
    let r: (f64, String) = fx.eval(
        "Classy",
        "return classy.clicks, classy.panel.button:GetText()",
    );
    assert_eq!(r, (2.0, "Count 2".into()));
    // Closing is cancelled while keepOpen is set.
    let _: () = fx.eval("Classy", "classy.keepOpen = true classy.panel:Close()");
    assert_eq!(visible_windows(&fx, "Classy"), 1);
    let _: () = fx.eval("Classy", "classy.keepOpen = false classy.panel:Close()");
    assert_eq!(visible_windows(&fx, "Classy"), 0);
    fx.assert_no_errors();
}

/// Every plugin of the suite together, as a user with all of them installed would run
/// them: all load, every window shows, a frame of input and ticks runs without errors.
#[test]
fn whole_suite_together() {
    let mut fx = suite();
    let names: Vec<String> = fx
        .manager
        .available_plugins()
        .iter()
        .map(|p| p.descriptor.name.clone())
        .filter(|n| n != "WatchReloader" && n != "AtlasSearch")
        .collect();
    assert_eq!(names.len(), 17, "{names:?}");
    for n in &names {
        fx.load(n);
    }
    for _ in 0..5 {
        fx.frame(0.016);
    }
    for ap in [
        "Atlas", "Refs", "Watch", "Ledger", "Stable", "Viewer", "Classy",
    ] {
        assert!(visible_windows(&fx, ap) >= 1, "{ap}");
    }
    assert!(visible_windows(&fx, "Default") >= 8);
    fx.click(800, 450, 1);
    fx.wheel(800, 450, true);
    fx.key("ToggleBags", 0x1000_0094);
    fx.manager.unload_all();
    fx.deliver();
    fx.assert_no_errors();
    assert!(fx.pui.ui().borrow().element_count() < 10);
}
