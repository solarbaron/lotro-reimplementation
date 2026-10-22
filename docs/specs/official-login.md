<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Official Server Login & Launcher Service Specification

| Field | Value |
|---|---|
| Status | `Revised` (changed after its 2026-10-20 review; re-review required, decision 0041) |
| Last updated | `2026-10-20` |
| Author role | `Spec writer (dirty room)` |
| Reviewer | `Independent clean-room review (2026-10-20; passed CLEAN_ROOM.md §5.3)` |
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
   - Per-world status and login server discovery (`StatusServer.aspx` XML, including host/path fallback rewriting and billing-role checks).
   - Account authentication (`LoginAccount` SOAP 1.1 call, fault handling, and subscription selection).
   - World login queue entry, polling, and cancellation (`LoginQueue.aspx` `TakeANumber` and `LeaveQueue` form POSTs).
   - Launcher-to-client parameter hand-off (command-line template placeholders and flags).
3. **Client-side ticket intake, session hand-off, and ticket refresh**:
   - Intake of `--glsticketdirect`, `--authserverurl`, `--glsticketlifetime`, and the legacy `--glsticket` Windows registry fallback.
   - Parameters prepared and handed over to the UDP session transport layer specified in `docs/specs/official-session.md`.
   - Periodic in-game `RefreshTicket` SOAP 1.1 requests, backoff schedule, and retry limits.
4. **End-to-end state machine**, timeouts, retry rules, and user-facing failure classes, starting with local configuration discovery (`S1_LOCAL_DISCOVERY`).
5. **Security and access-control rules**, including TLS validation requirements, credential and ticket redaction, and access-control stop conditions.
6. **Testable invariants and synthetic test vectors** using exclusively RFC 2606 `.example.invalid` placeholders and synthetic tokens.

In accordance with `CLAUDE.md`, `docs/CLEAN_ROOM.md`, and `docs/decisions/0034-official-server-compatibility.md`, this specification states **only where** the launcher and client discover URLs, hostnames, ports, and service endpoints at runtime in the user's local installation or in server responses; it never embeds or publishes live endpoint values, keys, certificates, or account data.

---

## 1. Provenance <a id="s1-provenance"></a>

### 1.1 Source tags <a id="s1-1-sources"></a>

| Tag | Method | Description |
|---|---|---|
| `OBS` | M1 | Direct read-only inspection of the local LOTRO installation (`lotro.launcherconfig`, `client_local_*.dat`, `x64/lotroclient64.exe`). |
| `LL` | M2 | Project owner's *lotro-launcher* — GPL-3.0-or-later, consulted for protocol facts, XML parsing tolerances, `<queueurls>` RFC 1918 filtering, SOAP fault classification, mandatory `TakeANumber` execution, HRESULT sign-bit evaluation, and launch argument template aliases. |
| `OL` | M2 | June Stepp, *OneLauncher* — GPL-3.0, consulted for protocol facts only (local/remote config keys, SOAP envelopes, world status and queue XML schemas, and URL fallback behavior). |
| `PL` | M2 | Alan Jackson, *PyLotRO* — GPL-3.0, consulted for protocol facts only (GLS SOAP calls, world status and queue handling, and client command-line template substitution). |
| `GLS` | M2 | `lotro-gls` — consulted for SOAP 1.1 service namespaces, operation signatures, and XML element schemas (`GetDatacenters`, `LoginAccount`, `RefreshTicket`). |
| `REDDO` | M2 | Xaymar, *RE-DDO* (`protocol/launcher.md`, `protocol/game.md`) — related-engine analogue consulted for cross-checking GLS and command-line conventions. |
| `DDOL` | M2 | Florian Stinglmayr & Scott Winburn, *ddolauncher* — BSD-2-Clause, related-engine analogue consulted for queue and SOAP parameter conventions. |
| `BIN` | M4 | Static analysis via Ghidra headless (`analyzeHeadless` 11.x) of a read-only copy of `x64/lotroclient64.exe` (PE FileVersion / ProductVersion `4906.0071.1794.4035`, SHA-256 `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee`) under standing authorisation (`docs/decisions/0011-standing-m4-authorisation.md`) and the M8 amendment (`docs/decisions/0034-official-server-compatibility.md`). |
| `IL` | — | Companion specification `docs/specs/install-layout.md`. |

### 1.2 M4 (`BIN`) questions log <a id="s1-2-bin-log"></a>

| ID | Binary & Build | SHA-256 | Tool | Question | Why M1–M2 were insufficient |
|---|---|---|---|---|---|
| `BIN-Q1` | `x64/lotroclient64.exe` (`4906.0071.1794.4035`) | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra 11.x headless (`-readOnly`) | How does the game client parse and prioritize `--glsticketdirect`, `--glsticket`, `--authserverurl`, and `--glsticketlifetime`, and how does `--glsticket` load the fallback ticket from the Windows registry? | Third-party launchers (`OL`, `PL`) only construct `--glsticketdirect`; they do not document client-side intake, wide-to-narrow conversion, or the `--glsticket` registry hive and value name. |
| `BIN-Q2` | `x64/lotroclient64.exe` (`4906.0071.1794.4035`) | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra 11.x headless (`-readOnly`) | How does the game client schedule, format, transmit, and parse the in-game `RefreshTicket` SOAP call, including retry limits, backoff formula, HTTP timeout, and TLS peer verification settings? | Third-party launchers exit once the client starts; `GLS` only shows the server stub signature `RefreshTicket` without the client's timer arithmetic, retry cap, or HTTP client configuration. |
| `BIN-Q3` | `x64/lotroclient64.exe` (`4906.0071.1794.4035`) | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra 11.x headless (`-readOnly`) & `objdump` | How does the game client construct its version identifier string (`"netver:<netver>; didver:<didver>"`), where do `<netver>` and `<didver>` originate in `.rdata` and `.dat` archives, and how is that string serialized on the wire in the post-handshake `VersionSend` (`0x0000012C`) message (`docs/specs/official-session.md` §9.3)? | M2 launcher sources do not cover the client's internal hand-off to the UDP login session, the origin of `<netver>` and `<didver>`, or how the session version string is sent in `VersionSend` (`0x0000012C`) after the `"180804"` `LoginRequest` (`0x00010000`) transport handshake. |
| `BIN-Q4` | `x64/lotroclient64.exe` (`4906.0071.1794.4035`) | `d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee` | Ghidra 11.x headless (`-readOnly`) & `objdump` | How are keys `0x10000002` and `0x10000001` resolved to `.dat` records, what is the byte layout of the target records (`0x0E010000`, `0x0E010002`, `0x0E010001`), which 16 bytes from `0x10000002` are copied into the session context, and where are they negotiated in the UDP handshake? | M1 shows `0x10000001` and `0x10000002` are absent from the B-tree root directory of all `.dat` files; M4 was required to trace their resolution through DID Map `3` (`0x28000010`), the `0x0E01xxxx` compression dictionary parser, and their use in `ConnectRequest` (`0x00040000`) and `ConnectResponse` (`0x00080000`). |

### 1.3 Provenance matrix <a id="s1-3-provenance-matrix"></a>

| ID | Section | Status | Sources | Notes |
|---|---|---|---|---|
| `P1` | §2.1 Local launcher config discovery & XML keys | **Confirmed** | `OBS`, `LL`, `OL`, `PL`, `IL` | Verified against local `lotro.launcherconfig` and third-party launcher parsers. |
| `P2` | §2.2–§2.3 Language discovery & `UserPreferences.ini` | **Confirmed** | `OBS`, `LL`, `OL`, `IL` | Cross-referenced with `docs/specs/install-layout.md` §3.1, §7, and §8, including 64-bit `UserPreferences64.ini` / `UserPreferences_x64.ini` lookup order in `LL`. |
| `P3` | §2.4 Client version identifiers (`netver`, `didver`), DID Map `3` (`0x28000010`), & compression dictionaries (`0x10000001`/`0x10000002` $\rightarrow$ `0x0E01xxxx`) | **Confirmed** | `BIN` (`BIN-Q3`, `BIN-Q4`), `OBS`, `IL` | Exact runtime derivation of `<netver>` (`.rdata` compatibility version) and `<didver>` (`.dat` superblock `0x17C..0x18B` GUID), `VLUInt` serialization in `VersionSend` (`0x0000012C`), and `0x0E01xxxx` compression dictionary layout and `ConnectRequest` (`0x00040000`) / `ConnectResponse` (`0x00080000`) placement confirmed in `lotroclient64.exe` and `client_general.dat`. |
| `P4` | §3.1 `GetDatacenters` SOAP 1.1 exchange | **Confirmed** | `LL`, `OL`, `PL`, `GLS`, `REDDO`, `DDOL` | All M2 sources agree on namespace, `SOAPAction`, request body, and response schema (including optional `SupportURL` and `Order` sorting in `LL`). |
| `P5` | §3.2 Remote launcher configuration XML (`LauncherConfigurationServer`) | **Confirmed** | `LL`, `OL`, `PL`, `REDDO` | `.NET` `appSettings` schema and key names confirmed across `LL`, `OL`, `PL`, and `REDDO`. |
| `P6` | §3.3 Per-world status query (`StatusServer.aspx`), fallback rewrite, RFC 1918 queue URL selection, & billing-role check | **Confirmed** | `LL`, `OL`, `PL`, `DDOL`, `REDDO` | XML schema, case-insensitive tag matching, host/path fallback rewrite, `<queueurls>` RFC 1918 filtering, and billing-role intersection confirmed across `LL`, `OL`, and `PL`. |
| `P7` | §3.4 `LoginAccount` SOAP 1.1 exchange, faults, & subscription selection | **Confirmed** | `LL`, `OL`, `PL`, `GLS`, `REDDO`, `DDOL` | Request/response elements, `GameSubscription/Name` vs username distinction, and `BIS` SOAP fault classification confirmed across all M2 sources. |
| `P8` | §3.5 World login queue (`TakeANumber`, `LeaveQueue`) | **Confirmed** | `LL`, `OL`, `PL`, `REDDO`, `DDOL` | Form templates, URL-encoding requirements, mandatory execution when `queue_url` is non-empty, and `<Result>` HRESULT sign-bit evaluation confirmed across `LL`, `OL`, `PL`, `REDDO`, and `DDOL`. |
| `P9` | §3.6 Launcher-to-client command-line template & placeholders | **Confirmed** | `LL`, `OL`, `PL`, `REDDO`, `BIN` (`BIN-Q1`) | Template placeholders (including `{LANGUAGE}`, `{PRODUCT}`, `{AUTHSERVER}`, `{HIGHRES}`, `{USERDIR}` aliases and unreplaced token filtering in `LL`) and client CLI option table confirmed in `lotroclient64.exe`. |
| `P10` | §4.1 Client ticket intake (`--glsticketdirect` vs `--glsticket` registry fallback) | **Confirmed** | `BIN` (`BIN-Q1`) | Confirmed in `lotroclient64.exe` startup and `HKEY_CURRENT_USER` `"Ticket"` reader. |
| `P11` | §4.2 Hand-off to UDP session layer (`docs/specs/official-session.md`) | **Confirmed** | `BIN` (`BIN-Q1`, `BIN-Q3`, `BIN-Q4`), `OBS` | Session hand-off parameters, `VersionSend` (`0x0000012C`) version string, compression dictionary registration, and listener notification confirmed in `lotroclient64.exe`. |
| `P12` | §4.3 In-game `RefreshTicket` SOAP 1.1 lifecycle, backoff math, & retry cap | **Confirmed** | `BIN` (`BIN-Q2`), `GLS` | Half-remaining-lifetime backoff ($0.5 L$, $0.75 L$, $0.875 L$), 3-attempt cap, 30 s timeout, and XML response parsing confirmed in `lotroclient64.exe`. |
| `P13` | §6.2 Official client TLS peer verification setting | **Confirmed** | `BIN` (`BIN-Q2`), `PL`, `DDOL` | Confirmed in `lotroclient64.exe` SOAP transport initializer; Tessera policy overrides this to require strict TLS verification. |

---

## 2. Runtime Configuration and Local Inputs <a id="s2-runtime-config"></a>

All service bootstrap endpoints, game identifiers, and local asset metadata are read at runtime from the user's read-only LOTRO installation directory and user preferences directory (`docs/specs/install-layout.md`). No service URL, hostname, IP address, or port may be hardcoded in source code or configuration defaults.

### 2.1 Local launcher configuration (`*.launcherconfig`) <a id="s2-1-launcherconfig"></a>

To locate the bootstrap configuration file inside the game installation root directory (`<InstallRoot>`), the client searches the following relative paths in order and uses the first file that exists (`docs/specs/install-layout.md` §6.1):

1. `lotro.launcherconfig`
2. The lexicographically first file matching `*.launcherconfig` in `<InstallRoot>`
3. `TurbineLauncher.exe.config` (legacy installation fallback)

The file is a UTF-8 XML document (with or without a leading UTF-8 byte-order mark `EF BB BF`) using the `.NET` application configuration schema: a root `<configuration>` element containing `<appSettings>`, which in turn contains zero or more `<add key="..." value="..." />` elements.

| `<add key="...">` Name | Required | Meaning |
|---|---|---|
| `Launcher.DataCenterService.GLS` | Yes | Full HTTP/HTTPS URL of the GLS `DataCenterServer` SOAP 1.1 endpoint used in Exchange 1 (§3.1). |
| `DataCenter.GameName` | Yes | Game identifier string passed in `<game>` to `GetDatacenters` and used to filter `GameSubscription/Game` in Exchange 4 (§3.4). In LOTRO installations this value is `LOTRO`. |
| `DataCenter.DefaultCenter` | No | Default datacenter name in `GetDatacentersResult` (in LOTRO installations, `LOTRO`). If absent, `DataCenter.GameName` is used. |
| `Product.DocumentFolder` | No | Subdirectory name under the user's documents directory where `UserPreferences.ini` is stored (in LOTRO installations, `The Lord of the Rings Online`; see `docs/specs/install-layout.md` §8). |
| `Launcher.DocumentFolder` | No | Subdirectory name for launcher logs and state (in LOTRO installations, `Launcher`). |

If the configuration file cannot be found, is not well-formed XML, or lacks either `Launcher.DataCenterService.GLS` or `DataCenter.GameName`, the login flow fails before any network activity (`F02_CONFIG_INVALID` in §5.3).

### 2.2 Language and locale resolution <a id="s2-2-language"></a>

The set of languages installed in `<InstallRoot>` is determined by scanning `<InstallRoot>` for archives matching `client_local_<Lang>.dat` (`docs/specs/install-layout.md` §3.1 and §7):

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

INI comment lines beginning with either `;` or `#` (after trimming leading whitespace) are ignored (`LL`). Relevant keys are:

| Section | Key | Meaning |
|---|---|---|
| `[Launcher]` | `GameClientType` | Integer selector used when launching the official binary: `1` = 32-bit (`GameClient.WIN32.Filename`), `2` = 32-bit legacy (`GameClient.WIN32Legacy.Filename`), `3` = 64-bit (`GameClient.WIN64.Filename`). |
| `[Launcher]` | `LastLocale` | Last selected locale tag (mapped to the `<Lang>` token in §2.2). |
| `[General]` | `LastWorld` | Optional display name of the last world selected by the user (`LL`). |

> **Note:** Tessera never writes to the official `UserPreferences*.ini` files unless explicitly configured by the user, and stores its own optional account preferences in Tessera's own configuration directory.

### 2.4 Client version, data version, and compression dictionary bootstrap (`netver`, `didver`, and `0x10000001`/`0x10000002`) <a id="s2-4-version-ids"></a>

During client initialization (`BIN-Q3`, `BIN-Q4`, `OBS`), the client prepares two artefacts for the UDP session layer (`docs/specs/official-session.md`): (1) a combined client/data version string (`"netver:<netver_string>; didver:<didver_string>"`) sent in the post-handshake `VersionSend` (`0x0000012C`) message (`docs/specs/official-session.md` §9.3), and (2) two Huffman/prefix-tree compression dictionaries resolved via enumerator keys `0x10000002` (`Client2Server`) and `0x10000001` (`Server2ClientDeferred`) in `client_general.dat` (`docs/specs/official-session.md` §4.1.3), whose 16-byte dictionary GUIDs (`codecUuid`) are negotiated in `ConnectRequest` (`0x00040000`) and `ConnectResponse` (`0x00080000`) (`docs/specs/official-session.md` §6.3–§6.4).

#### 2.4.1 Construction and runtime extraction of `"netver:<netver_string>; didver:<didver_string>"` <a id="s2-4-1-netver-didver"></a>

The combined version string has the exact ASCII format (with no trailing newline or NUL inside the string):

`netver:<netver_string>; didver:<didver_string>`

1. **Origin and runtime extraction of `<netver_string>` (`BIN-Q3`, `OBS`)**:
   - `<netver_string>` is the client's network protocol compatibility version (e.g. a 4-digit decimal string in observed builds, or `"1"` if the compatibility singleton is unpopulated). It is a standalone string constant compiled into the `.rdata` section of `x64/lotroclient64.exe` and is **distinct from** the 4-part PE `FileVersion` / `ProductVersion` resource (`docs/specs/install-layout.md` §2.2).
   - At process startup in `x64/lotroclient64.exe`, a dynamic initializer calls the compatibility-version singleton accessor with the address of `<netver_string>` in `.rdata`; the accessor copies that string into the static version singleton and rejects any subsequent attempt to set it with the error string `"Cannot set version string twice\0"`.
   - **Clean-room runtime extraction algorithm (without executing `lotroclient64.exe`)**:
     1. Open `<InstallRoot>/x64/lotroclient64.exe` read-only, validate the DOS (`"MZ"`) and PE-32+ (`"PE\0\0"`, magic `0x020B`) headers, read `ImageBase` (`u64 LE` at optional header offset `+0x18`), and parse the section headers to map virtual addresses (VAs) to file offsets in `.text` and `.rdata`.
     2. Search `.rdata` for the unique NUL-terminated ASCII string `"Cannot set version string twice\0"` and compute its virtual address $V_{\text{err}}$.
     3. **Primary method (instruction cross-reference)**: Scan `.text` for the 7-byte RIP-relative `LEA` instruction referencing $V_{\text{err}}$ and identify the enclosing accessor function entry point $V_{\text{fn}}$ (aligned on a 16-byte boundary preceded by `CC` padding). Then scan `.text` for the dynamic initializer sequence consisting of a 7-byte `LEA RCX, [RIP + disp32]` (`48 8D 0D <disp32:i32 LE>`) immediately followed by a 5-byte `CALL rel32` (`E8 <rel32:i32 LE>`) whose target equals $V_{\text{fn}}$. Resolve $V_{\text{str}} = \text{RIP}_{\text{next}} + \text{disp32}$ in `.rdata` and read the NUL-terminated ASCII string at $V_{\text{str}}$ as `<netver_string>`.
     4. **Secondary layout fallback**: In `.rdata`, `<netver_string>` is placed on the 16-byte-aligned boundary immediately following the 32-byte `"Cannot set version string twice\0"` literal (i.e. at file offset $\text{off}(\text{"Cannot set version string twice\0"}) + 0x20$).
     5. Validate that `<netver_string>` is non-empty and consists of 1 to 16 printable ASCII characters (`0x21..0x7E`, digits `0..9` in observed builds). If extraction fails, abort before network activity with `F02_CONFIG_INVALID`.

2. **Origin and runtime extraction of `<didver_string>` (`BIN-Q3`, `OBS`)**:
   - When opening `.dat` archives (`client_gamelogic.dat`, `client_general.dat`, etc.), the client reads the first `0x1A8` (`424`) bytes of the archive from file offset `0x000` and copies the `0x68`-byte superblock header at **file offset `0x140..0x1A7`** (`"BT\x00\x00"`, see `docs/specs/dat-container.md` §3.2).
   - Within that superblock header, **file offset `0x17C..0x18F` (`20` bytes)** holds the archive's data-version descriptor:

| File Offset in `.dat` | Offset in Descriptor | Size (bytes) | Type | Byte Order | Meaning |
|---|---|---|---|---|---|
| `0x17C` | `+0x00` | `4` | `u32` | Little-endian | `didver` GUID `Data1` (formatted as 8 uppercase hex digits `%08X`). |
| `0x180` | `+0x04` | `2` | `u16` | Little-endian | `didver` GUID `Data2` (formatted as 4 uppercase hex digits `%04hX`). |
| `0x182` | `+0x06` | `2` | `u16` | Little-endian | `didver` GUID `Data3` (formatted as 4 uppercase hex digits `%04hX`). |
| `0x184` | `+0x08` | `2` | `u8[2]` | Raw byte order | `didver` GUID `Data4[0..1]` (formatted as 4 uppercase hex digits `%02X%02X`). |
| `0x186` | `+0x0A` | `6` | `u8[6]` | Raw byte order | `didver` GUID `Data4[2..7]` (formatted as 12 uppercase hex digits `%02X%02X%02X%02X%02X%02X`). |
| `0x18C` | `+0x10` | `4` | `u32` | Little-endian | Archive version integer (`1100` in base archives, `17139` in expansion cell/map archives). |

   - All `client_*.dat` archives in a consistent LOTRO installation carry the identical 16-byte GUID at file offset `0x17C..0x18B`. At runtime, the client queries the opened general/gamelogic archive handle (`<InstallRoot>/client_gamelogic.dat` or `<InstallRoot>/client_general.dat`) for this 20-byte descriptor and passes it to a formatting helper that produces both:
     - Out-parameter 1 (return value): `"<guid_string> : <version_u32>"` (`"%s : %lu"`), and
     - Out-parameter 2: `<guid_string>` alone (`"%08X-%04hX-%04hX-%02X%02X-%02X%02X%02X%02X%02X%02X"`, exactly `36` uppercase ASCII characters).
   - Crucially (`BIN-Q3`), the version-string builder **discards** out-parameter 1 (`"<guid_string> : <version_u32>"`) and appends **only out-parameter 2 (`<guid_string>`, the 36-character uppercase ASCII GUID without `" : <version_u32>"`)** after `"; didver:"`.
   - **Clean-room runtime extraction algorithm**: Read bytes `0x17C..0x18B` (`16` bytes) directly from `<InstallRoot>/client_general.dat` (or `<InstallRoot>/client_gamelogic.dat`), verify that the superblock magic at `0x140..0x143` is `42 54 00 00` (`"BT\0\0"`), and format the 16-byte mixed-endian GUID as `XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX` (`36` uppercase ASCII characters).

#### 2.4.2 Wire placement in `LoginRequest` (`0x00010000`) vs `VersionSend` (`0x0000012C`) <a id="s2-4-2-loginrequest-wire"></a>

1. **Transport `LoginRequest` (`0x00010000`, `docs/specs/official-session.md` §6.2)**:
   - The transport-level `LoginRequest` optional header (`0x00010000`) always begins at offset `0x00` with the fixed 7-byte NUL-terminated transport protocol version string `"180804\0"` (`31 38 30 38 30 34 00`), followed at offset `0x07` by `u32 LE authBlockPaddedLen` and the serialized `authBlock` carrying the account/subscription name and GLS ticket (`docs/specs/official-session.md` §6.2.1).
2. **Post-handshake `VersionSend` (`0x0000012C`, `docs/specs/official-session.md` §9.3)**:
   - Immediately after `ConnectRequest` (`0x00040000`) and `ConnectResponse` (`0x00080000`) complete the transport handshake and the session enters State `5` (`Connected`), the server sends `ReqVersionSend` (`0x0000012F`) and the client sends `VersionSend` (`0x0000012C`).
   - In `VersionSend` (`0x0000012C`), the runtime `versionString` `"netver:<netver_string>; didver:<didver_string>"` constructed in §2.4.1 is serialized at payload offset `+0x00` (after `envelopeFlags`) as a length-prefixed narrow string (`PString`: `PackedDWORD` / `VLUInt` byte length $L$ followed by $L$ raw ASCII bytes with **no** trailing NUL):

| Offset in `VersionSend` (`0x0000012C`) Inner Payload | Size (bytes) | Type | Byte Order | Meaning |
|---|---|---|---|---|
| `+0x00` | `1`, `2`, or `4` | `VLUInt` (`PackedDWORD`) | Big-endian prefix bits | Character count $L$ of `versionString` (excluding NUL):<br>• If $L \le 127$ (`0x7F`): `1` byte `u8` equal to $L$.<br>• If $128 \le L \le 16383$ (`0x3FFF`): `2` bytes `[((L >> 8) | 0x80) as u8, (L & 0xFF) as u8]`.<br>• If $L > 16383$: `4` bytes `[((L >> 24) | 0xC0) as u8, ((L >> 16) & 0xFF) as u8, ((L >> 8) & 0xFF) as u8, (L & 0xFF) as u8]`.<br>For a 4-character `<netver_string>` and 36-character `<didver_string>`, $L = 7 + 4 + 9 + 36 = 56$ (`0x38`), so this field is the single byte `0x38`. |
| `+0x01` (when $L \le 127$) | $L$ (`56` when `<netver>` is 4 chars) | `u8[L]` | Raw ASCII | Raw ASCII bytes of `"netver:<netver_string>; didver:<didver_string>"` with **no** trailing NUL byte. |

#### 2.4.3 Bootstrap `.dat` compression dictionary records (`0x10000002` and `0x10000001`) <a id="s2-4-3-compression-dicts"></a>

Immediately after building the version string, the client loads two compression dictionaries identified by enumerator keys `0x10000002` and `0x10000001` (`BIN-Q4`, `OBS`; see `docs/specs/official-session.md` §4.1.3 for the full bit-tree compression/decompression algorithm).

1. **Resolution via Master DID Map `3` (`0x28000010` in `client_general.dat`)**:
   - `0x10000002` and `0x10000001` are **not** direct `.dat` B-tree file IDs; they are enumerator keys in **DID Map `3` (`"COMPRESSIONDICTIONARY"`)** inside `<InstallRoot>/client_general.dat`.
   - To resolve a `"COMPRESSIONDICTIONARY"` enumerator key from `<InstallRoot>/client_general.dat`:
     1. Read the Master DID Map file ID from superblock offset `0x170..0x173` (`u32 LE`, value `0x28000000` in `client_general.dat`; see `docs/specs/dat-container.md` §3.2).
     2. In record `0x28000000`, subsystem index `0x00000003` (`"COMPRESSIONDICTIONARY"`) maps to DID `0x28000010`.
     3. Record `0x28000010` (`116` bytes in `client_general.dat`) is a DID map table:

| Offset in `0x28000010` | Size (bytes) | Type | Byte Order | Meaning |
|---|---|---|---|---|
| `+0x00` | `4` | `u32` | Little-endian | Record self-DID (`0x28000010`). |
| `+0x04` | `1` | `u8` | — | Map format flags (`0x00`). |
| `+0x05` | `4` | `u32` | Little-endian | Entry count $M$ (`4` in `client_general.dat`). |
| `+0x09` | $8 \times M$ (`32`) | `(u32, u32)[M]` | Little-endian | $M$ pairs of `(enum_key: u32 LE, target_did: u32 LE)`:<br>• `[0]`: `0x00000000` $\rightarrow$ `0x00000000`<br>• `[1]`: `0x10000001` $\rightarrow$ `0x0E010002`<br>• `[2]`: `0x10000002` $\rightarrow$ `0x0E010000`<br>• `[3]`: `0x10000003` $\rightarrow$ `0x0E010001` |
| `+0x29` | `4` | `u32` | Little-endian | Name count $M$ (`4`). |
| `+0x2D..end` | Variable | `(u32 LE, cstr)[M]` | Little-endian | $M$ pairs of `(enum_key: u32 LE, name: NUL-terminated ASCII)`:<br>• `0x00000000` = `"Undef"`<br>• `0x10000001` = `"Server2ClientDeferred"`<br>• `0x10000002` = `"Client2Server"`<br>• `0x10000003` = `"Server2Client"` |

2. **Byte layout of `0x0E01xxxx` CompressionDictionary records in `client_general.dat` (`BIN-Q4`, `OBS`)**:
   - Each target record (`0x0E010000`, `0x0E010001`, `0x0E010002` in `client_general.dat`, with no leading DID prologue; see `docs/specs/official-session.md` §4.1.3) has a 44-byte (`0x2C`) header followed by the serialized node stream:

| Offset in Record | Size (bytes) | Type | Byte Order | Meaning |
|---|---|---|---|---|
| `+0x00` | `16` | GUID (`u32, u16, u16, u8[8]`) | Mixed-endian | Fixed CompressionDictionary schema GUID `E795878D-C633-471C-8315-C3DBBEAD2FAA` (raw bytes `8D 87 95 E7 33 C6 1C 47 83 15 C3 DB BE AD 2F AA`). Loader rejects the record if these 16 bytes do not match. |
| `+0x10` | `16` | GUID (`u8[16]`) | Mixed-endian | **Dictionary identifier GUID (`dict_guid` / `codecUuid`)**.<br>• For `0x10000002` (`"Client2Server"` $\rightarrow$ `0x0E010000`), these 16 bytes at `+0x10..+0x1F` are stored as the client's preferred `codecUuid`.<br>• For `0x10000001` (`"Server2ClientDeferred"` $\rightarrow$ `0x0E010002`) and `0x10000003` (`"Server2Client"` $\rightarrow$ `0x0E010001`), these 16 bytes identify the server-to-client dictionary selected by `codecUuid` in `ConnectRequest` (`0x00040000`). |
| `+0x20` | `4` | `u32` | Little-endian | `deferred_target_did` (`redirectDid`).<br>• If `0x00000000` (as in `0x0E010000`), this record contains the dictionary tree inline.<br>• If non-zero (as in `0x0E010002`, where `deferred_target_did == 0x0E010001`), this record is a 44-byte deferred stub; the loader immediately loads the record with DID `deferred_target_did` (`0x0E010001`) in its place. |
| `+0x24` | `4` | `u32` | Little-endian | `dict_format_version` (`version`). Must equal `1` in a non-deferred dictionary record (`0` in the 44-byte stub `0x0E010002`). |
| `+0x28` | `4` | `u32` | Little-endian | `tree_stream_len` (`bodyLen`) — byte length of the serialized prefix-tree stream starting at `+0x2C` (`0` in `0x0E010002`; `218,203` in `0x0E010000`; `214,909` in `0x0E010001`). |
| `+0x2C` | `4` | `u32` | Little-endian | `node_count` (present when `tree_stream_len > 0`) — total number of nodes in the Huffman/prefix tree (`32,743` in `0x0E010000`; `32,249` in `0x0E010001`). |
| `+0x30..+0x2C + tree_stream_len` | `tree_stream_len - 4` | Node stream | Little-endian | `node_count` consecutive tree nodes (`1`-based node indices `1..node_count`):<br>• `is_leaf` (`u8`):<br>  - If `is_leaf != 0` (leaf node): `symbol_len` (`u8`), `symbol_bytes` (`u8[symbol_len]`), `bit_len` (`u8`), `codeword_bytes` (`u8[(bit_len >> 3) + 1]`).<br>  - If `is_leaf == 0` (internal branch node): `left_child_index_1based` (`u16 LE`), `right_child_index_1based` (`u16 LE`). |

3. **Wire placement and negotiation in the UDP handshake (`BIN-Q4`, `docs/specs/official-session.md` §4.1.3, §6.3–§6.4)**:
   - During network session initialization, both resolved dictionaries (`0x10000002` $\rightarrow$ `0x0E010000` and `0x10000001` $\rightarrow$ `0x0E010002` $\rightarrow$ `0x0E010001`) are registered in the session layer's codec table keyed by their 16-byte `dict_guid` (`codecUuid` at `+0x10..+0x1F`), and the 16-byte `dict_guid` of `0x10000002` (`0x0E010000` bytes `+0x10..+0x1F`) is stored as the client's outgoing `codecUuid`.
   - When `ConnectRequest` (`0x00040000`, `docs/specs/official-session.md` §6.3, 44-byte payload) arrives from the server, the client reads the server's 16-byte `codecUuid` at `ConnectRequest` payload offset `+0x1C..+0x2B`:
     - If `codecUuid` is 16 zero bytes, packet bit-tree compression/decompression is disabled (`wirePrefix = 0x0000`).
     - Otherwise, the client looks up `codecUuid` in its loaded dictionary table. If `codecUuid` is not found in the table, the client aborts connection setup.
   - Immediately upon accepting `ConnectRequest`, the client transmits `ConnectResponse` (`0x00080000`, `docs/specs/official-session.md` §6.4, 28-byte payload), placing the 16-byte `codecUuid` at payload offset `+0x0C..+0x1B` (after the 8-byte `checkValue` at `+0x00..+0x07`, 2-byte `localRecId` at `+0x08..+0x09`, and 2-byte `clientConnIdx` at `+0x0A..+0x0B`).

---

## 3. Launcher-Side Service Exchanges <a id="s3-launcher-exchanges"></a>

All network exchanges in §3 use endpoints discovered at runtime in §2.1 or returned by earlier exchanges.

### 3.1 Exchange 1 — Datacenter and world discovery (`GetDatacenters`) <a id="s3-1-get-datacenters"></a>

The first network exchange queries the GLS `DataCenterServer` SOAP 1.1 service to discover the authentication server URL, remote launcher configuration URL, patch server address, and the list of game worlds (`P4`).

#### Request parameters

| Parameter | Value |
|---|---|
| Endpoint URL | Runtime value of `Launcher.DataCenterService.GLS` from `lotro.launcherconfig` (§2.1). |
| HTTP Method | `POST` |
| `Content-Type` header | `text/xml; charset=utf-8` |
| `SOAPAction` header | `"http://www.turbine.com/SE/GLS/GetDatacenters"` |
| Request encoding | UTF-8 XML with XML 1.0 declaration `<?xml version="1.0" encoding="utf-8"?>` |

#### Request XML structure

| XPath (relative to root) | Namespace | Content / Meaning |
|---|---|---|
| `/soap:Envelope` | `http://schemas.xmlsoap.org/soap/envelope/` | Standard SOAP 1.1 envelope (also declares `xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"` and `xmlns:xsd="http://www.w3.org/2001/XMLSchema"`). |
| `/soap:Envelope/soap:Body/GetDatacenters` | `http://www.turbine.com/SE/GLS` | Operation element. |
| `/soap:Envelope/soap:Body/GetDatacenters/game` | `http://www.turbine.com/SE/GLS` | Game identifier string from `DataCenter.GameName` in `lotro.launcherconfig` (e.g. `LOTRO`). |

#### Response XML structure (`HTTP 200 OK`)

The response is a SOAP 1.1 envelope in namespace `http://www.turbine.com/SE/GLS` under `/soap:Envelope/soap:Body/GetDatacentersResponse/GetDatacentersResult`:

| Element Path (under `GetDatacentersResult`) | Type | Cardinality | Meaning |
|---|---|---|---|
| `Datacenter` | Complex | `1..*` | Datacenter entry. Select the `Datacenter` whose `Name` equals `DataCenter.GameName` (or `DataCenter.DefaultCenter`). If no `Datacenter` matches, fail with `F03_SERVICE_UNAVAILABLE`. |
| `Datacenter/Name` | String | `1` | Datacenter identifier (matches `DataCenter.GameName`). |
| `Datacenter/AuthServer` | String (URL) | `1` | Runtime HTTPS URL of the GLS authentication service (`GLS.AuthServer/Service.asmx`), used for `LoginAccount` (§3.4) and passed to the client as `--authserverurl` for `RefreshTicket` (§4.3). |
| `Datacenter/PatchServer` | String (`host:port`) | `1` | Patch server address (used by the patcher; not contacted during login). |
| `Datacenter/LauncherConfigurationServer` | String (URL) | `1` | Runtime URL of the remote launcher configuration XML file (`lotrolauncher.server.config.xml`), fetched in Exchange 2 (§3.2). |
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
| `WorldQueue.LoginQueue.URL` | Yes | Runtime HTTPS URL of the login queue service (`LoginQueue.aspx`) used in Exchange 5 (§3.5). |
| `WorldQueue.TakeANumber.Parameters` | Yes | `.NET` format template (`{0}`, `{1}`, `{2}`) for the `TakeANumber` POST body in Exchange 5 (§3.5): `command=TakeANumber&subscription={0}&ticket={1}&ticket_type=GLS&queue_url={2}`. |
| `WorldQueue.LeaveQueue.Parameters` | No | `.NET` format template (`{0}`, `{1}`, `{2}`) for the `LeaveQueue` POST body in Exchange 5 (§3.5): `command=LeaveQueue&subscription={0}&context={1}&ticket_type=GLS&queue_url={2}`. |
| `WorldQueue.PollTimer` | No | Nominal queue poll interval in seconds configured by the server (e.g., `120`). |
| `WorldQueue.Threshold.WaitTimeMultiplier` | No | Multiplier for queue wait-time display estimation. |
| `WorldQueue.Threshold.Medium` | No | Queue threshold in seconds for medium wait indicator. |
| `WorldQueue.Threshold.Long` | No | Queue threshold in seconds for long wait indicator. |
| `URL.NewsFeed` | No | Optional newsfeed URL template where `{0}` is replaced with the two-letter language code (`en`, `fr`, `de`). |
| `URL.Alerts` | No | Optional alerts URL template where `{0}` is replaced with the two-letter language code (`en`, `fr`, `de`). |

---

### 3.3 Exchange 3 — Per-world status query (`StatusServer.aspx`) <a id="s3-3-world-status"></a>

To check whether a selected world is online, verify subscription billing-role permissions, and discover its active login server `host:port` and queue URL, the client queries the world's `StatusServerUrl` (`P6`). When refreshing status across multiple worlds for display in a world list, requests should be issued sequentially with a `10 s` per-world timeout (`LL`) so the status server is not rate-limited or overwhelmed.

#### Request parameters and URL fallback rule

| Parameter | Value |
|---|---|
| Primary Endpoint URL | Runtime value of `World/StatusServerUrl` for the selected world from Exchange 1 (§3.1). |
| HTTP Method | `GET` |
| Fallback Endpoint Rewrite Rule | Some worlds in `GetDatacentersResult` return a legacy unreachable host or outdated path prefix in `World/StatusServerUrl` that yields `HTTP 404 Not Found` or an empty response body (`Confirmed` in `OL`). If the initial `GET` to `World/StatusServerUrl` returns HTTP 404 or a zero-length body, the client constructs a fallback URL by taking the scheme, host, port, and service base path of `Launcher.DataCenterService.GLS` (removing its trailing `/GLS.DataCenterServer/Service.asmx` segments), appending `/StatusServer.aspx`, and preserving the query string (`?...`) from `World/StatusServerUrl`, then retries the `GET` once. |

#### Response XML structure (`HTTP 200 OK`)

The response is a UTF-8 XML document with root element `<Status>` and no XML namespace. Child element names under `<Status>` should be matched **case-insensitively** (`LL`):

| Element Name (under `<Status>`) | Type | Required | Meaning |
|---|---|---|---|
| `name` | String | No | World name string. |
| `loginservers` | String | Yes | Semicolon-separated (`;`) list of login server `host:port` endpoints (e.g., `login1.example.invalid:9000;login2.example.invalid:9000;`). Split on `;`, trim whitespace, and discard empty segments and `(invalid service specified)`; the **first non-empty segment** is the world's `login_server` (`host:port`). If no non-empty segment exists, the world is unavailable (`F06_WORLD_CLOSED`). |
| `queueurls` | String | Yes | Semicolon-separated (`;`) list of world queue URLs (e.g., `http://queue1.example.invalid:7080/command;http://queue2.example.invalid:7080/command;` or `;`). Split on `;`, trim whitespace, and discard empty segments. **Queue URL selection rule (`LL`):** Some worlds list internal RFC 1918 private IPv4 literals (`10.0.0.0/8`, `172.16.0.0/12` i.e. `172.16.*`–`172.31.*`, or `192.168.0.0/16`) alongside or instead of public hostnames because `queue_url` is passed as an identifier parameter to `LoginQueue.aspx` (§3.5) rather than dialed directly by the client. When selecting the primary `queue_url` from the non-empty segments, prefer the first segment whose host is **not** an RFC 1918 private IPv4 address (`://10.`, `://192.168.`, `://172.16.`..`://172.31.`); if all non-empty segments use RFC 1918 addresses, fall back to the first non-empty segment (`LL`). If no non-empty segment exists, `queue_url` is `None`. |
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
| `SOAPAction` header | `"http://www.turbine.com/SE/GLS/LoginAccount"` |
| Request encoding | UTF-8 XML with XML 1.0 declaration `<?xml version="1.0" encoding="utf-8"?>` |

#### Request XML structure

| XPath (relative to root) | Namespace | Content / Meaning |
|---|---|---|
| `/soap:Envelope` | `http://schemas.xmlsoap.org/soap/envelope/` | SOAP 1.1 envelope. |
| `/soap:Envelope/soap:Body/LoginAccount` | `http://www.turbine.com/SE/GLS` | Operation element. |
| `/soap:Envelope/soap:Body/LoginAccount/username` | `http://www.turbine.com/SE/GLS` | User-entered account login name, XML-escaped (`&`, `<`, `>`, `"`, `'` escaped as XML entities). |
| `/soap:Envelope/soap:Body/LoginAccount/password` | `http://www.turbine.com/SE/GLS` | User-entered account password, XML-escaped. |
| `/soap:Envelope/soap:Body/LoginAccount/additionalInfo` | `http://www.turbine.com/SE/GLS` | Empty element (`<additionalInfo></additionalInfo>`). |

#### Response XML structure (`HTTP 200 OK`)

A successful authentication returns `/soap:Envelope/soap:Body/LoginAccountResponse/LoginAccountResult` in namespace `http://www.turbine.com/SE/GLS`:

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
| `Ticket` | String | `1` | Non-empty opaque ASCII base64-encoded GLS session ticket (~180–256 characters, containing alphanumeric characters, `+`, `/`, and `=`). |

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

### 3.5 Exchange 5 — World login queue (`LoginQueue.aspx`: `TakeANumber` and `LeaveQueue`) <a id="s3-5-world-queue"></a>

#### Entry condition (`LL`, `OL`, `DDOL`)

After Exchange 3 (`StatusServer.aspx`), Exchange 4 (`LoginAccount`), subscription selection, and the billing-role check (§3.3) have all succeeded (`P8`):

- Let `queue_url` be the primary queue URL selected from `<queueurls>` in the selected world's `<Status>` XML (§3.3).
- **Mandatory queue registration when `queue_url` is non-empty (`LL`, `OL`, `DDOL`):** Whenever `queue_url` is non-empty, the client **must** execute at least one `TakeANumber` request against `WorldQueue.LoginQueue.URL` before connecting to the world's UDP login server — even when `lastassignedqueuenumber <= nowservingqueuenumber` — because the world server rejects the subsequent UDP session connection if the subscription and GLS ticket have not been registered with the login queue (`LL`). Only when `queue_url` itself is empty (`None`) is Exchange 5 skipped.

#### Request parameters (`TakeANumber` and `LeaveQueue`)

| Parameter | Value |
|---|---|
| Endpoint URL | Runtime value of `WorldQueue.LoginQueue.URL` from Exchange 2 (§3.2). |
| HTTP Method | `POST` |
| `Content-Type` header | `application/x-www-form-urlencoded` |
| `TakeANumber` POST body | Constructed from `WorldQueue.TakeANumber.Parameters` (`command=TakeANumber&subscription={0}&ticket={1}&ticket_type=GLS&queue_url={2}`), where:<br>• `{0}` = selected `GameSubscription/Name` (ASCII alphanumeric; percent-encode if any non-unreserved characters exist)<br>• `{1}` = **URL-encoded** (`application/x-www-form-urlencoded` / RFC 3986: `+` $\rightarrow$ `%2B`, `/` $\rightarrow$ `%2F`, `=` $\rightarrow$ `%3D`, space $\rightarrow$ `+` or `%20`) `LoginAccountResult/Ticket`<br>• `{2}` = **URL-encoded** (`:` $\rightarrow$ `%3A`, `/` $\rightarrow$ `%2F`, `?` $\rightarrow$ `%3F`, `&` $\rightarrow$ `%26`, `=` $\rightarrow$ `%3D`) `queue_url` from `<Status>/<queueurls>`. |
| `LeaveQueue` POST body | Sent if the user cancels while waiting in the queue. Constructed from `WorldQueue.LeaveQueue.Parameters` (`command=LeaveQueue&subscription={0}&context={1}&ticket_type=GLS&queue_url={2}`), where `{0}` is `GameSubscription/Name`, `{1}` is the `ContextNumber` from the last `TakeANumber` response, and `{2}` is the URL-encoded `queue_url`. |

> **Important encoding and template-substitution invariants (`LL`, `OL`):**
> 1. Because `Ticket` is base64-encoded and contains `+`, `/`, and `=`, failing to percent-encode `{1}` in the `application/x-www-form-urlencoded` body corrupts `+` into spaces (`0x20`) on the server and causes `TakeANumber` to fail with a negative `HResult`.
> 2. When substituting `{0}`, `{1}`, and `{2}` into `WorldQueue.TakeANumber.Parameters`, implementations **must not** pass the already-percent-encoded strings through a sequential positional formatter that interprets `%` sequences (such as `%2F` or `%3D`) as format specifiers (`LL`).

#### Response XML structure (`HTTP 200 OK`)

The response is a UTF-8 XML document with root element `<Result>` and no XML namespace:

| Element Name (under `<Result>`) | Type | Required | Meaning |
|---|---|---|---|
| `Command` | String | No | Echoes the command name (e.g., `TakeANumber`). |
| `HResult` | Hex string | Conditional | 32-bit HRESULT formatted with a `0x` prefix (e.g., `0x00000000` or `0x80004005`), parsed as a base-16 unsigned 32-bit integer `hr`. **Sign-bit rule (`LL`):** Per standard Windows `HRESULT` conventions (`FAILED(hr)`), the operation failed if and only if bit 31 is set (`(hr & 0x80000000) != 0`, `F08_QUEUE_ERROR`); any non-negative `hr` with bit 31 == `0` (`(hr & 0x80000000) == 0`) is treated as success. **Fallback tolerance (`LL`):** If `<HResult>` is absent from `<Result>`, the response is still treated as successful (`hr = 0`) provided both `<QueueNumber>` and `<NowServingNumber>` are present and valid. |
| `QueueName` | String | No | Internal queue name. |
| `QueueNumber` | Hex string | Yes (on success) | The user's assigned queue number with a `0x` prefix (e.g., `0x0000002B`), parsed as base-16 unsigned integer. |
| `NowServingNumber` | Hex string | Yes (on success) | The queue number currently being admitted with a `0x` prefix (e.g., `0x0000002A`), parsed as base-16 unsigned integer. |
| `LoginTier` | Integer/Hex string | No | Login priority tier assigned to the subscription. |
| `ContextNumber` | Hex string | No | Queue context handle with a `0x` prefix, used if `LeaveQueue` is called. |
| `WaitTimeSamples` | Complex | No | Historical wait time samples by tier. |

#### Queue polling loop and ready condition

1. Parse `HResult` as base-16 integer `hr` (defaulting to `0` if `<HResult>` is absent and both `<QueueNumber>` and `<NowServingNumber>` are present). If `(hr & 0x80000000) != 0` (bit 31 is set, such as `0x80004005` `E_FAIL`, which occurs if the world is closed, the ticket is invalid, or the client is polling too rapidly), abort or back off (`F08_QUEUE_ERROR`).
2. Parse `QueueNumber` ($q$) and `NowServingNumber` ($s$) as base-16 integers.
3. **Ready condition**: If $q \le s$, the user has reached the front of the queue; exit the queue loop immediately and proceed to §3.6.
4. **Waiting condition**: If $q > s$, the number of players ahead of the user is $q - s$. Display the queue position $q - s$ to the user, wait for the poll interval (between `5` seconds and `WorldQueue.PollTimer` seconds; polling faster than `2` seconds risks `0x80004005` rate-limiting), and repeat the `TakeANumber` POST unless the user cancels (in which case send `LeaveQueue` if `ContextNumber` is known and transition to `SF_ABORTED`).

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
| `{SUPPORTSERVICEURL}` | — | `GameClient.Arg.supportserviceurl` from Exchange 2 | In-game support SOAP endpoint URL (`SubmitTicket.asmx`). |
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

This section specifies what the game client (`x64/lotroclient64.exe`, confirmed via `BIN-Q1`–`BIN-Q3`) does with the hand-off parameters at startup and during an active session.

### 4.1 Command-line and registry ticket intake <a id="s4-1-ticket-intake"></a>

#### Login-related command-line options (`BIN-Q1`)

| Short Flag | Long Flag | Value Type | Meaning |
|---|---|---|---|
| `-a` | `--account` | Wide string | Subscription name (`GameSubscription/Name`) to log on with. |
| `-h` | `--host` | Narrow string | Target login server `host` or `host:port` to contact. |
| `-p` | `--port` | `u32` (`1..65535`) | Target login server UDP port (when not embedded as `:port` in `-h`). |
| — | `--outport` | `u32` (`1..65535`) | Local outgoing UDP port to bind (optional; OS ephemeral port if omitted). |
| `-z` | `--glsticketdirect` | Wide string | Direct GLS authentication ticket string. |
| — | `--glsticket` | Narrow string | Windows registry subkey under `HKEY_CURRENT_USER` from which to load the `"Ticket"` value if `--glsticketdirect` is not used. |
| — | `--authserverurl` | Wide string | GLS `AuthServer` URL for refreshing the GLS ticket. |
| — | `--glsticketlifetime` | `u32` (seconds) | Lifetime of GLS tickets in seconds. |
| — | `--language` | Narrow string | Language to run the client in (`English`, `EN_GB`, `FR`, `DE`). |
| — | `--chatserver` | Narrow string | Chat server `host:port` to connect to. |
| — | `--usechat` | Flag (boolean) | Enables chat server connection. |
| — | `--rodat` | Ignored argument | Retained only for backward compatibility; ignored by the client. |
| — | `--gametype` | Wide string | Game type identifier (`LOTRO`) used for support URLs. |
| `-f` | `--franchise` | Narrow string | Franchise name. |
| `-u` | `--user` | Wide string | Optional character name to auto-select on login. |

#### Ticket intake precedence (`BIN-Q1`)

At client startup, the client resolves its initial GLS ticket using the following strict precedence:

1. **Primary path — Direct CLI ticket (`--glsticketdirect` + `--authserverurl`)**:
   - Checked first: both `--authserverurl` and `--glsticketdirect` must be non-empty strings.
   - The client initializes its in-game ticket refresh handler with an XML SAX parser.
   - The client converts `--authserverurl` and `--glsticketdirect` from UTF-16 wide strings to UTF-8/ASCII (a direct character-encoding conversion, **not** URL percent-decoding).
   - The client stores the ticket string as the active account authentication ticket, initializes the refresh timer (§4.3) with the 32-bit `--glsticketlifetime` value (in seconds), and notifies registered network session listeners with the `(ticket, account_name)` pair.
2. **Fallback path — Windows registry key (`--glsticket`)**:
   - Used only if `--authserverurl` or `--glsticketdirect` is empty and `--glsticket` is non-empty.
   - The client opens `HKEY_CURRENT_USER\<--glsticket>` with read access (`KEY_READ = 0x20019`) and reads the string value named `"Ticket"` into a 4096-byte buffer (maximum string length `4095` bytes), then closes the registry key.
   - If the registry read succeeds, the client stores the ticket string as the active account authentication ticket and notifies network session listeners. Note that the in-game ticket refresh handler is **not** created in this fallback path, so periodic `RefreshTicket` calls do not run when `--authserverurl` or `--glsticketdirect` is omitted.
   - If the registry key or `"Ticket"` value is missing or unreadable, the client aborts startup (`F09_TICKET_INACCESSIBLE`).

---

### 4.2 Hand-off to the UDP session layer (`docs/specs/official-session.md`) <a id="s4-2-session-handoff"></a>

Once ticket intake (§4.1) and version/data initialization (§2.4) succeed, the login layer hands the following runtime values to the UDP session transport layer specified in `docs/specs/official-session.md` (`P11`):

| Parameter | Source | Value Handed to `docs/specs/official-session.md` |
|---|---|---|
| Login server endpoint | `-h` (`--host`) and `-p` (`--port`) CLI options | Target UDP IPv4/hostname and port from the first non-empty entry of `<Status>/<loginservers>` (§3.3). |
| Local UDP bind port | `--outport` CLI option | Optional local UDP port (`1..65535`), or `0` (ephemeral). |
| Subscription / Account ID | `-a` (`--account`) CLI option | Selected `GameSubscription/Name` string from `LoginAccountResult` (§3.4). |
| GLS Session Ticket | `--glsticketdirect` (or `--glsticket` registry fallback) | Raw ASCII ticket string from `LoginAccountResult/Ticket` (and updated on `RefreshTicket`). |
| Client & Data Version (`versionString`) | Version builder (§2.4.1–§2.4.2) | Runtime ASCII string `"netver:<netver_string>; didver:<didver_string>"` (where `<didver_string>` is the 36-char uppercase GUID from `.dat` superblock `0x17C..0x18B`), serialized with a `VLUInt` (`PackedDWORD`) length prefix at offset `+0x00` of `VersionSend` (`0x0000012C`; `docs/specs/official-session.md` §9.3), after the transport `LoginRequest` (`0x00010000`; `"180804\0"` header, `docs/specs/official-session.md` §6.2) completes the handshake. |
| Compression Dictionaries & `codecUuid` | `.dat` DID Map `3` (`0x28000010` in `client_general.dat`, §2.4.3) | Loaded `Client2Server` (`0x10000002` $\rightarrow$ `0x0E010000`) and `Server2Client` (`0x10000001` $\rightarrow$ `0x0E010002` $\rightarrow$ `0x0E010001`) dictionaries keyed by their 16-byte `dict_guid` (`codecUuid` at `+0x10..+0x1F`), negotiated via `ConnectRequest` (`0x00040000` offset `+0x1C..+0x2B`) and `ConnectResponse` (`0x00080000` offset `+0x0C..+0x1B`; `docs/specs/official-session.md` §6.3–§6.4). |
| Client Language | `--language` CLI option | Selected `<Lang>` string (`English`, `EN_GB`, `FR`, `DE`). |

All subsequent UDP packet framing, connection handshake messages, cryptographic key exchange, encryption, and world-server redirection are specified in `docs/specs/official-session.md`.

---

### 4.3 In-game GLS ticket refresh (`RefreshTicket`) <a id="s4-3-refresh-ticket"></a>

While the game client is running, its ticket refresh handler (`BIN-Q2`) periodically refreshes the active GLS ticket so that in-game services that consume the GLS ticket (such as customer-support SOAP calls `NewTicket` to `--supportserviceurl`, which embed `<glsTicket>...</glsTicket>`) continue to have a valid ticket across multi-hour sessions.

#### State variables of the ticket refresh handler

| Variable | Type | Initial Value | Meaning |
|---|---|---|---|
| `auth_server_url` | String | `--authserverurl` | HTTPS URL of `GLS.AuthServer/Service.asmx`. |
| `lifetime_sec` | `u32` | `--glsticketlifetime` | Nominal ticket lifetime $L$ in seconds (typically `21600`). |
| `start_time_sec` | `f64` | $t_0$ (monotonic clock) | Timestamp when the current ticket was installed. |
| `parsed_result_flag` | `bool` | `false` | Cleared before XML parsing; set to `true` when `<RefreshTicketResult>` is parsed. |
| `attempt_count` | `u32` | `0` | Number of `RefreshTicket` attempts made for the current ticket (`0..3`). |
| `next_refresh_offset_sec` | `f64` | $0.5 \times L$ | Elapsed seconds after `start_time_sec` at which the next refresh attempt will fire. |

#### Scheduling and exponential half-life backoff algorithm (`BIN-Q2`)

On every periodic update tick of the ticket refresh handler at monotonic time $t$:

1. **Guard checks**:
   - If `lifetime_sec` ($L$) is `0`, return immediately (ticket refresh disabled).
   - Compute elapsed age of the current ticket: $\Delta t = t - \text{start\_time\_sec}$.
   - If $\Delta t \le \text{next\_refresh\_offset\_sec}$, return immediately (not yet time to refresh).
   - If $\text{attempt\_count} > 2$ (i.e. 3 attempts have already been made for the current ticket), return immediately (no further retries).
2. **Read current ticket and account**:
   - Read the current active ticket string and subscription/account name. If either is empty or absent, return immediately.
3. **Update backoff schedule and attempt counter**:
   - Advance `next_refresh_offset_sec` by half of the remaining lifetime at time $t$:
     $$\text{next\_refresh\_offset\_sec} \leftarrow \text{next\_refresh\_offset\_sec} + 0.5 \times \bigl(L - (t - \text{start\_time\_sec})\bigr)$$
   - Increment `attempt_count`:
     $$\text{attempt\_count} \leftarrow \text{attempt\_count} + 1$$
   - If $\text{attempt\_count} > 2$ (this is the 3rd and final attempt), record a warning that failure of this final refresh will prevent submitting in-game support tickets.
   - Transmit the `RefreshTicket` SOAP request.

For the standard lifetime $L = 21600\text{ s}$ (6 hours), the three attempts occur at:

| Attempt (`attempt_count`) | Trigger Time ($t - t_0$) | Fraction of $L$ | Remaining Lifetime at Trigger | Next Offset Scheduled (if attempt fails) |
|---|---|---|---|---|
| `1` | $10800\text{ s}$ (`3h 00m`) | $0.500 \times L$ | $10800\text{ s}$ | $10800 + 0.5 \times 10800 = 16200\text{ s}$ |
| `2` | $16200\text{ s}$ (`4h 30m`) | $0.750 \times L$ | $5400\text{ s}$ | $16200 + 0.5 \times 5400 = 18900\text{ s}$ |
| `3` (final) | $18900\text{ s}$ (`5h 15m`) | $0.875 \times L$ | $2700\text{ s}$ | $18900 + 0.5 \times 2700 = 20250\text{ s}$ (never fires because $\text{attempt\_count} = 3 > 2$) |

#### `RefreshTicket` SOAP 1.1 request (`BIN-Q2`, `GLS`)

| Parameter | Value |
|---|---|
| Endpoint URL | `auth_server_url` (`--authserverurl`) |
| HTTP Method | `POST` |
| `Content-Type` header | `text/xml; charset=utf-8` |
| `SOAPAction` header | `"http://www.turbine.com/SE/GLS/RefreshTicket"` |
| HTTP Timeout | `30` seconds |
| Request XML Structure | SOAP 1.1 `/soap:Envelope/soap:Body/RefreshTicket` in namespace `http://www.turbine.com/SE/GLS`, containing a single child element `<ticket>` whose text is the current raw GLS ticket string. |

#### `RefreshTicket` SOAP 1.1 response handling (`BIN-Q2`)

1. **Success path (`HTTP 200 OK` with `<RefreshTicketResult>`)**:
   - The response body is parsed for the character data of `/soap:Envelope/soap:Body/RefreshTicketResponse/RefreshTicketResult` in namespace `http://www.turbine.com/SE/GLS`.
   - When `<RefreshTicketResult>` is present, `parsed_result_flag` is set to `true` and the new ticket is installed: the active authentication ticket is updated to `new_ticket`, `start_time_sec` is reset to the current time $t$, `next_refresh_offset_sec` is reset to $0.5 \times L$, and `attempt_count` is reset to `0`.
   - If `new_ticket` is non-empty and differs from the previous ticket string, active network session listeners are notified with `(new_ticket, account_name)`.
2. **Failure path (HTTP/network error, SOAP Fault, or missing `<RefreshTicketResult>`)**:
   - If `parsed_result_flag` remains `false` after XML parsing, the client treats the response as `0x80004005` (`E_FAIL`).
   - The existing ticket is left untouched, and `attempt_count` and `next_refresh_offset_sec` retain the values updated in step 3 of the scheduling algorithm so that the next retry (if `attempt_count <= 2`) occurs at the scheduled backoff time.

---

## 5. End-to-End State Machine, Timeouts, Retries, and Failure Classes <a id="s5-state-machine"></a>

### 5.1 State machine transitions <a id="s5-1-states"></a>

| State | Entry Condition | Action | Outgoing Transitions |
|---|---|---|---|
| `S1_LOCAL_DISCOVERY` | User selects an official server installation or launches official login mode. | Locate and parse `lotro.launcherconfig` (§2.1); scan `client_local_<Lang>.dat` (§2.2); read optional `UserPreferences.ini` (§2.3). | • Valid config & language archive present $\rightarrow$ `S2_GET_DATACENTERS`<br>• Missing/corrupt config or missing language `.dat` $\rightarrow$ `SF_FAILED(F02_CONFIG_INVALID)` |
| `S2_GET_DATACENTERS` | Local config loaded. | Send `GetDatacenters` SOAP 1.1 request to `Launcher.DataCenterService.GLS` (§3.1). | • `HTTP 200` & matching `Datacenter` found $\rightarrow$ `S3_GET_LAUNCHER_CONFIG`<br>• Network/HTTP/SOAP error or no matching `Datacenter` $\rightarrow$ `SF_FAILED(F03_SERVICE_UNAVAILABLE)` |
| `S3_GET_LAUNCHER_CONFIG` | `Datacenter` descriptor obtained. | Send `GET` to `Datacenter/LauncherConfigurationServer` (§3.2). Populate world list in UI. | • `HTTP 200` & valid XML $\rightarrow$ `S4_AWAIT_CREDENTIALS_AND_WORLD`<br>• Network/HTTP/XML error $\rightarrow$ `SF_FAILED(F03_SERVICE_UNAVAILABLE)` |
| `S4_AWAIT_CREDENTIALS_AND_WORLD` | Remote launcher config and world list loaded. | Prompt user for account `username`, `password`, and target `World`. | • User submits non-empty credentials and world $\rightarrow$ `S5_LOGIN_ACCOUNT`<br>• User cancels $\rightarrow$ `SF_ABORTED` |
| `S5_LOGIN_ACCOUNT` | User submitted credentials and selected world. | Send `LoginAccount` SOAP 1.1 request to `Datacenter/AuthServer` (§3.4). Filter `GameSubscription` where `Game == DataCenter.GameName`. | • `HTTP 200` & 1 matching subscription $\rightarrow$ `S7_QUERY_WORLD_STATUS`<br>• `HTTP 200` & $>1$ matching subscriptions $\rightarrow$ `S6_SELECT_SUBSCRIPTION`<br>• `HTTP 200` & 0 matching subscriptions $\rightarrow$ `SF_FAILED(F05_NO_GAME_SUBSCRIPTION)`<br>• SOAP `Fault` (`BIS`) $\rightarrow$ `SF_FAILED(F04_AUTH_REJECTED)` (return to `S4`)<br>• Network/HTTP error $\rightarrow$ `SF_FAILED(F03_SERVICE_UNAVAILABLE)` |
| `S6_SELECT_SUBSCRIPTION` | `LoginAccountResult` has $>1$ subscriptions for `DataCenter.GameName`. | Prompt user to select one subscription by `Description` / `Name` (§3.4). | • User selects subscription $\rightarrow$ `S7_QUERY_WORLD_STATUS`<br>• User cancels $\rightarrow$ `SF_ABORTED` |
| `S7_QUERY_WORLD_STATUS` | Subscription selected (`GameSubscription/Name` and `ProductTokens` known). | Send `GET` to `World/StatusServerUrl` (with fallback rewrite on HTTP 404/empty body; §3.3). Verify `<loginservers>` non-empty and check `allow_billing_role` / `deny_billing_role`. | • World open, role permitted, and `queue_url` non-empty $\rightarrow$ `S8_WORLD_QUEUE`<br>• World open, role permitted, and `queue_url` empty $\rightarrow$ `S9_SESSION_HANDOFF`<br>• `loginservers` empty or status unavailable $\rightarrow$ `SF_FAILED(F06_WORLD_CLOSED)`<br>• Billing role check failed $\rightarrow$ `SF_FAILED(F07_WORLD_ROLE_DENIED)` |
| `S8_WORLD_QUEUE` | World requires login queue (`queue_url` non-empty). | Send `TakeANumber` POST to `WorldQueue.LoginQueue.URL` (§3.5). Poll while `QueueNumber > NowServingNumber`. | • `HResult == 0` and `QueueNumber <= NowServingNumber` $\rightarrow$ `S9_SESSION_HANDOFF`<br>• `HResult != 0` or HTTP error $\rightarrow$ `SF_FAILED(F08_QUEUE_ERROR)`<br>• User cancels (send `LeaveQueue` if `ContextNumber` known) $\rightarrow$ `SF_ABORTED` |
| `S9_SESSION_HANDOFF` | Queue passed (or not required). | Initialize ticket refresh handler with `Ticket`, `AuthServer`, and `glsticketlifetime` (§4.1); prepare `netver`/`didver` (§2.4); hand off `(login_server, GameSubscription/Name, Ticket, version_info, language)` to `docs/specs/official-session.md` (§4.2). | • Session transport connected $\rightarrow$ `S10_IN_GAME_TICKET_REFRESH`<br>• Ticket missing/inaccessible $\rightarrow$ `SF_FAILED(F09_TICKET_INACCESSIBLE)` |
| `S10_IN_GAME_TICKET_REFRESH` | Active game session running. | Run periodic ticket refresh check (§4.3): send `RefreshTicket` at $0.5 L$ (with retries at $0.75 L$ and $0.875 L$, max 3 attempts). | • `RefreshTicketResult` received $\rightarrow$ update active ticket, reset timer to $0.5 L$ and attempts to `0`, remain in `S10_IN_GAME_TICKET_REFRESH`<br>• 3rd attempt fails $\rightarrow$ record support-ticket warning, stop refreshing, session continues until user exits |
| `SF_ABORTED` | User cancelled login or queue. | Zero in-memory credentials and tickets; close any open HTTP sockets. | Terminal state. |
| `SF_FAILED(class)` | Recoverable or fatal error encountered. | Zero in-memory password; display user-facing failure message for `class` (§5.3). | Recoverable classes (`F03`–`F08`) may return to `S4_AWAIT_CREDENTIALS_AND_WORLD`; `F02` returns to main menu. |

### 5.2 Timeouts and retry parameters <a id="s5-2-timeouts"></a>

| Operation | Timeout / Interval | Max Attempts / Retries | Provenance |
|---|---|---|---|
| HTTP/SOAP requests (`GetDatacenters`, `LauncherConfigurationServer`, `LoginAccount`, `TakeANumber`, `RefreshTicket`) | `30 s` per HTTP request | `1` attempt per user action (except `StatusServer.aspx` and `RefreshTicket` below) | `BIN` (`BIN-Q2`), `OL` |
| `StatusServer.aspx` host/path fallback (`S7`) | `30 s` per HTTP request | `2` attempts (`1` primary GET to `World/StatusServerUrl` + `1` fallback GET with host/path rewritten to `Launcher.DataCenterService.GLS` prefix on HTTP 404 or empty body) | `OL` |
| World login queue polling (`S8` `TakeANumber`) | Poll interval: `5 s` minimum recommended (`WorldQueue.PollTimer` server config value is `120 s`; polling faster than `2 s` risks `0x80004005`) | Indefinite while `HResult == 0` and `QueueNumber > NowServingNumber` until user cancels | `OL`, `PL`, `DDOL` |
| In-game `RefreshTicket` (`S10`) | Initial trigger at $0.5 \times L$; retry triggers add $0.5 \times (L - \text{elapsed})$ ($0.75 \times L$, $0.875 \times L$) | `3` attempts maximum per ticket (`attempt_count` `1`, `2`, `3`) | `BIN` (`BIN-Q2`) |

### 5.3 User-facing failure classes <a id="s5-3-failure-classes"></a>

| Code | Class Name | Trigger Condition | User-Facing Action / Message Summary |
|---|---|---|---|
| `F02_CONFIG_INVALID` | Local Install Configuration Invalid | `lotro.launcherconfig` missing/malformed, required keys missing, or `client_local_<Lang>.dat` missing. | Report which local file or key in `<InstallRoot>` is missing or unreadable. |
| `F03_SERVICE_UNAVAILABLE` | GLS / Config Service Unavailable | DNS failure, TLS verification failure, HTTP timeout, or non-200 response from `GetDatacenters`, `LauncherConfigurationServer`, or `AuthServer`. | Report that the login service could not be reached and allow retry. |
| `F04_AUTH_REJECTED` | Authentication Rejected | `LoginAccount` returns a SOAP `Fault` (e.g., `faultcode == "BIS"`). | Display authentication failure (invalid username or password) and prompt for credentials again. |
| `F05_NO_GAME_SUBSCRIPTION` | No Subscription for Game | `LoginAccountResult` contains zero `GameSubscription` entries with `Game == DataCenter.GameName`. | Inform the user that the authenticated account has no LOTRO subscription. |
| `F06_WORLD_CLOSED` | World Offline or Closed | `StatusServer.aspx` fails after fallback, returns an empty body, or `<loginservers>` has no non-empty entry. | Inform the user that the selected world is currently offline or unavailable. |
| `F07_WORLD_ROLE_DENIED` | World Billing Role Denied | Subscription `ProductTokens` fail the `allow_billing_role` / `deny_billing_role` check in §3.3. | Inform the user that the selected subscription tier (e.g., non-VIP) is not permitted on this world. |
| `F08_QUEUE_ERROR` | World Queue Error | `TakeANumber` returns `HResult != 0` (e.g., `0x80004005`) or malformed XML. | Inform the user that joining the world login queue failed and allow retry. |
| `F09_TICKET_INACCESSIBLE` | GLS Ticket Inaccessible | Client started without `--glsticketdirect`/`--authserverurl` and `--glsticket` registry read failed. | Abort session startup. |
| `F10_REFRESH_EXHAUSTED` | Ticket Refresh Exhausted | All 3 in-game `RefreshTicket` attempts failed. | Record warning that in-game support ticket submission will be unavailable; do not disconnect the active game session. |

---

## 6. Security and Access-Control Notes <a id="s6-security"></a>

### 6.1 Runtime endpoint and credential boundary <a id="s6-1-runtime-discovery"></a>

Per `CLAUDE.md` and `docs/CLEAN_ROOM.md`, all service URLs, hostnames, ports, and endpoints are discovered at runtime from the user's local installation (`lotro.launcherconfig`) or from server responses; no official endpoint, key, certificate, or account data is embedded in the repository, tests, or configuration defaults.

### 6.2 TLS and certificate validation <a id="s6-2-tls"></a>

1. **Observed behavior in official binary and legacy launchers (`BIN-Q2`, `PL`, `DDOL`)**:
   - In `x64/lotroclient64.exe`, the HTTP/SOAP client explicitly sets libcurl option `64` (`CURLOPT_SSL_VERIFYPEER`) to `0`, disabling TLS peer certificate verification for `RefreshTicket` and support SOAP requests.
   - Older third-party launchers (`PL`, `DDOL`) also disabled certificate verification or forced legacy TLS 1.0/SSLv23 ciphers to work around historical server configurations.
2. **Mandatory requirement for Tessera**:
   - Because `LoginAccount` transmits the user's plaintext password and returns the bearer `Ticket`, disabling TLS certificate or hostname verification exposes the user's credentials and session ticket to trivial man-in-the-middle interception.
   - Tessera **must** enforce standard TLS certificate chain validation (against the platform or Mozilla root store) and RFC 6125 hostname verification on all `https://` connections (`AuthServer`, `LoginQueue.aspx`, and any `https://` `DataCenterServer`, `LauncherConfigurationServer`, or `StatusServer.aspx` URLs).
   - Tessera **must not** disable certificate verification silently or by default.

> **OPEN (`OPEN-LOGIN-2`):** Because contacting live official servers from the dirty room is forbidden, verify during clean-room testing (with a user-opted-in test environment) whether current official GLS `https://` endpoints negotiate TLS 1.2+ with a certificate chain trusted by the standard Mozilla/OS root store, or whether a specific intermediate CA certificate must be bundled.

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

1. **No access-control circumvention**: The official GLS login flow (`GetDatacenters` $\rightarrow$ `LauncherConfigurationServer` $\rightarrow$ `StatusServer.aspx` $\rightarrow$ `LoginAccount` $\rightarrow$ `TakeANumber` $\rightarrow$ client hand-off $\rightarrow$ `RefreshTicket`) uses standard unauthenticated and password/ticket-authenticated HTTP/HTTPS XML and SOAP 1.1 calls. Static analysis of `x64/lotroclient64.exe` (`BIN-Q1`–`BIN-Q3`) and inspection of third-party launchers (`OL`, `PL`) confirm that **no client code-signing attestation, cryptographic challenge-response secret, anti-cheat module, or anti-tamper check** is present in the GLS login or ticket-refresh protocol.
2. **Binding stop condition**: If Standing Stone Games / Daybreak ever introduces client binary attestation, anti-cheat challenge tokens, or any other technical access control into the GLS authentication or world-queue endpoints, Tessera **must stop** and fail cleanly rather than attempting to emulate, spoof, or bypass the check.

---

## 7. Testable Invariants and Synthetic Test Vectors <a id="s7-test-vectors"></a>

All test vectors in this section use synthetic RFC 2606 `.example.invalid` endpoints, synthetic subscription identifiers, and synthetic base64 tickets containing zero real server addresses or secrets.

### 7.1 Testable invariants <a id="s7-1-invariants"></a>

- **`I1` (Local discovery failure network silence)**: When `S1_LOCAL_DISCOVERY` fails with `F02_CONFIG_INVALID` (missing or malformed `lotro.launcherconfig` or missing `client_local_<Lang>.dat`), zero DNS queries and zero TCP/UDP socket connections are initiated by the login module.
- **`I2` (Bootstrap config discovery order)**: Given a directory containing both `lotro.launcherconfig` and `TurbineLauncher.exe.config`, the parser loads `lotro.launcherconfig` (with or without a UTF-8 BOM `EF BB BF`).
- **`I3` (`LoginServerUrl` placeholder rejection)**: The literal value `(invalid service specified)` in `Datacenter/Worlds/World/LoginServerUrl` is never passed to `-h` or used as a socket target; the target `host:port` is always taken from the first non-empty `;`-delimited entry of `<Status>/<loginservers>`.
- **`I4` (`StatusServer.aspx` fallback rewrite)**: When `GET` to `World/StatusServerUrl` returns HTTP 404 or an empty body, the client retries exactly once against `<scheme>://<host_of_GLS_DataCenterService><base_path_of_GLS_DataCenterService>/StatusServer.aspx?<query_of_StatusServerUrl>`.
- **`I5` (Subscription `Name` vs `username`)**: When `LoginAccount` is called with `<username>alice_login</username>` and returns a matching `GameSubscription` with `<Game>LOTRO</Game>` and `<Name>SUB9876543210ABCDEF012345</Name>`, the value passed to `TakeANumber` (`subscription=`) and to the session layer (`-a`) is `SUB9876543210ABCDEF012345`, never `alice_login`.
- **`I6` (Billing role check)**: If `<Status>/<allow_billing_role>` is `RoleVIP,RoleFounder` and the selected subscription's `ProductTokens` set is `{"AngmarTrial", "MoriaExp"}`, the world status check rejects entry with `F07_WORLD_ROLE_DENIED` before calling `TakeANumber`.
- **`I7` (`TakeANumber` ticket percent-encoding)**: When constructing the `application/x-www-form-urlencoded` body for `TakeANumber`, `+`, `/`, and `=` characters in `Ticket` are percent-encoded as `%2B`, `%2F`, and `%3D`, while `--glsticketdirect` (and in-memory session hand-off) receives the raw unencoded `Ticket` string.
- **`I8` (Queue completion boundary)**: In `S8_WORLD_QUEUE`, `QueueNumber == NowServingNumber` and `QueueNumber < NowServingNumber` both terminate the queue loop immediately without sleeping for a poll interval.
- **`I9` (`RefreshTicket` backoff schedule)**: Given `--glsticketlifetime 21600` installed at $t_0 = 0.0\text{ s}$, if every `RefreshTicket` call fails, `RefreshTicket` is invoked at $t = 10800.0\text{ s}$, $t = 16200.0\text{ s}$, and $t = 18900.0\text{ s}$, and is never invoked a 4th time ($t > 18900.0\text{ s}$).
- **`I10` (`RefreshTicket` reset on success)**: If `RefreshTicket` succeeds at $t = 16200.0\text{ s}$ (attempt 2) with a new ticket, `start_time_sec` resets to $16200.0\text{ s}$, `next_refresh_offset_sec` resets to $10800.0\text{ s}$ (next fire at $t = 27000.0\text{ s}$), and `attempt_count` resets to `0`.
- **`I11` (Version string format and `VersionSend` `VLUInt` prefix)**: The combined client version string matches the exact 56-character ASCII pattern `netver:<netver>; didver:<XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX>` (where `<didver>` is the 36-character uppercase GUID from `.dat` superblock `0x17C..0x18B` without `" : <version_u32>"`) and is serialized at offset `+0x00` of `VersionSend` (`0x0000012C`, `docs/specs/official-session.md` §9.3) as a `VLUInt` (`PackedDWORD`) length prefix (`0x38` when length is `56`) followed by the raw ASCII bytes without a trailing NUL.
- **`I12` (Credential & ticket redaction)**: Formatting any error, debug, or info log event produced during `S4`–`S10` never outputs the substring of the user's password or the `Ticket` value.
- **`I13` (`<queueurls>` RFC 1918 private-IPv4 filtering with fallback)**: When `<Status>/<queueurls>` contains both an RFC 1918 private-IPv4 URL (`http://10.0.0.1:7080/command`) and a non-RFC-1918 URL (`http://queue-a.example.invalid:7080/command`), `queue_url` selects the non-RFC-1918 URL; when all non-empty entries use RFC 1918 private IPv4 addresses, `queue_url` falls back to the first non-empty entry (`LL`).
- **`I14` (`TakeANumber` HRESULT sign-bit evaluation and fallback)**: In `S8_WORLD_QUEUE`, a `<Result>` with `(HResult & 0x80000000) == 0` (any non-negative 32-bit `HRESULT`), or a `<Result>` omitting `<HResult>` while providing valid `<QueueNumber>` and `<NowServingNumber>`, is evaluated as a successful queue response (`LL`).
- **`I15` (`ArgTemplate` legacy alias substitution and unreplaced token filtering)**: Substituting `ArgTemplate` replaces `{LANGUAGE}`, `{PRODUCT}`, and `{AUTHSERVER}` alongside `{LANG}`, `{GAMETYPE}`, and `{AUTHSERVERURL}`, strips surrounding double quotes when splitting into `argv`, and drops any token still containing an unreplaced `{...}` placeholder (`LL`).
- **`I16` (Compression dictionary DID map resolution, deferred redirection, and `ConnectResponse` `codecUuid`)**: Resolving enumerator key `0x10000002` (`"Client2Server"`) via DID Map `3` (`0x28000010` in `client_general.dat`) loads record `0x0E010000`, validates schema GUID `E795878D-C633-471C-8315-C3DBBEAD2FAA` at `+0x00..+0x0F`, and registers the 16-byte `dict_guid` at `+0x10..+0x1F` for `codecUuid` negotiation in `ConnectRequest` (`0x00040000` at `+0x1C..+0x2B`) and `ConnectResponse` (`0x00080000` at `+0x0C..+0x1B`); resolving `0x10000001` (`"Server2ClientDeferred"`) loads stub `0x0E010002`, reads `deferred_target_did == 0x0E010001` at `+0x20..+0x23`, and redirects to load `0x0E010001` (`"Server2Client"`).

---

### 7.2 Synthetic test vectors <a id="s7-2-vectors"></a>

#### Vector `V1` — Local `lotro.launcherconfig` with UTF-8 BOM

Input bytes start with `EF BB BF` followed by:

| Element / Attribute | Synthetic Value |
|---|---|
| `/configuration/appSettings/add[@key="Launcher.DataCenterService.GLS"]/@value` | `https://gls.example.invalid/GLS.DataCenterServer/Service.asmx` |
| `/configuration/appSettings/add[@key="DataCenter.GameName"]/@value` | `LOTRO` |
| `/configuration/appSettings/add[@key="DataCenter.DefaultCenter"]/@value` | `LOTRO` |
| `/configuration/appSettings/add[@key="Product.DocumentFolder"]/@value` | `The Lord of the Rings Online` |
| `/configuration/appSettings/add[@key="Launcher.DocumentFolder"]/@value` | `Launcher` |

Expected parsed output:
- `gls_datacenter_service` = `"https://gls.example.invalid/GLS.DataCenterServer/Service.asmx"`
- `datacenter_game_name` = `"LOTRO"`
- `document_folder` = `"The Lord of the Rings Online"`

---

#### Vector `V2` — `GetDatacenters` SOAP request and response

- **Request**:
  - `POST https://gls.example.invalid/GLS.DataCenterServer/Service.asmx`
  - `Content-Type: text/xml; charset=utf-8`
  - `SOAPAction: "http://www.turbine.com/SE/GLS/GetDatacenters"`
  - Body elements: `/soap:Envelope/soap:Body/GetDatacenters[@xmlns="http://www.turbine.com/SE/GLS"]/game` = `LOTRO`
- **Synthetic Response Elements** (under `GetDatacentersResponse/GetDatacentersResult/Datacenter`):
  - `Name` = `LOTRO`
  - `AuthServer` = `https://gls-auth.example.invalid/GLS.AuthServer/Service.asmx`
  - `PatchServer` = `patch.example.invalid:6015`
  - `LauncherConfigurationServer` = `https://gls.example.invalid/Launcher/lotrolauncher.server.config.xml`
  - `Worlds/World[1]`: `Name` = `WorldAlpha [OLD]`, `LoginServerUrl` = `(invalid service specified)`, `ChatServerUrl` = `chat.example.invalid:2900`, `StatusServerUrl` = `http://legacy-status.example.invalid/GLS.DataCenterServer/StatusServer.aspx?s=shard-alpha`, `Order` = `2`
  - `Worlds/World[2]`: `Name` = `[EN] WorldBeta`, `LoginServerUrl` = `(invalid service specified)`, `ChatServerUrl` = `chat.example.invalid:2900`, `StatusServerUrl` = `https://gls.example.invalid/GLS.DataCenterServer/StatusServer.aspx?s=shard-beta`, `Order` = `1`
- **Expected parsed output**:
  - Sorted world display order places `[EN] WorldBeta` before `WorldAlpha [OLD]` (because `WorldAlpha [OLD]` ends case-insensitively with `[old]`).

---

#### Vector `V3` — `StatusServer.aspx` fallback URL rewrite and `<Status>` parsing

- **Input**:
  - `Launcher.DataCenterService.GLS` = `https://gls.example.invalid/GLS.DataCenterServer/Service.asmx`
  - `World[1].StatusServerUrl` = `http://legacy-status.example.invalid/GLS.DataCenterServer/StatusServer.aspx?s=shard-alpha`
  - Primary `GET` to `http://legacy-status.example.invalid/GLS.DataCenterServer/StatusServer.aspx?s=shard-alpha` returns `HTTP 404 Not Found`.
- **Expected fallback URL**:
  - `https://gls.example.invalid/StatusServer.aspx?s=shard-alpha`
- **Synthetic `<Status>` XML fields**:
  - `<loginservers>login-a.example.invalid:9001;login-b.example.invalid:9002;</loginservers>`
  - `<queueurls>http://queue-a.example.invalid:7080/command;http://queue-b.example.invalid:7080/command;</queueurls>`
  - `<nowservingqueuenumber>0x00000010</nowservingqueuenumber>`
  - `<lastassignedqueuenumber>0x00000015</lastassignedqueuenumber>`
  - `<allow_billing_role>RoleTierVIP</allow_billing_role>`
  - `<deny_billing_role></deny_billing_role>`
- **Expected parsed output**:
  - `login_server` = `"login-a.example.invalid:9001"`
  - `queue_url` = `"http://queue-a.example.invalid:7080/command"`
  - `now_serving` = `16` (`0x10`), `last_assigned` = `21` (`0x15`)
  - `allow_billing_role` = `{"RoleTierVIP"}`

---

#### Vector `V4` — `LoginAccount` multi-subscription filtering and SOAP fault

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

#### Vector `V5` — `TakeANumber` POST body encoding and `<Result>` evaluation

- **Inputs**:
  - `WorldQueue.LoginQueue.URL` = `https://gls-auth.example.invalid/GLS.AuthServer/LoginQueue.aspx`
  - `WorldQueue.TakeANumber.Parameters` = `command=TakeANumber&subscription={0}&ticket={1}&ticket_type=GLS&queue_url={2}`
  - `{0}` (`GameSubscription/Name`) = `LOTROSUB00000000000000002`
  - `{1}` (`Ticket`) = `SYNTH+TICKET/ABC123==`
  - `{2}` (`queue_url`) = `http://queue-a.example.invalid:7080/command`
- **Expected `application/x-www-form-urlencoded` POST body**:
  `command=TakeANumber&subscription=LOTROSUB00000000000000002&ticket=SYNTH%2BTICKET%2FABC123%3D%3D&ticket_type=GLS&queue_url=http%3A%2F%2Fqueue-a.example.invalid%3A7080%2Fcommand`
- **Response Poll 1**:
  - `<Result><Command>TakeANumber</Command><HResult>0x00000000</HResult><QueueNumber>0x00000014</QueueNumber><NowServingNumber>0x00000011</NowServingNumber><ContextNumber>0x00000099</ContextNumber></Result>`
  - **Expected action**: `HResult == 0`, `QueueNumber` (`20`) > `NowServingNumber` (`17`); display `3` players ahead and remain in `S8_WORLD_QUEUE`.
- **Response Poll 2**:
  - `<Result><Command>TakeANumber</Command><HResult>0x00000000</HResult><QueueNumber>0x00000014</QueueNumber><NowServingNumber>0x00000014</NowServingNumber><ContextNumber>0x00000099</ContextNumber></Result>`
  - **Expected action**: `QueueNumber` (`20`) <= `NowServingNumber` (`20`); transition immediately to `S9_SESSION_HANDOFF`.

---

#### Vector `V6` — `RefreshTicket` SOAP exchange and backoff arithmetic

- **Initial state**: $t_0 = 1000.0\text{ s}$, $L = 21600\text{ s}$, `ticket` = `"SYNTH+TICKET/ABC123=="`.
  - `start_time_sec` = $1000.0$, `next_refresh_offset_sec` = $10800.0$, `attempt_count` = `0`.
- **Tick at $t = 11800.0\text{ s}$** ($\Delta t = 10800.0 \le 10800.0$): no action.
- **Tick at $t = 11800.1\text{ s}$** ($\Delta t = 10800.1 > 10800.0$):
  - `next_refresh_offset_sec` $\leftarrow 10800.0 + 0.5 \times (21600.0 - 10800.1) = 16199.95\text{ s}$.
  - `attempt_count` $\leftarrow 1$.
  - Sends SOAP 1.1 `POST` to `--authserverurl` with `SOAPAction: "http://www.turbine.com/SE/GLS/RefreshTicket"` and `/soap:Envelope/soap:Body/RefreshTicket[@xmlns="http://www.turbine.com/SE/GLS"]/ticket` = `"SYNTH+TICKET/ABC123=="`.
- **Response at $t = 11800.5\text{ s}$**:
  - `/soap:Envelope/soap:Body/RefreshTicketResponse[@xmlns="http://www.turbine.com/SE/GLS"]/RefreshTicketResult` = `"SYNTH+TICKET/NEW456=="`.
  - **Expected state update**: Active ticket updated to `"SYNTH+TICKET/NEW456=="`; `start_time_sec` $\leftarrow 11800.5$; `next_refresh_offset_sec` $\leftarrow 10800.0$; `attempt_count` $\leftarrow 0$; session listeners notified via `(ticket="SYNTH+TICKET/NEW456==", account="LOTROSUB00000000000000002")`.

---

#### Vector `V7` — `netver`/`didver` serialization in `VersionSend` (`0x0000012C`) and `0x10000002`/`0x10000001` dictionary resolution

- **Synthetic inputs**:
  - Extracted `<netver_string>` from `.rdata`: `"1234"` (`4` ASCII bytes `31 32 33 34`).
  - Synthetic `.dat` superblock descriptor at file offset `0x17C..0x18F` (`20` bytes):
    `04 03 02 01 06 05 08 07 09 0A 0B 0C 0D 0E 0F 10 4C 04 00 00`
    (`Data1 = 0x01020304`, `Data2 = 0x0506`, `Data3 = 0x0708`, `Data4 = 09 0A 0B 0C 0D 0E 0F 10`, `ver_u32 = 1100`).
  - Synthetic `0x0E010000` (`"Client2Server"`, mapped by `0x10000002` in `0x28000010`) header (`44` bytes):
    - `+0x00..+0x0F` (schema GUID): `8D 87 95 E7 33 C6 1C 47 83 15 C3 DB BE AD 2F AA`
    - `+0x10..+0x1F` (`dict_guid` / `codecUuid`): `AA BB CC DD 11 22 33 44 55 66 77 88 99 00 AB CD`
    - `+0x20..+0x23` (`deferred_target_did`): `00 00 00 00`
    - `+0x24..+0x27` (`dict_format_version`): `01 00 00 00`
    - `+0x28..+0x2B` (`tree_stream_len`): `09 00 00 00`
  - Synthetic `0x0E010002` (`"Server2ClientDeferred"`, mapped by `0x10000001` in `0x28000010`) stub (`44` bytes):
    - `+0x00..+0x0F` (schema GUID): `8D 87 95 E7 33 C6 1C 47 83 15 C3 DB BE AD 2F AA`
    - `+0x10..+0x1F` (`dict_guid` / `codecUuid`): `10 20 30 40 50 60 70 80 90 A0 B0 C0 D0 E0 F0 01`
    - `+0x20..+0x23` (`deferred_target_did`): `01 00 01 0E` (`0x0E010001` LE)
    - `+0x24..+0x27` (`dict_format_version`): `00 00 00 00`
    - `+0x28..+0x2B` (`tree_stream_len`): `00 00 00 00`
- **Expected derived values and wire bytes**:
  - `<didver_string>` = `"01020304-0506-0708-090A-0B0C0D0E0F10"` (`36` uppercase ASCII chars; `ver_u32 = 1100` is discarded).
  - Combined `versionString` = `"netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10"` (`56` ASCII bytes).
  - `VersionSend` (`0x0000012C`, `docs/specs/official-session.md` §9.3) inner payload bytes `+0x00..+0x38` (`57` bytes):
    `38` (`VLUInt` / `PackedDWORD` length `56`) followed by the `56` ASCII bytes of `"netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10"` (no trailing NUL).
  - Resolving `0x10000001` redirects from stub `0x0E010002` to `0x0E010001`.
  - `ConnectResponse` (`0x00080000`, `docs/specs/official-session.md` §6.4) payload bytes `+0x0C..+0x1B` (`codecUuid`, `16` bytes, when echoing the `0x0E010000` dictionary):
    `AA BB CC DD 11 22 33 44 55 66 77 88 99 00 AB CD`.

---

## 8. Implementer Notes <a id="s8-implementer-notes"></a>

1. **XML namespace and case-sensitivity handling**:
   - Official GLS responses use default namespace `xmlns="http://www.turbine.com/SE/GLS"` on `<GetDatacentersResponse>`, `<LoginAccountResponse>`, and `<RefreshTicketResponse>`, while `<Status>` (`StatusServer.aspx`) and `<Result>` (`LoginQueue.aspx`) have **no XML namespace**. Clean-room XML parsers should match SOAP payload elements in the `http://www.turbine.com/SE/GLS` namespace (or match local element names after verifying the SOAP body), match SOAP `<Fault>` case-insensitively (`Fault` / `fault`, extracting either `<faultstring>` or `<Message>`), and match child element names under `<Status>` case-insensitively (`LL`).
2. **Splitting `;`-delimited lists and RFC 1918 filtering in `<Status>`**:
   - Both `<loginservers>` and `<queueurls>` in `StatusServer.aspx` typically have a trailing semicolon (e.g., `host:port;`). Splitting on `;` produces a trailing empty string `""`; implementations must filter out empty segments when extracting `login_server` and `queue_url`. When `<queueurls>` contains multiple non-empty entries, prefer the first non-RFC-1918 URL (`!contains("://10.")`, `!contains("://192.168.")`, `!contains("://172.16.".."://172.31.")`) and fall back to the first non-empty entry if all entries are RFC 1918 private IPs (`LL`).
3. **Hexadecimal integer parsing and HRESULT sign bit in `<Status>` and `<Result>`**:
   - `nowservingqueuenumber`, `lastassignedqueuenumber`, `HResult`, `QueueNumber`, `NowServingNumber`, and `ContextNumber` are formatted with a leading `0x` or `0X` prefix. Strip the `0x`/`0X` prefix and parse as base-16 `u32`. For `<Result>/<HResult>`, test bit 31 (`(hresult & 0x80000000) != 0`) for failure rather than `hresult != 0`, and treat a missing `<HResult>` as `0` when both `<QueueNumber>` and `<NowServingNumber>` are present (`LL`).
4. **In-memory hand-off vs CLI process spawning**:
   - Section 3.6 and Section 4.1 document the command-line flags (`-a`, `-h`, `--glsticketdirect`, `--authserverurl`, `--glsticketlifetime`, `--glsticket`) for completeness and interoperability. Inside Tessera, the launcher service client and the UDP session client reside in the same workspace and should pass the session hand-off structure directly in memory (§4.2) rather than round-tripping through process CLI arguments or the Windows registry.

---

## 9. Summary of Open Questions <a id="s9-open-questions"></a>

| ID | Section | Question | How to Resolve |
|---|---|---|---|
| `OPEN-LOGIN-2` | §6.2 | Whether current official GLS `https://` endpoints negotiate TLS 1.2+ with a certificate chain valid under the standard Mozilla/OS root store, or require a specific intermediate CA certificate. | Verify in clean-room integration testing with user-supplied credentials. |

---

## 10. Change Log <a id="s10-changelog"></a>

| Date | Author Role | Summary |
|---|---|---|
| `2026-10-20` | `Spec writer (dirty room)` | Initial draft of `docs/specs/official-login.md` covering runtime config discovery, GLS/world-queue exchanges, client ticket intake and `RefreshTicket` lifecycle, state machine, security notes, invariants, and synthetic test vectors (`OBS`, `OL`, `PL`, `GLS`, `REDDO`, `DDOL`, `BIN`). |
| `2026-10-20` | `Clean-room reviewer` | Independent review against `CLEAN_ROOM.md` §5.3: fixed SPDX header, set target crate to `crates/compat`, and marked status `Accepted`. |
| `2026-10-20` | `Spec writer (dirty room)` | Cross-referenced against project owner's `lotro-launcher` (`LL`): added 64-bit `UserPreferences64.ini`/`UserPreferences_x64.ini` lookup order and `LastWorld` (§2.3), optional `Datacenter/SupportURL` and `Order` sorting (§3.1), case-insensitive `<Status>` child matching and `<queueurls>` RFC 1918 private-IP filtering with fallback (§3.3, `I13`), SOAP `<Fault>`/`<Message>` classification for maintenance and locked accounts (§3.4), mandatory `TakeANumber` execution when `queue_url` is non-empty and `HResult` bit-31 sign evaluation with `<QueueNumber>`/`<NowServingNumber>` fallback (§3.5, `I14`), and legacy `ArgTemplate` aliases (`{LANGUAGE}`, `{PRODUCT}`, `{AUTHSERVER}`, `{HIGHRES}`, `{USERDIR}`) plus unreplaced `{...}` token filtering (§3.6, `I15`). |
| `2026-10-20` | `Spec writer (dirty room)` | Resolved `OPEN-LOGIN-1` (`BIN-Q3`, `BIN-Q4`, `OBS`): documented runtime extraction of `<netver_string>` from `x64/lotroclient64.exe` `.rdata` and `<didver_string>` from `.dat` superblock `0x17C..0x18B` (36-char uppercase GUID without `" : <version_u32>"`), `VLUInt`-prefixed wire encoding of `"netver:<netver>; didver:<didver>"` in `VersionSend` (`0x0000012C`), resolution of enumerator keys `0x10000002` and `0x10000001` via DID Map `3` (`0x28000010` in `client_general.dat`) to `0x0E010000` and `0x0E010002` $\rightarrow$ `0x0E010001`, byte layout of `0x0E01xxxx` compression dictionary records, and `codecUuid` negotiation in `ConnectRequest` (`0x00040000`) and `ConnectResponse` (`0x00080000`) (§1.2, §1.3, §2.4, §4.2, `I11`, `I16`, `V7`, §9). |

