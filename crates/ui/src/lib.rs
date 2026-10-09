// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui`: the game UI (`docs/ARCHITECTURE.md` §15, decisions 0014 and 0015).
//!
//! A retained element tree built from UI layout records (`docs/specs/ui-layout.md`), engine
//! widgets (`ui-widgets.md`), pointer and keyboard behaviour (`ui-behaviour.md`), user skins
//! (`skin-definition.md`), text (`fonts.md`), and the native-panel game types
//! (`game-panels.md`, `chat-window.md`). Art is painted on the CPU into sparse per-surface
//! canvases; the crate hands a composition list of finished surfaces to the client, which
//! passes it to `tessera-render`'s UI pass. No wgpu, no window, no Lua: `tessera-ui` depends
//! on neither `tessera-input` nor `tessera-plugins`; the client wires them together.
//!
//! Module ownership during M5 (parallel implementation):
//!
//! - [`paint`]: sparse CPU canvases, the byte-exact painter, damage, paint jobs, the
//!   composition list and the decoded-image cache (decision 0015; the image decoders
//!   themselves live in `tessera-material` and run on the asset workers).
//! - [`text`]: glyph atlases, glyph lookup, placement, line layout and text drawing
//!   (`fonts.md` §4–§9).
//! - [`game`]: game widget types of the native panels (`game-panels.md`) and the chat
//!   widgets (`chat-window.md`).
//! - [`skin`]: skin discovery, `SkinDefinition.xml`, art and geometry overrides
//!   (`skin-definition.md`).
//! - the rest of the crate: element tree, descriptor resolution, layout, states, widgets,
//!   behaviour, the UI side of the input bridge (decision 0014) and the plugin-control
//!   element API.
//!
//! # The core
//!
//! [`Ui`] owns the tree. Its methods are grouped by module:
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`resolve`] | base-element inheritance, the merge | UL §7 |
//! | [`layout`] | design-time and run-time layout arithmetic, plugin edge attachments | UL §8, LPA §13.16 |
//! | `build` | opening panels and creating element trees, the completion pass | UL §9.1–§9.2, UW §3.4 |
//! | `state`, `media` | states, property changes, the media player, the base-image setter with its skin hook | UL §9.4–§9.8 |
//! | [`widgets`] | Button, Meter, Scrollable, Scrollbar, ListBox, Tree, Panel, Menu, GroupBox | UW §3–§11 |
//! | `pointer`, `focus`, `tooltip`, `drag`, `wm`, `scale`, `visibility`, `text_ui` | the behaviour rules | UB §4–§11 |
//! | [`events`], `message` | UI messages, the listener registry, the outbox | UB §12, LPA §9.7 |
//! | [`bridge`] | offers, verdicts, band requests, change requests, the step-1 and step-7 entry points | IB D5, D7, D8 |
//! | [`surface`] | surfaces, display lists, damage, composition | UR §1–§2 |
//! | [`control`] | the plugin-control API | LPA §13, UR §5 |
//! | [`env`](mod@env) | the services the client provides (layouts, images, text, strings) | — |
//! | [`vocab`] | property IDs, messages, states, types by label | UL §3.3 |

pub mod bridge;
mod build;
pub mod control;
mod drag;
pub mod env;
pub mod events;
mod focus;
pub mod game;
pub mod geom;
mod handlers;
pub mod layout;
mod media;
mod message;
pub mod paint;
mod pointer;
pub mod props;
pub mod resolve;
mod scale;
pub mod skin;
mod state;
pub mod surface;
pub mod text;
mod text_ui;
mod tooltip;
mod tree;
mod ui;
mod visibility;
pub mod vocab;
pub mod widgets;
mod wm;

pub use bridge::{CommandOffer, Phase, UiChange, UiInput};
pub use control::{Background, ControlKind};
pub use env::{ImageRef, ImageState, MemoryEnv, NullEnv, PreparedText, TextRequest, UiEnv};
pub use events::{Answer, Command, ControlEvent, Message, MsgArg, UiEvent};
pub use focus::{BandChange, BandEntry};
pub use geom::IBox;
pub use surface::{GroupGeometry, UiFrame};
pub use tree::ElementKey;
pub use ui::{
    BuildCensus, MAX_BUILD_ELEMENTS, MAX_CREATE_NESTING, MAX_ELEMENTS, MAX_TREE_DEPTH, Ui,
    UiSettings, displayed_from, object_mode,
};
pub use vocab::Vocab;
pub use widgets::WidgetClass;
pub use wm::{SavedEntry, SavedStore, normalized_to_pixels, pixels_to_normalized};

/// Errors of the UI core.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UiError {
    /// The key names no live element.
    #[error("no such element")]
    NoSuchElement,
    /// Attaching would make an element its own ancestor.
    #[error("an element cannot be its own ancestor")]
    Cycle,
    /// Attaching would make the tree deeper than [`MAX_TREE_DEPTH`].
    #[error("element tree deeper than {MAX_TREE_DEPTH}")]
    TooDeep,
    /// More than [`MAX_ELEMENTS`] elements.
    #[error("too many elements")]
    TooManyElements,
    /// A layout record that is missing or does not decode.
    #[error("layout {0:#010x} is not available")]
    NoLayout(u32),
    /// A `UILAYOUT` key the map does not hold (`ui-layout.md` §3.2: no panel).
    #[error("no layout for UILAYOUT key {0:#010x}")]
    NoLayoutKey(u32),
    /// The element is not a top-level element of the layout.
    #[error("element {0:#010x} is not a top-level element of the layout")]
    NoElement(u32),
    /// The element was not created (resolution failed or no factory, `ui-layout.md` §13).
    #[error("element {0:#010x} was not created")]
    NotCreated(u32),
    /// The element is not a list box.
    #[error("not a list box")]
    NotAList,
    /// A list box template that is missing or malformed (`ui-widgets.md` §8.2).
    #[error("no such entry template")]
    NoTemplate,
    /// A list or table is full.
    #[error("capacity exhausted")]
    Full,
    /// A blend mode above 8 (`lua-plugin-api.md` §13.17).
    #[error("blend mode {0} out of range")]
    BadBlendMode(u32),
    /// A scale that is not greater than 0 (`lua-plugin-api.md` §13.3).
    #[error("scale must be greater than 0")]
    BadScale,
}
