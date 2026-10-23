// SPDX-License-Identifier: GPL-3.0-or-later
//! The screen figure's live scripts on a synthetic graph (every DID, key and PID invented):
//! a start-up idle, a `Chargen` Switch on the stage key with a stance, a prop and a looping
//! Sequence of Random gestures on stage 1, another stance on stage 6 and a transition port
//! from 1 to 6.

use std::collections::HashMap;

use tessera_formats::property::PropertyEntry;
use tessera_formats::script_table::{
    BaseBlock, MeshFxNode, Node, NodeData, NodeKind, Port, PortPayload, PortType, SWITCH_EQUAL,
    SwitchCase, SwitchNode, SwitchTransition,
};
use tessera_formats::{PropertyType, Script};
use tessera_havok::Transform;
use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, ClipRecordSpec, InterleavedClipSpec,
};
use tessera_scriptgraph::SyntheticCatalog;

use super::*;

const STAGE: u32 = 0x2001;
const STARTUP: u32 = 5;
const CHARGEN: u32 = 0x30;
const TABLE: Did = Did(0x0700_0001);
const IDLE: u32 = 0x0500_0001;
const STANCE_1: u32 = 0x0500_0002;
const GESTURE_A: u32 = 0x0500_0003;
const GESTURE_B: u32 = 0x0500_0004;
const STANCE_6: u32 = 0x0500_0005;
const TRANSITION: u32 = 0x0500_0006;
const PROP: Did = Did(0x4700_0100);
const LOCATION: u32 = 7;
/// Every synthetic animation node's `RampIn`, seconds.
const RAMP_IN: f32 = 0.3;

fn base(id: u32) -> BaseBlock {
    BaseBlock {
        driven_hint: 1,
        driven: Vec::new(),
        scriptlet_driven_hint: 1,
        scriptlet_driven: Vec::new(),
        scriptlet_params: Vec::new(),
        use_parent_intensity: true,
        intensity_multiplier: 1.0,
        intensity_variance: 0.0,
        probability: 1.0,
        node_id: id,
    }
}

fn node(id: u32, kind: NodeKind, data: NodeData, ports: Vec<Port>) -> Node {
    Node {
        kind,
        data,
        base: base(id),
        particle_flag: None,
        ports,
        targets: Vec::new(),
    }
}

fn port(port_type: PortType, payload: PortPayload, children: Vec<Node>) -> Port {
    Port {
        port_type,
        payload,
        children,
    }
}

fn anim(id: u32, clip: u32, layer: u32, looping: bool) -> Node {
    node(
        id,
        NodeKind::ANIMATION,
        NodeData::Animation(Box::new(AnimationNode {
            asset: Did(clip),
            loop_forever: looping,
            loop_count: 1,
            layer,
            end_time: 1.0,
            speed: 1.0,
            speed_multiplier: 1.0,
            ramp_in: RAMP_IN,
            ramp_out: 0.1,
            ..AnimationNode::default()
        })),
        Vec::new(),
    )
}

fn prop(id: u32) -> Node {
    node(
        id,
        NodeKind::MESH_FX,
        NodeData::MeshFx(Box::new(MeshFxNode {
            entity: PROP,
            attach_to_parent: true,
            allow_physics: false,
            fade_in_time: 0.0,
            life_time: 0.0,
            fade_out_time: 0.0,
            destroy_time: 0.0,
            holding_location: LOCATION,
            bone_offset: [0.0; 3],
            offset_rotation_yaw: 0.0,
            offset_rotation_pitch: 0.0,
            offset_rotation_roll: 0.0,
            remove_rotation: false,
            bone_type: 0,
            behavior: 0,
            persistent: true,
            on_water: false,
            use_parent_scale: true,
            use_particle_scale: false,
            ignore_child_scale: false,
            scale: [1.0; 3],
            appearances: Vec::new(),
            mesh_iid_target_property: 0,
        })),
        Vec::new(),
    )
}

fn case(value: i32, children: Vec<Node>) -> Port {
    port(
        PortType::SWITCH_VALUE,
        PortPayload::SwitchCase(Box::new(SwitchCase {
            values: vec![Some(PropertyEntry {
                pid: 0,
                value: PropertyValue::Int(value),
            })],
            ints: Vec::new(),
            operator: SWITCH_EQUAL,
            transition_time: 0.0,
            second_time: 0.0,
        })),
        children,
    )
}

fn transition(from: i32, to: i32, children: Vec<Node>) -> Port {
    port(
        PortType::SWITCH_TRANSITION,
        PortPayload::SwitchTransition(Box::new(SwitchTransition {
            flags: [false, false, false],
            from: Some(PropertyEntry {
                pid: 0,
                value: PropertyValue::Int(from),
            }),
            to: Some(PropertyEntry {
                pid: 0,
                value: PropertyValue::Int(to),
            }),
            floats: [0.0, 0.0, -1.0, 0.0],
        })),
        children,
    )
}

/// Gestures one after the other, each chosen at random.
fn gestures() -> Node {
    let random = node(
        30,
        NodeKind::RANDOM,
        NodeData::Random {
            no_repeats: false,
            autonomous: false,
        },
        vec![
            port(
                PortType::DEFAULT,
                PortPayload::U32(1),
                vec![anim(31, GESTURE_A, 90, false)],
            ),
            port(
                PortType::DEFAULT,
                PortPayload::U32(1),
                vec![anim(32, GESTURE_B, 90, false)],
            ),
        ],
    );
    node(
        29,
        NodeKind::SEQUENCE,
        NodeData::Sequence {
            loop_forever: true,
            loop_count: 0,
        },
        vec![port(PortType::DEFAULT, PortPayload::U32(1), vec![random])],
    )
}

fn table() -> ScriptTable {
    let switch = node(
        20,
        NodeKind::SWITCH,
        NodeData::Switch(SwitchNode {
            property: STAGE,
            persistent: true,
            autonomous: true,
            synchronized: false,
        }),
        vec![
            case(1, vec![anim(21, STANCE_1, 80, true), prop(22), gestures()]),
            case(6, vec![anim(23, STANCE_6, 80, true)]),
            transition(1, 6, vec![anim(24, TRANSITION, 85, false)]),
        ],
    );
    ScriptTable {
        did: TABLE,
        kind: 1,
        includes: Vec::new(),
        scripts_size: Default::default(),
        scripts: vec![
            Script {
                key: STARTUP,
                roots: vec![anim(10, IDLE, 10, true)],
            },
            Script {
                key: CHARGEN,
                roots: vec![switch],
            },
        ],
        properties_size: Default::default(),
        script_properties: Vec::new(),
        dids_size: Default::default(),
        script_dids: Vec::new(),
    }
}

/// Synthetic clips of one bone (shared, so a test can "decode" one later).
#[derive(Clone)]
struct Clips(Arc<std::sync::Mutex<HashMap<u32, Arc<BoundClip>>>>);

impl Clips {
    fn new() -> Self {
        let skel =
            Arc::new(tessera_anim::Skeleton::from_parts(&[-1], &[Transform::IDENTITY]).unwrap());
        let clip = |seconds: f32| {
            let spec = InterleavedClipSpec {
                duration: seconds,
                tracks: 1,
                transforms: vec![Transform::IDENTITY; 2],
            };
            let rec = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(spec)))
                .to_bytes();
            let c = tessera_havok::Clip::parse(&rec).unwrap();
            Arc::new(BoundClip::bind(Arc::new(c), Arc::clone(&skel)).unwrap())
        };
        Self(Arc::new(std::sync::Mutex::new(
            [
                (IDLE, 2.0),
                (STANCE_1, 2.0),
                (GESTURE_A, 1.0),
                (GESTURE_B, 1.5),
                (STANCE_6, 2.0),
                (TRANSITION, 0.5),
            ]
            .into_iter()
            .map(|(d, s)| (d, clip(s)))
            .collect(),
        )))
    }
}

impl LiveClips for Clips {
    fn bind(&self, did: u32) -> Option<Arc<BoundClip>> {
        self.0.lock().unwrap().get(&did).cloned()
    }
}

fn package(stage: i32) -> Arc<LivePackage> {
    let names: HashMap<String, u32> = HashMap::new();
    let mut vocab = RuntimeVocabulary::from_names(&names);
    vocab.startup_key = Some(STARTUP);
    let mut catalog = SyntheticCatalog::default();
    catalog.types.insert(STAGE, PropertyType::Int);
    let mut init = EntityScripts {
        additional_tables: vec![TABLE],
        startup_scripts: vec![CHARGEN],
        ..EntityScripts::default()
    };
    init.properties.set(STAGE, PropertyValue::Int(stage));
    Arc::new(LivePackage {
        records: Arc::new(LiveRecords::from_parts([(TABLE, table())], [])),
        vocab,
        catalog: Arc::new(catalog),
        init,
        seed: 7,
        stage: Some((STAGE, KeyType::Int)),
        action: None,
        built_keys: (u32::try_from(stage).ok(), None),
        props: vec![ScreenProp {
            entity: PROP,
            location: LOCATION,
            bone_type: 0,
            offset: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
        }],
        stage_variants: vec![(1, 0), (6, 1)],
        held_clips: Vec::new(),
        props_reached: 1,
        prop_notes: Vec::new(),
        mesh_fx_pids: MeshFxFieldPids::default(),
    })
}

fn start(stage: i32) -> LiveFigure {
    LiveFigure::start(
        package(stage),
        Box::new(Clips::new()),
        tessera_net::EntityId(9),
    )
    .expect("the figure's scripts start")
}

/// (clip, handle) of each Play command; handles of each Stop command.
fn split(commands: &[ClipCommand]) -> (Vec<(u32, u64)>, Vec<u64>) {
    let mut plays = Vec::new();
    let mut stops = Vec::new();
    for c in commands {
        match c {
            ClipCommand::Play { request, .. } => {
                plays.push((request.node.asset.0, request.handle));
            }
            ClipCommand::Stop { handle } => stops.push(*handle),
            ClipCommand::Update { .. } => {}
        }
    }
    (plays, stops)
}

/// At start the idle of `Startup` and the stage-1 branch of `Chargen` run: the stance
/// loops, the prop is attached, the first gesture plays.
#[test]
fn the_start_up_scripts_run_on_the_figure() {
    let mut f = start(1);
    let (plays, stops) = split(&f.take_clip_commands());
    let clips: Vec<u32> = plays.iter().map(|p| p.0).collect();
    assert!(
        clips.contains(&IDLE) && clips.contains(&STANCE_1),
        "{clips:x?}"
    );
    assert!(
        clips.contains(&GESTURE_A) || clips.contains(&GESTURE_B),
        "the first gesture: {clips:x?}"
    );
    assert!(stops.is_empty());
    assert!(f.prop_shown(PROP_KEY_BASE), "the class prop is attached");
    assert_eq!(f.variant(), 0);
    // The status the CSI9 stance check reads: the looping idle and stance, and the
    // gesture as a one-shot.
    let running = f.running_clips();
    assert!(running.contains(&(10, false, false)), "{running:?}");
    assert!(running.contains(&(80, false, false)), "{running:?}");
    assert!(running.contains(&(90, true, false)), "{running:?}");
    assert!(f.running_scripts().contains(&CHARGEN));
}

/// `character-screens.md` §2.1.4: the Random gestures play over time, one after the other
/// (the Sequence's later passes run).
#[test]
fn random_gestures_play_over_time() {
    let mut f = start(1);
    let mut gestures = Vec::new();
    for _ in 0..200 {
        let (plays, _) = split(&f.take_clip_commands());
        gestures.extend(
            plays
                .into_iter()
                .filter(|p| p.0 == GESTURE_A || p.0 == GESTURE_B)
                .map(|p| p.0),
        );
        f.advance(0.1);
    }
    // 20 s of gestures of 1 s and 1.5 s.
    assert!(gestures.len() >= 10, "gestures played: {}", gestures.len());
    assert!(
        gestures.contains(&GESTURE_A) && gestures.contains(&GESTURE_B),
        "both gestures are chosen at random: {gestures:x?}"
    );
}

/// A change of the stage key goes to the running scripts: the stage-1 branch stops (its
/// stance, its gesture, its prop), the transition port runs, the stage-6 stance starts, and
/// the idle keeps playing (it is not stopped and not started again).
#[test]
fn a_stage_change_switches_the_running_scripts() {
    let mut f = start(1);
    let (plays, _) = split(&f.take_clip_commands());
    let idle = plays.iter().find(|p| p.0 == IDLE).expect("idle").1;
    let stance = plays.iter().find(|p| p.0 == STANCE_1).expect("stance").1;
    f.advance(0.5);
    let _ = f.take_clip_commands();
    f.set_keys(Some(6), None);
    let (plays, stops) = split(&f.take_clip_commands());
    let clips: Vec<u32> = plays.iter().map(|p| p.0).collect();
    assert!(stops.contains(&stance), "the stage-1 stance stops");
    assert!(!stops.contains(&idle), "the idle keeps running");
    assert!(!clips.contains(&IDLE), "the idle does not restart");
    assert!(
        clips.contains(&TRANSITION),
        "the transition port runs: {clips:x?}"
    );
    assert!(
        clips.contains(&STANCE_6),
        "the stage-6 stance starts: {clips:x?}"
    );
    assert!(!f.prop_shown(PROP_KEY_BASE), "the prop is taken off");
    let running = f.running_clips();
    assert_eq!(
        running.iter().filter(|c| c.0 == 80).count(),
        1,
        "one stance, the stage-6 one: {running:?}"
    );
    assert!(
        !running.iter().any(|c| c.0 == 90),
        "the gestures stopped: {running:?}"
    );
    assert_eq!(f.variant(), 1, "the stage-6 pieces");
    assert_eq!(f.keys(), (Some(6), None));
    assert_eq!(f.stats().key_changes, 1);
    // The same key again changes nothing.
    f.set_keys(Some(6), None);
    assert_eq!(f.stats().key_changes, 1);
    assert!(f.take_clip_commands().is_empty());
}

/// Held proportion clips are not played a second time; a looping clip that is not decoded
/// yet is not read on the frame thread: its node runs and the clip starts once it is
/// decoded (a worker's job in the client).
#[test]
fn held_and_pending_clips() {
    let mut p = Arc::try_unwrap(package(1)).unwrap_or_else(|_| unreachable!());
    p.held_clips = vec![IDLE];
    let clips = Clips::new();
    let stance = clips
        .0
        .lock()
        .unwrap()
        .remove(&STANCE_1)
        .expect("stance clip");
    let mut f = LiveFigure::start(
        Arc::new(p),
        Box::new(clips.clone()),
        tessera_net::EntityId(9),
    )
    .expect("start");
    let (plays, _) = split(&f.take_clip_commands());
    let played: Vec<u32> = plays.iter().map(|p| p.0).collect();
    assert!(
        !played.contains(&IDLE) && !played.contains(&STANCE_1),
        "{played:x?}"
    );
    assert_eq!(f.stats().held_skipped, 1);
    assert_eq!(f.stats().clips_deferred, 1);
    assert_eq!(f.stats().clips_missing, 0);
    f.advance(0.1);
    assert!(
        !split(&f.take_clip_commands())
            .0
            .iter()
            .any(|p| p.0 == STANCE_1),
        "still being decoded"
    );
    // Decoded: the stance starts on the next advance, and the status shows it.
    clips.0.lock().unwrap().insert(STANCE_1, stance);
    f.advance(0.1);
    let (plays, _) = split(&f.take_clip_commands());
    assert!(plays.iter().any(|p| p.0 == STANCE_1), "{plays:x?}");
    assert!(f.running_clips().contains(&(80, false, false)));
}

/// Driven values reach the MeshFX node's fields (`script-table.md` §4.2): a driven
/// `Entity` picks the prop the figure was built with.
#[test]
fn driven_mesh_fx_fields_are_applied() {
    let NodeData::MeshFx(stored) = prop(1).data else {
        unreachable!()
    };
    let pids = MeshFxFieldPids {
        entity: Some(0x10),
        holding_location: Some(0x11),
        scale: Some(0x12),
        yaw: Some(0x13),
        ..MeshFxFieldPids::default()
    };
    let driven = [
        (0x10, PropertyValue::DidRef(Did(0x4700_0200))),
        (0x11, PropertyValue::Enum(9)),
        (0x12, PropertyValue::Float(2.0)),
        (0x13, PropertyValue::Float(f32::NAN)),
        (0x99, PropertyValue::Int(1)),
    ];
    let n = effective_mesh_fx(&stored, &driven, &pids);
    assert_eq!(n.entity, Did(0x4700_0200));
    assert_eq!(n.holding_location, 9);
    assert_eq!(n.scale, [2.0; 3]);
    assert_eq!(
        n.offset_rotation_yaw, 0.0,
        "a non-finite value is not taken"
    );
    assert_eq!(effective_mesh_fx(&stored, &[], &pids), *stored);
}

#[test]
fn key_types_write_their_values() {
    assert_eq!(KeyType::Enum.value(6), Some(PropertyValue::Enum(6)));
    assert_eq!(KeyType::Int.value(6), Some(PropertyValue::Int(6)));
    assert_eq!(KeyType::Int.value(u32::MAX), None);
    assert_eq!(KeyType::Int64.value(6), Some(PropertyValue::Int64(6)));
}

/// `script-table.md` §6.10 step 1: the clips the start-up scripts start appear at full
/// weight at once (`RampIn` 0); clips started later (the next gestures, a transition port)
/// keep their `RampIn`.
#[test]
fn start_up_clips_take_no_ramp_in() {
    let mut f = start(1);
    let startup: Vec<(u32, f32)> = f
        .take_clip_commands()
        .iter()
        .filter_map(|c| match c {
            ClipCommand::Play { request, .. } => Some((request.node.asset.0, request.node.ramp_in)),
            _ => None,
        })
        .collect();
    assert!(startup.iter().any(|c| c.0 == IDLE), "{startup:x?}");
    assert!(
        startup.iter().all(|c| c.1 == 0.0),
        "start-up clips at full weight at once: {startup:?}"
    );
    let mut later = Vec::new();
    for _ in 0..40 {
        f.advance(0.1);
        for c in f.take_clip_commands() {
            if let ClipCommand::Play { request, .. } = c {
                later.push(request.node.ramp_in);
            }
        }
    }
    f.set_keys(Some(6), None);
    for c in f.take_clip_commands() {
        if let ClipCommand::Play { request, .. } = c {
            later.push(request.node.ramp_in);
        }
    }
    assert!(
        !later.is_empty(),
        "later gestures and the transition played"
    );
    assert!(
        later.iter().all(|r| *r == RAMP_IN),
        "later clips keep their RampIn: {later:?}"
    );
}

/// Two MeshFX nodes that attach the same entity at the same holding location with another
/// bone or offset are two props; each node finds its own.
#[test]
fn props_differ_by_every_field() {
    let NodeData::MeshFx(a) = prop(1).data else {
        unreachable!()
    };
    let mut b = (*a).clone();
    b.bone_type = 3;
    let mut c = (*a).clone();
    c.bone_offset = [0.0, 0.1, 0.0];
    let mut list = PropList::default();
    for n in [&*a, &b, &c, &*a] {
        list.add(n);
    }
    assert_eq!(list.props.len(), 3, "{:?}", list.props);
    assert_eq!(list.reached, 3);
    assert_eq!(ScreenProp::find(&list.props, &a), Some(0));
    assert_eq!(ScreenProp::find(&list.props, &b), Some(1));
    assert_eq!(ScreenProp::find(&list.props, &c), Some(2));
    // A node whose offset matches no prop exactly falls back to its entity, location and
    // bone.
    let mut d = b.clone();
    d.scale = [1.5; 3];
    assert_eq!(ScreenProp::find(&list.props, &d), Some(1));
    let mut e = (*a).clone();
    e.holding_location = LOCATION + 1;
    assert_eq!(ScreenProp::find(&list.props, &e), None);
}

/// A deferred looping clip whose clip has been decoded stays deferred while the command
/// list is full, and starts once there is room.
#[test]
fn a_deferred_clip_waits_for_room_in_the_command_list() {
    let clips = Clips::new();
    let stance = clips
        .0
        .lock()
        .unwrap()
        .remove(&STANCE_1)
        .expect("stance clip");
    let mut f = LiveFigure::start(
        package(1),
        Box::new(clips.clone()),
        tessera_net::EntityId(9),
    )
    .expect("start");
    assert_eq!(f.stats().clips_deferred, 1);
    let _ = f.take_clip_commands();
    clips.0.lock().unwrap().insert(STANCE_1, stance);
    let filler = ClipCommand::Update {
        handle: 0,
        node: AnimationNode::default(),
    };
    f.commands = vec![filler; MAX_LIVE_COMMANDS];
    f.advance(0.1);
    assert!(
        !split(&f.take_clip_commands())
            .0
            .iter()
            .any(|p| p.0 == STANCE_1),
        "no room: not played"
    );
    f.advance(0.1);
    let (plays, _) = split(&f.take_clip_commands());
    assert!(plays.iter().any(|p| p.0 == STANCE_1), "{plays:x?}");
    assert!(f.running_clips().contains(&(80, false, false)));
}
