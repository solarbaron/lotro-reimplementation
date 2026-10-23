// SPDX-License-Identifier: GPL-3.0-or-later
//! The portraits of the player and target frames (`combat-ui.md` §4.1, §5.1, CU32;
//! `ui-widgets.md` §12, §12.1–§12.4): what a `TargetViewport` element asks the client to
//! draw ([`PortraitSpec`], read from the element's viewport properties), which entity it
//! shows ([`PortraitSpec::entity`], §12.1 step 1), and the element's state while the client
//! draws a scene into it or not ([`UnitFrame::set_portrait_content`]: the scene is the
//! element's external content; without one the element is hidden with its media and
//! children, §12.4).
//!
//! The 3-D drawing itself is the client's (`tessera-render`'s viewport targets); this module
//! only reads and writes the element tree.

use tessera_formats::did::Did;

use crate::props::as_argb;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::CombatUi;
use super::data::EntityId;
use super::frames::{FrameKind, UnitFrame};
use super::vocab::CombatVocab;

/// The largest render-to-texture side the properties may ask for (`ui-widgets.md` §12:
/// `UICore_Viewport_RenderToTextureWidth` and `…Height` are Int 1–256).
pub const MAX_RENDER_TO_TEXTURE: u32 = 256;

/// The background of a viewport without the property: the dictionary default opaque black
/// (`ui-widgets.md` §12, `UICore_Viewport_RenderToTextureBackgroundColor`).
pub const DEFAULT_BACKGROUND: u32 = 0xFF00_0000;

/// What one portrait element asks the client to draw (`ui-widgets.md` §12.1, §12.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortraitSpec {
    /// The frame it belongs to: [`FrameKind::Player`] (`PlayerPortrait`, P4) or
    /// [`FrameKind::Target`] (`OpponentPortrait`, P5).
    pub kind: FrameKind,
    /// The `TargetViewport` element.
    pub element: ElementKey,
    /// `UICore_Viewport_uiscene` (a 0x21 UI scene), when set.
    pub scene: Option<Did>,
    /// `UICore_Viewport_camera` (a `UISceneEntity` key), when set (§12.1 step 4).
    pub camera: Option<u32>,
    /// The render target's size: `RenderToTextureWidth` × `…Height` (§12.3 step 1), each
    /// clamped to 1–[`MAX_RENDER_TO_TEXTURE`]; a side the element and the dictionary leave
    /// unset takes the element's layout-box side (Tessera rule), clamped the same way.
    pub target_size: (u32, u32),
    /// `RenderToTextureBackgroundColor` as `0xAARRGGBB` (§12.3 step 1).
    pub background: u32,
    /// `RenderToTextureOpacityMask` (a 0x40 texture list), when set (§12.3 step 2).
    pub mask: Option<Did>,
    /// `UICore_Viewport_AmbientLight` as `0xAARRGGBB` (§12.2), when set or defaulted (not
    /// yet applied by the client's portrait draw).
    pub ambient: Option<u32>,
    /// `UI_TargetViewport_UsePlayerAsTarget` (§12.1 step 1).
    pub use_player: bool,
    /// `UI_TargetViewport_DisableObjectCreationFromPreference` (§12.1 step 2).
    pub needs_preference: bool,
    /// `UIViewport_Type` (enum key; 0 when unset, §12.1 step 3). Read for the target's own
    /// viewport description (§12.1 step 3), which the client does not build yet.
    pub viewport_type: u32,
}

impl PortraitSpec {
    /// Reads the viewport properties of `element` (effective value, then dictionary
    /// default, `ui-widgets.md` §2.2).
    #[must_use]
    pub fn read(ui: &Ui, cv: &CombatVocab, kind: FrameKind, element: ElementKey) -> Self {
        let p = &cv.pids;
        let key = |pid: u32| Some(ui.prop(element, pid).key()).filter(|k| *k != 0);
        let colour = |pid: u32| ui.prop(element, pid).value().and_then(as_argb);
        let layout = ui.layout_box(element).unwrap_or_default();
        let side = |pid: u32, fallback: i32| {
            let prop = ui.prop(element, pid);
            let v = if prop.value().is_some() {
                prop.int()
            } else {
                fallback
            };
            u32::try_from(v.clamp(1, MAX_RENDER_TO_TEXTURE as i32)).unwrap_or(1)
        };
        Self {
            kind,
            element,
            scene: key(p.viewport_scene).map(Did),
            camera: key(p.viewport_camera),
            target_size: (
                side(p.viewport_width, layout.w()),
                side(p.viewport_height, layout.h()),
            ),
            background: colour(p.viewport_background).unwrap_or(DEFAULT_BACKGROUND),
            mask: key(p.viewport_mask).map(Did),
            ambient: colour(p.viewport_ambient),
            use_player: ui.prop(element, p.viewport_use_player).bool(),
            needs_preference: ui.prop(element, p.viewport_from_preference).bool(),
            viewport_type: ui.prop(element, p.viewport_type).key(),
        }
    }

    /// The entity the portrait shows (`ui-widgets.md` §12.1 step 1): the player's own
    /// character with `UsePlayerAsTarget`, otherwise the entity its panel shows (for P5 the
    /// selection); `None` without one.
    #[must_use]
    pub fn entity(&self, player: Option<EntityId>, shown: Option<EntityId>) -> Option<EntityId> {
        if self.use_player { player } else { shown }
    }
}

impl UnitFrame {
    /// The portrait's [`PortraitSpec`] (install look with a portrait element only).
    #[must_use]
    pub fn portrait_spec(&self, ui: &Ui, cv: &CombatVocab) -> Option<PortraitSpec> {
        let k = self.portrait()?;
        ui.exists(k)
            .then(|| PortraitSpec::read(ui, cv, self.kind(), k))
    }

    /// The client draws a scene into the portrait as external content `content`
    /// (`ui-widgets.md` §12: a nested entry of the element's surface, between its own
    /// canvas and its children), or draws none (`None`): then the element is hidden with
    /// its media and children, so the frame art shows in its box (§12.4, `combat-ui.md`
    /// §5.1). Unchanged calls change nothing in the tree.
    pub fn set_portrait_content(&self, ui: &mut Ui, content: Option<u64>) {
        let Some(k) = self.portrait() else { return };
        if !ui.exists(k) {
            return;
        }
        if self.portrait_content.get() != Some(content) {
            ui.set_external_content(k, content);
            self.portrait_content.set(Some(content));
        }
        ui.set_shown(k, content.is_some());
    }
}

/// The portrait of a frame with what it should show now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortraitView {
    /// The element and its properties.
    pub spec: PortraitSpec,
    /// The entity to draw (§12.1 step 1), `None` for none.
    pub entity: Option<EntityId>,
    /// Whether the portrait's parent is displayed now: the frame's root and every element
    /// between it and the portrait shown (a hidden panel or a hidden vital field, where
    /// P5's portrait sits, draws no portrait). The portrait's own shown flag does not count
    /// (it is the client's to set, [`UnitFrame::set_portrait_content`]).
    pub displayed: bool,
}

impl CombatUi {
    /// The local player's entity, as last given ([`CombatUi::set_player_entity`] or
    /// [`CombatUi::set_player`]).
    #[must_use]
    pub fn player_entity(&self) -> Option<EntityId> {
        self.player_entity
    }

    /// The frame of `kind`.
    #[must_use]
    pub fn frame(&self, kind: FrameKind) -> Option<&UnitFrame> {
        match kind {
            FrameKind::Player => self.player.as_ref(),
            FrameKind::Target => self.target.as_ref(),
            FrameKind::SelectionTarget => self.selection_target.as_ref(),
        }
    }

    /// The portraits of the player frame (P4) and the target frame (P5) in their install
    /// look, with the entity each should show now: the player for P4 (and for any portrait
    /// with `UsePlayerAsTarget`), the selection for P5.
    #[must_use]
    pub fn portraits(&self, ui: &Ui) -> Vec<PortraitView> {
        [FrameKind::Player, FrameKind::Target]
            .into_iter()
            .filter_map(|kind| {
                let f = self.frame(kind)?;
                let spec = f.portrait_spec(ui, &self.vocab)?;
                let shown = match kind {
                    FrameKind::Player => self.player_entity,
                    _ => f.view().map(|v| v.entity),
                };
                let displayed = match ui.parent(spec.element) {
                    Some(parent) => ui.is_displayed(parent),
                    None => ui.is_displayed(f.root()),
                };
                Some(PortraitView {
                    spec,
                    entity: spec.entity(self.player_entity, shown),
                    displayed,
                })
            })
            .collect()
    }

    /// [`UnitFrame::set_portrait_content`] on the frame of `kind`.
    pub fn set_portrait_content(&self, ui: &mut Ui, kind: FrameKind, content: Option<u64>) {
        if let Some(f) = self.frame(kind) {
            f.set_portrait_content(ui, content);
        }
    }
}

#[cfg(test)]
mod tests {
    use tessera_formats::property::{PropertyEntry, PropertyValue};
    use tessera_formats::ui_layout::{ElementDesc, UiLayout};

    use super::*;
    use crate::game::combat::{CombatLayouts, CombatRoots, FloatySettings, FrameView};
    use crate::game::find_below;
    use crate::paint::EntryTexture;
    use crate::{MemoryEnv, Vocab};

    const L: u32 = 0x2200_0001;

    fn field(id: u32, x: i32, y: i32, w: i32, h: i32) -> ElementDesc {
        let mut d = ElementDesc {
            element_id: id,
            element_type: Vocab::default().types.field,
            ..ElementDesc::default()
        };
        d.block.rect.x = x;
        d.block.rect.y = y;
        d.block.rect.width = w;
        d.block.rect.height = h;
        d.block.has_position = true;
        d
    }

    fn prop(mut d: ElementDesc, pid: u32, value: PropertyValue) -> ElementDesc {
        d.block
            .properties
            .entries
            .push(PropertyEntry { pid, value });
        d.block.properties.size.count = d.block.properties.entries.len() as u32;
        d
    }

    fn child(mut d: ElementDesc, c: ElementDesc) -> ElementDesc {
        d.children.push(c);
        d.children_size.count = d.children.len() as u32;
        d
    }

    /// P4 and P5 with their portraits; P5's portrait has the §12.1 properties, P4's only
    /// `UsePlayerAsTarget` and a width of 9999 (clamped).
    fn ui_and_combat() -> (Ui, CombatUi, ElementKey, ElementKey) {
        let cv = CombatVocab::default();
        let (i, p) = (cv.ids, cv.pids);
        let mut avatar = field(i.avatar_panel, 0, 0, 280, 300);
        avatar = child(avatar, field(i.player_name, 87, 3, 191, 36));
        let mut pp = field(i.player_portrait, 21, 21, 40, 30);
        pp = prop(pp, p.viewport_use_player, PropertyValue::Bool(true));
        pp = prop(pp, p.viewport_width, PropertyValue::Int(9999));
        avatar = child(avatar, pp);
        let mut opp = field(i.opponent_panel, 300, 0, 305, 300);
        opp = child(opp, field(i.opponent_name, 2, 3, 216, 36));
        let mut op = field(i.opponent_portrait, 220, 19, 62, 62);
        op = prop(
            op,
            p.viewport_scene,
            PropertyValue::DidRef(Did(0x2100_0004)),
        );
        op = prop(op, p.viewport_camera, PropertyValue::Enum(0x1000_001E));
        op = prop(op, p.viewport_width, PropertyValue::Int(62));
        op = prop(op, p.viewport_height, PropertyValue::Int(-3));
        op = prop(
            op,
            p.viewport_background,
            PropertyValue::Colour([0, 0, 0, 255]),
        );
        op = prop(op, p.viewport_mask, PropertyValue::DidRef(Did(0x4000_0E4C)));
        op = prop(op, p.viewport_type, PropertyValue::Enum(2));
        op = child(op, field(0x1000_0E01, 0, 0, 62, 62));
        // P5's portrait sits in a field of its own (as in its vital field).
        opp = child(opp, child(field(0x1000_0E02, 0, 0, 305, 100), op));
        let mut l = UiLayout {
            did: Did(L),
            design_width: 1024,
            design_height: 768,
            ..UiLayout::default()
        };
        l.elements = vec![avatar, opp];
        l.elements_size.count = 2;
        let mut env = MemoryEnv::default();
        env.add_layout(l);
        let mut ui = Ui::new(Box::new(env), Vocab::default(), (1024, 768));
        let a = ui.open_panel(Did(L), i.avatar_panel).expect("P4");
        let o = ui.open_panel(Did(L), i.opponent_panel).expect("P5");
        let combat = CombatUi::new(
            &mut ui,
            cv,
            CombatLayouts::default(),
            CombatRoots {
                avatar_panel: Some(a),
                opponent_panel: Some(o),
                ..CombatRoots::default()
            },
            FloatySettings::default(),
        );
        (ui, combat, a, o)
    }

    fn paint(ui: &mut Ui) -> crate::UiFrame {
        let mut budget = crate::paint::Unlimited;
        for t in 0..3 {
            ui.paint(std::time::Duration::from_millis(16 * t), &mut budget);
        }
        ui.paint(std::time::Duration::from_millis(64), &mut budget)
    }

    fn view(entity: EntityId) -> FrameView {
        FrameView {
            entity,
            name: "x".into(),
            has_vitals: true,
            ..FrameView::default()
        }
    }

    #[test]
    fn specs_read_the_viewport_properties() {
        let (ui, combat, _, _) = ui_and_combat();
        let v = combat.portraits(&ui);
        assert_eq!(v.len(), 2);
        let (p4, p5) = (v[0].spec, v[1].spec);
        assert_eq!(p4.kind, FrameKind::Player);
        assert!(p4.use_player);
        // 9999 clamps to 256; the unset height takes the layout box (30).
        assert_eq!(p4.target_size, (256, 30));
        assert_eq!(p4.background, DEFAULT_BACKGROUND);
        assert_eq!((p4.scene, p4.mask, p4.camera), (None, None, None));
        assert_eq!(p5.kind, FrameKind::Target);
        assert_eq!(p5.scene, Some(Did(0x2100_0004)));
        assert_eq!(p5.camera, Some(0x1000_001E));
        // −3 clamps to 1.
        assert_eq!(p5.target_size, (62, 1));
        assert_eq!(p5.background, 0xFF00_0000);
        assert_eq!(p5.mask, Some(Did(0x4000_0E4C)));
        assert_eq!(p5.viewport_type, 2);
        assert!(!p5.use_player);
    }

    #[test]
    fn the_portraits_show_the_player_and_the_selection() {
        let (mut ui, mut combat, _, _) = ui_and_combat();
        let v = combat.portraits(&ui);
        assert_eq!((v[0].entity, v[1].entity), (None, None));
        combat.set_player(&mut ui, &view(7));
        combat.set_target(&mut ui, Some(&view(9)), None);
        let v = combat.portraits(&ui);
        assert_eq!((v[0].entity, v[1].entity), (Some(7), Some(9)));
        assert!(v[0].displayed && v[1].displayed);
        // Selecting oneself: P5 shows the player.
        combat.set_target(&mut ui, Some(&view(7)), None);
        assert_eq!(combat.portraits(&ui)[1].entity, Some(7));
        combat.set_target(&mut ui, None, None);
        assert_eq!(combat.portraits(&ui)[1].entity, None);
    }

    #[test]
    fn a_hidden_parent_hides_the_portrait_view_but_its_own_flag_does_not() {
        let (mut ui, mut combat, _, o) = ui_and_combat();
        combat.set_target(&mut ui, Some(&view(9)), None);
        let cv = CombatVocab::default();
        let portrait = find_below(&ui, o, cv.ids.opponent_portrait).expect("portrait");
        // Hidden by §12.4 itself (no content yet): still displayed for the client.
        assert!(!ui.is_displayed(portrait));
        assert!(combat.portraits(&ui)[1].displayed);
        // An element between the root and the portrait hidden (a vital field): not.
        let wrapper = ui.parent(portrait).expect("parent");
        assert_ne!(wrapper, o);
        ui.set_shown(wrapper, false);
        assert!(ui.is_displayed(o), "the root stays displayed");
        assert!(!combat.portraits(&ui)[1].displayed);
        ui.set_shown(wrapper, true);
        assert!(combat.portraits(&ui)[1].displayed);
    }

    #[test]
    fn external_content_shows_the_portrait_and_none_hides_it() {
        let (mut ui, mut combat, _, o) = ui_and_combat();
        combat.set_target(&mut ui, Some(&view(9)), None);
        let cv = CombatVocab::default();
        let portrait = find_below(&ui, o, cv.ids.opponent_portrait).expect("portrait");
        let image = find_below(&ui, portrait, 0x1000_0E01).expect("child");
        assert!(!ui.is_displayed(portrait), "§12.4: hidden without a scene");
        combat.set_portrait_content(&mut ui, FrameKind::Target, Some(0x0052_0001));
        assert!(ui.is_displayed(portrait) && ui.is_displayed(image));
        let frame = paint(&mut ui);
        assert!(
            frame
                .composition
                .entries
                .iter()
                .any(|e| e.texture == EntryTexture::External(0x0052_0001)),
            "the scene is an entry of the composition list"
        );
        // The same content again changes nothing; none hides the element and its child.
        combat.set_portrait_content(&mut ui, FrameKind::Target, Some(0x0052_0001));
        combat.set_portrait_content(&mut ui, FrameKind::Target, None);
        assert!(!ui.is_visible(portrait) && !ui.is_displayed(image));
        let frame = paint(&mut ui);
        assert!(
            !frame
                .composition
                .entries
                .iter()
                .any(|e| matches!(e.texture, EntryTexture::External(_))),
            "no external entry without a scene"
        );
    }
}
