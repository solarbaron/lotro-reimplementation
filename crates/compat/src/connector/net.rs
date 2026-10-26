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

/// The connector's rule over every HTTP exchange: a request that carries the password or
/// the ticket goes only over `https://` (`official-login.md` §6.2, §6.3: the plaintext
/// password and the bearer ticket must not be exposed). Engineering choice where the spec
/// names an endpoint "HTTP/HTTPS": refused, not sent.
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
/// [`guarded`].
#[derive(Debug)]
pub struct NetHttp<'a, N: ConnectorNet>(pub &'a mut N);

impl<N: ConnectorNet> HttpClient for NetHttp<'_, N> {
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        check_secret(request)?;
        self.0.http(request)
    }
}

/// The connector's network: the run's network-access value.
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
