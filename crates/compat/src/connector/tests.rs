// SPDX-License-Identifier: GPL-3.0-or-later
//! The live connector against a synthetic service: HTTP answers from fixtures and a scripted
//! UDP server over an in-memory transport. Nothing here opens a socket or resolves a name;
//! every endpoint is an RFC 2606 `.example.invalid` placeholder or a documentation address
//! (RFC 5737).

mod deadlines;
mod diagnostics;
mod world_feed;

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
    Backend, CharacterEvent, ChatChannel, Command, DisconnectReason, Event, NetError, Session, Vec3,
};

use super::data::SchemaView;
use super::data::{InstallData, LocalPaths};
use super::net::{ConnectorNet, NetFactory, SendHttpClient, carries_secret, is_https};
// The synthetic world's builders, for the world-feed tests (`world_feed`).
use super::session_phase::{
    DRIVER_EVENT_CAP, PORTAL_LOADING_TIMEOUT_SEC, SessionDriver, SessionStart, movement_input,
    refusal_cut,
};
// The synthetic service's world builders, used by `world_feed`.
use super::test_support::{
    DICTIONARY, NPC, NPC_AT, NPC_NAME, NPC_TEMPLATE, chat_line, entity_batch, entity_properties,
    position,
};
use super::worker::SHUTDOWN_LIMIT;
use super::{EVENT_QUEUE, LiveConfig, LiveConnector};
use crate::consent::{self, UserConfirmation, WarningText};
use crate::credentials::Credentials;
use crate::login::soap_encoder::{GLS_NAMESPACE, SOAP_ENVELOPE_NAMESPACE};
use crate::login::{
    HttpClient, HttpMethod, HttpRequest, HttpResponse, LoginError, SessionHandoff, TicketRefresher,
    VersionInfo,
};
use crate::net_access::{NetworkAccess, Revocation};
use crate::session::{
    BitTreeTable, ConnectChallengeHeader, ConnectionState, DatagramHeader, IsaacCipher,
    LoginQueueStatusMessage, OPCODE_BATCH_CONTAINER, OPCODE_SYSTEM_NOTIFICATION, OptionalHeaders,
    SessionError, UdpDatagramTransport, VersionStatusMessage, WorldReferralHeader, decode_datagram,
    encode_batch_container, encode_datagram, encode_variable_u32, split_blob,
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
fn datacenters(service: &Service) -> String {
    let Service {
        auth,
        chat,
        launcher_url,
        alpha_status_url,
        ..
    } = service;
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="{SOAP_ENVELOPE_NAMESPACE}"><soap:Body>
<GetDatacentersResponse xmlns="{GLS_NAMESPACE}"><GetDatacentersResult><Datacenter>
<Name>LOTRO</Name><AuthServer>{auth}</AuthServer><PatchServer>patch.example.invalid:1</PatchServer>
<LauncherConfigurationServer>{launcher_url}</LauncherConfigurationServer>
<Worlds>
<World><Name>Alpha</Name><LoginServerUrl>(invalid service specified)</LoginServerUrl><ChatServerUrl>{chat}</ChatServerUrl><StatusServerUrl>{alpha_status_url}</StatusServerUrl><Order>1</Order></World>
<World><Name>Beta</Name><LoginServerUrl>(invalid service specified)</LoginServerUrl><ChatServerUrl></ChatServerUrl><StatusServerUrl>https://gls.example.invalid/status/beta</StatusServerUrl><Order>2</Order></World>
</Worlds></Datacenter></GetDatacentersResult></GetDatacentersResponse></soap:Body></soap:Envelope>"#
    )
}

fn launcher(queue_url: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<configuration><appSettings>
<add key="GameClient.WIN32.ArgTemplate" value="-a {{SUBSCRIPTION}}" />
<add key="WorldQueue.LoginQueue.URL" value="{queue_url}" />
<add key="WorldQueue.TakeANumber.Parameters" value="command=TakeANumber&amp;subscription={{0}}&amp;ticket={{1}}&amp;ticket_type=GLS&amp;queue_url={{2}}" />
<add key="WorldQueue.LeaveQueue.Parameters" value="command=LeaveQueue&amp;subscription={{0}}&amp;context={{1}}&amp;ticket_type=GLS&amp;queue_url={{2}}" />
</appSettings></configuration>"#
    )
}

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

/// A gate that holds an answer (or a lookup) until the test opens it; the first `pass`
/// waits go through.
#[derive(Debug, Default)]
struct Gate {
    open: Mutex<bool>,
    cv: Condvar,
    reached: AtomicBool,
    pass: AtomicUsize,
}

impl Gate {
    /// A gate that lets the first `n` waits through.
    fn passing(n: usize) -> Self {
        Self {
            pass: AtomicUsize::new(n),
            ..Self::default()
        }
    }

    fn wait(&self) {
        let mut p = self.pass.load(Ordering::SeqCst);
        while p > 0 {
            match self
                .pass
                .compare_exchange(p, p - 1, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return,
                Err(now) => p = now,
            }
        }
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
    /// Holds the chat server's name lookup until the test opens it.
    chat_gate: Option<Arc<Gate>>,
    /// Holds the answers of Alpha's status (`Gate::passing` lets the world list's through).
    status_gate: Option<Arc<Gate>>,
    /// The chat binder makes working sockets over the chat server's own synthetic queue.
    chat_socket: bool,
    /// The synthetic network follows the run's network-access value, Withdraw's
    /// `revoke_all` included (the test holds [`crate::net_access::TEST_REVOKE_LOCK`]);
    /// otherwise only the value's own revocation (the reaper's), which `revoke_all` in
    /// another test does not touch.
    follow_withdraw: bool,
    /// `Datacenter/LauncherConfigurationServer`.
    launcher_url: String,
    /// Alpha's `StatusServerUrl`.
    alpha_status_url: String,
    /// `WorldQueue.LoginQueue.URL`.
    queue_url: String,
    /// The service answers plain `http://` requests too (otherwise they fail as a refused
    /// connection would).
    plain_http: bool,
    /// Addresses (host and path, without the scheme) whose HTTPS request fails as a failed
    /// TLS handshake would.
    https_down: Vec<String>,
    /// HTTPS answers that replace the routed one: (HTTPS URL, status, body).
    answers: Vec<(String, u16, String)>,
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
            chat_gate: None,
            status_gate: None,
            chat_socket: false,
            follow_withdraw: false,
            launcher_url: "https://gls.example.invalid/launcher.xml".to_owned(),
            alpha_status_url: "https://gls.example.invalid/status/alpha".to_owned(),
            queue_url: "https://gls.example.invalid/queue".to_owned(),
            plain_http: false,
            https_down: Vec::new(),
            answers: Vec::new(),
        }
    }
}

/// `url` as the routing of [`World::answer`] sees it: its HTTPS form (the scheme ignored,
/// an `http://` port dropped as the upgrade drops it).
fn route(url: &str) -> String {
    super::net::https_form(url).map_or_else(|| url.to_owned(), |h| h.url)
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
    /// What the chat server sends (the chat sockets read only this).
    chat_inbound: VecDeque<(Vec<u8>, SocketAddrV4)>,
    /// Requests, sends, binds and lookups tried while the right was revoked (refused, but
    /// counted: none may be tried).
    dead_attempts: usize,
    /// Name lookups made.
    resolves: usize,
    /// Local-address probes made.
    probes: usize,
    /// HTTPS answers set while the test runs ([`World::set_answers`]); they come before
    /// `Service::answers`.
    answers: Vec<(String, u16, String)>,
}

/// The network right as the synthetic network sees it (`Service::follow_withdraw`).
#[derive(Debug, Default)]
struct FakeRight {
    access: Option<NetworkAccess>,
    revocation: Option<Revocation>,
}

#[derive(Debug, Clone)]
struct World {
    service: Arc<Service>,
    shared: Arc<Mutex<Shared>>,
    /// The connector's network was dropped (its thread ended).
    net_dropped: Arc<AtomicBool>,
    chat_binds: Arc<AtomicUsize>,
    right: Arc<Mutex<FakeRight>>,
}

fn revoked_io() -> SessionError {
    SessionError::Io("the network access was revoked".to_owned())
}

impl World {
    /// Whether the run's network right holds, as the real network checks it before every
    /// operation (no right: nothing is recorded, sent or bound).
    fn live(&self) -> bool {
        let r = self.right.lock().expect("right");
        if let Some(a) = &r.access {
            return a.is_live();
        }
        r.revocation.as_ref().is_none_or(|v| !v.is_revoked())
    }

    fn answer(&self, req: &HttpRequest) -> Result<HttpResponse, LoginError> {
        let mut s = self.shared.lock().expect("shared");
        if !self.live() {
            s.dead_attempts += 1;
            return Err(LoginError::ServiceUnavailable {
                reason: "revoked".to_owned(),
            });
        }
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
        // Every attempt is recorded above; a plain request fails unless the service answers
        // plain HTTP, and an address in `https_down` fails over HTTPS.
        let plain = !is_https(&req.url);
        if plain && !self.service.plain_http {
            return Err(LoginError::ServiceUnavailable {
                reason: "the connection was refused".to_owned(),
            });
        }
        let rest = req.url.split_once("://").map_or("", |(_, r)| r);
        if !plain && self.service.https_down.iter().any(|d| d == rest) {
            return Err(LoginError::ServiceUnavailable {
                reason: "the TLS handshake or certificate check failed".to_owned(),
            });
        }
        let url = route(&req.url);
        let svc = &self.service;
        if !plain {
            let late = self.shared.lock().expect("shared").answers.clone();
            if let Some((_, code, body)) = late.iter().chain(&svc.answers).find(|a| a.0 == url) {
                return Ok(HttpResponse {
                    status_code: *code,
                    body: body.as_bytes().to_vec(),
                });
            }
        }
        match url.as_str() {
            "https://gls.example.invalid/datacenter" => {
                if let Some(g) = &self.service.gate {
                    g.wait();
                }
                ok(&datacenters(&self.service))
            }
            u if u == route(&svc.launcher_url) => ok(&launcher(&svc.queue_url)),
            u if u == route(&svc.alpha_status_url) => {
                if let Some(g) = &self.service.status_gate {
                    g.wait();
                }
                ok(&status(
                    self.service.alpha_queue,
                    &self.service.login_server,
                ))
            }
            "https://gls.example.invalid/status/beta" => match &self.service.beta_status {
                Some(s) => ok(s),
                None => Ok(HttpResponse {
                    status_code: 404,
                    body: Vec::new(),
                }),
            },
            u if u == route(&svc.queue_url) => {
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
            u if u == route(&svc.auth) => ok(&self.service.login),
            _ => Ok(HttpResponse {
                status_code: 404,
                body: Vec::new(),
            }),
        }
    }

    fn requests(&self) -> Vec<Recorded> {
        self.shared.lock().expect("shared").requests.clone()
    }

    /// HTTPS answers that replace the routed ones from now on.
    fn set_answers(&self, answers: Vec<(String, u16, String)>) {
        self.shared.lock().expect("shared").answers = answers;
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

    /// A datagram from the chat server, to the chat sockets.
    fn deliver_chat(&self, datagram: Vec<u8>) {
        self.shared
            .lock()
            .expect("shared")
            .chat_inbound
            .push_back((datagram, CHAT_SERVER));
    }

    fn chat_inbound_len(&self) -> usize {
        self.shared.lock().expect("shared").chat_inbound.len()
    }

    fn dead_attempts(&self) -> usize {
        self.shared.lock().expect("shared").dead_attempts
    }

    fn resolves(&self) -> usize {
        self.shared.lock().expect("shared").resolves
    }

    fn probes(&self) -> usize {
        self.shared.lock().expect("shared").probes
    }

    /// Takes every datagram sent so far.
    fn take_sent(&self) -> Vec<(Vec<u8>, SocketAddrV4)> {
        self.shared
            .lock()
            .expect("shared")
            .udp
            .outbound
            .drain(..)
            .collect()
    }
}

#[derive(Debug)]
struct FakeHttp(World);

impl HttpClient for FakeHttp {
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        self.0.answer(request)
    }
}

/// A synthetic socket: the session's (`false`) or a chat socket (`true`, reading the chat
/// server's queue). Both record what they send in one queue.
#[derive(Debug)]
struct FakeUdp(World, bool);

impl UdpDatagramTransport for FakeUdp {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        Ok(LOCAL)
    }

    fn send_to(&mut self, buf: &[u8], target: SocketAddrV4) -> Result<usize, SessionError> {
        // The right is checked under the lock, so a test that revokes and then empties the
        // queue sees every datagram sent before the revocation.
        let mut s = self.0.shared.lock().expect("shared");
        if !self.0.live() {
            s.dead_attempts += 1;
            return Err(revoked_io());
        }
        s.udp.outbound.push_back((buf.to_vec(), target));
        Ok(buf.len())
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        if !self.0.live() {
            return Err(revoked_io());
        }
        let mut s = self.0.shared.lock().expect("shared");
        let next = if self.1 {
            s.chat_inbound.pop_front()
        } else {
            s.udp.inbound.pop_front()
        };
        drop(s);
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

    fn is_live(&self) -> bool {
        self.0.live()
    }

    fn resolve(&mut self, host: &str, port: u16) -> Result<SocketAddrV4, SessionError> {
        let mut s = self.0.shared.lock().expect("shared");
        if !self.0.live() {
            s.dead_attempts += 1;
            return Err(revoked_io());
        }
        s.resolves += 1;
        drop(s);
        match (host, port) {
            ("login.example.invalid", 9001) => Ok(LOGIN_SERVER),
            ("chat.example.invalid", 9200) => {
                if let Some(g) = &self.0.service.chat_gate {
                    g.wait();
                }
                Ok(CHAT_SERVER)
            }
            _ => Err(SessionError::Io("unknown synthetic name".to_owned())),
        }
    }

    fn local_ipv4_toward(&mut self, _remote: SocketAddrV4) -> Option<Ipv4Addr> {
        let mut s = self.0.shared.lock().expect("shared");
        if !self.0.live() {
            s.dead_attempts += 1;
            return None;
        }
        s.probes += 1;
        Some(*LOCAL.ip())
    }

    fn bind_udp(
        &mut self,
        _local_ip: Ipv4Addr,
        _explicit_base_port: u16,
        _compute_unique: bool,
    ) -> Result<FakeUdp, SessionError> {
        let mut s = self.0.shared.lock().expect("shared");
        if !self.0.live() {
            s.dead_attempts += 1;
            return Err(revoked_io());
        }
        s.binds += 1;
        drop(s);
        Ok(FakeUdp(self.0.clone(), false))
    }

    fn chat_binder(
        &self,
    ) -> Option<Box<dyn FnMut() -> Result<FakeUdp, crate::play::PlayError> + Send>> {
        if self.0.service.chat_socket {
            let world = self.0.clone();
            return Some(Box::new(move || {
                world.chat_binds.fetch_add(1, Ordering::SeqCst);
                Ok(FakeUdp(world.clone(), true))
            }));
        }
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
        let mut r = self.0.right.lock().expect("right");
        r.revocation = Some(access.revocation());
        if self.0.service.follow_withdraw {
            r.access = Some(access);
        }
        drop(r);
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
        right: Arc::default(),
    };
    let config = LiveConfig {
        paths: LocalPaths {
            install_root: root,
            user_settings_dir: None,
            network_version: Some("9999".to_owned()),
        },
        loader: Box::new(|_, _| Ok(Arc::new(records()) as Arc<dyn InstallData>)),
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
        self.blob_to(opcode, None, envelope)
    }

    fn blob_to(&mut self, opcode: u32, recipient: Option<u64>, envelope: &[u8]) -> Vec<u8> {
        self.seq += 1;
        let frags = split_blob(
            2,
            (self.seq & 0xFFFF) as u16,
            1,
            0,
            recipient,
            opcode,
            envelope,
        )
        .expect("split");
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

    /// An event addressed to `recipient` (`FragmentedBlob` field 7).
    fn event_to(&mut self, opcode: u32, recipient: u64, inner: &[u8]) -> Vec<u8> {
        self.blob_to(opcode, Some(recipient), &encode_event_envelope(inner))
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

    /// [`Self::connect`] on a world without a running connector.
    fn connect_to(&self, world: &World, seed: u32, addr: SocketAddrV4) {
        world.deliver(self.challenge(seed), addr);
        world.deliver(
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
    character_list_of(&[(0x0200_0000_0000_0042, "Aldo")])
}

/// A character list (`0x6C`) of `characters` (ID, name).
fn character_list_of(characters: &[(u64, &str)]) -> Vec<u8> {
    CharacterListResponse {
        characters: characters
            .iter()
            .map(|(id, name)| CharacterSummaryEntry {
                character_id: *id,
                name: (*name).to_owned(),
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
            })
            .collect(),
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
    // The client's loading is done (`official-messages-play.md` §4.5 item 4).
    run.session
        .send(Instant::now(), Command::WorldLoaded)
        .expect("loaded");
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

// ---------------------------------------------------------------------------
// HTTPS first, then the address as given (`login_phase`; decision 0044 as amended)
// ---------------------------------------------------------------------------

/// Held by every test whose login goes over plain HTTP, so the log-capturing test sees
/// only its own `step over plain HTTP` lines.
static PLAIN_STEPS: Mutex<()> = Mutex::new(());

fn plain_lock() -> std::sync::MutexGuard<'static, ()> {
    PLAIN_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The plain-HTTP requests the service saw.
fn plain_requests(world: &World) -> Vec<Recorded> {
    world
        .requests()
        .into_iter()
        .filter(|r| !is_https(&r.1))
        .collect()
}

/// Whether a recorded request carries the password or the ticket.
fn recorded_carries_secret(r: &Recorded) -> bool {
    let mut headers = Vec::new();
    if let Some(a) = &r.2 {
        headers.push(("SOAPAction".to_owned(), a.clone()));
        headers.push(("Content-Type".to_owned(), "text/xml".to_owned()));
    }
    if r.3.starts_with(b"command=") {
        headers.push((
            "Content-Type".to_owned(),
            "application/x-www-form-urlencoded".to_owned(),
        ));
    }
    carries_secret(&HttpRequest {
        method: r.0,
        url: r.1.clone(),
        headers,
        body: Vec::new(),
        timeout_sec: 30,
        require_tls_validation: true,
    })
}

/// Alpha's status in the world list `e`.
fn alpha_in_list(e: &Event) -> Option<tessera_net::account::WorldStatus> {
    match e {
        Event::Account(AccountEvent::WorldList { worlds }) => {
            worlds.iter().find(|w| w.id == WorldId(0)).map(|w| w.status)
        }
        _ => None,
    }
}

const ALPHA_HTTP: &str = "http://gls.example.invalid/status/alpha";
const ALPHA_HTTPS: &str = "https://gls.example.invalid/status/alpha";
const AUTH_HTTP: &str = "http://auth.example.invalid/login";
const AUTH_HTTPS: &str = "https://auth.example.invalid/login";

/// A service whose every address is `http://`; `down` lists the host and path of those
/// whose HTTPS form fails as a refused TLS handshake would.
fn http_chain(down: &[&str]) -> Service {
    Service {
        auth: AUTH_HTTP.to_owned(),
        launcher_url: "http://gls.example.invalid/launcher.xml".to_owned(),
        alpha_status_url: ALPHA_HTTP.to_owned(),
        queue_url: "http://gls.example.invalid/queue".to_owned(),
        alpha_queue: true,
        plain_http: true,
        https_down: down.iter().map(|d| (*d).to_owned()).collect(),
        ..Service::default()
    }
}

/// Every address of [`http_chain`], as host and path.
const CHAIN: [&str; 5] = [
    "gls.example.invalid/datacenter",
    "gls.example.invalid/launcher.xml",
    "auth.example.invalid/login",
    "gls.example.invalid/status/alpha",
    "gls.example.invalid/queue",
];

#[test]
fn an_http_datacenter_url_is_asked_over_https_and_the_login_goes_on() {
    let mut run = start_with(
        Service {
            plain_http: true,
            ..Service::default()
        },
        Some("http://gls.example.invalid/datacenter"),
    );
    run.wait_event(is_world_list);
    assert_eq!(run.refused(), None);
    let reqs = run.world.requests();
    assert_eq!(
        reqs.first().map(|r| r.1.as_str()),
        Some("https://gls.example.invalid/datacenter")
    );
    assert!(plain_requests(&run.world).is_empty(), "{reqs:?}");
    assert_eq!(login_requests(&run.world), 1);
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn an_https_capable_chain_never_touches_plain_http() {
    // Every address `http://` (one with a plain-HTTP port), every HTTPS form working, the
    // service answering plain HTTP too: nothing goes over plain HTTP.
    let mut run = start_with(
        Service {
            auth: "http://auth.example.invalid:8080/login".to_owned(),
            queue_url: "HTTP://gls.example.invalid/queue".to_owned(),
            ..http_chain(&[])
        },
        Some("http://gls.example.invalid/datacenter"),
    );
    run.wait_event(is_world_list);
    run.choose(0);
    wait_binds(&mut run, 1);
    let urls: Vec<String> = run.world.requests().into_iter().map(|r| r.1).collect();
    // The port given for plain HTTP is dropped: the default HTTPS port.
    assert!(urls.contains(&AUTH_HTTPS.to_owned()));
    assert!(urls.contains(&"https://gls.example.invalid/launcher.xml".to_owned()));
    assert!(urls.contains(&"https://gls.example.invalid/queue".to_owned()));
    assert!(plain_requests(&run.world).is_empty(), "{urls:?}");
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn an_http_chain_whose_https_form_is_refused_logs_in_over_plain_http() {
    // Taken before the capture starts, so no other test's plain step is logged meanwhile.
    let _one = plain_lock();
    let logs = capture::logs(|| {
        let mut run = start_with(
            http_chain(&CHAIN),
            Some("http://gls.example.invalid/datacenter"),
        );
        run.wait_event(is_world_list);
        run.choose(0);
        wait_binds(&mut run, 1);
        let reqs = run.world.requests();
        // Each step: once at its HTTPS form (refused), then once as given.
        for step in CHAIN {
            let https = reqs
                .iter()
                .filter(|r| r.1 == format!("https://{step}"))
                .count();
            let plain = reqs
                .iter()
                .filter(|r| r.1 == format!("http://{step}"))
                .count();
            assert!(https >= 1 && https == plain, "{step}: {https} {plain}");
        }
        // The account and the ticket went over plain HTTP, the chain worked no other way.
        assert!(
            plain_requests(&run.world)
                .iter()
                .filter(|r| recorded_carries_secret(r))
                .count()
                >= 2
        );
        let _ = run.session.send(Instant::now(), Command::Disconnect);
        run.wait_exit(true);
    });
    let lines: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official login: step over plain HTTP"))
        .collect();
    // One line per step (the status URL was asked for the list and for the join), with the
    // step's name only.
    let mut steps: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.split("step=\"").nth(1)?.split('"').next())
        .collect();
    steps.sort_unstable();
    assert_eq!(
        steps,
        [
            "auth server",
            "datacenter document",
            "launcher configuration",
            "queue URL",
            "status URL"
        ],
        "{logs}"
    );
    for l in &lines {
        // The message and the step, nothing else.
        assert!(
            l.ends_with(&format!(
                "step=\"{}\" ",
                l.split("step=\"")
                    .nth(1)
                    .and_then(|r| r.split('"').next())
                    .unwrap_or("")
            )),
            "{l}"
        );
    }
    assert!(!logs.contains("example.invalid"), "{logs}");
    assert!(!logs.contains("/login"), "{logs}");
    assert!(!logs.contains("canary"), "{logs}");
}

#[test]
fn a_failed_https_form_of_one_step_sends_only_that_step_over_plain_http() {
    let _one = plain_lock();
    let mut run = start(
        Service {
            auth: AUTH_HTTP.to_owned(),
            plain_http: true,
            https_down: vec!["auth.example.invalid/login".to_owned()],
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_world_list);
    assert_eq!(run.refused(), None);
    let plain = plain_requests(&run.world);
    assert_eq!(plain.len(), 1, "{plain:?}");
    assert_eq!(plain[0].1, AUTH_HTTP);
    assert_eq!(
        login_requests(&run.world),
        2,
        "the HTTPS form, then as given"
    );
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn a_real_https_answer_is_not_retried_over_http() {
    // The service's answer over working HTTPS, whatever it says, is used as that answer.
    let maintenance = login_fault("The service is down for maintenance");
    for (gls, service, reason) in [
        (
            "http://gls.example.invalid/datacenter",
            Service {
                answers: vec![(
                    "https://gls.example.invalid/datacenter".to_owned(),
                    503,
                    String::new(),
                )],
                ..Service::default()
            },
            LoginRefusal::ServiceUnavailable,
        ),
        (
            GLS,
            Service {
                launcher_url: "http://gls.example.invalid/launcher.xml".to_owned(),
                answers: vec![(
                    "https://gls.example.invalid/launcher.xml".to_owned(),
                    500,
                    maintenance.clone(),
                )],
                ..Service::default()
            },
            LoginRefusal::ServiceUnavailable,
        ),
        (
            GLS,
            Service {
                auth: AUTH_HTTP.to_owned(),
                answers: vec![(AUTH_HTTPS.to_owned(), 500, login_fault("bad password"))],
                ..Service::default()
            },
            LoginRefusal::BadCredentials,
        ),
        (
            GLS,
            Service {
                auth: AUTH_HTTP.to_owned(),
                answers: vec![(AUTH_HTTPS.to_owned(), 404, String::new())],
                ..Service::default()
            },
            LoginRefusal::ServiceUnavailable,
        ),
    ] {
        let mut run = start_with(
            Service {
                plain_http: true,
                ..service
            },
            Some(gls),
        );
        run.wait_event(is_disconnected);
        assert_eq!(run.refused().map(|r| r.0), Some(reason));
        assert!(plain_requests(&run.world).is_empty());
    }
}

#[test]
fn a_real_https_answer_of_a_status_or_queue_is_not_retried_over_http() {
    let login = Service::default().login_server;
    // The world list: a 503 over HTTPS is the answer (Unknown), no plain request.
    // Joining: an empty 200 refuses the world, the login stays.
    let mut run = start(
        Service {
            alpha_status_url: ALPHA_HTTP.to_owned(),
            answers: vec![(ALPHA_HTTPS.to_owned(), 503, String::new())],
            beta_status: Some(status(false, &login)),
            plain_http: true,
            ..Service::default()
        },
        true,
    );
    let list = run.wait_event(is_world_list);
    assert_eq!(
        alpha_in_list(&list),
        Some(tessera_net::account::WorldStatus::Unknown)
    );
    run.world
        .set_answers(vec![(ALPHA_HTTPS.to_owned(), 200, String::new())]);
    run.choose(0);
    run.wait_event(|e| {
        matches!(
            e,
            Event::Account(AccountEvent::WorldRefused {
                world: WorldId(0),
                reason: WorldRefusal::Closed,
                ..
            })
        )
    });
    assert_eq!(run.refused(), None);
    assert!(plain_requests(&run.world).is_empty());
    run.choose(1);
    wait_binds(&mut run, 1);
    let _ = run.session.send(Instant::now(), Command::Disconnect);

    // The queue: a 503 over HTTPS refuses the world; the ticket never went plain.
    let mut run = start(
        Service {
            queue_url: "http://gls.example.invalid/queue".to_owned(),
            alpha_queue: true,
            answers: vec![(
                "https://gls.example.invalid/queue".to_owned(),
                503,
                String::new(),
            )],
            beta_status: Some(status(false, &login)),
            plain_http: true,
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
                ..
            })
        )
    });
    assert!(plain_requests(&run.world).is_empty());
    run.choose(1);
    wait_binds(&mut run, 1);
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn an_address_without_an_https_form_is_asked_as_given() {
    let _one = plain_lock();
    let given = "http://user@auth.example.invalid/login";
    let mut run = start(
        Service {
            auth: given.to_owned(),
            plain_http: true,
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_world_list);
    let to_auth: Vec<String> = run
        .world
        .requests()
        .into_iter()
        .filter(|r| r.1.contains("auth.example.invalid"))
        .map(|r| r.1)
        .collect();
    assert_eq!(to_auth, [given.to_owned()]);
    let _ = run.session.send(Instant::now(), Command::Disconnect);
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

/// The stop predicate of a driver whose attempt was not cancelled.
fn go() -> bool {
    false
}

/// The stop predicate of a cancelled attempt.
fn cancelled() -> bool {
    true
}

fn new_world(service: Service) -> World {
    World {
        service: Arc::new(service),
        shared: Arc::new(Mutex::new(Shared::default())),
        net_dropped: Arc::new(AtomicBool::new(false)),
        chat_binds: Arc::new(AtomicUsize::new(0)),
        right: Arc::default(),
    }
}

/// A driver whose `SessionAuthHeader` went out, with a ticket of `lifetime` seconds.
fn driver_at_login(service: Service, lifetime: u32) -> (World, SessionDriver<FakeNet>) {
    driver_at_login_with(service, lifetime, None, Arc::default())
}

/// The start values of a session with a ticket of `lifetime` seconds, the chat server at
/// `chat` and the attempt's `cancel` flag.
fn session_start(
    lifetime: u32,
    chat: Option<SocketAddrV4>,
    cancel: Arc<AtomicBool>,
) -> SessionStart {
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
    SessionStart {
        handoff,
        login_peer: LOGIN_SERVER,
        plain: Default::default(),
        refresher: TicketRefresher::new(auth, sub, TICKET, lifetime, 0.0),
        chat_endpoint: chat,
        version,
        codecs: crate::chat_session::ChatCodecs::default(),
        codec_uuids: Vec::new(),
        schema: SchemaView(Arc::new(records())),
        cancel,
        send_checksum: Default::default(),
    }
}

/// [`driver_at_login`] with the chat server at `chat` and the attempt's `cancel` flag.
fn driver_at_login_with(
    service: Service,
    lifetime: u32,
    chat: Option<SocketAddrV4>,
    cancel: Arc<AtomicBool>,
) -> (World, SessionDriver<FakeNet>) {
    let world = new_world(service);
    let mut net = FakeNet(world.clone());
    let start = session_start(lifetime, chat, cancel);
    let d = SessionDriver::start(&mut net, start, 0.0, &go).expect("start");
    (world, d)
}

/// A driver at the character list, with a ticket of `lifetime` seconds.
fn driver_at_characters(
    service: Service,
    lifetime: u32,
) -> (World, SessionDriver<FakeNet>, Server) {
    driver_at_characters_with(service, lifetime, None, Arc::default())
}

/// [`driver_at_characters`] with the chat server at `chat` and the attempt's `cancel` flag.
fn driver_at_characters_with(
    service: Service,
    lifetime: u32,
    chat: Option<SocketAddrV4>,
    cancel: Arc<AtomicBool>,
) -> (World, SessionDriver<FakeNet>, Server) {
    driver_at_characters_from(service, session_start(lifetime, chat, cancel))
}

/// A driver started from `start`, at the character list.
fn driver_at_characters_from(
    service: Service,
    start: SessionStart,
) -> (World, SessionDriver<FakeNet>, Server) {
    let world = new_world(service);
    let mut net = FakeNet(world.clone());
    let mut d = SessionDriver::start(&mut net, start, 0.0, &go).expect("start");
    let mut login = Server::new(0x0111, 0xBEEF_0001);
    world.deliver(login.challenge(0xBEEF_0001), LOGIN_SERVER);
    world.deliver(
        login.control(OptionalHeaders {
            cumulative_ack: Some(0),
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    d.step(0.0, true, &go);
    world.deliver(login.event(0x6C, &character_list()), LOGIN_SERVER);
    d.step(0.1, true, &go);
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
        d.step(t, true, &go);
        assert!(
            !d.take_events()
                .iter()
                .any(|e| matches!(e, Event::EnterWorldFailed { .. })),
            "too early at {t}"
        );
    }
    d.step(61.5, true, &go);
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
    d.step(31.0, true, &go);
    gate.wait_reached();
    assert!(d.refresh_running());
    d.begin_shutdown(32.0);
    assert!(!d.refresh_running(), "the shutdown stops the refresh");
    gate.release();
    for t in [45.0, 50.0, 59.0, 90.0] {
        let _ = d.shutdown_step(t);
        d.step(t, true, &go);
    }
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(refreshes(), 1, "none started after the shutdown");
}

/// One datagram of `server` carrying `n` `SystemNotification` messages in a batch container
/// (`official-session.md` §9.3, §9.4.2), with the texts `from`, `from + 1`, …
fn notifications(server: &mut Server, from: usize, n: usize) -> Vec<u8> {
    let subs: Vec<(u32, Vec<u8>)> = (from..from + n)
        .map(|i| {
            let units: Vec<u16> = i.to_string().encode_utf16().collect();
            let mut body = Vec::new();
            encode_variable_u32(u32::try_from(units.len()).expect("short"), &mut body);
            for u in units {
                body.extend_from_slice(&u.to_le_bytes());
            }
            (OPCODE_SYSTEM_NOTIFICATION, encode_event_envelope(&body))
        })
        .collect();
    server.blob(
        OPCODE_BATCH_CONTAINER,
        &encode_batch_container(&subs, false).expect("batch"),
    )
}

fn notice_text(e: Event) -> Option<String> {
    match e {
        Event::ServerNotice { text, .. } => Some(text),
        _ => None,
    }
}

const FLOOD_PER_DATAGRAM: usize = 48;
const FLOOD_DATAGRAMS: usize = 60;

#[test]
fn a_flooding_server_cannot_grow_the_driver_events() {
    let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
    let total = FLOOD_PER_DATAGRAM * FLOOD_DATAGRAMS;
    for k in 0..FLOOD_DATAGRAMS {
        let datagram = notifications(&mut login, k * FLOOD_PER_DATAGRAM, FLOOD_PER_DATAGRAM);
        world.deliver(datagram, LOGIN_SERVER);
    }
    d.step(1.0, true, &go);
    assert!(d.event_count() > 0);
    assert!(
        d.event_count() <= DRIVER_EVENT_CAP,
        "{} events held",
        d.event_count()
    );
    assert!(world.inbound_len() > 0, "reading stopped at the cap");
    // Taking the events lets it go on: everything arrives, in order.
    let mut texts = Vec::new();
    let mut t = 1.0;
    while texts.len() < total {
        texts.extend(d.take_events().into_iter().filter_map(notice_text));
        t += 0.01;
        d.step(t, true, &go);
        assert!(d.event_count() <= DRIVER_EVENT_CAP);
        assert!(t < 20.0, "only {} of {total}", texts.len());
    }
    let expected: Vec<String> = (0..total).map(|i| i.to_string()).collect();
    assert_eq!(texts, expected);
}

#[test]
fn a_flooding_server_is_read_only_as_events_are_taken() {
    let mut run = start(Service::default(), true);
    let mut login = to_character_list(&mut run);
    let total = FLOOD_PER_DATAGRAM * FLOOD_DATAGRAMS;
    for k in 0..FLOOD_DATAGRAMS {
        let datagram = notifications(&mut login, k * FLOOD_PER_DATAGRAM, FLOOD_PER_DATAGRAM);
        run.world.deliver(datagram, LOGIN_SERVER);
    }
    // Without a poll the thread holds back: datagrams stay unread.
    std::thread::sleep(Duration::from_millis(500));
    assert!(run.world.inbound_len() > 0, "the socket was read on");
    let until = Instant::now() + Duration::from_secs(20);
    let mut texts: Vec<String> = std::mem::take(&mut run.backlog)
        .into_iter()
        .filter_map(notice_text)
        .collect();
    while texts.len() < total {
        texts.extend(
            run.session
                .poll(Instant::now())
                .into_iter()
                .filter_map(notice_text),
        );
        assert!(Instant::now() < until, "only {} of {total}", texts.len());
        std::thread::sleep(Duration::from_millis(1));
    }
    let expected: Vec<String> = (0..total).map(|i| i.to_string()).collect();
    assert_eq!(texts, expected);
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}

#[test]
fn a_refusal_and_its_disconnect_are_never_parted() {
    let refused = Event::Account(AccountEvent::LoginRefused {
        reason: LoginRefusal::Other,
        retry_after_seconds: None,
        detail: None,
    });
    let gone = Event::Disconnected {
        reason: DisconnectReason::TimedOut,
    };
    let other = Event::Account(AccountEvent::LoginProgress {
        stage: LoginStage::JoiningWorld,
    });
    let events = vec![other, refused, gone];
    assert_eq!(refusal_cut(&events, 0), 0);
    assert_eq!(refusal_cut(&events, 1), 1);
    assert_eq!(refusal_cut(&events, 2), 3);
    assert_eq!(refusal_cut(&events, 3), 3);
    assert_eq!(refusal_cut(&events, 9), 3);
    // The driver, taken one event at a time: a transport refusal during login.
    let (world, mut d) = driver_at_login(Service::default(), 0);
    let login = Server::new(0x0111, 1);
    world.deliver(login.challenge(1), LOGIN_SERVER);
    world.deliver(
        login.control(OptionalHeaders {
            transport_error: Some(crate::session::TransportErrorHeader {
                error_code: 0x00A7_E948,
                error_subcode: 0,
            }),
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    d.step(0.5, true, &go);
    loop {
        let taken = d.take_events_up_to(1);
        let first = taken.first().expect("a refusal was produced");
        if matches!(first, Event::Account(AccountEvent::LoginRefused { .. })) {
            assert_eq!(taken.len(), 2, "{taken:?}");
            assert!(matches!(taken.get(1), Some(Event::Disconnected { .. })));
            break;
        }
    }
}

#[test]
fn a_datagram_naming_a_connection_from_another_address_is_discarded() {
    let (world, mut d, login) = driver_at_characters(Service::default(), 0);
    let enter = Command::EnterWorld {
        character: "Aldo".to_owned(),
    };
    d.command(&enter, 1.0).expect("enter");
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.1, true, &go);
    assert!(d.connection_ids().1.is_some(), "the referral was followed");
    // The world server assigns an ID other than the login connection's.
    let mut ws = Server::new(0x0222, 0xBEEF_0002);
    world.deliver(ws.challenge(0xBEEF_0002), WORLD_SERVER);
    world.deliver(
        ws.control(OptionalHeaders {
            cumulative_ack: Some(0),
            ..OptionalHeaders::default()
        }),
        WORLD_SERVER,
    );
    d.step(1.2, true, &go);
    assert_eq!(d.connection_ids(), (0x0111, Some(0x0222)));
    // The same datagram once naming the login connection's ID (`DatagramHeader.recId`
    // behind a zero wire prefix), from the world server's address.
    let ready = ws.event(0x90, &session_ready());
    let mut stray = ready.clone();
    stray
        .get_mut(2..4)
        .expect("header")
        .copy_from_slice(&0x0111u16.to_be_bytes());
    let before = d.counters();
    world.deliver(stray, WORLD_SERVER);
    d.step(1.3, true, &go);
    let after = d.counters();
    assert_eq!(after.address_mismatches, before.address_mismatches + 1);
    assert_eq!(after.refused_datagrams, before.refused_datagrams);
    assert!(!d.in_world(), "the stray datagram reached no connection");
    world.deliver(ready, WORLD_SERVER);
    d.step(1.4, true, &go);
    assert!(d.in_world());
}

#[test]
fn after_a_cancel_no_referral_is_followed_and_no_refresh_starts() {
    let (world, mut d, login) = driver_at_characters(Service::default(), 60);
    let enter = Command::EnterWorld {
        character: "Aldo".to_owned(),
    };
    d.command(&enter, 1.0).expect("enter");
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.1, true, &cancelled);
    assert_eq!(
        d.connection_ids().1,
        None,
        "no world login after the cancel"
    );
    assert_eq!(d.counters().cancelled_referrals, 1);
    while let Some((_, dst)) = world.pop_sent() {
        assert_ne!(dst, WORLD_SERVER);
    }
    // The refresh is due at half the lifetime; the stop holds it back.
    d.step(31.0, true, &cancelled);
    assert!(!d.refresh_running());
    d.step(31.5, true, &go);
    assert!(d.refresh_running(), "only the stop held it back");
}

#[test]
fn a_cancel_during_the_chat_lookup_binds_and_sends_nothing() {
    let gate = Arc::new(Gate::default());
    let mut run = start(
        Service {
            chat: "chat.example.invalid:9200".to_owned(),
            chat_gate: Some(Arc::clone(&gate)),
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_world_list);
    run.choose(0);
    gate.wait_reached();
    run.session
        .send(Instant::now(), Command::Disconnect)
        .expect("disconnect");
    gate.release();
    run.wait_exit(true);
    assert_eq!(run.world.binds(), 0, "no socket after the cancel");
    assert!(run.world.pop_sent().is_none(), "no SessionAuthHeader");
}

fn revoke_lock() -> std::sync::RwLockWriteGuard<'static, ()> {
    crate::net_access::TEST_REVOKE_LOCK
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn withdraw_ends_a_running_session_thread_at_once() {
    // Held from the start: no other test's `revoke_all` may end this run's right early.
    let guard = revoke_lock();
    let mut run = start(
        Service {
            follow_withdraw: true,
            ..Service::default()
        },
        true,
    );
    let _login = to_character_list(&mut run);
    // The session settles first (nothing is being sent when the right ends).
    let mut quiet = Instant::now();
    while quiet.elapsed() < Duration::from_millis(300) {
        if run.world.pop_sent().is_some() {
            quiet = Instant::now();
        }
        run.drain();
        std::thread::sleep(Duration::from_millis(5));
    }
    // Withdraw (`official-entry.md` §5.5): every right of the process ends, then the
    // session is dropped. The login server never answers the disconnect: without the
    // revocation the thread would try for `SHUTDOWN_LIMIT`.
    crate::net_access::revoke_all();
    let requests = run.world.requests().len();
    while run.world.pop_sent().is_some() {}
    let Run {
        _dir,
        world,
        session,
        ..
    } = run;
    let t0 = Instant::now();
    drop(session);
    while !world.net_dropped.load(Ordering::SeqCst) {
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "the thread did not end"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        t0.elapsed() < SHUTDOWN_LIMIT / 2,
        "the thread lingered {:?}",
        t0.elapsed()
    );
    assert!(
        world.pop_sent().is_none(),
        "a datagram after the revocation"
    );
    assert_eq!(
        world.requests().len(),
        requests,
        "a request after the revocation"
    );
    // Not even tried (the synthetic network refuses and counts what comes after).
    assert_eq!(world.dead_attempts(), 0, "a send or request was tried");
    drop(guard);
}

#[test]
fn withdraw_during_the_login_ends_the_thread_with_no_further_request() {
    let guard = revoke_lock();
    let gate = Arc::new(Gate::default());
    let mut run = start(
        Service {
            gate: Some(Arc::clone(&gate)),
            follow_withdraw: true,
            ..Service::default()
        },
        true,
    );
    gate.wait_reached();
    // The revocation alone, while the first exchange is on the wire.
    crate::net_access::revoke_all();
    gate.release();
    run.wait_event(is_disconnected);
    run.wait_exit(true);
    assert_eq!(run.world.requests().len(), 1, "{:?}", run.world.requests());
    assert_eq!(login_requests(&run.world), 0);
    assert_eq!(run.world.binds(), 0);
    assert_eq!(run.world.dead_attempts(), 0, "a request was tried");
    assert!(
        run.seen.iter().any(|e| matches!(
            e,
            Event::Disconnected { reason: DisconnectReason::TransportFailed(r) }
                if r.contains("revoked")
        )),
        "the end names the revocation, not a refusal: {:?}",
        run.seen
    );
    assert_eq!(run.refused(), None);
    drop(guard);
}

#[test]
fn withdraw_while_queued_ends_the_wait_without_a_world_refusal() {
    let guard = revoke_lock();
    let mut run = start(
        Service {
            alpha_queue: true,
            queue_waiting: true,
            follow_withdraw: true,
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_world_list);
    run.choose(0);
    run.wait_event(|e| matches!(e, Event::Account(AccountEvent::LoginQueueChanged { .. })));
    let requests = run.world.requests().len();
    let t0 = Instant::now();
    crate::net_access::revoke_all();
    let end = run.wait_event(is_disconnected);
    // The wait between two polls is 5 s or more: the revocation ended it at once.
    assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
    assert!(
        matches!(&end, Event::Disconnected { reason: DisconnectReason::TransportFailed(r) } if r.contains("revoked")),
        "{end:?}"
    );
    assert!(
        !run.seen
            .iter()
            .any(|e| matches!(e, Event::Account(AccountEvent::WorldRefused { .. }))),
        "{:?}",
        run.seen
    );
    run.wait_exit(true);
    assert_eq!(run.world.requests().len(), requests);
    assert_eq!(run.world.dead_attempts(), 0);
    drop(guard);
}

// ---------------------------------------------------------------------------
// Tests: nothing after the cancel
// ---------------------------------------------------------------------------

/// Whether `datagram` carries the given optional header.
fn has_session_auth(datagram: &[u8]) -> bool {
    decode_datagram(datagram, false, None, true).is_ok_and(|p| p.opt_headers.session_auth.is_some())
}

#[test]
fn a_cancel_two_seconds_into_the_login_handshake_sends_nothing() {
    let (world, mut d) = driver_at_login(Service::default(), 0);
    let first = world.take_sent();
    assert_eq!(first.len(), 1);
    assert!(has_session_auth(&first[0].0));
    d.step(1.0, true, &go);
    assert!(world.take_sent().is_empty(), "no retransmission before 2 s");
    // The handshake's retransmission is due at 2 s: the cancel comes after it.
    d.begin_shutdown(2.5);
    assert!(
        world.take_sent().is_empty(),
        "no SessionAuthHeader after the cancel"
    );
    assert_eq!(d.connection_states().0, None, "closed locally");
    for t in [3.0, 4.6, 8.0, 21.0] {
        assert!(!d.shutdown_step(t), "nothing to flush at {t}");
        assert!(world.take_sent().is_empty(), "a datagram at {t}");
    }
}

#[test]
fn a_cancel_during_a_step_with_the_resend_due_sends_no_session_auth() {
    let (world, mut d) = driver_at_login(Service::default(), 0);
    assert_eq!(world.take_sent().len(), 1);
    d.step(1.0, true, &go);
    assert!(world.take_sent().is_empty(), "no retransmission before 2 s");
    // The retransmission is due at 2 s; the cancel holds while this pass runs.
    d.step(2.5, true, &cancelled);
    assert!(
        world.take_sent().is_empty(),
        "a datagram (the SessionAuthHeader) after the cancel"
    );
    d.begin_shutdown(2.6);
    assert_eq!(d.connection_states().0, None, "closed locally");
    for t in [3.0, 4.6, 8.0, 21.0] {
        let _ = d.shutdown_step(t);
        assert!(world.take_sent().is_empty(), "a datagram at {t}");
    }
}

#[test]
fn a_challenge_read_in_a_cancelled_step_is_not_answered() {
    let (world, mut d) = driver_at_login(Service::default(), 0);
    assert_eq!(world.take_sent().len(), 1);
    let login = Server::new(0x0111, 0xBEEF_0001);
    world.deliver(login.challenge(0xBEEF_0001), LOGIN_SERVER);
    d.step(0.5, true, &cancelled);
    assert!(
        world.take_sent().is_empty(),
        "a datagram (the ConnectAccept) after the cancel"
    );
    d.begin_shutdown(0.6);
    assert_eq!(d.connection_states().0, None, "closed locally");
    for t in [1.0, 2.5, 4.6, 21.0] {
        let _ = d.shutdown_step(t);
        assert!(world.take_sent().is_empty(), "a datagram at {t}");
    }
}

#[test]
fn a_world_challenge_after_the_cancel_is_not_answered() {
    let (world, mut d, login) = driver_at_characters(Service::default(), 0);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.1, true, &go);
    assert!(
        world
            .take_sent()
            .iter()
            .any(|(_, dst)| *dst == WORLD_SERVER),
        "the referral was followed"
    );
    d.begin_shutdown(1.2);
    assert_eq!(
        d.connection_states().1,
        None,
        "the world handshake was closed"
    );
    // The world server's challenge arrives after the cancel (the ID the login connection
    // has, as in the end-to-end test).
    let ws = Server::new(0x0111, 0xBEEF_0002);
    ws.connect_to(&world, 0xBEEF_0002, WORLD_SERVER);
    for t in [1.3, 2.0, 3.5, 6.0, 25.0] {
        let _ = d.shutdown_step(t);
        for (datagram, dst) in world.take_sent() {
            assert_ne!(dst, WORLD_SERVER, "sent to the world server at {t}");
            let pkt = decode_datagram(&datagram, false, None, false);
            assert!(
                pkt.is_ok_and(|p| p.opt_headers.connect_accept.is_none()),
                "a ConnectAccept at {t}"
            );
        }
    }
}

/// A driver in the world on the login connection (no referral), with the chat server's
/// session attached over its own synthetic socket and its `SessionAuthHeader` sent; the
/// time is 2.3 s.
fn driver_with_chat_handshake() -> (World, SessionDriver<FakeNet>, Server) {
    let (world, mut d, mut login) = driver_at_characters_with(
        Service {
            chat_socket: true,
            ..Service::default()
        },
        0,
        Some(CHAT_SERVER),
        Arc::default(),
    );
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    world.deliver(login.event(0x90, &session_ready()), LOGIN_SERVER);
    d.step(1.1, true, &go);
    assert!(d.in_world());
    let (player, name) = d.wants_chat(1.2).expect("the chat session is due");
    let net = FakeNet(world.clone());
    d.attach_chat(net.chat_binder().expect("binder"), player, &name, 1.2, &go);
    // The first attempt follows the initial delay (§6.6.3, 1 s).
    d.step(2.3, true, &go);
    assert!(
        world
            .take_sent()
            .iter()
            .any(|(dgram, dst)| *dst == CHAT_SERVER && has_session_auth(dgram)),
        "the chat handshake began"
    );
    (world, d, login)
}

#[test]
fn a_cancel_during_the_chat_handshake_sends_no_chat_datagram() {
    let (world, mut d, _login) = driver_with_chat_handshake();
    d.begin_shutdown(2.5);
    // The world connection (here the login connection) has the unacknowledged `0x6F` to
    // flush before its `Disconnect`.
    assert!(
        matches!(
            d.connection_states().0,
            Some(ConnectionState::DisconnectingFlush | ConnectionState::DisconnectSent)
        ),
        "{:?}",
        d.connection_states()
    );
    let mut said_disconnect = false;
    let mut t = 2.5;
    while t < 30.0 {
        let _ = d.shutdown_step(t);
        for (datagram, dst) in world.take_sent() {
            assert_ne!(dst, CHAT_SERVER, "a chat datagram at {t}");
            if dst == LOGIN_SERVER {
                said_disconnect |= decode_datagram(&datagram, false, None, false)
                    .is_ok_and(|p| p.opt_headers.disconnect);
            }
        }
        t += 0.5;
    }
    assert!(said_disconnect, "the world connection said Disconnect");
}

#[test]
fn a_cancel_during_a_step_with_the_chat_resend_due_sends_nothing() {
    let (world, mut d, _login) = driver_with_chat_handshake();
    // The chat handshake's retransmission is due 2 s after 2.3 s; the cancel holds while
    // this pass runs.
    d.step(4.5, true, &cancelled);
    assert!(world.take_sent().is_empty(), "a datagram after the cancel");
    d.begin_shutdown(4.6);
    let mut t = 4.6;
    while t < 30.0 {
        let _ = d.shutdown_step(t);
        for (_, dst) in world.take_sent() {
            assert_ne!(dst, CHAT_SERVER, "a chat datagram at {t}");
        }
        t += 0.5;
    }
}

#[test]
fn a_chat_attachment_after_the_cancel_binds_nothing() {
    let (world, mut d, mut login) = driver_at_characters_with(
        Service {
            chat_socket: true,
            ..Service::default()
        },
        0,
        Some(CHAT_SERVER),
        Arc::default(),
    );
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    world.deliver(login.event(0x90, &session_ready()), LOGIN_SERVER);
    d.step(1.1, true, &go);
    let (player, name) = d.wants_chat(1.2).expect("the chat session is due");
    let net = FakeNet(world.clone());
    d.attach_chat(
        net.chat_binder().expect("binder"),
        player,
        &name,
        1.2,
        &cancelled,
    );
    assert_eq!(
        world.chat_binds.load(Ordering::SeqCst),
        0,
        "a chat socket was bound"
    );
    d.step(2.3, true, &cancelled);
    assert!(
        world.take_sent().iter().all(|(_, dst)| *dst != CHAT_SERVER),
        "a chat datagram after the cancel"
    );
}

#[test]
fn a_chat_flood_is_read_only_as_events_are_taken() {
    let (world, mut d, mut login) = driver_with_chat_handshake();
    let mut chat = Server::new(0x0111, 0xBEEF_0003);
    world.deliver_chat(chat.challenge(0xBEEF_0003));
    world.deliver_chat(chat.control(OptionalHeaders {
        cumulative_ack: Some(0),
        ..OptionalHeaders::default()
    }));
    d.step(2.4, true, &go);
    let _ = d.take_events();
    let total = 3 * DRIVER_EVENT_CAP;
    for i in 0..total {
        let add = crate::chat_session::events::ChatEvent::AddRoom {
            notice_flags: 0,
            room: crate::chat_session::wire::ChatRoomDescriptor::new(
                crate::chat_session::wire::DescriptorKind::UserRoom,
                0,
                format!("room{i}"),
            ),
        };
        let inner = add.encode().expect("event");
        world.deliver_chat(chat.event(add.opcode(), &inner));
    }
    // The session's own events come first in a pass: the chat client's reading must stop
    // at the room left under the cap.
    world.deliver(
        notifications(&mut login, 0, FLOOD_PER_DATAGRAM),
        LOGIN_SERVER,
    );
    let mut t = 2.5;
    for _ in 0..20 {
        d.step(t, true, &go);
        t += 0.01;
    }
    assert!(
        d.event_count() <= DRIVER_EVENT_CAP + 1,
        "{} events held",
        d.event_count()
    );
    assert!(d.event_count() > 0);
    assert!(
        world.chat_inbound_len() > 0,
        "chat reading stopped at the cap"
    );
    let mut joined = Vec::new();
    while joined.len() < total {
        joined.extend(d.take_events().into_iter().filter_map(|e| match e {
            Event::Social(tessera_net::SocialEvent::ChannelJoined { channel }) => Some(channel),
            _ => None,
        }));
        d.step(t, true, &go);
        assert!(d.event_count() <= DRIVER_EVENT_CAP + 1);
        t += 0.01;
        assert!(t < 60.0, "only {} of {total}", joined.len());
    }
    let expected: Vec<String> = (0..total).map(|i| format!("room{i}")).collect();
    assert_eq!(joined, expected);
}

#[test]
fn the_refresh_thread_looks_at_the_cancel_flag_before_its_request() {
    let cancel = Arc::new(AtomicBool::new(false));
    let (world, mut d, _login) =
        driver_at_characters_with(Service::default(), 60, None, Arc::clone(&cancel));
    // The connector cancels from its own thread; the driver's own stop predicate has not
    // seen it yet.
    cancel.store(true, Ordering::SeqCst);
    d.step(31.0, true, &go);
    assert!(d.refresh_running(), "a refresh thread was started");
    let until = Instant::now() + Duration::from_secs(10);
    let mut t = 31.0;
    while d.refresh_running() {
        assert!(Instant::now() < until, "the refresh thread did not end");
        t += 0.01;
        d.step(t, true, &go);
        std::thread::sleep(Duration::from_millis(2));
    }
    let refreshes = world
        .requests()
        .iter()
        .filter(|r| r.2.as_deref().is_some_and(|a| a.contains("RefreshTicket")))
        .count();
    assert_eq!(refreshes, 0, "no request after the cancel");
}

#[test]
fn a_cancelled_start_probes_binds_and_sends_nothing() {
    let world = new_world(Service::default());
    let mut net = FakeNet(world.clone());
    let start = session_start(0, None, Arc::default());
    let r = SessionDriver::start(&mut net, start, 0.0, &cancelled);
    assert!(matches!(
        r,
        Err(super::session_phase::StartFailure::Cancelled)
    ));
    assert_eq!(world.probes(), 0, "no local-address probe");
    assert_eq!(world.binds(), 0);
    assert!(world.take_sent().is_empty());
}

#[test]
fn a_cancel_while_the_world_status_is_asked_resolves_nothing() {
    // The world list's status of Alpha goes through; the one after `ChooseWorld` is held.
    let gate = Arc::new(Gate::passing(1));
    let mut run = start(
        Service {
            status_gate: Some(Arc::clone(&gate)),
            chat: "chat.example.invalid:9200".to_owned(),
            ..Service::default()
        },
        true,
    );
    run.wait_event(is_world_list);
    run.choose(0);
    gate.wait_reached();
    run.session
        .send(Instant::now(), Command::Disconnect)
        .expect("disconnect");
    gate.release();
    run.wait_exit(true);
    assert_eq!(run.world.resolves(), 0, "no name lookup after the cancel");
    assert_eq!(run.world.binds(), 0);
    assert!(run.world.pop_sent().is_none());
}

#[test]
fn the_shutdown_drops_play_messages_not_yet_queued() {
    let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    world.deliver(login.event(0x90, &session_ready()), LOGIN_SERVER);
    d.step(1.1, true, &go);
    assert!(d.in_world());
    let opcodes = |sent: Vec<(Vec<u8>, SocketAddrV4)>| -> Vec<u32> {
        sent.iter()
            .filter_map(|(d, _)| decode_datagram(d, false, None, false).ok())
            .flat_map(|p| p.fragments)
            .filter_map(|f| f.header.blob_header_word)
            .collect()
    };
    let before = opcodes(world.take_sent());
    d.command(
        &Command::Chat {
            channel: ChatChannel::Say,
            text: "hello".to_owned(),
        },
        1.2,
    )
    .expect("say");
    d.begin_shutdown(1.2);
    let mut after = opcodes(world.take_sent());
    for t in [1.5, 3.0, 25.0] {
        let _ = d.shutdown_step(t);
        after.extend(opcodes(world.take_sent()));
    }
    // Only what was queued on the connection before (its retransmissions) goes out.
    assert!(
        after.iter().all(|op| *op == 0x6F || before.contains(op)),
        "before {before:x?}, after {after:x?}"
    );
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
fn the_https_form_keeps_host_and_path_and_drops_a_plain_port() {
    use super::net::{https_form, scheme_label};
    let form = |u: &str| https_form(u).map(|h| (h.url, h.upgraded));
    let up = |u: &str| Some((u.to_owned(), true));
    assert_eq!(
        form(" https://a.example.invalid:8443/x "),
        Some(("https://a.example.invalid:8443/x".to_owned(), false))
    );
    assert_eq!(
        form("http://a.example.invalid/x/y?q=1#f"),
        up("https://a.example.invalid/x/y?q=1#f")
    );
    assert_eq!(
        form("HTTP://a.example.invalid:8080/x"),
        up("https://a.example.invalid/x")
    );
    assert_eq!(
        form("http://a.example.invalid"),
        up("https://a.example.invalid")
    );
    assert_eq!(
        form("http://a.example.invalid:?q"),
        up("https://a.example.invalid?q")
    );
    assert_eq!(
        form("http://[2001:db8::1]:80/x"),
        up("https://[2001:db8::1]/x")
    );
    assert_eq!(form("http://192.0.2.7:80/x"), up("https://192.0.2.7/x"));
    for bad in [
        "",
        "ftp://a.example.invalid/x",
        "a.example.invalid/x",
        "http://",
        "http:///x",
        "http://user:pw@a.example.invalid/x",
        "http://a.example.invalid:80x/x",
        "http://a:b:c/x",
        "http://[]/x",
        "http://[2001:db8::1]x/",
        "http://[2001:db8::1/x",
        "http://a.example.invalid/x y",
        "http://a.example.invalid/\u{7}",
        "http//a.example.invalid/x",
    ] {
        assert_eq!(https_form(bad), None, "{bad:?}");
    }
    assert_eq!(scheme_label("HTTPS://a/"), "https");
    assert_eq!(scheme_label(" http://a/"), "http");
    assert_eq!(scheme_label("ftp://a/"), "other");
    assert_eq!(scheme_label("a.example.invalid/x"), "none");
}

#[test]
fn an_exchange_tries_https_first_and_the_address_as_given_only_after_a_transport_failure() {
    use super::login_phase::{ChainStep, PlainSteps, exchange};
    let _one = plain_lock();
    /// Answers `answer` to HTTPS requests (an `Err` stands for a transport or TLS failure)
    /// and 200 to plain ones; records the URLs.
    struct Fake {
        https: Result<u16, ()>,
        urls: Vec<String>,
    }
    impl HttpClient for Fake {
        fn execute(&mut self, req: &HttpRequest) -> Result<HttpResponse, LoginError> {
            self.urls.push(req.url.clone());
            let code = if is_https(&req.url) {
                self.https.map_err(|()| LoginError::ServiceUnavailable {
                    reason: "TLS".to_owned(),
                })?
            } else {
                200
            };
            Ok(HttpResponse {
                status_code: code,
                body: Vec::new(),
            })
        }
    }
    let run = |given: &str, https: Result<u16, ()>| {
        let mut fake = Fake {
            https,
            urls: Vec::new(),
        };
        let plain = PlainSteps::default();
        let mut req = crate::login::build_world_status_request("unused");
        let code = exchange(&mut fake, ChainStep::TicketRefresh, given, &mut req, &plain)
            .map(|r| r.status_code)
            .ok();
        (code, fake.urls, plain.went_plain(ChainStep::TicketRefresh))
    };
    let s = |v: &[&str]| v.iter().map(|u| (*u).to_owned()).collect::<Vec<_>>();
    // An https:// address: as given, never anything else.
    assert_eq!(
        run("https://a.example.invalid/x", Err(())),
        (None, s(&["https://a.example.invalid/x"]), false)
    );
    // An http:// address whose HTTPS form answers, with any status: that answer.
    for code in [200, 404, 503] {
        assert_eq!(
            run("http://a.example.invalid:80/x", Ok(code)),
            (Some(code), s(&["https://a.example.invalid/x"]), false)
        );
    }
    // Its HTTPS form fails at the transport: once more, as given, over plain HTTP.
    assert_eq!(
        run("http://a.example.invalid:80/x", Err(())),
        (
            Some(200),
            s(&[
                "https://a.example.invalid/x",
                "http://a.example.invalid:80/x"
            ]),
            true
        )
    );
    // No HTTPS form: as given.
    assert_eq!(
        run("http://u@a.example.invalid/x", Ok(200)),
        (Some(200), s(&["http://u@a.example.invalid/x"]), true)
    );
}

#[test]
fn is_https_and_the_https_form_agree_on_every_form() {
    use super::net::https_form;
    // One parse decides both (nit F5): a form one refuses, the other refuses too.
    let hostile = [
        "https://a.example.invalid/x y",
        "https://a.example.invalid/\u{7}",
        "https://a.example.invalid/\u{85}",
        "https://a.example.invalid\u{a0}/x",
        "https://a.example.invalid/x\r\nHost: b",
        "\u{1}https://a.example.invalid/x",
        "https://user:pw@a.example.invalid/x",
        "https://",
        "https:///x",
        "https://a.example.invalid:44x/x",
        "https://[]/x",
        "https//a.example.invalid/x",
        "http://a.example.invalid/x\ty",
        "http://user@a.example.invalid/x",
    ];
    for u in hostile {
        assert!(!is_https(u), "{u:?}");
        assert_eq!(https_form(u), None, "{u:?}");
    }
    let good = [
        " https://a.example.invalid:8443/x ",
        "HTTPS://a.example.invalid",
        "https://[2001:db8::1]:443/x?q#f",
        "http://a.example.invalid/x",
        "HTTP://a.example.invalid:8080",
    ];
    for u in hostile.iter().chain(&good) {
        let form = https_form(u);
        assert_eq!(
            is_https(u),
            form.as_ref().is_some_and(|h| !h.upgraded),
            "{u:?}"
        );
        if let Some(h) = form {
            assert!(is_https(&h.url), "{u:?} -> {:?}", h.url);
            assert_eq!(https_form(&h.url).map(|f| f.url), Some(h.url.clone()));
        }
    }
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

// ---------------------------------------------------------------------------
// Tests: world loading and the way back to character selection
// ---------------------------------------------------------------------------

/// The opcodes of the blobs the client sent since the last call, with their targets.
fn sent_blobs(world: &World) -> Vec<(u32, SocketAddrV4)> {
    world
        .take_sent()
        .into_iter()
        .filter_map(|(d, dst)| {
            let pkt = decode_datagram(&d, false, None, false).ok()?;
            Some(
                pkt.fragments
                    .into_iter()
                    .filter_map(|f| f.header.blob_header_word)
                    .map(move |w| (w, dst))
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

/// A driver in the world as "Aldo" on the login connection (no referral) at `t = 1.1`.
fn driver_in_world() -> (World, SessionDriver<FakeNet>, Server) {
    let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
    let enter = Command::EnterWorld {
        character: "Aldo".to_owned(),
    };
    d.command(&enter, 1.0).expect("enter");
    world.deliver(login.event(0x90, &session_ready()), LOGIN_SERVER);
    d.step(1.1, true, &go);
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. }))
    );
    (world, d, login)
}

#[test]
fn the_portal_exit_waits_for_the_clients_loading() {
    let (world, mut d, _login) = driver_in_world();
    for t in [1.2, 5.0, 20.0] {
        d.step(t, true, &go);
    }
    assert!(
        !sent_blobs(&world).iter().any(|(op, _)| *op == 0xF4),
        "no portal exit before the client's loading is done"
    );
    d.command(&Command::WorldLoaded, 20.5).expect("loaded");
    d.step(20.6, true, &go);
    let sent = sent_blobs(&world);
    assert_eq!(
        sent.iter().filter(|(op, _)| *op == 0xF4).count(),
        1,
        "{sent:?}"
    );
    // Once: a later WorldLoaded outside portal space sends nothing.
    d.command(&Command::WorldLoaded, 21.0).expect("loaded");
    d.step(21.1, true, &go);
    assert!(!sent_blobs(&world).iter().any(|(op, _)| *op == 0xF4));
}

#[test]
fn the_portal_exit_follows_its_time_out_without_the_clients_signal() {
    let (world, mut d, _login) = driver_in_world();
    d.step(1.2, true, &go);
    d.step(1.2 + PORTAL_LOADING_TIMEOUT_SEC - 1.0, true, &go);
    assert!(!sent_blobs(&world).iter().any(|(op, _)| *op == 0xF4));
    d.step(1.3 + PORTAL_LOADING_TIMEOUT_SEC, true, &go);
    assert!(sent_blobs(&world).iter().any(|(op, _)| *op == 0xF4));
}

#[test]
fn leaving_the_world_returns_to_the_character_list_on_the_same_session() {
    let (world, mut d, mut login) = driver_in_world();
    let _ = sent_blobs(&world);
    // `LeaveWorld` is the logout request (`0x30`, `official-messages-play.md` §7.2 item 1).
    d.command(&Command::LeaveWorld, 2.0).expect("leave");
    d.step(2.1, true, &go);
    let sent = sent_blobs(&world);
    assert!(sent.contains(&(0x30, LOGIN_SERVER)), "{sent:?}");
    // The teardown (§7.2 item 3) keeps the session; the client unloads the world on the
    // notice, so the return comes at once with the latest list.
    world.deliver(login.event(0x3000_001B, &[0]), LOGIN_SERVER);
    d.step(3.0, true, &go);
    assert!(!d.ended());
    assert!(!d.in_world());
    let events = d.take_events();
    let back = events.iter().find_map(|e| match e {
        Event::ReturnedToCharacterSelect { characters } => Some(characters.clone()),
        _ => None,
    });
    assert_eq!(
        back.map(|c| c.into_iter().map(|c| c.name).collect::<Vec<_>>()),
        Some(vec!["Aldo".to_owned()]),
        "{events:?}"
    );
    // The list that follows changes nothing: no event.
    world.deliver(login.event(0x6C, &character_list()), LOGIN_SERVER);
    d.step(3.5, true, &go);
    assert!(d.take_events().is_empty());
    assert!(matches!(
        d.command(&Command::LeaveWorld, 3.6),
        Err(tessera_net::NetError::InvalidCommand(_))
    ));
    // The character enters again: `0x6F` once more (no referral: the login connection).
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        4.0,
    )
    .expect("enter again");
    d.step(4.1, true, &go);
    assert!(sent_blobs(&world).contains(&(0x6F, LOGIN_SERVER)));
    world.deliver(login.event(0x90, &session_ready()), LOGIN_SERVER);
    d.step(4.2, true, &go);
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. }))
    );
}

#[test]
fn a_list_received_in_the_world_is_the_one_the_return_offers() {
    let (world, mut d, mut login) = driver_in_world();
    let two = [
        (0x0200_0000_0000_0042, "Aldo"),
        (0x0200_0000_0000_0043, "Bera"),
    ];
    world.deliver(login.event(0x6C, &character_list_of(&two)), LOGIN_SERVER);
    d.step(1.5, true, &go);
    assert!(
        !d.take_events()
            .iter()
            .any(|e| matches!(e, Event::Character(_))),
        "in the world the list is only kept"
    );
    world.deliver(login.event(0x3000_0042, &[0]), LOGIN_SERVER);
    d.step(2.0, true, &go);
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::ReturnedToCharacterSelect { characters }
                if characters.iter().map(|c| c.name.as_str()).eq(["Aldo", "Bera"])
        )),
        "{events:?}"
    );
    assert!(!d.ended());
}

#[test]
fn a_later_list_at_character_selection_surfaces_its_changes() {
    let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
    let next = [(0x0200_0000_0000_0043, "Bera")];
    world.deliver(login.event(0x6C, &character_list_of(&next)), LOGIN_SERVER);
    d.step(0.5, true, &go);
    let events = d.take_events();
    assert!(
        matches!(
            events.as_slice(),
            [
                Event::Character(CharacterEvent::CharacterRemoved { name: gone }),
                Event::Character(CharacterEvent::CharacterInfo { name: new, .. }),
            ] if gone == "Aldo" && new == "Bera"
        ),
        "names gone first, then new names (`official-messages-play.md` §8): {events:?}"
    );
    // The new list is the one `EnterWorld` reads.
    d.command(
        &Command::EnterWorld {
            character: "Bera".to_owned(),
        },
        0.6,
    )
    .expect("enter");
    d.step(0.7, true, &go);
    assert!(sent_blobs(&world).contains(&(0x6F, LOGIN_SERVER)));
}

#[test]
fn moves_and_chat_after_the_teardown_are_refused_and_counted_and_chat_is_noticed() {
    let (world, mut d, mut login) = driver_in_world();
    world.deliver(login.event(0x3000_001B, &[0]), LOGIN_SERVER);
    d.step(2.0, true, &go);
    let _ = d.take_events();
    let _ = sent_blobs(&world);
    let mv = Command::Move {
        location: tessera_net::Location::default(),
        heading: 0.0,
        velocity: Vec3::ZERO,
        on_ground: true,
    };
    let chat = Command::Chat {
        channel: tessera_net::ChatChannel::Say,
        text: "hello".to_owned(),
    };
    assert!(matches!(
        d.command(&mv, 2.1),
        Err(tessera_net::NetError::InvalidCommand(_))
    ));
    assert!(matches!(
        d.command(&chat, 2.1),
        Err(tessera_net::NetError::InvalidCommand(_))
    ));
    d.step(2.2, true, &go);
    assert_eq!(d.counters().outside_world, 2);
    // The refused move is silent; the refused chat line gives one system notice.
    let events = d.take_events();
    assert!(
        matches!(
            events.as_slice(),
            [Event::ServerNotice { level: tessera_net::NoticeLevel::Info, seconds: 0, text }]
                if text == super::session_phase::NOT_SENT_OUTSIDE_WORLD
        ),
        "{events:?}"
    );
    let sent = sent_blobs(&world);
    assert!(
        !sent.iter().any(|(op, _)| *op == 0x24 || *op == 0x30),
        "nothing sent: {sent:?}"
    );
}

#[test]
fn a_teardown_held_during_the_entry_is_replayed_after_the_session_start() {
    let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    // The teardown arrives before `0x90` (kept for the session start).
    world.deliver(login.event(0x3000_001D, &[0]), LOGIN_SERVER);
    d.step(1.05, true, &go);
    assert!(d.take_events().is_empty());
    world.deliver(login.event(0x90, &session_ready()), LOGIN_SERVER);
    d.step(1.1, true, &go);
    let events = d.take_events();
    let entered = events
        .iter()
        .position(|e| matches!(e, Event::WorldEntered { .. }));
    let back = events
        .iter()
        .position(|e| matches!(e, Event::ReturnedToCharacterSelect { .. }));
    assert!(
        matches!((entered, back), (Some(a), Some(b)) if a < b),
        "{events:?}"
    );
    assert!(!d.in_world());
    assert!(!d.ended());
}

#[test]
fn a_stray_loading_signal_does_not_end_the_next_portal_space() {
    let (world, mut d, mut login) = driver_in_world();
    // Leaving while still in portal space (no `WorldLoaded` yet).
    d.command(&Command::LeaveWorld, 1.2)
        .expect("leave in portal space");
    d.step(1.3, true, &go);
    assert!(sent_blobs(&world).contains(&(0x30, LOGIN_SERVER)));
    world.deliver(login.event(0x3000_001B, &[0]), LOGIN_SERVER);
    d.step(1.4, true, &go);
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::ReturnedToCharacterSelect { .. }))
    );
    // A loading signal of the left world, outside portal space: ignored and counted.
    d.command(&Command::WorldLoaded, 1.5).expect("accepted");
    assert_eq!(d.counters().stray_loaded, 1);
    // Re-entry: a fresh portal space waits for its own signal.
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        2.0,
    )
    .expect("enter again");
    world.deliver(login.event(0x90, &session_ready()), LOGIN_SERVER);
    d.step(2.1, true, &go);
    d.step(5.0, true, &go);
    assert!(
        !sent_blobs(&world).iter().any(|(op, _)| *op == 0xF4),
        "the earlier signal does not count for this portal space"
    );
    // The portal space's time-out is a deadline of the driver.
    let next = d.next_deadline(5.0).expect("a deadline");
    assert!(next <= 2.1 + PORTAL_LOADING_TIMEOUT_SEC + 1e-9, "{next}");
    d.command(&Command::WorldLoaded, 5.5).expect("loaded");
    d.step(5.6, true, &go);
    assert_eq!(
        sent_blobs(&world)
            .iter()
            .filter(|(op, _)| *op == 0xF4)
            .count(),
        1
    );
}

#[test]
fn the_live_connector_returns_to_character_selection() {
    let mut run = start(Service::default(), true);
    let login = to_character_list(&mut run);
    let mut world = into_world(&mut run, &login);
    ack_portal_exit(&mut run, &world);
    run.session
        .send(Instant::now(), Command::LeaveWorld)
        .expect("leave");
    loop {
        let (op, _) = run.wait_blob(WORLD_SERVER);
        if op == 0x30 {
            break;
        }
    }
    // The teardown on the world connection, and no list after it: the return comes at
    // once, and the next `0x6F` goes to the world server, which keeps the session.
    run.world
        .deliver(world.event(0x3000_001C, &[0]), WORLD_SERVER);
    run.wait_event(|e| matches!(e, Event::ReturnedToCharacterSelect { .. }));
    assert_eq!(run.session.state(), tessera_net::SessionState::Connected);
    run.session
        .send(
            Instant::now(),
            Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
        )
        .expect("enter again");
    loop {
        let (op, _) = run.wait_blob(WORLD_SERVER);
        if op == 0x6F {
            break;
        }
    }
    // The world answers on the same connection: in the world again (referral path).
    let d = world.event(0x90, &session_ready());
    run.world.deliver(d, WORLD_SERVER);
    run.wait_event(|e| matches!(e, Event::WorldEntered { .. }));
    assert_eq!(run.session.state(), tessera_net::SessionState::InWorld);
    let _ = run.session.send(Instant::now(), Command::Disconnect);
}
