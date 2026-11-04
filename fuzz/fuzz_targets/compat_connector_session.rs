// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for the live connector's UDP session driver
//! (`tessera_compat::connector::session_phase`; `official-session.md` §3.4, §6, §9.3,
//! `official-messages-world.md` §3–§4, `official-messages-play.md` §6.6, §8). Nothing opens
//! a socket: the network is an in-memory stand-in.
//!
//! The first byte selects how far a synthetic, valid exchange goes before the fuzzed part:
//! bit 0 the login handshake, bit 1 a valid character list (`0x6C`), bit 2 `EnterWorld`, a
//! referral, the world handshake and `0x90` (the session is then in the world), bit 3 the
//! wire dictionaries of a synthetic install, bit 4 a chat server with an in-memory socket
//! binder, bit 5 a ticket lifetime (so the refresh runs, against a client that always
//! fails), bit 6 a cancelled attempt (the driver's stop predicate and the refresh thread's
//! cancel flag hold), bit 7 the end without sending (a revoked right, `abandon`) instead of
//! the graceful shutdown. The second byte: bit 0 a network with a source for readiness, so the
//! session reaches the world (no production path has one: with the bit clear the entry stops at
//! its readiness wait, and the world's messages go through the pending-entry store), bit 1 leaves
//! the setup's `0x90` out, so the input decides what the wait for it sees, bit 2 a stand-in for the
//! server gate, so `0x7C` and a `WorldLoaded` finish the entry in §9.3.9's order, bit 3 a stand-in for
//! the exact placement, which that order needs (no production path has one either). Then arbitrary
//! datagrams from the login, world and chat servers, arbitrary messages wrapped in valid
//! sequenced datagrams of the world connection once it exists, else of the login connection (the
//! widened opcodes of the bytes 0xC0 and above fall in the world-teardown band; a selector byte
//! addresses them to no recipient, the local player, a fixed entity or a recipient from the
//! input), neutral commands (`LeaveWorld` and `WorldLoaded` among them) and clock steps follow. A chunk
//! whose kind byte has bit 7 set has a 2-byte length. The synthetic install types the
//! entity and chat properties as the connector's test service does and has a baseline for
//! one template, and answers the initial exchange (`official-session.md` §9.3.2–§9.3.5): its
//! language key, name rules (7, 8 empty, 9 another language) and two archives; the first byte
//! `0xFF` asks for the unsupported answer instead. A message's selector byte chooses its queue with
//! bits 6 and 7 (queues 1 to 4, `0xDD`, `0xF5` and the window's other opcodes among queue 1's), and
//! with bit 5 a small fixed word and blob ID, so identities repeat across messages and the
//! tombstones, the queue-3 gate and the reused-identity rule of §9.3.9 are driven; bits 2 and 3 are the
//! sequence group. No panic, bounded state: the entity feed stays within its limits after every
//! step.
#![no_main]

use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use libfuzzer_sys::fuzz_target;
use tessera_compat::chat_session::ChatCodecs;
use tessera_compat::connector::data::{InstallData, SchemaView};
use tessera_compat::connector::entities::{
    MAX_ENTITIES, MAX_PENDING, PID_IS_INANIMATE_PROP, PID_IS_NPC_AGENT,
};
use tessera_compat::connector::net::{ConnectorNet, SendHttpClient};
use tessera_compat::connector::pending_world::{MAX_PENDING_BYTES, MAX_PENDING_ENTITIES};
use tessera_compat::connector::session_phase::{SessionDriver, SessionStart};
use tessera_compat::initial::{AdvertisedArchive, InstallUnsupported, IterationSpan};
use tessera_compat::login::{
    HttpClient, HttpRequest, HttpResponse, LoginError, NetPreferences, SessionHandoff,
    TicketRefresher, VersionInfo,
};
use tessera_compat::play::PlayError;
use tessera_compat::play::chat::{
    PID_CHAT_CATEGORY, PID_CHAT_SENDER_ID, PID_CHAT_SENDER_NAME, PID_CHAT_TEXT,
};
use tessera_compat::session::{
    BindPlan, BitTreeTable, CodecUuids, ConnectChallengeHeader, DatagramHeader, IsaacCipher,
    MAX_UNIQUE_PORT_ATTEMPTS, OptionalHeaders, SessionError, UdpDatagramTransport,
    WorldReferralHeader, encode_datagram, guid_fields_swapped, split_blob,
};
use tessera_compat::world::{
    CharacterListResponse, CharacterSummaryEntry, EncodedPropertyTable, EncodedPropertyValue,
    EntityAppearanceDesc, PID_AGENT_SPECIES, PID_ENTITY_DISPLAY_NAME, PID_SEX, PlayerSessionReady,
    PortalStateSchema, PropertyType, encode_event_envelope,
};
use tessera_net::{ChatChannel, Command, Location, SocialCommand, Vec3};

const LOCAL: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 5000);
const LOGIN: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 10), 9001);
const WORLD: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 20), 9100);
const CHAT: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 30), 9200);
const LOGIN_SEED: u32 = 0xBEEF_0001;
const WORLD_SEED: u32 = 0xBEEF_0002;
const MAX_STEPS: usize = 256;
/// The local player of the synthetic world.
const PLAYER: u64 = 0x0100_0000_0000_0007;
/// A fixed other entity the selector byte can address.
const ENTITY: u64 = 0x0300_0000_0000_0101;
/// The one template with a baseline.
const TEMPLATE: u32 = 0x7000_0123;
/// The types the synthetic dictionary gives the entity properties the feed reads
/// (`official-messages-world.md` §5.2.4) and the chat-line properties
/// (`official-messages-play.md` §6.5.1), as the connector's synthetic test service does;
/// every other PID gets a type from its number.
const DICTIONARY: [(u32, PropertyType); 9] = [
    (PID_ENTITY_DISPLAY_NAME, PropertyType::LocalizedTokenText),
    (PID_IS_NPC_AGENT, PropertyType::Boolean),
    (PID_IS_INANIMATE_PROP, PropertyType::Boolean),
    (PID_AGENT_SPECIES, PropertyType::Enum),
    (PID_SEX, PropertyType::Enum),
    (PID_CHAT_CATEGORY, PropertyType::Enum),
    (PID_CHAT_TEXT, PropertyType::LocalizedTokenText),
    (PID_CHAT_SENDER_NAME, PropertyType::LocalizedTokenText),
    (PID_CHAT_SENDER_ID, PropertyType::InstanceId),
];

#[derive(Debug)]
struct NoData {
    /// The install answers the initial exchange's questions.
    answers: bool,
    /// The one `0xCC` state schema, under every ID, so that message decodes.
    portal: PortalStateSchema,
}

impl InstallData for NoData {
    fn record(&self, _did: u32, _max: usize) -> Option<Vec<u8>> {
        None
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        DICTIONARY
            .iter()
            .find(|(p, _)| *p == pid)
            .map(|(_, t)| *t)
            .or_else(|| PropertyType::from_code(pid % 23).ok())
    }

    fn template_value(&self, template_did: u32, pid: u32) -> Option<EncodedPropertyValue> {
        (template_did == TEMPLATE && pid == PID_IS_NPC_AGENT)
            .then_some(EncodedPropertyValue::Boolean(true))
    }

    /// Only the one template counts as decoded: the others go through the read budget.
    fn template_cached(&self, template_did: u32) -> bool {
        template_did == TEMPLATE
    }

    fn portal_state_schema(&self, _id: u32) -> Option<&PortalStateSchema> {
        Some(&self.portal)
    }

    fn selected_language_key(&self) -> Result<u32, InstallUnsupported> {
        if self.answers {
            Ok(5)
        } else {
            Err(InstallUnsupported::LanguageAbsent)
        }
    }

    fn check_name_table(&self) -> Result<(), InstallUnsupported> {
        Ok(())
    }

    fn name_rule_languages(&self, rule_key: u32) -> Result<Vec<u32>, InstallUnsupported> {
        match rule_key {
            7 => Ok(vec![5]),
            8 => Ok(Vec::new()),
            9 => Ok(vec![6]),
            _ => Err(InstallUnsupported::NameRuleMissing),
        }
    }

    fn archive_inventory(&self) -> Result<Vec<AdvertisedArchive>, InstallUnsupported> {
        Ok((1u8..=2)
            .map(|i| AdvertisedArchive {
                resource_id: [0, 0, i, 1, 0, 0, 0, 0],
                iteration_count: 3,
                spans: vec![IterationSpan { start: 1, len: 3 }],
                notes: Vec::new(),
            })
            .collect())
    }
}

#[derive(Debug, Default)]
struct Queues {
    inbound: VecDeque<(Vec<u8>, SocketAddrV4)>,
    sent: usize,
}

#[derive(Debug)]
struct Udp(Arc<Mutex<Queues>>);

impl UdpDatagramTransport for Udp {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        Ok(LOCAL)
    }

    fn send_to(&mut self, buf: &[u8], _target: SocketAddrV4) -> Result<usize, SessionError> {
        if let Ok(mut q) = self.0.lock() {
            q.sent = q.sent.saturating_add(1);
        }
        Ok(buf.len())
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        let next = self.0.lock().ok().and_then(|mut q| q.inbound.pop_front());
        Ok(next.and_then(|(d, src)| {
            let n = d.len().min(buf.len());
            buf.get_mut(..n)?.copy_from_slice(d.get(..n)?);
            Some((n, src))
        }))
    }
}

#[derive(Debug)]
struct NoHttp;

impl HttpClient for NoHttp {
    fn execute(&mut self, _request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        Err(LoginError::ServiceUnavailable {
            reason: "no network in the fuzz target".to_owned(),
        })
    }
}

#[derive(Debug)]
struct Net {
    session: Arc<Mutex<Queues>>,
    chat: Option<Arc<Mutex<Queues>>>,
    /// Off, like every production path: the entry stops at its readiness wait.
    readiness: bool,
    /// Off, like every production path: no message is the exact placement (OPEN-WORLD-9).
    placement: bool,
    /// Off, like every production path: a test's stand-in for the server gate of §9.3.9's order.
    gate: Option<u32>,
}

impl ConnectorNet for Net {
    type Udp = Udp;

    // The test-only source of readiness, so the fuzzer reaches the world (no production path has
    // one, `official-session.md` §9.3.8) and also the chat server's code.
    fn readiness_specified(&self) -> bool {
        self.readiness
    }

    // The test-only source of the exact placement, so the fuzzer reaches the pending placement.
    fn placement_scripted(&self) -> bool {
        self.placement
    }

    // The order of §9.3.9 to its end: the one `0xF4`, `WorldEntered`, the pending state's release.
    fn server_gate(&self) -> Option<u32> {
        self.gate
    }

    fn chat_codec_specified(&self) -> bool {
        true
    }

    fn http(&mut self, _request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        Err(LoginError::ServiceUnavailable {
            reason: "no network in the fuzz target".to_owned(),
        })
    }

    fn background_http(&self) -> SendHttpClient {
        Box::new(NoHttp)
    }

    fn is_live(&self) -> bool {
        true
    }

    fn resolve(&mut self, host: &str, _port: u16) -> Result<SocketAddrV4, SessionError> {
        match host {
            "login.example.invalid" => Ok(LOGIN),
            "chat.example.invalid" => Ok(CHAT),
            _ => Err(SessionError::Io("unknown".to_owned())),
        }
    }

    fn local_ipv4_toward(&mut self, _remote: SocketAddrV4) -> Option<Ipv4Addr> {
        Some(*LOCAL.ip())
    }

    fn bind_udp(&mut self, plan: BindPlan) -> Result<Udp, SessionError> {
        // §3.2: Case B tries at most its bounded candidate list.
        assert!(plan.ports.candidates().len() <= MAX_UNIQUE_PORT_ATTEMPTS);
        Ok(Udp(Arc::clone(&self.session)))
    }

    fn chat_binder(
        &self,
        _first_local: Option<Ipv4Addr>,
    ) -> Option<Box<dyn FnMut() -> Result<Udp, PlayError> + Send>> {
        let chat = Arc::clone(self.chat.as_ref()?);
        Some(Box::new(move || Ok(Udp(Arc::clone(&chat)))))
    }
}

/// The synthetic dictionaries' on-disk UUIDs by direction (`official-session.md` §7.3).
const UUIDS: CodecUuids = CodecUuids {
    incoming: [
        0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0, 0xF0,
        0x01,
    ],
    outgoing: [0x5A; 16],
};

/// A primary challenge from incoming recipient `rec_id` naming the incoming dictionary
/// (`official-session.md` §6.3).
fn challenge(rec_id: u16, seed: u32) -> Option<Vec<u8>> {
    let opt = OptionalHeaders {
        connect_challenge: Some(ConnectChallengeHeader {
            clock_candidate: 1.0,
            cookie: [1, 2, 3, 4, 5, 6, 7, 8],
            reserved_16: [0, 0],
            outgoing_rec_id: 0x0222,
            incoming_seed: seed,
            outgoing_seed: 0xCAFE_1234,
            incoming_codec_guid: guid_fields_swapped(UUIDS.incoming),
        }),
        ..OptionalHeaders::default()
    };
    let hdr = DatagramHeader {
        rec_id,
        peer_word: 0x3344,
        ..DatagramHeader::default()
    };
    encode_datagram(hdr, &opt, &[], None, None).ok()
}

/// One synthetic server connection.
struct Server {
    isaac: IsaacCipher,
    seq: u32,
}

impl Server {
    fn new(seed: u32) -> Self {
        Self {
            isaac: IsaacCipher::from_seed(seed),
            seq: 1,
        }
    }

    fn control(&self, opt: OptionalHeaders) -> Option<Vec<u8>> {
        let hdr = DatagramHeader {
            rec_id: 0x0111,
            seq_num: self.seq,
            ..DatagramHeader::default()
        };
        encode_datagram(hdr, &opt, &[], None, None).ok()
    }

    fn blob(&mut self, opcode: u32, envelope: &[u8]) -> Option<Vec<u8>> {
        self.seq = self.seq.checked_add(1)?;
        let frags = split_blob(2, (self.seq & 0xFFFF) as u16, 1, 0, None, opcode, envelope).ok()?;
        let mask = self.isaac.next_mask();
        let hdr = DatagramHeader {
            rec_id: 0x0111,
            seq_num: self.seq,
            ..DatagramHeader::default()
        };
        encode_datagram(hdr, &OptionalHeaders::default(), &frags, Some(mask), None).ok()
    }

    /// A blob to `recipient` as sequenced datagrams of one fragment each, so a long
    /// message (a 2-byte length) still fits. `identity`: `(queue, word and blob ID, group)`, where
    /// a word and blob ID of `None` are fresh ones.
    fn blob_to(
        &mut self,
        identity: (u8, Option<u16>, u16),
        opcode: u32,
        recipient: Option<u64>,
        envelope: &[u8],
    ) -> Vec<Vec<u8>> {
        let (queue, repeated, group) = identity;
        let fresh = (self.seq.wrapping_add(1) & 0xFFFF) as u16;
        let word = repeated.unwrap_or(fresh);
        let Ok(frags) = split_blob(queue, word, word, group, recipient, opcode, envelope) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for frag in frags {
            let Some(seq) = self.seq.checked_add(1) else {
                break;
            };
            self.seq = seq;
            let mask = self.isaac.next_mask();
            let hdr = DatagramHeader {
                rec_id: 0x0111,
                seq_num: seq,
                ..DatagramHeader::default()
            };
            if let Ok(d) = encode_datagram(
                hdr,
                &OptionalHeaders::default(),
                std::slice::from_ref(&frag),
                Some(mask),
                None,
            ) {
                out.push(d);
            }
        }
        out
    }

    /// The list `0x6C` has one shared flags byte, which its payload already starts with
    /// (`official-session.md` §9.3.5).
    fn event(&mut self, opcode: u32, inner: &[u8]) -> Option<Vec<u8>> {
        if matches!(opcode, 0x6C | 0x90 | 0x6D | 0xB3) {
            return self.blob(opcode, inner);
        }
        self.blob(opcode, &encode_event_envelope(inner))
    }
}

fn character_list() -> Option<Vec<u8>> {
    CharacterListResponse {
        characters: vec![CharacterSummaryEntry {
            character_id: 0x0200_0000_0000_0042,
            name: "Aldo".to_owned(),
            status_or_world: 0,
            public_props: EncodedPropertyTable::new(),
            private_props: EncodedPropertyTable::new(),
            metadata_a: 0,
            visual_desc: EntityAppearanceDesc::default(),
            metadata_b: 0,
            metadata_c: u32::MAX,
            metadata_d: 0,
            metadata_e: u32::MAX,
            timestamp_or_meta: 0,
        }],
        account_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .ok()
}

fn session_ready() -> Option<Vec<u8>> {
    PlayerSessionReady {
        player_id: PLAYER,
        session_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .ok()
}

/// The wire codecs (bit 3: on) and the dictionaries' UUIDs, always given so the challenge
/// is accepted; without the codecs the client's datagrams go out uncompressed.
fn codecs(on: bool) -> (ChatCodecs, CodecUuids) {
    if !on {
        return (ChatCodecs::default(), UUIDS);
    }
    let out = Arc::new(BitTreeTable::synthetic_dictionary(UUIDS.outgoing));
    let inc = Arc::new(BitTreeTable::synthetic_dictionary(UUIDS.incoming));
    (
        ChatCodecs {
            incoming: Some(inc),
            outgoing: Some(out),
        },
        UUIDS,
    )
}

fn start(net: &mut Net, flags: u8) -> Option<SessionDriver<Net>> {
    let version = VersionInfo::new("9999", "00000000-0000-0000-0000-000000000000", 1).ok()?;
    let chat = if flags & 0x10 != 0 {
        "chat.example.invalid:9200"
    } else {
        ""
    };
    let lifetime = if flags & 0x20 != 0 { 60 } else { 0 };
    let handoff = SessionHandoff::new(
        "login.example.invalid:9001",
        0,
        "SUB0000000000000000000000",
        "AAECAwQFBgcICQ==",
        version.clone(),
        "English",
        chat,
        "https://auth.example.invalid/",
        lifetime,
    )
    .ok()?;
    let refresher = TicketRefresher::new(
        "https://auth.example.invalid/",
        "SUB0000000000000000000000",
        "AAECAwQFBgcICQ==",
        lifetime,
        0.0,
    );
    let (codecs, codec_uuids) = codecs(flags & 0x08 != 0);
    let start = SessionStart {
        handoff,
        login_peer: LOGIN,
        plain: Default::default(),
        refresher,
        chat_endpoint: (flags & 0x10 != 0).then_some(CHAT),
        version,
        codecs,
        codec_uuids,
        schema: SchemaView(Arc::new(NoData {
            answers: flags != 0xFF,
            portal: PortalStateSchema::default(),
        })),
        cancel: Arc::new(AtomicBool::new(flags & 0x40 != 0)),
        // Bit 5 of the first byte: `Net.ComputeUniquePort` off (§3.2 Case C).
        net: NetPreferences {
            compute_unique_port: flags & 0x20 == 0,
            connection_speed_kbps: f64::from(flags),
            ..NetPreferences::default()
        },
        login_server_entry: Default::default(),
        // The datagram dump writes files: never on in fuzzing.
        dump: None,
    };
    SessionDriver::start(net, start, 0.0, &|| false).ok()
}

fn ack() -> OptionalHeaders {
    OptionalHeaders {
        cumulative_ack: Some(0),
        ..OptionalHeaders::default()
    }
}

fuzz_target!(|data: &[u8]| {
    let session = Arc::new(Mutex::new(Queues::default()));
    let flags = data.first().copied().unwrap_or(0);
    let extra = data.get(1).copied().unwrap_or(0);
    let chat = (flags & 0x10 != 0).then(|| Arc::new(Mutex::new(Queues::default())));
    let mut net = Net {
        session: Arc::clone(&session),
        chat: chat.clone(),
        readiness: extra & 0x01 != 0,
        gate: (extra & 0x04 != 0).then_some(0x7C),
        placement: extra & 0x08 != 0,
    };
    let Some(mut driver) = start(&mut net, flags) else {
        return;
    };
    let deliver = |q: &Arc<Mutex<Queues>>, d: Option<Vec<u8>>, src: SocketAddrV4| {
        if let (Some(d), Ok(mut q)) = (d, q.lock()) {
            q.inbound.push_back((d, src));
        }
    };
    let mut login = Server::new(LOGIN_SEED);
    let mut world = Server::new(WORLD_SEED);
    // The world server takes the messages once its connection exists: on the production path that
    // is well before any world entry.
    let mut referred = false;
    let mut now = 0.0f64;
    let step = |driver: &mut SessionDriver<Net>, now: f64| {
        // Bit 6 of the first byte: the attempt is cancelled (no referral, no refresh, no
        // send, no chat socket).
        let stop = || flags & 0x40 != 0;
        driver.step(now, true, &stop);
        if let Some((player, name)) = driver.wants_chat(now) {
            match net.chat_binder(driver.first_local_ipv4()) {
                Some(b) => driver.attach_chat(b, player, &name, now, &stop),
                None => driver.chat_failed(now),
            }
        }
        let _ = driver.take_events();
        let (tracked, pending) = driver.feed_sizes();
        assert!(tracked <= MAX_ENTITIES && pending <= MAX_PENDING && pending <= tracked);
        let (entities, bytes) = driver.pending_sizes();
        assert!(entities <= MAX_PENDING_ENTITIES && bytes <= MAX_PENDING_BYTES);
    };
    if flags & 0x01 != 0 {
        deliver(&session, challenge(0x0111, LOGIN_SEED), LOGIN);
        deliver(&session, login.control(ack()), LOGIN);
        step(&mut driver, now);
        if flags & 0x02 != 0 {
            deliver(
                &session,
                character_list().and_then(|l| login.event(0x6C, &l)),
                LOGIN,
            );
            step(&mut driver, now);
            if flags & 0x04 != 0 {
                let _ = driver.command(
                    &Command::EnterWorld {
                        character: "Aldo".to_owned(),
                    },
                    now,
                );
                deliver(
                    &session,
                    login.control(OptionalHeaders {
                        world_referral: Some(WorldReferralHeader {
                            cookie: [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
                            target_addr: WORLD,
                            metadata: [0; 4],
                        }),
                        ..OptionalHeaders::default()
                    }),
                    LOGIN,
                );
                step(&mut driver, now);
                deliver(&session, challenge(0x0111, WORLD_SEED), WORLD);
                deliver(&session, world.control(ack()), WORLD);
                if extra & 0x02 == 0 {
                    deliver(
                        &session,
                        session_ready().and_then(|r| world.event(0x90, &r)),
                        WORLD,
                    );
                }
                step(&mut driver, now);
                referred = true;
            }
        }
    }
    let mut rest = data.get(2..).unwrap_or_default();
    let mut steps = 0;
    while let Some((&kind, tail)) = rest.split_first() {
        steps += 1;
        if steps > MAX_STEPS || driver.ended() {
            break;
        }
        // Bit 7 of the kind: a 2-byte length (large batches), else 1 byte.
        let (len, header) = if kind & 0x80 != 0 {
            let lo = tail.first().copied().unwrap_or(0);
            let hi = tail.get(1).copied().unwrap_or(0);
            (usize::from(u16::from_le_bytes([lo, hi])), 2)
        } else {
            (usize::from(tail.first().copied().unwrap_or(0)), 1)
        };
        let chunk = tail
            .get(header..header + len)
            .unwrap_or_else(|| tail.get(header..).unwrap_or_default());
        rest = tail.get(header + chunk.len()..).unwrap_or_default();
        match kind % 8 {
            0 => deliver(&session, Some(chunk.to_vec()), LOGIN),
            1 => deliver(&session, Some(chunk.to_vec()), WORLD),
            2 | 3 => {
                // A message: opcode (one byte, widened) and its envelope, on the world
                // connection once in the world, else on the login connection.
                let opcode = match chunk.first() {
                    // The world-teardown band (`official-messages-play.md` §7.2 item 3).
                    Some(&b) if kind % 8 == 3 && b >= 0xC0 => 0x3000_0010 | u32::from(b & 0x3F),
                    Some(&b) if kind % 8 == 3 => 0x0000_0100 | u32::from(b),
                    Some(&b) => u32::from(b),
                    None => 0x6C,
                };
                // The selector byte: no recipient, the player, the fixed entity, or eight
                // bytes of the input.
                let selector = chunk.get(1).copied().unwrap_or(0);
                let queue = 1 + (selector >> 6);
                let repeated = (selector & 0x20 != 0).then_some(u16::from(selector & 0x03));
                let group = u16::from((selector >> 2) & 0x03);
                let (recipient, body) = match selector % 4 {
                    0 => (None, chunk.get(2..)),
                    1 => (Some(PLAYER), chunk.get(2..)),
                    2 => (Some(ENTITY), chunk.get(2..)),
                    _ => {
                        let id = chunk
                            .get(2..10)
                            .and_then(|b| <[u8; 8]>::try_from(b).ok())
                            .map_or(0, u64::from_le_bytes);
                        (Some(id), chunk.get(10..))
                    }
                };
                let body = body.unwrap_or_default();
                let (server, src) = if referred {
                    (&mut world, WORLD)
                } else {
                    (&mut login, LOGIN)
                };
                for d in server.blob_to((queue, repeated, group), opcode, recipient, body) {
                    deliver(&session, Some(d), src);
                }
            }
            4 => {
                let text = String::from_utf8_lossy(chunk).into_owned();
                let cmd = match chunk.first().copied().unwrap_or(0) % 7 {
                    5 => Command::LeaveWorld,
                    6 => Command::WorldLoaded,
                    0 => Command::EnterWorld { character: text },
                    1 => Command::Chat {
                        channel: ChatChannel::Say,
                        text,
                    },
                    2 => Command::Social(SocialCommand::JoinChannel { name: text }),
                    3 => Command::Chat {
                        channel: ChatChannel::Fellowship,
                        text,
                    },
                    _ => Command::Move {
                        location: Location::default(),
                        heading: f32::from(chunk.len() as u8),
                        velocity: Vec3::new(1.0, 0.5, 0.0),
                        on_ground: true,
                    },
                };
                let _ = driver.command(&cmd, now);
            }
            5 => {
                if let Some(q) = &chat {
                    deliver(q, Some(chunk.to_vec()), CHAT);
                }
            }
            6 => {
                // A datagram from an address no connection has.
                deliver(
                    &session,
                    Some(chunk.to_vec()),
                    SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 99), 1),
                );
            }
            _ => now += f64::from(chunk.first().copied().unwrap_or(1)) * 0.25,
        }
        step(&mut driver, now);
    }
    if flags & 0x80 != 0 {
        driver.abandon(now);
    } else {
        driver.begin_shutdown(now);
        let _ = driver.shutdown_step(now + 1.0);
        let _ = driver.shutdown_step(now + 3.0);
    }
});
