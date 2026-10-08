// SPDX-License-Identifier: GPL-3.0-or-later
//! The services the UI core asks the client for: decoded layouts, images, prepared text and
//! string resolution.
//!
//! `tessera-ui` reads no archive and decodes nothing itself (decision 0015 §4.1: all
//! decoding runs on the asset workers). The client implements [`UiEnv`] over its caches;
//! every method answers at once from what is loaded and may start a load for later
//! (`ImageState::Pending`), so the UI never blocks the main thread (ARCHITECTURE §21).
//! [`NullEnv`] answers "nothing" to everything and serves tests and runs without data.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::did::Did;
use tessera_formats::string_info::StringInfo;
use tessera_formats::ui_layout::UiLayout;
use tessera_material::ui_image::UiImage;

use crate::text::{FontFace, TextChar, TextLayout};

/// What a base image or an overlay image is made from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ImageRef {
    /// A render surface (prefix 0x41) by DID.
    Did(u32),
    /// A file: an Image item path relative to the install (`ui-layout.md` §6.2), or a plugin
    /// image relative to `Plugins/` (`lua-plugin-api.md` §13.8). The client knows which.
    File(String),
    /// A user-skin replacement of a render surface (`skin-definition.md` §6.3): the
    /// art-asset value and the DID the image was requested with.
    Skin {
        /// The art-asset value the active skin maps.
        art_asset: u32,
        /// The original DID.
        did: u32,
    },
}

/// The state of an image request.
#[derive(Debug, Clone)]
pub enum ImageState {
    /// Decoded and available.
    Ready(Arc<UiImage>),
    /// Being loaded; draws nothing until it arrives (decision 0015 Decision 1.8).
    Pending,
    /// Failed for good; the client may return its placeholder as `Ready` instead.
    Failed,
}

/// A text element's text prepared for drawing: the layout of `fonts.md` §7, the character
/// list and the fonts by slot.
#[derive(Debug, Clone, Default)]
pub struct PreparedText {
    /// Laid-out lines.
    pub layout: TextLayout,
    /// The character list.
    pub chars: Vec<TextChar>,
    /// Fonts by slot (`fonts.md` §9.1).
    pub fonts: Vec<Option<Arc<FontFace>>>,
    /// `UICore_Text_outline`.
    pub outline: bool,
    /// The outline colour, `0xAARRGGBB`.
    pub outline_colour: u32,
    /// Whether the text was cut by `UICore_Text_truncate_text_to_fit` (`fonts.md` §7.6).
    pub truncated: bool,
    /// The measured extent of the text with the element's margins, in pixels
    /// (`ui-behaviour.md` §6.4 step 1).
    pub extent: (i32, i32),
}

/// What the UI asks a text shaper for.
#[derive(Debug, Clone)]
pub struct TextRequest<'a> {
    /// The text to lay out (the element's run-time text, or its resolved `UICore_Text_entry`).
    pub text: &'a str,
    /// The element's effective properties (font, colour, justification, margins, …).
    pub props: &'a [(u32, tessera_formats::property::PropertyValue)],
    /// The element's layout-box size.
    pub size: (i32, i32),
    /// The wrap width override for measuring (`ui-behaviour.md` §6.4), when not the box.
    pub wrap_width: Option<i32>,
}

/// Services of the client.
pub trait UiEnv {
    /// The decoded layout record with this DID (`ui-layout.md` §3.1), or `None` when it is
    /// missing or does not decode.
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>>;

    /// The layout DID of a `UILAYOUT` DID-map entry key (`ui-layout.md` §3.2, U31).
    fn layout_by_key(&mut self, key: u32) -> Option<Did> {
        let _ = key;
        None
    }

    /// The decoded image of a reference.
    fn image(&mut self, image: &ImageRef) -> ImageState {
        let _ = image;
        ImageState::Pending
    }

    /// Lays out a text element's text (`fonts.md` §6–§9). `None` when the fonts are not
    /// loaded yet or the element has no font (nothing is drawn, `fonts.md` §9.1 rule 4).
    fn shape_text(&mut self, request: &TextRequest<'_>) -> Option<Arc<PreparedText>> {
        let _ = request;
        None
    }

    /// The text of a StringInfo value (`string-table.md`).
    fn resolve_string(&mut self, info: &StringInfo) -> Option<String> {
        let _ = info;
        None
    }

    /// The truncated-text tooltip (`ui-behaviour.md` §10.2): the `UI_Helper` template
    /// rendered with `TEXT` = `full` and `TOOLTIP` = `tooltip`.
    fn truncated_tooltip(&mut self, full: &str, tooltip: &str) -> String {
        format!("{full}\n{tooltip}")
    }

    /// The InputMap name of a `UICore_Element_InputMapID` value (KM §12.5.4).
    fn input_map_name(&mut self, value: u32) -> Option<String> {
        let _ = value;
        None
    }

    /// A uniform random number in [0, 1] (Message, State and Jump items and Pause items,
    /// `ui-layout.md` §6.7–§6.8).
    fn random(&mut self) -> f32 {
        0.0
    }
}

/// An environment with no data: no layouts, every image pending, no text.
#[derive(Debug, Default, Clone)]
pub struct NullEnv;

impl UiEnv for NullEnv {
    fn layout(&mut self, _did: Did) -> Option<Arc<UiLayout>> {
        None
    }
}

/// An in-memory environment for tests and tools: layouts and images by key, a fixed random
/// value, and a trivial string resolver (literal StringInfo values only).
#[derive(Debug, Default, Clone)]
pub struct MemoryEnv {
    /// Layouts by DID.
    pub layouts: HashMap<u32, Arc<UiLayout>>,
    /// `UILAYOUT` keys → layout DIDs.
    pub keys: HashMap<u32, u32>,
    /// Images by reference.
    pub images: HashMap<ImageRef, Arc<UiImage>>,
    /// Prepared text for every request with this text.
    pub texts: HashMap<String, Arc<PreparedText>>,
    /// The value [`UiEnv::random`] returns.
    pub random: f32,
}

impl MemoryEnv {
    /// Adds a layout.
    pub fn add_layout(&mut self, layout: UiLayout) {
        self.layouts.insert(layout.did.0, Arc::new(layout));
    }
}

impl UiEnv for MemoryEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        self.layouts.get(&did.0).cloned()
    }

    fn layout_by_key(&mut self, key: u32) -> Option<Did> {
        self.keys.get(&key).map(|d| Did(*d))
    }

    fn image(&mut self, image: &ImageRef) -> ImageState {
        match self.images.get(image) {
            Some(i) => ImageState::Ready(i.clone()),
            None => ImageState::Failed,
        }
    }

    fn shape_text(&mut self, request: &TextRequest<'_>) -> Option<Arc<PreparedText>> {
        self.texts.get(request.text).cloned()
    }

    fn resolve_string(&mut self, info: &StringInfo) -> Option<String> {
        match &info.source {
            tessera_formats::string_info::StringSource::Literal(s) => Some(s.to_string()),
            _ => None,
        }
    }

    fn random(&mut self) -> f32 {
        self.random
    }
}
