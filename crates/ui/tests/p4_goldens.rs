// SPDX-License-Identifier: GPL-3.0-or-later
//! Committed synthetic goldens of the P4 social panels (decision 0031 tier 1,
//! `social-panels.md`): the fellowship display (§4), the invitation prompt (§5), the trade
//! window (§6) and the Social panel's friends and ignore pages (§3.2–§3.6). Each window is
//! a **synthetic layout** (invented element trees with the vocabulary's placeholder IDs,
//! the rects and sizes the spec gives, generated art), opened through its controller's
//! `open` from the layout key as the client does with the install's layouts, filled from a
//! hand-built model with invented names, painted by the CPU painter and compared with
//! `tests/golden/p4_<name>.png` (one step per channel, no outliers). `TESSERA_BLESS=1`
//! rewrites the images. No install file is opened; texts are Tessera's own wording
//! (`NoTables`) and the labels of the synthetic layout, shaped with the bundled font.
//!
//! These are **widget goldens**, not scenario goldens: `tessera-ui` links no session
//! layer. What each picture protects is listed at its test.

mod p5_common;

use std::collections::HashMap;
use std::sync::Arc;

use p5_common::ui_common::{
    array, b, child, en, field, image_item, int, layout, media, prop, state,
};
use p5_common::{ART, Rgba, TestEnv, capture, golden_file, settle};
use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::string_info::{StringInfo, StringSource};
use tessera_formats::ui_layout::{ElementDesc, UiLayout};
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::game::CharVocab;
use tessera_ui::game::IconImage;
use tessera_ui::game::chat::notice::NoTables;
use tessera_ui::game::social::SocialVocab;
use tessera_ui::game::social::fellowship::{Fellowship, FellowshipPanels, Member, VitalKind};
use tessera_ui::game::social::friends::{FriendEntry, FriendsModel, FriendsPage};
use tessera_ui::game::social::ignore::{IgnoreModel, IgnorePage};
use tessera_ui::game::social::invite::{Invitation, InviteWindow};
use tessera_ui::game::social::panel::{SocialPage, SocialPanel};
use tessera_ui::game::social::trade::{
    CoinField, CoinValues, Side, Trade, TradeEntry, TradeWindow,
};
use tessera_ui::{ElementKey, ImageRef, ImageState, PreparedText, TextRequest, Ui, UiEnv, Vocab};

/// The one synthetic layout every key resolves to.
const L: u32 = 0x22FF_6000;

/// Element IDs of the synthetic layout's own labels and frames (no vocabulary entry).
const OWN: u32 = 0x7D00_0000;

/// Draw mode of the generated art (the fallback widgets' mode).
const ART_MODE: u32 = 4;

/// Extra generated images of these goldens.
const P4_ART: u32 = 0x41FC_0000;
const HEALTH_FILL: u32 = P4_ART;
const POWER_FILL: u32 = P4_ART + 1;
const METER_BACK: u32 = P4_ART + 2;
const DEFEATED_ART: u32 = P4_ART + 3;
const ITEM_ROPE: u32 = P4_ART + 4;
const ITEM_LAMP: u32 = P4_ART + 5;
const ITEM_RING: u32 = P4_ART + 6;
const PORTRAIT: u32 = P4_ART + 7;
const PORTRAIT_SMALL: u32 = P4_ART + 8;
const SLOT32: u32 = P4_ART + 9;
const ACCEPTED: u32 = P4_ART + 10;

fn sv() -> SocialVocab {
    SocialVocab::default()
}

fn v() -> Vocab {
    Vocab::default()
}

fn own(n: u32) -> u32 {
    OWN + n
}

fn image(w: u32, h: u32, px: impl Fn(u32, u32) -> u32) -> UiImage {
    let data = (0..w * h).map(|i| px(i % w, i / w)).collect();
    UiImage::new(w, h, w, h, PixelLayout::A8R8G8B8, data).expect("image")
}

/// A round portrait frame of `d` pixels (the art draws at its own size, not stretched).
fn portrait(d: u32) -> UiImage {
    let r = d as i32;
    image(d, d, |x, y| {
        let (dx, dy) = (x as i32 * 2 + 1 - r, y as i32 * 2 + 1 - r);
        let q = dx * dx + dy * dy;
        if q <= (r - 6) * (r - 6) {
            0xFF3A_4A6A
        } else if q <= r * r {
            0xFFB0_9A60
        } else {
            0
        }
    })
}

/// A framed 32 × 32 item icon with a diagonal band.
fn item_icon(edge: u32, fill: u32, band: u32) -> UiImage {
    image(32, 32, |x, y| {
        if x == 0 || y == 0 || x == 31 || y == 31 {
            edge
        } else if (x + y).abs_diff(31) < 4 {
            band
        } else {
            fill
        }
    })
}

/// The environment: the P5 test environment (synthetic layouts, generated art, the bundled
/// font) with the layout keys of the social panels and the social vocabulary's colour
/// property.
struct SocialEnv {
    inner: TestEnv,
    keys: HashMap<u32, u32>,
    social_colour: u32,
    npc_colour: u32,
}

impl UiEnv for SocialEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        self.inner.layout(did)
    }

    fn layout_by_key(&mut self, key: u32) -> Option<Did> {
        self.keys.get(&key).map(|d| Did(*d))
    }

    fn image(&mut self, image: &ImageRef) -> ImageState {
        self.inner.image(image)
    }

    fn shape_text(&mut self, r: &TextRequest<'_>) -> Option<Arc<PreparedText>> {
        // The test environment reads the NPC vocabulary's colour property; the social
        // panels' texts carry the social vocabulary's.
        let props: Vec<(u32, PropertyValue)> = r
            .props
            .iter()
            .map(|(p, v)| {
                let p = if *p == self.social_colour {
                    self.npc_colour
                } else {
                    *p
                };
                (p, v.clone())
            })
            .collect();
        let req = TextRequest {
            text: r.text,
            props: &props,
            size: r.size,
            wrap_width: r.wrap_width,
        };
        self.inner.shape_text(&req)
    }

    fn resolve_string(&mut self, info: &StringInfo) -> Option<String> {
        match &info.source {
            StringSource::Literal(s) => Some(s.to_string()),
            _ => None,
        }
    }
}

/// A UI on a `w` × `h` screen whose layout keys of the social panels all name one layout
/// holding `elements`.
fn ui(w: i32, h: i32, elements: Vec<ElementDesc>) -> Ui {
    let vocab = sv();
    let mut inner = TestEnv::new();
    inner
        .layouts
        .insert(L, Arc::new(layout(L, 1024, 768, elements)));
    let mut add = |did: u32, img: UiImage| {
        inner.images.insert(ImageRef::Did(did), Arc::new(img));
    };
    add(
        HEALTH_FILL,
        image(99, 6, |_, y| if y < 2 { 0xFF5C_C86A } else { 0xFF2E_9A44 }),
    );
    add(
        POWER_FILL,
        image(99, 6, |_, y| if y < 2 { 0xFF6A_9CE8 } else { 0xFF2E_5AB8 }),
    );
    add(METER_BACK, image(4, 4, |_, _| 0xFF14_1414));
    add(DEFEATED_ART, image(4, 4, |_, _| 0xC060_1818));
    add(ITEM_ROPE, item_icon(0xFF8A_7A50, 0xFF4A_3A22, 0xFFC8_A870));
    add(ITEM_LAMP, item_icon(0xFF8A_7A50, 0xFF22_2A3A, 0xFFF0_D060));
    add(ITEM_RING, item_icon(0xFF8A_7A50, 0xFF20_2020, 0xFFD0_D0E0));
    add(PORTRAIT, portrait(50));
    add(PORTRAIT_SMALL, portrait(25));
    add(
        SLOT32,
        image(32, 32, |x, y| {
            if x == 0 || y == 0 || x == 31 || y == 31 {
                0xFF70_6050
            } else {
                0xFF10_1010
            }
        }),
    );
    add(ACCEPTED, image(4, 4, |_, _| 0xFF3A_4A22));
    let k = vocab.layouts;
    let keys = [
        k.social,
        k.friends,
        k.ignore,
        k.minifellow,
        k.fellowinvite,
        k.securetrade,
        k.itemdisplay,
    ]
    .into_iter()
    .map(|key| (key, L))
    .collect();
    let env = SocialEnv {
        inner,
        keys,
        social_colour: vocab.pids.text_font_color,
        npc_colour: vocab.npc.pids.text_font_color,
    };
    Ui::new(Box::new(env), Vocab::default(), (w, h))
}

fn colour(argb: u32) -> PropertyValue {
    PropertyValue::Colour(argb.to_le_bytes())
}

/// A field drawing generated art `i` (an index of the P5 art, or a DID of the P4 art).
fn art_field(id: u32, x: i32, y: i32, w: i32, h: i32, i: u32) -> ElementDesc {
    let did = if i < 0x100 { ART + i } else { i };
    media(field(id, x, y, w, h), image_item(did, ART_MODE))
}

/// A text element in `argb`, holding `default` as its layout text when not empty.
fn text(id: u32, x: i32, y: i32, w: i32, h: i32, argb: u32, default: &str) -> ElementDesc {
    let d = prop(
        typed(id, v().types.text, x, y, w, h),
        sv().pids.text_font_color,
        colour(argb),
    );
    if default.is_empty() {
        d
    } else {
        prop(
            d,
            v().pids.text_entry,
            PropertyValue::StringInfo(Box::new(StringInfo {
                source: StringSource::Literal(default.into()),
                args: None,
            })),
        )
    }
}

fn typed(id: u32, ty: u32, x: i32, y: i32, w: i32, h: i32) -> ElementDesc {
    p5_common::ui_common::typed(id, ty, x, y, w, h)
}

/// A text button with the button art (`normal`, `highlight`, ghosted greyed).
fn button(id: u32, x: i32, y: i32, w: i32, h: i32, label: &str) -> ElementDesc {
    let st = v().states;
    let c = sv().pids.text_font_color;
    let mut d = media(
        typed(id, v().types.button, x, y, w, h),
        image_item(ART + 5, ART_MODE),
    );
    d = prop(d, c, colour(0xFFF5_DF92));
    d = state(d, st.normal, 0, vec![], vec![image_item(ART + 5, ART_MODE)]);
    d = state(
        d,
        st.highlight,
        0,
        vec![],
        vec![image_item(ART + 9, ART_MODE)],
    );
    d = state(
        d,
        st.ghosted,
        0,
        vec![(c, colour(0xFF80_8080))],
        vec![image_item(ART + 6, ART_MODE)],
    );
    if label.is_empty() {
        d
    } else {
        prop(
            d,
            v().pids.text_entry,
            PropertyValue::StringInfo(Box::new(StringInfo {
                source: StringSource::Literal(label.into()),
                args: None,
            })),
        )
    }
}

/// An image button (close, assist).
fn icon_button(id: u32, x: i32, y: i32, w: i32, h: i32, i: u32) -> ElementDesc {
    let st = v().states;
    let mut d = media(
        typed(id, v().types.button, x, y, w, h),
        image_item(ART + i, ART_MODE),
    );
    for s in [st.normal, st.highlight, st.ghosted] {
        d = state(d, s, 0, vec![], vec![image_item(ART + i, ART_MODE)]);
    }
    d
}

/// A row template: a Button of `w` × `h` with the row art (`normal`) and the highlighted
/// row art (`highlight`, the selected list item).
fn row(id: u32, w: i32, h: i32) -> ElementDesc {
    let st = v().states;
    let mut d = media(
        typed(id, v().types.button, 0, 0, w, h),
        image_item(ART + 3, ART_MODE),
    );
    d = state(d, st.normal, 0, vec![], vec![image_item(ART + 3, ART_MODE)]);
    d = state(
        d,
        st.highlight,
        0,
        vec![],
        vec![image_item(ART + 4, ART_MODE)],
    );
    d
}

/// A window frame: the window art, a title bar with `title` and a close button.
fn frame(id: u32, x: i32, y: i32, w: i32, h: i32, title: &str, close: u32) -> ElementDesc {
    let bar = child(
        art_field(own(0x900) + id % 0x100, 0, 0, w, 24, 1),
        text(
            own(0xA00) + id % 0x100,
            8,
            4,
            w - 40,
            16,
            0xFFF5_DF92,
            title,
        ),
    );
    child(
        child(art_field(id, x, y, w, h, 0), bar),
        icon_button(close, w - 24, 4, 16, 16, 7),
    )
}

fn hidden(d: ElementDesc) -> ElementDesc {
    prop(d, v().pids.hide, b(true))
}

/// A child meter filling from the left with `fill` (`ui-widgets.md` §4.3).
fn meter(id: u32, x: i32, y: i32, w: i32, h: i32, fill: u32) -> ElementDesc {
    let p = v().pids;
    let mut d = typed(id, v().types.meter, x, y, w, h);
    d = prop(d, p.meter_child, b(true));
    d = prop(d, p.meter_direction, en(4));
    d = prop(d, p.meter_child_image, p5_common::ui_common::did(fill));
    d = prop(d, p.meter_child_drawmode, en(ART_MODE));
    d
}

/// A list whose items take `highlight` when selected and `normal` otherwise, selected by
/// a click (§3.2 `FriendListBox`, §3.6 `IgnoreListBox`).
fn selecting(d: ElementDesc) -> ElementDesc {
    let p = v().pids;
    let st = v().states;
    let mut d = d;
    for (pid, value) in [
        (p.lb_state_change, b(true)),
        (p.lb_normal_state, en(st.normal)),
        (p.lb_selected_state, en(st.highlight)),
        (p.lb_click_select, b(true)),
    ] {
        d = prop(d, pid, value);
    }
    d
}

fn find(ui: &Ui, root: ElementKey, id: u32) -> ElementKey {
    tessera_ui::game::find_below(ui, root, id).unwrap_or_else(|| panic!("no element {id:#x}"))
}

fn picture(ui: &mut Ui, root: ElementKey) -> Rgba {
    settle(ui);
    capture(ui, root)
}

// ----- §4 fellowship display ---------------------------------------------------------------

fn fellowship_layout() -> Vec<ElementDesc> {
    let i = sv().ids;
    let st = sv().states;
    let mut out = Vec::new();
    // §4.1: four panels of 221 × 452 with the list box at (0, 20); the drag-box frame is
    // hidden in play (GP §4), so the panels draw only their rows.
    for (id, x) in [
        (i.fellow_panel_1, 14),
        (i.fellow_panel_2, 234),
        (i.fellow_panel_3, 586),
        (i.fellow_panel_4, 810),
    ] {
        out.push(child(
            field(id, x, 121, 221, 452),
            typed(
                i.fellow_list,
                sv().types.listbox_mouse_invisible,
                0,
                20,
                209,
                432,
            ),
        ));
    }
    // §4.2: the mini template, 221 × 75.
    let mut leader = media(
        field(i.fellow_leader, 39, 34, 16, 16),
        image_item(ART + 26, ART_MODE),
    );
    for s in [st.leader_fellowship, st.leader_raid, st.leader_monster] {
        leader = state(leader, s, 0, vec![], vec![image_item(ART + 26, ART_MODE)]);
    }
    let display = |id: u32, x: i32, y: i32, current: u32, fill: u32, temp: u32, extra: u32| {
        let mut d = field(id, x, y, 99, 6);
        d = child(d, art_field(extra, 0, 0, 99, 6, METER_BACK));
        d = child(d, meter(current, 0, 0, 99, 6, fill));
        d = child(d, meter(temp, 0, 0, 99, 6, ART + 4));
        d
    };
    let mut health = display(
        i.vital_health,
        52,
        25,
        i.vital_health_meter,
        HEALTH_FILL,
        i.vital_temp_health,
        own(0x10),
    );
    health = child(health, meter(i.vital_dread, 0, 0, 99, 6, ART + 29));
    health = child(
        health,
        text(i.vital_health_text, 0, -2, 99, 10, 0xFFFF_FFFF, "100"),
    );
    let mut power = display(
        i.vital_power,
        51,
        32,
        i.vital_power_meter,
        POWER_FILL,
        i.vital_temp_power,
        own(0x11),
    );
    power = child(
        power,
        text(i.vital_power_text, 0, -2, 99, 10, 0xFFFF_FFFF, "100"),
    );
    power = child(
        power,
        child(
            field(i.vital_rage_field, 0, 0, 99, 6),
            meter(own(0x12), 0, 0, 99, 6, ART + 29),
        ),
    );
    let vitals = child(
        child(
            child(
                field(i.fellow_vitals, 3, -3, 170, 80),
                art_field(own(0x13), 0, 3, 200, 72, 2),
            ),
            health,
        ),
        power,
    );
    let status = |id: u32, t: u32, label: &str| {
        child(
            art_field(id, 55, 20, 112, 16, DEFEATED_ART),
            text(t, 4, 1, 104, 14, 0xFFFF_6060, label),
        )
    };
    let mut row = field(i.fellow_template, 0, 0, 221, 75);
    for c in [
        vitals,
        art_field(i.fellow_class, 5, 3, 50, 50, PORTRAIT),
        text(i.fellow_name, 56, 2, 133, 14, 0xFFF5_DF92, "Design name"),
        text(i.fellow_level, 12, 42, 32, 19, 0xFFFF_FFFF, "99"),
        leader,
        status(i.fellow_link_dead, own(0x20), "Link-dead"),
        status(i.fellow_defeated, own(0x21), "Defeated"),
        icon_button(i.fellow_assist, 153, 21, 16, 16, 20),
        // §4.2.1: none hidden by the layout, all hidden by TR14.
        icon_button(i.fellow_voice, 38, 0, 20, 20, 33),
        art_field(i.fellow_voice_squelched, 38, 0, 20, 20, 29),
        art_field(i.fellow_sending, 38, 0, 20, 20, 28),
        hidden(art_field(i.fellow_volume, 0, 0, 20, 60, 2)),
        hidden(art_field(i.fellow_pet_vitals, 0, 0, 20, 20, 2)),
    ] {
        row = child(row, c);
    }
    out.push(row);
    out
}

/// The fellowship display (§4, TR2, TR3, TR14, TR15): panel 1 with one row per member but
/// the player, in the feed's order; names and levels through their template; the leader
/// marker on the leader's row only; the defeated field on the defeated member's row; only
/// the current morale and power meters filled; no voice, squelched, sending-data, volume
/// or pet element; a member without an entity with an empty level and empty meters.
#[test]
fn fellowship_display() {
    let mut ui = ui(1024, 768, fellowship_layout());
    let vocab = sv();
    let mut panels = FellowshipPanels::open(&mut ui, &vocab);
    let mut f = Fellowship::new("Tamsin", Some(1));
    f.changed(&[
        Member {
            name: "Tamsin".into(),
            entity: Some(1),
            leader: false,
        },
        Member {
            name: "Brin".into(),
            entity: Some(2),
            leader: true,
        },
        Member {
            name: "Cael".into(),
            entity: Some(3),
            leader: false,
        },
        Member {
            name: "Dara".into(),
            entity: None,
            leader: false,
        },
    ]);
    f.level(2, 42);
    f.vital(2, VitalKind::Morale, 3000.0, 4000.0);
    f.vital(2, VitalKind::Power, 1200.0, 3000.0);
    f.level(3, 38);
    f.vital(3, VitalKind::Morale, 0.0, 3500.0);
    f.vital(3, VitalKind::Power, 2500.0, 2500.0);
    f.defeated(3, true);
    panels.refresh(&mut ui, &f, &mut NoTables);
    assert_eq!(panels.rows.len(), 3);
    let root = panels.panels[0];
    let img = picture(&mut ui, root);
    golden_file("p4_fellowship", &img);
}

// ----- §5 invitation prompt ----------------------------------------------------------------

fn invite_layout() -> Vec<ElementDesc> {
    let i = sv().ids;
    let mut win = art_field(i.invite, 62, 86, 354, 431, 0);
    for c in [
        child(
            art_field(own(0x30), 57, -7, 240, 45, 1),
            text(i.invite_title, 0, 15, 240, 20, 0xFFF5_DF92, "Design title"),
        ),
        text(i.invite_text, 20, 38, 316, 63, 0xFFC0_C0C0, ""),
        art_field(i.invite_list, 21, 104, 313, 220, 2),
        text(
            i.invite_warning,
            20,
            328,
            316,
            67,
            0xFFFF_6060,
            "Design warning",
        ),
        button(i.invite_accept, 67, 401, 100, 20, "Accept"),
        button(i.invite_decline, 187, 401, 100, 20, "Decline"),
        icon_button(i.invite_close, 334, 25, 16, 16, 7),
    ] {
        win = child(win, c);
    }
    // The list is a ListBox (§5.1); give it its type over the art field.
    if let Some(l) = win
        .children
        .iter_mut()
        .find(|c| c.element_id == i.invite_list)
    {
        l.element_type = v().types.list_box;
    }
    let mut r = media(
        field(i.invite_row, 0, 0, 296, 39),
        image_item(ART + 3, ART_MODE),
    );
    for c in [
        art_field(i.invite_row_class, 6, 7, 25, 25, PORTRAIT_SMALL),
        text(i.invite_row_level, 36, 10, 40, 20, 0xFFFF_FFFF, "123"),
        text(
            i.invite_row_name,
            80,
            10,
            110,
            20,
            0xFFF5_DF92,
            "Design name",
        ),
        text(
            i.invite_row_area,
            192,
            10,
            100,
            20,
            0xFFC0_C0C0,
            "Menu option 2",
        ),
    ] {
        r = child(r, c);
    }
    vec![win, r]
}

/// The invitation prompt (§5.1, §5.2, TR7): the title and the invitation text through
/// their templates, one row (the inviter) with an empty level and area (never the layout's
/// design-time texts), no warning, the accept and decline buttons.
#[test]
fn invitation_prompt() {
    let mut ui = ui(1024, 768, invite_layout());
    let vocab = sv();
    let mut w = InviteWindow::open(&mut ui, &vocab).unwrap();
    let mut m = Invitation::default();
    m.invite("Brin");
    w.refresh(&mut ui, &m, &mut NoTables);
    assert_eq!(w.rows.len(), 1);
    let root = w.root;
    let i = vocab.ids;
    // TR7: the layout's design-time texts are cleared, not shown.
    let warning = find(&ui, root, i.invite_warning);
    assert_eq!(ui.text_of(warning), "", "no warning");
    for id in [i.invite_row_level, i.invite_row_area] {
        let k = find(&ui, w.rows[0], id);
        assert_eq!(ui.text_of(k), "", "{id:#x}: no level or area");
    }
    let img = picture(&mut ui, root);
    golden_file("p4_invite", &img);
}

// ----- §6 trade window ---------------------------------------------------------------------

fn values() -> CoinValues {
    CoinValues {
        gold: 100_000,
        silver: 100,
        copper: 1,
    }
}

fn trade_layout() -> Vec<ElementDesc> {
    let vocab = sv();
    let cv = CharVocab::default();
    let i = vocab.ids;
    let st = v().states;
    let nv = &vocab.npc;
    // §6.1: the side fields' trade fields, `normal` and `highlight` (accepted).
    let trade_field = |id: u32, list: u32, list_w: i32, list_h: i32| {
        let mut d = art_field(id, 0, 48, 252, 264, 2);
        d = state(d, st.normal, 0, vec![], vec![image_item(ART + 2, ART_MODE)]);
        d = state(
            d,
            st.highlight,
            0,
            vec![],
            vec![image_item(ACCEPTED, ART_MODE)],
        );
        child(d, typed(list, v().types.list_box, 13, 6, list_w, list_h))
    };
    // §6.1 "Money displays": a horizontal list of one row and at most three columns,
    // only visible items laid out, filled in reverse.
    let money = |id: u32, ty: u32, x: i32, y: i32, suppress: bool| {
        let p = v().pids;
        let mut d = typed(id, ty, x, y, 160, 22);
        for (pid, value) in [
            (p.lb_horizontal, b(true)),
            (p.lb_max_rows, int(1)),
            (p.lb_max_columns, int(3)),
            (p.lb_only_visible, b(true)),
            (p.lb_reverse, b(true)),
            (nv.pids.force_show_all, b(true)),
            (nv.pids.suppress_leading_zeroes, b(suppress)),
        ] {
            d = prop(d, pid, value);
        }
        d
    };
    // §6.1: each coin field holds a label and its editable entry.
    let coin = |id: u32, entry: u32, x: i32, label: &str, n: u32| {
        child(
            child(
                field(id, x, 0, 50, 20),
                text(own(0x40 + n), 0, 2, 12, 16, 0xFFF5_DF92, label),
            ),
            prop(
                media(
                    text(entry, 13, 0, 36, 20, 0xFFFF_FFFF, ""),
                    image_item(ART + 10, ART_MODE),
                ),
                v().pids.text_editable,
                b(true),
            ),
        )
    };
    let mut coins = field(own(0x48), 44, 235, 170, 20);
    for (n, (id, entry, x, label)) in [
        (i.coin_gold, i.coin_gold_entry, 0, "g"),
        (i.coin_silver, i.coin_silver_entry, 58, "s"),
        (i.coin_copper, i.coin_copper_entry, 111, "c"),
    ]
    .into_iter()
    .enumerate()
    {
        coins = child(coins, coin(id, entry, x, label, n as u32));
    }
    let own_side = {
        let mut d = field(own(0x50), 7, 39, 252, 340);
        d = child(d, text(own(0x51), 4, 0, 200, 18, 0xFFF5_DF92, "Your offer"));
        d = child(
            d,
            child(
                child(
                    field(own(0x52), 0, 20, 252, 28),
                    text(own(0x53), 4, 6, 76, 16, 0xFFC0_C0C0, "Your money"),
                ),
                money(i.trade_wallet, nv.types.player_money_display, 83, 5, false),
            ),
        );
        d = child(
            d,
            child(
                trade_field(i.trade_own_field, i.trade_own_list, 226, 222),
                coins,
            ),
        );
        d = child(
            d,
            prop(
                button(i.trade_button, 31, 316, 96, 20, "Trade"),
                v().pids.button_boolean,
                b(true),
            ),
        );
        child(d, button(i.trade_cancel, 130, 316, 96, 20, "Cancel"))
    };
    let peer_side = {
        let mut d = field(own(0x60), 258, 38, 252, 340);
        d = child(d, text(i.trade_peer_label, 4, 1, 244, 18, 0xFFF5_DF92, ""));
        let tf = child(
            trade_field(i.trade_peer_field, i.trade_peer_list, 228, 225),
            money(i.trade_peer_money, nv.types.money_display, 45, 236, true),
        );
        child(d, tf)
    };
    let wait = child(
        art_field(i.trade_wait, 4, 85, 508, 266, 23),
        text(own(0x61), 150, 120, 200, 20, 0xFFFF_FFFF, "Waiting"),
    );
    let win = child(
        child(
            child(
                frame(i.trade, 254, 118, 518, 388, "Trade", i.trade_close),
                own_side,
            ),
            peer_side,
        ),
        wait,
    );
    // §6.2: the medium item display, the quantity overlay before the name.
    let pid = vocab.pids.text_font_colors;
    let name_colour =
        |argb: u32| vec![(pid, array(vocab.pids.text_font_color, vec![colour(argb)]))];
    let mut name = text(i.item_name, 35, 0, 165, 32, 0xFFFF_FFFF, "");
    for (s, argb) in [
        (vocab.states.object_name, 0xFFFF_FFFF),
        (vocab.states.quality_common, 0xFFE0_E0E0),
        (vocab.states.quality_uncommon, 0xFFF0_E040),
        (vocab.states.quality_rare, 0xFFE0_60F0),
        (vocab.states.quality_incomparable, 0xFF40_E0E0),
        (vocab.states.quality_legendary, 0xFFF0_A030),
    ] {
        name = state(name, s, 0, name_colour(argb), vec![]);
    }
    let quantity = hidden(text(cv.ids.quantity, 16, 18, 16, 14, 0xFFFF_FFFF, ""));
    let medium = child(
        child(
            field(i.item_medium, 0, 0, 200, 32),
            child(
                art_field(own(0x70), 0, 0, 32, 32, SLOT32),
                child(
                    typed(i.item_icon, cv.types.item_icon_display, 0, 0, 32, 32),
                    quantity,
                ),
            ),
        ),
        name,
    );
    vec![win, medium]
}

fn item(name: &str, item: u64, icon: u32, quantity: u32, quality: Option<u32>) -> TradeEntry {
    TradeEntry {
        name: name.into(),
        item: Some(item),
        quantity,
        icon: Some(IconImage::Composed(ImageRef::Did(icon))),
        quality,
        literal: false,
    }
}

/// The trade window (§6.1–§6.4, TR9, TR10): the peer label through its template; both
/// one-column lists of medium item displays with icons, the name in its quality colour,
/// the quantity shown above 1 and a literal entry without icon or quantity; the wallet
/// and the peer's offered coin as coin elements; the typed coin entries; the peer's side
/// field highlighted (accepted); the trade button not ghosted; the wait overlay hidden.
#[test]
fn trade_window() {
    let mut ui = ui(1024, 768, trade_layout());
    let vocab = sv();
    let st = vocab.states;
    let mut w = TradeWindow::open(&mut ui, &vocab, CharVocab::default()).unwrap();
    let mut t = Trade::new(values());
    t.set_wallet(2_503_040);
    t.open(1, Some("Dara"));
    t.set_offer(
        1,
        Side::Own,
        vec![
            item("Coil of rope", 70, ITEM_ROPE, 1, Some(st.quality_common)),
            item("Brass lamp", 71, ITEM_LAMP, 5, None),
        ],
        0,
    );
    t.set_offer(
        1,
        Side::Peer,
        vec![
            item("Silver ring", 80, ITEM_RING, 1, None),
            TradeEntry {
                name: "Old map".into(),
                quantity: 1,
                literal: true,
                ..TradeEntry::default()
            },
        ],
        1_234_567,
    );
    t.type_coin(CoinField::Gold, "2");
    t.type_coin(CoinField::Copper, "15");
    t.set_ready(1, Side::Peer, true);
    w.refresh(&mut ui, &t, &mut NoTables);
    assert_eq!((w.entries.0.len(), w.entries.1.len()), (2, 2));
    let root = w.root;
    let img = picture(&mut ui, root);
    golden_file("p4_trade", &img);
}

// ----- §3 Social panel: friends and ignore pages -------------------------------------------

fn social_layout() -> Vec<ElementDesc> {
    let i = sv().ids;
    let st = sv().states;
    let vp = v().pids;
    let pages_of = |list: &[(u32, u32, bool)]| {
        array(
            0x77,
            list.iter()
                .map(|(t, p, o)| {
                    p5_common::ui_common::strukt(vec![
                        (vp.panel_tab, en(*t)),
                        (vp.panel_page, en(*p)),
                        (vp.panel_page_open, b(*o)),
                    ])
                })
                .collect(),
        )
    };
    let tab = |id: u32, x: i32, y: i32, w: i32, h: i32, label: &str| {
        let s = v().states;
        let mut d = button(id, x, y, w, h, label);
        d.states.clear();
        d.block.media.clear();
        d = media(d, image_item(ART + 8, ART_MODE));
        d = state(d, s.normal, 0, vec![], vec![image_item(ART + 8, ART_MODE)]);
        d = state(
            d,
            s.highlight,
            0,
            vec![],
            vec![image_item(ART + 9, ART_MODE)],
        );
        state(
            d,
            s.ghosted,
            0,
            vec![(sv().pids.text_font_color, colour(0xFF80_8080))],
            vec![image_item(ART + 8, ART_MODE)],
        )
    };
    // §3.2: the friends page.
    let mut fpage = field(i.friends_page, 0, 0, 565, 470);
    fpage = child(fpage, art_field(own(0x80), 7, 54, 544, 380, 2));
    for (id, x, w, label) in [
        (i.friend_header_grouped, 3, 16, ""),
        (i.friend_header_class, 25, 40, "Class"),
        (i.friend_header_level, 64, 35, "Lvl"),
        (i.friend_header_member, 92, 120, "Name"),
        (i.friend_header_area, 215, 100, "Area"),
        (i.friend_header_last_on, 323, 109, "Last online"),
        (i.friend_header_note, 434, 109, "Note"),
    ] {
        fpage = child(fpage, button(id, 6 + x, 34, w, 16, label));
    }
    fpage = child(
        fpage,
        selecting(typed(i.friend_list, v().types.tree, 10, 58, 540, 371)),
    );
    for (id, x, label) in [
        (i.friend_add, 29, "Add"),
        (i.friend_remove, 124, "Remove"),
        (i.friend_note, 250, "Note"),
        (i.friend_invite, 343, "Invite"),
        (i.friend_tell, 437, "Tell"),
    ] {
        fpage = child(fpage, button(id, x, 439, 93, 20, label));
    }
    // §3.6: the ignore page.
    let mut ipage = field(i.ignore_page, 0, 0, 565, 470);
    ipage = child(ipage, art_field(own(0x81), 7, 54, 544, 380, 2));
    ipage = child(
        ipage,
        button(i.ignore_header_member, 6, 34, 260, 16, "Name"),
    );
    ipage = child(
        ipage,
        button(i.ignore_header_note, 272, 34, 270, 16, "Note"),
    );
    ipage = child(
        ipage,
        selecting(typed(i.ignore_list, v().types.list_box, 10, 58, 540, 371)),
    );
    for (id, x, label) in [
        (i.ignore_add, 29, "Add"),
        (i.ignore_remove, 124, "Remove"),
        (i.ignore_note, 250, "Note"),
    ] {
        ipage = child(ipage, button(id, x, 439, 93, 20, label));
    }
    // §3.1: `FriendsPagePanel` with its two tabs at the bottom.
    let mut fpanel = prop(
        typed(i.friends_page_panel, v().types.panel, 0, 0, 565, 500),
        vp.panel_pages,
        pages_of(&[
            (i.friends_tab_marker, i.friends_page, true),
            (i.ignore_tab_marker, i.ignore_page, false),
        ]),
    );
    fpanel = child(fpanel, fpage);
    fpanel = child(fpanel, ipage);
    fpanel = child(
        fpanel,
        tab(i.friends_tab_marker, 20, 470, 160, 20, "Friends"),
    );
    fpanel = child(
        fpanel,
        tab(i.ignore_tab_marker, 185, 470, 160, 20, "Ignore"),
    );
    // §3.1: `Social_Panel` with five tabs and pages; the set-up page is the grouping page.
    let mut panel = prop(
        typed(i.social_panel, v().types.panel, 11, 30, 565, 537),
        vp.panel_pages,
        pages_of(&[
            (i.friends_tab, i.friends_page_outer, false),
            (i.kinship_tab, i.kinship_page, false),
            (i.grouping_tab, i.grouping_page, true),
            (i.raid_tab, i.raid_page, false),
            (i.raid_lock_tab, i.raid_lock_page, false),
        ]),
    );
    for (id, x, w, label) in [
        (i.grouping_tab, 5, 100, "Grouping"),
        (i.friends_tab, 107, 100, "Friends"),
        (i.kinship_tab, 209, 100, "Kinship"),
        (i.raid_tab, 311, 100, "Raid"),
        (i.raid_lock_tab, 413, 150, "Raid locks"),
    ] {
        panel = child(panel, tab(id, x, 2, w, 29, label));
    }
    panel = child(
        panel,
        child(
            field(i.grouping_page, 0, 33, 565, 500),
            button(own(0x82), 20, 20, 120, 20, "Who"),
        ),
    );
    panel = child(
        panel,
        child(field(i.friends_page_outer, 0, 33, 565, 500), fpanel),
    );
    for id in [i.kinship_page, i.raid_page, i.raid_lock_page] {
        panel = child(panel, field(id, 0, 33, 565, 500));
    }
    let root = child(
        frame(i.social_field, 262, 118, 587, 577, "Social", i.social_close),
        panel,
    );
    // §3.2: the friend row template, 539 × 30.
    let mut grouped = field(i.friend_row_grouped, 8, 6, 16, 16);
    grouped = state(grouped, st.grouped_unknown, 0, vec![], vec![]);
    grouped = state(grouped, st.grouped_none, 0, vec![], vec![]);
    grouped = state(
        grouped,
        st.grouped_yes,
        0,
        vec![],
        vec![image_item(ART + 19, ART_MODE)],
    );
    let name = prop(
        prop(
            text(i.friend_row_name, 88, 4, 120, 20, 0xFFFF_FFFF, ""),
            sv().pids.online_colour,
            colour(0xFF00_FF00),
        ),
        sv().pids.offline_colour,
        colour(0xFFDB_DB00),
    );
    let mut frow = row(i.friend_template, 539, 30);
    for c in [
        grouped,
        hidden(text(i.friend_row_class, 27, 4, 30, 20, 0xFFFF_FFFF, "")),
        text(i.friend_row_level, 50, 4, 35, 20, 0xFFC0_C0C0, ""),
        name,
        text(i.friend_row_area, 210, 4, 106, 20, 0xFFC0_C0C0, ""),
        text(i.friend_row_last_on, 319, 4, 107, 20, 0xFFC0_C0C0, ""),
        text(
            i.friend_row_note,
            430,
            2,
            108,
            22,
            0xFFC0_C0C0,
            "Design note",
        ),
    ] {
        frow = child(frow, c);
    }
    // §3.6: the ignore row template, 539 × 30.
    let mut irow = row(i.ignore_template, 539, 30);
    irow = child(
        irow,
        text(i.ignore_row_name, 5, 4, 257, 20, 0xFFFF_FFFF, ""),
    );
    irow = child(
        irow,
        text(
            i.ignore_row_note,
            266,
            -1,
            267,
            30,
            0xFFC0_C0C0,
            "Design note",
        ),
    );
    vec![root, frow, irow]
}

fn social_ui() -> (Ui, SocialPanel) {
    let mut ui = ui(1024, 768, social_layout());
    let vocab = sv();
    let panel = SocialPanel::open(&mut ui, &vocab).unwrap();
    (ui, panel)
}

/// The friends page (§3.2–§3.5, TR5, TR8): the Social panel on its friends page with the
/// other pages' controls ghosted; one row per friend in name order (case-insensitive),
/// online names green and offline names yellow, the grouped marker on a fellow's row, the
/// unknown text in the level and area columns, last online through its template when online
/// and unknown when offline, no note (the layout's design-time note cleared); the selected
/// row highlighted; Remove, Invite and Tell normal for a selected online friend who is not
/// grouped, Note ghosted.
#[test]
fn friends_list() {
    let (mut ui, mut panel) = social_ui();
    let vocab = sv();
    panel.show_page(&mut ui, SocialPage::Friends);
    let mut page = FriendsPage::bind(&mut ui, &vocab, panel.root).unwrap();
    let mut m = FriendsModel::default();
    let e = |name: &str, online: bool| FriendEntry {
        name: name.into(),
        online,
    };
    m.set_list(&[
        e("Edda", true),
        e("brin", false),
        e("Aldric", true),
        e("Dara", false),
        e("Cael", true),
    ]);
    m.set_fellows(&["Cael".to_string()]);
    m.select(Some("Aldric"));
    page.refresh(&mut ui, &m, &mut NoTables);
    assert_eq!(page.rows.len(), 5);
    let root = panel.root;
    for r in &page.rows {
        let note = find(&ui, *r, vocab.ids.friend_row_note);
        assert_eq!(ui.text_of(note), "", "TR5: no note");
    }
    let img = picture(&mut ui, root);
    golden_file("p4_friends", &img);
}

/// The ignore page (§3.6, TR6): the Social panel on its friends page with the ignore tab
/// open; one row per name in name order (case-insensitive) with no note (the layout's
/// design-time note cleared), the selected row highlighted, Remove normal with a
/// selection and Note ghosted (TR5).
#[test]
fn ignore_list() {
    let (mut ui, mut panel) = social_ui();
    let vocab = sv();
    let i = vocab.ids;
    panel.show_page(&mut ui, SocialPage::Friends);
    let root = panel.root;
    let fpanel = find(&ui, root, i.friends_page_panel);
    ui.panel_open(fpanel, i.ignore_page, i.ignore_tab_marker);
    let mut page = IgnorePage::bind(&mut ui, &vocab, root).unwrap();
    let mut m = IgnoreModel::default();
    m.set_list(&[
        "Torvald".to_string(),
        "anwen".to_string(),
        "Gustav".to_string(),
    ]);
    m.select(Some("Torvald"));
    page.refresh(&mut ui, &m);
    assert_eq!(page.rows.len(), 3);
    for r in &page.rows {
        let note = find(&ui, *r, i.ignore_row_note);
        assert_eq!(ui.text_of(note), "", "TR6: no note");
    }
    let img = picture(&mut ui, root);
    golden_file("p4_ignore", &img);
}
