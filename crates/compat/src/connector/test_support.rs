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
//! | `0x30` (an RPC: the logout request of `LeaveWorld`) | a world teardown `0x3000001B` and the list `0x6C` again |
//! | `0xD0` (a missing-entity query) | nothing (counted) |
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
use crate::login::soap_encoder::{GLS_NAMESPACE, SOAP_ENVELOPE_NAMESPACE};
use crate::login::{HttpClient, HttpRequest, HttpResponse, LoginError};
use crate::net_access::NetworkAccess;
use crate::play::chat::{
    PID_CHAT_CATEGORY, PID_CHAT_SENDER_ID, PID_CHAT_SENDER_NAME, PID_CHAT_TEXT, UI_EVENT_CHAT_LINE,
    UiPropertyEvent,
};
use crate::session::{
    BindPlan, BitTreeTable, ConnectChallengeHeader, DatagramHeader, IsaacCipher, OptionalHeaders,
    SessionError, UdpDatagramTransport, VersionStatusMessage, decode_datagram, encode_datagram,
    split_blob,
};
use crate::world::{
    CharacterListResponse, CharacterSummaryEntry, EncodedPropertyTable, EncodedPropertyValue,
    EnterPortalSpace, EntityAppearanceDesc, EntityCreate, EntityPlacement, LocalizedTokenText,
    PID_AGENT_SPECIES, PID_ENTITY_DISPLAY_NAME, PID_SEX, PlayerSessionReady, PositionUpdate,
    PropertyTableUpdate, PropertyType, SubEventBatch, SubEventFrame, WirePosition,
    encode_event_envelope,
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
/// The synthetic ticket.
const TICKET: &str = "AAECAwQFBgcICQ==";
/// The install's datacenter URL.
const GLS: &str = "https://gls.example.invalid/datacenter";
/// The login server's assigned recipient ID and the seed of its outgoing masks.
const REC_ID: u16 = 0x0111;
const SEED: u32 = 0xBEEF_0001;

/// The name of the one character the synthetic account has.
pub const CHARACTER: &str = "Aldo";
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
pub const DICTIONARY: [(u32, PropertyType); 9] = [
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
}

#[derive(Debug, Default)]
struct State {
    seen: Seen,
    inbound: VecDeque<(Vec<u8>, SocketAddrV4)>,
    server: Option<Server>,
    /// The attempt's network right, kept for its liveness (its own revocation and
    /// `revoke_all` both end it).
    access: Option<NetworkAccess>,
}

impl State {
    /// Queues a datagram for the client (the oldest is dropped beyond [`MAX_KEPT`]).
    fn deliver(&mut self, datagram: Vec<u8>) {
        if self.inbound.len() >= MAX_KEPT {
            self.inbound.pop_front();
        }
        self.inbound.push_back((datagram, LOGIN_SERVER));
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
    /// A fresh service.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
            loader: Box::new(|_, _| Ok(Arc::new(records()) as Arc<dyn InstallData>)),
            locate: None,
        })
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
        let Some(d) = server.blob_to(opcode, recipient, &encode_event_envelope(inner)) else {
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
        if to != LOGIN_SERVER {
            return;
        }
        let mut st = self.state();
        let st = &mut *st;
        let first = decode_datagram(datagram, false, None, true).ok();
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
        let Ok(pkt) = decode_datagram(datagram, false, None, false) else {
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
                    .encode();
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

/// The synthetic login server's connection state.
#[derive(Debug)]
struct Server {
    isaac: IsaacCipher,
    seq: u32,
}

impl Server {
    fn new() -> Self {
        Self {
            isaac: IsaacCipher::from_seed(SEED),
            seq: 1,
        }
    }

    fn challenge(&self) -> Vec<u8> {
        let opt = OptionalHeaders {
            connect_challenge: Some(ConnectChallengeHeader {
                server_time: 1.0,
                assigned_client_rec_id: REC_ID,
                server_rec_id: 0x0222,
                outgoing_seed: 0xCAFE_1234,
                incoming_seed: SEED,
                reserved_dword_14: 0,
                client_conn_idx: 4,
                reserved_word_1a: 0,
                codec_uuid: [0u8; 16],
            }),
            ..OptionalHeaders::default()
        };
        encode_datagram(
            DatagramHeader {
                rec_id: REC_ID,
                ..DatagramHeader::default()
            },
            &opt,
            &[],
            None,
            None,
        )
        .unwrap_or_default()
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
        let frags = split_blob(
            2,
            u16::try_from(self.seq & 0xFFFF).unwrap_or(0),
            1,
            0,
            recipient,
            opcode,
            envelope,
        )
        .ok()?;
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

    fn event(&mut self, opcode: u32, inner: &[u8]) -> Option<Vec<u8>> {
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
    SessionError::Io("the network access was revoked".to_owned())
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

fn records() -> Records {
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
        BitTreeTable::synthetic_dictionary([1; 16]).to_dat_bytes(),
    );
    m.insert(
        0x0E01_0001,
        BitTreeTable::synthetic_dictionary([2; 16]).to_dat_bytes(),
    );
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

fn character_list() -> Vec<u8> {
    CharacterListResponse {
        characters: vec![CharacterSummaryEntry {
            character_id: 0x0200_0000_0000_0042,
            name: CHARACTER.to_owned(),
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
    use tessera_net::{Command, Event, Session, SessionState};

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
        let _ = s.send(Instant::now(), Command::WorldLoaded);
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
        s.send(Instant::now(), Command::WorldLoaded)
            .expect("loaded");
        s.send(Instant::now(), Command::LeaveWorld).expect("leave");
        wait(&mut s, "the return", |e| {
            matches!(e, Event::ReturnedToCharacterSelect { .. })
        });
        assert_eq!(s.state(), SessionState::Connected);
        let blobs = service.seen().blobs;
        for op in [0x12C, 0x6F, 0xF4, 0x30] {
            assert!(blobs.contains(&op), "{op:#x} in {blobs:x?}");
        }
        assert_eq!(service.seen().binds, 1);
    }
}
