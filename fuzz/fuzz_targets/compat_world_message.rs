// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use tessera_compat::world::{
    AccountBooted, ActionErrorResponse, ActionStatusResponse, CharacterCreateError,
    CharacterCreateSuccess, CharacterDeletedResult, CharacterError, CharacterSet,
    CheckCharacterName, ClientActionCommand, ClientMoveCommand, CreateCharacter, DeleteCharacter,
    DeleteEntity, DisplayStringInfo, EntityActionEvent, EntityCreate, EntityInitialProperties,
    EntityPositionUpdate, GameTimeRecord, PID_ADVANCEMENT_LEVEL, PID_AGENT_SPECIES,
    PID_CHARGEN_CHARACTER_LEVEL, PID_NAME, PlayerSessionStart, PortalPlacement, PortalSpaceExit,
    PortalSpaceReady, PortalSpaceState, PropertyRemove, PropertyType, QueryMissingEntity,
    SelectCharacter, SendSteamId, ServerLastWords, SinglePropertyUpdate, SubEventBatch,
    VisualDescUpdate, WirePosition, WirePropertySet, WireReader, WireStringInfo, WorldName,
    decode_net_blob_payload, decode_vle3, decode_vle4, decode_vle16,
};

fn fuzz_schema() -> BTreeMap<u32, PropertyType> {
    let mut map = BTreeMap::new();
    map.insert(PID_NAME, PropertyType::StringInfo);
    map.insert(PID_ADVANCEMENT_LEVEL, PropertyType::Int);
    map.insert(PID_CHARGEN_CHARACTER_LEVEL, PropertyType::Int);
    map.insert(PID_AGENT_SPECIES, PropertyType::Enum);
    map.insert(0x0000_0001, PropertyType::String);
    map.insert(0x0000_0002, PropertyType::StringToken);
    map.insert(0x0000_0003, PropertyType::Waveform);
    map.insert(0x0000_0004, PropertyType::Timestamp);
    map.insert(0x0000_0005, PropertyType::TriState);
    map.insert(0x0000_0006, PropertyType::Vector3);
    map.insert(0x0000_0007, PropertyType::InstanceId);
    map.insert(0x0000_0008, PropertyType::Enum);
    map.insert(0x0000_0009, PropertyType::Float);
    map.insert(0x0000_000A, PropertyType::PropertyId);
    map.insert(0x0000_000B, PropertyType::Struct);
    map.insert(0x0000_000C, PropertyType::Array);
    map.insert(0x0000_000D, PropertyType::StringInfo);
    map.insert(0x0000_000E, PropertyType::Bitfield64);
    map.insert(0x0000_000F, PropertyType::Int);
    map.insert(0x0000_0010, PropertyType::Color);
    map.insert(0x0000_0011, PropertyType::Position);
    map.insert(0x0000_0012, PropertyType::Bitfield32);
    map.insert(0x0000_0013, PropertyType::Int64);
    map.insert(0x0000_0014, PropertyType::DidRef);
    map.insert(0x0000_0015, PropertyType::Boolean);
    map.insert(0x0000_0016, PropertyType::Bitfield);
    map
}

fuzz_target!(|data: &[u8]| {
    let schema = fuzz_schema();

    let _ = decode_vle3(data);
    let _ = decode_vle4(data);
    let _ = decode_vle16(data);
    let _ = WirePosition::decode_wire(&mut WireReader::new(data));
    let _ = WirePosition::decode_base(&mut WireReader::new(data));
    let _ = WirePosition::decode_stored(&mut WireReader::new(data));
    let _ = WireStringInfo::from_bytes(data);
    let _ = WireStringInfo::decode_stored(&mut WireReader::new(data));
    let _ = WirePropertySet::from_bytes(data, &schema);
    let _ = decode_net_blob_payload(data, 0);

    let _ = CharacterSet::from_bytes(data, &schema);
    let _ = CheckCharacterName::from_bytes(data);
    let _ = CreateCharacter::from_bytes(data, &schema);
    let _ = CharacterCreateSuccess::from_bytes(data, &schema);
    let _ = CharacterCreateError::from_bytes(data);
    let _ = DeleteCharacter::from_bytes(data, false);
    let _ = DeleteCharacter::from_bytes(data, true);
    let _ = CharacterDeletedResult::from_bytes(data);
    let _ = CharacterError::from_bytes(data);
    let _ = AccountBooted::from_bytes(data);
    let _ = SendSteamId::from_bytes(data);
    let _ = SelectCharacter::from_bytes(data, &schema);

    let _ = WorldName::from_bytes(data);
    let _ = DisplayStringInfo::from_bytes(data);
    let _ = ServerLastWords::from_bytes(data);
    let _ = PlayerSessionStart::from_bytes(data, &schema);
    let _ = PortalPlacement::from_bytes(data);
    let _ = PortalSpaceExit::from_bytes(data);
    let _ = PortalSpaceState::from_bytes(data);
    let _ = PortalSpaceReady::from_bytes(data);
    let _ = GameTimeRecord::from_bytes(data);

    let _ = SubEventBatch::from_bytes(data);
    let _ = EntityCreate::from_bytes(data);
    let _ = EntityInitialProperties::from_bytes(data, &schema);
    let _ = VisualDescUpdate::from_bytes(data, &schema);
    let _ = DeleteEntity::from_bytes(data);
    let _ = QueryMissingEntity::from_bytes(data);
    let _ = EntityPositionUpdate::from_bytes(data);
    let _ = ClientMoveCommand::from_bytes(data, &schema);
    let _ = ClientActionCommand::from_bytes(data, &schema);
    let _ = EntityActionEvent::from_bytes(data, &schema);
    let _ = ActionErrorResponse::from_bytes(data);
    let _ = ActionStatusResponse::from_bytes(data);
    let _ = SinglePropertyUpdate::from_bytes(data, &schema);
    let _ = PropertyRemove::from_bytes(data);
});
