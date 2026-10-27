// SPDX-License-Identifier: GPL-3.0-or-later
//! The HTTP part of the live login (`official-login.md` §3, §5.1 `S2`–`S9`) and its
//! mapping onto the Account group (`official-entry.md` §8.1, §8.6).
//!
//! Order: `GetDatacenters` (§3.1), the remote launcher configuration (§3.2), `LoginAccount`
//! with the one matching subscription (§3.4), then each world's status for the world list
//! (§3.3, sequential, 10 s each). After `ChooseWorld`: the chosen world's status again, the
//! billing-role check (§3.3), the login queue when the status names one (§3.5), and the
//! hand-off values of §4.2.
//!
//! The account is authenticated before the world list (the progress texts of
//! `official-entry.md` §8.3 follow that order), so a world refusal keeps the login: the
//! ticket stays in memory and another world can be chosen without the password
//! (`official-entry.md` §8.1 OPEN; engineering choice, spec silent (question with the spec
//! writers): the ticket is kept for the session).
//!
//! # Where a secret may go
//!
//! Engineering choice, spec silent (question with the spec writers): every URL carries its
//! provenance ([`Secure`]); it is trusted only when it is `https://`
//! and arrived over HTTPS all the way from the install's configuration (the datacenter
//! service URL is the root; `GetDatacenters` names the authentication server and the remote
//! configuration's URL; the remote configuration names the queue and may override the
//! authentication URL). The password and the ticket are sent only to a trusted URL
//! (`official-login.md` §6.2–§6.3); anything else is refused before a byte is sent.
//!
//! # Upgrade to HTTPS
//!
//! Owner decision 2026-10-27 ("upgrade to HTTPS"; `official-login.md` §2.1 and §3 allow
//! "HTTP/HTTPS" addresses and are silent on it: question with the spec writers): an address
//! the chain gives as `http://` is requested at its HTTPS form ([`https_form`]: same host
//! and path, default HTTPS port), and a document read that way counts as read over HTTPS.
//!
//! - **Steps that carry or lead to the account or the ticket** ([`ChainStep`]): the
//!   datacenter document (it names the authentication server and the remote
//!   configuration), the remote configuration (it names the queue and may override the
//!   refresh URL), `LoginAccount`, the queue and the chosen world's status (it names the
//!   login server that receives the ticket). They are never fetched over plain HTTP: when
//!   the HTTPS request fails (connection, TLS, or an HTTP answer that is not the service's
//!   own), or the address has no HTTPS form, the login stops ([`LoginStop::Insecure`],
//!   `LoginRefusal::InsecureService`) with one `warn` line naming the step and the scheme
//!   ([`InsecureStop::log`]), never a host, a path or the account.
//! - **The world list's status queries** ([`list_world_status`]) carry nothing and lead to
//!   nothing that receives a credential: their answers only colour the list, and joining a
//!   world asks its status again ([`join_status`]). They alone fall back to the plain
//!   `http://` address the chain gave when its HTTPS form fails; a plain answer never gains
//!   HTTPS provenance.

use std::time::Duration;

use tessera_net::account::MAX_ACCOUNT_DETAIL_LEN;
use tessera_net::model::sanitize_text;

use tessera_net::account::{
    LoginRefusal, MAX_WORLD_NAME_LEN, MAX_WORLDS, WorldEntry, WorldId, WorldRefusal, WorldStatus,
};

use crate::credentials::Credentials;
use crate::login::{
    DatacenterInfo, GameSubscription, HttpClient, HttpRequest, HttpResponse, LoginAccountResult,
    LoginError, LoginFailureClass, QueueEvaluation, QueueResult, RemoteLauncherConfig,
    WorldDescriptor, WorldStatus as ServiceStatus, build_launcher_config_request,
    build_leave_queue_request, build_take_a_number_request, build_world_status_request,
    parse_get_datacenters_response, parse_login_account_response, soap_encoder, zeroize_vec,
};

use super::data::LocalInputs;
use super::net::{HttpsUrl, https_form, is_https, scheme_label};

/// The per-world timeout of the world-list status queries (`official-login.md` §3.3, §5.2).
pub const WORLD_LIST_STATUS_TIMEOUT_SEC: u64 = 10;

/// Shortest wait between two `TakeANumber` polls (§3.5 item 4, §5.2).
pub const MIN_QUEUE_POLL: Duration = Duration::from_secs(5);

/// Longest wait between two polls: the configuration's `WorldQueue.PollTimer` is used up to
/// this. Engineering choice, spec silent beyond "between 5 seconds and `PollTimer`"
/// (question with the spec writers).
pub const MAX_QUEUE_POLL: Duration = Duration::from_secs(120);

/// Tessera's category for a login failure (`official-entry.md` §8.6), from the classes of
/// `official-login.md` §5.3. Engineering choice, spec silent (question with the spec
/// writers; §8.6 leaves the mapping OPEN):
///
/// | Class | Category |
/// |---|---|
/// | `F02` local configuration | `ClientNotAccepted` (see [`local_refusal`]) |
/// | `F03` service unavailable (maintenance included) | `ServiceUnavailable` |
/// | `F04` rejected, account locked | `AccountNotAllowed` |
/// | `F04` rejected, any other fault | `BadCredentials` |
/// | `F05` no subscription for the game | `AccountNotAllowed` |
/// | `F06`–`F10` | `Other` (world failures are `WorldRefused`, [`world_refusal`]) |
#[must_use]
pub fn login_refusal(err: &LoginError) -> LoginRefusal {
    match err.failure_class() {
        LoginFailureClass::F02ConfigInvalid => local_refusal(),
        LoginFailureClass::F03ServiceUnavailable => LoginRefusal::ServiceUnavailable,
        LoginFailureClass::F04AuthRejected if err.is_account_locked() => {
            LoginRefusal::AccountNotAllowed
        }
        LoginFailureClass::F04AuthRejected => LoginRefusal::BadCredentials,
        LoginFailureClass::F05NoGameSubscription => LoginRefusal::AccountNotAllowed,
        _ => LoginRefusal::Other,
    }
}

/// The category for a missing install or a failed local discovery (`F02`, before any
/// network operation). `official-entry.md` §8.6 has no row for it; it is not a refusal of
/// the credentials, and the account form cannot fix it. The closest neutral category whose
/// row returns to the start screen is `ClientNotAccepted` ("did not accept this client":
/// this client cannot be set up from the install). Engineering choice, spec silent
/// (question with the spec writers).
#[must_use]
pub const fn local_refusal() -> LoginRefusal {
    LoginRefusal::ClientNotAccepted
}

/// The service's own text of a failure, cleaned, for `LoginRefused.detail`
/// (`official-entry.md` §8.6 "The service said: ⟨text⟩"): the `faultstring` of a SOAP fault.
/// Not for `BadCredentials` (the row shows no detail) and not for Tessera's own texts.
#[must_use]
pub fn service_detail(err: &LoginError, reason: LoginRefusal) -> Option<String> {
    if matches!(
        reason,
        LoginRefusal::BadCredentials | LoginRefusal::NotAvailable
    ) {
        return None;
    }
    let LoginError::AuthRejected { fault_string, .. } = err else {
        return None;
    };
    let clean = sanitize_text(fault_string, MAX_ACCOUNT_DETAIL_LEN);
    (!clean.trim().is_empty()).then(|| clean.into_owned())
}

/// The world refusal of a failure after `ChooseWorld` (`official-entry.md` §8.6): `F06`
/// closed, `F07` not allowed, anything else `Other`. Engineering choice, spec silent
/// (question with the spec writers; §8.6 leaves the mapping OPEN).
#[must_use]
pub fn world_refusal(err: &LoginError) -> WorldRefusal {
    match err.failure_class() {
        LoginFailureClass::F06WorldClosed => WorldRefusal::Closed,
        LoginFailureClass::F07WorldRoleDenied => WorldRefusal::NotAllowed,
        _ => WorldRefusal::Other,
    }
}

/// Why the login stopped before the world list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginStop {
    /// A refusal for the user, with the service's text when there is one.
    Refused(LoginRefusal, Option<String>),
    /// A step that carries or leads to a credential could not be reached over HTTPS
    /// (module docs "Upgrade to HTTPS"): `LoginRefusal::InsecureService`, logged by
    /// [`InsecureStop::log`] when it is reported.
    Insecure(InsecureStop),
    /// The backend let go; nothing more is reported.
    Cancelled,
}

/// A step of the login chain that carries or leads to the account or the ticket (module
/// docs "Upgrade to HTTPS").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChainStep {
    /// `Launcher.DataCenterService.GLS`, `GetDatacenters` (§3.1).
    DatacenterDocument,
    /// `Datacenter/LauncherConfigurationServer`, the remote configuration (§3.2).
    LauncherConfiguration,
    /// `Datacenter/AuthServer`, `LoginAccount` (§3.4).
    AuthServer,
    /// `WorldQueue.LoginQueue.URL`, `TakeANumber` (§3.5).
    QueueUrl,
    /// The chosen world's `StatusServerUrl` (§3.3).
    StatusUrl,
}

impl ChainStep {
    /// The step's name in a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DatacenterDocument => "datacenter document",
            Self::LauncherConfiguration => "launcher configuration",
            Self::AuthServer => "auth server",
            Self::QueueUrl => "queue URL",
            Self::StatusUrl => "status URL",
        }
    }
}

/// Why a step was not reached over HTTPS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InsecureCause {
    /// The address has no HTTPS form ([`https_form`]).
    NoHttpsForm,
    /// The address was `http://` and the request at its HTTPS form failed.
    HttpsFailed,
}

impl InsecureCause {
    /// The cause's name in a log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NoHttpsForm => "the address has no HTTPS form",
            Self::HttpsFailed => "the HTTPS request failed",
        }
    }
}

/// A login stopped by the HTTPS rule: the step, the scheme the chain gave it (only
/// `"http"`, `"https"`, `"other"` or `"none"`, [`scheme_label`]) and the cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InsecureStop {
    /// The step.
    pub step: ChainStep,
    /// The scheme of the address the chain gave.
    pub scheme: &'static str,
    /// Why.
    pub cause: InsecureCause,
}

impl InsecureStop {
    /// The step `step` whose address was `url` failed for `cause`.
    #[must_use]
    pub fn new(step: ChainStep, url: &str, cause: InsecureCause) -> Self {
        Self {
            step,
            scheme: scheme_label(url),
            cause,
        }
    }

    /// An `http://` address of `step` whose HTTPS form failed.
    #[must_use]
    pub const fn https_failed(step: ChainStep) -> Self {
        Self {
            step,
            scheme: "http",
            cause: InsecureCause::HttpsFailed,
        }
    }

    /// The one `warn` line of a login this rule stopped: the step, the scheme and the
    /// cause; never a host, a path or the account.
    pub fn log(&self) {
        tracing::warn!(
            step = self.step.label(),
            scheme = self.scheme,
            cause = self.cause.label(),
            "official login stopped: the service address was not secure"
        );
    }
}

/// A failure of a step after `ChooseWorld`.
#[derive(Debug)]
pub enum JoinError {
    /// The service's failure: the world is refused, the login stays.
    Service(LoginError),
    /// The HTTPS rule stopped the login ([`LoginStop::Insecure`]).
    Insecure(InsecureStop),
}

impl From<LoginError> for JoinError {
    fn from(e: LoginError) -> Self {
        Self::Service(e)
    }
}

impl From<&LoginError> for LoginStop {
    fn from(e: &LoginError) -> Self {
        // The class only: a local error's text can name a path of the user.
        tracing::info!(class = %e.failure_class(), "official login failed");
        let reason = login_refusal(e);
        Self::Refused(reason, service_detail(e, reason))
    }
}

/// Which of the authenticated login's URLs may receive the password or the ticket (module
/// docs "Where a secret may go").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Secure {
    /// `GetDatacenters` arrived over HTTPS.
    pub datacenters: bool,
    /// The remote configuration arrived over HTTPS through a trusted URL.
    pub remote: bool,
    /// The `LoginAccount` URL (`Datacenter/AuthServer`).
    pub auth: bool,
    /// The queue URL (`WorldQueue.LoginQueue.URL`).
    pub queue: bool,
    /// The in-game refresh URL (the remote override, else the authentication server).
    pub refresh: bool,
    /// Which of the authentication and queue URLs the chain gave as `http://` (they are
    /// requested at their HTTPS form, and a failure there is [`LoginStop::Insecure`]).
    pub upgraded: Upgraded,
}

/// Which credential URLs were upgraded from `http://` ([`upgrade_datacenter`],
/// [`upgrade_remote`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Upgraded {
    /// `Datacenter/AuthServer`.
    pub auth: bool,
    /// `Datacenter/LauncherConfigurationServer`.
    pub launcher: bool,
    /// `WorldQueue.LoginQueue.URL`.
    pub queue: bool,
}

/// Puts `url` in its HTTPS form in place when it has one and was `http://`; whether it was
/// upgraded. An address without an HTTPS form is left as it is (it is not trusted, so the
/// step that would use it stops).
fn upgrade_in_place(url: &mut String) -> bool {
    match https_form(url) {
        Some(HttpsUrl {
            url: https,
            upgraded: true,
        }) => {
            *url = https;
            true
        }
        _ => false,
    }
}

/// The datacenter's authentication and remote-configuration URLs in their HTTPS form
/// (module docs "Upgrade to HTTPS"), for a document read over HTTPS. The worlds' status
/// URLs are left as given: the world list may fall back to them ([`list_world_status`]).
pub fn upgrade_datacenter(dc: &mut DatacenterInfo, upgraded: &mut Upgraded) {
    upgraded.auth = upgrade_in_place(&mut dc.auth_server);
    upgraded.launcher = upgrade_in_place(&mut dc.launcher_configuration_server);
}

/// The remote configuration's queue URL and authentication override in their HTTPS form,
/// for a document read over HTTPS.
pub fn upgrade_remote(remote: &mut RemoteLauncherConfig, upgraded: &mut Upgraded) {
    upgraded.queue = upgrade_in_place(&mut remote.login_queue_url);
    if let Some(url) = remote.auth_server_url_override.as_mut() {
        upgrade_in_place(url);
    }
}

impl Secure {
    /// The provenance of the URLs named by `install_url`'s answer `dc` and by `remote`.
    #[must_use]
    pub fn of(install_url: &str, dc: &DatacenterInfo, remote: &RemoteLauncherConfig) -> Self {
        let datacenters = is_https(install_url);
        let remote_ok = datacenters && is_https(&dc.launcher_configuration_server);
        let auth = datacenters && is_https(&dc.auth_server);
        let refresh = match remote.auth_server_url_override.as_deref() {
            Some(url) => remote_ok && is_https(url),
            None => auth,
        };
        Self {
            datacenters,
            remote: remote_ok,
            auth,
            queue: remote_ok && is_https(&remote.login_queue_url),
            refresh,
            upgraded: Upgraded::default(),
        }
    }
}

/// The authenticated login: what the rest of the session needs from the HTTP exchanges.
/// The ticket (in [`LoginAccountResult`]) and the subscription name are wiped on drop and
/// never logged (`official-login.md` §6.3).
#[derive(Debug)]
pub struct Authenticated {
    /// `GetDatacenters`' datacenter (§3.1).
    pub datacenter: DatacenterInfo,
    /// The remote launcher configuration (§3.2).
    pub remote: RemoteLauncherConfig,
    /// The selected subscription (§3.4).
    pub subscription: GameSubscription,
    /// The ticket.
    pub account: LoginAccountResult,
    /// Where the ticket may go.
    pub secure: Secure,
}

fn checked_200(what: &str, status: u16) -> Result<(), LoginError> {
    if status == 200 {
        Ok(())
    } else {
        Err(LoginError::ServiceUnavailable {
            reason: format!("{what} returned HTTP {status}"),
        })
    }
}

/// One exchange of a step whose document leads to a credential, at `at` (an HTTPS form):
/// the 200 answer. A failure at an upgraded address is [`LoginStop::Insecure`] (no
/// plain-HTTP retry), otherwise the service's failure of class `F03`.
fn fetch_over_https(
    http: &mut dyn HttpClient,
    step: ChainStep,
    what: &str,
    at: &HttpsUrl,
    req: &HttpRequest,
) -> Result<HttpResponse, LoginStop> {
    let failed = |e: &LoginError| {
        if at.upgraded {
            LoginStop::Insecure(InsecureStop::https_failed(step))
        } else {
            LoginStop::from(e)
        }
    };
    let resp = http.execute(req).map_err(|e| failed(&e))?;
    checked_200(what, resp.status_code).map_err(|e| failed(&e))?;
    Ok(resp)
}

/// `S2_GET_DATACENTERS` (§3.1) at the HTTPS form of the install's datacenter URL (module
/// docs "Upgrade to HTTPS"): the datacenter, with its authentication and
/// remote-configuration URLs in their HTTPS form ([`upgrade_datacenter`]), and the URL the
/// document was read from.
///
/// # Errors
///
/// [`LoginStop::Insecure`] when the URL has no HTTPS form or its upgraded form failed;
/// otherwise the refusal of class `F03` for a failed exchange or a missing datacenter.
pub fn get_datacenters(
    http: &mut dyn HttpClient,
    local: &LocalInputs,
    upgraded: &mut Upgraded,
) -> Result<(DatacenterInfo, HttpsUrl), LoginStop> {
    let given = &local.config.gls_datacenter_service;
    let Some(at) = https_form(given) else {
        return Err(LoginStop::Insecure(InsecureStop::new(
            ChainStep::DatacenterDocument,
            given,
            InsecureCause::NoHttpsForm,
        )));
    };
    let req =
        soap_encoder::build_get_datacenters_request(&at.url, &local.config.datacenter_game_name);
    let resp = fetch_over_https(
        http,
        ChainStep::DatacenterDocument,
        "GetDatacenters",
        &at,
        &req,
    )?;
    let mut dc = parse_get_datacenters_response(&resp.body, local.config.target_datacenter())
        .map_err(|e| LoginStop::from(&e))?;
    upgrade_datacenter(&mut dc, upgraded);
    Ok((dc, at))
}

/// `S3_GET_LAUNCHER_CONFIG` (§3.2) at the datacenter's remote-configuration URL (already in
/// its HTTPS form, [`get_datacenters`]): the configuration with its queue URL and
/// authentication override in their HTTPS form ([`upgrade_remote`]).
///
/// # Errors
///
/// [`LoginStop::Insecure`] when the URL has no HTTPS form or its upgraded form failed;
/// otherwise the refusal of class `F03` for a failed exchange.
pub fn get_remote_config(
    http: &mut dyn HttpClient,
    dc: &DatacenterInfo,
    upgraded: &mut Upgraded,
) -> Result<RemoteLauncherConfig, LoginStop> {
    let given = &dc.launcher_configuration_server;
    let at = match https_form(given) {
        Some(at) => HttpsUrl {
            upgraded: upgraded.launcher || at.upgraded,
            ..at
        },
        None => {
            return Err(LoginStop::Insecure(InsecureStop::new(
                ChainStep::LauncherConfiguration,
                given,
                InsecureCause::NoHttpsForm,
            )));
        }
    };
    let req = build_launcher_config_request(&at.url);
    let resp = fetch_over_https(
        http,
        ChainStep::LauncherConfiguration,
        "LauncherConfigurationServer",
        &at,
        &req,
    )?;
    let mut remote = RemoteLauncherConfig::parse(&resp.body).map_err(|e| LoginStop::from(&e))?;
    upgrade_remote(&mut remote, upgraded);
    Ok(remote)
}

/// `S5_LOGIN_ACCOUNT` (§3.4): the one use of the credentials (`official-entry.md` §7.3).
/// The request body is wiped right after the exchange and the credentials are dropped
/// (and wiped) before this returns (§6.3); so is the response, which holds the ticket.
///
/// The password goes only to a trusted `LoginAccount` URL (module docs); otherwise the
/// login stops as [`LoginStop::Insecure`] and nothing is sent. At an address the chain gave
/// as `http://` (requested at its HTTPS form), a failed request or an answer that is
/// neither 200, the maintenance answer (503) nor a SOAP fault stops it the same way; the
/// password is never sent over plain HTTP.
///
/// A login with more than one subscription for the game needs a choice the account form
/// does not offer (§3.4 "Multiple matching subscriptions"): it is refused as
/// `UnsupportedStep` (`official-entry.md` §7.3 OPEN: no workaround).
///
/// # Errors
///
/// [`LoginStop`] with the §8.6 category.
pub fn authenticate(
    http: &mut dyn HttpClient,
    local: &LocalInputs,
    datacenter: DatacenterInfo,
    remote: RemoteLauncherConfig,
    secure: Secure,
    credentials: Credentials,
) -> Result<Authenticated, LoginStop> {
    if !secure.auth {
        drop(credentials);
        let stop = if secure.datacenters {
            InsecureStop::new(
                ChainStep::AuthServer,
                &datacenter.auth_server,
                InsecureCause::NoHttpsForm,
            )
        } else {
            InsecureStop::new(
                ChainStep::DatacenterDocument,
                &local.config.gls_datacenter_service,
                InsecureCause::NoHttpsForm,
            )
        };
        return Err(LoginStop::Insecure(stop));
    }
    let upgraded = secure.upgraded.auth;
    let built = credentials.expose().map(|(account, password)| {
        soap_encoder::build_login_account_request(&datacenter.auth_server, account, password)
    });
    drop(credentials);
    let Some(mut req) = built else {
        return Err(LoginStop::Refused(LoginRefusal::Other, None));
    };
    let result = http.execute(&req);
    req.zeroize_body();
    drop(req);
    let mut resp = result.map_err(|e| {
        if upgraded {
            LoginStop::Insecure(InsecureStop::https_failed(ChainStep::AuthServer))
        } else {
            LoginStop::from(&e)
        }
    })?;
    let status = resp.status_code;
    let parsed = parse_login_account_response(status, &resp.body);
    zeroize_vec(&mut resp.body);
    let account = parsed.map_err(|e| {
        let http_error = status != 200
            && status != 503
            && e.failure_class() == LoginFailureClass::F03ServiceUnavailable;
        if upgraded && http_error {
            LoginStop::Insecure(InsecureStop::https_failed(ChainStep::AuthServer))
        } else {
            LoginStop::from(&e)
        }
    })?;
    let mut matching = account
        .matching_subscriptions(&local.config.datacenter_game_name)
        .map_err(|e| LoginStop::from(&e))?;
    if matching.len() != 1 {
        tracing::info!(
            subscriptions = matching.len(),
            "official login: more than one subscription; Tessera has no choice for it"
        );
        return Err(LoginStop::Refused(LoginRefusal::UnsupportedStep, None));
    }
    let Some(subscription) = matching.pop() else {
        return Err(LoginStop::Refused(LoginRefusal::AccountNotAllowed, None));
    };
    if account.ticket().is_empty() {
        return Err(LoginStop::Refused(LoginRefusal::Other, None));
    }
    Ok(Authenticated {
        datacenter,
        remote,
        subscription,
        account,
        secure,
    })
}

/// Exchange 3 (§3.3) at `url` with the given timeout: the world's own `StatusServerUrl`
/// (or its HTTPS form) only, no fallback URL (decision 0042 item 3).
///
/// # Errors
///
/// [`LoginError`] of class `F06` when the URL is empty, the exchange fails or the status
/// has no login server.
pub fn world_status(
    http: &mut dyn HttpClient,
    url: &str,
    timeout_sec: u64,
) -> Result<ServiceStatus, LoginError> {
    if url.trim().is_empty() {
        return Err(LoginError::WorldClosed {
            reason: "the world has no status URL".to_owned(),
        });
    }
    let mut req = build_world_status_request(url);
    req.timeout_sec = timeout_sec;
    let resp = http.execute(&req).map_err(|e| LoginError::WorldClosed {
        reason: e.to_string(),
    })?;
    if resp.status_code != 200 || resp.body.iter().all(u8::is_ascii_whitespace) {
        return Err(LoginError::WorldClosed {
            reason: format!("the status query returned HTTP {}", resp.status_code),
        });
    }
    ServiceStatus::parse(&resp.body)
}

/// A world's status for the world list (§3.3, 10 s): at the HTTPS form of its
/// `StatusServerUrl` first. Only for the list (module docs "Upgrade to HTTPS"): the answer
/// colours the list and nothing is taken from it to send a credential to (joining asks
/// again, [`join_status`]), so when the HTTPS form fails, an `http://` address is asked once
/// more as given, over plain HTTP. An address without an HTTPS form is asked as given.
///
/// # Errors
///
/// [`LoginError`] of class `F06`, as [`world_status`].
pub fn list_world_status(
    http: &mut dyn HttpClient,
    world: &WorldDescriptor,
    timeout_sec: u64,
) -> Result<ServiceStatus, LoginError> {
    let given = world.status_server_url.as_str();
    match https_form(given) {
        Some(at) if at.upgraded => world_status(http, &at.url, timeout_sec).or_else(|_| {
            tracing::debug!("official login: a world-list status asked over plain HTTP");
            world_status(http, given, timeout_sec)
        }),
        Some(at) => world_status(http, &at.url, timeout_sec),
        None => world_status(http, given, timeout_sec),
    }
}

/// A world name as the Account group carries it: control characters removed, cut on a
/// character boundary at 64 bytes; `None` when nothing is left.
#[must_use]
pub fn world_name(name: &str) -> Option<String> {
    let clean = crate::play::clean_display_name(name, MAX_WORLD_NAME_LEN);
    (!clean.trim().is_empty()).then_some(clean)
}

/// The list status of a world from its status query (`official-entry.md` §8.1 OPEN;
/// engineering choice, spec silent (question with the spec writers), from the fields of
/// `official-login.md` §3.3):
///
/// - the query failed or gave no login server: `Unknown` (a world list does not call a
///   world closed on one failed query);
/// - the subscription fails the billing-role check: `Restricted`;
/// - `world_full` is `true`: `Full`;
/// - the last assigned queue number is above the one being served: `Busy`;
/// - otherwise `Open`.
#[must_use]
pub fn list_status(
    status: &Result<ServiceStatus, LoginError>,
    sub: &GameSubscription,
) -> WorldStatus {
    let Ok(s) = status else {
        return WorldStatus::Unknown;
    };
    if s.check_billing_role(&sub.product_tokens).is_err() {
        WorldStatus::Restricted
    } else if s.world_full == Some(true) {
        WorldStatus::Full
    } else if matches!(
        (s.last_assigned_queue_number, s.now_serving_queue_number),
        (Some(last), Some(now)) if last > now
    ) {
        WorldStatus::Busy
    } else {
        WorldStatus::Open
    }
}

/// The world list (§3.1 order, `official-entry.md` §8.1): one entry per world with a usable
/// name, at most 256, numbered by their place in the datacenter's list. `query` gives each
/// world's status (and is where the caller keeps the watchdog alive); it returns `None`
/// when the attempt was cancelled, and the list stops there (`None`).
#[must_use]
pub fn world_list(
    worlds: &[WorldDescriptor],
    sub: &GameSubscription,
    mut query: impl FnMut(&WorldDescriptor) -> Option<Result<ServiceStatus, LoginError>>,
) -> Option<Vec<(usize, WorldEntry)>> {
    let mut out = Vec::new();
    for (i, w) in worlds.iter().enumerate() {
        if out.len() >= MAX_WORLDS {
            break;
        }
        let Some(name) = world_name(&w.name) else {
            continue;
        };
        let Ok(id) = u32::try_from(i) else {
            break;
        };
        let status = query(w)?;
        out.push((
            i,
            WorldEntry {
                id: WorldId(id),
                name,
                status: list_status(&status, sub),
                load: None,
                note: None,
            },
        ));
    }
    Some(out)
}

/// The status of the chosen world for joining it (§3.3, §5.1 `S7`): the query with the
/// 30 s timeout and the billing-role check. The answer names the login server that
/// receives the ticket, so it is read over HTTPS only (module docs "Upgrade to HTTPS").
///
/// # Errors
///
/// [`JoinError::Service`] of class `F06` or `F07`; [`JoinError::Insecure`] when the
/// address has no HTTPS form, or was `http://` and its HTTPS form failed.
pub fn join_status(
    http: &mut dyn HttpClient,
    world: &WorldDescriptor,
    sub: &GameSubscription,
) -> Result<ServiceStatus, JoinError> {
    let given = world.status_server_url.as_str();
    if given.trim().is_empty() {
        return Err(JoinError::Service(LoginError::WorldClosed {
            reason: "the world has no status URL".to_owned(),
        }));
    }
    let Some(at) = https_form(given) else {
        return Err(JoinError::Insecure(InsecureStop::new(
            ChainStep::StatusUrl,
            given,
            InsecureCause::NoHttpsForm,
        )));
    };
    let status =
        world_status(http, &at.url, crate::login::DEFAULT_HTTP_TIMEOUT_SEC).map_err(|e| {
            if at.upgraded {
                JoinError::Insecure(InsecureStop::https_failed(ChainStep::StatusUrl))
            } else {
                JoinError::Service(e)
            }
        })?;
    status.check_billing_role(&sub.product_tokens)?;
    Ok(status)
}

/// One `TakeANumber` exchange (§3.5). The ticket goes only to a trusted queue URL; at an
/// address the chain gave as `http://` (requested at its HTTPS form) a failed request or
/// a non-200 answer stops the login (module docs "Upgrade to HTTPS").
///
/// # Errors
///
/// [`JoinError::Service`] of class `F08` on a failed exchange or a negative `HResult`;
/// [`JoinError::Insecure`] for an untrusted queue URL or a failed upgraded one.
pub fn take_a_number(
    http: &mut dyn HttpClient,
    auth: &Authenticated,
    queue_url: &str,
) -> Result<QueueEvaluation, JoinError> {
    if !auth.secure.queue {
        let stop = if auth.secure.remote {
            InsecureStop::new(
                ChainStep::QueueUrl,
                &auth.remote.login_queue_url,
                InsecureCause::NoHttpsForm,
            )
        } else {
            InsecureStop::new(
                ChainStep::LauncherConfiguration,
                &auth.datacenter.launcher_configuration_server,
                InsecureCause::NoHttpsForm,
            )
        };
        return Err(JoinError::Insecure(stop));
    }
    let upgraded = auth.secure.upgraded.queue;
    let insecure = || JoinError::Insecure(InsecureStop::https_failed(ChainStep::QueueUrl));
    let mut req = build_take_a_number_request(
        &auth.remote.login_queue_url,
        &auth.remote.take_a_number_parameters,
        auth.subscription.name(),
        auth.account.ticket(),
        queue_url,
    );
    let result = http.execute(&req);
    req.zeroize_body();
    let mut resp = match result {
        Ok(r) => r,
        Err(_) if upgraded => return Err(insecure()),
        Err(e) => {
            return Err(JoinError::Service(LoginError::QueueError {
                hresult: None,
                reason: e.to_string(),
            }));
        }
    };
    if resp.status_code != 200 {
        zeroize_vec(&mut resp.body);
        if upgraded {
            return Err(insecure());
        }
        return Err(JoinError::Service(LoginError::QueueError {
            hresult: None,
            reason: format!("the queue returned HTTP {}", resp.status_code),
        }));
    }
    let parsed = QueueResult::parse(&resp.body);
    zeroize_vec(&mut resp.body);
    Ok(parsed?.evaluate()?)
}

/// `LeaveQueue` (§3.5) when the configuration has its template and a `ContextNumber` is
/// known; the answer is not needed (the user is leaving).
pub fn leave_queue(
    http: &mut dyn HttpClient,
    auth: &Authenticated,
    queue_url: &str,
    context: Option<&str>,
) {
    let (Some(template), Some(ctx)) = (auth.remote.leave_queue_parameters.as_deref(), context)
    else {
        return;
    };
    if !auth.secure.queue {
        return;
    }
    let mut req = build_leave_queue_request(
        &auth.remote.login_queue_url,
        template,
        auth.subscription.name(),
        ctx,
        queue_url,
    );
    let result = http.execute(&req);
    req.zeroize_body();
    if let Err(e) = result {
        tracing::debug!(class = %e.failure_class(), "official login: LeaveQueue failed");
    }
}

/// The wait between two `TakeANumber` polls: the configuration's `WorldQueue.PollTimer`,
/// at least [`MIN_QUEUE_POLL`] and at most [`MAX_QUEUE_POLL`] (§3.5 item 4, §5.2).
#[must_use]
pub fn queue_poll_interval(remote: &RemoteLauncherConfig) -> Duration {
    remote
        .poll_timer_sec
        .map_or(MIN_QUEUE_POLL, Duration::from_secs)
        .clamp(MIN_QUEUE_POLL, MAX_QUEUE_POLL)
}
