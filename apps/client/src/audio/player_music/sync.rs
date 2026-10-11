// SPDX-License-Identifier: GPL-3.0-or-later
//! The synchronised `/play` of P4 in the player music (`instruments.md` §8.8–§8.9, §12 P4
//! Decision; `scenario-feed.md` §6.7; decision 0035 D4): the session's performance events
//! as queue items, the fellowship members' characters, the requests to the session and
//! the ready notices.
//!
//! The flow:
//!
//! - the player's `/play <song> … sync`, once read and accepted, waits as the local part,
//!   shows the own ready notice and asks the session for `ReadyPerformance { tune }`
//!   ([`MusicRequest::Ready`]);
//! - `/playstart` asks the session for `StartPerformance` ([`MusicRequest::Start`]) when the
//!   session answers it (the sandbox and the scenario feed), and starts nothing locally; a
//!   session that refuses it (the test server) gets the local start of Decision 3;
//! - `PerformanceReady` makes a member's part wait (read from the user's own music folders);
//!   one for the player acts as an accepted `/play … sync`;
//! - `PerformanceSyncStart` starts every waiting part with one start time *S*
//!   ([`StartClock`]); `PerformanceWithdrawn`, a fellowship without the member and
//!   `PerformanceStopped` withdraw or cancel a member's part (Decision 6).

use tessera_audio::abc::{ArgValue, MESSAGE_TABLE_KEY, MessageArg, MessageRef};
use tessera_audio::instruments::CharacterId;
use tessera_net::domain::SocialEvent;
use tessera_net::{EntityId, Event};

use super::{Messages, MusicCommand};

/// Most session items waiting in the player music's queue (bounded against a flood of
/// events; the session's own bounds are lower).
pub const MAX_SESSION_ITEMS: usize = 256;
/// Most fellowship members with a character in the player (the feed allows 64 waiting
/// members, `scenario-feed.md` §6.7).
pub const MAX_MEMBERS: usize = 64;
/// The first character number of a member (the local player is 1).
pub const FIRST_MEMBER: u64 = 2;

/// Argument ID of the title in the ready notices (`instruments.md` §8.8.6).
pub const ARG_READY_TITLE: u32 = 0x0005_8427;
/// Argument ID of the member's name in the ready notices.
pub const ARG_MEMBER: u32 = 0x0550_6DA2;
/// Own part accepted, with a title.
pub const TOKEN_OWN_READY: u32 = 0x0871_DF57;
/// Own part accepted, without a title.
pub const TOKEN_OWN_READY_UNTITLED: u32 = 0x02D8_DC54;
/// A member's part is ready, with a title.
pub const TOKEN_MEMBER_READY: u32 = 0x03A9_1887;
/// A member's part is ready, without a title.
pub const TOKEN_MEMBER_READY_UNTITLED: u32 = 0x0B63_0494;

/// What the player music asks of the world session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicRequest {
    /// `ReadyPerformance { tune }`: the player's `/play … sync` was accepted.
    Ready {
        /// The song name of the command.
        tune: String,
    },
    /// `StartPerformance`: `/playstart`.
    Start,
}

impl MusicRequest {
    /// The world session's command of the request (`scenario-feed.md` §6.7).
    #[must_use]
    pub fn command(&self) -> tessera_net::Command {
        use tessera_net::domain::SocialCommand;
        tessera_net::Command::Social(match self {
            Self::Ready { tune } => SocialCommand::ReadyPerformance { tune: tune.clone() },
            Self::Start => SocialCommand::StartPerformance,
        })
    }
}

/// How the start time *S* of a session's synchronised start is placed on the audio clock
/// (`scenario-feed.md` §6.7).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum StartClock {
    /// A windowed client: *S* = the audio-clock time at which the start is processed plus
    /// its `lead_ms`.
    #[default]
    Audio,
    /// A headless run on the scenario's virtual clock: *S* = `origin` + `at_ms` (the audio
    /// clock runs in step with the virtual clock from `origin`).
    Virtual {
        /// The audio-clock time of virtual time 0, seconds.
        origin: f64,
    },
}

impl StartClock {
    /// *S* for a start processed at audio-clock time `now`.
    #[must_use]
    pub fn start_secs(self, now: f64, lead_ms: u32, at_ms: u64) -> f64 {
        match self {
            Self::Audio => now + f64::from(lead_ms) / 1e3,
            // `at_ms` below 2^53 is exact in f64.
            Self::Virtual { origin } => origin + at_ms as f64 / 1e3,
        }
    }
}

/// A queue item of the player music: a command, or a session item (both in arrival order).
#[derive(Debug, Clone)]
pub(super) enum Queued {
    /// A player-music command.
    Command(MusicCommand),
    /// A session item.
    Social(SocialItem),
}

/// A performance or fellowship event, as the player music processes it.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum SocialItem {
    /// `PerformanceReady`: the member's (none: the player's) part waits.
    Ready {
        /// The member's name, or `None` for the player.
        member: Option<String>,
        /// The member's entity, when the event names one (a later `PerformanceStopped`
        /// finds the part by it).
        entity: Option<EntityId>,
        /// The tune.
        tune: String,
    },
    /// `PerformanceStarted` (`instruments.md` §12 P4 Decision 5: outside a synchronised
    /// start it keeps its meaning): the performer's tune starts solo-like at the
    /// processing time plus `delay_ms`.
    Started {
        /// The performer, or `None` for the player.
        performer: Option<EntityId>,
        /// The tune.
        tune: String,
        /// The delay.
        delay_ms: u32,
    },
    /// `PerformanceSyncStart`.
    Start {
        /// The lead.
        lead_ms: u32,
        /// The start time on the backend's base.
        at_ms: u64,
    },
    /// The session refused `StartPerformance`: the local start.
    LocalStart,
    /// `PerformanceWithdrawn`.
    Withdraw {
        /// The member's name, or `None` for the player.
        member: Option<String>,
    },
    /// `FellowshipChanged`: the members other than the player, with their entities. World
    /// entry and disconnection count as an empty list (the session's fellowship is gone).
    Fellowship {
        /// (name, entity).
        members: Vec<(String, Option<EntityId>)>,
    },
    /// `PerformanceStopped` of an entity.
    Stopped {
        /// The performer.
        entity: EntityId,
    },
}

fn name_equal(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl SocialItem {
    /// The item of a session event, if it is one (`player`: the local character's name and
    /// entity).
    #[must_use]
    pub(super) fn from_event(
        ev: &Event,
        player: &str,
        player_entity: Option<EntityId>,
    ) -> Option<Self> {
        let is_player = |name: &str, entity: Option<EntityId>| {
            (entity.is_some() && entity == player_entity)
                || (!player.is_empty() && name_equal(name, player))
        };
        let s = match ev {
            Event::Social(s) => s,
            Event::WorldEntered { .. } | Event::Disconnected { .. } => {
                return Some(Self::Fellowship {
                    members: Vec::new(),
                });
            }
            _ => return None,
        };
        Some(match s {
            SocialEvent::PerformanceReady {
                member,
                entity,
                tune,
            } => {
                let other = !is_player(member, *entity);
                Self::Ready {
                    member: other.then(|| member.clone()),
                    entity: if other { *entity } else { None },
                    tune: tune.clone(),
                }
            }
            SocialEvent::PerformanceStarted {
                entity,
                tune,
                delay_ms,
            } => Self::Started {
                performer: (!is_player("", Some(*entity))).then_some(*entity),
                tune: tune.clone(),
                delay_ms: *delay_ms,
            },
            SocialEvent::PerformanceSyncStart { at_ms, lead_ms, .. } => Self::Start {
                lead_ms: *lead_ms,
                at_ms: *at_ms,
            },
            SocialEvent::PerformanceWithdrawn { member } => Self::Withdraw {
                member: (!is_player(member, None)).then(|| member.clone()),
            },
            SocialEvent::FellowshipChanged { members } => Self::Fellowship {
                members: members
                    .iter()
                    .filter(|m| !is_player(&m.name, m.entity))
                    .take(MAX_MEMBERS)
                    .map(|m| (m.name.clone(), m.entity))
                    .collect(),
            },
            SocialEvent::PerformanceStopped { entity } => Self::Stopped { entity: *entity },
            _ => return None,
        })
    }
}

/// The fellowship members' characters in the player: one per member name (name-equal),
/// numbered from [`FIRST_MEMBER`], at most [`MAX_MEMBERS`].
#[derive(Debug, Clone, Default)]
pub(super) struct Members {
    /// (lower-cased name, name as first seen, character, entity).
    list: Vec<(String, String, CharacterId, Option<EntityId>)>,
    next: u64,
}

impl Members {
    /// The character of `name`, allocated on first use; `None` beyond [`MAX_MEMBERS`].
    pub(super) fn id_of(&mut self, name: &str) -> Option<CharacterId> {
        if let Some(id) = self.find(name) {
            return Some(id);
        }
        if self.list.len() >= MAX_MEMBERS {
            return None;
        }
        let n = FIRST_MEMBER.saturating_add(self.next);
        self.next = self.next.saturating_add(1);
        let id = CharacterId(n);
        self.list
            .push((name.to_lowercase(), name.to_owned(), id, None));
        Some(id)
    }

    /// The character of the performer `entity` (a `PerformanceStarted` of an entity the
    /// fellowship list may not name): the member with that entity, else one allocated under
    /// the name `#<entity>` (no character name holds `#`); `None` beyond [`MAX_MEMBERS`].
    pub(super) fn id_of_entity(&mut self, entity: EntityId) -> Option<CharacterId> {
        if let Some(id) = self.by_entity(entity) {
            return Some(id);
        }
        let name = format!("#{}", entity.0);
        let id = self.id_of(&name)?;
        self.set_entity(&name, Some(entity));
        Some(id)
    }

    /// Frees the members `keep` refuses (their characters are numbered anew if they come
    /// back); returns the freed characters.
    pub(super) fn retain(
        &mut self,
        mut keep: impl FnMut(&str, CharacterId) -> bool,
    ) -> Vec<CharacterId> {
        let mut freed = Vec::new();
        self.list.retain(|m| {
            let k = keep(&m.1, m.2);
            if !k {
                freed.push(m.2);
            }
            k
        });
        freed
    }

    /// The members' characters allocated now.
    pub(super) fn len(&self) -> usize {
        self.list.len()
    }

    /// The character of `name`, if allocated.
    pub(super) fn find(&self, name: &str) -> Option<CharacterId> {
        let k = name.to_lowercase();
        self.list.iter().find(|m| m.0 == k).map(|m| m.2)
    }

    /// Records the member's entity (from the fellowship list).
    pub(super) fn set_entity(&mut self, name: &str, entity: Option<EntityId>) {
        let k = name.to_lowercase();
        if let Some(m) = self.list.iter_mut().find(|m| m.0 == k) {
            m.3 = entity;
        }
    }

    /// The member whose entity is `entity`.
    pub(super) fn by_entity(&self, entity: EntityId) -> Option<CharacterId> {
        self.list.iter().find(|m| m.3 == Some(entity)).map(|m| m.2)
    }

    /// The name of a member's character.
    pub(super) fn name_of(&self, id: CharacterId) -> Option<String> {
        self.list.iter().find(|m| m.2 == id).map(|m| m.1.clone())
    }
}

/// The own ready notice (`instruments.md` §8.8.6): with the title, or the untitled one.
#[must_use]
pub fn own_ready_notice(title: &str) -> MessageRef {
    let token = if title.is_empty() {
        TOKEN_OWN_READY_UNTITLED
    } else {
        TOKEN_OWN_READY
    };
    MessageRef {
        table_key: MESSAGE_TABLE_KEY,
        token,
        args: vec![MessageArg {
            id: ARG_READY_TITLE,
            value: ArgValue::Text(title.to_owned()),
        }],
    }
}

/// A member's ready notice (`instruments.md` §8.8.6, §12 P4 Decision 5).
#[must_use]
pub fn member_ready_notice(member: &str, title: &str) -> MessageRef {
    let token = if title.is_empty() {
        TOKEN_MEMBER_READY_UNTITLED
    } else {
        TOKEN_MEMBER_READY
    };
    MessageRef {
        table_key: MESSAGE_TABLE_KEY,
        token,
        args: vec![
            MessageArg {
                id: ARG_MEMBER,
                value: ArgValue::Text(member.to_owned()),
            },
            MessageArg {
                id: ARG_READY_TITLE,
                value: ArgValue::Text(title.to_owned()),
            },
        ],
    }
}

/// A ready notice from the user's table, else Tessera's own words.
#[must_use]
pub fn render(messages: Option<&Messages>, m: &MessageRef) -> String {
    if let Some(t) = messages.and_then(|ms| ms.render(m)) {
        return t;
    }
    let arg = |id: u32| {
        m.args
            .iter()
            .find(|a| a.id == id)
            .map(|a| match &a.value {
                ArgValue::Text(t) => t.clone(),
                ArgValue::Integer(n) => n.to_string(),
            })
            .unwrap_or_default()
    };
    match m.token {
        TOKEN_OWN_READY => format!(
            "Music: ready to play {} (waiting for the start)",
            arg(ARG_READY_TITLE)
        ),
        TOKEN_OWN_READY_UNTITLED => "Music: ready to play (waiting for the start)".to_owned(),
        TOKEN_MEMBER_READY => format!(
            "Music: {} is ready to play {}",
            arg(ARG_MEMBER),
            arg(ARG_READY_TITLE)
        ),
        _ => format!("Music: {} is ready to play", arg(ARG_MEMBER)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::FellowMember;

    #[test]
    fn start_clock() {
        assert!((StartClock::Audio.start_secs(1.5, 700, 99) - 2.2).abs() < 1e-12);
        let v = StartClock::Virtual { origin: 0.25 };
        assert!((v.start_secs(9.0, 700, 2500) - 2.75).abs() < 1e-12);
    }

    #[test]
    fn members_are_numbered_once_and_bounded() {
        let mut m = Members::default();
        let a = m.id_of("Brin").expect("id");
        assert_eq!(a, CharacterId(FIRST_MEMBER));
        assert_eq!(m.id_of("BRIN"), Some(a));
        assert_eq!(m.name_of(a).as_deref(), Some("Brin"));
        m.set_entity("brin", Some(EntityId(7)));
        assert_eq!(m.by_entity(EntityId(7)), Some(a));
        for i in 0..MAX_MEMBERS * 2 {
            let _ = m.id_of(&format!("m{i}"));
        }
        assert!(m.id_of("one more").is_none());
        // Freed members make room again.
        let freed = m.retain(|name, _| name == "Brin");
        assert_eq!(freed.len(), MAX_MEMBERS - 1);
        assert_eq!(m.len(), 1);
        assert!(m.id_of("one more").is_some());
        let e = m.id_of_entity(EntityId(40)).expect("id");
        assert_eq!(m.id_of_entity(EntityId(40)), Some(e));
        assert_eq!(m.by_entity(EntityId(40)), Some(e));
    }

    #[test]
    fn events_become_items() {
        let ready = |member: &str, entity| {
            Event::Social(SocialEvent::PerformanceReady {
                member: member.into(),
                entity,
                tune: "Reel".into(),
            })
        };
        let p = Some(EntityId(1));
        assert_eq!(
            SocialItem::from_event(&ready("Tam", p), "Tam", p),
            Some(SocialItem::Ready {
                member: None,
                entity: None,
                tune: "Reel".into()
            })
        );
        assert_eq!(
            SocialItem::from_event(&ready("Cael", None), "Tam", p),
            Some(SocialItem::Ready {
                member: Some("Cael".into()),
                entity: None,
                tune: "Reel".into()
            })
        );
        let f = Event::Social(SocialEvent::FellowshipChanged {
            members: vec![
                FellowMember {
                    name: "Tam".into(),
                    entity: p,
                    leader: true,
                },
                FellowMember {
                    name: "Brin".into(),
                    entity: Some(EntityId(2)),
                    leader: false,
                },
            ],
        });
        assert_eq!(
            SocialItem::from_event(&f, "Tam", p),
            Some(SocialItem::Fellowship {
                members: vec![("Brin".into(), Some(EntityId(2)))]
            })
        );
        assert_eq!(
            SocialItem::from_event(&ready("Cael", Some(EntityId(9))), "Tam", p),
            Some(SocialItem::Ready {
                member: Some("Cael".into()),
                entity: Some(EntityId(9)),
                tune: "Reel".into()
            })
        );
        let started = |entity| {
            Event::Social(SocialEvent::PerformanceStarted {
                entity,
                tune: "Reel".into(),
                delay_ms: 250,
            })
        };
        assert_eq!(
            SocialItem::from_event(&started(EntityId(9)), "Tam", p),
            Some(SocialItem::Started {
                performer: Some(EntityId(9)),
                tune: "Reel".into(),
                delay_ms: 250
            })
        );
        assert_eq!(
            SocialItem::from_event(&started(EntityId(1)), "Tam", p),
            Some(SocialItem::Started {
                performer: None,
                tune: "Reel".into(),
                delay_ms: 250
            })
        );
        // The session's fellowship ends with a disconnection.
        assert_eq!(
            SocialItem::from_event(
                &Event::Disconnected {
                    reason: tessera_net::DisconnectReason::ClientRequested
                },
                "Tam",
                p
            ),
            Some(SocialItem::Fellowship {
                members: Vec::new()
            })
        );
        assert!(SocialItem::from_event(&Event::StaticsCleared, "Tam", p).is_none());
    }

    #[test]
    fn notices_fall_back_to_tesseras_words() {
        let t = render(None, &own_ready_notice("Reel"));
        assert!(t.contains("Reel"), "{t}");
        let t = render(None, &member_ready_notice("Brin", ""));
        assert!(t.contains("Brin"), "{t}");
        assert_eq!(own_ready_notice("").token, TOKEN_OWN_READY_UNTITLED);
    }
}
