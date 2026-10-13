// SPDX-License-Identifier: GPL-3.0-or-later
//! Game widget types of the native panels: drag-box frames, tutorial highlights,
//! conditional fields, pip displays and the page changer, the minimap (`Radar`), the world
//! map, quickslot bars (`game-panels.md`), and the chat log, tab list and entry line
//! (`chat-window.md`).
//!
//! In the original these are game-code classes; here they are **controllers** that sit on
//! top of the element tree. The tree creates every game type as the fallback Field (or the
//! engine class [`GameVocab::register_classes`] gives it); [`GamePanels::attach`] then finds
//! the instances below a panel root and runs each type's post-creation work. The data they
//! show comes in through small traits the client implements with sandbox values
//! ([`GameData`], [`MapSource`], [`Shortcuts`], [`chat::EntryHistory`]); this module depends
//! on neither the session layer nor the world.
//!
//! How the client drives it:
//!
//! - after opening a panel: [`GamePanels::attach`];
//! - every [`crate::UiEvent`] taken from the UI goes through [`GamePanels::handle_event`]
//!   (game-code handler messages, drag begin and end), which may return [`GameAction`]s;
//! - the input action `ToggleHiddenDragBoxes`: [`GamePanels::toggle_drag_boxes`];
//! - every position update: [`GamePanels::update_radar`] and
//!   [`GamePanels::update_world_map`];
//! - opening the world map P10: [`GamePanels::attach_world_map`];
//! - player property changes: [`GamePanels::refresh_player`];
//! - a request to show or hide an element, and a property change on an element:
//!   [`GamePanels::request_shown`] and [`GamePanels::element_property_changed`] (the
//!   `HideWhenField` triggers of `game-panels.md` §6.1);
//! - when panels close: [`GamePanels::prune`];
//! - a `menu` press the UI declined, with a UI attached: [`escape::declined_escape`].
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`vocab`] | labels of game types, element IDs, states, properties | naming notes |
//! | [`dragbox`] | `HiddenDragBox` | GP §4 |
//! | [`simple`] | `TutorialHighlighter`, `PipDisplay`, the page changer, the §11 types, mood and time of day | GP §5, §7, §8.7.2–§8.7.3, §11 |
//! | [`hidewhen`] | `HideWhenField` and its hide test | GP §6 |
//! | [`map`] | `UI_Map_GameMap`, the tile range, content layer, world → pixel | GP §8.4–§8.6 |
//! | [`radar`] | the minimap | GP §8 |
//! | [`worldmap`] | the world-map tree and node choice | GP §9 |
//! | [`quickslot`] | quickslot bars | GP §10 |
//! | [`escape`] | Escape with the game UI attached | GP §16 |
//! | [`chat`] | chat windows, tabs, logs, entry line, command line | `chat-window.md` |
//! | [`charvocab`] | labels of the P1 types, element IDs, states, properties | naming notes |
//! | [`examine`] | examination tooltips of items: build, assembly, placement, pins | `item-tooltips.md` |
//! | [`items`] | item slots, bag lists, item drag and drop | `character-panels.md` §3.3, §4.1, §5, §6; `ui-behaviour.md` §7 |
//! | [`avatar`] | viewports as external-content hosts, the paperdoll's turning | `render.md` §6.10, `character-panels.md` §3.2 |
//! | [`creation`] | choice lists, steppers, sliders, swatches of the creation screens | `character-screens.md` §3–§5 |
//! | [`npcvocab`] | labels of the P3 types, element IDs, states, properties | naming notes |
//! | [`npc`] | NPC windows: dialogue, quest offer, vendor, quantity, barter, loot, money, wallet | `npc-windows.md` §5–§9 |
//! | [`quest`] | quest log, details pane, quest tracker, quest markers on the radar and world map | `quest-journal.md` §4–§8; GP §8.8, §9.3 |
//! | [`p5vocab`] | labels of the P5 types, element IDs, states, properties | naming notes |
//! | [`mapmarks`] | note types, masks, filters and their menu, icon tables, notes, feed markers, minimap blips | `map-windows.md` §6; `map-data.md` §4.8 |
//! | [`mapwindow`] | the map window: presentations, navigation, dungeon maps, controls, player marker, place lookup | `map-windows.md` §3–§5, §7 |
//! | [`mapfallback`] | the fallback map window, its elements and a fallback minimap | decision 0031; `map-windows.md` §8 OPEN |
//! | [`popup`] | Tessera's popup list for the P5 menus | `map-windows.md` §4.5, §6.9; `travel-windows.md` §6.4 |
//! | [`travel`] | the travel window, the end-travel box, the travel presentation | `travel-windows.md` §4, §5 |
//! | [`finder`] | the instance finder, its queue page and alert, the instance-travel window | `travel-windows.md` §4.11, §6, §7.2 |
//! | [`travelfallback`] | fallback travel, finder and instance-travel windows | decision 0031 |

pub mod avatar;
pub mod charvocab;
pub mod chat;
pub mod creation;
pub mod data;
pub mod dragbox;
pub mod escape;
pub mod examine;
pub mod finder;
pub mod hidewhen;
pub mod items;
pub mod map;
pub mod mapfallback;
pub mod mapmarks;
pub mod mapwindow;
pub mod npc;
pub mod npcvocab;
pub mod p5vocab;
mod panels;
pub mod popup;
pub mod quest;
pub mod quickslot;
pub mod radar;
pub mod simple;
pub mod travel;
pub mod travelfallback;
pub mod vocab;
pub mod worldmap;

pub use charvocab::CharVocab;
pub use data::{
    GameData, HideCondition, MapLookup, MapSource, NoShortcuts, SandboxData, Shortcut, Shortcuts,
};
pub use npcvocab::NpcVocab;
pub use p5vocab::P5Vocab;
pub use panels::{GameAction, GameLayouts, GamePanels};
pub use vocab::GameVocab;

use std::sync::Arc;

use tessera_formats::ui_layout::ElementDesc;

use crate::env::ImageRef;
use crate::tree::{Element, ElementKey};
use crate::ui::{MAX_ELEMENTS, MAX_TREE_DEPTH, Ui};
use crate::widgets::WidgetClass;

/// Errors of the game widgets.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GameError {
    /// An element the widget needs is missing (`game-panels.md` §8.2 step 2). The value is
    /// always an element ID.
    #[error("element {0:#010x} not found")]
    MissingElement(u32),
    /// A property that names an element the widget needs is absent or zero (the value is
    /// the property ID).
    #[error("property {0:#010x} naming a needed element is absent")]
    MissingProperty(u32),
    /// A layout the widget creates elements from is not configured ([`GameLayouts`]).
    #[error("the {0} layout is not configured")]
    NoLayout(&'static str),
    /// A world-map node index out of range.
    #[error("no map node {0}")]
    NoSuchNode(usize),
    /// A chat window index out of range.
    #[error("no chat window {0}")]
    NoSuchWindow(usize),
    /// A property value has the wrong shape.
    #[error("malformed property {0:#010x}")]
    Malformed(u32),
    /// A tree or list exceeds Tessera's cap.
    #[error("{what} exceeds the cap of {cap}")]
    TooLarge {
        /// What was too large.
        what: &'static str,
        /// The cap.
        cap: usize,
    },
    /// The UI core refused an operation.
    #[error(transparent)]
    Ui(#[from] crate::UiError),
}

/// The first element with ID `id` strictly below `k`, in pre-order depth-first order
/// (`game-panels.md` §2: "found by element ID anywhere below the widget (depth-first)").
#[must_use]
pub fn find_below(ui: &Ui, k: ElementKey, id: u32) -> Option<ElementKey> {
    let mut stack: Vec<(ElementKey, usize)> =
        ui.children(k).into_iter().rev().map(|c| (c, 1)).collect();
    let mut n = 0usize;
    while let Some((c, d)) = stack.pop() {
        n += 1;
        if n > MAX_ELEMENTS {
            return None;
        }
        if ui.element_id(c) == Some(id) {
            return Some(c);
        }
        if d < MAX_TREE_DEPTH {
            stack.extend(ui.children(c).into_iter().rev().map(|g| (g, d + 1)));
        }
    }
    None
}

/// `k` and every element below it, in pre-order depth-first order (bounded).
#[must_use]
pub fn subtree(ui: &Ui, k: ElementKey) -> Vec<ElementKey> {
    let mut out = Vec::new();
    if !ui.exists(k) {
        return out;
    }
    let mut stack = vec![(k, 0usize)];
    while let Some((c, d)) = stack.pop() {
        if out.len() >= MAX_ELEMENTS {
            break;
        }
        out.push(c);
        if d < MAX_TREE_DEPTH {
            stack.extend(ui.children(c).into_iter().rev().map(|g| (g, d + 1)));
        }
    }
    out
}

/// Whether the element's resolved states table defines state `s`.
#[must_use]
pub fn defines_state(ui: &Ui, k: ElementKey, s: u32) -> bool {
    s != 0 && ui.el(k).is_some_and(|e| e.desc.state(s).is_some())
}

/// Switches to `s` only when the element defines it (`game-panels.md` §2 "State switch").
pub fn switch_if_defined(ui: &mut Ui, k: ElementKey, s: u32) {
    ui.switch_state_if_defined(k, s);
}

/// Shows or hides an element when it exists (`game-panels.md` §2 "hidden by code").
pub fn show(ui: &mut Ui, k: Option<ElementKey>, on: bool) {
    if let Some(k) = k {
        ui.set_shown(k, on);
    }
}

/// Sets an element's image from a DID with a draw mode, replacing its media (a DID of 0
/// clears the image), as game code's image setter does.
pub fn set_image(ui: &mut Ui, k: ElementKey, did: u32, draw_mode: u32) {
    use tessera_formats::did::Did;
    use tessera_formats::ui_layout::{DrawMode, MediaItem};
    if did == 0 {
        ui.clear_base(k);
        ui.play_media(k, Vec::new());
        return;
    }
    ui.play_media(
        k,
        vec![MediaItem::Image {
            did: Did(did),
            draw_mode: DrawMode(draw_mode),
            path: String::new(),
        }],
    );
}

/// The resolved text of a StringInfo property of an element, through the client's string
/// resolver.
pub(crate) fn string_prop(ui: &mut Ui, k: ElementKey, pid: u32) -> Option<String> {
    match ui.property(k, pid).cloned() {
        Some(tessera_formats::property::PropertyValue::StringInfo(si)) => {
            ui.env.resolve_string(&si)
        }
        Some(tessera_formats::property::PropertyValue::String(s)) => Some(s),
        _ => None,
    }
}

/// The rotation held by an element's own surface (decision 0015 §2.3), as game code set it
/// (for example the minimap marker, `game-panels.md` §8.6.3); zero for an element without
/// one.
#[must_use]
pub fn held_rotation(ui: &Ui, k: ElementKey) -> [f32; 3] {
    ui.el(k).map_or([0.0; 3], |e| e.rotation)
}

/// Sets an element's image from an [`ImageRef`] as [`set_image`] does: a DID (or the DID a
/// skin replaced) goes through the image setter, a file path becomes an Image item with
/// that path.
pub(crate) fn set_image_ref(ui: &mut Ui, k: ElementKey, image: &ImageRef, draw_mode: u32) {
    use tessera_formats::did::Did;
    use tessera_formats::ui_layout::{DrawMode, MediaItem};
    match image {
        ImageRef::Did(d) | ImageRef::Skin { did: d, .. } => set_image(ui, k, *d, draw_mode),
        ImageRef::File(path) => ui.play_media(
            k,
            vec![MediaItem::Image {
                did: Did(0),
                draw_mode: DrawMode(draw_mode),
                path: path.clone(),
            }],
        ),
    }
}

/// An item icon as the client gives it to the P1 widgets (`icon-layers.md` §3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IconImage {
    /// An icon the client composed (`tessera-material`'s icon composition, the exact §3.3
    /// arithmetic), named by an image reference its [`crate::UiEnv`] resolves.
    Composed(ImageRef),
    /// The layer surfaces, bottom to top (`icon-layers.md` §3.1–§3.2): the UI shows each
    /// as its own field at the icon's top-left corner, blended with the UI compositor's
    /// Alphablend (`ui-layout.md` §9.6.1). A Tessera fallback for a client that cannot
    /// name a composed icon; at most [`MAX_ICON_LAYERS`] layers are shown.
    ///
    /// Each layer field is sized to its image's own size, clipped to the icon element's
    /// box, so a smaller layer is drawn once and not tiled (§3.4: no layer is scaled, a
    /// smaller one leaves the rest untouched). While an image is not loaded its field has
    /// the icon's size; [`fit_icon_layers`] sizes the fields again once the images are
    /// loaded. Differences from the §3 composite: the layers are not clipped to the bottom
    /// layer's size when it is smaller than the icon element, a missing bottom layer still
    /// shows the others (§3.1 gives no icon), and the two blends round differently.
    Layers(Vec<ImageRef>),
}

/// Most layers an [`IconImage::Layers`] shows (Tessera cap; item stacks have five).
pub const MAX_ICON_LAYERS: usize = 16;

/// Element ID of the code-made layer fields of an [`IconImage::Layers`] icon, below the
/// icon element (Tessera's code range, see [`creation::CODE_ELEMENT_ID_BASE`]).
pub const ICON_LAYER_ID: u32 = 0x7EFF_FF01;

/// Shows `icon` on element `k` (or clears it): a composed icon becomes `k`'s image; layers
/// become code-made child fields at `k`'s top-left corner, bottom first, each of its
/// image's size within `k`'s box ([`IconImage::Layers`]). Layer fields of an earlier call
/// are removed first.
pub(crate) fn set_icon(ui: &mut Ui, k: ElementKey, icon: Option<&IconImage>) {
    for c in ui.children(k) {
        if ui.element_id(c) == Some(ICON_LAYER_ID) {
            ui.destroy(c);
        }
    }
    match icon {
        Some(IconImage::Composed(img)) => set_image_ref(ui, k, img, examine::DRAW_OVER),
        Some(IconImage::Layers(layers)) => {
            set_image(ui, k, 0, 0);
            let b = ui.layout_box(k).unwrap_or_default();
            for img in layers.iter().take(MAX_ICON_LAYERS) {
                let (w, h) = layer_size(ui, img, (b.w(), b.h()));
                let mut d = ElementDesc {
                    element_id: ICON_LAYER_ID,
                    ..ElementDesc::default()
                };
                d.block.rect.width = w;
                d.block.rect.height = h;
                d.block.has_position = true;
                let Some(f) = examine::code_element(ui, k, WidgetClass::Field, d) else {
                    break;
                };
                set_image_ref(ui, f, img, examine::DRAW_OVER);
            }
        }
        None => set_image(ui, k, 0, 0),
    }
}

/// The size of a layer field: the image's own size within `bx` (width, height) when the
/// image is loaded, else `bx`.
fn layer_size(ui: &mut Ui, img: &ImageRef, bx: (i32, i32)) -> (i32, i32) {
    let (bw, bh) = (bx.0.max(0), bx.1.max(0));
    match ui.env.image(img) {
        crate::env::ImageState::Ready(i) => (
            i32::try_from(i.width()).unwrap_or(i32::MAX).min(bw),
            i32::try_from(i.height()).unwrap_or(i32::MAX).min(bh),
        ),
        _ => (bw, bh),
    }
}

/// Sizes the layer fields of an [`IconImage::Layers`] icon on `k` again to their images'
/// sizes within `k`'s box: call after images that were still loading when the icon was
/// set have arrived (or after `k` was resized). Does nothing for an element without
/// layer fields.
pub fn fit_icon_layers(ui: &mut Ui, k: ElementKey) {
    let b = ui.layout_box(k).unwrap_or_default();
    for c in ui.children(k) {
        if ui.element_id(c) != Some(ICON_LAYER_ID) {
            continue;
        }
        let Some((img, _)) = ui.base_image(c) else {
            continue;
        };
        let (w, h) = layer_size(ui, &img, (b.w(), b.h()));
        ui.set_layout_box(c, crate::geom::IBox::from_xywh(0, 0, w, h));
    }
}

/// Moves `k` to the start of its parent's child list, so that it is drawn first (behind
/// every sibling) and hit-tested last. This is not the sorted insertion of `ui-layout.md`
/// §9.3: game code uses it where a Tessera rule names the position (`game-panels.md` §9.5:
/// the shown map node is the first child of `MapBackground`).
pub(crate) fn move_to_back(ui: &mut Ui, k: ElementKey) {
    reorder(ui, k, true);
}

/// Moves `k` to the end of its parent's child list (drawn last, in front of every sibling).
pub(crate) fn move_to_front(ui: &mut Ui, k: ElementKey) {
    reorder(ui, k, false);
}

fn reorder(ui: &mut Ui, k: ElementKey, to_start: bool) {
    let Some(parent) = ui.parent(k) else { return };
    let Some(p) = ui.el_mut(parent) else { return };
    let at = if to_start {
        p.children.first()
    } else {
        p.children.last()
    };
    if at == Some(&k) {
        return;
    }
    p.children.retain(|c| *c != k);
    if to_start {
        p.children.insert(0, k);
    } else {
        p.children.push(k);
    }
    ui.input.hover_dirty = true;
    ui.mark_dirty(k);
}

/// Creates a code-made Field of `w` × `h` at (0, 0) without layout, image or text, attached
/// to `parent` (element ID 0, not mouse-visible). Game code uses it for elements that have
/// no layout entry of their own (the world map's dungeon image and player marker,
/// `game-panels.md` §9.4–§9.5 Tessera rules).
///
/// # Errors
/// [`crate::UiError::TooManyElements`] past the element cap, or an attachment error.
pub(crate) fn code_field(
    ui: &mut Ui,
    parent: ElementKey,
    w: i32,
    h: i32,
) -> Result<ElementKey, GameError> {
    if ui.element_count() >= MAX_ELEMENTS {
        return Err(crate::UiError::TooManyElements.into());
    }
    let mut desc = ElementDesc::default();
    desc.block.rect.width = w.max(0);
    desc.block.rect.height = h.max(0);
    desc.block.has_position = true;
    let mut e = Element::new(0, WidgetClass::Field, Arc::new(desc));
    e.created = true;
    let k = ui.insert_element(e);
    if let Err(err) = ui.attach_raw(k, parent) {
        ui.destroy(k);
        return Err(err.into());
    }
    Ok(k)
}
