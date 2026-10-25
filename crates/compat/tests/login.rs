// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic integration tests for `docs/specs/official-login.md` §2–§7
//! covering invariants `I2`–`I16`, synthetic vectors `V1`–`V7`, the
//! end-to-end state machine `S1`–`S10`, ticket intake, and the `TESSERA_GAME_DIR`
//! read-only local configuration check.

use std::collections::VecDeque;
use std::path::Path;

use tessera_compat::login::soap_encoder::{
    SOAP_ACTION_GET_DATACENTERS, SOAP_ACTION_REFRESH_TICKET, build_get_datacenters_request,
    build_login_account_request,
};
use tessera_compat::login::{
    ArgTemplateParams, BitTreeTableHeader, Credentials, DAT_DIDVER_DESCRIPTOR_OFFSET,
    DAT_ROOT_DID_MAP_OFFSET, DAT_SUPERBLOCK_MAGIC, DAT_SUPERBLOCK_OFFSET,
    DEFAULT_TICKET_LIFETIME_SEC, DICT_KEY_CLIENT_TO_SERVER, DICT_KEY_SERVER_TO_CLIENT,
    DICT_KEY_SERVER_TO_CLIENT_DEFERRED, DidMap3Table, GameSubscription, HttpClient, HttpMethod,
    HttpRequest, HttpResponse, INVALID_LOGIN_SERVER_SENTINEL, LauncherConfig, LoginDriver,
    LoginError, LoginFailureClass, LoginHost, LoginStateId, MAX_XML_DEPTH, MAX_XML_INPUT_BYTES,
    MAX_XML_ITEMS, PREFIX_TREE_DICT_SCHEMA_GUID, QueueEvaluation, QueueResult,
    ROOT_KEY_COMPRESSION_DICTIONARY, RefreshOutcome, RegistryTicketReader, RemoteLauncherConfig,
    SessionHandoff, TicketIntakeArgs, TicketRefresher, UserPreferences, VersionInfo, WorldStatus,
    build_launch_argv, build_leave_queue_body, build_take_a_number_body, check_billing_role,
    compression_dictionary_map_did, encode_client_version_payload, extract_didver_from_dat_file,
    extract_didver_from_dat_header, fetch_world_status, is_rfc1918_queue_url,
    language_to_feed_code, parse_dat_superblock_didver_descriptor, parse_get_datacenters_response,
    parse_login_account_response, resolve_bit_tree_table, resolve_install_language,
    resolve_ticket_intake, root_did_map_from_superblock, split_arg_template_argv,
    substitute_arg_template,
};
use tessera_compat::version_resource::read_version_resource_from_file;

type RecordedRequest = (String, HttpMethod, Vec<(String, String)>, Vec<u8>);

#[derive(Default)]
struct MockHttpClient {
    requests: Vec<RecordedRequest>,
    responses: VecDeque<Result<HttpResponse, LoginError>>,
}

impl MockHttpClient {
    fn push_ok(&mut self, status_code: u16, body: impl Into<Vec<u8>>) {
        self.responses.push_back(Ok(HttpResponse {
            status_code,
            body: body.into(),
        }));
    }
}

impl HttpClient for MockHttpClient {
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        assert!(
            request.require_tls_validation,
            "every request must enforce TLS certificate and hostname validation (§6.2)"
        );
        self.requests.push((
            request.url.clone(),
            request.method,
            request.headers.clone(),
            request.body.clone(),
        ));
        self.responses.pop_front().unwrap_or_else(|| {
            Err(LoginError::ServiceUnavailable {
                reason: "mock queue empty".to_owned(),
            })
        })
    }
}

fn write_synthetic_install(root: &Path, with_bom: bool) {
    let mut bytes = Vec::new();
    if with_bom {
        bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    bytes.extend_from_slice(
        br#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <appSettings>
    <add key="Launcher.DataCenterService.GLS" value="https://gls.example.invalid/service/datacenter" />
    <add key="DataCenter.GameName" value="LOTRO" />
    <add key="DataCenter.DefaultCenter" value="LOTRO" />
    <add key="Product.DocumentFolder" value="The Lord of the Rings Online" />
    <add key="Launcher.DocumentFolder" value="Launcher" />
  </appSettings>
</configuration>
"#,
    );
    std::fs::write(root.join("lotro.launcherconfig"), bytes).unwrap();
    std::fs::write(root.join("client_local_English.dat"), b"SYNTH").unwrap();
}

const V2_DATACENTERS_RESPONSE_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
  <soap:Body>
    <GetDatacentersResponse xmlns="http://www.turbine.com/SE/GLS">
      <GetDatacentersResult>
        <Datacenter>
          <Name>LOTRO</Name>
          <AuthServer>https://gls-auth.example.invalid/service/auth</AuthServer>
          <PatchServer>patch.example.invalid:6015</PatchServer>
          <LauncherConfigurationServer>https://gls.example.invalid/Launcher/lotrolauncher.server.config.xml</LauncherConfigurationServer>
          <Worlds>
            <World>
              <Name>WorldAlpha [OLD]</Name>
              <LoginServerUrl>(invalid service specified)</LoginServerUrl>
              <ChatServerUrl>chat.example.invalid:2900</ChatServerUrl>
              <StatusServerUrl>http://legacy-status.example.invalid/service/status?s=shard-alpha</StatusServerUrl>
              <Order>2</Order>
            </World>
            <World>
              <Name>[EN] WorldBeta</Name>
              <LoginServerUrl>(invalid service specified)</LoginServerUrl>
              <ChatServerUrl>chat.example.invalid:2900</ChatServerUrl>
              <StatusServerUrl>https://gls.example.invalid/service/status?s=shard-beta</StatusServerUrl>
              <Order>1</Order>
            </World>
          </Worlds>
        </Datacenter>
      </GetDatacentersResult>
    </GetDatacentersResponse>
  </soap:Body>
</soap:Envelope>"#;

const SYNTHETIC_REMOTE_LAUNCHER_CONFIG_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <appSettings>
    <add key="GameClient.WIN64.Filename" value="x64\lotroclient64.exe" />
    <add key="GameClient.WIN32.ArgTemplate" value="-a {SUBSCRIPTION} -h {LOGIN} --glsticketdirect &quot;{GLS}&quot; --chatserver &quot;{CHAT}&quot; --language {LANG} --rodat on --gametype {GAMETYPE} --authserverurl &quot;{AUTHSERVERURL}&quot; --glsticketlifetime {GLSTICKETLIFETIME} --supporturl &quot;{SUPPORTURL}&quot; --supportserviceurl &quot;{SUPPORTSERVICEURL}&quot; --bugurl &quot;{BUGURL}&quot;" />
    <add key="GameClient.Arg.glsticketlifetime" value="21600" />
    <add key="GameClient.Arg.supporturl" value="https://support.example.invalid" />
    <add key="GameClient.Arg.supportserviceurl" value="https://support-svc.example.invalid/service/support" />
    <add key="GameClient.Arg.bugurl" value="https://bugs.example.invalid" />
    <add key="GameClient.HighResPatchArg" value=" --HighResOutOfDate" />
    <add key="WorldQueue.LoginQueue.URL" value="https://gls-auth.example.invalid/service/queue" />
    <add key="WorldQueue.TakeANumber.Parameters" value="command=TakeANumber&amp;subscription={0}&amp;ticket={1}&amp;ticket_type=GLS&amp;queue_url={2}" />
    <add key="WorldQueue.LeaveQueue.Parameters" value="command=LeaveQueue&amp;subscription={0}&amp;context={1}&amp;ticket_type=GLS&amp;queue_url={2}" />
    <add key="WorldQueue.PollTimer" value="120" />
  </appSettings>
</configuration>"#;

const V3_STATUS_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Status>
  <name>WorldAlpha</name>
  <loginservers>login-a.example.invalid:9001;login-b.example.invalid:9002;</loginservers>
  <queueurls>http://queue-a.example.invalid:7080/command;http://queue-b.example.invalid:7080/command;</queueurls>
  <nowservingqueuenumber>0x00000010</nowservingqueuenumber>
  <lastassignedqueuenumber>0x00000015</lastassignedqueuenumber>
  <allow_billing_role>RoleTierVIP</allow_billing_role>
  <deny_billing_role></deny_billing_role>
</Status>"#;

const V4_LOGIN_ACCOUNT_SUCCESS_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <LoginAccountResponse xmlns="http://www.turbine.com/SE/GLS">
      <LoginAccountResult>
        <Subscriptions>
          <GameSubscription>
            <Game>DDO</Game>
            <Name>DDOSUB0000000000000000001</Name>
            <Description>DDO Subscription</Description>
          </GameSubscription>
          <GameSubscription>
            <Game>LOTRO</Game>
            <Name>LOTROSUB00000000000000002</Name>
            <Description>LOTRO Primary</Description>
            <ProductTokens>
              <string>TokenBase</string>
              <string>RoleTierVIP</string>
            </ProductTokens>
          </GameSubscription>
        </Subscriptions>
        <Ticket>SYNTH+TICKET/ABC123==</Ticket>
      </LoginAccountResult>
    </LoginAccountResponse>
  </soap:Body>
</soap:Envelope>"#;

const V4_LOGIN_ACCOUNT_FAULT_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <soap:Fault>
      <faultcode>BIS</faultcode>
      <faultstring>Authentication rejected</faultstring>
      <detail>
        <result>0x8004C002</result>
      </detail>
    </soap:Fault>
  </soap:Body>
</soap:Envelope>"#;

// ============================================================================
// Synthetic Test Vectors V1–V6 (docs/specs/official-login.md §7.2)
// ============================================================================

/// Vector `V1` — Local `lotro.launcherconfig` with UTF-8 BOM (`EF BB BF`).
#[test]
fn v1_local_launcherconfig_with_utf8_bom() {
    let dir = tempfile::tempdir().unwrap();
    write_synthetic_install(dir.path(), true);

    let (path, cfg) = LauncherConfig::discover(dir.path()).unwrap();
    assert_eq!(path.file_name().unwrap(), "lotro.launcherconfig");
    assert_eq!(
        cfg.gls_datacenter_service,
        "https://gls.example.invalid/service/datacenter"
    );
    assert_eq!(cfg.datacenter_game_name, "LOTRO");
    assert_eq!(cfg.default_center.as_deref(), Some("LOTRO"));
    assert_eq!(
        cfg.document_folder.as_deref(),
        Some("The Lord of the Rings Online")
    );
    assert_eq!(cfg.launcher_document_folder.as_deref(), Some("Launcher"));
}

/// Vector `V2` — `GetDatacenters` SOAP request and response with `[OLD]` world sorting.
#[test]
fn v2_get_datacenters_soap_request_and_response() {
    let req =
        build_get_datacenters_request("https://gls.example.invalid/service/datacenter", "LOTRO");
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.url, "https://gls.example.invalid/service/datacenter");
    assert_eq!(req.content_type(), Some("text/xml; charset=utf-8"));
    assert_eq!(req.soap_action(), Some(SOAP_ACTION_GET_DATACENTERS));
    let body_str = std::str::from_utf8(&req.body).unwrap();
    assert!(body_str.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
    assert!(body_str.contains("<GetDatacenters xmlns=\"http://www.turbine.com/SE/GLS\">"));
    assert!(body_str.contains("<game>LOTRO</game>"));

    let dc =
        parse_get_datacenters_response(V2_DATACENTERS_RESPONSE_XML.as_bytes(), "LOTRO").unwrap();
    assert_eq!(dc.name, "LOTRO");
    assert_eq!(
        dc.auth_server,
        "https://gls-auth.example.invalid/service/auth"
    );
    assert_eq!(dc.patch_server, "patch.example.invalid:6015");
    assert_eq!(
        dc.launcher_configuration_server,
        "https://gls.example.invalid/Launcher/lotrolauncher.server.config.xml"
    );
    assert_eq!(dc.worlds.len(), 2);
    // [EN] WorldBeta must sort before WorldAlpha [OLD]
    assert_eq!(dc.worlds[0].name, "[EN] WorldBeta");
    assert!(!dc.worlds[0].is_old_world());
    assert_eq!(dc.worlds[1].name, "WorldAlpha [OLD]");
    assert!(dc.worlds[1].is_old_world());

    // Also verify optional Datacenter/SupportURL and Order ascending sorting within active group
    let order_xml = br#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <GetDatacentersResponse xmlns="http://www.turbine.com/SE/GLS">
      <GetDatacentersResult>
        <Datacenter>
          <Name>LOTRO</Name>
          <AuthServer>https://gls-auth.example.invalid/service/auth</AuthServer>
          <PatchServer>patch.example.invalid:6015</PatchServer>
          <LauncherConfigurationServer>https://gls.example.invalid/config.xml</LauncherConfigurationServer>
          <SupportURL>https://support-dc.example.invalid/help</SupportURL>
          <Worlds>
            <World><Name>WorldGamma [OLD]</Name><LoginServerUrl>(invalid service specified)</LoginServerUrl><ChatServerUrl>c:1</ChatServerUrl><StatusServerUrl>https://s/1</StatusServerUrl><Order>0</Order></World>
            <World><Name>WorldAlpha</Name><LoginServerUrl>(invalid service specified)</LoginServerUrl><ChatServerUrl>c:1</ChatServerUrl><StatusServerUrl>https://s/2</StatusServerUrl><Order>5</Order></World>
            <World><Name>WorldZeta</Name><LoginServerUrl>(invalid service specified)</LoginServerUrl><ChatServerUrl>c:1</ChatServerUrl><StatusServerUrl>https://s/3</StatusServerUrl><Order>1</Order></World>
          </Worlds>
        </Datacenter>
      </GetDatacentersResult>
    </GetDatacentersResponse>
  </soap:Body>
</soap:Envelope>"#;
    let dc2 = parse_get_datacenters_response(order_xml, "LOTRO").unwrap();
    assert_eq!(
        dc2.support_url.as_deref(),
        Some("https://support-dc.example.invalid/help")
    );
    assert_eq!(
        dc2.worlds
            .iter()
            .map(|w| w.name.as_str())
            .collect::<Vec<_>>(),
        vec!["WorldZeta", "WorldAlpha", "WorldGamma [OLD]"]
    );
}

/// Vector `V3` — `World/StatusServerUrl` `<Status>` parsing; no fallback URL is
/// built from built-in fragments (decision 0042 item 3).
#[test]
fn v3_status_server_query_and_status_parsing() {
    let primary_status_url = "http://legacy-status.example.invalid/service/status?s=shard-alpha";

    let mut http = MockHttpClient::default();
    http.push_ok(200, V3_STATUS_XML.as_bytes().to_vec());

    let status = fetch_world_status(&mut http, primary_status_url).unwrap();
    assert_eq!(http.requests.len(), 1);
    assert_eq!(http.requests[0].0, primary_status_url);

    assert_eq!(status.login_server(), "login-a.example.invalid:9001");
    assert_eq!(
        status.queue_url(),
        Some("http://queue-a.example.invalid:7080/command")
    );
    assert_eq!(status.now_serving_queue_number, Some(16));
    assert_eq!(status.last_assigned_queue_number, Some(21));
    assert!(status.allow_billing_role.contains("RoleTierVIP"));
    assert!(status.deny_billing_role.is_empty());
}

/// Vector `V4` — `LoginAccount` multi-subscription filtering and SOAP fault.
#[test]
fn v4_login_account_multi_subscription_filtering_and_soap_fault() {
    // Case A: Success with DDO + LOTRO subscriptions
    let res = parse_login_account_response(200, V4_LOGIN_ACCOUNT_SUCCESS_XML.as_bytes()).unwrap();
    assert_eq!(res.ticket(), "SYNTH+TICKET/ABC123==");
    assert_eq!(res.subscriptions.len(), 2);

    let matching = res.matching_subscriptions("LOTRO").unwrap();
    assert_eq!(matching.len(), 1);
    let sub = &matching[0];
    assert_eq!(sub.name(), "LOTROSUB00000000000000002");
    assert_eq!(sub.description.as_deref(), Some("LOTRO Primary"));
    assert_eq!(sub.product_tokens, vec!["TokenBase", "RoleTierVIP"]);

    let v3_status = WorldStatus::parse(V3_STATUS_XML.as_bytes()).unwrap();
    assert!(v3_status.check_billing_role(&sub.product_tokens).is_ok());

    // Case B: SOAP Fault on invalid credentials (HTTP 500 with BIS Fault)
    let fault_err =
        parse_login_account_response(500, V4_LOGIN_ACCOUNT_FAULT_XML.as_bytes()).unwrap_err();
    assert_eq!(
        fault_err.failure_class(),
        LoginFailureClass::F04_AUTH_REJECTED
    );
    match fault_err {
        LoginError::AuthRejected {
            fault_code,
            fault_string,
            detail_result,
        } => {
            assert_eq!(fault_code, "BIS");
            assert_eq!(fault_string, "Authentication rejected");
            assert_eq!(detail_result.as_deref(), Some("0x8004C002"));
        }
        other => panic!("expected AuthRejected, got {other:?}"),
    }
}

/// Vector `V5` — `TakeANumber` POST body encoding and `<Result>` evaluation.
#[test]
fn v5_take_a_number_post_body_encoding_and_result_evaluation() {
    let tmpl = "command=TakeANumber&subscription={0}&ticket={1}&ticket_type=GLS&queue_url={2}";
    let body = build_take_a_number_body(
        tmpl,
        "LOTROSUB00000000000000002",
        "SYNTH+TICKET/ABC123==",
        "http://queue-a.example.invalid:7080/command",
    );
    assert_eq!(
        body,
        "command=TakeANumber&subscription=LOTROSUB00000000000000002&ticket=SYNTH%2BTICKET%2FABC123%3D%3D&ticket_type=GLS&queue_url=http%3A%2F%2Fqueue-a.example.invalid%3A7080%2Fcommand"
    );

    let poll1_xml = br#"<Result><Command>TakeANumber</Command><HResult>0x00000000</HResult><QueueNumber>0x00000014</QueueNumber><NowServingNumber>0x00000011</NowServingNumber><ContextNumber>0x00000099</ContextNumber></Result>"#;
    let poll1 = QueueResult::parse(poll1_xml).unwrap();
    assert_eq!(
        poll1.evaluate().unwrap(),
        QueueEvaluation::Waiting {
            ahead: 3,
            context_number: Some("0x00000099".to_owned()),
        }
    );

    let poll2_xml = br#"<Result><Command>TakeANumber</Command><HResult>0x00000000</HResult><QueueNumber>0x00000014</QueueNumber><NowServingNumber>0x00000014</NowServingNumber><ContextNumber>0x00000099</ContextNumber></Result>"#;
    let poll2 = QueueResult::parse(poll2_xml).unwrap();
    assert_eq!(poll2.evaluate().unwrap(), QueueEvaluation::Ready);
}

/// Vector `V6` — `RefreshTicket` SOAP exchange and backoff arithmetic.
#[test]
fn v6_refresh_ticket_soap_exchange_and_backoff_arithmetic() {
    let mut refresher = TicketRefresher::new(
        "https://gls-auth.example.invalid/service/auth",
        "LOTROSUB00000000000000002",
        "SYNTH+TICKET/ABC123==",
        21600,
        1000.0,
    );
    assert_eq!(refresher.start_time_sec(), 1000.0);
    assert_eq!(refresher.next_refresh_offset_sec(), 10800.0);
    assert_eq!(refresher.attempt_count(), 0);

    // Tick at t = 11800.0 s (\Delta t = 10800.0 <= 10800.0): no action.
    assert!(refresher.poll_refresh_request(11800.0).is_none());
    assert_eq!(refresher.attempt_count(), 0);

    // Tick at t = 11800.1 s (\Delta t = 10800.1 > 10800.0):
    let req = refresher
        .poll_refresh_request(11800.1)
        .expect("should fire at 11800.1");
    assert!((refresher.next_refresh_offset_sec() - 16199.95).abs() < 1e-6);
    assert_eq!(refresher.attempt_count(), 1);
    assert_eq!(req.soap_action(), Some(SOAP_ACTION_REFRESH_TICKET));
    assert_eq!(req.timeout_sec, 30);
    let req_xml = std::str::from_utf8(&req.body).unwrap();
    assert!(req_xml.contains("<RefreshTicket xmlns=\"http://www.turbine.com/SE/GLS\">"));
    assert!(req_xml.contains("<ticket>SYNTH+TICKET/ABC123==</ticket>"));

    // Response at t = 11800.5 s:
    let resp_xml = br#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <RefreshTicketResponse xmlns="http://www.turbine.com/SE/GLS">
      <RefreshTicketResult>SYNTH+TICKET/NEW456==</RefreshTicketResult>
    </RefreshTicketResponse>
  </soap:Body>
</soap:Envelope>"#;
    let http_resp = HttpResponse {
        status_code: 200,
        body: resp_xml.to_vec(),
    };
    let outcome = refresher.handle_refresh_response(Ok(&http_resp), 11800.5);
    match outcome {
        RefreshOutcome::Refreshed {
            notification: Some(notif),
        } => {
            assert_eq!(notif.ticket(), "SYNTH+TICKET/NEW456==");
            assert_eq!(notif.account(), "LOTROSUB00000000000000002");
        }
        other => panic!("expected Refreshed with notification, got {other:?}"),
    }
    assert_eq!(refresher.ticket(), "SYNTH+TICKET/NEW456==");
    assert_eq!(refresher.start_time_sec(), 11800.5);
    assert_eq!(refresher.next_refresh_offset_sec(), 10800.0);
    assert_eq!(refresher.attempt_count(), 0);
    assert!(refresher.parsed_result_flag());
}

/// Vector `V7` — `netver`/`didver` serialization in `ClientVersionMessage` (`0x0000012C`)
/// and `0x10000002`/`0x10000001` dictionary resolution (§2.4, invariants `I11` & `I16`, §7.2).
#[test]
fn v7_client_version_and_prefix_tree_dictionary_resolution() {
    // 1. Synthetic 20-byte .dat superblock descriptor at file offset 0x17C..0x18F:
    let descriptor_20: [u8; 20] = [
        0x04, 0x03, 0x02, 0x01, 0x06, 0x05, 0x08, 0x07, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10, 0x4C, 0x04, 0x00, 0x00,
    ];
    let (didver_guid, archive_ver) =
        parse_dat_superblock_didver_descriptor(&descriptor_20).unwrap();
    assert_eq!(didver_guid, "01020304-0506-0708-090A-0B0C0D0E0F10");
    assert_eq!(archive_ver, 1100);

    // 2. Synthetic 0x0E010000 ("Client2Server", mapped by 0x10000002 in 0x28000010) header (44 bytes):
    let dict_0e010000: [u8; 44] = [
        0x8D, 0x87, 0x95, 0xE7, 0x33, 0xC6, 0x1C, 0x47, 0x83, 0x15, 0xC3, 0xDB, 0xBE, 0xAD, 0x2F,
        0xAA, 0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0x00,
        0xAB, 0xCD, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00,
    ];
    let hdr_0e010000 = BitTreeTableHeader::parse(&dict_0e010000, 0x0E01_0000).unwrap();
    assert_eq!(hdr_0e010000.format_uuid, PREFIX_TREE_DICT_SCHEMA_GUID);
    assert_eq!(
        hdr_0e010000.codec_uuid,
        [
            0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0x00,
            0xAB, 0xCD
        ]
    );
    assert_eq!(hdr_0e010000.redirect_did, 0);
    assert!(!hdr_0e010000.redirects);
    assert_eq!(hdr_0e010000.version, 1);
    assert_eq!(hdr_0e010000.body_len, 9);

    // 3. Synthetic 0x0E010002 ("Server2ClientDeferred", mapped by 0x10000001 in 0x28000010) stub (44 bytes):
    let dict_0e010002_stub: [u8; 44] = [
        0x8D, 0x87, 0x95, 0xE7, 0x33, 0xC6, 0x1C, 0x47, 0x83, 0x15, 0xC3, 0xDB, 0xBE, 0xAD, 0x2F,
        0xAA, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0,
        0xF0, 0x01, 0x01, 0x00, 0x01, 0x0E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let hdr_0e010002 = BitTreeTableHeader::parse(&dict_0e010002_stub, 0x0E01_0002).unwrap();
    assert!(hdr_0e010002.redirects);
    assert_eq!(hdr_0e010002.redirect_did, 0x0E01_0001);
    // A record whose redirectDid is its own DID (0x0E010001) is a literal dictionary.
    let mut self_ref = dict_0e010000;
    self_ref[32..36].copy_from_slice(&0x0E01_0001u32.to_le_bytes());
    let hdr_self = BitTreeTableHeader::parse(&self_ref, 0x0E01_0001).unwrap();
    assert!(!hdr_self.redirects);
    // A literal dictionary must be version 1.
    let mut v0 = dict_0e010000;
    v0[36] = 0;
    assert!(BitTreeTableHeader::parse(&v0, 0x0E01_0000).is_err());

    // 4. VersionInfo with netver = "1234": 56-byte versionString, 57-byte ClientVersionMessage payload.
    let info = VersionInfo::from_dat_superblock_descriptor("1234", &descriptor_20).unwrap();
    assert_eq!(info.didver_string(), "01020304-0506-0708-090A-0B0C0D0E0F10");
    assert_eq!(info.didver_version, 1100);
    assert_eq!(
        info.version_string(),
        "netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10"
    );
    assert_eq!(info.version_string().len(), 56);

    let encoded = info.encode_client_version_payload().unwrap();
    assert_eq!(encoded.len(), 57);
    assert_eq!(encoded[0], 0x38);
    assert_eq!(
        &encoded[1..],
        b"netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10"
    );
}

// ============================================================================
// Invariants I2–I16 (docs/specs/official-login.md §7.1)
// ============================================================================

/// Invariant `I2` (Bootstrap config discovery order): Given a directory
/// containing both `lotro.launcherconfig` and `TurbineLauncher.exe.config`,
/// the parser loads `lotro.launcherconfig` (with or without UTF-8 BOM).
#[test]
fn i2_bootstrap_config_discovery_order() {
    let dir = tempfile::tempdir().unwrap();
    write_synthetic_install(dir.path(), true);

    let legacy_xml = br#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <appSettings>
    <add key="Launcher.DataCenterService.GLS" value="https://legacy.example.invalid/service/datacenter" />
    <add key="DataCenter.GameName" value="LEGACY" />
  </appSettings>
</configuration>"#;
    std::fs::write(dir.path().join("TurbineLauncher.exe.config"), legacy_xml).unwrap();
    std::fs::write(dir.path().join("zz_other.launcherconfig"), legacy_xml).unwrap();

    // 1. lotro.launcherconfig wins over both *.launcherconfig and TurbineLauncher.exe.config
    let (path1, cfg1) = LauncherConfig::discover(dir.path()).unwrap();
    assert_eq!(path1.file_name().unwrap(), "lotro.launcherconfig");
    assert_eq!(cfg1.datacenter_game_name, "LOTRO");

    // 2. When lotro.launcherconfig is absent, lexicographically first *.launcherconfig wins
    std::fs::remove_file(dir.path().join("lotro.launcherconfig")).unwrap();
    std::fs::write(
        dir.path().join("aa_first.launcherconfig"),
        br#"<configuration><appSettings>
            <add key="Launcher.DataCenterService.GLS" value="https://aa.example.invalid/service/datacenter" />
            <add key="DataCenter.GameName" value="AA_FIRST" />
        </appSettings></configuration>"#,
    )
    .unwrap();
    let (path2, cfg2) = LauncherConfig::discover(dir.path()).unwrap();
    assert_eq!(path2.file_name().unwrap(), "aa_first.launcherconfig");
    assert_eq!(cfg2.datacenter_game_name, "AA_FIRST");

    // 3. When no *.launcherconfig exists, TurbineLauncher.exe.config is used
    std::fs::remove_file(dir.path().join("aa_first.launcherconfig")).unwrap();
    std::fs::remove_file(dir.path().join("zz_other.launcherconfig")).unwrap();
    let (path3, cfg3) = LauncherConfig::discover(dir.path()).unwrap();
    assert_eq!(path3.file_name().unwrap(), "TurbineLauncher.exe.config");
    assert_eq!(cfg3.datacenter_game_name, "LEGACY");
}

/// Invariant `I3` (`LoginServerUrl` placeholder rejection): The literal value
/// `(invalid service specified)` in `Datacenter/Worlds/World/LoginServerUrl` is
/// never passed to `-h` or used as a socket target; the target `host:port` is
/// always taken from the first non-empty `;`-delimited entry of `<Status>/<loginservers>`.
#[test]
fn i3_login_server_url_placeholder_rejection() {
    let dc =
        parse_get_datacenters_response(V2_DATACENTERS_RESPONSE_XML.as_bytes(), "LOTRO").unwrap();
    for world in &dc.worlds {
        assert_eq!(world.login_server_url, INVALID_LOGIN_SERVER_SENTINEL);
        assert!(world.usable_login_server_url().is_none());
    }

    let version = VersionInfo::new("1234", "00000000-0000-0000-0000-000000000001", 42).unwrap();

    // Attempting to construct SessionHandoff with `(invalid service specified)` is rejected.
    let bad = SessionHandoff::new(
        INVALID_LOGIN_SERVER_SENTINEL,
        0,
        "LOTROSUB00000000000000002",
        "SYNTH+TICKET/ABC123==",
        version,
        "English",
        "chat.example.invalid:2900",
        "https://gls-auth.example.invalid/service/auth",
        DEFAULT_TICKET_LIFETIME_SEC,
    );
    assert!(bad.is_err());
    assert_eq!(
        bad.unwrap_err().failure_class(),
        LoginFailureClass::F06_WORLD_CLOSED
    );

    // `<Status>` with only `;` or `(invalid service specified)` in `<loginservers>` is rejected as F06_WORLD_CLOSED.
    let closed_xml =
        br#"<Status><loginservers>;(invalid service specified);</loginservers><queueurls>;</queueurls></Status>"#;
    let err = WorldStatus::parse(closed_xml).unwrap_err();
    assert_eq!(err.failure_class(), LoginFailureClass::F06_WORLD_CLOSED);
}

/// Invariant `I4` as changed by decision 0042 item 3: on HTTP 404 or an
/// empty body the status query fails with `F06_WORLD_CLOSED` and a reason
/// naming the missing configuration value. No second request is made to a URL
/// built from built-in path fragments.
#[test]
fn i4_status_server_failure_builds_no_fallback_url() {
    let world_status_url = "http://unreachable.example.invalid/service/status?s=world-1";

    for (code, body) in [(200u16, Vec::new()), (404, b"Not Found".to_vec())] {
        let mut http = MockHttpClient::default();
        http.push_ok(code, body);
        http.push_ok(200, V3_STATUS_XML.as_bytes().to_vec());

        let err = fetch_world_status(&mut http, world_status_url).unwrap_err();
        assert_eq!(err.failure_class(), LoginFailureClass::F06_WORLD_CLOSED);
        assert!(err.to_string().contains("configuration"), "{err}");
        assert_eq!(http.requests.len(), 1, "no fallback request may be sent");
    }

    // An empty StatusServerUrl is refused before any request.
    let mut http = MockHttpClient::default();
    let err = fetch_world_status(&mut http, "  ").unwrap_err();
    assert_eq!(err.failure_class(), LoginFailureClass::F06_WORLD_CLOSED);
    assert!(http.requests.is_empty());
}

/// Invariant `I5` (Subscription `Name` vs `username`): `GameSubscription/Name`
/// (`SUB9876543210ABCDEF012345`), never `alice_login`, is passed to `TakeANumber`
/// and to the session layer (`-a`).
#[test]
fn i5_subscription_name_vs_username() {
    let dir = tempfile::tempdir().unwrap();
    write_synthetic_install(dir.path(), false);

    let login_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <LoginAccountResponse xmlns="http://www.turbine.com/SE/GLS">
      <LoginAccountResult>
        <Subscriptions>
          <GameSubscription>
            <Game>LOTRO</Game>
            <Name>SUB9876543210ABCDEF012345</Name>
            <Description>Primary Sub</Description>
          </GameSubscription>
        </Subscriptions>
        <Ticket>SYNTH+TICKET/ABC123==</Ticket>
      </LoginAccountResult>
    </LoginAccountResponse>
  </soap:Body>
</soap:Envelope>"#;

    let queue_ready_xml = r#"<Result><Command>TakeANumber</Command><HResult>0x00000000</HResult><QueueNumber>0x00000001</QueueNumber><NowServingNumber>0x00000005</NowServingNumber></Result>"#;

    let mut http = MockHttpClient::default();
    http.push_ok(200, V2_DATACENTERS_RESPONSE_XML.as_bytes().to_vec());
    http.push_ok(
        200,
        SYNTHETIC_REMOTE_LAUNCHER_CONFIG_XML.as_bytes().to_vec(),
    );
    http.push_ok(200, login_xml.as_bytes().to_vec());
    http.push_ok(
        200,
        br#"<Status><loginservers>login-a.example.invalid:9001;</loginservers><queueurls>http://queue-a.example.invalid:7080/command;</queueurls></Status>"#
            .to_vec(),
    );
    http.push_ok(200, queue_ready_xml.as_bytes().to_vec());

    let mut driver = LoginDriver::new();
    assert_eq!(driver.state(), LoginStateId::S1_LOCAL_DISCOVERY);
    driver.load_local_config(dir.path(), None).unwrap();
    driver.execute_get_datacenters(&mut http).unwrap();
    driver.execute_get_launcher_config(&mut http).unwrap();
    driver
        .submit_credentials_and_world(
            Credentials::new("alice_login", "secret_pass"),
            "[EN] WorldBeta",
        )
        .unwrap();
    driver.execute_login_account(&mut http).unwrap();
    assert_eq!(driver.state(), LoginStateId::S7_QUERY_WORLD_STATUS);
    driver.execute_query_world_status(&mut http).unwrap();
    assert_eq!(driver.state(), LoginStateId::S8_WORLD_QUEUE);

    let eval = driver.execute_queue_poll(&mut http).unwrap();
    assert_eq!(eval, QueueEvaluation::Ready);
    assert_eq!(driver.state(), LoginStateId::S9_SESSION_HANDOFF);

    // Check TakeANumber POST body used SUB9876543210ABCDEF012345, not alice_login
    let queue_req_body = std::str::from_utf8(&http.requests[4].3).unwrap();
    assert!(queue_req_body.contains("subscription=SUB9876543210ABCDEF012345"));
    assert!(!queue_req_body.contains("alice_login"));

    let version = VersionInfo::new("1234", "11111111-2222-3333-4444-555555555555", 100).unwrap();
    let handoff = driver.complete_session_handoff(version, 0, 0.0).unwrap();
    assert_eq!(handoff.subscription_name(), "SUB9876543210ABCDEF012345");
    assert_ne!(handoff.subscription_name(), "alice_login");
}

/// Invariant `I6` (Billing role check): `<allow_billing_role>RoleVIP,RoleFounder</allow_billing_role>`
/// rejects `ProductTokens = {"AngmarTrial", "MoriaExp"}` with `F07_WORLD_ROLE_DENIED`.
#[test]
fn i6_billing_role_check() {
    let status_xml = br#"<Status>
        <loginservers>login1.example.invalid:9000;</loginservers>
        <queueurls>http://queue1.example.invalid:7080/command;</queueurls>
        <allow_billing_role>RoleVIP,RoleFounder</allow_billing_role>
    </Status>"#;
    let status = WorldStatus::parse(status_xml).unwrap();
    let tokens = vec!["AngmarTrial".to_owned(), "MoriaExp".to_owned()];
    let err = status.check_billing_role(&tokens).unwrap_err();
    assert_eq!(
        err.failure_class(),
        LoginFailureClass::F07_WORLD_ROLE_DENIED
    );

    // Also test deny_billing_role when allow_billing_role is empty
    let allow_empty = std::collections::BTreeSet::new();
    let deny_set: std::collections::BTreeSet<String> =
        ["BannedRole".to_owned()].into_iter().collect();
    let err2 = check_billing_role(&["BannedRole".to_owned()], &allow_empty, &deny_set).unwrap_err();
    assert_eq!(
        err2.failure_class(),
        LoginFailureClass::F07_WORLD_ROLE_DENIED
    );
}

/// Invariant `I7` (`TakeANumber` ticket percent-encoding vs raw `--glsticketdirect`):
/// `+`, `/`, and `=` in `Ticket` are percent-encoded as `%2B`, `%2F`, and `%3D`
/// in `TakeANumber`, while `--glsticketdirect` and `SessionHandoff` receive the
/// raw unencoded `Ticket`.
#[test]
fn i7_take_a_number_ticket_percent_encoding() {
    let raw_ticket = "SYNTH+TICKET/ABC123==";
    let form_body = build_take_a_number_body(
        "command=TakeANumber&subscription={0}&ticket={1}&ticket_type=GLS&queue_url={2}",
        "SUB01",
        raw_ticket,
        "http://queue.example.invalid/cmd",
    );
    assert!(form_body.contains("ticket=SYNTH%2BTICKET%2FABC123%3D%3D"));

    let remote =
        RemoteLauncherConfig::parse(SYNTHETIC_REMOTE_LAUNCHER_CONFIG_XML.as_bytes()).unwrap();
    let cmdline = substitute_arg_template(
        &remote.arg_template,
        &ArgTemplateParams {
            subscription: "SUB01".to_owned(),
            login_server: "login.example.invalid:9000".to_owned(),
            gls_ticket: raw_ticket.to_owned(),
            chat_server: "chat.example.invalid:2900".to_owned(),
            language: "English".to_owned(),
            game_type: "LOTRO".to_owned(),
            auth_server_url: "https://gls-auth.example.invalid/service/auth".to_owned(),
            gls_ticket_lifetime_sec: 21600,
            support_url: "https://support.example.invalid".to_owned(),
            support_service_url: "https://support-svc.example.invalid".to_owned(),
            bug_url: "https://bugs.example.invalid".to_owned(),
            crash_receiver: String::new(),
            upload_throttle: String::new(),
            high_res_enabled: None,
            user_dir: None,
        },
        remote.high_res_patch_arg.as_deref(),
    );
    assert!(cmdline.contains("--glsticketdirect \"SYNTH+TICKET/ABC123==\""));
    assert!(cmdline.ends_with(" --HighResOutOfDate"));
}

/// Invariant `I8` (Queue completion boundary): Both `QueueNumber == NowServingNumber`
/// and `QueueNumber < NowServingNumber` terminate the queue loop immediately.
#[test]
fn i8_queue_completion_boundary() {
    let eq_xml = br#"<Result><Command>TakeANumber</Command><HResult>0x00000000</HResult><QueueNumber>0x00000010</QueueNumber><NowServingNumber>0x00000010</NowServingNumber></Result>"#;
    let lt_xml = br#"<Result><Command>TakeANumber</Command><HResult>0x00000000</HResult><QueueNumber>0x0000000F</QueueNumber><NowServingNumber>0x00000010</NowServingNumber></Result>"#;
    let fail_xml =
        br#"<Result><Command>TakeANumber</Command><HResult>0x80004005</HResult></Result>"#;

    assert_eq!(
        QueueResult::parse(eq_xml).unwrap().evaluate().unwrap(),
        QueueEvaluation::Ready
    );
    assert_eq!(
        QueueResult::parse(lt_xml).unwrap().evaluate().unwrap(),
        QueueEvaluation::Ready
    );
    let err = QueueResult::parse(fail_xml)
        .unwrap()
        .evaluate()
        .unwrap_err();
    assert_eq!(err.failure_class(), LoginFailureClass::F08_QUEUE_ERROR);
}

/// Invariant `I9` (`RefreshTicket` backoff schedule): Given `--glsticketlifetime 21600`
/// installed at $t_0 = 0.0\text{ s}$, if every `RefreshTicket` call fails,
/// `RefreshTicket` is invoked at $t = 10800.0\text{ s}$, $t = 16200.0\text{ s}$,
/// and $t = 18900.0\text{ s}$, and is never invoked a 4th time ($t > 18900.0\text{ s}$).
#[test]
fn i9_refresh_ticket_backoff_schedule() {
    assert_eq!(
        TicketRefresher::nominal_backoff_offsets(21600),
        [10800.0, 16200.0, 18900.0]
    );

    let mut http = MockHttpClient::default();
    http.push_ok(500, b"Internal Server Error".to_vec());
    http.push_ok(500, b"Internal Server Error".to_vec());
    http.push_ok(500, b"Internal Server Error".to_vec());

    let mut refresher = TicketRefresher::new(
        "https://gls-auth.example.invalid/service/auth",
        "SUB01",
        "SYNTH+TICKET/INITIAL==",
        21600,
        0.0,
    );

    // Attempt 1 at t = 10800.0 s
    let out1 = refresher
        .tick_at_nominal_schedule(&mut http, 10800.0)
        .unwrap();
    assert!(matches!(
        out1,
        RefreshOutcome::FailedWillRetry {
            attempt_count: 1,
            ..
        }
    ));
    assert_eq!(refresher.next_refresh_offset_sec(), 16200.0);

    // Attempt 2 at t = 16200.0 s
    let out2 = refresher
        .tick_at_nominal_schedule(&mut http, 16200.0)
        .unwrap();
    assert!(matches!(
        out2,
        RefreshOutcome::FailedWillRetry {
            attempt_count: 2,
            ..
        }
    ));
    assert_eq!(refresher.next_refresh_offset_sec(), 18900.0);

    // Attempt 3 at t = 18900.0 s (final attempt)
    let out3 = refresher
        .tick_at_nominal_schedule(&mut http, 18900.0)
        .unwrap();
    assert_eq!(
        out3,
        RefreshOutcome::Exhausted(LoginFailureClass::F10_REFRESH_EXHAUSTED)
    );
    assert_eq!(refresher.attempt_count(), 3);
    assert_eq!(refresher.next_refresh_offset_sec(), 20250.0);
    assert!(refresher.support_warning_recorded());

    // Never invoked a 4th time even at t = 20250.0 or t = 25000.0
    assert!(
        refresher
            .tick_at_nominal_schedule(&mut http, 20250.0)
            .is_none()
    );
    assert!(refresher.poll_refresh_request(25000.0).is_none());
    assert_eq!(http.requests.len(), 3);
}

/// Invariant `I10` (`RefreshTicket` reset on success): If `RefreshTicket`
/// succeeds at $t = 16200.0\text{ s}$ (attempt 2) with a new ticket,
/// `start_time_sec` resets to $16200.0\text{ s}$, `next_refresh_offset_sec`
/// resets to $10800.0\text{ s}$ (next fire at $t = 27000.0\text{ s}$), and
/// `attempt_count` resets to `0`.
#[test]
fn i10_refresh_ticket_reset_on_success() {
    let success_xml = br#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <RefreshTicketResponse xmlns="http://www.turbine.com/SE/GLS">
      <RefreshTicketResult>SYNTH+TICKET/REFRESHED==</RefreshTicketResult>
    </RefreshTicketResponse>
  </soap:Body>
</soap:Envelope>"#;

    let mut http = MockHttpClient::default();
    http.push_ok(500, b"Attempt 1 fails".to_vec());
    http.push_ok(200, success_xml.to_vec());
    http.push_ok(200, success_xml.to_vec());

    let mut refresher = TicketRefresher::new(
        "https://gls-auth.example.invalid/service/auth",
        "SUB01",
        "SYNTH+TICKET/INITIAL==",
        21600,
        0.0,
    );

    // Attempt 1 fails at t = 10800.0 s
    let _ = refresher
        .tick_at_nominal_schedule(&mut http, 10800.0)
        .unwrap();
    assert_eq!(refresher.attempt_count(), 1);
    assert_eq!(refresher.next_refresh_offset_sec(), 16200.0);

    // Attempt 2 succeeds at t = 16200.0 s
    let out2 = refresher
        .tick_at_nominal_schedule(&mut http, 16200.0)
        .unwrap();
    assert!(matches!(
        out2,
        RefreshOutcome::Refreshed {
            notification: Some(_)
        }
    ));
    assert_eq!(refresher.start_time_sec(), 16200.0);
    assert_eq!(refresher.next_refresh_offset_sec(), 10800.0);
    assert_eq!(refresher.attempt_count(), 0);
    assert_eq!(refresher.ticket(), "SYNTH+TICKET/REFRESHED==");

    // Does not fire before t = 27000.0 s (16200 + 10800)
    assert!(
        refresher
            .tick_at_nominal_schedule(&mut http, 26999.9)
            .is_none()
    );
    // Fires at t = 27000.0 s
    assert!(
        refresher
            .tick_at_nominal_schedule(&mut http, 27000.0)
            .is_some()
    );
}

/// Invariant `I11` (`ClientVersionMessage` string format): Given a 20-byte `.dat`
/// superblock descriptor (`[guid_16, version_u32_le]`) at `0x17C..0x18F` and a
/// `<netver_string>`, the combined version string serialized into `ClientVersionMessage`
/// (`0x0000012C`) matches the exact 56-byte ASCII pattern
/// `netver:<netver>; didver:<XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX>` (omitting
/// the `u32` archive version integer from `0x18C`) and is prefixed on the wire
/// by its `VariableU32` byte length (`0x38` for a 56-byte string).
#[test]
fn i11_version_string_format() {
    let info = VersionInfo::new("1234", "A1B2C3D4-E5F6-7890-ABCD-EF0123456789", 20261020).unwrap();
    assert_eq!(info.didver_string(), "A1B2C3D4-E5F6-7890-ABCD-EF0123456789");
    assert_eq!(
        info.version_string(),
        "netver:1234; didver:A1B2C3D4-E5F6-7890-ABCD-EF0123456789"
    );
    assert_eq!(info.version_string().len(), 56);
    let encoded = info.encode_client_version_payload().unwrap();
    assert_eq!(encoded.len(), 57);
    assert_eq!(encoded[0], 0x38);
    assert_eq!(&encoded[1..], info.version_string().as_bytes());

    // The length prefix is the session layer's 4-tier VariableU32 (official-session.md §5.6.1).
    let long_encoded = encode_client_version_payload(&"a".repeat(200)).unwrap();
    assert_eq!(&long_encoded[..2], &[0x80, 200]);
    let four_tier = encode_client_version_payload(&"b".repeat(0x4000)).unwrap();
    assert_eq!(&four_tier[..4], &[0xC0, 0x00, 0x00, 0x40]);
    assert_eq!(four_tier.len(), 0x4000 + 4);
    assert!(encode_client_version_payload("netver:é").is_err());

    // netver validation (§2.4.1): printable ASCII, no `;`, no whitespace; didver is a GUID.
    assert!(VersionInfo::new("12;34", "A1B2C3D4-E5F6-7890-ABCD-EF0123456789", 0).is_err());
    assert!(VersionInfo::new("1 2", "A1B2C3D4-E5F6-7890-ABCD-EF0123456789", 0).is_err());
    assert!(VersionInfo::new("1234", "0000-0000", 0).is_err());
}

/// The `-h` / `--host` forms of §4.1 with zero, one or two colons, and `-p` / `--port`.
#[test]
fn login_host_forms() {
    let zero = LoginHost::parse("login.example.invalid").unwrap();
    assert_eq!(zero.local_host, None);
    assert_eq!(zero.remote_host, "login.example.invalid");
    assert_eq!(zero.port, None);
    assert_eq!(zero.remote_port(Some(19000)).unwrap(), 19000);
    assert!(zero.remote_port(None).is_err());
    assert!(zero.remote_port(Some(70_000)).is_err());

    let one = LoginHost::parse("login.example.invalid:19001").unwrap();
    assert_eq!(one.port, Some(19001));
    // The embedded port wins over -p.
    assert_eq!(one.remote_port(Some(1)).unwrap(), 19001);

    let two = LoginHost::parse("192.0.2.10:login.example.invalid:19002").unwrap();
    assert_eq!(two.local_host.as_deref(), Some("192.0.2.10"));
    assert_eq!(two.remote_host, "login.example.invalid");
    assert_eq!(two.port, Some(19002));

    for bad in [
        "",
        "a:b:c:1",
        ":19000",
        "host:",
        "host:0",
        "host:65536",
        "host:+1",
        "<localHost:remoteHost:port>",
        "ho st:1",
    ] {
        assert!(LoginHost::parse(bad).is_err(), "{bad:?}");
    }
}

/// Invariant `I12` (Credential & ticket redaction): Formatting any error, debug,
/// or display output produced during `S4`–`S10` never outputs the substring of
/// the user's password, `Ticket`, or subscription `Name`.
#[test]
fn i12_credential_and_ticket_redaction() {
    let secret_pass = "SuperSecretPassword!98765";
    let secret_ticket = "SYNTH+BEARER+TICKET/SECRET999==";
    let secret_sub = "SUB_SECRET_IDENTIFIER_0001";

    let creds = Credentials::new("player_user", secret_pass);
    let sub = GameSubscription::new("LOTRO", secret_sub, Some("Main".to_owned()), vec![]);
    let version = VersionInfo::new("1.0", "00000000-0000-0000-0000-000000000000", 1).unwrap();
    let handoff = SessionHandoff::new(
        "login.example.invalid:9000",
        0,
        secret_sub,
        secret_ticket,
        version,
        "English",
        "chat.example.invalid:2900",
        "https://gls-auth.example.invalid/service/auth",
        21600,
    )
    .unwrap();
    let refresher = TicketRefresher::new(
        "https://gls-auth.example.invalid/service/auth",
        secret_sub,
        secret_ticket,
        21600,
        0.0,
    );
    let req = build_login_account_request(
        "https://gls-auth.example.invalid/service/auth",
        "player_user",
        secret_pass,
    );

    let formatted = format!("{creds:?} {creds} {sub:?} {handoff:?} {refresher:?} {req:?}");
    assert!(!formatted.contains(secret_pass));
    assert!(!formatted.contains(secret_ticket));
    assert!(!formatted.contains(secret_sub));
    assert!(formatted.contains("<REDACTED>"));
}

/// Invariant `I13` (`<queueurls>` RFC 1918 private-IPv4 filtering with fallback
/// and case-insensitive `<Status>` child tags):
/// - Prefers the first non-RFC-1918 entry when mixed with `10.0.0.0/8`,
///   `172.16.0.0/12`, or `192.168.0.0/16` URLs.
/// - Falls back to the first non-empty entry when all entries are RFC 1918.
#[test]
fn i13_queueurls_rfc1918_filtering_and_fallback() {
    assert!(is_rfc1918_queue_url("http://10.0.0.1:7080/command"));
    assert!(is_rfc1918_queue_url("http://192.168.1.42:7080/command"));
    assert!(is_rfc1918_queue_url("http://172.16.0.5:7080/command"));
    assert!(is_rfc1918_queue_url("http://172.31.255.9:7080/command"));
    assert!(!is_rfc1918_queue_url("http://172.15.0.1:7080/command"));
    assert!(!is_rfc1918_queue_url("http://172.32.0.1:7080/command"));
    assert!(!is_rfc1918_queue_url(
        "http://queue-a.example.invalid:7080/command"
    ));

    // Mixed RFC 1918 + public hostname (with mixed-case child tags under <Status>)
    let mixed_status_xml = br#"<Status>
        <LoginServers>login-a.example.invalid:9001;</LoginServers>
        <QueueUrls>http://10.0.0.1:7080/command;http://172.20.1.2:7080/command;http://queue-a.example.invalid:7080/command;</QueueUrls>
        <NowServingQueueNumber>0x00000010</NowServingQueueNumber>
        <LastAssignedQueueNumber>0x00000005</LastAssignedQueueNumber>
    </Status>"#;
    let mixed_status = WorldStatus::parse(mixed_status_xml).unwrap();
    assert_eq!(mixed_status.login_server(), "login-a.example.invalid:9001");
    assert_eq!(
        mixed_status.queue_url(),
        Some("http://queue-a.example.invalid:7080/command")
    );
    assert_eq!(mixed_status.now_serving_queue_number, Some(16));
    assert_eq!(mixed_status.last_assigned_queue_number, Some(5));

    // All-RFC-1918 fallback: selects the first non-empty entry
    let private_only_xml = br#"<Status>
        <loginservers>login-a.example.invalid:9001;</loginservers>
        <queueurls>http://10.1.2.3:7080/command;http://192.168.0.10:7080/command;</queueurls>
    </Status>"#;
    let private_status = WorldStatus::parse(private_only_xml).unwrap();
    assert_eq!(
        private_status.queue_url(),
        Some("http://10.1.2.3:7080/command")
    );
}

/// Invariant `I14` (`TakeANumber` `HResult` sign-bit evaluation and fallback):
/// - Any non-negative 32-bit `HResult` (`(HResult & 0x80000000) == 0`, e.g. `0x00000001` `S_FALSE`)
///   is evaluated as a successful queue response.
/// - Omitting `<HResult>` when `<QueueNumber>` and `<NowServingNumber>` are both
///   present defaults `HResult` to `0` and succeeds.
/// - Negative `HResult` (`(HResult & 0x80000000) != 0`) fails with `F08_QUEUE_ERROR`.
#[test]
fn i14_take_a_number_hresult_sign_bit_and_fallback() {
    // Non-zero positive HResult (bit 31 == 0) succeeds
    let pos_hr_xml = br#"<Result>
        <Command>TakeANumber</Command>
        <HResult>0x00000001</HResult>
        <QueueNumber>0x00000002</QueueNumber>
        <NowServingNumber>0x00000005</NowServingNumber>
    </Result>"#;
    let pos_qr = QueueResult::parse(pos_hr_xml).unwrap();
    assert_eq!(pos_qr.hresult, 1);
    assert_eq!(pos_qr.evaluate().unwrap(), QueueEvaluation::Ready);

    // Missing <HResult> with valid <QueueNumber> and <NowServingNumber> succeeds (hresult = 0)
    let no_hr_xml = br#"<Result>
        <Command>TakeANumber</Command>
        <QueueNumber>0x00000008</QueueNumber>
        <NowServingNumber>0x00000005</NowServingNumber>
    </Result>"#;
    let no_hr_qr = QueueResult::parse(no_hr_xml).unwrap();
    assert_eq!(no_hr_qr.hresult, 0);
    assert_eq!(
        no_hr_qr.evaluate().unwrap(),
        QueueEvaluation::Waiting {
            ahead: 3,
            context_number: None,
        }
    );

    // Missing <HResult> without queue numbers is rejected as malformed
    let invalid_no_hr_xml = br#"<Result><Command>TakeANumber</Command></Result>"#;
    let err = QueueResult::parse(invalid_no_hr_xml).unwrap_err();
    assert_eq!(err.failure_class(), LoginFailureClass::F08_QUEUE_ERROR);
}

/// Invariant `I15` (`ArgTemplate` legacy alias substitution and unreplaced token filtering):
/// - Replaces `{LANGUAGE}`, `{PRODUCT}`, `{AUTHSERVER}`, `{HIGHRES}`, `{USERDIR}`
///   alongside `{LANG}`, `{GAMETYPE}`, `{AUTHSERVERURL}`.
/// - Strips surrounding double quotes when splitting into `argv`.
/// - Drops any token still containing an unreplaced `{...}` placeholder.
#[test]
fn i15_arg_template_legacy_aliases_and_argv_filtering() {
    let template = r#"-a {SUBSCRIPTION} -h {LOGIN} --glsticketdirect "{GLS}" --chatserver "{CHAT}" --language {LANGUAGE} --gametype {PRODUCT} --authserverurl "{AUTHSERVER}" --glsticketlifetime {GLSTICKETLIFETIME} --highres {HIGHRES} --userdir "{USERDIR}" --crashreceiver "{CRASHRECEIVER}" {UNREPLACED_OPTIONAL}"#;

    let params = ArgTemplateParams {
        subscription: "SUB01".to_owned(),
        login_server: "login.example.invalid:9000".to_owned(),
        gls_ticket: "SYNTH+TICKET/ABC123==".to_owned(),
        chat_server: "chat.example.invalid:2900".to_owned(),
        language: "DE".to_owned(),
        game_type: "LOTRO".to_owned(),
        auth_server_url: "https://gls-auth.example.invalid/service/auth".to_owned(),
        gls_ticket_lifetime_sec: 21600,
        support_url: String::new(),
        support_service_url: String::new(),
        bug_url: String::new(),
        crash_receiver: String::new(), // empty -> "{CRASHRECEIVER}" remains and is filtered from argv
        upload_throttle: String::new(),
        high_res_enabled: Some(true),
        user_dir: Some("/tmp/user prefs".to_owned()),
    };

    let argv = build_launch_argv(template, &params, Some(" --HighResOutOfDate"));
    assert!(argv.contains(&"DE".to_owned()));
    assert!(argv.contains(&"LOTRO".to_owned()));
    assert!(argv.contains(&"https://gls-auth.example.invalid/service/auth".to_owned()));
    assert!(argv.contains(&"SYNTH+TICKET/ABC123==".to_owned()));
    assert!(argv.contains(&"1".to_owned()));
    assert!(argv.contains(&"/tmp/user prefs".to_owned()));
    assert!(argv.contains(&"--HighResOutOfDate".to_owned()));
    // Unreplaced placeholders `{CRASHRECEIVER}` and `{UNREPLACED_OPTIONAL}` must be omitted
    assert!(
        !argv
            .iter()
            .any(|tok| tok.contains('{') || tok.contains('}'))
    );
    assert_eq!(
        split_arg_template_argv(r#"--flag "quoted value" "{MISSING}" {OTHER_MISSING}"#),
        vec!["--flag".to_owned(), "quoted value".to_owned()]
    );
}

/// Builds a `DataIdMap` record (`records.md` §8.1) with one group of `(key, did, label)`
/// entries and an empty second group.
fn did_map_record(self_did: u32, entries: &[(u32, u32, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&self_did.to_le_bytes());
    out.push(1); // TSize hint
    out.push(u8::try_from(entries.len()).unwrap()); // VLE count
    for (key, did, _) in entries {
        out.extend_from_slice(&key.to_le_bytes());
        out.extend_from_slice(&did.to_le_bytes());
    }
    out.push(1);
    out.push(u8::try_from(entries.len()).unwrap());
    for (key, _, label) in entries {
        out.extend_from_slice(&key.to_le_bytes());
        out.push(u8::try_from(label.len()).unwrap());
        out.extend_from_slice(label.as_bytes());
    }
    out.extend_from_slice(&[1, 0, 1, 0]); // empty second group
    out
}

/// A synthetic 48-byte `0x0E01xxxx` record prefix (header + `nodeCount`).
fn dictionary_record(codec: u8, redirect: u32, version: u32, body_len: u32, nodes: u32) -> Vec<u8> {
    let mut out = PREFIX_TREE_DICT_SCHEMA_GUID.to_vec();
    out.extend_from_slice(&[codec; 16]);
    out.extend_from_slice(&redirect.to_le_bytes());
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&body_len.to_le_bytes());
    out.extend_from_slice(&nodes.to_le_bytes());
    out
}

/// Invariant `I16` (§2.4.3; `official-session.md` §4.1.3): the root map's key 3 names the
/// compression sub-map, whose keys `0x10000002` -> `0x0E010000` and `0x10000001` ->
/// `0x0E010002` -> (redirect) `0x0E010001` resolve; `0x0E010001` names itself and is literal.
/// The sub-map DID of the install the spec describes: a verification value only
/// (`property-system.md` §8.1), never built into the code.
const DID_MASTER_MAP_3_COMPRESSION: u32 = 0x2800_0010;

/// `official-session.md` §4.1.3 item 1: the root DID map comes from superblock `+0x170`.
#[test]
fn root_did_map_comes_from_the_superblock() {
    let mut header = vec![0u8; 0x1A8];
    header[DAT_SUPERBLOCK_OFFSET..DAT_SUPERBLOCK_OFFSET + 4].copy_from_slice(&DAT_SUPERBLOCK_MAGIC);
    assert_eq!(DAT_ROOT_DID_MAP_OFFSET, 0x170);
    header[0x170..0x174].copy_from_slice(&0x2800_0007u32.to_le_bytes());
    assert_eq!(root_did_map_from_superblock(&header).unwrap(), 0x2800_0007);
    // Not a DID map, no magic, too short: all refused.
    header[0x170..0x174].copy_from_slice(&0x0E01_0000u32.to_le_bytes());
    assert!(root_did_map_from_superblock(&header).is_err());
    let mut no_magic = header.clone();
    no_magic[DAT_SUPERBLOCK_OFFSET] = 0;
    assert!(root_did_map_from_superblock(&no_magic).is_err());
    assert!(root_did_map_from_superblock(&header[..0x172]).is_err());
}

#[test]
fn i16_prefix_tree_dictionary_did_map_and_deferred_redirection() {
    let root = did_map_record(
        0x2800_0000,
        &[
            (0, 0, "Undef"),
            (
                ROOT_KEY_COMPRESSION_DICTIONARY,
                DID_MASTER_MAP_3_COMPRESSION,
                "Synthetic3",
            ),
        ],
    );
    assert_eq!(
        compression_dictionary_map_did(&root).unwrap(),
        DID_MASTER_MAP_3_COMPRESSION
    );

    // The sub-map of official-session.md §4.1.3 (labels synthetic): 4 entries incl. Undef.
    let map_bytes = did_map_record(
        DID_MASTER_MAP_3_COMPRESSION,
        &[
            (0, 0, "Undef"),
            (
                DICT_KEY_SERVER_TO_CLIENT_DEFERRED,
                0x0E01_0002,
                "InDeferred",
            ),
            (DICT_KEY_CLIENT_TO_SERVER, 0x0E01_0000, "Out"),
            (DICT_KEY_SERVER_TO_CLIENT, 0x0E01_0001, "In"),
        ],
    );
    let map = DidMap3Table::parse(&map_bytes).unwrap();
    assert_eq!(map.self_did, DID_MASTER_MAP_3_COMPRESSION);
    assert_eq!(
        map.resolve_enum_key(0),
        None,
        "the Undef sentinel is a miss"
    );
    assert_eq!(
        map.resolve_enum_key(DICT_KEY_CLIENT_TO_SERVER),
        Some(0x0E01_0000)
    );
    assert_eq!(
        map.names
            .get(&DICT_KEY_SERVER_TO_CLIENT_DEFERRED)
            .map(String::as_str),
        Some("InDeferred")
    );
    // A label list that does not match the map list, or a non-0x28 record, is refused.
    let mut bad = map_bytes.clone();
    bad[4 + 2 + 32 + 1] = 3;
    assert!(DidMap3Table::parse(&bad).is_err());
    assert!(DidMap3Table::parse(&did_map_record(0x2500_0000, &[])).is_err());

    let rec_0e010000 = dictionary_record(0xA1, 0, 1, 9, 3);
    let rec_0e010002 = dictionary_record(0xB2, 0x0E01_0001, 0, 0, 0)[..44].to_vec();
    let rec_0e010001 = dictionary_record(0xB2, 0x0E01_0001, 1, 13, 5);
    let lookup = |did: u32| -> Option<&[u8]> {
        match did {
            0x0E01_0000 => Some(&rec_0e010000),
            0x0E01_0001 => Some(&rec_0e010001),
            0x0E01_0002 => Some(&rec_0e010002),
            _ => None,
        }
    };

    let out_dict = resolve_bit_tree_table(&map, DICT_KEY_CLIENT_TO_SERVER, lookup).unwrap();
    assert_eq!(out_dict.resolved_did, 0x0E01_0000);
    assert_eq!(out_dict.header.codec_uuid, [0xA1; 16]);
    assert_eq!(out_dict.header.node_count, Some(3));

    let in_dict = resolve_bit_tree_table(&map, DICT_KEY_SERVER_TO_CLIENT_DEFERRED, lookup).unwrap();
    assert_eq!(in_dict.initial_did, 0x0E01_0002);
    assert_eq!(in_dict.resolved_did, 0x0E01_0001);
    assert_eq!(in_dict.header.codec_uuid, [0xB2; 16]);
    assert!(!in_dict.header.redirects);
    assert_eq!(in_dict.header.body_len, 13);

    // Direct key 0x10000003 reaches 0x0E010001, whose redirectDid is its own DID.
    let direct = resolve_bit_tree_table(&map, DICT_KEY_SERVER_TO_CLIENT, lookup).unwrap();
    assert_eq!(direct.initial_did, 0x0E01_0001);
    assert_eq!(direct.resolved_did, 0x0E01_0001);

    // .dat superblock extraction (magic "BT\0\0" at 0x140 + descriptor at 0x17C).
    let mut dat_hdr = vec![0u8; 0x1A8];
    dat_hdr[DAT_SUPERBLOCK_OFFSET..DAT_SUPERBLOCK_OFFSET + 4]
        .copy_from_slice(&DAT_SUPERBLOCK_MAGIC);
    let desc_20: [u8; 20] = [
        0x04, 0x03, 0x02, 0x01, 0x06, 0x05, 0x08, 0x07, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10, 0x4C, 0x04, 0x00, 0x00,
    ];
    dat_hdr[DAT_DIDVER_DESCRIPTOR_OFFSET..DAT_DIDVER_DESCRIPTOR_OFFSET + 20]
        .copy_from_slice(&desc_20);
    let (guid, ver) = extract_didver_from_dat_header(&dat_hdr).unwrap();
    assert_eq!(guid, "01020304-0506-0708-090A-0B0C0D0E0F10");
    assert_eq!(ver, 1100);
}

// ============================================================================
// Additional §2.2, §2.3, §3.1, §3.4, §3.5, §4.1, and Hostile-Input Tests
// ============================================================================

#[test]
fn language_and_user_preferences_resolution() {
    assert_eq!(language_to_feed_code("English"), "en");
    assert_eq!(language_to_feed_code("EN_GB"), "en");
    assert_eq!(language_to_feed_code("FR"), "fr");
    assert_eq!(language_to_feed_code("DE"), "de");

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("client_local_DE.dat"), b"x").unwrap();
    std::fs::write(dir.path().join("client_local_FR.dat"), b"x").unwrap();

    // Without English, falls back to lexicographically first installed (DE)
    assert_eq!(resolve_install_language(dir.path(), None).unwrap(), "DE");

    // With UserPreferences LastLocale = fr and [General] LastWorld, parses both
    let ini = br#"
# Hash comment line
; Semicolon comment line
[General]
LastWorld=[EN] WorldBeta

[Launcher]
GameClientType=3
LastLocale=fr
"#;
    let prefs = UserPreferences::parse(ini).unwrap();
    assert_eq!(prefs.game_client_type, Some(3));
    assert_eq!(prefs.last_locale.as_deref(), Some("fr"));
    assert_eq!(prefs.last_world.as_deref(), Some("[EN] WorldBeta"));
    assert_eq!(
        resolve_install_language(dir.path(), prefs.last_locale.as_deref()).unwrap(),
        "FR"
    );

    // 64-bit UserPreferences discovery order (§2.3):
    // UserPreferences64.ini -> UserPreferences_x64.ini -> UserPreferences.ini
    let prefs_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        prefs_dir.path().join("UserPreferences.ini"),
        b"[General]\nLastWorld=LegacyWorld\n",
    )
    .unwrap();
    std::fs::write(
        prefs_dir.path().join("UserPreferences_x64.ini"),
        b"[General]\nLastWorld=X64UnderscoreWorld\n",
    )
    .unwrap();
    std::fs::write(
        prefs_dir.path().join("UserPreferences64.ini"),
        b"[General]\nLastWorld=Primary64World\n",
    )
    .unwrap();

    let (p1, u1) = UserPreferences::discover(prefs_dir.path())
        .unwrap()
        .unwrap();
    assert_eq!(p1.file_name().unwrap(), "UserPreferences64.ini");
    assert_eq!(u1.last_world.as_deref(), Some("Primary64World"));

    std::fs::remove_file(prefs_dir.path().join("UserPreferences64.ini")).unwrap();
    let (p2, u2) = UserPreferences::discover(prefs_dir.path())
        .unwrap()
        .unwrap();
    assert_eq!(p2.file_name().unwrap(), "UserPreferences_x64.ini");
    assert_eq!(u2.last_world.as_deref(), Some("X64UnderscoreWorld"));

    std::fs::remove_file(prefs_dir.path().join("UserPreferences_x64.ini")).unwrap();
    let (p3, u3) = UserPreferences::discover(prefs_dir.path())
        .unwrap()
        .unwrap();
    assert_eq!(p3.file_name().unwrap(), "UserPreferences.ini");
    assert_eq!(u3.last_world.as_deref(), Some("LegacyWorld"));
}

#[test]
fn soap_fault_maintenance_and_account_locked_classification() {
    // 1. HTTP 503 Service Unavailable -> F03_SERVICE_UNAVAILABLE
    let err_503 = parse_login_account_response(503, b"Service Unavailable").unwrap_err();
    assert_eq!(
        err_503.failure_class(),
        LoginFailureClass::F03_SERVICE_UNAVAILABLE
    );

    // 2. SOAP <fault> with <Message> containing "maintenance" -> F03_SERVICE_UNAVAILABLE
    let maint_xml = br#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <soap:fault>
      <faultcode>BIS</faultcode>
      <Message>The authentication server is down for scheduled Maintenance.</Message>
    </soap:fault>
  </soap:Body>
</soap:Envelope>"#;
    let err_maint = parse_login_account_response(500, maint_xml).unwrap_err();
    assert_eq!(
        err_maint.failure_class(),
        LoginFailureClass::F03_SERVICE_UNAVAILABLE
    );

    // 3. SOAP <Fault> with "account" and "lock" -> F04_AUTH_REJECTED and is_account_locked() == true
    let locked_xml = br#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <soap:Fault>
      <faultcode>BIS</faultcode>
      <faultstring>This Account has been Locked due to too many failed attempts.</faultstring>
    </soap:Fault>
  </soap:Body>
</soap:Envelope>"#;
    let err_locked = parse_login_account_response(500, locked_xml).unwrap_err();
    assert_eq!(
        err_locked.failure_class(),
        LoginFailureClass::F04_AUTH_REJECTED
    );
    assert!(err_locked.is_account_locked());
}

#[test]
fn ticket_intake_precedence_and_registry_fallback() {
    struct MockRegistry(Option<String>);
    impl RegistryTicketReader for MockRegistry {
        fn read_hkcu_ticket(&self, _subkey: &str) -> Result<String, LoginError> {
            self.0
                .clone()
                .ok_or_else(|| LoginError::TicketInaccessible {
                    reason: "missing registry key".to_owned(),
                })
        }
    }

    // 1. Primary path (--glsticketdirect + --authserverurl) enables refresh
    let primary = resolve_ticket_intake(
        &TicketIntakeArgs {
            gls_ticket_direct: Some("RAW+DIRECT/TICKET==".to_owned()),
            auth_server_url: Some("https://gls-auth.example.invalid/service/auth".to_owned()),
            gls_ticket_registry_key: Some("Software\\ IgnoredKey".to_owned()),
            gls_ticket_lifetime_sec: 21600,
        },
        Some(&MockRegistry(Some("REGISTRY_TICKET".to_owned()))),
    )
    .unwrap();
    assert_eq!(primary.ticket(), "RAW+DIRECT/TICKET==");
    assert!(primary.refresh_enabled);

    // 2. Fallback path (--glsticket) loads from registry and does NOT enable refresh
    let fallback = resolve_ticket_intake(
        &TicketIntakeArgs {
            gls_ticket_direct: None,
            auth_server_url: None,
            gls_ticket_registry_key: Some("Software\\GameLauncher".to_owned()),
            gls_ticket_lifetime_sec: 21600,
        },
        Some(&MockRegistry(Some("REGISTRY+TICKET==".to_owned()))),
    )
    .unwrap();
    assert_eq!(fallback.ticket(), "REGISTRY+TICKET==");
    assert!(!fallback.refresh_enabled);

    // 3. Missing both fails with F09_TICKET_INACCESSIBLE
    let err = resolve_ticket_intake(&TicketIntakeArgs::default(), None).unwrap_err();
    assert_eq!(
        err.failure_class(),
        LoginFailureClass::F09_TICKET_INACCESSIBLE
    );
}

#[test]
fn leave_queue_body_encoding_and_hostile_xml_bounds() {
    let body = build_leave_queue_body(
        "command=LeaveQueue&subscription={0}&context={1}&ticket_type=GLS&queue_url={2}",
        "SUB01",
        "0x00000099",
        "http://queue-a.example.invalid:7080/command",
    );
    assert_eq!(
        body,
        "command=LeaveQueue&subscription=SUB01&context=0x00000099&ticket_type=GLS&queue_url=http%3A%2F%2Fqueue-a.example.invalid%3A7080%2Fcommand"
    );

    // Oversized input (> MAX_XML_INPUT_BYTES) is rejected without panic
    let oversized = vec![b' '; MAX_XML_INPUT_BYTES + 1];
    assert!(LauncherConfig::parse(&oversized).is_err());

    // Deeply nested XML (> MAX_XML_DEPTH) is rejected without panic
    let mut deep = String::new();
    for _ in 0..=MAX_XML_DEPTH + 2 {
        deep.push_str("<a>");
    }
    for _ in 0..=MAX_XML_DEPTH + 2 {
        deep.push_str("</a>");
    }
    assert!(WorldStatus::parse(deep.as_bytes()).is_err());

    // Excessive item count (> MAX_XML_ITEMS) is rejected without panic
    let mut many = String::from("<configuration><appSettings>");
    for i in 0..=MAX_XML_ITEMS {
        many.push_str(&format!("<add key=\"k{i}\" value=\"v\" />"));
    }
    many.push_str("</appSettings></configuration>");
    assert!(LauncherConfig::parse(many.as_bytes()).is_err());
}

// ============================================================================
// Real-Data Test (gated on TESSERA_GAME_DIR; read-only, logs no URLs)
// ============================================================================

#[test]
fn real_data_discover_launcher_config_and_language() {
    let Some(game_dir) = std::env::var_os("TESSERA_GAME_DIR") else {
        return;
    };
    let root = Path::new(&game_dir);
    if !root.is_dir() {
        return;
    }

    let (config_path, cfg) =
        LauncherConfig::discover(root).expect("real install must contain a valid *.launcherconfig");
    assert!(config_path.is_file());
    assert!(!cfg.gls_datacenter_service.is_empty());
    assert!(!cfg.datacenter_game_name.is_empty());
    assert!(!cfg.target_datacenter().is_empty());

    let lang = resolve_install_language(root, None)
        .expect("real install must contain at least one client_local_<Lang>.dat archive");
    assert!(!lang.is_empty());
    assert!(matches!(language_to_feed_code(&lang), "en" | "fr" | "de"));

    let general_dat = root.join("client_general.dat");
    if general_dat.is_file() {
        let (didver_guid, archive_ver) = extract_didver_from_dat_file(&general_dat)
            .expect("client_general.dat must contain a valid .dat superblock didver descriptor");
        assert_eq!(didver_guid.len(), 36);
        assert!(archive_ver > 0);
        let mut header = vec![0u8; 0x1A8];
        std::io::Read::read_exact(&mut std::fs::File::open(&general_dat).unwrap(), &mut header)
            .unwrap();
        let root_map = root_did_map_from_superblock(&header)
            .expect("client_general.dat names a root DID map in its superblock");
        assert_eq!(root_map >> 24, 0x28);
    }

    // Decision 0042 item 1: the version resource of the client executable is
    // read (never scanned). The test asserts only what the reader guarantees:
    // the resource parses and its string table is not empty. Contents are not
    // printed.
    let exe_64 = root.join("x64").join("lotroclient64.exe");
    if exe_64.is_file() {
        let resource = read_version_resource_from_file(&exe_64)
            .expect("the version resource of x64/lotroclient64.exe must parse");
        let pairs = resource.string_pairs().count();
        assert!(
            pairs > 0,
            "the version resource string table must not be empty"
        );
        eprintln!("x64/lotroclient64.exe version resource: {pairs} string(s)");
    }
}

/// §4.2 / `official-session.md` §6.2.1: the hand-off builds the `SessionAuthHeader` with
/// authType `0x40000002`, authFlags `0x04` and an extra blob of the decoded ticket length.
#[test]
fn handoff_session_auth_header_uses_decoded_ticket_length() {
    let version = VersionInfo::new("1234", "01020304-0506-0708-090A-0B0C0D0E0F10", 1100).unwrap();
    let handoff = |ticket: &str| {
        SessionHandoff::new(
            "login.example.invalid:19001",
            0,
            "LOTROSUB00000000000000002",
            ticket,
            version.clone(),
            "English",
            "chat.example.invalid:2900",
            "https://gls-auth.example.invalid/service/auth",
            DEFAULT_TICKET_LIFETIME_SEC,
        )
        .unwrap()
    };
    // "U1lOVEhUSUNLRVQ=" is Base64 of the 11 bytes "SYNTHTICKET".
    let ok = handoff("U1lOVEhUSUNLRVQ=");
    assert_eq!(ok.login_host().unwrap().port, Some(19001));
    let header = ok.session_auth_header(7).unwrap();
    assert_eq!(header.auth_type, 0x4000_0002);
    assert_eq!(header.auth_flags, 0x04);
    assert_eq!(
        header.credentials.extra_blob.as_deref(),
        Some(&b"SYNTHTICKET"[..])
    );
    assert_eq!(
        header.credentials.account_or_ticket,
        "LOTROSUB00000000000000002"
    );
    // The V4/V6 synthetic ticket text is not well-formed Base64 (21 characters).
    let bad = handoff("SYNTH+TICKET/ABC123==");
    assert!(bad.session_auth_header(7).is_err());
}
