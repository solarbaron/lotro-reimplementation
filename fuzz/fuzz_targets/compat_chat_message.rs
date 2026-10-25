// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for `tessera_compat::chat_session`: every chat-server event and request
//! decoder (`official-messages-play.md` §6.6) on hostile bytes, the wire structures,
//! the endpoint and ticket readers, the state machine, and the client fed raw datagrams
//! over the in-memory transport (no socket).
#![no_main]

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddrV4};

use libfuzzer_sys::fuzz_target;
use tessera_compat::chat_session::events::{
    CHAT_OPCODE_ENTITY_OFFLINE, CHAT_OPCODE_FIRST, CHAT_OPCODE_LAST, ChatEvent, ChatRequest,
};
use tessera_compat::chat_session::state::ChatSessionState;
use tessera_compat::chat_session::wire::{
    ChatMessagePayload, ChatRoomDescriptor, DescriptorSet, U64ToU32Table, read_descriptor_list,
    read_u64_list,
};
use tessera_compat::chat_session::{
    ChatClient, ChatConfig, chat_auth_header, decode_base64, effective_endpoint, parse_endpoint,
};
use tessera_compat::session::InMemoryUdpTransport;
use tessera_compat::world::{PropertyType, WireReader};

fn fuzz_schema() -> BTreeMap<u32, PropertyType> {
    let mut map = BTreeMap::new();
    map.insert(0x0000_0001, PropertyType::String);
    map.insert(0x0000_0002, PropertyType::Int);
    map.insert(0x0000_0003, PropertyType::Struct);
    map.insert(0x0000_0004, PropertyType::LocalizedText);
    map
}

fuzz_target!(|data: &[u8]| {
    let schema = fuzz_schema();
    for op in (CHAT_OPCODE_FIRST..=CHAT_OPCODE_LAST).chain([CHAT_OPCODE_ENTITY_OFFLINE, 0x30]) {
        if let Ok(ev) = ChatEvent::decode(op, data) {
            let _ = ev.encode();
        }
        if let Ok(req) = ChatRequest::decode(op, data) {
            let _ = req.encode();
        }
    }
    if let Ok(p) = ChatMessagePayload::from_bytes(data, &schema) {
        let _ = p.to_bytes();
    }
    let _ = ChatRoomDescriptor::decode(&mut WireReader::new(data));
    let _ = read_descriptor_list(&mut WireReader::new(data));
    let _ = read_u64_list(&mut WireReader::new(data));
    let _ = DescriptorSet::decode(&mut WireReader::new(data));
    let _ = U64ToU32Table::decode(&mut WireReader::new(data));

    if let Ok(s) = std::str::from_utf8(data) {
        let _ = decode_base64(s);
        let _ = parse_endpoint(s);
        let mid = (0..=s.len() / 2)
            .rev()
            .find(|&i| s.is_char_boundary(i))
            .unwrap_or(0);
        let (a, b) = s.split_at(mid);
        let _ = effective_endpoint(a, data.len().is_multiple_of(2), b);
        let _ = chat_auth_header(a, Some(b), 0);
    }

    // The state machine, driven by the bytes.
    let mut st = ChatSessionState::new(0.0);
    let mut now = 0.0f64;
    for pair in data.chunks(2) {
        let b0 = pair.first().copied().unwrap_or(0);
        let b1 = pair.get(1).copied().unwrap_or(0);
        now += f64::from(b1);
        match b0 % 6 {
            0 => {
                let _ = st.tick(now);
            }
            1 => st.on_transport_connected(now),
            2 => st.on_event(now),
            3 => {
                let _ = st.on_login_status(i32::from(b1 as i8), now);
            }
            4 => st.on_transport_lost(now),
            _ => st.close(),
        }
    }

    // The client: the input as one raw datagram from the server, around connection.
    let client_addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 9100);
    let server = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 9), 9200);
    let config = ChatConfig::new(server, "fuzz", Some("AAEC".to_string()), 0);
    if let Ok(mut client) =
        ChatClient::with_transport(InMemoryUdpTransport::new(client_addr), config, None, 0.0)
    {
        let _ = client.poll(1.0, &schema);
        client.transport_mut().push_inbound(data.to_vec(), server);
        let _ = client.poll(1.1, &schema);
        let _ = client.poll(30.0, &schema);
    }
});
