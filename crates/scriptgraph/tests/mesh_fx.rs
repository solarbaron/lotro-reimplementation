// SPDX-License-Identifier: GPL-3.0-or-later
//! The MeshFX sink hook (`script-table.md` §5.2, `script-runtime.md` §18): by default a
//! MeshFX node completes at once, as before the hook; a sink that attaches the node's
//! entity keeps the node running until it is stopped, and is told when it stops. Every DID
//! and key is invented.

mod common;

use common::*;
use tessera_formats::Did;
use tessera_formats::script_table::{MeshFxNode, NodeData, NodeKind};
use tessera_scriptgraph::{
    EffectSink, EnvironmentHandle, EnvironmentRequest, MeshFxHandle, MeshFxOutcome, MeshFxStart,
    RecordingSink, ScriptStart, SoundHandle, SoundStart, SoundStartOutcome,
};

const PROP: Did = Did(0x4700_0100);
const KEY: u32 = 0x1000_0010;

fn mesh_fx() -> tessera_formats::script_table::Node {
    node(
        NodeKind::MESH_FX,
        NodeData::MeshFx(Box::new(MeshFxNode {
            entity: PROP,
            attach_to_parent: true,
            allow_physics: false,
            fade_in_time: 0.0,
            life_time: 0.0,
            fade_out_time: 0.0,
            destroy_time: 0.0,
            holding_location: 3,
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

/// A sink that attaches every MeshFX node and records the starts and stops.
#[derive(Default)]
struct Attaching {
    started: Vec<(Did, u32, MeshFxHandle)>,
    stopped: Vec<MeshFxHandle>,
}

impl EffectSink for Attaching {
    fn start_sound(&mut self, _request: &SoundStart) -> SoundStartOutcome {
        SoundStartOutcome::Refused
    }
    fn stop_sound(&mut self, _handle: SoundHandle) {}
    fn set_sound_volume(&mut self, _handle: SoundHandle, _volume: f32) {}
    fn register_environment(&mut self, _request: &EnvironmentRequest) -> EnvironmentHandle {
        EnvironmentHandle(0)
    }
    fn unregister_environment(&mut self, _handle: EnvironmentHandle) {}
    fn start_mesh_fx(&mut self, request: &MeshFxStart<'_>) -> MeshFxOutcome {
        let h = MeshFxHandle(self.started.len() as u64 + 1);
        self.started
            .push((request.node.entity, request.node.holding_location, h));
        MeshFxOutcome::RunUntilStopped(h)
    }
    fn stop_mesh_fx(&mut self, handle: MeshFxHandle) {
        self.stopped.push(handle);
    }
}

/// Without the hook (the default sink) the node completes at once: existing behaviour.
#[test]
fn a_mesh_fx_node_completes_at_once_by_default() {
    let mut rt = single(vec![(KEY, vec![mesh_fx()])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Completed
    );
    assert!(!rt.is_running(E, KEY));
}

/// A sink that attaches the entity keeps the node (and its script) running until it is
/// stopped, then takes the attachment off.
#[test]
fn an_attached_mesh_fx_node_runs_until_stopped() {
    let mut rt = single(vec![(KEY, vec![mesh_fx()])]);
    let mut sink = Attaching::default();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    assert_eq!(
        rt.start_script(&mut sink, E, KEY).unwrap(),
        ScriptStart::Running
    );
    assert_eq!(sink.started.len(), 1);
    let (entity, location, handle) = sink.started[0];
    assert_eq!((entity, location), (PROP, 3));
    for _ in 0..120 {
        rt.advance(&mut sink, 1.0 / 60.0);
    }
    assert!(rt.is_running(E, KEY), "the attachment holds the script");
    assert!(sink.stopped.is_empty());
    assert_eq!(rt.stop_script(&mut sink, E, KEY).unwrap(), 1);
    assert_eq!(sink.stopped, vec![handle], "the attachment is taken off");
    assert!(!rt.is_running(E, KEY));
}
