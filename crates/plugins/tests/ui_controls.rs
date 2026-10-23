// SPDX-License-Identifier: GPL-3.0-or-later
//! Behaviour of the `Turbine.UI` control classes driven from Lua (`lua-plugin-api.md`
//! §9.4, §9.6, §9.7, §13.1–§13.21, §14.2.3, §14.6): geometry, tree, visibility, events and
//! their delivery order, look, text, scroll-bar numbers, lists, trees, menus, quickslots.
//! Synthetic plugins written for these tests.

#[path = "fixtures/harness.rs"]
mod harness;

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

#[test]
fn initial_geometry_tree_and_identity() {
    let fx = fx_with("w = Turbine.UI.Window() c = Turbine.UI.Control()");
    let r: (f64, f64, f64, f64, bool, bool) = eval(
        &fx,
        "local l, t = c:GetPosition() local wd, h = c:GetSize() \
         return l, t, wd, h, c:IsVisible(), w:IsVisible()",
    );
    assert_eq!(r, (0.0, 0.0, 50.0, 50.0, false, false));
    let r: (bool, bool, f64, bool, bool) = eval(
        &fx,
        "c:SetParent(w) local list = w:GetControls() \
         return c:GetParent() == w, list:Get(1) == c, list:GetCount(), list:Contains(c), \
                w:GetControls() == list",
    );
    assert_eq!(r, (true, true, 1.0, true, true));
    let r: Result<(), _> = fx.try_eval("Probe", "c:SetParent(c)");
    assert!(r.is_err(), "a control cannot be its own parent");
    let r: Result<(), _> = fx.try_eval("Probe", "w:GetControls():Get(2)");
    assert!(r.is_err(), "index out of range raises (§8.5)");
    let r: (f64, bool) = eval(
        &fx,
        "local idx = w:GetControls():IndexOf(Turbine.UI.Label()) c:SetParent(nil) \
         return idx, c:GetParent() == nil",
    );
    assert_eq!(r, (-1.0, true));
    fx.assert_no_errors();
}

#[test]
fn displayed_follows_ancestors() {
    let fx = fx_with(
        "w = Turbine.UI.Window() w:SetSize(200, 100) c = Turbine.UI.Label() c:SetParent(w) c:SetVisible(true)",
    );
    let r: (bool, bool, bool) = eval(
        &fx,
        "local a = c:IsDisplayed() w:SetVisible(true) return a, c:IsDisplayed(), w:IsDisplayed()",
    );
    assert_eq!(r, (false, true, true));
}

/// §13.15 worked example through Lua.
#[test]
fn size_limits() {
    let fx = fx_with("c = Turbine.UI.Control() c:SetSize(100, 50)");
    let r: (f64, f64, f64, f64, f64) = eval(
        &fx,
        "c:SetMaximumWidth(80) local w0 = c:GetWidth() c:SetHeight(60) local w1, h1 = c:GetSize() \
         c:SetMinimumWidth(90) c:SetWidth(10) return w0, w1, h1, c:GetWidth(), c:GetMinimumHeight()",
    );
    assert_eq!(r, (100.0, 80.0, 60.0, 90.0, 0.0));
}

#[test]
fn edge_attachments_follow_the_parent() {
    let fx = fx_with(
        "w = Turbine.UI.Window() w:SetSize(200, 100) \
         c = Turbine.UI.Control() c:SetParent(w) c:SetPosition(10, 20) c:SetSize(50, 20) \
         local E = Turbine.UI.EdgeAttachmentType \
         c:AttachEdges(E.None, E.None, E.Same, E.Same) \
         w:SetSize(300, 150)",
    );
    let r: (f64, f64, f64, f64, f64, f64, f64, f64) = eval(
        &fx,
        "local x, y = c:GetPosition() local wd, h = c:GetSize() local a, b, cc, d = c:GetEdgeAttachments() \
         return x, y, wd, h, a, b, cc, d",
    );
    assert_eq!(r, (10.0, 20.0, 150.0, 70.0, 0.0, 0.0, 5.0, 5.0));
}

/// §9.7: outside a handler a setter's notification is delivered before the call returns;
/// inside a notification handler, after the current event, in call order.
#[test]
fn notification_order_worked_example() {
    let mut fx = fx_with(
        "log = {} \
         w = Turbine.UI.Window() w:SetSize(400, 300) w:SetPosition(100, 100) w:SetVisible(true) \
         c = Turbine.UI.Control() c:SetParent(w) c:SetSize(40, 20) c:SetPosition(10, 10) c:SetVisible(true) \
         c:SetBackColor(Turbine.UI.Color(1, 0, 0)) \
         function c:MouseClick(args) log[#log+1] = 'MouseClick' self:SetSize(50, 20) printed = self:GetWidth() self:SetVisible(false) end \
         function c:SizeChanged() log[#log+1] = 'SizeChanged' end \
         function c:VisibleChanged() log[#log+1] = 'VisibleChanged' end \
         top = Turbine.UI.Control() \
         function top:SizeChanged() sync = (sync or 0) + 1 end \
         top:SetSize(10, 10) after = sync",
    );
    let after: f64 = eval(&fx, "return after");
    assert_eq!(after, 1.0, "SizeChanged delivered before SetSize returned");
    fx.click(115, 115, 1);
    let log: String = eval(&fx, "return table.concat(log, ',') .. ' ' .. printed");
    assert_eq!(log, "MouseClick,SizeChanged,VisibleChanged 50");
    fx.assert_no_errors();
}

/// §9.4 worked example: A, B, C subscribe; during A's handler C unsubscribes and D
/// subscribes: A, B, D in that tick and the next.
#[test]
fn update_broadcast_order() {
    let mut fx = fx_with(
        "order = {} \
         A = Turbine.UI.Control() B = Turbine.UI.Control() C = Turbine.UI.Control() D = Turbine.UI.Control() \
         for name, c in pairs({ A = A, B = B, C = C, D = D }) do c.Update = function() order[#order+1] = name end end \
         local first = true \
         A.Update = function() order[#order+1] = 'A' if first then first = false C:SetWantsUpdates(false) D:SetWantsUpdates(true) end end \
         A:SetWantsUpdates(true) B:SetWantsUpdates(true) C:SetWantsUpdates(true) \
         A:SetWantsUpdates(true)",
    );
    fx.frame(0.016);
    let o: String = eval(&fx, "return table.concat(order, '')");
    assert_eq!(o, "ABD");
    fx.frame(0.016);
    let o: String = eval(&fx, "return table.concat(order, '')");
    assert_eq!(o, "ABDABD");
    fx.assert_no_errors();
}

/// Hidden, unparented timer controls keep receiving `Update` (§9.4) even when the plugin
/// holds no reference to them.
#[test]
fn unreferenced_timer_survives_collection() {
    let mut fx = fx_with(
        "ticks = 0 \
         do local t = Turbine.UI.Control() t.Update = function() ticks = ticks + 1 end t:SetWantsUpdates(true) end \
         collectgarbage('collect')",
    );
    for _ in 0..3 {
        fx.frame(0.016);
        let _: () = eval(&fx, "collectgarbage('collect')");
    }
    let t: f64 = eval(&fx, "return ticks");
    assert_eq!(t, 3.0);
}

/// Detached controls nobody references are destroyed after collection; parented ones
/// stay.
#[test]
fn collected_controls_are_destroyed() {
    let mut fx = fx_with(
        "w = Turbine.UI.Window() \
         for i = 1, 200 do local c = Turbine.UI.Label() c:SetText('x') end \
         keep = Turbine.UI.Label() keep:SetParent(w) \
         do local child = Turbine.UI.Label() child:SetParent(w) end \
         collectgarbage('collect') collectgarbage('collect')",
    );
    fx.deliver();
    let n = fx.pui.object_count("Probe");
    assert!(n < 20, "{n} records left");
    let r: f64 = eval(&fx, "return w:GetControls():GetCount()");
    assert_eq!(r, 2.0, "parented children survive");
}

#[test]
fn enabled_changed_is_direct_and_only_on_change() {
    let fx = fx_with(
        "n = 0 c = Turbine.UI.Control() function c:EnabledChanged() n = n + 1 end \
         c:SetEnabled(true) c:SetEnabled(false) c:SetEnabled(false) c:SetEnabled(true)",
    );
    let r: (f64, bool) = eval(&fx, "return n, c:IsEnabled()");
    assert_eq!(r, (2.0, true));
}

#[test]
fn close_with_cancel() {
    let fx = fx_with(
        "log = {} w = Turbine.UI.Window() w:SetVisible(true) \
         cancel = true \
         function w:Closing(a) log[#log+1] = 'Closing' a.Cancel = cancel end \
         function w:Closed() log[#log+1] = 'Closed' end \
         w:Close() v1 = w:IsVisible() cancel = false w:Close() v2 = w:IsVisible() w:Close()",
    );
    let r: (String, bool, bool) = eval(&fx, "return table.concat(log, ','), v1, v2");
    assert_eq!(r, ("Closing,Closing,Closed".into(), true, false));
}

#[test]
fn look_members() {
    let fx = fx_with("c = Turbine.UI.Control()");
    let r: (f64, f64, f64, f64, f64) = eval(
        &fx,
        "local b0 = c:GetBlendMode() local cb0 = c:GetBackColorBlendMode() \
         c:SetBlendMode(Turbine.UI.BlendMode.AlphaBlend) \
         c:SetBackColor(Turbine.UI.Color(0.5, 1, 0, 0)) local bc = c:GetBackColor() \
         return b0, cb0, c:GetBlendMode(), bc.A, bc.R",
    );
    assert_eq!(r, (2.0, 2.0, 4.0, 0.5, 1.0));
    let r: Result<(), _> = fx.try_eval("Probe", "c:SetBlendMode(9)");
    assert!(r.is_err(), "a blend mode above 8 raises (§13.17)");
    let r: Result<(), _> = fx.try_eval("Probe", "c:SetBackColorBlendMode(12)");
    assert!(r.is_err());
    let r: (f64, f64) = eval(
        &fx,
        "c:SetBackColor(nil) local bc = c:GetBackColor() return bc.A, c:GetBlendMode()",
    );
    assert_eq!(r, (0.0, 4.0));
    let r: (f64, f64, f64) = eval(
        &fx,
        "c:SetStretchMode(2) c:SetOpacity(0.25) c:SetZOrder(7) return c:GetStretchMode(), c:GetOpacity(), c:GetZOrder()",
    );
    assert_eq!(r, (2.0, 0.25, 7.0));
    let r: (f64, bool, bool) = eval(
        &fx,
        "c:SetBackground(0x41000123) local a = c:GetBackground() c:SetBackground(nil) local b = c:GetBackground() == nil \
         c:SetBackground({}) return a, b, c:GetBackground() == nil",
    );
    assert_eq!(r, (f64::from(0x4100_0123u32), true, true));
}

#[test]
fn background_paths_are_checked_at_call_time() {
    let mut fx = Fx::new();
    fx.file("Test/Probe/Art/frame.tga", harness::tga(4, 4));
    fx.file("Test/Probe/Art/photo.jpg", harness::jpeg(8, 8));
    fx.file("Test/Probe/Art/icon.png", b"\x89PNG\r\n\x1a\n");
    fx.file("Other/Shared/common.tga", harness::tga(2, 2));
    fx.plugin("Test", "Probe", Some("Probe"), "c = Turbine.UI.Control()");
    fx.refresh();
    fx.load("Probe");
    let ok: String = fx.eval(
        "Probe",
        "c:SetBackground('test/probe/art/FRAME.TGA') local a = c:GetBackground() \
         c:SetBackground('Test\\\\Probe\\\\Art\\\\photo.jpg') \
         c:SetBackground('Other/Shared/common.tga') \
         local g = Turbine.UI.Graphic('Test/Probe/Art/frame.tga') c:SetBackground(g) \
         return a .. (c:GetBackground() == g and ' graphic' or ' ?')",
    );
    assert_eq!(ok, "test/probe/art/FRAME.TGA graphic");
    for bad in [
        "Test/Probe/Art/missing.tga",
        "Test/Probe/Art/icon.png",
        "../outside.tga",
        "/etc/passwd",
    ] {
        let r: Result<(), _> = fx.try_eval("Probe", &format!("c:SetBackground('{bad}')"));
        assert!(r.is_err(), "{bad} must raise");
    }
    let r: Result<(), _> = fx.try_eval(
        "Probe",
        "c:SetBackground(Turbine.UI.Graphic('nothing.tga'))",
    );
    assert!(
        r.is_err(),
        "a Graphic with a missing path raises at SetBackground"
    );
}

#[test]
fn text_members() {
    let fx = fx_with("l = Turbine.UI.Label() t = Turbine.UI.TextBox()");
    let r: (String, f64, f64, f64) = eval(
        &fx,
        "l:SetText('Hello') l:AppendText(' world') l:SetText(l:GetText() .. '!') \
         return l:GetText(), l:GetTextLength(), l:GetTextAlignment(), l:GetFont()",
    );
    assert_eq!(r, ("Hello world!".into(), 12.0, 1.0, 0.0));
    let r: (f64, f64, String) = eval(
        &fx,
        "l:SetSelection(7, 5) return l:GetSelectionStart(), l:GetSelectionLength(), l:GetSelectedText()",
    );
    assert_eq!(r, (7.0, 5.0, "world".into()));
    let r: (f64, f64) = eval(
        &fx,
        "l:SetSelection(-3, 100) return l:GetSelectionStart(), l:GetSelectionLength()",
    );
    assert_eq!(r, (1.0, 12.0), "start clamped to 1, end to the text length");
    let r: (f64, f64) = eval(
        &fx,
        "l:SetSelectionLength(0) l:SetSelectionStart(4) return l:GetSelectionStart(), l:GetSelectionLength()",
    );
    assert_eq!(
        r,
        (1.0, 0.0),
        "SetSelectionStart without a selection only deselects"
    );
    let r: String = eval(
        &fx,
        "l:SetSelection(1, 5) l:InsertText('Bye') return l:GetText()",
    );
    assert_eq!(r, "Bye world!");
    let r: f64 = eval(&fx, "l:SetText('a\u{1F600}b') return l:GetTextLength()");
    assert_eq!(r, 4.0, "UTF-16 units");
    let r: (f64, f64) = eval(
        &fx,
        "l:SetFont(Turbine.UI.Lotro.Font.Verdana14) local a = l:GetFont() l:SetFont(12345) return a, l:GetFont()",
    );
    assert_eq!(r, (f64::from(0x4200_0001u32), f64::from(0x4200_0001u32)));
    let r: (f64, f64) = eval(
        &fx,
        "l:SetTextAlignment(Turbine.UI.ContentAlignment.MiddleCenter) local a = l:GetTextAlignment() \
         l:SetTextAlignment(42) return a, l:GetTextAlignment()",
    );
    assert_eq!(r, (5.0, 0.0));
    let r: Result<(), _> = fx.try_eval("Probe", "l:SetForeColor(nil)");
    assert!(r.is_err(), "SetForeColor(nil) raises");
    let r: (f64, f64, f64) = eval(
        &fx,
        "local d = l:GetForeColor() l:SetForeColor(Turbine.UI.Color.Yellow) local c = l:GetForeColor() return d.A, c.R, c.G",
    );
    assert_eq!(r, (1.0, 1.0, 1.0));
    let r: (bool, bool, bool, bool) = eval(
        &fx,
        "local a = t:IsReadOnly() t:SetReadOnly(true) l:SetMultiline(false) l:SetMarkupEnabled(true) \
         return a, t:IsReadOnly(), l:IsMultiline(), l:IsMarkupEnabled()",
    );
    assert_eq!(r, (false, true, false, true));
}

#[test]
fn check_box_and_button() {
    let fx = fx_with(
        "n = 0 cb = Turbine.UI.Lotro.CheckBox() function cb:CheckedChanged() n = n + 1 end \
         cb:SetChecked(true) cb:SetChecked(true) cb:SetChecked(false)",
    );
    let r: (f64, bool, f64) = eval(&fx, "return n, cb:IsChecked(), cb:GetCheckAlignment()");
    assert_eq!(r, (2.0, false, 4.0));
}

/// §9.6 worked example of the scroll-bar numbers, through Lua.
#[test]
fn scroll_bar_numbers() {
    let fx = fx_with(
        "n = 0 sb = Turbine.UI.ScrollBar() function sb:ValueChanged() n = n + 1 end \
         sb:SetMinimum(-10)",
    );
    let r: (f64, f64, f64, f64) = eval(
        &fx,
        "return sb:GetMinimum(), sb:GetMaximum(), sb:GetValue(), n",
    );
    assert_eq!(r, (4_294_967_286.0, 4_294_967_286.0, 4_294_967_286.0, 1.0));
    let r: (f64, f64, f64, f64) = eval(
        &fx,
        "sb:SetMaximum(10) return sb:GetMinimum(), sb:GetMaximum(), sb:GetValue(), n",
    );
    assert_eq!(r, (10.0, 10.0, 10.0, 2.0));
    let r: (f64, f64, f64, f64) = eval(
        &fx,
        "local b = Turbine.UI.ScrollBar() local d = { b:GetMinimum(), b:GetMaximum(), b:GetLargeChange(), b:GetSmallChange() } \
         return d[1], d[2], d[3], d[4]",
    );
    assert_eq!(r, (0.0, 100.0, 10.0, 1.0));
    let r: (f64, f64) = eval(
        &fx,
        "local b = Turbine.UI.ScrollBar() local m = 0 function b:ValueChanged() m = m + 1 end \
         b:SetValue(500) local v = b:GetValue() b:SetValue(100) return v, m",
    );
    assert_eq!(
        r,
        (100.0, 1.0),
        "the clamped value fires once; an equal value fires nothing"
    );
    let bound: bool = eval(
        &fx,
        "local lb = Turbine.UI.ListBox() local b = Turbine.UI.ScrollBar() b:SetOrientation(Turbine.UI.Orientation.Vertical) \
         lb:SetVerticalScrollBar(b) local ok = pcall(b.GetValue, b) return not ok and lb:GetVerticalScrollBar() == b",
    );
    assert!(bound, "a bound scroll bar's numbers raise (§13.7)");
}

#[test]
fn list_box_items_and_selection() {
    let fx = fx_with(
        "log = {} lb = Turbine.UI.ListBox() lb:SetSize(100, 60) \
         function lb:ItemAdded(a) log[#log+1] = 'add:' .. a.Item.name end \
         function lb:ItemRemoved(a) log[#log+1] = 'rem:' .. a.Item.name end \
         function lb:SelectedIndexChanged() log[#log+1] = 'sel' end \
         items = {} for i = 1, 5 do local l = Turbine.UI.Label() l.name = 'i' .. i l:SetSize(100, 20) items[i] = l lb:AddItem(l) end",
    );
    let r: (f64, f64, f64, f64) = eval(
        &fx,
        "local _, y3 = items[3]:GetPosition() return lb:GetItemCount(), lb:IndexOfItem(items[4]), y3, lb:GetSelectedIndex()",
    );
    assert_eq!(r, (5.0, 4.0, 40.0, 0.0), "a vertical list of 20-pixel rows");
    let r: (f64, bool, f64) = eval(
        &fx,
        "lb:SetSelectedIndex(2) lb:SetSelectedIndex(2) return lb:GetSelectedIndex(), lb:GetSelectedItem() == items[2], lb:GetItemCount()",
    );
    assert_eq!(r, (2.0, true, 5.0));
    let r: (f64, f64, bool) = eval(
        &fx,
        "lb:RemoveItemAt(2) lb:RemoveItem(items[5]) return lb:GetItemCount(), lb:GetSelectedIndex(), items[2]:GetParent() == nil",
    );
    assert_eq!(r, (3.0, 0.0, true));
    let r: Result<(), _> = fx.try_eval("Probe", "lb:GetItem(4)");
    assert!(r.is_err());
    let order: String = eval(
        &fx,
        "lb:Sort(function(a, b) return a.name > b.name end) \
         local t = {} for i = 1, lb:GetItemCount() do t[i] = lb:GetItem(i).name end return table.concat(t, ',')",
    );
    assert_eq!(order, "i4,i3,i1");
    // An inconsistent comparison must not break the host (§13.7).
    let _: () = eval(&fx, "lb:Sort(function() return math.random() < 0.5 end)");
    let r: f64 = eval(&fx, "return lb:GetItemCount()");
    assert_eq!(r, 3.0);
    let r: (f64, f64) = eval(
        &fx,
        "lb:SetOrientation(Turbine.UI.Orientation.Horizontal) lb:SetMaxItemsPerLine(2) \
         local x, y = lb:GetItem(2):GetPosition() local x3, y3 = lb:GetItem(3):GetPosition() return x + y3, y",
    );
    assert_eq!(r, (120.0, 0.0), "two items per line, the third wraps");
    let at: bool = eval(&fx, "return lb:GetItemAt(5, 25) == lb:GetItem(3)");
    assert!(at);
    let log: String = eval(&fx, "return table.concat(log, ' ')");
    assert_eq!(
        log,
        "add:i1 add:i2 add:i3 add:i4 add:i5 sel rem:i2 sel rem:i5"
    );
    let r: f64 = eval(&fx, "lb:ClearItems() return lb:GetItemCount()");
    assert_eq!(r, 0.0);
    fx.assert_no_errors();
}

#[test]
fn tree_view_nodes() {
    let fx = fx_with(
        "tv = Turbine.UI.TreeView() tv:SetSize(200, 200) \
         local function node(name) local n = Turbine.UI.TreeNode() n:SetSize(150, 20) n.name = name n:SetVisible(true) return n end \
         a = node('a') b = node('b') a1 = node('a1') a2 = node('a2') \
         tv:GetNodes():Add(a) tv:GetNodes():Add(b) a:GetChildNodes():Add(a1) a:GetChildNodes():Add(a2) \
         sel = 0 function tv:SelectedNodeChanged() sel = sel + 1 end",
    );
    let r: (f64, f64, bool, bool, bool) = eval(
        &fx,
        "local _, yb = b:GetPosition() return tv:GetNodes():GetCount(), yb, a1:IsVisible() and not a1:IsDisplayed(), \
         a1:GetParentNode() == a, a:IsExpanded()",
    );
    assert_eq!(r, (2.0, 20.0, true, true, false));
    let r: (f64, f64, f64) = eval(
        &fx,
        "a:Expand() local x1, y1 = a1:GetPosition() local _, yb = b:GetPosition() return x1, y1, yb",
    );
    assert_eq!(r, (20.0, 20.0, 60.0));
    let r: (bool, f64, bool) = eval(
        &fx,
        "tv:SetSelectedNode(a2) return a2:IsSelected(), sel, tv:GetSelectedNode() == a2",
    );
    assert_eq!(r, (true, 1.0, true));
    let r: f64 = eval(
        &fx,
        "tv:SetFilter(function(n) return n.name ~= 'b' end) return select(2, a2:GetPosition())",
    );
    assert_eq!(r, 40.0);
    let r: String = eval(
        &fx,
        "tv:SetSortMethod(function(x, y) return x.name > y.name end) \
         local t = {} for i = 1, a:GetChildNodes():GetCount() do t[i] = a:GetChildNodes():Get(i).name end return table.concat(t, ',')",
    );
    assert_eq!(r, "a2,a1");
    let r: Result<(), _> = fx.try_eval("Probe", "a1:GetChildNodes():Add(a)");
    assert!(r.is_err(), "a node cannot be added below itself");
    let r: (f64, bool) = eval(
        &fx,
        "tv:GetNodes():Remove(a) return tv:GetNodes():GetCount(), a1:GetParentNode() == a",
    );
    assert_eq!(r, (1.0, true));
    fx.assert_no_errors();
}

#[test]
fn context_menu_objects() {
    let fx = fx_with(
        "m = Turbine.UI.ContextMenu() items = m:GetItems() \
         a = Turbine.UI.MenuItem('Alpha') b = Turbine.UI.MenuItem('Beta', false, true) \
         items:Add(a) items:Add(b) sub = Turbine.UI.MenuItem('Sub') a:GetItems():Add(sub)",
    );
    let r: (f64, String, bool, bool, bool, f64) = eval(
        &fx,
        "return items:GetCount(), items:Get(2):GetText(), b:IsEnabled(), b:IsChecked(), a:IsEnabled(), a:GetItems():GetCount()",
    );
    assert_eq!(r, (2.0, "Beta".into(), false, true, true, 1.0));
    let r: (f64, bool) = eval(
        &fx,
        "items:RemoveAt(1) return items:GetCount(), m:GetItems() == items",
    );
    assert_eq!(r, (1.0, true));
    let r: Result<(), _> = fx.try_eval("Probe", "items:Add(Turbine.UI.Control())");
    assert!(r.is_err(), "only menu items go into a MenuItemList");
}

#[test]
fn shortcuts_and_quickslots() {
    let fx = fx_with(
        "n = 0 q = Turbine.UI.Lotro.Quickslot() function q:ShortcutChanged() n = n + 1 end \
         s = Turbine.UI.Lotro.Shortcut(Turbine.UI.Lotro.ShortcutType.Alias, '/say hi') q:SetShortcut(s)",
    );
    let r: (f64, String, f64, bool) = eval(
        &fx,
        "local g = q:GetShortcut() return g:GetType(), g:GetData(), n, g ~= s",
    );
    assert_eq!(r, (4.0, "/say hi".into(), 1.0, true));
    let r: Result<(), _> = fx.try_eval("Probe", "q:SetShortcut('nope')");
    assert!(r.is_err(), "an invalid shortcut raises (§14.6)");
    let r: (f64, f64, String) = eval(
        &fx,
        "local x = Turbine.UI.Lotro.Shortcut() x:SetType(3) local t3 = x:GetType() x:SetType(8) x:SetData('kept?') \
         local y = Turbine.UI.Lotro.Shortcut(Turbine.UI.Lotro.ShortcutType.Emote, '0x0000002A') \
         return t3, x:GetType(), y:GetData()",
    );
    assert_eq!(r, (0.0, 8.0, "0x0000002A".into()));
    let r: (bool, bool) = eval(
        &fx,
        "local a = q:IsUseOnRightClick() q:SetUseOnRightClick(true) return a, q:IsUseOnRightClick()",
    );
    assert_eq!(r, (false, true));
}

/// §13.21 worked example (parts that do not need a scaler layout).
#[test]
fn window_scaling_and_rotation() {
    let fx = fx_with("w = Turbine.UI.Window() w:SetSize(300, 200) w:SetPosition(100, 100)");
    let r: (f64, f64) = eval(&fx, "return w:GetScale(), w:GetUIScalingGroup()");
    assert_eq!(r, (1.0, 0.0), "unmarked");
    let r: (f64, f64) = eval(
        &fx,
        "w:RegisterForGlobalScaling() w:SetScale(1.5) return w:GetScale(), w:GetUIScalingGroup()",
    );
    assert_eq!(r, (1.5, 72.0));
    let r: Result<(), _> = fx.try_eval("Probe", "w:SetScale(0)");
    assert!(r.is_err(), "a scale not greater than 0 raises");
    let r: (f64, f64) = eval(
        &fx,
        "w:SetUIScalingGroup(Turbine.UI.Lotro.UIScalingGroupType.Global) return w:GetUIScalingGroup(), w:GetScale()",
    );
    assert_eq!(
        r,
        (72.0, 1.5),
        "Global on a plain window becomes 72; the window's own scale stays its value (rule 2)"
    );
    let r: (f64, f64, f64) = eval(
        &fx,
        "w:SetVisible(true) w:SetRotation({ x = 0, y = 0, z = 90 }) local r = w:GetRotation() return r.x, r.y, r.z",
    );
    assert_eq!(r, (0.0, 0.0, 90.0));
    let child: f64 = eval(
        &fx,
        "local c = Turbine.UI.Window() c:SetParent(w) c:SetScale(3) return c:GetScale()",
    );
    assert_eq!(
        child, 1.0,
        "scaling members do nothing on a window with a parent"
    );
}

/// §13.21 rule 1, §14.2.1 table (last row): the first marking of a plain window re-describes
/// it as a bare `UIScaler` element (type 0x100000E8): no size limits, the blend-mode mapping
/// of a rebuild (§14.2.1 step 3), and the window keeps its element, position, size and
/// children. A second marking changes nothing more.
#[test]
fn marked_plain_window_becomes_a_bare_scaler() {
    let fx = fx_with(
        "w = Turbine.UI.Window() w:SetSize(300, 200) w:SetPosition(100, 100) \
         w:SetMinimumSize(250, 150) w:SetBlendMode(Turbine.UI.BlendMode.AlphaBlend) \
         c = Turbine.UI.Control() c:SetParent(w) c:SetPosition(10, 20)",
    );
    let before = fx.control_key("Probe", "w");
    let ty = fx.pui.ui().borrow().element_type(before);
    assert_ne!(ty, Some(0x1000_00E8), "an unmarked window is no scaler");
    let r: (f64, f64, f64) = eval(
        &fx,
        "w:SetScale(1) local mw, mh = w:GetMinimumSize() return mw, mh, w:GetBlendMode()",
    );
    assert_eq!(
        r,
        (0.0, 0.0, 2.0),
        "no size limits; AlphaBlend (4) → operation 2"
    );
    let after = fx.control_key("Probe", "w");
    assert_eq!(after, before, "the element is kept");
    assert_eq!(fx.pui.ui().borrow().element_type(after), Some(0x1000_00E8));
    let r: (f64, f64, f64, f64, f64, f64) = eval(
        &fx,
        "local x, y = w:GetPosition() local cx, cy = c:GetPosition() \
         return x, y, w:GetWidth(), w:GetHeight(), cx, cy",
    );
    assert_eq!(r, (100.0, 100.0, 300.0, 200.0, 10.0, 20.0));
    let parent: bool = eval(&fx, "return c:GetParent() == w");
    assert!(parent, "the child stays the window's");
    // Marking again does not rebuild again (the mapping would turn 2 into 0).
    let m: f64 = eval(&fx, "w:SetScale(1.5) return w:GetBlendMode()");
    assert_eq!(m, 2.0);
}

/// §13.21 rule 2: `SetUIScalingGroup` keeps the window's own scale; only `SetScale` sets
/// the group back to 72.
#[test]
fn scaling_group_keeps_the_own_scale() {
    let fx = fx_with("w = Turbine.UI.Window() w:SetSize(300, 200)");
    let r: (f64, f64, f64, f64) = eval(
        &fx,
        "w:SetScale(2) local a = w:GetScale() \
         w:SetUIScalingGroup(Turbine.UI.Lotro.UIScalingGroupType.Toolbar) \
         return a, w:GetScale(), w:GetUIScalingGroup(), Turbine.UI.Lotro.UIScalingGroupType.Toolbar",
    );
    assert_eq!((r.0, r.1), (2.0, 2.0));
    assert_eq!(r.2, r.3, "the group is stored");
    let g: f64 = eval(&fx, "w:SetScale(1.5) return w:GetUIScalingGroup()");
    assert_eq!(g, 72.0);
}

/// §13.21 rules 5 and 7: the scaling origin is the point that stays in place.
#[test]
fn scaling_origin_point_stays_in_place() {
    let mut fx = fx_with(
        "a = Turbine.UI.Window() a:SetSize(300, 200) a:SetPosition(700, 500) a:SetVisible(true) \
         b = Turbine.UI.Window() b:SetSize(300, 200) b:SetPosition(700, 500) b:SetVisible(true)",
    );
    let _: () = eval(
        &fx,
        "a:SetScale(1) a:SetScalingOriginPoint(0, 0) a:SetScale(2) \
         b:SetScale(1) b:SetScalingOriginPoint(300, 200) b:SetScale(2)",
    );
    fx.frame(0.016);
    let (al, at, aw, ah) = fx.screen_box("Probe", "a");
    let (bl, bt, bw, bh) = fx.screen_box("Probe", "b");
    assert_eq!((al, at), (700, 500), "origin (0, 0) keeps the top-left");
    assert_eq!((aw, ah), (bw, bh), "same size either way");
    // The bottom-right corner (the origin 300, 200 at scale 1) stays at (1000, 700).
    assert!(
        (bl + bw - 1000).abs() <= 1 && (bt + bh - 700).abs() <= 1,
        "{bl},{bt},{bw},{bh}"
    );
    let o: (f64, f64) = eval(&fx, "return b:GetScalingOriginPoint()");
    assert_eq!(o, (300.0, 200.0));
}

/// `Display` reads in the one space (`ui-scaling.md` §3.5, §7.7): the size and the mouse
/// position are UI units, the global factor is 1.0 for plugins, and `Display` follows the UI
/// screen `F` when it changes.
#[test]
fn display_is_the_ui_screen_in_ui_units() {
    let mut fx = fx_with("");
    // The size and the per-axis getters are the UI screen in UI units.
    let (w, h, gw, gh): (f64, f64, f64, f64) = eval(
        &fx,
        "local w, h = Turbine.UI.Display.GetSize() \
         return w, h, Turbine.UI.Display.GetWidth(), Turbine.UI.Display.GetHeight()",
    );
    assert_eq!(
        (w, h, gw, gh),
        (
            f64::from(harness::SCREEN.0),
            f64::from(harness::SCREEN.1),
            f64::from(harness::SCREEN.0),
            f64::from(harness::SCREEN.1),
        )
    );
    // The global factor is 1.0 for plugins (§7.7); a group that was never scaled is 1.0 too.
    let (global, unset): (f64, f64) = eval(
        &fx,
        "return Turbine.UI.Display.GetUIGroupScale(Turbine.UI.Lotro.UIScalingGroupType.Global), \
                Turbine.UI.Display.GetUIGroupScale(Turbine.UI.Lotro.UIScalingGroupType.Toolbar)",
    );
    assert_eq!((global, unset), (1.0, 1.0));
    // When F changes (a window resize or a UI-scale change recomputes Wu x Hu), Display
    // follows it, still in UI units.
    fx.pui.ui().borrow_mut().resize_screen(1280, 720);
    let (w2, h2): (f64, f64) = eval(&fx, "local w, h = Turbine.UI.Display.GetSize() return w, h");
    assert_eq!((w2, h2), (1280.0, 720.0));
    // The mouse position is in UI units (floor of ux, uy; §7.7); the three getters agree.
    fx.move_to(640, 360);
    let (px, py, mx, my): (f64, f64, f64, f64) = eval(
        &fx,
        "local x, y = Turbine.UI.Display.GetMousePosition() \
         return x, y, Turbine.UI.Display.GetMouseX(), Turbine.UI.Display.GetMouseY()",
    );
    assert_eq!((px, py, mx, my), (640.0, 360.0, 640.0, 360.0));
}

/// A scaling group (A.7) under two scales (`ui-scaling.md` §7.4, §7.7; §13.21 rules 2, 6,
/// 8): the window follows the group's value, `Display.GetUIGroupScale` and `GetScale` report
/// it, and `UIScaleChanged` fires on each change. The global factor stays 1.0.
#[test]
fn scaling_group_under_two_scales() {
    let mut fx = fx_with(
        "w = Turbine.UI.Window() w:SetSize(300, 200) w:SetPosition(100, 100) w:SetVisible(true) \
         w.changes = {} \
         w.UIScaleChanged = function(_, a) w.changes[#w.changes + 1] = a.Value end \
         w:SetUIScalingGroup(Turbine.UI.Lotro.UIScalingGroupType.Toolbar)",
    );
    // Group membership: the window follows Toolbar (6); with no own scale, its effective
    // scale is the group's value, 1.0 by default (a group without a definition entry).
    let (group, scale): (f64, f64) = eval(&fx, "return w:GetUIScalingGroup(), w:GetScale()");
    assert_eq!((group, scale), (6.0, 1.0));
    // Unscaled, the window is its natural 300 x 200 on screen.
    fx.frame(0.016);
    let (_, _, w0, h0) = fx.screen_box("Probe", "w");
    assert_eq!((w0, h0), (300, 200));

    // First scale: the group's value becomes 1.5.
    let _ = fx.pui.scale_changed(&fx.manager, 6, 1.5);
    let (gs, ws): (f64, f64) = eval(
        &fx,
        "return Turbine.UI.Display.GetUIGroupScale(6), w:GetScale()",
    );
    assert_eq!(
        (gs, ws),
        (1.5, 1.5),
        "Display and GetScale follow the group"
    );
    // The group value was re-applied as a real node scale (§7.7, LPA §13.21 rule 5): the
    // window actually resizes on screen, not only in GetScale (pins the reapply loop of
    // `window::scale_changed`, which GetScale/GetUIGroupScale alone would not catch).
    fx.frame(0.016);
    let (_, _, w1, h1) = fx.screen_box("Probe", "w");
    assert!(
        (w1 - 450).abs() <= 1 && (h1 - 300).abs() <= 1,
        "group scale 1.5 resizes the window: {w1}x{h1}"
    );

    // Second scale: the group's value becomes 0.75.
    let _ = fx.pui.scale_changed(&fx.manager, 6, 0.75);
    let (gs, ws): (f64, f64) = eval(
        &fx,
        "return Turbine.UI.Display.GetUIGroupScale(6), w:GetScale()",
    );
    assert_eq!((gs, ws), (0.75, 0.75));
    // Re-applied again: the window shrinks on screen to the new factor.
    fx.frame(0.016);
    let (_, _, w2, h2) = fx.screen_box("Probe", "w");
    assert!(
        (w2 - 225).abs() <= 1 && (h2 - 150).abs() <= 1,
        "group scale 0.75 resizes the window: {w2}x{h2}"
    );

    // The global factor never enters a plugin's scale.
    let g: f64 = eval(&fx, "return Turbine.UI.Display.GetUIGroupScale(60)");
    assert_eq!(g, 1.0);

    // Both changes reached the window's UIScaleChanged handler, in order (§13.21 rule 8).
    let (a, b, n): (f64, f64, f64) = eval(&fx, "return w.changes[1], w.changes[2], #w.changes");
    assert_eq!((a, b, n), (1.5, 0.75, 2.0));
}

#[test]
fn game_styled_window_members() {
    let fx = fx_with("w = Turbine.UI.Lotro.Window() w:SetSize(400, 300) w:SetText('Caption')");
    let r: (String, bool, f64, bool) = eval(
        &fx,
        "local a = w:IsResizable() w:SetResizable(true) return w:GetText(), a, w:GetControls():GetCount(), w:IsResizable()",
    );
    assert_eq!(
        r,
        ("Caption".into(), false, 0.0, true),
        "frame parts are not plugin controls"
    );
}

#[test]
fn objects_per_apartment_are_bounded() {
    let fx = fx_with("");
    let r: (bool, f64) = eval(
        &fx,
        "local ok, n = true, 0 \
         keep = {} \
         while ok and n < 70000 do local o ok, o = pcall(Turbine.UI.Lotro.Shortcut) if ok then n = n + 1 keep[n] = o end end \
         return ok, n",
    );
    assert!(!r.0, "creation fails past the cap");
    assert!(r.1 <= tessera_plugins::ui::MAX_OBJECTS_PER_APARTMENT as f64);
}

/// LPA §9.6 (Thumb): the thumb fractions change only where the spec says.
#[test]
fn thumb_fractions_follow_the_spec() {
    let fx = fx_with(
        "sb = Turbine.UI.ScrollBar() sb:SetOrientation(Turbine.UI.Orientation.Vertical) sb:SetSize(10, 100) \
         sb:SetVisible(true) th = Turbine.UI.Control() th:SetVisible(true) sb:SetThumbButton(th)",
    );
    let h: f64 = eval(&fx, "return th:GetHeight()");
    assert_eq!(h, 10.0, "large 10 / range 100 of a 100-pixel track");
    let h: f64 = eval(&fx, "sb:SetLargeChange(50) return th:GetHeight()");
    assert_eq!(h, 10.0, "SetLargeChange does not update the size");
    let t: f64 = eval(&fx, "sb:SetValue(50) return th:GetTop()");
    assert_eq!(t, 45.0, "position 0.5 of the 90 free pixels");
    let r: (f64, f64) = eval(&fx, "sb:SetMaximum(200) return th:GetHeight(), th:GetTop()");
    assert_eq!(
        r,
        (25.0, 37.0),
        "a bound change sets the size (50 / 200); the value did not move, so the position stays 0.5"
    );
}
