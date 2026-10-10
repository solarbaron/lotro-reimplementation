// SPDX-License-Identifier: GPL-3.0-or-later
//! Item slots: the item-slot display (`character-panels.md` §5: composed icon, quantity,
//! state overlays), the bag item lists (§4.1: columns), the paperdoll slots (§3.3), and
//! item drag and drop routed by `ui-behaviour.md` §7.
//!
//! `tessera-ui` does not know items or the feed's commands. The client tells each slot
//! what it holds ([`SlotItem`]: its own item reference, the composed icon as an image
//! reference, quantity, wear, the slot bits it fits) and gets back neutral
//! [`SlotInteraction`]s (a drop with its target, an activation, a destroy request), which
//! it turns into commands by `character-panels.md` §6 (the `tessera-gamedata` inventory
//! rules).
//!
//! Drag and drop: a slot holding an item is `UICore_Element_dragable` with a drag
//! description ([`crate::DragPayload`]: the item and its `Inventory_CompatibleSlot`) and
//! gives its icon as the drag icon (§7.1 step 4); every slot is a
//! `UICore_Element_container`; a paperdoll slot's accept check takes only items whose
//! compatible slots include its `Container_Slot` bit (§7.3 step 1), so a misfit drop is
//! refused (§6 second row).

use std::collections::BTreeSet;
use std::ops::Range;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::DragPayload;
use crate::env::ImageRef;
use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::IconImage;
use super::charvocab::CharVocab;
use super::examine::ExamTemplate;
use super::{find_below, subtree};

/// Most slots one controller keeps (Tessera cap: 75 backpack slots plus the overflow
/// and the paperdoll fit easily).
pub const MAX_SLOTS: usize = 4096;

/// The `Equipment` bit of a slot bit field (`item-tooltips.md` §8.2: bit 29).
pub const EQUIPMENT_BIT: u32 = 0x2000_0000;

/// Where a slot lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SlotPlace {
    /// A backpack slot, by its 0-based index.
    Backpack(u32),
    /// A paperdoll slot, by its `ContainerSlot` data key (bit *b* ↔ key *b* + 1).
    Equipment(u32),
    /// A slot of another container (vendor, loot, overflow), by the client's container
    /// number and the slot index.
    Other {
        /// The client's container number.
        container: u32,
        /// The slot index.
        index: u32,
    },
}

/// Where a drag ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPlace {
    /// On a slot.
    Slot(SlotPlace),
    /// On a drop area outside the slots (a bag window: `character-panels.md` §6), by the
    /// number the client gave it.
    Area(u32),
}

/// The wear state an item slot shows (`character-panels.md` §5 Tessera rule: as the
/// durability row of `item-tooltips.md` §6.10 and §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wear {
    /// Not damaged (`notBroken`).
    #[default]
    Sound,
    /// Damaged (`almostBroken`).
    Damaged,
    /// Broken (`broken`).
    Broken,
}

/// What a slot holds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SlotItem {
    /// The client's item reference.
    pub item: u64,
    /// The item's icon (`icon-layers.md` §3).
    pub icon: Option<IconImage>,
    /// The stack quantity.
    pub quantity: i64,
    /// The wear state.
    pub wear: Wear,
    /// The viewer cannot use the item (`character-panels.md` §5 Tessera rule).
    pub unusable: bool,
    /// The cooldown sweep's position in [0, 1], `None` without a cooldown (P2).
    pub cooldown: Option<f32>,
    /// `Inventory_CompatibleSlot`: the slot bits the item fits.
    pub fits: u32,
}

/// A player's interaction with a slot, for the client to turn into a command
/// (`character-panels.md` §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotInteraction {
    /// An item was dropped on a target that accepted it.
    Dropped {
        /// The item.
        item: u64,
        /// Where it came from.
        from: SlotPlace,
        /// Where it was dropped.
        target: DropPlace,
        /// Whether the split modifier was held (the client's shift state).
        split: bool,
    },
    /// The slot's use command (right button by default; `character-panels.md` §6).
    Activated {
        /// The item.
        item: u64,
        /// The slot.
        at: SlotPlace,
    },
    /// The item was dragged out of every window: the client asks the player to confirm,
    /// then destroys it.
    DestroyRequested {
        /// The item.
        item: u64,
        /// The slot.
        from: SlotPlace,
    },
}

/// One item slot (an `ItemDisplay_SlottedTemplate` instance or a paperdoll slot element).
#[derive(Debug, Clone, PartialEq)]
pub struct ItemSlot {
    /// The slot element.
    pub element: ElementKey,
    /// Where it lives.
    pub place: SlotPlace,
    /// What it holds.
    pub item: Option<SlotItem>,
    /// `ItemDisplay_Icon` (the composed icon), or the slot itself when it has none.
    pub icon: ElementKey,
    /// `ItemDisplay_QuantityOverlay`.
    pub quantity: Option<ElementKey>,
    /// `ItemDisplay_BrokenOverlay`.
    pub broken: Option<ElementKey>,
    /// `ItemDisplay_UnusableOverlay`.
    pub unusable: Option<ElementKey>,
    /// `ItemDisplay_CooldownOverlay`.
    pub cooldown: Option<ElementKey>,
    /// `ItemDisplay_RolloverOverlay`.
    pub rollover: Option<ElementKey>,
    /// The overlays that stay off in P1.
    pub off: Vec<ElementKey>,
    /// The empty slot's tooltip text (`UI_Item_DisplayEmptyOverride`, resolved).
    pub empty_tooltip: Option<String>,
    /// For a slot without `ItemDisplay_Icon` (`icon` is the slot itself): the slot's own
    /// base image when it was attached, restored when the slot empties.
    designed_base: Option<(ImageRef, u32)>,
    /// Whether the slot element itself shows an item icon (the fallback without
    /// `ItemDisplay_Icon`), so that emptying it clears that icon.
    shows_item_icon: bool,
}

/// One bag window's item list (`character-panels.md` §4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BagView {
    /// The bag window.
    pub window: ElementKey,
    /// Its item list (`Backpack_ItemList`).
    pub list: ElementKey,
    /// The bag number (1–6).
    pub bag: u32,
    /// The backpack slots it shows.
    pub range: Range<u32>,
    /// The entries, in slot order.
    pub entries: Vec<ElementKey>,
}

/// The item-slot controller.
#[derive(Debug, Clone)]
pub struct ItemSlots {
    vocab: CharVocab,
    slots: Vec<ItemSlot>,
    areas: Vec<(ElementKey, u32)>,
    /// The slot element under the pointer and its rollover overlay (on).
    rollover: Option<(ElementKey, Option<ElementKey>)>,
    registered: BTreeSet<u32>,
}

impl ItemSlots {
    /// A controller over the given vocabulary.
    #[must_use]
    pub fn new(vocab: CharVocab) -> Self {
        Self {
            vocab,
            slots: Vec::new(),
            areas: Vec::new(),
            rollover: None,
            registered: BTreeSet::new(),
        }
    }

    /// The slots, in attach order.
    #[must_use]
    pub fn slots(&self) -> &[ItemSlot] {
        &self.slots
    }

    /// The slot at `place`.
    #[must_use]
    pub fn slot(&self, place: SlotPlace) -> Option<&ItemSlot> {
        self.slots.iter().find(|s| s.place == place)
    }

    /// The slot of `k` or of its nearest ancestor that is a slot.
    #[must_use]
    pub fn slot_of(&self, ui: &Ui, k: ElementKey) -> Option<&ItemSlot> {
        let i = self.index_of(ui, k)?;
        self.slots.get(i)
    }

    fn index_of(&self, ui: &Ui, k: ElementKey) -> Option<usize> {
        let mut cur = Some(k);
        let mut n = 0;
        while let Some(c) = cur {
            if let Some(i) = self.slots.iter().position(|s| s.element == c) {
                return Some(i);
            }
            n += 1;
            if n > crate::ui::MAX_TREE_DEPTH * 2 {
                return None;
            }
            cur = ui.parent(c);
        }
        None
    }

    /// Makes `element` an item slot at `place` (`character-panels.md` §5): finds its
    /// overlays by element ID, makes it a drop container, gives a paperdoll slot its
    /// accept check, hides every overlay and shows it empty. A slot already attached at the
    /// same element is replaced. Returns the slot's index, or `None` past [`MAX_SLOTS`].
    pub fn attach(&mut self, ui: &mut Ui, element: ElementKey, place: SlotPlace) -> Option<usize> {
        if !ui.exists(element) {
            return None;
        }
        // A slot attached again at the same element keeps the element's own image as it
        // was before any item icon.
        let earlier = self
            .slots
            .iter()
            .find(|s| s.element == element)
            .map(|s| (s.designed_base.clone(), s.shows_item_icon));
        self.slots
            .retain(|s| s.element != element && s.place != place);
        if self.rollover.is_some_and(|(k, _)| k == element) {
            // The new slot starts with its rollover overlay hidden.
            self.rollover = None;
        }
        self.attach_new(ui, element, place, earlier)
    }

    /// [`ItemSlots::attach`] for an element and place no slot uses (the caller checked).
    fn attach_new(
        &mut self,
        ui: &mut Ui,
        element: ElementKey,
        place: SlotPlace,
        earlier: Option<(Option<(ImageRef, u32)>, bool)>,
    ) -> Option<usize> {
        if self.slots.len() >= MAX_SLOTS {
            return None;
        }
        let ids = self.vocab.ids;
        let find = |id: u32| find_below(ui, element, id);
        let icon = find(ids.icon).unwrap_or(element);
        let (designed_base, shows_item_icon) = match earlier {
            Some((base, true)) if icon == element => (base, true),
            _ if icon == element => (ui.base_image(element), false),
            _ => (None, false),
        };
        let slot = ItemSlot {
            element,
            place,
            item: None,
            icon,
            quantity: find(ids.quantity),
            broken: find(ids.broken),
            unusable: find(ids.unusable),
            cooldown: find(ids.cooldown),
            rollover: find(ids.rollover),
            off: ids
                .off_overlays()
                .iter()
                .filter_map(|id| find(*id))
                .collect(),
            empty_tooltip: super::string_prop(ui, element, self.vocab.pids.empty_override)
                .filter(|s| !s.is_empty()),
            designed_base,
            shows_item_icon,
        };
        let pids = ui.vocab().pids;
        ui.set_property(element, pids.container, PropertyValue::Bool(true));
        // A drop container is hovered (`ui-behaviour.md` §4.3 type rule).
        ui.recompute_mouse_visible(element);
        let mask = match place {
            SlotPlace::Equipment(key) => Some(slot_bit(key)),
            _ => None,
        };
        ui.set_drop_mask(element, mask);
        ui.set_drag_icon_source(element, (icon != element).then_some(icon));
        // The slot's own messages reach game code: the use command (§6).
        if let Some(id) = ui.element_id(element)
            && id != 0
            && self.registered.insert(id)
        {
            let click = ui.vocab().msgs.mouse_click;
            ui.register_handler(id, click);
        }
        self.slots.push(slot);
        let i = self.slots.len() - 1;
        for k in self.slots[i].off.clone() {
            ui.set_shown(k, false);
        }
        if let Some(r) = self.slots[i].rollover {
            ui.set_shown(r, false);
        }
        self.show(ui, i);
        Some(i)
    }

    /// Attaches every paperdoll slot below `slot_field` (`character-panels.md` §3.3): each
    /// element with a one-bit `Container_Slot` becomes the slot of that key. Returns the
    /// number of slots attached.
    pub fn attach_paperdoll(&mut self, ui: &mut Ui, slot_field: ElementKey) -> usize {
        let pid = self.vocab.pids.container_slot;
        let mut n = 0;
        for k in subtree(ui, slot_field) {
            let bits = ui.prop(k, pid).key();
            if bits != 0
                && bits.count_ones() == 1
                && self
                    .attach(ui, k, SlotPlace::Equipment(bits.trailing_zeros() + 1))
                    .is_some()
            {
                n += 1;
            }
        }
        n
    }

    /// Makes `element` a drop area numbered `area` (a bag window outside its slots,
    /// `character-panels.md` §6).
    pub fn attach_area(&mut self, ui: &mut Ui, element: ElementKey, area: u32) {
        if !ui.exists(element) {
            return;
        }
        self.areas.retain(|(k, _)| *k != element);
        let pids = ui.vocab().pids;
        ui.set_property(element, pids.container, PropertyValue::Bool(true));
        ui.recompute_mouse_visible(element);
        self.areas.push((element, area));
    }

    /// Fills bag `bag`'s window (`character-panels.md` §4.1, §4.3): finds its item list
    /// (`Backpack_ItemList`, else the first `InventoryDisplay`), sets its column count
    /// (`columns`: the user's `UI.Bag<n>Columns` setting; `None` keeps the data's
    /// `max_columns`), creates one slot from `slot_template` (`ItemDisplay_SlottedTemplate`
    /// of `itemdisplay`) per backpack slot of `range` in ascending order (at most
    /// [`MAX_SLOTS`]; the list is laid out once), and makes the window and the list drop
    /// areas numbered `bag` (a drop on the bag window outside a slot, §6).
    ///
    /// # Errors
    /// [`super::GameError::MissingElement`] without an item list, or a creation error.
    pub fn attach_bag(
        &mut self,
        ui: &mut Ui,
        window: ElementKey,
        bag: u32,
        slot_template: ExamTemplate,
        range: Range<u32>,
        columns: Option<i32>,
    ) -> Result<BagView, super::GameError> {
        let ids = self.vocab.ids;
        let t = self.vocab.types.inventory_display;
        let list = find_below(ui, window, ids.backpack_item_list)
            .or_else(|| {
                subtree(ui, window)
                    .into_iter()
                    .skip(1)
                    .find(|k| ui.element_type(*k) == Some(t))
            })
            .ok_or(super::GameError::MissingElement(ids.backpack_item_list))?;
        if let Some(c) = columns {
            set_columns(ui, list, c);
        }
        let end = range.end.min(range.start.saturating_add(MAX_SLOTS as u32));
        // The old entries and every slot of these places go first, in one pass each.
        let old: std::collections::HashSet<ElementKey> =
            ui.list_entries(list).into_iter().collect();
        self.slots.retain(|s| {
            !old.contains(&s.element)
                && !matches!(s.place, SlotPlace::Backpack(i) if (range.start..end).contains(&i))
        });
        ui.list_clear(list);
        let mut entries = Vec::new();
        for _ in range.start..end {
            match ui.create_element(None, Did(slot_template.layout), slot_template.element) {
                Ok(k) => entries.push(k),
                Err(e) => {
                    ui.destroy_many(&entries);
                    return Err(e.into());
                }
            }
        }
        if let Err((added, e)) = ui.list_add_many(list, &entries) {
            ui.destroy_many(&entries[added..]);
            entries.truncate(added);
            self.attach_entries(ui, &entries, range.start);
            return Err(e.into());
        }
        self.attach_entries(ui, &entries, range.start);
        self.attach_area(ui, window, bag);
        self.attach_area(ui, list, bag);
        Ok(BagView {
            window,
            list,
            bag,
            range,
            entries,
        })
    }

    /// Attaches fresh bag entries as backpack slots from `first` on.
    fn attach_entries(&mut self, ui: &mut Ui, entries: &[ElementKey], first: u32) {
        for (i, k) in entries.iter().enumerate() {
            let index = first.saturating_add(u32::try_from(i).unwrap_or(u32::MAX));
            self.attach_new(ui, *k, SlotPlace::Backpack(index), None);
        }
    }

    /// Shows `item` in the slot at `place` (or empties it). Returns whether the slot exists.
    pub fn set(&mut self, ui: &mut Ui, place: SlotPlace, item: Option<SlotItem>) -> bool {
        let Some(i) = self.slots.iter().position(|s| s.place == place) else {
            return false;
        };
        self.slots[i].item = item;
        self.show(ui, i);
        true
    }

    /// Applies slot `i`'s item to its elements (`character-panels.md` §5 with its Tessera
    /// rules).
    fn show(&mut self, ui: &mut Ui, i: usize) {
        let st = self.vocab.states;
        let Some(s) = self.slots.get_mut(i) else {
            return;
        };
        // The icon. A slot without `ItemDisplay_Icon` shows it on itself; when it
        // empties, the item's icon goes and the slot's own image comes back.
        let icon = s.item.as_ref().and_then(|it| it.icon.as_ref());
        if s.icon != s.element {
            super::set_icon(ui, s.icon, icon);
        } else if icon.is_some() {
            super::set_icon(ui, s.icon, icon);
            s.shows_item_icon = true;
        } else if s.shows_item_icon {
            super::set_icon(ui, s.icon, None);
            if let Some((img, mode)) = &s.designed_base {
                super::set_image_ref(ui, s.icon, img, *mode);
            }
            s.shows_item_icon = false;
        }
        let s = &self.slots[i];
        let item = s.item.as_ref();
        // Quantity: shown when greater than 1.
        if let Some(q) = s.quantity {
            match item.map(|it| it.quantity).filter(|q| *q > 1) {
                Some(n) => {
                    ui.set_text(q, Some(n.to_string()));
                    ui.set_shown(q, true);
                }
                None => {
                    ui.set_text(q, None);
                    ui.set_shown(q, false);
                }
            }
        }
        // Wear.
        if let Some(b) = s.broken {
            let wear = item.map_or(Wear::Sound, |it| it.wear);
            let (state, on) = match wear {
                Wear::Broken => (st.broken, true),
                Wear::Damaged => (st.almost_broken, true),
                Wear::Sound => (st.not_broken, false),
            };
            ui.switch_state_if_defined(b, state);
            ui.set_shown(b, on);
        }
        // Unusable.
        if let Some(u) = s.unusable {
            let on = item.is_some_and(|it| it.unusable);
            ui.switch_state_if_defined(u, if on { st.overlay_on } else { st.overlay_off });
            ui.set_shown(u, on);
        }
        // Cooldown sweep.
        if let Some(c) = s.cooldown {
            match item.and_then(|it| it.cooldown).filter(|f| f.is_finite()) {
                Some(f) => {
                    let pos = ui.vocab().pids.meter_position;
                    ui.set_property(c, pos, PropertyValue::Float(f.clamp(0.0, 1.0)));
                    ui.set_shown(c, true);
                }
                None => ui.set_shown(c, false),
            }
        }
        // Drag source, examination request and the empty slot's text tooltip.
        let dragable = ui.vocab().pids.dragable;
        match item {
            Some(it) => {
                ui.set_property(s.element, dragable, PropertyValue::Bool(true));
                ui.set_drag_payload(
                    s.element,
                    Some(DragPayload {
                        item: it.item,
                        fits: it.fits,
                    }),
                );
                ui.set_tooltip_text(s.element, None);
                ui.set_examine(s.element, true);
            }
            None => {
                ui.set_property(s.element, dragable, PropertyValue::Bool(false));
                ui.set_drag_payload(s.element, None);
                ui.set_examine(s.element, false);
                ui.set_tooltip_text(s.element, s.empty_tooltip.clone());
            }
        }
    }

    /// Sizes the layer fields of every slot's [`IconImage::Layers`] icon again
    /// ([`super::fit_icon_layers`]): call after icon images that were still loading have
    /// arrived.
    pub fn fit_icons(&self, ui: &mut Ui) {
        for s in &self.slots {
            super::fit_icon_layers(ui, s.icon);
        }
    }

    /// The rollover overlay (`character-panels.md` §5): on while the pointer is over the
    /// slot. Call after pointer moves (or once per frame).
    pub fn update_rollover(&mut self, ui: &mut Ui) {
        let now = ui
            .hovered()
            .and_then(|h| self.index_of(ui, h))
            .and_then(|i| self.slots.get(i))
            .map(|s| (s.element, s.rollover));
        if now == self.rollover {
            return;
        }
        if let Some((_, Some(r))) = self.rollover
            && ui.exists(r)
        {
            ui.set_shown(r, false);
        }
        if let Some((_, Some(r))) = now {
            ui.set_shown(r, true);
        }
        self.rollover = now;
    }

    /// Turns a UI event into a slot interaction: the end of an item drag from a slot
    /// (`ui-behaviour.md` §7.4), or the use command (right button) clicked on a slot that
    /// holds an item. `split` is the client's shift state at the drop.
    pub fn handle_event(&self, ui: &Ui, ev: &UiEvent, split: bool) -> Option<SlotInteraction> {
        match ev {
            UiEvent::DragEnded {
                source,
                target,
                accepted,
            } => self.dropped(ui, *source, *target, *accepted, split),
            UiEvent::Game { message } => {
                if message.msg != ui.vocab().msgs.mouse_click {
                    return None;
                }
                let right = message
                    .arg
                    .command
                    .as_ref()
                    .is_some_and(|c| c.mouse_button() == 2);
                if !right {
                    return None;
                }
                let s = self.slot_of(ui, message.source)?;
                let it = s.item.as_ref()?;
                Some(SlotInteraction::Activated {
                    item: it.item,
                    at: s.place,
                })
            }
            _ => None,
        }
    }

    /// The interaction of a finished drag (§6): accepted on a slot or an area; not
    /// accepted with the pointer outside every window → a destroy request.
    fn dropped(
        &self,
        ui: &Ui,
        source: ElementKey,
        target: Option<ElementKey>,
        accepted: bool,
        split: bool,
    ) -> Option<SlotInteraction> {
        let from = self.slots.iter().find(|s| s.element == source)?;
        let item = from.item.as_ref()?.item;
        if !accepted {
            let outside = ui.hit_element().is_none_or(|h| h == ui.screen_root());
            return outside.then_some(SlotInteraction::DestroyRequested {
                item,
                from: from.place,
            });
        }
        let t = target?;
        let place = if let Some(s) = self.slots.iter().find(|s| s.element == t) {
            DropPlace::Slot(s.place)
        } else {
            DropPlace::Area(self.areas.iter().find(|(k, _)| *k == t)?.1)
        };
        Some(SlotInteraction::Dropped {
            item,
            from: from.place,
            target: place,
            split,
        })
    }

    /// Forgets slots and areas whose elements were destroyed (closed panels).
    pub fn prune(&mut self, ui: &Ui) {
        self.slots.retain(|s| ui.exists(s.element));
        self.areas.retain(|(k, _)| ui.exists(*k));
        if self.rollover.is_some_and(|(k, _)| !ui.exists(k)) {
            self.rollover = None;
        }
    }
}

/// The slot bit of a `ContainerSlot` key (bit *b* ↔ key *b* + 1, `property-system.md`
/// §7.3); 0 for keys outside 1–32.
#[must_use]
pub fn slot_bit(key: u32) -> u32 {
    if (1..=32).contains(&key) {
        1u32 << (key - 1)
    } else {
        0
    }
}

/// Sets a bag list's column count (`character-panels.md` §4.1: the user's
/// `UI.Bag<n>Columns` overrides the data's `max_columns`); at least 1.
pub fn set_columns(ui: &mut Ui, list: ElementKey, columns: i32) {
    let pid = ui.vocab().pids.lb_max_columns;
    ui.set_property(list, pid, PropertyValue::Int(columns.max(1)));
}

/// The column count a bag list of `list_width` pixels holds with cells of `cell_width`
/// (after a resize, `ui-behaviour.md` §8.2): at least 1.
#[must_use]
pub fn columns_for_width(list_width: i32, cell_width: i32) -> i32 {
    if cell_width <= 0 {
        return 1;
    }
    (list_width / cell_width).max(1)
}

/// The name of a bag window's column setting (`UI_UserSetting_NumItemsInRow`, for example
/// `UI.Bag1Columns`).
#[must_use]
pub fn column_setting(ui: &mut Ui, vocab: &CharVocab, window: ElementKey) -> Option<String> {
    let pid = vocab.pids.num_items_in_row;
    subtree(ui, window)
        .into_iter()
        .find(|k| ui.prop(*k, pid).is_set())
        .and_then(|k| super::string_prop(ui, k, pid))
}
