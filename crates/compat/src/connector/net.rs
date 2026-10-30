// SPDX-License-Identifier: GPL-3.0-or-later
//! The network operations of the live connector, all through [`NetworkAccess`]
//! (`official-entry.md` §6.1).
//!
//! [`ConnectorNet`] is what the connector's thread may do on a network: HTTP(S) requests,
//! name resolution, the local address towards a server and binding the session's UDP
//! socket. [`LiveNet`] does each through the run's network-access value; tests use a
//! synthetic implementation that opens nothing.

use std::net::{Ipv4Addr, SocketAddrV4};

use crate::login::{HttpClient, HttpRequest, HttpResponse, LoginError, NotSentKind};
use crate::net_access::{LoginHttpClient, NetworkAccess, StdUdpTransport};
use crate::session::{BindPlan, SessionError, UdpDatagramTransport};

/// An HTTP client the connector can hand to another thread (the ticket refresh, which
/// must not stall the session loop, `official-login.md` §4.3).
pub type SendHttpClient = Box<dyn HttpClient + Send>;

/// The network operations the connector's thread uses.
pub trait ConnectorNet: Send + std::fmt::Debug + 'static {
    /// The session's UDP transport.
    type Udp: UdpDatagramTransport + 'static;

    /// One HTTP exchange of the login (`official-login.md` §3).
    ///
    /// # Errors
    ///
    /// The client's failure ([`HttpClient::execute`]): [`LoginError::NotSent`] when nothing
    /// was sent, [`LoginError::ServiceUnavailable`] after the connection was made.
    fn http(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError>;

    /// A separate HTTP client for a background request.
    fn background_http(&self) -> SendHttpClient;

    /// Whether the network right still holds: `false` once it was revoked (the reaper, or
    /// Withdraw's [`crate::net_access::revoke_all`]). Nothing can be sent or received any
    /// more then, so the connector's thread ends instead of trying.
    fn is_live(&self) -> bool;

    /// Resolves `host` and `port` to an IPv4 address (`official-session.md` §3.3).
    ///
    /// # Errors
    ///
    /// [`SessionError::Io`] when the name does not resolve to an IPv4 address.
    fn resolve(&mut self, host: &str, port: u16) -> Result<SocketAddrV4, SessionError>;

    /// The first local IPv4 address, the Case B seed of `official-session.md` §3.2 item 2:
    /// the address the system routes towards `remote`.
    fn local_ipv4_toward(&mut self, remote: SocketAddrV4) -> Option<Ipv4Addr>;

    /// Binds the session's socket at `plan` (`official-session.md` §3.1–§3.2).
    ///
    /// # Errors
    ///
    /// [`SessionError::Io`] or [`SessionError::LocalPortsExhausted`] when no port can be
    /// bound.
    fn bind_udp(&mut self, plan: BindPlan) -> Result<Self::Udp, SessionError>;

    /// A maker of fresh sockets for the chat-server client (each connection attempt binds
    /// its own, `official-messages-play.md` §6.6), or `None` when this network has none.
    /// Every socket binds by the chat transport's defaults (`official-session.md` §3.2
    /// item 1: [`BindPlan::chat_defaults`]): the wildcard address and Case B seeded from
    /// `first_local`, the session's first local IPv4 address (Case C without one).
    #[allow(clippy::type_complexity)]
    fn chat_binder(
        &self,
        first_local: Option<Ipv4Addr>,
    ) -> Option<Box<dyn FnMut() -> Result<Self::Udp, crate::play::PlayError> + Send>>;
}

/// Whether `request` carries the password or the ticket: `LoginAccount` and
/// `RefreshTicket` (`official-login.md` §3.4, §4.3) and the queue's form posts (§3.5,
/// `ticket=` / the subscription). Tests only: such a request follows the rule of every
/// other (`login_phase` "HTTPS first, then the address as given"), and the tests use this
/// to check where a credential went.
#[cfg(test)]
#[must_use]
pub(crate) fn carries_secret(request: &HttpRequest) -> bool {
    if request.method != crate::login::HttpMethod::Post {
        return false;
    }
    let soap = request
        .soap_action()
        .is_some_and(|a| a.contains("LoginAccount") || a.contains("RefreshTicket"));
    let form = request
        .content_type()
        .is_some_and(|c| c.starts_with("application/x-www-form-urlencoded"));
    soap || form
}

/// Whether `url` is an `https://` URL that [`https_form`] accepts as it is (tests only).
#[cfg(test)]
#[must_use]
pub(crate) fn is_https(url: &str) -> bool {
    https_form(url).is_some_and(|h| !h.upgraded)
}

/// The HTTPS form of an address of the login chain, the one tried first (decision 0044 as
/// amended 2026-10-27; `official-login.md` §6.2 names the endpoints "HTTP/HTTPS" and is
/// silent on it: question with the spec writers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpsUrl {
    /// The `https://` URL to request.
    pub url: String,
    /// The chain gave the address as `http://`: this is its HTTPS form.
    pub upgraded: bool,
    /// The host without its port, ASCII lower case (brackets of an IPv6 literal kept): the
    /// key under which a login remembers that this host's HTTPS form was not reached
    /// (`login_phase::PlainSteps`).
    pub host: String,
}

/// The HTTPS form of `url`, `None` when it has none:
///
/// - an `https://` URL is used as given (surrounding ASCII whitespace removed);
/// - an `http://` URL becomes `https://` at the same host and path (query and fragment
///   kept). An explicit port is dropped, so the request goes to the default HTTPS port: a
///   port chosen for plain HTTP says nothing about where the service listens for TLS
///   (decision 0044);
/// - any other scheme, user information (`user@host`), an empty host, a port that is not a
///   decimal number, or a control character or space anywhere has no HTTPS form
///   (engineering choice, spec silent: no HTTPS form is guessed for it), for `https://` and
///   `http://` alike; the login refuses such an address and sends nothing to it
///   (`crate::connector::login_phase::exchange`).
#[must_use]
pub fn https_form(url: &str) -> Option<HttpsUrl> {
    let p = parse_chain_url(url)?;
    let host = p.host.to_ascii_lowercase();
    if p.https {
        return Some(HttpsUrl {
            url: p.trimmed.to_owned(),
            upgraded: false,
            host,
        });
    }
    Some(HttpsUrl {
        url: format!("https://{}{}", p.host, p.tail),
        upgraded: true,
        host,
    })
}

/// An `http://` or `https://` address as [`https_form`] reads it.
struct ChainUrl<'a> {
    /// The address without surrounding whitespace.
    trimmed: &'a str,
    /// `https://` (otherwise `http://`).
    https: bool,
    /// The host without its port (brackets of an IPv6 literal kept).
    host: &'a str,
    /// The path, query and fragment.
    tail: &'a str,
}

/// The parse behind [`https_form`].
fn parse_chain_url(url: &str) -> Option<ChainUrl<'_>> {
    // Only ASCII whitespace around the address is removed; any other space or control
    // character, anywhere, refuses it.
    let trimmed = url.trim_matches(|c: char| c.is_ascii_whitespace());
    if trimmed.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let (https, rest) = if let Some(rest) = strip_scheme(trimmed, "https://") {
        (true, rest)
    } else {
        (false, strip_scheme(trimmed, "http://")?)
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let host = authority_host(authority)?;
    Some(ChainUrl {
        trimmed,
        https,
        host,
        tail,
    })
}

/// `url` after `scheme` (compared without regard to ASCII case).
fn strip_scheme<'a>(url: &'a str, scheme: &str) -> Option<&'a str> {
    url.get(..scheme.len())
        .filter(|s| s.eq_ignore_ascii_case(scheme))
        .and_then(|_| url.get(scheme.len()..))
}

/// The host of a URL's authority without its port (brackets of an IPv6
/// literal kept); `None` for user information, an empty host or a malformed port.
fn authority_host(authority: &str) -> Option<&str> {
    if authority.contains('@') {
        return None;
    }
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        // `[` at 0, `]` at `close + 1`: the literal with its brackets.
        let close = inner.find(']')?;
        if close == 0 {
            return None;
        }
        let host = authority.get(..close + 2)?;
        let after = authority.get(close + 2..)?;
        let port = if after.is_empty() {
            ""
        } else {
            after.strip_prefix(':')?
        };
        (host, port)
    } else {
        let (h, p) = authority.rsplit_once(':').unwrap_or((authority, ""));
        if h.is_empty() || h.contains(['[', ']', ':']) {
            return None;
        }
        (h, p)
    };
    port.bytes().all(|b| b.is_ascii_digit()).then_some(host)
}

/// The scheme of `url`: `"https"`, `"http"`, `"other"` or `"none"`, never more of the URL
/// (a host or a path may identify the user's service, `official-entry.md` §7.3). The login
/// client logs it with a failure (`crate::net_access`).
#[must_use]
pub fn scheme_label(url: &str) -> &'static str {
    let url = url.trim();
    match url.find("://").and_then(|i| url.get(..i)) {
        Some(s) if s.eq_ignore_ascii_case("https") => "https",
        Some(s) if s.eq_ignore_ascii_case("http") => "http",
        Some(_) => "other",
        None => "none",
    }
}

/// The connector's network as the login module's HTTP client, stopped by the connector's
/// cancel flag: once the backend let go, no further exchange starts (`crate::connector`
/// "Ending a session"), nor once the network right was revoked. Which address a request
/// goes to is the login phase's rule (`login_phase` "HTTPS first, then the address as
/// given").
pub struct NetHttp<'a, N: ConnectorNet> {
    net: &'a mut N,
    stop: &'a dyn Fn() -> bool,
}

impl<N: ConnectorNet> std::fmt::Debug for NetHttp<'_, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetHttp")
            .field("net", &self.net)
            .finish_non_exhaustive()
    }
}

impl<'a, N: ConnectorNet> NetHttp<'a, N> {
    /// The network `net`; `stop` says whether the attempt was cancelled.
    pub fn new(net: &'a mut N, stop: &'a dyn Fn() -> bool) -> Self {
        Self { net, stop }
    }

    /// Whether the network right still holds ([`ConnectorNet::is_live`]).
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.net.is_live()
    }
}

impl<N: ConnectorNet> HttpClient for NetHttp<'_, N> {
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        if (self.stop)() || !self.net.is_live() {
            return Err(cancelled());
        }
        self.net.http(request)
    }
}

/// The refusal of a request after a stop or cancel: nothing was sent.
fn cancelled() -> LoginError {
    LoginError::NotSent {
        kind: NotSentKind::Refused,
        reason: "the login was cancelled".to_owned(),
    }
}

/// Any HTTP client, stopped by `stop`: checked before every request, so once it holds no
/// further request starts, the plain request after a failed HTTPS form included (the
/// ticket refresh's thread, `session_phase`).
pub struct StopCheckedHttp<'a> {
    http: &'a mut dyn HttpClient,
    stop: &'a dyn Fn() -> bool,
}

impl std::fmt::Debug for StopCheckedHttp<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StopCheckedHttp").finish_non_exhaustive()
    }
}

impl<'a> StopCheckedHttp<'a> {
    /// `http`, refused once `stop` holds.
    pub fn new(http: &'a mut dyn HttpClient, stop: &'a dyn Fn() -> bool) -> Self {
        Self { http, stop }
    }
}

impl HttpClient for StopCheckedHttp<'_> {
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        if (self.stop)() {
            return Err(cancelled());
        }
        self.http.execute(request)
    }
}

/// The connector's network: the attempt's network-access value, moved in (no copy of it
/// exists; the chat-server binder and the background clients share it, and its
/// revocation ends them all).
#[derive(Debug)]
pub struct LiveNet {
    access: std::sync::Arc<NetworkAccess>,
    http: LoginHttpClient,
}

impl LiveNet {
    /// The network of one login attempt, through `access`.
    #[must_use]
    pub fn new(access: NetworkAccess) -> Self {
        let http = access.login_http_client();
        Self {
            access: std::sync::Arc::new(access),
            http,
        }
    }

    /// The network-access value (for the chat-server client, which keeps it).
    #[must_use]
    pub fn access(&self) -> std::sync::Arc<NetworkAccess> {
        std::sync::Arc::clone(&self.access)
    }
}

impl ConnectorNet for LiveNet {
    type Udp = StdUdpTransport;

    fn http(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        self.http.execute(request)
    }

    fn background_http(&self) -> SendHttpClient {
        Box::new(self.access.login_http_client())
    }

    fn is_live(&self) -> bool {
        self.access.is_live()
    }

    fn resolve(&mut self, host: &str, port: u16) -> Result<SocketAddrV4, SessionError> {
        self.access.resolve_ipv4(host, port)
    }

    fn local_ipv4_toward(&mut self, remote: SocketAddrV4) -> Option<Ipv4Addr> {
        self.access.local_ipv4_toward(remote)
    }

    fn bind_udp(&mut self, plan: BindPlan) -> Result<StdUdpTransport, SessionError> {
        self.access.bind_udp(plan)
    }

    fn chat_binder(
        &self,
        first_local: Option<Ipv4Addr>,
    ) -> Option<Box<dyn FnMut() -> Result<StdUdpTransport, crate::play::PlayError> + Send>> {
        let access = self.access();
        // The chat transport reads no `[Net]` key: the defaults (§3.2 item 1).
        let plan = BindPlan::chat_defaults(first_local);
        Some(Box::new(move || {
            access.bind_udp(plan).map_err(crate::play::PlayError::from)
        }))
    }
}

/// Makes the connector's network from the run's network-access value. [`LiveNetFactory`]
/// is the one of this build; tests give one that opens nothing.
pub trait NetFactory: Send + std::fmt::Debug + 'static {
    /// The network.
    type Net: ConnectorNet;

    /// The network of one login attempt.
    fn make(&mut self, access: NetworkAccess) -> Self::Net;
}

/// Makes [`LiveNet`].
#[derive(Debug, Default, Clone, Copy)]
pub struct LiveNetFactory;

impl NetFactory for LiveNetFactory {
    type Net = LiveNet;

    fn make(&mut self, access: NetworkAccess) -> LiveNet {
        LiveNet::new(access)
    }
}
