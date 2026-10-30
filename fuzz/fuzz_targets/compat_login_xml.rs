// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for `tessera_compat::login` (`docs/specs/official-login.md` §2–§7):
//! exercises all XML, SOAP, `.launcherconfig`, `LauncherConfigurationServer`,
//! `UserPreferences.ini`, the world status and queue responses, and entity/URL
//! encoders and parsers on arbitrary hostile bytes, and the connector's HTTPS form of a
//! service address (`tessera_compat::connector::net::https_form`) and its reading of a
//! world-status answer (`tessera_compat::connector::login_phase::parse_world_status`).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_compat::login::{
    LauncherConfig, LoginFailureClass, QueueResult, RemoteLauncherConfig, UserPreferences,
    WorldStatus, build_leave_queue_body, build_take_a_number_body, form_urlencode,
    parse_get_datacenters_response, parse_hex_u32, parse_login_account_response,
    parse_refresh_ticket_response, xml_escape, xml_unescape,
};

fuzz_target!(|data: &[u8]| {
    let _ = LauncherConfig::parse(data);
    if let Ok(remote) = RemoteLauncherConfig::parse(data) {
        // official-login.md §3.5 item 4, §5.2: the poll interval stays within 5..=120 s.
        assert!((5..=120).contains(&remote.queue_poll_interval_sec()));
    }
    if let Ok(prefs) = UserPreferences::parse(data) {
        // official-session.md §3.2: the [Net] keys reach the bind and the accept rate.
        let _ = prefs.net.bind_address();
        let _ = tessera_compat::session::max_recv_rate_for_speed(prefs.net.connection_speed_kbps);
    }
    let _ = parse_get_datacenters_response(data, "LOTRO");
    if let Ok(status) = WorldStatus::parse(data) {
        let _ = status.login_server();
        let _ = status.queue_url();
        let _ = status.check_billing_role(&["RoleTierVIP".to_owned()]);
    }
    if let Ok(login_res) = parse_login_account_response(200, data) {
        let _ = login_res.matching_subscriptions("LOTRO");
        let _ = format!("{login_res:?}");
    }
    let _ = parse_login_account_response(500, data);
    // The connector's reading of a world-status answer (`connector::login_phase`).
    for status_code in [200, 503] {
        let resp = tessera_compat::login::HttpResponse {
            status_code,
            body: data.to_vec(),
        };
        let _ = tessera_compat::connector::login_phase::parse_world_status(&resp);
    }
    if let Ok(queue_res) = QueueResult::parse(data) {
        let _ = queue_res.evaluate();
    }
    let _ = parse_refresh_ticket_response(data);

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = xml_unescape(text);
        let escaped = xml_escape(text);
        if escaped.len() <= tessera_compat::login::MAX_XML_INPUT_BYTES {
            let roundtrip = xml_unescape(&escaped).expect("xml_escape output must unescape");
            assert_eq!(roundtrip, text);
        }
        let _ = form_urlencode(text);
        let _ = parse_hex_u32(text, LoginFailureClass::F08QueueError);
        let _ = tessera_compat::login::is_valid_network_version(text);
        let _ = tessera_compat::login::encode_client_version_payload(text);
        let _ = tessera_compat::login::LoginHost::parse(text);
        // The HTTPS form of a chain address (`connector::net`): an `https://` URL at the
        // same host, stable when taken again.
        if let Some(h) = tessera_compat::connector::net::https_form(text) {
            let again = tessera_compat::connector::net::https_form(&h.url).expect("stable");
            assert_eq!(again.url, h.url);
            assert_eq!(again.host, h.host);
            assert!(!again.upgraded);
        }
        let _ = tessera_compat::connector::net::scheme_label(text);
        let _ = build_take_a_number_body(
            "command=TakeANumber&subscription={0}&ticket={1}&ticket_type=GLS&queue_url={2}",
            text,
            text,
            text,
        );
        let _ = build_leave_queue_body(
            "command=LeaveQueue&subscription={0}&context={1}&ticket_type=GLS&queue_url={2}",
            text,
            text,
            text,
        );
    }
});
