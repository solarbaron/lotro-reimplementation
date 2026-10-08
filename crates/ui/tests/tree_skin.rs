// SPDX-License-Identifier: GPL-3.0-or-later
//! The element tree's two skin hooks (`ui-layout.md` §9.8, `skin-definition.md` §6.3, §6.6,
//! §8.2–§8.4): geometry overrides on the per-build resolved copy, and art substitution by the
//! base-image setter with the refresh at a switch. Synthetic skins in temporary folders.

mod ui_common;

use std::sync::Arc;

use tessera_formats::did::Did;
use tessera_ui::ImageRef;
use tessera_ui::geom::IBox;
use tessera_ui::skin::{
    BaseTable, LabelIndex, SkinEnvironment, SkinSelection, SkinSet, SkinTables,
};
use ui_common::*;

const LAYOUT: u32 = 0x22FF_6000;
const PANEL: u32 = 0x1000_0A01;
const CHILD: u32 = 0x1000_0A02;
const ART: u32 = 0x5000_0001;
const DID_ART: u32 = 0x4100_0A01;

fn tables() -> SkinTables {
    let mut arts = LabelIndex::new();
    arts.insert(ART, "Panel_Background");
    let mut els = LabelIndex::new();
    els.insert(PANEL, "SkinPanel");
    els.insert(CHILD, "SkinPanel_Child");
    let mut base = BaseTable::new();
    base.insert(DID_ART, ART);
    SkinTables {
        art_assets: arts,
        elements: els,
        base,
        ..SkinTables::default()
    }
}

#[test]
fn geometry_and_art_follow_the_active_skin() {
    let tmp = tempfile::tempdir().unwrap();
    let def = br#"<opt><SkinName Name="Blue"/><Mapping ArtAssetID="Panel_Background" FileName="bg.tga"/><PanelFile ID="p"><Element ID="SkinPanel" X="7" Y="8" Width="90" Height="60"><Element ID="SkinPanel_Child" X="3" Y="4" Width="5" Height="6"/></Element></PanelFile></opt>"#;
    std::fs::create_dir_all(tmp.path().join("S")).unwrap();
    std::fs::write(tmp.path().join("S/SkinDefinition.xml"), def).unwrap();
    let t = tables();
    let set = Arc::new(SkinSet::discover(tmp.path(), &t));
    let mut sel = SkinSelection::new(set, Arc::new(t), SkinEnvironment::default());
    sel.apply_option("blue");
    let overlay = sel.active().cloned().expect("skin active");

    let child = edges(field(CHILD, 10, 10, 20, 20), [5, 5, 5, 5]);
    let panel = media(field(PANEL, 100, 50, 300, 200), image_item(DID_ART, 4));
    let panel = edges(child_of(panel, child), [5, 5, 5, 5]);
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![panel], 1024, 768);
    // A panel built before the activation keeps its geometry and art (D20).
    let before = ui.open_panel(Did(LAYOUT), PANEL).unwrap();
    assert_eq!(
        ui.layout_box(before),
        Some(IBox::from_xywh(100, 50, 300, 200))
    );
    assert_eq!(ui.base_image(before), Some((ImageRef::Did(DID_ART), 4)));
    ui.set_skin(Some(overlay));
    // The refresh re-sets the base image from its original DID through the skin (§6.6).
    assert_eq!(
        ui.base_image(before),
        Some((
            ImageRef::Skin {
                art_asset: ART,
                did: DID_ART
            },
            4
        ))
    );
    // A panel built after the activation takes the override, with edge modes 0 (§8.4).
    let after = ui.open_panel(Did(LAYOUT), PANEL).unwrap();
    assert_eq!(ui.layout_box(after), Some(IBox::from_xywh(7, 8, 90, 60)));
    let c = ui.find_child(after, CHILD).unwrap();
    assert_eq!(ui.layout_box(c), Some(IBox::from_xywh(3, 4, 5, 6)));
    // Deactivating refreshes back to the base DID; geometry is not rebuilt.
    ui.set_skin(None);
    assert_eq!(ui.base_image(after), Some((ImageRef::Did(DID_ART), 4)));
    assert_eq!(ui.layout_box(after), Some(IBox::from_xywh(7, 8, 90, 60)));
}

fn child_of(
    p: tessera_formats::ui_layout::ElementDesc,
    c: tessera_formats::ui_layout::ElementDesc,
) -> tessera_formats::ui_layout::ElementDesc {
    child(p, z(c, 0, 1))
}
