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

pub mod chat;
pub mod data;
pub mod dragbox;
pub mod escape;
pub mod hidewhen;
pub mod map;
mod panels;
pub mod quickslot;
pub mod radar;
pub mod simple;
pub mod vocab;
pub mod worldmap;

pub use data::{
    GameData, HideCondition, MapLookup, MapSource, NoShortcuts, SandboxData, Shortcut, Shortcuts,
};
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
