// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data check of the game-styled plugin controls (`lua-plugin-api.md` §14.2.1–§14.2.5)
//! against the user's install, read-only. Skipped (reported as passed) unless
//! `TESSERA_GAME_DIR` names an install. Only pass/fail and counts are printed; no game data
//! leaves the install.
//!
//! - §14.2.5: the `UILAYOUT` map has key 0x100000C8, its layout has the ten base elements,
//!   each resolves; the window containers have the §14.2.3 parts; the text box and check
//!   box have their descendants.
//! - `Lotro.Window`, `GoldWindow`, `Button`, `GoldButton`, `CheckBox`, `TextBox` and both
//!   `ScrollBar` orientations are built from that layout (not the neutral fallback), and a
//!   window's title parts follow the §14.2.3 title layout from its width.

#[path = "fixtures/harness.rs"]
mod harness;

use std::collections::HashMap;
use std::sync::Arc;

use harness::Fx;
use tessera_formats::did::Did;
use tessera_formats::ui_layout::{UiEnum, UiLabels, UiLayout};
use tessera_props::GameData;
use tessera_ui::resolve::resolve;
use tessera_ui::{UiEnv, Vocab};

const KEY: u32 = 0x1000_00C8;
const BASES: [u32; 10] = [
    0x1000_22A7,
    0x1000_22A8,
    0x1000_25AF,
    0x1000_22A9,
    0x1000_239E,
    0x1000_239F,
    0x1000_2294,
    0x1000_4D37,
    0x1000_2299,
    0x1000_4D38,
];
const WINDOWS: [u32; 4] = [0x1000_2294, 0x1000_2299, 0x1000_4D37, 0x1000_4D38];

struct InstallEnv {
    data: Arc<GameData>,
    layouts: HashMap<u32, Did>,
    cache: HashMap<u32, Option<Arc<UiLayout>>>,
}

impl UiEnv for InstallEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        let data = &self.data;
        self.cache
            .entry(did.0)
            .or_insert_with(|| {
                let payload = data.read(did).ok()??;
                let registry = data.registry().ok()?;
                UiLayout::parse_expecting(&payload, did, &**registry)
                    .ok()
                    .map(Arc::new)
            })
            .clone()
    }

    fn layout_by_key(&mut self, key: u32) -> Option<Did> {
        self.layouts.get(&key).copied()
    }

    /// No art is decoded here: every image is a synthetic opaque 4 × 4 square, so the
    /// windows' parts paint something and their canvases are composed.
    fn image(&mut self, _image: &tessera_ui::ImageRef) -> tessera_ui::ImageState {
        let img = tessera_material::ui_image::UiImage::new(
            4,
            4,
            4,
            4,
            tessera_material::ui_image::PixelLayout::A8R8G8B8,
            vec![0xFF80_8080; 16],
        );
        match img {
            Some(i) => tessera_ui::ImageState::Ready(Arc::new(i)),
            None => tessera_ui::ImageState::Failed,
        }
    }
}

/// Whether `id` is among `children` or their descendants (bounded depth).
fn has_descendant(children: &[tessera_formats::ui_layout::ElementDesc], id: u32) -> bool {
    let mut stack: Vec<(&tessera_formats::ui_layout::ElementDesc, usize)> =
        children.iter().map(|c| (c, 0)).collect();
    while let Some((c, depth)) = stack.pop() {
        if c.element_id == id {
            return true;
        }
        if depth < 64 {
            stack.extend(c.children.iter().map(|g| (g, depth + 1)));
        }
    }
    false
}

#[test]
fn styled_controls_from_the_install_layout() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR") else {
        eprintln!("TESSERA_GAME_DIR is not set; skipping");
        return;
    };
    let data = Arc::new(GameData::open(&dir).expect("open the install read-only"));
    let registry = data.registry().expect("property registry").clone();
    let uilayout = data
        .did_map_by_label("UILAYOUT")
        .expect("DID map reads")
        .expect("UILAYOUT map");
    let emapper = data
        .did_map_by_label("EMAPPER")
        .expect("DID map reads")
        .expect("EMAPPER map");
    let labels = UiLabels::load(
        |label| emapper.by_label(label),
        |did| data.enum_mapper(did).ok().flatten().map(|m| (*m).clone()),
    );
    let mut vocab = Vocab::default();
    vocab.resolve_with(
        &mut |n| registry.pid(n),
        &mut |n| labels.key(UiEnum::Message, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |_| None,
    );
    let layouts: HashMap<u32, Did> = uilayout.iter().map(|e| (e.key, e.value)).collect();
    let did = *layouts
        .get(&KEY)
        .expect("UILAYOUT has key 0x100000C8 (§14.2.5)");
    let mut env = InstallEnv {
        data: data.clone(),
        layouts,
        cache: HashMap::new(),
    };

    // §14.2.5 invariants.
    let layout = env.layout(did).expect("the plugin layout decodes");
    let mut resolved = 0;
    for base in BASES {
        let i = layout
            .top_level_index(base)
            .unwrap_or_else(|| panic!("top-level element {base:#x} (§14.2.5)"));
        let desc = &layout.elements[i];
        let r = resolve(desc, &mut |d| env.layout(d))
            .unwrap_or_else(|e| panic!("{base:#x} does not resolve: {e}"));
        let has = |id: u32| r.children.iter().any(|c| c.element_id == id);
        if WINDOWS.contains(&base) {
            for id in (0x1000_2295..=0x1000_2298).chain(0x1000_2DFB..=0x1000_2E03) {
                assert!(has(id), "window {base:#x} lacks part {id:#x}");
            }
        }
        let parts: &[u32] = match base {
            0x1000_22A9 => &[0x1000_22AA],
            0x1000_25AF => &[0x1000_25B0, 0x1000_25B1],
            _ => &[],
        };
        for id in parts {
            assert!(
                has_descendant(&r.children, *id),
                "{base:#x} lacks descendant {id:#x}"
            );
        }
        resolved += 1;
    }
    eprintln!("§14.2.5: {resolved} / 10 base elements resolve");

    // The controls are built from the layout.
    let mut fx = Fx::with_ui_env(Box::new(env), vocab);
    fx.plugin("Test", "Probe", Some("Probe"), "");
    fx.refresh();
    fx.load("Probe");
    fx.eval::<()>(
        "Probe",
        "w = Turbine.UI.Lotro.Window() w:SetSize(400, 300) \
         g = Turbine.UI.Lotro.GoldWindow() g:SetSize(200, 300) \
         b = Turbine.UI.Lotro.Button() gb = Turbine.UI.Lotro.GoldButton() \
         c = Turbine.UI.Lotro.CheckBox() c:SetChecked(true) \
         t = Turbine.UI.Lotro.TextBox() t:SetText('Probe') \
         sh = Turbine.UI.Lotro.ScrollBar() \
         sv = Turbine.UI.Lotro.ScrollBar() sv:SetOrientation(Turbine.UI.Orientation.Vertical)",
    );
    let mut built = 0;
    for (name, part) in [
        ("w", 0x1000_2297),
        ("g", 0x1000_2297),
        ("c", 0x1000_25B0),
        ("c", 0x1000_25B1),
        ("t", 0x1000_22AA),
    ] {
        let k = fx.control_key("Probe", name);
        assert!(
            fx.pui.ui().borrow().find_descendant(k, part).is_some(),
            "{name} is not built from the layout (part {part:#x})"
        );
        built += 1;
    }
    for name in ["b", "gb"] {
        let k = fx.control_key("Probe", name);
        let ui = fx.pui.ui().borrow();
        assert!(
            ui.element_id(k).is_some_and(|id| id >= 0x7000_0000),
            "{name} has its run-time ID"
        );
        built += 1;
    }
    // The scroll bars carry the layout's scroll bar as a part (§14.2.2).
    for name in ["sh", "sv"] {
        let k = fx.control_key("Probe", name);
        let ui = fx.pui.ui().borrow();
        assert!(
            ui.children(k)
                .iter()
                .any(|c| ui.class(*c) == Some(tessera_ui::WidgetClass::Scrollbar)),
            "{name} has no layout scroll bar"
        );
        built += 1;
    }
    // §14.2.3 title layout with m = 0 (no text shaper in this test): t = min(W - 140, 120).
    for (name, w) in [("w", 400), ("g", 200)] {
        let k = fx.control_key("Probe", name);
        let ui = fx.pui.ui().borrow();
        let t = (w - 140).min(120);
        let text = ui
            .find_descendant(k, 0x1000_2297)
            .and_then(|p| ui.layout_box(p))
            .unwrap();
        assert_eq!((text.l, text.w()), ((w - t) / 2, t), "{name} title text");
        for id in [0x1000_2296, 0x1000_2DFB] {
            let b = ui
                .find_descendant(k, id)
                .and_then(|p| ui.layout_box(p))
                .unwrap();
            assert_eq!(
                (b.l, b.w()),
                ((w - (t + 80)) / 2, t + 80),
                "{name} part {id:#x}"
            );
        }
    }
    // §13.21 rule 1, §14.2.1 step 3: the first marking rebuilds both windows from their
    // scaled containers; the frame's parts are there again.
    let before = [fx.control_key("Probe", "w"), fx.control_key("Probe", "g")];
    fx.eval::<()>(
        "Probe",
        "w:SetScale(1.25) g:SetUIScalingGroup(5) w:SetVisible(true)",
    );
    for (name, old) in ["w", "g"].into_iter().zip(before) {
        let k = fx.control_key("Probe", name);
        assert_ne!(k, old, "{name} is rebuilt when first marked");
        let ui = fx.pui.ui().borrow();
        for id in [0x1000_2296, 0x1000_2297, 0x1000_2298, 0x1000_2DFB] {
            assert!(
                ui.find_descendant(k, id).is_some(),
                "{name}: scaled container lacks part {id:#x}"
            );
        }
    }
    eprintln!("both windows rebuilt from their scaled containers");
    // §14.2.3 step 3, §14.2.6 (d): the title bar rises above the window, but its rows above
    // the window's top edge are cut: the window's canvas is the window's box and no more.
    let w = fx.control_key("Probe", "w");
    let rise = {
        let ui = fx.pui.ui().borrow();
        let t = ui
            .find_descendant(w, 0x1000_2296)
            .and_then(|p| ui.layout_box(p))
            .unwrap()
            .t;
        (-t).max(0)
    };
    // The install's title bar rises above the window (§14.2.3: top −8); without that the
    // check below would test nothing.
    assert!(rise > 0, "the title bar starts above the window's top edge");
    let list = {
        let mut ui = fx.pui.ui().borrow_mut();
        let mut budget = tessera_ui::paint::Unlimited;
        let mut list = tessera_ui::paint::CompositionList::default();
        for t in 0..60u64 {
            list = ui
                .paint(std::time::Duration::from_millis(16 * t + 1), &mut budget)
                .composition;
            if !list.entries.is_empty() && ui.paint_settled() {
                break;
            }
        }
        list
    };
    {
        let ui = fx.pui.ui().borrow();
        let id = ui.surface_of(w).expect("the window has a surface");
        let e = list
            .entries
            .iter()
            .find(|e| {
                e.texture
                    == tessera_ui::paint::EntryTexture::Surface {
                        surface: id,
                        layer: tessera_ui::paint::Layer::Below,
                    }
            })
            .expect("the window is composed");
        assert_eq!(
            e.dest,
            ui.physical_box(w),
            "the canvas is the window's box: the title bar's rows above it are cut"
        );
        eprintln!("title bar rows above the window cut: {rise}");
    }
    fx.frame(0.016);
    fx.assert_no_errors();
    eprintln!("game-styled controls built from the install layout: {built} / 9");
}
