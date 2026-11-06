// SPDX-License-Identifier: GPL-3.0-or-later
//! A synthetic in-memory official service for other crates' tests (feature `test-support`,
//! which only the fuzz workspace and dev-dependencies enable, `official-entry.md` §6.1,
//! OE-T22): the client's official flow can run against the live connector without a network.
//!
//! [`SyntheticService`] answers the login's HTTP exchanges from fixtures, resolves the one
//! login server name to a documentation address (RFC 5737), and plays the login server over
//! an in-memory UDP transport. It answers what the client sends, as the client sends it:
//!
//! | The client sends | The synthetic server answers |
//! |---|---|
//! | `SessionAuthHeader` | the connect challenge, an acknowledgement and the version request `0x12F` |
//! | `0x12C` (version) | `0x12D` (accepted) and the character list `0x6C` (one character, "Aldo") |
//! | `0x6F` (enter world) | `0x90` (session start), `0x5E` (portal space, stamp 0, scene [`ENTRY_SCENE_ID`]), the player's placement `0xCE` (teleport stamp 0, at [`PLAYER_AT`]) and `0x7C` (`login_finished`), in the order of `official-messages-world.md` §4.3.3; no referral: the login connection carries the world |
//! | `0xF4` (portal exit) | nothing (counted) |
//! | `0x30` (an RPC; the driver sends none on these paths, `LeaveWorld` closes) | a world teardown `0x3000001B` and the list `0x6C` again |
//! | `0xD0` (a missing-entity query) | nothing (counted) |
//!
//! [`SyntheticService::new`] is that historical script, for the tests of what follows the
//! list. [`SyntheticService::observed_order`] plays the login in the order of
//! `official-session.md` §9.3.1 and §9.3.5 instead, on queue 1, with synthetic bodies:
//!
//! | The client does | The synthetic server answers |
//! |---|---|
//! | `SessionAuthHeader` | the connect challenge |
//! | `ConnectAcceptHeader` | `0x149` (with ClockSync), `0xAA`, five `0x149`, `0x30000040`, two `0xAA`, `0x43`, `0xDD` |
//! | a complete `0xEC` | `0x149`, `0xF5` |
//! | a complete `0xF5` | `0xFE`, `0xAA`, `0x54`, the list `0x6C` with one nonempty entry and account table ([`character_list`]) |
//! | a complete `0x6F` | `0x149`, one queue-2 `0x30`, two queue-1 `0x30`, then the referral to its world server |
//! | `ReferralAuthHeader` (world) | the connect challenge |
//! | `ConnectAcceptHeader` (world) | the stream of [`observed_world_stream`], with `0x90` and no readiness |
//!
//! [`SyntheticService::short_acks`] plays that script with the install's incoming dictionary
//! replaced by [`short_header_dictionary`]: each client datagram with a blob part is answered
//! first by a compressed sequenced packet that holds only its Ack, shorter on the wire than a
//! whole header.
//!
//! A test plays the rest of the world with [`SyntheticService::send_world_event`] and the
//! payload builders below ([`entity_batch`], [`position`], [`chat_line`]): other entities,
//! their moves, chat lines and despawns. The synthetic install's property dictionary types
//! the properties those builders use ([`DICTIONARY`], the types of
//! `official-messages-world.md` §5.2.4 and `official-messages-play.md` §6.5.1).
//!
//! Every endpoint is an RFC 2606 `.example.invalid` placeholder or an RFC 5737 address;
//! nothing opens a socket, resolves a real name or starts a process.

use std::collections::{BTreeMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::LiveConfig;
use super::data::{InstallData, LocalPaths};
use super::net::{ConnectorNet, NetFactory, SendHttpClient};
use crate::connector::entities::{PID_IS_INANIMATE_PROP, PID_IS_NPC_AGENT};
use crate::initial::{AdvertisedArchive, InstallUnsupported, IterationNote, IterationSpan};
use crate::login::soap_encoder::{GLS_NAMESPACE, SOAP_ENVELOPE_NAMESPACE};
use crate::login::{HttpClient, HttpRequest, HttpResponse, LoginError};
use crate::net_access::NetworkAccess;
use crate::play::chat::{
    PID_CHAT_CATEGORY, PID_CHAT_SENDER_ID, PID_CHAT_SENDER_NAME, PID_CHAT_TEXT, UI_EVENT_CHAT_LINE,
    UiPropertyEvent,
};
use crate::session::{
    BindPlan, BitTreeCodec, BitTreeTable, BlobFragmentHeader, BlobReassembler,
    ConnectChallengeHeader, DatagramHeader, IsaacCipher, OptionalHeaders, SessionError,
    UdpDatagramTransport, VersionStatusMessage, decode_datagram, encode_datagram,
    guid_fields_swapped, split_blob,
};
use crate::world::{
    CharacterListResponse, CharacterSummaryEntry, EncodedPropertyTable, EncodedPropertyValue,
    EnterPortalSpace, EntityCreate, EntityPlacement, LocalizedTokenText,
    PID_ACCOUNT_NUM_CHARACTERS_ALLOWED, PID_AGENT_CLASS, PID_AGENT_SPECIES,
    PID_CHARGEN_CHARACTER_HAS_DATA, PID_CHARGEN_CHARACTER_LEVEL, PID_ENTITY_DISPLAY_NAME, PID_SEX,
    PlayerSessionReady, PositionUpdate, PropertyTableUpdate, PropertyType, SubEventBatch,
    SubEventFrame, VisualDescriptionUpdate, WirePosition, encode_event_envelope,
};

/// The `scene_id` of the synthetic world entry's `0x5E`.
pub const ENTRY_SCENE_ID: u32 = 0x0000_0E01;

/// The inner payload of the world entry's `0x5E` (stamp 0, [`ENTRY_SCENE_ID`]).
fn entry_portal_space() -> Vec<u8> {
    let mut w = crate::world::WireWriter::new();
    let _ = EnterPortalSpace {
        stamp: 0,
        scene_id: ENTRY_SCENE_ID,
    }
    .encode(&mut w);
    w.into_vec()
}

/// The client's address on the synthetic network.
const LOCAL: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 5000);
/// The synthetic login server.
const LOGIN_SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 10), 9001);
/// The world server the observed-order script refers the client to.
const OBSERVED_WORLD_SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 11), 9002);
/// Its synthetic referral cookie.
const OBSERVED_COOKIE: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
/// `official-session.md` Vector 16's synthetic `0x89` data.
const OBSERVED_APPEARANCE: [u8; 37] = [
    0x00, 0x40, 0x07, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x3F, 0x00, 0x00, 0x80, 0x3E, 0x20, 0x00, 0x00,
    0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01, 0x01, 0xDF, 0x9B, 0x57, 0x13, 0xE0,
    0xAC, 0x68, 0x24, 0x01, 0x00,
];
/// The synthetic ticket.
const TICKET: &str = "AAECAwQFBgcICQ==";
/// The install's datacenter URL.
const GLS: &str = "https://gls.example.invalid/datacenter";
/// The login server's assigned recipient ID (the client's incoming recipient) and the seed
/// of its outgoing masks (the client's incoming seed).
const REC_ID: u16 = 0x0111;
const SEED: u32 = 0xBEEF_0001;
/// The client's outgoing recipient, named in the challenge's payload bytes 18..19.
const SERVER_REC_ID: u16 = 0x0222;
/// The challenge header's bytes 18..19, which the client copies (`official-session.md` §4.2).
const PEER_WORD: u16 = 0x5A5A;
/// The on-disk UUIDs of the synthetic install's dictionaries (`official-session.md` §7.3):
/// incoming (`0x0E010001`) and outgoing (`0x0E010000`). Asymmetric, so the challenge's
/// field-wise conversion matters.
pub const INCOMING_UUID: [u8; 16] = [
    0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0, 0xF0, 0x01,
];
/// See [`INCOMING_UUID`].
pub const OUTGOING_UUID: [u8; 16] = [
    0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0x00, 0xAB, 0xCD,
];

/// The selected language's full key of the synthetic install (`official-session.md` §10.2 Vector
/// 10's synthetic key, never a value for the service).
pub const SYNTHETIC_LANGUAGE_KEY: u32 = 0x1000_0005;
/// The name-rule key of the synthetic server's `0xDD` and the synthetic install's rule
/// (§10.2 Vector 11's synthetic key).
pub const SYNTHETIC_NAME_RULE: u32 = 7;

/// The synthetic install's archives: nine with invented identifiers and iteration lists, so
/// their `0xEC` takes several fragments (§5.6.4).
#[must_use]
pub fn synthetic_inventory() -> Vec<AdvertisedArchive> {
    (1u8..=9)
        .map(|i| AdvertisedArchive {
            resource_id: [0, 0, i, 1, 0, 0, 0, 0],
            iteration_count: 40,
            spans: vec![
                IterationSpan { start: 1, len: 30 },
                IterationSpan {
                    start: 10_031,
                    len: 10,
                },
            ],
            notes: (0..11)
                .map(|n| IterationNote {
                    // The 11 highest listed: the end of the first run, then the whole second.
                    iteration: if n == 0 { 30 } else { 10_030 + n },
                    opaque: [i, 0xA5, 0x5A, 3, 4, 5, 6, u8::try_from(n).unwrap_or(0)],
                })
                .collect(),
        })
        .collect()
}

/// The name of the one character the synthetic account has.
pub const CHARACTER: &str = "Aldo";
/// Its synthetic `agent_species` key.
pub const CHARACTER_SPECIES: u32 = 0x47;
/// Its `sex` key (`official-messages-world.md` §3.1.3).
pub const CHARACTER_SEX: u32 = 0x1000;
/// Its synthetic `agent_class` key.
pub const CHARACTER_CLASS: u32 = 0x18;
/// Its character-screen level.
pub const CHARACTER_LEVEL: i32 = 12;
/// The player entity of the synthetic world.
pub const PLAYER: u64 = 0x0100_0000_0000_0007;
/// Where the synthetic world places the player (offsets in its landblock, metres).
pub const PLAYER_AT: [f32; 3] = [12.0, 34.0, 5.0];

/// Another entity of the synthetic world, an NPC.
pub const NPC: u64 = 0x0300_0000_0000_0101;
/// The NPC's template game object (a synthetic DID).
pub const NPC_TEMPLATE: u32 = 0x7000_0123;
/// The NPC's display name.
pub const NPC_NAME: &str = "Watcher";
/// Where the synthetic world first places the NPC (offsets in the player's landblock).
pub const NPC_AT: [f32; 3] = [20.0, 40.0, 6.0];

/// The property types of the synthetic install's dictionary: the entity properties of
/// `official-messages-world.md` §5.2.4 and the chat-line properties of
/// `official-messages-play.md` §6.5.1 the builders use, the category as an enum.
///
/// §6.5.1 describes the chat text (`0x100015D7`) and the sender name (`0x10000914`) as
/// "Prefixed UTF-16LE string", which is none of the 22 property types of
/// `official-messages-world.md` §2.7.1 (type 1 is an 8-bit string). The stub types them as
/// tokenized texts (type 13, whose literal is a prefixed UTF-16LE string); real runs take
/// every type from the install's property dictionary (`0x34000000`), and the chat line
/// reads either form (`crate::play::chat`).
pub const DICTIONARY: [(u32, PropertyType); 14] = [
    (crate::world::PID_WORLD_TIME_OFFSET, PropertyType::Float),
    (PID_ENTITY_DISPLAY_NAME, PropertyType::LocalizedTokenText),
    (PID_IS_NPC_AGENT, PropertyType::Boolean),
    (PID_IS_INANIMATE_PROP, PropertyType::Boolean),
    (PID_AGENT_SPECIES, PropertyType::Enum),
    (PID_SEX, PropertyType::Enum),
    (PID_AGENT_CLASS, PropertyType::Enum),
    (PID_CHARGEN_CHARACTER_LEVEL, PropertyType::Int),
    (PID_CHARGEN_CHARACTER_HAS_DATA, PropertyType::Boolean),
    (PID_ACCOUNT_NUM_CHARACTERS_ALLOWED, PropertyType::Int),
    (PID_CHAT_CATEGORY, PropertyType::Enum),
    (PID_CHAT_TEXT, PropertyType::LocalizedTokenText),
    (PID_CHAT_SENDER_NAME, PropertyType::LocalizedTokenText),
    (PID_CHAT_SENDER_ID, PropertyType::InstanceId),
];

/// Most datagrams waiting for the client and most blob opcodes kept in [`Seen::blobs`]
/// (the oldest are dropped beyond it): a test that runs long never grows the service
/// without bound.
pub const MAX_KEPT: usize = 4096;

/// What the synthetic service saw, for the test's assertions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seen {
    /// HTTP requests answered.
    pub requests: usize,
    /// Sockets bound.
    pub binds: usize,
    /// Opcodes of the client's blobs, in order (the last [`MAX_KEPT`]).
    pub blobs: VecDeque<u32>,
    /// Datagrams with the transport disconnect.
    pub disconnects: usize,
    /// `ReferralAuth`s the observed-order world server saw.
    pub referral_auths: u64,
    /// Opcodes the client sent on that world connection.
    pub world_blobs: Vec<u32>,
    /// The complete data of each `0xEC`, `0xF5` and `0x6F` of [`SyntheticService::observed_order`].
    pub advertisements: Vec<Vec<u8>>,
    /// See [`Self::advertisements`].
    pub completions: Vec<Vec<u8>>,
    /// See [`Self::advertisements`].
    pub selections: Vec<Vec<u8>>,
    /// The fragment headers of the client's blobs in that mode (the last [`MAX_KEPT`]).
    pub fragment_headers: VecDeque<BlobFragmentHeader>,
    /// The sequence numbers and flags of the client's sequenced datagrams in that mode (the
    /// last [`MAX_KEPT`]).
    pub sequenced: VecDeque<(u32, u32)>,
    /// The raw lengths of the short Acks sent in the [`SyntheticService::short_acks`] mode (the
    /// last [`MAX_KEPT`]).
    pub short_acks: VecDeque<usize>,
    /// The client's datagrams with a `SequenceNakHeader` in the observed-order script.
    pub naks: u64,
}

#[derive(Debug, Default)]
struct State {
    seen: Seen,
    inbound: VecDeque<(Vec<u8>, SocketAddrV4)>,
    server: Option<Server>,
    /// The attempt's network right, kept for its liveness (its own revocation and
    /// `revoke_all` both end it).
    access: Option<NetworkAccess>,
    /// Plays [`SyntheticService::observed_order`]'s script.
    observed: bool,
    /// [`SyntheticService::short_acks`].
    short_acks: bool,
    /// [`SyntheticService::allow_re_entry`].
    re_entry: bool,
    /// Whether this service's network has a source for readiness: the historical script's in-world
    /// tests do; [`SyntheticService::observed_order`] does not, like every production path.
    readiness: bool,
    /// Whether this service's network names the exact placement ([`SyntheticService::script_placement`]).
    placement: bool,
    /// The world server of the observed-order script, after its referral.
    world_server: Option<Server>,
    /// The client's blobs being put together in that mode.
    reassembler: BlobReassembler,
}

impl State {
    /// Queues a datagram for the client (the oldest is dropped beyond [`MAX_KEPT`]).
    fn deliver(&mut self, datagram: Vec<u8>) {
        if self.inbound.len() >= MAX_KEPT {
            self.inbound.pop_front();
        }
        self.inbound.push_back((datagram, LOGIN_SERVER));
    }

    /// [`Self::deliver`] from the observed-order script's world server.
    fn deliver_world(&mut self, datagram: Vec<u8>) {
        if self.inbound.len() >= MAX_KEPT {
            self.inbound.pop_front();
        }
        self.inbound.push_back((datagram, OBSERVED_WORLD_SERVER));
    }

    /// Notes a blob opcode the client sent (the oldest is dropped beyond [`MAX_KEPT`]).
    fn saw_blob(&mut self, op: u32) {
        if self.seen.blobs.len() >= MAX_KEPT {
            self.seen.blobs.pop_front();
        }
        self.seen.blobs.push_back(op);
    }
}

/// The synthetic service (module docs). Clones share one state.
#[derive(Debug, Clone, Default)]
pub struct SyntheticService(Arc<Mutex<State>>);

impl SyntheticService {
    /// A fresh service with the historical script (module docs).
    #[must_use]
    pub fn new() -> Self {
        let service = Self::default();
        service.state().readiness = true;
        service
    }

    /// A fresh service with the observed order of `official-session.md` §9.3.1 and §9.3.5
    /// (module docs).
    #[must_use]
    pub fn observed_order() -> Self {
        let service = Self::default();
        service.state().observed = true;
        service.state().readiness = false;
        service
    }

    /// [`Self::observed_order`] with the short Acks of the module docs: the server's compressed
    /// sequenced packets come in the form `official-session.md` §4.1.2 names (an Ack piggybacked on
    /// a sequenced packet), and the install's incoming dictionary compresses it under a whole header.
    #[must_use]
    pub fn short_acks() -> Self {
        let service = Self::observed_order();
        service.state().short_acks = true;
        service
    }

    /// Lets this service's network name the local player's exact placement, which
    /// `official-session.md` §9.3.9 leaves OPEN-WORLD-9 on every production path, so a test reaches
    /// the road from `WorldPlacementPending` to `WorldLoaded`.
    pub fn script_placement(&self) {
        self.state().placement = true;
    }

    /// Lets the client enter the world again after a return, which `official-session.md` §9.3.7
    /// leaves unsupported on every production path, so a test reaches what a second world must not
    /// inherit from the first.
    pub fn allow_re_entry(&self) {
        self.state().re_entry = true;
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What the service saw so far.
    #[must_use]
    pub fn seen(&self) -> Seen {
        self.state().seen.clone()
    }

    /// Writes a synthetic install under `root` (the launcher configuration naming the
    /// synthetic datacenter, a language archive and a superblock) and returns the live
    /// connector's configuration over it, with the synthetic records (the compression
    /// dictionaries).
    ///
    /// # Errors
    ///
    /// The files cannot be written.
    pub fn live_config(&self, root: &Path) -> std::io::Result<LiveConfig> {
        let short = self.state().short_acks;
        std::fs::create_dir_all(root)?;
        std::fs::write(
            root.join("lotro.launcherconfig"),
            format!(
                r#"<?xml version="1.0" encoding="utf-8"?>
<configuration><appSettings>
<add key="Launcher.DataCenterService.GLS" value="{GLS}" />
<add key="DataCenter.GameName" value="LOTRO" />
</appSettings></configuration>"#
            ),
        )?;
        std::fs::write(root.join("client_local_English.dat"), b"SYNTH")?;
        let mut header = vec![0u8; 0x1A8];
        if let Some(h) = header.get_mut(0x140..0x144) {
            h.copy_from_slice(b"BT\0\0");
        }
        if let Some(h) = header.get_mut(0x170..0x174) {
            h.copy_from_slice(&0x2800_0000u32.to_le_bytes());
        }
        if let Some(h) = header.get_mut(0x17C..0x18C) {
            for (b, v) in h.iter_mut().zip(0x10u8..) {
                *b = v;
            }
        }
        if let Some(h) = header.get_mut(0x18C..0x190) {
            h.copy_from_slice(&1100u32.to_le_bytes());
        }
        std::fs::write(root.join("client_general.dat"), header)?;
        Ok(LiveConfig {
            paths: LocalPaths {
                install_root: root.to_path_buf(),
                user_settings_dir: None,
                network_version: Some("9999".to_owned()),
            },
            loader: Box::new(move |_, _| Ok(Arc::new(records(short)) as Arc<dyn InstallData>)),
            locate: None,
        })
    }

    /// The synthetic install's records as [`Self::live_config`]'s loader makes them.
    #[must_use]
    pub fn install_data(&self) -> Arc<dyn InstallData> {
        Arc::new(records(self.state().short_acks))
    }

    /// The network factory of the live connector over this service.
    #[must_use]
    pub fn factory(&self) -> SyntheticFactory {
        SyntheticFactory(self.clone())
    }

    /// Whether the attempt's network right still holds (no attempt yet: live).
    fn live(&self) -> bool {
        self.state()
            .access
            .as_ref()
            .is_none_or(NetworkAccess::is_live)
    }

    /// Sends event `opcode` with the inner payload `inner` to the client as the synthetic
    /// world's next blob, addressed to `recipient` (`None`: the default recipient). `false`
    /// before the session's handshake (there is no server connection to send it on).
    pub fn send_world_event(&self, opcode: u32, recipient: Option<u64>, inner: &[u8]) -> bool {
        let mut st = self.state();
        let Some(server) = st.server.as_mut() else {
            return false;
        };
        let envelope = if crate::world::one_shared_prefix(opcode) {
            inner.to_vec()
        } else {
            encode_event_envelope(inner)
        };
        let Some(d) = server.blob_to(opcode, recipient, &envelope) else {
            return false;
        };
        st.deliver(d);
        true
    }

    /// Revokes the attempt's network right, as the reaper does (tests).
    pub fn revoke(&self) {
        if let Some(a) = self.state().access.as_ref() {
            a.revocation().revoke();
        }
    }

    fn answer(&self, req: &HttpRequest) -> Result<HttpResponse, LoginError> {
        if !self.live() {
            return Err(LoginError::ServiceUnavailable {
                reason: "revoked".to_owned(),
            });
        }
        self.state().seen.requests += 1;
        let body = match req.url.as_str() {
            GLS => datacenters(),
            "https://gls.example.invalid/launcher.xml" => LAUNCHER.to_owned(),
            "https://gls.example.invalid/status/alpha" => {
                "<Status><loginservers>login.example.invalid:9001;</loginservers>\
                 <queueurls></queueurls></Status>"
                    .to_owned()
            }
            "https://auth.example.invalid/login" => login_ok(),
            _ => {
                return Ok(HttpResponse {
                    status_code: 404,
                    body: Vec::new(),
                });
            }
        };
        Ok(HttpResponse {
            status_code: 200,
            body: body.into_bytes(),
        })
    }

    /// A datagram from the client: the synthetic login server's answers (module docs).
    fn client_sent(&self, datagram: &[u8], to: SocketAddrV4) {
        let mut st = self.state();
        let st = &mut *st;
        if to == OBSERVED_WORLD_SERVER {
            if st.observed {
                observed_world_answer(st, datagram);
            }
            return;
        }
        if to != LOGIN_SERVER {
            return;
        }
        if st.observed {
            observed_answer(st, datagram);
            return;
        }
        let first = decode_client_datagram(datagram, true).ok();
        if first
            .as_ref()
            .is_some_and(|p| p.opt_headers.session_auth.is_some())
        {
            let server = Server::new();
            st.deliver(server.challenge());
            st.deliver(server.control(OptionalHeaders {
                cumulative_ack: Some(0),
                ..OptionalHeaders::default()
            }));
            let mut server = server;
            if let Some(d) = server.event(0x12F, &[]) {
                st.deliver(d);
            }
            st.server = Some(server);
            return;
        }
        let Ok(pkt) = decode_client_datagram(datagram, false) else {
            return;
        };
        if pkt.opt_headers.disconnect {
            st.seen.disconnects += 1;
        }
        let Some(mut server) = st.server.take() else {
            return;
        };
        let acked = pkt.header.seq_num;
        let reliable = !pkt.fragments.is_empty();
        for f in pkt.fragments {
            let Some(op) = f.header.blob_header_word else {
                continue;
            };
            st.saw_blob(op);
            let answers: Vec<Option<Vec<u8>>> = match op {
                0x12C => {
                    let ok = VersionStatusMessage {
                        status: 1,
                        server_version_info: "ok".to_owned(),
                    }
                    .encode()
                    .unwrap_or_default();
                    vec![
                        server.blob(0x12D, &encode_event_envelope(&ok)),
                        server.event(0x6C, &character_list()),
                    ]
                }
                0x6F => vec![
                    server.event(0x90, &session_ready()),
                    // §4.3.3 step 2: portal space before the placement.
                    server.event(EnterPortalSpace::EVENT_ID, &entry_portal_space()),
                    server.event(PositionUpdate::EVENT_ID, &placement()),
                    // `login_finished`, the server gate of the portal exit
                    // (`official-messages-world.md` §4.3.3).
                    server.event(0x7C, &[0]),
                ],
                0x30 => vec![
                    server.event(0x3000_001B, &[0]),
                    server.event(0x6C, &character_list()),
                ],
                _ => Vec::new(),
            };
            for d in answers.into_iter().flatten() {
                st.deliver(d);
            }
        }
        if reliable {
            // Acknowledged at once, so nothing is sent twice.
            let ack = server.control(OptionalHeaders {
                cumulative_ack: Some(acked),
                ..OptionalHeaders::default()
            });
            st.deliver(ack);
        }
        st.server = Some(server);
    }
}

/// The client's outgoing dictionary, with which the synthetic server reads the client's
/// compressed datagrams (`official-session.md` §4.1.2); built once.
pub fn client_to_server_codec() -> &'static BitTreeTable {
    static CODEC: std::sync::OnceLock<BitTreeTable> = std::sync::OnceLock::new();
    CODEC.get_or_init(|| BitTreeTable::synthetic_dictionary(OUTGOING_UUID))
}

/// A synthetic dictionary that compresses a `DatagramHeader` with few payload bytes below its own
/// size: `0` is four zero bytes (as in [`BitTreeTable::synthetic_dictionary`]), `10` one zero byte,
/// and `11` then the eight bits of any other byte, LSB first. Read back through
/// [`BitTreeTable::from_dat_bytes`], as the install's records are.
#[must_use]
pub fn short_header_dictionary(codec_uuid: [u8; 16]) -> BitTreeTable {
    /// One leaf of the record's body (`official-session.md` §4.1.3).
    fn leaf(body: &mut Vec<u8>, symbol: &[u8], bits: u8, code: u16) {
        body.push(1);
        body.push(u8::try_from(symbol.len()).unwrap_or(0));
        body.extend_from_slice(symbol);
        body.push(bits);
        let code = code.to_le_bytes();
        body.extend_from_slice(code.get(..usize::from(bits >> 3) + 1).unwrap_or(&code));
    }
    // Node indices: leaves 0 (four zeros), 1 (one zero), `1 + b` (byte `b`), then the root, its
    // `1` child and the eight levels of the other bytes. The record's child links are 1-based.
    const LEAVES: u16 = 257;
    const ROOT_1BASED: u16 = LEAVES + 1;
    let bytes_root_1based = ROOT_1BASED + 2;
    let mut levels: Vec<[u16; 2]> = vec![[0, 0]];
    for b in 1..=255u8 {
        let leaf_1based = u16::from(b) + 2;
        let mut at = 0usize;
        for bit_at in 0..8u8 {
            let bit = usize::from((b >> bit_at) & 1);
            let next = levels
                .get(at)
                .and_then(|n| n.get(bit))
                .copied()
                .unwrap_or(0);
            let child = if bit_at == 7 {
                leaf_1based
            } else if next != 0 {
                next
            } else {
                levels.push([0, 0]);
                bytes_root_1based + u16::try_from(levels.len() - 1).unwrap_or(0)
            };
            if let Some(slot) = levels.get_mut(at).and_then(|n| n.get_mut(bit)) {
                *slot = child;
            }
            at = usize::from(child.saturating_sub(bytes_root_1based));
        }
    }
    let nodes = usize::from(LEAVES) + 2 + levels.len();
    let mut body = u32::try_from(nodes).unwrap_or(0).to_le_bytes().to_vec();
    leaf(&mut body, &[0, 0, 0, 0], 1, 0b0);
    leaf(&mut body, &[0], 2, 0b01);
    for b in 1..=255u8 {
        leaf(&mut body, &[b], 10, (u16::from(b) << 2) | 0b11);
    }
    let mut branch = |children: [u16; 2]| {
        body.push(0);
        body.extend_from_slice(&children[0].to_le_bytes());
        body.extend_from_slice(&children[1].to_le_bytes());
    };
    branch([1, ROOT_1BASED + 1]);
    branch([2, bytes_root_1based]);
    for children in levels {
        branch(children);
    }
    // The format's identifying fields come from the synthetic dictionary's own record.
    let plain = BitTreeTable::synthetic_dictionary(codec_uuid);
    let mut record = plain.to_dat_bytes().get(..40).unwrap_or_default().to_vec();
    record.extend_from_slice(&u32::try_from(body.len()).unwrap_or(0).to_le_bytes());
    record.extend_from_slice(&body);
    BitTreeTable::from_dat_bytes(&record).unwrap_or(plain)
}

/// A datagram the client sent, decoded as a synthetic server reads it: compressed with the
/// outgoing dictionary or not (`official-session.md` §4.1); the checksum is verified when
/// `verify` (unmasked packets only, as [`decode_datagram`] does).
///
/// # Errors
///
/// What [`decode_datagram`] returns.
pub fn decode_client_datagram(
    datagram: &[u8],
    verify: bool,
) -> Result<crate::session::DecodedPacket, SessionError> {
    decode_datagram(datagram, false, Some(client_to_server_codec()), verify)
}

/// A primary `ConnectChallengeHeader` datagram of a synthetic server (`official-session.md`
/// §6.3): header `recId` `incoming_rec_id` (the client's incoming recipient) and bytes
/// 18..19 `peer_word`; the payload names `outgoing_rec_id`, the two seeds and the incoming
/// dictionary `incoming_uuid` (on-disk form, converted field-wise here).
#[must_use]
pub fn challenge_datagram(
    incoming_rec_id: u16,
    outgoing_rec_id: u16,
    peer_word: u16,
    incoming_seed: u32,
    outgoing_seed: u32,
    incoming_uuid: [u8; 16],
) -> Vec<u8> {
    let opt = OptionalHeaders {
        connect_challenge: Some(ConnectChallengeHeader {
            clock_candidate: 1.0,
            cookie: [1, 2, 3, 4, 5, 6, 7, 8],
            reserved_16: [0, 0],
            outgoing_rec_id,
            incoming_seed,
            outgoing_seed,
            incoming_codec_guid: guid_fields_swapped(incoming_uuid),
        }),
        ..OptionalHeaders::default()
    };
    encode_datagram(
        DatagramHeader {
            rec_id: incoming_rec_id,
            peer_word,
            ..DatagramHeader::default()
        },
        &opt,
        &[],
        None,
        None,
    )
    .unwrap_or_default()
}

/// What the observed-order script answers to one client datagram (module docs).
fn observed_answer(st: &mut State, datagram: &[u8]) {
    let Ok(pkt) = decode_client_datagram(datagram, false) else {
        return;
    };
    if pkt.opt_headers.session_auth.is_some() {
        let server = Server::with_codec(st.short_acks);
        st.deliver(server.challenge());
        st.server = Some(server);
        return;
    }
    if pkt.opt_headers.disconnect {
        st.seen.disconnects += 1;
    }
    if pkt.opt_headers.sequence_nak.is_some() {
        st.seen.naks += 1;
    }
    let Some(mut server) = st.server.take() else {
        return;
    };
    if pkt.opt_headers.connect_accept.is_some() && !server.greeted {
        server.greeted = true;
        for (i, op) in [
            0x149,
            0xAA,
            0x149,
            0x149,
            0x149,
            0x149,
            0x149,
            0x3000_0040,
            0xAA,
            0xAA,
            0x43,
        ]
        .into_iter()
        .enumerate()
        {
            let d = server.observed_blob(op, &opaque_body(op), i == 0);
            if let Some(d) = d {
                st.deliver(d);
            }
        }
        if let Some(d) = server.observed_blob(0xDD, &install_request(), false) {
            st.deliver(d);
        }
    }
    if pkt.header.flags & crate::session::FLAG_MASKED_CHECKSUM != 0 {
        if st.seen.sequenced.len() >= MAX_KEPT {
            st.seen.sequenced.pop_front();
        }
        st.seen
            .sequenced
            .push_back((pkt.header.seq_num, pkt.header.flags));
    }
    let acked = pkt.header.seq_num;
    let has_fragments = !pkt.fragments.is_empty();
    if has_fragments
        && st.short_acks
        && let Some(d) = server.short_ack(acked)
    {
        if st.seen.short_acks.len() >= MAX_KEPT {
            st.seen.short_acks.pop_front();
        }
        st.seen.short_acks.push_back(d.len());
        st.deliver(d);
    }
    for f in pkt.fragments {
        if st.seen.fragment_headers.len() >= MAX_KEPT {
            st.seen.fragment_headers.pop_front();
        }
        st.seen.fragment_headers.push_back(f.header.clone());
        let Ok(Some(blob)) = st.reassembler.ingest(SERVER_REC_ID, f, 0.0) else {
            continue;
        };
        st.saw_blob(blob.blob_header_word);
        let answers: Vec<Option<Vec<u8>>> = match blob.blob_header_word {
            0xEC => {
                st.seen.advertisements.push(blob.payload);
                vec![
                    server.observed_blob(0x149, &opaque_body(0x149), false),
                    server.observed_blob(0xF5, &[0], false),
                ]
            }
            0xF5 => {
                st.seen.completions.push(blob.payload);
                vec![
                    server.observed_blob(0xFE, &opaque_body(0xFE), false),
                    server.observed_blob(0xAA, &opaque_body(0xAA), false),
                    server.observed_blob(0x54, &opaque_body(0x54), false),
                    server.observed_blob(0x6C, &character_list(), false),
                ]
            }
            0x6F => {
                st.seen.selections.push(blob.payload);
                vec![
                    server.observed_blob(0x149, &opaque_body(0x149), false),
                    server.blob(0x30, &opaque_body(0x30)),
                    server.observed_blob(0x30, &opaque_body(0x30), false),
                    server.observed_blob(0x30, &opaque_body(0x30), false),
                    Some(server.control(OptionalHeaders {
                        world_referral: Some(crate::session::WorldReferralHeader {
                            cookie: OBSERVED_COOKIE,
                            target_addr: OBSERVED_WORLD_SERVER,
                            metadata: [0; 4],
                        }),
                        ..OptionalHeaders::default()
                    })),
                ]
            }
            _ => Vec::new(),
        };
        for d in answers.into_iter().flatten() {
            st.deliver(d);
        }
    }
    if has_fragments && !st.short_acks {
        let ack = server.control(OptionalHeaders {
            cumulative_ack: Some(acked),
            ..OptionalHeaders::default()
        });
        st.deliver(ack);
    }
    st.server = Some(server);
}

/// The world server's answers in the observed-order script (§9.3.1, §9.3.8): its handshake after
/// the `ReferralAuth`, then the observed complete messages up to `0x90` and the checked messages
/// after it, and nothing else: no readiness (OPEN-WORLD-9), so an entry stops at that stage.
fn observed_world_answer(st: &mut State, datagram: &[u8]) {
    let Ok(pkt) = decode_client_datagram(datagram, false) else {
        return;
    };
    if pkt.opt_headers.referral_auth.is_some() {
        st.seen.referral_auths += 1;
        let server = Server::with_codec(st.short_acks);
        st.deliver_world(server.challenge());
        st.world_server = Some(server);
        return;
    }
    if pkt.opt_headers.disconnect {
        st.seen.disconnects += 1;
    }
    let Some(mut server) = st.world_server.take() else {
        return;
    };
    if pkt.opt_headers.connect_accept.is_some() && !server.greeted {
        server.greeted = true;
        for messages in observed_world_stream() {
            if let Some(d) = server.observed_datagram(&messages) {
                st.deliver_world(d);
            }
        }
        // A scripted network stands in for the controlled-target resolver that has no source
        // (WORLD-9): the stream's own `0xCE`s go to the default recipient, which is never the player
        // (`official-session.md` §9.3.9, §9.3.12 Question 1), so it addresses the player's placement
        // to the player explicitly.
        if st.placement {
            let scripted = [observed(
                1,
                PositionUpdate::EVENT_ID,
                placement(),
                ObservedForm::To(PLAYER),
            )];
            if let Some(d) = server.observed_datagram(&scripted) {
                st.deliver_world(d);
            }
        }
    }
    for f in pkt.fragments {
        if let Some(op) = f.header.blob_header_word {
            st.seen.world_blobs.push(op);
        }
    }
    st.world_server = Some(server);
}

/// How one message of the observed world stream is framed (§9.3.9, Vector 18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedForm {
    /// The default recipient.
    Plain,
    /// An explicit recipient.
    To(u64),
    /// An extension word with bit `0x0001`, and the 28-byte attachment after the padded header.
    Attached,
    /// `blobFlags` `0x04`: unchanged framing.
    Bit04,
    /// `OmitChunkInfo`: no opcode, no length and no data.
    EmptyControl,
}

/// One complete message of the observed world stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedMessage {
    /// Its delivery queue.
    pub queue: u8,
    /// Its opcode.
    pub opcode: u32,
    /// Its complete data, the one shared byte included.
    pub data: Vec<u8>,
    /// Its framing.
    pub form: ObservedForm,
}

fn observed(queue: u8, opcode: u32, data: Vec<u8>, form: ObservedForm) -> ObservedMessage {
    ObservedMessage {
        queue,
        opcode,
        data,
        form,
    }
}

/// The compact header of `m`, with its pad and attachment, and its data: the two checksum buffers
/// of §9.3.9. `word` is its ordering word and blob ID.
#[must_use]
pub fn observed_buffers(m: &ObservedMessage, word: u16) -> (Vec<u8>, Vec<u8>) {
    if m.form == ObservedForm::EmptyControl {
        return (vec![m.queue, 0xC0], Vec::new());
    }
    let recipient = match m.form {
        ObservedForm::To(id) => Some(id),
        _ => None,
    };
    let frags =
        split_blob(m.queue, word, word, 0, recipient, m.opcode, &m.data).unwrap_or_default();
    let mut header = Vec::new();
    if let Some(mut f) = frags.into_iter().next() {
        if m.form == ObservedForm::Attached {
            f.header.attachment = Some(crate::session::FragmentAttachment {
                words64: [0x0102_0304, 0x0506_0708, 0x090A_0B0C],
                word32: 0x0D0E_0F10,
            });
        }
        let _ = f.header.encode(&mut header);
        if m.form == ObservedForm::Bit04
            && let Some(flags) = header.get_mut(1)
        {
            *flags |= 0x04;
        }
    }
    (header, m.data.clone())
}

/// The complete datagram of `buffers` (§4.4.2: each buffer checksummed by itself), without the wire
/// compression when `codec` is `None`.
#[must_use]
pub fn observed_raw_datagram(
    rec_id: u16,
    seq: u32,
    mask: u32,
    buffers: &[(Vec<u8>, Vec<u8>)],
    codec: Option<&dyn BitTreeCodec>,
) -> Vec<u8> {
    let mut payload = Vec::new();
    let mut sum = 0u32;
    for (header, data) in buffers {
        sum = sum
            .wrapping_add(crate::session::compute_datagram_checksum(header))
            .wrapping_add(crate::session::compute_datagram_checksum(data));
        payload.extend_from_slice(header);
        payload.extend_from_slice(data);
    }
    let mut hdr = DatagramHeader {
        rec_id,
        seq_num: seq,
        pak_size: u16::try_from(payload.len()).unwrap_or(u16::MAX),
        flags: crate::session::FLAG_MASKED_CHECKSUM | crate::session::FLAG_HAS_FRAGMENTED_BLOBS,
        ..DatagramHeader::default()
    };
    hdr.checksum = crate::session::combine_packet_checksum(hdr, sum, Some(mask)).unwrap_or(0);
    let wire = hdr.to_wire_bytes();
    let mut out = Vec::new();
    match codec {
        Some(codec) => {
            let mut plain = wire.get(2..).unwrap_or_default().to_vec();
            plain.extend_from_slice(&payload);
            out.extend_from_slice(&rec_id.to_be_bytes());
            let _ = codec.encode(&plain, &mut out, crate::session::MAX_DATAGRAM_BUF - 2);
        }
        None => {
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(&wire);
            out.extend_from_slice(&payload);
        }
    }
    out
}

/// The world server's datagrams in the observed-order script, each its messages, in the order of
/// `official-session.md` §9.3.9: the observed messages up to `0x90`; the first `0x4C`; the first
/// bit-`0x04` datagram on queue 4, then the attached queue-1 `0x19A` with its siblings on queues 1,
/// 3 and 4; the first `0xCC` in its default-recipient form; and at the end a queue-3 `0xCE`. No
/// readiness the client could act on: `0x7C` and `0x0D` are only counted. The local player's exact
/// placement is its own `0xCE`.
#[must_use]
pub fn observed_world_stream() -> Vec<Vec<ObservedMessage>> {
    use ObservedForm::{Attached, Bit04, EmptyControl, Plain, To};
    let one = |queue, opcode, data, form| vec![observed(queue, opcode, data, form)];
    vec![
        one(1, 0x149, opaque_body(0x149), Plain),
        one(2, 0x88, opaque_body(0x88), Plain),
        one(4, 0x92, opaque_body(0x92), Plain),
        one(4, 0xA8, opaque_body(0xA8), Plain),
        one(4, 0x89, OBSERVED_APPEARANCE.to_vec(), Plain),
        one(1, 0x90, session_ready(), Plain),
        one(2, 0x4C, observed_batch(), To(NPC)),
        one(4, 0x89, OBSERVED_APPEARANCE.to_vec(), To(NPC)),
        one(4, 0x40, vec![0x00], Bit04),
        vec![
            observed(1, 0x19A, opaque_body(0x19A), Attached),
            observed(1, 0x30, opaque_body(0x30), Plain),
            observed(3, PositionUpdate::EVENT_ID, npc_position(), Plain),
            observed(4, 0x60, opaque_body(0x60), Plain),
            observed(4, 0x92, opaque_body(0x92), Plain),
        ],
        one(1, 0x40, Vec::new(), EmptyControl),
        one(1, PositionUpdate::EVENT_ID, placement(), Plain),
        one(1, EnterPortalSpace::EVENT_ID, entry_portal_space(), Plain),
        one(1, 0x149, opaque_body(0x149), Plain),
        one(1, 0x7C, vec![0x00], Plain),
        one(1, 0x0D, vec![0x00], Plain),
        one(3, PositionUpdate::EVENT_ID, placement(), Plain),
    ]
}

/// Vector 16's complete `0x89` data (`official-session.md` §10.2).
#[must_use]
pub const fn observed_appearance() -> &'static [u8] {
    &OBSERVED_APPEARANCE
}

/// A `0x4C` with an embedded `0x98` and `0x52`, each with its own one shared byte (§9.3.8).
fn observed_batch() -> Vec<u8> {
    let create = EntityCreate {
        template_did: NPC_TEMPLATE,
        entity_id: NPC,
    }
    .to_bytes()
    .unwrap_or_default();
    let props = PropertyTableUpdate {
        scope: crate::world::PropertyScope::Entity,
        properties: EncodedPropertyTable::new(),
    };
    let mut w = crate::world::WireWriter::new();
    let props = props
        .encode(&mut w)
        .map(|()| w.into_vec())
        .unwrap_or_default();
    SubEventBatch {
        event_id: SubEventBatch::EVENT_ID_SUB_EVENT,
        frames: vec![
            SubEventFrame::Event {
                sub_event_id: EntityCreate::EVENT_ID,
                payload: create,
            },
            SubEventFrame::Event {
                sub_event_id: PropertyTableUpdate::EVENT_ID_ENTITY,
                payload: props,
            },
        ],
        entry_batch_tail_flag: None,
    }
    .to_bytes()
    .unwrap_or_default()
}

/// A synthetic body of a message Tessera skips: one zero flags byte and opaque bytes whose
/// first is not zero, so a false read of a second base byte would show.
fn opaque_body(opcode: u32) -> Vec<u8> {
    let tag = opcode.to_le_bytes();
    vec![0x00, 0xA5, tag[0], tag[1], tag[2], tag[3]]
}

/// The synthetic `0xDD` (§10.2 Vector 11).
fn install_request() -> Vec<u8> {
    let mut out = vec![0x00];
    out.extend_from_slice(&0x0102_0304u32.to_le_bytes());
    out.extend_from_slice(&SYNTHETIC_NAME_RULE.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

/// The server's dictionary the observed-order script compresses with; built once.
fn server_to_client_codec() -> &'static BitTreeTable {
    static CODEC: std::sync::OnceLock<BitTreeTable> = std::sync::OnceLock::new();
    CODEC.get_or_init(|| BitTreeTable::synthetic_dictionary(INCOMING_UUID))
}

/// That dictionary in the [`SyntheticService::short_acks`] mode; built once.
fn short_server_to_client_codec() -> &'static BitTreeTable {
    static CODEC: std::sync::OnceLock<BitTreeTable> = std::sync::OnceLock::new();
    CODEC.get_or_init(|| short_header_dictionary(INCOMING_UUID))
}

/// The synthetic login server's connection state.
#[derive(Debug)]
struct Server {
    isaac: IsaacCipher,
    seq: u32,
    /// The observed-order script's first blobs went out.
    greeted: bool,
    /// What it compresses with.
    codec: &'static BitTreeTable,
}

impl Server {
    fn new() -> Self {
        Self {
            isaac: IsaacCipher::from_seed(SEED),
            seq: 1,
            greeted: false,
            codec: server_to_client_codec(),
        }
    }

    /// A server that compresses with [`short_server_to_client_codec`] when `short`.
    fn with_codec(short: bool) -> Self {
        let mut server = Self::new();
        if short {
            server.codec = short_server_to_client_codec();
        }
        server
    }

    /// The next sequenced packet, compressed, with nothing but the Ack of `acked`
    /// (`official-session.md` §4.1.2: an Ack piggybacked on a sequenced packet).
    fn short_ack(&mut self, acked: u32) -> Option<Vec<u8>> {
        self.seq += 1;
        let mask = self.isaac.next_mask();
        let codec: &dyn BitTreeCodec = self.codec;
        encode_datagram(
            DatagramHeader {
                rec_id: REC_ID,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders {
                cumulative_ack: Some(acked),
                ..OptionalHeaders::default()
            },
            &[],
            Some(mask),
            Some(codec),
        )
        .ok()
    }

    /// A blob of the observed-order script: queue 1, the default recipient, compressed with
    /// the incoming dictionary; `clock` adds the first packet's ClockSync (§6.4.1).
    fn observed_blob(&mut self, opcode: u32, data: &[u8], clock: bool) -> Option<Vec<u8>> {
        self.seq += 1;
        let frags = split_blob(1, 0xFFFF, 0, 0, None, opcode, data).ok()?;
        let mask = self.isaac.next_mask();
        let codec: &dyn BitTreeCodec = self.codec;
        encode_datagram(
            DatagramHeader {
                rec_id: REC_ID,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders {
                clock_sync: clock.then_some(1000.0),
                ..OptionalHeaders::default()
            },
            &frags,
            Some(mask),
            Some(codec),
        )
        .ok()
    }

    /// One datagram of the world connection in the observed-order script (§9.3.9): each message in
    /// its own form, verified by the client with the same dictionaries and masks as any other.
    fn observed_datagram(&mut self, messages: &[ObservedMessage]) -> Option<Vec<u8>> {
        self.seq += 1;
        let word = u16::try_from(self.seq & 0xFFFF).unwrap_or(0);
        let buffers: Vec<(Vec<u8>, Vec<u8>)> =
            messages.iter().map(|m| observed_buffers(m, word)).collect();
        let mask = self.isaac.next_mask();
        let codec: &dyn BitTreeCodec = self.codec;
        Some(observed_raw_datagram(
            REC_ID,
            self.seq,
            mask,
            &buffers,
            Some(codec),
        ))
    }

    fn challenge(&self) -> Vec<u8> {
        challenge_datagram(
            REC_ID,
            SERVER_REC_ID,
            PEER_WORD,
            SEED,
            0xCAFE_1234,
            INCOMING_UUID,
        )
    }

    fn control(&self, opt: OptionalHeaders) -> Vec<u8> {
        encode_datagram(
            DatagramHeader {
                rec_id: REC_ID,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &opt,
            &[],
            None,
            None,
        )
        .unwrap_or_default()
    }

    fn blob(&mut self, opcode: u32, envelope: &[u8]) -> Option<Vec<u8>> {
        self.blob_to(opcode, None, envelope)
    }

    fn blob_to(&mut self, opcode: u32, recipient: Option<u64>, envelope: &[u8]) -> Option<Vec<u8>> {
        self.seq += 1;
        let word = u16::try_from(self.seq & 0xFFFF).unwrap_or(0);
        let frags = split_blob(2, word, word, 0, recipient, opcode, envelope).ok()?;
        let mask = self.isaac.next_mask();
        encode_datagram(
            DatagramHeader {
                rec_id: REC_ID,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            &frags,
            Some(mask),
            None,
        )
        .ok()
    }

    /// An event of the historical script. `0x6C`, `0x90`, `0x6D`, `0xB3` and the checked world
    /// family have one shared flags byte, which their payloads already start with
    /// (`official-session.md` §9.3.8).
    fn event(&mut self, opcode: u32, inner: &[u8]) -> Option<Vec<u8>> {
        if matches!(opcode, 0x6C | 0x90 | 0x6D | 0xB3) || crate::world::one_shared_prefix(opcode) {
            return self.blob(opcode, inner);
        }
        self.blob(opcode, &encode_event_envelope(inner))
    }
}

/// The network factory over a [`SyntheticService`].
#[derive(Debug)]
pub struct SyntheticFactory(SyntheticService);

impl NetFactory for SyntheticFactory {
    type Net = SyntheticNet;

    fn make(&mut self, access: NetworkAccess) -> SyntheticNet {
        // The synthetic network keeps the attempt's own right and follows its liveness (the
        // reaper's revocation and Withdraw's `revoke_all`).
        self.0.state().access = Some(access);
        SyntheticNet(self.0.clone())
    }
}

/// The synthetic network of one attempt.
#[derive(Debug)]
pub struct SyntheticNet(SyntheticService);

/// An HTTP client of the synthetic service (the ticket refresh's).
#[derive(Debug)]
struct SyntheticHttp(SyntheticService);

impl HttpClient for SyntheticHttp {
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        self.0.answer(request)
    }
}

/// The synthetic socket.
#[derive(Debug)]
pub struct SyntheticUdp(SyntheticService);

fn revoked() -> SessionError {
    SessionError::AccessRevoked
}

impl UdpDatagramTransport for SyntheticUdp {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        Ok(LOCAL)
    }

    fn send_to(&mut self, buf: &[u8], target: SocketAddrV4) -> Result<usize, SessionError> {
        if !self.0.live() {
            return Err(revoked());
        }
        self.0.client_sent(buf, target);
        Ok(buf.len())
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        if !self.0.live() {
            return Err(revoked());
        }
        let next = self.0.state().inbound.pop_front();
        Ok(next.and_then(|(d, src)| {
            let n = d.len().min(buf.len());
            buf.get_mut(..n)?.copy_from_slice(d.get(..n)?);
            Some((n, src))
        }))
    }
}

impl ConnectorNet for SyntheticNet {
    type Udp = SyntheticUdp;

    fn readiness_specified(&self) -> bool {
        self.0.state().readiness
    }

    fn placement_scripted(&self) -> bool {
        self.0.state().placement
    }

    fn chat_codec_specified(&self) -> bool {
        self.0.state().readiness
    }

    fn re_entry_allowed(&self) -> bool {
        self.0.state().re_entry
    }

    fn http(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        self.0.answer(request)
    }

    fn background_http(&self) -> SendHttpClient {
        Box::new(SyntheticHttp(self.0.clone()))
    }

    fn is_live(&self) -> bool {
        self.0.live()
    }

    fn resolve(&mut self, host: &str, port: u16) -> Result<SocketAddrV4, SessionError> {
        if !self.0.live() {
            return Err(revoked());
        }
        match (host, port) {
            ("login.example.invalid", 9001) => Ok(LOGIN_SERVER),
            _ => Err(SessionError::Io("unknown synthetic name".to_owned())),
        }
    }

    fn local_ipv4_toward(&mut self, _remote: SocketAddrV4) -> Option<Ipv4Addr> {
        self.0.live().then_some(*LOCAL.ip())
    }

    fn bind_udp(&mut self, _plan: BindPlan) -> Result<SyntheticUdp, SessionError> {
        if !self.0.live() {
            return Err(revoked());
        }
        self.0.state().seen.binds += 1;
        Ok(SyntheticUdp(self.0.clone()))
    }

    fn chat_binder(
        &self,
        _first_local: Option<Ipv4Addr>,
    ) -> Option<Box<dyn FnMut() -> Result<SyntheticUdp, crate::play::PlayError> + Send>> {
        None
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The synthetic records: a DID map naming two synthetic compression dictionaries.
#[derive(Debug)]
struct Records(BTreeMap<u32, Vec<u8>>);

impl InstallData for Records {
    fn record(&self, did: u32, max_bytes: usize) -> Option<Vec<u8>> {
        self.0.get(&did).filter(|r| r.len() <= max_bytes).cloned()
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        DICTIONARY.iter().find(|(p, _)| *p == pid).map(|(_, t)| *t)
    }

    fn selected_language_key(&self) -> Result<u32, InstallUnsupported> {
        Ok(SYNTHETIC_LANGUAGE_KEY)
    }

    fn check_name_table(&self) -> Result<(), InstallUnsupported> {
        Ok(())
    }

    fn name_rule_languages(&self, rule_key: u32) -> Result<Vec<u32>, InstallUnsupported> {
        if rule_key == SYNTHETIC_NAME_RULE {
            Ok(vec![SYNTHETIC_LANGUAGE_KEY])
        } else {
            Err(InstallUnsupported::NameRuleMissing)
        }
    }

    fn archive_inventory(&self) -> Result<Vec<AdvertisedArchive>, InstallUnsupported> {
        Ok(synthetic_inventory())
    }
}

/// A DID map record (`official-session.md` §4.1.3): one group of `(key, target)` pairs with
/// labels.
fn did_map(self_did: u32, entries: &[(u32, u32)]) -> Vec<u8> {
    let n = u8::try_from(entries.len()).unwrap_or(u8::MAX);
    let mut out = self_did.to_le_bytes().to_vec();
    out.push(0);
    out.push(n);
    for (k, t) in entries {
        out.extend_from_slice(&k.to_le_bytes());
        out.extend_from_slice(&t.to_le_bytes());
    }
    out.push(0);
    out.push(n);
    for (k, _) in entries {
        out.extend_from_slice(&k.to_le_bytes());
        out.push(1);
        out.push(b'k');
    }
    out
}

/// The synthetic records, with the incoming dictionary of [`SyntheticService::short_acks`] when
/// `short`.
fn records(short: bool) -> Records {
    let mut m = BTreeMap::new();
    m.insert(0x2800_0000, did_map(0x2800_0000, &[(3, 0x2800_0010)]));
    m.insert(
        0x2800_0010,
        did_map(
            0x2800_0010,
            &[(0x1000_0002, 0x0E01_0000), (0x1000_0001, 0x0E01_0001)],
        ),
    );
    m.insert(
        0x0E01_0000,
        BitTreeTable::synthetic_dictionary(OUTGOING_UUID).to_dat_bytes(),
    );
    let incoming = if short {
        short_header_dictionary(INCOMING_UUID)
    } else {
        BitTreeTable::synthetic_dictionary(INCOMING_UUID)
    };
    m.insert(0x0E01_0001, incoming.to_dat_bytes());
    Records(m)
}

fn datacenters() -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="{SOAP_ENVELOPE_NAMESPACE}"><soap:Body>
<GetDatacentersResponse xmlns="{GLS_NAMESPACE}"><GetDatacentersResult><Datacenter>
<Name>LOTRO</Name><AuthServer>https://auth.example.invalid/login</AuthServer><PatchServer>patch.example.invalid:1</PatchServer>
<LauncherConfigurationServer>https://gls.example.invalid/launcher.xml</LauncherConfigurationServer>
<Worlds>
<World><Name>Alpha</Name><LoginServerUrl>(invalid service specified)</LoginServerUrl><ChatServerUrl></ChatServerUrl><StatusServerUrl>https://gls.example.invalid/status/alpha</StatusServerUrl><Order>1</Order></World>
</Worlds></Datacenter></GetDatacentersResult></GetDatacentersResponse></soap:Body></soap:Envelope>"#
    )
}

const LAUNCHER: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration><appSettings>
<add key="GameClient.WIN32.ArgTemplate" value="-a {SUBSCRIPTION}" />
<add key="WorldQueue.LoginQueue.URL" value="https://gls.example.invalid/queue" />
<add key="WorldQueue.TakeANumber.Parameters" value="command=TakeANumber&amp;subscription={0}&amp;ticket={1}&amp;ticket_type=GLS&amp;queue_url={2}" />
<add key="WorldQueue.LeaveQueue.Parameters" value="command=LeaveQueue&amp;subscription={0}&amp;context={1}&amp;ticket_type=GLS&amp;queue_url={2}" />
</appSettings></configuration>"#;

fn login_ok() -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="{SOAP_ENVELOPE_NAMESPACE}"><soap:Body>
<LoginAccountResponse xmlns="{GLS_NAMESPACE}"><LoginAccountResult>
<Subscriptions><GameSubscription><Game>LOTRO</Game><Name>SUB0000000000000000000000</Name><ProductTokens><string>TokenBase</string></ProductTokens></GameSubscription></Subscriptions><Ticket>{TICKET}</Ticket>
</LoginAccountResult></LoginAccountResponse></soap:Body></soap:Envelope>"#
    )
}

/// The list's one entry, through the real encoder (`official-messages-world.md` §3.1.2): nonempty
/// public and private tables, Vector 16's appearance, and a nonempty account table.
#[must_use]
pub fn character_list() -> Vec<u8> {
    let mut public_props = EncodedPropertyTable::new();
    public_props.push(
        PID_AGENT_SPECIES,
        EncodedPropertyValue::Enum(CHARACTER_SPECIES),
    );
    public_props.push(PID_SEX, EncodedPropertyValue::Enum(CHARACTER_SEX));
    public_props.push(PID_AGENT_CLASS, EncodedPropertyValue::Enum(CHARACTER_CLASS));
    public_props.push(
        PID_CHARGEN_CHARACTER_LEVEL,
        EncodedPropertyValue::Int(CHARACTER_LEVEL),
    );
    let mut private_props = EncodedPropertyTable::new();
    private_props.push(
        PID_CHARGEN_CHARACTER_HAS_DATA,
        EncodedPropertyValue::Boolean(true),
    );
    let mut account_props = EncodedPropertyTable::new();
    account_props.push(
        PID_ACCOUNT_NUM_CHARACTERS_ALLOWED,
        EncodedPropertyValue::Int(3),
    );
    CharacterListResponse {
        characters: vec![CharacterSummaryEntry {
            character_id: 0x0200_0000_0000_0042,
            name: CHARACTER.to_owned(),
            status_or_world: 0,
            public_props,
            private_props,
            metadata_a: 0,
            visual_desc: VisualDescriptionUpdate::from_bytes(&OBSERVED_APPEARANCE)
                .map(|u| u.visual_desc)
                .unwrap_or_default(),
            metadata_b: 0,
            metadata_c: u32::MAX,
            metadata_d: 0,
            metadata_e: u32::MAX,
            timestamp_or_meta: 0,
        }],
        account_props,
    }
    .to_bytes()
    .unwrap_or_default()
}

/// The player's placement after the session start: `0xCE` with the teleport stamp 0
/// (`official-messages-play.md` §4.5 item 3), the client's first location report.
fn placement() -> Vec<u8> {
    PositionUpdate {
        entity_id: PLAYER,
        seq_byte: Some(0),
        velocity: [None, None, None],
        anim_state: None,
        placement: EntityPlacement::WorldWithHeading {
            position: WirePosition::from_components(1, 1, 2, 0, 0, PLAYER_AT, [1.0, 0.0, 0.0, 0.0]),
            heading_raw: 0,
        },
        interp_param_a: None,
        interp_param_b: None,
    }
    .to_bytes()
    .unwrap_or_default()
}

/// The local player's `0xCE` of the observed stream: its exact placement.
#[must_use]
pub fn player_position() -> Vec<u8> {
    placement()
}

/// The observed stream's other entity.
#[must_use]
pub const fn observed_npc() -> u64 {
    NPC
}

/// That entity's template.
#[must_use]
pub const fn observed_npc_template() -> u32 {
    NPC_TEMPLATE
}

/// That entity's `0xCE`.
#[must_use]
pub fn observed_npc_position() -> Vec<u8> {
    npc_position()
}

/// Where the observed stream's other entity is, in its `0xCE`.
fn npc_position() -> Vec<u8> {
    PositionUpdate {
        entity_id: NPC,
        seq_byte: None,
        velocity: [None, None, None],
        anim_state: None,
        placement: EntityPlacement::WorldWithHeading {
            position: WirePosition::from_components(1, 1, 2, 0, 0, PLAYER_AT, [1.0, 0.0, 0.0, 0.0]),
            heading_raw: 0,
        },
        interp_param_a: None,
        interp_param_b: None,
    }
    .to_bytes()
    .unwrap_or_default()
}

/// The local player's `0x90`.
fn session_ready() -> Vec<u8> {
    PlayerSessionReady {
        player_id: PLAYER,
        session_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .unwrap_or_default()
}

/// The properties of an entity: its display name (a literal text) and the NPC flag
/// (`official-messages-world.md` §5.2.4).
#[must_use]
pub fn entity_properties(name: &str, npc: bool) -> EncodedPropertyTable {
    let mut t = EncodedPropertyTable::new();
    t.push(
        PID_ENTITY_DISPLAY_NAME,
        EncodedPropertyValue::LocalizedTokenText(LocalizedTokenText::literal(name)),
    );
    t.push(PID_IS_NPC_AGENT, EncodedPropertyValue::Boolean(npc));
    t
}

/// A `0xCE` placing `entity` at `at` in the player's landblock with the quantized heading
/// `heading_raw` and the velocity `velocity` (`official-messages-world.md` §5.4).
#[must_use]
pub fn position(entity: u64, at: [f32; 3], heading_raw: u8, velocity: [f32; 3]) -> Vec<u8> {
    PositionUpdate {
        entity_id: entity,
        seq_byte: None,
        velocity: velocity.map(Some),
        anim_state: None,
        placement: EntityPlacement::WorldWithHeading {
            position: WirePosition::from_components(1, 1, 2, 0, 0, at, [1.0, 0.0, 0.0, 0.0]),
            heading_raw,
        },
        interp_param_a: None,
        interp_param_b: None,
    }
    .to_bytes()
    .unwrap_or_default()
}

/// An area-of-interest batch `0x0D` (`official-messages-world.md` §5.1) that creates
/// `entity` from `template` (`0x98`), gives it `properties` (`0x52`) and places it at `at`
/// (`0xCE`).
#[must_use]
pub fn entity_batch(
    entity: u64,
    template: u32,
    properties: &EncodedPropertyTable,
    at: [f32; 3],
) -> Vec<u8> {
    let create = EntityCreate {
        template_did: template,
        entity_id: entity,
    }
    .to_bytes()
    .unwrap_or_default();
    let props = PropertyTableUpdate {
        scope: crate::world::PropertyScope::Entity,
        properties: properties.clone(),
    };
    let mut w = crate::world::WireWriter::new();
    let props = props
        .encode(&mut w)
        .map(|()| w.into_vec())
        .unwrap_or_default();
    SubEventBatch {
        event_id: SubEventBatch::EVENT_ID_AOI_ENTER,
        frames: vec![
            SubEventFrame::Event {
                sub_event_id: EntityCreate::EVENT_ID,
                payload: create,
            },
            SubEventFrame::Event {
                sub_event_id: PropertyTableUpdate::EVENT_ID_ENTITY,
                payload: props,
            },
            SubEventFrame::Event {
                sub_event_id: PositionUpdate::EVENT_ID,
                payload: position(entity, at, 0, [0.0; 3]),
            },
        ],
        entry_batch_tail_flag: Some(1),
    }
    .to_bytes()
    .unwrap_or_default()
}

/// A world-server chat line `0x1C` (`official-messages-play.md` §6.5.1, event code
/// `0x0102000C`) in chat category `category` from `sender` (entity `sender_id`).
#[must_use]
pub fn chat_line(category: u32, sender: &str, sender_id: u64, text: &str) -> Vec<u8> {
    let mut properties = EncodedPropertyTable::new();
    properties.push(PID_CHAT_CATEGORY, EncodedPropertyValue::Enum(category));
    properties.push(
        PID_CHAT_TEXT,
        EncodedPropertyValue::LocalizedTokenText(LocalizedTokenText::literal(text)),
    );
    properties.push(
        PID_CHAT_SENDER_NAME,
        EncodedPropertyValue::LocalizedTokenText(LocalizedTokenText::literal(sender)),
    );
    properties.push(
        PID_CHAT_SENDER_ID,
        EncodedPropertyValue::InstanceId(sender_id),
    );
    UiPropertyEvent {
        event_code: UI_EVENT_CHAT_LINE,
        properties,
    }
    .to_bytes()
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use tessera_net::account::{AccountCommand, AccountEvent, WorldId};
    use tessera_net::{Command, Event, Session};

    use super::*;
    use crate::consent::{self, UserConfirmation, WarningText};
    use crate::credentials::Credentials;
    use crate::{CompatBackend, CompatConfig, LiveConnector};

    fn wait(s: &mut Session, what: &str, pred: impl Fn(&Event) -> bool) -> Event {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(e) = s.poll(Instant::now()).into_iter().find(|e| pred(e)) {
                return e;
            }
            assert!(Instant::now() < until, "no {what}: {:?}", s.state());
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// A session over the synthetic service, in the world as [`CHARACTER`].
    fn in_world(dir: &Path, service: &SyntheticService) -> Session {
        let (acc, write) = consent::accept(
            Some(dir),
            &WarningText::shipped(),
            UserConfirmation::confirmed_by_user(),
        );
        let _ = write.wait();
        let live = service.live_config(&dir.join("install")).expect("install");
        let connector = LiveConnector::with_factory(live, service.factory());
        let creds = Credentials::new(b"a", b"b").expect("creds");
        let backend =
            CompatBackend::with_connector(CompatConfig::new(acc.proof(), creds), connector)
                .expect("backend");
        let mut s = Session::new(backend);
        s.send(Instant::now(), Command::Connect).expect("connect");
        let list = wait(&mut s, "world list", |e| {
            matches!(e, Event::Account(AccountEvent::WorldList { .. }))
        });
        let Event::Account(AccountEvent::WorldList { worlds }) = list else {
            unreachable!("matched")
        };
        let world: WorldId = worlds.first().expect("a world").id;
        s.send(
            Instant::now(),
            Command::Account(AccountCommand::ChooseWorld { world }),
        )
        .expect("choose");
        wait(&mut s, "Connected", |e| {
            matches!(e, Event::Connected { .. })
        });
        s.send(
            Instant::now(),
            Command::EnterWorld {
                character: CHARACTER.to_owned(),
            },
        )
        .expect("enter");
        wait(&mut s, "WorldEntered", |e| {
            matches!(e, Event::WorldEntered { .. })
        });
        s
    }

    /// The synthetic network follows the attempt's right: once it is revoked in the world,
    /// nothing more is sent or bound, and the session ends.
    #[test]
    fn revoking_the_right_in_the_world_stops_every_send_at_once() {
        let _guard = crate::net_access::TEST_REVOKE_LOCK
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().expect("temp");
        let service = SyntheticService::new();
        let mut s = in_world(dir.path(), &service);
        service.revoke();
        let before = service.seen();
        // A loading signal and a logout request would each send a blob.
        let _ = s.send(Instant::now(), Command::WorldLoaded { generation: None });
        let _ = s.send(Instant::now(), Command::LeaveWorld);
        wait(&mut s, "the end", |e| {
            matches!(e, Event::Disconnected { .. })
        });
        let after = service.seen();
        assert_eq!(
            after.blobs, before.blobs,
            "nothing sent after the revocation"
        );
        assert_eq!(after.binds, before.binds, "no bind after the revocation");
        assert_eq!(
            after.disconnects, before.disconnects,
            "not even the goodbye"
        );
    }

    /// The world feed over the whole connector: an NPC is created at its place with its
    /// name, moves, speaks on Say and leaves (`official-messages-world.md` §5.1–§5.4,
    /// `official-messages-play.md` §6.5.1, §8).
    #[test]
    fn the_synthetic_world_spawns_moves_speaks_and_despawns_an_entity() {
        use tessera_net::{Body, ChatChannel, DataId, EntityId, EntityKind};
        let _guard = crate::net_access::TEST_REVOKE_LOCK
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().expect("temp");
        let service = SyntheticService::new();
        let mut s = in_world(dir.path(), &service);
        assert!(service.send_world_event(
            SubEventBatch::EVENT_ID_AOI_ENTER,
            Some(NPC),
            &entity_batch(
                NPC,
                NPC_TEMPLATE,
                &entity_properties(NPC_NAME, true),
                NPC_AT
            ),
        ));
        let created = wait(
            &mut s,
            "the NPC",
            |e| matches!(e, Event::EntityCreated { id, .. } if *id == EntityId(NPC)),
        );
        let Event::EntityCreated {
            kind,
            name,
            class,
            location,
            appearance,
            ..
        } = created
        else {
            unreachable!("matched")
        };
        assert_eq!(kind, EntityKind::NonPlayer);
        assert_eq!(name, NPC_NAME);
        assert_eq!(class, Some(DataId(NPC_TEMPLATE)));
        assert_eq!(appearance.body, Body::Object(DataId(NPC_TEMPLATE)));
        // Block (1, 2) of region 1: 160 m blocks (`coordinates.md`).
        assert!((location.x - (160.0 + 20.0)).abs() < 1e-3, "{location:?}");
        assert!((location.y - (320.0 + 40.0)).abs() < 1e-3, "{location:?}");
        assert!(service.send_world_event(
            PositionUpdate::EVENT_ID,
            None,
            &position(NPC, [25.0, 40.0, 6.0], 64, [2.5, 0.0, 0.0]),
        ));
        let moved = wait(
            &mut s,
            "the move",
            |e| matches!(e, Event::EntityMoved { id, .. } if *id == EntityId(NPC)),
        );
        let Event::EntityMoved { velocity, .. } = moved else {
            unreachable!("matched")
        };
        assert!((velocity.x - 2.5).abs() < 1e-6);
        assert!(service.send_world_event(
            crate::play::chat::OPCODE_UI_PROPERTY_EVENT,
            None,
            &chat_line(crate::play::chat::CATEGORY_SAY, NPC_NAME, NPC, "Well met."),
        ));
        let line = wait(&mut s, "the chat line", |e| {
            matches!(e, Event::ChatMessage { .. })
        });
        assert_eq!(
            line,
            Event::ChatMessage {
                channel: ChatChannel::Say,
                sender: Some(NPC_NAME.to_owned()),
                text: "Well met.".to_owned(),
            }
        );
        assert!(service.send_world_event(0x40, Some(NPC), &[0]));
        wait(
            &mut s,
            "the despawn",
            |e| matches!(e, Event::EntityDespawned { id } if *id == EntityId(NPC)),
        );
    }

    #[test]
    fn the_synthetic_service_caps_what_it_keeps() {
        let service = SyntheticService::new();
        let mut st = service.state();
        for i in 0..(MAX_KEPT + 10) {
            st.deliver(vec![0]);
            st.saw_blob(u32::try_from(i).unwrap_or(0));
        }
        assert_eq!(st.inbound.len(), MAX_KEPT);
        assert_eq!(st.seen.blobs.len(), MAX_KEPT);
        assert_eq!(st.seen.blobs.front(), Some(&10), "the oldest are dropped");
    }

    #[test]
    fn the_synthetic_service_walks_login_entry_and_return() {
        // No other test's `revoke_all` may end this run's right early.
        let _guard = crate::net_access::TEST_REVOKE_LOCK
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().expect("temp");
        let (acc, write) = consent::accept(
            Some(dir.path()),
            &WarningText::shipped(),
            UserConfirmation::confirmed_by_user(),
        );
        let _ = write.wait();
        let service = SyntheticService::new();
        let live = service
            .live_config(&dir.path().join("install"))
            .expect("install");
        let connector = LiveConnector::with_factory(live, service.factory());
        let creds = Credentials::new(b"a", b"b").expect("creds");
        let backend =
            CompatBackend::with_connector(CompatConfig::new(acc.proof(), creds), connector)
                .expect("backend");
        let mut s = Session::new(backend);
        s.send(Instant::now(), Command::Connect).expect("connect");
        let list = wait(&mut s, "world list", |e| {
            matches!(e, Event::Account(AccountEvent::WorldList { .. }))
        });
        let Event::Account(AccountEvent::WorldList { worlds }) = list else {
            unreachable!("matched")
        };
        let world: WorldId = worlds.first().expect("a world").id;
        s.send(
            Instant::now(),
            Command::Account(AccountCommand::ChooseWorld { world }),
        )
        .expect("choose");
        wait(&mut s, "Connected", |e| {
            matches!(e, Event::Connected { .. })
        });
        s.send(
            Instant::now(),
            Command::EnterWorld {
                character: CHARACTER.to_owned(),
            },
        )
        .expect("enter");
        wait(&mut s, "WorldEntered", |e| {
            matches!(e, Event::WorldEntered { .. })
        });
        s.send(Instant::now(), Command::WorldLoaded { generation: None })
            .expect("loaded");
        // `official-session.md` §9.3.9: a typed logout closes with one Disconnect and no RPC.
        s.send(Instant::now(), Command::LeaveWorld).expect("leave");
        wait(&mut s, "the close", |e| {
            matches!(
                e,
                Event::Disconnected {
                    reason: tessera_net::DisconnectReason::ClientRequested
                }
            )
        });
        let seen = service.seen();
        for op in [0x12C, 0x6F, 0xF4] {
            assert!(seen.blobs.contains(&op), "{op:#x} in {:x?}", seen.blobs);
        }
        assert!(!seen.blobs.contains(&0x30), "{:x?}", seen.blobs);
        assert_eq!(seen.disconnects, 1);
        assert_eq!(service.seen().binds, 1);
    }
}
