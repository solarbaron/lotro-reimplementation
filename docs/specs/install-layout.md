# Install layout: locating, validating and identifying a game install

| | |
|---|---|
| **Status** | Draft |
| **Last updated** | 2026-10-03 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | (not yet reviewed) |
| **Consumers** | `crates/install`; also `crates/dat` (archive-set opening), `apps/client` (command line), `crates/script` (plugin folders) |

---

## 0. Purpose

This spec lets an implementer build the `install` crate without reading any dirty material. The crate must:

1. find candidate game installs on Linux, Windows and macOS (Steam, the standalone installer, Wine/Proton/Lutris/OneLauncher prefixes);
2. decide whether a directory is a usable install, and list which archives it has;
3. find the per-user settings folder that belongs to the install;
4. compute a **version fingerprint** for the install from the archive headers and a few housekeeping records, so that tests and caches can tell content versions apart;
5. accept the same command-line options as the original client, so existing third-party launchers can start our client unchanged.

The spec does not cover the dat container itself (header, extents, B-tree, compression). It repeats only the few header fields that identification needs. Lookup of a subfile by DID is the job of the dat container spec.

**Hard rule, which applies to every algorithm here:** the install directory is **read-only**. Nothing in this spec ever creates, renames, locks, truncates or writes a file under the install directory. Open archives read-only with shared access. On Windows that means allowing other readers and writers, because the official launcher may be patching at the same time.

---

## 1. Provenance

Marks: **Confirmed** = observed in the user's own install on 2026-10-03 (file listings, header bytes, small read-only probes of housekeeping records, Steam and Proton metadata files). **Reported** = from published sources only.

Two data versions were observed, both legally installed copies in the same Steam library:

- **Version A** (older): client file version `4601.0070.2401.4020`; newest subfile timestamp 2026-02-04.
- **Version B** (current): client file version `4906.0071.1794.4035`; newest subfile timestamp 2026-10-03.

Short source names:

- **OL**: OneLauncher (GPL-3), its game-dir search, launcher-config handling and launch code, plus the client help text it records (captured 2024-07-10).
- **LC**: LotroCompanion dat utilities.
- **EL**: Endevir Elanor launcher.
- **KD**: koniecdev LotroKoniecDev.
- **PL**: PyLotRO.
- **OS**: OpenSAGE Steam locator.
- **AC-DRW**: Asheron's Call dat readers (ACE DatLoader, Chorizite DatReaderWriter).
- **NOTES**: the project research notes (`dat-format.md`, `network.md`, `engine-client-re.md`).

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| P1 | Archive names are `client_<key>.dat`. Aux archives are `client_<key>_aux_<n>.datx` | Observed in user install 2026-10-03; LC; NOTES dat-format §1 | Confirmed |
| P2 | Archive inventory and sizes in §3.2. This includes `client_cell_15.dat` and `client_map_15.dat`, which the notes did not list | Observed in user install 2026-10-03 | Confirmed (corrects NOTES) |
| P3 | No archive reaches 4 GiB. The largest is 4,287,627,264 bytes | Observed in user install 2026-10-03 | Confirmed |
| P4 | Header word 0x14C identifies the archive kind, and word 0x150 holds the region number (cell/map) or a language code (local) | Observed in user install 2026-10-03 (values); AC-DRW "DataSet/SubSet" naming (meaning) | Confirmed (values), Reported (meaning) |
| P5 | Header 0x174 = 112 in every archive in both versions. Header 0x178 = 3 in general/gamelogic/anim/mesh/sound/surface/highres/local and **0** in every cell and map archive | Observed in user install 2026-10-03; NOTES dat-format §2 said "observed as 3" | Confirmed (corrects NOTES: cell/map are 0) |
| P6 | The 16-byte value at 0x17C and the u32 at 0x18C did **not** change between version A and version B. Two aux archives have zeros there | Observed in user install 2026-10-03 (diff of A and B) | Confirmed |
| P7 | Housekeeping subfiles with DIDs 0xFFFF0001, 0xFFFF0003, 0xFFFF0004, 0xFFFF0005 and 0xFFFF0008 exist, with the layouts in §9.2 | Observed in user install 2026-10-03; AC-DRW (0xFFFF0001 = "iteration" record in AC) | Confirmed (layouts), Reported (AC analogy) |
| P8 | 0xFFFF0003 carries a pair of 16-byte stamps that is identical in every archive that has the record, and that changed between A and B | Observed in user install 2026-10-03 | Confirmed |
| P9 | 0xFFFF0005 carries a 16-byte stamp shared by a base archive and its aux archives, plus the member index and the member count | Observed in user install 2026-10-03 | Confirmed |
| P10 | The 64-bit client is `x64/lotroclient64.exe`. A 32-bit `lotroclient.exe` and the launcher `LotroLauncher.exe` sit in the install root | Observed in user install 2026-10-03; OL; NOTES engine-client-re §1 | Confirmed |
| P11 | The PE version resource gives a four-part file version and a product-version string ending in a channel word | Observed in user install 2026-10-03 | Confirmed |
| P12 | `ProjectVersion` is a 3-byte ASCII file whose content is `120` in both A and B, with no line terminator | Observed in user install 2026-10-03 | Confirmed |
| P13 | `lotro.launcherconfig` is a .NET appSettings XML file. Element and key names are in §6 | Observed in user install 2026-10-03; OL; NOTES network §1.1 | Confirmed |
| P14 | Launcher-config lookup order: `<gametype>.launcherconfig`, then any `*.launcherconfig`, then `TurbineLauncher.exe.config`, then (DDO) `DNDLauncher.exe.config` | OL; NOTES network §1.1 | Reported |
| P15 | Steam app id 212500. `installdir` is `Lord of the Rings Online`. Depots and acf keys are in §5.3 | Observed in user install 2026-10-03 (`appmanifest_212500.acf`); EL; NOTES analogues | Confirmed |
| P16 | `libraryfolders.vdf` structure, its locations under `~/.local/share/Steam` (`steamapps/` and `config/`, identical copies), and `~/.steam/steam` being a symlink to it | Observed on user machine 2026-10-03; OS | Confirmed |
| P17 | The Proton prefix lives in `<library>/steamapps/compatdata/212500/pfx`, in the library **that holds the game** (not necessarily the default library) | Observed on user machine 2026-10-03 | Confirmed |
| P18 | The settings folder inside a Proton prefix is `pfx/drive_c/users/steamuser/Documents/The Lord of the Rings Online/` | Observed on user machine 2026-10-03; OL | Confirmed |
| P19 | A settings folder can also appear directly as `$HOME/The Lord of the Rings Online/` when a Wine prefix maps Documents to the home directory | Observed on user machine 2026-10-03 (folder present); mapping behaviour is Wine's (from memory) | Confirmed (folder), Reported (cause) |
| P20 | `UserPreferences.ini` is UTF-8 with BOM, CRLF line ends, INI sections. Key names are in §8.3 | Observed on user machine 2026-10-03 (two files); OL; NOTES engine-client-re §2 | Confirmed |
| P21 | Keys may appear lower-cased (e.g. `graphicscore`), because third-party launchers rewrite them | Observed on user machine 2026-10-03; OL writes through a case-folding INI writer | Confirmed (observation), Reported (cause) |
| P22 | `PluginData/<account>/<world>/PluginOptions.xml` exists | Observed on user machine 2026-10-03 | Confirmed |
| P23 | `Plugins/`, `PluginData/`, `Music/` (`.abc`), `ui/skins/<Skin>/SkinDefinition.xml`, `lotro.keymap`, screenshots, chat logs | NOTES engine-client-re §2, §6, §7; wiki pages via Wayback | Reported (Plugins/, PluginData/, lotro.keymap are Confirmed) |
| P24 | Windows registry: `…\Uninstall\Steam App 212500` → `InstallLocation` | EL | Reported |
| P25 | Windows registry: `SOFTWARE\[WOW6432Node\]StandingStoneGames\The Lord of the Rings Online` and `SOFTWARE\WOW6432Node\Turbine\The Lord of the Rings Online` | KD | Reported |
| P26 | The install carries an Inno Setup uninstall log (`unins000.dat`) whose AppId is `12bbe590-c890-11d9-9669-0800200c9a66`. Inno Setup registers uninstall keys as `<AppId>_is1` | Observed in user install 2026-10-03 (AppId); Inno Setup convention (from memory) | Confirmed (AppId), Reported (key name) |
| P27 | Steam registry: `HKCU\Software\Valve\Steam` `SteamPath`; `HKLM\SOFTWARE\WOW6432Node\Valve\Steam` `InstallPath`; `HKCU\Software\Valve\Steam\Apps\212500` with `Installed`/`Running`/`Updating` DWORDs | Observed 2026-10-03 in the Proton prefix registry files; OS (InstallPath) | Confirmed |
| P28 | Standalone default install path `C:\Program Files (x86)\StandingStoneGames\The Lord of the Rings Online` | KD | Reported |
| P29 | OneLauncher search roots, prefix layouts, its built-in prefix path and its per-game config file (§5.6) | OL | Reported |
| P30 | Client command-line options (§10) | OL (help text the client prints with no arguments, captured 2024-07-10); NOTES network §1.7 | Reported |
| P31 | Launch-argument templates put option names in argv that the help text does not list | NOTES network §1.4; OL | Reported |
| P32 | `--prefs <file>` also moves the settings folder to the parent of `<file>`. `--cwd` does not work in practice | OL | Reported |
| P33 | Language archive names: `English`, `DE`, `FR` | OL; `English` observed in user install 2026-10-03 | Confirmed (English), Reported (DE, FR) |
| P34 | Iteration record decoding (§9.3): runs and singles; the run and single counts sum to the record's count; the trailing list consumes the rest exactly | Observed in user install 2026-10-03 (all 29 archives of A and B parse with zero bytes left over); AC-DRW (base layout) | Confirmed |
| P35 | Highest directory-entry iteration = highest iteration in the 0xFFFF0001 record − 1 | Observed in user install 2026-10-03, for `client_general.dat` and `client_map_5.dat` in both A and B | Confirmed (two archives only) |

---

## 2. Terms

| Term | Meaning |
|---|---|
| **Install directory** (`GAME_DIR`) | The directory that directly holds `client_general.dat`. |
| **Archive key** | The `<key>` part of `client_<key>.dat`, e.g. `general`, `cell_3`, `local_English`. |
| **Split set** | A base archive `client_<key>.dat` plus its aux archives `client_<key>_aux_1.datx … _aux_<n>.datx`, read together as one logical archive. |
| **Region** | The numeric suffix of `cell_<r>` and `map_<r>`. |
| **Settings folder** (`PREFS_DIR`) | The per-user folder in the user's Documents that holds `UserPreferences.ini`, plugins and so on. |
| **Prefix** | A Wine/Proton directory tree that contains `drive_c/`. Proton nests it one level deeper, under `pfx/`. |
| **Housekeeping record** | A subfile whose DID is in 0xFFFF0000–0xFFFFFFFF. |

All file and directory name comparisons in this spec are **ASCII case-insensitive**. The game is a Windows program and its data may be copied onto a case-sensitive file system with any case. On a case-sensitive file system, resolve each path component by listing its parent directory and matching case-insensitively. If more than one entry matches, take the exact-case one if present; otherwise take the lexicographically smallest and log a warning.

---

## 3. Install directory contents

### 3.1 Archive name grammar

| Pattern | Kind | Notes |
|---|---|---|
| `client_<word>.dat` | base archive | `<word>` is lower-case letters (general, gamelogic, anim, mesh, sound, surface, highres) |
| `client_local_<Lang>.dat` | language archive | `<Lang>` keeps the case it is written in (see §3.5) |
| `client_cell_<r>.dat`, `client_map_<r>.dat` | region archives | `<r>` is decimal with no leading zeros |
| `client_<word>_aux_<n>.datx` | aux member of a split set | `<n>` is decimal ≥ 1. Note the **`.datx`** extension |

Anything else that starts with `client_` is ignored with a debug log.

> **OPEN:** Whether region or language archives can ever have aux members. None were observed.

### 3.2 Observed inventory (version B)

All values are Confirmed (observed in user install 2026-10-03). "0x14C" and "0x150" are the u32 little-endian header words at those offsets (§9.1). "Set" is the split-set member index and count from record 0xFFFF0005 (§9.2.4).

| File | Size (bytes) | Block size (0x144) | 0x14C | 0x150 | 0x178 | Set |
|---|---|---|---|---|---|---|
| client_general.dat | 781,189,120 | 1024 | 0x01030000 | 0 | 3 | 0 of 1 |
| client_gamelogic.dat | 1,052,770,304 | 256 | 0x01010000 | 0 | 3 | 0 of 1 |
| client_anim.dat | 738,197,504 | 24000 | 0x01040000 | 0 | 3 | 0 of 1 |
| client_mesh.dat | 4,286,578,688 | 17408 | 0x01020000 | 0 | 3 | 0 of 2 |
| client_mesh_aux_1.datx | 1,672,478,720 | 17408 | 0x01020000 | 0 | 3 | 1 of 2 |
| client_sound.dat | 4,287,627,264 | 17408 | 0x01070000 | 0 | 3 | 0 of 2 |
| client_sound_aux_1.datx | 3,902,799,872 | 17408 | 0x01070000 | 0 | 3 | 1 of 2 |
| client_surface.dat | 4,287,627,264 | 4096 | 0x01090000 | 0 | 3 | 0 of 3 |
| client_surface_aux_1.datx | 4,287,627,264 | 4096 | 0x01090000 | 0 | 3 | 1 of 3 |
| client_surface_aux_2.datx | 2,640,314,368 | 4096 | 0x01090000 | 0 | 3 | 2 of 3 |
| client_highres.dat | 4,287,627,264 | 40960 | 0x01080000 | 0 | 3 | 0 of 4 |
| client_highres_aux_1.datx | 4,287,627,264 | 40960 | 0x01080000 | 0 | 3 | 1 of 4 |
| client_highres_aux_2.datx | 4,286,578,688 | 40960 | 0x01080000 | 0 | 3 | 2 of 4 |
| client_highres_aux_3.datx | 566,231,040 | 40960 | 0x01080000 | 0 | 3 | 3 of 4 |
| client_local_English.dat | 1,906,171,856 | 256 | 0x03030000 | 2 | 3 | 0 of 1 |
| client_cell_1.dat … _5.dat | 416–641 MB | 256 | 0x02030000 | 1…5 | 0 | 0 of 1 |
| client_cell_14.dat | 65,011,712 | 256 | 0x02030000 | 14 (0x0E) | 0 | 0 of 1 |
| client_cell_15.dat | 1,048,576 | 256 | 0x02030000 | 15 (0x0F) | 0 | 0 of 1 |
| client_map_1.dat … _5.dat | 22–23 MB | 256 | 0x020A0000 | 1…5 | 0 | 0 of 1 |
| client_map_14.dat | 23,068,672 | 256 | 0x020A0000 | 14 | 0 | 0 of 1 |
| client_map_15.dat | 1,048,576 | 256 | 0x020A0000 | 15 | 0 | 0 of 1 |

Observations (Confirmed):

- The most significant byte of 0x14C is a family code: **1** = shared game data, **2** = region (cell/map) data, **3** = language data. The next byte distinguishes archives within the family (general 3, gamelogic 1, mesh 2, anim 4, sound 7, highres 8, surface 9; cell 3, map 0x0A; local 3). The low 16 bits are zero.
- Every aux member has the same 0x14C, 0x150 and block size as its base.
- For `local_English`, 0x150 = 2. That matches the scripting API value for the English language (NOTES engine-client-re §5; Reported).
- Version A had the same set of files, with different sizes for some of them.

> **OPEN:** The meaning of the second byte of 0x14C, and the language codes in 0x150 for the DE and FR language archives (expected to follow the scripting API's language enumeration; not observed).
> **OPEN:** What regions 1–5, 14 and 15 stand for. Region 15 archives are 1 MiB placeholders in version B.

### 3.3 Required and optional archives

This classification is a **project decision**, informed by the facts above. LC treats `anim` as optional and `highres` as optional (Reported).

| Tier | Archives | If missing |
|---|---|---|
| **Core** | `general`, `gamelogic`, at least one `local_<Lang>` | The directory is **not** a usable install (§4) |
| **Content** | `mesh`, `surface`, `sound` (each with all its aux members), `anim`, every `cell_<r>` with its matching `map_<r>` | Usable, but flagged *incomplete*. The client warns, and subfile lookups that would land in the missing archive fail with a "missing archive" error, not a crash |
| **Optional** | `highres` with its aux members | Usable. High-resolution lookups fall back as if `--HighResOutOfDate` were given (§10) |

Region archives come in pairs: if `cell_<r>` exists, `map_<r>` should exist too (Confirmed for every observed region). An unpaired region archive is recorded but flagged.

### 3.4 Discovering split-set members

Do not hard-code aux counts. They differ between versions and grow as content is added. For each base key in the shared-data family:

1. Probe `client_<key>_aux_1.datx`, `_aux_2.datx`, … in order. Stop at the first index that does not exist, or at index 16, whichever comes first.
2. Read record 0xFFFF0005 from the base and from each member found (§9.2.4). The stamps must all be equal. Each member's index must equal its `<n>` (the base is index 0). Every member must report the same set count.
3. If the set count is larger than the number of files found, the set is **incomplete**. Record the missing indices.
4. If an aux file exists whose index is ≥ the set count, or whose stamp differs, it is a **stale leftover**. Exclude it from the set and log a warning. Patches can leave such files behind; see `backup/` in §3.6.
5. If 0xFFFF0005 cannot be read (damaged archive), fall back to "every consecutive aux file found is a member" and mark the set *unverified*.

Invariant (Confirmed in A and B): in a healthy install, step 2 holds for all four split sets.

### 3.5 Language archives

- The language name is whatever follows `client_local_` and comes before `.dat`. Observed: `English` (Confirmed). Reported by OL: `DE` and `FR`. Keep the name exactly as written, because it is also the value passed with `--language` (§10).
- An install may hold several languages. The default is the one named by `--language`. Otherwise use `English` if it is present. Otherwise use the first in lexicographic order.

### 3.6 Other files in the install directory

All Confirmed (file listing of user install 2026-10-03) unless marked otherwise. Our client reads none of these at runtime except where noted. They are listed so that validation and diagnostics can recognise them.

| Path (relative to `GAME_DIR`) | What it is |
|---|---|
| `x64/` | The 64-bit client: `lotroclient64.exe`, `lotrobrowserhelper64.exe`, and 64-bit copies of the middleware DLLs. The 64-bit client binary lives **only** here |
| `lotroclient.exe` | 32-bit client (legacy; its servers were shut down in Aug 2025 per NOTES; Reported) |
| `LotroLauncher.exe` | Official launcher (32-bit) |
| `lotro.launcherconfig` | Local launcher bootstrap config (§6) |
| `ProjectVersion` | 3-byte ASCII number (§7) |
| `patchcache.bin` | Patcher state. Contains the absolute install path as UTF-16. Never parse or write it |
| `unins000.dat`, `unins000.exe`, `uninstall_manifest.txt` | Inno Setup uninstaller files and a list of the folder/file patterns the uninstaller removes |
| `raw/<lang>/…` | Loose media outside the archives: `chargen/{class_videos,race_videos}`, `introcinematic/*.bik`, `logo/*.jpg`. `<lang>` is lower-case `en`, `de`, `fr` |
| `en/`, `de/`, `fr/`, `themes/`, `locales/`, `*.png`, Qt and CEF files | Launcher UI and embedded browser resources |
| `backup/` | Previous copies of binaries saved by the patcher. **Never** treat it as an install. Skip it while searching (§5.8) |
| `bindat/`, `forward/`, `tmp/`, `downloading/` | Patcher working directories (empty or near-empty when idle) |
| `troubleshooting/`, `_CommonRedist/`, `3rdparty/`, `DXSETUP.exe`, `*.cab` | Redistributable installers |
| `icon.ico` | Product icon (OL uses it in its UI; Reported) |

> **OPEN:** Whether the client reads anything from `raw/` beyond videos and splash images, and the exact role of `bindat/` and `forward/`.

### 3.7 Game-type identification

DDO uses the same engine and the same archive naming, so `client_general.dat` alone does not prove the directory is a LOTRO install (Reported: OL, PL). Decide the game type in this order:

1. The launcher config's file stem (`lotro.launcherconfig` → LOTRO; `ddo.launcherconfig` → DDO).
2. Otherwise, the `DataCenter.GameName` value in whichever launcher config was found (§6): `LOTRO` (historically also `LOTROEU`) or `DDO`.
3. Otherwise, the client binary present: `x64/lotroclient64.exe` or `lotroclient.exe` → LOTRO; a `dndclient*.exe` → DDO (PL; Reported).
4. Otherwise *unknown*. Our client accepts *unknown* with a warning.

Our client targets LOTRO. A DDO install is reported as "found, unsupported" and is not opened.

---

## 4. Validating a candidate directory

A directory `D` is a **usable install** when all of these hold:

1. `D` is a directory and can be listed.
2. `D` contains `client_general.dat`, `client_gamelogic.dat` and at least one `client_local_*.dat` (case-insensitive), each a regular file (after following symlinks) and each at least 1024 bytes long.
3. For each of those core archives, the 4 bytes at offset 0x140 read as the u32 little-endian value 0x00005442, and the u32 at 0x148 equals the real file length.
4. The game type (§3.7) is LOTRO or *unknown*.

Optional checks, run when building the full report:

- every archive found passes check 3;
- split sets pass §3.4;
- region archives are paired (§3.3);
- `x64/lotroclient64.exe` exists. Its absence is only a warning: our client does not need the original binary.

Validation reads at most the first 0x400 bytes of each archive, plus the housekeeping records for the full report. It never needs to open more than one archive at a time.

The validation result is a structured report. A lone boolean is not enough. The report holds: core OK or not; the game type; per archive its path, size, header fields (§9.1) and status (ok, bad magic, size mismatch, unreadable); per split set its status (complete, incomplete with missing indices, unverified, with any stale members); languages found; and warnings.

---

## 5. Locating installs

### 5.1 Search order

Candidates are gathered from every source below in the order given, de-duplicated (§5.8), and validated (§4). The **first usable install in this order is the default**. All usable installs are returned so a UI can offer a choice.

1. **Explicit**: a `--game-dir` option of our own client or tools, if given. Nothing else is searched.
2. **Environment**: `TESSERA_GAME_DIR`, if set and non-empty. Nothing else is searched.
3. **Steam** (§5.3 Linux/macOS, §5.4 Windows).
4. **OneLauncher configs** (§5.6.2).
5. **Standalone installer** paths and registry (§5.5).
6. **Wine/Proton/Lutris/CrossOver prefixes** (§5.6).
7. **Bounded scan** of the user's home games folders (§5.6.4).

If 1 or 2 names a directory that is not a usable install, report that as an error. Do not silently fall back to searching: the user asked for that directory.

### 5.2 Paths inside a prefix

When a source yields a Windows path (registry value, `--prefs` argument, OneLauncher setting) and we run on Linux or macOS, translate it as follows:

- `C:\…` maps to `<prefix>/drive_c/…`.
- `Z:\…` maps to `/…` (Wine's default mapping of `Z:` to the host root; Reported, from memory).
- Other drive letters are resolved through `<prefix>/dosdevices/<letter>:` symlinks if present. Otherwise they are unresolvable, which is logged and skipped.
- Backslashes become `/`, and each component is matched case-insensitively (§2).

### 5.3 Steam on Linux and macOS

**Steam roots** (try each; a root counts only if it exists):

| Root | Mark |
|---|---|
| `~/.local/share/Steam` | Confirmed (user machine) |
| `~/.steam/steam` (normally a symlink to the above) | Confirmed (symlink observed) |
| `~/.steam/root`, `~/.steam` | Reported (OL searches `~/.steam/steamapps`) |
| `~/.var/app/com.valvesoftware.Steam/.local/share/Steam` (Flatpak) | Reported (packaging convention, from memory) |
| `~/snap/steam/common/.local/share/Steam` (Snap) | Reported (from memory) |
| `~/Library/Application Support/Steam` (macOS) | Reported (OS) |

> **OPEN:** Confirm the Flatpak and Snap roots on a machine that uses them.

**Library list.** For each root, read `<root>/steamapps/libraryfolders.vdf`. If that file is absent, read `<root>/config/libraryfolders.vdf`. On the user machine both exist and are byte-identical (Confirmed). The root itself is always a library, even if the file is missing.

**libraryfolders.vdf structure** (Confirmed; key names are needed for interoperability):

- Top-level key `libraryfolders`, whose value is a block.
- Inside it, numbered keys `"0"`, `"1"`, …, each a block with these entries:
  - `path`: the library root, a native path;
  - `label`, `contentid`, `totalsize`, `update_clean_bytes_tally`, `time_last_update_verified`;
  - `apps`: a block mapping an app-id string to a size string.
- Presence of `"212500"` in `apps` means the library *should* hold the game. The size may be `"0"` even when the game is installed (Confirmed). **Use presence only, never the size.** Even when 212500 is not listed, still probe each library for the manifest (step below). The list can be stale.

> **OPEN:** The pre-2021 format (top-level key `LibraryFolders`, with numbered keys whose values are plain path strings; Reported, from memory). Parse it too: when a numbered value is a string, not a block, treat the string as `path`.

**App manifest.** In each library, look for `<library>/steamapps/appmanifest_212500.acf` (Confirmed). Its top-level key is `AppState`. Keys used:

| Key | Use | Observed value |
|---|---|---|
| `appid` | must equal `212500` | `212500` |
| `installdir` | folder name under `steamapps/common/` | `Lord of the Rings Online` (**no leading "The"**) |
| `StateFlags` | bitmask; value 4 = fully installed (Steam convention; Reported) | `4` |
| `buildid` | Steam build number, a coarse version label (§9.5) | an integer |
| `InstalledDepots` | block of depot-id → {`manifest`, `size`} | 212501 (~35.2 GB), 212502 (~3.5 GB), plus redistributable depots 228983, 228989, 228990 |
| `UserConfig` → `language` | Steam language choice | `english` |

The game directory is `<library>/steamapps/common/<installdir>`. If the manifest is missing but `apps` lists 212500, also try `<library>/steamapps/common/Lord of the Rings Online`.

> **OPEN:** Which content depot 212502 holds (possibly the high-resolution textures). Do not rely on depot ids.

**VDF text syntax** (the subset these files use; Confirmed by observation, OS for the general shape):

- Tokens are double-quoted strings, `{` and `}`.
- A key token is followed either by a quoted value or by a `{ … }` block.
- Whitespace (spaces, tabs, CR, LF) separates tokens.
- A backslash inside a quoted string escapes the next character. `\\` is a single backslash, as seen in Windows paths in `InstallScripts`.
- `//` starts a comment that runs to the end of the line (Reported, from memory).
- Keys are compared case-insensitively.

The parser must reject unbalanced braces and unterminated strings with an error, never a panic. It must limit nesting depth (16 is plenty) and file size (1 MiB is plenty).

**Proton prefix of a Steam install** (Confirmed): `<library>/steamapps/compatdata/212500/pfx/`, where `<library>` is the library holding the game. The file `<library>/steamapps/compatdata/212500/version` holds the Proton tool name as text, e.g. a GE-Proton release. The prefix user is `steamuser`, so the settings folder is under `pfx/drive_c/users/steamuser/Documents/` (§8.1).

### 5.4 Steam on Windows

1. Find the Steam root from registry value `SteamPath` under `HKCU\Software\Valve\Steam`. Otherwise use `InstallPath` under `HKLM\SOFTWARE\WOW6432Node\Valve\Steam`, then the same key without `WOW6432Node` (Confirmed in prefix registry; OS).
2. From there, follow §5.3 (library list → manifest → `steamapps\common\<installdir>`).
3. In addition, read `InstallLocation` from `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Steam App 212500`, and from the same path under `SOFTWARE\WOW6432Node\…` (EL; Reported).
4. Fallback: `C:\Program Files (x86)\Steam\steamapps\common\Lord of the Rings Online` (path shape Confirmed via `installdir`).

`HKCU\Software\Valve\Steam\Apps\212500` holds DWORDs `Installed`, `Running` and `Updating` (Confirmed in the prefix registry). If `Updating` = 1, warn that Steam is patching the game at this moment.

### 5.5 Standalone installer (Windows, or inside a prefix)

| Source | Detail | Mark |
|---|---|---|
| Default path | `C:\Program Files (x86)\StandingStoneGames\The Lord of the Rings Online` | Reported (KD) |
| Older default path | `C:\Program Files (x86)\Turbine\The Lord of the Rings Online` | Reported (inferred from the KD registry key name) |
| Registry | `HKLM\SOFTWARE\StandingStoneGames\The Lord of the Rings Online`, `HKLM\SOFTWARE\WOW6432Node\StandingStoneGames\The Lord of the Rings Online`, `HKLM\SOFTWARE\WOW6432Node\Turbine\The Lord of the Rings Online`; try value names `InstallLocation`, `GameFolder`, `Install Location` | Reported (KD) |
| Inno Setup uninstall key | `HKLM\SOFTWARE\[WOW6432Node\]Microsoft\Windows\CurrentVersion\Uninstall\<AppId>_is1`, with value `InstallLocation`. The AppId in the uninstall log is `12bbe590-c890-11d9-9669-0800200c9a66` | Confirmed (AppId), Reported (key form) |
| Program Files scan | Walk `C:\Program Files` and `C:\Program Files (x86)` under the §5.8 limits | Reported (OL, PL) |

> **OPEN:** Whether the Inno key name includes braces around the AppId, and which of the KD value names really exist. Look these up with a case-insensitive match over the key's values, and accept any of them.
> **OPEN:** The historical Codemasters (EU) default install path.

### 5.6 Wine, Proton, OneLauncher, Lutris, CrossOver

#### 5.6.1 Prefix discovery

A directory `X` is a prefix if `X/drive_c` exists. It is a Proton-style wrapper if `X/pfx/drive_c` exists; then the prefix is `X/pfx` (OL; Confirmed for Steam Proton). For each prefix found, search `drive_c/Program Files` and `drive_c/Program Files (x86)` under the §5.8 limits.

Where to look for prefixes (Reported, OL, unless marked):

| Location | Glob |
|---|---|
| Home | `~/*wine*/` (catches `~/.wine`, `~/.wine32`, …) and `~/.wine` itself |
| Steam compatdata, **in every library** | `<library>/steamapps/compatdata/*/` (non-Steam games added to Steam get their own app ids). Confirmed for 212500 |
| Games folder | `~/games/*/` and `~/Games/*/`. Lutris places prefixes in `~/Games/<slug>/` by default (Reported, from memory) |
| CrossOver (macOS) | `~/Library/Application Support/CrossOver/Bottles/*/` |
| `WINEPREFIX` | if set in the environment |
| OneLauncher built-in prefix | `<XDG cache>/onelauncher/wine/prefix` (normally `~/.cache/onelauncher/wine/prefix`) |
| OneLauncher user prefix | the `wine.user_prefix_path` setting in each OneLauncher game config (§5.6.2) |

OL only scans the compatdata folder of the default Steam root. That misses prefixes in other libraries, as on the user machine (Confirmed). Our search covers every library.

#### 5.6.2 OneLauncher configs (Reported, OL)

OneLauncher keeps one TOML file per configured game at `<XDG data>/onelauncher/games/<game-id>/config.toml` (normally `~/.local/share/onelauncher/games/*/config.toml`). Top-level keys of interest:

- `game_directory`: the install directory;
- `game_settings_directory`: a custom settings folder, optional;
- `game_type`: `LOTRO` or `DDO`;
- `client_type`: `WIN64`, `WIN32` or `WIN32Legacy`;
- `high_res_enabled`;
- the table `[wine]` with `user_prefix_path`.

Use `game_directory` as a candidate, and `game_settings_directory` as the settings folder for that candidate. Parse only these keys and ignore the rest. Never write the file.

#### 5.6.3 Lutris

Lutris game entries name a prefix and an executable path.

> **OPEN:** Location and keys of Lutris game YAML files. Until this is answered, Lutris installs are found through the `~/Games/*/` prefix scan.

#### 5.6.4 Bounded home scan

As a last resort, scan `~/games`, `~/Games` and each Steam `steamapps/common` folder for usable installs, to a depth of 3 (OL).

### 5.7 Security and privacy of discovery

- Discovery reads only metadata files, directory listings and the first 0x400 bytes of archives. It must not send anything over a network.
- Registry and file reads that fail (no access, missing) are skipped silently at debug log level.

### 5.8 Search limits and de-duplication

- Maximum recursion depth is 5 below a search root (OL), and 3 for the §5.6.4 scan.
- Skip directories whose name starts with `.` (except an explicitly named prefix root), and directories named `dosdevices`, `backup` (case-insensitive) or `compatdata` when walking inside `drive_c`. `dosdevices` contains drive symlinks that would loop back to `/`.
- Do not follow directory symlinks while recursing. Do follow them when they are an explicit candidate, e.g. `~/.steam/steam`.
- Cap total directories visited per search at 20,000, and log when the cap is hit.
- Two candidates are the same install if their canonical (symlink-resolved) paths are equal. Keep the first one found and record every source that named it.

---

## 6. `lotro.launcherconfig`

Format (Confirmed): UTF-8 **with BOM**, CRLF line ends, XML declaration, root element `configuration`. The element names present are:

| Element | Attributes / content |
|---|---|
| `configuration` | root |
| `system.windows.forms` | attribute `jitDebugging` |
| `startup` → `requiredRuntime`, `supportedRuntime` | attributes `version` (and `safemode` on `requiredRuntime`) |
| `appSettings` → `add` ×5 | attributes `key`, `value` |
| `system.net` → `settings` → `servicePointManager` | attribute `expect100Continue` |

The `appSettings` keys observed (Confirmed). Values are shown only where they are not network addresses:

| Key | Meaning | Value seen |
|---|---|---|
| `Launcher.DataCenterService.GLS` | URL of the login data-center web service | (a URL; not recorded) |
| `DataCenter.GameName` | game identifier passed to that service, and the game type (§3.7) | `LOTRO` |
| `DataCenter.DefaultCenter` | default data-center name | `LOTRO` |
| `Product.DocumentFolder` | name of the settings folder under Documents (§8.1) | `The Lord of the Rings Online` |
| `Launcher.DocumentFolder` | launcher's own documents sub-folder name | `Launcher` |

Rules for our parser:

- Use an XML parser that rejects DTDs and external entities.
- Accept a BOM or no BOM. Cap the file size at 64 KiB.
- Read `add` elements only under `configuration/appSettings`, and match keys case-sensitively, then case-insensitively as a fallback.
- Duplicate keys: the last one wins.
- Never write the file.

Lookup order and legacy names are in P14. `Launcher.DataCenterService.GLS`, `DataCenter.GameName` and `Product.DocumentFolder` are required by OL. If `Product.DocumentFolder` is missing, our client uses `The Lord of the Rings Online` as the default (the Confirmed value).

> **OPEN:** Where `Launcher.DocumentFolder` is resolved relative to (presumably the settings folder).

---

## 7. `ProjectVersion`

Confirmed: a file of exactly 3 bytes, ASCII digits `120`, with no line terminator. It is identical in versions A and B, so it does **not** identify a content version.

Our parser reads at most 64 bytes, trims ASCII whitespace, and accepts 1–10 ASCII digits as an unsigned integer. Anything else is reported as *malformed*, which is not an error. The value is recorded in the fingerprint report (§9.4) as metadata only.

> **OPEN:** What `ProjectVersion` counts.

---

## 8. Per-user settings folder

### 8.1 Location

Let `NAME` be the launcher config's `Product.DocumentFolder` (default `The Lord of the Rings Online`). Resolve the settings folder in this order:

1. If our client is started with `--prefs <file>`, the folder is the parent directory of `<file>` (P32; Reported). The preferences file is `<file>` itself, which need not be called `UserPreferences.ini`.
2. A per-install override from the source that found the install (OneLauncher `game_settings_directory`).
3. Native Windows: `<Documents>/NAME`, where `<Documents>` is the user's known Documents folder. It may be redirected (OneDrive, roaming profiles), so use the shell's known-folder lookup, not `%USERPROFILE%\Documents`.
4. Install inside a prefix (Steam Proton, Wine, Lutris): `<prefix>/drive_c/users/<user>/Documents/NAME`. For Proton `<user>` is `steamuser` (Confirmed). For other prefixes, try the host user name, then every directory under `drive_c/users/` except `Public`. Wine may map `Documents` to a symlink into the host home (P19). Follow it.
5. Native Linux run of our client against a Steam/Proton install: use the prefix folder from step 4, so that plugins and preferences are shared with the official client.
6. Otherwise: `$XDG_DOCUMENTS_DIR/NAME` if set, else `~/Documents/NAME`, else `~/NAME` (P19).

When several candidates exist, prefer the one that already contains a `UserPreferences.ini`.

### 8.2 Contents

| Path (relative to settings folder) | Content | Mark |
|---|---|---|
| `UserPreferences.ini` | Client and launcher preferences (§8.3). The file name is fixed when `--prefs` is not given (OL) | Confirmed |
| `lotro.keymap` | Key bindings. Plain text starting with `#` comment lines. Base name for `--keymap` meta-key variants (§10) | Confirmed (presence, comment syntax) |
| `Plugins/` | Lua plugins: `Plugins/<Author>/<Name>.plugin` descriptor files plus package folders; also `.plugincompendium`, `.skincompendium`, `.musiccompendium` manager files | Confirmed (folder), Reported (layout) |
| `PluginData/<account>/<world>/…` | Saved plugin data. Observed: `PluginOptions.xml` at the world level | Confirmed |
| `PluginData/…/*.plugindata` | Data saved by plugins per scope (account, server, character) | Reported |
| `Music/*.abc` | Player-music files for `/play` | Reported |
| `ui/skins/<Skin>/SkinDefinition.xml` + TGA images | Custom UI skins, read at client start | Reported |
| screenshots, chat logs | Written by the client | Reported |

> **OPEN:** The exact folder for each `PluginData` scope (account-wide vs. per-world vs. per-character), and the screenshot and chat-log folder names.

### 8.3 `UserPreferences.ini`

**Format** (Confirmed on two files): UTF-8 with BOM, CRLF line ends, `[Section]` headers, `Key=Value` lines with no spaces around `=`. Values are untyped text. Observed value shapes: `True`/`False`; decimals with two places (`0.30`, `1.00`); integers; `WxH` resolutions (`1024x768`); enumeration words (`FullScreen`, `D3D11`).

**Reading rules:**

- Section and key names are compared **case-insensitively**, because lower-cased keys occur in real files (P21).
- If a key repeats within a section, the last one wins.
- Lines without `=` outside a section header are ignored.
- Cap the file at 1 MiB.

**Writing rules** (when our client saves preferences):

- Preserve every section and key it does not own, in the original order, with the original case.
- Write UTF-8 with BOM and CRLF.
- Never touch `[Launcher]` or `[User_N]`.

> **OPEN:** Whether our client shares this file with the official client, or keeps its own file and only imports from it. This is a project decision; until it is made, our client treats the official file as **read-only** and writes its own.

**Sections and keys observed** (Confirmed; key names only):

| Section | Keys |
|---|---|
| `[Launcher]` (written by the official launcher) | AllowMultipleInstances, FirstRun, UseHighResolutionGameData, WindowGeometry, PreCache, SelectLastPlayedWorld |
| `[User_1]` (launcher; numbered) | UserName |
| `[Render]` | D3DVersionPromptedForAtStartup, GraphicsCore, AllowGammaChanges, AlphaToCoverage, AmbientLightBoost, AmbientOcclusion, AnisotropicQuality, AspectRatio, AtmosphericsDetail, BlobShadows, BloomIntensity, BlurFilterQuality, Brightness, Contrast, DisplayAdapter, DisplayOutput, DistantImposters, DynamicParticles, EnablePortraits, EnvironmentStencilShadows, FarLandscapeNormalMaps, FrillDensity, FrillDistance, FrillTerrainColor, Gamma, GlowMapping, HavePromptedForD3D10AtStartup, InteractiveWater, LandscapeDrawDistance, LandscapeLightingQuality, LandscapeShoreEffects, LandscapeStaticObjectShadows, MaterialDetail, MaxHardwareClass, MeasuredFillRate, MemoryUsage, MeshCombining, ModelDetail, MultiPassLighting, ObjectDrawDistance, OverbrightBloomFilter, PerPixelLightingAttenuation, PlayerCrowdQuality, PrecipitationEffects, ShadowMapQuality, SpecularLighting, StaticEnvironmentalObjects, StencilShadows, SurfaceReflections, TextureDetail, TextureFiltering, VideoPostEffects, VolumetricLight |
| `[Camera]` | AlignToSlope, ResetYawWhenPlayerMoves |
| `[Display]` | AllowDesktopCompositing, AllowFakeFullScreen, AllowWindowResizing, Antialiasing, ConfineFullScreenMouseCursor, FixedOutputScaling, ForceFakeFullScreen, FullScreen, RefreshRate, Resolution, ScreenMode, SyncToRefresh, TripleBuffering, WindowedResolution |
| `[Graphics]` | AvatarTextureCompositing, AvatarUpdateVisible, PVMPPerformanceOverride |
| `[Input]` | InvertMouseLookYAxis, JoystickDeadZone, MouseLookSensitivity, MouseLookSmoothingAmount, PitchKeySpeed, RollKeySpeed, TurnKeySpeed, XInputDetection |
| `[International]` | UseIME |
| `[Misc]` | TooltipDelay, TooltipEnable, PlayedIntroMovie |
| `[Net]` | BindInterface, ComputeUniquePort, ConnectionSpeed, UserSpecifiedPort |
| `[Sound]` | AmbientVolume, CombatVolume, DefaultVolume, EAXEnabled, FocusBehavior, MasterVolume, MaxPlayingSounds, MusicDisabled, MusicVolume, PersistentSound, PlayerMusicVolume, QuantizedPlayerMusicVolume, SFXVolume, SoundDisabled, SoundFeatures, SoundProvider, UIVolume, VOVolume |
| `[Troubleshooting]` | EngineSpeed, MaximumFrameRate |
| `[UI]` | CharSelectSortType, FloatyTextOnTop, FloatyTextOpacity, FloatyTextScale, HotspotVisualizationColorOverride_Boon, HotspotVisualizationColorOverride_Harm, RadarMovementSmoothing, WebstoreExternalWindow, UIScale_Cursor, UIScale_Global, UIScale_Pregame |
| `[Voice]` | DefaultGroupVolume, DeviceCapture, DevicePlayback, HandsFree, MicCaptureThreshold, MicVolume, NetworkLatencyThresh, VoiceVolume |

Known values (Reported unless marked):

- `[Render] GraphicsCore` is one of `D3D9`, `D3D10`, `D3D11`, `OpenGL` (`D3D11` Confirmed).
- `[Display] ScreenMode` includes `FullScreen` (Confirmed) and `FullScreenWindowed`.
- `[Render] D3DVersionPromptedForAtStartup` = `11` is written by OL.
- The launcher reads the client type (`Win64` / `Win32` / `Win32Legacy`) from this file (NOTES; DDO-ML).

> **OPEN:** The full key list (keys appear only after the client has saved them), value ranges and defaults, and the `[Launcher]` key that holds the client type.

---

## 9. Version fingerprint

### 9.1 Header fields used

All offsets are absolute file offsets. All integers are little-endian. Only these fields are needed here; the dat container spec owns the rest.

| Offset | Size | Type | Meaning | Changes between A and B? |
|---|---|---|---|---|
| 0x000 | 256 | bytes | version string area; empty (first byte 0) in every observed archive | no |
| 0x100 | 4 | bytes | `00 50 4C 00` in every observed archive | no |
| 0x140 | 4 | u32 | magic, must be 0x00005442 | no |
| 0x144 | 4 | u32 | block size (§3.2) | no |
| 0x148 | 4 | u32 | file length, must equal the real length | yes (layout) |
| 0x14C | 4 | u32 | archive kind (§3.2) | no |
| 0x150 | 4 | u32 | region or language code (§3.2) | no |
| 0x160 | 4 | u32 | root directory node offset | yes (layout) |
| 0x170 | 4 | u32 | 0x28000000 in `general`, 0 elsewhere | no |
| 0x174 | 4 | u32 | 112 in all archives | no |
| 0x178 | 4 | u32 | 3 (shared-data and language families) or 0 (region family) | no |
| 0x17C | 16 | bytes | a 16-byte identifier, identical in 27 of 29 archives; all zero in `highres_aux_3` and `mesh_aux_1` | no |
| 0x18C | 4 | u32 | 1100 in shared-data and language archives and in `cell_1`/`map_1`; 17139 in the other region archives; 0 where 0x17C is zero | no |
| 0x190 | 12 | 3 × u32 | 104, 32, 8 in every archive | no |
| 0x19C | 4 | u32 | varies | yes |

Fields marked "layout" change whenever the patcher moves data. Two installs with the same content can differ there. They are **not** part of the content fingerprint.

> **OPEN:** The meaning of 0x170, 0x17C, 0x18C, 0x190–0x19F (0x19C may be a free-byte count).

### 9.2 Housekeeping records

These are ordinary subfiles found by DID lookup in the archive's directory (dat container spec). All the entries observed were stored uncompressed, but readers must honour the compression flag as for any subfile. Integers are little-endian. "stamp" means 16 opaque bytes, compared bytewise, never interpreted. All layouts are Confirmed (user install 2026-10-03, versions A and B).

**Which archives carry which record** (Confirmed):

| Record | Present in |
|---|---|
| 0xFFFF0001 | every archive |
| 0xFFFF0003 | every archive except `highres_aux_3`, `mesh_aux_1` |
| 0xFFFF0004 | `general`, `gamelogic`, `anim`, `mesh`, `sound`, `surface`, `highres`, `local_*`, `cell_1`, `map_1` |
| 0xFFFF0005 | every archive |
| 0xFFFF0008 | every base (`.dat`) archive; no `.datx` |

#### 9.2.1 0xFFFF0001: iteration record

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | i32 | `N`: number of iterations this archive holds (≥ 0) |
| 4 | var | i32 tokens | run list, decoded as in §9.3 |
| after runs | 4 | u32 | `K`: number of trailing entries (observed 11, or 0 when `N` = 0) |
| +4 | 12 × K | — | `K` entries, each a u32 iteration number followed by 8 opaque bytes |

Observed sizes: 148–160 bytes for archives with 1–2 runs; 288 bytes for `highres_aux_3` (30 runs); 8 bytes (`N` = 0, `K` = 0) for `highres_aux_1`, `highres_aux_2`, `surface_aux_1`, `surface_aux_2` and `sound_aux_1`.

> **OPEN:** What the 8 bytes in each trailing entry mean. They are not a Windows FILETIME. They look like hashes or random stamps.

#### 9.2.2 0xFFFF0003: install version stamp

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 16 | stamp | stamp S1 |
| 16 | 16 | stamp | stamp S2 |
| 32 | 1 | u8 | string length `L` (observed 4) |
| 33 | L | ASCII | a short decimal string: `5758` in shared-data/language archives and in `cell_1`/`map_1`; another value shared by each other `cell_<r>`/`map_<r>` pair |

Total size observed: 37 bytes. The S1/S2 pair is **the same in every archive that has the record**, and it changed between A and B (P8).

> **OPEN:** Whether the length byte is a general variable-length count (values ≥ 0x80 never observed). Readers accept only `L` < 0x80, and require `33 + L` = the record size. The meaning of the decimal string is unknown.

#### 9.2.3 0xFFFF0004

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | i32 | `M` (> 0) |
| 4 | 4 | i32 | −`M` |
| 8 | 4 | i32 | 1 |

Unchanged between A and B. Not used in the fingerprint.

> **OPEN:** Meaning.

#### 9.2.4 0xFFFF0005: split-set membership

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 16 | stamp | set stamp: equal for all members of one split set; changed between A and B |
| 16 | 4 | u32 | member index (0 for the `.dat` base, `n` for `_aux_<n>`) |
| 20 | 4 | u32 | member count of the set (1 for unsplit archives) |

#### 9.2.5 0xFFFF0008

16 bytes: one stamp, identical in every archive that has it, in both A and B. Not used in the fingerprint.

> **OPEN:** Meaning.

### 9.3 Decoding the iteration run list

Read the i32 `N`. Keep a counter `remaining` = `N`. While `remaining` > 0:

- Read an i32 token `t`.
- If `t` < 0, it is a **run**: read the next i32 as `start`. The run covers the iterations `start` … `start + (−t) − 1`. Subtract `−t` from `remaining`.
- If `t` ≥ 0, it is a **single** iteration `t`. Subtract 1 from `remaining`.

Then read `K` and the trailing entries.

The record is **malformed** (report it, do not panic) when any of these happen:

- `N` < 0;
- `remaining` would go below 0;
- a run length is 0;
- the record ends early;
- `K` > 64;
- bytes remain after the trailing entries.

Bound the number of tokens by the record size.

Derived values:

- `N`, the iteration count;
- `HI`, the highest iteration: the largest of `start + len − 1` over all runs and singles, or none when `N` = 0;
- `RUNS`, the number of runs plus singles.

Observed (Confirmed):

- Shared-data and language base archives have 2 runs: one starting at 1, and a second that starts exactly 10000 above the last iteration of the first run. Example: `1…3630` and `13630…20167` for `general` in B.
- Region archives 3, 4, 5, 14 and 15 have one run starting at 1.
- `highres_aux_3` and `mesh_aux_1` have many runs and singles.
- `N` and `HI` grew from A to B in every archive with `N` > 0. Example: `general` went from `N` 9770, `HI` 19769 to `N` 10168, `HI` 20167.
- The trailing list holds the `K` highest iterations of the archive in ascending order.

### 9.4 Fingerprint algorithm

The fingerprint has three levels. Each level is a list of per-archive tuples, sorted by file name in lower case.

**Level 0, schema identity** (header only; cheap). Per archive: lower-case file name; 0x14C; 0x150; 0x174; 0x178; block size; the 0x17C bytes; 0x18C. Use this level to check that an install is the kind of data our decoders expect (e.g. 0x174 = 112). It is the same across content updates (P6).

**Level 1, content version** (one directory lookup per housekeeping record). Per archive:

- lower-case file name;
- `N` and `HI` from 0xFFFF0001;
- the set stamp, index and count from 0xFFFF0005;
- the S1/S2 pair and the decimal string from 0xFFFF0003, if the record is present.

Also add one install-wide value: the S1/S2 pair, which must be identical across archives; if it is not, record "mixed".

**Level 2, deep** (optional; walks every directory). Per archive: the number of directory entries, the highest entry timestamp, and the highest entry iteration. Use this level only for diagnostics, e.g. to detect a half-applied patch.

**Fingerprint string.** Serialise level 1 as UTF-8 text, one line per archive, fields separated by a single space, integers in decimal, stamps as 32 lower-case hex digits in byte order. Hash the text with SHA-256 and give the first 16 hex digits as the short fingerprint. The serialisation is our own convention. It must stay stable, because caches key on it.

**Uses:**

- Real-data tests record the short fingerprint they ran against.
- Caches derived from archives (indexes, decoded assets) are keyed by it.
- Two installs with equal level-1 fingerprints are treated as content-identical (Reported as an inference; it held for the two observed copies of version A).

### 9.5 Build labels that are not part of the fingerprint

- **Client file version**: the PE version resource of `x64/lotroclient64.exe`. Its fixed-info file version has the form `AAAA.BBBB.CCCC.DDDD`, observed `4906.0071.1794.4035` (B) and `4601.0070.2401.4020` (A). The product-version string adds a channel word, observed `lotro_live` for the clients and `lotro` for the launcher.
  - `lotroclient.exe` and `LotroLauncher.exe` carry the same file version as the 64-bit client (Confirmed).
  - Reading the PE resource section is a bounded parse of a public file format. The client's code is never executed or disassembled.
  - Show this label to users. Do not use it to choose decoders.
- **Steam `buildid`** from the app manifest (§5.3). Steam-only; coarse.
- **`ProjectVersion`** (§7). Unchanged across the observed versions.

> **OPEN:** Whether the first group of the file version (`4906`) encodes the content update number (it rose from 4601 to 4906 between A and B).

### 9.6 Invariants

Each of these can be checked on real data and on synthetic fixtures. All of them held in versions A and B (Confirmed):

1. Every archive has magic 0x5442 at 0x140, and 0x148 equals the file length.
2. Every archive is smaller than 2³² bytes.
3. 0x174 = 112 everywhere. 0x178 = 0 exactly for archives whose 0x14C family byte is 2.
4. Aux members share their base's 0x14C, 0x150 and 0x144.
5. For each split set, all 0xFFFF0005 stamps are equal, the indices are exactly 0…count−1, and every member reports the same count.
6. The 0xFFFF0003 S1/S2 pair is identical in every archive that carries the record.
7. Every 0xFFFF0001 record decodes with no bytes left over. The sum of run lengths and singles equals `N`.
8. Region archives come in `cell_<r>`/`map_<r>` pairs with equal 0x150 = `r`, and equal 0xFFFF0003 decimal strings.
9. For `general` and `map_5`: the highest directory-entry iteration equals `HI` − 1.

---

## 10. Client command line

Our client accepts the option set of the original 64-bit client, so third-party launchers can start it without changes. The table restates the help text recorded by OL (P30) in our own words. "Ours" says what our client does with each option. Options that would need a feature we do not have are accepted and ignored with an info log.

| Short | Long | Argument | Meaning | Ours |
|---|---|---|---|---|
| `-a` | `--account` | string | Account (subscription) name to log in with. Launchers pass the subscription name, not the login user name | use |
| | `--authserverurl` | URL | Auth service URL, used to refresh the login ticket | use (net) |
| | `--chatserver` | host:port | Chat server to use | use (net) |
| | `--connspeed` | 0.0–640.0 | Connection speed in kbit/s; 0 = probe | accept |
| `-r` | `--create` | name | Character name to create or play | use |
| | `--cwd` | dir | Change directory at start-up (reported not to work in the original; launchers chdir themselves) | use (works in ours) |
| | `--debug` | 32-bit bit field | Which debug outputs are on | map to log filters |
| `-f` | `--franchise` | string | Franchise name | accept |
| | `--gametype` | string | Game type, used for support URLs | accept; also a game-type hint (§3.7) |
| | `--glsticket` | registry key | Load the login ticket from a registry key | accept (Windows only) |
| `-z` | `--glsticketdirect` | string | Login ticket data given directly | use (net); **never log the value** |
| | `--glsticketlifetime` | seconds | Ticket lifetime | use (net) |
| | `--HighResOutOfDate` | none | The high-resolution archive was not updated; do not switch to very high texture detail | use (§3.3) |
| `-h` | `--host` | host or IP | Server to connect to (also see `--port`) | use (net) |
| | `--keymap` | file | Base key-map file; the client also looks for `<name>c<.ext>` and `<name>s<.ext>` for meta-key variants | use |
| | `--language` | string | Language to run in. Matches the `<Lang>` of `client_local_<Lang>.dat` (§3.5) | use |
| | `--loadfile` | file | Load file for an auto-generated character | accept |
| `-m` | `--mps` | file (`.wc`) | Monster-play session to start with | accept |
| | `--outport` | 1–65535 | Outgoing network port | use (net) |
| `-p` | `--port` | 1–65535 | Server port | use (net) |
| | `--prefs` | file | Preferences file; also moves the settings folder to the file's parent (§8.1) | use |
| | `--remotemouse` | none | Optimise for a mouse over a remote connection | accept |
| | `--resource-set` | list | Comma-separated resource sets; the last is the default | accept |
| | `--rodat` | (value) | No-op kept for compatibility; launchers pass `--rodat on` | accept (always read-only) |
| | `--safe` | none | Force safe display settings | use |
| `-s` | `--specify` | race,class | Race/class for a character to create | accept |
| | `--surveyurl` | URL | URL for quest surveys | accept |
| | `--usechat` | none | Connect to a chat server | use (net) |
| | `--useexiturl` | none | Open a browser at an exit URL | accept, never open |
| `-u` | `--user` | name | Character name to play | use |
| | `--voicenetdelay` | int (ms) | Voice network delay threshold | accept |
| | `--voiceoff` | none | Disable voice chat | accept |
| | `--wfilelog` | 64-bit mask or names | File logging for event types; names are comma-separated and OR'd | map to log filters |
| | `--wprintlog` | 64-bit mask or names | Print logging, same argument form | map to log filters |

Older or template-only option names (Reported: NOTES network §1.4, §1.7), all to be **accepted and ignored**: `--dbwlib`, `--nodbwlib`, `--usetdm`, `--webjournalurl`, `--bugurl`, `--supporturl`, `--supportserviceurl`, `--crashreceiver`, `--DefaultUploadThrottleMbps`. Our own additions use a `--tessera-` prefix, plus `--game-dir`, to avoid clashes.

**Parsing rules:**

1. Trim leading and trailing ASCII whitespace from every argv element before parsing. At least one launcher appends `" --HighResOutOfDate"` with a leading space as a single element (P31; Reported).
2. Long options are matched **case-insensitively**, since `--HighResOutOfDate` is mixed-case. Short options are case-sensitive.
3. The option and its value may be separate argv elements (the observed form). `--opt=value` is also accepted.
4. Whether an option takes a value is decided by the table above. Unknown options that look like `--name` take the next element as their value only if that element does not start with `-`. This keeps unknown future options from swallowing known ones.
5. Unknown options are a warning, never fatal. Repeated options: the last one wins.
6. Values for `--glsticketdirect`, `--account` and `--glsticket` are secrets. Redact them in every log line.
7. Path values (`--prefs`, `--keymap`, `--cwd`, `--loadfile`) may be Windows paths. Translate them with §5.2 when running natively on Linux or macOS.

> **OPEN:** Whether the original client accepts `--opt=value`, and whether its long options are case-insensitive. Our parser is deliberately lenient in both cases.
> **OPEN:** The meaning of the `--debug` bits and the `--wfilelog` / `--wprintlog` enumeration names.

---

## 11. Summary of open questions

All the `> **OPEN:**` callouts above, collected:

- §3.1 aux members for region and language archives.
- §3.2 the 0x14C second byte; the 0x150 language codes for DE and FR; the meaning of regions.
- §3.6 the roles of `raw/`, `bindat/` and `forward/`.
- §5.3 the Flatpak and Snap roots; the legacy VDF format; depot 212502.
- §5.5 the Inno key brace form; the KD value names; the Codemasters path.
- §5.6.3 the Lutris config format.
- §6 the base of `Launcher.DocumentFolder`.
- §7 the meaning of `ProjectVersion`.
- §8.2 the PluginData scope folders; screenshot and log folders.
- §8.3 the full preference key list; whether our client shares the file with the official client.
- §9.1 the meaning of the unknown header fields.
- §9.2 the trailing iteration bytes; the 0xFFFF0003 string; 0xFFFF0004; 0xFFFF0008.
- §9.5 the file-version semantics.
- §10 the original client's option-parsing leniency; the debug and log masks.

---

## 12. Change log

| Date | Change |
|---|---|
| 2026-10-03 | First draft. Corrects the research notes: cell/map region 15 exists; 0x178 is 0 in cell/map archives; the 0x17C identifier and 0x18C do not change with content updates; the Steam folder is `Lord of the Rings Online` (no "The"). Adds the housekeeping-record layouts and the fingerprint. |

---

## 13. Open items raised during implementation (`crates/install`, 2026-10-04)

The implementer chose the conservative behaviour below where this spec is silent. Spec writers: confirm or correct.

> **OPEN:** §9.4 gives no serialisation for level 0, nor for absent values in level 1. `crates/install` uses: level 0 = one line per archive, `name kind_word sub_code 0x174 0x178 block_size <0x17C as 32 hex> 0x18C` (decimal integers, lower-case file name, sorted), archives with an unreadable header left out. Level 1 = the §9.4 field order, `-` for an unavailable value (no record, `HI` when `N` = 0, empty label), the 0xFFFF0003 fields only when the record exists, and a final line `install <S1> <S2>` / `install mixed` / `install none` carrying the install-wide value. Every line ends with `\n`. Changing this later invalidates cache keys.

> **OPEN:** §5.8 "cap total directories visited per search at 20,000": implemented as 20,000 **per walk root** (each prefix `Program Files` folder, each home-scan root), so one huge root cannot starve the others. Confirm whether a global cap was intended.

> **OPEN:** §5.8 skips `compatdata` "when walking inside `drive_c`". The implementation skips `dosdevices`, `backup` and `compatdata` in every walk, since compatdata prefixes are enumerated separately (§5.6.1).

> **OPEN:** §5.3 VDF escapes: the spec says a backslash escapes the next character, so `\n` reads as the letter `n`, not a newline. Implemented as written. Confirm whether Steam's writer ever emits `\n`/`\t` sequences that must become control characters.

> **OPEN:** §9.2.4 does not say whether 0xFFFF0005 must be exactly 24 bytes. The implementation requires at least 24 and ignores extra bytes (debug log).

> **OPEN:** §8.1 does not say which folder to propose when no candidate exists yet. The implementation proposes the first step-6 path (`$XDG_DOCUMENTS_DIR/NAME`, else `~/Documents/NAME`), never a path in the install or prefix.

> **OPEN:** §2 "take the exact-case one": for archive names the implementation treats the canonical spelling of §3.1 (`client_general.dat`, `client_<key>_aux_<n>.datx`, the language part as written) as the exact case.
