
# Analogous open-source engine reimplementations: lessons for a clean-room LOTRO client

Scope: architecture, tooling, legal posture and stack choice, drawn from projects that reimplement or port other game engines. I shallow-cloned and read the source of most repos listed below. All clones are in `/tmp/claude-1000/-home-solarbaron-git-lotro-reimplementation/5f7fd9e6-9dcf-4273-b5fe-0179f3226498/scratchpad/clones/` (that directory is shared with other agents). The GitHub API rate limit ran out partway through, so some of the later metadata comes from `git ls-remote`, raw.githubusercontent or web search.

---------------------------------------------------------------------
## 1. Asheron's Call lineage (same developer, Turbine; HIGHEST PRIORITY)

Turbine licensed its AC engine as "Turbine 3D game engine, WorldBuilder 2.0" in May 2000 (gamedeveloper.com). The engine covered rendering, animation, sound, physics, AI and client-server code. LOTRO and DDO descend from it. The source code confirms the link: the DDO/LOTRO dat header has the same layout as the AC dat header (details below).

### 1.1 AC dat container format (from Chorizite/DatReaderWriter, verified in source)
- **Header at file offset 0x000.** `Version` is a 256-byte ASCII string, then a 64-byte `Transactions` blob. The structured header starts at **0x140**. All fields are little-endian int32 unless noted:
  - 0x140 Magic = `0x00005442` ("BT")
  - 0x144 BlockSize (default 1024)
  - 0x148 FileSize
  - 0x14C DatFileType (1 Portal, 2 Cell, 3 Local)
  - 0x150 SubSet (region id for cell dats; HighRes is a portal type with a different subset)
  - 0x154 FirstFreeBlock, 0x158 LastFreeBlock, 0x15C FreeBlockCount
  - 0x160 RootBlock (B-tree root)
  - 0x164 NewLRU, 0x168 OldLRU, 0x16C UseLRU
  - 0x170 MasterMapId
  - 0x174 EngineVersion, 0x178 GameVersion
  - 0x17C MajorVersion (16-byte GUID)
  - 0x18C MinorVersion (uint32)
  - The header is 400 bytes in total. A new header's Transactions blob starts with bytes `00 50 4C 00`.
- **Block chain.** File data is stored as a linked list of fixed-size blocks. The first int32 of each block is the offset of the next block. On the free list the high bit is set (`next | 0x80000000`), and the last block holds `0x80000000`.
- **B-tree node** (1720 bytes; `MAX_BRANCHES=62`, `MAX_FILES=61`):
  - 62 int32 branch offsets
  - int32 fileCount at byte 248
  - 61 entries of 24 bytes: `u16 Flags`, `u16 Version` (default 2), `u32 Id`, `i32 Offset`, `u32 Size`, `u32 Date` (unix time), `i32 Iteration`
  - A node is a leaf when it has 0 branches.
- **Compression.** Flag `0x01` means IsCompressed. The payload is a u32 uncompressed size followed by a zlib stream; DRW decodes it with .NET `ZLibStream` and loops until the stream is drained.
- **Object type by ID range.** For example, GfxObj is `0x01000000..0x0100FFFF` with flags `HasId`. Records use `CompressedUInt` counts, `QualifiedDataId<T>` cross-references, and PixelFormat enums that are the D3D `D3DFMT` values (A8R8G8B8=0x15, P8=0x29, DXT via FourCC, and so on).

### 1.2 Turbine dat format as used by DDO/LOTRO (Endevir/DatExplorer = Mogwai.DDO.Explorer, C#, AGPL-3.0, 2018; verified in source)
- `_offset = 0x140`. The field order is the same as AC: magic, blockSize, fileSize, fileVersion (type), unknown (subset), firstFree, lastFree, freeCount, root, then four zero dwords. 0x174 is "datpack version engine" and 0x178 is "datpack version game", the names quoted from `dndclient.log`. This is direct evidence that the formats share a lineage.
- Directory node layout:
  - 8 bytes of zeros
  - 62 × (u32 size, u32 offset) sub-directory pairs. Iteration stops at offset 0, at 0xCDCDCDCD, or when the size differs.
  - 61 × 32-byte file entries: `unk1, fileType, fileId, offset, size1, timestamp, unk2(version/iteration?), size2`.
  - Compared with AC: the fan-out is the same (62/61), but branches carry a size and file entries are 32 bytes instead of 24.
- CompressionType enum: 0 Uncompressed, 1 Maximum, 2 unknown, 3 Default. These look like zlib levels.
- Content sniffing: `RIFF....WAVE` → Wave, `OggS` → Ogg, `DXT1` and other FourCCs → DDS-like textures.
- Practical consequence: the C++/Rust dat layer can be written once with a "Turbine dat profile" parameter (AC vs DDO/LOTRO entry size, branch format, compression variant), and AC dats can be used as a free test fixture during development.

### 1.3 DatReaderWriter architecture (Chorizite, C#, MIT, net8/netstandard2.0/net48, NuGet `Chorizite.DatReaderWriter` 2.1.x)
- **Schema-first design.** `DatReaderWriter/dats.xml` (4,300 lines) defines enums, types and DBObjs using `<field>`, `<vector length=...>`, `<maskmap>`/`<mask>` for conditional fields, and `first/last/mask` ID ranges.
  - A Roslyn source generator (`DatReaderWriter.SourceGenerator`: XMLDefParser, TypesGenerator, DBObjsGenerator, EnumsGenerator, HashTableGenerator) emits about 260 `*.generated.cs` files with Unpack/Pack.
  - The protocol uses the same pattern: `Chorizite.ACProtocol/protocol.xml` (8,562 lines, derived from zegeger's messages.xml and ACE enums) feeds `C2SMessageHandler.generated.cs`/`S2CMessageHandler.generated.cs`.
  - **Recommendation: adopt this pattern for LOTRO.** Keep a language-neutral schema (XML/KDL/YAML, or Kaitai `.ksy`) for dat records and network messages, then generate engine code, tool code and documentation from it.
- Block allocators are pluggable: `MemoryMappedBlockAllocator` and `StreamBlockAllocator`. Caching options are OnDemand or None. It has an async API, full B-tree insert/delete/range queries, and a benchmarks project.
- **Testing (copy this approach):**
  1. Synthetic tests (`CanInsertAndReadActionMaps`, etc.) create an empty dat in a temp file (`BlockAllocator.InitNew(DatFileType.Portal,0)`), write objects, and read them back. They need no copyrighted data and run in CI.
  2. Real-data tests are tagged `[TestCategory("EOR")]` and read the directory from env var `EOR_DAT_DIR`.
  3. Combinatorial attributes run each test across reader modes.
- ACE.DatLoader.Tests (older) assert exact file counts per dat. That is a useful regression check, but it ties tests to one client version.

### 1.4 ACE (ACEmulator, C#, AGPL-3.0)
- Server emulator. The solution is split into ACE.DatLoader, ACE.Entity, ACE.Database (MySQL), ACE.Server and ACE.Server.Physics, plus test projects.
- **Legal caution:** ACE's physics engine was ported from a decompiled client. Its comments say, for example, "The client decompile has this branch of code as well" (Physics/Polygon.cs) and "acclient checks both of these here".
- Chorizite.Core ships `acclient.map` (10,915 symbol lines). Chorizite also has `ida-scripts` for "porting info from pdb client to eor client", because AC debug symbols are public.
- AC is a dead game whose trademarks have lapsed, and the community depends on decompilation. **LOTRO is a live commercial product. Do not port ACE or ACViewer physics code into the LOTRO client.** Use it only for high-level concepts (BSP collision, sphere-path transitions, landblock/cell model). Its AGPL license also binds any project that copies it.

### 1.5 ACViewer (ACEmulator, C#, GPL-3.0)
- MonoGame 3.8 WindowsDX inside WPF (MonoGame.Framework.WpfInterop). AssimpNet and FbxSharpie are used for export. Effects are HLSL `.fx` files (texture.fx, texture_clamp.fx).
- The code is split into Render/, Model/ (GfxObj, Setup, caches, instancing), Physics/ (a copy of ACE.Server physics), and viewers for models, textures, particles, the world and the map.
- It is Windows-only and therefore a poor template for portability. Its architectural split (model cache → instance batches → shader) is still a reasonable reference.

### 1.6 Chorizite ecosystem
- **Chorizite** (MIT) is a plugin manager that injects into the original acclient: NativeClientBootstrapper, an injector, SigScan, and an RmlUi- and Lua-based UI. It is not a reimplementation.
- Other repos in the org (verified via the GitHub org listing): RmlUi.Net, TaffySharp (Taffy layout bindings), ACBindings, LuaPlugin and Chorizite.VSCode (Lua intellisense).
- **WorldBuilder** (C#, last push 2026-04) is the best modern cross-platform AC renderer.
  - Stack: Avalonia UI, **Silk.NET.OpenGL + Silk.NET.SDL**, BCnEncoder, MemoryPack, SQLite (FluentMigrator), SignalR for collaborative editing, xUnit plus Avalonia.Headless tests.
  - The backend `Chorizite.OpenGLSDLBackend` has GLSL 330 shaders: Landscape, StaticObject (plus a "Modern" variant), Particle, PortalStencil, Outline, Text and Gizmo. Managers include TerrainChunk, TerrainGeometryGenerator, TextureAtlasManager, GlobalMeshBuffer, VisibilityManager, PortalRenderManager, EnvCellRenderManager, SceneryRenderManager and SkyboxRenderManager.
  - AC terrain shading uses a base texture plus up to 3 overlay textures with alpha masks (`sampler2DArray xOverlays/xAlphas`) plus 2 road layers. A `maskBlend3` function composites them, and a std140 `SceneData` UBO holds view/projection, light direction, sunlight and ambient colour.
  - Indoor cells are rendered with portal stencils.
  - **The repo has no LICENSE file**, which means all rights reserved. Read it for ideas only and do not copy code.
- **aclog/aclogview** (ACEmulator) capture and analyse client traffic. A LOTRO equivalent (capture plus a Wireshark Lua dissector generated from the protocol schema) is a core tool.
- **ACUnreal** (gitlab.com/acunreal/acunreal; last activity 2025-05; 4 stars) is an AC client prototype in Unreal Engine. It has C++ dat tools (ACUnrealDatTools: Portal/Cell file, Surface, Palette, Setup, Region, Landscape, Scene, Sound, plus `decompress.c`) and FBX/T3D exporters, so it is a C++ precedent for Turbine dat parsing.

---------------------------------------------------------------------
## 2. Other reimplementations: language, graphics, user data, tests, structure

| Project | Language / stack | User-supplied data | Testing | Structure lessons |
|---|---|---|---|---|
| **OpenMW** (Morrowind, GPL-3) | C++20 (CMAKE_CXX_STANDARD 20), CMake ≥3.16, OpenSceneGraph 3.6.5, SDL2 ≥2.26, OpenAL, Bullet (float64), MyGUI 3.4.3, LuaJIT/Lua, Recast/Detour, FFmpeg, LZ4, ICU, yaml-cpp, Qt6 (launcher/OpenCS), Boost, GTest/GMock | `openmw.cfg` `data=` paths plus `content=` ESM/ESP; an installation wizard extracts from CD (libunshield) | GTest components_tests, openmw_tests and opencs_tests; benchmarks (Google Benchmark); GitLab CI with ASan, UBSan and TSan jobs, clang-tidy, Coverity; **integration tests: the real engine runs headless under `xvfb-run` against the freely licensed `gitlab.com/OpenMW/example-suite` content and Lua test scripts (`scripts/integration_tests.py`), then osg_stats logs are checked** | `components/` (format libraries: bsa, esm3, esm4, nif, nifosg, nifbullet, vfs, resource, terrain, shader, lua, l10n, sqlite3, …) kept separate from `apps/` (openmw, opencs editor, launcher, wizard, **bsatool, esmtool, niftest, navmeshtool, bulletobjecttool**). The standalone format CLIs are the main thing to copy. |
| **OpenRW** (GTA III, GPL-3) | C++17, CMake, SDL2, OpenGL 3.3 (gl_core_3_3 loader), Bullet, OpenAL, FFmpeg, GLM, Boost; Conan; clang-tidy and IWYU options | Points to the user's GTA3 install | Boost.Test; `TEST_DATA` CMake option; tests that need game data carry label `data-test` and run with `--run_test=@data-test`; codecov, CDash | Split into `rwcore` (loaders: DFF, TXD, IMG, SDT), `rwengine`, `rwgame`, `rwviewer`, `rwtools`. Development has slowed (last commit 2025-06). |
| **OpenSAGE** (C&C Generals, GPL-3) | C# .NET, **Veldrid** (+SPIRV, SDL2, ImGui.NET, RenderDoc integration), SharpAudio, MoonSharp (Lua), LiteNetLib, ImageSharp, central package management | `InstallationLocator` + `SteamInstallationLocator` (VDF parser) find installed games | xUnit; `[GameFact(SageGame.X)]` skips a test when that game is not installed; codecov | Separate FileFormats.* projects (Big, RefPack, W3d), Game.CodeGen, Tools (BigEditor, BigTool, Sav2Json). EA released the Generals source under GPL-3 in 2025 (LICENSE-EA.md in repo), which changes this project's legal basis. |
| **OpenTESArena** (MIT) | C++20, SDL2, OpenAL, optional WildMIDI, **Jolt Physics**, **Vulkan renderer with a software-renderer fallback** (`GraphicsAPI=0 Vulkan / 1 software`) | User provides Arena (free from Bethesda) | Few automated tests | `components/` (archives, dos, vfs, utilities, debug) + `OpenTESArena/src` (Rendering backend abstraction: RenderBackend, VulkanRenderBackend). Defaults go in `options-default.txt` and user overrides in `options-changes.txt`. Very active (push 2026-09-26). |
| **Daggerfall Unity** (MIT) | C# on the **Unity** engine (proprietary runtime) | Free DOS Daggerfall (Steam app 1812390) | Essentially no automated tests | The data readers (`Assets/Scripts/API`, from DaggerfallConnect) are separate from the game layer; a large mod ecosystem. Lesson: a proprietary engine ties you to its licensing; avoid. |
| **ScummVM** (GPL-3+) | C++, own configure/Make, SDL/OpenGL plus many platform backends (`backends/graphics/{sdl,opengl,openglsdl,null,...}`) | Detects game data by MD5/file-size tables (AdvancedDetector) | `test/` unit tests; GitLab and GitHub CI | Many engines as plugins on a shared `common/graphics/audio/video/gui` core. **`AI-GUIDELINES.md` requires an `Assisted-by: AGENT:MODEL` trailer, bans AI (co-)authorship, and makes the human contributor responsible for license compliance.** Relevant to how this project records provenance. |
| **ACE / ACViewer / Chorizite** | C# (see §1) | AC dat files from the user | MSTest; synthetic and real-data split | See §1 |
| **SWGEmu Core3** (AGPL-3) | C++ server (engine3 submodule `github.com/swgemu/engine3`), Lua configs, MariaDB, Docker dev env | Users copy their client `.tre` files into a docker volume (`/tre`) | — | Server-side only; a command-line Core3Client exists. Clean-room by policy. Projects built on the leaked SWG source are a separate, legally risky branch (from memory). |
| **OpenLara** (BSD-2) | C++ header-heavy code, many GAPI backends (`gapi/gl.h, d3d8/9/11.h, vk.h, gxm.h, gu.h, c3d.h, sw.h`) and about 25 platform shims (sdl2, sdl3, web, psp, 3ds, gba, ...) | TR level files from the user | — | Shows a thin graphics abstraction can cover very different APIs. Not a good model for a large codebase. |
| **OpenXcom** (GPL-3) | C++, SDL1.2, yaml-cpp rulesets | Copy the UFO/TFTD folders; the Windows installer auto-detects Steam | GitHub CI | All game rules live in YAML **rulesets**, so mods override data rather than code. |
| **Arx Libertatis** (GPL-3+) | C++17 (up to 20), CMake ≥3.12, OpenGL (epoxy/GLEW), SDL, OpenAL, FreeType, GLM, Boost headers | `scripts/arx-install-data` (3,307-line shell script) extracts from GOG installers (innoextract), Steam (app 1700), CD (cabextract), demo | `tests/` (graphics, io, math, util); `CONTINUOUS_INTEGRATION`→`WERROR` | **Built on the officially released source, not a clean-room reimplementation.** It has a crash handler/reporter, profiler, savetool and unpak tools, and an option to build a standalone IO library. Copy the idea of one data-install script per store. |
| **fheroes2** (GPL-2) | C++17, SDL2 | GOG/Ubisoft data or the free **demo** (download script) | SonarCloud, CI | Written from scratch; started on SourceForge. |
| **devilutionX** (Sustainable Use / Unlicense mix; from memory) | C/C++, SDL2 (SDL1 and SDL3 builds also exist) | DIABDAT.MPQ (GOG) or shareware spawn.mpq | codecov, many CI targets (consoles, Amiga) | **Based on decompilation.** Devilution "reconstructed" Diablo's source with help from debug symbols in a PSX build. Blizzard has tolerated it. **Not a legal template for a live game.** |
| **re3 / reVC** (GTA3/VC) | C++, librw | — | — | **Cautionary.** Decompiled reimplementation. Take-Two filed a DMCA notice in Feb 2021; GitHub restored the repos after a counter-notice; Take-Two sued 14 developers in N.D. Cal. in Sep 2021, and the repos were removed again. A decompilation-derived codebase cannot survive a determined rights-holder. |
| **ClassicUO** (UO client reimpl, BSD-2) | C# on **FNA** (XNA reimplementation over SDL; D3D11, OpenGL, Vulkan and Metal through FNA3D), **NativeAOT** builds (`scripts/build-naot.sh`), browser build | Legally obtained UO client files | GitHub Actions Build-Test | **Closest MMO-client analogue.** Split into Assets (one loader per file type), IO, Renderer, Client/Network (Huffman, encryption, PacketsTable, PacketLogger, plugin API). Its legal section says: **"Using a custom client to connect to official UO servers is strictly forbidden."** For LOTRO: target emulator or private servers and never connect to SSG's official servers. |
| **OTClient** (Tibia, opentibiabr fork; C++20 + Lua) | C++20, OpenGL, Lua modules for the whole UI | Tibia .dat/.spr files | — | The UI and game features are Lua modules over a C++ core. This maps well onto LOTRO's XML UI and Lua plugin API. |

Other precedents worth knowing:
- OpenTTD and OpenRCT2 started from disassembly and were tolerated (from memory).
- City of Heroes "Homecoming" received an official NCSoft license on 2024-01-04 after years of tacit tolerance (massivelyop.com, verified by search). **Asking SSG/Daybreak for a license is a real option and worth pursuing in parallel.**

---------------------------------------------------------------------
## 3. Legal framework (summary for project policy)
- **US.**
  - *Sega v. Accolade* (1992) and *Sony v. Connectix* (2000) allow intermediate copying and disassembly for interoperability as fair use (from memory).
  - DMCA §1201(f) has an interoperability exemption.
  - However, **Davidson & Assocs. v. Jung (bnetd), 8th Cir. 2005, 422 F.3d 630** held that EULA/ToU clauses banning reverse engineering are enforceable contracts and waive §1201(f) (verified by search). LOTRO's EULA/ToS almost certainly bans reverse engineering, so US contributors who accepted it are exposed in contract.
- **EU.** Directive 2009/24/EC:
  - Art. 6 allows decompilation when it is *indispensable* for interoperability of an independently created program, the information is not otherwise readily available, and only the necessary parts are decompiled.
  - Art. 6(2) forbids using that information to create a program substantially similar in expression, and forbids passing it on except as needed for interoperability (from memory).
  - **Art. 8: contract terms contrary to Art. 6 or Art. 5(2)/(3) are null and void** (verified by search).
  - Art. 5(3) allows observing, studying and testing a program while lawfully running it (black-box analysis). That covers packet captures, RenderDoc frame captures and log analysis.
- **Policy recommendations:**
  1. Use a two-team clean-room: researchers write format and protocol *specs* (plain-language docs plus schema), and implementers read only the specs.
  2. Never commit decompiled code, IDA/Ghidra output, symbol maps, shader bytecode or extracted assets.
  3. Prefer black-box methods: dat structure analysis, traffic capture against your own account, and frame captures.
  4. Ship no assets. Users point the client at their own LOTRO install (LOTRO is free-to-play; Steam app 212500, verified).
  5. Do not connect to official servers; target an emulator.
  6. Record provenance in commit trailers (as ScummVM's AI guidelines do).
  7. Prefer contributors who have not accepted the LOTRO EULA for implementation work.
  8. Code from LotroCompanion repos (lotro-core, lotro-tools, lotro-companion, lotro-maps, lotro-jukebox-core, lotro-data-extractor) has **no LICENSE file**, so it is all rights reserved by default. Facts can be learned from it but code cannot be copied without Damien Morcellet's permission. The `delta-lotro-dat-utils` (v11.11) dat library they all depend on is **not public**; `git ls-remote` asks for credentials.
  9. GPL-3 (DATUnpacker, Elanor) and AGPL-3 (DatExplorer, ACE) code can only be reused if the project adopts a compatible license.

---------------------------------------------------------------------
## 4. Transferable architecture and tooling (recommended for the LOTRO base)
1. **Separate format libraries from the engine.** OpenMW (`components/`), OpenRW (`rwcore`), OpenSAGE (`FileFormats.*`), ClassicUO (`Assets`), Daggerfall Unity (`API`) and Arx (IO library) all do this. Plan for `libturbinedat` (container), `libturbineformats` (records), `libproto` (messages), `engine`, `client`, `tools/`.
2. **Ship a CLI tool per format from the first day**, after OpenMW's bsatool, esmtool and niftest: `datls`, `datcat`, `datdiff` (between client patches), `meshview`, `texconv`, `protodump`. This speeds up reverse-engineering iteration considerably.
3. **Generate code from a schema** (DatReaderWriter `dats.xml`, ACProtocol `protocol.xml`).
   - Options: Kaitai Struct `.ksy`. v0.11, released 2025-09-07, has C++/STL, C#, Java, Python, Lua, Go and **Rust (entry-level)** targets, plus serialization for Java and Python.
   - Or a custom XML/KDL schema with project-owned generators for the engine language, Python (RE scripts), a Wireshark Lua dissector and Markdown docs.
   - ImHex patterns and 010 Editor templates are useful for interactive RE.
4. **Use two test tiers:**
   - (a) Synthetic fixtures that run anywhere. Write a test-dat generator that produces valid Turbine dats with CC0 content (DatReaderWriter's InitNew pattern), and build an "example-suite" of free test content for headless integration tests (OpenMW xvfb + example-suite).
   - (b) Real-data tests gated by env var or CMake option (`LOTRO_DAT_DIR`, OpenRW `TEST_DATA`, OpenSAGE `[GameFact]`, DRW `[TestCategory("EOR")]`). Run them on self-hosted CI that has a legally installed client.
   - Also add golden-image render tests (SwiftShader/lavapipe), fuzzing of all parsers (libFuzzer/cargo-fuzz), and sanitizers (OpenMW runs ASan, UBSan and TSan jobs).
5. **Locate installs automatically**: Steam VDF parser (OpenSAGE), Windows registry, Wine/Proton/Lutris prefixes, and OneLauncher/PyLotRO game directories. Add per-version detection (ScummVM-style hash tables, keyed on dat header Engine/GameVersion and iteration numbers).
6. **Abstract the renderer backend** (OpenTESArena Vulkan + software; OpenLara GAPI headers; ScummVM backends). Include a null backend for headless tests.
7. **Put UI and gameplay scripting in Lua** (OTClient, OpenMW Lua, Chorizite Lua + RmlUi). LOTRO's own plugin API is Lua 5.1-based ("Turbine.*" namespaces; see lunarwtr API docs; from memory). A compatible implementation would make existing LOTRO plugins work. RmlUi (HTML/CSS-like, MIT) is a strong fit for LOTRO's XML-defined UI layouts.
8. **Build network tooling first**: an aclog/aclogview equivalent (pcap → parsed message log) plus a schema-generated Wireshark dissector. ClassicUO's PacketLogger and PacketsTable are good references.
9. **Distribution and CI**: GitHub Actions matrix (Linux/Windows/macOS), CMake presets + vcpkg (C++), or `cargo` (Rust), or NativeAOT (C#, like ClassicUO); crash reporter (Arx); Tracy profiler; RenderDoc integration (OpenSAGE has Veldrid.RenderDoc).

---------------------------------------------------------------------
## 5. Stack recommendation for a LOTRO client reimplementation

What the client needs: a large streamed outdoor world (landblocks, terrain splatting, many skinned characters, particles, sky, weather), DX9/DX11-era shading to reproduce, D3D-style pixel formats (DXT1/3/5, A8R8G8B8, P8), Ogg/WAV audio, a complex XML-based UI, Lua plugins, UDP netcode with fragment reassembly (AC lineage: PacketHeaderFlags, BlobFragments, ISAAC-style checksums), and Linux/macOS/Windows support.

**Option A: C++20/23 + CMake + vcpkg + SDL3 + Vulkan 1.3. RECOMMENDED primary choice.**
- Library versions and status:
  - SDL 3.4.18 (released 2026-10-02, zlib license); its SDL_GPU API is an alternative to raw Vulkan.
  - Vulkan via volk + VMA (both MIT), with MoltenVK on macOS. bgfx (BSD-2, active) is a lower-effort alternative if D3D11/Metal/GL backends are wanted.
  - Jolt Physics (MIT) for client-side collision and prediction.
  - RmlUi (MIT) plus Dear ImGui (debug tools).
  - LuaJIT or Lua 5.1 for plugin compatibility.
  - OpenAL Soft or miniaudio, plus libvorbis/stb_vorbis.
  - meshoptimizer; Tracy; GameNetworkingSockets is not needed (custom Turbine UDP).
- Why: the largest pool of reimplementation contributors (OpenMW, OpenRW, OpenTESArena, Arx, OpenXcom, fheroes2, devilutionX, ScummVM, OTClient); the best Kaitai target maturity; predictable performance for a big MMO world; a precedent for C++ Turbine dat parsing (ACUnreal, LotroDat-era tools).
- Risks: memory safety when parsing hostile network and dat input. Mitigate with fuzzing and sanitizers from the start.

**Option B: Rust + wgpu (v30.0.1, 2026-08) + winit (+ optionally Bevy ECS pieces).**
- Pros: memory safety for the parsers and netcode; cargo, cargo-fuzz and cross-compilation are excellent; wgpu targets Vulkan, Metal, DX12, GL and WebGPU (a browser client is possible).
- Cons: a much smaller pool of game-reimplementation contributors; Kaitai Rust support is entry-level; no existing LOTRO or AC Rust code to start from; slower iteration on very large codebases.
- A good choice if the core team already knows Rust. A hybrid also works: Rust for `libturbinedat`/`libproto` exposed through a C ABI, with the engine in C++.

**Option C: C# (.NET 8/10) + Silk.NET (2.23) / Veldrid / FNA, NativeAOT.**
- Pros: the AC lineage is almost entirely C# (DatReaderWriter MIT, ACE, ACViewer, WorldBuilder, ACProtocol), and the LOTRO community tools are too (DatExplorer, DATUnpacker GPL-3, Elanor GPL-3). The schema-plus-source-generator pattern can be reused as-is. ClassicUO shows C# + FNA + NativeAOT works for a shipping MMO client. Iteration is the fastest of the three.
- Cons: GC pauses in a large streaming world (manageable with pooling and Span<T>); Veldrid is now in maintenance mode (last push 2026-03); fewer engine-level contributors than C++.
- **Best choice for the tooling** (dat explorer, format RE workbench), even if the client itself uses Option A.

**Java (LotroCompanion).** The data knowledge (stat formulas, enums, property IDs, item/quest/deed structures in lotro-core and lotro-tools) is the most complete public LOTRO data model. Use it as reference documentation, and ask for permission before porting any code, because it has no license. Java is not suited to the client itself.

**Concrete recommendation:**
- C++20 engine and client: SDL3 + Vulkan (volk/VMA) behind a thin RHI with null and software backends, Jolt, RmlUi + Lua 5.1, OpenAL Soft. GPL-3.0-or-later license, which keeps GPL LOTRO tools and ACViewer ideas compatible; avoid AGPL code.
- A language-neutral **format and protocol schema repo** with generators for C++, Python and C# (and Rust later).
- Python and Kaitai for RE scripting; C# or ImGui-based tools for the dat explorer.
- Clean-room spec documents as the only interface between researchers and implementers.
- Synthetic test dats plus a free example-suite for CI, and gated real-data tests on self-hosted runners.

---------------------------------------------------------------------
## 6. Open questions / to verify
- The exact LOTRO dat file-entry semantics (`unk1`, `unk2`, `size1` vs `size2`, compression variants) and the LOTRO-specific header at 0x100 (if any) need confirming against the current 64-bit client. LOTRO-focused research should cover this.
- Where Gi1dor/Endevir's **LotroDat** C++ library lives now. `github.com/Gi1dor/LotroDat` and `Endevir/LotroDat` ask for credentials (private or deleted); look for forks or a Wayback snapshot.
- AC2D (an old open-source AC 2D client) is not under ACEmulator any more; it was not located.
- Whether SSG/Daybreak would consider an official license (Homecoming precedent).
- The LOTRO Lua version (5.1 assumed) and the Turbine.UI API surface (lunarwtr/lotro-api-docs clone exists in the scratch dir).
