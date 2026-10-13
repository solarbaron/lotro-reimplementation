// SPDX-License-Identifier: GPL-3.0-or-later
//! The session's mount model of the local player (`lua-plugin-api.md` §15.5, Tessera rule
//! P103) and the plugin mount source over it.
//!
//! [`SessionMount`] keeps one [`MountModel`] for the plugin manager's lifetime (its
//! revision and period ids continue across world sessions) and feeds it:
//!
//! - with a feed (a scenario session, or a server backend that reports mounts): from each
//!   frame's `MountChanged` and `MountVitalChanged` of the local player
//!   ([`SessionMount::apply`]); the class is the feed's `combat` when scripted, else the
//!   war-steed rule on the ref's record (`mounts.md` §2.8, §4.1, through
//!   `tessera-gamedata`'s [`MountPresenter::plugin_class`]), a literal ref basic;
//! - without a feed (the plain offline sandbox, `walker.md` §16): from the walker's mount
//!   state ([`SessionMount::walker_state`]): entering state 3 (mounted) is a change to the
//!   walker's mount effect, leaving it a change to none.
//!
//! Every model change gives a [`MountViewEvent`] the caller raises on the plugin manager
//! ([`tessera_plugins::GameplayEvent::Mount`]). [`MountPlayer`] hands the model to the
//! plugins as [`LocalPlayerSource::mount`], wrapping the local player the other views use.

use std::sync::Arc;

use tessera_formats::Did;
use tessera_gamedata::mount::{MountClass as GClass, MountPresenter};
use tessera_gamedata::quest::RecordRef;
use tessera_net::EntityId;
use tessera_net::domain::{MountVital as NetVital, Ref};
use tessera_plugins::{
    LocalPlayerInfo, LocalPlayerSource, MountClass, MountModel, MountRef, MountSource,
    MountViewEvent, MountVital, WalletSource,
};

use super::state::P5Change;
use crate::npc::NpcData;
use crate::npc::present::record_ref;

/// The plugin vital of a feed vital.
#[must_use]
pub fn plugin_vital(v: NetVital) -> MountVital {
    match v {
        NetVital::Durability => MountVital::Durability,
        NetVital::Morale => MountVital::Morale,
        NetVital::Power => MountVital::Power,
        NetVital::Fury => MountVital::Fury,
    }
}

/// The identity key of a ref (records and literals kept distinct).
fn ref_key(r: &RecordRef) -> String {
    match r {
        RecordRef::Record(d) => format!("record:{:08x}", d.0),
        RecordRef::Literal(s) => format!("literal:{s}"),
    }
}

/// The resolved mount ref of the §15.5 model table: class from `combat` when scripted,
/// else the war-steed rule over the install (`data`), a literal (or a record without an
/// install) basic; name by the `GetName()` row.
#[must_use]
pub fn mount_ref(r: &Ref, combat: Option<bool>, data: Option<&NpcData>) -> MountRef {
    let rr = record_ref(r);
    let presenter = data.map(|d| MountPresenter::new(d.data()));
    let class = match combat {
        Some(true) => MountClass::Combat,
        Some(false) => MountClass::Basic,
        None => match presenter.as_ref().map(|p| p.plugin_class(&rr)) {
            Some(GClass::Combat) => MountClass::Combat,
            _ => MountClass::Basic,
        },
    };
    let name = match (&rr, &presenter) {
        (RecordRef::Literal(s), _) => s.clone(),
        (RecordRef::Record(_), Some(p)) => p.plugin_name(&rr),
        (RecordRef::Record(_), None) => String::new(),
    };
    MountRef {
        key: ref_key(&rr),
        class,
        name,
    }
}

/// The session's mount model (module docs).
#[derive(Debug)]
pub struct SessionMount {
    model: Arc<MountModel>,
    /// The walker's mounted effect last fed (no-feed sessions).
    walker_effect: Option<u32>,
}

impl Default for SessionMount {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionMount {
    /// A model with no mount and every vital 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            model: Arc::new(MountModel::new()),
            walker_effect: None,
        }
    }

    /// The model (the plugin mount source).
    #[must_use]
    pub fn model(&self) -> Arc<MountModel> {
        Arc::clone(&self.model)
    }

    /// The plugin source of the model.
    #[must_use]
    pub fn source(&self) -> Arc<dyn MountSource> {
        Arc::clone(&self.model) as Arc<dyn MountSource>
    }

    /// One frame's P5 changes from a feed: `MountChanged` and `MountVitalChanged` of
    /// `player` change the model; `Reset` (leaving the world) ends the mount. Returns the
    /// plugin events to raise, in order.
    pub fn apply(
        &mut self,
        changes: &[P5Change],
        player: Option<EntityId>,
        data: Option<&NpcData>,
    ) -> Vec<MountViewEvent> {
        let mut out = Vec::new();
        for c in changes {
            match c {
                P5Change::Mount {
                    entity,
                    mount,
                    combat,
                } => {
                    let local = Some(*entity) == player;
                    let r = mount.as_ref().map(|m| mount_ref(m, *combat, data));
                    out.extend(self.model.mount_changed(local, r));
                }
                P5Change::MountVital {
                    entity,
                    vital,
                    current,
                    max,
                } => {
                    let local = Some(*entity) == player;
                    out.extend(self.model.vital_changed(
                        local,
                        plugin_vital(*vital),
                        *current,
                        *max,
                    ));
                }
                P5Change::Reset => out.extend(self.leave_world()),
                _ => {}
            }
        }
        out
    }

    /// The walker's mount state without a feed (`walker.md` §16.5): `state` 3 with
    /// `effect` ≠ 0 is mounted on that effect, anything else on foot. Returns the plugin
    /// event of a model change.
    pub fn walker_state(
        &mut self,
        state: u8,
        effect: u32,
        data: Option<&NpcData>,
    ) -> Option<MountViewEvent> {
        let now = (state == 3 && effect != 0).then_some(effect);
        if now == self.walker_effect {
            return None;
        }
        self.walker_effect = now;
        let r = now.map(|e| mount_ref(&Ref::Record(tessera_net::DataId(e)), None, data));
        self.model.mount_changed(true, r)
    }

    /// Leaving the world (§15.5): the mount ends with one event; vitals return to 0.
    pub fn leave_world(&mut self) -> Option<MountViewEvent> {
        self.walker_effect = None;
        self.model.leave_world()
    }
}

/// The plugins' local player with the mount views: every other view is `inner`'s.
pub struct MountPlayer {
    inner: Arc<dyn LocalPlayerSource>,
    mount: Arc<dyn MountSource>,
}

impl std::fmt::Debug for MountPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountPlayer").finish_non_exhaustive()
    }
}

impl MountPlayer {
    /// `inner` with the mount source `mount`.
    #[must_use]
    pub fn new(inner: Arc<dyn LocalPlayerSource>, mount: Arc<dyn MountSource>) -> Arc<Self> {
        Arc::new(Self { inner, mount })
    }
}

impl LocalPlayerSource for MountPlayer {
    fn local_player(&self) -> Option<LocalPlayerInfo> {
        self.inner.local_player()
    }

    fn wallet(&self) -> Option<Arc<dyn WalletSource>> {
        self.inner.wallet()
    }

    fn mount(&self) -> Option<Arc<dyn MountSource>> {
        Some(Arc::clone(&self.mount))
    }
}

/// The mount effect DID a feed ref names for the walker (`walker.md` §16.1: the feed's
/// `mount` step runs §16.5 with the ref resolved as a SOURCE of §16.2; a literal names
/// nothing the walker can draw).
#[must_use]
pub fn walker_source(r: &Ref) -> Option<u32> {
    match r {
        Ref::Record(d) => Some(d.0).filter(|d| *d != 0),
        Ref::Literal(_) => None,
    }
}

/// The DID of a record ref (tests and tools).
#[must_use]
pub fn did_of(r: &Ref) -> Option<Did> {
    walker_source(r).map(Did)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_changes_drive_the_model() {
        let mut m = SessionMount::new();
        let me = Some(EntityId(1));
        let lit = Ref::Literal("Pony".into());
        let ev = m.apply(
            &[P5Change::Mount {
                entity: EntityId(1),
                mount: Some(lit.clone()),
                combat: None,
            }],
            me,
            None,
        );
        assert_eq!(ev.len(), 1);
        let cur = m.model().current().expect("mounted");
        assert_eq!(cur.class, MountClass::Basic);
        assert_eq!(cur.name, "Pony");
        // Another entity, or the same mount again: nothing.
        let ev = m.apply(
            &[
                P5Change::Mount {
                    entity: EntityId(2),
                    mount: None,
                    combat: None,
                },
                P5Change::Mount {
                    entity: EntityId(1),
                    mount: Some(lit),
                    combat: None,
                },
            ],
            me,
            None,
        );
        assert!(ev.is_empty());
        // The scripted class wins.
        let ev = m.apply(
            &[P5Change::Mount {
                entity: EntityId(1),
                mount: Some(Ref::Literal("Steed".into())),
                combat: Some(true),
            }],
            me,
            None,
        );
        assert_eq!(ev.len(), 1);
        assert_eq!(
            m.model().current().map(|p| p.class),
            Some(MountClass::Combat)
        );
        let ev = m.apply(
            &[P5Change::MountVital {
                entity: EntityId(1),
                vital: NetVital::Morale,
                current: 10.0,
                max: 20.0,
            }],
            me,
            None,
        );
        assert_eq!(ev.len(), 1);
        assert_eq!(m.apply(&[P5Change::Reset], me, None).len(), 1);
        assert!(m.model().current().is_none());
    }

    #[test]
    fn walker_state_without_a_feed() {
        let mut m = SessionMount::new();
        assert!(m.walker_state(1, 0x7001_0000, None).is_none());
        assert!(m.walker_state(3, 0x7001_0000, None).is_some());
        assert!(m.walker_state(3, 0x7001_0000, None).is_none());
        assert!(m.walker_state(4, 0x7001_0000, None).is_some());
        assert!(m.model().current().is_none());
    }

    #[test]
    fn walker_sources() {
        assert_eq!(
            walker_source(&Ref::Record(tessera_net::DataId(0x7000_0001))),
            Some(0x7000_0001)
        );
        assert_eq!(walker_source(&Ref::Record(tessera_net::DataId(0))), None);
        assert_eq!(walker_source(&Ref::Literal("x".into())), None);
    }
}
