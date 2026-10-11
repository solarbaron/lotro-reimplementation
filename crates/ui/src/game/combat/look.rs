// SPDX-License-Identifier: GPL-3.0-or-later
//! The fallback looks of the combat widgets (`combat-ui.md` §13, TR41): what each element
//! draws when the install's element (or font, image, string) is missing, so that a
//! `--no-data` run renders deterministically.
//!
//! The looks are made of code-made elements: filled rectangles (a Field with a colour
//! overlay, `ui-layout.md` §9.6 item 2), 1 px borders made of four such rectangles, text
//! elements styled through [`LookPids`], and two procedural images that the UI generates
//! itself ([`procedural_image`]): the selection indicator's triangle and the cooldown
//! sector. Geometry is given in pixels of the 1024 × 768 reference; [`Hud`] places the
//! panels in a root scaled uniformly to the screen height and centred horizontally (§13).

use std::sync::Arc;

use tessera_formats::property::PropertyValue;
use tessera_formats::ui_layout::{DrawMode, ElementDesc, MediaItem};
use tessera_material::ui_image::{PixelLayout, UiImage};

use crate::env::ImageRef;
use crate::geom::IBox;
use crate::tree::{Element, ElementKey};
use crate::ui::{MAX_ELEMENTS, Ui, object_mode};
use crate::widgets::WidgetClass;

use super::vocab::LookPids;

/// Reference width of the fallback geometry (§13).
pub const REF_W: i32 = 1024;
/// Reference height of the fallback geometry (§13).
pub const REF_H: i32 = 768;

/// Element ID of code-made fallback elements (Tessera's code range; game-code handlers of
/// clickable parts use [`next_look_id`] instead).
pub const LOOK_ELEMENT_ID: u32 = 0x7EFF_FE00;

/// Draw mode of the fills: 4, Alphablend (the compositor's Over).
pub const FILL_DRAW_MODE: u32 = 4;

/// Prefix of the procedural images ([`procedural_image`]).
pub const PROCEDURAL_PREFIX: &str = "tessera:look/";

/// Largest side of a procedural image (Tessera cap, hostile paths).
pub const MAX_PROCEDURAL_SIDE: u32 = 256;

/// Frames of the cooldown sweep (§9.3: 60 frames, indices 0–59).
pub const SWEEP_FRAMES: i32 = 60;

static NEXT_LOOK_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// A fresh element ID for a clickable fallback part, in 0x7EF0_0000–0x7EFE_FFFF (Tessera's
/// code range, below the layer fields of [`crate::game::ICON_LAYER_ID`]).
#[must_use]
pub fn next_look_id() -> u32 {
    let n = NEXT_LOOK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    0x7EF0_0000 + (n % 0x000F_0000)
}

/// A code-made element of `class` at `bx` inside `parent` (element ID `id`), not
/// mouse-visible unless a handler is registered for its ID.
pub(crate) fn element(
    ui: &mut Ui,
    parent: ElementKey,
    class: WidgetClass,
    bx: IBox,
    id: u32,
) -> Option<ElementKey> {
    if ui.element_count() >= MAX_ELEMENTS {
        return None;
    }
    let mut d = ElementDesc {
        element_id: id,
        ..ElementDesc::default()
    };
    d.block.rect.x = bx.l;
    d.block.rect.y = bx.t;
    d.block.rect.width = bx.w().max(0);
    d.block.rect.height = bx.h().max(0);
    d.block.has_position = true;
    super::super::examine::code_element(ui, parent, class, d)
}

/// Sets (or clears) an element's colour fill (`0xAARRGGBB`).
pub fn set_fill(ui: &mut Ui, k: ElementKey, argb: Option<u32>) {
    use tessera_formats::did::Did;
    let items = match argb {
        Some(c) => vec![MediaItem::ColouredImage {
            did: Did(0),
            draw_mode: DrawMode(FILL_DRAW_MODE),
            colour: tessera_formats::ui_layout::Argb(c),
        }],
        None => vec![MediaItem::ColouredImage {
            did: Did(0),
            draw_mode: DrawMode(8),
            colour: tessera_formats::ui_layout::Argb(0),
        }],
    };
    ui.play_media(k, items);
}

/// A filled rectangle.
pub fn rect(ui: &mut Ui, parent: ElementKey, bx: IBox, argb: u32) -> Option<ElementKey> {
    let k = element(ui, parent, WidgetClass::Field, bx, LOOK_ELEMENT_ID)?;
    set_fill(ui, k, Some(argb));
    Some(k)
}

/// A "box" of §13: a container at `bx` holding a fill and a 1 px border (four lines drawn
/// over the fill). Returns the container; `fill` or `border` `None` leaves that part out.
pub fn boxed(
    ui: &mut Ui,
    parent: ElementKey,
    bx: IBox,
    fill: Option<u32>,
    border: Option<u32>,
) -> Option<ElementKey> {
    let c = element(ui, parent, WidgetClass::Field, bx, LOOK_ELEMENT_ID)?;
    let (w, h) = (bx.w(), bx.h());
    if let Some(f) = fill {
        rect(ui, c, IBox::from_xywh(0, 0, w, h), f);
    }
    if let Some(b) = border {
        border_lines(ui, c, w, h, 1, b);
    }
    Some(c)
}

/// Four border lines of thickness `t` inside a `w` × `h` box.
pub fn border_lines(ui: &mut Ui, parent: ElementKey, w: i32, h: i32, t: i32, argb: u32) {
    let t = t.max(1);
    rect(ui, parent, IBox::from_xywh(0, 0, w, t), argb);
    rect(ui, parent, IBox::from_xywh(0, h - t, w, t), argb);
    rect(
        ui,
        parent,
        IBox::from_xywh(0, t, t, (h - 2 * t).max(0)),
        argb,
    );
    rect(
        ui,
        parent,
        IBox::from_xywh(w - t, t, t, (h - 2 * t).max(0)),
        argb,
    );
}

/// Horizontal alignment of a fallback text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Left.
    Left,
    /// Centred.
    Centre,
}

/// The style of a fallback text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextStyle {
    /// Pixel size of the bundled font.
    pub px: i32,
    /// Colour `0xAARRGGBB`.
    pub colour: u32,
    /// Alignment (vertically centred in the box).
    pub align: Align,
    /// An outline and its colour.
    pub outline: Option<u32>,
}

impl TextStyle {
    /// A left-aligned style without outline.
    #[must_use]
    pub fn new(px: i32, colour: u32) -> Self {
        Self {
            px,
            colour,
            align: Align::Left,
            outline: None,
        }
    }

    /// Centred.
    #[must_use]
    pub fn centred(mut self) -> Self {
        self.align = Align::Centre;
        self
    }

    /// With an outline.
    #[must_use]
    pub fn outlined(mut self, colour: u32) -> Self {
        self.outline = Some(colour);
        self
    }
}

/// A one-line text element at `bx` with `style` (§13: the bundled UI font at the given
/// pixel size).
pub fn text(
    ui: &mut Ui,
    pids: &LookPids,
    parent: ElementKey,
    bx: IBox,
    content: &str,
    style: TextStyle,
) -> Option<ElementKey> {
    let k = element(ui, parent, WidgetClass::Text, bx, LOOK_ELEMENT_ID)?;
    style_text(ui, pids, k, style);
    ui.set_text(k, Some(super::data::cap_label(content)));
    Some(k)
}

/// Applies `style` to a text element.
pub fn style_text(ui: &mut Ui, pids: &LookPids, k: ElementKey, style: TextStyle) {
    use crate::text::{JUSTIFY_CENTER, JUSTIFY_LEFT};
    let colour = |c: u32| PropertyValue::Colour(c.to_le_bytes());
    ui.set_property(k, pids.font_color, colour(style.colour));
    ui.set_property(k, pids.text_px, PropertyValue::Int(style.px));
    ui.set_property(k, pids.one_line, PropertyValue::Bool(true));
    let h = match style.align {
        Align::Left => JUSTIFY_LEFT,
        Align::Centre => JUSTIFY_CENTER,
    };
    ui.set_property(k, pids.horizontal, PropertyValue::Enum(h));
    ui.set_property(k, pids.vertical, PropertyValue::Enum(JUSTIFY_CENTER));
    ui.set_property(
        k,
        pids.outline,
        PropertyValue::Bool(style.outline.is_some()),
    );
    if let Some(o) = style.outline {
        ui.set_property(k, pids.outline_color, colour(o));
    }
    if let Some(e) = ui.el_mut(k) {
        e.text.dirty = true;
    }
    ui.note_paint_dirty(k);
}

/// A meter bar of §13: a back fill and a fill rectangle that covers the fraction *p* of
/// the width, from the left or from the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bar {
    /// The back (container).
    pub back: ElementKey,
    /// The fill.
    pub fill: ElementKey,
    /// Size.
    pub size: (i32, i32),
    /// Fills from the right edge.
    pub from_right: bool,
}

impl Bar {
    /// Creates the bar at `bx`.
    pub fn new(
        ui: &mut Ui,
        parent: ElementKey,
        bx: IBox,
        back: u32,
        fill: u32,
        from_right: bool,
    ) -> Option<Self> {
        let b = rect(ui, parent, bx, back)?;
        let f = rect(ui, b, IBox::from_xywh(0, 0, 0, bx.h()), fill)?;
        Some(Self {
            back: b,
            fill: f,
            size: (bx.w(), bx.h()),
            from_right,
        })
    }

    /// The fill width for *p* (clamped to [0, 1]; truncated).
    #[must_use]
    pub fn fill_width(&self, p: f32) -> i32 {
        let p = if p.is_finite() {
            p.clamp(0.0, 1.0)
        } else {
            0.0
        };
        (self.size.0 as f32 * p) as i32
    }

    /// Sets the position *p*.
    pub fn set(&self, ui: &mut Ui, p: f32) {
        let w = self.fill_width(p);
        let x = if self.from_right { self.size.0 - w } else { 0 };
        ui.set_layout_box(self.fill, IBox::from_xywh(x, 0, w, self.size.1));
    }

    /// Sets the fill colour.
    pub fn set_colour(&self, ui: &mut Ui, argb: u32) {
        set_fill(ui, self.fill, Some(argb));
    }
}

/// The uniform scale and horizontal offset of the reference geometry on a screen (§13:
/// scaled to the screen height, centred horizontally).
#[must_use]
pub fn hud_transform(screen: (i32, i32)) -> (f32, i32) {
    let h = screen.1.max(1) as f32;
    let s = h / REF_H as f32;
    let w = REF_W as f32 * s;
    let x = ((screen.0 as f32 - w) / 2.0) as i32;
    (s, x)
}

/// The root of the fallback panels: a code-made root panel of the reference size at the
/// screen, scaled by [`hud_transform`]. Created on first use; [`Hud::relayout`] follows
/// screen size changes.
#[derive(Debug, Clone, Default)]
pub struct Hud {
    root: Option<ElementKey>,
}

impl Hud {
    /// The root, created when missing (or destroyed).
    pub fn root(&mut self, ui: &mut Ui) -> Option<ElementKey> {
        if let Some(r) = self.root.filter(|r| ui.exists(*r)) {
            return Some(r);
        }
        let r = screen_root(ui, IBox::from_xywh(0, 0, REF_W, REF_H), 0)?;
        self.root = Some(r);
        self.relayout(ui);
        Some(r)
    }

    /// The root if it exists.
    #[must_use]
    pub fn existing(&self) -> Option<ElementKey> {
        self.root
    }

    /// Places and scales the root for the current screen size.
    pub fn relayout(&mut self, ui: &mut Ui) {
        let Some(r) = self.root.filter(|r| ui.exists(*r)) else {
            return;
        };
        let (s, x) = hud_transform(ui.screen_size());
        ui.set_layout_box(r, IBox::from_xywh(x, 0, REF_W, REF_H));
        let scale = if (s - 1.0).abs() < f32::EPSILON {
            None
        } else {
            Some([s, s, 1.0])
        };
        ui.set_node_scale(r, scale);
    }
}

/// A code-made root panel at `bx` attached to the screen root, with its own surface,
/// not mouse-visible; `z` orders it among the root panels.
pub(crate) fn screen_root(ui: &mut Ui, bx: IBox, z: i32) -> Option<ElementKey> {
    if ui.element_count() >= MAX_ELEMENTS {
        return None;
    }
    let mut d = ElementDesc {
        element_id: LOOK_ELEMENT_ID,
        ..ElementDesc::default()
    };
    d.block.rect.x = bx.l;
    d.block.rect.y = bx.t;
    d.block.rect.width = bx.w();
    d.block.rect.height = bx.h();
    d.block.has_position = true;
    let mut e = Element::new(LOOK_ELEMENT_ID, WidgetClass::Field, Arc::new(d));
    e.effective = Ui::compute_effective(&e);
    e.created = true;
    e.root_panel = true;
    e.screen_level = true;
    e.z = z;
    let k = ui.insert_element(e);
    let screen = ui.screen_root();
    if ui.attach_raw(k, screen).is_err() {
        ui.destroy(k);
        return None;
    }
    let om = ui.vocab().pids.object_mode;
    ui.set_property(k, om, PropertyValue::Enum(object_mode::CANVAS_RESIZE));
    Some(k)
}

/// Gives an element its own surface (for its own opacity and scale).
pub(crate) fn own_surface(ui: &mut Ui, k: ElementKey) {
    let om = ui.vocab().pids.object_mode;
    ui.set_property(k, om, PropertyValue::Enum(object_mode::CANVAS_RESIZE));
}

/// Sets an element's image from a procedural reference unless it already shows it (so a
/// per-frame call does not repaint).
pub fn set_procedural(ui: &mut Ui, k: ElementKey, r: &ImageRef) {
    if ui.base_image(k).is_some_and(|(cur, _)| &cur == r) {
        return;
    }
    crate::game::set_image_ref(ui, k, r, FILL_DRAW_MODE);
}

/// Sets an element's opacity (`UICore_Element_ObjectOpacity`, clamped to [0, 1]).
pub fn set_opacity(ui: &mut Ui, k: ElementKey, o: f32) {
    let o = if o.is_finite() {
        o.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let pid = ui.vocab().pids.object_opacity;
    ui.set_property(k, pid, PropertyValue::Float(o));
}

/// The procedural image reference of a filled triangle of `w` × `h` pixels in `argb`
/// pointing in direction `deg` (0 = down, clockwise, quantised to 15°).
#[must_use]
pub fn triangle_ref(argb: u32, w: u32, h: u32, deg: i32) -> ImageRef {
    let q = (deg.rem_euclid(360) + 7) / 15 * 15 % 360;
    ImageRef::File(format!("{PROCEDURAL_PREFIX}tri/{argb:08x}/{w}x{h}/{q}"))
}

/// The procedural image reference of the cooldown sweep (§13.7): a `size` × `size` image
/// whose pixels in the clockwise sector from 12 o'clock of angle 360° × `frame` ÷ 59 are
/// `argb`, the rest transparent.
#[must_use]
pub fn sweep_ref(argb: u32, size: u32, frame: i32) -> ImageRef {
    let f = frame.clamp(0, SWEEP_FRAMES - 1);
    ImageRef::File(format!("{PROCEDURAL_PREFIX}sweep/{argb:08x}/{size}/{f}"))
}

/// Generates a procedural image from its path ([`triangle_ref`], [`sweep_ref`]); `None`
/// for any other path or malformed parameters.
#[must_use]
pub fn procedural_image(path: &str) -> Option<UiImage> {
    let rest = path.strip_prefix(PROCEDURAL_PREFIX)?;
    let mut parts = rest.split('/');
    let kind = parts.next()?;
    let argb = u32::from_str_radix(parts.next()?, 16).ok()?;
    match kind {
        "tri" => {
            let (w, h) = parts.next()?.split_once('x')?;
            let w: u32 = w.parse().ok()?;
            let h: u32 = h.parse().ok()?;
            let deg: i32 = parts.next()?.parse().ok()?;
            if parts.next().is_some() {
                return None;
            }
            triangle_image(argb, w, h, deg)
        }
        "sweep" => {
            let size: u32 = parts.next()?.parse().ok()?;
            let frame: i32 = parts.next()?.parse().ok()?;
            if parts.next().is_some() {
                return None;
            }
            sweep_image(argb, size, frame)
        }
        _ => None,
    }
}

fn side_ok(v: u32) -> bool {
    (1..=MAX_PROCEDURAL_SIDE).contains(&v)
}

/// A triangle filling the `w` × `h` box pointing down (apex at the bottom centre), rotated
/// about the centre by `deg` (point sampling at pixel centres).
fn triangle_image(argb: u32, w: u32, h: u32, deg: i32) -> Option<UiImage> {
    if !side_ok(w) || !side_ok(h) || !(0..360).contains(&deg) {
        return None;
    }
    let (wf, hf) = (w as f32, h as f32);
    let (cx, cy) = (wf / 2.0, hf / 2.0);
    // Corners of the downward triangle relative to the centre.
    let v = [(-cx, -cy), (cx, -cy), (0.0, cy)];
    let a = (deg as f32).to_radians();
    let (s, c) = a.sin_cos();
    let rot: Vec<(f32, f32)> = v
        .iter()
        .map(|(x, y)| (x * c - y * s + cx, x * s + y * c + cy))
        .collect();
    let edge = |p: (f32, f32), q: (f32, f32), x: f32, y: f32| {
        (q.0 - p.0) * (y - p.1) - (q.1 - p.1) * (x - p.0)
    };
    let mut px = vec![0u32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let e0 = edge(rot[0], rot[1], fx, fy);
            let e1 = edge(rot[1], rot[2], fx, fy);
            let e2 = edge(rot[2], rot[0], fx, fy);
            let inside =
                (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0);
            if inside {
                px[(y * w + x) as usize] = argb;
            }
        }
    }
    UiImage::new(w, h, w, h, PixelLayout::A8R8G8B8, px)
}

/// The angle of the sweep for a frame index *f*: 360° × *f* ÷ 59.
#[must_use]
pub fn sweep_degrees(frame: i32) -> f32 {
    360.0 * frame.clamp(0, SWEEP_FRAMES - 1) as f32 / (SWEEP_FRAMES - 1) as f32
}

fn sweep_image(argb: u32, size: u32, frame: i32) -> Option<UiImage> {
    if !side_ok(size) || !(0..SWEEP_FRAMES).contains(&frame) {
        return None;
    }
    let limit = sweep_degrees(frame);
    let c = size as f32 / 2.0;
    let mut px = vec![0u32; (size * size) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - c;
            let dy = y as f32 + 0.5 - c;
            // Clockwise from 12 o'clock, screen y down.
            let mut ang = dx.atan2(-dy).to_degrees();
            if ang < 0.0 {
                ang += 360.0;
            }
            if frame == SWEEP_FRAMES - 1 || ang < limit {
                px[(y * size + x) as usize] = argb;
            }
        }
    }
    UiImage::new(size, size, size, size, PixelLayout::A8R8G8B8, px)
}

/// Scales a reference box by the uniform factor `s` (truncating).
#[must_use]
pub fn scaled(bx: IBox, s: f32) -> IBox {
    let f = |v: i32| (v as f32 * s) as i32;
    IBox::from_xywh(f(bx.l), f(bx.t), f(bx.w()), f(bx.h()))
}

/// Most procedural images the UI keeps (Tessera cap; the cache is emptied when full).
pub const MAX_PROCEDURAL_CACHED: usize = 512;

impl Ui {
    /// The procedural image of `path` ([`procedural_image`]), generated once and cached.
    pub(crate) fn procedural_image(&mut self, path: &str) -> Option<Arc<UiImage>> {
        if let Some(i) = self.procedural.get(path) {
            return Some(i.clone());
        }
        let img = Arc::new(procedural_image(path)?);
        if self.procedural.len() >= MAX_PROCEDURAL_CACHED {
            self.procedural.clear();
        }
        self.procedural.insert(path.to_string(), img.clone());
        Some(img)
    }
}
