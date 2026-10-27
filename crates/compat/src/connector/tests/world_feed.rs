// SPDX-License-Identifier: GPL-3.0-or-later
//! The world feed of the session driver against a scripted synthetic server: entities
//! created, moved, changed and removed (`official-messages-world.md` §5.1–§5.4, §5.7,
//! §5.8.1, §6), remote motion (`official-messages-play.md` §4.4–§4.6) and chat categories
//! from the install (§6.7).

use super::*;
use crate::connector::entities::{FeedCounters, PID_IS_INANIMATE_PROP, PID_IS_NPC_AGENT};
use crate::connector::session_phase::MAX_BATCH_DEPTH;
// The synthetic service's builders come through the parent test module.

use crate::play::chat::{CATEGORY_SAY, CATEGORY_TELL, OPCODE_UI_PROPERTY_EVENT};
use crate::play::movement::{EntitySplinePath, MotionControllerUpdate, ServerSetPosition};
use crate::world::{
    EncodedPropertyEntry, EncodedPropertyValue, EntityAppearanceDesc, EntityCreate,
    PID_AGENT_SPECIES, PID_SEX, PositionUpdate, PropertyRemoval, SinglePropertyUpdate,
    SubEventBatch, SubEventFrame, VisualDescriptionUpdate, WirePosition,
};
use tessera_net::{Body, DataId, EntityId, EntityKind};

/// The local player of [`session_ready`].
const PLAYER: u64 = 0x0100_0000_0000_0007;
/// A second entity.
const OTHER: u64 = 0x0300_0000_0000_0202;
/// A template whose baseline marks an inanimate object.
const PROP_TEMPLATE: u32 = 0x7000_0456;

/// The synthetic install with a typed dictionary, template baselines and, optionally, the
/// chat category enum's keys.
#[derive(Debug)]
struct Typed {
    base: Records,
    chat_keys: Option<Vec<u32>>,
}

impl InstallData for Typed {
    fn record(&self, did: u32, max_bytes: usize) -> Option<Vec<u8>> {
        self.base.record(did, max_bytes)
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        DICTIONARY.iter().find(|(p, _)| *p == pid).map(|(_, t)| *t)
    }

    fn template_value(&self, template_did: u32, pid: u32) -> Option<EncodedPropertyValue> {
        (template_did == PROP_TEMPLATE && pid == PID_IS_INANIMATE_PROP)
            .then_some(EncodedPropertyValue::Boolean(true))
    }

    fn chat_category_keys(&self) -> Option<Vec<u32>> {
        self.chat_keys.clone()
    }
}

/// A driver in the world as "Aldo" over the typed install, on the login connection.
fn in_world_with(chat_keys: Option<Vec<u32>>) -> (World, SessionDriver<FakeNet>, Server) {
    let mut start = session_start(0, None, Arc::default());
    start.schema = SchemaView(Arc::new(Typed {
        base: records(),
        chat_keys,
    }));
    let (world, mut d, mut login) = driver_at_characters_from(Service::default(), start);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    world.deliver(login.event(0x90, &session_ready()), LOGIN_SERVER);
    d.step(1.1, true, &go);
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. }))
    );
    (world, d, login)
}

fn in_world() -> (World, SessionDriver<FakeNet>, Server) {
    in_world_with(None)
}

/// Delivers `datagram` and runs a pass at `t`; the events it made.
fn deliver(world: &World, d: &mut SessionDriver<FakeNet>, datagram: Vec<u8>, t: f64) -> Vec<Event> {
    world.deliver(datagram, LOGIN_SERVER);
    d.step(t, true, &go);
    d.take_events()
}

fn created(events: &[Event]) -> Vec<&Event> {
    events
        .iter()
        .filter(|e| matches!(e, Event::EntityCreated { .. }))
        .collect()
}

/// The inner payloads the client sent with opcode `op` (after the envelope byte).
fn sent_payloads(world: &World, op: u32) -> Vec<Vec<u8>> {
    world
        .take_sent()
        .into_iter()
        .filter_map(|(d, _)| decode_datagram(&d, false, None, false).ok())
        .flat_map(|pkt| pkt.fragments)
        .filter(|f| f.header.blob_header_word == Some(op))
        .filter_map(|f| f.payload.get(1..).map(<[u8]>::to_vec))
        .collect()
}

fn create(entity: u64, template: u32) -> Vec<u8> {
    EntityCreate {
        template_did: template,
        entity_id: entity,
    }
    .to_bytes()
    .expect("0x98")
}

fn props_payload(table: &EncodedPropertyTable) -> Vec<u8> {
    let mut w = crate::world::WireWriter::new();
    crate::world::PropertyTableUpdate {
        scope: crate::world::PropertyScope::Entity,
        properties: table.clone(),
    }
    .encode(&mut w)
    .expect("0x52");
    w.into_vec()
}

fn batch(event_id: u32, frames: Vec<SubEventFrame>) -> Vec<u8> {
    SubEventBatch {
        event_id,
        frames,
        entry_batch_tail_flag: (event_id == SubEventBatch::EVENT_ID_AOI_ENTER).then_some(1),
    }
    .to_bytes()
    .expect("batch")
}

fn frame(sub_event_id: u32, payload: Vec<u8>) -> SubEventFrame {
    SubEventFrame::Event {
        sub_event_id,
        payload,
    }
}

fn counters(d: &SessionDriver<FakeNet>) -> FeedCounters {
    d.feed_counters()
}

#[test]
fn an_area_batch_creates_the_npc_at_its_place_with_its_name() {
    let (world, mut d, mut login) = in_world();
    let b = entity_batch(
        NPC,
        NPC_TEMPLATE,
        &entity_properties(NPC_NAME, true),
        NPC_AT,
    );
    let events = deliver(&world, &mut d, login.event_to(0x0D, NPC, &b), 2.0);
    let made = created(&events);
    assert_eq!(made.len(), 1, "{events:?}");
    let Event::EntityCreated {
        id,
        kind,
        name,
        class,
        heading,
        appearance,
        tag,
        ..
    } = made[0]
    else {
        unreachable!("filtered")
    };
    assert_eq!(*id, EntityId(NPC));
    assert_eq!(*kind, EntityKind::NonPlayer);
    assert_eq!(name, NPC_NAME);
    assert_eq!(*class, Some(DataId(NPC_TEMPLATE)));
    assert!(heading.is_finite());
    assert_eq!(appearance.body, Body::Object(DataId(NPC_TEMPLATE)));
    assert!(tag.is_none());
    // The batch's own move is the creation's place, not a separate event.
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::EntityMoved { id, .. } if *id == EntityId(NPC)))
    );
    assert_eq!(counters(&d).created, 1);
}

#[test]
fn a_batch_to_the_default_recipient_applies_to_the_entity_it_created() {
    let (world, mut d, mut login) = in_world();
    let b = entity_batch(
        NPC,
        NPC_TEMPLATE,
        &entity_properties(NPC_NAME, true),
        NPC_AT,
    );
    let events = deliver(&world, &mut d, login.event(0x0D, &b), 2.0);
    let made = created(&events);
    assert_eq!(made.len(), 1, "{events:?}");
    assert!(matches!(made[0], Event::EntityCreated { name, .. } if name == NPC_NAME));
    assert_eq!(counters(&d).no_target, 0);
}

#[test]
fn a_player_with_species_and_sex_has_a_playable_body() {
    let (world, mut d, mut login) = in_world();
    let mut props = entity_properties("Rowan", false);
    props.push(PID_AGENT_SPECIES, EncodedPropertyValue::Enum(0x17));
    props.push(PID_SEX, EncodedPropertyValue::Enum(0x1000));
    let b = entity_batch(OTHER, 0x7000_0001, &props, NPC_AT);
    let events = deliver(&world, &mut d, login.event_to(0x0D, OTHER, &b), 2.0);
    let made = created(&events);
    assert!(
        matches!(
            made.as_slice(),
            [Event::EntityCreated { kind: EntityKind::Player, appearance, .. }]
                if appearance.body == Body::Character { species: 0x17, sex: 0x1000 }
        ),
        "{events:?}"
    );
    // A later sex change is an appearance change (`0x88`, §5.8.1).
    let mut w = crate::world::WireWriter::new();
    SinglePropertyUpdate {
        event_id: 0x88,
        entry: EncodedPropertyEntry::new(PID_SEX, EncodedPropertyValue::Enum(0x2000)),
    }
    .encode(&mut w)
    .expect("0x88");
    let change = w.into_vec();
    let events = deliver(&world, &mut d, login.event_to(0x88, OTHER, &change), 2.5);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::AppearanceChanged { id, appearance }
                if *id == EntityId(OTHER)
                    && appearance.body == Body::Character { species: 0x17, sex: 0x2000 }
        )),
        "{events:?}"
    );
    // Removing the species leaves the template as the body (`0x0E`).
    let mut w = crate::world::WireWriter::new();
    PropertyRemoval {
        event_id: 0x0E,
        property_id: PID_AGENT_SPECIES,
    }
    .encode(&mut w)
    .expect("0x0E");
    let removal = w.into_vec();
    let events = deliver(&world, &mut d, login.event_to(0x0E, OTHER, &removal), 2.6);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::AppearanceChanged { appearance, .. }
                if appearance.body == Body::Object(DataId(0x7000_0001))
        )),
        "{events:?}"
    );
}

#[test]
fn the_template_baseline_classifies_and_unknown_kinds_are_counted() {
    let (world, mut d, mut login) = in_world();
    // No flags on the wire: the template's baseline says inanimate object.
    let b = entity_batch(OTHER, PROP_TEMPLATE, &EncodedPropertyTable::new(), NPC_AT);
    let events = deliver(&world, &mut d, login.event_to(0x0D, OTHER, &b), 2.0);
    assert!(
        matches!(
            created(&events).as_slice(),
            [Event::EntityCreated { kind: EntityKind::Object, name, .. }] if name.is_empty()
        ),
        "{events:?}"
    );
    assert_eq!(counters(&d).unclassified, 0);
    // Neither flag anywhere: shown as a non-player and counted.
    let b = entity_batch(NPC, NPC_TEMPLATE, &EncodedPropertyTable::new(), NPC_AT);
    let events = deliver(&world, &mut d, login.event_to(0x0D, NPC, &b), 2.5);
    assert!(matches!(
        created(&events).as_slice(),
        [Event::EntityCreated {
            kind: EntityKind::NonPlayer,
            ..
        }]
    ));
    assert_eq!(counters(&d).unclassified, 1);
}

#[test]
fn a_creation_waits_for_its_first_position() {
    let (world, mut d, mut login) = in_world();
    let events = deliver(
        &world,
        &mut d,
        login.event(0x98, &create(NPC, NPC_TEMPLATE)),
        2.0,
    );
    assert!(created(&events).is_empty(), "{events:?}");
    let mut props = EncodedPropertyTable::new();
    props.push(PID_IS_NPC_AGENT, EncodedPropertyValue::Boolean(true));
    let events = deliver(
        &world,
        &mut d,
        login.event_to(0x52, NPC, &props_payload(&props)),
        2.1,
    );
    assert!(created(&events).is_empty());
    // An attachment is no place (counted); a world position is.
    let attached = PositionUpdate {
        entity_id: NPC,
        seq_byte: None,
        velocity: [None; 3],
        anim_state: None,
        placement: crate::world::EntityPlacement::Attachment {
            parent_id: PLAYER,
            attach_slot: 1,
            extension: None,
        },
        interp_param_a: None,
        interp_param_b: None,
    }
    .to_bytes()
    .expect("0xCE");
    let events = deliver(&world, &mut d, login.event(0xCE, &attached), 2.2);
    assert!(created(&events).is_empty());
    assert_eq!(counters(&d).unplaced, 1);
    let events = deliver(
        &world,
        &mut d,
        login.event(0xCE, &position(NPC, NPC_AT, 0, [0.0; 3])),
        2.3,
    );
    assert!(
        matches!(
            created(&events).as_slice(),
            [Event::EntityCreated {
                kind: EntityKind::NonPlayer,
                ..
            }]
        ),
        "{events:?}"
    );
    // `0xBF` places a waiting entity too.
    deliver(
        &world,
        &mut d,
        login.event(0x98, &create(OTHER, NPC_TEMPLATE)),
        2.4,
    );
    let set = ServerSetPosition {
        state_flags: 1,
        position: Some(WirePosition::from_components(
            1,
            1,
            2,
            0,
            0,
            [1.0, 2.0, 3.0],
            [1.0, 0.0, 0.0, 0.0],
        )),
        velocity: None,
        set_pos_flags: 0,
    }
    .to_bytes()
    .expect("0xBF");
    let events = deliver(&world, &mut d, login.event_to(0xBF, OTHER, &set), 2.5);
    assert_eq!(created(&events).len(), 1, "{events:?}");
}

#[test]
fn created_entities_move_by_position_set_position_and_path() {
    let (world, mut d, mut login) = in_world();
    let b = entity_batch(
        NPC,
        NPC_TEMPLATE,
        &entity_properties(NPC_NAME, true),
        NPC_AT,
    );
    deliver(&world, &mut d, login.event_to(0x0D, NPC, &b), 2.0);
    let moved = |events: &[Event]| -> Vec<(Vec3, Vec3)> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::EntityMoved {
                    id,
                    transform,
                    velocity,
                } if *id == EntityId(NPC) => Some((transform.position, *velocity)),
                _ => None,
            })
            .collect()
    };
    let events = deliver(
        &world,
        &mut d,
        login.event(0xCE, &position(NPC, [30.0, 40.0, 6.0], 0, [1.5, 0.0, 0.0])),
        2.1,
    );
    let m = moved(&events);
    assert_eq!(m.len(), 1, "{events:?}");
    assert!((m[0].1.x - 1.5).abs() < 1e-6);
    let set = ServerSetPosition {
        state_flags: 0x05,
        position: Some(WirePosition::from_components(
            1,
            1,
            2,
            0,
            0,
            [31.0, 40.0, 6.0],
            [1.0, 0.0, 0.0, 0.0],
        )),
        velocity: Some([0.5, 0.0, 0.0]),
        set_pos_flags: 0,
    }
    .to_bytes()
    .expect("0xBF");
    let events = deliver(&world, &mut d, login.event_to(0xBF, NPC, &set), 2.2);
    assert_eq!(moved(&events).len(), 1, "{events:?}");
    // A path: a move to its destination, facing along its last step (+X: heading 0).
    let wp =
        |x: f32| WirePosition::from_components(1, 1, 2, 0, 0, [x, 40.0, 6.0], [1.0, 0.0, 0.0, 0.0]);
    let path = EntitySplinePath {
        path_type: 1,
        entity_id: NPC,
        path_flags: 0,
        speed_param: Some(2.0),
        waypoints: Some(vec![wp(32.0), wp(36.0), wp(40.0)]),
    }
    .to_bytes()
    .expect("0x59");
    let events = deliver(&world, &mut d, login.event(0x59, &path), 2.3);
    let m: Vec<&Event> = events
        .iter()
        .filter(|e| matches!(e, Event::EntityMoved { .. }))
        .collect();
    assert!(
        matches!(m.as_slice(), [Event::EntityMoved { transform, velocity, .. }]
            if (transform.position.x - 200.0).abs() < 1e-3
                && transform.heading.abs() < 1e-5
                && *velocity == Vec3::new(0.0, 0.0, 0.0)),
        "{events:?}"
    );
    assert_eq!(counters(&d).paths, 1);
    // Motion controllers and animation layers reach no event, counted.
    let ed = MotionControllerUpdate::NoController.to_bytes();
    let events = deliver(&world, &mut d, login.event_to(0xED, NPC, &ed), 2.4);
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(counters(&d).motion_controllers, 1);
    let bd = crate::play::movement::encode_anim_layers(&crate::world::EntityAnimLayerState {
        layers: Vec::new(),
    })
    .expect("0xBD");
    let events = deliver(&world, &mut d, login.event_to(0xBD, NPC, &bd), 2.5);
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(counters(&d).anim_layers, 1);
}

#[test]
fn an_unknown_entity_reaches_no_event_and_is_queried_once() {
    let (world, mut d, mut login) = in_world();
    let _ = world.take_sent();
    for (i, t) in [2.0, 2.1].into_iter().enumerate() {
        let x = 10.0 + f32::from(u8::try_from(i).expect("small"));
        let events = deliver(
            &world,
            &mut d,
            login.event(0xCE, &position(OTHER, [x, 1.0, 1.0], 0, [0.0; 3])),
            t,
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::EntityMoved { .. })),
            "{events:?}"
        );
    }
    let mut props = EncodedPropertyTable::new();
    props.push(PID_IS_NPC_AGENT, EncodedPropertyValue::Boolean(true));
    deliver(
        &world,
        &mut d,
        login.event_to(0x52, OTHER, &props_payload(&props)),
        2.2,
    );
    let queries = sent_payloads(&world, 0xD0);
    let mut expected = vec![0u8];
    expected.extend_from_slice(&OTHER.to_le_bytes());
    assert_eq!(queries, vec![expected], "one `0xD0` for the entity");
    let c = counters(&d);
    assert_eq!(c.unknown_entity_events, 3);
    assert_eq!(c.queries, 1);
}

#[test]
fn a_despawn_removes_a_created_entity_and_counts_the_rest() {
    let (world, mut d, mut login) = in_world();
    let b = entity_batch(
        NPC,
        NPC_TEMPLATE,
        &entity_properties(NPC_NAME, true),
        NPC_AT,
    );
    deliver(&world, &mut d, login.event_to(0x0D, NPC, &b), 2.0);
    let events = deliver(&world, &mut d, login.event_to(0x40, NPC, &[0]), 2.1);
    assert_eq!(events, vec![Event::EntityDespawned { id: EntityId(NPC) }]);
    // Again: it is gone; the local player is never despawned by this.
    let events = deliver(&world, &mut d, login.event_to(0x40, NPC, &[0]), 2.2);
    assert!(events.is_empty());
    let events = deliver(&world, &mut d, login.event_to(0x40, PLAYER, &[0]), 2.3);
    assert!(events.is_empty());
    assert_eq!(counters(&d).despawn_unknown, 2);
    assert_eq!(counters(&d).despawned, 1);
}

#[test]
fn the_local_player_is_never_created() {
    let (world, mut d, mut login) = in_world();
    let b = entity_batch(
        PLAYER,
        0x7000_0001,
        &entity_properties("Aldo", false),
        NPC_AT,
    );
    let events = deliver(&world, &mut d, login.event_to(0x0D, PLAYER, &b), 2.0);
    assert!(created(&events).is_empty(), "{events:?}");
    assert_eq!(counters(&d).own_player, 1);
}

#[test]
fn appearance_updates_take_a_uniform_scale_and_count_the_rest() {
    let (world, mut d, mut login) = in_world();
    let b = entity_batch(
        NPC,
        NPC_TEMPLATE,
        &entity_properties(NPC_NAME, true),
        NPC_AT,
    );
    deliver(&world, &mut d, login.event_to(0x0D, NPC, &b), 2.0);
    let visual = |desc: EntityAppearanceDesc| {
        VisualDescriptionUpdate { visual_desc: desc }
            .to_bytes()
            .expect("0x89")
    };
    let events = deliver(
        &world,
        &mut d,
        login.event_to(
            0x89,
            NPC,
            &visual(EntityAppearanceDesc {
                scale: Some([1.5; 3]),
                param_a: Some(0.5),
                ..EntityAppearanceDesc::default()
            }),
        ),
        2.1,
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::AppearanceChanged { appearance, .. } if (appearance.scale - 1.5).abs() < 1e-6
        )),
        "{events:?}"
    );
    assert_eq!(counters(&d).appearance_fields_skipped, 1, "param_a");
    // A scale that is not uniform is not shown (no change).
    let events = deliver(
        &world,
        &mut d,
        login.event_to(
            0x89,
            NPC,
            &visual(EntityAppearanceDesc {
                scale: Some([1.0, 2.0, 1.0]),
                ..EntityAppearanceDesc::default()
            }),
        ),
        2.2,
    );
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(counters(&d).appearance_fields_skipped, 2);
    // A base model cannot be decoded (§2.8 OPEN): refused and counted.
    let mut base_model = vec![0u8];
    base_model.extend_from_slice(&1u32.to_le_bytes());
    base_model.extend_from_slice(&0x0400_0001u32.to_le_bytes());
    let before = d.counters();
    let events = deliver(&world, &mut d, login.event_to(0x89, NPC, &base_model), 2.3);
    assert!(events.is_empty());
    assert_eq!(
        d.counters().appearance_base_model,
        before.appearance_base_model + 1
    );
    assert_eq!(d.counters().decode_errors, before.decode_errors + 1);
}

#[test]
fn nested_batches_are_read_and_their_limits_counted() {
    let (world, mut d, mut login) = in_world();
    let inner = batch(
        SubEventBatch::EVENT_ID_SUB_EVENT,
        vec![
            frame(0x98, create(NPC, NPC_TEMPLATE)),
            frame(0x52, props_payload(&entity_properties(NPC_NAME, true))),
            frame(0xCE, position(NPC, NPC_AT, 0, [0.0; 3])),
        ],
    );
    let nested_aoi = batch(SubEventBatch::EVENT_ID_AOI_ENTER, Vec::new());
    let outer = batch(
        SubEventBatch::EVENT_ID_AOI_ENTER,
        vec![
            SubEventFrame::Null {
                field_a: 1,
                field_b: 2,
            },
            frame(SubEventBatch::EVENT_ID_SUB_EVENT, inner),
            frame(SubEventBatch::EVENT_ID_AOI_ENTER, nested_aoi),
        ],
    );
    let events = deliver(&world, &mut d, login.event(0x0D, &outer), 2.0);
    assert_eq!(created(&events).len(), 1, "{events:?}");
    let c = counters(&d);
    assert_eq!(c.null_sub_events, 1);
    assert_eq!(c.nested_aoi_batches, 1);
    // A chain of `0x4C` deeper than the limit stops there.
    let mut deep = batch(SubEventBatch::EVENT_ID_SUB_EVENT, Vec::new());
    for _ in 0..MAX_BATCH_DEPTH {
        deep = batch(
            SubEventBatch::EVENT_ID_SUB_EVENT,
            vec![frame(SubEventBatch::EVENT_ID_SUB_EVENT, deep)],
        );
    }
    deliver(&world, &mut d, login.event(0x4C, &deep), 2.1);
    assert_eq!(counters(&d).deep_batches, 1);
}

#[test]
fn leaving_the_world_forgets_its_entities() {
    let (world, mut d, mut login) = in_world();
    let b = entity_batch(
        NPC,
        NPC_TEMPLATE,
        &entity_properties(NPC_NAME, true),
        NPC_AT,
    );
    deliver(&world, &mut d, login.event_to(0x0D, NPC, &b), 2.0);
    let events = deliver(&world, &mut d, login.event(0x3000_001B, &[0]), 2.1);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::ReturnedToCharacterSelect { .. }))
    );
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        3.0,
    )
    .expect("enter again");
    deliver(&world, &mut d, login.event(0x90, &session_ready()), 3.1);
    let before = counters(&d).unknown_entity_events;
    let events = deliver(
        &world,
        &mut d,
        login.event(0xCE, &position(NPC, NPC_AT, 0, [0.0; 3])),
        3.2,
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::EntityMoved { .. }))
    );
    assert_eq!(counters(&d).unknown_entity_events, before + 1);
}

#[test]
fn chat_categories_come_from_the_installs_enum() {
    // The install's enum has Tell but not Say: a Say line is not mapped (counted).
    let (world, mut d, mut login) = in_world_with(Some(vec![CATEGORY_TELL, 0x1C]));
    let line = chat_line(CATEGORY_SAY, NPC_NAME, NPC, "Hello.");
    let events = deliver(
        &world,
        &mut d,
        login.event(OPCODE_UI_PROPERTY_EVENT, &line),
        2.0,
    );
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(d.play_counters().unmapped_category, 1);
    let tell = chat_line(CATEGORY_TELL, NPC_NAME, NPC, "Psst.");
    let events = deliver(
        &world,
        &mut d,
        login.event(OPCODE_UI_PROPERTY_EVENT, &tell),
        2.1,
    );
    assert_eq!(
        events,
        vec![Event::ChatMessage {
            channel: ChatChannel::Tell {
                peer: NPC_NAME.to_owned()
            },
            sender: Some(NPC_NAME.to_owned()),
            text: "Psst.".to_owned(),
        }]
    );
    // Outgoing Say has no category now: refused, never sent.
    assert!(
        d.command(
            &Command::Chat {
                channel: ChatChannel::Say,
                text: "hi".to_owned(),
            },
            2.2,
        )
        .is_err()
    );
}

#[test]
fn without_the_installs_enum_the_spec_numbers_map_say_and_tell() {
    let (world, mut d, mut login) = in_world();
    let line = chat_line(CATEGORY_SAY, NPC_NAME, NPC, "Hello.");
    let events = deliver(
        &world,
        &mut d,
        login.event(OPCODE_UI_PROPERTY_EVENT, &line),
        2.0,
    );
    assert_eq!(
        events,
        vec![Event::ChatMessage {
            channel: ChatChannel::Say,
            sender: Some(NPC_NAME.to_owned()),
            text: "Hello.".to_owned(),
        }]
    );
    // An unnamed category (a user slot, Yell or Fellowship) is counted, not shown.
    let slot = chat_line(0x1C, NPC_NAME, NPC, "Room line.");
    let events = deliver(
        &world,
        &mut d,
        login.event(OPCODE_UI_PROPERTY_EVENT, &slot),
        2.1,
    );
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(d.play_counters().unmapped_category, 1);
}
