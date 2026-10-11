// SPDX-License-Identifier: GPL-3.0-or-later
//! Revision 4 of `docs/specs/scenario-feed.md` (decision 0025): the overlay cases of §14.9
//! against a test double of the test-server backend, the plain-sandbox creation cases of
//! §14.10, and resolution of property values and looks with an install (§5.8, §8.6 item 7).
#![cfg(feature = "scenario")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tessera_net::domain::{CharacterCommand, CharacterEvent, InventoryCommand, ItemId, Text};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{
    Candidate, CandidateOutcome, CandidateStatus, EmptyResolver, Family, LabelContext,
    OverlayBackend, OverlayServer, PropertyInfo, ResolvedScenario, Scenario, ScenarioErrorKind,
    SelectorQuery, SelectorResolver,
};
use tessera_net::{
    Appearance, Backend, Body, ChatChannel, Command, DataId, Destination, EntityId, Equipped,
    Event, Location, LocationCause, Look, LookEntry, NetError, PropertyKind, PropertyValue,
    RegionId, Session, Transform,
};

// --- a synthetic resolver ----------------------------------------------------------------

/// `did` selectors return their listed DIDs; a small property dictionary and enum mapper.
#[derive(Debug, Default)]
struct Synthetic;

impl SelectorResolver for Synthetic {
    fn has_install(&self) -> bool {
        true
    }

    fn candidates(
        &mut self,
        query: &SelectorQuery<'_>,
        _limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        if query.family != Family::Did {
            return CandidateOutcome::new(CandidateStatus::Unsupported);
        }
        out.extend(query.dids.iter().map(|&did| Candidate {
            did,
            row: 0,
            location: None,
            anchor: None,
        }));
        CandidateOutcome::new(CandidateStatus::Ready)
    }

    fn enum_value(&mut self, context: LabelContext, label: &str) -> Option<u32> {
        match (context, label) {
            (LabelContext::PropertyKey(0x7000_0003), "@Red") => Some(1),
            (LabelContext::PropertyKey(0x7000_0003), "@Blue") => Some(3),
            (LabelContext::PropertyKey(0x7000_0003), "@Huge") => Some(40),
            (LabelContext::PropertyKey(0x7000_0004), "@Tall") => Some(7),
            (LabelContext::Nationality, "@Example_Nationality") => Some(9),
            _ => None,
        }
    }

    fn display_name(&mut self, _did: DataId, _row: Option<u32>) -> Option<String> {
        None
    }

    fn property(&mut self, label: &str) -> Option<PropertyInfo> {
        let (id, kind) = match label {
            "@Example_Stat" => (0x7000_0001, PropertyKind::Int),
            "@Example_Text" => (0x7000_0002, PropertyKind::String),
            "@Example_Flags" => (0x7000_0003, PropertyKind::Bitfield32),
            "@Example_Shape" => (0x7000_0004, PropertyKind::Enum),
            "@Example_Odd" => (0x7000_0005, PropertyKind::Unsupported),
            "@Example_Height" => (0x7000_0006, PropertyKind::Float),
            _ => return None,
        };
        Some(PropertyInfo { id, kind })
    }

    fn property_by_id(&mut self, id: u32) -> Option<PropertyInfo> {
        match id {
            0x7000_0001 => Some(PropertyInfo {
                id,
                kind: PropertyKind::Int,
            }),
            0x7000_0006 => Some(PropertyInfo {
                id,
                kind: PropertyKind::Float,
            }),
            _ => None,
        }
    }
}

fn resolve_with(text: &str, r: &mut dyn SelectorResolver) -> ResolvedScenario {
    Scenario::parse(text.as_bytes())
        .expect("parse")
        .resolve(r)
        .expect("resolve")
}

#[test]
fn property_values_resolve_with_an_install() {
    let text = r#"tessera-scenario 1
look $l @Example_Height=1.5 @Example_Shape=@Tall @Example_Height=2 @Nope=1 @Example_Text="no space"
player "Testa" look=$l
item $i "Ring"
at 0s give $i
+0s stat player @Example_Stat=3
+0s stat player @Example_Stat="x"
+0s stat player @Example_Odd=1
+0s stat player pid:0x70000002:string="unknown id"
+0s stat player pid:0x70000006:int=1
+0s stat player pid:0x70000006:float=2.5
+0s item-property $i @Example_Flags=@Red|@Blue
+0s item-property $i @Example_Flags=@Huge
+0s item-property $i @Example_Flags=@Green
+0s item-property $i @Example_Text=none
"#;
    let resolved = resolve_with(text, &mut Synthetic);
    let notes: Vec<String> = resolved.notes().iter().map(ToString::to_string).collect();
    assert_eq!(
        notes,
        vec![
            "label line=2 attribute=property label=@Nope",
            "property line=2 property=@Example_Text note=mismatch",
            "property line=2 property=@Example_Height note=duplicate",
            "property line=7 property=@Example_Stat note=mismatch",
            "property line=8 property=@Example_Odd note=unsupported",
            "property line=9 property=pid:0x70000002 note=unknown",
            "property line=10 property=pid:0x70000006 note=mismatch",
            "property line=13 property=@Example_Flags note=mismatch",
            "label line=14 attribute=value label=@Green",
        ]
    );
    let log = run_sandbox(resolved);
    assert!(
        log.contains(
            "pre CharacterInfo name=\"Testa\" body=none class=none level=none look=[{property=0x70000004 value=enum:7},{property=0x70000006 value=float:2}]"
        ),
        "{log}"
    );
    assert!(log.contains("0.000 PropertyChanged entity=player property=0x70000001 value=int:3"));
    assert!(
        log.contains("0.000 PropertyChanged entity=player property=0x70000006 value=float:2.5")
    );
    assert!(
        log.contains("0.000 ItemProperty item=$i property=0x70000003 value=bitfield32:0x00000005")
    );
    assert!(log.contains("0.000 ItemProperty item=$i property=0x70000002 value=none"));
    // Five steps dropped: "x", Odd, unknown id, the int kind, @Huge (key 40), @Green.
    assert!(log.contains("skipped=6"), "{log}");
}

#[test]
fn looks_are_cut_to_768_bytes() {
    // Seven strings of 120 bytes: 1 + 7 × (4 + 1 + 2 + 120) = 890 bytes; one is cut.
    let s = "x".repeat(120);
    let entries: Vec<String> = (1..=7).map(|i| format!("pid:{i}:string=\"{s}\"")).collect();
    let text = format!(
        "tessera-scenario 1\nlook $l {}\nplayer \"A\" look=$l\n",
        entries.join(" ")
    );
    let resolved = resolve_with(&text, &mut EmptyResolver);
    let notes: Vec<String> = resolved.notes().iter().map(ToString::to_string).collect();
    assert_eq!(notes, vec!["look line=2 cut=1"]);
    let log = run_sandbox(resolved);
    assert_eq!(log.matches("value=string:").count(), 6, "{log}");
}

fn run_sandbox(resolved: ResolvedScenario) -> String {
    let resolved = Arc::new(resolved);
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let session = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let mut driver =
        tessera_net::scenario::HeadlessDriver::new(session, resolved, Some(Vec::new()));
    let _ = driver.run();
    String::from_utf8(driver.into_log().expect("log")).expect("utf-8")
}

#[test]
fn accept_character_look_asked_and_entry_appearance() {
    let text = r#"tessera-scenario 1
look $l pid:0x7FFF0001:float=0.25
on create-character
+0s accept-character "Cora" species=7 sex=1 look=asked
end
on create-character name="Dina"
+0s accept-character "Dina" species=7 sex=1 look=$l
end
"#;
    let resolved = Arc::new(resolve_with(text, &mut EmptyResolver));
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let mut s = Session::new(SandboxBackend::new(config).unwrap());
    let t = Instant::now();
    s.send(t, Command::Connect).unwrap();
    let mut ev = Vec::new();
    s.poll_into(t, &mut ev);
    let look = Look {
        entries: vec![LookEntry {
            property: 9,
            value: PropertyValue::Colour([1, 2, 3, 4]),
        }],
    };
    let app = Appearance::new(Body::Character { species: 7, sex: 1 }).with_look(Some(look.clone()));
    s.send(
        t,
        Command::Character(CharacterCommand::CreateCharacter {
            name: "Cora".into(),
            appearance: Some(Box::new(app)),
            class: None,
            nationality: None,
        }),
    )
    .unwrap();
    let mut ev = Vec::new();
    s.poll_into(t, &mut ev);
    let info = ev.iter().find_map(|e| match e {
        Event::Character(CharacterEvent::CharacterInfo { name, look, .. }) if name == "Cora" => {
            Some(look.clone())
        }
        _ => None,
    });
    assert_eq!(info, Some(Some(Box::new(look.clone()))), "{ev:?}");
    s.send(
        t,
        Command::EnterWorld {
            character: "Cora".into(),
        },
    )
    .unwrap();
    let mut ev = Vec::new();
    for _ in 0..3 {
        s.poll_into(t, &mut ev);
    }
    let a = ev
        .iter()
        .find_map(|e| match e {
            Event::AppearanceChanged { appearance, .. } => Some(appearance.clone()),
            _ => None,
        })
        .expect("base appearance of the accepted character");
    assert_eq!(a.body, Body::Character { species: 7, sex: 1 });
    assert_eq!(a.look.as_deref(), Some(&look));
}

// --- §14.10: the plain sandbox ----------------------------------------------------------

fn plain() -> (Session, Instant) {
    let config = SandboxConfig {
        characters: vec!["Testa".into()],
        avatar: Some(Appearance::new(Body::Character { species: 1, sex: 1 })),
        ..SandboxConfig::default()
    };
    let mut s = Session::new(SandboxBackend::new(config).unwrap());
    let t = Instant::now();
    s.send(t, Command::Connect).unwrap();
    let mut ev = Vec::new();
    s.poll_into(t, &mut ev);
    assert!(matches!(ev.first(), Some(Event::Connected { .. })));
    (s, t)
}

fn create_cmd(name: &str, appearance: Option<Appearance>) -> Command {
    Command::Character(CharacterCommand::CreateCharacter {
        name: name.into(),
        appearance: appearance.map(Box::new),
        class: None,
        nationality: None,
    })
}

fn character_events(s: &mut Session, t: Instant) -> Vec<CharacterEvent> {
    let mut ev = Vec::new();
    s.poll_into(t, &mut ev);
    ev.into_iter()
        .filter_map(|e| match e {
            Event::Character(c) => Some(c),
            _ => None,
        })
        .collect()
}

fn entry_appearance(s: &mut Session, t: Instant, name: &str) -> Appearance {
    s.send(
        t,
        Command::EnterWorld {
            character: name.into(),
        },
    )
    .unwrap();
    let mut ev = Vec::new();
    for _ in 0..3 {
        s.poll_into(t, &mut ev);
    }
    ev.into_iter()
        .find_map(|e| match e {
            Event::AppearanceChanged { appearance, .. } => Some(*appearance),
            _ => None,
        })
        .expect("player appearance")
}

#[test]
fn p1_p5_plain_sandbox_creation() {
    let look = Look {
        entries: vec![LookEntry {
            property: 0x7FFF_0001,
            value: PropertyValue::Float(0.25),
        }],
    };
    let cora = Appearance::new(Body::Character {
        species: 23,
        sex: 1,
    })
    .with_look(Some(look.clone()));
    // P1.
    let (mut s, t) = plain();
    s.send(
        t,
        Command::Character(CharacterCommand::CreateCharacter {
            name: "Cora".into(),
            appearance: Some(Box::new(cora.clone())),
            class: Some(5),
            nationality: Some(2),
        }),
    )
    .unwrap();
    assert_eq!(
        character_events(&mut s, t),
        vec![CharacterEvent::CharacterInfo {
            name: "Cora".into(),
            body: Some(Body::Character {
                species: 23,
                sex: 1
            }),
            class: Some(5),
            level: None,
            look: Some(Box::new(look.clone())),
        }]
    );
    // P5: deletion is accepted and ignored.
    s.send(
        t,
        Command::Character(CharacterCommand::DeleteCharacter {
            name: "Cora".into(),
        }),
    )
    .unwrap();
    assert!(character_events(&mut s, t).is_empty());
    assert_eq!(entry_appearance(&mut s, t, "Cora"), cora);

    // P2.
    let (mut s, t) = plain();
    s.send(t, create_cmd("Testa", None)).unwrap();
    assert_eq!(
        character_events(&mut s, t),
        vec![CharacterEvent::CharacterRefused {
            name: "Testa".into(),
            reason: Text::Literal("A character with this name already exists.".into()),
        }]
    );
    // P3: 63 new names fit beside "Testa"; the 64th new one does not.
    for i in 0..63 {
        s.send(t, create_cmd(&format!("N{i}"), None)).unwrap();
    }
    let ev = character_events(&mut s, t);
    assert_eq!(ev.len(), 63);
    assert!(
        ev.iter()
            .all(|e| matches!(e, CharacterEvent::CharacterInfo { .. }))
    );
    s.send(t, create_cmd("Full", None)).unwrap();
    assert_eq!(
        character_events(&mut s, t),
        vec![CharacterEvent::CharacterRefused {
            name: "Full".into(),
            reason: Text::Literal("The character list is full.".into()),
        }]
    );
    // P4.
    let (mut s, t) = plain();
    s.send(t, create_cmd("Dora", None)).unwrap();
    assert_eq!(
        character_events(&mut s, t),
        vec![CharacterEvent::CharacterInfo {
            name: "Dora".into(),
            body: None,
            class: None,
            level: None,
            look: None,
        }]
    );
    assert_eq!(
        entry_appearance(&mut s, t, "Dora").body,
        Body::Character { species: 1, sex: 1 },
        "the configured avatar"
    );
}

// --- §14.9: overlay runs ------------------------------------------------------------------

const OVERLAY: &str = r#"tessera-scenario 1
title "Overlay"
tick 100ms
select $gear = did 0x70001234,0x70005678 all
item $sword $gear[0]
item $shield $gear[1]
at 0s give $sword
+0s give $shield
+0s equip $sword slot=17
+0s equip $shield slot=18
at 0.1s unequip $shield
at 0.2s equip $shield slot=18
"#;

#[derive(Debug, Default)]
struct Log {
    sent: Vec<(Duration, Appearance)>,
    commands: Vec<&'static str>,
}

/// A test double of the test-server backend (§14.9).
#[derive(Debug)]
struct Double {
    appearance_change: bool,
    minor: u16,
    refuse: bool,
    b0: Appearance,
    t0: Option<Instant>,
    queue: Vec<(Instant, Event)>,
    log: Arc<Mutex<Log>>,
}

const PLAYER: EntityId = EntityId(42);

impl Double {
    fn new(appearance_change: bool, minor: u16) -> (Self, Arc<Mutex<Log>>) {
        let log = Arc::new(Mutex::new(Log::default()));
        (
            Self {
                appearance_change,
                minor,
                refuse: false,
                b0: Appearance::new(Body::Character {
                    species: 23,
                    sex: 1,
                }),
                t0: None,
                queue: Vec::new(),
                log: Arc::clone(&log),
            },
            log,
        )
    }
}

impl Backend for Double {
    fn name(&self) -> &str {
        "double"
    }

    fn submit(&mut self, now: Instant, command: Command) -> Result<(), NetError> {
        self.log.lock().unwrap().commands.push(command.name());
        match command {
            Command::Connect => self.queue.push((
                now,
                Event::Connected {
                    characters: vec![tessera_net::CharacterSummary {
                        name: "Aldith".into(),
                    }],
                },
            )),
            Command::EnterWorld { character } => {
                self.t0 = Some(now);
                let loc = Location::new(RegionId(1), 10.0, 10.0, 0.0);
                self.queue.push((
                    now,
                    Event::WorldEntered {
                        player: PLAYER,
                        name: character,
                        transform: Transform::new(loc.position(), 0.0),
                    },
                ));
                self.queue.push((
                    now,
                    Event::AppearanceChanged {
                        id: PLAYER,
                        appearance: Box::new(self.b0.clone()),
                    },
                ));
                self.queue.push((
                    now,
                    Event::LocationReport {
                        id: PLAYER,
                        location: loc,
                        heading: 0.0,
                        anchor: None,
                        cause: LocationCause::Entered,
                    },
                ));
            }
            Command::Combat(_) | Command::Npc(_) | Command::Character(_) => {
                return Err(NetError::InvalidCommand("no gameplay on the test server"));
            }
            _ => {}
        }
        Ok(())
    }

    fn poll(&mut self, now: Instant, out: &mut Vec<Event>) {
        let (due, later): (Vec<_>, Vec<_>) = self.queue.drain(..).partition(|(t, _)| *t <= now);
        self.queue = later;
        out.extend(due.into_iter().map(|(_, e)| e));
    }
}

impl OverlayServer for Double {
    fn appearance_change(&self) -> bool {
        self.appearance_change
    }

    fn send_appearance(&mut self, appearance: &Appearance) -> Result<(), NetError> {
        let mut wire = appearance.clone();
        // The look never travels with AppearanceSet; slots only at minor ≥ 1.
        wire.look = None;
        if self.minor == 0 {
            for e in &mut wire.equipment {
                e.slot = None;
            }
        }
        let t = CLOCK.with(std::cell::Cell::get).expect("clock set");
        let rel = t.saturating_duration_since(self.t0.unwrap_or(t));
        self.log.lock().unwrap().sent.push((rel, wire.clone()));
        let answer = if self.refuse { self.b0.clone() } else { wire };
        self.queue.push((
            t + Duration::from_millis(50),
            Event::AppearanceChanged {
                id: PLAYER,
                appearance: Box::new(answer),
            },
        ));
        Ok(())
    }
}

thread_local! {
    /// The test's clock (the double reads it to stamp its answers).
    static CLOCK: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
}

struct Run {
    session: Session,
    log: Arc<Mutex<Log>>,
    t0: Instant,
    cursor: Duration,
    events: Vec<(Duration, Event)>,
}

fn overlay_run(double: Double, log: Arc<Mutex<Log>>) -> Run {
    let scenario = Scenario::parse(OVERLAY.as_bytes()).unwrap();
    scenario.check_overlay().unwrap();
    let resolved = Arc::new(scenario.resolve(&mut Synthetic).unwrap());
    let mut session = Session::new(OverlayBackend::new(double, resolved));
    let t0 = Instant::now();
    CLOCK.with(|c| c.set(Some(t0)));
    session.send(t0, Command::Connect).unwrap();
    let mut ev = Vec::new();
    session.poll_into(t0, &mut ev);
    session
        .send(
            t0,
            Command::EnterWorld {
                character: "Aldith".into(),
            },
        )
        .unwrap();
    Run {
        session,
        log,
        t0,
        cursor: Duration::ZERO,
        events: Vec::new(),
    }
}

impl Run {
    /// Polls every 10 ms up to `until` after world entry.
    fn advance(&mut self, until: Duration) {
        let mut buf = Vec::new();
        while self.cursor <= until {
            let t = self.cursor;
            let now = self.t0 + t;
            CLOCK.with(|c| c.set(Some(now)));
            self.session.poll_into(now, &mut buf);
            for e in buf.drain(..) {
                self.events.push((t, e));
            }
            self.cursor += Duration::from_millis(10);
        }
    }

    fn player_appearances(&self) -> Timeline {
        self.events
            .iter()
            .filter_map(|(t, e)| match e {
                Event::AppearanceChanged { id, appearance } if *id == PLAYER => Some((
                    *t,
                    appearance
                        .equipment
                        .iter()
                        .map(|e| (e.item.0, e.slot))
                        .collect(),
                )),
                _ => None,
            })
            .collect()
    }

    fn sent(&self) -> Timeline {
        self.log
            .lock()
            .unwrap()
            .sent
            .iter()
            .map(|(t, a)| (*t, a.equipment.iter().map(|e| (e.item.0, e.slot)).collect()))
            .collect()
    }
}

/// (time after world entry, the items and slots of an appearance).
type Timeline = Vec<(Duration, Vec<(u32, Option<u8>)>)>;

const BOTH: [(u32, Option<u8>); 2] = [(0x7000_1234, Some(17)), (0x7000_5678, Some(18))];

#[test]
fn o1_o2_coalesced_appearance_sets() {
    let (d, log) = Double::new(true, 1);
    let mut r = overlay_run(d, log);
    r.advance(Duration::from_millis(600));
    let sent = r.sent();
    // O1: one send at tick 0 with both items (both equips coalesced).
    assert_eq!(sent[0], (Duration::ZERO, BOTH.to_vec()));
    // O2: the changes at 0.1 s and 0.2 s give one more send, at 0.25 s, with both again.
    assert_eq!(sent.len(), 2, "{sent:?}");
    assert_eq!(sent[1], (Duration::from_millis(250), BOTH.to_vec()));
    // The feed emits no AppearanceChanged of its own: only the server's answers.
    let seen = r.player_appearances();
    assert_eq!(seen[0], (Duration::ZERO, Vec::new()), "B0 at world entry");
    assert_eq!(seen[1], (Duration::from_millis(50), BOTH.to_vec()));
    assert_eq!(seen.len(), 3, "{seen:?}");
    // The inventory events of the feed follow the server's, at tick 0.
    assert!(r.events.iter().any(|(t, e)| *t == Duration::ZERO
        && matches!(
            e,
            Event::Inventory(tessera_net::InventoryEvent::ItemMoved { .. })
        )));
    let o = r
        .session
        .backend::<OverlayBackend<Double>>()
        .expect("overlay");
    assert!(o.appearance_shared());
    assert_eq!(o.appearance_sets_sent(), 2);
}

#[test]
fn o3_without_appearance_change_the_feed_emits() {
    let (d, log) = Double::new(false, 1);
    let mut r = overlay_run(d, log);
    r.advance(Duration::from_millis(300));
    assert!(r.sent().is_empty());
    // Every change of the list emits (§5.5): at tick 0 the sword, then both.
    let seen: Vec<_> = r
        .player_appearances()
        .into_iter()
        .filter(|(_, items)| !items.is_empty())
        .collect();
    assert_eq!(
        seen,
        vec![
            (Duration::ZERO, vec![BOTH[0]]),
            (Duration::ZERO, BOTH.to_vec()),
            (Duration::from_millis(100), vec![BOTH[0]]),
            (Duration::from_millis(200), BOTH.to_vec()),
        ]
    );
}

/// O3 with stored equipment on the server: the feed's appearance is *B* with its equipment
/// replaced by *E* (§12.5.4), not *B*'s items plus *E*.
#[test]
fn o3_without_appearance_change_stored_equipment_is_replaced() {
    let (mut d, log) = Double::new(false, 1);
    d.b0.equipment.push(Equipped::new(DataId(0x7000_9999)));
    let mut r = overlay_run(d, log);
    r.advance(Duration::from_millis(300));
    assert!(r.sent().is_empty());
    let stored = vec![(0x7000_9999, None)];
    let seen = r.player_appearances();
    assert_eq!(
        seen[0],
        (Duration::ZERO, stored.clone()),
        "B0 at world entry"
    );
    let fed: Vec<_> = seen.into_iter().skip(1).collect();
    assert_eq!(
        fed,
        vec![
            (Duration::ZERO, vec![BOTH[0]]),
            (Duration::ZERO, BOTH.to_vec()),
            (Duration::from_millis(100), vec![BOTH[0]]),
            (Duration::from_millis(200), BOTH.to_vec()),
        ]
    );
}

#[test]
fn o4_minor_0_sends_no_slots() {
    let (d, log) = Double::new(true, 0);
    let mut r = overlay_run(d, log);
    r.advance(Duration::from_millis(100));
    let sent = r.sent();
    assert_eq!(
        sent[0],
        (
            Duration::ZERO,
            vec![(0x7000_1234, None), (0x7000_5678, None)]
        )
    );
}

#[test]
fn o5_stored_equipment_is_replaced() {
    let (mut d, log) = Double::new(true, 1);
    d.b0.equipment.push(Equipped::new(DataId(0x7000_9999)));
    let mut r = overlay_run(d, log);
    r.advance(Duration::from_millis(60));
    assert_eq!(r.sent(), vec![(Duration::ZERO, BOTH.to_vec())]);
}

#[test]
fn o6_refused_sets_are_not_retried() {
    let (mut d, log) = Double::new(true, 1);
    d.refuse = true;
    let mut r = overlay_run(d, log);
    r.advance(Duration::from_millis(90));
    assert_eq!(r.sent().len(), 1, "no retry before the equipment changes");
    r.advance(Duration::from_millis(600));
    let sent = r.sent();
    assert_eq!(sent.len(), 2, "{sent:?}");
    assert!(sent[1].0 >= Duration::from_millis(250));
    // Never more than one in flight: each send comes after the previous answer (50 ms).
    assert!(
        sent.windows(2)
            .all(|w| w[1].0 >= w[0].0 + Duration::from_millis(50))
    );
}

#[test]
fn o7_files_outside_the_subset_are_refused() {
    // §14.9 O7 writes `at 0s say "x"`, which misses the sender (a parse error, exit 3
    // too); the overlay check needs a file that parses.
    let e = Scenario::parse(b"tessera-scenario 1\nat 0s say player \"x\"\n")
        .unwrap()
        .check_overlay()
        .unwrap_err();
    assert_eq!(e.kind, ScenarioErrorKind::NotAllowedInOverlay);
}

#[test]
fn o8_routing() {
    let (d, log) = Double::new(true, 1);
    let mut r = overlay_run(d, log);
    r.advance(Duration::from_millis(20));
    let now = r.t0 + Duration::from_millis(30);
    r.session
        .send(
            now,
            Command::Chat {
                channel: ChatChannel::Say,
                text: "hi".into(),
            },
        )
        .unwrap();
    r.session
        .send(
            now,
            Command::Teleport {
                destination: Destination::Named("Bree".into()),
            },
        )
        .unwrap();
    r.session
        .send(
            now,
            Command::Inventory(InventoryCommand::EquipItem {
                item: ItemId(1),
                slot: Some(17),
            }),
        )
        .unwrap();
    // Every other command reaches the server half, which refuses `UseSkill` (§9.7). The
    // revision-5 part of O8 (`SelectTarget` answered locally by the test-server backend,
    // nothing sent) needs the real route and mapper: `tests/remote_selection.rs`.
    assert!(matches!(
        r.session.send(
            now,
            Command::Combat(tessera_net::CombatCommand::UseSkill {
                what: tessera_net::domain::Ref::Literal("x".into()),
                target: None,
            })
        ),
        Err(NetError::InvalidCommand(_))
    ));
    let commands = r.log.lock().unwrap().commands.clone();
    assert_eq!(
        commands,
        vec!["Connect", "EnterWorld", "Chat", "Teleport", "UseSkill"]
    );
}

/// §14.9 O10–O12 (revision 6, decision 0032 D3): no Combat-group step, trigger or input form
/// in an overlay file, refused at the statement's first token.
#[test]
fn o10_o12_combat_group_is_not_in_the_overlay_subset() {
    let refused = |text: &str, line: u32, col: u32| {
        let e = Scenario::parse(text.as_bytes())
            .unwrap_or_else(|e| panic!("{text:?} does not parse: {e}"))
            .check_overlay()
            .unwrap_err();
        assert_eq!(e.kind, ScenarioErrorKind::NotAllowedInOverlay, "{text:?}");
        assert_eq!((e.line, e.column), (line, col), "{text:?}");
    };
    // O10: steps.
    refused("tessera-scenario 1\nat 0s target player none\n", 2, 1);
    refused("tessera-scenario 1\nat 0s vital player health 5/10\n", 2, 1);
    // O11: a trigger on a Combat-group command.
    refused("tessera-scenario 1\non target any\nend\n", 2, 1);
    // O12: an input form.
    refused("tessera-scenario 1\ninput\nat 1s target none\nend\n", 3, 1);
}

/// `Session::remote_backend` finds the test-server backend inside an overlay, where
/// `backend::<RemoteBackend>()` cannot.
#[cfg(feature = "remote")]
#[test]
fn remote_backend_is_found_through_the_overlay() {
    use tessera_net::remote::{Password, RemoteBackend, RemoteConfig};
    let config =
        RemoteConfig::new("tessera://127.0.0.1:4000", "alice", Password::new("pw")).unwrap();
    let scenario = Scenario::parse(OVERLAY.as_bytes()).unwrap();
    let resolved = Arc::new(scenario.resolve(&mut Synthetic).unwrap());
    let session = Session::new(OverlayBackend::new(
        RemoteBackend::new(config.clone()).unwrap(),
        resolved,
    ));
    assert!(session.backend::<RemoteBackend>().is_none());
    assert!(session.remote_backend().is_some());
    let plain = Session::new(RemoteBackend::new(config).unwrap());
    assert!(plain.remote_backend().is_some());
    let sandbox = Session::new(SandboxBackend::new(SandboxConfig::default()).unwrap());
    assert!(sandbox.remote_backend().is_none());
}
