// SPDX-License-Identifier: GPL-3.0-or-later
//! The retained element tree: element keys, the arena and the per-element run-time record
//! (`ui-layout.md` §2 "Element (run time)", §9).

use std::collections::BTreeMap;
use std::sync::Arc;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::ui_layout::{ElementDesc, MediaItem};

use crate::env::{ImageRef, PreparedText};
use crate::geom::IBox;
use crate::widgets::{Widget, WidgetClass};

/// A live element, stable while it exists. Keys of destroyed elements are never reused for
/// another element (a generation counter guards each slot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementKey {
    idx: u32,
    generation: u32,
}

impl ElementKey {
    /// The key as a non-zero `u64`, used as the owner of input-band entries (decision 0014
    /// D5 reserves owner 0 for code).
    #[must_use]
    pub fn owner(self) -> u64 {
        (u64::from(self.generation) << 32) | u64::from(self.idx)
    }

    /// The key from [`ElementKey::owner`], if it can be one.
    #[must_use]
    pub fn from_owner(v: u64) -> Option<Self> {
        let generation = (v >> 32) as u32;
        (generation != 0).then_some(Self {
            idx: v as u32,
            generation,
        })
    }

    pub(crate) fn index(self) -> usize {
        self.idx as usize
    }
}

/// The base image layer (`ui-layout.md` §9.6 item 1).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BaseImage {
    /// What to draw (after skin substitution).
    pub image: ImageRef,
    /// The DID it was requested with (0 for a file), for the skin refresh
    /// (`skin-definition.md` §6.6).
    pub did: u32,
    /// Whether the skin refresh re-sets it (§6.8).
    pub refreshable: bool,
    /// The draw mode (`DrawModeType`).
    pub draw_mode: u32,
}

/// The overlay layer (`ui-layout.md` §9.6 item 2).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Overlay {
    /// The image, or `None` for a colour fill.
    pub image: Option<ImageRef>,
    /// `0xAARRGGBB`.
    pub colour: u32,
    /// The draw mode.
    pub draw_mode: u32,
}

/// The media player of one element (`ui-layout.md` §9.5).
#[derive(Debug, Clone, Default)]
pub(crate) struct MediaPlayer {
    /// The running list.
    pub list: Arc<[MediaItem]>,
    /// Current position.
    pub pos: usize,
    /// Start time of the running item (Animation, Fade, Pause).
    pub item_start: Option<f64>,
    /// The duration a Pause picked.
    pub pause: f64,
    /// The frame an Animation showed last.
    pub last_frame: Option<usize>,
    /// Whether the list has completed.
    pub complete: bool,
}

/// An automatic fade (`ui-behaviour.md` §11.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AutoFade {
    pub start: f64,
    pub from: f32,
    pub to: f32,
    pub duration: f32,
}

/// The text side of a text element.
#[derive(Debug, Clone, Default)]
pub(crate) struct TextState {
    /// Text set at run time (plugins, game code); overrides `UICore_Text_entry`.
    pub runtime: Option<String>,
    /// The resolved text last shaped.
    pub prepared: Option<Arc<PreparedText>>,
    /// Shaping must run again.
    pub dirty: bool,
    /// The text element is cut (`fonts.md` §7.6).
    pub cut: bool,
    /// Caret position (UTF-16 units) for editing.
    pub caret: usize,
}

/// Plugin-control data (`lua-plugin-api.md` §13).
#[derive(Debug, Clone, Default)]
pub(crate) struct PluginData {
    pub kind: crate::control::ControlKind,
    pub apartment: u32,
    pub z_order: i32,
    pub mouse_visible: bool,
    pub wants_updates: bool,
    pub wants_keys: bool,
    pub allow_drop: bool,
    pub enabled: bool,
    pub stretch_set: bool,
    pub blend_mode: u32,
    pub back_colour: [f32; 4],
    pub back_colour_set: bool,
    pub back_blend_mode: u32,
    pub limits: [Option<i32>; 4],
    pub attach: [i32; 4],
    pub attached: bool,
    pub ref_r: IBox,
    pub ref_q: Option<(i32, i32)>,
    pub focus_control: Option<ElementKey>,
    pub rotation: [f32; 3],
    pub rotation_set: bool,
    pub remembered_rotation: [f32; 3],
    pub background: Option<ImageRef>,
}

/// One live element.
#[derive(Debug, Clone)]
pub(crate) struct Element {
    /// Element ID (`UIElementID`).
    pub element_id: u32,
    /// Element type of the resolved descriptor.
    pub element_type: u32,
    /// Class.
    pub class: WidgetClass,
    /// Resolved descriptor without its children.
    pub desc: Arc<ElementDesc>,
    /// The layout record the element came from (0 for code-made elements).
    pub layout_did: Did,
    /// Design size of that layout record (`ui-layout.md` §8.3).
    pub layout_size: (i32, i32),
    pub parent: Option<ElementKey>,
    /// Children in sibling order (`ui-layout.md` §9.3).
    pub children: Vec<ElementKey>,
    pub z: i32,
    pub read_order: u32,
    /// Current layout box, relative to the parent.
    pub bx: IBox,
    /// Design box.
    pub design: IBox,
    pub edges: [u32; 4],
    pub flags: u8,
    /// Code-assigned reference box (`ui-layout.md` §8.3).
    pub ref_box: Option<IBox>,
    /// Laid out against the screen (`ui-layout.md` §8.3).
    pub screen_level: bool,
    /// A root panel (`ui-layout.md` §9.1).
    pub root_panel: bool,
    pub created: bool,
    /// Own visibility flag.
    pub visible: bool,
    pub state: u32,
    /// Run-time properties.
    pub runtime: BTreeMap<u32, PropertyValue>,
    /// Effective properties.
    pub effective: BTreeMap<u32, PropertyValue>,
    pub media: MediaPlayer,
    pub base: Option<BaseImage>,
    pub overlay: Option<Overlay>,
    pub image_offset: (i32, i32),
    pub cursor: Option<(u32, u32, u32)>,
    pub hit_mask: u32,
    pub mouse_visible: bool,
    pub blocks_clicks: bool,
    /// Mouse-button commands pressed on the element (`ui-behaviour.md` §4.6).
    pub pressed: Vec<Arc<str>>,
    pub draw_after_children: bool,
    /// Elements attached as listeners (`ui-behaviour.md` §12.1 step 2.2).
    pub listeners: Vec<ElementKey>,
    pub widget: Widget,
    pub plugin: Option<Box<PluginData>>,
    /// Display-node scale (`ui-behaviour.md` §9.2.1).
    pub scale: Option<[f32; 3]>,
    /// Cached unscaled size and scale of a scaled panel (`ui-behaviour.md` §9.2).
    pub scaler: Option<((i32, i32), f32)>,
    pub fade: Option<AutoFade>,
    pub text: TextState,
    /// Paint must be rebuilt. Set only through `Ui::note_paint_dirty` (or
    /// `Ui::mark_dirty`), which also records the element for the paint step's damage pass.
    pub paint_dirty: bool,
    /// Registered for per-frame work.
    pub frame_update: bool,
    /// Accepts double clicks (`ui-behaviour.md` §4.6; Tessera: plugin controls do).
    pub accepts_double_click: bool,
    /// Code asked for move messages (`ui-behaviour.md` §4.6).
    pub wants_moves: bool,
    /// Code marked the element for hover messages (`ui-behaviour.md` §6.2 step 4.2).
    pub hover_messages: bool,
    /// Run-time tooltip text (`ui-behaviour.md` §6.3).
    pub tooltip_text: Option<String>,
    /// Moving or resizing (`ui-behaviour.md` §8.1–§8.2).
    pub wm: Option<crate::wm::WmOp>,
    /// Drag icon in progress.
    pub drag_icon: bool,
    /// Angles held by the element's surface (decision 0015 §2.3), degrees.
    pub rotation: [f32; 3],
    /// Pending scale application while hidden (`lua-plugin-api.md` §13.21 rule 5): the
    /// scale and the `scaler_type` it was asked with (`ui-behaviour.md` §9.2 step 6).
    pub pending_scale: Option<(f32, bool)>,
    /// A modal-dialog container (`Ui::open_panel`): destroyed with its last dialog.
    pub dialog_container: bool,
    /// Game code asks for the examination tooltip of this element on hover
    /// (`item-tooltips.md` §9, Tessera rule: every item slot requests it).
    pub examine: bool,
    /// The drag description game code gives this element as a drag source
    /// (`ui-behaviour.md` §7.1 step 4).
    pub drag_payload: Option<crate::drag::DragPayload>,
    /// The element whose copy is the drag icon when this element is the source (§7.1
    /// step 4: a game widget's own icon); `None` leaves it to the container (step 5).
    pub drag_icon_from: Option<ElementKey>,
    /// The accept check game code sets on a drop target (§7.3 step 1): with `Some(mask)`
    /// a drag whose payload has no bit of `mask` in its `fits` is refused.
    pub drop_mask: Option<u32>,
}

impl Element {
    pub(crate) fn new(element_id: u32, class: WidgetClass, desc: Arc<ElementDesc>) -> Self {
        let r = desc.block.rect;
        let design = IBox::from_xywh(r.x, r.y, r.width, r.height).clamped();
        Self {
            element_id,
            element_type: desc.element_type,
            class,
            layout_did: Did(0),
            layout_size: (0, 0),
            parent: None,
            children: Vec::new(),
            z: desc.block.z_level,
            read_order: desc.read_order,
            bx: design,
            design,
            edges: desc.edges.to_array().map(|e| e.0),
            flags: desc.block.flags.0,
            ref_box: None,
            screen_level: false,
            root_panel: false,
            created: false,
            visible: true,
            state: 0,
            runtime: BTreeMap::new(),
            effective: BTreeMap::new(),
            media: MediaPlayer::default(),
            base: None,
            overlay: None,
            image_offset: (0, 0),
            cursor: None,
            hit_mask: 0,
            mouse_visible: false,
            blocks_clicks: false,
            pressed: Vec::new(),
            draw_after_children: false,
            listeners: Vec::new(),
            widget: Widget::for_class(class),
            plugin: None,
            scale: None,
            scaler: None,
            fade: None,
            text: TextState::default(),
            paint_dirty: true,
            frame_update: false,
            accepts_double_click: false,
            wants_moves: false,
            hover_messages: false,
            tooltip_text: None,
            wm: None,
            drag_icon: false,
            rotation: [0.0; 3],
            pending_scale: None,
            dialog_container: false,
            examine: false,
            drag_payload: None,
            drag_icon_from: None,
            drop_mask: None,
            desc,
        }
    }
}

/// The arena of elements.
#[derive(Debug, Default)]
pub(crate) struct Arena {
    slots: Vec<(u32, Option<Element>)>,
    free: Vec<u32>,
    live: usize,
}

impl Arena {
    pub fn insert(&mut self, e: Element) -> ElementKey {
        self.live += 1;
        if let Some(idx) = self.free.pop() {
            let slot = &mut self.slots[idx as usize];
            slot.0 = slot.0.wrapping_add(1).max(1);
            slot.1 = Some(e);
            ElementKey {
                idx,
                generation: slot.0,
            }
        } else {
            let idx = self.slots.len() as u32;
            self.slots.push((1, Some(e)));
            ElementKey { idx, generation: 1 }
        }
    }

    pub fn remove(&mut self, k: ElementKey) -> Option<Element> {
        let slot = self.slots.get_mut(k.index())?;
        if slot.0 != k.generation {
            return None;
        }
        let e = slot.1.take()?;
        self.free.push(k.idx);
        self.live -= 1;
        Some(e)
    }

    pub fn get(&self, k: ElementKey) -> Option<&Element> {
        let slot = self.slots.get(k.index())?;
        if slot.0 != k.generation {
            return None;
        }
        slot.1.as_ref()
    }

    pub fn get_mut(&mut self, k: ElementKey) -> Option<&mut Element> {
        let slot = self.slots.get_mut(k.index())?;
        if slot.0 != k.generation {
            return None;
        }
        slot.1.as_mut()
    }

    pub fn len(&self) -> usize {
        self.live
    }

    /// Every live element, mutably, with its key.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (ElementKey, &mut Element)> {
        self.slots.iter_mut().enumerate().filter_map(|(i, s)| {
            let generation = s.0;
            s.1.as_mut().map(|e| {
                (
                    ElementKey {
                        idx: i as u32,
                        generation,
                    },
                    e,
                )
            })
        })
    }

    pub fn keys(&self) -> Vec<ElementKey> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.1.is_some())
            .map(|(i, s)| ElementKey {
                idx: i as u32,
                generation: s.0,
            })
            .collect()
    }
}
