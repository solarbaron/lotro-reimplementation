// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for `tessera_compat::world` (`docs/specs/official-messages-world.md`): wire
//! value encodings, encoded property tables, the event envelope and every world-session
//! message decoder on arbitrary hostile bytes. The property and portal-state schemas are
//! synthetic stand-ins for the run-time lookups.
#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use tessera_compat::world::{
    AccountEvictionNotice, ActionCancel, ActionChainSync, ActionPayload, ActionQueueEnable,
    ActionStart, ActionStateReset, ActionStepComplete, CharacterCreateAck, CharacterCreateReject,
    CharacterCreateRequest, CharacterDeleteRequest, CharacterDeleteStatus,
    CharacterEnterWorldRequest, CharacterListResponse, CharacterNameCheckRequest,
    CharacterOperationError, ClientActionCommand, ClientPhysicsSync, CompactDid, CompactPid,
    DEFAULT_PORTAL_STATE_SCHEMA_ID, DisplayLocalizedText, EncodedPropertyTable, EnterPortalSpace,
    EntityAnimLayerState, EntityAppearanceDesc, EntityCreate, EntityDespawn, EntityStringEvent,
    LandblockPropertyEvent, LocalizedTokenText, LocalizedTokenTextArgPayload, LoginFinished,
    MissingEntityQuery, PID_AGENT_SPECIES, PID_ENTITY_DISPLAY_NAME, PlatformAccountIdReport,
    PlayerSessionReady, PortalPositionSync, PortalReadyAck, PortalStateNotice, PortalStateSchema,
    PortalTransitionComplete, PositionUpdate, PredictedPropertyAck, PropertyRemoval,
    PropertyTableUpdate, PropertyType, ServerShutdownText, SinglePropertyUpdate, SubEventBatch,
    VisualDescriptionUpdate, WireBitfield, WirePosition, WireReader, WireWriter,
    WorldCalendarConfig, WorldNameAnnouncement, active_target, decode_event_envelope, decode_vle3,
    decode_vle4, decode_vle16,
};

fn fuzz_schema() -> BTreeMap<u32, PropertyType> {
    let mut map = BTreeMap::new();
    map.insert(PID_ENTITY_DISPLAY_NAME, PropertyType::LocalizedTokenText);
    map.insert(PID_AGENT_SPECIES, PropertyType::Enum);
    for code in 1u32..=22 {
        if let Ok(kind) = PropertyType::from_code(code) {
            map.insert(code, kind);
        }
    }
    map
}

fn portal_schemas() -> BTreeMap<u32, PortalStateSchema> {
    let mut map = BTreeMap::new();
    map.insert(
        DEFAULT_PORTAL_STATE_SCHEMA_ID,
        PortalStateSchema {
            bool_pids: (1..=8).collect(),
            enum_pids: (9..=14).collect(),
        },
    );
    map.insert(0x0000_0100, PortalStateSchema::default());
    map
}

fuzz_target!(|data: &[u8]| {
    let schema = fuzz_schema();
    let portal = portal_schemas();
    let reader = || WireReader::new(data);

    let _ = decode_vle3(data);
    let _ = decode_vle4(data);
    let _ = decode_vle16(data);
    let _ = CompactDid::from_bytes(data);
    let _ = CompactPid::from_bytes(data);
    let _ = WirePosition::decode_wire(&mut reader());
    let _ = WirePosition::decode_stored(&mut reader());
    // A wire text that decodes (arguments included) can be written again and read back,
    // and its source and argument lookups are total.
    if let Ok(t) = LocalizedTokenText::from_bytes(data) {
        let _ = t.source();
        let _ = t.arguments.first().map(|a| t.argument(a.var_id));
        let bytes = t.to_bytes().expect("a decoded text encodes");
        assert!(LocalizedTokenText::from_bytes(&bytes).is_ok());
    }
    let _ = LocalizedTokenText::decode_stored(&mut reader());
    for kind in 0u16..=8 {
        let _ = LocalizedTokenTextArgPayload::decode(&mut reader(), kind, true);
        let _ = LocalizedTokenTextArgPayload::decode(&mut reader(), kind, false);
    }
    let _ = EncodedPropertyTable::from_bytes(data, &schema);
    let _ = WireBitfield::decode(&mut reader());
    if let Ok(desc) = EntityAppearanceDesc::decode(&mut reader()) {
        let _ = desc.encode(&mut WireWriter::new());
    }
    let _ = ActionPayload::decode(&mut reader(), &schema);
    if let Some((&id, rest)) = data.split_first()
        && let Ok(env) = decode_event_envelope(u32::from(id), 2, rest, 65_536)
    {
        assert_eq!(
            env.target_entity_id,
            active_target(2, env.routing_header.as_ref())
        );
    }

    // §3 character screen
    let _ = CharacterListResponse::from_bytes(data, &schema);
    let _ = CharacterNameCheckRequest::from_bytes(data);
    let _ = CharacterCreateRequest::from_bytes(data, &schema);
    let _ = CharacterCreateAck::from_bytes(data, &schema);
    let _ = CharacterCreateReject::from_bytes(data);
    let _ = CharacterDeleteRequest::from_bytes(data, 0x7A);
    let _ = CharacterDeleteRequest::from_bytes(data, 0xD1);
    let _ = CharacterDeleteStatus::from_bytes(data);
    let _ = CharacterOperationError::from_bytes(data);
    let _ = AccountEvictionNotice::from_bytes(data);
    let _ = PlatformAccountIdReport::from_bytes(data);
    let _ = CharacterEnterWorldRequest::from_bytes(data, &schema);

    // §4 world entry, portal space, calendar
    let _ = WorldNameAnnouncement::from_bytes(data);
    let _ = DisplayLocalizedText::from_bytes(data);
    let _ = ServerShutdownText::from_bytes(data);
    let _ = PlayerSessionReady::from_bytes(data, &schema);
    let _ = PortalPositionSync::from_bytes(data, &portal);
    let _ = PortalTransitionComplete::from_bytes(data);
    let _ = LoginFinished::from_bytes(data);
    let _ = EnterPortalSpace::from_bytes(data);
    let _ = PortalStateNotice::from_bytes(data);
    let _ = PortalReadyAck::from_bytes(data);
    let _ = WorldCalendarConfig::from_bytes(data, 0x3500_0000);

    // §5 object lifecycle, movement, actions, properties
    let _ = SubEventBatch::from_bytes(data, SubEventBatch::EVENT_ID_AOI_ENTER);
    let _ = SubEventBatch::from_bytes(data, SubEventBatch::EVENT_ID_SUB_EVENT);
    let _ = EntityCreate::from_bytes(data);
    let _ = EntityDespawn::from_bytes(data);
    let _ = MissingEntityQuery::from_bytes(data);
    let _ = PositionUpdate::from_bytes(data);
    let _ = ClientPhysicsSync::from_bytes(data, &schema);
    let _ = ClientActionCommand::from_bytes(data, &schema);
    let _ = ActionStart::from_bytes(data, 0x5A, &schema);
    let _ = ActionStart::from_bytes(data, 0xA8, &schema);
    let _ = ActionStepComplete::from_bytes(data);
    let _ = ActionCancel::from_bytes(data);
    let _ = ActionChainSync::from_bytes(data, &schema);
    let _ = EntityAnimLayerState::from_bytes(data, &schema);
    let _ = ActionQueueEnable::from_bytes(data);
    let _ = ActionStateReset::from_bytes(data);
    let _ = VisualDescriptionUpdate::from_bytes(data);
    for id in [0x52u32, 0xFE] {
        let _ = PropertyTableUpdate::decode(&mut reader(), id, &schema);
    }
    for id in [0x88u32, 0x92, 0xB0, 0x149] {
        let _ = SinglePropertyUpdate::decode(&mut reader(), id, &schema);
    }
    for id in [0x0Eu32, 0xE7, 0x76, 0x14A] {
        let _ = PropertyRemoval::decode(&mut reader(), id);
    }
    for id in [0x146u32, 0x147, 0x148] {
        let _ = LandblockPropertyEvent::decode(&mut reader(), id, &schema);
    }
    let _ = EntityStringEvent::from_bytes(data);
    let _ = PredictedPropertyAck::from_bytes(data);
});
