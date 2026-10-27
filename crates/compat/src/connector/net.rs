// SPDX-License-Identifier: GPL-3.0-or-later
//! The network operations of the live connector, all through [`NetworkAccess`]
//! (`official-entry.md` §6.1).
//!
//! [`ConnectorNet`] is what the connector's thread may do on a network: HTTPS requests,
//! name resolution, the local address towards a server and binding the session's UDP
//! socket. [`LiveNet`] does each through the run's network-access value; tests use a
//! synthetic implementation that opens nothing.

use std::net::{Ipv4Addr, SocketAddrV4};

use crate::login::{HttpClient, HttpMethod, HttpRequest, HttpResponse, LoginError};
use crate::net_access::{HttpsClient, NetworkAccess, StdUdpTransport};
use crate::session::{SessionError, UdpDatagramTransport};

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
    /// [`LoginError::ServiceUnavailable`] when the exchange fails.
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

    /// The local address towards `remote` (`official-session.md` §3.2 item 1).
    fn local_ipv4_toward(&mut self, remote: SocketAddrV4) -> Option<Ipv4Addr>;

    /// Binds the session's socket (`official-session.md` §3.1–§3.2).
    ///
    /// # Errors
    ///
    /// [`SessionError::Io`] when no port can be bound.
    fn bind_udp(
        &mut self,
        local_ip: Ipv4Addr,
        explicit_base_port: u16,
        compute_unique: bool,
    ) -> Result<Self::Udp, SessionError>;

    /// A maker of fresh sockets for the chat-server client (each connection attempt binds
    /// its own, `official-messages-play.md` §6.6), or `None` when this network has none.
    #[allow(clippy::type_complexity)]
    fn chat_binder(
        &self,
    ) -> Option<Box<dyn FnMut() -> Result<Self::Udp, crate::play::PlayError> + Send>>;
}

/// Whether `request` carries the password or the ticket: `LoginAccount` and
/// `RefreshTicket` (`official-login.md` §3.4, §4.3) and the queue's form posts (§3.5,
/// `ticket=` / the subscription).
#[must_use]
pub fn carries_secret(request: &HttpRequest) -> bool {
    if request.method != HttpMethod::Post {
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

/// Whether `url` is an `https://` URL.
#[must_use]
pub fn is_https(url: &str) -> bool {
    url.trim_start()
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("https://"))
}

/// An address of the login chain in the form it is requested at (owner decision
/// 2026-10-27, "upgrade to HTTPS"; `official-login.md` §6.2 names the endpoints
/// "HTTP/HTTPS" and is silent on the upgrade: question with the spec writers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpsUrl {
    /// The `https://` URL to request.
    pub url: String,
    /// The chain gave the address as `http://`: this is its HTTPS form.
    pub upgraded: bool,
}

/// The HTTPS form of `url`, `None` when it has none:
///
/// - an `https://` URL is used as given (surrounding whitespace removed);
/// - an `http://` URL becomes `https://` at the same host and path (query and fragment
///   kept). An explicit port is dropped, so the request goes to the default HTTPS port: a
///   port chosen for plain HTTP says nothing about where the service listens for TLS, and
///   the spec does not say to keep it (engineering choice, question with the spec writers);
/// - any other scheme, an `http://` URL with user information (`user@host`), an empty
///   host, a port that is not a decimal number, or a control character or space anywhere
///   has no HTTPS form (engineering choice, spec silent: such an address is refused rather
///   than guessed at).
#[must_use]
pub fn https_form(url: &str) -> Option<HttpsUrl> {
    let url = url.trim();
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    if is_https(url) {
        return Some(HttpsUrl {
            url: url.to_owned(),
            upgraded: false,
        });
    }
    let scheme = url.get(..7)?;
    if !scheme.eq_ignore_ascii_case("http://") {
        return None;
    }
    let rest = url.get(7..)?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let host = authority_host(authority)?;
    Some(HttpsUrl {
        url: format!("https://{host}{tail}"),
        upgraded: true,
    })
}

/// The host of an `http://` URL's authority without its port (brackets of an IPv6
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

/// The scheme of `url` for a log line: `"https"`, `"http"`, `"other"` or `"none"`, never
/// more of the URL (a host or a path may identify the user's service, `official-entry.md`
/// §7.3).
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

/// The connector's rule over every HTTP exchange: a request that carries the password or
/// the ticket goes only over `https://` (`official-login.md` §6.2, §6.3: the plaintext
/// password and the bearer ticket must not be exposed). Where the spec names an endpoint
/// "HTTP/HTTPS", such a request is refused, not sent: engineering choice, spec silent
/// (question with the spec writers).
///
/// # Errors
///
/// [`LoginError::ServiceUnavailable`] for a refused request or a failed exchange.
pub fn guarded(
    http: &mut (dyn HttpClient + Send),
    request: &HttpRequest,
) -> Result<HttpResponse, LoginError> {
    check_secret(request)?;
    http.execute(request)
}

fn check_secret(request: &HttpRequest) -> Result<(), LoginError> {
    if carries_secret(request) && !is_https(&request.url) {
        tracing::warn!("official login: a credential was not sent over plain HTTP");
        return Err(LoginError::ServiceUnavailable {
            reason: "the service URL for a credential is not HTTPS".to_owned(),
        });
    }
    Ok(())
}

/// The connector's network as the login module's HTTP client, under the rule of
/// [`guarded`], and stopped by the connector's cancel flag: once the backend let go, no
/// further exchange starts (`crate::connector` "Ending a session"), nor once the network
/// right was revoked.
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
            return Err(LoginError::ServiceUnavailable {
                reason: "the login was cancelled".to_owned(),
            });
        }
        check_secret(request)?;
        self.net.http(request)
    }
}

/// The connector's network: the attempt's network-access value, moved in (no copy of it
/// exists; the chat-server binder and the background clients share it, and its
/// revocation ends them all).
#[derive(Debug)]
pub struct LiveNet {
    access: std::sync::Arc<NetworkAccess>,
    http: HttpsClient,
}

impl LiveNet {
    /// The network of one login attempt, through `access`.
    #[must_use]
    pub fn new(access: NetworkAccess) -> Self {
        let http = access.https_client();
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
        Box::new(self.access.https_client())
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

    fn bind_udp(
        &mut self,
        local_ip: Ipv4Addr,
        explicit_base_port: u16,
        compute_unique: bool,
    ) -> Result<StdUdpTransport, SessionError> {
        self.access
            .bind_udp(local_ip, explicit_base_port, compute_unique)
    }

    fn chat_binder(
        &self,
    ) -> Option<Box<dyn FnMut() -> Result<StdUdpTransport, crate::play::PlayError> + Send>> {
        let access = self.access();
        // `0.0.0.0:0`: the chat session's own socket takes no preference
        // (`Net.BindInterface`, `Net.ComputeUniquePort` are not read) and no computed port.
        // Engineering choice, spec silent (question with the spec writers).
        Some(Box::new(move || {
            access
                .bind_udp(Ipv4Addr::UNSPECIFIED, 0, false)
                .map_err(crate::play::PlayError::from)
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
