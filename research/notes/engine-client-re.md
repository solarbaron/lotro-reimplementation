# LOTRO client engine internals & client-side RE — research notes (2026-10-03)

## 0. Environment warning for the orchestrator
- **The root filesystem `/` was 100% full (500G/500G) while I worked.** `/tmp` is on `/`, and the shared scratchpad holds about 4.7 GB of clones from all agents. I freed about 500 MB by deleting only my own redundant clones (`digero_maestro`, `mer-lotro-analyzer`). Later agents may get ENOSPC errors.
- Clones I used are in `/tmp/claude-1000/-home-solarbaron-git-lotro-reimplementation/5f7fd9e6-9dcf-4273-b5fe-0179f3226498/scratchpad/clones/`:
  - `NeuralFoundry_lotro-freecam-toolkit`
  - `mklinik_lotro-fov`
  - `lunarwtr_vscode-lotro-api`
  - `lunarwtr_lotro-api-docs`
  - `NikolaiVChr_maestro`
  - `Mirarkitty_lotro_extractor`
  - `JuneStepp_OneLauncher`
  - `Middle-earth-Revenge_lotro-server`
  - `mer-RevengeBlog`
  - `Felix-Helleckes_MithrilUI`
  - `Chorizite` (Asheron's Call symbol map)
- Wiki page text I pulled through the Wayback Machine (lotro-wiki.com itself sits behind a bot challenge) is in `scratchpad/web/*.txt`: Music, Commands, Emotes, Lua_Plugins, Lua_Scripting, Mac_Client, coords.

## 1. Executables, bitness, graphics APIs (verified)
- **Client binaries.**
  - 64-bit client: `x64/lotroclient64.exe`.
  - The legacy 32-bit `lotroclient.exe` went offline on 27 Aug 2025, and all 32-bit worlds closed on 31 Aug 2025 (MassivelyOP). **Target 64-bit only.**
  - The launcher is `LotroLauncher.exe` / `TurbineLauncher.exe`, configured through `TurbineLauncher.exe.config` / `launcher.config`.
  - The in-game store browser runs as a separate process: `lotrobrowserhelper64.exe` plus `libcef.dll` (Chromium Embedded). Seen in a Wine backtrace from 2026 (WineHQ bug 59762).
- **Recent build IDs.** `4905.0070.9961.4022` (Sept 2026) and `4901.0070.8449.4010`. The 64-bit image base is `0x140000000` and SizeOfImage is about 36 MB.
- **Graphics backends.** These are pluggable "GraphicsCore" DLLs, taken from `strings` on both executables (WineHQ bug 51133, 2021):
  - `D3D11GraphicsCore.dll`, `D3D10GraphicsCore.dll`, `OpenGLGraphicsCore.dll`.
  - The D3D9 path is built into the exe; it loads `d3d9.dll`, and its error string is `"RENDER| failed to load D3D9.dll."`.
  - The setting is `UserPreferences.ini [Render] GraphicsCore=D3D9|D3D10|D3D11|OpenGL`. The in-game option is called "Graphics Hardware Level".
  - The OpenGL core goes back to the native Mac client (about 2014 to April 2019).
  - The freecam source (2026) says: "The legacy D3D9 backend … the active DX11 backend lives in D3D11GraphicsCore.dll". So DX9 and DX11 both still exist in the 64-bit client.
  - The client also uses D3DX: `d3dx9_42.dll` and `d3dx11_42.dll`. `D3DX11LoadTextureFromTexture` is used by the grass ("frills") system.
- **Other middleware, from the import strings:**
  - Havok, at least the base library and hkStackTracer. Turbine licensed Havok Physics, Animation and Behavior 6 in 2008.
  - `umbra.dll` (Umbra occlusion culling / PVS).
  - `binkw64.dll` (Bink video).
  - `OpenAL32.dll`, `vorbisfile.dll` (Ogg Vorbis) and `DSOUND.dll`.
  - `xerces-c_2_6.dll` (XML).
  - `lua51.dll` (the plugin Lua is **5.1**).
  - `libcef.dll`, `steam_api64.dll`, `LightFX.dll` (Alienware FX), `xinput9_1_0.dll`.
  - `corestrings.dll`, `pl.dll` / `lotropl.dll` (purpose unknown).
  - MSVC 2015+ runtime (`VCRUNTIME140`).
- **SpeedTree** is unconfirmed; I found no evidence of it. The engine has its own tree imposter LOD chain (see section 3).

## 2. Wine / Proton / DXVK (verified)
- **ProtonDB**, Steam app 212500: tier gold, best reported platinum, 166 reports.
- **OneLauncher** (JuneStepp, Python, active as of Aug 2026) is the reference Linux setup:
  - `WINEDLLOVERRIDES`:
    - `mscoree=d;mshtml=d`
    - `d3dx11_42=n`: native d3dx11_42 from lutris d3d_extras v2, needed because Wine's `D3DX11LoadTextureFromTexture` is a stub, which causes a frills memory leak.
    - DXVK DLLs `d3d11=n;dxgi=n;d3d10core=n;d3d9=n`.
  - Uses DXVK 3.0.1 on Linux and `WINEESYNC` / `WINEFSYNC`.
  - On first run it writes `[Render] D3DVersionPromptedForAtStartup=11`, `GraphicsCore=D3D11`.
  - On macOS it also writes `[Display] FullScreen=False`, `ScreenMode=FullScreenWindowed`, and `[Render] FrillTerrainColor=False`, because the game crashed on startup after Update 49.
- **CrossOver note.** DXVK in CrossOver 25 is incompatible with LOTRO's DX9 path; DX11 works.
- **Older WineHQ bugs:** 44545 (D3D11 startup crash), 51133 (64-bit client exits after the loading screen), 48892, 59762 (CEF store crash, 2026).
- **Settings folder:** `Documents/The Lord of the Rings Online/`. It holds `UserPreferences.ini`, `lotro.keymap`, screenshots, chat logs, `Plugins/`, `PluginData/`, `Music/` and `ui/skins/`.
- **Known `UserPreferences.ini` keys:**
  - `[Display]`: `Resolution=WxH`, `WindowedResolution=WxH`, `FullScreen`, `ScreenMode`.
  - `[Render]`: `GraphicsCore`, `D3DVersionPromptedForAtStartup`, `FrillTerrainColor`.
  - Engine variables seen in the freecam code: `Graphics.MemoryUsage` (max 2.0), TextureFiltering, AnisotropicQuality, TextureDetail, MaterialDetail, ModelDetail, ObjectDrawDistance, LandscapeDrawDistance, DistantImposters, FrillDistanceQuality, FrillDensity, FrillTerrainColor, LandscapeStaticObjectShadows, LandscapeLightingQuality, FarLandscapeNormalMaps, NearFarSeamBlend.
  - The launcher reads the client type (Win64 / Win32 / Win32Legacy) from this file (DDO-ML notes, same engine).
  - **The full key list is still an open question.**
- **Client command line.** The `lotroclient64.exe` help output was captured in OneLauncher's `start_game.py`. Options:
  - Account and login: `-a/--account`, `--authserverurl`, `--chatserver`, `-h/--host`, `-p/--port`, `--outport`, `--glsticket`, `-z/--glsticketdirect`, `--glsticketlifetime`, `-f/--franchise`, `--gametype`, `--connspeed`.
  - Characters: `-r/--create`, `-u/--user`, `-s/--specify race,class`, `--loadfile`, `-m/--mps` (monster play session).
  - Client settings: `--keymap` (also reads `<file>c.ext` and `<file>s.ext` for Ctrl/Shift meta keys), `--language`, `--prefs` (also moves the settings folder), `--resource-set`, `--HighResOutOfDate`, `--safe`, `--remotemouse`, `--rodat` (no-op).
  - Debug: `--debug <bitfield>`, and `--wfilelog` / `--wprintlog`, whose help text says "weenie event types". "Weenie" is a term inherited from Asheron's Call.
  - Other: `--usechat`, `--voiceoff`, `--voicenetdelay`, `--useexiturl`, `--surveyurl`.
  - The launcher fills a template with `{SUBSCRIPTION}` `{LOGIN}` `{GLS}` `{CHAT}` `{LANG}` `{CRASHRECEIVER}` `{AUTHSERVERURL}` and similar.

## 3. Renderer, world streaming and camera (from the freecam Frida script, 64-bit builds 4808–4905; verified by reading its source)
- **Main thread and services.**
  - The main thread has a tick function, MAIN_THREAD_TICK, and engine variable setters `T_SET_BYTE` / `T_SET_INT`.
  - An internal engine console dispatches text commands such as `PurgeGraphicsResources` and `GraphicsResourceManager.FreeSystemMemory`.
  - Services are found through a ServiceLocator with GUID-like interface IDs, for example LAND_PROVIDER_SERVICE_IID.
- **"SmartBox".** A global world/scene manager. One of its fields, `+0x250 LoadWorldForPlayerPosition`, makes world streaming follow the avatar. The same name exists in the Asheron's Call client: `SmartBox::SetOverrideFovDistance`, `teleport_in_progress` and others in Chorizite's `acclient.map`. This is direct engine lineage.
- **Landscape.**
  - Units are 16 "sectors" per landblock.
  - Landscape is drawn in three bands: full detail to about 14 blocks (224 sectors), "medium" to 24 blocks (384 sectors), then the lowest distant representation, which uses "distant imposters".
  - Full-detail terrain is capped by a resident-landblock budget, at roughly 16–20 blocks.
  - Terrain materials have a base material and a far normal-map material (`FarLandscapeNormalMaps`), plus near/far seam blending.
  - Static object shadows are baked into the landscape.
  - There is a landscape lighting quality setting, a texture mip-bias function and an object draw-distance multiplier.
  - Flags: `STATIC_PVS_FLAG` (Umbra/PVS) and `ALLOW_LODS_FLAG`.
  - Objects use a distance LOD chain: distant 2D tree imposters can be promoted to a medium 3D model once it is resident.
- **Grass ("frills").**
  - A separate `FrillEngine` system with its own streaming ring.
  - Struct fields: density, distance, depthTransitions, preCache, depthFade, reductionDistance/Enabled, playerBlend, loadRing, activeDraw.
  - It has a frame time budget (native value 0.0005, which gives about 8 ms in a 16 ms frame).
  - It gets grass surfaces from a landscape provider in 3×3-landblock "source groups", and the stream object has 9 residency slots.
- **AsyncCache.** Per-database-type caches with numeric type IDs: LandblockData 0x11, LandblockBlockmap 0x2f, LandblockInfo 0x2e, KynmapLandblock 0x64 (probably navigation/AI map data), EntityGroup 0x1a, LandblockProperties 0x29.
- **Landblock resource keys** look like `landcell:blockX:blockY:region:subcell`.
- **Position struct.** `{u32 valid; u8 blockX; u8 blockY; u16 region; ...; f32 x,y,z (local to the block)}`. There are three world-cell listeners on cell change, at +0x38 and +0x40.
- **Camera.**
  - Cameras live on a stack (PushCamera / PopCamera).
  - A built-in `FlightCameraController` exists for cinematics. It moves forward, strafe and vertical along direction vectors, and has a "use physics" flag and a move-speed global.
  - Camera input tick; `MakeFpsSlow`.
  - Field of view is computed as aspect ratio × a per-scene factor: 45 for third person and about 60 at character select (lotro-fov cheat table). On 64-bit the FOV pointer chain is `lotroclient64.exe+26E5240` → +18 → +70 → +0 → +10 (old build).
- **From dev interviews (2008):**
  - The DX10 path added dynamic tree and object shadows, cell shadowing on buildings, and better water, particle and distant-terrain lighting.
  - Moria's caves are landscape terrain with an inverted terrain ceiling.
- **Old (2007) Advanced Graphics options**, a useful checklist of renderer features:
  - Objects: Object Draw Distance, Model Detail, Animation Smoothness, Material Detail.
  - Scenery: Landscape Draw Distance, Frill Distance, Distant Imposters, Atmospherics Detail.
  - Texturing: Texture Detail, Texture Filtering, Anisotropic.
  - Lighting: High Quality Lighting, Specular, Surface Reflections, Landscape Lighting Quality.
  - Shadows: Landscape Shadows, Blob Shadows, Stencil Shadows, Environment Stencil Shadows.
  - Post-processing: Glow Mapping, Overbright Bloom, Blur Quality, Bloom Intensity.
  - Performance: Player Mesh Combining, 3D Object Portraits, Texture Cache Size, Player Crowd Quality.
  - Later releases added DX10/11 dynamic shadows, ambient occlusion and other options; I did not confirm the exact list.

## 4. Coordinate systems (verified, lotro-wiki Mechanics:Coordinate_Systems)
- **Region grid.** Each region is a grid of 160×160-unit landblocks, at most 256×256 blocks.
- **Regions:** r1 Eriador, r2 Rhovanion, r3 Gondor, r4 Mordor, r5 Haradwaith, r14 Tales of Yore. The maps for different regions overlap in coordinate space.
- **`/loc` output.** `You are on <World> server N at r1 lx1046 ly1147 [i43] [cInside] ox129.00 oy75.72 oz417.78 h358.6. Game timestamp T.`
  - `lx` / `ly` are landcells, 8 per block (20 m cells): X = INT(lx/8)*160 + ox.
  - `oz` is absolute height.
  - `h` is the azimuth (0 = north). It is omitted within about 1.3° of north.
  - `i` is the instance number.
  - `cInside` marks an interior cell. Inside, `o*` offsets are unbounded and `l*` is fixed.
  - The timestamp equals `Turbine.Engine.GetGameTime()`.
- **Map coordinates** shown under the minimap: ((X − 29360)/200, (Y − 24880)/200), displayed as N/S and E/W.
- Rendering uses camera-relative coordinates: world = 160·cell + float.

## 5. Lua plugin system (official, legal-safe spec — highest value)
- **History.** Introduced in beta in summer 2010 (Sept 2010 docs). The in-game Plugin Manager arrived in Update 5 (Dec 2011). Official HTML docs were posted on lotrointerface; the latest is "Official Update 25 Lua Documentation" (file id 1054), © 2016 SSG. Helm's Deep docs are id 862.
- **Runtime.**
  - Embedded Lua 5.1 (`lua51.dll`), sandboxed. Community sources say only string, math and table plus a few basics are exposed, with no io and no os writes.
  - Plugins cannot issue game commands, a deliberate anti-bot rule.
  - Plugins have a custom `import "Author.Package"` that resolves dots to folders and runs `__init__.lua`.
  - Turbine's sample plugins supply the `class()` helper (`Turbine/Class.lua`).
- **Files.**
  - Layout: `Plugins/<Author>/<Name>.plugin`, plus `.plugincompendium` (LPC manager), `.skincompendium` and `.musiccompendium`.
  - The `.plugin` file is XML: `<Plugin><Information><Name/><Author/><Version/><Description/><Image/></Information><Package>Author.Name.Main</Package><Configuration Apartment="x"/></Plugin>`.
  - Image: 32×32 JPG/TGA.
  - "Apartments" are separate Lua states; `/plugins unload` unloads whole apartments.
  - Commands: `/plugins load|unload|list|refresh`.
  - Persistence: `Turbine.PluginData.Save/Load(scope, key, data, cb)` with DataScope Account=1, Server=2, Character=3. Data is stored as `.plugindata` under `PluginData/<Account>/…`.
- **API tree, from the in-game dump TurbineDump.json in vscode-lotro-api (v1.0.18, March 2026):**
  - `Turbine`: Chat (event Received), ChatType, DataScope, Engine (GetCallStack, GetDate, GetGameTime, GetLanguage, GetLocalTime, GetLocale, GetScriptVersion, ScriptLog), Language (English=2, EnglishGB=0x10000001, French=0x10000003, German=0x10000004), Object, Plugin (GetAuthor, GetConfiguration, GetName, GetVersion; events Load/Unload), PluginData, PluginManager / LotroPluginManager, Shell (AddCommand, GetCommands, IsCommand, RemoveCommand, WriteLine), ShellCommand.
  - `Turbine.Gameplay` (48 members): Actor (morale/power/temp/effects/target/level), LocalPlayer (attributes, backpack, equipment, mount, vault, shared storage, wallet, trained/untrained skills, IsInCombat), Party, Effect / EffectList, Item / ItemInfo / ItemCategory (188 entries), Equipment, Bank, Wallet, Recipe, Profession, Class, Race, CombatMount, per-class Attributes and stances.
  - `Turbine.UI`: Control (about 51 members, with drag/drop, key and mouse events and an `Update` per-frame event), Window, Label, TextBox, Button, CheckBox, ListBox, ScrollBar, TreeView / TreeNode, MenuItem / ContextMenu, Graphic, Display, Color (143 named colors), BlendMode (Color=1, Normal=2, Multiply=3, AlphaBlend=4, Overlay=5, Grayscale=6, Screen=7, None=8), ContentAlignment 1–9, MouseButton (L=1, R=2, M=4), FontStyle (None=0, Outline=8).
  - `Turbine.UI.Lotro`:
    - Action enum: 270 key-binding action IDs, mostly 0x10000002+, plus Undefined=0, CameraInstantMouseLook=92, CaptureScreenshot=116, Escape=145. Effectively the keymap action table.
    - Font enum: DIDs 0x420000xx, e.g. Verdana14=0x42000001, TrajanPro20=0x42000000, BookAntiqua*, Arial12, LucidaConsole12, FixedSys15.
    - LotroUI / LotroUIElement (hide native panels): PluginsManager, Party, Backpack1-6, Vitals, Target.
    - Quickslot / Shortcut / ShortcutType (Item=2, Alias=4, Emote=5, Skill=6, Pet=7, Hobby=9), EntityControl, ItemControl, EffectDisplay, GoldWindow / GoldButton.
  - ChatType values: Undef 0, Error 1, Admin 3, Standard 4, Say 5, Tell 6, Emote 7, Fellowship 11, Kinship 12, Officer 13, Advancement 14, Trade 15, LFF 16, OOC 18, Regional 19, Death 20, Quest 21, Raid 23, Unfiltered 25, Roleplay 27, UserChat1-4 28–31, Tribe 32, PlayerCombat 34, EnemyCombat 35, SelfLoot 36, FellowLoot 37, World 38, UserChat5-8 39–42.
  - The event dump (68 events) is in TurbineEventsDump.json.

## 6. UI skinning (officially documented, Feb 2007, lotrointerface thread t=11)
- **Location.** `<documents>/ui/skins/<SkinDir>/SkinDefinition.xml` plus TGA media (RGB+alpha). Skins are read only at client startup and chosen under Options → UI Settings → Current User Skin.
- **Format.** `<opt><SkinName Name="..."/><Mapping ArtAssetID="..." FileName="..."/>…<PanelFile ID="..."><Element ID="..." X Y Width Height>…</Element></PanelFile></opt>`. Extra attributes in the XSD: Detach, ScaleOriginX/Y, MappingURI.
- **SkinDictionary** (about 2 MB, about 27k tags) holds the complete PanelFile/Element tree with default geometry for every native panel. It is in vscode-lotro-api `resources/skinning/SkinDictionary.xml`, with community extras in a separate file.
- The ArtAssetID list from 2007 covers only about 5% of modern assets (MithrilUI mined about 2,795 IDs).
- **Native UI.** It is C++, data-driven by UI layout records in the dat files (DIDs 0x22xxxxxx; e.g. 0x22000041 maps, 0x22000033 toolbar, 0x2200084B slot icons, 0x220008BB travel map). UI element IDs come from enum DID 0x230000D1 (lotro-tools usage). The layout parser itself (LotroCompanion `lotro-dat`) is not public.
- `/ui layout save|load <file>` exports and imports the user layout. Ctrl+\ repositions the UI.

## 7. Player music (ABC) — legal-safe spec plus empirical dialect
- **Usage.**
  - Files: `Documents/The Lord of the Rings Online/Music/*.abc`.
  - Commands: `/music` (toggle music mode, auto-equips an instrument), `/play <file> [sync]`, `/playstart`, `/playlist /[filter]`, and `.` to stop.
  - The "Quantize Player Music" audio option exists.
  - Playing live: keys 1–8 play C3..C4; Ctrl lowers by a semitone; Shift moves up an octave. Bindable notes run from C2 to C5 (37 semitones).
- **Instruments (24 in Maestro).**
  - Lute of Ages, Basic Lute, Basic and Misty Mountain Harp, Basic Theorbo.
  - Fiddles: Traveller's Trusty, Bardic, Basic, Lonely Mountain, Sprightly, Student's (FX notes up to D2, chromatic from G2).
  - Winds: Bagpipe (drones up to B2), Basic and Brusque and Lonely Mountain Bassoon, Clarinet, Flute, Horn, Pibgorn.
  - Percussion: Basic and Moor Cowbell, Drum (pitch selects the drum sound), Jaunty Hand-knells.
  - The sustain flag and octave offset differ per instrument (e.g. Flute +2, Theorbo −1 octave relative to the Lute).
- **Limits, from Maestro's in-game test suite.**
  - Notes: ABC `C,` to `c'` (MIDI 36–72; LOTRO ABC sounds one octave off from standard ABC).
  - At most 6 notes per chord.
  - Shortest note 60 ms, computed as `n/d * L * 60/bpm * meterDenominator` in double precision. Some "strange" BPMs (9, 11, 13, 15, 37, 43, 45, 51 and those values doubled repeatedly) round below 60 ms and are refused.
  - Longest note or rest is 8 s.
  - LOTRO takes the beat from the M: denominator whatever Q: says. Release is about 0.2 s.
  - Volume marks `+pppp+`..`+ffff+` are supported, including capitals.
- **Constructs LOTRO refuses or ignores** (Maestro's B-numbered tests, documented in `AbcToMidi.java`): `:|:` / `:|]`, inline fields, `!decorations!`, grace notes (ignored), `Z`, `y`, `$`, a backtick, `[|]`, a length or tie after a chord, broken rhythm next to a chord, an empty K: or text after the key, M:none, text in Q:, an L: after notes, T/H/L/M/O/P/S/u/v, typographic quotes.
- Chord symbols ("Am") are ignored. LOTRO plays repeats `|: :|` and `[1 [2`.
- Fellowship sync uses `/play x sync` followed by `/playstart`.

## 8. Chat, commands, emotes
- **Chat channels.** `/say /shout /tell(/t /w /msg) /reply /retell /f /k /ko /o /ra /regional /ooc /advice /lff /trade`. User channels: `/joinchannel <name> [pw]`, `/leavechannel`, `/1../4`, `/uc#`, `/listchannels`.
- **Chat options and filters.** `/chatfont`, `/filter`, `/filterlist`, `/FullIRCmode`.
- **Utilities.** `/alias ;x …` (with a `;target` substitution), `/shortcut`, `/loc`, `/time`, `/localtime`, `/servertime`, `/played`, `/ui layout`, `/help`, `/bug`, `/stuck`, `/follow`, `/useselection`.
- **Social.** `/pet …`, `/raid …`, `/kinship …`, `/friend`, `/ignore`, `/anon`, `/rp`, `/lfg`, `/who`, `/duel`, `/spar`, `/yield`, `/damagetext`, `/tutorial`, `/emotelist`.
- **Emotes.** `/<emote>` produces four text variants (self or others × with or without target) plus an animation. `/me` / `/em` give free text. Emotes are unlocked by collections, deeds and the store. The emote strings live in the localized dat.
- **Default keymap.** Fully listed on the lotro-wiki Commands page:
  - Quickslot bars: 1–12, Ctrl, Alt, Shift.
  - Targeting and panels: F1–F6 select fellow, Tab cycles foes, M map, I bags, K skills, J traits, L quests, etc.
  - Camera: numpad keys, Num* toggles first person, X lock-on.
  - Non-rebindable: Ctrl+F (FPS counter), Alt+F10 (lantern).
- **`lotro.keymap` file format:** not found; this is an open question.

## 9. Client memory and data model (lotro_extractor, Linux, 2026)
- **Base types.** 64-bit, with "intSize" 8 for hashes and `ReferenceCountTemplate` headers of 16 bytes. The name ReferenceCountTemplate, along with PStringBase and PackObj, also appears in the Asheron's Call client symbols.
- **Entities.**
  - A hash table maps instanceID (u64) to world entity (332 bytes); the property provider sits at +192.
  - In-world IDs have bit 0x0200000000000000 set.
- **PropertiesSet.** pid → (descriptor{pid,type}, value). Type codes are the same as on disk:
  - 1 STRING, 4 TIMESTAMP (f64), 6 VECTOR, 7 INSTANCE_ID, 8 ENUM_MAPPER, 9 FLOAT, 10 PROPERTY_ID, 11 STRUCT, 12 ARRAY, 13 STRING_INFO, 14 BITFIELD64, 15 INT, 16 COLOR (4×f32), 17 POSITION, 18 BITFIELD32, 19 LONG64, 20 DATA_FILE, 21 BOOLEAN, 22 BITFIELD.
  - The property-name registry is master DID 0x34000000 (27,268 entries).
- **Gameplay script layer.** A "WSL" object plane plus the WLib class library, stored at DID 0x56000000 in client_gamelogic.
  - Container format: chunked, FOURCC 0xB8C577F4. Chunks: CLASS_DEFS=16, CLASS_VARS=512, PARENTS_MAP=1024. Versions 30, 36 and 38.
  - A references table holds 262,144 entries with a GC generation byte; bit 0x10000000 marks native packages.
  - Native package IDs: 166 PropertiesSet, 52 DBProperties, 225 UTF-16 string, 199 StringInfo, 160 Position, plus hash, list and array types.
  - Slot values carry type codes: REFERENCE 1, INTEGER 2, FLOAT 3, LONG 130, SIGNED_DOUBLE 131, DOUBLE 195.
  - The dat variable-length integer encoding (VLE) is documented.
- **Implication.** Gameplay objects (quests, deeds, inventory) are instances of scripted classes defined in the dat. A reimplemented client should model a generic class/attribute runtime driven by WLib rather than hardcoded C++ structs.

## 10. Engine lineage
- Turbine engine: Asheron's Call (1999), then the AC2 "G2" engine (2002), then DDO (2006) and LOTRO (2007), which share the client codebase. DDO-ML and RE-DDO apply too.
- **Asheron's Call client symbols.** Chorizite `acclient.map` has 10,916 symbols. It shows classes LOTRO still uses by name: UserPreferences::RegisterPreference (bool/ulong/float/string preferences, the basis of `UserPreferences.ini`), SmartBox, CLandCell, LandBlock, CameraManager, CInputManager, UIElement (814 hits), ClientCommunicationSystem, GraphicsResource.
- **Use for knowledge only.** These symbols are useful for vocabulary and architecture.

## 11. Network / launcher (overlaps with other domains; brief)
- **Protocol.**
  - UDP. The header is 22 bytes during session setup and 20 bytes after.
  - Packets are RC4-encrypted after the handshake, with a checksum table.
  - Login goes through a GLS ticket from the launcher web service, then a world server address and port.
- **RE sources.** tAmMo's "LOTRO Investigation" PDF (2013) and the bwgypyth blog (2009–2016, mirrored in MER RevengeBlog), which includes a checksum-bypass patch for the 32-bit exe.

## 12. Legal-safe specs vs knowledge-only material
- **Safe to implement from directly:**
  - The official Lua API (implement the behavior; don't redistribute the SSG HTML).
  - The skin XML format.
  - ABC (an open standard), plus the LOTRO dialect learned from observing the game (Maestro, MIT).
  - Command and emote names.
  - INI and folder layout.
  - Coordinate math.
- **Knowledge only:**
  - Memory offsets (freecam, fov, Palantir, lotro_extractor). Memory tools violate the ToS.
  - The decompiled LotroCompanion jars. Use them as format hints and re-derive independently; do not copy code.
  - The AC PDB symbols.
- **Recommended clean-room process:** observe behavior (RenderDoc/apitrace captures, Wine traces, Lua dumps), write specs, then implement from the specs.

## 13. Tools a reimplementation will want
- **Rendering RE:**
  - RenderDoc (DX11 on Windows; also works on Vulkan through DXVK under Wine).
  - apitrace for D3D9/D3D11.
  - DXVK with `DXVK_HUD`.
  - PIX (Windows).
- **Binary RE:** Ghidra or IDA with masked-signature transfer between builds (the freecam method). The launcher keeps the previous client under `backup\x64\`, which allows binary diffing across patches.
- **Runtime instrumentation:** x64dbg, Frida, Cheat Engine (knowledge only), and Process Monitor.
- **Network:** Wireshark.
- **Data:** a dat reader and the propset/WLib decoders.
- **Lua 5.1:** the reference implementation (MIT), or LuaJIT in 5.1 mode, to host plugins.
- **Audio:** OpenAL Soft plus libvorbis to match the original audio stack.
- **XML:** a Xerces-compatible parser for skins, plugins and launcher config.
- **CEF:** optional, for the store and web panels.
- **Launcher and settings:** OneLauncher as a launcher and settings reference. Maestro / ABC Player as an ABC behavior oracle.

## Open questions
- `lotro.keymap` syntax.
- The full `UserPreferences.ini` key list.
- Whether the D3D10 and OpenGL cores still ship in 2026.
- The sky, water and weather systems.
- The animation format and how much Havok is used (physics, animation, behavior?).
- The meaning of `pl.dll` / `lotropl.dll`.
- The UILayout dat record format (no public parser).
- A newer official Lua doc than Update 25.
