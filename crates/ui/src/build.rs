// SPDX-License-Identifier: GPL-3.0-or-later
//! Building native panels and element trees (`ui-layout.md` §9.1–§9.2) and the completion
//! pass (`ui-widgets.md` §3.4).

use std::collections::BTreeMap;
use std::sync::Arc;

use tessera_formats::did::Did;
use tessera_formats::ui_layout::{ElementDesc, UiLayout};

use crate::UiError;
use crate::resolve::resolve;
use crate::tree::{Element, ElementKey};
use crate::ui::{
    MAX_BUILD_ELEMENTS, MAX_CREATE_NESTING, MAX_ELEMENTS, MAX_TREE_DEPTH, Ui, object_mode,
};
use crate::widgets::WidgetClass;

/// Game types mapped onto engine classes by game code (`ui-layout.md` §9.2 step 3: game
/// types 0x1000xxxx are registered by game code).
#[derive(Debug, Clone, Default)]
pub(crate) struct GameTypes {
    pub map: BTreeMap<u32, WidgetClass>,
}

impl Ui {
    /// Registers the class a game element type is created with (for example the Tessera
    /// rule of `ui-widgets.md` §13 P3: `LevelProgressMeter` as an engine Meter). Types
    /// without a registration use the fallback Field and are counted in the census.
    pub fn register_game_type(&mut self, element_type: u32, class: WidgetClass) {
        self.game_types.map.insert(element_type, class);
    }

    fn lookup_layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        self.env.layout(did)
    }

    /// The class of a resolved type, or `None` when no factory exists (§9.2 step 3).
    fn factory(&mut self, element_type: u32) -> Option<WidgetClass> {
        if let Some(c) = self.v.class_of(element_type) {
            return Some(c);
        }
        if let Some(c) = self.game_types.map.get(&element_type) {
            return Some(*c);
        }
        if element_type >= 0x1000_0000 {
            *self.census.fallback_types.entry(element_type).or_insert(0) += 1;
            return Some(WidgetClass::Fallback(element_type));
        }
        *self.census.no_factory.entry(element_type).or_insert(0) += 1;
        None
    }

    /// Steps 1–6 of `ui-layout.md` §9.2 for one descriptor: resolve, apply the user skin,
    /// create through the factory, register, create the children (each with its
    /// post-creation before it is attached). Returns `None` when the element is not created.
    ///
    /// The depth bound is shared by nested creations: `build_depth` counts the build
    /// levels open in this and every enclosing creation, so a creation started from the
    /// post-creation of a deep element (a Menu's popup, a list template) only gets the
    /// levels that are left, and the call stack stays within [`MAX_TREE_DEPTH`] build levels
    /// however the creations nest.
    fn build_element(
        &mut self,
        desc: &ElementDesc,
        layout: (Did, (i32, i32)),
        path: &mut Vec<u32>,
    ) -> Option<ElementKey> {
        if self.arena.len() >= MAX_ELEMENTS {
            return None;
        }
        if self.build_depth > MAX_TREE_DEPTH {
            self.census.creation_refused += 1;
            tracing::debug!(
                element = desc.element_id,
                depth = self.build_depth,
                "element beyond the shared build depth not created"
            );
            return None;
        }
        self.build_depth += 1;
        let k = self.build_element_level(desc, layout, path);
        self.build_depth -= 1;
        k
    }

    /// One level of [`Ui::build_element`], with the shared depth already counted.
    fn build_element_level(
        &mut self,
        desc: &ElementDesc,
        layout: (Did, (i32, i32)),
        path: &mut Vec<u32>,
    ) -> Option<ElementKey> {
        if self.build_budget == 0 {
            self.census.creation_refused += 1;
            return None;
        }
        self.build_budget -= 1;
        // Step 1.
        let mut resolved = {
            let env = &mut self.env;
            match resolve(desc, &mut |d| env.layout(d)) {
                Ok(r) => r,
                Err(e) => {
                    *self
                        .census
                        .resolve_failures
                        .entry(e.category().name())
                        .or_insert(0) += 1;
                    tracing::debug!("element {:#x} not created: {e}", desc.element_id);
                    return None;
                }
            }
        };
        // Step 2: the user skin on the per-build resolved copy.
        if let Some(skin) = self.skin.clone() {
            skin.apply(&mut resolved, path);
        }
        // Step 3.
        let class = self.factory(resolved.element_type)?;
        // Step 4.
        let children = std::mem::take(&mut resolved.children);
        resolved.children_size.count = 0;
        let element_id = resolved.element_id;
        let mut e = Element::new(element_id, class, Arc::new(resolved));
        e.layout_did = layout.0;
        e.layout_size = layout.1;
        e.effective = Ui::compute_effective(&e);
        let k = self.insert_element(e);
        self.census.created += 1;
        // Step 5: the listener registry check.
        if self.registry_has_mouse(element_id)
            && let Some(e) = self.el_mut(k)
        {
            e.mouse_visible = true;
        }
        // Step 6.
        path.push(element_id);
        for c in &children {
            if let Some(ck) = self.build_element(c, layout, path) {
                self.post_create(ck);
                if self.attach_raw(ck, k).is_err() {
                    self.destroy(ck);
                }
            }
        }
        path.pop();
        Some(k)
    }

    fn top_level(
        &mut self,
        layout: Did,
        element_id: u32,
    ) -> Result<(Arc<UiLayout>, usize), UiError> {
        let l = self
            .lookup_layout(layout)
            .ok_or(UiError::NoLayout(layout.0))?;
        let i = l
            .top_level_index(element_id)
            .ok_or(UiError::NoElement(element_id))?;
        Ok((l, i))
    }

    /// "Create element" (`ui-layout.md` §9.1, DID form): creates the top-level element
    /// `element_id` of layout `layout` inside `parent` (or without one) by §9.2, runs its
    /// post-creation and the completion pass.
    ///
    /// # Errors
    /// [`UiError::NoLayout`], [`UiError::NoElement`], [`UiError::NotCreated`] (resolution
    /// failed or no factory), or an attachment error.
    ///
    /// A creation started while the same (layout, element) pair is being created further up
    /// (a Menu whose popup contains the menu, a list box whose template is the list), or
    /// nested deeper than [`MAX_CREATE_NESTING`], is refused with
    /// [`UiError::NotCreated`]; one outermost creation builds at most
    /// [`MAX_BUILD_ELEMENTS`] elements (counted in [`BuildCensus::creation_refused`]).
    ///
    /// [`BuildCensus::creation_refused`]: crate::BuildCensus::creation_refused
    pub fn create_element(
        &mut self,
        parent: Option<ElementKey>,
        layout: Did,
        element_id: u32,
    ) -> Result<ElementKey, UiError> {
        self.enter_creation(layout, element_id)?;
        let r = self.create_element_inner(parent, layout, element_id);
        self.leave_creation();
        r
    }

    /// The creation guard: refuses a pair already being created and nesting beyond the cap,
    /// and starts the build budget for an outermost creation.
    fn enter_creation(&mut self, layout: Did, element_id: u32) -> Result<(), UiError> {
        let pair = (layout.0, element_id);
        if self.creating.len() >= MAX_CREATE_NESTING || self.creating.contains(&pair) {
            self.census.creation_refused += 1;
            tracing::debug!(
                layout = layout.0,
                element = element_id,
                nesting = self.creating.len(),
                "nested element creation refused"
            );
            return Err(UiError::NotCreated(element_id));
        }
        if self.creating.is_empty() {
            self.build_budget = MAX_BUILD_ELEMENTS;
        }
        self.creating.push(pair);
        Ok(())
    }

    fn leave_creation(&mut self) {
        self.creating.pop();
    }

    fn create_element_inner(
        &mut self,
        parent: Option<ElementKey>,
        layout: Did,
        element_id: u32,
    ) -> Result<ElementKey, UiError> {
        let (l, i) = self.top_level(layout, element_id)?;
        let desc = &l.elements[i];
        let size = (
            l.design_width.min(i32::MAX as u32) as i32,
            l.design_height.min(i32::MAX as u32) as i32,
        );
        let mut path = Vec::new();
        let k = self
            .build_element(desc, (layout, size), &mut path)
            .ok_or(UiError::NotCreated(element_id))?;
        // Step 7, then step 8 for a top-level element.
        if let Some(p) = parent
            && let Err(e) = self.attach_raw(k, p)
        {
            self.destroy(k);
            return Err(e);
        }
        self.post_create(k);
        self.completion_pass(k);
        Ok(k)
    }

    /// "Create element" by `UILAYOUT` entry key (`ui-layout.md` §9.1, U31).
    ///
    /// # Errors
    /// [`UiError::NoLayoutKey`] when the map has no entry, else as
    /// [`Ui::create_element`].
    pub fn create_element_by_key(
        &mut self,
        parent: Option<ElementKey>,
        key: u32,
        element_id: u32,
    ) -> Result<ElementKey, UiError> {
        let did = self
            .env
            .layout_by_key(key)
            .ok_or(UiError::NoLayoutKey(key))?;
        self.create_element(parent, did, element_id)
    }

    /// "Open panel" by `UILAYOUT` entry key (`ui-layout.md` §9.1).
    ///
    /// # Errors
    /// [`UiError::NoLayoutKey`] when the map has no entry, else as [`Ui::open_panel`].
    pub fn open_panel_by_key(&mut self, key: u32, element_id: u32) -> Result<ElementKey, UiError> {
        let did = self
            .env
            .layout_by_key(key)
            .ok_or(UiError::NoLayoutKey(key))?;
        self.open_panel(did, element_id)
    }

    /// Opens a root panel (`ui-layout.md` §9.1 steps 1–5): builds the tree without a parent,
    /// runs post-creation, marks it screen-level with its own object mode, attaches it to the
    /// screen root (which lays it out from the layout's design size to the screen) and shows
    /// it. A dialog whose `UICore_Dialog_modal` is true is created inside a screen-sized
    /// container (a Field with edges (5, 5, 5, 5)), and gets CanvasResize when it has no
    /// object mode; the dialog override properties replace its layout and element.
    ///
    /// # Errors
    /// As [`Ui::create_element`].
    pub fn open_panel(&mut self, layout: Did, element_id: u32) -> Result<ElementKey, UiError> {
        self.enter_creation(layout, element_id)?;
        let r = self.open_panel_inner(layout, element_id);
        self.leave_creation();
        r
    }

    fn open_panel_inner(&mut self, layout: Did, element_id: u32) -> Result<ElementKey, UiError> {
        let requested = (layout, element_id);
        let (mut l, mut i) = self.top_level(layout, element_id)?;
        let mut layout = layout;
        let mut element_id = element_id;
        // Dialog overrides, read from the resolved descriptor.
        let (modal, lo, eo) = {
            let env = &mut self.env;
            match resolve(&l.elements[i], &mut |d| env.layout(d)) {
                Ok(r) => {
                    let p = self.v.pids;
                    let get = |pid| r.block.properties.get(pid);
                    (
                        get(p.dialog_modal).is_some_and(crate::props::as_bool),
                        get(p.dialog_layout_override)
                            .map(crate::props::as_key)
                            .unwrap_or(0),
                        get(p.dialog_element_override)
                            .map(crate::props::as_key)
                            .unwrap_or(0),
                    )
                }
                Err(_) => (false, 0, 0),
            }
        };
        if lo != 0 || eo != 0 {
            if lo != 0 {
                layout = Did(lo);
            }
            if eo != 0 {
                element_id = eo;
            }
            let (l2, i2) = self.top_level(layout, element_id)?;
            l = l2;
            i = i2;
        }
        if (layout, element_id) != requested {
            self.enter_creation(layout, element_id)?;
            let r = self.open_panel_build(&l, i, layout, element_id, modal);
            self.leave_creation();
            return r;
        }
        self.open_panel_build(&l, i, layout, element_id, modal)
    }

    /// Steps 1–5 of `ui-layout.md` §9.1 for the (possibly overridden) top-level element.
    fn open_panel_build(
        &mut self,
        l: &UiLayout,
        i: usize,
        layout: Did,
        element_id: u32,
        modal: bool,
    ) -> Result<ElementKey, UiError> {
        let size = (
            l.design_width.min(i32::MAX as u32) as i32,
            l.design_height.min(i32::MAX as u32) as i32,
        );
        let mut path = Vec::new();
        let k = self
            .build_element(&l.elements[i], (layout, size), &mut path)
            .ok_or(UiError::NotCreated(element_id))?;
        self.post_create(k);
        let parent = if modal {
            let c = match self.create_container() {
                Ok(c) => c,
                Err(e) => {
                    self.destroy(k);
                    return Err(e);
                }
            };
            if self.object_mode(k).is_none() {
                self.set_property(
                    k,
                    self.v.pids.object_mode,
                    tessera_formats::property::PropertyValue::Enum(object_mode::CANVAS_RESIZE),
                );
            }
            c
        } else {
            self.screen
        };
        if let Some(e) = self.el_mut(k) {
            e.screen_level = true;
            e.root_panel = true;
        }
        if modal {
            self.mark_dialog(k);
        }
        if let Err(e) = self.attach_raw(k, parent) {
            self.destroy(k);
            if parent != self.screen {
                self.destroy(parent);
            }
            return Err(e);
        }
        self.completion_pass(k);
        // Step 5: shown with the screen root; the show reactions of UB §11.2
        // (`activate_on_show`, `focus_on_show`) run now that the panel is displayed.
        if self.is_visible(self.screen) && !self.prop(k, self.v.pids.hide).bool() {
            if self.is_visible(k) {
                self.on_shown(k);
            } else {
                self.set_shown(k, true);
            }
        }
        self.panel_opened(k);
        Ok(k)
    }

    /// The modal-dialog container: a screen-sized, screen-level Field with edges
    /// (5, 5, 5, 5), no images and no text (no canvas, decision 0015 Decision 1.1). It is
    /// destroyed with its dialog ([`Ui::destroy`]).
    fn create_container(&mut self) -> Result<ElementKey, UiError> {
        if self.arena.len() >= MAX_ELEMENTS {
            return Err(UiError::TooManyElements);
        }
        let mut desc = ElementDesc {
            element_type: self.v.types.field,
            ..ElementDesc::default()
        };
        desc.block.rect.width = self.screen_size.0;
        desc.block.rect.height = self.screen_size.1;
        for e in [
            &mut desc.edges.left,
            &mut desc.edges.top,
            &mut desc.edges.right,
            &mut desc.edges.bottom,
        ] {
            e.0 = 5;
        }
        let mut e = Element::new(0, WidgetClass::Field, Arc::new(desc));
        e.screen_level = true;
        e.layout_size = self.screen_size;
        e.created = true;
        e.dialog_container = true;
        let k = self.insert_element(e);
        if let Err(err) = self.attach_raw(k, self.screen) {
            self.destroy(k);
            return Err(err);
        }
        Ok(k)
    }

    /// The completion pass (`ui-widgets.md` §3.4): post-order over the tree, each class
    /// acting after its children.
    pub(crate) fn completion_pass(&mut self, k: ElementKey) {
        self.completion_depth(k, 0);
    }

    fn completion_depth(&mut self, k: ElementKey, depth: usize) {
        if depth > MAX_TREE_DEPTH {
            return;
        }
        for c in self.children(k) {
            self.completion_depth(c, depth + 1);
        }
        self.class_completion(k);
    }

    /// Activatables loop (`ui-layout.md` §9.1, U32): opens as root panels the top-level
    /// elements of `layout` whose resolved `UI_Element_CreateAsRootElement` is true, in
    /// table order. Returns the panels opened.
    pub fn open_activatables(&mut self, layout: Did) -> Vec<ElementKey> {
        let Some(l) = self.lookup_layout(layout) else {
            return Vec::new();
        };
        let pid = self.v.pids.create_as_root;
        let mut out = Vec::new();
        for desc in &l.elements {
            let flag = {
                let env = &mut self.env;
                resolve(desc, &mut |d| env.layout(d))
                    .ok()
                    .and_then(|r| r.block.properties.get(pid).map(crate::props::as_bool))
                    .unwrap_or(false)
            };
            if flag && let Ok(k) = self.open_panel(layout, desc.element_id) {
                out.push(k);
            }
        }
        out
    }
}
