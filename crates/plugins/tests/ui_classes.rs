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

// ----- game-styled controls built from a layout (§14.2.1–§14.2.3) ------------------------

mod styled {
    use std::sync::Arc;

    use super::harness::Fx;
    use tessera_formats::did::Did;
    use tessera_formats::property::PropertyValue;
    use tessera_formats::ui_layout::{EdgeMode, ElementDesc, UiLayout};
    use tessera_ui::text::TextLayout;
    use tessera_ui::{ElementKey, MemoryEnv, PreparedText, Vocab};

    /// Invented DID of the synthetic `luaplugins` layout.
    const LAYOUT: u32 = 0x2200_0001;
    /// The `UILAYOUT` entry key of §14.2.1 step 1.
    const KEY: u32 = 0x1000_00C8;

    fn typed(id: u32, ty: u32, (x, y, w, h): (i32, i32, i32, i32), e: [u32; 4]) -> ElementDesc {
        let mut d = ElementDesc {
            element_id: id,
            element_type: ty,
            ..ElementDesc::default()
        };
        d.block.rect.x = x;
        d.block.rect.y = y;
        d.block.rect.width = w;
        d.block.rect.height = h;
        d.block.has_position = true;
        d.edges.left = EdgeMode(e[0]);
        d.edges.top = EdgeMode(e[1]);
        d.edges.right = EdgeMode(e[2]);
        d.edges.bottom = EdgeMode(e[3]);
        d
    }

    fn z(mut d: ElementDesc, z: i32, read_order: u32) -> ElementDesc {
        d.block.z_level = z;
        d.read_order = read_order;
        d
    }

    fn with_children(mut d: ElementDesc, children: Vec<ElementDesc>) -> ElementDesc {
        d.children_size.count = children.len() as u32;
        d.children = children;
        d
    }

    /// Invented DID of the title-bar art: a solid colour, so the part's pixels can be found
    /// in the window's canvas.
    const TITLE_ART: u32 = 0x41FF_0001;
    /// The title-bar art's colour.
    const TITLE_COLOUR: u32 = 0xFFC0_8040;
    /// A marker property the synthetic scaled containers carry (a Tessera placeholder PID no
    /// dictionary uses), so a test can tell which container a window was built from.
    const SCALED_MARK: u32 = 0xFFFD_0001;

    /// A window container of 300 × 300 with the parts of §14.2.3; their design rectangles
    /// and edge modes give the table's rectangles through the 50 × 50 creation size.
    fn window(id: u32) -> ElementDesc {
        let t = Vocab::default().types;
        let (field, text, button, drag, resize) =
            (t.field, t.text, t.button, t.dragbar, t.resize_bar);
        let parts = vec![
            z(
                typed(0x1000_2295, field, (0, 16, 300, 282), [0, 0, 5, 5]),
                100,
                1,
            ),
            z(
                {
                    let mut d = typed(0x1000_2296, field, (50, -8, 200, 41), [0; 4]);
                    d.block
                        .media
                        .push(tessera_formats::ui_layout::MediaItem::Image {
                            did: Did(TITLE_ART),
                            draw_mode: tessera_formats::ui_layout::DrawMode(2),
                            path: String::new(),
                        });
                    d
                },
                90,
                2,
            ),
            // The layout gives the title text margins; the title layout sets them to 0
            // (§14.2.3 step 2).
            z(
                prop(
                    prop(
                        typed(0x1000_2297, text, (90, 5, 120, 24), [0; 4]),
                        Vocab::default().pids.text_left_margin,
                        PropertyValue::Int(10),
                    ),
                    Vocab::default().pids.text_right_margin,
                    PropertyValue::Int(10),
                ),
                80,
                3,
            ),
            z(typed(0x1000_2DFC, resize, (0, 16, 20, 20), [0; 4]), 10, 4),
            z(
                typed(0x1000_2DFD, resize, (20, 16, 260, 10), [0, 0, 5, 0]),
                10,
                5,
            ),
            z(
                typed(0x1000_2DFE, resize, (280, 16, 20, 20), [4, 0, 5, 0]),
                10,
                6,
            ),
            z(
                typed(0x1000_2DFF, resize, (0, 36, 10, 244), [0, 0, 0, 5]),
                10,
                7,
            ),
            z(
                typed(0x1000_2E00, resize, (290, 36, 10, 244), [4, 0, 5, 5]),
                10,
                8,
            ),
            z(
                typed(0x1000_2E01, resize, (0, 280, 20, 20), [0, 4, 0, 5]),
                10,
                9,
            ),
            z(
                typed(0x1000_2E02, resize, (20, 290, 260, 10), [0, 4, 5, 5]),
                10,
                10,
            ),
            z(
                typed(0x1000_2E03, resize, (280, 280, 20, 20), [4, 4, 5, 5]),
                10,
                11,
            ),
            z(
                typed(0x1000_2298, button, (279, 21, 16, 16), [4, 0, 5, 0]),
                5,
                12,
            ),
            z(typed(0x1000_2DFB, drag, (50, -8, 200, 41), [0; 4]), 0, 13),
        ];
        with_children(typed(id, field, (0, 0, 300, 300), [0; 4]), parts)
    }

    /// Arrow buttons and thumb of the synthetic scroll bars.
    const DEC: u32 = 0x1000_3001;
    const INC: u32 = 0x1000_3002;

    fn prop(mut d: ElementDesc, pid: u32, value: PropertyValue) -> ElementDesc {
        d.block
            .properties
            .entries
            .push(tessera_formats::property::PropertyEntry { pid, value });
        d.block.properties.size.count = d.block.properties.entries.len() as u32;
        d
    }

    /// A scroll bar of 200 × 16 (horizontal) or 16 × 200 with its arrow buttons and thumb.
    fn scroll_bar(id: u32, vertical: bool) -> ElementDesc {
        let v = Vocab::default();
        let (t, p) = (v.types, v.pids);
        let (rect, dec, inc, thumb, inc_edges) = if vertical {
            (
                (0, 0, 16, 200),
                (0, 0, 16, 16),
                (0, 184, 16, 16),
                (0, 16, 16, 16),
                [0, 4, 0, 5],
            )
        } else {
            (
                (0, 0, 200, 16),
                (0, 0, 16, 16),
                (184, 0, 16, 16),
                (16, 0, 16, 16),
                [4, 0, 5, 0],
            )
        };
        let mut bar = typed(id, t.scrollbar, rect, [0; 4]);
        bar = prop(bar, p.sb_horizontal, PropertyValue::Bool(!vertical));
        bar = prop(bar, p.sb_dec_button, PropertyValue::Enum(DEC));
        bar = prop(bar, p.sb_inc_button, PropertyValue::Enum(INC));
        with_children(
            bar,
            vec![
                z(typed(DEC, t.button, dec, [0; 4]), 0, 1),
                z(typed(INC, t.button, inc, inc_edges), 0, 2),
                z(typed(v.ids.widget, t.button, thumb, [0; 4]), 0, 3),
            ],
        )
    }

    fn layout() -> UiLayout {
        let t = Vocab::default().types;
        let text_box = with_children(
            typed(0x1000_22A9, t.field, (0, 0, 200, 24), [0; 4]),
            vec![typed(0x1000_22AA, t.text, (4, 4, 192, 16), [0, 0, 5, 5])],
        );
        let check = with_children(
            typed(0x1000_25AF, t.button, (0, 0, 120, 20), [0; 4]),
            vec![
                typed(0x1000_25B0, t.button, (0, 2, 16, 16), [0; 4]),
                typed(0x1000_25B1, t.text, (20, 0, 100, 20), [0, 0, 5, 5]),
            ],
        );
        let scaled = |id: u32| prop(window(id), SCALED_MARK, PropertyValue::Int(id as i32));
        let elements = vec![
            window(0x1000_2294),
            window(0x1000_2299),
            scaled(0x1000_4D37),
            scaled(0x1000_4D38),
            typed(0x1000_22A7, t.button, (0, 0, 100, 24), [0; 4]),
            typed(0x1000_22A8, t.button, (0, 0, 100, 24), [0; 4]),
            check,
            text_box,
            scroll_bar(0x1000_239E, false),
            scroll_bar(0x1000_239F, true),
        ];
        let mut l = UiLayout {
            did: Did(LAYOUT),
            design_width: 1024,
            design_height: 768,
            ..UiLayout::default()
        };
        l.elements_size.count = elements.len() as u32;
        l.elements = elements;
        l
    }

    fn env() -> MemoryEnv {
        let mut env = MemoryEnv::default();
        env.add_layout(layout());
        env.keys.insert(KEY, LAYOUT);
        env.images.insert(
            tessera_ui::ImageRef::Did(TITLE_ART),
            Arc::new(
                tessera_material::ui_image::UiImage::new(
                    4,
                    4,
                    4,
                    4,
                    tessera_material::ui_image::PixelLayout::A8R8G8B8,
                    vec![TITLE_COLOUR; 16],
                )
                .expect("solid image"),
            ),
        );
        // The caption of the worked example measures 96 pixels.
        let p = PreparedText {
            layout: TextLayout {
                content_width: 96,
                ..TextLayout::default()
            },
            ..PreparedText::default()
        };
        env.texts.insert("Caption".into(), Arc::new(p));
        env
    }

    fn fx() -> Fx {
        let mut fx = Fx::with_env(env());
        fx.plugin("Test", "Probe", Some("Probe"), "");
        fx.refresh();
        fx.load("Probe");
        fx
    }

    /// The rectangle (left, top, width, height) of descendant `id` of `k`.
    fn part(fx: &Fx, k: ElementKey, id: u32) -> (i32, i32, i32, i32) {
        let ui = fx.pui.ui().borrow();
        let p = ui
            .find_descendant(k, id)
            .unwrap_or_else(|| panic!("part {id:#x}"));
        let b = ui.layout_box(p).unwrap();
        (b.l, b.t, b.w(), b.h())
    }

    fn hidden(fx: &Fx, k: ElementKey, id: u32) -> bool {
        let ui = fx.pui.ui().borrow();
        let p = ui.find_descendant(k, id).unwrap();
        let pid = ui.vocab().pids.hide;
        ui.prop(p, pid).bool()
    }

    /// The §14.2.3 part rectangles of a window built from the layout, at 400 × 300 with a
    /// caption of width 96 and after `SetSize(200, 300)` (the worked example), the
    /// resize bars hidden until `SetResizable(true)`, and a `GoldWindow` built from its
    /// own container.
    #[test]
    fn window_parts_follow_the_layout() {
        let fx = fx();
        let r: (f64, f64) = fx.eval(
            "Probe",
            "w = Turbine.UI.Lotro.Window() w:SetSize(400, 300) w:SetText('Caption') \
             g = Turbine.UI.Lotro.GoldWindow() \
             local cw, ch = w:GetSize() return cw, ch",
        );
        assert_eq!(r, (400.0, 300.0));
        let w = fx.control_key("Probe", "w");
        // The top element carries the run-time ID; the parts keep their layout IDs.
        assert!(fx.pui.ui().borrow().element_id(w).unwrap() >= 0x7000_0000);
        assert_eq!(part(&fx, w, 0x1000_2295), (0, 16, 400, 282));
        assert_eq!(part(&fx, w, 0x1000_2297), (140, 5, 120, 24));
        assert_eq!(part(&fx, w, 0x1000_2296), (100, -8, 200, 41));
        assert_eq!(part(&fx, w, 0x1000_2DFB), (100, -8, 200, 41));
        assert_eq!(part(&fx, w, 0x1000_2298), (379, 21, 16, 16));
        let bars = [
            (0x1000_2DFC, (0, 16, 20, 20)),
            (0x1000_2DFD, (20, 16, 360, 10)),
            (0x1000_2DFE, (380, 16, 20, 20)),
            (0x1000_2DFF, (0, 36, 10, 244)),
            (0x1000_2E00, (390, 36, 10, 244)),
            (0x1000_2E01, (0, 280, 20, 20)),
            (0x1000_2E02, (20, 290, 360, 10)),
            (0x1000_2E03, (380, 280, 20, 20)),
        ];
        for (id, rect) in bars {
            assert_eq!(part(&fx, w, id), rect, "resize bar {id:#x}");
            assert!(hidden(&fx, w, id), "resize bar {id:#x} hidden");
        }
        {
            let ui = fx.pui.ui().borrow();
            let t = ui.find_descendant(w, 0x1000_2297).unwrap();
            drop(ui);
            assert_eq!(fx.pui.ui().borrow_mut().text_of(t), "Caption");
        }
        fx.eval::<()>("Probe", "w:SetSize(200, 300)");
        assert_eq!(part(&fx, w, 0x1000_2297), (70, 5, 60, 24));
        assert_eq!(part(&fx, w, 0x1000_2296), (30, -8, 140, 41));
        assert_eq!(part(&fx, w, 0x1000_2298), (179, 21, 16, 16));
        assert_eq!(part(&fx, w, 0x1000_2295), (0, 16, 200, 282));
        let r: bool = fx.eval("Probe", "w:SetResizable(true) return w:IsResizable()");
        assert!(r);
        assert!(!hidden(&fx, w, 0x1000_2E03));
        fx.eval::<()>("Probe", "w:SetResizable(false)");
        assert!(hidden(&fx, w, 0x1000_2E03));
        // A new window lays its title out with m = 0 at 50 × 50: t = min(-90, 120).
        let g = fx.control_key("Probe", "g");
        assert_eq!(part(&fx, g, 0x1000_2297), (70, 5, 0, 24));
        // Plugin children are not mixed with the frame: `GetControls` stays empty.
        let n: f64 = fx.eval("Probe", "return w:GetControls():GetCount()");
        assert_eq!(n, 0.0);
        fx.assert_no_errors();
    }

    /// The synthetic layout and texts, with text shaping held back until `ready` is set
    /// (the client's fonts still loading).
    struct LateFonts {
        inner: MemoryEnv,
        ready: Arc<std::sync::atomic::AtomicBool>,
    }

    impl tessera_ui::UiEnv for LateFonts {
        fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
            self.inner.layout(did)
        }
        fn layout_by_key(&mut self, key: u32) -> Option<Did> {
            self.inner.layout_by_key(key)
        }
        fn image(&mut self, image: &tessera_ui::ImageRef) -> tessera_ui::ImageState {
            self.inner.image(image)
        }
        fn shape_text(
            &mut self,
            request: &tessera_ui::TextRequest<'_>,
        ) -> Option<Arc<PreparedText>> {
            if self.ready.load(std::sync::atomic::Ordering::Relaxed) {
                self.inner.shape_text(request)
            } else {
                None
            }
        }
        fn resolve_string(
            &mut self,
            info: &tessera_formats::string_info::StringInfo,
        ) -> Option<String> {
            self.inner.resolve_string(info)
        }
        fn random(&mut self) -> f32 {
            self.inner.random()
        }
    }

    /// Regression: a caption set while its fonts are still loading could not be measured
    /// and stayed laid out with *m* = 0 (cut to 120 px) until the next `SetText` or resize.
    /// The title is laid out again on a later frame once the caption measures (§14.2.3).
    #[test]
    fn a_caption_set_before_its_fonts_load_is_measured_later() {
        let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut inner = env();
        let wide = PreparedText {
            layout: TextLayout {
                content_width: 200,
                ..TextLayout::default()
            },
            ..PreparedText::default()
        };
        inner
            .texts
            .insert("Long plugin caption".into(), Arc::new(wide));
        let env = LateFonts {
            inner,
            ready: Arc::clone(&ready),
        };
        let mut fx = Fx::with_ui_env(Box::new(env), Vocab::default());
        fx.plugin("Test", "Probe", Some("Probe"), "");
        fx.refresh();
        fx.load("Probe");
        fx.eval::<()>(
            "Probe",
            "w = Turbine.UI.Lotro.Window() w:SetSize(400, 300) \
             w:SetText('Long plugin caption')",
        );
        let w = fx.control_key("Probe", "w");
        // Not measured: t = min(400 - 140, 120).
        assert_eq!(part(&fx, w, 0x1000_2297), (140, 5, 120, 24));
        fx.frame(0.016);
        assert_eq!(part(&fx, w, 0x1000_2297), (140, 5, 120, 24));
        ready.store(true, std::sync::atomic::Ordering::Relaxed);
        fx.frame(0.016);
        // Measured: m = 200, t = min(260, 200) = 200, bars t + 80 = 280.
        assert_eq!(part(&fx, w, 0x1000_2297), (100, 5, 200, 24));
        assert_eq!(part(&fx, w, 0x1000_2296), (60, -8, 280, 41));
        assert_eq!(part(&fx, w, 0x1000_2DFB), (60, -8, 280, 41));
        fx.assert_no_errors();
    }

    /// Paints the UI until it settles and returns the last frame's composition list.
    fn painted(fx: &Fx) -> tessera_ui::paint::CompositionList {
        let mut ui = fx.pui.ui().borrow_mut();
        let mut budget = tessera_ui::paint::Unlimited;
        for t in 0..6u64 {
            ui.paint(std::time::Duration::from_millis(16 * t + 1), &mut budget);
        }
        ui.paint(std::time::Duration::from_millis(200), &mut budget)
            .composition
    }

    /// The composition entry of the canvas of `k`.
    fn entry_of(
        fx: &Fx,
        list: &tessera_ui::paint::CompositionList,
        k: ElementKey,
    ) -> tessera_ui::paint::Entry {
        let id = fx.pui.ui().borrow().surface_of(k).expect("a surface");
        list.entries
            .iter()
            .find(|e| {
                e.texture
                    == tessera_ui::paint::EntryTexture::Surface {
                        surface: id,
                        layer: tessera_ui::paint::Layer::Below,
                    }
            })
            .cloned()
            .expect("an entry")
    }

    /// §14.2.3 step 3, §14.2.6 (d): the title bar's rectangle rises 8 pixels above the
    /// window's, but the rows above the window's top edge are not drawn: every element is
    /// clipped to its own box and its ancestors' boxes (`ui-layout.md` §9.3), so the window's
    /// canvas covers the window's rectangle and nothing more. What shows is the title bar from
    /// window row 0 down; a plugin child control placed above the top is cut at it too. The
    /// window's own box (`GetSize`) is unchanged. At the identity and under a UI screen with
    /// `Se` = 2.
    #[test]
    fn the_title_bar_is_cut_at_the_window_top() {
        for se in [None, Some(2.0)] {
            let fx = fx();
            fx.eval::<()>(
                "Probe",
                "w = Turbine.UI.Lotro.Window() w:SetPosition(100, 100) w:SetSize(400, 300) \
                 w:SetVisible(true) c = Turbine.UI.Control() c:SetParent(w) \
                 c:SetPosition(10, -20) c:SetSize(40, 60) \
                 c:SetBackColor(Turbine.UI.Color(1, 1, 0, 0)) c:SetVisible(true)",
            );
            let w = fx.control_key("Probe", "w");
            if let Some(se) = se {
                let f = tessera_ui::scaling::UiScreen::new(2048, 1536, se).expect("screen");
                let a = tessera_ui::scaling::safe_area(&f, None);
                fx.pui.ui().borrow_mut().set_ui_screen(f, a);
            }
            let k = if se.is_some() { 2 } else { 1 };
            let list = painted(&fx);
            let e = entry_of(&fx, &list, w);
            let ui = fx.pui.ui().borrow();
            let b = ui.physical_box(w);
            assert_eq!(e.dest, b, "{se:?}: the canvas is the window's box");
            assert_eq!(e.canvas, [b.width(), b.height()], "{se:?}: 1 : 1");
            // Canvas row 0 is window row 0: the title bar (left 100, width 200, rows -8 to 32)
            // shows from there down.
            let px = |x: i32, y: i32| ui.surface_pixel(w, x * k, y * k).unwrap_or(0);
            assert_eq!(px(150, 0), TITLE_COLOUR, "{se:?}: title bar from row 0");
            assert_eq!(px(150, 20), TITLE_COLOUR, "{se:?}");
            assert_eq!(px(50, 0) >> 24, 0, "{se:?}: nothing beside the title bar");
            // The plugin child at top -20 shows from the window's top edge down.
            assert_eq!(
                px(20, 5),
                0xFFFF_0000,
                "{se:?}: plugin child inside the window"
            );
            drop(ui);
            let size: (f64, f64) = fx.eval("Probe", "return w:GetSize()");
            assert_eq!(size, (400.0, 300.0));
            fx.assert_no_errors();
        }
    }

    /// §13.12, `render.md` §6.8 (review finding): a nested surface below a game-styled window
    /// (a child `Window`, a root panel with its own surface) placed above the window's top
    /// edge is clipped to the window's own box; the window turns about its own box's centre (P72, §6.5). At the identity and at
    /// `Se` = 2.
    #[test]
    fn a_nested_surface_stays_clipped_to_the_window() {
        for se in [None, Some(2.0)] {
            let fx = fx();
            fx.eval::<()>(
                "Probe",
                "w = Turbine.UI.Lotro.Window() w:SetPosition(100, 100) w:SetSize(400, 300) \
                 w:SetVisible(true) c = Turbine.UI.Window() c:SetParent(w) \
                 c:SetPosition(10, -20) c:SetSize(40, 60) \
                 c:SetBackColor(Turbine.UI.Color(1, 1, 0, 0)) c:SetVisible(true)",
            );
            let (w, c) = (fx.control_key("Probe", "w"), fx.control_key("Probe", "c"));
            if let Some(se) = se {
                let f = tessera_ui::scaling::UiScreen::new(2048, 1536, se).expect("screen");
                let a = tessera_ui::scaling::safe_area(&f, None);
                fx.pui.ui().borrow_mut().set_ui_screen(f, a);
            }
            let list = painted(&fx);
            let b = fx.pui.ui().borrow().physical_box(w);
            let e = entry_of(&fx, &list, w);
            assert_eq!(e.dest, b, "{se:?}: the window's canvas is its box");
            let ce = entry_of(&fx, &list, c);
            let items = list.clips_of(&ce);
            assert!(!items.is_empty(), "{se:?}: the child is clipped");
            for it in items {
                assert!(
                    it.rect[1] >= b.y0,
                    "{se:?}: clip {:?} reaches above the window's top {}",
                    it.rect,
                    b.y0
                );
            }
            // Rotated, the window and its child turn about the window's own centre.
            fx.eval::<()>("Probe", "w:SetRotation({ x = 0, y = 0, z = 90 })");
            let list = painted(&fx);
            let e = entry_of(&fx, &list, w);
            let r = list.rotations_of(&e);
            assert_eq!(r.len(), 1, "{se:?}");
            let want = [(b.x0 + b.x1) as f32 / 2.0, (b.y0 + b.y1) as f32 / 2.0];
            assert!(
                (r[0].centre[0] - want[0]).abs() < 1e-3 && (r[0].centre[1] - want[1]).abs() < 1e-3,
                "{se:?}: centre {:?} vs {want:?}",
                r[0].centre
            );
            fx.assert_no_errors();
        }
    }

    /// §14.2.3 step 3, §14.2.6 (d): the hit test reaches children only inside their parent's
    /// rectangle (§13.11, P49), so the title bar and drag bar rows above the window's top edge
    /// are not hit; from the window's row 0 down the bars are hit, so only that part of the
    /// drag bar moves the window. A plugin child placed above the top is not hit there either.
    #[test]
    fn the_rows_above_the_window_are_not_hit() {
        let fx = fx();
        fx.eval::<()>(
            "Probe",
            "w = Turbine.UI.Lotro.Window() w:SetPosition(100, 100) w:SetSize(400, 300) \
             w:SetVisible(true) c = Turbine.UI.Control() c:SetParent(w) \
             c:SetPosition(10, -20) c:SetSize(40, 60) c:SetMouseVisible(true) c:SetVisible(true)",
        );
        let (w, c) = (fx.control_key("Probe", "w"), fx.control_key("Probe", "c"));
        let _ = painted(&fx);
        let mut ui = fx.pui.ui().borrow_mut();
        let parts: Vec<ElementKey> = [0x1000_2296, 0x1000_2DFB]
            .into_iter()
            .filter_map(|id| ui.find_descendant(w, id))
            .collect();
        // Above the window (rows -8 to -1 of the bars, and the plugin child's rows there):
        // neither the bars, the plugin child nor the window.
        for (x, y) in [(210, 96), (100 + 100, 92), (100 + 299, 99), (100 + 20, 96)] {
            let hit = ui.hit_test(x, y);
            assert!(
                hit.is_none_or(|h| !parts.contains(&h) && h != w && h != c),
                "({x}, {y}): {hit:?} is hit above the window"
            );
        }
        // From the window's row 0 down, beside the title text, the bars are hit.
        for (x, y) in [(210, 100), (210, 102), (100 + 299, 130)] {
            let hit = ui.hit_test(x, y);
            assert!(
                hit.is_some_and(|h| parts.contains(&h)),
                "({x}, {y}): {hit:?} is not the title bar or drag bar"
            );
        }
        // Inside the window the plugin child is hit as before.
        assert_eq!(ui.hit_test(100 + 20, 100 + 5), Some(c));
        drop(ui);
        fx.assert_no_errors();
    }

    /// A light frame that moves a part above the window (review finding of the earlier
    /// extended canvas): the window's canvas stays its own box whatever the part's top.
    #[test]
    fn moving_the_title_bar_keeps_the_canvas_at_the_window() {
        let fx = fx();
        fx.eval::<()>(
            "Probe",
            "w = Turbine.UI.Lotro.Window() w:SetPosition(100, 100) w:SetSize(400, 300) \
             w:SetVisible(true)",
        );
        let w = fx.control_key("Probe", "w");
        let list = painted(&fx);
        let b = fx.pui.ui().borrow().physical_box(w);
        assert_eq!(entry_of(&fx, &list, w).dest, b);
        let bar = fx
            .pui
            .ui()
            .borrow()
            .find_descendant(w, 0x1000_2296)
            .unwrap();
        for top in [-16, -8, 4, -100] {
            {
                let mut ui = fx.pui.ui().borrow_mut();
                let pb = ui.layout_box(bar).unwrap();
                ui.set_element_box(bar, pb.l, top, pb.w(), pb.h());
            }
            let list = painted(&fx);
            let e = entry_of(&fx, &list, w);
            assert_eq!(e.dest, b, "title bar at {top}");
            assert_eq!(e.canvas, [b.width(), b.height()], "title bar at {top}");
        }
        fx.assert_no_errors();
    }

    /// §14.2.3 step 2 (review finding): the title layout sets the title text's four text
    /// margins to 0 before measuring, so the layout's own margins (10 and 10 here) do not
    /// widen the measured caption.
    #[test]
    fn the_title_text_margins_are_zero() {
        let mut fx = Fx::with_ui_env(Box::new(MarginFonts { inner: env() }), Vocab::default());
        fx.plugin("Test", "Probe", Some("Probe"), "");
        fx.refresh();
        fx.load("Probe");
        fx.eval::<()>(
            "Probe",
            "w = Turbine.UI.Lotro.Window() w:SetSize(400, 300) w:SetText('Caption')",
        );
        let w = fx.control_key("Probe", "w");
        let p = Vocab::default().pids;
        {
            let ui = fx.pui.ui().borrow();
            let t = ui.find_descendant(w, 0x1000_2297).unwrap();
            for pid in [
                p.text_left_margin,
                p.text_right_margin,
                p.text_top_margin,
                p.text_bottom_margin,
            ] {
                assert_eq!(ui.prop(t, pid).int(), 0, "margin {pid:#x}");
            }
        }
        // m = 160 (not 180 with the layout's margins): t = min(260, 160), left 120.
        assert_eq!(part(&fx, w, 0x1000_2297), (120, 5, 160, 24));
        fx.assert_no_errors();
    }

    /// Text shaping whose measured width is 160 plus the element's left and right margins
    /// (`fonts.md` §7: the content width includes them).
    struct MarginFonts {
        inner: MemoryEnv,
    }

    impl tessera_ui::UiEnv for MarginFonts {
        fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
            self.inner.layout(did)
        }
        fn layout_by_key(&mut self, key: u32) -> Option<Did> {
            self.inner.layout_by_key(key)
        }
        fn image(&mut self, image: &tessera_ui::ImageRef) -> tessera_ui::ImageState {
            self.inner.image(image)
        }
        fn shape_text(
            &mut self,
            request: &tessera_ui::TextRequest<'_>,
        ) -> Option<Arc<PreparedText>> {
            let p = Vocab::default().pids;
            let margin = |pid: u32| {
                request
                    .props
                    .iter()
                    .find(|(k, _)| *k == pid)
                    .map_or(0, |(_, v)| match v {
                        PropertyValue::Int(i) => *i,
                        _ => 0,
                    })
            };
            let w = 160 + margin(p.text_left_margin) + margin(p.text_right_margin);
            Some(Arc::new(PreparedText {
                layout: TextLayout {
                    content_width: w,
                    ..TextLayout::default()
                },
                ..PreparedText::default()
            }))
        }
        fn resolve_string(
            &mut self,
            info: &tessera_formats::string_info::StringInfo,
        ) -> Option<String> {
            self.inner.resolve_string(info)
        }
        fn random(&mut self) -> f32 {
            self.inner.random()
        }
    }

    /// §13.21 rule 1, §14.2.1 step 3: the first `SetScale` of a game-styled window rebuilds
    /// it from `LotroScalerWindow_Container` (a `GoldWindow` from its scaled variant): a new
    /// element keeping visibility, position, size, z-level and background, the old element's
    /// operation number passed to `SetBlendMode` (4 `AlphaBlend` → 2), the title set again and
    /// the plugin's child controls kept; a second marking call rebuilds nothing.
    #[test]
    fn marking_a_game_styled_window_rebuilds_it() {
        let mut fx = fx();
        fx.eval::<()>(
            "Probe",
            "vis = 0 w = Turbine.UI.Lotro.Window() w:SetPosition(120, 80) w:SetSize(400, 300) \
             w:SetZOrder(3) w:SetText('Caption') w:SetBlendMode(Turbine.UI.BlendMode.AlphaBlend) \
             w:SetVisible(true) w.VisibleChanged = function() vis = vis + 1 end \
             sz = 0 pos = 0 w.SizeChanged = function() sz = sz + 1 end \
             w.PositionChanged = function() pos = pos + 1 end \
             c = Turbine.UI.Control() c:SetParent(w) c:SetPosition(5, 40) c:SetSize(10, 10) \
             g = Turbine.UI.Lotro.GoldWindow()",
        );
        fx.frame(0.016);
        let old = fx.control_key("Probe", "w");
        let mark = |fx: &Fx, k: ElementKey| fx.pui.ui().borrow().prop(k, SCALED_MARK).int() as u32;
        assert_eq!(mark(&fx, old), 0, "built from the plain container");
        assert_eq!(fx.eval::<f64>("Probe", "return w:GetBlendMode()"), 4.0);
        fx.eval::<()>("Probe", "w:SetScale(1.5)");
        let new = fx.control_key("Probe", "w");
        assert_ne!(new, old, "a new element");
        assert!(!fx.pui.ui().borrow().exists(old), "the old element is gone");
        assert_eq!(mark(&fx, new), 0x1000_4D37);
        let r: (f64, f64, f64, f64, f64, bool, f64) = fx.eval(
            "Probe",
            "local x, y = w:GetPosition() local cw, ch = w:GetSize() \
             return x, y, cw, ch, w:GetZOrder(), w:IsVisible(), w:GetBlendMode()",
        );
        assert_eq!(r, (120.0, 80.0, 400.0, 300.0, 3.0, true, 2.0));
        // The child control moved with it; the frame's parts are the scaled container's.
        let r: (f64, bool) = fx.eval(
            "Probe",
            "return w:GetControls():GetCount(), c:GetParent() == w",
        );
        assert_eq!(r, (1.0, true));
        assert_eq!(
            part(&fx, new, 0x1000_2297),
            (140, 5, 120, 24),
            "title set again"
        );
        assert!(
            hidden(&fx, new, 0x1000_2E03),
            "resize bars follow IsResizable"
        );
        assert_eq!(fx.eval::<f64>("Probe", "return w:GetScale()"), 1.5);
        // Marked once: further calls keep the element.
        fx.eval::<()>(
            "Probe",
            "w:SetScale(2) w:RegisterForGlobalScaling() w:SetUIScalingGroup(5)",
        );
        assert_eq!(fx.control_key("Probe", "w"), new);
        // A GoldWindow uses its scaled variant.
        fx.eval::<()>("Probe", "g:SetUIScalingGroup(5)");
        let g = fx.control_key("Probe", "g");
        assert_eq!(mark(&fx, g), 0x1000_4D38);
        fx.frame(0.016);
        assert_eq!(
            fx.eval::<f64>("Probe", "return vis"),
            0.0,
            "visibility kept, no event"
        );
        // Size and position are kept too: no `SizeChanged` or `PositionChanged` (review
        // finding), while a real change afterwards still fires.
        assert_eq!(fx.eval::<(f64, f64)>("Probe", "return sz, pos"), (0.0, 0.0));
        fx.eval::<()>("Probe", "w:SetSize(410, 300) w:SetPosition(121, 80)");
        fx.frame(0.016);
        assert_eq!(fx.eval::<(f64, f64)>("Probe", "return sz, pos"), (1.0, 1.0));
        fx.assert_no_errors();
    }

    /// §14.2.6 gap (b) and its round-3 answer: the plugin-set element state survives the
    /// first-marking rebuild: back colour, opacity, stretch mode, mouse visibility, drop
    /// target and rotation are those the plugin set, on the new element too, and an active
    /// window stays the active panel.
    #[test]
    fn the_rebuild_carries_the_plugin_state_over() {
        let mut fx = fx();
        fx.eval::<()>(
            "Probe",
            "w = Turbine.UI.Lotro.Window() w:SetPosition(120, 80) w:SetSize(400, 300) \
             w:SetVisible(true) w:SetOpacity(0.5) \
             w:SetBackColor(Turbine.UI.Color(1, 0.25, 0.5, 0.75)) \
             w:SetMouseVisible(false) w:SetAllowDrop(true) \
             w:SetRotation({ x = 0, y = 0, z = 30 }) w:Activate()",
        );
        fx.frame(0.016);
        let old = fx.control_key("Probe", "w");
        let before = {
            let ui = fx.pui.ui().borrow();
            (ui.control_back_colour(old), ui.window_rotation(old))
        };
        fx.eval::<()>("Probe", "w:SetScale(1.5)");
        fx.frame(0.016);
        let new = fx.control_key("Probe", "w");
        assert_ne!(new, old, "a new element");
        let ui = fx.pui.ui().borrow();
        assert!((ui.control_opacity(new) - 0.5).abs() < 1e-6, "opacity");
        assert_eq!(ui.control_back_colour(new), before.0, "back colour");
        assert_eq!(ui.window_rotation(new), before.1, "rotation");
        assert_eq!(ui.active_panel(), Some(new), "still the active window");
        drop(ui);
        let r: (f64, bool, bool) = fx.eval(
            "Probe",
            "return w:GetOpacity(), w:IsMouseVisible(), w:GetAllowDrop()",
        );
        assert_eq!(r, (0.5, false, true));
        fx.assert_no_errors();
    }

    /// §14.2.6 round-5 answer 1 (Tessera rule): a hidden, rotated game-styled window keeps
    /// its rotation across the first-marking rebuild: `GetRotation` is unchanged, and the
    /// new surface gets the remembered angles when the window is shown again. A shown
    /// window's new surface gets the old surface's angles at once.
    #[test]
    fn a_hidden_rotated_window_keeps_its_rotation_across_the_rebuild() {
        let mut fx = fx();
        fx.eval::<()>(
            "Probe",
            "w = Turbine.UI.Lotro.Window() w:SetPosition(120, 80) w:SetSize(400, 300) \
             w:SetVisible(true) w:SetRotation({ x = 0, y = 0, z = 30 }) \
             v = Turbine.UI.Lotro.Window() v:SetPosition(600, 80) v:SetSize(200, 200) \
             v:SetVisible(true) v:SetRotation({ x = 0, y = 0, z = 15 })",
        );
        fx.frame(0.016);
        fx.eval::<()>("Probe", "w:SetVisible(false)");
        fx.frame(0.016);
        let old = fx.control_key("Probe", "w");
        let old_v = fx.control_key("Probe", "v");
        let (before, before_v, held_v) = {
            let ui = fx.pui.ui().borrow();
            (
                ui.window_rotation(old),
                ui.window_rotation(old_v),
                tessera_ui::game::held_rotation(&ui, old_v),
            )
        };
        assert_ne!(before, [0.0; 3], "rotated");
        assert_ne!(
            held_v, [0.0; 3],
            "the shown window's surface holds its rotation"
        );
        fx.eval::<()>("Probe", "w:SetScale(1.5) v:SetScale(1.5)");
        fx.frame(0.016);
        let new = fx.control_key("Probe", "w");
        let new_v = fx.control_key("Probe", "v");
        assert_ne!(new, old, "rebuilt");
        assert_ne!(new_v, old_v, "rebuilt");
        {
            let ui = fx.pui.ui().borrow();
            assert_eq!(
                ui.window_rotation(new),
                before,
                "GetRotation kept while hidden"
            );
            assert_eq!(
                tessera_ui::game::held_rotation(&ui, new),
                [0.0; 3],
                "a hidden window's surface holds nothing"
            );
            assert_eq!(
                ui.window_rotation(new_v),
                before_v,
                "GetRotation kept while shown"
            );
            assert_eq!(
                tessera_ui::game::held_rotation(&ui, new_v),
                held_v,
                "the shown window's new surface has the old surface's rotation"
            );
        }
        fx.eval::<()>("Probe", "w:SetVisible(true)");
        fx.frame(0.016);
        let ui = fx.pui.ui().borrow();
        assert_eq!(
            tessera_ui::game::held_rotation(&ui, new),
            before,
            "shown again: the remembered rotation"
        );
        drop(ui);
        fx.assert_no_errors();
    }

    /// §14.2.6 round-5 answer 2 (Tessera rule): the rebuild of the active window posts no
    /// `Activated`, `Deactivated`, `FocusGained` or `FocusLost`; the window stays the active
    /// window with its focus control, keeps its entry in the activation history (hiding it
    /// activates the window active before it) and its place among its siblings.
    #[test]
    fn the_rebuild_of_the_active_window_posts_no_activation_events() {
        let mut fx = fx();
        fx.eval::<()>(
            "Probe",
            "acts = {} \
             a = Turbine.UI.Lotro.Window() a:SetPosition(600, 80) a:SetSize(200, 200) \
             a:SetVisible(true) \
             w = Turbine.UI.Lotro.Window() w:SetPosition(120, 80) w:SetSize(400, 300) \
             w:SetVisible(true) \
             t = Turbine.UI.TextBox() t:SetParent(w) t:SetPosition(20, 60) t:SetSize(100, 20) \
             t:SetVisible(true) \
             function a:Activated() acts[#acts + 1] = 'a+' end \
             function a:Deactivated() acts[#acts + 1] = 'a-' end \
             function w:Activated() acts[#acts + 1] = 'w+' end \
             function w:Deactivated() acts[#acts + 1] = 'w-' end \
             t.FocusGained = function() acts[#acts + 1] = 't+' end \
             t.FocusLost = function() acts[#acts + 1] = 't-' end \
             a:Activate() w:Activate() t:Focus()",
        );
        fx.frame(0.016);
        let set_up: String = fx.eval("Probe", "return table.concat(acts, ' ')");
        assert!(set_up.ends_with("w+ t+"), "{set_up}");
        let old = fx.control_key("Probe", "w");
        let tk = fx.control_key("Probe", "t");
        let place = |fx: &Fx, k: ElementKey| {
            let ui = fx.pui.ui().borrow();
            ui.children(ui.screen_root()).iter().position(|c| *c == k)
        };
        let old_place = place(&fx, old);
        assert!(old_place.is_some());
        fx.eval::<()>("Probe", "acts = {} w:SetScale(1.5)");
        fx.frame(0.016);
        let new = fx.control_key("Probe", "w");
        assert_ne!(new, old, "rebuilt");
        let acts: String = fx.eval("Probe", "return table.concat(acts, ' ')");
        assert_eq!(acts, "", "no activation or focus events");
        {
            let ui = fx.pui.ui().borrow();
            assert_eq!(ui.active_panel(), Some(new), "still the active window");
            assert_eq!(ui.focus_element(), Some(tk), "the focus stays");
        }
        assert_eq!(
            place(&fx, new),
            old_place,
            "the same place among its siblings"
        );
        assert!(fx.eval::<bool>("Probe", "return t:HasFocus()"));
        // The activation history is kept: hiding it activates `a` again.
        fx.eval::<()>("Probe", "w:SetVisible(false)");
        fx.frame(0.016);
        let acts: String = fx.eval("Probe", "return table.concat(acts, ' ')");
        assert!(acts.ends_with("a+"), "{acts}");
        assert!(acts.contains("w-"), "{acts}");
        fx.assert_no_errors();
    }

    /// Review finding: a hidden game-styled window first marked as scaled and then shown in
    /// the same chunk fires its `VisibleChanged` (the rebuild of a hidden window changes no
    /// visibility, so it must leave no quiet mark that swallows the real event); the same
    /// across frames.
    #[test]
    fn a_hidden_rebuilt_window_reports_being_shown() {
        let mut fx = fx();
        fx.eval::<()>(
            "Probe",
            "vis = 0 w = Turbine.UI.Lotro.Window() w:SetSize(400, 300) \
             w.VisibleChanged = function() vis = vis + 1 end \
             w:SetScale(1.5) w:SetVisible(true)",
        );
        fx.frame(0.016);
        assert_eq!(fx.eval::<f64>("Probe", "return vis"), 1.0, "same chunk");
        fx.eval::<()>(
            "Probe",
            "vis = 0 h = Turbine.UI.Lotro.GoldWindow() \
             h.VisibleChanged = function() vis = vis + 1 end h:SetUIScalingGroup(5)",
        );
        fx.frame(0.016);
        fx.eval::<()>("Probe", "h:SetVisible(true)");
        fx.frame(0.016);
        assert_eq!(fx.eval::<f64>("Probe", "return vis"), 1.0, "a later frame");
        fx.assert_no_errors();
    }

    /// The close button's `Button_click` runs the window's native `Close()` (§14.2.3):
    /// `Closing`, then hidden and `Closed`.
    #[test]
    fn close_button_closes_the_window() {
        let mut fx = fx();
        fx.eval::<()>(
            "Probe",
            "closed = 0 w = Turbine.UI.Lotro.Window() w:SetPosition(100, 100) \
             w:SetSize(400, 300) w:SetVisible(true) \
             w.Closed = function() closed = closed + 1 end",
        );
        fx.frame(0.016);
        // The close button sits at (379, 21) inside the window: its centre on the screen.
        let (x, y) = (100 + 379 + 8, 100 + 21 + 8);
        fx.move_to(x, y);
        fx.frame(0.016);
        let hold = fx.press(x, y);
        fx.release(hold, x, y);
        fx.frame(0.016);
        let r: (f64, bool) = fx.eval("Probe", "return closed, w:IsVisible()");
        assert_eq!(r, (1.0, false));
        fx.assert_no_errors();
    }

    /// A game-styled check box writes its text to `LotroCheckBox_Text` and shows its state
    /// with the check mark `LotroCheckBox_CheckBox` (§14.2.2).
    #[test]
    fn check_box_text_and_mark() {
        let fx = fx();
        fx.eval::<()>(
            "Probe",
            "c = Turbine.UI.Lotro.CheckBox() c:SetText('Option') c:SetChecked(true)",
        );
        let c = fx.control_key("Probe", "c");
        let mut ui = fx.pui.ui().borrow_mut();
        let text = ui.find_descendant(c, 0x1000_25B1).unwrap();
        let mark = ui.find_descendant(c, 0x1000_25B0).unwrap();
        assert_eq!(ui.text_of(text), "Option");
        assert_eq!(ui.text_of(c), "");
        let pid = ui.vocab().pids.button_highlighted;
        assert!(ui.prop(mark, pid).bool());
        drop(ui);
        fx.eval::<()>("Probe", "c:SetChecked(false)");
        let ui = fx.pui.ui().borrow();
        assert!(!ui.prop(mark, pid).bool());
        drop(ui);
        let r: (bool, String) = fx.eval("Probe", "return c:IsChecked(), c:GetText()");
        assert_eq!(r, (false, "Option".into()));
        fx.assert_no_errors();
    }

    /// Buttons are built from their base elements; without the layout every class falls
    /// back to Tessera's neutral art (§14.2.1 step 5) and still works.
    #[test]
    fn buttons_and_the_fallback() {
        let fx = fx();
        fx.eval::<()>(
            "Probe",
            "b = Turbine.UI.Lotro.Button() b:SetText('Go') \
             gb = Turbine.UI.Lotro.GoldButton()",
        );
        let b = fx.control_key("Probe", "b");
        {
            let mut ui = fx.pui.ui().borrow_mut();
            assert_eq!(ui.text_of(b), "Go");
            assert_eq!(ui.layout_box(b).map(|x| (x.w(), x.h())), Some((50, 50)));
        }
        let plain = Fx::new();
        plain.plugin("Test", "Probe", Some("Probe"), "");
        let mut plain = plain;
        plain.refresh();
        plain.load("Probe");
        let r: (f64, f64, bool) = plain.eval(
            "Probe",
            "w = Turbine.UI.Lotro.Window() w:SetSize(400, 300) \
             c = Turbine.UI.Lotro.CheckBox() c:SetChecked(true) \
             t = Turbine.UI.Lotro.TextBox() t:SetText('x') \
             sb = Turbine.UI.Lotro.ScrollBar() sb:SetOrientation(Turbine.UI.Orientation.Vertical) \
             local x, y = w:GetSize() return x, y, c:IsChecked()",
        );
        assert_eq!(r, (400.0, 300.0, true));
        let t = plain.control_key("Probe", "t");
        assert_eq!(plain.pui.ui().borrow_mut().text_of(t), "x");
        let w = plain.control_key("Probe", "w");
        assert!(
            plain
                .pui
                .ui()
                .borrow()
                .find_descendant(w, 0x1000_2297)
                .is_none()
        );
        fx.assert_no_errors();
        plain.assert_no_errors();
    }

    /// A plain window at (100, 100), 400 × 300, shown, for the controls below.
    const WINDOW: &str = "w = Turbine.UI.Window() w:SetPosition(100, 100) w:SetSize(400, 300) \
                          w:SetVisible(true) ";

    /// A game-styled text box edits in `LotroTextBox_Text` (§14.2.2): its members act on
    /// that element, its notifications (focus, keys, text changes, mouse) become the box's
    /// events, and a read-only box takes no typing.
    #[test]
    fn text_box_edits_in_its_text_element() {
        let mut fx = fx();
        fx.eval::<()>(
            "Probe",
            &format!(
                "{WINDOW} t = Turbine.UI.Lotro.TextBox() t:SetParent(w) t:SetPosition(10, 10) \
                 t:SetSize(200, 24) t:SetVisible(true) t:SetText('ab') \
                 changes = 0 t.TextChanged = function() changes = changes + 1 end \
                 focus = '' t.FocusGained = function() focus = focus .. '+' end \
                 t.FocusLost = function() focus = focus .. '-' end \
                 keys = 0 t.KeyDown = function() keys = keys + 1 end \
                 clicks = {{}} t.MouseClick = function(s, a) clicks[#clicks + 1] = a.X .. ',' .. a.Y end"
            ),
        );
        fx.frame(0.016);
        let t = fx.control_key("Probe", "t");
        assert_eq!(part(&fx, t, 0x1000_22AA), (4, 4, 192, 16));
        {
            let mut ui = fx.pui.ui().borrow_mut();
            let text = ui.find_descendant(t, 0x1000_22AA).unwrap();
            assert_eq!(ui.text_of(text), "ab");
            assert_eq!(ui.text_of(t), "");
        }
        // A click on the text element focuses it and is the box's click, at box coordinates.
        fx.click(100 + 10 + 50, 100 + 10 + 12, 1);
        fx.type_char('c');
        fx.frame(0.016);
        let r: (bool, String, f64, String) =
            fx.eval("Probe", "return t:HasFocus(), t:GetText(), changes, focus");
        assert!(r.0, "focused");
        // `SetText` leaves the caret at 0, as in a plain text box.
        assert_eq!(r.1, "cab");
        assert!(r.2 >= 1.0, "TextChanged after the edit, got {}", r.2);
        assert!(r.3.starts_with('+'), "{}", r.3);
        let c: String = fx.eval("Probe", "return table.concat(clicks, ';')");
        assert_eq!(c, "50,12");
        // Keys reach the box; Escape keeps the focus (as a plain text box does).
        fx.key("Escape", 0x2C);
        let r: (f64, bool) = fx.eval("Probe", "return keys, t:HasFocus()");
        assert!(r.0 >= 1.0, "KeyDown, got {}", r.0);
        assert!(r.1, "Escape keeps the focus");
        // Read-only: typing changes nothing.
        fx.eval::<()>("Probe", "t:SetReadOnly(true) changes = 0");
        fx.type_char('x');
        fx.frame(0.016);
        let r: (String, f64) = fx.eval("Probe", "return t:GetText(), changes");
        assert_eq!(r, ("cab".into(), 0.0));
        fx.assert_no_errors();
    }

    /// The layout widget of a game-styled scroll bar (§14.2.2) fills the control, shows the
    /// value, and moves it: the arrow buttons step by the small change. The control keeps
    /// its own visibility.
    #[test]
    fn scroll_bar_widget_drives_the_value() {
        let mut fx = fx();
        fx.eval::<()>(
            "Probe",
            &format!(
                "{WINDOW} sb = Turbine.UI.Lotro.ScrollBar() sb:SetParent(w) sb:SetPosition(10, 10) \
                 sb:SetSize(200, 16) sb:SetVisible(true) sb:SetValue(50) sb:SetSmallChange(3) \
                 vals = {{}} sb.ValueChanged = function() vals[#vals + 1] = sb:GetValue() end"
            ),
        );
        fx.frame(0.016);
        let sb = fx.control_key("Probe", "sb");
        let pid = Vocab::default().pids.sb_position;
        let widget = |fx: &Fx| {
            let ui = fx.pui.ui().borrow();
            let w = ui.children(sb).into_iter().find(|c| {
                ui.element_id(*c).is_some_and(|id| id >= 0x7000_0000)
                    && ui.find_child(*c, INC).is_some()
            });
            w.expect("the layout widget below the control")
        };
        let w = widget(&fx);
        {
            let ui = fx.pui.ui().borrow();
            let b = ui.layout_box(w).unwrap();
            assert_eq!((b.l, b.t, b.w(), b.h()), (0, 0, 200, 16));
            assert!((ui.prop(w, pid).float() - 0.5).abs() < 1e-6);
        }
        // The increment button at the bar's right end.
        let (x, y) = (100 + 10 + 192, 100 + 10 + 8);
        let hold = fx.press(x, y);
        fx.release(hold, x, y);
        fx.frame(0.016);
        let r: (f64, String) = fx.eval("Probe", "return sb:GetValue(), table.concat(vals, ',')");
        assert_eq!(r, (53.0, "53".into()));
        {
            let ui = fx.pui.ui().borrow();
            assert!((ui.prop(w, pid).float() - 0.53).abs() < 1e-6);
        }
        // Code changes show on the widget, and the bar stays hidden when hidden.
        fx.eval::<()>("Probe", "sb:SetVisible(false) sb:SetValue(20)");
        fx.frame(0.016);
        let r: bool = fx.eval("Probe", "return sb:IsVisible()");
        assert!(!r);
        {
            let ui = fx.pui.ui().borrow();
            assert!(!ui.is_visible(sb));
            assert!((ui.prop(w, pid).float() - 0.2).abs() < 1e-6);
        }
        fx.assert_no_errors();
    }

    /// §14.2.1 step 3: an orientation change rebuilds the game-styled bar from the other
    /// base element, fires `OrientationChanged`, keeps the value, and passes the old
    /// operation number to `SetBlendMode` (`AlphaBlend` 4 → 2; never set → 0).
    #[test]
    fn scroll_bar_orientation_rebuilds() {
        let fx = fx();
        fx.eval::<()>(
            "Probe",
            "sb = Turbine.UI.Lotro.ScrollBar() sb:SetSize(16, 200) sb:SetValue(40) \
             n = 0 sb.OrientationChanged = function() n = n + 1 end \
             b = Turbine.UI.Lotro.ScrollBar()",
        );
        let sb = fx.control_key("Probe", "sb");
        let hp = Vocab::default().pids.sb_horizontal;
        let horizontal = |fx: &Fx| {
            let ui = fx.pui.ui().borrow();
            let w = ui
                .children(sb)
                .into_iter()
                .find(|c| ui.find_child(*c, INC).is_some())
                .expect("widget");
            (ui.element_id(w).unwrap(), ui.prop(w, hp).bool())
        };
        let (first, h) = horizontal(&fx);
        assert!(h, "a new bar is horizontal");
        let r: (f64, f64, f64) = fx.eval(
            "Probe",
            "sb:SetBlendMode(Turbine.UI.BlendMode.AlphaBlend) \
             sb:SetOrientation(Turbine.UI.Orientation.Vertical) \
             b:SetOrientation(Turbine.UI.Orientation.Vertical) \
             return sb:GetBlendMode(), b:GetBlendMode(), sb:GetValue()",
        );
        assert_eq!(r, (2.0, 0.0, 40.0));
        let (second, h) = horizontal(&fx);
        assert!(!h, "rebuilt from LotroScrollBar_Vertical");
        assert_ne!(first, second, "a new element");
        {
            let ui = fx.pui.ui().borrow();
            let w = ui
                .children(sb)
                .into_iter()
                .find(|c| ui.find_child(*c, INC).is_some())
                .unwrap();
            let b = ui.layout_box(w).unwrap();
            assert_eq!((b.w(), b.h()), (16, 200));
            let q = ui.prop(w, Vocab::default().pids.sb_position).float();
            assert!((q - 0.4).abs() < 1e-6, "{q}");
        }
        let n: f64 = fx.eval("Probe", "return n");
        assert_eq!(n, 1.0);
        fx.assert_no_errors();
    }
}
