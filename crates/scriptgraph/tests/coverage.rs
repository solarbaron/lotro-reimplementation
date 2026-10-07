// SPDX-License-Identifier: GPL-3.0-or-later
//! Paths with spec-stated numbers: lag catch-up (§10.4, §11.2), driven Timer fields (§11.4),
//! the Sound start of `sound.md` §9.2, sink outcomes, Animation (§18), ScalarBlend,
//! `UnbindOnDeath` and late replies (§12.4), When against a property (§7).

mod common;

use common::*;
use tessera_formats::property::PropertyEntry;
use tessera_formats::script_table::{
    AnimationNode, Node, NodeData, NodeKind, PortPayload, PortType, SoundNode, WhenTest, when_op,
};
use tessera_formats::{Did, PropertyValue};
use tessera_scriptgraph::{
    AnimationHandle, AnimationOutcome, EffectSink, EntityId, EnvironmentHandle, EnvironmentRequest,
    MemorySource, RecordingSink, Runtime, RuntimeVocabulary, ScriptStart, SoundHandle,
    SoundPositioner, SoundStart, SoundStartOutcome,
};

const KEY: u32 = 0x1000_0001;
const OTHER_KEY: u32 = 0x1000_0002;
const S1: u32 = 0x2A00_0001;
const S2: u32 = 0x2A00_0002;
const PLAYER: EntityId = EntityId(42);
/// Synthetic field PIDs for the vocabulary entries the shared builders leave unset.
const SPEED_FIELD: u32 = 0x1101;
const MUTE: u32 = 0x1102;
const IN_VIEWPORT: u32 = 0x1103;

fn runtime_with(scripts: Vec<(u32, Vec<Node>)>, v: RuntimeVocabulary) -> Runtime<'static> {
    let mut src = MemorySource::new();
    src.insert_table(Did(TABLE), table(2, Vec::new(), scripts));
    Runtime::new(src, catalog(), v).with_seed(3)
}

fn sound_with(f: impl FnOnce(&mut SoundNode)) -> Node {
    let mut n = sound(S1, false, 0.0, 0);
    if let NodeData::Sound(s) = &mut n.data {
        f(s);
    }
    n
}

fn starts(sink: &RecordingSink) -> Vec<SoundStart> {
    sink.sound_starts().map(|(_, s)| *s).collect()
}

fn offset_of(p: SoundPositioner) -> [f32; 3] {
    match p {
        SoundPositioner::FollowEntity { offset, .. }
        | SoundPositioner::Frozen { offset, .. }
        | SoundPositioner::Camera { offset, .. } => offset,
    }
}

fn entity_of(p: SoundPositioner) -> EntityId {
    match p {
        SoundPositioner::FollowEntity { entity, .. }
        | SoundPositioner::Frozen { entity, .. }
        | SoundPositioner::Camera { entity, .. } => entity,
    }
}

// ----- lag catch-up -----

#[test]
fn looping_timer_skips_whole_cycles_after_a_long_frame() {
    // A 1 s looping Timer with a port at 0, advanced by 10 s in one frame: the port fires
    // at 0 and at 10 only (cycles 1-9 are skipped, §11.2).
    let t = timer(true, 1.0, vec![(0.0, vec![sound(S1, true, 0.0, 0)])]);
    let mut rt = single(vec![(KEY, vec![t])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.advance(&mut sink, 10.0);
    assert_eq!(sink.sounds_started, 2);
    rt.advance(&mut sink, 1.0);
    assert_eq!(sink.sounds_started, 3, "the rhythm continues from 10");
}

#[test]
fn sequence_resumes_at_the_frame_time_after_a_long_frame() {
    // Passes at 0 and 0.1; the pass due at 0.2 lags the frame time (10) by more than
    // 1.111 s and resumes at 10 (§10.4).
    let s = sequence(true, 1, vec![vec![sound(S1, true, 0.0, 0)]]);
    let mut rt = single(vec![(KEY, vec![s])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(sink.sounds_started, 1);
    rt.advance(&mut sink, 10.0);
    assert_eq!(sink.sounds_started, 3);
    rt.advance(&mut sink, 0.05);
    assert_eq!(sink.sounds_started, 3, "next pass at 10.1");
    rt.advance(&mut sink, 0.06);
    assert_eq!(sink.sounds_started, 4);
}

// ----- driven Timer fields (§11.4) -----

#[test]
fn driven_timer_speed_reschedules_the_pending_port() {
    let mut v = vocab();
    v.timer.speed = Some(SPEED_FIELD);
    let t = driven(
        timer(false, 10.0, vec![(4.0, vec![sound(S1, true, 0.0, 0)])]),
        SPEED_FIELD,
        pid::VALUE,
    );
    let mut rt = runtime_with(vec![(KEY, vec![t])], v);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(pid::VALUE, PropertyValue::Float(1.0))]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    run_until(&mut rt, &mut sink, 1.0);
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Float(2.0))
        .unwrap();
    // From the cycle start (0) at speed 2 the port is due at 4 / 2 = 2.
    run_until(&mut rt, &mut sink, 1.95);
    assert_eq!(sink.sounds_started, 0);
    run_until(&mut rt, &mut sink, 2.05);
    assert_eq!(sink.sounds_started, 1);
    // The cycle (10 / 2 = 5 s) ends at 5 and the Timer completes.
    run_until(&mut rt, &mut sink, 5.05);
    assert!(!rt.is_running(E, KEY));
}

// ----- Sound start (sound.md §9.2) -----

#[test]
fn internal_objects_are_muted_and_set_parent_unmutes() {
    const HOLDER: EntityId = EntityId(9);
    let mut v = vocab();
    v.mute_when_internal = Some(MUTE);
    let mut rt = runtime_with(vec![(KEY, vec![sound(S1, true, 0.0, 0)])], v);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, HOLDER, init(Vec::new()))
        .unwrap();
    let mut i = init(vec![(MUTE, PropertyValue::Bool(true))]);
    i.parent = Some(HOLDER);
    rt.spawn_entity(&mut sink, E, i).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(starts(&sink).len(), 0, "step 2: muted inside its holder");
    rt.set_parent(E, None).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(starts(&sink).len(), 1, "a top-level entity is not muted");
    assert!(rt.set_parent(EntityId(77), None).is_err());
}

#[test]
fn run_in_viewport_redirects_to_the_viewport_entity() {
    const VIEW: EntityId = EntityId(5);
    const CHILD: EntityId = EntityId(6);
    let mut v = vocab();
    v.run_in_viewport = Some(IN_VIEWPORT);
    let mut rt = runtime_with(vec![(KEY, vec![sound(S1, true, 0.0, 0)])], v);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, VIEW, init(Vec::new())).unwrap();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![(IN_VIEWPORT, PropertyValue::Bool(true))]),
    )
    .unwrap();
    let mut c = init(Vec::new());
    c.parent = Some(E);
    rt.spawn_entity(&mut sink, CHILD, c).unwrap();
    // No viewport entity: the sound stays on its entity.
    rt.start_script(&mut sink, CHILD, KEY).unwrap();
    rt.set_viewport_entity(Some(VIEW));
    // Step 3: the topmost entity of the chain (E) runs in the viewport.
    rt.start_script(&mut sink, CHILD, KEY).unwrap();
    let s = starts(&sink);
    assert_eq!(s.len(), 2);
    assert_eq!(entity_of(s[0].positioner), CHILD);
    assert_eq!(entity_of(s[1].positioner), VIEW);
    assert_eq!(s[1].entity, CHILD, "the request names the running entity");
}

#[test]
fn random_offset_has_the_drawn_distance() {
    for (min, max, expect) in [(2.0f32, 2.0f32, 2.0f32), (5.0, 2.0, 2.0), (0.0, 0.0, 0.0)] {
        let n = sound_with(|s| {
            s.fire_and_forget = true;
            s.min_random_distance = min;
            s.max_random_distance = max;
        });
        let mut rt = single(vec![(KEY, vec![n])]);
        let mut sink = RecordingSink::new();
        rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
        rt.start_script(&mut sink, E, KEY).unwrap();
        let o = offset_of(starts(&sink)[0].positioner);
        let len = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt();
        assert!((len - expect).abs() < 1e-4, "{min} {max}: {len}");
    }
    // Between min and max.
    let n = sound_with(|s| {
        s.fire_and_forget = true;
        s.min_random_distance = 1.0;
        s.max_random_distance = 3.0;
        s.offset = [10.0, 0.0, 0.0];
    });
    let mut rt = single(vec![(KEY, vec![n])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    for _ in 0..20 {
        rt.start_script(&mut sink, E, KEY).unwrap();
    }
    for s in starts(&sink) {
        let o = offset_of(s.positioner);
        let d = [o[0] - 10.0, o[1], o[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!((1.0 - 1e-4..=3.0 + 1e-4).contains(&len), "{len}");
    }
}

#[test]
fn pitch_maps_to_a_rate_with_a_floor() {
    for (pitch, variance, lo, hi) in [
        (-1.0f32, 0.0f32, 0.5f32, 0.5f32),
        (1.0, 0.0, 1.5, 1.5),
        (0.0, 0.0, 1.0, 1.0),
        (-3.0, 0.0, 0.01, 0.01),
        (0.0, 0.5, 0.5, 1.5),
        (-2.0, 0.001, 0.01, 0.01),
    ] {
        let n = sound_with(|s| {
            s.fire_and_forget = true;
            s.pitch = pitch;
            s.pitch_variance = variance;
        });
        let mut rt = single(vec![(KEY, vec![n])]);
        let mut sink = RecordingSink::new();
        rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
        for _ in 0..10 {
            rt.start_script(&mut sink, E, KEY).unwrap();
        }
        for s in starts(&sink) {
            assert!(
                s.pitch >= lo - 1e-6 && s.pitch <= hi + 1e-6,
                "{pitch} {variance}: {}",
                s.pitch
            );
        }
    }
}

#[test]
fn inaudible_fire_and_forget_sounds_are_not_requested() {
    let roots = vec![
        with_intensity(sound(S1, true, 0.0, 0), 0.004, 0.0, true),
        with_intensity(sound(S2, true, 0.0, 0), 0.006, 0.0, true),
        with_intensity(sound(0x2A00_0003, false, 0.0, 0), 0.004, 0.0, true),
    ];
    let mut rt = single(vec![(KEY, roots)]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    let sounds: Vec<(u32, f32)> = starts(&sink)
        .iter()
        .map(|s| (s.sound.0, s.volume))
        .collect();
    assert_eq!(sounds, vec![(S2, 0.006), (0x2A00_0003, 0.004)]);
}

// ----- sink outcomes -----

/// A sink that answers sound requests and animations as configured.
#[derive(Debug)]
struct ProbeSink {
    inner: RecordingSink,
    silent: bool,
    animation: AnimationOutcome,
    animations_stopped: Vec<AnimationHandle>,
}

impl ProbeSink {
    fn new() -> Self {
        Self {
            inner: RecordingSink::new(),
            silent: false,
            animation: AnimationOutcome::CompleteAtOnce,
            animations_stopped: Vec::new(),
        }
    }
}

impl EffectSink for ProbeSink {
    fn start_sound(&mut self, request: &SoundStart) -> SoundStartOutcome {
        if self.silent {
            return SoundStartOutcome::AcceptedSilent;
        }
        self.inner.start_sound(request)
    }
    fn stop_sound(&mut self, handle: SoundHandle) {
        self.inner.stop_sound(handle);
    }
    fn set_sound_volume(&mut self, handle: SoundHandle, volume: f32) {
        self.inner.set_sound_volume(handle, volume);
    }
    fn register_environment(&mut self, request: &EnvironmentRequest) -> EnvironmentHandle {
        self.inner.register_environment(request)
    }
    fn unregister_environment(&mut self, handle: EnvironmentHandle) {
        self.inner.unregister_environment(handle);
    }
    fn start_animation(
        &mut self,
        _entity: EntityId,
        _node: &AnimationNode,
        _intensity: f32,
    ) -> AnimationOutcome {
        self.animation
    }
    fn stop_animation(&mut self, handle: AnimationHandle) {
        self.animations_stopped.push(handle);
    }
}

#[test]
fn accepted_silent_completes_a_tracked_sound_at_once() {
    let mut rt = single(vec![(KEY, vec![sound(S1, false, 0.0, 0)])]);
    let mut sink = ProbeSink::new();
    sink.silent = true;
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Completed
    );
    let s = rt.stats();
    assert_eq!((s.sounds_requested, s.sounds_accepted), (1, 1));
    assert_eq!(s.tracked_sounds, 0);
}

fn animation() -> Node {
    node(
        NodeKind::ANIMATION,
        NodeData::Animation(Box::default()),
        Vec::new(),
    )
}

#[test]
fn animation_runs_for_the_sink_given_time() {
    let mut rt = single(vec![(KEY, vec![animation()])]);
    let mut sink = ProbeSink::new();
    sink.animation = AnimationOutcome::RunFor(AnimationHandle(1), 2.0);
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Running
    );
    rt.advance(&mut sink, 1.9);
    assert!(rt.is_running(E, KEY));
    rt.advance(&mut sink, 0.2);
    assert!(!rt.is_running(E, KEY));
    assert!(sink.animations_stopped.is_empty(), "it ran out");
    // Stopped early, the sink is told.
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.stop_script(&mut sink, E, KEY).unwrap();
    assert_eq!(sink.animations_stopped, vec![AnimationHandle(1)]);
}

#[test]
fn looping_animation_runs_until_stopped() {
    let mut rt = single(vec![(KEY, vec![animation()])]);
    let mut sink = ProbeSink::new();
    sink.animation = AnimationOutcome::RunUntilStopped(AnimationHandle(7));
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    rt.advance(&mut sink, 100.0);
    assert!(rt.is_running(E, KEY));
    rt.despawn(&mut sink, E).unwrap();
    assert_eq!(sink.animations_stopped, vec![AnimationHandle(7)]);
    // The default sink completes animations at once.
    let mut rt = single(vec![(KEY, vec![animation()])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Completed
    );
}

// ----- ScalarBlend -----

#[test]
fn scalar_blend_starts_the_bracketing_anchors() {
    let blend = node(
        NodeKind::SCALAR_BLEND,
        NodeData::ScalarBlend {
            property: pid::VALUE,
            persistent: false,
        },
        vec![
            default_port(PortPayload::F32(1.0), vec![sound(S2, true, 0.0, 0)]),
            default_port(PortPayload::F32(0.0), vec![sound(S1, true, 0.0, 0)]),
        ],
    );
    for (value, expect) in [
        (0.5f32, vec![S1, S2]),
        (-1.0, vec![S1]),
        (0.0, vec![S1]),
        (1.0, vec![S2]),
        (7.0, vec![S2]),
    ] {
        let mut rt = single(vec![(KEY, vec![blend.clone()])]);
        let mut sink = RecordingSink::new();
        rt.spawn_entity(
            &mut sink,
            E,
            init(vec![(pid::VALUE, PropertyValue::Float(value))]),
        )
        .unwrap();
        rt.start_script(&mut sink, E, KEY).unwrap();
        let got: Vec<u32> = starts(&sink).iter().map(|s| s.sound.0).collect();
        assert_eq!(got, expect, "{value}");
    }
}

// ----- Run: UnbindOnDeath and late replies (§12.4 step 5) -----

fn unbind(mut n: Node) -> Node {
    if let NodeData::Run(r) = &mut n.data {
        r.unbind_on_death = true;
    }
    n
}

#[test]
fn unbind_on_death_keeps_the_started_run() {
    for (unbound, still_running) in [(false, false), (true, true)] {
        let r = run(OTHER_KEY, true, false);
        let r = if unbound { unbind(r) } else { r };
        let mut rt = single(vec![
            (KEY, vec![r]),
            (OTHER_KEY, vec![sound(S1, false, 0.0, 0)]),
        ]);
        let mut sink = RecordingSink::new();
        rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
        rt.start_script(&mut sink, E, KEY).unwrap();
        assert!(rt.is_running(E, OTHER_KEY));
        rt.stop_script(&mut sink, E, KEY).unwrap();
        assert_eq!(rt.is_running(E, OTHER_KEY), still_running, "{unbound}");
    }
}

#[test]
fn a_reply_after_the_run_stopped_is_answered_with_a_stop() {
    for (unbound, still_running) in [(false, false), (true, true)] {
        let r = run_on_player(OTHER_KEY, true);
        let r = if unbound { unbind(r) } else { r };
        let mut rt = single(vec![
            (KEY, vec![r]),
            (OTHER_KEY, vec![sound(S1, false, 0.0, 0)]),
        ]);
        let mut sink = RecordingSink::new();
        rt.spawn_entity(&mut sink, PLAYER, init(Vec::new()))
            .unwrap();
        rt.set_player(Some(PLAYER));
        rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
        rt.start_script(&mut sink, E, KEY).unwrap();
        // The Run stops before its request reaches the player.
        rt.stop_script(&mut sink, E, KEY).unwrap();
        rt.advance(&mut sink, 0.0);
        assert_eq!(rt.is_running(PLAYER, OTHER_KEY), still_running, "{unbound}");
        assert_eq!(playing(&sink, S1).len(), usize::from(still_running));
    }
}

// ----- When against another property (§7) -----

#[test]
fn when_compares_with_a_property_on_the_right() {
    let w = node(
        NodeKind::WHEN,
        NodeData::Switch(tessera_formats::script_table::SwitchNode {
            property: pid::VALUE,
            persistent: true,
            autonomous: true,
            synchronized: false,
        }),
        vec![
            port(
                PortType::WHEN_TEST,
                PortPayload::WhenTest(Box::new(WhenTest {
                    constant: Some(PropertyEntry {
                        pid: pid::VALUE,
                        value: PropertyValue::Int(99),
                    }),
                    rhs_property: pid::OTHER.cast_signed(),
                    operator: when_op::LESS_THAN,
                    tail: [0, 0],
                })),
                vec![sound(S1, false, 0.0, 0)],
            ),
            default_port(PortPayload::None, vec![sound(S2, false, 0.0, 0)]),
        ],
    );
    let mut rt = single(vec![(KEY, vec![w])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(
        &mut sink,
        E,
        init(vec![
            (pid::VALUE, PropertyValue::Int(3)),
            (pid::OTHER, PropertyValue::Int(5)),
        ]),
    )
    .unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    // 3 < OTHER (5), not 3 < 99 decided by the constant.
    assert_eq!(playing(&sink, S1).len(), 1);
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(6))
        .unwrap();
    assert!(playing(&sink, S1).is_empty());
    assert_eq!(playing(&sink, S2).len(), 1);
}
