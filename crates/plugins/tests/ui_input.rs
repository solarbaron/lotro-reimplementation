// SPDX-License-Identifier: GPL-3.0-or-later
//! Plugin input handlers under synthetic input offered as the client offers it
//! (`lua-plugin-api.md` §9.7, §13.11–§13.14, §14.2.3, §14.6; decision 0014 D7, D8): mouse
//! buttons, hover, the wheel, the key broadcast, focus and typing, activation, the layer's
//! own reactions (check boxes, lists, scroll bars, menus, quickslots, the neutral window
//! frame) and the per-call instruction budget of each handler call. Synthetic plugins
//! written for these tests.

#[path = "fixtures/harness.rs"]
mod harness;

use harness::Fx;
use tessera_plugins::Limits;
use tessera_plugins::ui::Modifiers;

const WINDOW: &str = "w = Turbine.UI.Window() w:SetSize(400, 300) w:SetPosition(100, 100) w:SetVisible(true) \
     w:SetBackColor(Turbine.UI.Color(0.2, 0.2, 0.2)) \
     log = {} local function add(s) log[#log+1] = s end \
     function track(c, name) \
       for _, e in ipairs({ 'MouseDown', 'MouseUp', 'MouseClick', 'MouseDoubleClick', 'MouseEnter', 'MouseLeave', 'Click', 'MouseWheel' }) do \
         c[e] = function(sender, a) \
           local s = name .. ':' .. e if a.Button then s = s .. '(' .. a.Button .. ',' .. a.X .. ',' .. a.Y .. ')' end \
           if a.Direction then s = s .. '(' .. a.Direction .. ')' end add(s) end \
       end \
     end ";

fn fx_with(source: &str) -> Fx {
    let mut fx = Fx::new();
    fx.plugin(
        "Test",
        "Probe",
        Some("Probe"),
        &format!("{WINDOW} {source}"),
    );
    fx.refresh();
    fx.load("Probe");
    fx
}

fn log(fx: &Fx) -> String {
    let s: String = fx.eval(
        "Probe",
        "local s = table.concat(log, ' ') log = {} return s",
    );
    s
}

#[test]
fn button_click_sequence_and_hover() {
    let mut fx = fx_with(
        "b = Turbine.UI.Button() b:SetParent(w) b:SetPosition(20, 30) b:SetSize(80, 20) b:SetVisible(true) track(b, 'b')",
    );
    fx.move_to(50, 50);
    fx.click(125, 135, 1);
    assert_eq!(
        log(&fx),
        "b:MouseEnter b:MouseDown(1,5,5) b:MouseUp(1,5,5) b:Click b:MouseClick(1,5,5)"
    );
    fx.move_to(600, 600);
    assert_eq!(log(&fx), "b:MouseLeave");
    // A right click gives the button events but no Click (§13.6 Tessera rule).
    fx.click(125, 135, 2);
    assert_eq!(
        log(&fx),
        "b:MouseEnter b:MouseDown(2,5,5) b:MouseUp(2,5,5) b:MouseClick(2,5,5)"
    );
    // A disabled button gives no Click.
    let _: () = fx.eval("Probe", "b:SetEnabled(false)");
    fx.click(125, 135, 1);
    let l = log(&fx);
    assert!(!l.contains("b:Click"), "{l}");
    fx.assert_no_errors();
}

#[test]
fn mouse_invisible_controls_pass_the_mouse_through() {
    let mut fx = fx_with(
        "under = Turbine.UI.Control() under:SetParent(w) under:SetSize(100, 100) under:SetVisible(true) track(under, 'under') \
         over = Turbine.UI.Control() over:SetParent(w) over:SetSize(100, 100) over:SetZOrder(5) over:SetVisible(true) \
         over:SetMouseVisible(false) track(over, 'over')",
    );
    fx.click(150, 150, 1);
    let l = log(&fx);
    assert!(
        l.contains("under:MouseClick") && !l.contains("over:"),
        "{l}"
    );
}

#[test]
fn key_broadcast_reaches_subscribers_only() {
    let mut fx = fx_with(
        "keys = {} hot = Turbine.UI.Control() hot:SetWantsKeyEvents(true) \
         function hot:KeyDown(a) keys[#keys+1] = 'down:' .. a.Action .. ':' .. tostring(a.Shift) end \
         function hot:KeyUp(a) keys[#keys+1] = 'up:' .. a.Action end \
         quiet = Turbine.UI.Control() function quiet:KeyDown() keys[#keys+1] = 'quiet' end",
    );
    fx.pui.set_modifiers(Modifiers {
        shift: true,
        ..Modifiers::default()
    });
    let (down, up) = fx.key("ToggleBags", 0x1000_0094);
    assert!(!down && !up, "no element consumed the key");
    let k: String = fx.eval("Probe", "return table.concat(keys, ' ')");
    assert_eq!(
        k,
        format!("down:{}:true up:{}", 0x1000_0094u32, 0x1000_0094u32)
    );
    let shifted: bool = fx.eval("Probe", "return Turbine.UI.Control.IsShiftKeyDown()");
    assert!(shifted);
    // Turning it off stops both at once.
    let _: () = fx.eval("Probe", "hot:SetWantsKeyEvents(false) keys = {}");
    fx.key("ToggleBags", 0x1000_0094);
    let k: String = fx.eval("Probe", "return table.concat(keys, ' ')");
    assert_eq!(k, "");
    // The key broadcast also goes to the client (game code).
    assert!(
        fx.client
            .iter()
            .any(|e| matches!(e, tessera_ui::UiEvent::KeyBroadcast { .. }))
    );
}

#[test]
fn text_box_focus_and_typing() {
    let mut fx = fx_with(
        "t = Turbine.UI.TextBox() t:SetParent(w) t:SetPosition(10, 10) t:SetSize(200, 20) t:SetVisible(true) \
         changes = 0 function t:TextChanged() changes = changes + 1 end \
         focus = '' function t:FocusGained() focus = focus .. '+' end function t:FocusLost() focus = focus .. '-' end",
    );
    // The click alone gives focus (LPA §13.1 Tessera rule).
    fx.click(150, 115, 1);
    for c in "hi".chars() {
        fx.type_char(c);
    }
    let r: (bool, String, f64) = fx.eval("Probe", "return t:HasFocus(), t:GetText(), changes");
    assert!(r.0, "focused");
    assert_eq!(r.1, "hi");
    assert!(r.2 >= 2.0, "TextChanged per edit, got {}", r.2);
    let f: String = fx.eval("Probe", "return focus");
    assert!(f.starts_with('+'), "{f}");
}

#[test]
fn wheel_direction() {
    let mut fx = fx_with(
        "c = Turbine.UI.Control() c:SetParent(w) c:SetSize(50, 50) c:SetVisible(true) track(c, 'c')",
    );
    fx.wheel(120, 120, true);
    fx.wheel(120, 120, false);
    let l = log(&fx);
    assert!(
        l.contains("c:MouseWheel(1)") && l.contains("c:MouseWheel(-1)"),
        "{l}"
    );
}

#[test]
fn check_box_toggles_on_click() {
    let mut fx = fx_with(
        "cb = Turbine.UI.CheckBox() cb:SetParent(w) cb:SetSize(100, 20) cb:SetVisible(true) \
         n = 0 function cb:CheckedChanged() n = n + 1 end",
    );
    fx.click(110, 110, 1);
    fx.click(110, 110, 1);
    fx.click(110, 110, 1);
    let r: (bool, f64) = fx.eval("Probe", "return cb:IsChecked(), n");
    assert_eq!(r, (true, 3.0));
}

#[test]
fn list_selection_and_wheel_scrolling() {
    let mut fx = fx_with(
        "lb = Turbine.UI.ListBox() lb:SetParent(w) lb:SetPosition(0, 0) lb:SetSize(200, 100) lb:SetVisible(true) \
         sb = Turbine.UI.ScrollBar() sb:SetOrientation(Turbine.UI.Orientation.Vertical) sb:SetParent(w) \
         sb:SetPosition(200, 0) sb:SetSize(10, 100) sb:SetVisible(true) lb:SetVerticalScrollBar(sb) \
         for i = 1, 20 do local l = Turbine.UI.Label() l:SetText('row ' .. i) l:SetSize(200, 20) l:SetVisible(true) lb:AddItem(l) end \
         changed = 0 function lb:SelectedIndexChanged() changed = changed + 1 end",
    );
    fx.click(150, 145, 1); // row 3 (y 40..59 inside the list)
    let r: (f64, f64) = fx.eval("Probe", "return lb:GetSelectedIndex(), changed");
    assert_eq!(r, (3.0, 1.0));
    fx.wheel(150, 150, false);
    let y: f64 = fx.eval("Probe", "return select(2, lb:GetItem(1):GetPosition())");
    assert!(y < 0.0, "the wheel scrolled the list (row 1 at {y})");
    fx.assert_no_errors();
}

#[test]
fn scroll_bar_track_and_thumb() {
    let mut fx = fx_with(
        "sb = Turbine.UI.ScrollBar() sb:SetParent(w) sb:SetOrientation(Turbine.UI.Orientation.Vertical) \
         sb:SetSize(16, 200) sb:SetVisible(true) sb:SetMaximum(100) sb:SetLargeChange(10) \
         thumb = Turbine.UI.Control() thumb:SetVisible(true) thumb:SetBackColor(Turbine.UI.Color.White) sb:SetThumbButton(thumb) \
         values = {} function sb:ValueChanged() values[#values+1] = sb:GetValue() end",
    );
    let th: (f64, f64) = fx.eval("Probe", "return thumb:GetSize()");
    assert_eq!(
        th,
        (16.0, 20.0),
        "thumb length = large change / range of the track"
    );
    fx.click(108, 250, 1); // below the thumb: page down
    let v: f64 = fx.eval("Probe", "return sb:GetValue()");
    assert_eq!(v, 10.0);
    let h = fx.press(108, 100 + 18 + 5); // on the thumb (it moved to y 18)
    fx.move_to(108, 100 + 18 + 5 + 90);
    fx.release(h, 108, 100 + 18 + 5 + 90);
    let v: f64 = fx.eval("Probe", "return sb:GetValue()");
    assert_eq!(v, 60.0, "dragged by 90 of 180 free pixels");
    fx.assert_no_errors();
}

#[test]
fn quickslot_alias_runs_a_chat_command() {
    let mut fx = fx_with(
        "q = Turbine.UI.Lotro.Quickslot() q:SetParent(w) q:SetSize(32, 32) q:SetVisible(true) \
         q:SetShortcut(Turbine.UI.Lotro.Shortcut(Turbine.UI.Lotro.ShortcutType.Alias, '/myplugin toggle')) \
         e = Turbine.UI.Lotro.Quickslot() e:SetParent(w) e:SetPosition(40, 0) e:SetSize(32, 32) e:SetVisible(true) \
         e:SetShortcut(Turbine.UI.Lotro.Shortcut(Turbine.UI.Lotro.ShortcutType.Emote, '0x00000001'))",
    );
    fx.click(110, 110, 1);
    fx.click(110, 110, 2); // right click: not used until SetUseOnRightClick
    fx.click(150, 110, 1); // an emote needs game state
    assert_eq!(
        fx.pui.take_chat_commands(),
        vec!["/myplugin toggle".to_owned()]
    );
    let _: () = fx.eval("Probe", "q:SetUseOnRightClick(true)");
    fx.click(110, 110, 2);
    assert_eq!(
        fx.pui.take_chat_commands(),
        vec!["/myplugin toggle".to_owned()]
    );
    // The client runs it like a typed command: plugin commands go to the manager.
    let _: () = fx.eval(
        "Probe",
        "ran = 0 local cmd = Turbine.ShellCommand() function cmd:Execute() ran = ran + 1 end \
         Turbine.Shell.AddCommand('myplugin', cmd)",
    );
    fx.click(110, 110, 1);
    for line in fx.pui.take_chat_commands() {
        assert!(fx.manager.execute_command(&line));
    }
    let ran: f64 = fx.eval("Probe", "return ran");
    assert_eq!(ran, 1.0);
}

#[test]
fn context_menu_click_and_dismiss() {
    let mut fx = fx_with(
        "clicked = {} menu = Turbine.UI.ContextMenu() \
         local a = Turbine.UI.MenuItem('Alpha') a.Click = function() clicked[#clicked+1] = 'Alpha' end \
         local b = Turbine.UI.MenuItem('Beta') b.Click = function() clicked[#clicked+1] = 'Beta' end \
         local off = Turbine.UI.MenuItem('Off', false) off.Click = function() clicked[#clicked+1] = 'Off' end \
         menu:GetItems():Add(a) menu:GetItems():Add(b) menu:GetItems():Add(off) \
         function w:MouseClick(args) if args.Button == Turbine.UI.MouseButton.Right then menu:ShowMenu() end end",
    );
    fx.click(300, 300, 2);
    // The popup opens at the pointer: rows of 20 pixels.
    fx.click(310, 330, 1); // second row
    let c: String = fx.eval("Probe", "return table.concat(clicked, ',')");
    assert_eq!(c, "Beta");
    // Closed after the click: clicking there again reaches the window.
    fx.click(310, 330, 1);
    let c: String = fx.eval("Probe", "return table.concat(clicked, ',')");
    assert_eq!(c, "Beta");
    // Open, click a disabled row (nothing), then click outside: closed, nothing fired.
    fx.click(300, 300, 2);
    fx.click(310, 350, 1);
    fx.click(1000, 800, 1);
    fx.click(310, 310, 1);
    let c: String = fx.eval("Probe", "return table.concat(clicked, ',')");
    assert_eq!(c, "Beta");
    fx.assert_no_errors();
}

#[test]
fn game_styled_frame_close_move_and_resize() {
    let mut fx = Fx::new();
    fx.plugin(
        "Test",
        "Probe",
        Some("Probe"),
        "w = Turbine.UI.Lotro.Window() w:SetSize(400, 300) w:SetPosition(100, 100) w:SetText('Frame') w:SetVisible(true) \
         log = {} function w:Closing(a) log[#log+1] = 'Closing' end function w:Closed() log[#log+1] = 'Closed' end",
    );
    fx.refresh();
    fx.load("Probe");
    // Drag the title bar (centre top) by (50, 40).
    let h = fx.press(300, 110);
    fx.move_to(350, 150);
    fx.release(h, 350, 150);
    let p: (f64, f64) = fx.eval("Probe", "return w:GetPosition()");
    assert_eq!(p, (150.0, 140.0));
    // Resize from the bottom-right handle once resizable.
    let _: () = fx.eval("Probe", "w:SetResizable(true)");
    let h = fx.press(150 + 390, 140 + 290);
    fx.move_to(150 + 390 + 60, 140 + 290 + 30);
    fx.release(h, 150 + 390 + 60, 140 + 290 + 30);
    let s: (f64, f64) = fx.eval("Probe", "return w:GetSize()");
    assert_eq!(s, (460.0, 330.0));
    // The close button (W − 21, 21, 16 × 16) runs the native Close.
    fx.click(150 + 460 - 21 + 8, 140 + 21 + 8, 1);
    let r: (String, bool) = fx.eval("Probe", "return table.concat(log, ','), w:IsVisible()");
    assert_eq!(r, ("Closing,Closed".into(), false));
    fx.assert_no_errors();
}

#[test]
fn activation_by_press_and_by_call() {
    let mut fx = fx_with(
        "acts = {} function w:Activated() acts[#acts+1] = 'w+' end function w:Deactivated() acts[#acts+1] = 'w-' end \
         v = Turbine.UI.Window() v:SetSize(100, 100) v:SetPosition(700, 100) v:SetVisible(true) \
         function v:Activated() acts[#acts+1] = 'v+' end function v:Deactivated() acts[#acts+1] = 'v-' end",
    );
    fx.click(150, 150, 1);
    fx.click(750, 150, 1);
    let _: () = fx.eval("Probe", "w:Activate() w:Activate()");
    fx.deliver();
    let a: String = fx.eval("Probe", "return table.concat(acts, ' ')");
    assert_eq!(a, "w+ w- v+ v- w+");
}

/// Each handler call is one host call with its own instruction budget: a runaway handler
/// is stopped and reported, the next handler and later input still run (decision 0014
/// D8; `lua-plugin-api.md` §3.3).
#[test]
fn runaway_handler_is_bounded_by_the_call_budget() {
    let limits = Limits {
        instructions_per_call: Some(200_000),
        ..Limits::default()
    };
    let mut fx = Fx::with_limits(limits);
    fx.plugin(
        "Test",
        "Probe",
        Some("Probe"),
        &format!(
            "{WINDOW} c = Turbine.UI.Control() c:SetParent(w) c:SetSize(50, 50) c:SetVisible(true) \
             clicks = 0 c.MouseDown = function() while true do end end \
             c.MouseClick = function() clicks = clicks + 1 end"
        ),
    );
    fx.refresh();
    fx.load("Probe");
    fx.click(110, 110, 1);
    fx.click(110, 110, 1);
    let clicks: f64 = fx.eval("Probe", "return clicks");
    assert_eq!(clicks, 2.0);
    let errors = fx.errors();
    assert_eq!(errors.len(), 2, "{errors:#?}");
}

/// A handler chain that never settles (each `SizeChanged` resizes again) is bounded by
/// the drain bound and the budget; the host keeps running.
#[test]
fn endless_notification_chain_is_bounded() {
    let mut fx = fx_with(
        "c = Turbine.UI.Control() c:SetParent(w) c:SetSize(50, 50) c:SetVisible(true) n = 0 \
         function c:SizeChanged() n = n + 1 local x = self:GetWidth() self:SetWidth(x == 50 and 51 or 50) end \
         function c:MouseClick() self:SetWidth(52) end",
    );
    fx.click(110, 110, 1);
    let n: f64 = fx.eval("Probe", "return n");
    assert!(n >= 1.0);
    fx.frame(0.016);
    fx.frame(0.016);
    let alive: f64 = fx.eval("Probe", "return 1 + 1");
    assert_eq!(alive, 2.0);
}

/// LPA §13.1 Tessera rule: a click gives a text box focus.
#[test]
fn a_click_focuses_a_text_box() {
    let mut fx = fx_with(
        "t = Turbine.UI.TextBox() t:SetParent(w) t:SetPosition(10, 10) t:SetSize(200, 30) t:SetVisible(true)",
    );
    fx.click(150, 125, 1);
    for c in "abc".chars() {
        fx.type_char(c);
    }
    let r: (bool, String) = fx.eval("Probe", "return t:HasFocus(), t:GetText()");
    assert_eq!(r, (true, "abc".into()));
}

/// LPA §13.5: typing into a read-only text box changes nothing and fires no
/// `TextChanged`.
#[test]
fn a_read_only_text_box_takes_no_typing() {
    let mut fx = fx_with(
        "t = Turbine.UI.TextBox() t:SetParent(w) t:SetPosition(10, 10) t:SetSize(200, 30) t:SetVisible(true) \
         n = 0 t.TextChanged = function() n = n + 1 end",
    );
    let _: () = fx.eval("Probe", "t:SetText('ro') t:SetReadOnly(true) n = 0");
    fx.click(150, 125, 1);
    for c in "xyz".chars() {
        fx.type_char(c);
    }
    let r: (String, bool, f64) = fx.eval("Probe", "return t:GetText(), t:IsReadOnly(), n");
    assert_eq!(r, ("ro".into(), true, 0.0));
    // Writable again: typing works.
    let _: () = fx.eval("Probe", "t:SetReadOnly(false)");
    fx.type_char('!');
    let r: (String, f64) = fx.eval("Probe", "return t:GetText(), n");
    assert_eq!(r.1, 1.0);
    assert!(r.0.contains('!'), "{}", r.0);
}

/// LPA §13.4: the selection of a text box the user edits follows the caret (estimated at
/// the end of each edit), so `InsertText` inserts where the user typed.
#[test]
fn the_selection_follows_typing() {
    let mut fx = fx_with(
        "t = Turbine.UI.TextBox() t:SetParent(w) t:SetPosition(10, 10) t:SetSize(200, 30) t:SetVisible(true)",
    );
    fx.click(150, 125, 1);
    for c in "abc".chars() {
        fx.type_char(c);
    }
    let r: (f64, f64) = fx.eval(
        "Probe",
        "return t:GetSelectionStart(), t:GetSelectionLength()",
    );
    assert_eq!(r, (4.0, 0.0));
    let s: String = fx.eval("Probe", "t:InsertText('X') return t:GetText()");
    assert_eq!(s, "abcX");
}

/// LPA §9.6 OPEN (Tessera rule): `SetText` on a text box fires `TextChanged` through the
/// queue when the text changes, and only then.
#[test]
fn set_text_fires_text_changed() {
    let fx = fx_with(
        "t = Turbine.UI.TextBox() log = {} t.TextChanged = function() log[#log+1] = 'TC' end \
         l = Turbine.UI.Label() l.TextChanged = function() log[#log+1] = 'label' end",
    );
    let s: String = fx.eval(
        "Probe",
        "t:SetText('abc') t:SetText('abc') t:AppendText('d') l:SetText('x') return table.concat(log, ',')",
    );
    assert_eq!(s, "TC,TC");
}

/// LPA §13.7 OPEN (Tessera rule): the wheel over an item scrolls by the bound scroll
/// bar's small change, as the wheel over the bar does.
#[test]
fn wheel_over_an_item_uses_the_bound_small_change() {
    let mut fx = fx_with(
        "lb = Turbine.UI.ListBox() lb:SetParent(w) lb:SetSize(200, 100) lb:SetVisible(true) \
         sb = Turbine.UI.ScrollBar() sb:SetOrientation(Turbine.UI.Orientation.Vertical) sb:SetParent(w) \
         sb:SetPosition(200, 0) sb:SetSize(10, 100) sb:SetVisible(true) sb:SetSmallChange(5) \
         lb:SetVerticalScrollBar(sb) \
         for i = 1, 30 do local l = Turbine.UI.Label() l:SetSize(200, 20) l:SetVisible(true) lb:AddItem(l) end",
    );
    let t0: f64 = fx.eval("Probe", "return lb:GetItem(3):GetTop()");
    assert_eq!(t0, 40.0);
    fx.wheel(150, 150, false);
    let t1: f64 = fx.eval("Probe", "return lb:GetItem(3):GetTop()");
    fx.wheel(205, 150, false);
    let t2: f64 = fx.eval("Probe", "return lb:GetItem(3):GetTop()");
    assert_eq!(
        (t1, t2),
        (35.0, 30.0),
        "5 pixels over an item and over the bar"
    );
    fx.assert_no_errors();
}

/// An open context menu stays alive when its only reference was a local of the handler
/// that showed it.
#[test]
fn an_open_context_menu_survives_collection() {
    let mut fx = fx_with(
        "clicked = 0 \
         function show() local menu = Turbine.UI.ContextMenu() local item = Turbine.UI.MenuItem('Do it') \
           item.Click = function() clicked = clicked + 1 end menu:GetItems():Add(item) menu:ShowMenuAt(500, 400) end",
    );
    let _: () = fx.eval("Probe", "show()");
    fx.deliver();
    let _: () = fx.eval(
        "Probe",
        "collectgarbage('collect') collectgarbage('collect')",
    );
    fx.frame(0.016);
    fx.frame(0.016);
    fx.click(510, 410, 1);
    let c: f64 = fx.eval("Probe", "return clicked");
    assert_eq!(c, 1.0);
    // Closed, it is no longer pinned.
    let _: () = fx.eval(
        "Probe",
        "collectgarbage('collect') collectgarbage('collect')",
    );
    fx.frame(0.016);
    let _: () = fx.eval(
        "Probe",
        "collectgarbage('collect') collectgarbage('collect')",
    );
    fx.frame(0.016);
    let _: () = fx.eval(
        "Probe",
        "w = nil collectgarbage('collect') collectgarbage('collect')",
    );
    fx.frame(0.016);
    fx.assert_no_errors();
}

/// A game-styled window closed by its frame's close button is no longer pinned: once
/// plugin code drops it, it is collected.
#[test]
fn a_window_closed_by_its_frame_is_unpinned() {
    let mut fx = Fx::new();
    fx.plugin(
        "Test",
        "Probe",
        Some("Probe"),
        "do local w = Turbine.UI.Lotro.Window() w:SetSize(400, 300) w:SetPosition(100, 100) w:SetVisible(true) end",
    );
    fx.refresh();
    fx.load("Probe");
    assert!(fx.pui.object_count("Probe") > 0);
    fx.click(100 + 400 - 21 + 8, 100 + 21 + 8, 1);
    for _ in 0..3 {
        let _: () = fx.eval(
            "Probe",
            "collectgarbage('collect') collectgarbage('collect')",
        );
        fx.frame(0.016);
    }
    assert_eq!(fx.pui.object_count("Probe"), 0);
    fx.assert_no_errors();
}
