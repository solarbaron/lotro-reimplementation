
# Toolchain for a legal, clean-room LOTRO client reimplementation on Gentoo

Everything below was checked in this session (2026-10-03) unless it is marked *(from memory)* or *(unverified)*. Facts about the target come from public repos I cloned and from looking at the header and string level of the user's own install (no decompiling):
- `~/nvme/SteamLibrary/steamapps/common/lotro-working-new`
- `.../Lord of the Rings Online/x64`

---

## 0. Target facts that drive tool choice (observed)

**Executables**
- `x64/lotroclient64.exe`: PE32+, 33.7 MB, MSVC linker 14.25 (VS2019 16.5). Sections: `.text .rdata .data .pdata .rsrc .reloc`. It is not packed and contains about 615 `.?AV` MSVC RTTI type-descriptor strings, so RTTI-based class recovery will work.
- A **32-bit `lotroclient.exe` (PE32 i386, 26.4 MB, built 2026-02-09, the same day as the x64 build) still ships in the install root.**
  - This matters because CERT's OOAnalyzer only handles 32-bit MSVC binaries.
  - Plan: run OOAnalyzer on the 32-bit build, then port names and types to x64 with BSim or Version Tracking.
  - Context: public reporting says the 32-bit *worlds* closed in Aug 2025.
- The client links Havok statically (strings: "Havok REPORT", "HAVOK ASSERT"). An old unpacker changelog says animations are Havok `.hks` files.

**Renderer plug-ins**
- Files: `D3D10GraphicsCore.dll`, `D3D11GraphicsCore.dll`, `OpenGLGraphicsCore.dll`, and (in the 32-bit root) `NullGraphicsCore.dll`.
- Each core DLL exports only `GraphicsCoreDLLInterface_ExchangeData` (plus `_vsprintf`/`_vsprintf_s`). So the engine↔renderer boundary is one function-table exchange.
- The exe holds the preference key string `Render.GraphicsCore`.
- Strings mention "DirectX 9.0 detected". The freecam toolkit calls D3D9 a "legacy backend" and says the active backend is DX11 in `D3D11GraphicsCore.dll`.

**Middleware DLLs in x64/**
- Media and scripting: `binkw64.dll` (Bink video), `lua51.dll` (123 exports; Lua 5.1 for the plugin API), `OpenAL32.dll` + `soft_oal.dll` (OpenAL Soft), `ogg/vorbis/vorbisfile`.
- Engine middleware: `umbra.dll` (Umbra occlusion, 158 C++-mangled exports such as `Umbra::Commander`, `Umbra::Tile`; the root also has the older `dpvs.dll`), `xerces-c_2_6.dll`.
- Platform and web: `libcef.dll` (Chromium Embedded, in-game store/browser), `libcurl.dll` (7.81.0-DEV built with **schannel** TLS), `vulkan-1.dll`, `vk_swiftshader.dll`, `steam_api64.dll`, `zlib1.dll`.
- The 32-bit root also has `PatchClient.dll`, `TTEPatchClient.dll`, `zlib1T.dll`, the Qt4 launcher DLLs, `awesomium`, `ijl15`, `libeay32`/`ssleay32`, `msvcr71`.

**Data files and DAT header**
- Archives: `client_anim.dat`, `client_cell_{1..5,14,15}.dat`, `client_map_{1..5,14,15}.dat`, `client_gamelogic.dat`, `client_general.dat`, `client_local_English.dat`, `client_mesh.dat`, `client_sound.dat`, `client_surface.dat`, `client_highres.dat`.
- Overflow archives: `*_aux_N.datx`. Each archive is capped at 0xFF900000 bytes (4,287,627,264), then spills into the next aux file. The full DAT set is about 41 GB.
- Header checked on `client_general.dat`:
  - `u32@0x101 = 0x4C50` ("PL")
  - `u32@0x140 = 0x5442` ("BT")
  - `block_size@0x144 = 0x400`
  - `file_size@0x148 = 0x2C500000` (matches the real size)
  - `version@0x14C = 0x01030000`
  - `root_dir_offset@0x160`

**B-tree directory node** (from `Mirarkitty/lotro_extractor/datfile.py`, Apache-2.0)
- 8 zero bytes.
- Up to 62 child pointers as `(u32 block_size, u32 offset)`.
- At `+8*63`, a `u32 count`.
- `count` entries of 0x20 bytes each: `[flags:u16|policy:u16][file_id][file_off][size1][timestamp][version][size2][unk]`. `flags & 1` means compressed.

**File body**
- Layout: `[numExtraBlocks:u32][legacy:u32=0]`, then the first chunk (`block_size - 8 - 8*numExtra` bytes), then `numExtra` pairs of `(size, offset)` pointing at continuation blocks.
- Compressed files: the payload starts with `u32 uncompressed_size`, followed by a zlib stream.
- Decoded content starts with its own DID as a u32 LE.
- Asset records in mesh/surface/aux archives: `[nextptr:4][pad:4][usize:4][zlib…]`.
- DID high byte is the type: 0x25 = text/strings, 0x19/0x28/0x40/0x41/0x42/0x78 appear in the old DATUnpacker `FORMATS/*.txt`.

**Text subfile format (0x25)** (from the koniecdev `.claude/agents/dat-format-expert.md`)
- `FileId u32 | unk u32 | unk u8 | FragmentCount VarLen`.
- Each fragment: `u64 id`, `u32 pieceCount`, then UTF-16LE pieces whose length is a VarLen char count. VarLen is 1 byte for 0–127; for 128–32767 it is 2 bytes with the 0x80 flag.
- Then `u32 argRefCount` with 4 bytes each, then `u8 argStringGroupCount`.

**Versioning**
- `OpenDatFileEx2` reports `vnumDatFile/vnumGameData = 112/3`. This stayed frozen across updates 45.x → 49.x, so it is a schema version, not a content version.
- Per-subfile `version` and `iteration` act as content fingerprints.
- The live content version (e.g. "49.4") appears only in SSG announcements. The Steam News API (`ISteamNews/GetNewsForApp/v2/?appid=212500`) is a machine-readable source.

**Patching**
- The launcher calls SOAP `GetDatacenters` on `http://gls.lotro.com/GLS.DataCenterServer/Service.asmx`. The reply includes `PatchServer=patch.lotro.com:6015` and `LauncherConfigurationServer=…/launcher/lotro/lotrolauncher.server.config.xml`.
- `patchclient.dll` (32-bit) speaks a binary protocol: `POST /stateless2` to `patch.lotro.com:6015`.
- Patches are applied in place, chunk by chunk ("GUID version stamp", "Extent stamp mismatch").

**Client command line** (from OneLauncher `start_game.py`, captured by running the client with no args)
- Login and connection: `-a` account, `-h host`, `-p port`, `--glsticketdirect`, `--authserverurl`, `--chatserver`, `--language`, `--gametype`.
- Diagnostics: `--safe`, `--prefs`, `--debug <32-bit field>`, **`--wfilelog`/`--wprintlog <64-bit mask or enum names>`** (Asheron's Call-era "weenie event" logging). These logging switches are a free dynamic-analysis aid.
- GLS hosts: `gls.lotro.com`, `gls-auth.lotro.com`, `gls-bullroarer.lotro.com`. `moria.gls.lotro.com` has a TLS certificate that does not match the hostname.

**UDP game protocol** (old public notes)
- The 2013 Middle-earth Revenge (MER) "LOTRO-Investigation.pdf" (Riders of Rohan era) and its `liblotro` code describe:
  - A 20-byte "in-session" pseudo-header; 22 bytes during session setup, when it starts `00 00`.
  - Session-setup packets are unencrypted.
  - Later packets are processed in 4-byte blocks using jump and lookup tables. The tables exist only in memory at runtime and were dumped from a running client.
  - The server sends a temporary session key plus two 32-bit checksum seeds.
  - Port 2900 is chat.
- This is useful history, but expect the modern 64-bit client to differ.

**Memory layout**
- Entity, reference and client-data tables are reachable from `.data` globals.
- LOTRO Companion's closed `delta-lotro-memory` jar hard-codes RVAs per build. `lotro_extractor` finds them at runtime instead, and its `docs/discovery.md` warns at length about false positives from "signature anchors".
- Strings are null-terminated UTF-16.
- Freecam RVAs (build `4905.0070.9961.4022`, image size 0x22bd000): `FlightCameraController` vtable at 0x15701e0. They show the binary is stable enough for masked-signature transfer between builds.

**Install and disk constraints on this machine**
- Installed: Ghidra 12.1.3 (pentoo overlay; `/usr/share/ghidra`, release name "DEV", so user settings live in `~/.config/ghidra/ghidra_12.1.3_DEV`), wine-staging 11.19 (eselect), gdb 17.2, wireshark 4.6.7 **built without USE=lua**, openjdk 21/25, dotnet SDK 9/10, rust 1.97.1, clang 22, uv/uvx, maven, gradle, docker, lutris, umu-launcher, steam.
- Disk: `/` is 100% full (2.8 GB free) and `/home` is 95% full (21 GB free). `~/nvme` (XFS, `reflink=1`) has 243 GB free, so put Ghidra projects, the BSim database, captures and DAT snapshots there. `/home` is also `reflink=1`.
- Machine: 24 cores, 31 GB RAM.

---

## 1. Clean-room process and repo layout (with Claude Code enforcement)

The ReactOS IP policy is a good template. The person who disassembles writes an interface or behaviour document and may not implement; a different person or group implements from that document only. All contributors sign the policy.

**Proposed repos**
1. **`lotro-re-lab`** (private, dirty)
   - Contents: Ghidra projects and Ghidra Server, BSim database, capture files (pcapng, RenderDoc `.rdc`, Frida logs), scratch scripts, raw notes with addresses and pseudo-code.
   - Never pushed publicly.
   - MCP servers (Ghidra, Frida, x64dbg, RenderDoc) are configured *only* here.
2. **`lotro-specs`** (reviewed, publishable)
   - mdBook of behavioural and format specs, plus machine-readable schemas: Kaitai `.ksy`, ImHex `.hexpat`, and an XML/TOML protocol and DBObj schema in the style of AC's `dats.xml`/`protocol.xml`.
   - No decompiled code, no copied tables, no addresses, no RTTI-derived source text beyond names needed for interoperability.
   - Every spec page goes through a "spec auditor" review before it is merged.
3. **`lotro-client`** (public, clean)
   - The Rust implementation. It may read only `lotro-specs`.
   - Claude Code enforcement in this repo's `.claude/settings.json`:
     - `permissions.deny`: `Read(/home/solarbaron/…/lotro-re-lab/**)`, plus `Bash(ghidra*)`, `Bash(analyzeHeadless*)`, `Bash(frida*)`.
     - No Ghidra or Frida MCP in its `.mcp.json`.
   - Use separate Claude sessions or subagents per role: an "RE analyst" agent in the lab and an "implementer" agent in the client repo. A shared Claude session would leak dirty context into clean code.
   - Keep a dated `docs/knowledge-base/` log of settled empirical findings. koniecdev/LotroKoniecDev does this well, with a `dat-format-expert` subagent and `/spec` and `/adr` commands.

**Do not use `datexport.dll`, even as an oracle.** It is Turbine's proprietary DAT library that has circulated in fan tools since about 2011, and it is *not* part of the retail install. koniecdev's THIRD-PARTY-NOTICES admits it has no licence to it. A clean-room native reader already exists publicly (lotro_extractor's `datfile.py`, Apache-2.0), and LotroCompanion's Java `DATArchive` is the reference it matched.

Legal notes *(from memory; confirm with counsel)*:
- US: Sega v. Accolade and Sony v. Connectix (intermediate copying for interoperability is fair use); DMCA §1201(f) interoperability exception.
- EU: Software Directive 2009/24/EC Art. 5(3) and Art. 6 (observe/study/test; decompile for interoperability).
- The SSG EULA/ToS almost certainly bans RE and third-party tools, and the freecam README says memory editing breaks the ToS. Breaking a contract is a separate risk from copyright.
- Do dynamic analysis offline where possible. Use a throwaway account, never touch live servers with modified clients, and ship no assets.

---

## 2. Static analysis

**Ghidra 12.1.3** (installed, pentoo `dev-util/ghidra-12.1.3-r1`; the ebuild needs gradle-bin below 9 selected)

Modules present:
- `BSim`, `VersionTracking`, `VersionTrackingBSim`
- `PyGhidra` (`support/pyghidraRun`; Jython is now an optional extension)
- `MicrosoftCodeAnalyzer`/`MicrosoftDemangler`, `PDB`, `FunctionID`
- FID databases `vs2012/2015/2017/2019_{x86,x64}.fidbf` (vs2019 matches linker 14.25)
- `GhidraServer`, `CodeCompare`, `ProgramDiff`, `Sarif`, `SystemEmulation`
- Debugger agents: gdb, lldb, dbgeng, drgn, **x64dbg**, plus Trace RMI

Workflow:
1. Import `lotroclient64.exe` and the graphics-core DLLs into a shared project. Run auto-analysis with the "Windows x86 PE RTTI Analyzer" and FID.
2. Run `Ghidra/Features/Decompiler/ghidra_scripts/RecoverClassesFromRTTIScript.java` (Windows class recovery is the better-tested path). It recovers hierarchy, constructors/destructors, vftable structs and class data types. Follow-up scripts: `ApplyClassFunctionSignatureUpdatesScript` and `ApplyClassFunctionDefinitionsUpdatesScript`.
3. Optionally run **OOAnalyzer** (Pharos, CMU SEI; x86-32 MSVC only) on the 32-bit `lotroclient.exe`.
   - The OOAnalyzer Ghidra importer now ships in **Kaiju** (CERTCC/kaiju). Release `260910` (2026-09-10) has a `ghidra_12.1.3_PUBLIC_…_kaiju.zip`.
   - Pharos itself builds in a Docker image (`seipharos/pharos`) *(from memory)*.
   - Kaiju also provides function hashing (FnHash) and GhiHorn.
4. Port names and types from the 32-bit to the x64 build, and from patch N to patch N+1:
   - **BSim** compares decompiler-p-code features, which can work across architectures. Databases: H2 file, PostgreSQL, or Elasticsearch.
   - **Version Tracking** sessions, now with BSim correlators.
5. `Ghidra-Cpp-Class-Analyzer` (astrelsky) was archived in October 2023, so use it only for older Ghidra.
6. **Ghidra Server** for the team:
   - Location: `/usr/share/ghidra/server` (`svrInstall`, `ghidraSvr`, `svrAdmin`, `server.conf`; default ports 13100–13102 *(from memory)*).
   - Run it as a dedicated user with repos on `~/nvme`.
   - Note from bethington/ghidra-mcp: 12.1.3 clients need a 12.1 or 12.0.5+ server.
7. Headless batch runs: `support/analyzeHeadless` or PyGhidra. Use them for nightly re-analysis of each new client build: auto-analysis, RTTI script, BSim signature generation, and a symbol export to SARIF.
8. Patch diffing:
   - **ghidriff** (clearbluejar, PyPI) writes Markdown diffs.
   - **BinExport + BinDiff 8**: BinDiff is open-source (google/bindiff). The last binary release was v8 (2023, `.deb`). The last BinExport binary release is `v12-20240417-ghidra_11.0.3`, so you have to build the BinExport Ghidra extension from source with gradle against 12.1.3.
   - Built-in ProgramDiff and Version Tracking.
9. Other useful items:
   - `nccgroup/ghostrings` (Go/Rust-style strings; minor here)
   - `0xdea/ghidra-scripts`
   - `HackOvert/GhidraSnippets`
   - `capa`/FLOSS (Mandiant), Detect-It-Easy
   - LIEF (pentoo `dev-util/lief`) for scripted PE parsing
   - rizin/cutter (`::gentoo`)
   - IDA Pro or Binary Ninja are optional and paid; both have MCP servers.

---

## 3. Dynamic analysis under Wine

- **Ghidra Debugger with the "gdb + wine" launcher** (`Debugger-agent-gdb/data/debugger-launchers/wine-gdb.sh`):
  - It runs wine under gdb, uses `ghidra wine run-to-image` (catches `mprotect` until the PE is mapped), synthesizes the PE module map from `/proc/pid/maps`, and syncs with Trace RMI.
  - Set `OPT_WINE_PATH`. The default `/usr/lib/wine/wine64` does not exist on Gentoo; use `/usr/lib/wine-staging-11.19/bin/wine` (or `wine64`). Also set `OPT_ARCH=i386:x86-64`.
  - It also ships `wine32_info_proc_mappings.gdb`.
- `winedbg --gdb` / `winedbg --gdb --no-start <pid>` as a gdb remote stub. Plain gdb 17 is installed.
- **Wine's own tracing** acts as a free API monitor:
  - Channels: `WINEDEBUG=+relay,+seh,+loaddll,+tid,+timestamp` (filter with the `HKCU\Software\Wine\Debug` `RelayInclude`/`RelayExclude` keys), `+winsock` and `+secur32` for network, `+d3d11,+dxgi` with wined3d.
  - Add the client's own `--wfilelog`/`--wprintlog` and `--debug` switches.
- **Frida**:
  - Gentoo: pentoo `dev-util/frida-tools` 14.10.4, `dev-libs/frida-core` 17.17.0, `dev-python/frida`.
  - Option A, proven by the Freat project (MIT): run the **Windows** `frida-server.exe --listen 127.0.0.1:PORT` inside the same `WINEPREFIX` (`wine frida-server.exe …`), then connect from Linux with `frida -H 127.0.0.1:PORT`. This is how Freat's `WineTargetProvider` and `ProtonTargetProvider` work; the Proton variant reads `WINEPREFIX`/`WINELOADER` from wineserver's environment.
  - Option B *(unverified)*: attach Linux frida to the wine-preloader process and work with raw addresses.
  - NeuralFoundry's freecam (about 5k lines of Frida JS) shows Frida hooking `lotroclient64.exe` on Windows, with per-build RVA tables and prologue-byte validation.
  - **High-value hook target:** `GraphicsCoreDLLInterface_ExchangeData`. Interposing on the exchanged function table logs every engine→renderer call. That gives a spec for the renderer interface without reading renderer code.
- **Memory reading without injection:** under Wine the client is an ordinary Linux process, so `/proc/PID/mem` plus `/proc/PID/maps` does what `ReadProcessMemory`/`EnumProcessModules` do. `lotro_extractor` wraps this in a root-owned sudo helper and does ground-truth-driven structure discovery.
- Memory scanners (Cheat Engine–style):
  - `dev-util/scanmem` (also GameConqueror) in `::gentoo` and the user's overlay.
  - PINCE (korcankaraokcu/PINCE; GDB-based, works on Wine processes).
  - Cheat Engine itself under Wine. Existing `.CT` tables: mklinik/lotro-fov.
  - `edb-debugger` (pentoo).
- **x64dbg under Wine is unreliable.** WineHQ bugs report crashes on attach, `NtCreateDebugObject` stubs, and failure to open files. Use a **Windows VM** (QEMU/KVM) for x64dbg, ScyllaHide, API Monitor (rohitab), Process Monitor, and native-D3D11 RenderDoc. Ghidra 12's `Debugger-agent-x64dbg` can drive x64dbg locally or over SSH (`ssh-x64dbg.ps1`).
- `rr` (`dev-debug/rr`) is in `::gentoo`, but record/replay of Wine and DXVK workloads is *(unverified)* and likely too heavy.

---

## 4. Graphics capture

- **DXVK + RenderDoc (Vulkan) is the main path.**
  - Gentoo: `media-gfx/renderdoc-1.36-r1` (amd64), `app-emulation/dxvk` 2.7.1–3.1.1 (or the DXVK bundled with Proton/umu).
  - Launch: `ENABLE_VULKAN_RENDERDOC_CAPTURE=1` with the Vulkan implicit layer registered, or `renderdoccmd capture -w wine lotroclient64.exe …`, or qrenderdoc with "capture child processes" on.
  - Verified in DXVK source (`dxvk_instance.cpp`): `ENABLE_VULKAN_RENDERDOC_CAPTURE=1` (or `MESA_VK_TRACE`) turns on DXVK's Capture debug flag automatically when `DXVK_DEBUG` is unset.
  - `DXVK_DEBUG=markers` forwards the app's debug names; `DXVK_LOG_LEVEL=debug`, `DXVK_LOG_PATH`, `DXVK_HUD=full`.
  - **`DXVK_SHADER_DUMP_PATH=/dir`** dumps the original D3D shader source (`dumpSource`) and the SPIR-V (`dumpSpv`) in `dxvk_shader_ir.cpp`; it also disables the shader cache. Use it to get the DXBC from the game, then disassemble with `d3dcompiler_47` in Wine or dxbc tools.
  - Downside: RenderDoc then shows Vulkan/SPIR-V, not D3D11 state.
- **OpenGL backend option (worth testing).** The client ships `OpenGLGraphicsCore.dll` and has a `Render.GraphicsCore` preference. If it can be selected, Wine's `opengl32` forwards to the host GL, so **Linux-native RenderDoc (GL) and Linux apitrace (`dev-debug/apitrace-13.0`, `~amd64`)** capture directly.
- **apitrace for D3D11/D3D9:** use the *Windows* apitrace build. Under Wine, prefer the wrapper-DLL method (put apitrace's `d3d11.dll`/`dxgi.dll` wrappers next to the exe with `native,builtin` overrides) over `apitrace trace -a dxgi`, whose injection may fail under Wine *(unverified)*. Replay with `d3dretrace.exe` under Wine+DXVK. The openblack project (Black & White reimplementation) wiki has a similar frame-capture guide.
- `NullGraphicsCore.dll` (32-bit) suggests a headless renderer exists, which is useful for automated logic-only runs.
- Windows VM with native D3D11 + RenderDoc: keeps DXBC reflection and D3D11 pipeline state. PIX does not support D3D11 *(from memory)*.
- Vulkan tooling: `dev-util/vulkan-tools`, `dev-util/spirv-tools`, Khronos validation layers (`VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation`).
- RenderDoc MCP: `JiaboLi-GitHub/renderdoc-mcp` exposes 59 tools over the replay API. It is Windows-oriented; a Linux build needs the RenderDoc source and cmake.

---

## 5. Network

- **Wireshark:** `net-analyzer/wireshark` 4.6.x. The installed 4.6.7 lacks `USE=lua`; **rebuild with `USE=lua`** (`lua_single_target_lua5-4`) to load Lua dissectors. The user is already in the `pcap` group.
  - Capture filter from MER: `udp and not port 53 and not port 2900 and not broadcast and not multicast`.
- Dissector templates from the Asheron's Call side (same Turbine heritage):
  - `AverageJamz/ac-dissector` and `ccressent/wireshark-asheronscall` (Lua).
  - `ACClientLib/ACProtocol/protocol.xml` (8.5k lines; enums such as `PacketHeaderFlags` with Retransmission 0x1, EncryptedChecksum 0x2, BlobFragments 0x4 … Flow 0x08000000, plus message definitions used for codegen).
  - `ACEmulator/aclogview` (C# pcap/log viewer).
- Process:
  1. Prototype in Lua.
  2. Keep the protocol definition as XML/TOML in `lotro-specs`.
  3. Generate a Rust parser, a Lua dissector, and docs from it.
  4. Optionally port to Rust with **wsdf** (ghpr-asia/wsdf, a proc-macro Wireshark dissector framework, MIT/Apache, tested against Wireshark 4.0, so it may need ABI updates for 4.6).
- **TLS without MITM:**
  - The client's `libcurl` uses schannel. Under Wine, schannel is `secur32.so` → dlopened GnuTLS, and the system libgnutls contains the `SSLKEYLOGFILE` keylog code.
  - So `SSLKEYLOGFILE=/path/keys.log wine lotroclient64.exe` should produce Wireshark-usable keys. The pieces are verified individually; the end-to-end run is *(unverified)*.
- **mitmproxy** (`net-proxy/mitmproxy-12.2.3`, amd64; `mitmproxy-linux` for local transparent mode) for launcher SOAP and HTTP:
  - Endpoints: `GLS.DataCenterServer/Service.asmx` (`GetDatacenters`), `GLS.AuthServer/Service.asmx` (`LoginAccount`), world login queue (`WorldQueue.LoginQueue.URL`, `TakeANumber` params), launcher config XML.
  - OneLauncher (GPL-3, Python, httpx + SOAP) and Xaymar/RE-DDO (`protocol/launcher.md`, 499 lines of full request/response captures for the DDO launcher) document this already. Clean side: reimplement the launcher from those public protocol notes.
- Old MER code (`Middle-earth-Revenge/lotro-server`, C#, 2013): SharpPcap capture with auto-decrypt, PacketBrowser (WPF), liblotro (`BEBinaryReader`, jump/lookup-table "decrypt", `Checksum` stubbed).
  - **Legal caution:** its `data/table_*` files are tables dumped from the client, and it ships `Helper.dll`. Treat it as dirty material.
- MCP for packets: `0xKoda/WireMCP` (tshark-based, Node). Plain `tshark -T json -X lua_script:…` called through Bash is usually enough.

---

## 6. Binary-format work

- **ImHex** (`guru` `app-editors/imhex-1.38.1`, `~amd64`, bundles the ImHex-Patterns tag) for interactive work with `.hexpat` patterns.
  - **`plcli`** from WerWolv/PatternLanguage (LGPL-2.1) runs a pattern headlessly: `plcli format -i file -p pat.hexpat -f json`. Use it in CI for differential testing: the spec pattern's JSON vs. the Rust parser's JSON on the same sample.
  - Third-party MCP: `jmpnop/imhexmcp` (patched ImHex plus a Python MCP).
- **Kaitai Struct 0.11** (2025-09-07) adds a **Rust** target. Packages: `kaitai_struct_rust_runtime`, `dev-python/kaitaistruct` in `::gentoo`; the compiler `kaitai-struct-compiler` is a Scala/JVM release jar, not in Gentoo, so use the upstream `.deb`/zip or the Web IDE.
  - Write specs as `.ksy` for documentation and visualisation. Production parsers stay hand-written Rust, or are generated from the project schema.
- 010 Editor: commercial, has a native Linux build, templates are `.bt`. Optional.
- **Schema-as-data codegen pattern:** Chorizite/DatReaderWriter (AC, MIT, `net8.0`/`netstandard2.0`) keeps `dats.xml` (4.3k lines of enums and types with DID ranges, e.g. `Setup` 0x02000000–0x0200FFFF) and a Roslyn SourceGenerator that emits readers and writers, with BTree insert/remove and async I/O.
  - Copy this pattern: `lotro-specs/schema/*.xml|toml` → Rust `build.rs` / proc-macro → readers, writers and docs.
  - Related AC references: ACE (server emulator), ACViewer.
- `jtauber/lotro` (2014 Python) and `Mogwai-TheFurry/DatExplorer` (C#, DDO) are historical readers.
- `Endevir/Elanor` and `LOTRO-Enchanced-text-patcher` (RU localisation) and koniecdev (PL localisation) all depend on `datexport.dll`. Do not use them as code sources.
- Diff tools: `dev-util/diffoscope`, `difftastic` for JSON/structural dumps, `sqlitebrowser`.

---

## 7. Diffing data across game patches

- Before each launcher patch, snapshot the DAT set with `cp --reflink=always` on `~/nvme` (XFS `reflink=1`), so 41 GB of DATs cost almost nothing until they diverge. Btrfs or ZFS snapshots are alternatives.
- Build a `datmanifest` tool. For each archive: walk the B-tree and record `(did, version, iteration, size, compressed flag, timestamp, sha256-of-decoded)` in SQLite or Parquet. Label each snapshot with the Steam News "Update X.Y" title and the exe build string (e.g. `4905.0070.9961.4022`).
- Manifest diff → changed DIDs by type → typed structural diff of decoded objects (JSON dumps through difftastic).
- Exe diffing per build: ghidriff, BSim and Version Tracking. The freecam author's "masked-signature transfer" (wildcard rel32 and RIP displacements, then confirm instruction by instruction) is a good lightweight method for carrying addresses across builds in the lab.

---

## 8. Fuzzing, testing, CI

- **Fuzzing:**
  - `cargo-fuzz` (bentoo overlay `dev-util/cargo-fuzz-0.13.2`, or `cargo install cargo-fuzz`; needs nightly for `-Zsanitizer`).
  - `afl.rs` / `cargo-afl` with `app-forensics/aflplusplus` 4.35c in `::gentoo`; `honggfuzz`.
  - `arbitrary` for structure-aware inputs.
  - Targets: B-tree directory walk, block-chain assembly (cycles, out-of-range offsets, `numExtra` overflow), zlib wrapper, VarLen, text fragments, each DBObj decoder, packet framing and reassembly.
  - Seed corpora are built locally from the user's DATs and **never committed**. Commit only minimised synthetic crashers produced by our own writer.
- **Golden-file tests against user-supplied DATs:**
  - Gate on an env var such as `LOTRO_DATA_DIR`; skip when it is unset. DatReaderWriter does the same with `EORCommonData.DatDirectory`, and koniecdev uses `SkippableFact`.
  - Snapshot (`insta`) **only derived, non-expressive metadata**: counts, DID lists, hashes, structural summaries. Never decoded text, textures or meshes, so no copyrighted content enters the repo.
  - Round-trip invariant: parse → serialize gives identical bytes (koniecdev's main regression shape).
  - Runners: `cargo-nextest` (`::gentoo`); coverage with `cargo-llvm-cov` (bentoo) or `grcov`.
- **CI (GitHub Actions, no game data):**
  - `cargo build/clippy/test` against synthetic fixtures made by our own writer.
  - `cargo fuzz run … -- -max_total_time=60` smoke runs.
  - `plcli`/kaitai differential checks on synthetic samples.
  - `mdbook build` plus link checking; `cargo-deny` licence and advisory checks; `reuse lint` (`dev-util/reuse`).
  - A grep-based "dirty-content guard" that rejects hex addresses or RVAs, `FUN_`/`DAT_` Ghidra names, and binary blobs larger than N KB in `lotro-client`/`lotro-specs`.
  - Optional self-hosted runner on the owner's machine with `LOTRO_DATA_DIR` for real-data golden tests. Results go out as pass/fail only.
- **Docs:** `app-text/mdbook` 0.5.4 (amd64). Add mdbook-mermaid and mdbook-linkcheck via `cargo install` (not in Gentoo). Kaitai `ksv`/Web IDE renders for format pages.

---

## 9. Claude Code aids (MCP servers and agents)

**Ghidra**
- **bethington/ghidra-mcp** (Apache-2.0, active 2026-09, built against **Ghidra 12.1.3**, Java 21/Maven or Gradle, Python 3.10+, uv).
  - Hundreds of tools with lazy loading; stdio or streamable HTTP; multi-program routing; read-only allowlist mode.
  - Optional debugger bridge through `GHIDRA_DEBUGGER_URL` (that server moved to a game-specific repo; the project grew out of Diablo 2 RE).
  - Install: `python -m tools.setup preflight|ensure-prereqs|build|deploy --ghidra-path /usr/share/ghidra`. Because `/usr/share` is root-owned, install the extension into the user profile.
- **clearbluejar/pyghidra-mcp** (headless-first, PyPI).
  - Start: `uvx pyghidra-mcp --transport streamable-http --project-path ~/nvme/re/ghidra-projects /path/bin`.
  - Register: `claude mcp add --transport http pyghidra-mcp http://127.0.0.1:8000/mcp`.
  - A `.mcp.json` entry needs `"type":"http"`.
- LaurieWired/GhidraMCP: the original; last commit 2025-06, so probably stale for 12.x.
- Also: themixednuts/GhidraMCP (embedded, about 30 tools), jtang613/GhidrAssistMCP, cyberkaida/reverse-engineering-assistant (ReVa).

**Other MCP servers**
- Paid disassemblers: mrexodia/ida-pro-mcp, fosdickio/binary_ninja_mcp.
- Debugger: x64dbg MCP for the Windows VM (Wasdubya/x64dbgMCP, bromoket/x64dbg_mcp, SetsunaYukiOvO/x64dbg-mcp in Zig, AgentSmithers/x64DbgMCPServer in C#).
- Runtime: dnakov/frida-mcp, nonsleepr/frida-mcp.ts.
- Graphics and packets: JiaboLi-GitHub/renderdoc-mcp, 0xKoda/WireMCP, jmpnop/imhexmcp.

**Agent setup**
- Lab repo: an `re-analyst` subagent with Ghidra and Frida MCP access whose output is spec drafts.
- Spec repo: a `spec-auditor` subagent that checks drafts for expressive copying.
- Client repo: an `implementer` subagent with no RE tools.
- Slash commands: `/spec`, `/adr`, `/knowledge`.
- Hooks: a PreToolUse hook in the client repo that blocks reads outside the allowed paths.

---

## 10. Gentoo install cheat sheet

| Tool | Source | Notes |
|---|---|---|
| Ghidra 12.1.3 | pentoo `dev-util/ghidra` | installed; needs `eselect gradle` < 9 to build |
| Kaiju / OOAnalyzer importer | CERTCC/kaiju release zip | install via File → Install Extensions; Pharos via Docker |
| BinExport | build from source with `GHIDRA_INSTALL_DIR=/usr/share/ghidra` | |
| BinDiff 8 | `.deb` → unpack to `/opt` | |
| ghidriff, pyghidra-mcp | `uv tool install …` | |
| renderdoc | `media-gfx/renderdoc` | |
| dxvk | `app-emulation/dxvk` | or via umu/Proton |
| apitrace | `dev-debug/apitrace` | `~amd64`; plus the Windows release zip for D3D |
| wireshark | `net-analyzer/wireshark` | **add `USE=lua`** |
| mitmproxy | `net-proxy/mitmproxy` | also `net-proxy/mitmproxy-linux` |
| imhex | guru `app-editors/imhex` | `~amd64` |
| plcli | build WerWolv/PatternLanguage with cmake | |
| frida | pentoo `dev-util/frida-tools`, `dev-libs/frida-core`, `dev-python/frida` | plus the Windows `frida-server.exe` from GitHub releases for the Wine prefix |
| scanmem / GameConqueror | `dev-util/scanmem` | |
| PINCE | upstream install script | |
| edb-debugger | pentoo `dev-debug/edb-debugger` | |
| rizin / cutter | `::gentoo` | |
| lief | pentoo | |
| yara-x / yara | `::gentoo` | |
| capstone | `::gentoo` | |
| fuzzers | `app-forensics/aflplusplus`, bentoo `dev-util/cargo-fuzz` | |
| test and lint | `dev-util/cargo-nextest`, bentoo `cargo-llvm-cov` and `cargo-deny`, `dev-util/reuse` | |
| docs | `app-text/mdbook` | |
| data and diff | `dev-util/diffoscope`, `dev-db/sqlitebrowser`, `dev-vcs/git-lfs` | git-lfs for the private lab only |
| Vulkan / graphics debug | `dev-util/vulkan-tools`, `dev-util/spirv-tools`, `gui-wm/gamescope`, mangohud | |
| Wine | `wine-staging` 11.19 (installed), `winetricks` | separate prefix per analysis setup |
| Windows VM | `app-emulation/qemu` + libvirt + virt-manager | for x64dbg, API Monitor, native RenderDoc D3D11 |

---

## 11. Open questions / TODO

- Can `Render.GraphicsCore` actually select OpenGL on the current client? It is in `UserPreferences.ini` in the prefix's Documents folder, which was not found in this session.
- Does SSLKEYLOGFILE work end to end through Wine schannel → GnuTLS with the client's libcurl?
- Is the modern UDP framing and "encryption" the same as MER 2013? Capture a session and compare headers.
- Does Frida Option B (Linux frida directly on the Wine process) work, or only Windows `frida-server` in the prefix?
- Does the Havok `.hks`/`.hkx` packfile version need its own clean-room parser? Havok is proprietary, so no SDK code can be used.
- Umbra occlusion data in the DATs (DID types unknown) will need a replacement culling system.
- There was no LOTRO-specific discussion on OwnedCore, Elitepvpers or RageZone in this session's searches. MER's org and its archived blog "lotro revenge" (bwgypyth.blogspot.com; Wayback 2023-11-27, posts on packet capture and headers) and lotrounpacker.ucoz.com (Wayback 2025-09-16) are the main dead or historical sources.
