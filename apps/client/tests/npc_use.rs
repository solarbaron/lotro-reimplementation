// SPDX-License-Identifier: GPL-3.0-or-later
//! `USE` and the walk-away close through the NPC hub's frame (`npc-windows.md` §3.1 Tessera
//! rule, §5.7; `interaction.md` §3.4.1 I6a), with the use-range cylinders read by a test
//! loader on the hub's worker: `USE` with no selection posts the no-selection line and
//! sends nothing; a use whose cylinder is still being read waits for it and runs once it
//! arrives, or after the hub's bound with Decision D13's cylinder; and an open window's
//! walk-away close waits for its entity's cylinder. Sandbox session, no install.

mod ui_common;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use tessera_client::net::World;
use tessera_client::npc::commands::{Cylinder, NO_SELECTION_LINE};
use tessera_client::npc::hub::USE_WAIT;
use tessera_client::npc::{NpcHub, NpcRefs, UseOutcome};
use tessera_net::domain::{CombatEvent, NpcEvent, Price, VendorEntry, WindowId};
use tessera_net::{
    Appearance, Body, DataId, EntityId, EntityKind, Event, Location, RegionId, Vec3,
};
use ui_common::Rig;

const KEEPER: EntityId = EntityId(900);

struct Client {
    rig: Rig,
    world: World,
    npc: NpcHub,
}

impl Client {
    /// The sandbox in the world with a keeper NPC (body record `0x7000_0001`) and the hub
    /// on the fallback names; the hub's cylinder worker answers through `loader`.
    fn new(loader: impl FnMut(Body) -> Cylinder + Send + 'static) -> Self {
        let mut world = World::sandbox(Instant::now()).expect("sandbox");
        for _ in 0..500 {
            if world.player().is_some() {
                break;
            }
            world.update(Instant::now());
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(world.player().is_some(), "the sandbox enters the world");
        let body = Body::Object(DataId(0x7000_0001));
        world.apply_polled(
            Instant::now(),
            &[Event::EntityCreated {
                id: KEEPER,
                kind: EntityKind::NonPlayer,
                name: "Keeper".into(),
                class: Some(DataId(0x7000_0001)),
                location: Location::new(RegionId(1), 0.0, 0.0, 0.0),
                heading: 0.0,
                appearance: Box::new(Appearance::new(body)),
                tag: None,
            }],
        );
        let mut npc = NpcHub::new(Vec::new());
        npc.set_loaded(Arc::new(NpcRefs::fallback()), None);
        npc.start_shapes_with(loader);
        Self {
            rig: Rig::new(|_| {}),
            world,
            npc,
        }
    }

    /// The keeper's position in the session's frame.
    fn keeper_at(&self) -> Vec3 {
        self.world.entities()[&KEEPER].transform.position
    }

    /// The player stands `d` metres east of the keeper.
    fn avatar(&self, d: f32) -> Option<Vec3> {
        let k = self.keeper_at();
        Some(Vec3::new(k.x + d, k.y, k.z))
    }

    fn select(&mut self, target: Option<EntityId>) {
        let player = self.world.player().expect("player");
        self.world.apply_polled(
            Instant::now(),
            &[Event::Combat(CombatEvent::TargetChanged {
                entity: player,
                target,
            })],
        );
    }

    /// One hub frame at `now` with the player at `avatar`.
    fn frame(&mut self, now: Instant, avatar: Option<Vec3>) {
        let events = self.rig.host.take_game_events();
        self.npc.frame(
            &mut self.rig.host,
            &mut self.world,
            &events,
            false,
            avatar,
            now,
        );
    }
}

/// A loader that answers each body once `gate` lets it through, with a large cylinder
/// (radius 6 m) for the keeper and D13's for every other body.
fn gated(gate: Receiver<()>) -> impl FnMut(Body) -> Cylinder + Send + 'static {
    move |b| {
        let _ = gate.recv();
        if b == Body::Object(DataId(0x7000_0001)) {
            Cylinder {
                radius: 6.0,
                height: 4.0,
                ..Cylinder::NO_SETUP
            }
        } else {
            Cylinder::NO_SETUP
        }
    }
}

#[test]
fn use_without_a_selection_posts_the_line_and_sends_nothing() {
    let mut c = Client::new(|_| Cylinder::NO_SETUP);
    c.select(None);
    c.npc.request_use();
    let at = c.avatar(1.0);
    c.frame(Instant::now(), at);
    assert_eq!(c.npc.last_use(), Some(&UseOutcome::NoSelection));
    assert_eq!(UseOutcome::NoSelection.line(), Some(NO_SELECTION_LINE));
    assert!(
        !c.rig.host.settled(),
        "the line is queued for the chat window"
    );
}

#[test]
fn a_use_waits_for_its_cylinder_and_then_runs_with_it() {
    let (open, gate) = channel();
    let mut c = Client::new(gated(gate));
    c.select(Some(KEEPER));
    assert_eq!(c.world.combat_state().selection(), Some(KEEPER));
    // 9 m from the keeper's centre: out of range as a D13 cylinder (gap 8.9 m above D14's
    // 6 m), in range with its own 6 m radius (9 - 0.05 - 6 = 2.95 m).
    let at = c.avatar(9.0);
    let t0 = Instant::now();
    c.npc.request_use();
    c.frame(t0, at);
    assert_eq!(
        c.npc.last_use(),
        None,
        "nothing runs while the cylinder is read"
    );
    // The loader answers; a later frame (before the bound) runs the use with it.
    for _ in 0..8 {
        let _ = open.send(());
    }
    let mut ran = false;
    for i in 1..2000 {
        c.frame(t0 + Duration::from_micros(i), at);
        if c.npc.last_use().is_some() {
            ran = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(ran, "the use ran once the cylinder arrived");
    assert_eq!(
        c.npc.last_use(),
        Some(&UseOutcome::Sent),
        "in range from the edge"
    );
}

#[test]
fn a_use_whose_cylinder_never_arrives_runs_after_the_bound() {
    let (_open, gate) = channel::<()>();
    let mut c = Client::new(gated(gate));
    c.select(Some(KEEPER));
    // 3 m: in range even as a D13 cylinder (3 - 0.1 = 2.9 m <= 6).
    let at = c.avatar(3.0);
    let t0 = Instant::now();
    c.npc.request_use();
    c.frame(t0, at);
    c.frame(t0 + USE_WAIT / 2, at);
    assert_eq!(c.npc.last_use(), None, "still waiting within the bound");
    c.frame(t0 + USE_WAIT, at);
    assert_eq!(
        c.npc.last_use(),
        Some(&UseOutcome::Sent),
        "run with D13's cylinder after the bound"
    );
}

#[test]
fn the_walk_away_close_waits_for_the_entitys_cylinder() {
    let (_open, gate) = channel::<()>();
    let mut c = Client::new(gated(gate));
    // A window the session opens on its own (no use read the cylinders first).
    c.world.apply_polled(
        Instant::now(),
        &[Event::Npc(NpcEvent::VendorShown {
            window: WindowId(1),
            vendor: Some(KEEPER),
            entries: vec![VendorEntry {
                entry: 1,
                what: tessera_net::domain::Ref::Literal("Bread".into()),
                price: Price::default(),
                stock: None,
            }],
        })],
    );
    assert_eq!(c.world.player_state().npc().windows().count(), 1);
    // 9 m: far as a D13 cylinder, in range with the keeper's own.
    let at = c.avatar(9.0);
    let t0 = Instant::now();
    c.frame(t0, at);
    c.frame(t0 + USE_WAIT / 2, at);
    assert!(
        c.npc.ui().is_some_and(|u| u.open_windows().count() == 1),
        "the window is open"
    );
    assert!(
        !c.npc.ui().is_some_and(|u| u.close_sent(1)),
        "no walk-away close while the cylinder is read"
    );
    // After the bound the close measures with what has arrived (D13 here): far.
    c.frame(t0 + USE_WAIT, at);
    assert!(
        c.npc.ui().is_some_and(|u| u.close_sent(1)),
        "closed after the bound"
    );
}
