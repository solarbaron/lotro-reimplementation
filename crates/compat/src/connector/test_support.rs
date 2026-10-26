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
//! | `0x6F` (enter world) | `0x90` (session start); no referral: the login connection carries the world |
//! | `0xF4` (portal exit) | nothing (counted) |
//! | `0x30` (an RPC: the logout request of `LeaveWorld`) | a world teardown `0x3000001B` and the list `0x6C` again |
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
use crate::login::soap_encoder::{GLS_NAMESPACE, SOAP_ENVELOPE_NAMESPACE};
use crate::login::{HttpClient, HttpRequest, HttpResponse, LoginError};
use crate::net_access::{NetworkAccess, Revocation};
use crate::session::{
    BitTreeTable, ConnectChallengeHeader, DatagramHeader, IsaacCipher, OptionalHeaders,
    SessionError, UdpDatagramTransport, VersionStatusMessage, decode_datagram, encode_datagram,
    split_blob,
};
use crate::world::{
    CharacterListResponse, CharacterSummaryEntry, EncodedPropertyTable, EntityAppearanceDesc,
    PlayerSessionReady, PropertyType, encode_event_envelope,
};

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

/// What the synthetic service saw, for the test's assertions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seen {
    /// HTTP requests answered.
    pub requests: usize,
    /// Sockets bound.
    pub binds: usize,
    /// Opcodes of the client's blobs, in order.
    pub blobs: Vec<u32>,
    /// Datagrams with the transport disconnect.
    pub disconnects: usize,
}

#[derive(Debug, Default)]
struct State {
    seen: Seen,
    inbound: VecDeque<(Vec<u8>, SocketAddrV4)>,
    server: Option<Server>,
    revocation: Option<Revocation>,
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
            loader: Box::new(|_| Ok(Arc::new(records()) as Arc<dyn InstallData>)),
            locate: None,
        })
    }

    /// The network factory of the live connector over this service.
    #[must_use]
    pub fn factory(&self) -> SyntheticFactory {
        SyntheticFactory(self.clone())
    }

    fn live(&self) -> bool {
        self.state()
            .revocation
            .as_ref()
            .is_none_or(|r| !r.is_revoked())
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
            st.inbound.push_back((server.challenge(), LOGIN_SERVER));
            st.inbound.push_back((
                server.control(OptionalHeaders {
                    cumulative_ack: Some(0),
                    ..OptionalHeaders::default()
                }),
                LOGIN_SERVER,
            ));
            let mut server = server;
            if let Some(d) = server.event(0x12F, &[]) {
                st.inbound.push_back((d, LOGIN_SERVER));
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
        let Some(server) = st.server.as_mut() else {
            return;
        };
        let acked = pkt.header.seq_num;
        let reliable = !pkt.fragments.is_empty();
        for f in pkt.fragments {
            let Some(op) = f.header.blob_header_word else {
                continue;
            };
            st.seen.blobs.push(op);
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
                0x6F => vec![server.event(0x90, &session_ready())],
                0x30 => vec![
                    server.event(0x3000_001B, &[0]),
                    server.event(0x6C, &character_list()),
                ],
                _ => Vec::new(),
            };
            st.inbound
                .extend(answers.into_iter().flatten().map(|d| (d, LOGIN_SERVER)));
        }
        if reliable {
            // Acknowledged at once, so nothing is sent twice.
            let ack = server.control(OptionalHeaders {
                cumulative_ack: Some(acked),
                ..OptionalHeaders::default()
            });
            st.inbound.push_back((ack, LOGIN_SERVER));
        }
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
        self.seq += 1;
        let frags = split_blob(
            2,
            u16::try_from(self.seq & 0xFFFF).unwrap_or(0),
            1,
            0,
            None,
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
        // The synthetic network follows the attempt's own right (the reaper's revocation
        // and Withdraw's).
        self.0.state().revocation = Some(access.revocation());
        drop(access);
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

    fn bind_udp(
        &mut self,
        _local_ip: Ipv4Addr,
        _explicit_base_port: u16,
        _compute_unique: bool,
    ) -> Result<SyntheticUdp, SessionError> {
        if !self.0.live() {
            return Err(revoked());
        }
        self.0.state().seen.binds += 1;
        Ok(SyntheticUdp(self.0.clone()))
    }

    fn chat_binder(
        &self,
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

    fn property_type(&self, _pid: u32) -> Option<PropertyType> {
        None
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

fn session_ready() -> Vec<u8> {
    PlayerSessionReady {
        player_id: PLAYER,
        session_props: EncodedPropertyTable::new(),
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
            assert!(Instant::now() < until, "no {what}");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn the_synthetic_service_walks_login_entry_and_return() {
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
