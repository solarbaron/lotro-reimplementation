// SPDX-License-Identifier: GPL-3.0-or-later
//! Lifetime, bookkeeping and cost of plugin UI objects (the lifetime rules of the
//! `tessera_plugins::ui` module documentation, `lua-plugin-api.md` §8.2 OPEN, §9.4, §13.7;
//! decision 0015 §7): what survives the collection of a parent, the per-apartment counts,
//! hostile values and the cost of the per-frame work with many objects. Synthetic plugins
//! written for these tests.
//!
//! The timing assertions run only with `TESSERA_PERF=1` (release build, idle machine,
//! decision 0010); without it the tests check the behaviour and print the times.

#[path = "fixtures/harness.rs"]
mod harness;

use std::time::{Duration, Instant};

use harness::Fx;

fn fx_with(source: &str) -> Fx {
    let mut fx = Fx::new();
    fx.plugin("Test", "Probe", Some("Probe"), source);
    fx.refresh();
    fx.load("Probe");
    fx
}

fn eval<R: tessera_plugins::mlua::FromLuaMulti>(fx: &Fx, code: &str) -> R {
    fx.eval("Probe", code)
}

/// Collect garbage fully and run a frame (the reap happens at the next entry).
fn collect(fx: &mut Fx) {
    for _ in 0..3 {
        let _: () = eval(fx, "collectgarbage('collect') collectgarbage('collect')");
        fx.frame(0.016);
    }
}

fn perf() -> bool {
    std::env::var("TESSERA_PERF").as_deref() == Ok("1")
}

fn timed(label: &str, limit: Duration, f: impl FnOnce()) {
    let t = Instant::now();
    f();
    let e = t.elapsed();
    eprintln!("{label}: {e:?} (indicative)");
    if perf() {
        assert!(e <= limit, "{label} took {e:?}, budget {limit:?}");
    }
}

/// A subscriber whose parent window is collected keeps receiving `Update`: it is detached,
/// not destroyed with the window.
#[test]
fn a_subscriber_outlives_its_collected_parent() {
    let mut fx = fx_with(
        "N = 0 \
         do local w = Turbine.UI.Window() local c = Turbine.UI.Control() c:SetParent(w) \
           c.Update = function() N = N + 1 end c:SetWantsUpdates(true) end",
    );
    collect(&mut fx);
    let before: f64 = eval(&fx, "return N");
    for _ in 0..6 {
        fx.frame(0.016);
    }
    let after: f64 = eval(&fx, "return N");
    assert!(after >= before + 6.0, "Update stopped: {before} -> {after}");
    assert_eq!(
        fx.pui.object_count("Probe"),
        1,
        "only the subscriber remains"
    );
    fx.assert_no_errors();
}

/// A child that plugin code still references survives the collection of its parent.
#[test]
fn a_referenced_child_outlives_its_collected_parent() {
    let mut fx = fx_with(
        "do local w = Turbine.UI.Window() keep = Turbine.UI.Label() keep:SetParent(w) \
           local gone = Turbine.UI.Label() gone:SetParent(w) end",
    );
    assert_eq!(fx.pui.object_count("Probe"), 3);
    collect(&mut fx);
    assert_eq!(fx.pui.object_count("Probe"), 1);
    let r: (String, bool) = eval(
        &fx,
        "keep:SetText('x') return keep:GetText(), keep:GetParent() == nil",
    );
    assert_eq!(r, ("x".into(), true));
    fx.assert_no_errors();
}

/// The text of children destroyed with their parent leaves the apartment's text budget.
#[test]
fn text_of_reaped_children_is_given_back() {
    let mut fx = fx_with(
        "big = string.rep('t', 1048576) \
         function round() local w = Turbine.UI.Window() for i = 1, 4 do \
           local l = Turbine.UI.Label() l:SetParent(w) l:SetText(big) end end",
    );
    // 4 MiB per round: past 64 MiB after 16 rounds if the text leaked.
    for _ in 0..24 {
        let r: Result<(), _> = fx.try_eval("Probe", "round()");
        assert!(r.is_ok(), "{r:?}");
        collect(&mut fx);
    }
    assert_eq!(fx.pui.object_count("Probe"), 0);
    fx.assert_no_errors();
}

/// Reaping many collected controls costs time linear in their number: a window with
/// 8,000 children and 8,000 parentless controls, collected while 8,000 others live.
#[test]
fn reaping_is_linear() {
    let mut fx = fx_with("");
    let _: () = eval(
        &fx,
        "win = Turbine.UI.Window() for i = 1, 8000 do local c = Turbine.UI.Control() c:SetParent(win) end \
         loose = {} for i = 1, 8000 do loose[i] = Turbine.UI.Control() end \
         others = {} for i = 1, 8000 do others[i] = Turbine.UI.Control() end",
    );
    assert_eq!(fx.pui.object_count("Probe"), 24_001);
    let _: () = eval(&fx, "win = nil loose = nil");
    let mut spent = Duration::ZERO;
    for _ in 0..3 {
        let _: () = eval(&fx, "collectgarbage('collect') collectgarbage('collect')");
        let t = Instant::now();
        fx.deliver();
        spent += t.elapsed();
    }
    eprintln!("reap of 16,001 controls among 24,001: {spent:?} (indicative)");
    if perf() {
        assert!(spent <= Duration::from_millis(50), "{spent:?}");
    }
    assert_eq!(fx.pui.object_count("Probe"), 8000);
}

/// One `Update` tick with many subscribers and no handlers costs time linear in their
/// number; the §9.4 rules (added subscribers called in the same tick, removed ones
/// skipped) hold.
#[test]
fn update_broadcast_is_linear_and_keeps_its_rules() {
    let fx = fx_with(
        "subs = {} for i = 1, 8000 do local c = Turbine.UI.Control() c:SetWantsUpdates(true) subs[i] = c end \
         order = {} \
         A = Turbine.UI.Control() B = Turbine.UI.Control() C = Turbine.UI.Control() \
         A.Update = function() order[#order+1] = 'A' B:SetWantsUpdates(false) C:SetWantsUpdates(true) end \
         B.Update = function() order[#order+1] = 'B' end \
         C.Update = function() order[#order+1] = 'C' end \
         A:SetWantsUpdates(true) B:SetWantsUpdates(true)",
    );
    timed(
        "Update tick, 8,003 subscribers",
        Duration::from_millis(50),
        || fx.pui.update_tick(&fx.manager),
    );
    let o: String = eval(&fx, "return table.concat(order, '')");
    assert_eq!(
        o, "AC",
        "B removed before its turn, C added during the tick"
    );
    fx.assert_no_errors();
}

/// Filling a list box in one call lays it out once; the items are where the list rules
/// put them before anything reads them.
#[test]
fn filling_a_list_lays_it_out_once() {
    let mut fx = fx_with(
        "w = Turbine.UI.Window() w:SetSize(300, 300) w:SetVisible(true) \
         L = Turbine.UI.ListBox() L:SetParent(w) L:SetSize(200, 200) L:SetVisible(true)",
    );
    timed("AddItem x 4,000", Duration::from_millis(100), || {
        let _: () = eval(
            &fx,
            "for i = 1, 4000 do local it = Turbine.UI.Label() it:SetSize(200, 20) it:SetVisible(true) L:AddItem(it) end",
        );
    });
    let r: (f64, f64, f64) = eval(
        &fx,
        "return L:GetItem(1):GetTop(), L:GetItem(2):GetTop(), L:GetItem(4000):GetTop()",
    );
    assert_eq!(r, (0.0, 20.0, 79_980.0));
    // A hit test right after plugin code sees the laid-out items.
    let _: () = eval(
        &fx,
        "L.SelectedIndexChanged = function() sel = L:GetSelectedIndex() end",
    );
    let (lx, ly, _, _) = fx.screen_box("Probe", "L");
    fx.click(lx + 10, ly + 45, 1);
    let sel: f64 = eval(&fx, "return sel");
    assert_eq!(sel, 3.0);
    fx.assert_no_errors();
}

/// One wheel step over a long list costs little when the items have no handlers.
#[test]
fn wheel_over_a_long_list() {
    let mut fx = fx_with(
        "w = Turbine.UI.Window() w:SetSize(300, 300) w:SetVisible(true) \
         L = Turbine.UI.ListBox() L:SetParent(w) L:SetSize(200, 200) L:SetVisible(true) \
         for i = 1, 10000 do local it = Turbine.UI.Label() it:SetSize(200, 20) it:SetVisible(true) L:AddItem(it) end",
    );
    fx.deliver();
    let (lx, ly, _, _) = fx.screen_box("Probe", "L");
    timed(
        "wheel step over 10,000 items",
        Duration::from_millis(50),
        || fx.wheel(lx + 10, ly + 10, false),
    );
    let top: f64 = eval(&fx, "return L:GetItem(1):GetTop()");
    assert_eq!(top, -20.0);
    fx.assert_no_errors();
}

/// Plugin-controlled layout values at the ends of the 32-bit range do not overflow.
#[test]
fn extreme_layout_values_do_not_overflow() {
    let mut fx = fx_with(
        "lb = Turbine.UI.ListBox() lb:SetSize(100, 100) lb:SetReverseFill(true) \
         lb:SetWrapIndentAmount(-2147483648) lb:SetMaxItemsPerLine(1) \
         for i = 1, 3 do lb:AddItem(Turbine.UI.Control()) end \
         lh = Turbine.UI.ListBox() lh:SetOrientation(Turbine.UI.Orientation.Horizontal) lh:SetSize(100, 100) \
         lh:SetWrapIndentAmount(2147483647) lh:SetMaxItemsPerLine(1) lh:SetFlippedLayout(true) \
         for i = 1, 3 do local c = Turbine.UI.Control() c:SetSize(2147483647, 2147483647) lh:AddItem(c) end \
         tv = Turbine.UI.TreeView() tv:SetSize(100, 100) tv:SetIndentationWidth(-2147483648) \
         local n = Turbine.UI.TreeNode() tv:GetNodes():Add(n) local k = Turbine.UI.TreeNode() n:GetChildNodes():Add(k) \
         n:Expand() tv:SetIndentationWidth(2147483647)",
    );
    let r: (f64, f64) = eval(
        &fx,
        "return lb:GetItem(3):GetTop(), lh:GetItem(3):GetLeft()",
    );
    assert!(r.0.is_finite() && r.1.is_finite());
    fx.frame(0.016);
    // Only the oversized-control warnings of §13.2 (no error from the layout).
    let e = fx.errors();
    assert!(
        e.iter()
            .all(|m| m.contains("larger than the largest texture")),
        "{e:#?}"
    );
}

/// `LotroUI.SetEnabled` keeps only `LotroUIElement` values.
#[test]
fn panel_switches_are_bounded() {
    let fx = fx_with("");
    let r: (bool, bool, bool) = eval(
        &fx,
        "local P = Turbine.UI.Lotro.LotroUI local E = Turbine.UI.Lotro.LotroUIElement \
         for i = 1, 1000 do P.SetEnabled(123456 + i, false) end \
         P.SetEnabled(E.Backpack1, false) \
         return P.IsEnabled(123457), P.IsEnabled(E.Backpack1), P.IsEnabled(E.Vitals)",
    );
    assert_eq!(r, (true, false, true));
}

/// A `Graphic` built from an absurdly long path keeps no copy of it and still works as a
/// background value.
#[test]
fn long_graphic_paths_are_not_kept() {
    let fx = fx_with("");
    let r: Result<(), _> = fx.try_eval(
        "Probe",
        "local big = string.rep('p', 1000000) local c = Turbine.UI.Control() \
         for i = 1, 50 do c:SetBackground(Turbine.UI.Graphic(big)) end",
    );
    assert!(r.is_ok(), "{r:?}");
}

/// Controls created up to the apartment's memory cap are all given back once collected
/// (the failing-bind path itself is a unit test of `ui::model`).
#[test]
fn failed_constructors_do_not_leak_object_slots() {
    let mut fx = Fx::with_limits(tessera_plugins::Limits {
        memory_bytes: 4 << 20,
        ..tessera_plugins::Limits::default()
    });
    fx.plugin("Test", "Probe", Some("Probe"), "");
    fx.refresh();
    fx.load("Probe");
    // The memory error may end the whole call (the cap is checked against the peak).
    let _ = fx.try_eval::<()>(
        "Probe",
        "_G.keep = {} local ok = true local n = 0 \
         while ok and n < 200000 do local c ok, c = pcall(Turbine.UI.Control) n = n + 1 if ok then _G.keep[n] = c end end",
    );
    // At the cap even compiling a chunk fails: drop the table and collect from the host.
    fx.manager
        .with_lua("Probe", |lua| {
            let _ = lua
                .globals()
                .raw_set("keep", tessera_plugins::mlua::Value::Nil);
            let _ = lua.gc_collect();
            let _ = lua.gc_collect();
        })
        .expect("apartment");
    collect(&mut fx);
    assert_eq!(fx.pui.object_count("Probe"), 0);
}

/// `VisibleChanged` follows the plugin's flag on a node the layer hides.
#[test]
fn visible_changed_on_a_hidden_tree_node() {
    let fx = fx_with(
        "tv = Turbine.UI.TreeView() tv:SetSize(100, 100) \
         P = Turbine.UI.TreeNode() tv:GetNodes():Add(P) K = Turbine.UI.TreeNode() P:GetChildNodes():Add(K) \
         K:SetVisible(true) P:Collapse() n = 0 K.VisibleChanged = function() n = n + 1 end",
    );
    let r: (bool, f64) = eval(&fx, "K:SetVisible(false) return K:IsVisible(), n");
    assert_eq!(r, (false, 1.0));
}

/// `ControlList:Insert` and `Set` keep the control in the list (Tessera appends it; the
/// child order of a control list is an LPA §13.7 OPEN).
#[test]
fn control_list_insert_and_set() {
    let fx = fx_with(
        "P = Turbine.UI.Control() A = Turbine.UI.Control() B = Turbine.UI.Control() C = Turbine.UI.Control() \
         A:SetParent(P) B:SetParent(P)",
    );
    let r: (f64, bool, bool) = eval(
        &fx,
        "local l = P:GetControls() l:Insert(1, C) return l:GetCount(), l:Contains(C), C:GetParent() == P",
    );
    assert_eq!(r, (3.0, true, true));
}

/// Twenty thousand live controls in one apartment raise no error: a native object holds no
/// host-side handle to its class table (`lua-plugin-api.md` §8.3), so the count is bounded
/// by memory, not by the Lua stack. `IsA` still answers from the class.
#[test]
fn twenty_thousand_live_controls_raise_no_error() {
    let mut fx = fx_with("");
    let r: Result<bool, _> = fx.try_eval(
        "Probe",
        "keep = {} for i = 1, 20000 do keep[i] = Turbine.UI.Control() end \
         return keep[20000]:IsA(Turbine.UI.Control) and keep[1]:IsA(Turbine.Object) \
           and not keep[1]:IsA(Turbine.UI.Window)",
    );
    assert_eq!(r.ok(), Some(true));
    assert_eq!(fx.pui.object_count("Probe"), 20_000);
    fx.frame(0.016);
    fx.assert_no_errors();
}
