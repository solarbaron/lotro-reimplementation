// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for `tessera_compat::play`: every play message decoder
//! (`official-messages-play.md` §4–§7) on hostile bytes, the script argument frame reader,
//! and the adapter and state machines fed with what decodes.
#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use tessera_compat::play::adapter::PlayAdapter;
use tessera_compat::play::chat::{ChatCategories, LiteralTexts, UiPropertyEvent};
use tessera_compat::play::inbound::{PLAY_OPCODES, decode_play_message};
use tessera_compat::play::logout::WORLD_TEARDOWN_OPCODES;
use tessera_compat::play::movement::{
    AnimLayerState, EntitySplinePath, MotionControllerUpdate, MovementInput, ServerSetPosition,
    TeleportSync, decode_signed_normalized,
};
use tessera_compat::play::rpc::{ArgKind, PlayRpc, ScriptRpcMessage, decode_args};
use tessera_compat::world::PropertyType;

fn fuzz_schema() -> BTreeMap<u32, PropertyType> {
    let mut map = BTreeMap::new();
    map.insert(0x1000_00B1, PropertyType::Enum);
    map.insert(0x1000_15D7, PropertyType::LocalizedText);
    map.insert(0x1000_0914, PropertyType::LocalizedText);
    map.insert(0x1000_215A, PropertyType::InstanceId);
    map.insert(0x1000_133D, PropertyType::Boolean);
    map.insert(0x0000_0001, PropertyType::String);
    map.insert(0x0000_0002, PropertyType::Int);
    map.insert(0x0000_0003, PropertyType::Struct);
    map.insert(0x0000_0004, PropertyType::Array);
    map.insert(0x0000_0005, PropertyType::Position);
    map.insert(0x0000_0006, PropertyType::Bitfield);
    map
}

const KINDS: [ArgKind; 8] = [
    ArgKind::I32,
    ArgKind::U32,
    ArgKind::I64,
    ArgKind::U64,
    ArgKind::F32,
    ArgKind::F64,
    ArgKind::Ref,
    ArgKind::OpaqueRef,
];

fuzz_target!(|data: &[u8]| {
    let schema = fuzz_schema();
    let player = 0x0102_0304_0506_0708u64;
    let mut adapter = PlayAdapter::new(ChatCategories::default());
    adapter.enter_world(player, Some("Self".to_string()));
    adapter.set_ignored(["Ignored".to_string()]);

    for &op in PLAY_OPCODES.iter().chain(WORLD_TEARDOWN_OPCODES.iter()) {
        if let Ok(Some(msg)) = decode_play_message(op, player, data, &schema) {
            let _ = adapter.handle(&msg, &LiteralTexts);
        }
    }

    // Direct decoders, and re-encoding of what decodes.
    if let Ok(m) = ScriptRpcMessage::from_bytes(data) {
        let _ = m.to_bytes();
        if let Ok(Some(rpc)) = PlayRpc::from_message(&m) {
            let _ = rpc.to_bytes();
        }
    }
    if let Ok(m) = ServerSetPosition::from_bytes(data) {
        let _ = m.to_bytes();
    }
    if let Ok(m) = EntitySplinePath::from_bytes(data) {
        let _ = m.to_bytes();
        let _ = m.waypoint_locations();
    }
    if let Ok(m) = MotionControllerUpdate::from_bytes(data) {
        let _ = m.to_bytes();
    }
    if let Ok(m) = AnimLayerState::from_bytes(data, &schema) {
        let _ = m.to_bytes();
    }
    if let Ok(m) = UiPropertyEvent::from_bytes(data, &schema) {
        let _ = m.to_bytes();
        let _ = m.chat_line(&LiteralTexts);
    }

    // An argument frame with a signature taken from the first bytes.
    if let Some((&sig_len, rest)) = data.split_first() {
        let n = usize::from(sig_len % 6);
        if rest.len() >= n {
            let (sig_bytes, frame) = rest.split_at(n);
            let sig: Vec<ArgKind> = sig_bytes
                .iter()
                .map(|b| KINDS[usize::from(b % 8)])
                .collect();
            let _ = decode_args(frame, &sig);
        }
    }

    // State machines driven by the bytes.
    let mut sync = TeleportSync::new(data.first().copied().unwrap_or(0));
    let mut now = 0.0f64;
    for chunk in data.chunks(4) {
        let b = |i: usize| chunk.get(i).copied().unwrap_or(0);
        match b(0) % 6 {
            0 => sync.begin_portal_space(b(1)),
            1 => {
                let _ = sync.on_local_position(b(1));
            }
            2 => {
                let _ = sync.try_exit_portal_space(b(1) & 1 != 0, b(1) & 2 != 0);
            }
            3 => adapter.begin_portal_space(b(1)),
            4 => {
                let _ = adapter.portal_loading(b(1) & 1 != 0, b(1) & 2 != 0);
            }
            _ => {
                now += f64::from(b(1)) / 100.0;
                let input = MovementInput {
                    move_state: b(2),
                    strafe: decode_signed_normalized(b(3) as i8),
                    forward: decode_signed_normalized(b(1) as i8),
                    turn: 0.0,
                };
                let _ = adapter.movement_step(input, now, None);
            }
        }
    }
});
