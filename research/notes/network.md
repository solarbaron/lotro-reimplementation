<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# LOTRO networking and server side: research notes (verified 2026-10-03)

All repos listed below were cloned into `~/.cache/tessera/clones/` and I read their source. Read-only SOAP and HTTP calls were made to the live public GLS endpoints to confirm the schemas. No credentials were used.

---
## 0. Summary of findings

1. **The launcher/GLS layer is fully documented, and it is plain public SOAP and XML over HTTP(S).** I confirmed it live today. OneLauncher (GPL-3) is the reference implementation.
2. **The game protocol is the Turbine/Asheron's Call network layer, changed in a few places.** The LOTRO transport uses the same header flag values as AC. It also uses the same checksum seed (`0xBADD70DD` plus `len<<16`, the AC `Hash32`), the same Login, ConnectRequest and ConnectResponse handshake (cookie, net-id and two checksum seeds), and the same optional-header sizes. The main differences are big-endian header fields in a different order, an RSA-1024 + RC4 encrypted login block, and static Huffman-style payload compression. Because of this, ACE, ACProtocol, aclogview, actestclient, gromnie and OpenAC are the best available models for a LOTRO netcode layer.
3. **There is no open-source, working LOTRO server emulator.** The furthest public attempt is tAmMo's 2013 C# "LOTRO-Tools" server (gitlab.com/lotro/server, mirrored by Middle-earth-Revenge). It gets a real client as far as the **character creation screen**. The real checksum code exists only as a closed `Helper.dll`; the public source holds a mock. The community also reports a lost ~2010 prototype. **Echoes of Angmar** is a running closed-source private server (2007 client, Vol I Book 11). It describes itself as "based on original code from the time", so it is presumed to use leaked server code. It received a cease-and-desist from Middle-earth Enterprises. It is legally tainted, so do not touch it.
4. **No DDO server emulator exists either.** Xaymar/RE-DDO (2024) has launcher and patch captures and only rough game-protocol notes.

---
## 1. Launcher / GLS (Global Login System): exact endpoints and formats

### 1.1 Local bootstrap config (in the game install directory)
- Lookup order: `<game>.launcherconfig` (e.g. `lotro.launcherconfig`), then any `*.launcherconfig`, then legacy `TurbineLauncher.exe.config`, then `DNDLauncher.exe.config` for DDO.
- Format: .NET appSettings XML: `<configuration><appSettings><add key=".." value=".."/>`.
- Required keys: `Launcher.DataCenterService.GLS` (e.g. `http://gls.lotro.com/GLS.DataCenterServer/Service.asmx`), `DataCenter.GameName` (`LOTRO`, `DDO`; historically `LOTROEU` for Codemasters EU), and `Product.DocumentFolder` (the Documents subfolder name).

### 1.2 GLS.DataCenterServer (SOAP 1.1, namespace `http://www.turbine.com/SE/GLS`)
- URL: `https://gls.lotro.com/GLS.DataCenterServer/Service.asmx`. Append `?WSDL` for the WSDL. DDO uses `gls.ddo.com`.
- Operations: `GetDatacenters(game)`, `GetDatacenterStatus(game)`, `ServerInfo(s)`, `DumpCache()`.
- SOAPAction: `"http://www.turbine.com/SE/GLS/GetDatacenters"`.
- Request body: `<GetDatacenters xmlns="http://www.turbine.com/SE/GLS"><game>LOTRO</game></GetDatacenters>`.
- Response schema: `ArrayOfDatacenter/Datacenter{Name, Worlds/World{Name, LoginServerUrl, ChatServerUrl, StatusServerUrl, Order:int, Language}, AuthServer, PatchServer, LauncherConfigurationServer}`.
- Live response today (LOTRO):
  - AuthServer=`https://gls.lotro.com/GLS.AuthServer/Service.asmx`
  - PatchServer=`patch.lotro.com:6015` (DDO: `patch.ddo.com:5015`)
  - LauncherConfigurationServer=`http://gls.lotro.com/launcher/lotro/lotrolauncher.server.config.xml`
  - Worlds: Grond [EU], Sting, Peregrin [RP], Meriadoc [EU-RP], Glamdring, Orcrist [EU], Angmar, Mordor[EU]. `LoginServerUrl` is always the literal `(invalid service specified)`; the real login server comes from the status XML. ChatServerUrl looks like `64.37.178.18:2900`. StatusServerUrl looks like `http://gls.lotro.com/GLS.DataCenterServer/StatusServer.aspx?s=10.193.144.40`, where the parameter is an internal IP.
- Domains: `gls.lotro.com`, `gls-auth.lotro.com`, and `gls-bullroarer.lotro.com` (preview/Bullroarer). `moria.gls.lotro.com` has a TLS certificate mismatch, so rewrite it to gls.lotro.com. Legendary "Mordor" uses the path prefix `GLS.STG.DataCenterServer`. Some legendary status URLs carry a broken IP host; OneLauncher rewrites the host and path to the GLS service host plus `/StatusServer.aspx`.

### 1.3 GLS.AuthServer
- Operations: `LoginAccount(username, password, additionalInfo)` (OneLauncher passes `""`) and `RefreshTicket(ticket)`, which returns a string.
- SOAPAction: `http://www.turbine.com/SE/GLS/LoginAccount`.
- Response: `UserProfile{Subscriptions/GameSubscription[]{Game, Name, Description, ProductTokens/string[], CustomerServiceTokens/string[], ExpirationDate, Status, NextBillingDate, PendingCancelDate, AutoRenew, BillingSystemTime, AdditionalInfo}, Ticket}`.
- Filter subscriptions by `Game == DataCenter.GameName`. One account can hold several subscriptions; the user picks one. **`GameSubscription.Name` is what gets passed as `-a`/`{SUBSCRIPTION}`, not the login username.** DDO accounts also return `Web_DDO` subscriptions.
- Error faults (faultcode `BIS`): "No Subscriber Formal Entity was found." (bad username or password, detail `<result>0x8004C002</result>`), "User name is too short", "Password is too short".
- Ticket: an opaque base64 string. A sample in lotro-gls is 256 characters, which is 192 bytes. The 2009 blog says "256 byte ticket". In the client's first UDP packet the ticket is length-prefixed (0x102 = 258 in a 2013 capture). Default lifetime is `21600` s (the server config sets `GameClient.Arg.glsticketlifetime`).
- TLS: one 2024 report needed `DEFAULT@SECLEVEL=1` against gls-auth.ddo.com. Plain curl to gls.lotro.com worked today.

### 1.4 Server launcher config (`lotrolauncher.server.config.xml`, appSettings XML; DDO uses `dndlauncher.server.config.xml`)
Keys confirmed live:
- `GameClient.WIN64.Filename=lotroclient64.exe`, `GameClient.WIN32.Filename=lotroclient.exe`, `GameClient.WIN32Legacy.Filename=lotroclient_awesomium.exe`. Very old configs used `GameClient.Filename` and `GameClient.ArgTemplate` with positional `{0}..{4}`.
- `GameClient.WIN32.ArgTemplate`: `-a {SUBSCRIPTION} -h {LOGIN} --glsticketdirect {GLS} --chatserver {CHAT} --rodat on --language {LANG} --gametype LOTRO --authserverurl {AUTHSERVERURL} --glsticketlifetime {GLSTICKETLIFETIME}`
- `GameClient.Arg.crashreceiver=http://crash.lotro.com:8080/CrashReceiver-1.0`, `GameClient.Arg.authserverurl=https://gls.lotro.com/gls.authserver/service.asmx`, `GameClient.Arg.glsticketlifetime=21600`, `GameClient.Arg.DefaultUploadThrottleMbps=1`. Optional: `GameClient.Arg.bugurl`, `.supporturl`, `.supportserviceurl`, `GameClient.HighResPatchArg` (default ` --HighResOutOfDate`), `GameClient.ResetGraphicsArg=" --safe"`.
- `Patching.ProductCode=LOTRO`, `PatchWindow`, `PatchConnectRetryInterval`.
- `WorldQueue.LoginQueue.URL=https://gls.lotro.com/GLS.AuthServer/LoginQueue.aspx`
- `WorldQueue.TakeANumber.Parameters=command=TakeANumber&subscription={0}&ticket={1}&ticket_type=GLS&queue_url={2}`
- `WorldQueue.LeaveQueue.Parameters=command=LeaveQueue&subscription={0}&context={1}&ticket_type=GLS&queue_url={2}`, plus `WorldQueue.PollTimer=120` and thresholds.
- `URL.NewsFeed=https://www.lotro.com/{lang}/launcher-feed.xml`, `URL.DownloadFilesList=http://akamai.lotro.com/lotro/patch/splashscreen/DownloadFilesList.xml`, `URL.AkamaiDownloadURL=http://installer.lotro.com/lotro/`, `Game.Version=3601.0066.7272.4024`, `ShardTransfer.ServiceURL=http://xfer.lotro.com/TurbineTransferService/TurbineTransferService.svc`, plus account and e-commerce URL templates.

### 1.5 World status (GET StatusServerUrl → XML `<Status>`)
- Fields: name, logintiers, world_full, loginservers, logintierlastnumbers, logintiermultipliers, queuenames, queueurls, nowservingqueuenumber (hex), lastassignedqueuenumber, allow_billing_role, deny_billing_role, allow_admin_role, deny_admin_role, farmid, wait_hint, plus optional we_server_legendary_active, we_perma_death, world_requiresubscription, world_ghostworldmode, we_pvp_war_event_control, world_pvppermission.
- Live example (Sting): `loginservers=64.37.178.18:9004;64.37.178.18:9003;` `queueurls=http://10.193.144.40:7082/LoginQueue;http://10.193.144.40:7081/LoginQueue;`
- Admin roles: Server, Observer, CustomerService, and others. Billing roles: TurbineEmployee, AngmarStd, MoriaSTD, MirkStd, and others.
- Use the first login server as `{LOGIN}` (`-h`). A 404 or empty body means the world is down.

### 1.6 Login queue
- POST form-encoded to LoginQueue.aspx. The ticket and queue_url must be URL-encoded (`quote_plus`).
- Response: `<Result><Command>TakeANumber</Command><HResult>0x00000000</HResult><QueueName>GUID</QueueName><QueueNumber>0x..</QueueNumber><NowServingNumber>0x..</NowServingNumber><LoginTier>1</LoginTier><ContextNumber>0x..</ContextNumber></Result>`.
- Poll until `QueueNumber <= NowServingNumber`. `HResult 0x80004005` is a generic failure (preview closed, servers down, or rate limit).

### 1.7 Client hand-off
`lotroclient64.exe -a <SubscriptionName> -h <ip:port> --glsticketdirect <ticket> --chatserver <ip:2900> --rodat on --language English --gametype LOTRO --authserverurl ... --glsticketlifetime 21600 [--HighResOutOfDate] --prefs <UserPreferences.ini path>`

- `--prefs` also sets the settings directory. `--cwd` is broken, so chdir instead.
- Full option list (running the client with no args prints it): `-a/--account`, `--authserverurl`, `--chatserver`, `--connspeed`, `-r/--create`, `--cwd`, `--debug`, `-f/--franchise`, `--gametype`, `--glsticket` (registry), `-z/--glsticketdirect`, `--glsticketlifetime`, `--HighResOutOfDate`, `-h/--host`, `--keymap`, `--language`, `--loadfile`, `-m/--mps` (monster play session), `--outport`, `-p/--port`, `--prefs`, `--remotemouse`, `--resource-set`, `--rodat`, `--safe`, `-s/--specify race,class`, `--surveyurl`, `--usechat`, `--useexiturl`, `-u/--user`, `--voicenetdelay`, `--voiceoff`, `--wfilelog/--wprintlog` (64-bit weenie event masks). 2009-era extras: `--dbwlib`, `--nodbwlib`, `--usetdm`, `--webjournalurl`, `--bugurl`, `--supporturl`, `--supportserviceurl`.
- **For our client:** implement the same CLI surface so OneLauncher and other launchers can start it unchanged.

### 1.8 Patching
- The official patcher is the 32-bit `patchclient.dll` (with `TTEPatchClient.dll` beside it). It exports a rundll32-style `Patch(hwnd, hinst, char* args)`.
- Invocation: `rundll32 patchclient.dll,Patch <patchserver> --language English [--highres] --productcode LOTRO --filesonly|--dataonly`. OneLauncher runs filesonly, filesonly, dataonly (it runs twice because the DLL patches itself), then parses stdout ("checking files", "files to patch: N", "data patches: N", "applying N forward iterations", "downloading").
- Wire protocol: HTTP/1.1 `POST /stateless2` to `patch.<game>.com:<port>` with `Content-Type: application/x-www-form-urlencoded`. The server replies `Content-Type: binary` (`Server: Microsoft-HTTPAPI/2.0`). The body is encrypted binary; the ASCII marker `3des` is visible in the request. The 2009 version used `/stateless` on port 5015. **No public RE of the patch protocol exists.**
- Initial data download: `{AkamaiDownloadURL}/{Game.Version}/{lang}_{highres|lowres}_download_list.xml` → `<FileList><File><From/><To/><Size/><MD5/></File>`. Splashscreens: `<FileList><File><Description/><FileName/><DownloadUrl/></File>`.
- A reimplementation should not ship a patcher. It should rely on the user's official install, which is the OpenMW model.

---
## 2. Game protocol (UDP): what is publicly known

### 2.1 Topology and ports
- Game uses UDP 9000-9010. Status XML login servers are on :9003/:9004; the 2013 server emulator config used 9000 with world server 9001.
- Chat server uses UDP 2900 (2900-2910; the official firewall article says TCP/UDP 9000-9100 and 2900-2910).
- Flow: the client logs into the login server, which validates the GLS ticket and sends the account and character list. On world entry the server sends the IP and port of the world server, and the client opens a new connection there (from the LOTRO-Investigation.pdf flow description). This matches AC's login-server-to-world-server referral.
- The chat connection also sends netver/didver, the subscription name in UTF-16 and the ticket. Later traffic mixes plaintext and binary, with UTF-16 names visible (RE-DDO notes and the 2009 blog).

### 2.2 Packet header (big-endian; 20 bytes, or 22 during setup with a `00 00` prefix)
From LOTRO-Investigation.pdf (tAmMo 2013), bwgypyth's blog and lotro-server `PayloadHeader.cs`:
```
off size field
0x00 2  SessionID (client or server id; 0 during handshake)
0x02 2  Body/data length
0x04 4  Flags ("action"/opcode in community docs)
0x08 4  Sequence
0x0C 4  Checksum
0x10 2  "packet group"/time (e.g. 0xFFEB)
0x12 2  iteration/"increased by server"
```
AC's header (ACE `PacketHeader.cs`, little-endian) holds the same fields in a different order: `Sequence u32, Flags u32, Checksum u32, Id u16, Time u16, Size u16, Iteration u16`.

### 2.3 Flags map 1:1 to AC `PacketHeaderFlags`
Hex values are as written in LOTRO captures and code:

| Flag | AC name | LOTRO meaning |
|---|---|---|
| `0x00010000` | LoginRequest | first client packet |
| `0x00040000` | ConnectRequest | first server packet |
| `0x00080000` | ConnectResponse | second client packet |
| `0x00004000` | AckSequence | lotro-server "ConfirmSequence" |
| `0x00100000` | NetError | "OldClientVersion" |
| `0x00200002` | NetErrorDisconnect + EncryptedChecksum | "WrongGLSTicket" |
| `0x00008000` | Disconnect | "TerminateServer" |
| `0x00000006` | EncryptedChecksum + BlobFragments | normal data ("MultiDataObject") |
| `0x01000006` | adds TimeSync | |
| `0x0B000002` | Flow + EchoRequest + TimeSync + EncChecksum | keepalive, **18 body bytes** |
| `0x0B004002` | adds AckSequence | **22 bytes** |

The 18-byte size equals AC's TimeSync(8) + EchoRequest(4) + Flow(6), and the 22-byte variant adds AckSeq(4). This is an exact size match. PacketMaster (2019) also saw the header size vary with the flags' low nibble: 0x14, 0x1A, 0x20, 0x21, 0x26. That is consistent with AC's optional header fields.

### 2.4 Checksum
- lotro-server `generateChecksumForHeader`: `0xBADD70DD + (sessionID<<16 | dataLength) + 0x00140000 + flags + seq + ack + dataChecksum`.
- This equals AC `Hash32` over the 20-byte header with the checksum field set to `0xBADD70DD`, where the `len<<16` seed is 20<<16 = 0x00140000, plus the payload checksum. The payload checksum is a sum of per-fragment Hash32 values in both games (lotro-server `DataObject.Serialize` sums header and data checksums).
- **The real LOTRO payload checksum code was only ever in the closed `Helper.dll`.** The public source has a mock (commit 9319eab, 2020).
- The server's first packet carries a "server checksum table base" and a "client checksum table base" (two 32-bit seeds). The header-serialize code XORs the checksum with a per-packet value. In AC these are the ISAAC seeds for the "EncryptedChecksum" xor stream (ACE `CryptoSystem : ISAAC`; verify: `(hdr.Checksum - headerHash) ^ payloadHash == isaac.Next()`). **Hypothesis, unverified for LOTRO:** the LOTRO "checksum tables" are the same ISAAC-style keystream.

### 2.5 Handshake bodies

**Client LoginRequest (0x00010000), unencrypted header.** Body from the 2013 PDF and 2016 blog, mirroring ACE `PacketInboundLoginRequest`:
- u8-len ASCII version string, e.g. `061004_netver:7537; didver:926CD8E3-2984-4CA9-9C6B-6DF2C8EB6BC3`. The 2013 server required `netver:7779`. The prefix is the protocol version date; didver is the DAT/data GUID.
- u32 remaining length
- u32 auth type (AC: GlsTicket = 0x40000002)
- u32 flags (`04 00 00 00` when a GLS ticket is present)
- u32 unix timestamp
- length-prefixed UTF-16 account (subscription) name
- u32 ticket length + ticket
- In newer clients (around 2012): a 128-byte RSA-1024-encrypted block carrying a 128-bit session key (CryptoAPI SIMPLEBLOB; reverse the bytes before RSA-decrypting), and the account+ticket block is RC4-encrypted with that session key.
- The server public key is embedded in the client (a CryptoAPI PUBLICKEYBLOB `06 02 00 00 00 A4 00 00 'RSA1' 00 04 00 00 01 00 01 00 ...`). Turbine's private key is unknown. The 2013 tools replaced the client's public key with their own (they patched the client). The "private key" blob the client imports via CryptImportKey (2010 wine trace) is the classic **exponent-of-one** PRIVATEKEYBLOB trick for importing a raw RC4 key, not Turbine's key.

**Server ConnectRequest (0x00040000), 44-byte body.** Matches AC `CConnectHeader`:
- 0x14 8 bytes: server time / "ping-pong base" (AC: double ServerTime)
- 0x1C 8 bytes: cookie / "session key", which the client echoes byte-reversed (AC: u64 Cookie)
- 0x24 u32: client session/net id, e.g. `00 00 0A 50` (AC: NetID)
- 0x28 u32: server checksum seed (AC: OutgoingSeed)
- 0x2C u32: client checksum seed (AC: IncomingSeed)
- 0x30 16 bytes: a constant that is "the same for every account and server" (`DA 82 CF 61 37 12 45 85 9C 4E 1C 6D 01 F1 62 BB`; AC has 4 bytes of padding here)

**Client ConnectResponse (0x00080000):** the 8-byte cookie, reversed.

After that, packets are compressed. The server then sends the client IP, server name, account name, character list and so on as "data objects" with sub-headers such as `06 81 49`, `06 80 AA`, `06 43`, `06 80 DD`, `06 80 F5`, `06 6D` (lotro-server Protocol/Header). Most of these are not understood.

### 2.6 Payload "encryption" is actually static Huffman-style compression
lotro-server `Decrypt.cs` and `Encrypt.cs` show:
- The first 2 bytes (session id) are kept.
- If byte[0..1]==0 the packet is a cleartext handshake.
- If byte[2]==0 the packet is uncompressed (the server sometimes skips compression).
- Otherwise the rest is read as 32-bit LE words, bit-serial LSB-first, skipping 4 bits of the first block. Each bit walks a binary tree "jump table" (pairs of int32 child indices). A negative index plus an offset (client 16372, server 16125) selects a leaf in a "lookup table", and each leaf is a **multi-byte string** (length-prefixed entry + code length + code bits + end marker).
- There are separate trees for client→server and server→client.
- The tables "don't exist in the .exe while not running", so they were dumped from client memory. They are probably built at runtime from data.
- The bwgypyth blog confirms: the first 3 packets are uncompressed, and the rest are compressed and checksummed. (A 2011 blog guess that it was RC4-only was wrong.)
- Legal note: those table files in the repo are extracted from the client. Do not redistribute them. Extract them at runtime from the user's install, or reconstruct them from the DAT data if they are loaded from there (open question).

### 2.7 Message layer
- PacketMaster (2019, hooks the live client) identified `PrivateMessagePacket` id `0xC7100000`. A position update has a modifier byte and then a float Vec3 (x, y, z).
- Property IDs: a "MasterProperty" DAT file (DID `0x34000000`) maps 32-bit property ids to names (master-property-viewer, 2025). LOTRO objects use property bags over the wire, so this table is how you name network fields.
- AC's message layer (fragments with header `seq u32, id u32, count u16, size u16, index u16, queue u16`; game messages and events with opcodes) is the likely model. Expect LOTRO's to differ more than the transport does.

### 2.8 Open questions
- Exact checksum and keystream algorithm (ISAAC?).
- Where the Huffman tables come from.
- The current 64-bit client protocol after 2013 (netver has surely changed, and the 2024 servers moved to 64-bit).
- The chat protocol.
- The patch protocol (3DES).
- RefreshTicket use in-game (`--authserverurl`).
- How the message-layer opcodes and property encoding work.

---
## 3. Implications for the clean-room client

1. **The launcher layer is safe to implement from public facts.** These are public web-service schemas; implement them from the WSDL and the notes above, not by copying GPL code. The client should accept the official CLI args. The netcode should be designed so the transport (AC-style reliable UDP: seq, ack, retransmit, flow, fragments) is generic and testable against our own test server.
2. **Keep a strict boundary for anything taken from the user's client binary** (RSA public key, Huffman tables, version strings): load it at runtime from the user's install and never ship it.
3. **Connecting a third-party client to SSG's official servers likely violates the ToS** and could carry anti-circumvention (DMCA §1201 / EU) risk because of the RSA/RC4 login. Frame live connection as user-at-own-risk and make "own test server" the main development target. OpenMW-style projects avoid this because they are single-player.
4. **Never use Echoes of Angmar material or any "leaked" server code.** Treat lotro-server (no license file, closed Helper.dll, extracted tables) as documentation only.
5. **Good architectural templates:** ACE (server, AGPL), ACProtocol/protocol.xml (codegen), aclogview (pcap message viewer), actestclient/gromnie (headless clients), and OpenAC (a full from-scratch AC client in .NET 10 with Vulkan, user-supplied data and a plugin API). OpenAC is the closest existing analogue to this project.
6. **Tooling to build:** a pcap ingester and dissector (Wireshark Lua or Rust) for the LOTRO header, flags and decompression; a fake GLS/launcher server (lotro-gls shows the minimal ASP.NET version; any SOAP stack can do it); and a packet-corpus database (as lotro-analyzer and VortexPacketAnalyzer did).
