// SPDX-License-Identifier: GPL-3.0-or-later
//! The session's wallet model for the plugin wallet views (`lua-plugin-api.md` §15.3;
//! `npc-windows.md` §9.6): the coin amount *C* and the ordered entry list *E* of the
//! currencies with a non-zero amount, changed by each `WalletChanged` in arrival order
//! (carried as [`NpcChange::Wallet`]), with the plugin events of the §15.3 model table.
//! [`SessionWallet`] is the [`WalletSource`] the plugin host reads on its thread; one
//! [`SessionWallet`] lives as long as the plugin manager, so its change count and entry IDs
//! continue across world sessions (§15.3 implementer note).
//!
//! The `WalletItem` values ([`WalletItemData`]) are computed once per entry, when it enters
//! *E*, by the caller's fact source ([`wallet_item_facts`] over the install; a literal ref's
//! text and nothing else without one).

use std::sync::{Arc, PoisonError, RwLock};

use tessera_net::domain::{Currency, Ref};
use tessera_plugins::{
    LocalPlayerInfo, LocalPlayerSource, WalletEntry, WalletItemData, WalletSource, WalletViewEvent,
};

use super::state::NpcChange;
use crate::character::PlayerChange;

/// Most entries of *E* (the player state keeps at most this many currencies).
pub const MAX_WALLET_ENTRIES: usize = crate::character::state::MAX_WALLET_ENTRIES;

#[derive(Debug, Default)]
struct Model {
    coin: u64,
    entries: Vec<(Ref, Arc<WalletEntry>)>,
    revision: u64,
    next_id: u64,
}

/// The session's wallet model and the plugin host's [`WalletSource`] (module docs).
#[derive(Debug, Default)]
pub struct SessionWallet {
    model: RwLock<Model>,
}

impl SessionWallet {
    /// An empty wallet (coin 0, no entries, change count 0).
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Applies one frame's player-state changes in order and returns the plugin events
    /// they make (`lua-plugin-api.md` §15.3 model table; leaving the world on
    /// [`PlayerChange::Reset`]). `facts` gives the values of a currency entering *E*.
    pub fn apply(
        &self,
        changes: &[PlayerChange],
        facts: &dyn Fn(&Ref) -> Arc<WalletItemData>,
    ) -> Vec<WalletViewEvent> {
        let mut out = Vec::new();
        let mut m = self.model.write().unwrap_or_else(PoisonError::into_inner);
        for c in changes {
            match c {
                PlayerChange::Npc(NpcChange::Wallet { currency, amount }) => {
                    m.set(currency, *amount, facts, &mut out);
                }
                PlayerChange::Reset => m.leave_world(&mut out),
                _ => {}
            }
        }
        out
    }

    /// Rebuilds *E* and *C* from `wallet` without events (the changes were dropped before
    /// the views saw them); entries keep their IDs when their currency is still held.
    pub fn resync(&self, wallet: &[(Currency, u64)], facts: &dyn Fn(&Ref) -> Arc<WalletItemData>) {
        let mut m = self.model.write().unwrap_or_else(PoisonError::into_inner);
        m.coin = wallet
            .iter()
            .find(|(c, _)| *c == Currency::Coin)
            .map_or(0, |(_, a)| *a);
        let old = std::mem::take(&mut m.entries);
        for (c, a) in wallet {
            let Currency::Item(r) = c else { continue };
            if *a == 0 || m.entries.len() >= MAX_WALLET_ENTRIES {
                continue;
            }
            let entry = match old.iter().find(|(o, _)| o == r) {
                Some((_, e)) => Arc::new(WalletEntry {
                    quantity: *a,
                    ..(**e).clone()
                }),
                None => m.new_entry(r, *a, facts),
            };
            m.entries.push((r.clone(), entry));
        }
        m.revision = m.revision.saturating_add(1);
    }
}

impl Model {
    fn new_entry(
        &mut self,
        r: &Ref,
        amount: u64,
        facts: &dyn Fn(&Ref) -> Arc<WalletItemData>,
    ) -> Arc<WalletEntry> {
        self.next_id = self.next_id.saturating_add(1);
        Arc::new(WalletEntry {
            id: self.next_id,
            info: facts(r),
            quantity: amount,
        })
    }

    fn bump(&mut self) -> u64 {
        self.revision = self.revision.saturating_add(1);
        self.revision
    }

    fn set(
        &mut self,
        currency: &Currency,
        amount: u64,
        facts: &dyn Fn(&Ref) -> Arc<WalletItemData>,
        out: &mut Vec<WalletViewEvent>,
    ) {
        match currency {
            Currency::Coin => {
                if amount != self.coin {
                    self.coin = amount;
                    let revision = self.bump();
                    out.push(WalletViewEvent::MoneyChanged { revision });
                }
            }
            Currency::Item(r) => match self.entries.iter().position(|(c, _)| c == r) {
                None if amount == 0 => {}
                None => {
                    if self.entries.len() >= MAX_WALLET_ENTRIES {
                        tracing::debug!("wallet views hold {MAX_WALLET_ENTRIES} entries; dropped");
                        return;
                    }
                    let entry = self.new_entry(r, amount, facts);
                    self.entries.push((r.clone(), Arc::clone(&entry)));
                    let revision = self.bump();
                    out.push(WalletViewEvent::ItemAdded {
                        revision,
                        index: u32::try_from(self.entries.len()).unwrap_or(u32::MAX),
                        entry,
                    });
                }
                Some(k) if amount == 0 => {
                    let (_, entry) = self.entries.remove(k);
                    let revision = self.bump();
                    out.push(WalletViewEvent::ItemRemoved {
                        revision,
                        index: u32::try_from(k + 1).unwrap_or(u32::MAX),
                        entry,
                    });
                }
                Some(k) => {
                    if self.entries[k].1.quantity == amount {
                        return;
                    }
                    let entry = Arc::new(WalletEntry {
                        quantity: amount,
                        ..(*self.entries[k].1).clone()
                    });
                    self.entries[k].1 = Arc::clone(&entry);
                    let revision = self.bump();
                    out.push(WalletViewEvent::QuantityChanged { revision, entry });
                }
            },
        }
    }

    /// Leaving the world (§15.3): the entries leave from the last to the first, then *C*
    /// becomes 0 with `MoneyChanged` when it was not 0.
    fn leave_world(&mut self, out: &mut Vec<WalletViewEvent>) {
        while let Some((_, entry)) = self.entries.pop() {
            let index = u32::try_from(self.entries.len() + 1).unwrap_or(u32::MAX);
            let revision = self.bump();
            out.push(WalletViewEvent::ItemRemoved {
                revision,
                index,
                entry,
            });
        }
        if self.coin != 0 {
            self.coin = 0;
            let revision = self.bump();
            out.push(WalletViewEvent::MoneyChanged { revision });
        }
    }
}

impl WalletSource for SessionWallet {
    fn revision(&self) -> u64 {
        self.model
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .revision
    }

    fn money(&self) -> u64 {
        self.model
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .coin
    }

    fn entry_count(&self) -> u32 {
        let m = self.model.read().unwrap_or_else(PoisonError::into_inner);
        u32::try_from(m.entries.len()).unwrap_or(u32::MAX)
    }

    fn entry(&self, position: u32) -> Option<Arc<WalletEntry>> {
        let m = self.model.read().unwrap_or_else(PoisonError::into_inner);
        m.entries
            .get(usize::try_from(position).ok()?)
            .map(|(_, e)| Arc::clone(e))
    }

    fn entry_by_id(&self, id: u64) -> Option<Arc<WalletEntry>> {
        let m = self.model.read().unwrap_or_else(PoisonError::into_inner);
        m.entries
            .iter()
            .find(|(_, e)| e.id == id)
            .map(|(_, e)| Arc::clone(e))
    }
}

/// The local player the plugins see with the wallet views: the names of `player` and the
/// wallet of `wallet` (`gameplay::LocalPlayerSource::wallet`).
pub struct WalletPlayer<P: LocalPlayerSource> {
    player: Arc<P>,
    wallet: Arc<SessionWallet>,
}

impl<P: LocalPlayerSource> std::fmt::Debug for WalletPlayer<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalletPlayer").finish_non_exhaustive()
    }
}

impl<P: LocalPlayerSource> WalletPlayer<P> {
    /// `player`'s character with `wallet`.
    #[must_use]
    pub fn new(player: Arc<P>, wallet: Arc<SessionWallet>) -> Arc<Self> {
        Arc::new(Self { player, wallet })
    }
}

impl<P: LocalPlayerSource> LocalPlayerSource for WalletPlayer<P> {
    fn local_player(&self) -> Option<LocalPlayerInfo> {
        self.player.local_player()
    }

    fn wallet(&self) -> Option<Arc<dyn WalletSource>> {
        Some(Arc::clone(&self.wallet) as Arc<dyn WalletSource>)
    }
}

/// The `WalletItem` values of a currency ref (§15.3 getter table): for a literal ref its
/// text and nothing else; for a record the install's values when `data` is given, else an
/// empty record entry.
#[must_use]
pub fn wallet_item_facts(data: Option<&super::data::NpcData>, r: &Ref) -> Arc<WalletItemData> {
    match r {
        Ref::Literal(s) => Arc::new(WalletItemData::literal(s.clone())),
        Ref::Record(did) => Arc::new(match data {
            Some(d) => d.wallet_item(tessera_formats::Did(did.0)),
            None => WalletItemData {
                record: Some(did.0),
                ..WalletItemData::default()
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(s: &str) -> Ref {
        Ref::Literal(s.into())
    }

    fn set(c: Currency, a: u64) -> PlayerChange {
        PlayerChange::Npc(NpcChange::Wallet {
            currency: c,
            amount: a,
        })
    }

    fn facts(r: &Ref) -> Arc<WalletItemData> {
        wallet_item_facts(None, r)
    }

    fn kinds(evs: &[WalletViewEvent]) -> Vec<String> {
        evs.iter()
            .map(|e| match e {
                WalletViewEvent::ItemAdded { index, entry, .. } => {
                    format!("added {index} {}", entry.info.name)
                }
                WalletViewEvent::ItemRemoved { index, entry, .. } => {
                    format!("removed {index} {}", entry.info.name)
                }
                WalletViewEvent::QuantityChanged { entry, .. } => {
                    format!("quantity {} {}", entry.info.name, entry.quantity)
                }
                WalletViewEvent::MoneyChanged { .. } => "money".into(),
            })
            .collect()
    }

    #[test]
    fn model_table_of_section_15_3() {
        let w = SessionWallet::new();
        let a = Currency::Item(lit("A"));
        let b = Currency::Item(lit("B"));
        let c = Currency::Item(lit("C"));
        let ev = w.apply(
            &[
                set(a.clone(), 5),
                set(b.clone(), 7),
                set(c.clone(), 1),
                set(Currency::Item(lit("D")), 0),
                set(Currency::Coin, 50),
                set(Currency::Coin, 50),
            ],
            &facts,
        );
        assert_eq!(kinds(&ev), ["added 1 A", "added 2 B", "added 3 C", "money"]);
        let b_id = w.entry(1).map(|e| e.id).expect("B");
        let ev = w.apply(
            &[set(b.clone(), 0), set(a.clone(), 9), set(a.clone(), 9)],
            &facts,
        );
        assert_eq!(kinds(&ev), ["removed 2 B", "quantity A 9"]);
        assert_eq!(w.entry_count(), 2);
        assert_eq!(w.entry(1).map(|e| e.info.name.clone()), Some("C".into()));
        let ev = w.apply(&[set(b, 4)], &facts);
        assert_eq!(kinds(&ev), ["added 3 B"]);
        assert_ne!(
            w.entry(2).map(|e| e.id),
            Some(b_id),
            "a new entry, a new ID"
        );
        // Revisions grow by one per event and the source reports the latest.
        let r = w.revision();
        let ev = w.apply(&[PlayerChange::Reset], &facts);
        assert_eq!(
            kinds(&ev),
            ["removed 3 B", "removed 2 C", "removed 1 A", "money"]
        );
        let revs: Vec<u64> = ev
            .iter()
            .map(|e| match e {
                WalletViewEvent::ItemAdded { revision, .. }
                | WalletViewEvent::ItemRemoved { revision, .. }
                | WalletViewEvent::QuantityChanged { revision, .. }
                | WalletViewEvent::MoneyChanged { revision } => *revision,
            })
            .collect();
        assert_eq!(revs, vec![r + 1, r + 2, r + 3, r + 4]);
        assert_eq!((w.money(), w.entry_count()), (0, 0));
        // Amounts above 32 bits are kept unchanged.
        w.apply(&[set(Currency::Coin, 1 << 40)], &facts);
        assert_eq!(w.money(), 1 << 40);
    }

    #[test]
    fn literal_and_record_facts_without_an_install() {
        let l = wallet_item_facts(None, &lit("Token"));
        assert_eq!(l.name, "Token");
        assert_eq!((l.image, l.small_image, l.max_quantity), (0, 0, 0));
        let r = wallet_item_facts(None, &Ref::Record(tessera_net::DataId(0x7000_0001)));
        assert_eq!(r.record, Some(0x7000_0001));
        assert!(r.name.is_empty());
    }

    #[test]
    fn resync_keeps_ids_of_currencies_still_held() {
        let w = SessionWallet::new();
        w.apply(&[set(Currency::Item(lit("A")), 1)], &facts);
        let id = w.entry(0).map(|e| e.id);
        w.resync(
            &[
                (Currency::Item(lit("A")), 3),
                (Currency::Item(lit("Z")), 0),
                (Currency::Coin, 9),
            ],
            &facts,
        );
        assert_eq!(w.entry(0).map(|e| (e.id, e.quantity)), id.map(|i| (i, 3)));
        assert_eq!((w.entry_count(), w.money()), (1, 9));
    }
}
