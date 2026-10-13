// SPDX-License-Identifier: GPL-3.0-or-later
//! The [`Ui`]: the element tree with everything that acts on it.
//!
//! This module holds the struct, the arena accessors, effective properties
//! (`ui-layout.md` §9.4 step 5, `ui-widgets.md` §2.2), sibling order (§9.3), attachment and
//! the run-time layout of §8.4 with its reference boxes (§8.3), element moves and resizes
//! (`ui-behaviour.md` §9.2.1) and displayed boxes. Behaviour, widgets, input, painting and
//! the plugin-control API add `impl Ui` blocks in their own modules.

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::Arc;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::ui_layout::ElementDesc;

use crate::UiError;
use crate::env::UiEnv;
use crate::events::{Message, UiEvent};
use crate::geom::{IBox, trunc_f32};
use crate::layout::{RuntimeInput, runtime};
use crate::props::Prop;
use crate::skin::SkinOverlay;
use crate::tree::{Arena, Element, ElementKey};
use crate::vocab::Vocab;
use crate::widgets::WidgetClass;

/// Deepest element tree the UI accepts (Tessera cap against hostile plugin trees; layouts
/// nest at most 64 deep, `ui-layout.md` §4.7). Attaching beyond it fails, and one outermost
/// creation builds at most this many levels below its root, the levels of the creations
/// nested in it included (they share one depth, so the build recursion stays bounded).
pub const MAX_TREE_DEPTH: usize = 128;

/// Most live elements (Tessera cap, hostile input): creating more fails.
pub const MAX_ELEMENTS: usize = 1 << 20;

/// Deepest nesting of element creations started from inside another creation (a Menu's
/// popup, a list box template, a tree icon; Tessera cap, hostile input). A creation of a
/// (layout, element) pair that is already being created further up is refused as well.
pub const MAX_CREATE_NESTING: usize = 16;

/// Most elements one outermost creation (`Ui::create_element`, `Ui::open_panel`) may build,
/// nested creations included (Tessera cap, hostile input).
pub const MAX_BUILD_ELEMENTS: usize = 1 << 16;

/// Object modes (`ui-layout.md` §5.9).
pub mod object_mode {
    /// None.
    pub const NONE: u32 = 0;
    /// Stretch.
    pub const STRETCH: u32 = 1;
    /// Tile.
    pub const TILE: u32 = 2;
    /// CanvasResize.
    pub const CANVAS_RESIZE: u32 = 3;
}

/// Tessera's UI settings that the behaviour rules read (`ui-behaviour.md` §6.1, §9.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiSettings {
    /// `[Misc] TooltipEnable`.
    pub tooltip_enable: bool,
    /// `[Misc] TooltipDelay`, seconds.
    pub tooltip_delay: f32,
    /// `[UI] UIScale_Global`.
    pub scale_global: f32,
    /// `[UI] UIScale_Cursor` (reset to 1 when ≤ 0 or > 4, §9.3).
    pub scale_cursor: f32,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            tooltip_enable: true,
            tooltip_delay: 0.25,
            scale_global: 1.0,
            scale_cursor: 1.0,
        }
    }
}

/// Counts for ROADMAP M5 exit criterion 1 (`ui-layout.md` §13): resolution failures and the
/// game types that fell back to a plain Field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildCensus {
    /// Elements not created because their base chain failed, by §13 category name.
    pub resolve_failures: BTreeMap<&'static str, u64>,
    /// Game types built with the fallback widget, with counts.
    pub fallback_types: BTreeMap<u32, u64>,
    /// Engine types without a factory (not created).
    pub no_factory: BTreeMap<u32, u64>,
    /// Elements created.
    pub created: u64,
    /// Creations refused by the nesting guard ([`MAX_CREATE_NESTING`], a (layout, element)
    /// pair created inside itself), elements refused by the per-build budget
    /// ([`MAX_BUILD_ELEMENTS`]) or elements beyond the build depth shared by nested
    /// creations ([`MAX_TREE_DEPTH`]).
    pub creation_refused: u64,
}

/// The game UI.
pub struct Ui {
    pub(crate) env: Box<dyn UiEnv>,
    pub(crate) v: Vocab,
    pub(crate) arena: Arena,
    pub(crate) screen: ElementKey,
    pub(crate) screen_size: (i32, i32),
    pub(crate) now: f64,
    pub(crate) skin: Option<Arc<SkinOverlay>>,
    pub(crate) settings: UiSettings,
    pub(crate) queue: VecDeque<Message>,
    pub(crate) delivering: bool,
    pub(crate) seq: u64,
    pub(crate) registry: BTreeMap<(u32, u32), u32>,
    pub(crate) events: Vec<UiEvent>,
    pub(crate) input: crate::pointer::InputState,
    pub(crate) focus: crate::focus::FocusState,
    pub(crate) tooltip: crate::tooltip::TooltipState,
    pub(crate) drag: crate::drag::DragState,
    pub(crate) changes: VecDeque<crate::bridge::UiChange>,
    pub(crate) painter: crate::surface::Painter,
    pub(crate) census: BuildCensus,
    pub(crate) subs: crate::control::Subscriptions,
    pub(crate) media_depth: u32,
    pub(crate) next_plugin_order: u32,
    pub(crate) saved: crate::wm::SavedStore,
    pub(crate) game_types: crate::build::GameTypes,
    pub(crate) bridge: crate::bridge::BridgeState,
    pub(crate) lists: std::collections::BTreeSet<ElementKey>,
    pub(crate) frame_set: std::collections::BTreeSet<ElementKey>,
    pub(crate) suppress_size: bool,
    /// The (layout DID, element ID) pairs being created, outermost first.
    pub(crate) creating: Vec<(u32, u32)>,
    /// Elements the current outermost creation may still build.
    pub(crate) build_budget: usize,
    /// Build levels open across the current creation and every creation nested in it
    /// (bounded by [`MAX_TREE_DEPTH`], so nested creations share one stack depth).
    pub(crate) build_depth: usize,
    /// Procedural images of the fallback looks (`game::combat::look`), by path; bounded.
    pub(crate) procedural:
        std::collections::HashMap<String, Arc<tessera_material::ui_image::UiImage>>,
}

impl std::fmt::Debug for Ui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ui")
            .field("elements", &self.arena.len())
            .field("screen_size", &self.screen_size)
            .field("now", &self.now)
            .finish_non_exhaustive()
    }
}

impl Ui {
    /// A UI with an empty screen root of `screen` pixels (`ui-layout.md` §8.5: element ID
    /// 0x13, type Field, edges (5, 5, 5, 5), design size = the screen size; 1024 × 768 when
    /// a side is not positive).
    pub fn new(env: Box<dyn UiEnv>, vocab: Vocab, screen: (i32, i32)) -> Self {
        let screen = if screen.0 > 0 && screen.1 > 0 {
            (
                screen.0.min(crate::geom::COORD_LIMIT),
                screen.1.min(crate::geom::COORD_LIMIT),
            )
        } else {
            (1024, 768)
        };
        let mut desc = ElementDesc {
            element_id: vocab.ids.screen_root,
            element_type: vocab.types.field,
            ..ElementDesc::default()
        };
        desc.block.rect.width = screen.0;
        desc.block.rect.height = screen.1;
        desc.edges.left.0 = 5;
        desc.edges.top.0 = 5;
        desc.edges.right.0 = 5;
        desc.edges.bottom.0 = 5;
        let mut root = Element::new(desc.element_id, WidgetClass::Field, Arc::new(desc));
        root.created = true;
        root.layout_size = screen;
        let mut arena = Arena::default();
        let screen_key = arena.insert(root);
        Self {
            env,
            v: vocab,
            arena,
            screen: screen_key,
            screen_size: screen,
            now: 0.0,
            skin: None,
            settings: UiSettings::default(),
            queue: VecDeque::new(),
            delivering: false,
            seq: 0,
            registry: BTreeMap::new(),
            events: Vec::new(),
            input: crate::pointer::InputState::default(),
            focus: crate::focus::FocusState::default(),
            tooltip: crate::tooltip::TooltipState::default(),
            drag: crate::drag::DragState::default(),
            changes: VecDeque::new(),
            painter: crate::surface::Painter::new(crate::paint::PaintLimits::default()),
            census: BuildCensus::default(),
            subs: crate::control::Subscriptions::default(),
            media_depth: 0,
            next_plugin_order: 1,
            saved: crate::wm::SavedStore::default(),
            game_types: crate::build::GameTypes::default(),
            bridge: crate::bridge::BridgeState::default(),
            lists: std::collections::BTreeSet::new(),
            frame_set: std::collections::BTreeSet::new(),
            suppress_size: false,
            creating: Vec::new(),
            build_budget: MAX_BUILD_ELEMENTS,
            build_depth: 0,
            procedural: std::collections::HashMap::new(),
        }
    }

    // ----- accessors ---------------------------------------------------------------------

    /// The vocabulary in use.
    #[must_use]
    pub fn vocab(&self) -> &Vocab {
        &self.v
    }

    /// The screen root.
    #[must_use]
    pub fn screen_root(&self) -> ElementKey {
        self.screen
    }

    /// The screen size.
    #[must_use]
    pub fn screen_size(&self) -> (i32, i32) {
        self.screen_size
    }

    /// The UI clock (seconds; decision 0014 Amendment 1 item 4).
    #[must_use]
    pub fn time(&self) -> f64 {
        self.now
    }

    /// Advances the UI clock. The client calls it once per frame, right after present.
    pub fn set_time(&mut self, now: f64) {
        if now.is_finite() {
            self.now = now;
        }
    }

    /// The behaviour settings.
    #[must_use]
    pub fn settings(&self) -> UiSettings {
        self.settings
    }

    /// The build census (`ui-layout.md` §13).
    #[must_use]
    pub fn census(&self) -> &BuildCensus {
        &self.census
    }

    /// Number of live elements.
    #[must_use]
    pub fn element_count(&self) -> usize {
        self.arena.len()
    }

    /// Whether the key names a live element.
    #[must_use]
    pub fn exists(&self, k: ElementKey) -> bool {
        self.arena.get(k).is_some()
    }

    pub(crate) fn el(&self, k: ElementKey) -> Option<&Element> {
        self.arena.get(k)
    }

    pub(crate) fn el_mut(&mut self, k: ElementKey) -> Option<&mut Element> {
        self.arena.get_mut(k)
    }

    /// The element ID (`UIElementID`).
    #[must_use]
    pub fn element_id(&self, k: ElementKey) -> Option<u32> {
        self.el(k).map(|e| e.element_id)
    }

    /// The element type of the resolved descriptor (`UIElementType`; 0 for code-made
    /// elements).
    #[must_use]
    pub fn element_type(&self, k: ElementKey) -> Option<u32> {
        self.el(k).map(|e| e.element_type)
    }

    /// The element's class.
    #[must_use]
    pub fn class(&self, k: ElementKey) -> Option<WidgetClass> {
        self.el(k).map(|e| e.class)
    }

    /// The parent.
    #[must_use]
    pub fn parent(&self, k: ElementKey) -> Option<ElementKey> {
        self.el(k).and_then(|e| e.parent)
    }

    /// The children in sibling order (`ui-layout.md` §9.3: drawing order, back to front).
    #[must_use]
    pub fn children(&self, k: ElementKey) -> Vec<ElementKey> {
        self.el(k).map(|e| e.children.clone()).unwrap_or_default()
    }

    /// The layout box relative to the parent (`ui-behaviour.md` §9.2.1).
    #[must_use]
    pub fn layout_box(&self, k: ElementKey) -> Option<IBox> {
        self.el(k).map(|e| e.bx)
    }

    /// The design box.
    #[must_use]
    pub fn design_box(&self, k: ElementKey) -> Option<IBox> {
        self.el(k).map(|e| e.design)
    }

    /// The z-level.
    #[must_use]
    pub fn z_level(&self, k: ElementKey) -> Option<i32> {
        self.el(k).map(|e| e.z)
    }

    /// The own visibility flag.
    #[must_use]
    pub fn is_visible(&self, k: ElementKey) -> bool {
        self.el(k).is_some_and(|e| e.visible)
    }

    /// Visible with every ancestor (`ui-behaviour.md` §2 "Displayed"), up to the screen
    /// root. An element not attached below the screen root is not displayed.
    #[must_use]
    pub fn is_displayed(&self, k: ElementKey) -> bool {
        let mut cur = Some(k);
        let mut depth = 0;
        while let Some(c) = cur {
            let Some(e) = self.el(c) else { return false };
            if !e.visible {
                return false;
            }
            if c == self.screen {
                return true;
            }
            cur = e.parent;
            depth += 1;
            if depth > MAX_TREE_DEPTH + 2 {
                return false;
            }
        }
        false
    }

    /// The current state (`ui-layout.md` §9.4).
    #[must_use]
    pub fn state(&self, k: ElementKey) -> Option<u32> {
        self.el(k).map(|e| e.state)
    }

    /// The mouse-visible flag (`ui-behaviour.md` §4.3).
    #[must_use]
    pub fn mouse_visible(&self, k: ElementKey) -> bool {
        self.el(k).is_some_and(|e| e.mouse_visible)
    }

    /// The blocks-clicks flag.
    #[must_use]
    pub fn blocks_clicks(&self, k: ElementKey) -> bool {
        self.el(k).is_some_and(|e| e.blocks_clicks)
    }

    /// Whether the element has finished creation.
    #[must_use]
    pub fn is_created(&self, k: ElementKey) -> bool {
        self.el(k).is_some_and(|e| e.created)
    }

    /// The element's base image (reference and draw mode).
    #[must_use]
    pub fn base_image(&self, k: ElementKey) -> Option<(crate::env::ImageRef, u32)> {
        self.el(k)
            .and_then(|e| e.base.as_ref())
            .map(|b| (b.image.clone(), b.draw_mode))
    }

    /// The element's overlay (image, colour, draw mode).
    #[must_use]
    pub fn overlay(&self, k: ElementKey) -> Option<(Option<crate::env::ImageRef>, u32, u32)> {
        self.el(k)
            .and_then(|e| e.overlay.as_ref())
            .map(|o| (o.image.clone(), o.colour, o.draw_mode))
    }

    /// The element's cursor (image DID and hot spot), set by a Cursor item.
    #[must_use]
    pub fn cursor_of(&self, k: ElementKey) -> Option<(u32, u32, u32)> {
        self.el(k).and_then(|e| e.cursor)
    }

    /// The layout record and its design size the element came from.
    #[must_use]
    pub fn layout_of(&self, k: ElementKey) -> Option<(Did, (i32, i32))> {
        self.el(k).map(|e| (e.layout_did, e.layout_size))
    }

    /// Takes the events of the outbox (see [`crate::events`]).
    pub fn take_events(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.events)
    }

    /// The events waiting in the outbox.
    #[must_use]
    pub fn pending_events(&self) -> &[UiEvent] {
        &self.events
    }

    pub(crate) fn emit(&mut self, e: UiEvent) {
        if self.events.len() < 1 << 20 {
            self.events.push(e);
        }
    }

    // ----- properties --------------------------------------------------------------------

    /// A property read by `ui-widgets.md` §2.2: effective value, then dictionary default.
    #[must_use]
    pub fn prop(&self, k: ElementKey, pid: u32) -> Prop<'_> {
        Prop {
            set: self.el(k).and_then(|e| e.effective.get(&pid)),
            default: self.v.defaults.get(&pid),
        }
    }

    /// The effective value of a property (run-time, then current state, then own).
    #[must_use]
    pub fn property(&self, k: ElementKey, pid: u32) -> Option<&PropertyValue> {
        self.el(k).and_then(|e| e.effective.get(&pid))
    }

    /// Every effective property of an element.
    #[must_use]
    pub fn properties(&self, k: ElementKey) -> Vec<(u32, PropertyValue)> {
        self.el(k)
            .map(|e| e.effective.iter().map(|(p, v)| (*p, v.clone())).collect())
            .unwrap_or_default()
    }

    /// Sets a run-time property (overrides state and own values, `ui-layout.md` §9.4 step 5)
    /// and reports the change to the element's class. PID 0 is ignored.
    pub fn set_property(&mut self, k: ElementKey, pid: u32, value: PropertyValue) {
        if pid == 0 {
            return;
        }
        let Some(e) = self.el_mut(k) else { return };
        if e.runtime.get(&pid) == Some(&value) {
            return;
        }
        e.runtime.insert(pid, value);
        self.refresh_effective(k);
    }

    /// Removes a run-time property.
    pub fn clear_property(&mut self, k: ElementKey, pid: u32) {
        let Some(e) = self.el_mut(k) else { return };
        if e.runtime.remove(&pid).is_some() {
            self.refresh_effective(k);
        }
    }

    pub(crate) fn compute_effective(e: &Element) -> BTreeMap<u32, PropertyValue> {
        let mut m: BTreeMap<u32, PropertyValue> = BTreeMap::new();
        for p in &e.desc.block.properties.entries {
            m.insert(p.pid, p.value.clone());
        }
        if e.state != 0
            && let Some(s) = e.desc.state(e.state)
        {
            for p in &s.properties.entries {
                m.insert(p.pid, p.value.clone());
            }
        }
        for (p, v) in &e.runtime {
            m.insert(*p, v.clone());
        }
        m
    }

    /// Recomputes the effective properties and reports each that changed (or disappeared)
    /// to the class (`ui-layout.md` §9.4 step 5). Before post-creation nothing is reported.
    pub(crate) fn refresh_effective(&mut self, k: ElementKey) {
        let Some(e) = self.el_mut(k) else { return };
        let new = Self::compute_effective(e);
        let old = std::mem::replace(&mut e.effective, new);
        if !e.created {
            return;
        }
        let mut changed: Vec<u32> = Vec::new();
        for (p, v) in &e.effective {
            if old.get(p) != Some(v) {
                changed.push(*p);
            }
        }
        for p in old.keys() {
            if !e.effective.contains_key(p) {
                changed.push(*p);
            }
        }
        changed.sort_unstable();
        self.note_paint_dirty(k);
        for p in changed {
            self.property_changed(k, p);
        }
    }

    // ----- sibling order and attachment --------------------------------------------------

    /// Inserts `child` into `parent`'s child list by `ui-layout.md` §9.3: walking from the
    /// end backwards past every child with a lower z-level, or the same z-level and a higher
    /// read order.
    pub(crate) fn insert_sorted(&mut self, parent: ElementKey, child: ElementKey) {
        let Some((z, ro)) = self.el(child).map(|e| (e.z, e.read_order)) else {
            return;
        };
        let kids = self
            .el(parent)
            .map(|e| e.children.clone())
            .unwrap_or_default();
        let mut pos = kids.len();
        while pos > 0 {
            let Some(s) = self.el(kids[pos - 1]) else {
                pos -= 1;
                continue;
            };
            if s.z < z || (s.z == z && s.read_order > ro) {
                pos -= 1;
            } else {
                break;
            }
        }
        if let Some(p) = self.el_mut(parent) {
            p.children.insert(pos, child);
        }
    }

    /// Moves `k` to the front-most position among its siblings with the same z-level
    /// (`ui-behaviour.md` §5.1 step 4): after the last sibling whose z-level is ≥ its own.
    pub(crate) fn bring_to_front(&mut self, k: ElementKey) {
        let Some((parent, z)) = self.el(k).and_then(|e| e.parent.map(|p| (p, e.z))) else {
            return;
        };
        let Some(mut kids) = self.el(parent).map(|e| e.children.clone()) else {
            return;
        };
        kids.retain(|c| *c != k);
        let mut pos = 0;
        for (i, c) in kids.iter().enumerate() {
            if self.el(*c).is_some_and(|s| s.z >= z) {
                pos = i + 1;
            }
        }
        kids.insert(pos, k);
        if let Some(p) = self.el_mut(parent) {
            p.children = kids;
        }
        self.input.hover_dirty = true;
        self.mark_dirty(k);
    }

    /// Changes an element's z-level and re-inserts it (`lua-plugin-api.md` §13.14).
    pub(crate) fn set_z(&mut self, k: ElementKey, z: i32) {
        let Some(e) = self.el_mut(k) else { return };
        if e.z == z {
            return;
        }
        e.z = z;
        let parent = e.parent;
        if let Some(p) = parent {
            if let Some(pe) = self.el_mut(p) {
                pe.children.retain(|c| *c != k);
            }
            self.insert_sorted(p, k);
        }
        self.input.hover_dirty = true;
        self.mark_dirty(k);
    }

    /// The depth of an element below the topmost ancestor (0 for a root).
    pub(crate) fn depth(&self, k: ElementKey) -> usize {
        let mut d = 0;
        let mut cur = self.el(k).and_then(|e| e.parent);
        while let Some(c) = cur {
            d += 1;
            if d > MAX_TREE_DEPTH * 4 {
                break;
            }
            cur = self.el(c).and_then(|e| e.parent);
        }
        d
    }

    fn subtree_height(&self, k: ElementKey) -> usize {
        let mut best = 0;
        let mut stack = vec![(k, 0usize)];
        while let Some((c, d)) = stack.pop() {
            best = best.max(d);
            if d > MAX_TREE_DEPTH * 2 {
                break;
            }
            if let Some(e) = self.el(c) {
                for ch in &e.children {
                    stack.push((*ch, d + 1));
                }
            }
        }
        best
    }

    /// Whether `a` is `b` or an ancestor of `b`.
    #[must_use]
    pub fn is_ancestor_or_self(&self, a: ElementKey, b: ElementKey) -> bool {
        let mut cur = Some(b);
        let mut n = 0;
        while let Some(c) = cur {
            if c == a {
                return true;
            }
            n += 1;
            if n > MAX_TREE_DEPTH * 4 {
                return false;
            }
            cur = self.el(c).and_then(|e| e.parent);
        }
        false
    }

    /// Attaches `child` (without a parent) to `parent` and lays it out (`ui-layout.md` §9.3,
    /// §8.4).
    pub(crate) fn attach_raw(
        &mut self,
        child: ElementKey,
        parent: ElementKey,
    ) -> Result<(), UiError> {
        if !self.exists(child) || !self.exists(parent) {
            return Err(UiError::NoSuchElement);
        }
        if self.is_ancestor_or_self(child, parent) {
            return Err(UiError::Cycle);
        }
        if self.depth(parent) + 1 + self.subtree_height(child) > MAX_TREE_DEPTH {
            return Err(UiError::TooDeep);
        }
        if self.el(child).is_some_and(|e| e.parent.is_some()) {
            self.detach_raw(child);
        }
        if let Some(e) = self.el_mut(child) {
            e.parent = Some(parent);
        }
        self.insert_sorted(parent, child);
        self.layout_element(child);
        self.input.hover_dirty = true;
        self.mark_dirty(child);
        Ok(())
    }

    /// Removes `k` from its parent's child list (the element stays alive). A list box
    /// parent drops it from its entries, and a tree parent drops its node (destroying the
    /// node's icon part).
    pub(crate) fn detach_raw(&mut self, k: ElementKey) {
        let Some(parent) = self.el(k).and_then(|e| e.parent) else {
            return;
        };
        self.mark_dirty(k);
        let mut tree_fix = None;
        if let Some(p) = self.el_mut(parent) {
            p.children.retain(|c| *c != k);
            if let Some(l) = p.widget.list.as_mut() {
                l.entries.retain(|c| *c != k);
                l.dirty = true;
            }
            if let Some(t) = p.widget.tree.as_mut() {
                let r = crate::widgets::listbox::tree_remove_nodes(t, |x| x == k);
                if !r.is_empty() {
                    tree_fix = Some(r);
                }
            }
        }
        if let Some(e) = self.el_mut(k) {
            e.parent = None;
        }
        self.input.hover_dirty = true;
        if let Some(r) = tree_fix {
            self.tree_after_removal(parent, r);
        }
    }

    /// Destroys an element and its subtree. References to them (hover, capture, focus,
    /// activation, listeners, list entries, tree nodes) are dropped in one pass over the
    /// UI, so destroying *m* elements costs O(*m* + *N*) for *N* live elements. A modal
    /// dialog's container goes with its last dialog.
    pub fn destroy(&mut self, k: ElementKey) {
        self.destroy_many(&[k]);
    }

    /// Destroys several elements and their subtrees as [`Ui::destroy`] does, with one pass
    /// over the UI for all of them: O(*m* + *N*) for *m* destroyed elements and *N* live
    /// ones, however many roots `keys` names. Keys that are gone, repeated, the screen
    /// root, or below another key of the list are fine.
    pub fn destroy_many(&mut self, keys: &[ElementKey]) {
        let mut doomed: HashSet<ElementKey> = HashSet::new();
        let mut all = Vec::new();
        let mut old_parents = Vec::new();
        for &k in keys {
            if k == self.screen || !self.exists(k) || doomed.contains(&k) {
                continue;
            }
            old_parents.push(self.parent(k));
            self.detach_raw(k);
            let mut stack = vec![k];
            while let Some(c) = stack.pop() {
                if doomed.contains(&c) {
                    continue;
                }
                if let Some(e) = self.el(c) {
                    stack.extend(e.children.iter().copied());
                    doomed.insert(c);
                    all.push(c);
                }
            }
        }
        if all.is_empty() {
            return;
        }
        let tree_fixes = self.forget_all(&all, &doomed);
        for c in all {
            if self.arena.remove(c).is_some() {
                self.painter.forget(c);
            }
        }
        for (tree, r) in tree_fixes {
            self.tree_after_removal(tree, r);
        }
        for p in old_parents.into_iter().flatten() {
            if self
                .el(p)
                .is_some_and(|e| e.dialog_container && e.children.is_empty())
            {
                self.destroy(p);
            }
        }
    }

    /// Drops every reference the UI keeps to the elements of `all` (before they are
    /// removed): per-key state first, then one pass over the arena. Returns the trees that
    /// lost nodes, with what is left to repair.
    fn forget_all(
        &mut self,
        all: &[ElementKey],
        doomed: &HashSet<ElementKey>,
    ) -> Vec<(ElementKey, crate::widgets::listbox::TreeRemoval)> {
        for k in all {
            let k = *k;
            self.input.forget(k);
            self.tooltip_forget(k);
            self.drag_forget(k);
            self.band_pop_all(k);
            self.lists.remove(&k);
            self.frame_set.remove(&k);
            self.saved.registered.remove(&k);
        }
        self.focus_forget_set(doomed);
        self.subs.forget_set(doomed);
        let gone = |x: &ElementKey| doomed.contains(x);
        let mut fixes = Vec::new();
        for (key, e) in self.arena.iter_mut() {
            if doomed.contains(&key) {
                continue;
            }
            e.listeners.retain(|l| !gone(l));
            if let Some(s) = e.widget.scroll.as_mut() {
                if s.vbar.as_ref().is_some_and(gone) {
                    s.vbar = None;
                }
                if s.hbar.as_ref().is_some_and(gone) {
                    s.hbar = None;
                }
                s.hover_pushes.retain(|h| !gone(h));
            }
            if let Some(l) = e.widget.list.as_mut() {
                l.entries.retain(|c| !gone(c));
                l.laid_out.retain(|c| !gone(c));
                if l.selected.as_ref().is_some_and(gone) {
                    l.selected = None;
                }
            }
            if let Some(m) = e.widget.meter.as_mut() {
                if m.fill.as_ref().is_some_and(gone) {
                    m.fill = None;
                }
                if m.fill_image.as_ref().is_some_and(gone) {
                    m.fill_image = None;
                }
            }
            if let Some(m) = e.widget.menu.as_mut()
                && m.popup.as_ref().is_some_and(gone)
            {
                m.popup = None;
                m.list = None;
            }
            if let Some(t) = e.widget.tree.as_mut() {
                let r = crate::widgets::listbox::tree_remove_nodes(t, |x| doomed.contains(&x));
                if !r.is_empty() {
                    fixes.push((key, r));
                }
            }
            if let Some(p) = e.plugin.as_mut()
                && p.focus_control.as_ref().is_some_and(gone)
            {
                p.focus_control = None;
            }
        }
        fixes
    }

    // ----- layout ------------------------------------------------------------------------

    /// The object mode an element stores, if any.
    pub(crate) fn object_mode(&self, k: ElementKey) -> Option<u32> {
        self.prop(k, self.v.pids.object_mode)
            .set
            .map(crate::props::as_key)
    }

    /// The reference boxes (*O*, *N*) of `ui-layout.md` §8.3.
    pub(crate) fn reference_boxes(&self, k: ElementKey) -> (IBox, IBox) {
        let Some(e) = self.el(k) else {
            return (IBox::default(), IBox::default());
        };
        let parent = e.parent.and_then(|p| self.el(p));
        match parent {
            Some(p) if !e.screen_level => match e.ref_box {
                Some(rb) => (rb, p.bx),
                None => (p.design, p.bx),
            },
            Some(_) if e.screen_level && e.parent != Some(self.screen) => {
                // A screen-level element below another element (a modal dialog inside its
                // container): the screen-level boxes still apply.
                (
                    IBox::from_xywh(0, 0, e.layout_size.0, e.layout_size.1),
                    IBox::from_xywh(0, 0, self.screen_size.0, self.screen_size.1),
                )
            }
            _ => (
                IBox::from_xywh(0, 0, e.layout_size.0, e.layout_size.1),
                IBox::from_xywh(0, 0, self.screen_size.0, self.screen_size.1),
            ),
        }
    }

    /// Lays an element out against its parent (`ui-layout.md` §8.4).
    pub(crate) fn layout_element(&mut self, k: ElementKey) {
        let Some(e) = self.el(k) else { return };
        if e.plugin.is_some() {
            // Plugin controls follow `lua-plugin-api.md` §13.16, never §8.4.
            return;
        }
        // Step 1 (Tessera choice: only under a root-panel parent, §8.4 scope OPEN).
        if self.object_mode(k).is_none()
            && let Some(p) = e.parent
            && self.el(p).is_some_and(|pe| pe.root_panel)
            && self.object_mode(p).unwrap_or(object_mode::CANVAS_RESIZE) == object_mode::STRETCH
        {
            return;
        }
        let (old, new) = self.reference_boxes(k);
        let Some(e) = self.el(k) else { return };
        let inp = RuntimeInput {
            design: e.design,
            current: e.bx,
            edges: e.edges,
            old,
            new,
            finished: e.created,
            keep_inside: e.flags & 0x02 != 0,
        };
        let b = runtime(&inp);
        self.set_layout_box(k, b);
    }

    /// Sets the layout box (step 6 of §8.4, moves and resizes): when the size changed the
    /// children are laid out again and the class is told; plugin notifications are queued.
    pub(crate) fn set_layout_box(&mut self, k: ElementKey, b: IBox) {
        self.set_layout_box_with(k, b, true);
    }

    pub(crate) fn set_layout_box_with(&mut self, k: ElementKey, b: IBox, notify_size: bool) {
        let b = b.clamped();
        let Some(e) = self.el(k) else { return };
        let old = e.bx;
        if old == b {
            return;
        }
        self.mark_dirty(k);
        if let Some(e) = self.el_mut(k) {
            e.bx = b;
        }
        self.mark_dirty(k);
        self.input.hover_dirty = true;
        let size_changed = old.w() != b.w() || old.h() != b.h();
        let moved = old.l != b.l || old.t != b.t;
        if size_changed {
            self.relayout_children(k);
            self.class_resized(k);
        }
        let is_plugin = self.el(k).is_some_and(|e| e.plugin.is_some());
        if is_plugin {
            if size_changed && notify_size && !self.suppress_size {
                self.post(
                    k,
                    crate::vocab::tessera_msg::SIZE_CHANGED,
                    Default::default(),
                );
            }
            if moved || size_changed {
                // A position change on screen: the left or top edge moved.
                if moved {
                    self.post(
                        k,
                        crate::vocab::tessera_msg::POSITION_CHANGED,
                        Default::default(),
                    );
                }
            }
        }
    }

    /// Lays out every child again (`ui-layout.md` §8.4: "whenever that element's size
    /// changes"); plugin controls with attachments use `lua-plugin-api.md` §13.16.
    pub(crate) fn relayout_children(&mut self, k: ElementKey) {
        let kids = self.children(k);
        for c in kids {
            if self.el(c).is_some_and(|e| e.plugin.is_some()) {
                self.attachment_layout(c, false);
            } else {
                self.layout_element(c);
            }
        }
    }

    /// Assigns (or removes) a code reference box (`ui-layout.md` §8.3, second row: the
    /// element is laid out from this box to its parent's current box) and lays it out.
    pub fn set_reference_box(&mut self, k: ElementKey, b: Option<IBox>) {
        if let Some(e) = self.el_mut(k) {
            e.ref_box = b;
        }
        self.layout_element(k);
    }

    /// Moves an element to (`x`, `y`) relative to its parent, keeping its size.
    pub fn move_to(&mut self, k: ElementKey, x: i32, y: i32) {
        let Some(e) = self.el(k) else { return };
        let b = e.bx;
        self.set_layout_box(k, IBox::from_xywh(x, y, b.w(), b.h()));
    }

    /// Whether the element owns a drawing surface (decision 0015 Decision 1.1; stand-in
    /// surfaces excluded).
    #[must_use]
    pub fn owns_surface(&self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return false };
        let wants = match self.object_mode(k) {
            Some(m) => m != object_mode::NONE,
            None => e.root_panel,
        } || e.widget.meter.as_ref().is_some_and(|m| m.alpha_test)
            || e.drag_icon
            || self.tooltip.panel == Some(k)
            || self.tooltip.extra.contains(&k);
        let shown = e.visible || self.prop(k, self.v.pids.keep_object_when_hidden).bool();
        wants && shown
    }

    /// The object mode of a surface-owning element (CanvasResize when it has no value).
    pub(crate) fn surface_mode(&self, k: ElementKey) -> u32 {
        self.object_mode(k).unwrap_or(object_mode::CANVAS_RESIZE)
    }

    /// The element resize of `ui-behaviour.md` §9.2.1: limit pass, parent scale, then by
    /// object mode.
    pub fn resize(&mut self, k: ElementKey, w: i32, h: i32) {
        let Some((mut w, mut h)) = self.el(k).map(|_| (w, h)) else {
            return;
        };
        // Step 1: limit pass.
        let p = self.v.pids;
        let (maxh, minh, maxw, minw) = (
            self.prop(k, p.max_height),
            self.prop(k, p.min_height),
            self.prop(k, p.max_width),
            self.prop(k, p.min_width),
        );
        let mut height_limited = false;
        let mut width_limited = false;
        if maxh.is_set() && h > maxh.int() {
            h = maxh.int();
            height_limited = true;
        }
        if minh.is_set() && h < minh.int() {
            h = minh.int();
            height_limited = true;
        }
        if maxw.is_set() && w > maxw.int() {
            w = maxw.int();
            width_limited = true;
        }
        if minw.is_set() && w < minw.int() {
            w = minw.int();
            width_limited = true;
        }
        let ratio = self.prop(k, p.resize_aspect_ratio);
        let ratio_v = ratio.float();
        if ratio.is_set() && ratio_v != 0.0 && ratio_v.is_finite() {
            let width_drives = !(height_limited && !width_limited);
            if width_drives {
                h = trunc_f32(w as f32 / ratio_v);
            } else {
                w = trunc_f32(h as f32 * ratio_v);
            }
        }
        // Step 2: parent scale for surface-owning elements with a parent.
        let owns = self.owns_surface(k);
        if owns && let Some(parent) = self.parent(k) {
            let mut surf = Some(parent);
            while let Some(s) = surf {
                if self.owns_surface(s) {
                    break;
                }
                surf = self.parent(s);
            }
            let parent_stretch = surf.is_some_and(|s| self.surface_mode(s) == object_mode::STRETCH);
            if parent_stretch || self.prop(k, p.force_match_parent_scale).bool() {
                let (px, py, _) = self.node_scale_at_or_above(parent);
                w = trunc_f32((w as f32 + 1.0) * px - 1.0);
                h = trunc_f32((h as f32 + 1.0) * py - 1.0);
            }
        }
        // Step 3: by object mode.
        let Some(e) = self.el(k) else { return };
        let b = e.bx;
        if owns {
            match self.surface_mode(k) {
                object_mode::STRETCH => {
                    let (cw, ch) = (b.w().max(1) as f32, b.h().max(1) as f32);
                    let s = [w as f32 / cw, h as f32 / ch, 1.0];
                    if let Some(e) = self.el_mut(k) {
                        e.scale = Some(s.map(|v| if v.is_finite() { v } else { 1.0 }));
                    }
                    // A new displayed box of a surface: composition input, not damage. The
                    // next paint with a change runs the full surface pass.
                    self.painter.note_structure();
                    self.input.hover_dirty = true;
                    return;
                }
                object_mode::TILE => return,
                object_mode::NONE => {}
                _ => {}
            }
        }
        self.set_layout_box(k, IBox::from_xywh(b.l, b.t, w, h));
    }

    // ----- displayed boxes ---------------------------------------------------------------

    /// The scale of the nearest display node at or above `k` (`ui-behaviour.md` §4.1 step 3,
    /// UB46): (1, 1, 1) when none.
    #[must_use]
    pub fn node_scale_at_or_above(&self, k: ElementKey) -> (f32, f32, f32) {
        let mut cur = Some(k);
        let mut n = 0;
        while let Some(c) = cur {
            let Some(e) = self.el(c) else { break };
            if let Some(s) = e.scale {
                return (s[0], s[1], s[2]);
            }
            cur = e.parent;
            n += 1;
            if n > MAX_TREE_DEPTH * 4 {
                break;
            }
        }
        (1.0, 1.0, 1.0)
    }

    /// The displayed box of `k` relative to its parent's displayed box
    /// (`ui-behaviour.md` §9.2.1).
    #[must_use]
    pub fn displayed_rel(&self, k: ElementKey) -> IBox {
        let Some(e) = self.el(k) else {
            return IBox::default();
        };
        let (px, py) = match e.parent {
            Some(p) => {
                let s = self.node_scale_at_or_above(p);
                (s.0, s.1)
            }
            None => (1.0, 1.0),
        };
        let (ex, ey, _) = self.node_scale_at_or_above(k);
        displayed_from(e.bx, (px, py), (ex, ey))
    }

    /// The displayed box of `k` in screen pixels (made absolute through its ancestors'
    /// displayed boxes).
    #[must_use]
    pub fn displayed_box(&self, k: ElementKey) -> IBox {
        let mut b = self.displayed_rel(k);
        let mut cur = self.parent(k);
        let mut n = 0;
        while let Some(c) = cur {
            let r = self.displayed_rel(c);
            b = b.offset(r.l, r.t);
            cur = self.parent(c);
            n += 1;
            if n > MAX_TREE_DEPTH * 4 {
                break;
            }
        }
        b
    }

    /// The layout box made absolute through the ancestors' layout boxes (screen pixels when
    /// no scale applies).
    #[must_use]
    pub fn absolute_layout_box(&self, k: ElementKey) -> IBox {
        let Some(mut b) = self.layout_box(k) else {
            return IBox::default();
        };
        let mut cur = self.parent(k);
        let mut n = 0;
        while let Some(c) = cur {
            if let Some(p) = self.layout_box(c) {
                b = b.offset(p.l, p.t);
            }
            cur = self.parent(c);
            n += 1;
            if n > MAX_TREE_DEPTH * 4 {
                break;
            }
        }
        b
    }

    /// Marks an element's paint dirty (decision 0015 Decision 1.4).
    pub(crate) fn mark_dirty(&mut self, k: ElementKey) {
        self.note_paint_dirty(k);
        self.painter.touched = true;
    }

    /// Sets an element's paint-dirty flag and records it for the next damage pass, without
    /// asking for one (decision 0015 Decision 1.4).
    pub(crate) fn note_paint_dirty(&mut self, k: ElementKey) {
        if let Some(e) = self.arena.get_mut(k) {
            e.paint_dirty = true;
            self.painter.note_dirty(k);
        }
    }

    /// Registers the element for per-frame work.
    pub(crate) fn want_frame(&mut self, k: ElementKey) {
        if let Some(e) = self.el_mut(k) {
            e.frame_update = true;
            self.frame_set.insert(k);
        }
    }

    /// Inserts a new element into the arena, registering list boxes for the layout flush.
    pub(crate) fn insert_element(&mut self, e: Element) -> ElementKey {
        let is_list = e.widget.list.is_some();
        let k = self.arena.insert(e);
        if is_list {
            self.lists.insert(k);
        }
        k
    }

    /// Converts a point relative to an element's displayed box back to its unscaled pixels
    /// (`ui-behaviour.md` §9.2.1 "Points inside a scaled panel", FO §6.3 item 3: always
    /// divides).
    #[must_use]
    pub fn point_to_unscaled(&self, k: ElementKey, x: i32, y: i32) -> (i32, i32) {
        let (ex, ey, _) = self.node_scale_at_or_above(k);
        (
            trunc_f32(x as f32 / if ex == 0.0 { 1.0 } else { ex }),
            trunc_f32(y as f32 / if ey == 0.0 { 1.0 } else { ey }),
        )
    }
}

/// The displayed box of a layout box `b` under the parent scale (`px`, `py`) and the
/// element scale (`ex`, `ey`) (`ui-behaviour.md` §9.2.1 step 2), 32-bit floats.
#[must_use]
pub fn displayed_from(b: IBox, (px, py): (f32, f32), (ex, ey): (f32, f32)) -> IBox {
    if px == 1.0 && py == 1.0 && ex == 1.0 && ey == 1.0 {
        return b;
    }
    let l = trunc_f32(b.l as f32 * px);
    let t = trunc_f32(b.t as f32 * py);
    let w = trunc_f32(((b.w() as f32 + 1.0) * ex - 1.0).max(0.0));
    let h = trunc_f32(((b.h() as f32 + 1.0) * ey - 1.0).max(0.0));
    IBox::from_xywh(l, t, w, h)
}
