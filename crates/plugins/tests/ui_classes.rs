// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Turbine.UI` and `Turbine.UI.Lotro` tables: classes, enumerations, `Color`, the
//! object model of the UI classes and writable class tables (`lua-plugin-api.md` §8,
//! §13.9, §16, Appendix A; `plugin-api-usage.md` §6.3, §7.1, §7.3). Synthetic plugins
//! written for these tests.

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

/// Every class, static and enumeration of the first API surface's UI part
/// (`plugin-api-usage.md` §7.1) exists, with `IsA`.
#[test]
fn first_surface_names_exist() {
    let fx = fx_with("");
    let names = [
        "Turbine.UI.Control",
        "Turbine.UI.ScrollableControl",
        "Turbine.UI.Window",
        "Turbine.UI.Label",
        "Turbine.UI.TextBox",
        "Turbine.UI.Button",
        "Turbine.UI.CheckBox",
        "Turbine.UI.ScrollBar",
        "Turbine.UI.ListBox",
        "Turbine.UI.ControlList",
        "Turbine.UI.Color",
        "Turbine.UI.Display",
        "Turbine.UI.Graphic",
        "Turbine.UI.DragDropInfo",
        "Turbine.UI.ContextMenu",
        "Turbine.UI.MenuItem",
        "Turbine.UI.MenuItemList",
        "Turbine.UI.TreeView",
        "Turbine.UI.TreeNode",
        "Turbine.UI.TreeNodeList",
        "Turbine.UI.BlendMode",
        "Turbine.UI.ContentAlignment",
        "Turbine.UI.EdgeAttachmentType",
        "Turbine.UI.FontStyle",
        "Turbine.UI.MouseButton",
        "Turbine.UI.Orientation",
        "Turbine.UI.HorizontalLayout",
        "Turbine.UI.VerticalLayout",
        "Turbine.UI.Lotro.Window",
        "Turbine.UI.Lotro.GoldWindow",
        "Turbine.UI.Lotro.Button",
        "Turbine.UI.Lotro.GoldButton",
        "Turbine.UI.Lotro.CheckBox",
        "Turbine.UI.Lotro.TextBox",
        "Turbine.UI.Lotro.ScrollBar",
        "Turbine.UI.Lotro.LotroUI",
        "Turbine.UI.Lotro.LotroUIElement",
        "Turbine.UI.Lotro.Action",
        "Turbine.UI.Lotro.Font",
        "Turbine.UI.Lotro.Quickslot",
        "Turbine.UI.Lotro.Shortcut",
        "Turbine.UI.Lotro.ShortcutType",
        "Turbine.UI.Lotro.DragDropInfo",
        "Turbine.UI.Lotro.UIScalingGroupType",
        "Turbine.UI.Lotro.BaseItemControl",
        "Turbine.UI.Lotro.ItemControl",
        "Turbine.UI.Lotro.ItemInfoControl",
        "Turbine.UI.Lotro.EquipmentSlot",
        "Turbine.UI.Lotro.EntityControl",
        "Turbine.UI.Lotro.EffectDisplay",
    ];
    for n in names {
        let ok: bool = fx.eval(
            "Probe",
            &format!("return type({n}) == 'table' and type({n}.IsA) == 'function'"),
        );
        assert!(ok, "{n}");
    }
    // The class chain of §16.
    let chain: bool = fx.eval(
        "Probe",
        "local w = Turbine.UI.Lotro.GoldWindow() \
         return w:IsA(Turbine.UI.Lotro.Window) and w:IsA(Turbine.UI.Window) \
            and w:IsA(Turbine.UI.Control) and w:IsA(Turbine.Object) \
            and not w:IsA(Turbine.UI.Label)",
    );
    assert!(chain);
    let label_chain: bool = fx.eval(
        "Probe",
        "local b = Turbine.UI.Lotro.GoldButton() \
         return b:IsA(Turbine.UI.Button) and b:IsA(Turbine.UI.Label) \
            and b:IsA(Turbine.UI.ScrollableControl)",
    );
    assert!(label_chain);
    fx.assert_no_errors();
}

#[test]
fn enumeration_values() {
    let fx = fx_with("");
    let checks = [
        ("Turbine.UI.BlendMode.AlphaBlend", 4.0),
        ("Turbine.UI.BlendMode.None", 8.0),
        ("Turbine.UI.BlendMode.Undefined", 8.0),
        ("Turbine.UI.ContentAlignment.MiddleCenter", 5.0),
        ("Turbine.UI.FontStyle.Outline", 8.0),
        ("Turbine.UI.MouseButton.Middle", 4.0),
        ("Turbine.UI.Orientation.Vertical", 1.0),
        ("Turbine.UI.EdgeAttachmentType.RatioProportional", 6.0),
        ("Turbine.UI.Lotro.ShortcutType.Alias", 4.0),
        ("Turbine.UI.Lotro.ShortcutType.Undef", 0.0),
        (
            "Turbine.UI.Lotro.Font.TrajanPro14",
            f64::from(0x4200_000Cu32),
        ),
        (
            "Turbine.UI.Lotro.Font.LucidaConsole12",
            f64::from(0x4200_0028u32),
        ),
        ("Turbine.UI.Lotro.Action.Escape", 145.0),
        (
            "Turbine.UI.Lotro.Action.ToggleBags",
            f64::from(0x1000_0094u32),
        ),
        ("Turbine.UI.Lotro.UIScalingGroupType.Global", 60.0),
        ("Turbine.UI.Lotro.UIScalingGroupType.LootRoll", 71.0),
        (
            "Turbine.UI.Lotro.LotroUIElement.Target",
            f64::from(0x1000_000Bu32),
        ),
    ];
    for (expr, want) in checks {
        let v: f64 = fx.eval("Probe", &format!("return {expr}"));
        assert_eq!(v, want, "{expr}");
    }
    let counts: (u32, u32, u32) = fx.eval(
        "Probe",
        "local function n(t) local c = 0 for k, v in pairs(t) do if type(v) == 'number' then c = c + 1 end end return c end \
         return n(Turbine.UI.Lotro.Action), n(Turbine.UI.Lotro.Font), n(Turbine.UI.Lotro.UIScalingGroupType)",
    );
    assert_eq!(counts, (270, 49, 57));
}

/// `Color` (§13.9): constructor forms, single precision, named colours.
#[test]
fn colours() {
    let fx = fx_with("");
    let (a, r, g, b): (f64, f64, f64, f64) = fx.eval(
        "Probe",
        "local c = Turbine.UI.Color(0.1, 0.2, 0.3) return c.A, c.R, c.G, c.B",
    );
    assert_eq!(
        (a, r, g, b),
        (1.0, f64::from(0.1f32), f64::from(0.2f32), f64::from(0.3f32))
    );
    let four: (f64, f64) = fx.eval(
        "Probe",
        "local c = Turbine.UI.Color(0.5, 1, 0, 0) return c.A, c.R",
    );
    assert_eq!(four, (0.5, 1.0));
    let none: (f64, f64, f64, f64) = fx.eval(
        "Probe",
        "local c = Turbine.UI.Color() return c.A, c.R, c.G, c.B",
    );
    assert_eq!(none, (0.0, 0.0, 0.0, 0.0));
    let two: (f64, f64) = fx.eval("Probe", "local c = Turbine.UI.Color(1, 1) return c.A, c.R");
    assert_eq!(two, (0.0, 0.0));
    let nonnum: (f64, f64) = fx.eval(
        "Probe",
        "local c = Turbine.UI.Color('x', 1, 1) return c.A, c.R",
    );
    assert_eq!(nonnum, (1.0, 0.0));
    let alice: (f64, f64, f64) = fx.eval(
        "Probe",
        "local c = Turbine.UI.Color.AliceBlue return c.R, c.G, c.B",
    );
    assert_eq!(alice, (f64::from(0.941f32), f64::from(0.973f32), 1.0));
    let gray: f64 = fx.eval("Probe", "return Turbine.UI.Color.Gray.G");
    assert_eq!(gray, f64::from(0.502f32));
    let transparent: (f64, f64) = fx.eval(
        "Probe",
        "return Turbine.UI.Color.Transparent.A, Turbine.UI.Color.Transparent.R",
    );
    assert_eq!(transparent, (0.0, 1.0));
    let isa: bool = fx.eval("Probe", "return Turbine.UI.Color.Red:IsA(Turbine.UI.Color)");
    assert!(isa);
}

/// Writable class tables (§8.6; `plugin-api-usage.md` §6.3): a wrapper stored on
/// `Control` reaches instances of every class below it, created before or after; added
/// methods are found; enumeration values can be added and replaced.
#[test]
fn writable_class_tables() {
    let fx = fx_with(
        "early = Turbine.UI.Label() \
         local original = Turbine.UI.Control.SetBackground \
         wrapped = 0 \
         function Turbine.UI.Control.SetBackground(self, v) wrapped = wrapped + 1 return original(self, v) end \
         function Turbine.UI.Control:SetToolTip(t) self.tip = t end \
         Turbine.UI.Lotro.Action.MyAction = 0x10009999 \
         Turbine.UI.BlendMode.None = 99",
    );
    let n: f64 = fx.eval(
        "Probe",
        "early:SetBackground(nil) \
         local w = Turbine.UI.Lotro.Window() w:SetBackground(0x41000001) \
         local b = Turbine.UI.Button() b:SetToolTip('hi') \
         return wrapped + (b.tip == 'hi' and 100 or 0)",
    );
    assert_eq!(n, 102.0);
    let v: (f64, f64) = fx.eval(
        "Probe",
        "return Turbine.UI.Lotro.Action.MyAction, Turbine.UI.BlendMode.None",
    );
    assert_eq!(v, (f64::from(0x1000_9999u32), 99.0));
    fx.assert_no_errors();
}

/// Subclassing from Lua (§8.3) with the built-in class helper and with a hand-written
/// one; native methods and events work on the subclass instance.
#[test]
fn lua_subclasses() {
    let fx = fx_with(
        "MyWindow = class(Turbine.UI.Lotro.Window) \
         function MyWindow:Constructor(title) \
           Turbine.UI.Lotro.Window.Constructor(self) \
           self:SetText(title) self:SetSize(300, 200) \
           self.label = Turbine.UI.Label() self.label:SetParent(self) \
         end \
         function MyWindow:GetSize() return 1, 2 end \
         local Hand = {} \
         setmetatable(Hand, { __index = Turbine.UI.Control, __call = function(c, ...) \
            local o = setmetatable({}, { __index = c }) c.Constructor(o, ...) return o end }) \
         function Hand:Constructor() Turbine.UI.Control.Constructor(self) self.hand = true end \
         win = MyWindow('Title') \
         hand = Hand()",
    );
    let r: (String, f64, f64, f64, bool, bool) = fx.eval(
        "Probe",
        "local w, h = win:GetSize() \
         return win:GetText(), w, h, Turbine.UI.Control.GetWidth(win), win:IsA(Turbine.UI.Window), \
                hand:IsA(Turbine.UI.Control) and hand.hand",
    );
    assert_eq!(r, ("Title".into(), 1.0, 2.0, 300.0, true, true));
    let parent_is_win: bool = fx.eval("Probe", "return win.label:GetParent() == win");
    assert!(parent_is_win, "identity of GetParent (§8.2)");
    fx.assert_no_errors();
}

/// A method called with the wrong `self` raises a Lua error, never a crash (§8.2).
#[test]
fn wrong_self_is_an_error() {
    let fx = fx_with("");
    for code in [
        "Turbine.UI.Control.GetWidth({})",
        "Turbine.UI.Control.GetWidth(nil)",
        "Turbine.UI.Label.SetText(Turbine.UI.Window(), 'x')",
        "Turbine.UI.ListBox.AddItem(Turbine.UI.Control(), Turbine.UI.Label())",
        "Turbine.UI.Lotro.Shortcut.GetData(Turbine.UI.Control())",
    ] {
        let r: Result<(), _> = fx.try_eval("Probe", code);
        assert!(r.is_err(), "{code} should raise");
    }
    let ok: bool = fx.eval(
        "Probe",
        "return pcall(Turbine.UI.Control.SetWidth, 5, 5) == false",
    );
    assert!(ok);
}

/// `Display` and `LotroUI` (§13.10, §14.3).
#[test]
fn statics() {
    let fx = fx_with("");
    let size: (f64, f64, f64, f64) = fx.eval(
        "Probe",
        "local w, h = Turbine.UI.Display.GetSize() return w, h, Turbine.UI.Display.GetWidth(), Turbine.UI.Display.GetUIGroupScale(60)",
    );
    assert_eq!(
        size,
        (
            f64::from(harness::SCREEN.0),
            f64::from(harness::SCREEN.1),
            f64::from(harness::SCREEN.0),
            1.0
        )
    );
    let flags: (bool, bool, bool) = fx.eval(
        "Probe",
        "local L = Turbine.UI.Lotro.LotroUI local E = Turbine.UI.Lotro.LotroUIElement \
         local a = L.IsEnabled(E.Backpack1) L.SetEnabled(E.Backpack1, false) local b = L.IsEnabled(E.Backpack1) \
         L.Reset() return a, b, L.IsEnabled(E.Backpack1)",
    );
    assert_eq!(flags, (true, false, true));
    let keys: (bool, bool, bool) = fx.eval(
        "Probe",
        "return Turbine.UI.Control.IsAltKeyDown(), Turbine.UI.Control.IsControlKeyDown(), Turbine.UI.Control.IsShiftKeyDown()",
    );
    assert_eq!(keys, (false, false, false));
}
