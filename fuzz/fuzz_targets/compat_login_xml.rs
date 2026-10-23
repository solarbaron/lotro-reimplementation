// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for `tessera_compat::login` (`docs/specs/official-login.md` §2–§7):
//! exercises all XML, SOAP, `.launcherconfig`, `LauncherConfigurationServer`,
//! `UserPreferences.ini`, `StatusServer.aspx`, `LoginQueue.aspx`, and entity/URL
//! encoders and parsers on arbitrary hostile bytes.

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
    let _ = RemoteLauncherConfig::parse(data);
    let _ = UserPreferences::parse(data);
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
        let _ = tessera_compat::login::encode_version_response_payload(text);
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
