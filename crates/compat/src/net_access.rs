// SPDX-License-Identifier: GPL-3.0-or-later
//! The network-access type of `tessera-compat` (`docs/specs/official-entry.md` §6.1).
//!
//! Every operation of this crate that can send or receive on a network (sockets, name
//! resolution, a request of any protocol, whatever its purpose) is reachable only through a
//! [`NetworkAccess`] value, whose constructor takes a reference to a
//! [`ConsentProof`]. This file is the only one of the crate that names a socket type or a
//! network-I/O crate; a source-scan test (OE-T8 (b)) enforces that. The UDP transport of the
//! session module ([`StdUdpTransport`], `crate::session`) is made from a network-access value,
//! never from a bare address.

use std::io::Read as _;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use crate::CompatError;
use crate::consent::ConsentProof;
use crate::login::{
    HttpClient, HttpMethod, HttpRequest, HttpResponse, LoginError, MAX_XML_INPUT_BYTES, NotSentKind,
};
use crate::session::{MAX_DATAGRAM_BUF, SessionError, UdpDatagramTransport, candidate_bind_ports};

/// Network-access values made in this process (tests: OE-T8 (a), OE-T9).
static CREATED: AtomicU64 = AtomicU64::new(0);

/// How many [`NetworkAccess`] values were made in this process.
#[must_use]
pub fn created_count() -> u64 {
    CREATED.load(Ordering::SeqCst)
}

/// The right to use the network, held only after the ban-risk warning was accepted (§6.1).
///
/// It can be made only from a proof. Each `compile_fail` example below differs from the
/// compiling one only in the step it is about (the error code is checked by nightly rustdoc):
///
/// ```
/// use tessera_compat::CompatError;
/// use tessera_compat::consent::ConsentProof;
/// use tessera_compat::net_access::NetworkAccess;
/// fn made(p: &ConsentProof) -> Result<NetworkAccess, CompatError> {
///     NetworkAccess::new(p)
/// }
/// ```
///
/// No default value (`NetworkAccess: Default` is not satisfied):
///
/// ```compile_fail,E0277
/// use tessera_compat::net_access::NetworkAccess;
/// fn made() -> NetworkAccess {
///     Default::default()
/// }
/// ```
///
/// No construction from parts (the field is private):
///
/// ```compile_fail,E0451
/// use tessera_compat::net_access::NetworkAccess;
/// fn made() -> NetworkAccess {
///     NetworkAccess { _private: () }
/// }
/// ```
#[derive(Debug)]
pub struct NetworkAccess {
    _private: (),
    right: Right,
}

/// The revocation epoch of this process: [`revoke_all`] moves it on, and every value made
/// before that is dead (Withdraw, `official-entry.md` §5.5).
static EPOCH: AtomicU64 = AtomicU64::new(0);

/// Ends every network right of this process at once: each [`NetworkAccess`] made so far and
/// everything made from one (the HTTPS clients, the UDP transports) refuses every further
/// operation. The client calls it when the user withdraws the acceptance
/// (`official-entry.md` §5.5 "Withdraw always ends the run's acceptance, at once"), so no
/// live network right is left anywhere, a worker that is still running included. Values
/// made afterwards (from a new acceptance) are live.
pub fn revoke_all() {
    EPOCH.fetch_add(1, Ordering::SeqCst);
    tracing::info!("official network access revoked");
}

/// One network right: live until its own revocation flag is set or [`revoke_all`] runs.
/// Shared by a [`NetworkAccess`] and everything made from it.
#[derive(Debug, Clone)]
struct Right {
    epoch: u64,
    revoked: Arc<AtomicBool>,
}

impl Right {
    fn new() -> Self {
        Self {
            epoch: EPOCH.load(Ordering::SeqCst),
            revoked: Arc::new(AtomicBool::new(false)),
        }
    }

    fn live(&self) -> bool {
        !self.revoked.load(Ordering::SeqCst) && self.epoch == EPOCH.load(Ordering::SeqCst)
    }

    fn check(&self) -> Result<(), SessionError> {
        if self.live() {
            Ok(())
        } else {
            Err(SessionError::Io(
                "the network access was revoked".to_owned(),
            ))
        }
    }
}

/// Ends one [`NetworkAccess`] and everything made from it (the live connector uses it once
/// a cancelled session's thread had its bounded time to close, `crate::connector`).
#[derive(Debug, Clone)]
pub struct Revocation(Arc<AtomicBool>);

impl Revocation {
    /// Revokes the right.
    pub fn revoke(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether the right was revoked through this handle.
    #[must_use]
    pub fn is_revoked(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

impl NetworkAccess {
    /// A network-access value for `proof`.
    ///
    /// # Errors
    ///
    /// [`CompatError::ConsentMismatch`] when the proof is not for the shipped warning text.
    pub fn new(proof: &ConsentProof) -> Result<Self, CompatError> {
        if !proof.is_current() {
            return Err(CompatError::ConsentMismatch);
        }
        CREATED.fetch_add(1, Ordering::SeqCst);
        Ok(Self {
            _private: (),
            right: Right::new(),
        })
    }

    /// Whether this value may still use the network (not revoked, [`revoke_all`] not run
    /// since it was made).
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.right.live()
    }

    /// The handle that revokes this value and everything made from it.
    #[must_use]
    pub fn revocation(&self) -> Revocation {
        Revocation(Arc::clone(&self.right.revoked))
    }

    /// Binds a non-blocking IPv4 UDP socket following the port-selection rules of the
    /// session module ([`candidate_bind_ports`]).
    ///
    /// # Errors
    ///
    /// [`SessionError::Io`] if binding or configuring non-blocking mode fails.
    pub fn bind_udp(
        &self,
        local_ip: Ipv4Addr,
        explicit_base_port: u16,
        compute_unique: bool,
    ) -> Result<StdUdpTransport, SessionError> {
        self.right.check()?;
        StdUdpTransport::bind(
            local_ip,
            explicit_base_port,
            compute_unique,
            self.right.clone(),
        )
    }

    /// The HTTP(S) client of the login exchanges (`official-login.md` §3, §4.3, §6.2). It
    /// shares this value's right: once that is revoked the client refuses every request.
    #[must_use]
    pub fn login_http_client(&self) -> LoginHttpClient {
        LoginHttpClient::new(self.right.clone())
    }

    /// Resolves `host` (a name or an IPv4 literal) and `port` to the first IPv4 address
    /// (`official-session.md` §3.3: endpoints are resolved at run time; the session is
    /// IPv4 only, §3.1).
    ///
    /// # Errors
    ///
    /// [`SessionError::Io`] when the name does not resolve or has no IPv4 address.
    pub fn resolve_ipv4(&self, host: &str, port: u16) -> Result<SocketAddrV4, SessionError> {
        self.right.check()?;
        if let Ok(ip) = host.parse::<Ipv4Addr>() {
            return Ok(SocketAddrV4::new(ip, port));
        }
        // The system resolver has no timeout of its own: it runs on a helper thread and is
        // waited for at most `RESOLVE_TIMEOUT` (engineering choice, spec silent). A lookup
        // that outlasts it is abandoned; its thread ends when the resolver returns.
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let name = host.to_owned();
        std::thread::Builder::new()
            .name("tessera-official-resolve".into())
            .spawn(move || {
                let found = (name.as_str(), port)
                    .to_socket_addrs()
                    .ok()
                    .and_then(|addrs| {
                        addrs.take(MAX_RESOLVED_ADDRESSES).find_map(|a| match a {
                            SocketAddr::V4(v4) => Some(v4),
                            SocketAddr::V6(_) => None,
                        })
                    });
                let _ = tx.send(found);
            })
            .map_err(|_| SessionError::Io("the name lookup could not start".to_owned()))?;
        match rx.recv_timeout(RESOLVE_TIMEOUT) {
            Ok(Some(v4)) => Ok(v4),
            Ok(None) => Err(SessionError::Io(
                "the server name did not resolve to an IPv4 address".to_owned(),
            )),
            Err(_) => Err(SessionError::Io("the name lookup timed out".to_owned())),
        }
    }

    /// The local IPv4 address the operating system routes towards `remote`: the first
    /// non-loopback local address of `official-session.md` §3.2 item 1 without an
    /// interface list. A UDP socket is bound to `0.0.0.0:0` and connected (a connected UDP
    /// socket sends nothing); `None` when that fails or gives a loopback or unspecified
    /// address. The session then binds to that address (Case B of §3.2 needs it); the
    /// address the system routes towards the server stands for "the first non-loopback
    /// local address" of §3.2, which names no order (engineering choice, spec silent
    /// (question with the spec writers)). `Net.BindInterface` and `Net.ComputeUniquePort`
    /// of `UserPreferences.ini` are not read (the same question): no interface mask is
    /// applied and the computed port is always tried.
    #[must_use]
    pub fn local_ipv4_toward(&self, remote: SocketAddrV4) -> Option<Ipv4Addr> {
        if !self.right.live() {
            return None;
        }
        let sock = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).ok()?;
        sock.connect(SocketAddr::V4(remote)).ok()?;
        match sock.local_addr().ok()? {
            SocketAddr::V4(v4) if !v4.ip().is_loopback() && !v4.ip().is_unspecified() => {
                Some(*v4.ip())
            }
            _ => None,
        }
    }
}

/// Most addresses looked at when a name resolves to many.
const MAX_RESOLVED_ADDRESSES: usize = 64;

/// Longest wait for a name lookup (engineering choice, spec silent).
pub const RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);

/// Longest response head (status line and headers) of a login exchange. Engineering
/// choice, spec silent: the service's heads are small.
pub const MAX_RESPONSE_HEADER_BYTES: usize = 16 * 1024;

/// The `User-Agent` of the login exchanges: a neutral value that names no product or mark.
/// Engineering choice, spec silent (question with the spec writers).
pub const USER_AGENT: &str = "login-client/1";

/// Longest response body read from a login exchange: the login module's parsers refuse
/// anything larger than [`MAX_XML_INPUT_BYTES`], so one more byte is enough to tell.
const MAX_RESPONSE_BYTES: u64 = MAX_XML_INPUT_BYTES as u64 + 1;

/// The longest wait for a connection of the login client, TCP and TLS handshake together,
/// and, separately, for its name lookup (decision 0044 as amended 2026-10-27: a short connect
/// timeout for the HTTPS form of an `http://` address; engineering choice, 5 s, applied to
/// every login request). Only a failure within these two phases lets the login ask an
/// upgraded address as given, so that attempt costs at most twice this before the plain
/// request starts (`crate::connector::login_phase::exchange`).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// The HTTP(S) client of the login exchanges (`official-login.md` §3, §4.3), made only by
/// [`NetworkAccess::login_http_client`].
///
/// - TLS 1.2 or 1.3 through rustls with the ring provider, the operating system's
///   certificate verifier and hostname verification; verification is never turned off
///   (§6.2).
/// - No redirect is followed: a login request goes only to the URL the install or a
///   service response named (§6.1); a redirect answer is returned as its status code.
/// - No proxy: the environment's proxy settings (`HTTP_PROXY` and the like, which the HTTP
///   crate's defaults read) are not used, so a request goes only to the host its URL names
///   (engineering choice, spec silent).
/// - A neutral `User-Agent` ([`USER_AGENT`], engineering choice, spec silent).
/// - The request's `timeout_sec` bounds the whole exchange (§5.2); the name lookup and the
///   connection (TCP and TLS handshake) are each given at most [`CONNECT_TIMEOUT`]; a
///   response head is at most [`MAX_RESPONSE_HEADER_BYTES`].
/// - Response bodies are read up to the login parsers' input bound plus one byte, into a
///   buffer allocated once at that size, so no copy of a body is left behind by a
///   reallocation. The HTTP and TLS crates' own buffers cannot be wiped from here (known
///   limitation, `official-login.md` §6.3 is best effort).
/// - A URL is sent with the scheme it has: `https://` with the checks above, `http://` as
///   given. Which one a step uses is the login phase's rule (decision 0044 as amended
///   2026-10-27: HTTPS first, plain HTTP only when the HTTPS form failed before the request
///   was sent).
/// - Where a failure happened is reported: [`LoginError::NotSent`] with
///   [`NotSentKind::Unreached`] when the connection was never made (name lookup, connect,
///   connect timeout, TLS handshake or certificate check: no byte of the request left the
///   client), [`NotSentKind::Refused`] when the client refused to send, and
///   [`LoginError::ServiceUnavailable`] for any failure once the connection was made (the
///   request may have been sent). Each request opens its own connection (no connection is
///   kept for reuse), so "the connection was made" is known for every request
///   (engineering choice, spec silent).
/// - It shares the right of the [`NetworkAccess`] that made it: once that is revoked every
///   request is refused before anything is sent.
#[derive(Debug)]
pub struct LoginHttpClient {
    agent: ureq::Agent,
    right: Right,
    /// Set by [`ReachWatch`] once the request's connection (TCP and, for `https://`, TLS)
    /// was made; cleared before each request.
    reached: Arc<AtomicBool>,
}

/// The HTTP crate's connector chain, watched: it marks the moment a connection is made.
#[derive(Debug)]
struct ReachWatch {
    inner: ureq::unversioned::transport::DefaultConnector,
    reached: Arc<AtomicBool>,
}

impl ureq::unversioned::transport::Connector<()> for ReachWatch {
    type Out = Box<dyn ureq::unversioned::transport::Transport>;

    fn connect(
        &self,
        details: &ureq::unversioned::transport::ConnectionDetails,
        chained: Option<()>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        let out = self.inner.connect(details, chained)?;
        if out.is_some() {
            self.reached.store(true, Ordering::SeqCst);
        }
        Ok(out)
    }
}

impl LoginHttpClient {
    fn new(right: Right) -> Self {
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::Rustls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .disable_verification(false)
            .build();
        let config = ureq::Agent::config_builder()
            .tls_config(tls)
            .proxy(None)
            .user_agent(USER_AGENT)
            .http_status_as_error(false)
            .timeout_resolve(Some(CONNECT_TIMEOUT))
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .max_redirects(0)
            .max_redirects_will_error(false)
            .max_response_header_size(MAX_RESPONSE_HEADER_BYTES)
            .max_idle_connections(0)
            .max_idle_connections_per_host(0)
            .build();
        let reached = Arc::new(AtomicBool::new(false));
        let connector = ReachWatch {
            inner: ureq::unversioned::transport::DefaultConnector::default(),
            reached: Arc::clone(&reached),
        };
        Self {
            agent: ureq::Agent::with_parts(
                config,
                connector,
                ureq::unversioned::resolver::DefaultResolver::default(),
            ),
            right,
            reached,
        }
    }
}

/// The failure of a request: [`NotSentKind::Unreached`] when `reached` is `false` and the
/// failure is one of reaching the service, otherwise final.
fn http_failure(e: &ureq::Error, reached: bool, scheme: &'static str) -> LoginError {
    let reason = match e {
        ureq::Error::Timeout(_) => "the request timed out",
        ureq::Error::HostNotFound => "the service name did not resolve",
        ureq::Error::ConnectionFailed => "the connection failed",
        ureq::Error::Io(_) if !reached => "the connection failed",
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => {
            "the TLS handshake or certificate check failed"
        }
        ureq::Error::BodyExceedsLimit(_) => "the response is too large",
        ureq::Error::LargeResponseHeader(_, _) => "the response head is too large",
        ureq::Error::BadUri(_) => "the service URL is not valid",
        ureq::Error::RequireHttpsOnly(_) => "a request that needs HTTPS named another scheme",
        _ => "the request failed",
    };
    let unreached = !reached
        && matches!(
            e,
            ureq::Error::Timeout(_)
                | ureq::Error::HostNotFound
                | ureq::Error::ConnectionFailed
                | ureq::Error::Io(_)
                | ureq::Error::Tls(_)
                | ureq::Error::Rustls(_)
        );
    // The kind and the scheme only: the HTTP crate's text can name the URL.
    tracing::debug!(
        reason,
        scheme,
        before_sending = unreached,
        "official login request failed"
    );
    if unreached {
        LoginError::NotSent {
            kind: NotSentKind::Unreached,
            reason: reason.to_owned(),
        }
    } else {
        LoginError::ServiceUnavailable {
            reason: reason.to_owned(),
        }
    }
}

impl HttpClient for LoginHttpClient {
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        if !self.right.live() {
            return Err(LoginError::NotSent {
                kind: NotSentKind::Refused,
                reason: "the network access was revoked".to_owned(),
            });
        }
        if !request.require_tls_validation {
            // Every request of the login module asks for validation (§6.2); one that does
            // not is refused rather than sent with weaker checks.
            return Err(LoginError::NotSent {
                kind: NotSentKind::Refused,
                reason: "a request without certificate validation was refused".to_owned(),
            });
        }
        let scheme = crate::connector::net::scheme_label(&request.url);
        let timeout = Some(Duration::from_secs(request.timeout_sec.clamp(1, 300)));
        self.reached.store(false, Ordering::SeqCst);
        let result = match request.method {
            HttpMethod::Get => {
                let mut b = self
                    .agent
                    .get(request.url.as_str())
                    .config()
                    .timeout_global(timeout)
                    .build();
                for (k, v) in &request.headers {
                    b = b.header(k.as_str(), v.as_str());
                }
                b.call()
            }
            HttpMethod::Post => {
                let mut b = self
                    .agent
                    .post(request.url.as_str())
                    .config()
                    .timeout_global(timeout)
                    .build();
                for (k, v) in &request.headers {
                    b = b.header(k.as_str(), v.as_str());
                }
                b.send(request.body.as_slice())
            }
        };
        let mut response =
            result.map_err(|e| http_failure(&e, self.reached.load(Ordering::SeqCst), scheme))?;
        let status_code = response.status().as_u16();
        // One allocation of the full bound: `read_to_end` never grows it (the reader stops
        // at the bound), so no partial copy of a body is freed unwiped.
        let mut body = Vec::with_capacity(MAX_RESPONSE_BYTES as usize);
        let read = response
            .body_mut()
            .as_reader()
            .take(MAX_RESPONSE_BYTES)
            .read_to_end(&mut body);
        if read.is_err() {
            crate::login::zeroize_vec(&mut body);
            // A status line arrived: this is the service's answer, never one to ask again.
            tracing::debug!(scheme, "official login request failed: body unreadable");
            return Err(LoginError::ServiceUnavailable {
                reason: "the response could not be read".to_owned(),
            });
        }
        Ok(HttpResponse { status_code, body })
    }
}

/// Standard library non-blocking IPv4 `UdpSocket` transport of the session module, made
/// only by [`NetworkAccess::bind_udp`].
#[derive(Debug)]
pub struct StdUdpTransport {
    socket: UdpSocket,
    local_addr: SocketAddrV4,
    right: Right,
}

impl StdUdpTransport {
    fn bind(
        local_ip: Ipv4Addr,
        explicit_base_port: u16,
        compute_unique: bool,
        right: Right,
    ) -> Result<Self, SessionError> {
        let ports = candidate_bind_ports(local_ip, explicit_base_port, compute_unique);
        let mut bound_socket = None;
        for port in ports {
            if let Ok(sock) = UdpSocket::bind(SocketAddrV4::new(local_ip, port)) {
                bound_socket = Some(sock);
                break;
            }
        }
        // Fallback to an ephemeral port on `local_ip`, then INADDR_ANY (0.0.0.0:0).
        if bound_socket.is_none() {
            bound_socket = UdpSocket::bind(SocketAddrV4::new(local_ip, 0))
                .or_else(|_| UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)))
                .ok();
        }
        let socket = bound_socket
            .ok_or_else(|| SessionError::Io("failed to bind IPv4 UDP socket".to_string()))?;
        socket.set_nonblocking(true)?;
        let local_addr = match socket.local_addr()? {
            SocketAddr::V4(v4) => v4,
            SocketAddr::V6(_) => {
                return Err(SessionError::Io("expected IPv4 socket address".to_string()));
            }
        };
        Ok(Self {
            socket,
            local_addr,
            right,
        })
    }
}

impl UdpDatagramTransport for StdUdpTransport {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        Ok(self.local_addr)
    }

    fn send_to(&mut self, buf: &[u8], target: SocketAddrV4) -> Result<usize, SessionError> {
        self.right.check()?;
        if buf.len() > MAX_DATAGRAM_BUF {
            return Err(SessionError::DatagramTooLarge { len: buf.len() });
        }
        Ok(self.socket.send_to(buf, SocketAddr::V4(target))?)
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        self.right.check()?;
        match self.socket.recv_from(buf) {
            Ok((len, SocketAddr::V4(src))) => Ok(Some((len, src))),
            Ok((_, SocketAddr::V6(_))) => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(SessionError::Io(e.to_string())),
        }
    }
}

/// [`revoke_all`] ends every right of the process: a test that calls it holds this for
/// writing, and a test that needs its rights to stay live holds it for reading.
#[cfg(test)]
pub(crate) static TEST_REVOKE_LOCK: std::sync::RwLock<()> = std::sync::RwLock::new(());

#[cfg(test)]
mod tests {
    //! The HTTPS client against a loopback HTTP server (plain HTTP: no certificate is
    //! needed to check the request and answer handling). Nothing leaves the machine.
    use std::io::Write as _;
    use std::net::TcpListener;

    use super::*;
    use crate::consent::{self, UserConfirmation, WarningText};

    use super::TEST_REVOKE_LOCK as REVOKE;

    struct Fixture {
        _guard: std::sync::RwLockReadGuard<'static, ()>,
        _dir: tempfile::TempDir,
    }

    fn proof_in(dir: &std::path::Path) -> ConsentProof {
        let (acc, write) = consent::accept(
            Some(dir),
            &WarningText::shipped(),
            UserConfirmation::confirmed_by_user(),
        );
        let _ = write.wait();
        acc.proof()
    }

    fn access() -> (Fixture, NetworkAccess) {
        let guard = REVOKE
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().expect("temp");
        let access = NetworkAccess::new(&proof_in(dir.path())).expect("access");
        (
            Fixture {
                _guard: guard,
                _dir: dir,
            },
            access,
        )
    }

    /// Serves one connection with `answer` and returns the request it read.
    fn serve(answer: Vec<u8>) -> (String, std::thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().expect("accept");
            s.set_read_timeout(Some(Duration::from_secs(5)))
                .expect("timeout");
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            while let Ok(n) = s.read(&mut buf) {
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..n]);
                if let Some(end) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&req[..end]).to_ascii_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if req.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            let _ = s.write_all(&answer);
            req
        });
        (format!("http://{addr}/path"), handle)
    }

    fn request(method: HttpMethod, url: &str, body: &[u8]) -> HttpRequest {
        HttpRequest {
            method,
            url: url.to_owned(),
            headers: vec![(
                "Content-Type".to_owned(),
                "text/xml; charset=utf-8".to_owned(),
            )],
            body: body.to_vec(),
            timeout_sec: 5,
            require_tls_validation: true,
        }
    }

    #[test]
    fn a_post_returns_status_and_body() {
        let (_f, access) = access();
        let (url, server) =
            serve(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\nConnection: close\r\n\r\nfault".to_vec());
        let resp = access
            .login_http_client()
            .execute(&request(HttpMethod::Post, &url, b"<x/>"))
            .expect("answer");
        assert_eq!(resp.status_code, 500);
        assert_eq!(resp.body, b"fault");
        let req = String::from_utf8(server.join().expect("server")).expect("utf8");
        assert!(req.starts_with("POST /path HTTP/1.1"), "{req}");
        assert!(req.ends_with("<x/>"), "{req}");
    }

    #[test]
    fn a_redirect_is_not_followed() {
        let (_f, access) = access();
        let (url, server) = serve(
            b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_vec(),
        );
        let resp = access
            .login_http_client()
            .execute(&request(HttpMethod::Get, &url, b""))
            .expect("answer");
        assert_eq!(resp.status_code, 302);
        server.join().expect("server");
    }

    #[test]
    fn a_large_body_is_cut_after_the_parsers_bound() {
        let (_f, access) = access();
        let size = MAX_XML_INPUT_BYTES + 4096;
        let mut answer =
            format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n")
                .into_bytes();
        answer.extend(std::iter::repeat_n(b'a', size));
        let (url, server) = serve(answer);
        let resp = access
            .login_http_client()
            .execute(&request(HttpMethod::Get, &url, b""))
            .expect("answer");
        assert_eq!(resp.body.len() as u64, MAX_RESPONSE_BYTES);
        server.join().expect("server");
    }

    #[test]
    fn a_request_without_validation_is_refused() {
        let (_f, access) = access();
        let mut req = request(HttpMethod::Get, "https://a.example.invalid/", b"");
        req.require_tls_validation = false;
        assert!(access.login_http_client().execute(&req).is_err());
    }

    #[test]
    fn literal_addresses_resolve_without_a_lookup() {
        let (_f, access) = access();
        assert_eq!(
            access.resolve_ipv4("192.0.2.7", 9000).expect("literal"),
            SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 7), 9000)
        );
    }

    /// A server that accepts one connection, reads the request head and then sends `head`
    /// one byte every `gap`.
    fn drip(head: Vec<u8>, gap: Duration) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let Ok((mut s, _)) = listener.accept() else {
                return;
            };
            let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            for b in head {
                if s.write_all(&[b]).is_err() {
                    return;
                }
                std::thread::sleep(gap);
            }
        });
        (format!("http://{addr}/slow"), handle)
    }

    #[test]
    fn a_drip_feeding_server_times_out() {
        let (_f, access) = access();
        let mut head = b"HTTP/1.1 200 OK\r\nContent-Length: 4000\r\n\r\n".to_vec();
        head.extend(std::iter::repeat_n(b'a', 4000));
        let (url, server) = drip(head, Duration::from_millis(20));
        let mut req = request(HttpMethod::Get, &url, b"");
        req.timeout_sec = 1;
        let t = std::time::Instant::now();
        assert!(access.login_http_client().execute(&req).is_err());
        assert!(t.elapsed() < Duration::from_secs(4), "{:?}", t.elapsed());
        drop(server);
    }

    #[test]
    fn an_oversized_response_head_is_refused() {
        let (_f, access) = access();
        let mut answer = b"HTTP/1.1 200 OK\r\n".to_vec();
        for i in 0..2_000 {
            answer.extend(format!("X-Filler-{i}: {}\r\n", "b".repeat(20)).into_bytes());
        }
        answer.extend(b"Content-Length: 0\r\n\r\n");
        let (url, server) = serve(answer);
        let err = access
            .login_http_client()
            .execute(&request(HttpMethod::Get, &url, b""))
            .expect_err("refused");
        assert!(matches!(err, LoginError::ServiceUnavailable { .. }));
        assert!(!err.is_unreached(), "a head arrived: final");
        let _ = server.join();
    }

    /// A server that accepts one connection, keeps what it reads for `read_for` (or until
    /// the peer closes), then sends `answer` and closes; returns what it read.
    fn take_then(
        answer: Vec<u8>,
        read_for: Duration,
    ) -> (SocketAddr, std::thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let Ok((mut s, _)) = listener.accept() else {
                return Vec::new();
            };
            let _ = s.set_read_timeout(Some(Duration::from_millis(200)));
            let start = std::time::Instant::now();
            let mut seen = Vec::new();
            let mut buf = [0u8; 4096];
            while start.elapsed() < read_for {
                match s.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => seen.extend_from_slice(&buf[..n]),
                    Err(_) => {}
                }
            }
            let _ = s.write_all(&answer);
            seen
        });
        (addr, handle)
    }

    #[test]
    fn a_refused_connection_is_not_reached() {
        let (_f, access) = access();
        let port = {
            let l = TcpListener::bind("127.0.0.1:0").expect("loopback");
            l.local_addr().expect("addr").port()
        };
        let err = access
            .login_http_client()
            .execute(&request(
                HttpMethod::Post,
                &format!("http://127.0.0.1:{port}/x"),
                b"secret",
            ))
            .expect_err("refused");
        assert!(err.is_unreached(), "{err:?}");
    }

    #[test]
    fn a_failed_tls_handshake_is_not_reached_and_sends_no_request() {
        // The server speaks plain HTTP on the HTTPS form's port: the handshake fails, and
        // the request (its path and body) never left the client.
        let (_f, access) = access();
        let (addr, server) = take_then(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec(),
            Duration::from_millis(500),
        );
        let mut req = request(HttpMethod::Post, &format!("https://{addr}/path"), b"secret");
        req.timeout_sec = 5;
        let err = access
            .login_http_client()
            .execute(&req)
            .expect_err("no TLS");
        assert!(err.is_unreached(), "{err:?}");
        let seen = server.join().expect("server");
        assert!(!seen.windows(5).any(|w| w == b"/path"));
        assert!(!seen.windows(6).any(|w| w == b"secret"));
    }

    #[test]
    fn a_server_that_takes_the_request_and_stalls_or_resets_is_final() {
        let (_f, access) = access();
        // Stalls past the request's timeout.
        let (addr, server) = take_then(Vec::new(), Duration::from_secs(3));
        let mut req = request(HttpMethod::Post, &format!("http://{addr}/x"), b"secret");
        req.timeout_sec = 1;
        let err = access.login_http_client().execute(&req).expect_err("stall");
        assert!(
            matches!(err, LoginError::ServiceUnavailable { .. }),
            "{err:?}"
        );
        assert!(server.join().expect("server").ends_with(b"secret"));
        // Closes the connection after taking the request.
        let (addr, server) = take_then(Vec::new(), Duration::from_millis(300));
        let mut req = request(HttpMethod::Post, &format!("http://{addr}/x"), b"secret");
        req.timeout_sec = 5;
        let err = access.login_http_client().execute(&req).expect_err("reset");
        assert!(!err.is_unreached(), "{err:?}");
        assert!(server.join().expect("server").ends_with(b"secret"));
        // A status line, then a body cut short.
        let (addr, server) = take_then(
            b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nshort".to_vec(),
            Duration::from_millis(300),
        );
        let mut req = request(HttpMethod::Get, &format!("http://{addr}/x"), b"");
        req.timeout_sec = 5;
        let err = access.login_http_client().execute(&req).expect_err("body");
        assert!(
            matches!(err, LoginError::ServiceUnavailable { .. }),
            "{err:?}"
        );
        let _ = server.join();
    }

    #[test]
    fn a_revoked_or_unvalidated_request_is_refused_not_unreached() {
        let (_f, access) = access();
        let mut http = access.login_http_client();
        let mut req = request(HttpMethod::Get, "https://a.example.invalid/", b"");
        req.require_tls_validation = false;
        let err = http.execute(&req).expect_err("refused");
        assert!(matches!(
            err,
            LoginError::NotSent {
                kind: NotSentKind::Refused,
                ..
            }
        ));
        access.revocation().revoke();
        req.require_tls_validation = true;
        let err = http.execute(&req).expect_err("revoked");
        assert!(matches!(
            err,
            LoginError::NotSent {
                kind: NotSentKind::Refused,
                ..
            }
        ));
    }

    #[test]
    fn a_secret_goes_to_the_url_with_the_scheme_it_has() {
        // Decision 0044 as amended 2026-10-27: the login phase decides when a credential may
        // go over plain HTTP (only after its HTTPS form failed); the client sends it as given.
        let (_f, access) = access();
        let (url, server) = serve(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec());
        let mut req = request(HttpMethod::Post, &url, b"secret");
        req.headers
            .push(("SOAPAction".to_owned(), "\"x/LoginAccount\"".to_owned()));
        let resp = access.login_http_client().execute(&req).expect("answered");
        assert_eq!(resp.status_code, 200);
        let seen = server.join().expect("server");
        assert!(seen.ends_with(b"secret"));
    }

    #[test]
    fn revocation_ends_every_right_made_from_a_value() {
        let (_f, access) = access();
        let mut udp = access
            .bind_udp(Ipv4Addr::LOCALHOST, 0, false)
            .expect("bind");
        let me = udp.local_addr().expect("addr");
        let mut http = access.login_http_client();
        assert!(access.is_live());
        access.revocation().revoke();
        assert!(!access.is_live());
        assert!(udp.send_to(b"x", me).is_err());
        assert!(udp.recv_from(&mut [0u8; 8]).is_err());
        assert!(access.bind_udp(Ipv4Addr::LOCALHOST, 0, false).is_err());
        assert!(access.resolve_ipv4("192.0.2.1", 1).is_err());
        assert!(
            access
                .local_ipv4_toward(SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 1))
                .is_none()
        );
        assert!(
            http.execute(&request(HttpMethod::Get, "http://127.0.0.1:9/", b""))
                .is_err()
        );
    }

    #[test]
    fn the_connector_network_holds_no_right_after_revocation() {
        use crate::connector::net::{ConnectorNet, LiveNet};
        let (_f, access) = access();
        let revocation = access.revocation();
        let mut net = LiveNet::new(access);
        let mut binder = net.chat_binder().expect("binder");
        let mut background = net.background_http();
        revocation.revoke();
        assert!(net.bind_udp(Ipv4Addr::LOCALHOST, 0, false).is_err());
        assert!(binder().is_err());
        assert!(net.resolve("192.0.2.1", 1).is_err());
        let req = request(HttpMethod::Get, "http://127.0.0.1:9/", b"");
        assert!(net.http(&req).is_err());
        assert!(background.execute(&req).is_err());
    }

    #[test]
    fn revoke_all_ends_every_value_made_before_it() {
        let guard = REVOKE
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().expect("temp");
        let proof = proof_in(dir.path());
        let old = NetworkAccess::new(&proof).expect("access");
        let mut udp = old.bind_udp(Ipv4Addr::LOCALHOST, 0, false).expect("bind");
        let me = udp.local_addr().expect("addr");
        revoke_all();
        assert!(!old.is_live());
        assert!(udp.send_to(b"x", me).is_err());
        assert!(old.bind_udp(Ipv4Addr::LOCALHOST, 0, false).is_err());
        // A value made afterwards (a new acceptance) is live.
        let new = NetworkAccess::new(&proof).expect("access");
        assert!(new.is_live());
        drop(guard);
    }
}
