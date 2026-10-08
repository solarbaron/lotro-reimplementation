// SPDX-License-Identifier: GPL-3.0-or-later
//! Panel geometry overrides (`skin-definition.md` §8; D19, D20, D28): the worked examples
//! §12.3, §12.5 and §12.8 and the invariants SP14–SP16, SP22, SP23, on synthetic layout
//! descriptors.

mod skin_common;

use std::sync::Arc;

use skin_common::{resolver, write};
use tessera_formats::ui_layout::{EdgeMode, Edges, ElementDesc, Rect};
use tessera_formats::{PropertyEntry, PropertyValue};
use tessera_ui::skin::{
    GeometryPids, ScaleChange, SkinEnvironment, SkinSelection, SkinSet, SkinTables, apply_override,
    parse_definition, scale_about_origin,
};

const PANEL_A: u32 = 0x1000_0100;
const TITLE: u32 = 0x1000_0101;
const BODY: u32 = 0x1000_0102;
const FRAME: u32 = 0x1000_0200;
const CLOSE: u32 = 0x1000_0201;
const PANEL_B: u32 = 0x1000_0300;

const PIDS: GeometryPids = GeometryPids {
    cannot_be_moved: Some(0x728),
    cannot_be_resized: Some(0x727),
    cannot_be_detached: Some(0x729),
    scaler_origin_x: Some(0x1000_1840),
    scaler_origin_y: Some(0x1000_1841),
};

fn names() -> (tessera_ui::skin::LabelIndex, tessera_ui::skin::LabelIndex) {
    resolver(
        &[],
        &[
            ("PanelA", PANEL_A),
            ("PanelA_Title", TITLE),
            ("PanelA_Body", BODY),
            ("Frame", FRAME),
            ("Frame_Close", CLOSE),
            ("PanelB", PANEL_B),
        ],
    )
}

fn edges(l: u32, t: u32, r: u32, b: u32) -> Edges {
    Edges {
        left: EdgeMode(l),
        top: EdgeMode(t),
        right: EdgeMode(r),
        bottom: EdgeMode(b),
    }
}

fn elem(id: u32, rect: (i32, i32, i32, i32), e: Edges, has_position: bool) -> ElementDesc {
    let mut d = ElementDesc {
        element_id: id,
        edges: e,
        ..ElementDesc::default()
    };
    d.block.rect = Rect {
        x: rect.0,
        y: rect.1,
        width: rect.2,
        height: rect.3,
    };
    d.block.has_position = has_position;
    d.block.z_level = 3;
    d
}

fn protect(d: &mut ElementDesc, pid: u32) {
    d.block.properties.entries.push(PropertyEntry {
        pid,
        value: PropertyValue::Bool(true),
    });
}

fn rect_of(d: &ElementDesc) -> (i32, i32, i32, i32) {
    let r = d.block.rect;
    (r.x, r.y, r.width, r.height)
}

fn int_prop(d: &ElementDesc, pid: u32) -> Option<i32> {
    match d.block.properties.get(pid) {
        Some(PropertyValue::Int(v)) => Some(*v),
        _ => None,
    }
}

#[test]
fn worked_example_12_3() {
    let def = parse_definition(
        br#"<opt><SkinName Name="Geometry example"/><PanelFile ID="whatever">
            <Element ID="PanelA" X="10" Y="0x14" Width="320" Height="240">
              <Element ID="PanelA_Title" X="0" Y="0" Width="1" Height="1"/>
              <Element ID="PanelA_Body" X="0" Y="020" Width="999" Height="999"/>
            </Element></PanelFile></opt>"#,
        &names(),
    )
    .unwrap();
    let mut panel = elem(PANEL_A, (100, 50, 300, 200), edges(5, 5, 5, 5), true);
    panel
        .children
        .push(elem(TITLE, (0, 0, 300, 20), edges(0, 0, 0, 0), false));
    let mut body = elem(BODY, (0, 20, 300, 180), edges(4, 0, 4, 0), true);
    protect(&mut body, 0x727);
    panel.children.push(body);

    // Built as a panel root (§8.2): match with an empty path, apply to the copy.
    let ov = def.find_override(&[], PANEL_A).unwrap();
    assert!(apply_override(&mut panel, ov, &PIDS));
    assert_eq!(rect_of(&panel), (10, 20, 320, 240));
    assert_eq!(panel.edges, edges(0, 0, 0, 0));
    assert!(panel.block.has_position);
    assert_eq!(rect_of(&panel.children[0]), (0, 0, 1, 1));
    assert_eq!(panel.children[0].edges, edges(0, 0, 0, 0));
    assert!(!panel.children[0].block.has_position);
    assert_eq!(rect_of(&panel.children[1]), (0, 16, 300, 180));
    assert_eq!(panel.children[1].edges, edges(0, 0, 0, 0));
    assert!(panel.children[1].block.has_position);
}

#[test]
fn sp14_protections_are_independent() {
    let def = parse_definition(
        br#"<opt><PanelFile ID="p"><Element ID="PanelA" X="1" Y="2" Width="3" Height="4"/></PanelFile></opt>"#,
        &names(),
    )
    .unwrap();
    let ov = def.top_level(PANEL_A).unwrap();
    let base = elem(PANEL_A, (10, 20, 30, 40), edges(1, 2, 3, 4), true);
    let cases = [
        (0x728, (10, 20, 3, 4), edges(0, 0, 0, 0)),
        (0x727, (1, 2, 30, 40), edges(0, 0, 0, 0)),
        (0x729, (1, 2, 3, 4), edges(1, 2, 3, 4)),
    ];
    for (pid, rect, e) in cases {
        let mut d = base.clone();
        protect(&mut d, pid);
        apply_override(&mut d, ov, &PIDS);
        assert_eq!((rect_of(&d), d.edges), (rect, e), "{pid:#x}");
    }
    // A protection property with a non-boolean value counts as false.
    let mut d = base.clone();
    d.block.properties.entries.push(PropertyEntry {
        pid: 0x728,
        value: PropertyValue::Int(1),
    });
    apply_override(&mut d, ov, &PIDS);
    assert_eq!(rect_of(&d), (1, 2, 3, 4));
    // A different element ID: nothing happens.
    let mut other = elem(PANEL_B, (10, 20, 30, 40), edges(1, 2, 3, 4), true);
    assert!(!apply_override(&mut other, ov, &PIDS));
    assert_eq!(rect_of(&other), (10, 20, 30, 40));
}

#[test]
fn sp15_only_geometry_fields_change() {
    let def = parse_definition(
        br#"<opt><PanelFile ID="p"><Element ID="PanelA" X="1" Y="2" Width="3" Height="4" ScaleOriginY="9"/></PanelFile></opt>"#,
        &names(),
    )
    .unwrap();
    let ov = def.top_level(PANEL_A).unwrap();
    for has_position in [false, true] {
        let mut d = elem(PANEL_A, (10, 20, 30, 40), edges(1, 2, 3, 4), has_position);
        d.block.flags.0 = 0x5A;
        d.default_state = 77;
        let before = d.clone();
        apply_override(&mut d, ov, &PIDS);
        assert_eq!(d.block.has_position, has_position);
        assert_eq!(d.block.z_level, before.block.z_level);
        assert_eq!(d.block.flags, before.block.flags);
        assert_eq!(d.block.media, before.block.media);
        assert_eq!(d.default_state, 77);
        assert_eq!(d.states, before.states);
        // Both origin properties are written, a missing one as 0.
        assert_eq!(int_prop(&d, 0x1000_1840), Some(0));
        assert_eq!(int_prop(&d, 0x1000_1841), Some(9));
    }
    // Without a scaler origin nothing is written.
    let def = parse_definition(
        br#"<opt><PanelFile ID="p"><Element ID="PanelA" Width="3" Height="4"/></PanelFile></opt>"#,
        &names(),
    )
    .unwrap();
    let mut d = elem(PANEL_A, (10, 20, 30, 40), edges(1, 2, 3, 4), true);
    apply_override(&mut d, def.top_level(PANEL_A).unwrap(), &PIDS);
    assert!(d.block.properties.entries.is_empty());
}

#[test]
fn sp16_worked_example_12_5_and_d19_two_applications() {
    let def = parse_definition(
        br#"<opt><PanelFile ID="b"><Element ID="PanelB" X="40" Y="40" Width="250" Height="120">
              <Element ID="Frame_Close" X="230" Y="0" Width="20" Height="20"/>
            </Element></PanelFile></opt>"#,
        &names(),
    )
    .unwrap();
    // The loaded layout data: `Frame` typed with a protection and a child; `PanelB` based on it.
    let mut frame = elem(FRAME, (0, 0, 200, 100), edges(5, 5, 5, 5), true);
    protect(&mut frame, 0x729);
    frame
        .children
        .push(elem(CLOSE, (180, 0, 20, 20), edges(4, 5, 5, 4), true));
    let loaded_frame = frame.clone();
    // `PanelB`'s resolved per-build copy (ui-layout.md §7): its rectangle and edge modes,
    // Frame's property and the inherited child.
    let mut panel_b = elem(PANEL_B, (300, 300, 200, 100), edges(5, 5, 5, 5), true);
    panel_b.block.properties = frame.block.properties.clone();
    panel_b.children = frame.children.clone();

    let ov = def.find_override(&[], PANEL_B).unwrap();
    apply_override(&mut panel_b, ov, &PIDS);
    assert_eq!(rect_of(&panel_b), (40, 40, 250, 120));
    assert_eq!(
        panel_b.edges,
        edges(5, 5, 5, 5),
        "protection inherited from Frame"
    );
    assert_eq!(rect_of(&panel_b.children[0]), (230, 0, 20, 20));
    assert_eq!(panel_b.children[0].edges, edges(0, 0, 0, 0));

    // The child is matched again when it is built, on its own copy, with the same result.
    let mut close_copy = panel_b.children[0].clone();
    let again = def.find_override(&[PANEL_B], CLOSE).unwrap();
    apply_override(&mut close_copy, again, &PIDS);
    assert_eq!(close_copy, panel_b.children[0]);

    // The loaded data is unchanged, and a build from `Frame` itself finds no override.
    assert_eq!(frame, loaded_frame);
    assert!(def.find_override(&[], FRAME).is_none());
}

#[test]
fn sp22_geometry_reaches_only_panels_built_after_activation() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        &tmp.path().join("S/SkinDefinition.xml"),
        br#"<opt><SkinName Name="Moved"/><PanelFile ID="p"><Element ID="PanelA" X="7" Y="8" Width="90" Height="60"/></PanelFile></opt>"#,
    );
    let tables = SkinTables {
        elements: names().1,
        geometry: PIDS,
        ..SkinTables::default()
    };
    let set = Arc::new(SkinSet::discover(tmp.path(), &tables));
    let mut sel = SkinSelection::new(set, Arc::new(tables), SkinEnvironment::default());
    let layout = elem(PANEL_A, (100, 50, 300, 200), edges(5, 5, 5, 5), true);

    // Built before activation: the tree has no overlay to consult.
    let built_before = layout.clone();
    assert!(sel.active().is_none());

    let switch = sel.apply_option("moved");
    assert!(switch.previous.is_none() && switch.current.is_some());
    let overlay = sel.active().unwrap();
    // The panel built before keeps its rectangle (no rebuild, D20) ...
    assert_eq!(rect_of(&built_before), (100, 50, 300, 200));
    // ... the same panel built afterwards gets the override and edge modes 0.
    let mut built_after = layout.clone();
    assert!(overlay.apply(&mut built_after, &[]));
    assert_eq!(rect_of(&built_after), (7, 8, 90, 60));
    assert_eq!(built_after.edges, edges(0, 0, 0, 0));
    // After deactivation a new build is unmodified.
    let off = sel.deactivate();
    assert!(off.current.is_none() && off.previous.is_some());
    assert!(sel.active().is_none());
}

#[test]
fn d28_runtime_ids_never_match() {
    let def = parse_definition(
        br#"<opt><PanelFile ID="p"><Element ID="PanelA" X="5" Y="6" Width="50" Height="10"><Element ID="PanelA_Title" Width="1" Height="1"/></Element></PanelFile></opt>"#,
        &names(),
    )
    .unwrap();
    // A control's descendants are matched through a path that starts with the control's
    // run-time ID, which no skin can name.
    let runtime_id = 0x7FFF_0001;
    assert!(def.find_override(&[runtime_id], TITLE).is_none());
}

#[test]
fn sp23_worked_example_12_8() {
    let c = ScaleChange {
        position: (448, 980),
        origin: (512, 100),
        old_scale: 1.0,
        new_scale: 1.25,
        new_size: (1280, 125),
        screen: (1920, 1080),
    };
    assert_eq!(scale_about_origin(&c), (320, 955));
    assert_eq!(
        scale_about_origin(&ScaleChange {
            position: (1000, 1000),
            origin: (0, 0),
            ..c
        }),
        (640, 955)
    );
}
