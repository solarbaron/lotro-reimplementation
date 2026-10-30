// SPDX-License-Identifier: GPL-3.0-or-later
//! The entity feed on its own: its limits, the batch-target rule, the memory kept per
//! entity and the events one message makes (module docs of [`super`]).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tessera_net::model::RegionId;

use super::*;
use crate::connector::data::InstallData;
use crate::play::movement::PATH_TYPE_SPLINE;
use crate::world::PropertyType;

const PLAYER: u64 = 0x0100_0000_0000_0007;
const NPC: u64 = 0x0300_0000_0000_0101;
const TEMPLATE: u32 = 0x7000_0123;
const PROP_TEMPLATE: u32 = 0x7000_0456;

/// A synthetic install: literal texts resolve as they are, a reference to `"Ref <token>"`;
/// one template is an inanimate object; whether templates count as decoded is chosen.
#[derive(Debug, Default)]
struct Stub {
    uncached: bool,
    uncached_texts: bool,
    resolves: AtomicUsize,
    template_reads: AtomicUsize,
}

impl InstallData for Stub {
    fn record(&self, _did: u32, _max_bytes: usize) -> Option<Vec<u8>> {
        None
    }

    fn property_type(&self, _pid: u32) -> Option<PropertyType> {
        None
    }

    fn resolve_text(&self, text: &LocalizedTokenText) -> Option<String> {
        self.resolves.fetch_add(1, Ordering::Relaxed);
        match &text.content {
            LocalizedTokenTextContent::Literal(s) => Some(s.clone()),
            LocalizedTokenTextContent::Reference { token_id, .. } => (*token_id != 0).then(|| {
                if text.arguments.is_empty() {
                    format!("Ref {token_id}")
                } else {
                    format!("Ref {token_id} with {}", text.arguments.len())
                }
            }),
        }
    }

    fn template_value(&self, template_did: u32, pid: u32) -> Option<EncodedPropertyValue> {
        if pid == FEED_PIDS[0] {
            self.template_reads.fetch_add(1, Ordering::Relaxed);
        }
        (template_did == PROP_TEMPLATE && pid == PID_IS_INANIMATE_PROP)
            .then_some(EncodedPropertyValue::Boolean(true))
    }

    fn template_cached(&self, _template_did: u32) -> bool {
        !self.uncached
    }

    fn text_cached(&self, _table_did: u32) -> bool {
        !self.uncached_texts
    }
}

fn data(stub: Stub) -> (SchemaView, Arc<Stub>) {
    let stub = Arc::new(stub);
    (SchemaView(Arc::clone(&stub) as Arc<dyn InstallData>), stub)
}

fn feed() -> EntityFeed {
    let mut f = EntityFeed::default();
    f.reset(Some(PLAYER));
    f
}

fn at(x: f64, y: f64) -> Location {
    Location::new(RegionId(1), x, y, 5.0)
}

fn create(f: &mut EntityFeed, id: u64, template: u32, out: &mut FeedOutput) -> CreateOutcome {
    f.create(
        EntityCreate {
            template_did: template,
            entity_id: id,
        },
        out,
    )
}

fn name_entry(text: LocalizedTokenText) -> EncodedPropertyEntry {
    EncodedPropertyEntry::new(
        PID_ENTITY_DISPLAY_NAME,
        EncodedPropertyValue::LocalizedTokenText(text),
    )
}

fn npc_entry(npc: bool) -> EncodedPropertyEntry {
    EncodedPropertyEntry::new(PID_IS_NPC_AGENT, EncodedPropertyValue::Boolean(npc))
}

/// Creates, names and places `id`, then flushes; the events.
fn shown(f: &mut EntityFeed, d: &SchemaView, id: u64) -> Vec<Event> {
    let mut out = FeedOutput::default();
    create(f, id, TEMPLATE, &mut out);
    f.properties(
        id,
        &[
            name_entry(LocalizedTokenText::literal("Watcher")),
            npc_entry(true),
        ],
        d,
        &mut out,
    );
    f.place(id, Some((at(10.0, 20.0), 0.5)), [0.0; 3], &mut out);
    f.flush(d, &mut out);
    out.events
}

fn count(events: &[Event], pred: impl Fn(&Event) -> bool) -> usize {
    events.iter().filter(|e| pred(e)).count()
}

#[test]
fn the_0x52_after_a_0x98_in_a_batch_takes_the_created_entity() {
    let created = Some(CreateOutcome::Tracked(NPC));
    let d = DEFAULT_RECIPIENT;
    // §5.1 item 2: the `0x52` after a `0x98` is its initial property set, in a `0x0D` or a
    // `0x4C`, whatever the batch's target.
    for aoi in [true, false] {
        assert_eq!(sub_event_target(aoi, 0x52, d, created), SubTarget::To(NPC));
        assert_eq!(sub_event_target(aoi, 0x52, 77, created), SubTarget::To(NPC));
    }
    // Every other sub-event, or a `0x52` after no `0x98`: the batch's target.
    for op in [0x40, 0x88, 0x89, 0xCC, 0xCE, 0x0E] {
        assert_eq!(
            sub_event_target(true, op, d, created),
            SubTarget::To(d),
            "{op:#x}"
        );
        assert_eq!(sub_event_target(false, op, 77, created), SubTarget::To(77));
    }
    assert_eq!(sub_event_target(true, 0x52, d, None), SubTarget::To(d));
    // After the player's own `0x98` the properties are the player's (dropped by the feed).
    assert_eq!(
        sub_event_target(true, 0x52, d, Some(CreateOutcome::Player(PLAYER))),
        SubTarget::To(PLAYER)
    );
    // After a refused `0x98`, nowhere; after one for the default recipient, the batch's.
    assert_eq!(
        sub_event_target(true, 0x52, d, Some(CreateOutcome::Refused)),
        SubTarget::Skip
    );
    assert_eq!(
        sub_event_target(true, 0x52, d, Some(CreateOutcome::NoTarget)),
        SubTarget::To(d)
    );
    assert_eq!(paired_target(None, 77), SubTarget::To(77));
}

fn ev(id: u32) -> SubEventFrame {
    SubEventFrame::Event {
        sub_event_id: id,
        payload: vec![0],
    }
}

const NULL: SubEventFrame = SubEventFrame::Null {
    field_a: 0,
    field_b: 0,
};

#[test]
fn each_0x98_takes_the_first_free_0x52_after_it() {
    // §5.1 item 2: null frames and other sub-events between them are skipped.
    let frames = [ev(0x98), NULL, ev(0xCE), ev(0x52), ev(0x52)];
    assert_eq!(
        pair_initial_properties(&frames),
        vec![None, None, None, Some(0), None]
    );
    // Two creations take the next two `0x52`s, in order; a `0x52` before any `0x98` is
    // no creation's.
    let frames = [
        ev(0x52),
        ev(0x98),
        ev(0x98),
        NULL,
        ev(0x52),
        ev(0x89),
        ev(0x52),
    ];
    assert_eq!(
        pair_initial_properties(&frames),
        vec![None, None, None, None, Some(1), None, Some(2)]
    );
    // A `0x98` with no `0x52` after it pairs with nothing.
    assert_eq!(
        pair_initial_properties(&[ev(0x98), ev(0xCE)]),
        vec![None, None]
    );
    assert!(pair_initial_properties(&[]).is_empty());
}

#[test]
fn waiting_entities_beyond_the_pending_limit_drop_the_oldest() {
    let mut f = feed();
    let mut out = FeedOutput::default();
    for i in 0..=MAX_PENDING as u64 {
        create(&mut f, 1000 + i, TEMPLATE, &mut out);
    }
    assert_eq!(f.pending(), MAX_PENDING);
    assert_eq!(f.tracked(), MAX_PENDING);
    assert!(!f.is_pending(1000), "the oldest is dropped");
    assert!(f.is_pending(1001));
    assert_eq!(f.counters().pending_dropped, 1);
}

#[test]
fn at_the_entity_limit_waiting_entities_make_room_and_placed_ones_never_do() {
    let (d, _) = data(Stub::default());
    let mut f = feed();
    let mut out = FeedOutput::default();
    // MAX_ENTITIES - 1 shown entities and one waiting.
    for i in 0..(MAX_ENTITIES - 1) as u64 {
        create(&mut f, 10_000 + i, TEMPLATE, &mut out);
        f.place(10_000 + i, Some((at(1.0, 1.0), 0.0)), [0.0; 3], &mut out);
    }
    f.flush(&d, &mut out);
    assert_eq!(f.counters().created, (MAX_ENTITIES - 1) as u64);
    create(&mut f, 5, TEMPLATE, &mut out);
    assert_eq!(f.tracked(), MAX_ENTITIES);
    // A new one at the limit drops the waiting one first.
    assert_eq!(
        create(&mut f, 6, TEMPLATE, &mut out),
        CreateOutcome::Tracked(6)
    );
    assert!(!f.is_pending(5));
    assert_eq!(f.counters().pending_dropped, 1);
    // Placed (not yet shown) entities are not dropped either: the next one is refused.
    f.place(6, Some((at(1.0, 1.0), 0.0)), [0.0; 3], &mut out);
    assert_eq!(
        create(&mut f, 7, TEMPLATE, &mut out),
        CreateOutcome::Refused
    );
    assert_eq!(f.counters().overflow, 1);
    assert_eq!(f.tracked(), MAX_ENTITIES);
    f.flush(&d, &mut out);
    assert!(f.is_created(6));
}

#[test]
fn queries_stop_at_their_limit_and_are_sent_once_per_entity() {
    let mut f = feed();
    let mut out = FeedOutput::default();
    for i in 0..=MAX_QUERIED as u64 {
        f.unknown_entity(50_000 + i, &mut out);
    }
    f.unknown_entity(50_000, &mut out);
    assert_eq!(out.outgoing.len(), MAX_QUERIED);
    assert_eq!(f.counters().queries, MAX_QUERIED as u64);
    assert_eq!(f.counters().unknown_entity_events, MAX_QUERIED as u64 + 2);
    // The default recipient and the player are never queried.
    let mut out = FeedOutput::default();
    f.reset(Some(PLAYER));
    f.unknown_entity(DEFAULT_RECIPIENT, &mut out);
    f.unknown_entity(PLAYER, &mut out);
    assert!(out.outgoing.is_empty());
}

#[test]
fn a_second_0x98_for_a_shown_entity_despawns_it_then_creates_it_again() {
    let (d, _) = data(Stub::default());
    let mut f = feed();
    assert_eq!(
        count(&shown(&mut f, &d, NPC), |e| matches!(
            e,
            Event::EntityCreated { .. }
        )),
        1
    );
    let events = shown(&mut f, &d, NPC);
    assert!(
        matches!(
            events.as_slice(),
            [Event::EntityDespawned { id }, Event::EntityCreated { id: again, .. }]
                if *id == EntityId(NPC) && *again == EntityId(NPC)
        ),
        "{events:?}"
    );
    assert_eq!(f.counters().recreated, 1);
}

#[test]
fn a_0x98_for_the_default_recipient_or_the_player_tracks_nothing() {
    let mut f = feed();
    let mut out = FeedOutput::default();
    assert_eq!(
        create(&mut f, DEFAULT_RECIPIENT, TEMPLATE, &mut out),
        CreateOutcome::NoTarget
    );
    assert_eq!(
        create(&mut f, PLAYER, TEMPLATE, &mut out),
        CreateOutcome::Player(PLAYER)
    );
    assert_eq!(f.tracked(), 0);
    assert_eq!(f.counters().no_target, 1);
    assert_eq!(f.counters().own_player, 1);
}

#[test]
fn a_huge_literal_name_keeps_only_the_cut_name() {
    let (d, _) = data(Stub::default());
    let mut f = feed();
    let mut out = FeedOutput::default();
    create(&mut f, NPC, TEMPLATE, &mut out);
    let huge = "N".repeat(2 * 1024 * 1024);
    f.properties(
        NPC,
        &[name_entry(LocalizedTokenText::literal(huge))],
        &d,
        &mut out,
    );
    let kept = f.name_bytes(NPC).expect("tracked");
    assert!(kept <= MAX_NAME_LEN, "{kept}");
    f.place(NPC, Some((at(1.0, 1.0), 0.0)), [0.0; 3], &mut out);
    f.flush(&d, &mut out);
    assert!(
        matches!(out.events.as_slice(), [Event::EntityCreated { name, .. }] if name.len() <= MAX_NAME_LEN && !name.is_empty()),
        "{:?}",
        out.events.len()
    );
    // A reference keeps no text at all.
    create(&mut f, 9, TEMPLATE, &mut out);
    f.properties(
        9,
        &[name_entry(LocalizedTokenText::reference(7, 0x2500_0001))],
        &d,
        &mut out,
    );
    assert_eq!(f.name_bytes(9), Some(0));
}

#[test]
fn uncached_templates_are_read_a_few_per_pass_and_the_rest_wait() {
    let (d, stub) = data(Stub {
        uncached: true,
        ..Stub::default()
    });
    let mut f = feed();
    let mut out = FeedOutput::default();
    let n = MAX_TEMPLATE_LOADS_PER_PASS + 5;
    for i in 0..n as u64 {
        create(&mut f, 100 + i, TEMPLATE + 1 + i as u32, &mut out);
        f.place(100 + i, Some((at(1.0, 1.0), 0.0)), [0.0; 3], &mut out);
    }
    f.begin_pass();
    f.flush(&d, &mut out);
    assert_eq!(f.counters().created, MAX_TEMPLATE_LOADS_PER_PASS as u64);
    assert_eq!(
        stub.template_reads.load(Ordering::Relaxed),
        MAX_TEMPLATE_LOADS_PER_PASS
    );
    assert!(f.has_deferred());
    assert_eq!(f.counters().template_loads_deferred, 5);
    // Later messages of the same pass do not retry them (counted once per pass).
    for _ in 0..3 {
        f.flush(&d, &mut out);
    }
    assert_eq!(f.counters().template_loads_deferred, 5);
    assert_eq!(f.counters().created, MAX_TEMPLATE_LOADS_PER_PASS as u64);
    // The next pass reads the rest, in their `0x98` order.
    let mut out = FeedOutput::default();
    f.begin_pass();
    f.flush(&d, &mut out);
    assert!(out.events.is_empty(), "a message's flush retries nothing");
    f.retry_deferred(&d, &mut out);
    assert_eq!(f.counters().created, n as u64);
    assert!(!f.has_deferred());
    let ids: Vec<u64> = out
        .events
        .iter()
        .filter_map(|e| match e {
            Event::EntityCreated { id, .. } => Some(id.0),
            _ => None,
        })
        .collect();
    let expected: Vec<u64> = (MAX_TEMPLATE_LOADS_PER_PASS as u64..n as u64)
        .map(|i| 100 + i)
        .collect();
    assert_eq!(ids, expected);
}

#[test]
fn names_in_uncached_tables_are_rendered_a_few_per_pass_and_the_rest_wait() {
    let (d, stub) = data(Stub {
        uncached_texts: true,
        ..Stub::default()
    });
    let mut f = feed();
    let mut out = FeedOutput::default();
    let n = MAX_TEXT_LOADS_PER_PASS + 3;
    for i in 0..n as u64 {
        create(&mut f, 100 + i, TEMPLATE, &mut out);
        f.properties(
            100 + i,
            &[name_entry(LocalizedTokenText::reference(
                7 + i as u32,
                0x2500_0001,
            ))],
            &d,
            &mut out,
        );
        f.place(100 + i, Some((at(1.0, 1.0), 0.0)), [0.0; 3], &mut out);
    }
    // A literal name and a reference to a DID that is not a string table read no table:
    // they are created whatever the budget.
    create(&mut f, 900, TEMPLATE, &mut out);
    f.properties(
        900,
        &[name_entry(LocalizedTokenText::literal("Watcher"))],
        &d,
        &mut out,
    );
    f.place(900, Some((at(1.0, 1.0), 0.0)), [0.0; 3], &mut out);
    create(&mut f, 901, TEMPLATE, &mut out);
    f.properties(
        901,
        &[name_entry(LocalizedTokenText::reference(7, 0x0700_0001))],
        &d,
        &mut out,
    );
    f.place(901, Some((at(1.0, 1.0), 0.0)), [0.0; 3], &mut out);
    let resolves = stub.resolves.load(Ordering::Relaxed);
    f.begin_pass();
    f.flush(&d, &mut out);
    let c = f.counters();
    assert_eq!(c.created, MAX_TEXT_LOADS_PER_PASS as u64 + 2);
    assert_eq!(c.text_loads_deferred, 3);
    // The non-table reference is not looked up and gives no name.
    assert_eq!(
        stub.resolves.load(Ordering::Relaxed) - resolves,
        MAX_TEXT_LOADS_PER_PASS
    );
    assert_eq!(c.unresolved_names, 1);
    assert!(f.has_deferred());
    // The next pass renders the rest.
    let mut out = FeedOutput::default();
    f.begin_pass();
    f.retry_deferred(&d, &mut out);
    assert_eq!(f.counters().created, n as u64 + 2);
    assert!(!f.has_deferred());
    assert_eq!(
        count(
            &out.events,
            |e| matches!(e, Event::EntityCreated { name, .. } if name.starts_with("Ref "))
        ),
        3
    );
}

#[test]
fn one_message_makes_one_appearance_change_and_one_move_per_entity() {
    let (d, _) = data(Stub::default());
    let mut f = feed();
    shown(&mut f, &d, NPC);
    let mut out = FeedOutput::default();
    for i in 0..50u16 {
        let scale = 1.0 + f32::from(i) / 100.0;
        f.visual(
            NPC,
            &EntityAppearanceDesc {
                scale: Some([scale; 3]),
                ..EntityAppearanceDesc::default()
            },
            &mut out,
        );
        f.place(
            NPC,
            Some((at(f64::from(i), 0.0), 0.0)),
            [1.0, 0.0, 0.0],
            &mut out,
        );
    }
    assert!(out.events.is_empty(), "nothing before the flush");
    f.flush(&d, &mut out);
    assert_eq!(out.events.len(), 2, "{:?}", out.events);
    assert!(
        matches!(&out.events[0], Event::AppearanceChanged { appearance, .. }
        if (appearance.scale - 1.49).abs() < 1e-6)
    );
    assert!(
        matches!(&out.events[1], Event::EntityMoved { transform, .. }
        if (transform.position.x - 49.0).abs() < 1e-6)
    );
}

#[test]
fn properties_the_feed_does_not_read_render_nothing_and_change_nothing() {
    let (d, stub) = data(Stub::default());
    let mut f = feed();
    shown(&mut f, &d, NPC);
    let resolves = stub.resolves.load(Ordering::Relaxed);
    let mut out = FeedOutput::default();
    for i in 0..100u16 {
        // A morale tick (`0x100012D2`, §5.2.4).
        f.properties(
            NPC,
            &[EncodedPropertyEntry::new(
                0x1000_12D2,
                EncodedPropertyValue::Float(f32::from(i)),
            )],
            &d,
            &mut out,
        );
        f.flush(&d, &mut out);
    }
    // The same flag again changes nothing either.
    f.properties(NPC, &[npc_entry(true)], &d, &mut out);
    f.flush(&d, &mut out);
    assert!(out.events.is_empty(), "{:?}", out.events);
    assert_eq!(stub.resolves.load(Ordering::Relaxed), resolves);
}

#[test]
fn name_and_kind_changes_after_the_creation_are_counted() {
    let (d, stub) = data(Stub::default());
    let mut f = feed();
    shown(&mut f, &d, NPC);
    let resolves = stub.resolves.load(Ordering::Relaxed);
    let mut out = FeedOutput::default();
    f.properties(
        NPC,
        &[name_entry(LocalizedTokenText::reference(5, 0x2500_0001))],
        &d,
        &mut out,
    );
    f.properties(NPC, &[npc_entry(false)], &d, &mut out);
    f.flush(&d, &mut out);
    let c = f.counters();
    assert_eq!(c.name_changes_unshown, 1);
    assert_eq!(c.kind_changes_unshown, 1);
    // A reference is not rendered for a name nobody is shown.
    assert_eq!(stub.resolves.load(Ordering::Relaxed), resolves);
}

#[test]
fn the_local_players_despawn_and_properties_are_counted_apart() {
    let (d, _) = data(Stub::default());
    let mut f = feed();
    let mut out = FeedOutput::default();
    f.despawn(PLAYER, &mut out);
    f.properties(PLAYER, &[npc_entry(false)], &d, &mut out);
    f.remove_property(PLAYER, PID_SEX, &mut out);
    f.visual(PLAYER, &EntityAppearanceDesc::default(), &mut out);
    f.despawn(DEFAULT_RECIPIENT, &mut out);
    f.despawn(NPC, &mut out);
    let c = f.counters();
    assert_eq!(c.player_despawns, 1);
    assert_eq!(c.player_property_events, 3);
    assert_eq!(c.no_target, 1);
    assert_eq!(c.despawn_unknown, 1);
    assert!(out.events.is_empty());
}

#[test]
fn a_path_moves_towards_its_destination_and_one_waypoint_keeps_the_heading() {
    let (d, _) = data(Stub::default());
    let mut f = feed();
    shown(&mut f, &d, NPC);
    let mut out = FeedOutput::default();
    // The last heading came from a position (0.5 at creation, now 1.25).
    f.place(NPC, Some((at(10.0, 20.0), 1.25)), [0.0; 3], &mut out);
    f.flush(&d, &mut out);
    let mut out = FeedOutput::default();
    f.path(NPC, &[at(13.0, 24.0)], &mut out);
    f.flush(&d, &mut out);
    let [
        Event::EntityMoved {
            transform,
            velocity,
            ..
        },
    ] = out.events.as_slice()
    else {
        panic!("{:?}", out.events);
    };
    assert!((transform.heading - 1.25).abs() < 1e-6);
    // 5 m away: 5 m/s at most 7, towards (3, 4) / 5.
    assert!((velocity.x - 3.0).abs() < 1e-4 && (velocity.y - 4.0).abs() < 1e-4);
    // Far: the speed is bounded.
    let mut out = FeedOutput::default();
    f.path(NPC, &[at(13.0, 24.0), at(113.0, 24.0)], &mut out);
    f.flush(&d, &mut out);
    let [
        Event::EntityMoved {
            transform,
            velocity,
            ..
        },
    ] = out.events.as_slice()
    else {
        panic!("{:?}", out.events);
    };
    assert!(transform.heading.abs() < 1e-6, "along the last step");
    let speed = velocity.x.hypot(velocity.y);
    assert!((f64::from(speed) - MAX_PATH_SPEED).abs() < 1e-4, "{speed}");
}

#[test]
fn a_path_moves_at_its_speed_and_undefined_types_are_dropped() {
    let (d, _) = data(Stub::default());
    let mut f = feed();
    shown(&mut f, &d, NPC);
    // `speed_param` is metres per second (§4.6.1): 2 m/s towards a point 5 m away.
    let mut out = FeedOutput::default();
    f.path_along(
        NPC,
        PATH_TYPE_SPLINE,
        Some(2.0),
        &[at(13.0, 24.0)],
        &mut out,
    );
    f.flush(&d, &mut out);
    let [Event::EntityMoved { velocity, .. }] = out.events.as_slice() else {
        panic!("{:?}", out.events);
    };
    assert!((velocity.x - 1.2).abs() < 1e-4 && (velocity.y - 1.6).abs() < 1e-4);
    // A huge speed is cut; a non-positive one falls back to the nominal speed.
    let mut out = FeedOutput::default();
    f.path_along(
        NPC,
        PATH_TYPE_LOOP,
        Some(1.0e9),
        &[at(13.0, 124.0)],
        &mut out,
    );
    f.flush(&d, &mut out);
    let [Event::EntityMoved { velocity, .. }] = out.events.as_slice() else {
        panic!("{:?}", out.events);
    };
    assert!((f64::from(velocity.x.hypot(velocity.y)) - MAX_SPEED_PARAM).abs() < 1e-2);
    // Types outside 1..=3 are dropped and counted.
    let mut out = FeedOutput::default();
    for t in [0u32, 4, u32::MAX] {
        f.path_along(NPC, t, Some(1.0), &[at(1.0, 2.0)], &mut out);
    }
    f.flush(&d, &mut out);
    assert!(out.events.is_empty());
    assert_eq!(f.counters().unknown_path_types, 3);
}

fn category_entry(code: u32) -> EncodedPropertyEntry {
    EncodedPropertyEntry::new(PID_OBJECT_CATEGORY_CODE, EncodedPropertyValue::Enum(code))
}

#[test]
fn entities_are_classified_by_their_category_code() {
    // §5.2.3: the inanimate flag first, then the category code, then the NPC flag for an
    // agent.
    let cases: [(Vec<EncodedPropertyEntry>, EntityKind, u64); 9] = [
        (vec![category_entry(0x1F)], EntityKind::Player, 0),
        (vec![category_entry(0x0002_004F)], EntityKind::NonPlayer, 0),
        (
            vec![category_entry(0x0001_004F), npc_entry(true)],
            EntityKind::NonPlayer,
            0,
        ),
        (
            vec![category_entry(0x0001_004F), npc_entry(false)],
            EntityKind::Creature,
            0,
        ),
        (vec![category_entry(0x0001_004F)], EntityKind::Creature, 0),
        (vec![category_entry(0x0003_0003)], EntityKind::Object, 0),
        (
            vec![
                category_entry(0x1F),
                EncodedPropertyEntry::new(
                    PID_IS_INANIMATE_PROP,
                    EncodedPropertyValue::Boolean(true),
                ),
            ],
            EntityKind::Object,
            0,
        ),
        // A code with low nibble 0xF the spec names no rule for, or none: the flags, counted.
        (
            vec![category_entry(0x0005_004F), npc_entry(true)],
            EntityKind::NonPlayer,
            1,
        ),
        (vec![], EntityKind::NonPlayer, 1),
    ];
    for (i, (entries, kind, unclassified)) in cases.into_iter().enumerate() {
        let (d, _) = data(Stub::default());
        let mut f = feed();
        let mut out = FeedOutput::default();
        let id = NPC + i as u64;
        create(&mut f, id, TEMPLATE, &mut out);
        f.properties(id, &entries, &d, &mut out);
        f.place(id, Some((at(1.0, 2.0), 0.0)), [0.0; 3], &mut out);
        f.flush(&d, &mut out);
        let [Event::EntityCreated { kind: k, .. }] = out.events.as_slice() else {
            panic!("case {i}: {:?}", out.events);
        };
        assert_eq!(*k, kind, "case {i}");
        assert_eq!(f.counters().unclassified, unclassified, "case {i}");
    }
}

#[test]
fn names_render_by_mode_with_their_arguments() {
    use crate::world::{LocalizedTokenTextArgPayload, TextSource};
    let (d, _) = data(Stub::default());
    let name_of = |text: LocalizedTokenText| {
        let mut f = feed();
        let mut out = FeedOutput::default();
        create(&mut f, NPC, TEMPLATE, &mut out);
        f.properties(NPC, &[name_entry(text), npc_entry(true)], &d, &mut out);
        f.place(NPC, Some((at(1.0, 2.0), 0.0)), [0.0; 3], &mut out);
        f.flush(&d, &mut out);
        let [Event::EntityCreated { name, .. }] = out.events.as_slice() else {
            panic!("{:?}", out.events);
        };
        (name.clone(), f.counters().name_arguments_dropped)
    };
    // A reference keeps its arguments for the resolver.
    let mut with_args = LocalizedTokenText::reference(7, 0x2500_0001);
    with_args.arguments.push(LocalizedTokenTextArg::new(
        0x11,
        6,
        LocalizedTokenTextArgPayload::int(3),
    ));
    assert_eq!(name_of(with_args.clone()), ("Ref 7 with 1".to_owned(), 0));
    // Mode 2: the second authoring string shown like a literal, never the table.
    let mut m2 = with_args.clone();
    if let LocalizedTokenTextContent::Reference { mode, .. } = &mut m2.content {
        *mode = 2;
    }
    m2.authoring_strings = Some(["tok".into(), "Watcher".into(), "note".into()]);
    assert_eq!(m2.source(), TextSource::AuthoringSource("Watcher"));
    assert_eq!(name_of(m2), ("Watcher".to_owned(), 0));
    // An argument table over the bound is dropped and counted; the name still renders.
    let mut big = LocalizedTokenText::reference(8, 0x2500_0001);
    for i in 0..200u32 {
        big.arguments.push(LocalizedTokenTextArg::new(
            i,
            2,
            LocalizedTokenTextArgPayload::Float {
                value: 1.0,
                decimal_places: 0,
            },
        ));
    }
    assert_eq!(name_of(big), ("Ref 8".to_owned(), 1));
    // The kept bytes stay within the bound.
    let mut f = feed();
    let mut out = FeedOutput::default();
    create(&mut f, NPC, TEMPLATE, &mut out);
    f.properties(NPC, &[name_entry(with_args)], &d, &mut out);
    assert_eq!(f.name_bytes(NPC), Some(5 + 2));
}

#[test]
fn a_reset_forgets_every_entity() {
    let (d, _) = data(Stub::default());
    let mut f = feed();
    shown(&mut f, &d, NPC);
    let mut out = FeedOutput::default();
    create(&mut f, 9, TEMPLATE, &mut out);
    f.reset(None);
    assert_eq!((f.tracked(), f.pending()), (0, 0));
    assert!(!f.has_deferred());
}
