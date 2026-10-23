// SPDX-License-Identifier: GPL-3.0-or-later
//! The NPC windows' session state (decision 0024 P3; `npc-windows.md` §10.1): which feed
//! windows are open and what they hold, as the session's events report it. Fed by
//! [`crate::character::PlayerState::apply`], which every event of a world session (the
//! windowed walker, the headless scenario driver, the tests) already reaches, so every
//! frame loop sees the same state.
//!
//! The state only mirrors events: `DialogueShown`, `QuestOffered`, `VendorShown` and
//! `LootShown` open (or replace) a window, `QuestRewards`, `BuybackShown` and `LootTaken`
//! change one, `WindowClosed` removes it (`npc-windows.md` §5.7: the client never hides a
//! feed window before `WindowClosed`). A session that ends (`Disconnected`, or a second
//! `WorldEntered`) drops every window. The entities the windows name (speaker, giver,
//! vendor, loot source) are remembered from `EntitySpawned` and `EntityCreated` for their
//! name, kind and body record; an `EntityDespawned` of a window's entity (or a
//! `StaticsCleared` while it is an entity created with a tag) marks that window for the one
//! `CloseWindow` of §5.7's Tessera rule.
//!
//! # Hostile input
//!
//! At most [`MAX_WINDOWS`] windows (a further window is dropped with a debug line),
//! [`MAX_ENTRIES`] entries per list, [`MAX_OPTIONS`] options per page, [`MAX_REWARDS`]
//! reward entries and [`MAX_ENTITIES`] remembered entities; literal texts longer than
//! [`MAX_TEXT_BYTES`] are cut on a character boundary.

use std::collections::{BTreeMap, BTreeSet};

use tessera_net::domain::{
    BuybackEntry, Currency, DialogueOption, LootEntry, MAX_REWARD_ENTRIES, NpcEvent, Ref,
    RewardEntry, Text, VendorEntry, WindowId,
};
use tessera_net::{DataId, EntityId, EntityKind, Event};

/// Most feed windows kept open at once (the feed keeps at most one per kind, §5.6; a
/// server may open more; Tessera bound).
pub const MAX_WINDOWS: usize = 64;
/// Most entries of one vendor, buy-back or loot list (`scenario-feed.md` §6.3: 1,024).
pub const MAX_ENTRIES: usize = 1024;
/// Most options of one dialogue page (the feed allows 16; `tessera-ui` shows at most 64).
pub const MAX_OPTIONS: usize = 64;
/// Most reward entries of one offer (`scenario-feed.md` §9.2).
pub const MAX_REWARDS: usize = MAX_REWARD_ENTRIES;
/// Most entities remembered for their names and bodies (Tessera bound; further entities
/// are not remembered until others despawn).
pub const MAX_ENTITIES: usize = 65_536;
/// Longest literal text kept (bytes; the feed's chat limit is smaller).
pub const MAX_TEXT_BYTES: usize = 8192;

/// The kind of a feed window (`scenario-feed.md` §5.6; the NPC kinds of P3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FeedKind {
    /// `dialogue`: a dialogue page.
    Dialogue,
    /// `quest-offer`.
    QuestOffer,
    /// `vendor` (also shown as the barter window, `npc-windows.md` §7.3).
    Vendor,
    /// `loot`.
    Loot,
}

impl FeedKind {
    /// The kind's word of `scenario-feed.md` §5.6.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Dialogue => "dialogue",
            Self::QuestOffer => "quest-offer",
            Self::Vendor => "vendor",
            Self::Loot => "loot",
        }
    }
}

/// What a feed window holds.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowContent {
    /// `DialogueShown`.
    Dialogue {
        /// Who speaks.
        speaker: Option<EntityId>,
        /// The page text.
        text: Text,
        /// The options, in ascending option number.
        options: Vec<DialogueOption>,
    },
    /// `QuestOffered`, with the latest `QuestRewards` of the window.
    QuestOffer {
        /// The quest.
        quest: Ref,
        /// Who offers it.
        giver: Option<EntityId>,
        /// The reward amounts (`QuestRewards`; empty before one arrives).
        rewards: Vec<RewardEntry>,
    },
    /// `VendorShown`, with the latest `BuybackShown` of the window.
    Vendor {
        /// The vendor.
        vendor: Option<EntityId>,
        /// The entries, in feed order.
        entries: Vec<VendorEntry>,
        /// The buy-back page (`None` until a `BuybackShown`; the tab is hidden until then,
        /// `npc-windows.md` §7.8).
        buyback: Option<Vec<BuybackEntry>>,
    },
    /// `LootShown`, with the entries taken since (`LootTaken`).
    Loot {
        /// The source.
        source: Option<EntityId>,
        /// The entries, in feed order.
        entries: Vec<LootEntry>,
        /// Coin in the loot.
        coin: u64,
        /// The entry numbers taken so far.
        taken: BTreeSet<u16>,
    },
}

impl WindowContent {
    /// The window's kind.
    #[must_use]
    pub fn kind(&self) -> FeedKind {
        match self {
            Self::Dialogue { .. } => FeedKind::Dialogue,
            Self::QuestOffer { .. } => FeedKind::QuestOffer,
            Self::Vendor { .. } => FeedKind::Vendor,
            Self::Loot { .. } => FeedKind::Loot,
        }
    }

    /// The window entity (`npc-windows.md` §2): speaker, giver, vendor or loot source.
    #[must_use]
    pub fn entity(&self) -> Option<EntityId> {
        match self {
            Self::Dialogue { speaker, .. } => *speaker,
            Self::QuestOffer { giver, .. } => *giver,
            Self::Vendor { vendor, .. } => *vendor,
            Self::Loot { source, .. } => *source,
        }
    }
}

/// One open feed window.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedWindow {
    /// The window.
    pub id: WindowId,
    /// What it holds.
    pub content: WindowContent,
    /// Increases with every change of the window (a UI rebuilds when it differs from the
    /// one it showed).
    pub revision: u64,
    /// The open order (a later window is drawn above an earlier one, §4 Tessera rule).
    pub opened: u64,
    /// The revision of the window's last content replacement (an open, or a later window
    /// of the same kind, `scenario-feed.md` §5.6); changes that only amend the content
    /// (`QuestRewards`, `BuybackShown`, `LootTaken`) leave it. A UI that showed this
    /// content needs only the amendments.
    pub shown: u64,
    /// The window entity despawned while the window was open (§5.7 Tessera rule: the
    /// client sends `CloseWindow` once).
    pub entity_gone: bool,
}

/// What the session told about an entity a window may name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityInfo {
    /// Its kind.
    pub kind: EntityKind,
    /// Its display name (may be empty).
    pub name: String,
    /// Its body record (`EntityCreated.class`): the NPC record dialogue texts and action
    /// labels read (`npc-windows.md` §5.3, §6.4).
    pub body: Option<DataId>,
}

/// One change of the NPC windows' state, in the order it happened. Carried to the
/// consumers as [`crate::character::PlayerChange::Npc`].
#[derive(Debug, Clone, PartialEq)]
pub enum NpcChange {
    /// A window opened or its content was replaced.
    Shown {
        /// The window.
        window: WindowId,
        /// Its kind.
        kind: FeedKind,
    },
    /// The reward amounts of a quest offer arrived (`QuestRewards`).
    Rewards {
        /// The window.
        window: WindowId,
    },
    /// The buy-back page of a vendor window arrived (`BuybackShown`).
    Buyback {
        /// The window.
        window: WindowId,
    },
    /// An entry of a loot window was taken (`LootTaken`).
    LootTaken {
        /// The window.
        window: WindowId,
        /// The entry.
        entry: u16,
    },
    /// A window closed (`WindowClosed`).
    Closed {
        /// The window.
        window: WindowId,
    },
    /// The entity of an open window despawned.
    EntityGone {
        /// The window.
        window: WindowId,
    },
    /// A currency's amount changed (`WalletChanged` that changed the wallet; the wallet
    /// views of `lua-plugin-api.md` §15.3 derive their events from these, in order).
    Wallet {
        /// The currency.
        currency: Currency,
        /// Its new amount.
        amount: u64,
    },
    /// The session ended: every window was dropped.
    Cleared,
}

/// The NPC windows' session state (module docs).
#[derive(Debug, Clone, Default)]
pub struct NpcWindows {
    windows: BTreeMap<WindowId, FeedWindow>,
    entities: BTreeMap<EntityId, EntityInfo>,
    /// The remembered entities created with a tag (placed by the world data): a
    /// `StaticsCleared` removes them (`Event::StaticsCleared`).
    tagged: BTreeSet<EntityId>,
    /// The bodies the entities' appearances name (`EntityCreated`, `AppearanceChanged`; the
    /// player's arrives with an `AppearanceChanged` after `WorldEntered`): what their
    /// use-range cylinders are built from (`interaction.md` §3.4.1 I6a).
    figures: BTreeMap<EntityId, tessera_net::Body>,
    player: Option<EntityId>,
    revision: u64,
    opened: u64,
    dropped: u64,
}

/// Cuts a literal text to [`MAX_TEXT_BYTES`] on a character boundary.
fn cut_text(t: &Text) -> Text {
    match t {
        Text::Literal(s) if s.len() > MAX_TEXT_BYTES => {
            let mut end = MAX_TEXT_BYTES;
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            Text::Literal(s[..end].to_owned())
        }
        other => other.clone(),
    }
}

impl NpcWindows {
    /// No window open.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Increases with every change of any window (a UI resyncs when it differs).
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The open windows, by window ID.
    pub fn windows(&self) -> impl Iterator<Item = &FeedWindow> {
        self.windows.values()
    }

    /// The open window `id`.
    #[must_use]
    pub fn window(&self, id: WindowId) -> Option<&FeedWindow> {
        self.windows.get(&id)
    }

    /// The open window of `kind` opened last.
    #[must_use]
    pub fn latest(&self, kind: FeedKind) -> Option<&FeedWindow> {
        self.windows
            .values()
            .filter(|w| w.content.kind() == kind)
            .max_by_key(|w| w.opened)
    }

    /// What the session told about `id`.
    #[must_use]
    pub fn entity(&self, id: EntityId) -> Option<&EntityInfo> {
        self.entities.get(&id)
    }

    /// The body `id`'s appearance names (the player's included), when the session told it.
    #[must_use]
    pub fn figure(&self, id: EntityId) -> Option<tessera_net::Body> {
        self.figures.get(&id).copied()
    }

    /// The entities the session told about.
    pub fn entities(&self) -> impl Iterator<Item = (EntityId, &EntityInfo)> {
        self.entities.iter().map(|(k, v)| (*k, v))
    }

    /// Windows dropped because [`MAX_WINDOWS`] were open.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    fn bump(&mut self) -> u64 {
        self.revision = self.revision.saturating_add(1);
        self.revision
    }

    /// Drops every window (the session ended).
    pub fn clear(&mut self, out: &mut Vec<NpcChange>) {
        self.entities.clear();
        self.tagged.clear();
        self.figures.clear();
        if !self.windows.is_empty() {
            self.windows.clear();
            self.bump();
            out.push(NpcChange::Cleared);
        }
    }

    /// Opens window `id` with `content`, or replaces its content (`scenario-feed.md` §5.6:
    /// a window of a kind already open keeps its ID and gets new content).
    fn show(&mut self, id: WindowId, content: WindowContent, out: &mut Vec<NpcChange>) {
        let kind = content.kind();
        let revision = self.bump();
        if let Some(w) = self.windows.get_mut(&id) {
            w.content = content;
            w.revision = revision;
            w.shown = revision;
            w.entity_gone = false;
        } else {
            if self.windows.len() >= MAX_WINDOWS {
                self.dropped = self.dropped.saturating_add(1);
                tracing::debug!(
                    window = id.0,
                    "{MAX_WINDOWS} NPC windows open; window dropped"
                );
                return;
            }
            self.opened = self.opened.saturating_add(1);
            self.windows.insert(
                id,
                FeedWindow {
                    id,
                    content,
                    revision,
                    opened: self.opened,
                    shown: revision,
                    entity_gone: false,
                },
            );
        }
        out.push(NpcChange::Shown { window: id, kind });
    }

    /// Changes window `id` with `f` when it exists; returns whether `f` changed it.
    fn change(&mut self, id: WindowId, f: impl FnOnce(&mut WindowContent) -> bool) -> bool {
        let next = self.revision.saturating_add(1);
        let changed = match self.windows.get_mut(&id) {
            Some(w) => {
                let c = f(&mut w.content);
                if c {
                    w.revision = next;
                }
                c
            }
            None => false,
        };
        if changed {
            self.revision = next;
        }
        changed
    }

    fn remember(&mut self, id: EntityId, info: EntityInfo, tagged: bool) {
        if !self.entities.contains_key(&id) && self.entities.len() >= MAX_ENTITIES {
            tracing::debug!(
                entity = id.0,
                "{MAX_ENTITIES} entities remembered; not kept"
            );
            return;
        }
        self.entities.insert(id, info);
        if tagged {
            self.tagged.insert(id);
        } else {
            self.tagged.remove(&id);
        }
    }

    /// Remembers `id`'s body (at most [`MAX_ENTITIES`] entities besides the player).
    fn remember_figure(&mut self, id: EntityId, body: tessera_net::Body) {
        if !self.figures.contains_key(&id)
            && self.figures.len() >= MAX_ENTITIES
            && self.player != Some(id)
        {
            return;
        }
        self.figures.insert(id, body);
    }

    /// `id` is gone (`EntityDespawned`, or a tagged entity at `StaticsCleared`): its
    /// windows are marked for the one `CloseWindow` of §5.7's Tessera rule.
    fn despawn(&mut self, id: EntityId, out: &mut Vec<NpcChange>) {
        self.entities.remove(&id);
        self.tagged.remove(&id);
        self.figures.remove(&id);
        let gone: Vec<WindowId> = self
            .windows
            .values()
            .filter(|w| w.content.entity() == Some(id) && !w.entity_gone)
            .map(|w| w.id)
            .collect();
        for window in gone {
            if let Some(w) = self.windows.get_mut(&window) {
                w.entity_gone = true;
            }
            self.bump();
            out.push(NpcChange::EntityGone { window });
        }
    }

    /// Applies one session event; its changes are appended to `out`, in order.
    pub fn apply(&mut self, ev: &Event, out: &mut Vec<NpcChange>) {
        match ev {
            Event::WorldEntered { player, .. } => {
                if self.player.is_some() {
                    // A second entry starts a new session model.
                    self.clear(out);
                }
                self.player = Some(*player);
            }
            Event::Disconnected { .. } => {
                self.clear(out);
                self.player = None;
            }
            Event::EntitySpawned { id, kind, name, .. } => self.remember(
                *id,
                EntityInfo {
                    kind: *kind,
                    name: name.clone(),
                    body: None,
                },
                false,
            ),
            Event::EntityCreated {
                id,
                kind,
                name,
                class,
                tag,
                appearance,
                ..
            } => {
                self.remember(
                    *id,
                    EntityInfo {
                        kind: *kind,
                        name: name.clone(),
                        body: *class,
                    },
                    tag.is_some(),
                );
                self.remember_figure(*id, appearance.body);
            }
            Event::AppearanceChanged { id, appearance } => {
                self.remember_figure(*id, appearance.body);
            }
            Event::EntityDespawned { id } => self.despawn(*id, out),
            // Every tagged entity is gone, as if each had been despawned (session.rs).
            Event::StaticsCleared => {
                for id in std::mem::take(&mut self.tagged) {
                    self.despawn(id, out);
                }
            }
            Event::Npc(n) => self.apply_npc(n, out),
            _ => {}
        }
    }

    fn apply_npc(&mut self, ev: &NpcEvent, out: &mut Vec<NpcChange>) {
        match ev {
            NpcEvent::DialogueShown {
                window,
                speaker,
                text,
                options,
            } => {
                let mut options: Vec<DialogueOption> = options
                    .iter()
                    .take(MAX_OPTIONS)
                    .map(|o| DialogueOption {
                        option: o.option,
                        text: cut_text(&o.text),
                    })
                    .collect();
                options.sort_by_key(|o| o.option);
                let content = WindowContent::Dialogue {
                    speaker: *speaker,
                    text: cut_text(text),
                    options,
                };
                self.show(*window, content, out);
            }
            NpcEvent::QuestOffered {
                window,
                quest,
                giver,
            } => {
                let content = WindowContent::QuestOffer {
                    quest: quest.clone(),
                    giver: *giver,
                    rewards: Vec::new(),
                };
                self.show(*window, content, out);
            }
            NpcEvent::QuestRewards { window, entries } => {
                let rewards: Vec<RewardEntry> = entries
                    .iter()
                    .filter(|e| e.is_valid())
                    .take(MAX_REWARDS)
                    .cloned()
                    .collect();
                if self.change(*window, |c| match c {
                    WindowContent::QuestOffer { rewards: r, .. } => {
                        *r = rewards;
                        true
                    }
                    _ => false,
                }) {
                    out.push(NpcChange::Rewards { window: *window });
                }
            }
            NpcEvent::VendorShown {
                window,
                vendor,
                entries,
            } => {
                // A later `vendor` step on the open window keeps its buy-back page
                // (`scenario-feed.md` §6.3 OPEN (a)).
                let buyback = match self.windows.get(window).map(|w| &w.content) {
                    Some(WindowContent::Vendor { buyback, .. }) => buyback.clone(),
                    _ => None,
                };
                let content = WindowContent::Vendor {
                    vendor: *vendor,
                    entries: entries.iter().take(MAX_ENTRIES).cloned().collect(),
                    buyback,
                };
                self.show(*window, content, out);
            }
            NpcEvent::BuybackShown { window, entries } => {
                let list: Vec<BuybackEntry> = entries.iter().take(MAX_ENTRIES).cloned().collect();
                if self.change(*window, |c| match c {
                    WindowContent::Vendor { buyback, .. } => {
                        *buyback = Some(list);
                        true
                    }
                    _ => false,
                }) {
                    out.push(NpcChange::Buyback { window: *window });
                }
            }
            NpcEvent::LootShown {
                window,
                source,
                entries,
                coin,
            } => {
                let content = WindowContent::Loot {
                    source: *source,
                    entries: entries.iter().take(MAX_ENTRIES).cloned().collect(),
                    coin: *coin,
                    taken: BTreeSet::new(),
                };
                self.show(*window, content, out);
            }
            NpcEvent::LootTaken { window, entry } => {
                if self.change(*window, |c| match c {
                    WindowContent::Loot { entries, taken, .. } => {
                        entries.iter().any(|e| e.entry == *entry) && taken.insert(*entry)
                    }
                    _ => false,
                }) {
                    out.push(NpcChange::LootTaken {
                        window: *window,
                        entry: *entry,
                    });
                }
            }
            NpcEvent::WindowClosed { window } if self.windows.remove(window).is_some() => {
                self.bump();
                out.push(NpcChange::Closed { window: *window });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::{Price, RewardKind};
    use tessera_net::{DisconnectReason, Transform, Vec3};

    fn npc(e: NpcEvent) -> Event {
        Event::Npc(e)
    }

    fn apply(s: &mut NpcWindows, ev: Event) -> Vec<NpcChange> {
        let mut out = Vec::new();
        s.apply(&ev, &mut out);
        out
    }

    fn lit(s: &str) -> Ref {
        Ref::Literal(s.into())
    }

    #[test]
    fn windows_open_change_and_close_only_on_events() {
        let mut s = NpcWindows::new();
        let ch = apply(
            &mut s,
            Event::EntitySpawned {
                id: EntityId(9),
                kind: EntityKind::NonPlayer,
                name: "Grocer".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
        );
        assert!(ch.is_empty());
        assert_eq!(
            s.entity(EntityId(9)).map(|e| e.name.as_str()),
            Some("Grocer")
        );
        let ch = apply(
            &mut s,
            npc(NpcEvent::DialogueShown {
                window: WindowId(1),
                speaker: Some(EntityId(9)),
                text: Text::Literal("Hello".into()),
                options: vec![
                    DialogueOption {
                        option: 2,
                        text: Text::Literal("B".into()),
                    },
                    DialogueOption {
                        option: 1,
                        text: Text::Literal("A".into()),
                    },
                ],
            }),
        );
        assert_eq!(
            ch,
            vec![NpcChange::Shown {
                window: WindowId(1),
                kind: FeedKind::Dialogue
            }]
        );
        let Some(WindowContent::Dialogue { options, .. }) =
            s.window(WindowId(1)).map(|w| &w.content)
        else {
            panic!("dialogue");
        };
        assert_eq!(options[0].option, 1, "ascending option number");
        let r1 = s.revision();
        // Loot: taken entries; unknown entries and windows change nothing.
        apply(
            &mut s,
            npc(NpcEvent::LootShown {
                window: WindowId(2),
                source: None,
                entries: vec![
                    LootEntry {
                        entry: 1,
                        what: lit("Strap"),
                        quantity: 1,
                    },
                    LootEntry {
                        entry: 2,
                        what: lit("Buckle"),
                        quantity: 3,
                    },
                ],
                coin: 75,
            }),
        );
        assert!(s.revision() > r1);
        assert_eq!(
            apply(
                &mut s,
                npc(NpcEvent::LootTaken {
                    window: WindowId(2),
                    entry: 2
                })
            ),
            vec![NpcChange::LootTaken {
                window: WindowId(2),
                entry: 2
            }]
        );
        for (w, e) in [(2, 2), (2, 9), (7, 1), (1, 1)] {
            assert!(
                apply(
                    &mut s,
                    npc(NpcEvent::LootTaken {
                        window: WindowId(w),
                        entry: e
                    })
                )
                .is_empty()
            );
        }
        // The speaker despawns: the dialogue window is marked once, not closed.
        assert_eq!(
            apply(&mut s, Event::EntityDespawned { id: EntityId(9) }),
            vec![NpcChange::EntityGone {
                window: WindowId(1)
            }]
        );
        assert!(apply(&mut s, Event::EntityDespawned { id: EntityId(9) }).is_empty());
        assert!(s.window(WindowId(1)).is_some_and(|w| w.entity_gone));
        assert_eq!(
            apply(
                &mut s,
                npc(NpcEvent::WindowClosed {
                    window: WindowId(1)
                })
            ),
            vec![NpcChange::Closed {
                window: WindowId(1)
            }]
        );
        assert!(
            apply(
                &mut s,
                npc(NpcEvent::WindowClosed {
                    window: WindowId(1)
                })
            )
            .is_empty()
        );
        assert_eq!(
            apply(
                &mut s,
                Event::Disconnected {
                    reason: DisconnectReason::ClientRequested
                }
            ),
            vec![NpcChange::Cleared]
        );
        assert_eq!(s.windows().count(), 0);
    }

    #[test]
    fn rewards_and_buyback_attach_to_their_window_kind() {
        let mut s = NpcWindows::new();
        apply(
            &mut s,
            npc(NpcEvent::QuestOffered {
                window: WindowId(3),
                quest: lit("Lost satchel"),
                giver: None,
            }),
        );
        apply(
            &mut s,
            npc(NpcEvent::VendorShown {
                window: WindowId(4),
                vendor: None,
                entries: vec![VendorEntry {
                    entry: 1,
                    what: lit("Bread"),
                    price: Price {
                        coin: 12,
                        items: Vec::new(),
                    },
                    stock: None,
                }],
            }),
        );
        let reward = RewardEntry {
            kind: RewardKind::Coin,
            amount: 1250,
            what: None,
        };
        let bad = RewardEntry {
            kind: RewardKind::Reputation,
            amount: 5,
            what: None,
        };
        // Rewards on the vendor window and buy-back on the offer change nothing.
        assert!(
            apply(
                &mut s,
                npc(NpcEvent::QuestRewards {
                    window: WindowId(4),
                    entries: vec![reward.clone()]
                })
            )
            .is_empty()
        );
        assert!(
            apply(
                &mut s,
                npc(NpcEvent::BuybackShown {
                    window: WindowId(3),
                    entries: Vec::new()
                })
            )
            .is_empty()
        );
        apply(
            &mut s,
            npc(NpcEvent::QuestRewards {
                window: WindowId(3),
                entries: vec![reward.clone(), bad],
            }),
        );
        let Some(WindowContent::QuestOffer { rewards, .. }) =
            s.window(WindowId(3)).map(|w| &w.content)
        else {
            panic!("offer");
        };
        assert_eq!(rewards, &vec![reward], "invalid entries are dropped");
        apply(
            &mut s,
            npc(NpcEvent::BuybackShown {
                window: WindowId(4),
                entries: vec![BuybackEntry {
                    entry: 1,
                    what: lit("Lamp"),
                    quantity: 1,
                    price: Price::default(),
                }],
            }),
        );
        // A later vendor step keeps the buy-back page (scenario-feed.md §6.3 OPEN (a)).
        apply(
            &mut s,
            npc(NpcEvent::VendorShown {
                window: WindowId(4),
                vendor: None,
                entries: Vec::new(),
            }),
        );
        assert!(matches!(
            s.window(WindowId(4)).map(|w| &w.content),
            Some(WindowContent::Vendor {
                buyback: Some(b),
                ..
            }) if b.len() == 1
        ));
        assert_eq!(
            s.latest(FeedKind::QuestOffer).map(|w| w.id),
            Some(WindowId(3))
        );
    }

    fn created(id: u64, tag: Option<u64>) -> Event {
        Event::EntityCreated {
            id: EntityId(id),
            kind: EntityKind::NonPlayer,
            name: format!("npc {id}"),
            class: None,
            location: tessera_net::Location::new(tessera_net::RegionId(1), 0.0, 0.0, 0.0),
            heading: 0.0,
            appearance: Box::new(tessera_net::Appearance::new(tessera_net::Body::Object(
                DataId(0x7000_0001),
            ))),
            tag,
        }
    }

    #[test]
    fn statics_cleared_despawns_only_tagged_entities() {
        let mut s = NpcWindows::new();
        apply(&mut s, created(5, Some(77)));
        apply(&mut s, created(6, None));
        for (w, e) in [(1, 5), (2, 6)] {
            apply(
                &mut s,
                npc(NpcEvent::DialogueShown {
                    window: WindowId(w),
                    speaker: Some(EntityId(e)),
                    text: Text::Literal("Hi".into()),
                    options: Vec::new(),
                }),
            );
        }
        assert_eq!(
            apply(&mut s, Event::StaticsCleared),
            vec![NpcChange::EntityGone {
                window: WindowId(1)
            }]
        );
        assert!(s.window(WindowId(1)).is_some_and(|w| w.entity_gone));
        assert!(s.window(WindowId(2)).is_some_and(|w| !w.entity_gone));
        assert!(s.entity(EntityId(5)).is_none());
        assert!(s.entity(EntityId(6)).is_some());
        // Nothing tagged is left: a second clear changes nothing.
        assert!(apply(&mut s, Event::StaticsCleared).is_empty());
        // A tagged entity re-created without a tag is no longer cleared.
        apply(&mut s, created(7, Some(1)));
        apply(&mut s, created(7, None));
        apply(&mut s, Event::StaticsCleared);
        assert!(s.entity(EntityId(7)).is_some());
    }

    #[test]
    fn shown_follows_replacements_not_amendments() {
        let mut s = NpcWindows::new();
        let loot = |what: &str| {
            npc(NpcEvent::LootShown {
                window: WindowId(3),
                source: None,
                entries: vec![LootEntry {
                    entry: 1,
                    what: lit(what),
                    quantity: 1,
                }],
                coin: 0,
            })
        };
        apply(&mut s, loot("Strap"));
        let first = s.window(WindowId(3)).map(|w| w.shown).unwrap();
        apply(
            &mut s,
            npc(NpcEvent::LootTaken {
                window: WindowId(3),
                entry: 1,
            }),
        );
        let w = s.window(WindowId(3)).unwrap();
        assert_eq!(w.shown, first, "a taken entry amends the content");
        assert!(w.revision > first);
        apply(&mut s, loot("Buckle"));
        let w = s.window(WindowId(3)).unwrap();
        assert!(w.shown > first, "a later loot window replaces the content");
        assert_eq!(w.shown, w.revision);
    }

    #[test]
    fn bounds_hold_for_hostile_sessions() {
        let mut s = NpcWindows::new();
        for w in 0..(MAX_WINDOWS as u32 + 10) {
            apply(
                &mut s,
                npc(NpcEvent::LootShown {
                    window: WindowId(w),
                    source: None,
                    entries: (0..2000)
                        .map(|e| LootEntry {
                            entry: e,
                            what: lit("x"),
                            quantity: 1,
                        })
                        .collect(),
                    coin: 0,
                }),
            );
        }
        assert_eq!(s.windows().count(), MAX_WINDOWS);
        assert_eq!(s.dropped(), 10);
        assert!(s.windows().all(|w| matches!(
            &w.content,
            WindowContent::Loot { entries, .. } if entries.len() == MAX_ENTRIES
        )));
        let long = "é".repeat(MAX_TEXT_BYTES);
        apply(
            &mut s,
            npc(NpcEvent::DialogueShown {
                window: WindowId(0),
                speaker: None,
                text: Text::Literal(long),
                options: Vec::new(),
            }),
        );
        let Some(WindowContent::Dialogue {
            text: Text::Literal(t),
            ..
        }) = s.window(WindowId(0)).map(|w| &w.content)
        else {
            panic!("dialogue");
        };
        assert!(t.len() <= MAX_TEXT_BYTES && t.chars().all(|c| c == 'é'));
    }
}
