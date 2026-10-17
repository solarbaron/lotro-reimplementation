// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.UI` and `Turbine.UI.Lotro` bindings: thin classes over `tessera-ui` elements
//! (`lua-plugin-api.md` §13–§14; decision 0015 §5; decision 0023, the `plugins` → `ui`
//! allowance).
//!
//! # Wiring
//!
//! The client owns the [`tessera_ui::Ui`] in an `Rc<RefCell<…>>` and wraps it in a
//! [`PluginUi`] on the thread that drives the [`PluginManager`]. Its [`UiBinding`] (a
//! plain number, `Send`) goes into the client's [`HostApi`](crate::HostApi), whose
//! [`install_natives`](crate::HostApi::install_natives) calls [`install`] for every new
//! apartment. Then, per frame (ARCHITECTURE §20.3, decision 0014 D8):
//!
//! - **Step 1.** After each [`Ui::offer`](tessera_ui::Ui::offer) (and after any other UI
//!   call the client makes), [`PluginUi::deliver`] drains the UI outbox: plugin
//!   notifications become Lua events, each handler call one host call with its own
//!   instruction budget; every other [`UiEvent`] is returned to the client (game-code
//!   handlers, sounds, the key broadcast, …). [`PluginUi::set_modifiers`] keeps the
//!   modifier state of `Control.IsAltKeyDown()` and the key events current.
//! - **Step 7.** [`Ui::tick_hover`](tessera_ui::Ui::tick_hover), then
//!   [`PluginUi::deliver`] (the `MouseHover` events come before the broadcast, ARCHITECTURE
//!   §20.3), then [`PluginUi::update_tick`] (the `Update` broadcast, §9.4), then
//!   [`Ui::frame_work`](tessera_ui::Ui::frame_work) and [`PluginUi::deliver`] again.
//! - After any other entry into plugin code (the manager's pump, chat commands, chat
//!   dispatch, loads), call [`PluginUi::deliver`] as well.
//! - Chat commands that quickslot `Alias` shortcuts run (§14.6) are collected by
//!   [`PluginUi::take_chat_commands`]; the client runs them like typed commands, through
//!   its command dispatcher (plugin commands through [`PluginManager::run_command`]).
//! - Plugin list boxes and trees are laid out lazily: once before any plugin call that may
//!   read geometry, and at the end of [`PluginUi::deliver`] and [`PluginUi::update_tick`].
//!   A client that runs plugin code without a following `deliver` calls
//!   [`PluginUi::flush_layouts`] before its next [`Ui::offer`](tessera_ui::Ui::offer) or
//!   paint, so hit tests and drawing see the current list geometry.
//!
//! Inside a plugin call, the events a call causes are delivered before it returns when
//! no notification is being delivered, and after the current one otherwise (§9.7). Events
//! of another apartment met during such a synchronous delivery wait for the next
//! [`PluginUi::deliver`] (a recorded Tessera difference).
//!
//! # Lifetime of plugin objects
//!
//! A control stays alive while plugin code references it, while it is a child of a live
//! control, list or tree, while it is a shown top-level window, or while it receives the
//! `Update` or key broadcasts; an open context menu stays alive with its items. A control
//! that none of these keep is destroyed when its Lua table is collected, together with
//! the descendants collected with it; a descendant that is still alive (referenced, or a
//! broadcast subscriber) is detached and lives on without a parent (a Tessera rule;
//! `lua-plugin-api.md` §8.2 keeps identity, not lifetime). Every object of an apartment is
//! destroyed when it unloads.
//!
//! A parent keeps its children's tables through a weak-keyed table. Lua 5.1 has no
//! ephemeron tables, so a hidden window that plugin code no longer references is still
//! kept when one of its descendants' tables refers back to it (for example a `Click`
//! handler closure that captures the window); it is destroyed when its apartment unloads
//! (recorded in `lua-plugin-api.md` §8.2 OPEN and the compatibility table).
//!
//! # Compatibility table (exit criterion 4)
//!
//! The incompatibilities found with the synthetic plugin suite are listed in the module
//! documentation of `tests/compat_plugins.rs`.

mod colour;
mod control;
mod enums;
mod events;
mod lists;
mod menu;
mod model;
mod reg;
mod scroll;
mod shortcut;
mod state;
mod text;
mod window;

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Table};
use tessera_ui::{ElementKey, Ui, UiEvent};

use crate::api::NativeApi;
use crate::manager::PluginManager;
use state::State;

pub use shortcut::GameShortcut;
pub use state::{MAX_EVENTS_PER_DRAIN, MAX_OBJECTS_PER_APARTMENT, MAX_QUEUED_EVENTS};
pub use text::MAX_TEXT_BYTES_PER_APARTMENT;

/// The modifier keys held, as the client's input layer reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Alt.
    pub alt: bool,
    /// Control.
    pub control: bool,
    /// Shift.
    pub shift: bool,
}

/// Property IDs of the text properties the `Label` members write
/// (`lua-plugin-api.md` §13.4, `fonts.md` §7.3, §7.4, §8.4, §9.1). Labels are
/// property-dictionary names; the client resolves them with [`TextPids::resolve_with`].
/// Until then the values are Tessera placeholders (0xFFFC_00xx) that no dictionary uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPids {
    /// `UICore_Text_font` (DID).
    pub font: u32,
    /// `UICore_Text_font_color` (colour).
    pub font_color: u32,
    /// `UICore_Text_font_outline_color` (colour).
    pub outline_color: u32,
    /// `UICore_Text_outline` (Bool).
    pub outline: u32,
    /// `UICore_Text_horizontal_justification` (enum).
    pub horizontal: u32,
    /// `UICore_Text_vertical_justification` (enum).
    pub vertical: u32,
    /// `UICore_Text_one_line` (Bool).
    pub one_line: u32,
    /// `UICore_Text_parse_markup_tags` (Bool).
    pub markup: u32,
}

impl Default for TextPids {
    fn default() -> Self {
        Self {
            font: 0xFFFC_0001,
            font_color: 0xFFFC_0002,
            outline_color: 0xFFFC_0003,
            outline: 0xFFFC_0004,
            horizontal: 0xFFFC_0005,
            vertical: 0xFFFC_0006,
            one_line: 0xFFFC_0007,
            markup: 0xFFFC_0008,
        }
    }
}

impl TextPids {
    /// Every (label, value) pair.
    #[must_use]
    pub fn entries(&self) -> [(&'static str, u32); 8] {
        [
            ("UICore_Text_font", self.font),
            ("UICore_Text_font_color", self.font_color),
            ("UICore_Text_font_outline_color", self.outline_color),
            ("UICore_Text_outline", self.outline),
            ("UICore_Text_horizontal_justification", self.horizontal),
            ("UICore_Text_vertical_justification", self.vertical),
            ("UICore_Text_one_line", self.one_line),
            ("UICore_Text_parse_markup_tags", self.markup),
        ]
    }

    /// Replace each value whose label `lookup` resolves.
    pub fn resolve_with(&mut self, lookup: &mut dyn FnMut(&str) -> Option<u32>) {
        let fields: [(&str, &mut u32); 8] = [
            ("UICore_Text_font", &mut self.font),
            ("UICore_Text_font_color", &mut self.font_color),
            ("UICore_Text_font_outline_color", &mut self.outline_color),
            ("UICore_Text_outline", &mut self.outline),
            ("UICore_Text_horizontal_justification", &mut self.horizontal),
            ("UICore_Text_vertical_justification", &mut self.vertical),
            ("UICore_Text_one_line", &mut self.one_line),
            ("UICore_Text_parse_markup_tags", &mut self.markup),
        ];
        for (label, slot) in fields {
            if let Some(v) = lookup(label) {
                *slot = v;
            }
        }
    }
}

/// The token a [`HostApi`](crate::HostApi) keeps to install the UI classes into new
/// apartments with [`install`]. It names a [`PluginUi`] of the same thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiBinding(u64);

/// The plugin UI layer of one client: the binding state between the plugin manager and
/// the client's [`Ui`]. Not `Send`: it lives on the thread that owns both.
#[derive(Debug)]
pub struct PluginUi {
    shared: Rc<state::Shared>,
}

impl Drop for PluginUi {
    fn drop(&mut self) {
        state::unregister(self.shared.id);
    }
}

impl PluginUi {
    /// The layer over `ui`, registered on the current thread.
    #[must_use]
    pub fn new(ui: Rc<RefCell<Ui>>) -> Self {
        Self {
            shared: state::register(ui),
        }
    }

    /// The token for [`install`].
    #[must_use]
    pub fn binding(&self) -> UiBinding {
        UiBinding(self.shared.id)
    }

    /// The UI.
    #[must_use]
    pub fn ui(&self) -> &Rc<RefCell<Ui>> {
        &self.shared.ui
    }

    /// Set the property IDs of the text properties (resolved by the client).
    pub fn set_text_pids(&self, pids: TextPids) {
        self.shared.st.borrow_mut().text_pids = pids;
    }

    /// Set the modifier keys held now.
    pub fn set_modifiers(&self, modifiers: Modifiers) {
        self.shared.st.borrow_mut().modifiers = modifiers;
    }

    /// Drain the UI outbox: plugin notifications (and the events of other apartments left
    /// over from synchronous deliveries) are delivered as Lua events through `manager`;
    /// everything else is returned to the client in order. The client must not hold a
    /// borrow of the UI while calling this.
    pub fn deliver(&self, manager: &PluginManager) -> Vec<UiEvent> {
        let sh = &self.shared;
        sh.reap_all(manager);
        lists::flush(sh);
        {
            let mut st = sh.st.borrow_mut();
            let mut q = std::mem::take(&mut st.foreign);
            q.extend(std::mem::take(&mut st.queue));
            st.queue = q;
        }
        sh.drain(&state::ManagerSink { manager });
        sh.reap_all(manager);
        self.repin(manager);
        lists::flush(sh);
        std::mem::take(&mut sh.st.borrow_mut().client)
    }

    /// Evaluate the queued pins (`State::repin`) in their apartments' Lua states.
    fn repin(&self, manager: &PluginManager) {
        let sh = &self.shared;
        let aps: Vec<String> = {
            let st = sh.st.borrow();
            let mut v: Vec<String> = st
                .repin
                .iter()
                .filter_map(|id| st.objs.get(id).map(|o| o.apartment.clone()))
                .collect();
            v.sort();
            v.dedup();
            v
        };
        for ap in aps {
            manager.with_lua(&ap, |lua| sh.process_repins(lua, &ap));
        }
        // Entries whose apartment is gone, or whose record is.
        let mut st = sh.st.borrow_mut();
        let State { repin, objs, .. } = &mut *st;
        repin.retain(|id| objs.contains_key(id));
        if repin.len() > state::MAX_REPIN {
            repin.clear();
        }
    }

    /// The `Update` broadcast of frame-loop step 7 (§9.4): every subscribed control in
    /// subscription order, each handler call one host call; calls made by the handlers
    /// deliver their notifications synchronously.
    pub fn update_tick(&self, manager: &PluginManager) {
        let sh = &self.shared;
        sh.reap_all(manager);
        lists::flush(sh);
        let sink = state::ManagerSink { manager };
        sh.broadcast_updates(&sink);
        lists::flush(sh);
    }

    /// Lay out the plugin list boxes and trees whose layout is out of date (see the
    /// Wiring section). [`Self::deliver`] and [`Self::update_tick`] do it themselves.
    pub fn flush_layouts(&self) {
        lists::flush(&self.shared);
    }

    /// The chat commands run by quickslot `Alias` shortcuts since the last call.
    pub fn take_chat_commands(&self) -> Vec<String> {
        std::mem::take(&mut self.shared.st.borrow_mut().commands)
    }

    /// The game actions of plugin quickslots activated since the last call
    /// (`lua-plugin-api.md` §15.2 "Quickslot shortcuts"), in order: the client sends
    /// `UseSkill { what: Ref::Record(did), target }` for [`GameShortcut::Skill`] and
    /// `UseItem { item, target }` for [`GameShortcut::Item`], with the current selection
    /// as target. At most 256 wait; further activations are dropped.
    pub fn take_game_actions(&self) -> Vec<GameShortcut> {
        std::mem::take(&mut self.shared.st.borrow_mut().game_actions)
    }

    /// The content of the game's quickslot-bar slot a drag started on: the client calls
    /// this after [`Self::deliver`] returned the drag's
    /// [`UiEvent::DragBegan`] for such a slot, with
    /// `None` for a literal skill or an empty slot. A plugin control's `DragDrop` event
    /// then carries it as a new `Shortcut` (`Lotro.DragDropInfo:GetShortcut()`, §15.2).
    /// Every new drag clears it.
    pub fn set_game_drag(&self, content: Option<GameShortcut>) {
        self.shared.st.borrow_mut().game_drag = content.map(|g| {
            let (kind, data) = g.to_type_and_data();
            shortcut::ShortcutData { kind, data }
        });
    }

    /// The game content of the shortcut held by the current drag's source, when it is a
    /// plugin quickslot with a well-formed `Skill` or `Item` shortcut: the client asks
    /// this when such a drag ends on a slot of the game's quickslot bar and sends
    /// `SetQuickslot { slot, content }` (§15.2).
    #[must_use]
    pub fn drag_source_shortcut(&self) -> Option<GameShortcut> {
        shortcut::drag_source_game_shortcut(&self.shared.st.borrow())
    }

    /// A scaling group's value changed (the optional per-window scale of `ui-scaling.md`
    /// §7.4): records it for `Display.GetUIGroupScale`, re-scales the marked windows that
    /// follow the group, and fires `UIScaleChanged` (§13.21 rule 8) on every marked window
    /// of the group. The `Global` group (60) is ignored, because Tessera's UI scale lies
    /// below the UI-unit space and is 1.0 for plugins (decision 0040, §7.7); so is a
    /// non-finite or non-positive value. The events are delivered at once, like a
    /// [`Self::deliver`] call, whose client events (everything waiting in the UI outbox)
    /// are returned for the client to handle.
    #[must_use = "the client events of the delivery are returned"]
    pub fn scale_changed(&self, manager: &PluginManager, group: u32, value: f32) -> Vec<UiEvent> {
        window::scale_changed(&self.shared, group, value);
        self.deliver(manager)
    }

    /// The element of a plugin object table of `lua` (tests, diagnostics).
    #[must_use]
    pub fn element_of(&self, lua: &Lua, obj: &Table) -> Option<ElementKey> {
        let id = state::obj_id(lua, obj)?;
        self.shared.key_of(id)
    }

    /// The elements of the controls of `apartment` that are top-level windows, in creation
    /// order.
    #[must_use]
    pub fn windows(&self, apartment: &str) -> Vec<ElementKey> {
        let st = self.shared.st.borrow();
        let mut v: Vec<(u64, ElementKey)> = st
            .objs
            .iter()
            .filter(|(_, o)| o.apartment == apartment)
            .filter_map(|(id, o)| match &o.data {
                model::ObjData::Ctl(c) if c.kind == model::Kind::Window && c.internal.is_none() => {
                    Some((*id, c.key))
                }
                _ => None,
            })
            .collect();
        v.sort_by_key(|(id, _)| *id);
        let Ok(ui) = self.shared.ui.try_borrow() else {
            return Vec::new();
        };
        v.into_iter()
            .map(|(_, k)| k)
            .filter(|k| ui.control_parent(*k).is_none())
            .collect()
    }

    /// Number of records (controls and other objects) of `apartment`; records whose
    /// tables were collected count until the next [`Self::deliver`] removes them.
    #[must_use]
    pub fn object_count(&self, apartment: &str) -> usize {
        self.shared
            .st
            .borrow()
            .per_apartment
            .get(apartment)
            .copied()
            .unwrap_or(0)
    }

    /// Events dropped because the queue was full (diagnostics).
    #[must_use]
    pub fn dropped_events(&self) -> u64 {
        self.shared.st.borrow().dropped_events
    }
}

/// Install `Turbine.UI` and `Turbine.UI.Lotro` into a new apartment (call from
/// [`HostApi::install_natives`](crate::HostApi::install_natives)).
///
/// # Errors
///
/// A Lua error when the tables cannot be created, or when `binding` names no live
/// [`PluginUi`] of this thread.
pub fn install(api: &NativeApi<'_>, binding: UiBinding) -> mlua::Result<()> {
    let sh = state::shared(binding.0)?;
    reg::install_all(api, &sh)
}
