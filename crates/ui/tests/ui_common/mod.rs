// SPDX-License-Identifier: GPL-3.0-or-later
//! Builders for synthetic layouts and UIs (test side only). All values are invented.
#![allow(dead_code)]

use std::sync::Arc;

use tessera_formats::did::Did;
use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyStruct, PropertyValue};
use tessera_formats::ui_layout::{DrawMode, EdgeMode, StateFlags};
use tessera_formats::ui_layout::{ElementDesc, MediaItem, StateBlock, UiLayout};
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::{Command, ImageRef, MemoryEnv, Ui, Vocab};

pub fn v() -> Vocab {
    Vocab::default()
}

/// A typed element at (x, y) of size w × h.
pub fn typed(id: u32, ty: u32, x: i32, y: i32, w: i32, h: i32) -> ElementDesc {
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
    d
}

/// A based element.
pub fn based(id: u32, base: u32, layout: u32, x: i32, y: i32, w: i32, h: i32) -> ElementDesc {
    let mut d = typed(id, 0, x, y, w, h);
    d.base_element = base;
    d.base_layout = Did(layout);
    d
}

pub fn field(id: u32, x: i32, y: i32, w: i32, h: i32) -> ElementDesc {
    typed(id, v().types.field, x, y, w, h)
}

pub fn edges(mut d: ElementDesc, e: [u32; 4]) -> ElementDesc {
    d.edges.left = EdgeMode(e[0]);
    d.edges.top = EdgeMode(e[1]);
    d.edges.right = EdgeMode(e[2]);
    d.edges.bottom = EdgeMode(e[3]);
    d
}

pub fn z(mut d: ElementDesc, z: i32, read_order: u32) -> ElementDesc {
    d.block.z_level = z;
    d.read_order = read_order;
    d
}

pub fn flags(mut d: ElementDesc, f: u8) -> ElementDesc {
    d.block.flags = StateFlags(f);
    d
}

pub fn prop(mut d: ElementDesc, pid: u32, value: PropertyValue) -> ElementDesc {
    d.block.properties.entries.retain(|e| e.pid != pid);
    d.block
        .properties
        .entries
        .push(PropertyEntry { pid, value });
    d.block.properties.size.count = d.block.properties.entries.len() as u32;
    d
}

pub fn media(mut d: ElementDesc, m: MediaItem) -> ElementDesc {
    d.block.media.push(m);
    d
}

pub fn state(
    mut d: ElementDesc,
    id: u32,
    flags: u8,
    props: Vec<(u32, PropertyValue)>,
    media: Vec<MediaItem>,
) -> ElementDesc {
    let mut s = StateBlock {
        state_id: id,
        flags: StateFlags(flags),
        media,
        ..StateBlock::default()
    };
    for (pid, value) in props {
        s.properties.entries.push(PropertyEntry { pid, value });
    }
    s.properties.size.count = s.properties.entries.len() as u32;
    d.states.push(s);
    d.states_size.count = d.states.len() as u32;
    d
}

pub fn default_state(mut d: ElementDesc, s: u32) -> ElementDesc {
    d.default_state = s;
    d
}

pub fn child(mut d: ElementDesc, c: ElementDesc) -> ElementDesc {
    d.children.push(c);
    d.children_size.count = d.children.len() as u32;
    d
}

pub fn image_item(did: u32, mode: u32) -> MediaItem {
    MediaItem::Image {
        did: Did(did),
        draw_mode: DrawMode(mode),
        path: String::new(),
    }
}

pub fn layout(did: u32, w: u32, h: u32, elements: Vec<ElementDesc>) -> UiLayout {
    let mut l = UiLayout {
        did: Did(did),
        design_width: w,
        design_height: h,
        ..UiLayout::default()
    };
    l.elements_size.count = elements.len() as u32;
    l.elements = elements;
    l
}

pub fn env_with(layouts: Vec<UiLayout>) -> MemoryEnv {
    let mut env = MemoryEnv::default();
    for l in layouts {
        env.add_layout(l);
    }
    env
}

pub fn ui_with(env: MemoryEnv, w: i32, h: i32) -> Ui {
    Ui::new(Box::new(env), Vocab::default(), (w, h))
}

/// A UI with one layout holding `elements`, design size `dw` × `dh`, on a `w` × `h` screen.
pub fn ui_one(did: u32, dw: u32, dh: u32, elements: Vec<ElementDesc>, w: i32, h: i32) -> Ui {
    ui_with(env_with(vec![layout(did, dw, dh, elements)]), w, h)
}

/// A solid image.
pub fn solid(w: u32, h: u32, argb: u32) -> Arc<UiImage> {
    Arc::new(
        UiImage::new(
            w,
            h,
            w,
            h,
            PixelLayout::A8R8G8B8,
            vec![argb; (w * h) as usize],
        )
        .unwrap(),
    )
}

pub fn add_image(env: &mut MemoryEnv, did: u32, img: Arc<UiImage>) {
    env.images.insert(ImageRef::Did(did), img);
}

pub fn cmd(name: &str) -> Command {
    Command::named(name)
}

pub fn b(v: bool) -> PropertyValue {
    PropertyValue::Bool(v)
}

pub fn f(v: f32) -> PropertyValue {
    PropertyValue::Float(v)
}

pub fn int(v: i32) -> PropertyValue {
    PropertyValue::Int(v)
}

pub fn en(v: u32) -> PropertyValue {
    PropertyValue::Enum(v)
}

pub fn did(v: u32) -> PropertyValue {
    PropertyValue::DidRef(Did(v))
}

pub fn strukt(fields: Vec<(u32, PropertyValue)>) -> PropertyValue {
    let mut s = PropertyStruct::default();
    for (pid, value) in fields {
        s.fields.push(PropertyEntry { pid, value });
    }
    s.size.count = s.fields.len() as u32;
    PropertyValue::Struct(s)
}

pub fn array(pid: u32, values: Vec<PropertyValue>) -> PropertyValue {
    let mut a = PropertyArray::default();
    for value in values {
        a.elements.push(PropertyEntry { pid, value });
    }
    a.count = a.elements.len() as u32;
    PropertyValue::Array(a)
}
