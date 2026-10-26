// SPDX-License-Identifier: GPL-3.0-or-later
//! The live connector against a synthetic service: HTTP answers from fixtures and a scripted
//! UDP server over an in-memory transport. Nothing here opens a socket or resolves a name;
//! every endpoint is an RFC 2606 `.example.invalid` placeholder or a documentation address
//! (RFC 5737).

use std::collections::{BTreeMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tessera_net::account::{AccountCommand, AccountEvent, LoginRefusal, WorldId, WorldRefusal};
use tessera_net::{Command, DisconnectReason, Event, Session, Vec3};

use super::data::{InstallData, LocalPaths};
use super::net::{ConnectorNet, NetFactory, SendHttpClient, carries_secret, is_https};
use super::session_phase::movement_input;
use super::{LiveConfig, LiveConnector};
use crate::consent::{self, UserConfirmation, WarningText};
use crate::credentials::Credentials;
use crate::login::{HttpClient, HttpMethod, HttpRequest, HttpResponse, LoginError};
use crate::net_access::NetworkAccess;
use crate::session::{
    BitTreeTable, ConnectChallengeHeader, DatagramHeader, IsaacCipher, OptionalHeaders,
    SessionError, UdpDatagramTransport, VersionStatusMessage, WorldReferralHeader, decode_datagram,
    encode_datagram, split_blob,
};
use crate::world::{
    CharacterListResponse, CharacterSummaryEntry, EncodedPropertyTable, PlayerSessionReady,
    PropertyType, encode_event_envelope,
};
use crate::{CompatBackend, CompatConfig};

const LOCAL: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 5000);
const LOGIN_SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 10), 9001);
const WORLD_SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 20), 9100);
const TICKET: &str = "AAECAwQFBgcICQ==";
const ACCOUNT: &[u8] = b"canary-account";
const PASSWORD: &[u8] = b"canary-password";

// ---------------------------------------------------------------------------
// The synthetic install
// ---------------------------------------------------------------------------

/// A DID map record (`official-session.md` §4.1.3): one group of `(key, target)` pairs with
/// labels.
fn did_map(self_did: u32, entries: &[(u32, u32)]) -> Vec<u8> {
    let mut out = self_did.to_le_bytes().to_vec();
    out.push(0);
    out.push(u8::try_from(entries.len()).expect("small"));
    for (k, t) in entries {
        out.extend_from_slice(&k.to_le_bytes());
        out.extend_from_slice(&t.to_le_bytes());
    }
    out.push(0);
    out.push(u8::try_from(entries.len()).expect("small"));
    for (k, _) in entries {
        out.extend_from_slice(&k.to_le_bytes());
        out.push(1);
        out.push(b'k');
    }
    out
}

#[derive(Debug)]
struct Records(BTreeMap<u32, Vec<u8>>);

impl InstallData for Records {
    fn record(&self, did: u32) -> Option<Vec<u8>> {
        self.0.get(&did).cloned()
    }

    fn property_type(&self, _pid: u32) -> Option<PropertyType> {
        None
    }
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

fn write_install(root: &Path, gls: &str) {
    std::fs::write(
        root.join("lotro.launcherconfig"),
        format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<configuration><appSettings>
<add key="Launcher.DataCenterService.GLS" value="{gls}" />
<add key="DataCenter.GameName" value="LOTRO" />
</appSettings></configuration>"#
        ),
    )
    .expect("config");
    std::fs::write(root.join("client_local_English.dat"), b"SYNTH").expect("lang");
    let mut header = vec![0u8; 0x1A8];
    header[0x140..0x144].copy_from_slice(b"BT\0\0");
    header[0x170..0x174].copy_from_slice(&0x2800_0000u32.to_le_bytes());
    for (i, b) in header[0x17C..0x18C].iter_mut().enumerate() {
        *b = u8::try_from(i).expect("small") + 0x10;
    }
    header[0x18C..0x190].copy_from_slice(&1100u32.to_le_bytes());
    std::fs::write(root.join("client_general.dat"), header).expect("dat");
}

// ---------------------------------------------------------------------------
// The synthetic service
// ---------------------------------------------------------------------------

const DATACENTERS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body>
<GetDatacentersResponse xmlns="http://www.turbine.com/SE/GLS"><GetDatacentersResult><Datacenter>
<Name>LOTRO</Name><AuthServer>AUTH</AuthServer><PatchServer>patch.example.invalid:1</PatchServer>
<LauncherConfigurationServer>https://gls.example.invalid/launcher.xml</LauncherConfigurationServer>
<Worlds>
<World><Name>Alpha</Name><LoginServerUrl>(invalid service specified)</LoginServerUrl><ChatServerUrl></ChatServerUrl><StatusServerUrl>https://gls.example.invalid/status/alpha</StatusServerUrl><Order>1</Order></World>
<World><Name>Beta</Name><LoginServerUrl>(invalid service specified)</LoginServerUrl><ChatServerUrl></ChatServerUrl><StatusServerUrl>https://gls.example.invalid/status/beta</StatusServerUrl><Order>2</Order></World>
</Worlds></Datacenter></GetDatacentersResult></GetDatacentersResponse></soap:Body></soap:Envelope>"#;

const LAUNCHER: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration><appSettings>
<add key="GameClient.WIN32.ArgTemplate" value="-a {SUBSCRIPTION}" />
<add key="WorldQueue.LoginQueue.URL" value="https://gls.example.invalid/queue" />
<add key="WorldQueue.TakeANumber.Parameters" value="command=TakeANumber&amp;subscription={0}&amp;ticket={1}&amp;ticket_type=GLS&amp;queue_url={2}" />
<add key="WorldQueue.LeaveQueue.Parameters" value="command=LeaveQueue&amp;subscription={0}&amp;context={1}&amp;ticket_type=GLS&amp;queue_url={2}" />
</appSettings></configuration>"#;

fn login_ok(subscriptions: usize) -> String {
    let subs: String = (0..subscriptions)
        .map(|i| {
            format!(
                "<GameSubscription><Game>LOTRO</Game><Name>SUB{i:022}</Name>\
                 <ProductTokens><string>TokenBase</string></ProductTokens></GameSubscription>"
            )
        })
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body>
<LoginAccountResponse xmlns="http://www.turbine.com/SE/GLS"><LoginAccountResult>
<Subscriptions>{subs}</Subscriptions><Ticket>{TICKET}</Ticket>
</LoginAccountResult></LoginAccountResponse></soap:Body></soap:Envelope>"#
    )
}

const LOGIN_FAULT: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body>
<soap:Fault><faultcode>BIS</faultcode><faultstring>Authentication rejected</faultstring></soap:Fault>
</soap:Body></soap:Envelope>"#;

fn status(queue: bool) -> String {
    let q = if queue {
        "https://queue.example.invalid/q"
    } else {
        ""
    };
    format!(
        "<Status><loginservers>login.example.invalid:9001;</loginservers><queueurls>{q}</queueurls></Status>"
    )
}

/// The service's behaviour, per test.
#[derive(Debug, Clone)]
struct Service {
    auth: String,
    login: String,
    beta_status: Option<String>,
    alpha_queue: bool,
    queue_waiting: bool,
}

impl Default for Service {
    fn default() -> Self {
        Self {
            auth: "https://auth.example.invalid/login".to_owned(),
            login: login_ok(1),
            beta_status: Some(status(false)),
            alpha_queue: false,
            queue_waiting: false,
        }
    }
}

/// One recorded request: method, URL, `SOAPAction` and body.
type Recorded = (HttpMethod, String, Option<String>, Vec<u8>);

#[derive(Debug, Default)]
struct UdpState {
    inbound: VecDeque<(Vec<u8>, SocketAddrV4)>,
    outbound: VecDeque<(Vec<u8>, SocketAddrV4)>,
}

#[derive(Debug, Default)]
struct Shared {
    requests: Vec<Recorded>,
    binds: usize,
    udp: UdpState,
}

#[derive(Debug, Clone)]
struct World {
    service: Arc<Service>,
    shared: Arc<Mutex<Shared>>,
}

impl World {
    fn answer(&self, req: &HttpRequest) -> Result<HttpResponse, LoginError> {
        let mut s = self.shared.lock().expect("shared");
        s.requests.push((
            req.method,
            req.url.clone(),
            req.soap_action().map(str::to_owned),
            req.body.clone(),
        ));
        drop(s);
        let ok = |body: &str| {
            Ok(HttpResponse {
                status_code: 200,
                body: body.as_bytes().to_vec(),
            })
        };
        let url = req.url.as_str();
        match url {
            "https://gls.example.invalid/datacenter" => {
                ok(&DATACENTERS.replace("AUTH", &self.service.auth))
            }
            "https://gls.example.invalid/launcher.xml" => ok(LAUNCHER),
            "https://gls.example.invalid/status/alpha" => ok(&status(self.service.alpha_queue)),
            "https://gls.example.invalid/status/beta" => match &self.service.beta_status {
                Some(s) => ok(s),
                None => Ok(HttpResponse {
                    status_code: 404,
                    body: Vec::new(),
                }),
            },
            "https://gls.example.invalid/queue" => {
                let body = String::from_utf8_lossy(&req.body).into_owned();
                if body.starts_with("command=LeaveQueue") {
                    ok("<Result><HResult>0x00000000</HResult></Result>")
                } else if self.service.queue_waiting {
                    ok(
                        "<Result><HResult>0x00000000</HResult><QueueNumber>0x00000013</QueueNumber>\
                        <NowServingNumber>0x00000010</NowServingNumber><ContextNumber>0x00000007</ContextNumber></Result>",
                    )
                } else {
                    ok(
                        "<Result><HResult>0x00000000</HResult><QueueNumber>0x00000010</QueueNumber>\
                        <NowServingNumber>0x00000010</NowServingNumber></Result>",
                    )
                }
            }
            u if u == self.service.auth => ok(&self.service.login),
            _ => Ok(HttpResponse {
                status_code: 404,
                body: Vec::new(),
            }),
        }
    }

    fn requests(&self) -> Vec<Recorded> {
        self.shared.lock().expect("shared").requests.clone()
    }

    fn binds(&self) -> usize {
        self.shared.lock().expect("shared").binds
    }

    fn pop_sent(&self) -> Option<(Vec<u8>, SocketAddrV4)> {
        self.shared.lock().expect("shared").udp.outbound.pop_front()
    }

    fn deliver(&self, datagram: Vec<u8>, from: SocketAddrV4) {
        self.shared
            .lock()
            .expect("shared")
            .udp
            .inbound
            .push_back((datagram, from));
    }
}

#[derive(Debug)]
struct FakeHttp(World);

impl HttpClient for FakeHttp {
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        self.0.answer(request)
    }
}

#[derive(Debug)]
struct FakeUdp(Arc<Mutex<Shared>>);

impl UdpDatagramTransport for FakeUdp {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        Ok(LOCAL)
    }

    fn send_to(&mut self, buf: &[u8], target: SocketAddrV4) -> Result<usize, SessionError> {
        self.0
            .lock()
            .expect("shared")
            .udp
            .outbound
            .push_back((buf.to_vec(), target));
        Ok(buf.len())
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        let next = self.0.lock().expect("shared").udp.inbound.pop_front();
        Ok(next.and_then(|(d, src)| {
            let n = d.len().min(buf.len());
            buf.get_mut(..n)?.copy_from_slice(d.get(..n)?);
            Some((n, src))
        }))
    }
}

#[derive(Debug)]
struct FakeNet(World);

impl ConnectorNet for FakeNet {
    type Udp = FakeUdp;

    fn http(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        self.0.answer(request)
    }

    fn background_http(&self) -> SendHttpClient {
        Box::new(FakeHttp(self.0.clone()))
    }

    fn resolve(&mut self, host: &str, port: u16) -> Result<SocketAddrV4, SessionError> {
        match (host, port) {
            ("login.example.invalid", 9001) => Ok(LOGIN_SERVER),
            _ => Err(SessionError::Io("unknown synthetic name".to_owned())),
        }
    }

    fn local_ipv4_toward(&mut self, _remote: SocketAddrV4) -> Option<Ipv4Addr> {
        Some(*LOCAL.ip())
    }

    fn bind_udp(
        &mut self,
        _local_ip: Ipv4Addr,
        _explicit_base_port: u16,
        _compute_unique: bool,
    ) -> Result<FakeUdp, SessionError> {
        self.0.shared.lock().expect("shared").binds += 1;
        Ok(FakeUdp(Arc::clone(&self.0.shared)))
    }

    fn chat_binder(
        &self,
    ) -> Option<Box<dyn FnMut() -> Result<FakeUdp, crate::play::PlayError> + Send>> {
        None
    }
}

#[derive(Debug)]
struct FakeFactory(World);

impl NetFactory for FakeFactory {
    type Net = FakeNet;

    fn make(&mut self, access: NetworkAccess) -> FakeNet {
        let _ = access;
        FakeNet(self.0.clone())
    }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Run {
    _dir: tempfile::TempDir,
    world: World,
    session: Session,
    seen: Vec<Event>,
}

fn proof_dir() -> (tempfile::TempDir, consent::Acceptance) {
    let dir = tempfile::tempdir().expect("temp");
    let (acc, write) = consent::accept(
        Some(dir.path()),
        &WarningText::shipped(),
        UserConfirmation::confirmed_by_user(),
    );
    let _ = write.wait();
    (dir, acc)
}

fn start(service: Service, install: bool) -> Run {
    let (dir, acc) = proof_dir();
    let root = dir.path().join("install");
    std::fs::create_dir_all(&root).expect("root");
    if install {
        write_install(&root, "https://gls.example.invalid/datacenter");
    }
    let world = World {
        service: Arc::new(service),
        shared: Arc::new(Mutex::new(Shared::default())),
    };
    let config = LiveConfig {
        paths: LocalPaths {
            install_root: root,
            user_settings_dir: None,
            network_version: Some("9999".to_owned()),
        },
        loader: Box::new(|_| Ok(Arc::new(records()) as Arc<dyn InstallData>)),
        locate: None,
    };
    let connector = LiveConnector::with_factory(config, FakeFactory(world.clone()));
    let creds = Credentials::new(ACCOUNT, PASSWORD).expect("creds");
    let backend = CompatBackend::with_connector(CompatConfig::new(acc.proof(), creds), connector)
        .expect("backend");
    let mut session = Session::new(backend);
    session
        .send(Instant::now(), Command::Connect)
        .expect("connect");
    Run {
        _dir: dir,
        world,
        session,
        seen: Vec::new(),
    }
}

impl Run {
    /// Polls until `pred` holds for an event seen (kept in `seen`), at most 10 s.
    fn wait_event(&mut self, pred: impl Fn(&Event) -> bool) -> Event {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let events = self.session.poll(Instant::now());
            for e in events {
                let hit = pred(&e);
                self.seen.push(e.clone());
                if hit {
                    return e;
                }
            }
            assert!(Instant::now() < until, "timed out; seen {:?}", self.seen);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// The next datagram the client sent, at most 10 s.
    fn wait_sent(&mut self) -> (Vec<u8>, SocketAddrV4) {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(d) = self.world.pop_sent() {
                return d;
            }
            let _ = self
                .session
                .poll(Instant::now())
                .into_iter()
                .map(|e| self.seen.push(e));
            assert!(Instant::now() < until, "nothing sent; seen {:?}", self.seen);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// The next client blob `(opcode, envelope)` sent to `to`, skipping transport packets.
    fn wait_blob(&mut self, to: SocketAddrV4) -> (u32, Vec<u8>) {
        loop {
            let (d, dst) = self.wait_sent();
            if dst != to {
                continue;
            }
            let pkt = decode_datagram(&d, false, None, false).expect("client datagram");
            if let Some(f) = pkt.fragments.into_iter().next() {
                return (f.header.blob_header_word.expect("word"), f.payload);
            }
        }
    }
}

/// A synthetic UDP server connection.
struct Server {
    rec_id: u16,
    isaac: IsaacCipher,
    seq: u32,
}

impl Server {
    fn new(rec_id: u16, seed: u32) -> Self {
        Self {
            rec_id,
            isaac: IsaacCipher::from_seed(seed),
            seq: 1,
        }
    }

    fn challenge(&self, seed: u32) -> Vec<u8> {
        let opt = OptionalHeaders {
            connect_challenge: Some(ConnectChallengeHeader {
                server_time: 1.0,
                assigned_client_rec_id: self.rec_id,
                server_rec_id: 0x0222,
                outgoing_seed: 0xCAFE_1234,
                incoming_seed: seed,
                reserved_dword_14: 0,
                client_conn_idx: 4,
                reserved_word_1a: 0,
                codec_uuid: [0u8; 16],
            }),
            ..OptionalHeaders::default()
        };
        encode_datagram(
            DatagramHeader {
                rec_id: self.rec_id,
                ..DatagramHeader::default()
            },
            &opt,
            &[],
            None,
            None,
        )
        .expect("challenge")
    }

    fn control(&self, opt: OptionalHeaders) -> Vec<u8> {
        encode_datagram(
            DatagramHeader {
                rec_id: self.rec_id,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &opt,
            &[],
            None,
            None,
        )
        .expect("control")
    }

    fn blob(&mut self, opcode: u32, envelope: &[u8]) -> Vec<u8> {
        self.seq += 1;
        let frags =
            split_blob(2, (self.seq & 0xFFFF) as u16, 1, 0, None, opcode, envelope).expect("split");
        let mask = self.isaac.next_mask();
        encode_datagram(
            DatagramHeader {
                rec_id: self.rec_id,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            &frags,
            Some(mask),
            None,
        )
        .expect("datagram")
    }

    fn event(&mut self, opcode: u32, inner: &[u8]) -> Vec<u8> {
        self.blob(opcode, &encode_event_envelope(inner))
    }
}

fn worlds_of(e: &Event) -> Vec<(WorldId, String)> {
    match e {
        Event::Account(AccountEvent::WorldList { worlds }) => {
            worlds.iter().map(|w| (w.id, w.name.clone())).collect()
        }
        _ => Vec::new(),
    }
}

fn is_world_list(e: &Event) -> bool {
    matches!(e, Event::Account(AccountEvent::WorldList { .. }))
}

fn login_requests(world: &World) -> usize {
    world
        .requests()
        .iter()
        .filter(|r| r.2.as_deref().is_some_and(|a| a.contains("LoginAccount")))
        .count()
}

fn wait_binds(run: &mut Run, n: usize) {
    let until = Instant::now() + Duration::from_secs(10);
    while run.world.binds() < n {
        let _ = run.session.poll(Instant::now());
        assert!(Instant::now() < until, "no bind; seen {:?}", run.seen);
        std::thread::sleep(Duration::from_millis(2));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn i1_local_discovery_failure_reaches_no_network() {
    let mut run = start(Service::default(), false);
    run.wait_event(|e| matches!(e, Event::Disconnected { .. }));
    assert!(run.seen.iter().any(|e| matches!(
        e,
        Event::Account(AccountEvent::LoginRefused {
            reason: LoginRefusal::Other,
            ..
        })
    )));
    assert!(run.world.requests().is_empty());
    assert_eq!(run.world.binds(), 0);
}

#[test]
fn login_lists_worlds_after_authenticating_once() {
    let mut run = start(Service::default(), true);
    let list = run.wait_event(is_world_list);
    assert_eq!(
        worlds_of(&list),
        vec![
            (WorldId(0), "Alpha".to_owned()),
            (WorldId(1), "Beta".to_owned())
        ]
    );
    let reqs = run.world.requests();
    let order: Vec<&str> = reqs.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(
        order,
        vec![
            "https://gls.example.invalid/datacenter",
            "https://gls.example.invalid/launcher.xml",
            "https://auth.example.invalid/login",
            "https://gls.example.invalid/status/alpha",
            "https://gls.example.invalid/status/beta",
        ]
    );
    assert_eq!(login_requests(&run.world), 1);
    // The password went once, inside the LoginAccount body; nothing printable holds it.
    let dbg = format!("{:?}", run.session);
    assert!(!dbg.contains("canary"), "{dbg}");
    assert!(
        run.seen
            .iter()
            .all(|e| !format!("{e:?}").contains("canary"))
    );
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn bad_credentials_are_refused_with_their_category() {
    let mut run = start(
        Service {
            login: LOGIN_FAULT.to_owned(),
            ..Service::default()
        },
        true,
    );
    run.wait_event(|e| matches!(e, Event::Disconnected { .. }));
    assert!(run.seen.iter().any(|e| matches!(
        e,
        Event::Account(AccountEvent::LoginRefused {
            reason: LoginRefusal::BadCredentials,
            ..
        })
    )));
    assert_eq!(run.world.binds(), 0);
}

#[test]
fn several_subscriptions_are_an_unsupported_step() {
    let mut run = start(
        Service {
            login: login_ok(2),
            ..Service::default()
        },
        true,
    );
    run.wait_event(|e| matches!(e, Event::Disconnected { .. }));
    assert!(run.seen.iter().any(|e| matches!(
        e,
        Event::Account(AccountEvent::LoginRefused {
            reason: LoginRefusal::UnsupportedStep,
            ..
        })
    )));
}

#[test]
fn credentials_never_go_over_plain_http() {
    let mut run = start(
        Service {
            auth: "http://auth.example.invalid/login".to_owned(),
            ..Service::default()
        },
        true,
    );
    run.wait_event(|e| matches!(e, Event::Disconnected { .. }));
    assert!(run.seen.iter().any(|e| matches!(
        e,
        Event::Account(AccountEvent::LoginRefused {
            reason: LoginRefusal::ServiceUnavailable,
            ..
        })
    )));
    assert_eq!(login_requests(&run.world), 0, "no LoginAccount was sent");
}

#[test]
fn a_world_refusal_keeps_the_login() {
    let mut run = start(
        Service {
            beta_status: None,
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_world_list);
    run.session
        .send(
            Instant::now(),
            Command::Account(AccountCommand::ChooseWorld { world: WorldId(1) }),
        )
        .expect("choose");
    run.wait_event(|e| {
        matches!(
            e,
            Event::Account(AccountEvent::WorldRefused {
                world: WorldId(1),
                reason: WorldRefusal::Closed,
                ..
            })
        )
    });
    // Another world, without the password again.
    run.session
        .send(
            Instant::now(),
            Command::Account(AccountCommand::ChooseWorld { world: WorldId(0) }),
        )
        .expect("choose again");
    wait_binds(&mut run, 1);
    assert_eq!(login_requests(&run.world), 1);
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn leaving_the_queue_sends_leave_queue_and_lists_the_worlds_again() {
    let mut run = start(
        Service {
            alpha_queue: true,
            queue_waiting: true,
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_world_list);
    run.session
        .send(
            Instant::now(),
            Command::Account(AccountCommand::ChooseWorld { world: WorldId(0) }),
        )
        .expect("choose");
    run.wait_event(|e| {
        matches!(
            e,
            Event::Account(AccountEvent::LoginQueueChanged {
                position: Some(3),
                ..
            })
        )
    });
    let take = run
        .world
        .requests()
        .into_iter()
        .find(|r| r.3.starts_with(b"command=TakeANumber"))
        .expect("TakeANumber");
    let body = String::from_utf8(take.3).expect("utf8");
    assert!(body.contains("ticket=AAECAwQFBgcICQ%3D%3D"), "{body}");
    run.session
        .send(
            Instant::now(),
            Command::Account(AccountCommand::LeaveLoginQueue),
        )
        .expect("leave");
    let n = run.seen.iter().filter(|e| is_world_list(e)).count();
    run.wait_event(is_world_list);
    assert!(run.seen.iter().filter(|e| is_world_list(e)).count() > n);
    assert!(
        run.world
            .requests()
            .iter()
            .any(|r| r.3.starts_with(b"command=LeaveQueue")),
        "LeaveQueue sent"
    );
    assert_eq!(run.world.binds(), 0);
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

fn version_payload(env: &[u8]) -> String {
    // envelopeFlags, then a one-byte length for a short string.
    let len = usize::from(*env.get(1).expect("len"));
    String::from_utf8(env.get(2..2 + len).expect("string").to_vec()).expect("ascii")
}

#[test]
fn the_session_reaches_the_character_list_and_the_world() {
    let mut run = start(Service::default(), true);
    run.wait_event(is_world_list);
    run.session
        .send(
            Instant::now(),
            Command::Account(AccountCommand::ChooseWorld { world: WorldId(0) }),
        )
        .expect("choose");
    // SessionAuthHeader to the login server, with the subscription and the ticket.
    let (auth, dst) = run.wait_sent();
    assert_eq!(dst, LOGIN_SERVER);
    let sa = decode_datagram(&auth, false, None, true)
        .expect("decode")
        .opt_headers
        .session_auth
        .expect("SessionAuth");
    assert_eq!(sa.auth_type, 0x4000_0002);
    let mut login = Server::new(0x0111, 0xBEEF_0001);
    run.world
        .deliver(login.challenge(0xBEEF_0001), LOGIN_SERVER);
    run.world.deliver(
        login.control(OptionalHeaders {
            cumulative_ack: Some(0),
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    let req = login.event(0x12F, &[]);
    run.world.deliver(req, LOGIN_SERVER);
    // 0x12C with the version string (`official-login.md` §2.4.1).
    let (op, env) = run.wait_blob(LOGIN_SERVER);
    assert_eq!(op, 0x12C);
    let v = version_payload(&env);
    assert!(v.starts_with("netver:9999; didver:"), "{v}");
    assert_eq!(v.len(), "netver:9999; didver:".len() + 36);
    let status_ok = VersionStatusMessage {
        status: 1,
        server_version_info: "ok".to_owned(),
    }
    .encode();
    let d = login.blob(0x12D, &encode_event_envelope(&status_ok));
    run.world.deliver(d, LOGIN_SERVER);
    let list = CharacterListResponse {
        characters: vec![CharacterSummaryEntry {
            character_id: 0x0200_0000_0000_0042,
            name: "Aldo".to_owned(),
            status_or_world: 0,
            public_props: EncodedPropertyTable::new(),
            private_props: EncodedPropertyTable::new(),
            metadata_a: 0,
            visual_desc: crate::world::EntityAppearanceDesc::default(),
            metadata_b: 0,
            metadata_c: u32::MAX,
            metadata_d: 0,
            metadata_e: u32::MAX,
            timestamp_or_meta: 0,
        }],
        account_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .expect("list");
    let d = login.event(0x6C, &list);
    run.world.deliver(d, LOGIN_SERVER);
    let connected = run.wait_event(|e| matches!(e, Event::Connected { .. }));
    let Event::Connected { characters } = connected else {
        unreachable!("matched")
    };
    assert_eq!(characters.len(), 1);
    assert_eq!(characters[0].name, "Aldo");
    // Enter the world: 0x6F with the character's ID.
    run.session
        .send(
            Instant::now(),
            Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
        )
        .expect("enter");
    let (op, env) = run.wait_blob(LOGIN_SERVER);
    assert_eq!(op, 0x6F);
    assert_eq!(
        env.get(2..10),
        Some(&0x0200_0000_0000_0042u64.to_le_bytes()[..])
    );
    // The referral: the world server assigns the client the recipient ID the login
    // connection has. Its own multiplexer takes it (module docs of `session_phase`).
    run.world.deliver(
        login.control(OptionalHeaders {
            world_referral: Some(WorldReferralHeader {
                referral_cookie: 0x1122_3344_5566_7788,
                sin_family: 2,
                target_addr: WORLD_SERVER,
                reserved_word_10: 0,
            }),
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    let ra = loop {
        let (d, dst) = run.wait_sent();
        if dst == WORLD_SERVER {
            break decode_datagram(&d, false, None, false)
                .expect("referral auth")
                .opt_headers
                .referral_auth
                .expect("ReferralAuthHeader");
        }
    };
    assert_eq!(ra.referral_cookie, 0x1122_3344_5566_7788);
    let mut world = Server::new(0x0111, 0xBEEF_0002);
    run.world
        .deliver(world.challenge(0xBEEF_0002), WORLD_SERVER);
    run.world.deliver(
        world.control(OptionalHeaders {
            cumulative_ack: Some(0),
            ..OptionalHeaders::default()
        }),
        WORLD_SERVER,
    );
    let ready = PlayerSessionReady {
        player_id: 0x0100_0000_0000_0007,
        session_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .expect("ready");
    let d = world.event(0x90, &ready);
    run.world.deliver(d, WORLD_SERVER);
    let entered = run.wait_event(|e| matches!(e, Event::WorldEntered { .. }));
    let Event::WorldEntered { player, name, .. } = entered else {
        unreachable!("matched")
    };
    assert_eq!(player.0, 0x0100_0000_0000_0007);
    assert_eq!(name, "Aldo");
    // Disconnect: the session ends at once; the connection says Disconnect.
    run.session
        .send(Instant::now(), Command::Disconnect)
        .expect("disconnect");
    run.wait_event(|e| {
        matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::ClientRequested
            }
        )
    });
    assert_eq!(run.world.binds(), 1);
}

#[test]
fn a_transport_refusal_during_login_is_account_not_allowed() {
    let mut run = start(Service::default(), true);
    run.wait_event(is_world_list);
    run.session
        .send(
            Instant::now(),
            Command::Account(AccountCommand::ChooseWorld { world: WorldId(0) }),
        )
        .expect("choose");
    let _ = run.wait_sent();
    let login = Server::new(0x0111, 1);
    run.world.deliver(login.challenge(1), LOGIN_SERVER);
    run.world.deliver(
        login.control(OptionalHeaders {
            transport_error: Some(crate::session::TransportErrorHeader {
                error_code: 0x00A7_E948,
                error_subcode: 0,
            }),
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    run.wait_event(|e| matches!(e, Event::Disconnected { .. }));
    assert!(run.seen.iter().any(|e| matches!(
        e,
        Event::Account(AccountEvent::LoginRefused {
            reason: LoginRefusal::AccountNotAllowed,
            ..
        })
    )));
}

#[test]
fn secrets_are_recognised() {
    let mk = |method, action: Option<&str>, ct: Option<&str>| HttpRequest {
        method,
        url: "https://a.example.invalid/".to_owned(),
        headers: action
            .map(|a| ("SOAPAction".to_owned(), a.to_owned()))
            .into_iter()
            .chain(ct.map(|c| ("Content-Type".to_owned(), c.to_owned())))
            .collect(),
        body: Vec::new(),
        timeout_sec: 30,
        require_tls_validation: true,
    };
    assert!(carries_secret(&mk(
        HttpMethod::Post,
        Some("\"x/LoginAccount\""),
        None
    )));
    assert!(carries_secret(&mk(
        HttpMethod::Post,
        None,
        Some("application/x-www-form-urlencoded")
    )));
    assert!(!carries_secret(&mk(
        HttpMethod::Post,
        Some("\"x/GetDatacenters\""),
        None
    )));
    assert!(!carries_secret(&mk(HttpMethod::Get, None, None)));
    assert!(is_https("HTTPS://x.example.invalid"));
    assert!(!is_https("http://x.example.invalid"));
}

#[test]
fn moves_become_input_axes() {
    // Facing east (heading 0) and moving east: forward.
    let i = movement_input(0.0, Vec3::new(4.0, 0.0, 0.0));
    assert!((i.forward - 1.0).abs() < 1e-5 && i.strafe.abs() < 1e-5);
    // Facing north, moving east: to the right.
    let i = movement_input(std::f32::consts::FRAC_PI_2, Vec3::new(3.0, 0.0, 0.0));
    assert!(i.forward.abs() < 1e-5 && (i.strafe - 1.0).abs() < 1e-5);
    // Standing, or a non-finite heading: no input.
    assert_eq!(movement_input(0.0, Vec3::new(0.0, 0.0, 0.0)).forward, 0.0);
    assert_eq!(
        movement_input(f32::NAN, Vec3::new(1.0, 0.0, 0.0)).forward,
        0.0
    );
}

#[test]
fn login_failures_map_to_the_entry_categories() {
    use super::login_phase::{login_refusal, world_refusal};
    let locked = LoginError::AuthRejected {
        fault_code: "BIS".to_owned(),
        fault_string: "Account is locked".to_owned(),
        detail_result: None,
    };
    assert_eq!(login_refusal(&locked), LoginRefusal::AccountNotAllowed);
    let bad = LoginError::AuthRejected {
        fault_code: "BIS".to_owned(),
        fault_string: "bad password".to_owned(),
        detail_result: None,
    };
    assert_eq!(login_refusal(&bad), LoginRefusal::BadCredentials);
    let down = LoginError::ServiceUnavailable {
        reason: String::new(),
    };
    assert_eq!(login_refusal(&down), LoginRefusal::ServiceUnavailable);
    assert_eq!(
        world_refusal(&LoginError::WorldRoleDenied),
        WorldRefusal::NotAllowed
    );
    assert_eq!(
        world_refusal(&LoginError::WorldClosed {
            reason: String::new()
        }),
        WorldRefusal::Closed
    );
}
