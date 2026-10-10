// SPDX-License-Identifier: GPL-3.0-or-later
//! The command a player's item interaction sends (`character-panels.md` §6, Tessera
//! rules CP10), as a pure function. Nothing changes locally until the session's event
//! arrives.

use tessera_formats::gameplay::items::Slots;

use super::model::{Container, Inventory, ItemId, ItemPlace};
use super::slots::SlotRules;

/// Where an item was dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropTarget {
    /// A paperdoll slot, by data key.
    PaperdollSlot(u16),
    /// A slot of a container (backpack slots: the 0-based slot).
    Slot(ItemPlace),
    /// A bag window outside its slots.
    BagWindow,
}

/// A player's interaction with an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAction {
    /// A drop on `target`; `split` when the split modifier (shift) was held.
    Drop {
        /// The drop target.
        target: DropTarget,
        /// Whether the split modifier was held.
        split: bool,
    },
    /// The slot's use command (right button by default).
    Activate,
    /// The item was dragged out of every window and the player confirmed destroying it.
    DestroyConfirmed,
}

/// A command for the session (`scenario-feed.md` §9.4, Inventory group).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryCommand {
    /// `EquipItem { item, slot }` (the slot is a data key).
    EquipItem {
        /// The item.
        item: ItemId,
        /// The data key.
        slot: u16,
    },
    /// `UnequipItem { item }`.
    UnequipItem {
        /// The item.
        item: ItemId,
    },
    /// `MoveItem { item, to }`.
    MoveItem {
        /// The item.
        item: ItemId,
        /// The target.
        to: ItemPlace,
    },
    /// `SplitStack { item, quantity }`.
    SplitStack {
        /// The item.
        item: ItemId,
        /// The quantity split off.
        quantity: u32,
    },
    /// `DestroyItem { item }`.
    DestroyItem {
        /// The item.
        item: ItemId,
    },
    /// `UseItem { item, target }` (target: the session's entity ID of the current target).
    UseItem {
        /// The item.
        item: ItemId,
        /// The current target.
        target: Option<u64>,
    },
}

/// What [`choose_command`] needs to know about the item and the drop target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemContext {
    /// The item.
    pub item: ItemId,
    /// Where it is.
    pub place: ItemPlace,
    /// Its quantity.
    pub quantity: u32,
    /// Its record's slot bit fields (all `None` for a literal item).
    pub slots: Slots,
    /// Whether the drop target slot (for [`DropTarget::Slot`]) holds another item.
    pub target_occupied: bool,
    /// The player's current target.
    pub current_target: Option<u64>,
}

/// The command for `action` on an item (`character-panels.md` §6), or `None` when the
/// interaction sends nothing.
///
/// | Item in | Action | Command |
/// |---|---|---|
/// | backpack | drop on a paperdoll slot it fits ([`SlotRules::fits`]) | `EquipItem` with that key |
/// | backpack | drop on a paperdoll slot it does not fit | none |
/// | backpack | activate, default slot equippable ([`SlotRules::default_equip_key`]) | `EquipItem` with the lowest equipment key |
/// | backpack | activate otherwise | `UseItem` on the current target |
/// | equipment | drop on a backpack slot | `MoveItem` to it |
/// | equipment | drop on a bag window outside a slot | `UnequipItem` |
/// | equipment | activate | `UnequipItem` |
/// | any container | drop on another slot | `MoveItem` |
/// | any container | split-drop of a stack of 2 or more on an empty slot | `SplitStack` of half, rounded down |
/// | any | destroy confirmed | `DestroyItem` |
///
/// A drop on the item's own slot, a split-drop of a stack of 1 (an ordinary drop), and
/// every combination the table does not list (for example an equipped item dropped on a
/// paperdoll slot, or an item outside the backpack and the equipment activated) send
/// nothing beyond what an ordinary drop sends. A [`DropTarget::Slot`] in the
/// [`Container::Equipment`] is treated as the [`DropTarget::PaperdollSlot`] of that key.
///
/// `SplitStack` carries no target: the feed puts the new stack at the first free slot of
/// the item's own container (`scenario-feed.md` §7.6), not at the slot the player chose,
/// even when that slot is in another bag or container (OPEN in `character-panels.md` §6).
#[must_use]
pub fn choose_command(
    ctx: &ItemContext,
    action: ItemAction,
    rules: &SlotRules,
) -> Option<InventoryCommand> {
    let item = ctx.item;
    let from = ctx.place.container;
    match action {
        ItemAction::DestroyConfirmed => Some(InventoryCommand::DestroyItem { item }),
        ItemAction::Activate => match from {
            Container::Backpack => Some(match rules.default_equip_key(&ctx.slots) {
                Some(key) => InventoryCommand::EquipItem {
                    item,
                    slot: u16::try_from(key).ok()?,
                },
                None => InventoryCommand::UseItem {
                    item,
                    target: ctx.current_target,
                },
            }),
            Container::Equipment => Some(InventoryCommand::UnequipItem { item }),
            Container::Vault | Container::SharedStorage => None,
        },
        ItemAction::Drop { target, split } => match target {
            DropTarget::PaperdollSlot(key) => paperdoll_drop(ctx, key, rules),
            DropTarget::BagWindow => {
                (from == Container::Equipment).then_some(InventoryCommand::UnequipItem { item })
            }
            // A place of the equipment is a paperdoll slot, whichever way it is named:
            // the fit check and `EquipItem` apply, never `MoveItem` or `SplitStack`.
            DropTarget::Slot(to) if to.container == Container::Equipment => {
                paperdoll_drop(ctx, to.slot, rules)
            }
            DropTarget::Slot(to) => {
                if to == ctx.place {
                    return None;
                }
                if split && ctx.quantity >= 2 && !ctx.target_occupied {
                    return Some(InventoryCommand::SplitStack {
                        item,
                        quantity: ctx.quantity / 2,
                    });
                }
                Some(InventoryCommand::MoveItem { item, to })
            }
        },
    }
}

/// A drop on the paperdoll slot of data key `key` (§6 rows 1–2): `EquipItem` for a
/// backpack item that fits it, else nothing.
fn paperdoll_drop(ctx: &ItemContext, key: u16, rules: &SlotRules) -> Option<InventoryCommand> {
    (ctx.place.container == Container::Backpack && rules.fits(&ctx.slots, u32::from(key)))
        .then_some(InventoryCommand::EquipItem {
            item: ctx.item,
            slot: key,
        })
}

impl Inventory {
    /// [`choose_command`] for item `item` of this inventory, filling its place, quantity
    /// and the target's occupancy from the model; `None` when the item is unknown.
    #[must_use]
    pub fn choose_command(
        &self,
        item: ItemId,
        slots: Slots,
        action: ItemAction,
        rules: &SlotRules,
        current_target: Option<u64>,
    ) -> Option<InventoryCommand> {
        let it = self.item(item)?;
        let target_occupied = match action {
            ItemAction::Drop {
                target: DropTarget::Slot(to),
                ..
            } => self.at(to).is_some_and(|o| o.id != item),
            _ => false,
        };
        choose_command(
            &ItemContext {
                item,
                place: it.place,
                quantity: it.quantity,
                slots,
                target_occupied,
                current_target,
            },
            action,
            rules,
        )
    }
}
