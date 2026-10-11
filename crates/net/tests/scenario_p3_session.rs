// SPDX-License-Identifier: GPL-3.0-or-later
//! Session rules of the P3 revision (`docs/specs/scenario-feed.md` §9.5): `QuestRewards`
//! and `BuybackShown` follow the entity rule, invalid `QuestRewards` are dropped, buy-back
//! entries are cleaned like vendor entries, `BuyBack` needs `InWorld` and an entry from 1,
//! the sandbox without a scenario accepts it, and the test-server backend refuses it
//! (LEGAL.md §11.1 rule 5).

use std::time::Instant;

use tessera_net::domain::{
    BuybackEntry, DialogueOption, MAX_LITERAL_REF_LEN, MAX_PRICE_ITEMS, MAX_REWARD_ENTRIES, Price,
    Ref, RewardEntry, RewardKind, Text, WindowId,
};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{
    Backend, Command, EntityId, Event, NetError, NpcCommand, NpcEvent, Session, Transform,
};

/// A backend that replays a script, to check what `Session` lets through.
#[derive(Debug)]
struct Scripted(Vec<Vec<Event>>);

impl Backend for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }
    fn submit(&mut self, _now: Instant, _command: Command) -> Result<(), NetError> {
        Ok(())
    }
    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        if !self.0.is_empty() {
            out.extend(self.0.remove(0));
        }
    }
}

const W: WindowId = WindowId(7);

fn reward(kind: RewardKind, amount: i64, what: Option<&str>) -> RewardEntry {
    RewardEntry {
        kind,
        amount,
        what: what.map(|s| Ref::Literal(s.to_owned())),
    }
}

fn rewards(entries: Vec<RewardEntry>) -> Event {
    Event::Npc(NpcEvent::QuestRewards { window: W, entries })
}

#[test]
fn reward_entry_rules() {
    use RewardKind as K;
    for (kind, lo, hi) in [
        (K::Coin, 0, 1_000_000_000_000_000),
        (K::Glory, 0, 1_000_000_000_000_000),
        (K::ItemXp, 0, 1_000_000_000_000_000),
        (K::CraftXp, 0, 1_000_000_000_000_000),
        (K::MountXp, 0, 1_000_000_000_000_000),
        (K::VirtueXp, 0, 1_000_000_000_000_000),
        (K::TraitPoints, 0, 1_000_000),
        (K::Reputation, -1_000_000_000_000, 1_000_000_000_000),
        (K::Wallet, 0, 1_000_000_000_000_000),
    ] {
        assert_eq!(kind.amount_range(), lo..=hi, "{kind:?}");
        let what = kind.has_what().then_some("x");
        assert!(reward(kind, lo, what).is_valid());
        assert!(reward(kind, hi, what).is_valid());
        assert!(!reward(kind, lo - 1, what).is_valid());
        assert!(!reward(kind, hi + 1, what).is_valid());
        let wrong = if kind.has_what() { None } else { Some("x") };
        assert!(!reward(kind, lo, wrong).is_valid());
    }
    assert_eq!(
        [K::Reputation, K::Wallet].map(K::has_what),
        [true, true],
        "what is present exactly for reputation and wallet"
    );
}

#[test]
fn session_drops_invalid_rewards_and_cleans_buyback_entries() {
    let player = EntityId(1);
    let good = rewards(vec![
        reward(RewardKind::Coin, 1250, None),
        reward(RewardKind::Reputation, -300, Some("Smiths")),
    ]);
    let long = "x".repeat(100);
    let buyback = Event::Npc(NpcEvent::BuybackShown {
        window: W,
        entries: vec![BuybackEntry {
            entry: 1,
            what: Ref::Literal(long.clone()),
            quantity: 1,
            price: Price {
                coin: 40,
                items: vec![(Ref::Literal("t\u{7}".into()), 1); MAX_PRICE_ITEMS + 2],
            },
        }],
    });
    let dialogue = Event::Npc(NpcEvent::DialogueShown {
        window: W,
        speaker: None,
        text: Text::Action(2),
        options: vec![DialogueOption {
            option: 1,
            text: Text::Action(u32::MAX),
        }],
    });
    let script = vec![
        vec![Event::Connected { characters: vec![] }],
        vec![
            // Before world entry the entity rule drops them.
            good.clone(),
            buyback.clone(),
            Event::WorldEntered {
                player,
                name: "P".into(),
                transform: Transform::default(),
            },
            good.clone(),
            rewards(Vec::new()),
            rewards(vec![
                reward(RewardKind::Coin, 1, None);
                MAX_REWARD_ENTRIES + 1
            ]),
            rewards(vec![reward(RewardKind::Coin, 1, None); MAX_REWARD_ENTRIES]),
            rewards(vec![reward(RewardKind::Coin, 1, Some("x"))]),
            rewards(vec![reward(RewardKind::Wallet, 1, None)]),
            rewards(vec![reward(RewardKind::TraitPoints, 1_000_001, None)]),
            rewards(vec![reward(
                RewardKind::Reputation,
                -1_000_000_000_001,
                Some("f"),
            )]),
            buyback,
            dialogue.clone(),
        ],
    ];
    let mut s = Session::new(Scripted(script));
    let t = Instant::now();
    s.send(t, Command::Connect).unwrap();
    assert_eq!(s.poll(t).len(), 1);
    s.send(
        t,
        Command::EnterWorld {
            character: "P".into(),
        },
    )
    .unwrap();
    let ev = s.poll(t);
    let npc: Vec<&NpcEvent> = ev
        .iter()
        .filter_map(|e| match e {
            Event::Npc(n) => Some(n),
            _ => None,
        })
        .collect();
    assert_eq!(npc.len(), 4, "{npc:#?}");
    assert_eq!(
        Some(npc[0]),
        match &good {
            Event::Npc(n) => Some(n),
            _ => None,
        }
    );
    assert!(
        matches!(npc[1], NpcEvent::QuestRewards { entries, .. } if entries.len() == MAX_REWARD_ENTRIES)
    );
    let NpcEvent::BuybackShown { entries, .. } = npc[2] else {
        panic!("{:?}", npc[2]);
    };
    let Ref::Literal(what) = &entries[0].what else {
        panic!()
    };
    assert_eq!(what.len(), MAX_LITERAL_REF_LEN);
    assert_eq!(entries[0].price.items.len(), MAX_PRICE_ITEMS);
    assert!(
        entries[0]
            .price
            .items
            .iter()
            .all(|(r, _)| matches!(r, Ref::Literal(s) if !s.chars().any(char::is_control)))
    );
    // Action texts need no cleaning and pass unchanged.
    assert_eq!(
        Some(npc[3]),
        match &dialogue {
            Event::Npc(n) => Some(n),
            _ => None,
        }
    );
    assert_eq!(NpcEvent::name(npc[0]), "QuestRewards");
    assert_eq!(NpcEvent::name(npc[2]), "BuybackShown");
}

fn buy_back(entry: u16) -> Command {
    Command::Npc(NpcCommand::BuyBack { window: W, entry })
}

#[test]
fn buy_back_needs_the_world_and_an_entry_from_one() {
    let mut s = Session::new(SandboxBackend::new(SandboxConfig::default()).expect("sandbox"));
    let t = Instant::now();
    assert!(matches!(
        s.send(t, buy_back(1)),
        Err(NetError::InvalidState { .. })
    ));
    s.send(t, Command::Connect).unwrap();
    let character = s
        .poll(t)
        .iter()
        .find_map(|e| match e {
            Event::Connected { characters } => characters.first().map(|c| c.name.clone()),
            _ => None,
        })
        .expect("a character");
    assert!(matches!(
        s.send(t, buy_back(1)),
        Err(NetError::InvalidState { .. })
    ));
    s.send(t, Command::EnterWorld { character }).unwrap();
    let _ = s.poll(t);
    assert!(matches!(
        s.send(t, buy_back(0)),
        Err(NetError::InvalidCommand(_))
    ));
    // The sandbox without a scenario accepts and ignores it (§9.5).
    s.send(t, buy_back(1)).unwrap();
    s.send(t, buy_back(u16::MAX)).unwrap();
    assert_eq!(buy_back(1).name(), "BuyBack");
}

/// The test-server backend refuses `BuyBack` like every other gameplay command.
#[cfg(feature = "remote")]
mod remote_support;

#[cfg(feature = "remote")]
#[test]
fn the_test_server_backend_refuses_buy_back() {
    use remote_support::{PeerOptions, connect, pump, spawn};
    let peer = spawn(PeerOptions::default(), |mut p| async move {
        p.login(&["Aldith"]).await;
        p.enter_world().await;
        let _ = p.closed().await;
    });
    let mut s = connect(peer.config());
    pump(&mut s, |e| matches!(e, Event::Connected { .. }));
    s.send(
        Instant::now(),
        Command::EnterWorld {
            character: "Aldith".into(),
        },
    )
    .unwrap();
    pump(&mut s, |e| matches!(e, Event::WorldEntered { .. }));
    assert!(matches!(
        s.send(Instant::now(), buy_back(1)),
        Err(NetError::InvalidCommand(_))
    ));
    s.send(Instant::now(), Command::Disconnect).unwrap();
    pump(&mut s, |e| matches!(e, Event::Disconnected { .. }));
    peer.join();
}
