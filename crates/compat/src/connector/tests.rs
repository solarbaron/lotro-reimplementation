// SPDX-License-Identifier: GPL-3.0-or-later
//! The live connector against a synthetic service: HTTP answers from fixtures and a scripted
//! UDP server over an in-memory transport. Nothing here opens a socket or resolves a name;
//! every endpoint is an RFC 2606 `.example.invalid` placeholder or a documentation address
//! (RFC 5737).

use std::collections::{BTreeMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use tessera_net::account::{
    AccountCommand, AccountEvent, LoginRefusal, LoginStage, WorldId, WorldRefusal,
};
use tessera_net::{
    Backend, ChatChannel, Command, DisconnectReason, Event, NetError, Session, Vec3,
};

use super::data::SchemaView;
use super::data::{InstallData, LocalPaths};
use super::net::{ConnectorNet, NetFactory, SendHttpClient, carries_secret, is_https};
use super::session_phase::{SessionDriver, SessionStart, movement_input};
use super::{EVENT_QUEUE, LiveConfig, LiveConnector};
use crate::consent::{self, UserConfirmation, WarningText};
use crate::credentials::Credentials;
use crate::login::soap_encoder::{GLS_NAMESPACE, SOAP_ENVELOPE_NAMESPACE};
use crate::login::{
    HttpClient, HttpMethod, HttpRequest, HttpResponse, LoginError, SessionHandoff, TicketRefresher,
    VersionInfo,
};
use crate::net_access::NetworkAccess;
use crate::session::{
    BitTreeTable, ConnectChallengeHeader, DatagramHeader, IsaacCipher, LoginQueueStatusMessage,
    OptionalHeaders, SessionError, UdpDatagramTransport, VersionStatusMessage, WorldReferralHeader,
    decode_datagram, encode_datagram, split_blob,
};
use crate::world::{
    AccountEvictionNotice, CharacterListResponse, CharacterSummaryEntry, EncodedPropertyTable,
    LocalizedTokenText, PlayerSessionReady, PropertyType, WorldNameAnnouncement,
    encode_event_envelope,
};
use crate::{CompatBackend, CompatConfig};

const LOCAL: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 5000);
const LOGIN_SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 10), 9001);
const WORLD_SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 20), 9100);
const WORLD_SERVER_2: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 21), 9101);
const CHAT_SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 30), 9200);
const TICKET: &str = "AAECAwQFBgcICQ==";
const ACCOUNT: &[u8] = b"canary-account";
const PASSWORD: &[u8] = b"canary-password";
const GLS: &str = "https://gls.example.invalid/datacenter";

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
    fn record(&self, did: u32, max_bytes: usize) -> Option<Vec<u8>> {
        self.0.get(&did).filter(|r| r.len() <= max_bytes).cloned()
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

/// The datacenter answer; the namespaces come from the SOAP encoder's constants
/// (decision 0042 item 3).
fn datacenters(auth: &str, chat: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="{SOAP_ENVELOPE_NAMESPACE}"><soap:Body>
<GetDatacentersResponse xmlns="{GLS_NAMESPACE}"><GetDatacentersResult><Datacenter>
<Name>LOTRO</Name><AuthServer>{auth}</AuthServer><PatchServer>patch.example.invalid:1</PatchServer>
<LauncherConfigurationServer>https://gls.example.invalid/launcher.xml</LauncherConfigurationServer>
<Worlds>
<World><Name>Alpha</Name><LoginServerUrl>(invalid service specified)</LoginServerUrl><ChatServerUrl>{chat}</ChatServerUrl><StatusServerUrl>https://gls.example.invalid/status/alpha</StatusServerUrl><Order>1</Order></World>
<World><Name>Beta</Name><LoginServerUrl>(invalid service specified)</LoginServerUrl><ChatServerUrl></ChatServerUrl><StatusServerUrl>https://gls.example.invalid/status/beta</StatusServerUrl><Order>2</Order></World>
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
<soap:Envelope xmlns:soap="{SOAP_ENVELOPE_NAMESPACE}"><soap:Body>
<LoginAccountResponse xmlns="{GLS_NAMESPACE}"><LoginAccountResult>
<Subscriptions>{subs}</Subscriptions><Ticket>{TICKET}</Ticket>
</LoginAccountResult></LoginAccountResponse></soap:Body></soap:Envelope>"#
    )
}

fn login_fault(text: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="{SOAP_ENVELOPE_NAMESPACE}"><soap:Body>
<soap:Fault><faultcode>BIS</faultcode><faultstring>{text}</faultstring></soap:Fault>
</soap:Body></soap:Envelope>"#
    )
}

fn status(queue: bool, login: &str) -> String {
    let q = if queue {
        "https://queue.example.invalid/q"
    } else {
        ""
    };
    format!("<Status><loginservers>{login};</loginservers><queueurls>{q}</queueurls></Status>")
}

/// A gate that holds the `GetDatacenters` answer until the test opens it.
#[derive(Debug, Default)]
struct Gate {
    open: Mutex<bool>,
    cv: Condvar,
    reached: AtomicBool,
}

impl Gate {
    fn wait(&self) {
        self.reached.store(true, Ordering::SeqCst);
        let mut open = self.open.lock().expect("gate");
        while !*open {
            open = self.cv.wait(open).expect("gate");
        }
    }

    fn release(&self) {
        *self.open.lock().expect("gate") = true;
        self.cv.notify_all();
    }

    fn wait_reached(&self) {
        let until = Instant::now() + Duration::from_secs(10);
        while !self.reached.load(Ordering::SeqCst) {
            assert!(Instant::now() < until, "the gate was not reached");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

/// The service's behaviour, per test.
#[derive(Debug, Clone)]
struct Service {
    auth: String,
    login: String,
    login_server: String,
    chat: String,
    beta_status: Option<String>,
    alpha_queue: bool,
    queue_waiting: bool,
    gate: Option<Arc<Gate>>,
    /// Holds every `RefreshTicket` answer until the test opens it.
    refresh_gate: Option<Arc<Gate>>,
    /// The chat binder fails (and counts its calls) instead of being absent.
    failing_chat: bool,
}

impl Default for Service {
    fn default() -> Self {
        Self {
            auth: "https://auth.example.invalid/login".to_owned(),
            login: login_ok(1),
            login_server: "login.example.invalid:9001".to_owned(),
            chat: String::new(),
            beta_status: Some(status(false, "login.example.invalid:9001")),
            alpha_queue: false,
            queue_waiting: false,
            gate: None,
            refresh_gate: None,
            failing_chat: false,
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
    /// The connector's network was dropped (its thread ended).
    net_dropped: Arc<AtomicBool>,
    chat_binds: Arc<AtomicUsize>,
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
        if let Some(g) = &self.service.refresh_gate
            && req
                .soap_action()
                .is_some_and(|a| a.contains("RefreshTicket"))
        {
            g.wait();
        }
        // Routing ignores the scheme: the provenance tests use `http://` variants.
        let url = req.url.replacen("http://", "https://", 1);
        match url.as_str() {
            "https://gls.example.invalid/datacenter" => {
                if let Some(g) = &self.service.gate {
                    g.wait();
                }
                ok(&datacenters(&self.service.auth, &self.service.chat))
            }
            "https://gls.example.invalid/launcher.xml" => ok(LAUNCHER),
            "https://gls.example.invalid/status/alpha" => ok(&status(
                self.service.alpha_queue,
                &self.service.login_server,
            )),
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
            u if u == self.service.auth.replacen("http://", "https://", 1) => {
                ok(&self.service.login)
            }
            _ => Ok(HttpResponse {
                status_code: 404,
                body: Vec::new(),
            }),
        }
    }

    fn requests(&self) -> Vec<Recorded> {
        self.shared.lock().expect("shared").requests.clone()
    }

    fn requests_to(&self, url: &str) -> usize {
        self.requests().iter().filter(|r| r.1 == url).count()
    }

    fn binds(&self) -> usize {
        self.shared.lock().expect("shared").binds
    }

    fn pop_sent(&self) -> Option<(Vec<u8>, SocketAddrV4)> {
        self.shared.lock().expect("shared").udp.outbound.pop_front()
    }

    fn inbound_len(&self) -> usize {
        self.shared.lock().expect("shared").udp.inbound.len()
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

impl Drop for FakeNet {
    fn drop(&mut self) {
        self.0.net_dropped.store(true, Ordering::SeqCst);
    }
}

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
            ("chat.example.invalid", 9200) => Ok(CHAT_SERVER),
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
        if !self.0.service.failing_chat {
            return None;
        }
        let calls = Arc::clone(&self.0.chat_binds);
        Some(Box::new(move || {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(crate::play::PlayError::from(SessionError::Io(
                "no synthetic chat socket".to_owned(),
            )))
        }))
    }
}

#[derive(Debug)]
struct FakeFactory(World);

impl NetFactory for FakeFactory {
    type Net = FakeNet;

    fn make(&mut self, access: NetworkAccess) -> FakeNet {
        drop(access);
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
    /// Events of a poll after the one `wait_event` returned.
    backlog: VecDeque<Event>,
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

/// A backend over the live connector and the synthetic service; the install's datacenter
/// URL is `gls` (`None`: no install at all).
fn backend(service: Service, gls: Option<&str>) -> (tempfile::TempDir, World, Box<dyn Backend>) {
    let (dir, acc) = proof_dir();
    let root = dir.path().join("install");
    std::fs::create_dir_all(&root).expect("root");
    if let Some(gls) = gls {
        write_install(&root, gls);
    }
    let world = World {
        service: Arc::new(service),
        shared: Arc::new(Mutex::new(Shared::default())),
        net_dropped: Arc::new(AtomicBool::new(false)),
        chat_binds: Arc::new(AtomicUsize::new(0)),
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
    (dir, world, Box::new(backend))
}

fn start_with(service: Service, gls: Option<&str>) -> Run {
    let (dir, world, backend) = backend(service, gls);
    let mut session = Session::from_boxed(backend);
    session
        .send(Instant::now(), Command::Connect)
        .expect("connect");
    Run {
        _dir: dir,
        world,
        session,
        seen: Vec::new(),
        backlog: VecDeque::new(),
    }
}

fn start(service: Service, install: bool) -> Run {
    start_with(service, install.then_some(GLS))
}

impl Run {
    /// Polls until `pred` holds for an event seen (kept in `seen`), at most 10 s.
    fn wait_event(&mut self, pred: impl Fn(&Event) -> bool) -> Event {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let mut events: VecDeque<Event> = std::mem::take(&mut self.backlog);
            events.extend(self.session.poll(Instant::now()));
            while let Some(e) = events.pop_front() {
                let hit = pred(&e);
                self.seen.push(e.clone());
                if hit {
                    self.backlog = events;
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
            self.backlog.extend(self.session.poll(Instant::now()));
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

    /// Waits until the connector's thread has ended (its network was dropped), at most
    /// 10 s, polling meanwhile when `poll`.
    fn wait_exit(&mut self, poll: bool) {
        let until = Instant::now() + Duration::from_secs(10);
        while !self.world.net_dropped.load(Ordering::SeqCst) {
            if poll {
                self.drain();
            }
            assert!(Instant::now() < until, "the thread did not end");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Moves the backlog and a new poll into `seen`.
    fn drain(&mut self) {
        let events = std::mem::take(&mut self.backlog);
        self.seen.extend(events);
        let polled = self.session.poll(Instant::now());
        self.seen.extend(polled);
    }

    fn choose(&mut self, world: u32) {
        self.session
            .send(
                Instant::now(),
                Command::Account(AccountCommand::ChooseWorld {
                    world: WorldId(world),
                }),
            )
            .expect("choose");
    }

    fn progress(&self) -> Vec<LoginStage> {
        self.seen
            .iter()
            .filter_map(|e| match e {
                Event::Account(AccountEvent::LoginProgress { stage }) => Some(*stage),
                _ => None,
            })
            .collect()
    }

    fn refused(&self) -> Option<(LoginRefusal, Option<String>)> {
        self.seen.iter().find_map(|e| match e {
            Event::Account(AccountEvent::LoginRefused { reason, detail, .. }) => {
                Some((*reason, detail.clone()))
            }
            _ => None,
        })
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

    /// The handshake and the first acknowledgement, from `addr`.
    fn connect(&self, run: &Run, seed: u32, addr: SocketAddrV4) {
        run.world.deliver(self.challenge(seed), addr);
        run.world.deliver(
            self.control(OptionalHeaders {
                cumulative_ack: Some(0),
                ..OptionalHeaders::default()
            }),
            addr,
        );
    }

    fn referral(&self, cookie: u64, target: SocketAddrV4) -> Vec<u8> {
        self.control(OptionalHeaders {
            world_referral: Some(WorldReferralHeader {
                referral_cookie: cookie,
                sin_family: 2,
                target_addr: target,
                reserved_word_10: 0,
            }),
            ..OptionalHeaders::default()
        })
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

fn is_disconnected(e: &Event) -> bool {
    matches!(e, Event::Disconnected { .. })
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
        run.drain();
        assert!(Instant::now() < until, "no bind; seen {:?}", run.seen);
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn version_payload(env: &[u8]) -> String {
    // envelopeFlags, then a one-byte length for a short string.
    let len = usize::from(*env.get(1).expect("len"));
    String::from_utf8(env.get(2..2 + len).expect("string").to_vec()).expect("ascii")
}

fn character_list() -> Vec<u8> {
    CharacterListResponse {
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
    .expect("list")
}

fn session_ready() -> Vec<u8> {
    PlayerSessionReady {
        player_id: 0x0100_0000_0000_0007,
        session_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .expect("ready")
}

/// Chooses Alpha and runs the login connection to the character list; returns the login
/// server.
fn to_character_list(run: &mut Run) -> Server {
    run.wait_event(is_world_list);
    run.choose(0);
    let (auth, dst) = run.wait_sent();
    assert_eq!(dst, LOGIN_SERVER);
    assert!(
        decode_datagram(&auth, false, None, true)
            .expect("decode")
            .opt_headers
            .session_auth
            .is_some()
    );
    let mut login = Server::new(0x0111, 0xBEEF_0001);
    login.connect(run, 0xBEEF_0001, LOGIN_SERVER);
    let req = login.event(0x12F, &[]);
    run.world.deliver(req, LOGIN_SERVER);
    let (op, _) = run.wait_blob(LOGIN_SERVER);
    assert_eq!(op, 0x12C);
    let status_ok = VersionStatusMessage {
        status: 1,
        server_version_info: "ok".to_owned(),
    }
    .encode();
    let d = login.blob(0x12D, &encode_event_envelope(&status_ok));
    run.world.deliver(d, LOGIN_SERVER);
    let d = login.event(0x6C, &character_list());
    run.world.deliver(d, LOGIN_SERVER);
    run.wait_event(|e| matches!(e, Event::Connected { .. }));
    login
}

/// From the character list into the world through a referral to `WORLD_SERVER`; returns
/// the world server.
fn into_world(run: &mut Run, login: &Server) -> Server {
    run.session
        .send(
            Instant::now(),
            Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
        )
        .expect("enter");
    let (op, _) = run.wait_blob(LOGIN_SERVER);
    assert_eq!(op, 0x6F);
    run.world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    loop {
        let (d, dst) = run.wait_sent();
        if dst == WORLD_SERVER
            && decode_datagram(&d, false, None, false)
                .expect("referral auth")
                .opt_headers
                .referral_auth
                .is_some()
        {
            break;
        }
    }
    let mut world = Server::new(0x0111, 0xBEEF_0002);
    world.connect(run, 0xBEEF_0002, WORLD_SERVER);
    let d = world.event(0x90, &session_ready());
    run.world.deliver(d, WORLD_SERVER);
    run.wait_event(|e| matches!(e, Event::WorldEntered { .. }));
    world
}

/// The world acknowledges what the client sent reliably after entering (the portal exit,
/// `0xF4`), so a later graceful disconnect need not wait for its flush.
fn ack_portal_exit(run: &mut Run, world: &Server) {
    let acked = loop {
        let (d, dst) = run.wait_sent();
        let Ok(pkt) = decode_datagram(&d, false, None, false) else {
            continue;
        };
        if dst == WORLD_SERVER
            && pkt
                .fragments
                .iter()
                .any(|f| f.header.blob_header_word == Some(0xF4))
        {
            break pkt.header.seq_num;
        }
    };
    run.world.deliver(
        world.control(OptionalHeaders {
            cumulative_ack: Some(acked),
            ..OptionalHeaders::default()
        }),
        WORLD_SERVER,
    );
}

// ---------------------------------------------------------------------------
// Tests: login
// ---------------------------------------------------------------------------

#[test]
fn i1_local_discovery_failure_reaches_no_network() {
    let mut run = start(Service::default(), false);
    run.wait_event(is_disconnected);
    // Not a refusal of the credentials: the start screen's category (§8.6).
    assert_eq!(run.refused(), Some((LoginRefusal::ClientNotAccepted, None)));
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
fn the_watchdog_is_kept_alive_through_every_stage() {
    let mut run = start(
        Service {
            alpha_queue: true,
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_world_list);
    use LoginStage::{Authenticating as A, JoiningWorld as J, ListingWorlds as L};
    // Start, S1, S2, S3; the list's start and each of its two worlds.
    assert_eq!(run.progress(), vec![A, A, A, A, L, L, L]);
    run.choose(0);
    // The chosen world, the queue's `Ready`, then the login connection.
    let _ = run.wait_sent();
    let login = Server::new(0x0111, 0xBEEF_0001);
    login.connect(&run, 0xBEEF_0001, LOGIN_SERVER);
    let until = Instant::now() + Duration::from_secs(10);
    while run.progress().len() < 10 {
        run.drain();
        assert!(Instant::now() < until, "{:?}", run.progress());
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(run.progress(), vec![A, A, A, A, L, L, L, J, J, J]);
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn bad_credentials_are_refused_with_their_category() {
    let mut run = start(
        Service {
            login: login_fault("Authentication rejected"),
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_disconnected);
    // The row shows no detail (§8.6).
    assert_eq!(run.refused(), Some((LoginRefusal::BadCredentials, None)));
    assert_eq!(run.world.binds(), 0);
}

#[test]
fn a_locked_account_carries_the_service_text() {
    let mut run = start(
        Service {
            login: login_fault("Account is locked\u{7}"),
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_disconnected);
    let (reason, detail) = run.refused().expect("refused");
    assert_eq!(reason, LoginRefusal::AccountNotAllowed);
    let detail = detail.expect("detail");
    assert!(detail.starts_with("Account is locked"), "{detail:?}");
    assert!(!detail.contains('\u{7}'));
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
    run.wait_event(is_disconnected);
    assert_eq!(run.refused(), Some((LoginRefusal::UnsupportedStep, None)));
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
    run.wait_event(is_disconnected);
    assert_eq!(
        run.refused(),
        Some((LoginRefusal::ServiceUnavailable, None))
    );
    assert_eq!(login_requests(&run.world), 0, "no LoginAccount was sent");
}

#[test]
fn credentials_go_only_to_urls_that_arrived_over_https() {
    // The authentication URL is `https://`, but it came from a `GetDatacenters` answer read
    // over plain HTTP (the install's URL): it is not trusted.
    let mut run = start_with(
        Service::default(),
        Some("http://gls.example.invalid/datacenter"),
    );
    run.wait_event(is_disconnected);
    assert_eq!(
        run.refused(),
        Some((LoginRefusal::ServiceUnavailable, None))
    );
    assert_eq!(login_requests(&run.world), 0, "no LoginAccount was sent");
}

#[test]
fn disconnect_during_the_first_exchange_sends_nothing_more() {
    let gate = Arc::new(Gate::default());
    let mut run = start(
        Service {
            gate: Some(Arc::clone(&gate)),
            ..Service::default()
        },
        true,
    );
    gate.wait_reached();
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
    gate.release();
    run.wait_exit(true);
    assert_eq!(
        login_requests(&run.world),
        0,
        "no LoginAccount after the cancel"
    );
    assert_eq!(run.world.requests().len(), 1, "{:?}", run.world.requests());
    assert_eq!(run.world.binds(), 0);
}

#[test]
fn dropping_the_session_during_the_first_exchange_ends_the_thread() {
    let gate = Arc::new(Gate::default());
    let run = start(
        Service {
            gate: Some(Arc::clone(&gate)),
            ..Service::default()
        },
        true,
    );
    gate.wait_reached();
    let Run {
        _dir,
        world,
        session,
        ..
    } = run;
    drop(session);
    gate.release();
    let until = Instant::now() + Duration::from_secs(10);
    while !world.net_dropped.load(Ordering::SeqCst) {
        assert!(Instant::now() < until, "the thread did not end");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(login_requests(&world), 0);
    assert_eq!(world.requests().len(), 1);
}

#[test]
fn a_full_command_queue_is_send_queue_full_and_moves_never_fill_it() {
    let gate = Arc::new(Gate::default());
    let (_dir, world, mut backend) = backend(
        Service {
            gate: Some(Arc::clone(&gate)),
            ..Service::default()
        },
        Some(GLS),
    );
    let now = Instant::now();
    backend.submit(now, Command::Connect).expect("connect");
    gate.wait_reached();
    // The thread is held in the first exchange: nothing drains the queue.
    let choose = || Command::Account(AccountCommand::ChooseWorld { world: WorldId(0) });
    for _ in 0..super::COMMAND_QUEUE {
        backend.submit(now, choose()).expect("room");
    }
    assert!(matches!(
        backend.submit(now, choose()),
        Err(NetError::SendQueueFull)
    ));
    for i in 0..1_000u16 {
        let mv = Command::Move {
            location: tessera_net::Location::default(),
            heading: f32::from(i),
            velocity: Vec3::new(1.0, 0.0, 0.0),
            on_ground: true,
        };
        backend.submit(now, mv).expect("a move takes the slot");
    }
    backend
        .submit(now, Command::Disconnect)
        .expect("disconnect");
    gate.release();
    drop(backend);
    let until = Instant::now() + Duration::from_secs(10);
    while !world.net_dropped.load(Ordering::SeqCst) {
        assert!(Instant::now() < until, "the thread did not end");
        std::thread::sleep(Duration::from_millis(2));
    }
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
    run.choose(1);
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
    run.choose(0);
    wait_binds(&mut run, 1);
    assert_eq!(login_requests(&run.world), 1);
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn an_unusable_login_server_refuses_the_world_not_the_login() {
    let mut run = start(
        Service {
            login_server: "nowhere.example.invalid:9001".to_owned(),
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_world_list);
    run.choose(0);
    run.wait_event(|e| {
        matches!(
            e,
            Event::Account(AccountEvent::WorldRefused {
                world: WorldId(0),
                reason: WorldRefusal::Other,
                ..
            })
        )
    });
    assert_eq!(run.world.binds(), 0);
    // The session stays: Beta can still be chosen.
    run.choose(1);
    wait_binds(&mut run, 1);
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
    run.choose(0);
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
    let beta_before = run
        .world
        .requests_to("https://gls.example.invalid/status/beta");
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
    // The statuses were asked again for the new list.
    assert!(
        run.world
            .requests_to("https://gls.example.invalid/status/beta")
            > beta_before
    );
    assert_eq!(run.world.binds(), 0);
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn a_cancel_while_queued_sends_no_leave_queue() {
    let mut run = start(
        Service {
            alpha_queue: true,
            queue_waiting: true,
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_world_list);
    run.choose(0);
    run.wait_event(|e| matches!(e, Event::Account(AccountEvent::LoginQueueChanged { .. })));
    let before = run.world.requests().len();
    run.session
        .send(Instant::now(), Command::Disconnect)
        .expect("disconnect");
    run.wait_exit(true);
    assert_eq!(run.world.requests().len(), before);
}

// ---------------------------------------------------------------------------
// Tests: the session
// ---------------------------------------------------------------------------

#[test]
fn the_session_reaches_the_character_list_and_the_world() {
    let mut run = start(Service::default(), true);
    run.wait_event(is_world_list);
    run.choose(0);
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
    login.connect(&run, 0xBEEF_0001, LOGIN_SERVER);
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
    let d = login.event(0x6C, &character_list());
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
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
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
    world.connect(&run, 0xBEEF_0002, WORLD_SERVER);
    // The world name arrives before `0x90` and is not lost.
    let name = WorldNameAnnouncement {
        world_name: LocalizedTokenText::literal("Alpha world"),
    }
    .to_bytes()
    .expect("name");
    let d = world.event(0x43, &name);
    run.world.deliver(d, WORLD_SERVER);
    let d = world.event(0x90, &session_ready());
    run.world.deliver(d, WORLD_SERVER);
    let entered = run.wait_event(|e| matches!(e, Event::WorldEntered { .. }));
    let Event::WorldEntered { player, name, .. } = entered else {
        unreachable!("matched")
    };
    assert_eq!(player.0, 0x0100_0000_0000_0007);
    assert_eq!(name, "Aldo");
    run.wait_event(|e| {
        matches!(
            e,
            Event::ChatMessage { channel: ChatChannel::System, text, .. } if text == "Alpha world"
        )
    });
    ack_portal_exit(&mut run, &world);
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
    // The graceful disconnect goes out to the world server before the thread ends.
    let mut said_disconnect = false;
    let until = Instant::now() + Duration::from_secs(10);
    while !said_disconnect {
        if let Some((d, dst)) = run.world.pop_sent() {
            said_disconnect = dst == WORLD_SERVER
                && decode_datagram(&d, false, None, false).is_ok_and(|p| p.opt_headers.disconnect);
        } else {
            assert!(Instant::now() < until, "no Disconnect was sent");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    run.wait_exit(false);
    assert_eq!(run.world.binds(), 1);
}

#[test]
fn without_a_referral_the_login_connection_carries_the_world() {
    let mut run = start(Service::default(), true);
    let mut login = to_character_list(&mut run);
    run.session
        .send(
            Instant::now(),
            Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
        )
        .expect("enter");
    let (op, _) = run.wait_blob(LOGIN_SERVER);
    assert_eq!(op, 0x6F);
    let d = login.event(0x90, &session_ready());
    run.world.deliver(d, LOGIN_SERVER);
    run.wait_event(|e| matches!(e, Event::WorldEntered { .. }));
    let name = WorldNameAnnouncement {
        world_name: LocalizedTokenText::literal("Login-carried"),
    }
    .to_bytes()
    .expect("name");
    let d = login.event(0x43, &name);
    run.world.deliver(d, LOGIN_SERVER);
    run.wait_event(|e| matches!(e, Event::ChatMessage { text, .. } if text == "Login-carried"));
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn an_eviction_at_the_character_list_ends_the_session() {
    let mut run = start(Service::default(), true);
    let mut login = to_character_list(&mut run);
    let evict = AccountEvictionNotice {
        boot_reason: 7,
        reason_text: LocalizedTokenText::literal("Maintenance"),
    }
    .to_bytes()
    .expect("eviction");
    let d = login.event(0xB3, &evict);
    run.world.deliver(d, LOGIN_SERVER);
    let end = run.wait_event(is_disconnected);
    assert_eq!(
        end,
        Event::Disconnected {
            reason: DisconnectReason::ServerClosed("Maintenance".to_owned())
        }
    );
}

#[test]
fn a_referral_outside_world_entry_is_ignored() {
    let mut run = start(Service::default(), true);
    let login = to_character_list(&mut run);
    run.world
        .deliver(login.referral(0x99, WORLD_SERVER), LOGIN_SERVER);
    let until = Instant::now() + Duration::from_millis(300);
    while Instant::now() < until {
        if let Some((_, dst)) = run.world.pop_sent() {
            assert_ne!(dst, WORLD_SERVER, "a stray referral was followed");
        }
        run.drain();
        std::thread::sleep(Duration::from_millis(5));
    }
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn a_second_referral_disconnects_the_replaced_world_connection() {
    let mut run = start(Service::default(), true);
    let login = to_character_list(&mut run);
    let world = into_world(&mut run, &login);
    ack_portal_exit(&mut run, &world);
    run.world
        .deliver(world.referral(0x55, WORLD_SERVER_2), WORLD_SERVER);
    let (mut to_new, mut old_disconnect) = (false, false);
    let until = Instant::now() + Duration::from_secs(10);
    while !(to_new && old_disconnect) {
        match run.world.pop_sent() {
            Some((d, dst)) => {
                let pkt = decode_datagram(&d, false, None, false);
                if dst == WORLD_SERVER_2 {
                    to_new |= pkt
                        .as_ref()
                        .is_ok_and(|p| p.opt_headers.referral_auth.is_some());
                }
                if dst == WORLD_SERVER {
                    old_disconnect |= pkt.is_ok_and(|p| p.opt_headers.disconnect);
                }
            }
            None => {
                run.drain();
                assert!(
                    Instant::now() < until,
                    "new {to_new}, old disconnect {old_disconnect}"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn a_failed_chat_attachment_is_not_retried_at_once() {
    let mut run = start(
        Service {
            chat: "chat.example.invalid:9200".to_owned(),
            failing_chat: true,
            ..Service::default()
        },
        true,
    );
    let login = to_character_list(&mut run);
    let _world = into_world(&mut run, &login);
    let until = Instant::now() + Duration::from_secs(10);
    while run.world.chat_binds.load(Ordering::SeqCst) == 0 {
        assert!(Instant::now() < until, "no chat attempt");
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(run.world.chat_binds.load(Ordering::SeqCst), 1);
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn a_full_event_queue_stops_reading_the_socket() {
    let mut run = start(Service::default(), true);
    run.wait_event(is_world_list);
    run.choose(0);
    let _ = run.wait_sent();
    let mut login = Server::new(0x0111, 0xBEEF_0001);
    login.connect(&run, 0xBEEF_0001, LOGIN_SERVER);
    let total = 3 * EVENT_QUEUE;
    for i in 0..total {
        let q = LoginQueueStatusMessage {
            login_result: 0,
            detail_text: String::new(),
            queue_position: u32::try_from(i + 1).expect("small"),
            queue_total: 10_000,
        }
        .encode();
        let d = login.blob(0x130, &encode_event_envelope(&q));
        run.world.deliver(d, LOGIN_SERVER);
    }
    // Without a poll the thread holds back: datagrams stay unread.
    std::thread::sleep(Duration::from_millis(500));
    assert!(run.world.inbound_len() > 0, "the socket was read on");
    // Polling drains everything, in order.
    let until = Instant::now() + Duration::from_secs(20);
    let mut last = 0u32;
    while last < u32::try_from(total).expect("small") {
        for e in run.session.poll(Instant::now()) {
            if let Event::Account(AccountEvent::LoginQueueChanged {
                position: Some(p), ..
            }) = e
            {
                assert!(p > last, "out of order: {p} after {last}");
                last = p;
            }
        }
        assert!(Instant::now() < until, "only {last} of {total}");
        std::thread::sleep(Duration::from_millis(1));
    }
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn a_transport_refusal_during_login_is_account_not_allowed() {
    let mut run = start(Service::default(), true);
    run.wait_event(is_world_list);
    run.choose(0);
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
    run.wait_event(is_disconnected);
    assert_eq!(run.refused(), Some((LoginRefusal::AccountNotAllowed, None)));
}

// ---------------------------------------------------------------------------
// Tests: the session driver on a virtual clock
// ---------------------------------------------------------------------------

fn new_world(service: Service) -> World {
    World {
        service: Arc::new(service),
        shared: Arc::new(Mutex::new(Shared::default())),
        net_dropped: Arc::new(AtomicBool::new(false)),
        chat_binds: Arc::new(AtomicUsize::new(0)),
    }
}

/// A driver at the character list, with a ticket of `lifetime` seconds.
fn driver_at_characters(
    service: Service,
    lifetime: u32,
) -> (World, SessionDriver<FakeNet>, Server) {
    let world = new_world(service);
    let mut net = FakeNet(world.clone());
    let version =
        VersionInfo::new("9999", "00000000-0000-0000-0000-000000000000", 1).expect("version");
    let auth = "https://auth.example.invalid/login";
    let sub = "SUB0000000000000000000000";
    let handoff = SessionHandoff::new(
        "login.example.invalid:9001",
        0,
        sub,
        TICKET,
        version.clone(),
        "English",
        "",
        auth,
        lifetime,
    )
    .expect("handoff");
    let start = SessionStart {
        handoff,
        login_peer: LOGIN_SERVER,
        refresh_secure: true,
        refresher: TicketRefresher::new(auth, sub, TICKET, lifetime, 0.0),
        version,
        codecs: crate::chat_session::ChatCodecs::default(),
        codec_uuids: Vec::new(),
        schema: SchemaView(Arc::new(records())),
    };
    let mut d = SessionDriver::start(&mut net, start, 0.0).expect("start");
    let mut login = Server::new(0x0111, 0xBEEF_0001);
    world.deliver(login.challenge(0xBEEF_0001), LOGIN_SERVER);
    world.deliver(
        login.control(OptionalHeaders {
            cumulative_ack: Some(0),
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    d.step(0.0, true);
    world.deliver(login.event(0x6C, &character_list()), LOGIN_SERVER);
    d.step(0.1, true);
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::Connected { .. }))
    );
    (world, d, login)
}

#[test]
fn a_world_that_never_answers_the_entry_returns_to_the_character_list() {
    let (world, mut d, login) = driver_at_characters(Service::default(), 0);
    let enter = Command::EnterWorld {
        character: "Aldo".to_owned(),
    };
    d.command(&enter, 1.0).expect("enter");
    for t in [20.0, 40.0, 60.0] {
        // The login server stays alive meanwhile.
        world.deliver(
            login.control(OptionalHeaders {
                cumulative_ack: Some(0),
                ..OptionalHeaders::default()
            }),
            LOGIN_SERVER,
        );
        d.step(t, true);
        assert!(
            !d.take_events()
                .iter()
                .any(|e| matches!(e, Event::EnterWorldFailed { .. })),
            "too early at {t}"
        );
    }
    d.step(61.5, true);
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::EnterWorldFailed { character, .. } if character == "Aldo"
        )),
        "{events:?}"
    );
    // Back at the character list: the character can be entered again.
    d.command(&enter, 62.0).expect("again");
}

#[test]
fn no_ticket_refresh_runs_after_the_shutdown_began() {
    let gate = Arc::new(Gate::default());
    let (world, mut d, _login) = driver_at_characters(
        Service {
            refresh_gate: Some(Arc::clone(&gate)),
            ..Service::default()
        },
        60,
    );
    let refreshes = || {
        world
            .requests()
            .iter()
            .filter(|r| r.2.as_deref().is_some_and(|a| a.contains("RefreshTicket")))
            .count()
    };
    d.step(31.0, true);
    gate.wait_reached();
    assert!(d.refresh_running());
    d.begin_shutdown(32.0);
    assert!(!d.refresh_running(), "the shutdown stops the refresh");
    gate.release();
    for t in [45.0, 50.0, 59.0, 90.0] {
        let _ = d.shutdown_step(t);
        d.step(t, true);
    }
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(refreshes(), 1, "none started after the shutdown");
}

// ---------------------------------------------------------------------------
// OE-T10: no canary in any log line, at every level, on every thread
// ---------------------------------------------------------------------------

mod capture {
    //! A process-wide subscriber that records every event and span of every thread while a
    //! capture is active (the connector logs on its own threads).
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    static ACTIVE: AtomicBool = AtomicBool::new(false);
    static BUF: Mutex<String> = Mutex::new(String::new());
    static ONE: Mutex<()> = Mutex::new(());

    struct All;

    struct Fields<'a>(&'a mut String);

    impl tracing::field::Visit for Fields<'_> {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            self.0.push_str(&format!("{}={value:?} ", field.name()));
        }
    }

    /// The connector's threads and the capturing test's own (other tests of this binary
    /// run alongside and are not part of the check).
    fn watched() -> bool {
        std::thread::current()
            .name()
            .is_some_and(|n| n.starts_with("tessera-official") || n.contains("oe_t10"))
    }

    fn with_buf(f: impl FnOnce(&mut String)) {
        if ACTIVE.load(Ordering::SeqCst)
            && watched()
            && let Ok(mut b) = BUF.lock()
        {
            f(&mut b);
        }
    }

    impl tracing::Subscriber for All {
        fn register_callsite(
            &self,
            _: &'static tracing::Metadata<'static>,
        ) -> tracing::subscriber::Interest {
            tracing::subscriber::Interest::sometimes()
        }
        fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
            Some(tracing::level_filters::LevelFilter::TRACE)
        }
        fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
            ACTIVE.load(Ordering::SeqCst) && watched()
        }
        fn new_span(&self, attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            with_buf(|b| {
                b.push_str(attrs.metadata().name());
                attrs.record(&mut Fields(b));
                b.push('\n');
            });
            tracing::span::Id::from_u64(1)
        }
        fn record(&self, _: &tracing::span::Id, values: &tracing::span::Record<'_>) {
            with_buf(|b| {
                values.record(&mut Fields(b));
                b.push('\n');
            });
        }
        fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
        fn event(&self, event: &tracing::Event<'_>) {
            with_buf(|b| {
                b.push_str(event.metadata().target());
                b.push(' ');
                event.record(&mut Fields(b));
                b.push('\n');
            });
        }
        fn enter(&self, _: &tracing::span::Id) {}
        fn exit(&self, _: &tracing::span::Id) {}
    }

    /// Runs `f` with every thread's output captured; returns the text.
    pub fn logs(f: impl FnOnce()) -> String {
        static INSTALLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let global =
            *INSTALLED.get_or_init(|| tracing::subscriber::set_global_default(All).is_ok());
        assert!(global, "the capturing subscriber is the global default");
        let _one = ONE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Ok(mut b) = BUF.lock() {
            b.clear();
        }
        ACTIVE.store(true, Ordering::SeqCst);
        tracing::callsite::rebuild_interest_cache();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        ACTIVE.store(false, Ordering::SeqCst);
        if let Err(p) = r {
            std::panic::resume_unwind(p);
        }
        BUF.lock().map(|b| b.clone()).unwrap_or_default()
    }
}

#[test]
fn oe_t10_the_live_connector_logs_no_credential_at_any_level() {
    let logs = capture::logs(|| {
        // A login to the world, and a refused one.
        let mut run = start(Service::default(), true);
        let login = to_character_list(&mut run);
        let _world = into_world(&mut run, &login);
        let _ = run.session.send(Instant::now(), Command::Disconnect);
        run.wait_exit(true);
        let mut bad = start(
            Service {
                login: login_fault("Authentication rejected"),
                ..Service::default()
            },
            true,
        );
        bad.wait_event(is_disconnected);
        bad.wait_exit(true);
    });
    assert!(
        logs.contains("official login"),
        "the capture saw the connector"
    );
    assert!(!logs.contains("canary"), "{logs}");
    assert!(!logs.contains(TICKET), "{logs}");
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

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
    let local = LoginError::ConfigInvalid {
        reason: String::new(),
    };
    assert_eq!(login_refusal(&local), LoginRefusal::ClientNotAccepted);
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
