// SPDX-License-Identifier: GPL-3.0-or-later
//! The local player's state as the session's events report it (decision 0024 P1): the
//! inventory model of `tessera-gamedata` (`character-panels.md` §4.3, §9), the item and
//! player properties the feed scripts (`scenario-feed.md` §6.1 `stat`, `item-property`;
//! decision 0025 D5), the header values (name, class, body, level), vitals, the wallet and
//! the player's worn look.
//!
//! The state changes only on events, never on the player's commands
//! (`character-panels.md` §6). Every event yields a list of [`PlayerChange`]s, in order,
//! which the panels, the tooltips and the plugin views consume.
//!
//! # Hostile input
//!
//! Every value comes from a session and is bounded: at most [`MAX_PROPERTIES`] properties
//! per entity and per item (decision 0025 D5; further ones are dropped with a warning),
//! item properties only for items the inventory holds (and dropped with them), at most
//! [`MAX_WALLET_ENTRIES`] currencies and [`MAX_CHARACTER_INFOS`] character entries. The
//! inventory model bounds its own items (`tessera_gamedata::inventory::MAX_ITEMS`).

use std::collections::{BTreeMap, HashMap};

use tessera_formats::Did;
use tessera_gamedata::inventory::{
    BagLayout, Container as GContainer, Inventory, InventoryChange, InventoryEvent as GEvent,
    ItemId, ItemPlace as GPlace, ItemRef,
};
use tessera_net::domain::{
    CharacterEvent, CombatEvent, Container, Currency, InventoryEvent, ItemPlace, Ref, Vital,
};
use tessera_net::{Appearance, Body, EntityId, Event, PropertyValue};

/// Most properties kept per entity and per item (decision 0025 D5: "the client keeps at
/// most 1,024 properties per entity and per item and drops further ones with a warning").
pub const MAX_PROPERTIES: usize = 1024;
/// Most wallet currencies held at once (Tessera bound, above the 610 wallet items of the
/// reference install; further currencies are dropped). A currency set to 0 is no longer
/// held (coin excepted), so it does not count.
pub const MAX_WALLET_ENTRIES: usize = 4096;
/// Most character-list entries kept (Tessera bound; a character list is at most a few
/// dozen entries).
pub const MAX_CHARACTER_INFOS: usize = 1024;

/// The bag layout without an install: one bag shows every backpack slot, and the size *N*
/// starts at 0 and grows as items arrive (`character-panels.md` §4.3 rule 1). A Tessera
/// fallback: the install's `InventoryControl` values (§4.2) are read whenever an install
/// is present.
pub const NO_INSTALL_BAGS: BagLayout = BagLayout {
    base_slots: 0,
    slots_per_bag: 0,
    bags: 1,
};

/// One change of the player's state, in the order it happened.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerChange {
    /// The inventory model changed (`tessera_gamedata::inventory`).
    Inventory(InventoryChange),
    /// A property of an item instance was set or cleared (`ItemProperty`).
    ItemProperty {
        /// The item.
        item: ItemId,
        /// The property.
        property: u32,
    },
    /// A property of the player was set or cleared (`PropertyChanged`).
    Property {
        /// The property.
        property: u32,
    },
    /// The header values changed: name, class, body or level.
    Header,
    /// A vital of the player changed.
    Vital(VitalKind),
    /// The wallet changed.
    Wallet,
    /// The player's appearance (worn look) changed.
    Appearance,
    /// The player left the world (or the session ended): everything was cleared.
    Reset,
    /// The NPC windows changed, or a currency's amount did (decision 0024 P3;
    /// [`crate::npc::state`]).
    Npc(crate::npc::NpcChange),
}

/// The vitals the character sheet shows (`character-panels.md` §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VitalKind {
    /// Morale (`Health`).
    Morale,
    /// Power.
    Power,
}

/// A vital's reported values.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VitalValue {
    /// Current value.
    pub current: f32,
    /// Maximum value.
    pub max: f32,
}

/// What a `CharacterInfo` reported for one character name.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharacterDetails {
    /// Its body.
    pub body: Option<Body>,
    /// Its class value.
    pub class: Option<u32>,
    /// Its level.
    pub level: Option<u32>,
}

/// The local player's state (module docs).
#[derive(Debug, Clone)]
pub struct PlayerState {
    inventory: Inventory,
    player: Option<EntityId>,
    name: Option<String>,
    /// `CharacterInfo` by name key (`crate::net::name_key`).
    infos: BTreeMap<String, CharacterDetails>,
    level: Option<u32>,
    vitals: BTreeMap<u8, VitalValue>,
    properties: BTreeMap<u32, PropertyValue>,
    item_properties: HashMap<ItemId, BTreeMap<u32, PropertyValue>>,
    wallet: Vec<(Currency, u64)>,
    appearance: Option<Appearance>,
    dropped_properties: u64,
    /// The NPC windows (decision 0024 P3).
    npc: crate::npc::NpcWindows,
}

fn vital_index(k: VitalKind) -> u8 {
    match k {
        VitalKind::Morale => 0,
        VitalKind::Power => 1,
    }
}

/// The inventory model's container of a session container.
#[must_use]
pub fn model_container(c: Container) -> GContainer {
    match c {
        Container::Backpack => GContainer::Backpack,
        Container::Equipment => GContainer::Equipment,
        Container::Vault => GContainer::Vault,
        Container::SharedStorage => GContainer::SharedStorage,
    }
}

/// The session container of an inventory-model container.
#[must_use]
pub fn session_container(c: GContainer) -> Container {
    match c {
        GContainer::Backpack => Container::Backpack,
        GContainer::Equipment => Container::Equipment,
        GContainer::Vault => Container::Vault,
        GContainer::SharedStorage => Container::SharedStorage,
    }
}

/// The inventory model's place of a session place.
#[must_use]
pub fn model_place(p: ItemPlace) -> GPlace {
    GPlace {
        container: model_container(p.container),
        slot: p.slot,
    }
}

/// The session place of an inventory-model place.
#[must_use]
pub fn session_place(p: GPlace) -> ItemPlace {
    ItemPlace {
        container: session_container(p.container),
        slot: p.slot,
    }
}

/// The inventory model's item reference of a session reference.
#[must_use]
pub fn model_ref(r: &Ref) -> ItemRef {
    match r {
        Ref::Record(d) => ItemRef::Record(Did(d.0)),
        Ref::Literal(s) => ItemRef::Literal(s.clone()),
    }
}

/// The inventory-model event of a session inventory event (`None` for the wallet and item
/// properties, which the model does not hold).
#[must_use]
pub fn model_event(e: &InventoryEvent) -> Option<GEvent> {
    Some(match e {
        InventoryEvent::ContainerSize { container, slots } => GEvent::ContainerSize {
            container: model_container(*container),
            slots: *slots,
        },
        InventoryEvent::ItemAdded {
            item,
            what,
            at,
            quantity,
        } => GEvent::ItemAdded {
            item: item.0,
            what: model_ref(what),
            at: model_place(*at),
            quantity: *quantity,
        },
        InventoryEvent::ItemMoved { item, at } => GEvent::ItemMoved {
            item: item.0,
            at: model_place(*at),
        },
        InventoryEvent::ItemQuantity { item, quantity } => GEvent::ItemQuantity {
            item: item.0,
            quantity: *quantity,
        },
        InventoryEvent::ItemRemoved { item } => GEvent::ItemRemoved { item: item.0 },
        InventoryEvent::ItemCondition { item, current, max } => GEvent::ItemCondition {
            item: item.0,
            current: *current,
            max: *max,
        },
        _ => return None,
    })
}

/// Sets or clears `property` in `map`, keeping at most [`MAX_PROPERTIES`] entries; returns
/// whether the map changed (`false` also for a dropped new property).
fn set_property(
    map: &mut BTreeMap<u32, PropertyValue>,
    property: u32,
    value: Option<&PropertyValue>,
    dropped: &mut u64,
) -> bool {
    match value {
        None => map.remove(&property).is_some(),
        Some(v) => {
            if !map.contains_key(&property) && map.len() >= MAX_PROPERTIES {
                *dropped += 1;
                if (*dropped).is_power_of_two() {
                    tracing::warn!(
                        property,
                        dropped = *dropped,
                        "more than {MAX_PROPERTIES} properties on one entity or item; dropped"
                    );
                }
                return false;
            }
            map.insert(property, v.clone()) != Some(v.clone())
        }
    }
}

impl PlayerState {
    /// An empty state whose backpack follows `bags` (the install's `InventoryControl`, or
    /// [`NO_INSTALL_BAGS`]).
    #[must_use]
    pub fn new(bags: BagLayout) -> Self {
        Self {
            inventory: Inventory::new(bags),
            player: None,
            name: None,
            infos: BTreeMap::new(),
            level: None,
            vitals: BTreeMap::new(),
            properties: BTreeMap::new(),
            item_properties: HashMap::new(),
            wallet: Vec::new(),
            appearance: None,
            dropped_properties: 0,
            npc: crate::npc::NpcWindows::new(),
        }
    }

    /// Replaces the bag layout (the install's `InventoryControl` arrived after the session
    /// started). The items stay: the model is rebuilt from the container sizes and the
    /// items at their places, so *N* follows `character-panels.md` §4.3 rule 1 under the
    /// new layout: before the first backpack `ContainerSize` it never shrinks (the new
    /// *N* is at least the old one), and every place keeps showing the item it showed.
    /// Its changes (a new *N*) are appended to `out`.
    pub fn set_bags(&mut self, bags: BagLayout, out: &mut Vec<PlayerChange>) {
        if *self.inventory.bags() == bags {
            return;
        }
        let old_n = self.inventory.backpack_size();
        let sized = self
            .inventory
            .container_size(GContainer::Backpack)
            .is_some();
        let mut inv = Inventory::new(bags);
        if !sized
            && old_n > inv.backpack_size()
            && let Ok(last) = u16::try_from(old_n - 1)
        {
            // Rule 1 growth to the old *N*: a transient arrival at its last slot (an ID the
            // model does not hold), removed again at once.
            let probe = (0..64)
                .map(|k| u64::MAX - k)
                .find(|id| self.inventory.item(*id).is_none());
            if let Some(probe) = probe {
                inv.apply(&GEvent::ItemAdded {
                    item: probe,
                    what: ItemRef::Literal(String::new()),
                    at: GPlace::backpack(last),
                    quantity: 0,
                });
                inv.apply(&GEvent::ItemRemoved { item: probe });
            }
        }
        for c in [
            GContainer::Backpack,
            GContainer::Equipment,
            GContainer::Vault,
            GContainer::SharedStorage,
        ] {
            if let Some(slots) = self.inventory.container_size(c) {
                inv.apply(&GEvent::ContainerSize {
                    container: c,
                    slots,
                });
            }
        }
        // Items hidden under another at their place first, then the shown ones: the latest
        // arrival at a place is the one shown, so each place keeps its item.
        let shown = |it: &&tessera_gamedata::inventory::Item| {
            self.inventory.at(it.place).map(|s| s.id) == Some(it.id)
        };
        let order = self
            .inventory
            .items()
            .filter(|it| !shown(it))
            .chain(self.inventory.items().filter(|it| shown(it)));
        for it in order {
            inv.apply(&GEvent::ItemAdded {
                item: it.id,
                what: it.what.clone(),
                at: it.place,
                quantity: it.quantity,
            });
            if let Some((current, max)) = it.condition {
                inv.apply(&GEvent::ItemCondition {
                    item: it.id,
                    current,
                    max,
                });
            }
        }
        self.inventory = inv;
        let new_n = self.inventory.backpack_size();
        if new_n != old_n {
            out.push(PlayerChange::Inventory(InventoryChange::BackpackSize {
                old: old_n,
                new: new_n,
            }));
        }
    }

    /// The inventory model.
    #[must_use]
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    /// The player entity, once in the world.
    #[must_use]
    pub fn player(&self) -> Option<EntityId> {
        self.player
    }

    /// The entered character's name.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The entered character's details (`CharacterInfo` of its name).
    #[must_use]
    pub fn details(&self) -> Option<&CharacterDetails> {
        self.infos.get(&crate::net::name_key(self.name.as_deref()?))
    }

    /// The details a `CharacterInfo` reported for `name`.
    #[must_use]
    pub fn details_of(&self, name: &str) -> Option<&CharacterDetails> {
        self.infos.get(&crate::net::name_key(name))
    }

    /// The player's class value (from `CharacterInfo`).
    #[must_use]
    pub fn class(&self) -> Option<u32> {
        self.details().and_then(|d| d.class)
    }

    /// The player's body: the latest appearance's, else `CharacterInfo`'s.
    #[must_use]
    pub fn body(&self) -> Option<Body> {
        self.appearance
            .as_ref()
            .map(|a| a.body)
            .or_else(|| self.details().and_then(|d| d.body))
    }

    /// The player's level: the latest `LevelChanged`, else `CharacterInfo`'s.
    #[must_use]
    pub fn level(&self) -> Option<u32> {
        self.level.or_else(|| self.details().and_then(|d| d.level))
    }

    /// A vital's last reported values.
    #[must_use]
    pub fn vital(&self, k: VitalKind) -> Option<VitalValue> {
        self.vitals.get(&vital_index(k)).copied()
    }

    /// The player's property `pid`, last reported by `PropertyChanged`.
    #[must_use]
    pub fn property(&self, pid: u32) -> Option<&PropertyValue> {
        self.properties.get(&pid)
    }

    /// Every player property reported, ascending ID.
    pub fn properties(&self) -> impl Iterator<Item = (u32, &PropertyValue)> {
        self.properties.iter().map(|(k, v)| (*k, v))
    }

    /// The instance properties of item `item` (`ItemProperty`), ascending ID.
    pub fn item_properties(&self, item: ItemId) -> impl Iterator<Item = (u32, &PropertyValue)> {
        self.item_properties
            .get(&item)
            .into_iter()
            .flat_map(|m| m.iter().map(|(k, v)| (*k, v)))
    }

    /// The wallet: each currency's last reported amount, in arrival order.
    #[must_use]
    pub fn wallet(&self) -> &[(Currency, u64)] {
        &self.wallet
    }

    /// The coin amount of the wallet.
    #[must_use]
    pub fn coin(&self) -> Option<u64> {
        self.wallet
            .iter()
            .find(|(c, _)| *c == Currency::Coin)
            .map(|(_, a)| *a)
    }

    /// The player's latest appearance (`AppearanceChanged` of the player).
    #[must_use]
    pub fn appearance(&self) -> Option<&Appearance> {
        self.appearance.as_ref()
    }

    /// Properties dropped by the [`MAX_PROPERTIES`] bound so far.
    #[must_use]
    pub fn dropped_properties(&self) -> u64 {
        self.dropped_properties
    }

    /// The NPC windows (decision 0024 P3).
    #[must_use]
    pub fn npc(&self) -> &crate::npc::NpcWindows {
        &self.npc
    }

    /// Applies one session event; its changes are appended to `out`, in order.
    pub fn apply(&mut self, ev: &Event, out: &mut Vec<PlayerChange>) {
        let mut npc = Vec::new();
        self.npc.apply(ev, &mut npc);
        out.extend(npc.into_iter().map(PlayerChange::Npc));
        match ev {
            Event::Connected { .. } => {
                // A new session: the character list is about to be reported again.
                self.infos.clear();
            }
            Event::WorldEntered { player, name, .. } => {
                if self.player.is_some() {
                    self.reset(out);
                }
                self.player = Some(*player);
                self.name = Some(name.clone());
                out.push(PlayerChange::Header);
            }
            Event::Disconnected { .. } => {
                self.reset(out);
                self.infos.clear();
            }
            Event::AppearanceChanged { id, appearance } if Some(*id) == self.player => {
                if self.appearance.as_ref() != Some(appearance.as_ref()) {
                    self.appearance = Some(appearance.as_ref().clone());
                    out.push(PlayerChange::Appearance);
                    out.push(PlayerChange::Header);
                }
            }
            Event::Character(c) => self.apply_character(c, out),
            Event::Inventory(i) => self.apply_inventory(i, out),
            Event::Combat(CombatEvent::VitalChanged {
                entity,
                vital,
                current,
                max,
            }) if Some(*entity) == self.player => {
                let kind = match vital {
                    Vital::Health => VitalKind::Morale,
                    Vital::Power => VitalKind::Power,
                    _ => return,
                };
                let v = VitalValue {
                    current: *current,
                    max: *max,
                };
                if self.vitals.insert(vital_index(kind), v) != Some(v) {
                    out.push(PlayerChange::Vital(kind));
                }
            }
            _ => {}
        }
    }

    fn apply_character(&mut self, c: &CharacterEvent, out: &mut Vec<PlayerChange>) {
        match c {
            CharacterEvent::CharacterInfo {
                name,
                body,
                class,
                level,
                ..
            } => {
                let key = crate::net::name_key(name);
                if !self.infos.contains_key(&key) && self.infos.len() >= MAX_CHARACTER_INFOS {
                    tracing::debug!("character list too long; CharacterInfo ignored");
                    return;
                }
                let d = CharacterDetails {
                    body: *body,
                    class: *class,
                    level: *level,
                };
                let mine = self.name.as_deref().map(crate::net::name_key) == Some(key.clone());
                if self.infos.insert(key, d.clone()) != Some(d) && mine {
                    out.push(PlayerChange::Header);
                }
            }
            CharacterEvent::CharacterRemoved { name } => {
                self.infos.remove(&crate::net::name_key(name));
            }
            CharacterEvent::LevelChanged { entity, level } if Some(*entity) == self.player => {
                if self.level != Some(*level) {
                    self.level = Some(*level);
                    out.push(PlayerChange::Header);
                }
            }
            CharacterEvent::PropertyChanged {
                entity,
                property,
                value,
            } if Some(*entity) == self.player
                && set_property(
                    &mut self.properties,
                    *property,
                    value.as_ref(),
                    &mut self.dropped_properties,
                ) =>
            {
                out.push(PlayerChange::Property {
                    property: *property,
                });
            }
            _ => {}
        }
    }

    fn apply_inventory(&mut self, e: &InventoryEvent, out: &mut Vec<PlayerChange>) {
        match e {
            InventoryEvent::WalletChanged { currency, amount } => {
                let full = self.wallet.len() >= MAX_WALLET_ENTRIES;
                // A wallet item set to 0 is no longer held (P3: the wallet window and the
                // plugin wallet views list held currencies only); coin stays known at 0.
                let gone = *amount == 0 && *currency != Currency::Coin;
                match self.wallet.iter().position(|(c, _)| c == currency) {
                    Some(i) if self.wallet[i].1 == *amount => return,
                    Some(i) if gone => {
                        self.wallet.remove(i);
                    }
                    Some(i) => self.wallet[i].1 = *amount,
                    None if gone => return,
                    None if full => {
                        tracing::debug!("wallet holds {MAX_WALLET_ENTRIES} currencies; dropped");
                        return;
                    }
                    None => self.wallet.push((currency.clone(), *amount)),
                }
                out.push(PlayerChange::Wallet);
                out.push(PlayerChange::Npc(crate::npc::NpcChange::Wallet {
                    currency: currency.clone(),
                    amount: *amount,
                }));
            }
            InventoryEvent::ItemProperty {
                item,
                property,
                value,
            } => {
                if self.inventory.item(item.0).is_none() {
                    tracing::debug!(item = item.0, "ItemProperty for an unknown item; ignored");
                    return;
                }
                let map = self.item_properties.entry(item.0).or_default();
                let changed =
                    set_property(map, *property, value.as_ref(), &mut self.dropped_properties);
                if map.is_empty() {
                    self.item_properties.remove(&item.0);
                }
                if changed {
                    out.push(PlayerChange::ItemProperty {
                        item: item.0,
                        property: *property,
                    });
                }
            }
            other => {
                let Some(ge) = model_event(other) else {
                    return;
                };
                for c in self.inventory.apply(&ge) {
                    if let InventoryChange::Removed { item, .. } = c {
                        self.item_properties.remove(&item);
                    }
                    out.push(PlayerChange::Inventory(c));
                }
            }
        }
    }

    /// Clears everything of the world (inventory, properties, vitals, wallet, look).
    fn reset(&mut self, out: &mut Vec<PlayerChange>) {
        let bags = *self.inventory.bags();
        let had = self.player.is_some() || !self.inventory.is_empty();
        self.inventory = Inventory::new(bags);
        self.player = None;
        self.name = None;
        self.level = None;
        self.vitals.clear();
        self.properties.clear();
        self.item_properties.clear();
        self.wallet.clear();
        self.appearance = None;
        if had {
            out.push(PlayerChange::Reset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::ItemId as NetItemId;
    use tessera_net::{DataId, Transform, Vec3};

    fn entered(state: &mut PlayerState) -> Vec<PlayerChange> {
        let mut out = Vec::new();
        state.apply(
            &Event::WorldEntered {
                player: EntityId(7),
                name: "Tamsin".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            &mut out,
        );
        out
    }

    fn inv(e: InventoryEvent) -> Event {
        Event::Inventory(e)
    }

    fn added(id: u64, slot: u16) -> Event {
        inv(InventoryEvent::ItemAdded {
            item: NetItemId(id),
            what: Ref::Literal("Coil of rope".into()),
            at: ItemPlace {
                container: Container::Backpack,
                slot,
            },
            quantity: 1,
        })
    }

    #[test]
    fn a_late_bag_layout_keeps_n_and_the_shown_items() {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        entered(&mut s);
        let mut out = Vec::new();
        // N grows to 51, then the item that raised it leaves (N stays: rule 1).
        s.apply(&added(9, 50), &mut out);
        s.apply(
            &inv(InventoryEvent::ItemRemoved { item: NetItemId(9) }),
            &mut out,
        );
        // Two items at slot 0: the later arrival (3) is shown.
        s.apply(&added(5, 0), &mut out);
        s.apply(&added(3, 0), &mut out);
        assert_eq!(s.inventory().backpack_size(), 51);
        assert_eq!(s.inventory().at(GPlace::backpack(0)).map(|i| i.id), Some(3));
        out.clear();
        let install = BagLayout {
            base_slots: 45,
            slots_per_bag: 15,
            bags: 6,
        };
        s.set_bags(install, &mut out);
        assert_eq!(s.inventory().backpack_size(), 51, "N never shrinks");
        assert!(out.is_empty(), "no new N: {out:?}");
        assert_eq!(s.inventory().at(GPlace::backpack(0)).map(|i| i.id), Some(3));
        assert_eq!(s.inventory().len(), 2);
        // A larger base still raises N.
        let bigger = BagLayout {
            base_slots: 60,
            ..install
        };
        s.set_bags(bigger, &mut out);
        assert_eq!(s.inventory().backpack_size(), 60);
        assert!(matches!(
            out.as_slice(),
            [PlayerChange::Inventory(InventoryChange::BackpackSize {
                old: 51,
                new: 60
            })]
        ));
    }

    #[test]
    fn inventory_events_reach_the_model_in_order() {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        entered(&mut s);
        let mut out = Vec::new();
        s.apply(&added(1, 4), &mut out);
        assert_eq!(s.inventory().backpack_size(), 5);
        assert!(matches!(
            out.as_slice(),
            [
                PlayerChange::Inventory(InventoryChange::BackpackSize { old: 0, new: 5 }),
                PlayerChange::Inventory(InventoryChange::Added { item: 1, .. })
            ]
        ));
        out.clear();
        s.apply(
            &inv(InventoryEvent::ItemMoved {
                item: NetItemId(1),
                at: ItemPlace {
                    container: Container::Equipment,
                    slot: 3,
                },
            }),
            &mut out,
        );
        assert_eq!(
            s.inventory().equipped().map(|i| i.id).collect::<Vec<_>>(),
            vec![1]
        );
    }

    #[test]
    fn item_properties_are_bounded_and_dropped_with_their_item() {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        entered(&mut s);
        let mut out = Vec::new();
        // Unknown item: ignored.
        s.apply(
            &inv(InventoryEvent::ItemProperty {
                item: NetItemId(9),
                property: 5,
                value: Some(PropertyValue::Int(1)),
            }),
            &mut out,
        );
        assert!(out.is_empty());
        s.apply(&added(1, 0), &mut out);
        for p in 0..(MAX_PROPERTIES as u32 + 10) {
            s.apply(
                &inv(InventoryEvent::ItemProperty {
                    item: NetItemId(1),
                    property: p + 1,
                    value: Some(PropertyValue::Int(1)),
                }),
                &mut out,
            );
        }
        assert_eq!(s.item_properties(1).count(), MAX_PROPERTIES);
        assert_eq!(s.dropped_properties(), 10);
        s.apply(
            &inv(InventoryEvent::ItemRemoved { item: NetItemId(1) }),
            &mut out,
        );
        assert_eq!(s.item_properties(1).count(), 0);
    }

    #[test]
    fn header_values_follow_character_info_and_level() {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        let mut out = Vec::new();
        s.apply(
            &Event::Character(CharacterEvent::CharacterInfo {
                name: "TAMSIN".into(),
                body: Some(Body::Character { species: 1, sex: 2 }),
                class: Some(23),
                level: Some(12),
                look: None,
            }),
            &mut out,
        );
        entered(&mut s);
        assert_eq!(s.class(), Some(23));
        assert_eq!(s.level(), Some(12));
        out.clear();
        s.apply(
            &Event::Character(CharacterEvent::LevelChanged {
                entity: EntityId(7),
                level: 13,
            }),
            &mut out,
        );
        assert_eq!(out, vec![PlayerChange::Header]);
        assert_eq!(s.level(), Some(13));
        // Another entity's level is not the player's.
        out.clear();
        s.apply(
            &Event::Character(CharacterEvent::LevelChanged {
                entity: EntityId(8),
                level: 50,
            }),
            &mut out,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn wallet_drops_spent_currencies_and_keeps_room() {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        entered(&mut s);
        let mut out = Vec::new();
        let set = |s: &mut PlayerState, name: &str, amount: u64, out: &mut Vec<PlayerChange>| {
            s.apply(
                &inv(InventoryEvent::WalletChanged {
                    currency: Currency::Item(Ref::Literal(name.into())),
                    amount,
                }),
                out,
            );
        };
        // More currencies than the old bound of 256, each spent again.
        for i in 0..300 {
            set(&mut s, &format!("T{i}"), 1, &mut out);
        }
        assert_eq!(s.wallet().len(), 300);
        for i in 0..300 {
            set(&mut s, &format!("T{i}"), 0, &mut out);
        }
        assert!(s.wallet().is_empty(), "spent currencies are not held");
        out.clear();
        set(&mut s, "X", 5, &mut out);
        assert_eq!(s.wallet(), &[(Currency::Item(Ref::Literal("X".into())), 5)]);
        assert!(
            out.contains(&PlayerChange::Npc(crate::npc::NpcChange::Wallet {
                currency: Currency::Item(Ref::Literal("X".into())),
                amount: 5,
            }))
        );
        // Setting an unheld currency to 0 changes nothing; coin stays known at 0.
        out.clear();
        set(&mut s, "Y", 0, &mut out);
        assert!(out.is_empty());
        s.apply(
            &inv(InventoryEvent::WalletChanged {
                currency: Currency::Coin,
                amount: 0,
            }),
            &mut out,
        );
        assert_eq!(s.coin(), Some(0));
        // The bound still holds.
        for i in 0..(MAX_WALLET_ENTRIES + 10) {
            set(&mut s, &format!("B{i}"), 1, &mut out);
        }
        assert_eq!(s.wallet().len(), MAX_WALLET_ENTRIES);
    }

    #[test]
    fn properties_wallet_vitals_and_reset() {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        entered(&mut s);
        let mut out = Vec::new();
        s.apply(
            &Event::Character(CharacterEvent::PropertyChanged {
                entity: EntityId(7),
                property: 100,
                value: Some(PropertyValue::Float(12.5)),
            }),
            &mut out,
        );
        s.apply(
            &inv(InventoryEvent::WalletChanged {
                currency: Currency::Coin,
                amount: 4520,
            }),
            &mut out,
        );
        s.apply(
            &Event::Combat(CombatEvent::VitalChanged {
                entity: EntityId(7),
                vital: Vital::Power,
                current: 10.0,
                max: 20.0,
            }),
            &mut out,
        );
        assert_eq!(s.property(100), Some(&PropertyValue::Float(12.5)));
        assert_eq!(s.coin(), Some(4520));
        assert_eq!(s.vital(VitalKind::Power).map(|v| v.max), Some(20.0));
        out.clear();
        // The same amount again changes nothing.
        s.apply(
            &inv(InventoryEvent::WalletChanged {
                currency: Currency::Coin,
                amount: 4520,
            }),
            &mut out,
        );
        assert!(out.is_empty());
        s.apply(&added(1, 0), &mut out);
        s.apply(
            &Event::Disconnected {
                reason: tessera_net::DisconnectReason::ClientRequested,
            },
            &mut out,
        );
        assert_eq!(out.last(), Some(&PlayerChange::Reset));
        assert!(s.inventory().is_empty());
        assert_eq!(s.coin(), None);
        assert_eq!(s.property(100), None);
    }

    #[test]
    fn a_later_bag_layout_keeps_the_items() {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        entered(&mut s);
        let mut out = Vec::new();
        s.apply(&added(1, 3), &mut out);
        assert_eq!(s.inventory().backpack_size(), 4);
        out.clear();
        let bags = BagLayout {
            base_slots: 45,
            slots_per_bag: 15,
            bags: 6,
        };
        s.set_bags(bags, &mut out);
        assert_eq!(s.inventory().backpack_size(), 45);
        assert_eq!(s.inventory().offered_bags().len(), 3);
        assert_eq!(
            out,
            vec![PlayerChange::Inventory(InventoryChange::BackpackSize {
                old: 4,
                new: 45
            })]
        );
        assert!(s.inventory().item(1).is_some());
    }

    #[test]
    fn the_players_appearance_is_kept() {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        entered(&mut s);
        let mut a = Appearance::new(Body::Character { species: 1, sex: 2 });
        a.equipment
            .push(tessera_net::Equipped::new(DataId(0x7000_0001)));
        let mut out = Vec::new();
        s.apply(
            &Event::AppearanceChanged {
                id: EntityId(7),
                appearance: Box::new(a.clone()),
            },
            &mut out,
        );
        assert!(out.contains(&PlayerChange::Appearance));
        assert_eq!(s.appearance(), Some(&a));
        // Another entity's appearance is not the player's.
        out.clear();
        s.apply(
            &Event::AppearanceChanged {
                id: EntityId(8),
                appearance: Box::new(Appearance::new(Body::Object(DataId(1)))),
            },
            &mut out,
        );
        assert!(out.is_empty());
    }
}
