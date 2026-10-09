// SPDX-License-Identifier: GPL-3.0-or-later
//! The data the game widgets show, as small traits the client implements. In the M5
//! sandbox the client answers with what `tessera-net`'s sandbox backend models; every
//! other value is **absent** (`game-panels.md` §2 "Local player").

use std::collections::BTreeMap;

use tessera_formats::property::PropertyValue;

use super::map::GameMap;

/// An account or player test of the `HideWhenField` hide test (`game-panels.md` §6.2 rows
/// 1–10 and 14). [`GameData::hide_condition`] answers whether the hiding condition holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HideCondition {
    /// Row 1: the store is disabled for the account.
    StoreDisabled,
    /// Row 2: the account test for this value **fails**.
    RipCordFails(u32),
    /// Row 3: the account test for this value **passes**.
    RipCordPasses(u32),
    /// Row 4: the account is not an administrator.
    NotAdmin,
    /// Row 5: the account is not a subscriber.
    NotSubscriber,
    /// Row 6: the account is a subscriber.
    Subscriber,
    /// Row 7: the player is a monster player.
    MonsterPlayer,
    /// Row 8: the player is in a session-play character.
    SessionPlayer,
    /// Row 9: the player is in PvP.
    PvP,
    /// Row 10: the client's player-kind test fails.
    PlayerKindFails,
    /// Row 14: the player is not on a war-steed.
    NotOnWarsteed,
}

/// The local player, the account and the property dictionary, as the hide test and the
/// other game widgets read them.
///
/// Every method has the sandbox answer as its default: a local player exists, has no
/// properties, the dictionary knows no types or defaults, and no account or player
/// condition holds (`game-panels.md` §6.2 OPEN: the sandbox answer for row 7 is "not a
/// monster player").
pub trait GameData {
    /// Whether a local player exists (`game-panels.md` §6.2: without one the hide test
    /// stops after row 6).
    fn has_local_player(&self) -> bool {
        true
    }

    /// The local player's value of property `pid`, or `None` when absent.
    fn player_property(&self, pid: u32) -> Option<PropertyValue> {
        let _ = pid;
        None
    }

    /// The property-dictionary type code of `pid` (`property-system.md` §3), when known.
    fn property_type(&self, pid: u32) -> Option<u32> {
        let _ = pid;
        None
    }

    /// The property-dictionary default of `pid` (`property-system.md` §2.2, flag 0x800),
    /// when it has one.
    fn property_default(&self, pid: u32) -> Option<PropertyValue> {
        let _ = pid;
        None
    }

    /// Whether an account or player condition holds.
    fn hide_condition(&self, c: HideCondition) -> bool {
        let _ = c;
        false
    }
}

/// A [`GameData`] backed by maps: the sandbox player and a dictionary excerpt.
#[derive(Debug, Clone)]
pub struct SandboxData {
    /// Whether a local player exists.
    pub local_player: bool,
    /// The player's properties.
    pub player: BTreeMap<u32, PropertyValue>,
    /// Dictionary type codes.
    pub types: BTreeMap<u32, u32>,
    /// Dictionary defaults.
    pub defaults: BTreeMap<u32, PropertyValue>,
    /// Conditions that hold.
    pub conditions: Vec<HideCondition>,
}

impl Default for SandboxData {
    fn default() -> Self {
        Self {
            local_player: true,
            player: BTreeMap::new(),
            types: BTreeMap::new(),
            defaults: BTreeMap::new(),
            conditions: Vec::new(),
        }
    }
}

impl GameData for SandboxData {
    fn has_local_player(&self) -> bool {
        self.local_player
    }

    fn player_property(&self, pid: u32) -> Option<PropertyValue> {
        self.player.get(&pid).cloned()
    }

    fn property_type(&self, pid: u32) -> Option<u32> {
        self.types.get(&pid).copied()
    }

    fn property_default(&self, pid: u32) -> Option<PropertyValue> {
        self.defaults.get(&pid).cloned()
    }

    fn hide_condition(&self, c: HideCondition) -> bool {
        self.conditions.contains(&c)
    }
}

/// The answer of a map-record lookup: the records load on the asset workers, so the
/// answer may come later (`game-panels.md` §8.4.2: the grid is built when every requested
/// record has arrived).
#[derive(Debug, Clone, PartialEq)]
pub enum MapLookup {
    /// The record has arrived; `None` when it does not exist or has no `UI_Map_GameMap`.
    Ready(Option<GameMap>),
    /// Requested, not arrived yet.
    Pending,
}

/// Map records for the minimap and the world map.
pub trait MapSource {
    /// The `UI_Map_GameMap` of landblock-properties record 0x804RXXYY (`game-panels.md`
    /// §8.4.2; region *R*, landblock (*X*, *Y*)). Implementations start the load on the
    /// first call and answer [`MapLookup::Pending`] until it arrives.
    fn landblock_map(&mut self, region: u8, x: u8, y: u8) -> MapLookup;

    /// The dungeon map for an interior position (`game-panels.md` §8.5.1 Tessera rule: the
    /// cell's `Dungeon_DID`, its `Dungeon_MapData`, that set's `UI_Map_GameMap`).
    fn dungeon_map(&mut self, region: u8, x: u8, y: u8, cell: u16) -> MapLookup {
        let _ = (region, x, y, cell);
        MapLookup::Ready(None)
    }
}

/// One quickslot shortcut (`game-panels.md` §10.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shortcut {
    /// An empty slot.
    Empty,
    /// A shortcut of the given type (the value of the shortcut-type data enum,
    /// `lua-plugin-api.md` §15, A.1).
    Kind(u32),
}

/// The quickslot shortcuts and their key labels (`game-panels.md` §10).
pub trait Shortcuts {
    /// The shortcut at a shortcut index (default: empty, the sandbox).
    fn shortcut(&self, index: i32) -> Shortcut {
        let _ = index;
        Shortcut::Empty
    }

    /// The label of the key bound to a shortcut index (`game-panels.md` §10.1 OPEN Tessera
    /// rule: Tessera's own short name of the first key bound, or none).
    fn key_label(&self, index: i32) -> Option<String> {
        let _ = index;
        None
    }

    /// The content template (an element ID of the `quickslots` layout) for a non-empty
    /// shortcut type (`game-panels.md` §10.3 OPEN: the mapping is not known; default none).
    fn content_template(&self, kind: u32) -> Option<u32> {
        let _ = kind;
        None
    }
}

/// The sandbox's shortcuts: every slot empty.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoShortcuts;

impl Shortcuts for NoShortcuts {}
