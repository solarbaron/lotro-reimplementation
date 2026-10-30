<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Official Server Login & Launcher Service Specification

| Field | Value |
|---|---|
| Status | **Accepted** (independent Stage 1 core pass 9 review, 2026-10-30; acceptance gated by decision 0042 items 4 & 8; 2 OPEN) |
| Last updated | 2026-10-30 |
| Author role | `Spec writer (dirty room)` |
| Reviewer | Codex / GPT-6, independent session; UUID unavailable (host does not expose it); task `/root/pass9_review_coordinator/review_session_login`; 2026-10-30; PASS. |
| **Review record** | Reviewed writer commit `b2cc48e0dcaac912c77b13d909ed8e5e9abf485a`. Full CLEAN_ROOM §5.3 checklist; login/session consistency, Case B arithmetic, historical-log preservation and OPEN counts; sampled launcher XML, DAT superblock and matching-backup binary identifier/constant presence. Source checks are sampled data/presence checks, not exhaustive binary re-derivation or a live session. Writer session ID not recorded (host unavailable); earlier reviewers’ checks are not adopted. |
| Implements into | `crates/compat` (`tessera-compat`: GLS/world-queue login client & ticket refresh), `apps/client` (login flow & session hand-off) |
| Companion specs | `docs/specs/install-layout.md`, `docs/specs/official-session.md` |

---

## 0. Purpose <a id="s0-purpose"></a>

This specification defines the official Global Login System (GLS) and world-queue authentication flow required for a third-party client to authenticate a user with their own credentials against an official *The Lord of the Rings Online* (LOTRO) server installation, up to the boundary where the game client opens its UDP session with the world login server (`docs/specs/official-session.md`), together with the in-game GLS ticket refresh lifecycle (`RefreshTicket`).

Specifically, this document covers:

1. **Runtime configuration discovery** in the user's local game installation (`*.launcherconfig`, language archives, `UserPreferences.ini`, and client version/data identifiers `netver` and `didver`).
2. **Launcher-side HTTP/SOAP exchanges** in execution order:
   - Datacenter and world directory discovery (`GetDatacenters` SOAP 1.1 call).
   - Remote launcher configuration retrieval (`LauncherConfigurationServer` XML).
   - Per-world status and login server discovery (`World/StatusServerUrl` XML and billing-role checks; no fallback URL is constructed from path fragments, per `docs/decisions/0042-m8-remediation-decisions.md` item 3).
   - Account authentication (`LoginAccount` SOAP 1.1 call, fault handling, and subscription selection).
   - World login queue entry, polling, and cancellation (`WorldQueue.LoginQueue.URL` `TakeANumber` and optional `LeaveQueue` form POSTs).
   - Launcher-to-client parameter hand-off (command-line template placeholders and flags).
3. **Client-side ticket intake, session hand-off, and ticket refresh**:
   - Intake of `--glsticketdirect`, `--authserverurl`, `--glsticketlifetime`, and the legacy `--glsticket` Windows registry fallback.
   - Parameters prepared and handed over to the UDP session transport layer specified in `docs/specs/official-session.md`.
   - Periodic in-game `RefreshTicket` SOAP 1.1 requests, backoff schedule, and retry limits.
4. **End-to-end state machine**, timeouts, retry rules, and user-facing failure classes, starting with local configuration discovery (`S1_LOCAL_DISCOVERY`).
5. **Security and access-control rules**, including TLS validation requirements, credential and ticket redaction, and access-control stop conditions.
6. **Testable invariants and synthetic test vectors** using exclusively RFC 2606 `.example.invalid` placeholders and synthetic tokens.

In accordance with `CLAUDE.md`, `docs/CLEAN_ROOM.md`, `docs/decisions/0034-official-server-compatibility.md`, and `docs/decisions/0042-m8-remediation-decisions.md`, this specification states **only where** the launcher and client discover URLs, hostnames, ports, and service endpoints at runtime in the user's local installation or in server responses; it never embeds or publishes live endpoint values, hostnames, service path fragments, keys, certificates, or account data.

**Interoperability identifier rule (`docs/CLEAN_ROOM.md` §3.3, §5.3; `docs/decisions/0042-m8-remediation-decisions.md` items 1 & 3):** Every literal identifier retained in this specification — specifically XML element and attribute names, SOAP 1.1 action URIs (`"http://www.turbine.com/SE/GLS/GetDatacenters"`, `"http://www.turbine.com/SE/GLS/LoginAccount"`, `"http://www.turbine.com/SE/GLS/RefreshTicket"`) and XML namespaces (`http://www.turbine.com/SE/GLS`), `.NET` `appSettings` configuration keys, `.ini` section and key names, installation filenames (`lotro.launcherconfig`, `TurbineLauncher.exe.config`, `UserPreferences*.ini`, `client_local_<Lang>.dat`, `client_general.dat`, `client_gamelogic.dat`, `x64/lotroclient64.exe`, `lotroclient.exe`, `LotroLauncher.exe`), HTTP `application/x-www-form-urlencoded` field names, command-line option flags and template `{PLACEHOLDER}` tokens, the wire version prefix literals (`"180804_"`, `"netver:"`, `"; didver:"`), and the PE `VS_VERSION_INFO` `StringFileInfo` version resource key (`"CompatibiltyVersion"` / `"CompatibilityVersion"`) — appears literally on the HTTP/SOAP/UDP wire, in the installation's XML/INI/`.dat`/PE files on disk, or on the client executable's command line, and is required for interoperability. Per `docs/decisions/0042-m8-remediation-decisions.md` item 3, the XML namespace URI (`http://www.turbine.com/SE/GLS`) and `SOAPAction` strings are XML/SOAP protocol identifiers that are never contacted as network endpoints and may appear in code as constants only in the SOAP message encoder. Client-internal and launcher-internal C++/C# class, struct, method, and variable names are excluded; all wire messages, header fields, and state variables are named using neutral descriptive terminology.

---

## 1. Provenance <a id="s1-provenance"></a>

### 1.1 Source tags <a id="s1-1-sources"></a>

| Tag | Method | Description |
|---|---|---|
| `OBS` | M1 | Direct read-only inspection of the local LOTRO installation (`lotro.launcherconfig`, `client_local_*.dat`, `x64/lotroclient64.exe`, `LotroLauncher.exe`) and passive observation of the owner's own unmodified client sessions (`docs/decisions/0034-official-server-compatibility.md` §2). |
| `LL` | M2 | Project owner's *lotro-launcher* — GPL-3.0-or-later, consulted for protocol facts, XML parsing tolerances, `<queueurls>` RFC 1918 filtering, SOAP fault classification, `TakeANumber` execution, HRESULT sign-bit evaluation, and launch argument template aliases. |
| `OL` | M2 | June Stepp, *OneLauncher* — GPL-3.0, consulted for protocol facts only (local/remote config keys, SOAP envelopes, and world status and queue XML schemas). |
| `PL` | M2 | Alan Jackson, *PyLotRO* — GPL-3.0, consulted for protocol facts only (GLS SOAP calls, world status and queue handling, and client command-line template substitution). |
| `GLS` | M2 | `lotro-gls` — consulted for SOAP 1.1 service namespaces, operation signatures, and XML element schemas (`GetDatacenters`, `LoginAccount`, `RefreshTicket`). |
| `REDDO` | M2 | Xaymar, *RE-DDO* (`protocol/launcher.md`, `protocol/game.md`) — related-engine analogue consulted for cross-checking GLS and command-line conventions. |
| `DDOL` | M2 | Florian Stinglmayr & Scott Winburn, *ddolauncher* — BSD-2-Clause, related-engine analogue consulted for queue and SOAP parameter conventions. |
| `BIN` | M4 | Static analysis via Ghidra headless (`analyzeHeadless` (version not recorded in the original login research log)) and `objdump` of read-only copies of `x64/lotroclient64.exe` (PE FileVersion / ProductVersion `4906.0071.1794.4035`, SHA-256 `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee`) and `LotroLauncher.exe` (PE FileVersion / ProductVersion `4906.0071.1794.4035`, SHA-256 `4c995de339f2dd0a435a92a2564e7f781da5a5aa0ba494a1f6d7353d30d9c820`) under standing authorisation (`docs/decisions/0011-standing-m4-authorisation.md`) and the M8 amendment (`docs/decisions/0034-official-server-compatibility.md`). |
| `IL` | — | Companion specification `docs/specs/install-layout.md`. |

### 1.2 M4 (`BIN`) questions log <a id="s1-2-bin-log"></a>

| ID | Binary & Build | SHA-256 | Tool | Question | Why M1–M2 were insufficient |
|---|---|---|---|---|---|
| `BIN-Q1` | `x64/lotroclient64.exe` (`4906.0071.1794.4035`) | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra (version not recorded in the original login research log) headless (`-readOnly`) & `objdump` | How does the game client parse and prioritize `--glsticketdirect`, `--glsticket`, `--authserverurl`, and `--glsticketlifetime`, how does `--glsticket` load the fallback ticket from the Windows registry, and how are `authType`, `authFlags`, and `extraBlob` populated in `SessionAuthHeader` (`0x00010000`)? | Third-party launchers (`OL`, `PL`) only construct `--glsticketdirect`; they do not document client-side intake, wide-to-narrow conversion, the `--glsticket` registry hive and value name, or how `SessionAuthHeader` serializes the ASCII Base64 ticket in `extraBlob`. |
| `BIN-Q2` | `x64/lotroclient64.exe` (`4906.0071.1794.4035`) | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra (version not recorded in the original login research log) headless (`-readOnly`) | How does the game client schedule, format, transmit, and parse the in-game `RefreshTicket` SOAP call, including retry limits, backoff formula, HTTP timeout, and TLS peer verification settings? | Third-party launchers exit once the client starts; `GLS` only shows the server stub signature `RefreshTicket` without the client's timer arithmetic, retry cap, or HTTP client configuration. |
| `BIN-Q3` | `x64/lotroclient64.exe` (`4906.0071.1794.4035`) | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra (version not recorded in the original login research log) headless (`-readOnly`) & `objdump` | How does the game client construct its wire version string (`"netver:<netver>; didver:<didver>"`), where do `<netver>` and `<didver>` reside in the PE `.rsrc` `VS_VERSION_INFO` version resource and `.dat` archives, and how is that string serialized on the wire in both `SessionAuthHeader` (`0x00010000`, `docs/specs/official-session.md` §6.2) and the post-handshake `ClientVersionMessage` (`0x0000012C`, `docs/specs/official-session.md` §9.3)? | M2 launcher sources do not cover the client's hand-off to the UDP login session, the origin of `<netver>` and `<didver>`, or how the session version string is formatted in `SessionAuthHeader` (`"180804_netver:<netver>; didver:<didver>"`) and `ClientVersionMessage` (`0x0000012C`). |
| `BIN-Q4` | `x64/lotroclient64.exe` (`4906.0071.1794.4035`) | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra (version not recorded in the original login research log) headless (`-readOnly`) & `objdump` | How are keys `0x10000002` and `0x10000001` resolved to `.dat` records, what is the byte layout of the target records (`0x0E010000`, `0x0E010002`, `0x0E010001`), which 16 bytes from `0x10000002` are copied into the session context, and where are they negotiated in the UDP handshake? | M1 shows `0x10000001` and `0x10000002` are absent from the B-tree root directory of all `.dat` files; M4 was required to trace their resolution through the dictionary sub-map resolved from root key `0x00000003`, the `0x0E01xxxx` compression dictionary record format, and their use in `ConnectChallengeHeader` (`0x00040000`) and `ConnectAcceptHeader` (`0x00080000`). |
| `BIN-Q5` | `LotroLauncher.exe` (`4906.0071.1794.4035`) | `4c995de339f2dd0a435a92a2564e7f781da5a5aa0ba494a1f6d7353d30d9c820` | `objdump` & read-only string/import inspection | How does the official launcher handle `<Status>/<queueurls>` when empty vs non-empty, what configuration key sets its queue poll interval (`WorldQueue.LoginQueue.RefreshSeconds` default `20` s), does it send `LeaveQueue` on user cancel, and what HTTP/HTTPS scheme, `User-Agent`, and proxy behavior does it use? | Third-party launchers differ on queue polling defaults and `LeaveQueue`; the pass-8 inspection reports the queue behavior. Its HTTP/HTTPS scheme, `User-Agent`, and proxy claims remain reported, with details OPEN in §3; string/import presence alone does not settle per-request behavior. |

### 1.3 Provenance matrix <a id="s1-3-provenance-matrix"></a>

| ID | Section | Status | Sources | Notes |
|---|---|---|---|---|
| `P1` | §2.1 Local launcher config discovery & XML keys | **Confirmed** | `OBS`, `LL`, `OL`, `PL`, `IL` | Verified against local `lotro.launcherconfig` and third-party launcher parsers. |
| `P2` | §2.2–§2.3 Language discovery & `UserPreferences.ini` | **Confirmed** | `OBS`, `LL`, `OL`, `IL` | Cross-referenced with `docs/specs/install-layout.md` §3.1, §7, and §8, including 64-bit `UserPreferences64.ini` / `UserPreferences_x64.ini` lookup order in `LL`. |
| `P3` | §2.4 Client version identifiers (`netver`, `didver`) & the dictionary sub-map resolved from root key `0x00000003` compression dictionary resolution | **Confirmed** | `BIN` (`BIN-Q3`, `BIN-Q4`), `OBS`, `IL` | Read-only discovery of `<netver>` (via user-supplied setting/CLI value, PE `.rsrc` `VS_VERSION_INFO` `StringFileInfo` version resource, or user prompt per `docs/decisions/0042-m8-remediation-decisions.md` item 1) and `<didver>` (`.dat` superblock `0x17C..0x18B` GUID), `VariableU32` serialization in `SessionAuthHeader` (`0x00010000`, prefixed by `"180804_"`) and `ClientVersionMessage` (`0x0000012C`, `docs/specs/official-session.md` §5.6.1, §6.2, and §9.3), and the dictionary sub-map resolved from root key `0x00000003` resolution of `0x10000001`/`0x10000002` to the `0x0E01xxxx` compression dictionary records specified in `docs/specs/official-session.md` §4.1.2–§4.1.3 and §6.3–§6.4. |
| `P4` | §3.1 `GetDatacenters` SOAP 1.1 exchange | **Confirmed** | `LL`, `OL`, `PL`, `GLS`, `REDDO`, `DDOL` | All M2 sources agree on namespace, `SOAPAction`, request body, and response schema (including optional `SupportURL` and `Order` sorting in `LL`). |
| `P5` | §3.2 Remote launcher configuration XML (`LauncherConfigurationServer`) | **Confirmed** | `BIN` (`BIN-Q5`), `LL`, `OL`, `PL`, `REDDO` | `.NET` `appSettings` schema and key names (including `WorldQueue.LoginQueue.RefreshSeconds` and `WorldQueue.PollTimer`) confirmed across `LotroLauncher.exe`, `LL`, `OL`, `PL`, and `REDDO`. |
| `P6` | §3.3 Per-world status query (`World/StatusServerUrl`), RFC 1918 queue URL selection, & billing-role check | **Confirmed** | `LL`, `OL`, `PL`, `DDOL`, `REDDO` | XML schema, case-insensitive tag matching, `<queueurls>` RFC 1918 filtering, and billing-role intersection confirmed across `LL`, `OL`, and `PL`; no fallback URL is constructed from path fragments (`docs/decisions/0042-m8-remediation-decisions.md` item 3). |
| `P7` | §3.4 `LoginAccount` SOAP 1.1 exchange, faults, & subscription selection | **Confirmed** | `LL`, `OL`, `PL`, `GLS`, `REDDO`, `DDOL` | Request/response elements, `GameSubscription/Name` vs username distinction, and `BIS` SOAP fault classification confirmed across all M2 sources. |
| `P8` | §3.5 World login queue (`TakeANumber`, `LeaveQueue`) | **Confirmed** | `BIN` (`BIN-Q5`), `LL`, `OL`, `PL`, `REDDO`, `DDOL` | Form templates, URL-encoding requirements, `TakeANumber` execution when `queue_url` is non-empty (and skipping when `<queueurls>` is empty), `WorldQueue.LoginQueue.RefreshSeconds` (`20` s default), optional `LeaveQueue`, and `<Result>` HRESULT sign-bit evaluation confirmed across `LotroLauncher.exe`, `LL`, `OL`, `PL`, `REDDO`, and `DDOL`. |
| `P9` | §3.6 Launcher-to-client command-line template & placeholders | **Confirmed** | `LL`, `OL`, `PL`, `REDDO`, `BIN` (`BIN-Q1`) | Template placeholders (including `{LANGUAGE}`, `{PRODUCT}`, `{AUTHSERVER}`, `{HIGHRES}`, `{USERDIR}` aliases and unreplaced token filtering in `LL`) and client CLI option table confirmed in `lotroclient64.exe`. |
| `P10` | §4.1 Client ticket intake (`--glsticketdirect` vs `--glsticket` registry fallback) | **Confirmed** | `BIN` (`BIN-Q1`), `OBS` | Confirmed in `lotroclient64.exe` startup and `HKEY_CURRENT_USER` `"Ticket"` reader. |
| `P11` | §4.2 Hand-off to UDP session layer (`docs/specs/official-session.md`) | **Confirmed** | `BIN` (`BIN-Q1`, `BIN-Q3`, `BIN-Q4`), `OBS` | Session hand-off parameters, `SessionAuthHeader` (`0x00010000`) `netVersion` (`"180804_netver:<netver>; didver:<didver>"`), `authType` (`0x00000001` on primary login/world vs `0x40000002` on `ChatSession`), `extraBlob` carrying the `VariableU32`-prefixed raw ASCII Base64 ticket string, `ClientVersionMessage` (`0x0000012C`) version string, and compression dictionary hand-off (`codecUuid` at `+0x08..+0x17` of `ConnectAcceptHeader`) to `docs/specs/official-session.md` confirmed in `lotroclient64.exe` and live session observation. |
| `P12` | §4.3 In-game `RefreshTicket` SOAP 1.1 lifecycle, backoff math, & retry cap | **Confirmed** | `BIN` (`BIN-Q2`), `GLS` | Half-remaining-lifetime backoff ($0.5 L$, $0.75 L$, $0.875 L$), 3-attempt cap, 30 s timeout, and XML response parsing confirmed in `lotroclient64.exe`. |
| `P13` | §6.2 Official client TLS peer verification setting | **Confirmed** | `BIN` (`BIN-Q2`), `PL`, `DDOL` | Confirmed in `lotroclient64.exe` SOAP transport configuration; Tessera policy overrides this to require strict TLS verification. |
| `P14` | §2, §3.3, §6.2 HTTPS-first and explicit-port policy | **Confirmed (owner policy)** | Decision 0044 items 1–2 and amendment items 3–4, 2026-10-27 | Default HTTPS port; original address only after failure before request transmission, credentials permitted on that HTTP retry. |
| `P15` | §3 launcher HTTP scheme, agent and proxy questions | **Reported; details OPEN** | Pass-8 `BIN-Q5` and `BIN-Q2` reports; pass-9 source-scope reconciliation | Launcher and in-game transport evidence are separate; no new per-step observation claimed. |
| `P16` | §4.3, I9/I10 timer sampling arithmetic | **Confirmed (synthetic arithmetic)** | BIN-Q2 strict comparison retained; independently recomputed synthetic timer checks, 2026-10-30 | Nominal thresholds are not transmission times; no new binary timing observation claimed. |

---

## 2. Runtime Configuration and Local Inputs <a id="s2-runtime-config"></a>

All service bootstrap endpoints, game identifiers, and local asset metadata are read at runtime from the user's read-only LOTRO installation directory and user preferences directory (`docs/specs/install-layout.md`). No service URL, hostname, IP address, or port may be hardcoded in source code or configuration defaults.

**Tessera login address policy (decision 0044, amended):** For every login-chain address supplied as `http://`, first try `https://` at the same host and path, retaining the query and using the default HTTPS port; do not carry over an explicit HTTP port. Use a short connection timeout. Only a failure before the request is sent (name lookup, connect, TLS handshake or certificate verification) permits retrying the address exactly as supplied, including its original explicit port, over HTTP. Use any service answer over working HTTPS as the answer; do not retry it over HTTP. Credentials and tickets may be sent on that permitted HTTP fallback. Log the step only, never the address or account. HTTPS keeps certificate and hostname verification, and redirects remain disabled. This applies to the datacenter service, launcher configuration, authentication, queue, world status and auth-server override/refresh addresses. An answer read over HTTPS counts as HTTPS provenance; an answer read over plain HTTP never counts as HTTPS provenance (decision 0044 item 2). This is owner policy, distinct from observed launcher behavior.

### 2.1 Local launcher configuration (`*.launcherconfig`) <a id="s2-1-launcherconfig"></a>

To locate the bootstrap configuration file inside the game installation root directory (`<InstallRoot>`), the client searches the following relative paths in order and uses the first file that exists (`docs/specs/install-layout.md` §6.1). All filenames (`lotro.launcherconfig`, `TurbineLauncher.exe.config`), XML element/attribute names (`configuration`, `appSettings`, `add`, `key`, `value`), and `<add key="...">` key strings in the table below appear literally in the installation's XML configuration files on disk and are required for interoperability:

1. `lotro.launcherconfig`
2. The lexicographically first file matching `*.launcherconfig` in `<InstallRoot>`
3. `TurbineLauncher.exe.config` (legacy installation fallback)

The file is a UTF-8 XML document (with or without a leading UTF-8 byte-order mark `EF BB BF`) using the `.NET` application configuration schema: a root `<configuration>` element containing `<appSettings>`, which in turn contains zero or more `<add key="..." value="..." />` elements.

| `<add key="...">` Name | Required | Meaning |
|---|---|---|
| `Launcher.DataCenterService.GLS` | Yes | Full HTTP/HTTPS URL of the GLS datacenter directory SOAP 1.1 endpoint used in Exchange 1 (§3.1). |
| `DataCenter.GameName` | Yes | Game identifier string passed in `<game>` to `GetDatacenters` and used to filter `GameSubscription/Game` in Exchange 4 (§3.4). In LOTRO installations this value is `LOTRO`. |
| `DataCenter.DefaultCenter` | No | Default datacenter name in `GetDatacentersResult` (in LOTRO installations, `LOTRO`). If absent, `DataCenter.GameName` is used. |
| `Product.DocumentFolder` | No | Subdirectory name under the user's documents directory where `UserPreferences.ini` is stored (in LOTRO installations, `The Lord of the Rings Online`; see `docs/specs/install-layout.md` §8). |
| `Launcher.DocumentFolder` | No | Subdirectory name for launcher logs and state (in LOTRO installations, `Launcher`). |

If the configuration file cannot be found, is not well-formed XML, or lacks either `Launcher.DataCenterService.GLS` or `DataCenter.GameName`, the login flow fails before any network activity (`F02_CONFIG_INVALID` in §5.3).

### 2.2 Language and locale resolution <a id="s2-2-language"></a>

The set of languages installed in `<InstallRoot>` is determined by scanning `<InstallRoot>` for archives matching `client_local_<Lang>.dat` (`docs/specs/install-layout.md` §3.1 and §7). The archive filename pattern `client_local_<Lang>.dat`, the `<Lang>` tokens (`English`, `EN_GB`, `FR`, `DE`), and the two-letter feed codes (`en`, `fr`, `de`) appear literally in the installation directory on disk, in feed URLs, and on the `--language` command line, and are required for interoperability:

| `<Lang>` Token (`--language` Argument) | Two-Letter Feed Code (`{0}`) | Language |
|---|---|---|
| `English` | `en` | English (United States) |
| `EN_GB` | `en` | English (United Kingdom) |
| `FR` | `fr` | French |
| `DE` | `de` | German |

The client selects the active `<Lang>` token from the user's configured locale preference (§2.3), falling back to `English` if present, or to the lexicographically first installed `client_local_<Lang>.dat` archive. Before starting a session, the client verifies that `client_local_<Lang>.dat` exists in `<InstallRoot>`; if it is missing, the installation is incomplete or unpatched for that language (`F02_CONFIG_INVALID`).

### 2.3 User preferences (`UserPreferences.ini`) <a id="s2-3-user-prefs"></a>

The launcher and client read optional user preferences from `<Documents>/<Product.DocumentFolder>/` (and, on Wine/Proton installations, the corresponding prefix documents path; see `docs/specs/install-layout.md` §8). When targeting the 64-bit client (`LL`, `IL`), the settings directory is searched in the following filename order, using the first file that exists:

1. `UserPreferences64.ini`
2. `UserPreferences_x64.ini`
3. `UserPreferences.ini`

INI comment lines beginning with either `;` or `#` (after trimming leading whitespace) are ignored (`LL`). All INI filenames above, section headers (`[Launcher]`, `[General]`), and key names (`GameClientType`, `LastLocale`, `LastWorld`) appear literally in the user preferences INI files on disk and are required for interoperability:

| Section | Key | Meaning |
|---|---|---|
| `[Launcher]` | `GameClientType` | Integer selector used when launching the official binary: `1` = 32-bit (`GameClient.WIN32.Filename`), `2` = 32-bit legacy (`GameClient.WIN32Legacy.Filename`), `3` = 64-bit (`GameClient.WIN64.Filename`). |
| `[Launcher]` | `LastLocale` | Last selected locale tag (mapped to the `<Lang>` token in §2.2). |
| `[General]` | `LastWorld` | Optional display name of the last world selected by the user (`LL`). |

> **Note:** Tessera never writes to the official `UserPreferences*.ini` files unless explicitly configured by the user, and stores its own optional account preferences in Tessera's own configuration directory.

### 2.4 Client version, data version, and compression dictionary bootstrap (`netver`, `didver`, and `0x10000001`/`0x10000002`) <a id="s2-4-version-ids"></a>

During client initialization (`BIN-Q3`, `BIN-Q4`, `OBS`), the client prepares two artefacts for the UDP session layer (`docs/specs/official-session.md`): (1) a combined client/data version string (`"netver:<netver_string>; didver:<didver_string>"`) sent on the wire both in `SessionAuthHeader` (`0x00010000`, prefixed with `"180804_"` as `"180804_netver:<netver_string>; didver:<didver_string>"`, `docs/specs/official-session.md` §6.2) and in the post-handshake `ClientVersionMessage` (`0x0000012C`, `docs/specs/official-session.md` §9.3), and (2) two Huffman/prefix-tree compression dictionaries resolved via the dictionary sub-map obtained from Master DID Map root key `0x00000003` ( outgoing dictionary key `0x10000002` $\rightarrow$ `0x0E010000` and incoming dictionary key `0x10000001` $\rightarrow$ `0x0E010002` $\rightarrow$ `0x0E010001`), whose root-key resolution, `0x0E01xxxx` record layout, bit-tree algorithm, and 16-byte dictionary identifier GUID (`codecUuid`) negotiation in `ConnectChallengeHeader` (`0x00040000`) and `ConnectAcceptHeader` (`0x00080000`) are canonically specified in [`docs/specs/official-session.md`](official-session.md) §4.1.2–§4.1.3 and §6.3–§6.4.

#### 2.4.1 Construction and runtime discovery of `"netver:<netver_string>; didver:<didver_string>"` <a id="s2-4-1-netver-didver"></a>

The combined version string transmitted on the UDP wire has the exact ASCII format (with no trailing newline or NUL inside the string):

`netver:<netver_string>; didver:<didver_string>`

The wire prefix literals `"180804_"`, `"netver:"`, and `"; didver:"` appear on the UDP wire in `SessionAuthHeader` (`0x00010000`) and `ClientVersionMessage` (`0x0000012C`), and the `VS_VERSION_INFO` `StringFileInfo` key `"CompatibiltyVersion"` (spelled literally without the second `i` in the PE version resource; or `"CompatibilityVersion"`) appears literally in the documented PE version resource (`.rsrc`) of `<InstallRoot>/x64/lotroclient64.exe` and `<InstallRoot>/lotroclient.exe` (`BIN-Q3`, `OBS`). They are documented here because they define on the wire or locate in the PE version resource on disk the `netver:<netver_string>; didver:<didver_string>` payload required by `SessionAuthHeader` (`0x00010000`) and the UDP version exchange (`0x0000012C`).

1. **Origin and clean discovery of `<netver_string>` (`BIN-Q3`, `OBS`; `docs/decisions/0042-m8-remediation-decisions.md` item 1, confirming `O1` of `docs/decisions/0041-incident-m8-spec-contamination.md`)**:
   - `<netver_string>` is the client's network protocol compatibility version token (for example, a 4-digit decimal string such as `"9386"` in build `4906.0071.1794.4035`, or a 4-part dotted decimal string such as `"1808.0052.0882.4202"` in earlier builds). It is stored in the PE version resource (`.rsrc`) of `<InstallRoot>/x64/lotroclient64.exe` (and `<InstallRoot>/lotroclient.exe`) and may also be overridden on the command line via `--netver`; it is **distinct from** the 4-part PE `FileVersion` / `ProductVersion` fields (`docs/specs/install-layout.md` §2.2). (`lotroclient64.exe` itself imposes no 24-character limit when copying `--netver` or `"CompatibiltyVersion"`; a clean-room implementation accepts `1` to `64` printable ASCII characters as an input sanity bound.)
   - **Normative rule (`docs/decisions/0042-m8-remediation-decisions.md` item 1):** Tessera **never** scans or disassembles the client binary's code section, uses **no** hard-coded data-section signature, and uses **no** diagnostic or error format string as an anchor. No anchor, signature, or scan heuristic is kept. Instead, `<netver_string>` is obtained from the following three sources in strict order:
     1. **Step 1 — Value supplied by the user (setting or command line)**:
        - If the user supplies a non-empty network version string via configuration setting or command-line option (`--netver`), validate that the trimmed value consists of `1` to `64` printable ASCII characters (`0x21..0x7E`, excluding `;` and whitespace) and use it directly as `<netver_string>`.
     2. **Step 2 — String table of the executable's PE version resource (`.rsrc` `VS_VERSION_INFO` `StringFileInfo`, a documented PE structure)**:
        - Otherwise, open `<InstallRoot>/x64/lotroclient64.exe` (falling back to `<InstallRoot>/lotroclient.exe` if the 64-bit binary is absent) read-only, validate the DOS (`"MZ"`) and PE (`"PE\0\0"`) headers, parse the documented PE `VS_VERSION_INFO` version resource in the `.rsrc` section, and look up the `StringFileInfo` string-table entry `"CompatibiltyVersion"` (or `"CompatibilityVersion"`). In observed builds (`OBS`), the entry value is the NUL-terminated UTF-16LE `<netver_string>` (e.g., `"9386"`). Convert the UTF-16LE characters to ASCII and validate that the trimmed value consists of `1` to `64` printable ASCII characters (`0x21..0x7E`, excluding `;` and whitespace).
     3. **Step 3 — Ask the user (or fail with `F02_CONFIG_INVALID`)**:
        - Otherwise, the user is asked to supply `<netver_string>` (or, if no value can be obtained from the user, the login flow aborts before any network activity with `F02_CONFIG_INVALID`).

2. **Origin and runtime extraction of `<didver_string>` (`BIN-Q3`, `OBS`)**:
   - When opening `.dat` archives (`client_gamelogic.dat`, `client_general.dat`, etc.), the client reads the first `0x1A8` (`424`) bytes of the archive from file offset `0x000` and reads the `0x68`-byte superblock header at **file offset `0x140..0x1A7`** (`"BT\x00\x00"`, see `docs/specs/dat-container.md` §3.2).
   - Within that superblock header, **file offset `0x17C..0x18F` (`20` bytes)** holds the archive's data-version descriptor:

| File Offset in `.dat` | Offset in Descriptor | Size (bytes) | Type | Byte Order | Meaning |
|---|---|---|---|---|---|
| `0x17C` | `+0x00` | `4` | `u32` | Little-endian | `didver` GUID `Data1` (formatted as 8 uppercase hexadecimal digits). |
| `0x180` | `+0x04` | `2` | `u16` | Little-endian | `didver` GUID `Data2` (formatted as 4 uppercase hexadecimal digits). |
| `0x182` | `+0x06` | `2` | `u16` | Little-endian | `didver` GUID `Data3` (formatted as 4 uppercase hexadecimal digits). |
| `0x184` | `+0x08` | `2` | `u8[2]` | Raw byte order | `didver` GUID `Data4[0..1]` (formatted as 4 uppercase hexadecimal digits). |
| `0x186` | `+0x0A` | `6` | `u8[6]` | Raw byte order | `didver` GUID `Data4[2..7]` (formatted as 12 uppercase hexadecimal digits). |
| `0x18C` | `+0x10` | `4` | `u32` | Little-endian | Archive version integer (`1100` in base archives, `17139` in expansion cell/map archives). |

   - All `client_*.dat` archives in a consistent LOTRO installation carry the identical 16-byte GUID at file offset `0x17C..0x18B`. Although the 20-byte superblock descriptor at `0x17C..0x18F` includes the 4-byte archive version integer at `0x18C..0x18F`, the wire version string sent in the UDP session (`BIN-Q3`) **omits** that version integer and appends **only `<guid_string>` (the 36-character uppercase ASCII GUID `XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX`)** after `"; didver:"`.
   - **Clean-room runtime extraction algorithm**: Read bytes `0x17C..0x18B` (`16` bytes) directly from `<InstallRoot>/client_general.dat` (or `<InstallRoot>/client_gamelogic.dat`), verify that the superblock magic at `0x140..0x143` is `42 54 00 00` (`"BT\0\0"`), and format the 16-byte mixed-endian GUID as `XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX` (`36` uppercase ASCII characters).

#### 2.4.2 Wire placement in `SessionAuthHeader` (`0x00010000`) vs `ClientVersionMessage` (`0x0000012C`) <a id="s2-4-2-loginrequest-wire"></a>

1. **Transport `SessionAuthHeader` (`0x00010000`, [`docs/specs/official-session.md`](official-session.md) §6.2)**:
   - On the primary login/world UDP connection, the transport-level `SessionAuthHeader` (`0x00010000`) begins at offset `0x00` with `netVersion` serialized as a 4-tier `VariableU32` ([`docs/specs/official-session.md`](official-session.md) §5.6.1) byte-length prefix $L_{\text{auth}}$ followed by the $L_{\text{auth}}$ raw ASCII bytes of `"180804_"` (`7` bytes) concatenated with the runtime version stamp string `"netver:<netver_string>; didver:<didver_string>"` (`"180804_netver:<netver_string>; didver:<didver_string>"`, with **no** trailing NUL and **no** 4-byte alignment padding; for a 4-character `<netver_string>` and 36-character `<didver_string>`, $L_{\text{auth}} = 7 + 56 = 63$ = `0x3F`, occupying `64` bytes in total), followed immediately at offset `1 + L_auth` by `u32 LE authBlockLen` (the exact unpadded byte length of `authBlock`) and the serialized `authBlock` carrying the account/subscription name and GLS ticket ([`docs/specs/official-session.md`](official-session.md) §6.2–§6.2.1). (On the secondary `ChatSession` UDP connection, `netVersion` is the 6-byte `"180804"` string with `VariableU32` length `0x06`, occupying `7` bytes; [`docs/specs/official-session.md`](official-session.md) §6.2.)
2. **Post-handshake `ClientVersionMessage` (`0x0000012C`, [`docs/specs/official-session.md`](official-session.md) §9.3)**:
   - Immediately after `ConnectChallengeHeader` (`0x00040000`) and `ConnectAcceptHeader` (`0x00080000`) complete the transport handshake and the session enters State `5` (`Connected`), the server sends `VersionRequestMessage` (`0x0000012F`) and the client responds with `ClientVersionMessage` (`0x0000012C`), followed by `VersionStatusMessage` (`0x0000012D`) and optional `LoginQueueStatusMessage` (`0x00000130`) in [`docs/specs/official-session.md`](official-session.md) §9.3, character selection in [`docs/specs/official-messages-world.md`](official-messages-world.md) §3, and `WorldReferralHeader` (`0x00000800`) in [`docs/specs/official-session.md`](official-session.md) §6.5 upon world entry.
   - In `ClientVersionMessage` (`0x0000012C`), the runtime client version string `"netver:<netver_string>; didver:<didver_string>"` constructed in §2.4.1 (without the `"180804_"` prefix) is serialized at payload offset `+0x00` (after the 1-byte `envelopeFlags`) as a length-prefixed ASCII string: its byte length $L$ is encoded as a `VariableU32` ([`docs/specs/official-session.md`](official-session.md) §5.6.1; for a 56-byte string, a single byte `0x38`) followed by $L$ raw ASCII bytes with **no** trailing NUL:

| Offset in `ClientVersionMessage` (`0x0000012C`) Inner Payload | Size (bytes) | Type | Byte Order | Meaning |
|---|---|---|---|---|
| `+0x00` | `1`, `2`, or `4` | `VariableU32` | Per [`docs/specs/official-session.md`](official-session.md) §5.6.1 | Byte length $L$ of `"netver:<netver_string>; didver:<didver_string>"` (excluding NUL), encoded as a `VariableU32` ([`docs/specs/official-session.md`](official-session.md) §5.6.1). For a 4-character `<netver_string>` and 36-character `<didver_string>`, $L = 7 + 4 + 9 + 36 = 56$ (`0x38`), which encodes as the single byte `0x38`. |
| `+0x01` (when $L \le 127$) | $L$ (`56` when `<netver>` is 4 chars) | `u8[L]` | Raw ASCII | Raw ASCII bytes of `"netver:<netver_string>; didver:<didver_string>"` with **no** trailing NUL byte ([`docs/specs/official-session.md`](official-session.md) §9.3). |

#### 2.4.3 Bootstrap `.dat` compression dictionary records (`0x10000002` and `0x10000001`) <a id="s2-4-3-compression-dicts"></a>

Immediately after building the version string, the client resolves the two compression dictionary enumerator keys via root key `0x00000003` in the Master DID Map of `<InstallRoot>/client_general.dat` (`BIN-Q4`, `OBS`), then the returned dictionary sub-map:

- Outgoing dictionary key `0x10000002` $\rightarrow$ `0x0E010000` (outgoing client-to-server compression dictionary).
- Incoming dictionary key `0x10000001` $\rightarrow$ `0x0E010002` (deferred redirect stub) $\rightarrow$ `0x0E010001` (incoming server-to-client decompression dictionary).

[`docs/specs/official-session.md` §4.1.2–§4.1.3 and §6.3–§6.4](official-session.md) is the single canonical specification of the dictionary sub-map resolved from root key `0x00000003` in `client_general.dat` (reached via superblock offset `0x170..0x173` $\rightarrow$ `0x28000000` entry `0x00000003`), the enumerator keys `0x10000002` ($\rightarrow$ `0x0E010000` outgoing compression dictionary) and `0x10000001` ($\rightarrow$ `0x0E010002` deferred redirect stub $\rightarrow$ `0x0E010001` incoming decompression dictionary), the `0x0E01xxxx` record layout and redirect rule, the bit-tree compression/decompression algorithm, and the 16-byte `codecUuid` (`+0x10..+0x1F`) negotiation in `ConnectChallengeHeader` (`0x00040000` payload offset `+0x1C..+0x2B`) and `ConnectAcceptHeader` (`0x00080000` payload offset `+0x08..+0x17`).

---

## 3. Launcher-Side Service Exchanges <a id="s3-launcher-exchanges"></a>

All network exchanges in §3 use endpoints discovered at runtime in §2.1 or returned by earlier exchanges. Every SOAP 1.1 action URI, XML namespace, XML element and attribute name, `.NET` `appSettings` key, HTTP `application/x-www-form-urlencoded` field name, and command-line template placeholder documented in §3.1–§3.6 appears literally on the HTTP/SOAP wire, in the remote configuration XML, or on the client executable's command line, and is required for interoperability. Per `docs/decisions/0042-m8-remediation-decisions.md` item 3, the XML namespace URI (`http://www.turbine.com/SE/GLS`) and the `SOAPAction` header values (`"http://www.turbine.com/SE/GLS/GetDatacenters"`, `"http://www.turbine.com/SE/GLS/LoginAccount"`, `"http://www.turbine.com/SE/GLS/RefreshTicket"`) are XML/SOAP protocol identifiers that are never contacted as network endpoints and may appear in code as constants only in the SOAP message encoder.

- **Official launcher URL schemes (`OBS`, `BIN-Q5`, `LL`):** The pass-8 research reports preserving the scheme of runtime URLs. This does not override Tessera’s HTTPS-first policy in §2 and §6.2.

> **OPEN:** Which login-chain steps does the current official launcher actually send over plain HTTP? A per-step observation of the owner’s unmodified launcher, or sufficient offline evidence for each step, remains needed; static URL and string presence alone does not establish the scheme of a request.
- **`User-Agent` and HTTP proxy behavior:** Pass-8 launcher observations are attributed to `BIN-Q5`; the in-game `RefreshTicket` transport observations are attributed separately to `BIN-Q2`. These source scopes must not be interchanged.

> **OPEN:** Confirm the current launcher’s per-request `User-Agent` and proxy selection, and whether either is required by the service. Static header strings do not establish transmission or service validation. Tessera may use its own agent identifier and configured proxy without claiming parity on these unconfirmed details.

### 3.1 Exchange 1 — Datacenter and world discovery (`GetDatacenters`) <a id="s3-1-get-datacenters"></a>

The first network exchange queries the GLS datacenter directory SOAP 1.1 service (`Launcher.DataCenterService.GLS`) to discover the authentication server URL, remote launcher configuration URL, patch server address, and the list of game worlds (`P4`).

#### Request parameters

| Parameter | Value |
|---|---|
| Endpoint URL | Runtime value of `Launcher.DataCenterService.GLS` from `lotro.launcherconfig` (§2.1). |
| HTTP Method | `POST` |
| `Content-Type` header | `text/xml; charset=utf-8` |
| `SOAPAction` header | `"http://www.turbine.com/SE/GLS/GetDatacenters"` (a SOAP 1.1 protocol action identifier that is never contacted as a network endpoint; `docs/decisions/0042-m8-remediation-decisions.md` item 3). |
| Request encoding | UTF-8 XML with XML 1.0 declaration `<?xml version="1.0" encoding="utf-8"?>` |

#### Request XML structure

| XPath (relative to root) | Namespace | Content / Meaning |
|---|---|---|
| `/soap:Envelope` | `http://schemas.xmlsoap.org/soap/envelope/` | Standard SOAP 1.1 envelope (also declares `xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"` and `xmlns:xsd="http://www.w3.org/2001/XMLSchema"`). |
| `/soap:Envelope/soap:Body/GetDatacenters` | `http://www.turbine.com/SE/GLS` | Operation element (the namespace URI is an XML protocol identifier that is never contacted as a network endpoint; `docs/decisions/0042-m8-remediation-decisions.md` item 3). |
| `/soap:Envelope/soap:Body/GetDatacenters/game` | `http://www.turbine.com/SE/GLS` | Game identifier string from `DataCenter.GameName` in `lotro.launcherconfig` (e.g. `LOTRO`). |

#### Response XML structure (`HTTP 200 OK`)

The response is a SOAP 1.1 envelope in XML namespace `http://www.turbine.com/SE/GLS` (an XML protocol identifier, never contacted) under `/soap:Envelope/soap:Body/GetDatacentersResponse/GetDatacentersResult`:

| Element Path (under `GetDatacentersResult`) | Type | Cardinality | Meaning |
|---|---|---|---|
| `Datacenter` | Complex | `1..*` | Datacenter entry. Select the `Datacenter` whose `Name` equals `DataCenter.GameName` (or `DataCenter.DefaultCenter`). If no `Datacenter` matches, fail with `F03_SERVICE_UNAVAILABLE`. |
| `Datacenter/Name` | String | `1` | Datacenter identifier (matches `DataCenter.GameName`). |
| `Datacenter/AuthServer` | String (URL) | `1` | Runtime HTTP or HTTPS URL of the GLS authentication service (apply §2 address policy), used for `LoginAccount` (§3.4) and passed to the client as `--authserverurl` for `RefreshTicket` (§4.3). |
| `Datacenter/PatchServer` | String (`host:port`) | `1` | Patch server address (used by the patcher; not contacted during login). |
| `Datacenter/LauncherConfigurationServer` | String (URL) | `1` | Runtime URL of the remote launcher configuration XML document, fetched in Exchange 2 (§3.2). |
| `Datacenter/SupportURL` | String (URL) | `0..1` | Optional customer-support URL associated with the datacenter (`LL`). |
| `Datacenter/Worlds/World` | Complex | `0..*` | Descriptor for each game world (shard) in this datacenter. |
| `Datacenter/Worlds/World/Name` | String | `1` | Display name of the world (e.g., `[EN-RP] ...` or `... [OLD]`). Worlds whose trimmed `Name` ends case-insensitively with `[old]` are sorted after active worlds; within each group, worlds are sorted by `Order` ascending and then alphabetically by `Name` (`LL`, `OL`). |
| `Datacenter/Worlds/World/LoginServerUrl` | String | `1` | Legacy field. Official servers return the literal string `(invalid service specified)`. Implementations **must not** use this field as the login server address; the actual login server `host:port` is obtained in Exchange 3 (§3.3). |
| `Datacenter/Worlds/World/ChatServerUrl` | String (`host:port`) | `1` | Chat server `host:port` string passed to the client via `--chatserver`. |
| `Datacenter/Worlds/World/StatusServerUrl` | String (URL) | `1` | Runtime URL queried in Exchange 3 (§3.3) to obtain the world's live status, login server address, and queue URL. |
| `Datacenter/Worlds/World/Order` | Integer | `1` | Server-assigned display order integer. |
| `Datacenter/Worlds/World/Language` | String | `0..1` | Optional language tag for the world. |

---

### 3.2 Exchange 2 — Remote launcher configuration (`LauncherConfigurationServer`) <a id="s3-2-launcher-config-server"></a>

Immediately after Exchange 1 succeeds, the client fetches the remote launcher configuration XML document (`P5`).

#### Request parameters

| Parameter | Value |
|---|---|
| Endpoint URL | Runtime value of `Datacenter/LauncherConfigurationServer` from Exchange 1 (§3.1). |
| HTTP Method | `GET` |

#### Response XML structure (`HTTP 200 OK`)

The response is a UTF-8 `.NET` configuration XML document (`<configuration>/<appSettings>/<add key="..." value="..." />`). The keys relevant to login, queue handling, and client launch are:

| `<add key="...">` Name | Required | Meaning |
|---|---|---|
| `GameClient.WIN64.Filename` | No | Relative path from `<InstallRoot>` to the 64-bit client executable (e.g., `x64\lotroclient64.exe`; backslashes normalized to platform path separators). |
| `GameClient.WIN32.Filename` | No | Relative path to the 32-bit client executable. |
| `GameClient.WIN32Legacy.Filename` | No | Relative path to the legacy 32-bit client executable. |
| `GameClient.Filename` | No | Fallback relative path to the client executable when `GameClient.WIN32.Filename` is absent. |
| `GameClient.WIN32.ArgTemplate` | Yes (or `GameClient.ArgTemplate`) | Command-line argument template with `{PLACEHOLDER}` tokens used to construct the client's startup arguments (§3.6). If `GameClient.WIN32.ArgTemplate` is absent, fall back to `GameClient.ArgTemplate`. |
| `GameClient.Arg.authserverurl` | No | Override URL for `--authserverurl`; if absent, `Datacenter/AuthServer` from Exchange 1 is used. |
| `GameClient.Arg.glsticketlifetime` | No | Integer lifetime in seconds of GLS tickets, passed to `--glsticketlifetime` (typically `21600` seconds = 6 hours). Defaults to `21600` if absent. |
| `GameClient.Arg.bugurl` | No | URL value substituted for `{BUGURL}` in the argument template. |
| `GameClient.Arg.supporturl` | No | URL value substituted for `{SUPPORTURL}` in the argument template. |
| `GameClient.Arg.supportserviceurl` | No | URL value substituted for `{SUPPORTSERVICEURL}` in the argument template (used by in-game `NewTicket` / `AccountHasUpdatedTickets` support SOAP calls). |
| `GameClient.Arg.crashreceiver` | No | URL value substituted for `{CRASHRECEIVER}` in the argument template. |
| `GameClient.Arg.DefaultUploadThrottleMbps` | No | Numeric string substituted for `{UPLOADTHROTTLE}` in the argument template. |
| `GameClient.HighResPatchArg` | No | Argument appended when high-resolution textures (`client_highres.dat`) are missing or out of date (typically `" --HighResOutOfDate"`). |
| `GameClient.ResetGraphicsArg` | No | Argument appended when safe display settings are requested (typically `" --safe"`). |
| `WorldQueue.LoginQueue.URL` | Yes | Runtime HTTP or HTTPS URL of the login queue service used in Exchange 5 (§3.5); apply §2 address policy. |
| `WorldQueue.TakeANumber.Parameters` | Yes | `.NET` format template (`{0}`, `{1}`, `{2}`) for the `TakeANumber` POST body in Exchange 5 (§3.5): `command=TakeANumber&subscription={0}&ticket={1}&ticket_type=GLS&queue_url={2}`. |
| `WorldQueue.LeaveQueue.Parameters` | No | `.NET` format template (`{0}`, `{1}`, `{2}`) for the optional `LeaveQueue` POST body in Exchange 5 (§3.5): `command=LeaveQueue&subscription={0}&context={1}&ticket_type=GLS&queue_url={2}`. |
| `WorldQueue.LoginQueue.RefreshSeconds` | No | Queue poll interval in seconds read by `LotroLauncher.exe` (`BIN-Q5`); defaults to `20` seconds (`20,000` ms) when absent. |
| `WorldQueue.PollTimer` | No | Legacy queue poll interval in seconds present in remote configuration XML (e.g., `120`; read by legacy `TurbineLauncher.exe` / `OL` / `PL`). |
| `WorldQueue.Threshold.WaitTimeMultiplier` | No | Multiplier for queue wait-time display estimation. |
| `WorldQueue.Threshold.Medium` | No | Queue threshold in seconds for medium wait indicator. |
| `WorldQueue.Threshold.Long` | No | Queue threshold in seconds for long wait indicator. |
| `URL.NewsFeed` | No | Optional newsfeed URL template where `{0}` is replaced with the two-letter language code (`en`, `fr`, `de`). |
| `URL.Alerts` | No | Optional alerts URL template where `{0}` is replaced with the two-letter language code (`en`, `fr`, `de`). |

---

### 3.3 Exchange 3 — Per-world status query (`World/StatusServerUrl`) <a id="s3-3-world-status"></a>

To check whether a selected world is online, verify subscription billing-role permissions, and discover its active login server `host:port` and queue URL, the client queries the world's `StatusServerUrl` (`P6`). When refreshing status across multiple worlds for display in a world list, requests should be issued sequentially with a `10 s` per-world timeout (`LL`) so the status server is not rate-limited or overwhelmed.

#### Request parameters (`docs/decisions/0042-m8-remediation-decisions.md` item 3)

| Parameter | Value |
|---|---|
| Endpoint URL | Runtime value of `World/StatusServerUrl` for the selected world from Exchange 1 (§3.1, reached via `Launcher.DataCenterService.GLS` from the user's install `*.launcherconfig`). |
| HTTP Method | `GET` |
| No Fallback URL Rule (`docs/decisions/0042-m8-remediation-decisions.md` item 3) | Tessera hard-codes no service URL, host, or path fragment. The per-world status query contacts only the runtime `World/StatusServerUrl` returned by `GetDatacenters`. If `World/StatusServerUrl` is missing, malformed, or returns an HTTP error (such as `HTTP 404 Not Found`) or an empty body, there is **no** fallback URL rewrite and the status query fails (`F06_WORLD_CLOSED` / `F03_SERVICE_UNAVAILABLE`), reporting that the configuration/datacenter response provides no usable status URL. |

Apply the HTTPS-first and pre-request-failure-only HTTP retry rule of §2 and §6.2 to this status query, including world-list queries. An HTTP service error or empty service answer does not permit an HTTP retry.

#### Response XML structure (`HTTP 200 OK`)

The response is a UTF-8 XML document with root element `<Status>` and no XML namespace. Child element names under `<Status>` should be matched **case-insensitively** (`LL`):

| Element Name (under `<Status>`) | Type | Required | Meaning |
|---|---|---|---|
| `name` | String | No | World name string. |
| `loginservers` | String | Yes | Semicolon-separated (`;`) list of login server `host:port` endpoints (e.g., `login1.example.invalid:19000;login2.example.invalid:19000;`). Split on `;`, trim whitespace, and discard empty segments and `(invalid service specified)`; the **first non-empty segment** is the world's `login_server` (`host:port`). If no non-empty segment exists, the world is unavailable (`F06_WORLD_CLOSED`). |
| `queueurls` | String | Yes | Semicolon-separated (`;`) list of world queue URLs (e.g., `http://queue1.example.invalid:8080/queue;http://queue2.example.invalid:8080/queue;` or `;`). Split on `;`, trim whitespace, and discard empty segments. **Queue URL selection rule (`LL`):** Some worlds list internal RFC 1918 private IPv4 literals (`10.0.0.0/8`, `172.16.0.0/12` i.e. `172.16.*`–`172.31.*`, or `192.168.0.0/16`) alongside or instead of public hostnames because `queue_url` is passed as an identifier parameter to `WorldQueue.LoginQueue.URL` (§3.5) rather than dialed directly by the client. When selecting the primary `queue_url` from the non-empty segments, prefer the first segment whose host is **not** an RFC 1918 private IPv4 address (`://10.`, `://192.168.`, `://172.16.`..`://172.31.`); if all non-empty segments use RFC 1918 addresses, fall back to the first non-empty segment (`LL`). If no non-empty segment exists, `queue_url` is `None`. |
| `nowservingqueuenumber` | Hex string | No | Current queue number being served, formatted with a `0x` prefix (e.g., `0x0000010A`), parsed as an unsigned 32-bit base-16 integer. |
| `lastassignedqueuenumber` | Hex string | No | Last queue number assigned, formatted with a `0x` prefix, parsed as an unsigned 32-bit base-16 integer. |
| `allow_billing_role` | String | No | Comma-separated (`,`) list of billing role tokens permitted to enter this world (e.g. VIP/subscription tiers on legendary worlds). Empty or absent means all billing roles are permitted. |
| `deny_billing_role` | String | No | Comma-separated (`,`) list of billing role tokens denied entry to this world. |
| `allow_admin_role` | String | No | Comma-separated (`,`) list of administrative role tokens. |
| `deny_admin_role` | String | No | Comma-separated (`,`) list of denied administrative role tokens. |
| `world_full` | Boolean string (`true`/`false`) | No | Indicates whether the world is at capacity. |
| `wait_hint` | Float string | No | Estimated queue wait hint in seconds. |
| `logintiers` | String | No | Semicolon-separated login tier identifiers. |
| `farmid` | Integer string | No | Server farm identifier. |
| `lastupdated` | String | No | Timestamp of last status update. |
| `StatusServerResult` | String | No | Result status string. |

#### Billing-role access check (`allow_billing_role` / `deny_billing_role`)

Before entering the world queue or launching the client for a selected `GameSubscription` (§3.4), if the subscription contains a non-empty `ProductTokens` set $P$:

1. Parse `allow_billing_role` by splitting on `,` into set $A$ (empty if element is absent or empty).
2. Parse `deny_billing_role` by splitting on `,` into set $D$ (empty if element is absent or empty).
3. If $A$ is non-empty and $P \cap A = \emptyset$, the subscription lacks the required billing tier for this world (e.g., a non-VIP account attempting to log into a VIP-only legendary world); abort with `F07_WORLD_ROLE_DENIED`.
4. If $D$ is non-empty, $P \cap D \neq \emptyset$, and $A = \emptyset$, abort with `F07_WORLD_ROLE_DENIED`.

---

### 3.4 Exchange 4 — Account authentication (`LoginAccount`) <a id="s3-4-login-account"></a>

When the user submits their account username and password, the client authenticates against the GLS `AuthServer` SOAP 1.1 endpoint discovered in Exchange 1 (`P7`).

#### Request parameters

| Parameter | Value |
|---|---|
| Endpoint URL | Runtime value of `Datacenter/AuthServer` from Exchange 1 (§3.1). |
| HTTP Method | `POST` |
| `Content-Type` header | `text/xml; charset=utf-8` |
| `SOAPAction` header | `"http://www.turbine.com/SE/GLS/LoginAccount"` (a SOAP 1.1 protocol action identifier that is never contacted as a network endpoint; `docs/decisions/0042-m8-remediation-decisions.md` item 3). |
| Request encoding | UTF-8 XML with XML 1.0 declaration `<?xml version="1.0" encoding="utf-8"?>` |

#### Request XML structure

| XPath (relative to root) | Namespace | Content / Meaning |
|---|---|---|
| `/soap:Envelope` | `http://schemas.xmlsoap.org/soap/envelope/` | SOAP 1.1 envelope. |
| `/soap:Envelope/soap:Body/LoginAccount` | `http://www.turbine.com/SE/GLS` | Operation element (the namespace URI is an XML protocol identifier that is never contacted as a network endpoint; `docs/decisions/0042-m8-remediation-decisions.md` item 3). |
| `/soap:Envelope/soap:Body/LoginAccount/username` | `http://www.turbine.com/SE/GLS` | User-entered account login name, XML-escaped (`&`, `<`, `>`, `"`, `'` escaped as XML entities). |
| `/soap:Envelope/soap:Body/LoginAccount/password` | `http://www.turbine.com/SE/GLS` | User-entered account password, XML-escaped. |
| `/soap:Envelope/soap:Body/LoginAccount/additionalInfo` | `http://www.turbine.com/SE/GLS` | Empty element (`<additionalInfo></additionalInfo>`). |

#### Response XML structure (`HTTP 200 OK`)

A successful authentication returns `/soap:Envelope/soap:Body/LoginAccountResponse/LoginAccountResult` in XML namespace `http://www.turbine.com/SE/GLS` (an XML protocol identifier, never contacted):

| Element Path (under `LoginAccountResult`) | Type | Cardinality | Meaning |
|---|---|---|---|
| `Subscriptions/GameSubscription` | Complex | `0..*` | List of game subscriptions across all Turbine/Standing Stone Games titles attached to this account. |
| `Subscriptions/GameSubscription/Game` | String | `1` | Game identifier (e.g., `LOTRO`, `DDO`). |
| `Subscriptions/GameSubscription/Name` | String | `1` | **Subscription identifier token** (typically a 25-character alphanumeric string or account ID). **Crucial interoperability rule:** This `Name` field — *not* the user-entered `username` — is the account identifier passed as `{0}` (`subscription`) to the world login queue (§3.5) and as `-a` / `{SUBSCRIPTION}` to the game client (§3.6). |
| `Subscriptions/GameSubscription/Description` | String | `0..1` | Human-readable label for the subscription, displayed to the user when disambiguating multiple subscriptions. |
| `Subscriptions/GameSubscription/ProductTokens/string` | String | `0..*` | Billing/product entitlement tokens (e.g., expansion and VIP tokens) used for the world billing-role check in §3.3. |
| `Subscriptions/GameSubscription/CustomerServiceTokens/string` | String | `0..*` | Customer service entitlement tokens. |
| `Subscriptions/GameSubscription/Status` | String | `0..1` | Subscription status string (e.g., `Active`). |
| `Subscriptions/GameSubscription/ExpirationDate` | String | `0..1` | Expiration date string. |
| `Subscriptions/GameSubscription/NextBillingDate` | String | `0..1` | Next billing date string. |
| `Subscriptions/GameSubscription/PendingCancelDate` | String | `0..1` | Pending cancellation date string. |
| `Subscriptions/GameSubscription/AutoRenew` | String | `0..1` | Auto-renew flag string (`TRUE`/`FALSE`). |
| `Subscriptions/GameSubscription/BillingSystemTime` | String | `0..1` | Server billing timestamp string. |
| `Subscriptions/GameSubscription/AdditionalInfo` | String | `0..1` | Optional additional metadata XML/text. |
| `Ticket` | String | `1` | Non-empty opaque ASCII Base64-encoded GLS session ticket of variable character length $L$ (observed `144` to `256` ASCII Base64 characters, containing alphanumeric characters, `+`, `/`, and `=`), which is carried **verbatim as a 4-tier `VariableU32` byte-length prefix followed by the $L$ raw ASCII Base64 characters (without Base64 decoding)** inside `extraBlob` of `SessionAuthHeader` (`0x00010000`, §4.2.1; [`docs/specs/official-session.md`](official-session.md) §6.2.1). |

#### Subscription filtering and selection

From `LoginAccountResult/Subscriptions`, the client filters `GameSubscription` elements whose `<Game>` text equals `DataCenter.GameName` from `lotro.launcherconfig` (e.g., `LOTRO`):

1. **Zero matching subscriptions**: The user's credentials are valid for the GLS billing system, but the account has no subscription for this game (for example, a DDO-only account). Transition to failure `F05_NO_GAME_SUBSCRIPTION`.
2. **Exactly one matching subscription**: Automatically select that `GameSubscription`.
3. **Multiple matching subscriptions**: Present a selection prompt listing each matching subscription by its `Description` (falling back to `Name` if `Description` is empty). If the user previously selected a subscription `Name` for this account, pre-select it. Once the user confirms a subscription, use its `Name` and `ProductTokens` for all subsequent steps.

#### SOAP Fault and error cases (`LL`, `OL`, `PL`)

When authentication fails, `AuthServer` returns a SOAP 1.1 `<soap:Fault>` (matched case-insensitively as `Fault` or `fault`) inside `<soap:Body>` (frequently with HTTP status `500 Internal Server Error`). Implementations extract the human-readable fault message from `<faultstring>` or, if `<faultstring>` is absent, `<Message>` (`LL`):

| Fault Element / Condition | Example / Pattern | Failure Class & Sub-Reason |
|---|---|---|
| `faultcode` | `BIS` (Billing Information System) or `soap:Server` / `soap:Client` | — |
| `faultstring` or `Message` containing `"maintenance"` (case-insensitive), or `HTTP 503 Service Unavailable` | Authentication service down for scheduled maintenance | `F03_SERVICE_UNAVAILABLE` (maintenance mode; `LL`) |
| `faultstring` or `Message` containing both `"account"` and `"lock"` (case-insensitive) | Account locked due to failed login attempts or administrative hold | `F04_AUTH_REJECTED` (account locked sub-reason; `LL`) |
| `faultstring` or `Message` containing `"no subscriber"`, `"password"`, `"username"`, or any other `BIS` fault | Non-existent account, username/password too short, or wrong credentials | `F04_AUTH_REJECTED` (invalid credentials; `LL`, `OL`) |
| `detail/result` | Hex HRESULT string (e.g., `0x8004C002`) | `F04_AUTH_REJECTED` |
| Other HTTP 4xx/5xx or network timeout | Non-SOAP error response or socket error | `F03_SERVICE_UNAVAILABLE` |

#### Multi-factor authentication

The GLS `LoginAccount` SOAP 1.1 service does not define or support a multi-factor authentication (MFA) challenge/response step (`Confirmed` via `LL`, `OL`, `PL`, `GLS`, `REDDO`).

---

### 3.5 Exchange 5 — World login queue (`WorldQueue.LoginQueue.URL`: `TakeANumber` and `LeaveQueue`) <a id="s3-5-world-queue"></a>

#### Entry condition (`BIN-Q5`, `OBS`, `LL`, `OL`, `DDOL`)

After Exchange 3 (`World/StatusServerUrl`), Exchange 4 (`LoginAccount`), subscription selection, and the billing-role check (§3.3) have all succeeded (`P8`):

- Let `queue_url` be the primary queue URL selected from `<queueurls>` in the selected world's `<Status>` XML (§3.3).
- **When `queue_url` is empty (`None`, e.g. `<queueurls>;</queueurls>` or `<queueurls></queueurls>`):** Both `LotroLauncher.exe` (`BIN-Q5`, `OBS`) and third-party launchers (`LL`, `OL`) **skip Exchange 5 (`TakeANumber`) entirely** and proceed directly to client launch / UDP session hand-off (§3.6, §4.2).
- **Mandatory queue registration when `queue_url` is non-empty (`BIN-Q5`, `LL`, `OL`, `DDOL`):** Whenever `queue_url` is non-empty, `LotroLauncher.exe` executes `TakeANumber` against `WorldQueue.LoginQueue.URL` and waits until `QueueNumber ≤ NowServingNumber` before launching the client — even when `<Status>` reported `lastassignedqueuenumber ≤ nowservingqueuenumber`. (Once the UDP login connection completes its version check `0x0000012D`, the login server may also send zero or more `0x00000130` `LoginQueueStatusMessage` envelopes with `logonResult = 0x80041464` while in an in-band UDP queue or `logonResult = 0x00000000` when admitted; see [`docs/specs/official-session.md`](official-session.md) §9.3.)

#### Request parameters (`TakeANumber` and `LeaveQueue`)

| Parameter | Value |
|---|---|
| Endpoint URL | Runtime value of `WorldQueue.LoginQueue.URL` from Exchange 2 (§3.2). |
| HTTP Method | `POST` |
| `Content-Type` header | `application/x-www-form-urlencoded` |
| `TakeANumber` POST body | Constructed from `WorldQueue.TakeANumber.Parameters` (`command=TakeANumber&subscription={0}&ticket={1}&ticket_type=GLS&queue_url={2}`), where:<br>• `{0}` = selected `GameSubscription/Name` (ASCII alphanumeric; percent-encode if any non-unreserved characters exist)<br>• `{1}` = **URL-encoded** (`application/x-www-form-urlencoded` / RFC 3986: `+` $\rightarrow$ `%2B`, `/` $\rightarrow$ `%2F`, `=` $\rightarrow$ `%3D`, space $\rightarrow$ `+` or `%20`) `LoginAccountResult/Ticket`<br>• `{2}` = **URL-encoded** (`:` $\rightarrow$ `%3A`, `/` $\rightarrow$ `%2F`, `?` $\rightarrow$ `%3F`, `&` $\rightarrow$ `%26`, `=` $\rightarrow$ `%3D`) `queue_url` from `<Status>/<queueurls>`. |
| `LeaveQueue` POST body | Optional when the user cancels while waiting in the queue (`BIN-Q5`: `LotroLauncher.exe` does not send `LeaveQueue` when the user cancels or closes the launcher, simply stopping its poll timer and aborting locally; third-party launchers `OL`/`DDOL` optionally send it). If sent, constructed from `WorldQueue.LeaveQueue.Parameters` (`command=LeaveQueue&subscription={0}&context={1}&ticket_type=GLS&queue_url={2}`), where `{0}` is `GameSubscription/Name`, `{1}` is the `ContextNumber` from the last `TakeANumber` response, and `{2}` is the URL-encoded `queue_url`. |

> **Important encoding and template-substitution invariants (`LL`, `OL`):**
> 1. Because `Ticket` is base64-encoded and contains `+`, `/`, and `=`, failing to percent-encode `{1}` in the `application/x-www-form-urlencoded` body corrupts `+` into spaces (`0x20`) on the server and causes `TakeANumber` to fail with a negative `HResult`.
> 2. When substituting `{0}`, `{1}`, and `{2}` into `WorldQueue.TakeANumber.Parameters`, implementations **must not** pass the already-percent-encoded strings through a sequential positional formatter that interprets `%` sequences (such as `%2F` or `%3D`) as format specifiers (`LL`).

#### Response XML structure (`HTTP 200 OK`)

The response is a UTF-8 XML document with root element `<Result>` and no XML namespace:

| Element Name (under `<Result>`) | Type | Required | Meaning |
|---|---|---|---|
| `Command` | String | No | Echoes the command name (e.g., `TakeANumber`). |
| `HResult` | Hex string | Conditional | 32-bit HRESULT formatted with a `0x` prefix (e.g., `0x00000000` or `0x80004005`), parsed as a base-16 unsigned 32-bit integer `hr`. **Sign-bit rule (`LL`):** Per standard Windows `HRESULT` conventions, the operation failed if and only if bit 31 (the sign bit, mask `0x80000000`) is set (`F08_QUEUE_ERROR`); any non-negative `hr` with bit 31 clear is treated as success. **Fallback tolerance (`LL`):** If `<HResult>` is absent from `<Result>`, the response is still treated as successful (`hr = 0`) provided both `<QueueNumber>` and `<NowServingNumber>` are present and valid. |
| `QueueName` | String | No | Internal queue name. |
| `QueueNumber` | Hex string | Yes (on success) | The user's assigned queue number with a `0x` prefix (e.g., `0x0000002B`), parsed as base-16 unsigned integer. |
| `NowServingNumber` | Hex string | Yes (on success) | The queue number currently being admitted with a `0x` prefix (e.g., `0x0000002A`), parsed as base-16 unsigned integer. |
| `LoginTier` | Integer/Hex string | No | Login priority tier assigned to the subscription. |
| `ContextNumber` | Hex string | No | Queue context handle with a `0x` prefix, used if `LeaveQueue` is called. |
| `WaitTimeSamples` | Complex | No | Historical wait time samples by tier. |

#### Queue polling loop and ready condition

1. Parse `HResult` as base-16 integer `hr` (defaulting to `0` if `<HResult>` is absent and both `<QueueNumber>` and `<NowServingNumber>` are present). If bit 31 (the sign bit, mask `0x80000000`) of `hr` is set (such as `0x80004005` `E_FAIL`, which occurs if the world is closed, the ticket is invalid, or the client is polling too rapidly), abort or back off (`F08_QUEUE_ERROR`).
2. Parse `QueueNumber` ($q$) and `NowServingNumber` ($s$) as base-16 integers.
3. **Ready condition**: If $q \le s$, the user has reached the front of the queue; exit the queue loop immediately and proceed to §3.6.
4. **Waiting condition**: If $q > s$, the number of players ahead of the user is $q - s$. Display the queue position $q - s$ to the user, wait for the poll interval (`LotroLauncher.exe` reads `WorldQueue.LoginQueue.RefreshSeconds` with a default of `20` seconds when absent, while legacy configs also carry `WorldQueue.PollTimer` = `120`; a clean-room implementation clamps the poll interval to `[5, 120]` seconds with a `20`-second default so absent or extreme values neither busy-loop nor stall, as polling faster than `2` seconds risks `0x80004005` rate-limiting), and repeat the `TakeANumber` POST unless the user cancels (in which case stop the local poll timer and transition to `SF_ABORTED`; `LotroLauncher.exe` sends no `LeaveQueue` POST on cancel, though a clean-room implementation may optionally send `LeaveQueue` if `ContextNumber` is known).

---

### 3.6 Launcher-to-client hand-off <a id="s3-6-client-handoff"></a>

When launching the official client binary (or populating the internal session configuration in Tessera), the launcher substitutes runtime values into `GameClient.WIN32.ArgTemplate` (or `GameClient.ArgTemplate`) from Exchange 2 (§3.2) (`P9`).

The standard remote template has the form:

`-a {SUBSCRIPTION} -h {LOGIN} --glsticketdirect "{GLS}" --chatserver "{CHAT}" --language {LANG} --rodat on --gametype {GAMETYPE} --authserverurl "{AUTHSERVERURL}" --glsticketlifetime {GLSTICKETLIFETIME} --supporturl "{SUPPORTURL}" --supportserviceurl "{SUPPORTSERVICEURL}" --bugurl "{BUGURL}"`

| Placeholder Token | Alternate / Legacy Alias (`LL`) | Runtime Source | Meaning |
|---|---|---|---|
| `{SUBSCRIPTION}` | — | `GameSubscription/Name` from Exchange 4 (§3.4) | Subscription identifier passed to `-a` (`--account`). |
| `{LOGIN}` | — | First non-empty `host:port` in `<Status>/<loginservers>` from Exchange 3 (§3.3) | World login server UDP `host:port` passed to `-h` (`--host`). |
| `{GLS}` | — | `LoginAccountResult/Ticket` from Exchange 4 (§3.4) | Raw (unencoded) GLS session ticket string passed to `--glsticketdirect`. |
| `{CHAT}` | — | `World/ChatServerUrl` from Exchange 1 (§3.1) | Chat server `host:port` passed to `--chatserver`. |
| `{LANG}` | `{LANGUAGE}` | Selected `<Lang>` token from §2.2 (`English`, `EN_GB`, `FR`, `DE`) | Client language name passed to `--language`. |
| `{GAMETYPE}` | `{PRODUCT}` | `DataCenter.GameName` from `lotro.launcherconfig` (§2.1) | Game type identifier (`LOTRO`) passed to `--gametype`. |
| `{AUTHSERVERURL}` | `{AUTHSERVER}` | `GameClient.Arg.authserverurl` from Exchange 2, or `Datacenter/AuthServer` from Exchange 1 | GLS `AuthServer` SOAP URL passed to `--authserverurl` for in-game ticket refresh (§4.3). |
| `{GLSTICKETLIFETIME}` | — | `GameClient.Arg.glsticketlifetime` from Exchange 2 (default `21600`) | Ticket lifetime in seconds passed to `--glsticketlifetime`. |
| `{SUPPORTURL}` | — | `GameClient.Arg.supporturl` from Exchange 2 (or `Datacenter/SupportURL` from Exchange 1) | In-game support browser URL. |
| `{SUPPORTSERVICEURL}` | — | `GameClient.Arg.supportserviceurl` from Exchange 2 | In-game support SOAP endpoint URL. |
| `{BUGURL}` | — | `GameClient.Arg.bugurl` from Exchange 2 | In-game bug-report URL. |
| `{CRASHRECEIVER}` | — | `GameClient.Arg.crashreceiver` from Exchange 2 | Crash upload receiver URL (if present in template). |
| `{UPLOADTHROTTLE}` | — | `GameClient.Arg.DefaultUploadThrottleMbps` from Exchange 2 | Crash upload throttle (if present in template). |
| `{HIGHRES}` | — | `"1"` when high-resolution textures are enabled, `"0"` when disabled (`LL`) | High-resolution texture toggle token (if present in template). |
| `{USERDIR}` | — | Optional user preferences / settings directory path (`LL`) | Custom user directory passed to `--userdir` (if present in template). |

When splitting an expanded template into an `argv` argument vector for process execution (`LL`):
1. Surrounding double quotes (`"..."`) on individual argument tokens are stripped.
2. Any token that still contains an unreplaced `{...}` placeholder (because an optional `GameClient.Arg.*` key was absent from Exchange 2) is omitted so literal `{PLACEHOLDER}` strings are never passed to the client.
3. If `client_highres.dat` is absent in `<InstallRoot>` (or out of date), the launcher appends `GameClient.HighResPatchArg` (`--HighResOutOfDate`).
4. When launching an external executable, the process working directory (`cwd`) **must** be `<InstallRoot>` (even when the executable path is `x64/lotroclient64.exe`); when launching under Wine/Proton on Linux, `SteamAppId=212500` and `SteamGameId=212500` may be set in the environment (`LL`, `IL`).

---

## 4. Game Client Ticket Intake, Session Hand-off, and Ticket Refresh <a id="s4-client-ticket-and-refresh"></a>

This section specifies what the game client (`x64/lotroclient64.exe`, confirmed via `BIN-Q1`–`BIN-Q3`) does with the hand-off parameters at startup and during an active session. Every command-line flag, Windows registry hive and value name (`HKEY_CURRENT_USER`, `"Ticket"`), and SOAP 1.1 action URI and XML element name in §4.1–§4.3 appears literally on the client executable's command line, in the OS registry hand-off contract, or on the HTTP/SOAP wire, and is required for interoperability.

### 4.1 Command-line and registry ticket intake <a id="s4-1-ticket-intake"></a>

#### Login-related command-line options (`BIN-Q1`, `LL`)

| Short Flag | Long Flag | Value Type | Meaning |
|---|---|---|---|
| `-a` | `--account` | Wide string | Subscription name (`GameSubscription/Name`) to log on with. |
| `-h` | `--host` | Narrow string | Target login server address string: accepts a colon-delimited string with zero colons (`remoteHost`), one colon (`remoteHost:port`), or two colons (`localHost:remoteHost:port`, where the first segment before the first `:` specifies the local bind address/interface, the second segment specifies the remote login server host, and the third segment specifies the remote UDP port; any angle brackets in `<localHost:remoteHost:port>` help text are metasyntactic placeholders, not literal `<` `>` characters on the command line; [`docs/specs/official-session.md`](official-session.md) §3.2–§3.3). |
| `-p` | `--port` | `u32` (`1..65535`) | Target login server remote UDP port (when not embedded as `:port` in `-h`; [`docs/specs/official-session.md`](official-session.md) §3.3). |
| — | `--outport` | `u32` (`1..65535`) | Local outgoing UDP bind port (optional; resolved alongside `Net.BindInterface` and `Net.ComputeUniquePort` in `UserPreferences.ini` and any `localHost:` prefix in the two-colon `localHost:remoteHost:port` form of `-h`, per [`docs/specs/official-session.md`](official-session.md) §3.2). |
| `-z` | `--glsticketdirect` | Wide string | Direct ASCII Base64 GLS authentication ticket string of variable character length $L$ (observed `144` to `256` characters), serialized **verbatim as a 4-tier `VariableU32` byte-length prefix followed by the $L$ raw ASCII Base64 characters (without Base64 decoding)** in `extraBlob` of `SessionAuthHeader` (`0x00010000`, §4.2.1; [`docs/specs/official-session.md`](official-session.md) §6.2.1). |
| — | `--glsticket` | Narrow string | Windows registry subkey under `HKEY_CURRENT_USER` from which to load the `"Ticket"` value if `--glsticketdirect` is not used (also serialized verbatim as a `VariableU32`-prefixed ASCII Base64 string in `extraBlob` of `SessionAuthHeader` `0x00010000`, §4.2.1; [`docs/specs/official-session.md`](official-session.md) §6.2.1). |
| — | `--authserverurl` | Wide string | GLS `AuthServer` URL for refreshing the GLS ticket. |
| — | `--glsticketlifetime` | `u32` (seconds) | Lifetime of GLS tickets in seconds. |
| — | `--netver` | Narrow string | Optional network compatibility version override (`<netver_string>`, §2.4.1; `lotroclient64.exe` imposes no 24-character limit when copying `--netver`, while a clean-room implementation accepts `1` to `64` printable ASCII characters). |
| — | `--language` | Narrow string | Language to run the client in (`English`, `EN_GB`, `FR`, `DE`). |
| — | `--chatserver` | Narrow string | Chat server `host:port` to connect to. Supplying `--chatserver` also enables the chat subsystem (`--usechat` = `1`). If chat is enabled and `--chatserver` is empty after argument parsing, the client copies the `--host` (`-h`) string into `--chatserver` ([`docs/specs/official-session.md`](official-session.md) §3.3; [`docs/specs/official-messages-play.md`](official-messages-play.md) §6.1.1). |
| — | `--usechat` | Flag (boolean) | Enables chat server connection (also enabled automatically when `--chatserver` is supplied; [`docs/specs/official-messages-play.md`](official-messages-play.md) §6.1.1). |
| — | `--voiceoff` | Flag (boolean) | Disables the in-game voice subsystem (default: voice enabled; [`docs/specs/official-messages-play.md`](official-messages-play.md) §6.1.1). |
| — | `--rodat` | Ignored argument | Retained only for backward compatibility; ignored by the client. |
| — | `--gametype` | Wide string | Game type identifier (`LOTRO`) used for support URLs. |
| — | `--highres` | Narrow string (`0`/`1`) | High-resolution texture toggle token (`{HIGHRES}`) when present in `ArgTemplate` (`LL`). |
| — | `--HighResOutOfDate` | Flag (boolean) | Appended via `GameClient.HighResPatchArg` when `client_highres.dat` is absent or out of date. |
| — | `--safe` | Flag (boolean) | Appended via `GameClient.ResetGraphicsArg` when safe display settings are requested. |
| — | `--userdir` | Path string | Optional custom user preferences / settings directory path (`{USERDIR}`, `LL`). |
| — | `--bugurl` | Wide string (URL) | In-game bug-report URL (`{BUGURL}`). |
| — | `--supporturl` | Wide string (URL) | In-game support browser URL (`{SUPPORTURL}`). |
| — | `--supportserviceurl` | Wide string (URL) | In-game support SOAP endpoint URL (`{SUPPORTSERVICEURL}`). |
| `-f` | `--franchise` | Narrow string | Franchise name. |
| `-u` | `--user` | Wide string | Optional character name to auto-select on login. |

#### Ticket intake precedence (`BIN-Q1`)

At client startup, the client resolves its initial GLS ticket using the following strict precedence:

1. **Primary path — Direct CLI ticket (`--glsticketdirect` + `--authserverurl`)**:
   - Checked first: both `--authserverurl` and `--glsticketdirect` must be non-empty strings.
   - The `--authserverurl` and `--glsticketdirect` arguments are converted from UTF-16 wide strings to UTF-8/ASCII (a direct character-encoding conversion, **not** URL percent-decoding and **not** Base64-decoding).
   - The raw ASCII Base64 ticket string and `--account` (`-a`) subscription identifier are passed to the UDP session transport layer (§4.2–§4.2.1, [`docs/specs/official-session.md`](official-session.md) §6.2), and periodic `RefreshTicket` scheduling (§4.3) is armed with the 32-bit `--glsticketlifetime` value (in seconds).
2. **Fallback path — Windows registry key (`--glsticket`)**:
   - Used only if `--authserverurl` or `--glsticketdirect` is empty and `--glsticket` is non-empty.
   - Opens `HKEY_CURRENT_USER\<--glsticket>` with read access and reads the string value named `"Ticket"` into a 4096-byte buffer (maximum string length `4095` bytes), then closes the registry key.
   - If the registry read succeeds, the raw ASCII Base64 ticket string and `--account` (`-a`) subscription identifier are passed to the UDP session transport layer (§4.2–§4.2.1). Periodic `RefreshTicket` requests (§4.3) do not run when `--authserverurl` or `--glsticketdirect` is omitted.
   - If the registry key or `"Ticket"` value is missing or unreadable, startup fails (`F09_TICKET_INACCESSIBLE`).

---

### 4.2 Hand-off to the UDP session layer (`docs/specs/official-session.md`) <a id="s4-2-session-handoff"></a>

Once ticket intake (§4.1) and version/data initialization (§2.4) succeed, the login layer hands the following runtime values to the UDP session transport layer specified in `docs/specs/official-session.md` (`P11`):

| Parameter | Source | Value Handed to `docs/specs/official-session.md` |
|---|---|---|
| Login server endpoint | `-h` (`--host`) and `-p` (`--port`) CLI options | Target UDP IPv4/hostname and port from the first non-empty entry of `<Status>/<loginservers>` (§3.3). `-h` (`--host`) accepts a colon-delimited string with zero colons (`remoteHost`), one colon (`remoteHost:port`), or two colons (`localHost:remoteHost:port`, where the first segment before the first `:` specifies the local bind address/interface, the second segment specifies the remote login server host, and the third segment specifies the remote UDP port; any angle brackets in `<localHost:remoteHost:port>` help text are metasyntactic placeholders, not literal `<` `>` characters on the command line), and `-p` (`--port`) sets the remote UDP port when not embedded in `-h` ([`docs/specs/official-session.md`](official-session.md) §3.2–§3.3). |
| Local UDP bind port | `--outport` CLI option (alongside `Net.BindInterface` and `Net.ComputeUniquePort` in `UserPreferences.ini`, and any `localHost:` prefix in the two-colon `localHost:remoteHost:port` form of `-h`) | Optional local UDP bind port (`1..65535`), or `0` (resolved alongside `Net.BindInterface` and `Net.ComputeUniquePort` in `UserPreferences.ini` as specified in [`docs/specs/official-session.md`](official-session.md) §3.2). |
| Subscription / Account ID | `-a` (`--account`) CLI option | Selected `GameSubscription/Name` string from `LoginAccountResult` (§3.4). |
| GLS Session Ticket | `--glsticketdirect` (or `--glsticket` registry fallback) | Raw ASCII Base64 ticket string from `LoginAccountResult/Ticket` (and updated on `RefreshTicket`), of variable character length $L$ (observed `144` to `256` ASCII Base64 characters), serialized **verbatim as a 4-tier `VariableU32` byte-length prefix followed by the $L$ raw ASCII Base64 characters (never Base64-decoded)** inside `extraBlob` of `SessionAuthHeader` (`0x00010000`, §4.2.1; [`docs/specs/official-session.md`](official-session.md) §6.2.1). |
| Client & Data Version (`client_version_string`) | Version builder (§2.4.1–§2.4.2) | Runtime ASCII string `"netver:<netver_string>; didver:<didver_string>"` (where `<didver_string>` is the 36-char uppercase GUID from `.dat` superblock `0x17C..0x18B`), serialized both in `SessionAuthHeader` (`0x00010000`, prefixed by `"180804_"` as `"180804_netver:<netver_string>; didver:<didver_string>"` with a `VariableU32` byte-length prefix and no trailing NUL or padding, [`docs/specs/official-session.md`](official-session.md) §6.2) and at offset `+0x00` of `ClientVersionMessage` (`0x0000012C`, as `"netver:<netver_string>; didver:<didver_string>"` with a `VariableU32` byte-length prefix, sent in response to `VersionRequestMessage` `0x0000012F` and followed by `VersionStatusMessage` `0x0000012D`, optional `LoginQueueStatusMessage` `0x00000130`, character selection in [`docs/specs/official-messages-world.md`](official-messages-world.md) §3, and `WorldReferralHeader` `0x00000800` upon world entry; [`docs/specs/official-session.md`](official-session.md) §6.2, §6.5, and §9.3). |
| Compression Dictionaries & `codecUuid` | Dictionary sub-map resolved through root key `0x00000003` in `client_general.dat` (§2.4.3; [`docs/specs/official-session.md`](official-session.md) §4.1.2–§4.1.3) | Resolved outgoing dictionary key `0x10000002` ($\rightarrow$ `0x0E010000`) and incoming dictionary key `0x10000001` ($\rightarrow$ `0x0E010002` $\rightarrow$ `0x0E010001`) dictionaries and their 16-byte `codecUuid` (`+0x10..+0x1F`), loaded and negotiated via `ConnectChallengeHeader` (`0x00040000` offset `+0x1C..+0x2B`) and `ConnectAcceptHeader` (`0x00080000` offset `+0x08..+0x17`) as canonically specified in [`docs/specs/official-session.md`](official-session.md) §4.1.2–§4.1.3 and §6.3–§6.4. |
| Client Language | `--language` CLI option | Selected `<Lang>` string (`English`, `EN_GB`, `FR`, `DE`). |

#### 4.2.1 `SessionAuthHeader` (`0x00010000`) ticket and `authType` selection rule <a id="s4-2-1-session-auth-mode"></a>

In `SessionAuthHeader` (`0x00010000`, [`docs/specs/official-session.md`](official-session.md) §6.2.1):

1. **Primary login/world UDP connection (`OBS`, `BIN-Q1`, `BIN-Q3`)**:
   - Always sets **`authType = 0x00000001`**.
   - Whenever a GLS ticket is present (`--glsticketdirect` or `--glsticket`), sets **`authFlags = 0x00000004` (`HasExtraBlob`)**, sets `accountOrTicketStr` to the UTF-16LE `--account` (`-a`) subscription identifier (`4-tier VariableU32` UTF-16 code-unit count + UTF-16LE bytes), and sets `extraBlob` to `u32` LE `extraBlobLen` followed by a 4-tier `VariableU32` byte-length prefix $L$ and the $L$ **raw ASCII Base64 ticket bytes** (never Base64-decoded; for a `144`-character Base64 ticket, `VariableU32` `0x80 0x90` + `144` ASCII bytes = `146` bytes, so `extraBlobLen = 146` = `0x00000092`).
   - When no GLS ticket is supplied, sets `authFlags = 0x00000000` (carrying only `accountOrTicketStr`).
2. **Secondary Chat Server (`--chatserver`, `ChatSession`) UDP connection (`OBS`, `BIN-Q1`)**:
   - Sets **`authType = 0x40000002`** and, when a GLS ticket is present, **`authFlags = 0x00000004` (`HasExtraBlob`)** with the identical `accountOrTicketStr` and `extraBlob` (`u32` LE `extraBlobLen` + `VariableU32`-prefixed raw ASCII Base64 ticket string; [`docs/specs/official-session.md`](official-session.md) §6.2.1 and [`docs/specs/official-messages-play.md`](official-messages-play.md) §6.6.2).

All subsequent UDP packet framing, connection handshake messages (`SessionAuthHeader` `0x00010000`, `ConnectChallengeHeader` `0x00040000`, `ConnectAcceptHeader` `0x00080000`, `VersionRequestMessage` `0x0000012F`, `ClientVersionMessage` `0x0000012C`, `VersionStatusMessage` `0x0000012D`, `LoginQueueStatusMessage` `0x00000130`, and `WorldReferralHeader` `0x00000800`), cryptographic key exchange, encryption, and world-server redirection are specified in `docs/specs/official-session.md`.

---

### 4.3 In-game GLS ticket refresh (`RefreshTicket`) <a id="s4-3-refresh-ticket"></a>

While an authenticated game session is active, the client periodically sends the SOAP 1.1 `RefreshTicket` request (`BIN-Q2`, `GLS`) to `--authserverurl` so that services consuming the GLS ticket (such as customer-support SOAP `NewTicket` requests to `--supportserviceurl`, which embed `<glsTicket>...</glsTicket>`, and any subsequent UDP `SessionAuthHeader` `0x00010000` or `WorldReferralHeader` `0x00000800` reconnect handshake in [`docs/specs/official-session.md`](official-session.md) §6.2 and §6.5) continue to hold a valid ticket across multi-hour sessions.

#### Timing parameters

| Parameter | Source / Initial Value | Meaning |
|---|---|---|
| Endpoint URL | `--authserverurl` | Runtime HTTP or HTTPS URL of the GLS authentication service (`Datacenter/AuthServer` or `GameClient.Arg.authserverurl`); apply §2 address policy. |
| Nominal ticket lifetime $L$ | `--glsticketlifetime` (`u32`, seconds) | Nominal ticket lifetime in seconds (typically `21600` seconds = 6 hours). When $L$ equals `0`, periodic `RefreshTicket` requests are disabled. |
| Ticket installation time $t_0$ | Monotonic timestamp (seconds) | Timestamp at which the current ticket was installed (at startup or upon receiving a valid `<RefreshTicketResult>`). |
| Next refresh offset $\Delta t_{\text{next}}$ | Initialized to $0.5 \times L$ (seconds) | Elapsed-age threshold; transmission occurs only at a timer check strictly after this offset. |
| Attempt count $k$ | Initialized to `0` (`0..3`) | Number of `RefreshTicket` requests transmitted for the current ticket. |

#### Scheduling and exponential half-life backoff rule (`BIN-Q2`)

At monotonic time $t$:

1. **Preconditions**:
   - If the nominal lifetime $L$ is `0`, or either the active ticket string or the subscription/account name is empty, or `3` attempts have already been transmitted for the current ticket ($k > 2$), no request is sent.
   - If the elapsed ticket age $\Delta t = t - t_0$ does not exceed $\Delta t_{\text{next}}$ ($\Delta t \le \Delta t_{\text{next}}$), no request is sent.
2. **Schedule update and transmission when $\Delta t > \Delta t_{\text{next}}$**:
   - Advance the next refresh offset $\Delta t_{\text{next}}$ by half of the remaining lifetime at time $t$:
     $$\Delta t_{\text{next}} \leftarrow \Delta t_{\text{next}} + 0.5 \times \bigl(L - (t - t_0)\bigr)$$
   - Increment the attempt count:
     $$k \leftarrow k + 1$$
   - When $k = 3$ (the 3rd and final attempt for the current ticket), record a warning that failure of this final refresh will prevent submitting in-game support tickets.
   - Transmit the `RefreshTicket` SOAP 1.1 request.

For the standard lifetime $L = 21600\text{ s}$ (6 hours), the nominal threshold fractions approach one half, three quarters and seven eighths of the lifetime when timer sampling delay tends to zero. These are not exact transmission times: equality sends nothing, and each next threshold depends on the actual sampled age. A synthetic failed-attempt schedule with $t_0 = 0$ and checks at the following ages gives:

| Attempt | Sampled age (s) | Remaining lifetime (s) | Updated next offset (s) |
|---|---|---|---|
| 1 | 10800.5 | 10799.5 | 16199.75 |
| 2 | 16200.5 | 5399.5 | 18899.5 |
| 3 (final) | 18900.5 | 2699.5 | 20249.25 (no further attempt: cap reached) |

#### `RefreshTicket` SOAP 1.1 request (`BIN-Q2`, `GLS`)

| Parameter | Value |
|---|---|
| Endpoint URL | `--authserverurl` (`Datacenter/AuthServer` or `GameClient.Arg.authserverurl`) |
| HTTP Method | `POST` |
| `Content-Type` header | `text/xml; charset=utf-8` |
| `SOAPAction` header | `"http://www.turbine.com/SE/GLS/RefreshTicket"` (a SOAP 1.1 protocol action identifier that is never contacted as a network endpoint; `docs/decisions/0042-m8-remediation-decisions.md` item 3). |
| HTTP Timeout | `30` seconds |
| Request XML Structure | SOAP 1.1 `/soap:Envelope/soap:Body/RefreshTicket` in XML namespace `http://www.turbine.com/SE/GLS` (an XML protocol identifier, never contacted), containing a single child element `<ticket>` whose text is the current raw GLS ticket string. |

#### `RefreshTicket` SOAP 1.1 response handling (`BIN-Q2`)

1. **Success path (`HTTP 200 OK` with `<RefreshTicketResult>`)**:
   - The response body is parsed for the character data of `/soap:Envelope/soap:Body/RefreshTicketResponse/RefreshTicketResult` in XML namespace `http://www.turbine.com/SE/GLS` (an XML protocol identifier, never contacted).
   - When `<RefreshTicketResult>` is present, the returned ticket string replaces the active GLS ticket, the ticket installation timestamp $t_0$ resets to the current time $t$, the next refresh offset $\Delta t_{\text{next}}$ resets to $0.5 \times L$, and the attempt count $k$ resets to `0`.
   - If the refreshed ticket is non-empty and differs from the previous ticket string, the updated `(ticket, account_name)` pair is supplied to the UDP session transport layer for any subsequent `SessionAuthHeader` (`0x00010000`) or `WorldReferralHeader` (`0x00000800`) handshake ([`docs/specs/official-session.md`](official-session.md) §6.2, §6.5).
2. **Failure path (HTTP/network error, SOAP Fault, or missing `<RefreshTicketResult>`)**:
   - If the HTTP request fails, returns a SOAP `Fault`, or omits `<RefreshTicketResult>`, the refresh attempt fails.
   - The existing ticket remains active, and $k$ and $\Delta t_{\text{next}}$ retain the values updated when the request was sent so that the next retry (when $k \le 2$) occurs at the scheduled backoff time.

---

## 5. End-to-End State Machine, Timeouts, Retries, and Failure Classes <a id="s5-state-machine"></a>

### 5.1 State machine transitions <a id="s5-1-states"></a>

| State | Entry Condition | Action | Outgoing Transitions |
|---|---|---|---|
| `S1_LOCAL_DISCOVERY` | User selects an official server installation or launches official login mode. | Locate and parse `lotro.launcherconfig` (§2.1); scan `client_local_<Lang>.dat` (§2.2); read optional `UserPreferences.ini` (§2.3). | • Valid config & language archive present $\rightarrow$ `S2_GET_DATACENTERS`<br>• Missing/corrupt config or missing language `.dat` $\rightarrow$ `SF_FAILED(F02_CONFIG_INVALID)` |
| `S2_GET_DATACENTERS` | Local config loaded. | Send `GetDatacenters` SOAP 1.1 request to `Launcher.DataCenterService.GLS` (§3.1). | • `HTTP 200` & matching `Datacenter` found $\rightarrow$ `S3_GET_LAUNCHER_CONFIG`<br>• Network/HTTP/SOAP error or no matching `Datacenter` $\rightarrow$ `SF_FAILED(F03_SERVICE_UNAVAILABLE)` |
| `S3_GET_LAUNCHER_CONFIG` | `Datacenter` descriptor obtained. | Send `GET` to `Datacenter/LauncherConfigurationServer` (§3.2). Populate world list in UI. | • `HTTP 200` & valid XML $\rightarrow$ `S4_AWAIT_CREDENTIALS_AND_WORLD`<br>• Network/HTTP/XML error $\rightarrow$ `SF_FAILED(F03_SERVICE_UNAVAILABLE)` |
| `S4_AWAIT_CREDENTIALS_AND_WORLD` | Remote launcher config and world list loaded. | Prompt user for account `username`, `password`, and target `World`. | • User submits non-empty credentials and world $\rightarrow$ `S5_LOGIN_ACCOUNT`<br>• User cancels $\rightarrow$ `SF_ABORTED` |
| `S5_LOGIN_ACCOUNT` | User submitted credentials and selected world. | Send `LoginAccount` SOAP 1.1 request to `Datacenter/AuthServer` (§3.4). Filter `GameSubscription` entries whose `Game` element equals `DataCenter.GameName`. | • `HTTP 200` & 1 matching subscription $\rightarrow$ `S7_QUERY_WORLD_STATUS`<br>• `HTTP 200` & $>1$ matching subscriptions $\rightarrow$ `S6_SELECT_SUBSCRIPTION`<br>• `HTTP 200` & 0 matching subscriptions $\rightarrow$ `SF_FAILED(F05_NO_GAME_SUBSCRIPTION)`<br>• SOAP `Fault` (`BIS`) $\rightarrow$ `SF_FAILED(F04_AUTH_REJECTED)` (return to `S4`)<br>• Network/HTTP error $\rightarrow$ `SF_FAILED(F03_SERVICE_UNAVAILABLE)` |
| `S6_SELECT_SUBSCRIPTION` | `LoginAccountResult` has $>1$ subscriptions for `DataCenter.GameName`. | Prompt user to select one subscription by `Description` / `Name` (§3.4). | • User selects subscription $\rightarrow$ `S7_QUERY_WORLD_STATUS`<br>• User cancels $\rightarrow$ `SF_ABORTED` |
| `S7_QUERY_WORLD_STATUS` | Subscription selected (`GameSubscription/Name` and `ProductTokens` known). | Send `GET` to `World/StatusServerUrl` (§3.3; no fallback URL is constructed on HTTP 404 or empty body, per `docs/decisions/0042-m8-remediation-decisions.md` item 3). Verify `<loginservers>` non-empty and check `allow_billing_role` / `deny_billing_role`. | • World open, role permitted, and `queue_url` non-empty $\rightarrow$ `S8_WORLD_QUEUE`<br>• World open, role permitted, and `queue_url` empty $\rightarrow$ `S9_SESSION_HANDOFF`<br>• `loginservers` empty or status unavailable $\rightarrow$ `SF_FAILED(F06_WORLD_CLOSED)`<br>• Billing role check failed $\rightarrow$ `SF_FAILED(F07_WORLD_ROLE_DENIED)` |
| `S8_WORLD_QUEUE` | World requires login queue (`queue_url` non-empty). | Send `TakeANumber` POST to `WorldQueue.LoginQueue.URL` (§3.5). Poll while `QueueNumber > NowServingNumber`. | • `HResult` non-negative (bit 31 clear) and `QueueNumber ≤ NowServingNumber` $\rightarrow$ `S9_SESSION_HANDOFF`<br>• `HResult` negative (bit 31 set) or HTTP error $\rightarrow$ `SF_FAILED(F08_QUEUE_ERROR)`<br>• User cancels (stop polling; optionally send `LeaveQueue` if `ContextNumber` known) $\rightarrow$ `SF_ABORTED` |
| `S9_SESSION_HANDOFF` | Queue passed (or not required). | Arm periodic `RefreshTicket` schedule with `Ticket`, `AuthServer`, and `glsticketlifetime` (§4.1, §4.3); prepare `netver`/`didver` (§2.4); hand off `(login_server, GameSubscription/Name, Ticket, version_info, language)` to `docs/specs/official-session.md` (§4.2). | • Session transport connected $\rightarrow$ `S10_IN_GAME_TICKET_REFRESH`<br>• Ticket missing/inaccessible $\rightarrow$ `SF_FAILED(F09_TICKET_INACCESSIBLE)` |
| `S10_IN_GAME_TICKET_REFRESH` | Active game session running. | Run the sampled ticket refresh schedule of §4.3: transmit only when the sampled age strictly exceeds the next offset; at most 3 attempts. The half/three-quarter/seven-eighth fractions are nominal limits, not exact transmit times. | • `RefreshTicketResult` received $\rightarrow$ update active ticket, reset $\Delta t_{\text{next}}$ to $0.5 L$ and attempt count $k$ to `0`, remain in `S10_IN_GAME_TICKET_REFRESH`<br>• 3rd attempt fails $\rightarrow$ record support-ticket warning, stop refreshing, session continues until user exits |
| `SF_ABORTED` | User cancelled login or queue. | Zero in-memory credentials and tickets; close any open HTTP sockets. | Terminal state. |
| `SF_FAILED(class)` | Recoverable or fatal error encountered. | Zero in-memory password; display user-facing failure message for `class` (§5.3). | Recoverable classes (`F03`–`F08`) may return to `S4_AWAIT_CREDENTIALS_AND_WORLD`; `F02` returns to main menu. |

### 5.2 Timeouts and retry parameters <a id="s5-2-timeouts"></a>

| Operation | Timeout / Interval | Max Attempts / Retries | Provenance |
|---|---|---|---|
| HTTP/SOAP requests (`GetDatacenters`, `LauncherConfigurationServer`, `World/StatusServerUrl`, `LoginAccount`, `TakeANumber`, `RefreshTicket`) | `30 s` per HTTP request (`10 s` per world when refreshing multiple world statuses sequentially) | `1` attempt per user action (except `RefreshTicket` and queue polling below; no fallback URL retry on `World/StatusServerUrl`, per `docs/decisions/0042-m8-remediation-decisions.md` item 3) | `BIN` (`BIN-Q2`), `LL`, `OL` |
| World login queue polling (`S8` `TakeANumber`) | Poll interval: `WorldQueue.LoginQueue.RefreshSeconds` (`LotroLauncher.exe` default `20 s`; legacy `WorldQueue.PollTimer` is `120 s`; clean-room clamp `[5 s, 120 s]` with `20 s` default, as polling faster than `2 s` risks `0x80004005`) | Indefinite while `HResult` is non-negative (bit 31 clear) and `QueueNumber > NowServingNumber` until user cancels | `BIN` (`BIN-Q5`), `LL`, `OL`, `PL`, `DDOL` |
| In-game `RefreshTicket` (`S10`) | Initial offset is half the lifetime; each attempt requires a sampled age strictly greater than the offset and advances it by half the remaining lifetime at that sample (§4.3) | `3` attempts maximum per ticket (attempt count $k$ of `1`, `2`, `3`) | `BIN` (`BIN-Q2`) |

### 5.3 User-facing failure classes <a id="s5-3-failure-classes"></a>

| Code | Class Name | Trigger Condition | User-Facing Action / Message Summary |
|---|---|---|---|
| `F02_CONFIG_INVALID` | Local Install Configuration Invalid | `lotro.launcherconfig` missing/malformed, required keys missing, `client_local_<Lang>.dat` missing, or `<netver_string>` unavailable after checking user override, `.rsrc` `VS_VERSION_INFO`, and user prompt (§2.4.1). | Report which local file, key, or version string in `<InstallRoot>` is missing or unreadable. |
| `F03_SERVICE_UNAVAILABLE` | GLS / Config Service Unavailable | DNS failure, TLS verification failure, HTTP timeout, or non-200 response from `GetDatacenters`, `LauncherConfigurationServer`, or `AuthServer`. | Report that the login service could not be reached and allow retry. |
| `F04_AUTH_REJECTED` | Authentication Rejected | `LoginAccount` returns a SOAP `Fault` (e.g., `faultcode` equals `"BIS"`). | Display authentication failure (invalid username or password) and prompt for credentials again. |
| `F05_NO_GAME_SUBSCRIPTION` | No Subscription for Game | `LoginAccountResult` contains zero `GameSubscription` entries whose `Game` element equals `DataCenter.GameName`. | Inform the user that the authenticated account has no LOTRO subscription. |
| `F06_WORLD_CLOSED` | World Offline or Closed | `World/StatusServerUrl` is missing, returns an HTTP error (such as HTTP 404) or an empty body (with no fallback URL constructed, per `docs/decisions/0042-m8-remediation-decisions.md` item 3), or `<loginservers>` has no non-empty entry. | Inform the user that the selected world is currently offline or that the configuration provides no usable status URL. |
| `F07_WORLD_ROLE_DENIED` | World Billing Role Denied | Subscription `ProductTokens` fail the `allow_billing_role` / `deny_billing_role` check in §3.3. | Inform the user that the selected subscription tier (e.g., non-VIP) is not permitted on this world. |
| `F08_QUEUE_ERROR` | World Queue Error | `TakeANumber` returns a negative `HResult` (bit 31 set, e.g., `0x80004005`) or malformed XML. | Inform the user that joining the world login queue failed and allow retry. |
| `F09_TICKET_INACCESSIBLE` | GLS Ticket Inaccessible | Client started without `--glsticketdirect`/`--authserverurl` and `--glsticket` registry read failed. | Abort session startup. |
| `F10_REFRESH_EXHAUSTED` | Ticket Refresh Exhausted | All 3 in-game `RefreshTicket` attempts failed. | Record warning that in-game support ticket submission will be unavailable; do not disconnect the active game session. |

---

## 6. Security and Access-Control Notes <a id="s6-security"></a>

### 6.1 Runtime endpoint and credential boundary <a id="s6-1-runtime-discovery"></a>

Per `CLAUDE.md`, `docs/CLEAN_ROOM.md`, `docs/decisions/0034-official-server-compatibility.md` §1, and `docs/decisions/0042-m8-remediation-decisions.md` item 3, Tessera hard-codes no service URL, host, or path fragment: all service URLs, hostnames, ports, and endpoints are discovered at runtime from the user's local installation (`lotro.launcherconfig`) or from server responses, and if a required endpoint is absent there is no fallback URL. No official endpoint, key, certificate, or account data is embedded in the repository, tests, or configuration defaults. The XML namespace URI (`http://www.turbine.com/SE/GLS`) and `SOAPAction` strings are XML/SOAP protocol identifiers that are never contacted as network endpoints and may appear in code as constants only in the SOAP message encoder.

### 6.2 TLS and certificate validation <a id="s6-2-tls"></a>

1. **Observed behavior in official binary and legacy launchers (`BIN-Q2`, `PL`, `DDOL`)**:
   - In `x64/lotroclient64.exe`, the HTTP/SOAP client disables TLS peer certificate verification for `RefreshTicket` and support SOAP requests.
   - Older third-party launchers (`PL`, `DDOL`) also disabled certificate verification or forced legacy TLS 1.0/SSLv23 ciphers to work around historical server configurations.
2. **Mandatory requirement for Tessera**:
   - Because `LoginAccount` transmits the user's plaintext password and returns the bearer `Ticket`, disabling TLS certificate or hostname verification exposes the user's credentials and session ticket to trivial man-in-the-middle interception.
   - Apply the amended decision 0044 address policy in §2: try each supplied HTTP address over HTTPS at the default HTTPS port first, then use the original HTTP address (including any explicit port) only after failure before sending the HTTPS request. A service answer never triggers a plain HTTP retry. Credentials may be included in the permitted HTTP retry. Tessera **must** enforce standard TLS 1.2+ certificate chain validation (against the platform or Mozilla root store, plus any optional user-configured custom CA bundle path if supplied in user settings) and RFC 6125 hostname verification on all `https://` connections (`Datacenter/AuthServer`, `WorldQueue.LoginQueue.URL`, and any `https://` `Launcher.DataCenterService.GLS`, `Datacenter/LauncherConfigurationServer`, or `World/StatusServerUrl` URLs).
   - Tessera **must not** disable certificate verification silently or by default.

### 6.3 Credential, ticket, and personal data handling <a id="s6-3-credentials"></a>

1. **Passwords**:
   - User passwords **must never** be written to configuration files, log files, crash reports, command-line arguments, or environment variables.
   - In memory, the password buffer **must** be zeroed immediately after the `LoginAccount` HTTP request body buffer is transmitted and zeroed.
   - Password persistence is disabled by default; if an optional "remember password" feature is ever offered, it **must** store credentials exclusively in the operating system's encrypted secret store (e.g., Secret Service / Keychain / Windows Credential Manager) and never in plaintext or obfuscated local files.
2. **GLS Session Tickets (`Ticket`) and Subscription Identifiers (`GameSubscription/Name`)**:
   - The `Ticket` string returned by `LoginAccount` and `RefreshTicket` is a bearer credential valid for ~6 hours (`21600` seconds).
   - Both `Ticket` and `GameSubscription/Name` **must** be redacted (`<REDACTED>`) in all log output, diagnostic traces, and error reports.
   - When Tessera runs the login flow and the UDP game session within the same process, it **must** pass `Ticket` and `GameSubscription/Name` directly in memory (§4.2) rather than spawning a child process with `--glsticketdirect` on the OS command line (where other local processes could inspect `/proc/<pid>/cmdline`).
   - Tessera **must not** write the `Ticket` to the Windows registry (`HKEY_CURRENT_USER\...\Ticket`) or to any file on disk.

### 6.4 Access-control and anti-cheat rules <a id="s6-4-stop-conditions"></a>

Per `docs/decisions/0034-official-server-compatibility.md` and `docs/CLEAN_ROOM.md`:

1. **Research scope:** Decision 0034 §2 excludes anti-cheat, anti-tamper, code-signing and access-control circumvention research. No presence or absence finding about these mechanisms is claimed by this login specification.
2. **Binding stop condition**: If Standing Stone Games / Daybreak ever introduces client binary attestation, anti-cheat challenge tokens, or any other technical access control into the GLS authentication or world-queue endpoints, Tessera **must stop** and fail cleanly rather than attempting to emulate, spoof, or bypass the check.

---

## 7. Testable Invariants and Synthetic Test Vectors <a id="s7-test-vectors"></a>

All test vectors in this section use synthetic RFC 2606 `.example.invalid` endpoints, synthetic subscription identifiers, and synthetic base64 tickets containing zero real server addresses or secrets.

### 7.1 Testable invariants <a id="s7-1-invariants"></a>

- **`I1` (Local discovery failure network silence)**: When `S1_LOCAL_DISCOVERY` fails with `F02_CONFIG_INVALID` (missing or malformed `lotro.launcherconfig`, missing `client_local_<Lang>.dat`, or missing `<netver_string>`), zero DNS queries and zero TCP/UDP socket connections are initiated by the login module.
- **`I2` (Bootstrap config discovery order)**: Given a directory containing both `lotro.launcherconfig` and `TurbineLauncher.exe.config`, the parser loads `lotro.launcherconfig` (with or without a UTF-8 BOM `EF BB BF`).
- **`I3` (`LoginServerUrl` placeholder rejection)**: The literal value `(invalid service specified)` in `Datacenter/Worlds/World/LoginServerUrl` is never passed to `-h` or used as a socket target; the target `host:port` is always taken from the first non-empty `;`-delimited entry of `<Status>/<loginservers>`.
- **`I4` (`World/StatusServerUrl` runtime endpoint only; no fallback URL rewrite)**: Per `docs/decisions/0042-m8-remediation-decisions.md` item 3, the per-world status query contacts only the runtime `World/StatusServerUrl` from `GetDatacentersResult`. When `World/StatusServerUrl` is missing, returns `HTTP 404 Not Found`, or returns an empty body, zero fallback URLs are constructed from `Launcher.DataCenterService.GLS` or any hard-coded path fragment, and the query fails immediately with `F06_WORLD_CLOSED`.
- **`I5` (Subscription `Name` vs `username`)**: When `LoginAccount` is called with `<username>alice_login</username>` and returns a matching `GameSubscription` with `<Game>LOTRO</Game>` and `<Name>SUB9876543210ABCDEF012345</Name>`, the value passed to `TakeANumber` (`subscription=`) and to the session layer (`-a`) is `SUB9876543210ABCDEF012345`, never `alice_login`.
- **`I6` (Billing role check)**: If `<Status>/<allow_billing_role>` is `RoleVIP,RoleFounder` and the selected subscription's `ProductTokens` set is `{"SyntheticTokenA", "SyntheticTokenB"}`, the world status check rejects entry with `F07_WORLD_ROLE_DENIED` before calling `TakeANumber`.
- **`I7` (`TakeANumber` ticket percent-encoding)**: When constructing the `application/x-www-form-urlencoded` body for `TakeANumber`, `+`, `/`, and `=` characters in `Ticket` are percent-encoded as `%2B`, `%2F`, and `%3D`, while `--glsticketdirect` (and in-memory session hand-off) receives the raw unencoded `Ticket` string.
- **`I8` (Queue completion boundary)**: In `S8_WORLD_QUEUE`, `QueueNumber` equal to `NowServingNumber` and `QueueNumber` less than `NowServingNumber` (`QueueNumber ≤ NowServingNumber`) both terminate the queue loop immediately without sleeping for a poll interval.
- **`I9` (`RefreshTicket` backoff schedule)**: Given lifetime 21600 s and installation time zero, a check at 10800.0 s sends nothing. With failed attempts sampled at 10800.5, 16200.5 and 18900.5 s, the next offsets become 16199.75, 18899.5 and 20249.25 s respectively (§4.3). After the third attempt no fourth is sent. Actual transmission times depend on timer checks.
- **`I10` (`RefreshTicket` reset on success)**: A successful response sampled at 16200.5 s resets installation time to 16200.5 s, next offset to 10800.0 s and attempt count to zero. A check at 27000.5 s sends nothing; the next request requires a check strictly after 27000.5 s.
- **`I11` (Version string discovery, format, and `SessionAuthHeader` `0x00010000` / `ClientVersionMessage` `0x0000012C` `VariableU32` prefix)**: `<netver_string>` is obtained only from (1) a user-supplied setting or CLI value, (2) the PE `.rsrc` `VS_VERSION_INFO` `StringFileInfo` version resource (`"CompatibiltyVersion"` / `"CompatibilityVersion"`), or (3) a user prompt (`docs/decisions/0042-m8-remediation-decisions.md` item 1), with no code scan or data-section signature. The combined client version string matches the exact ASCII pattern `netver:<netver>; didver:<XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX>` (where `<didver>` is the 36-character uppercase GUID from `.dat` superblock `0x17C..0x18B` without the archive version integer) and is serialized both at offset `+0x00` of `SessionAuthHeader` (`0x00010000`, [`docs/specs/official-session.md`](official-session.md) §6.2, prefixed by `"180804_"` as `"180804_netver:<netver>; didver:<didver>"` with a `VariableU32` byte-length prefix — `0x3F` when length is `63` — and no NUL or padding) and at offset `+0x00` of `ClientVersionMessage` (`0x0000012C`, [`docs/specs/official-session.md`](official-session.md) §9.3, as a `VariableU32` byte-length prefix — `0x38` when length is `56` — followed by the raw ASCII bytes without a trailing NUL).
- **`I12` (Credential & ticket redaction)**: Formatting any error, debug, or info log event produced during `S4`–`S10` never outputs the substring of the user's password or the `Ticket` value.
- **`I13` (`<queueurls>` RFC 1918 private-IPv4 filtering with fallback)**: When `<Status>/<queueurls>` contains both an RFC 1918 private-IPv4 URL (`http://10.0.0.1:8080/queue`) and a non-RFC-1918 URL (`http://queue-a.example.invalid:8080/queue`), `queue_url` selects the non-RFC-1918 URL; when all non-empty entries use RFC 1918 private IPv4 addresses, `queue_url` falls back to the first non-empty entry (`LL`).
- **`I14` (`TakeANumber` HRESULT sign-bit evaluation and fallback)**: In `S8_WORLD_QUEUE`, a `<Result>` whose 32-bit `HResult` has bit 31 clear (any non-negative `HRESULT`), or a `<Result>` omitting `<HResult>` while providing valid `<QueueNumber>` and `<NowServingNumber>`, is evaluated as a successful queue response (`LL`).
- **`I15` (`ArgTemplate` legacy alias substitution and unreplaced token filtering)**: Substituting `ArgTemplate` replaces `{LANGUAGE}`, `{PRODUCT}`, and `{AUTHSERVER}` alongside `{LANG}`, `{GAMETYPE}`, and `{AUTHSERVERURL}`, strips surrounding double quotes when splitting into `argv`, and drops any token still containing an unreplaced `{...}` placeholder (`LL`).
- **`I16` (Compression dictionary DID map resolution, deferred redirection, and `ConnectAcceptHeader` `0x00080000` `codecUuid`)**: Resolving outgoing dictionary key `0x10000002` via the dictionary sub-map resolved from root key `0x00000003` in `client_general.dat` ( [`docs/specs/official-session.md`](official-session.md) §4.1.2–§4.1.3) resolves to record `0x0E010000`, validates format UUID `E795878D-C633-471C-8315-C3DBBEAD2FAA` at `+0x00..+0x0F`, and registers the 16-byte `codecUuid` at `+0x10..+0x1F` for negotiation in `ConnectChallengeHeader` (`0x00040000` at `+0x1C..+0x2B`) and `ConnectAcceptHeader` (`0x00080000` at `+0x08..+0x17`); resolving incoming dictionary key `0x10000001` resolves to stub `0x0E010002`, reads `redirectDid` equal to `0x0E010001` at `+0x20..+0x23`, and redirects to load `0x0E010001` ([`docs/specs/official-session.md`](official-session.md) §4.1.2–§4.1.3, §6.3–§6.4).

---

### 7.2 Synthetic test vectors <a id="s7-2-vectors"></a>

#### 7.2.1 Vector 1 (`V1` / `TV1`) — Local `lotro.launcherconfig` with UTF-8 BOM

Input bytes start with `EF BB BF` followed by:

| Element / Attribute | Synthetic Value |
|---|---|
| `/configuration/appSettings/add[@key="Launcher.DataCenterService.GLS"]/@value` | `https://gls.example.invalid/service/datacenter` |
| `/configuration/appSettings/add[@key="DataCenter.GameName"]/@value` | `LOTRO` |
| `/configuration/appSettings/add[@key="DataCenter.DefaultCenter"]/@value` | `LOTRO` |
| `/configuration/appSettings/add[@key="Product.DocumentFolder"]/@value` | `The Lord of the Rings Online` |
| `/configuration/appSettings/add[@key="Launcher.DocumentFolder"]/@value` | `Launcher` |

Expected parsed output:
- `gls_datacenter_service` = `"https://gls.example.invalid/service/datacenter"`
- `datacenter_game_name` = `"LOTRO"`
- `document_folder` = `"The Lord of the Rings Online"`

---

#### 7.2.2 Vector 2 (`V2` / `TV2`) — `GetDatacenters` SOAP request and response

- **Request**:
  - `POST https://gls.example.invalid/service/datacenter`
  - `Content-Type: text/xml; charset=utf-8`
  - `SOAPAction: "http://www.turbine.com/SE/GLS/GetDatacenters"`
  - Body elements: `/soap:Envelope/soap:Body/GetDatacenters[@xmlns="http://www.turbine.com/SE/GLS"]/game` = `LOTRO`
- **Synthetic Response Elements** (under `GetDatacentersResponse/GetDatacentersResult/Datacenter`):
  - `Name` = `LOTRO`
  - `AuthServer` = `https://gls-auth.example.invalid/service/auth`
  - `PatchServer` = `patch.example.invalid:18000`
  - `LauncherConfigurationServer` = `https://gls.example.invalid/config/launcher-config.xml`
  - `Worlds/World[1]`: `Name` = `WorldAlpha [OLD]`, `LoginServerUrl` = `(invalid service specified)`, `ChatServerUrl` = `chat.example.invalid:18001`, `StatusServerUrl` = `http://legacy-status.example.invalid/service/status?s=shard-alpha`, `Order` = `2`
  - `Worlds/World[2]`: `Name` = `[EN] WorldBeta`, `LoginServerUrl` = `(invalid service specified)`, `ChatServerUrl` = `chat.example.invalid:18001`, `StatusServerUrl` = `https://gls.example.invalid/service/status?s=shard-beta`, `Order` = `1`
- **Expected parsed output**:
  - Sorted world display order places `[EN] WorldBeta` before `WorldAlpha [OLD]` (because `WorldAlpha [OLD]` ends case-insensitively with `[old]`).

---

#### 7.2.3 Vector 3 (`V3` / `TV3`) — `World/StatusServerUrl` `<Status>` parsing and no-fallback failure on `HTTP 404`

- **Case A (`HTTP 200 OK` on `World[2].StatusServerUrl` = `https://gls.example.invalid/service/status?s=shard-beta`)**:
  - **Synthetic `<Status>` XML fields**:
    - `<loginservers>login-a.example.invalid:19001;login-b.example.invalid:19002;</loginservers>`
    - `<queueurls>http://queue-a.example.invalid:8080/queue;http://queue-b.example.invalid:8080/queue;</queueurls>`
    - `<nowservingqueuenumber>0x00000010</nowservingqueuenumber>`
    - `<lastassignedqueuenumber>0x00000015</lastassignedqueuenumber>`
    - `<allow_billing_role>RoleTierVIP</allow_billing_role>`
    - `<deny_billing_role></deny_billing_role>`
  - **Expected parsed output**:
    - `login_server` = `"login-a.example.invalid:19001"`
    - `queue_url` = `"http://queue-a.example.invalid:8080/queue"`
    - `now_serving` = `16` (`0x10`), `last_assigned` = `21` (`0x15`)
    - `allow_billing_role` = `{"RoleTierVIP"}`
- **Case B (`HTTP 404 Not Found` or empty body on `World[1].StatusServerUrl` = `http://legacy-status.example.invalid/service/status?s=shard-alpha`)**:
  - **Expected result (`docs/decisions/0042-m8-remediation-decisions.md` item 3)**: Fails immediately with `F06_WORLD_CLOSED`; zero fallback URLs are constructed or requested.

---

#### 7.2.4 Vector 4 (`V4` / `TV4`) — `LoginAccount` multi-subscription filtering and SOAP fault

- **Case A (Success with multiple subscriptions across games)**:
  - `LoginAccountResult/Ticket` = `"SYNTH+TICKET/ABC123=="`
  - `GameSubscription[1]`: `Game` = `"DDO"`, `Name` = `"DDOSUB0000000000000000001"`, `Description` = `"DDO Subscription"`
  - `GameSubscription[2]`: `Game` = `"LOTRO"`, `Name` = `"LOTROSUB00000000000000002"`, `Description` = `"LOTRO Primary"`, `ProductTokens` = `["TokenBase", "RoleTierVIP"]`
  - **Expected result**: `GameSubscription[1]` (`DDO`) is ignored; `GameSubscription[2]` is auto-selected as the sole matching `LOTRO` subscription; selected account identifier is `"LOTROSUB00000000000000002"`; billing-role check against Vector `V3` (`RoleTierVIP`) passes.
- **Case B (SOAP Fault on invalid credentials)**:
  - HTTP status: `500 Internal Server Error`
  - Body: `/soap:Envelope/soap:Body/soap:Fault` with `<faultcode>BIS</faultcode>`, `<faultstring>Authentication rejected</faultstring>`, `<detail><result>0x8004C002</result></detail>`.
  - **Expected result**: Fails with `F04_AUTH_REJECTED`.

---

#### 7.2.5 Vector 5 (`V5` / `TV5`) — `TakeANumber` POST body encoding and `<Result>` evaluation

- **Inputs**:
  - `WorldQueue.LoginQueue.URL` = `https://gls-auth.example.invalid/service/queue`
  - `WorldQueue.TakeANumber.Parameters` = `command=TakeANumber&subscription={0}&ticket={1}&ticket_type=GLS&queue_url={2}`
  - `{0}` (`GameSubscription/Name`) = `LOTROSUB00000000000000002`
  - `{1}` (`Ticket`) = `SYNTH+TICKET/ABC123==`
  - `{2}` (`queue_url`) = `http://queue-a.example.invalid:8080/queue`
- **Expected `application/x-www-form-urlencoded` POST body**:
  `command=TakeANumber&subscription=LOTROSUB00000000000000002&ticket=SYNTH%2BTICKET%2FABC123%3D%3D&ticket_type=GLS&queue_url=http%3A%2F%2Fqueue-a.example.invalid%3A8080%2Fqueue`
- **Response Poll 1**:
  - `<Result><Command>TakeANumber</Command><HResult>0x00000000</HResult><QueueNumber>0x00000014</QueueNumber><NowServingNumber>0x00000011</NowServingNumber><ContextNumber>0x00000099</ContextNumber></Result>`
  - **Expected action**: `HResult` is `0` (non-negative), `QueueNumber` (`20`) > `NowServingNumber` (`17`); display `3` players ahead and remain in `S8_WORLD_QUEUE`.
- **Response Poll 2**:
  - `<Result><Command>TakeANumber</Command><HResult>0x00000000</HResult><QueueNumber>0x00000014</QueueNumber><NowServingNumber>0x00000014</NowServingNumber><ContextNumber>0x00000099</ContextNumber></Result>`
  - **Expected action**: `QueueNumber` (`20`) ≤ `NowServingNumber` (`20`); transition immediately to `S9_SESSION_HANDOFF`.

---

#### 7.2.6 Vector 6 (`V6` / `TV6`) — `RefreshTicket` SOAP exchange and backoff arithmetic

- **Initial state**: $t_0 = 1000.0\text{ s}$, $L = 21600\text{ s}$, `ticket` = `"SYNTH+TICKET/ABC123=="`.
  - Installation timestamp $t_0 = 1000.0\text{ s}$, next refresh offset $\Delta t_{\text{next}} = 10800.0\text{ s}$, attempt count $k = 0$.
- **Check at $t = 11800.0\text{ s}$** ($\Delta t = 10800.0 \le 10800.0$): no request sent.
- **Check at $t = 11800.1\text{ s}$** ($\Delta t = 10800.1 > 10800.0$):
  - $\Delta t_{\text{next}} \leftarrow 10800.0 + 0.5 \times (21600.0 - 10800.1) = 16199.95\text{ s}$.
  - $k \leftarrow 1$.
  - Sends SOAP 1.1 `POST` to `--authserverurl` with `SOAPAction: "http://www.turbine.com/SE/GLS/RefreshTicket"` and `/soap:Envelope/soap:Body/RefreshTicket[@xmlns="http://www.turbine.com/SE/GLS"]/ticket` = `"SYNTH+TICKET/ABC123=="`.
- **Response at $t = 11800.5\text{ s}$**:
  - `/soap:Envelope/soap:Body/RefreshTicketResponse[@xmlns="http://www.turbine.com/SE/GLS"]/RefreshTicketResult` = `"SYNTH+TICKET/NEW456=="`.
  - **Expected state update**: Active ticket updated to `"SYNTH+TICKET/NEW456=="` (used for subsequent support SOAP calls and any UDP `SessionAuthHeader` `0x00010000` / `WorldReferralHeader` `0x00000800` reconnect handshake); $t_0 \leftarrow 11800.5\text{ s}$; $\Delta t_{\text{next}} \leftarrow 10800.0\text{ s}$; $k \leftarrow 0$.

---

#### 7.2.7 Vector 7 (`V7` / `TV7`) — `netver`/`didver` serialization in `SessionAuthHeader` (`0x00010000`) and `ClientVersionMessage` (`0x0000012C`), and `0x10000002`/`0x10000001` dictionary resolution

- **Synthetic inputs**:
  - Discovered `<netver_string>` from user-supplied value or PE `.rsrc` `VS_VERSION_INFO` `StringFileInfo` (`"CompatibiltyVersion"`): `"1234"` (`4` ASCII bytes `31 32 33 34`).
  - Synthetic `.dat` superblock descriptor at file offset `0x17C..0x18F` (`20` bytes):
    `04 03 02 01 06 05 08 07 09 0A 0B 0C 0D 0E 0F 10 4C 04 00 00`
    (`Data1 = 0x01020304`, `Data2 = 0x0506`, `Data3 = 0x0708`, `Data4 = 09 0A 0B 0C 0D 0E 0F 10`, `ver_u32 = 1100`).
  - Synthetic dictionary sub-map mappings, with DID `0x28000010` obtained by looking up root key `0x00000003`, (§2.4.3; [`docs/specs/official-session.md`](official-session.md) §4.1.2–§4.1.3): outgoing dictionary key `0x10000002` $\rightarrow$ `0x0E010000` (with 16-byte `codecUuid` `AA BB CC DD 11 22 33 44 55 66 77 88 99 00 AB CD` at `+0x10..+0x1F`) and incoming dictionary key `0x10000001` $\rightarrow$ `0x0E010002` (44-byte deferred stub with `redirectDid = 0x0E010001` at `+0x20..+0x23`, [`docs/specs/official-session.md`](official-session.md) §4.1.2–§4.1.3).
- **Expected derived values and wire bytes**:
  - `<didver_string>` = `"01020304-0506-0708-090A-0B0C0D0E0F10"` (`36` uppercase ASCII chars; `ver_u32 = 1100` is omitted).
  - Combined `client_version_string` = `"netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10"` (`56` ASCII bytes).
  - `SessionAuthHeader` (`0x00010000`, [`docs/specs/official-session.md`](official-session.md) §6.2, §10.2 Vector 6) `netVersion` bytes `+0x00..+0x3F` (`64` bytes):
    `3F` (`VariableU32` length `63`, [`docs/specs/official-session.md`](official-session.md) §5.6.1) followed by the `63` ASCII bytes of `"180804_netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10"` (no trailing NUL, no 4-byte padding; `authBlockLen` immediately follows at `+0x40..+0x43`).
  - `ClientVersionMessage` (`0x0000012C`, [`docs/specs/official-session.md`](official-session.md) §9.3) inner payload bytes `+0x00..+0x38` (`57` bytes):
    `38` (`VariableU32` length `56`, [`docs/specs/official-session.md`](official-session.md) §5.6.1) followed by the `56` ASCII bytes of `"netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10"` (no trailing NUL).
  - Resolving incoming dictionary key `0x10000001` redirects from stub `0x0E010002` to `0x0E010001` ([`docs/specs/official-session.md`](official-session.md) §4.1.3), and `ConnectAcceptHeader` (`0x00080000`, [`docs/specs/official-session.md`](official-session.md) §6.4) payload bytes `+0x08..+0x17` carry the 16-byte `codecUuid` `AA BB CC DD 11 22 33 44 55 66 77 88 99 00 AB CD` (followed by `u32` LE `maxRecvRate` at `+0x18..+0x1B`).

---

## 8. Implementer Notes <a id="s8-implementer-notes"></a>

1. **XML namespace, `SOAPAction` protocol identifiers, and case-sensitivity handling**:
   - Official GLS responses use default namespace `xmlns="http://www.turbine.com/SE/GLS"` on `<GetDatacentersResponse>`, `<LoginAccountResponse>`, and `<RefreshTicketResponse>`, while `<Status>` (`World/StatusServerUrl`) and `<Result>` (`WorldQueue.LoginQueue.URL`) have **no XML namespace**. Per `docs/decisions/0042-m8-remediation-decisions.md` item 3, the XML namespace URI `http://www.turbine.com/SE/GLS` and the `SOAPAction` strings (`"http://www.turbine.com/SE/GLS/GetDatacenters"`, `"http://www.turbine.com/SE/GLS/LoginAccount"`, `"http://www.turbine.com/SE/GLS/RefreshTicket"`) are XML/SOAP protocol identifiers that are never contacted as network endpoints and may appear in code as constants only in the SOAP message encoder. Clean-room XML parsers should match SOAP payload elements in the `http://www.turbine.com/SE/GLS` namespace (or match local element names after verifying the SOAP body), match SOAP `<Fault>` case-insensitively (`Fault` / `fault`, extracting either `<faultstring>` or `<Message>`), and match child element names under `<Status>` case-insensitively (`LL`).
2. **Splitting `;`-delimited lists and RFC 1918 filtering in `<Status>`**:
   - Both `<loginservers>` and `<queueurls>` in the `World/StatusServerUrl` `<Status>` response typically have a trailing semicolon (e.g., `host:port;`). Splitting on `;` produces a trailing empty string `""`; implementations must filter out empty segments when extracting `login_server` and `queue_url`. When `<queueurls>` contains multiple non-empty entries, prefer the first non-RFC-1918 URL whose host is not in the RFC 1918 private IPv4 ranges `10.0.0.0/8`, `172.16.0.0/12` (`172.16.*` through `172.31.*`), or `192.168.0.0/16`, and fall back to the first non-empty entry if all entries are RFC 1918 private IPv4 addresses (`LL`).
3. **Hexadecimal integer parsing and HRESULT sign bit in `<Status>` and `<Result>`**:
   - `nowservingqueuenumber`, `lastassignedqueuenumber`, `HResult`, `QueueNumber`, `NowServingNumber`, and `ContextNumber` are formatted with a leading `0x` or `0X` prefix. Strip the `0x`/`0X` prefix and parse as base-16 `u32`. For `<Result>/<HResult>`, test whether bit 31 (the sign bit, mask `0x80000000`) is set for failure rather than testing whether the value is non-zero, and treat a missing `<HResult>` as `0` when both `<QueueNumber>` and `<NowServingNumber>` are present (`LL`).
4. **In-memory hand-off vs CLI process spawning**:
   - Section 3.6 and Section 4.1 document the command-line flags (`-a`, `-h`, `--glsticketdirect`, `--authserverurl`, `--glsticketlifetime`, `--glsticket`, etc.) for completeness and interoperability. Inside Tessera, the launcher service client and the UDP session client reside in the same workspace and should pass the session hand-off structure directly in memory (§4.2) rather than round-tripping through process CLI arguments or the Windows registry.

---

## 9. Summary of Open Questions <a id="s9-open-questions"></a>

Two questions remain in §3: official-launcher per-step HTTP use, and agent/proxy behavior. Earlier questions (`OPEN-LOGIN-1` in §2.4, owner decision `O1` of `docs/decisions/0041-incident-m8-spec-contamination.md` and `docs/decisions/0042-m8-remediation-decisions.md` items 1 & 3 in §2.4.1 and §3.3, and `OPEN-LOGIN-2` in §6.2) have been resolved.

---

## 10. Change Log <a id="s10-changelog"></a>

| Date | Author Role | Summary |
|---|---|---|
| `2026-10-20` | `Spec writer (dirty room)` | Initial draft of `docs/specs/official-login.md` covering runtime config discovery, GLS/world-queue exchanges, client ticket intake and `RefreshTicket` lifecycle, state machine, security notes, invariants, and synthetic test vectors (`OBS`, `OL`, `PL`, `GLS`, `REDDO`, `DDOL`, `BIN`). |
| `2026-10-20` | `Clean-room reviewer` | Independent review against `CLEAN_ROOM.md` §5.3: fixed SPDX header, set target crate to `crates/compat`, and marked status `Accepted`. |
| `2026-10-20` | `Spec writer (dirty room)` | Cross-referenced against project owner's `lotro-launcher` (`LL`): added 64-bit `UserPreferences64.ini`/`UserPreferences_x64.ini` lookup order and `LastWorld` (§2.3), optional `Datacenter/SupportURL` and `Order` sorting (§3.1), case-insensitive `<Status>` child matching and `<queueurls>` RFC 1918 private-IP filtering with fallback (§3.3, `I13`), SOAP `<Fault>`/`<Message>` classification for maintenance and locked accounts (§3.4), mandatory `TakeANumber` execution when `queue_url` is non-empty and `HResult` bit-31 sign evaluation with `<QueueNumber>`/`<NowServingNumber>` fallback (§3.5, `I14`), and legacy `ArgTemplate` aliases (`{LANGUAGE}`, `{PRODUCT}`, `{AUTHSERVER}`, `{HIGHRES}`, `{USERDIR}`) plus unreplaced `{...}` token filtering (§3.6, `I15`). |
| `2026-10-20` | `Spec writer (dirty room)` | Resolved `OPEN-LOGIN-1` (`BIN-Q3`, `BIN-Q4`, `OBS`): documented runtime discovery of `<netver_string>` and `<didver_string>` (`.dat` superblock `0x17C..0x18B` 36-char uppercase GUID without the archive version integer), `VariableU32`-prefixed wire encoding of `"netver:<netver>; didver:<didver>"` in `ClientVersionMessage` (`0x0000012C`), resolution of enumerator keys `0x10000002` and `0x10000001` via DID Map `3` (`0x28000010` in `client_general.dat`) to `0x0E010000` and `0x0E010002` $\rightarrow$ `0x0E010001`, byte layout of `0x0E01xxxx` compression dictionary records, and `dictionary_guid` negotiation in `ConnectChallengeHeader` (`0x00040000`) and `ConnectAcceptHeader` (`0x00080000`) (§1.2, §1.3, §2.4, §4.2, `I11`, `I16`, `V7`, §9). |
| `2026-10-23` | `Spec writer (dirty room)` | Remediated under incident 0041 (`docs/decisions/0041-incident-m8-spec-contamination.md`, `docs/m8-remediation.md` W2/C2): removed client-internal names and in-memory structure descriptions, replaced the `.text` instruction-pattern scan in §2.4.1, added explicit interoperability statements for all wire/file/CLI identifiers, and set status to `Revised`. |
| `2026-10-23` | `Spec reviewer (a8686178-b869-4b5e-a35f-08a974755163)` | Pass 1 W4 review (`docs/m8-remediation.md` §4 item 4, `docs/CLEAN_ROOM.md` §5.3): spot-checked `lotro.launcherconfig`, `client_local_English.dat` and `client_general.dat` superblocks, `client_general.dat` records `0x28000000`/`0x28000010` and `0x0E010000`/`0x0E010001`/`0x0E010002`, and `x64/lotroclient64.exe` `.rsrc` `CompatibiltyVersion` (`"9386"`). |
| `2026-10-23` | `Spec writer (dirty room, 8abac49d-40d9-4385-8491-f8bd2e0dedba)` | Remediation Pass 2 (`docs/m8-remediation.md` §7 `B1`, `B4` items 6 & 7, `N1`, `N2`, `N6`; `docs/decisions/0042-m8-remediation-decisions.md` items 1 & 3): (1) `N1` & Decision 0042 item 1 — dropped Step B2 and all data-section signatures and diagnostic format strings from §2.4.1, §0, and §1, establishing the three-step `<netver_string>` discovery order (user-supplied value, PE `.rsrc` `VS_VERSION_INFO` `StringFileInfo` `"CompatibiltyVersion"` / `"CompatibilityVersion"`, or user prompt / `F02_CONFIG_INVALID`); (2) `B4` items 6 & 7 and `N2` — removed the duplicate variable-length integer byte-order formula and Rust casts in §2.4.2 and replaced the legacy type name with `VariableU32` linked to `docs/specs/official-session.md` §5.6.1 and §9.3, corrected the `0x28000010` `DataIdMap` `TSize` offsets (`u8` bucket-count hint + `VariableU32` `VLE` count at `+0x04..+0x05`, `+0x26..+0x27`, and `+0x70..+0x73`) in §2.4.3 per `docs/specs/records.md` §1.3 & §8.1, and replaced bitwise/code expressions in §2.4.3, §3.5, §7.1, and §8 with plain prose; (3) `N6` & Decision 0042 item 3 — removed the fallback world-status URL path-fragment rewrite from §0, §1, §3.3, §5, `I4`, and Vector `V3`, and noted explicitly in §0, §3.1, §3.4, §4.3, §6.1, and §8 that the XML namespace URI (`http://www.turbine.com/SE/GLS`) and `SOAPAction` strings are XML/SOAP protocol identifiers that are never contacted as network endpoints; (4) `B1` — returned status to `Review (remediation pass 2; awaiting independent review)`. |
| `2026-10-23` | `Spec reviewer (1d1d854d-7fcb-4e05-b387-04da8ffbe515)` | Independent Remediation Pass 2 review (`docs/m8-remediation.md` §7, `docs/decisions/0042-m8-remediation-decisions.md` item 7, `docs/CLEAN_ROOM.md` §5.3; review only, zero body edits): verified `N1` & Decision 0042 item 1 (Step B2 and all data-section signatures and diagnostic format strings removed; `<netver_string>` discovery restricted to user-supplied value, PE `.rsrc` `VS_VERSION_INFO` `StringFileInfo` `"CompatibiltyVersion"` / `"CompatibilityVersion"`, or user prompt), `B4` items 6 & 7 and `N2` (`VariableU32` linked to `docs/specs/official-session.md` §5.6.1 and §9.3; zero code or pseudo-code fragments), `N6` & Decision 0042 item 3 (no fallback status URL rewrite; XML namespace and `SOAPAction` strings noted as non-contacted protocol identifiers), and `N9` / `CLEAN_ROOM.md` §5.3 (zero `file:///` links, `LEGAL.md` references, Ghidra addresses/symbols, registers/stack/vtable offsets, internal C++ identifiers/memory layouts, secrets, or captures); independently spot-checked against the user's install `lotro.launcherconfig` (UTF-8 BOM and all five keys), `client_general.dat`/`client_local_English.dat`/`client_gamelogic.dat` superblocks (`"BT\0\0"`, `0x170` = `0x28000000`, `0x17C..0x18B` 36-char uppercase GUID, `0x18C` = `1100`), `client_general.dat` records `0x28000000` (key `0x00000003` $\rightarrow$ `0x28000010`), `0x28000010` (`116` bytes `+0x00..+0x73` byte-for-byte match), and `0x0E010000`/`0x0E010001`/`0x0E010002` (schema GUID, sizes `218,247`/`214,953`/`44`, node counts `32,743`/`32,249`, redirect `0x0E010002` $\rightarrow$ `0x0E010001`), and `x64/lotroclient64.exe` (`d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee`) `.rsrc` `CompatibiltyVersion` (`"9386"`). Result: `Pass`; status set to `Accepted`. |
| `2026-10-24` | `Spec writer (dirty room, 5ae75fbf-2c81-45ed-9881-94448337644c)` | Remediation Pass 3 (`docs/m8-remediation.md` §8 `F2`, `F3`, `F7`, `F9`; `docs/decisions/0042-m8-remediation-decisions.md` items 3 & 4): (1) `F3` & Decision 0042 item 3 — replaced all remaining service path fragments and real port numbers across §0–§7.2 and Vectors `V1`–`V5` (`TV1`–`TV5`) in §7.2 with runtime XML/configuration key names (`Launcher.DataCenterService.GLS`, `Datacenter/AuthServer`, `Datacenter/LauncherConfigurationServer`, `World/StatusServerUrl`, `WorldQueue.LoginQueue.URL`, `GameClient.Arg.supportserviceurl`) and neutral synthetic `.example.invalid` paths/ports; (2) `F7` — reconciled §4.1 with `docs/specs/official-messages-play.md` §6.1.1 by adding `--voiceoff` and the `--chatserver`/`--usechat` post-parse fallback rule, and reconciled §1.2, §1.3, §2.4.2, §2.4.3, §4.2, `I11`, `I16`, and `V7` (`TV7`) with the canonical handshake header and message names in `docs/specs/official-session.md` §6 and §9.3 (`SessionAuthHeader` `0x00010000`, `ConnectChallengeHeader` `0x00040000`, `ConnectAcceptHeader` `0x00080000`, `VersionRequestMessage` `0x0000012F`, `ClientVersionMessage` `0x0000012C`, `VersionStatusMessage` `0x0000012D`, `LoginQueueStatusMessage` `0x00000130`, `WorldReferralHeader` `0x00000800`); (3) `F2` & `F9` — set status to `Review (remediation pass 3; awaiting independent review; acceptance gated by decision 0042 item 4)` and logged full 36-character session UUID. |
| `2026-10-24` | `Spec writer (dirty room, f0a991a9-0eab-4b03-b4eb-0ef3b61fc5fe)` | Pass 3 pre-review audit fixes (`docs/m8-remediation.md` §9): (1) deduplicated `0x0E01xxxx` compression dictionary record layout and `codecUuid` negotiation in §2.4.3, §4.2, `I16`, and `V7` (`TV7`) by cross-referencing `docs/specs/official-session.md` §4.1.2–§4.1.3 and §6.3–§6.4, and fixed incoming dictionary key `0x10000001` ($\rightarrow$ `0x0E010002` $\rightarrow$ `0x0E010001`) description in §4.2; (2) removed client-internal handler/struct/listener residue from §1.3, §4.1, §4.3, §5.1, `I10`, and `V6` (`TV6`); (3) replaced remaining `==`/`!=` operators in prose with English words and aligned §7.2 test vector headings (`V1`/`TV1`–`V7`/`TV7`) with §10. |
| `2026-10-24` | `Spec reviewer (a4c73c75-5bab-45c5-aff3-4b5331946564)` | Independent Remediation Pass 3 review (`docs/m8-remediation.md` §8–§9, `docs/decisions/0042-m8-remediation-decisions.md` item 7, `docs/CLEAN_ROOM.md` §5.3; review only, zero body edits): verified all `CLEAN_ROOM.md` §5.3 checklist items and all `docs/m8-remediation.md` §8–§9 requirements (§2.4.1 `<netver_string>` discovery per Decision 0042 item 1; §2.4.3 and §4.2 cross-references to `official-session.md` §4.1.2–§4.1.3 and §6.3–§6.4 for compression dictionary records and `codecUuid` negotiation; §3.3 no-fallback status URL and non-endpoint SOAP/XML protocol identifiers per Decision 0042 item 3; §4.1 and §4.3 alignment with `official-messages-play.md` §6.1.1–§6.1.2; neutral `.example.invalid` synthetic test vectors `V1`/`TV1`–`V7`/`TV7` in §7.2); independently spot-checked against the user's install `lotro.launcherconfig` (UTF-8 BOM `EF BB BF` and all five `<add key="...">` keys), `client_general.dat`/`client_local_English.dat`/`client_gamelogic.dat` superblocks (`"BT\0\0"` magic at `0x140..0x143`, `0x170..0x173` = `0x28000000` in `client_general.dat`, identical 36-char uppercase GUID at `0x17C..0x18B`, `0x18C..0x18F` = `1100`), and `x64/lotroclient64.exe` (build `4906.0071.1794.4035`, SHA-256 `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee`) `.rsrc` `VS_VERSION_INFO` `StringFileInfo` `"CompatibiltyVersion"` (`"9386"`). Result: `Pass`; status set to `Accepted`. |
| `2026-10-24` | `Spec writer (dirty room, 5f20d593-e8c6-4c5f-b050-7cc670ee5d2d)` | Remediation Pass 4 — Stage 1 Core (`docs/m8-remediation.md` §10, `docs/decisions/0042-m8-remediation-decisions.md` items 4, 8, & 9): (1) replaced the duplicate `0x28000010` layout table in §2.4.3 with a concise cross-reference to `docs/specs/official-session.md` §4.1.2–§4.1.3 and §6.3–§6.4 and removed all `.dat` label strings across §0, §2.4, §2.4.3, §4.2, `I16`, `V7` (`TV7`), and §10; (2) clarified the post-handshake version negotiation, character selection (`official-messages-world.md` §3), and `WorldReferralHeader` (`0x00000800`) sequence in §2.4.2 and corrected the `WorldReferralHeader` citation in §4.2–§4.3 to `official-session.md` §6.5; (3) corrected §10 change-log section citations (`§0–§7.2`) and removed quoted internal names from change-log history; (4) set status to `Review (remediation pass 4 — Stage 1 core; awaiting independent review; acceptance gated by decision 0042 items 4 & 8)`. |
| `2026-10-24` | `Spec reviewer (2cbf5815-ab41-434e-9e3d-60ea0d3009b8)` | Independent Remediation Pass 4 — Stage 1 Core review (`docs/m8-remediation.md` §10, `docs/decisions/0042-m8-remediation-decisions.md` items 1–9, `docs/CLEAN_ROOM.md` §5.3; review only, zero body edits): verified all `CLEAN_ROOM.md` §5.3 checklist items and all `docs/m8-remediation.md` §10 requirements (§2.4.3 delegates Master DID Map `3` `0x28000010`, `0x0E01xxxx` record layout, and `codecUuid` negotiation to `official-session.md` §4.1.2–§4.1.3 and §6.3–§6.4 with no duplicate layout table and no `.dat` label strings; `WorldReferralHeader` `0x00000800` cites `official-session.md` §6.5 in §2.4.2, §4.2, and §4.3; §10 change log cites `§7.2` for synthetic test vectors and quotes no removed internal values; zero references to the held gameplay spec); independently spot-checked against the user's install `lotro.launcherconfig` (UTF-8 BOM `EF BB BF` and all five `<add key="...">` keys), `client_general.dat`/`client_local_English.dat`/`client_gamelogic.dat` superblocks (`"BT\0\0"` magic at `0x140..0x143`, `0x170..0x173` = `0x28000000` in `client_general.dat`, identical 36-char uppercase GUID at `0x17C..0x18B`, `0x18C..0x18F` = `1100`), `client_general.dat` records `0x28000000` (entry `0x00000003` $\rightarrow$ `0x28000010`), `0x28000010` (`116` bytes mapping `0x10000001` $\rightarrow$ `0x0E010002`, `0x10000002` $\rightarrow$ `0x0E010000`, `0x10000003` $\rightarrow$ `0x0E010001`), and `0x0E010000`/`0x0E010001`/`0x0E010002` (`44`-byte stub `0x0E010002` redirecting at `+0x20` to `0x0E010001`), and `x64/lotroclient64.exe` (build `4906.0071.1794.4035`, SHA-256 `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee`) `.rsrc` `VS_VERSION_INFO` `StringFileInfo` `"CompatibiltyVersion"` (`"9386"`). Result: `Pass`; status set to `Accepted`. |
| `2026-10-25` | `Spec writer (dirty room, 9013ee1d-b5e8-4dec-94e9-01362f120e0d)` | Remediation Pass 5 — Stage 1 Core (`docs/m8-remediation.md` §11 `B1`–`B3` and content note, `docs/decisions/0042-m8-remediation-decisions.md` items 4 & 8): (1) `B1` & `B2` — reconciled §4.1 and §4.2 launch parameters (`-h`/`--host` accepting `host`, `host:port`, or `<localHost:remoteHost:port>`, `-p`/`--port` remote UDP port, `--outport` local UDP bind port alongside `Net.BindInterface` and `Net.ComputeUniquePort` in `UserPreferences.ini`, and `--chatserver` fallback to `--host`) with `docs/specs/official-session.md` §3.2–§3.3; (2) `B3` — reconciled `<Ticket>` variable Base64 character length $L$ (`144..256` characters) and Base64-decoded byte length $B = \lfloor L \times 3 / 4 \rfloor$ (`108` to `192` bytes) in §3.4, §4.1, and §4.2, and added §4.2.1 stating the deterministic `SessionAuthHeader` (`0x00010000`) `authType` selection rule matching `docs/specs/official-session.md` §6.2.1 (`authType = 0x40000002`, `authFlags = 0x00000004` `HasExtraBlob` with `extraBlob` carrying `u32` LE `blobLen` + Base64-decoded ticket bytes whenever a GLS ticket is present; `authType = 0x00000001`, `authFlags = 0x00000000` only when no ticket is supplied); (3) replaced the `I6` billing-role test tokens in §7.1 with neutral synthetic tokens (`{"SyntheticTokenA", "SyntheticTokenB"}`); (4) set status to `Review (remediation pass 5 — Stage 1 core; awaiting independent review; acceptance gated by decision 0042 items 4 & 8)`. |
| `2026-10-25` | `Spec reviewer (bd1d6391-5a95-4501-8b24-1ae896887eb3)` | Independent Remediation Pass 5 — Stage 1 Core review (`docs/m8-remediation.md` §11, `docs/decisions/0042-m8-remediation-decisions.md` items 7–9, `docs/CLEAN_ROOM.md` §5.3; review only, zero body edits): verified all `CLEAN_ROOM.md` §5.3 checklist items and all `docs/m8-remediation.md` §11 requirements (`B1` & `B2` launch parameters `-h`/`--host` including `<localHost:remoteHost:port>`, `-p`/`--port`, `--outport`, and `--chatserver` fallback matching `official-session.md` §3.2–§3.3; `B3` variable-length Base64 GLS ticket `144..256` chars decoding to `108..192` bytes and deterministic `SessionAuthHeader` `0x00010000` `authType = 0x40000002` / `0x00000001` selection rule in §4.2.1 matching `official-session.md` §6.2.1; neutral synthetic tokens in §7.1 `I6`); independently spot-checked against the user's install `lotro.launcherconfig` (UTF-8 BOM `EF BB BF` and all five `<add key="...">` keys), `client_general.dat`/`client_local_English.dat`/`client_gamelogic.dat` superblocks (`"BT\0\0"` magic at `0x140..0x143`, `0x170..0x173` = `0x28000000` in `client_general.dat`, identical 36-char uppercase GUID at `0x17C..0x18B`, `0x18C..0x18F` = `1100`), and `x64/lotroclient64.exe` (build `4906.0071.1794.4035`, SHA-256 `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee`) `.rsrc` `VS_VERSION_INFO` `StringFileInfo` `"CompatibiltyVersion"` (`"9386"`). Result: `Pass`; status set to `Accepted`. |
| `2026-10-25` | `Spec writer (dirty room, e39a5eb7-89c1-4e5b-a082-a2d6fbbaba04)` | Remediation Pass 6 — Stage 1 Core (`docs/m8-remediation.md` §12 notes, `docs/decisions/0042-m8-remediation-decisions.md` items 4 & 8): (1) replaced the $\lfloor L \times 3 / 4 \rfloor$ formula in §3.4, §4.1, §4.2, and §4.2.1 with the decoded byte length of the Base64 ticket string; (2) clarified in §4.1 and §4.2 that `-h` / `--host` accepts a colon-delimited string with zero colons (`remoteHost`), one colon (`remoteHost:port`), or two colons (`localHost:remoteHost:port`, where the three colon-separated segments specify the local bind address/interface, remote login server host, and remote UDP port, and any `<` `>` brackets in help text are metasyntactic placeholders rather than literal command-line characters; `docs/specs/official-session.md` §3.2–§3.3); (3) set status to `Review (remediation pass 6 — Stage 1 core; awaiting independent review; acceptance gated by decision 0042 items 4 & 8)`. |
| `2026-10-25` | `Spec reviewer (a971230f-5b5e-4d3f-880a-cbae947c26eb)` | Independent Remediation Pass 6 — Stage 1 Core review (`docs/m8-remediation.md` §12, `docs/decisions/0042-m8-remediation-decisions.md` items 7–9, `docs/CLEAN_ROOM.md` §5.3; `Reviewer: a971230f-5b5e-4d3f-880a-cbae947c26eb`; review only, zero body edits): verified all `CLEAN_ROOM.md` §5.3 checklist items and all `docs/m8-remediation.md` §12 requirements (decoded byte length of the Base64 ticket string in §3.4, §4.1, §4.2, and §4.2.1 with no floor formula; `-h`/`--host` zero-colon `remoteHost`, one-colon `remoteHost:port`, and two-colon `localHost:remoteHost:port` definition with metasyntactic angle brackets in §4.1–§4.2 matching `official-session.md` §3.2–§3.3; optional `LoginQueueStatusMessage` `0x00000130` in §2.4.2 and §4.2); independently spot-checked against the user's install `lotro.launcherconfig` (UTF-8 BOM `EF BB BF` and all five `<add key="...">` keys), `client_general.dat`/`client_local_English.dat`/`client_gamelogic.dat` superblocks (`"BT\0\0"` magic at `0x140..0x143`, `0x170..0x173` = `0x28000000` in `client_general.dat`, identical 36-char uppercase GUID at `0x17C..0x18B`, `0x18C..0x18F` = `1100`), and `x64/lotroclient64.exe` (build `4906.0071.1794.4035`, SHA-256 `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee`) `.rsrc` `VS_VERSION_INFO` `StringFileInfo` `"CompatibiltyVersion"` (`"9386"`). Result: `Pass`; status set to `Accepted`. |
| `2026-10-25` | `Spec writer (dirty room, 685c8123-ea45-4b5e-85a5-6bf376f44935)` | Remediation Pass 7 — Stage 1 Core (`docs/m8-remediation.md` §13 note, `docs/decisions/0042-m8-remediation-decisions.md` items 4 & 8): (1) clarified Base64-decoded GLS ticket byte length examples in §3.4 (`L385`), §4.1 (`L509`), §4.2 (`L554`), and §4.2.1 (`L561`) to say "up to `108` raw bytes (`0x0000006C`, or `106..108` bytes depending on `'='` padding) for a 144-character Base64 ticket or up to `135..192` raw bytes (`133..192` bytes depending on `'='` padding) for a `180..256`-character Base64 ticket"; (2) set status to `Review (remediation pass 7 — Stage 1 core; awaiting independent review; acceptance gated by decision 0042 items 4 & 8; 0 OPEN)`. |
| `2026-10-25` | `Spec reviewer (51f17dc6-f21c-4df8-9e60-d0e41af7ff2b)` | Independent Remediation Pass 7 — Stage 1 Core review (`docs/m8-remediation.md` §13, `docs/decisions/0042-m8-remediation-decisions.md` items 7–9, `docs/CLEAN_ROOM.md` §5.3; `Reviewer: 51f17dc6-f21c-4df8-9e60-d0e41af7ff2b`; review only, zero body edits): verified all `CLEAN_ROOM.md` §5.3 checklist items and the `docs/m8-remediation.md` §13 note ("up to `108` raw bytes (`0x0000006C`, or `106..108` bytes depending on `'='` padding) for a 144-character Base64 ticket or up to `135..192` raw bytes (`133..192` bytes depending on `'='` padding) for a `180..256`-character Base64 ticket" in §3.4 `L385`, §4.1 `L509`, §4.2 `L554`, and §4.2.1 `L561`); independently spot-checked against the user's install `lotro.launcherconfig` (UTF-8 BOM `EF BB BF` and all five `<add key="...">` keys), `client_general.dat`/`client_local_English.dat`/`client_gamelogic.dat` superblocks (`"BT\0\0"` magic at `0x140..0x143`, `0x170..0x173` = `0x28000000` in `client_general.dat`, identical 36-char uppercase GUID at `0x17C..0x18B`, `0x18C..0x18F` = `1100`), and `x64/lotroclient64.exe` (build `4906.0071.1794.4035`, SHA-256 `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee`) `.rsrc` `VS_VERSION_INFO` `StringFileInfo` `"CompatibiltyVersion"` (`"9386"`). Result: `Pass`; status set to `Accepted`. |
| `2026-10-30` | `Spec writer (dirty room, c0194bf2-43a9-4f65-a6c2-3e421509c2f7)` | Remediation Pass 8 — Stage 1 Core (`docs/m8-remediation.md` §14–§15, `docs/decisions/0042-m8-remediation-decisions.md` items 4 & 8): (1) updated §2.4, §2.4.2, §4.2, `I11`, and `V7` (`TV7`) to match `official-session.md` §6.2: `SessionAuthHeader` (`0x00010000`) serializes `netVersion` as `VariableU32` byte length + `"180804_"` + `"netver:<netver_string>; didver:<didver_string>"` (with no NUL or 4-byte padding; and 6-byte `"180804"` with `VariableU32` `0x06` on `ChatSession`) followed immediately by unpadded `u32` LE `authBlockLen`, and `ConnectAcceptHeader` (`0x00080000`) places the 16-byte `codecUuid` at `+0x08..+0x17` (§2.4.3, §4.2, `I16`, `V7`); (2) updated §3.4, §4.1, §4.2, and §4.2.1 to match `official-session.md` §6.2.1: on the primary login/world UDP connection `SessionAuthHeader` uses `authType = 0x00000001` (while `ChatSession` uses `authType = 0x40000002`), and `extraBlob` (`authFlags = 0x00000004`) carries `u32` LE `extraBlobLen` followed by the `VariableU32`-prefixed raw ASCII Base64 ticket string without Base64 decoding; (3) documented `LotroLauncher.exe` (`BIN-Q5`) HTTP/HTTPS scheme preservation, `User-Agent`, and proxy behavior (§3, §6.2), `WorldQueue.LoginQueue.RefreshSeconds` default `20 s` poll interval with `[5, 120] s` clamp (§3.2, §3.5, §5.2), `TakeANumber` skipping when `<queueurls>` has no non-empty entry (§3.5), local poll cancel without `LeaveQueue` in `LotroLauncher.exe` vs optional `LeaveQueue` (§3.5, §5.1), and the `1..64`-character clean-room sanity bound vs un-capped client `--netver` copy (§2.4.1, §4.1); (4) set status to `Review (remediation pass 8 — Stage 1 core; awaiting independent review; acceptance gated by decision 0042 items 4 & 8; 0 OPEN)`. |
| 2026-10-30 | Writer: Codex (GPT-6), pass 9; session ID not recorded (not exposed by the host) | Imported the pass-8 writer facts; reconciled §2, §3.3 and §6.2 with amended decision 0044, including explicit ports; retained OPENs for official-launcher HTTP steps and agent/proxy behavior; corrected dictionary lookup descriptions and provenance tool-version uncertainty. No acceptance or new reviewer claimed. |
| 2026-10-30 | Writer: Codex (GPT-6), pass 9 follow-up; same session ID not recorded | §4.3 and I9/I10 use sampled times consistent with the strict greater-than trigger; synthetic arithmetic independently recomputed. §6.4 states excluded-research policy without an absence assertion. |
| `2026-10-30` | `Codex (GPT-6), pass-9 writer continuation; same session ID not recorded` | Remediation §17: BIN-Q5 now treats launcher scheme/agent/proxy details as reported and OPEN; §2 states decision 0044 item 2 for HTTP versus HTTPS answer provenance. No reviewer or result recorded; status remains Review. |
| `2026-10-30` | `Codex (GPT-6), pass-9 writer continuation; same session ID not recorded` | Further §17 consistency: joined P14–P16 to the provenance table; §3.1/§3.2/§4.3 URL descriptions apply §2 HTTP/HTTPS policy; §5.1/§5.2 refresh summaries use the strict sampled trigger of §4.3. No new observation or review claimed. |
| 2026-10-30 | Reviewer: Codex / GPT-6, independent session; task `/root/pass9_review_coordinator/review_session_login`; UUID unavailable | PASS on writer commit `b2cc48e0dcaac912c77b13d909ed8e5e9abf485a`, gated by decision 0042 items 4 & 8; 2 OPEN. Full CLEAN_ROOM §5.3 checklist; login/session consistency, Case B arithmetic, historical-log preservation and OPEN counts; sampled launcher XML, DAT superblock and matching-backup binary identifier/constant presence. Source sampling only; no exhaustive binary re-derivation or live session. Writer session ID unavailable. Review metadata only; prior history retained. |
