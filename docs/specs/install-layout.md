<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Install layout: locating, validating and identifying a game install

| | |
|---|---|
| **Status** | Accepted (re-review of 2026-10-07 passed; the 6th revision of 2026-10-07 is the Accepted text; see §12) |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Re-review 2026-10-07 of the 6th revision: **passed**, Accepted. Full record moved to the review log (decision 0018). |
| **Consumers** | `crates/install`; also `crates/dat` (archive-set opening), `apps/client` (command line), `crates/script` (plugin folders) |
| **Companion specs** | [`dat-container.md`](dat-container.md) (header fields, archive table, set opening, iteration semantics), [`keymap.md`](keymap.md) (`lotro.keymap` format, `--keymap`), [`lua-plugin-api.md`](lua-plugin-api.md) §10 (plugin-data folders) |

---

<a id="s0"></a>
## 0. Purpose

This spec lets an implementer build the `install` crate without reading any dirty material. The crate must:

1. find candidate game installs on Linux, Windows and macOS (Steam, the standalone installer, Wine/Proton/Lutris/OneLauncher prefixes and other third-party launcher prefixes);
2. decide whether a directory is a usable install, and list which archives it has;
3. find the per-user settings folder that belongs to the install;
4. compute a **version fingerprint** for the install from the archive headers and a few housekeeping records, so that tests and caches can tell content versions apart;
5. accept the same command-line options as the original client, so existing third-party launchers can start our client unchanged.

The spec does not cover the dat container itself (header, extents, B-tree, compression). It repeats only the few header fields that identification needs. Lookup of a subfile by DID is the job of [`dat-container.md`](dat-container.md).

**Hard rule, which applies to every algorithm here:** the install directory is **read-only**. Nothing in this spec ever creates, renames, locks, truncates or writes a file under the install directory. Open archives read-only with shared access. On Windows that means allowing other readers and writers, because the official launcher may be patching at the same time.

---

<a id="s1"></a>
## 1. Provenance

Marks: **Confirmed** = observed in the user's own install or on the user's machine (file listings, header bytes, small read-only probes of housekeeping records, Steam and Proton metadata files, the client's own log files), with the date. A fact is marked Confirmed only after a probe checked it against **every** record or file of its kind present on that date. **Reported** = from published sources only, or an inference not checked by a census. **BIN** = read from the offline client executable by disassembly under research method M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)); each use is logged in the BIN table below.

<a id="s1-1"></a>
### 1.1 Data versions observed

All values in this spec belong to snapshot **B2** unless a row says otherwise. Values observed in one install are **observations**: an implementation reads them at run time and never hard-codes them.

| Label | Client file version | Observed | Description |
|---|---|---|---|
| **A** | `4601.0070.2401.4020` | 2026-10-03 | An older, separately installed copy (newest subfile timestamp 2026-02-04). It is no longer on the machine (2026-10-05). |
| **B1** | `4906.0071.1794.4035` | 2026-10-03 | The Steam install as first observed (newest subfile timestamp 2026-10-03). |
| **B2** | `4906.0071.1794.4035` (unchanged) | 2026-10-05 | The same Steam install after a data patch on 2026-10-04 at 22:08 local time (20:08 UTC). The patch rewrote every archive: all 29 archive files carry the modification time 20:08:53 UTC, and each archive's newest subfile timestamp lies between 20:08:20 and 20:08:53 UTC. The client executable was rewritten at 20:08:14 UTC with the same version. Six archive sizes and two aux iteration records changed (P43). |

<a id="s1-2"></a>
### 1.2 Sources

- **OL**: OneLauncher (GPL-3), its game-dir search, launcher-config handling and launch code, plus the client help text it records (captured 2024-07-10).
- **LC**: LotroCompanion dat utilities.
- **EL**: Endevir Elanor launcher.
- **KD**: koniecdev LotroKoniecDev.
- **PL**: PyLotRO.
- **OS**: OpenSAGE Steam locator.
- **AC-DRW**: Asheron's Call dat readers (ACE DatLoader, Chorizite DatReaderWriter).
- **NOTES**: the project research notes (`dat-format.md`, `network.md`, `engine-client-re.md`).
- **LOG**: the official client's own log files, written under the install's prefix (`AppData/Local/The Lord of the Rings Online/`), read read-only (method M3). No account names, tickets or network content are taken from them.
- **DC**, **KM**: the companion specs [`dat-container.md`](dat-container.md) and [`keymap.md`](keymap.md), whose own provenance (Confirmed or BIN) is given there. Their recorded corrections to this spec are applied here (§12).
- **BIN**: the offline client executable `x64/lotroclient64.exe`, build 4906.0071.1794.4035, by disassembly (method M4, decision 0011).
- **DEP**: Steam's cached depot manifests for app 212500 under `<Steam root>/depotcache/` on the user machine, read read-only on 2026-10-07 for the file names they list (no content copied).
- **LL**: the `lotro-launcher` program on the user machine: its installed package metadata (Gentoo package database, 2026-10-07) and its published GPL-3 source (https://github.com/solarbaron/lotro-launcher, commit f880b07 of 2026-07-22), read for configuration key names only.
- **LUT**: Lutris, published source (https://github.com/lutris/lutris, `lutris/settings.py`, `lutris/util/steam/config.py`, `lutris/runners/wine.py`, read 2026-10-07), for its configuration locations and keys and its Steam library parsing.
- **INNO**: Inno Setup conventions (general knowledge of the published installer), for uninstall registry key names.

<a id="s1-3"></a>
### 1.3 BIN (M4) uses

| Use | Date | Build | Question | Why M1–M3 were not enough | What was read |
|---|---|---|---|---|---|
| BIN-1 | 2026-10-05 | 4906.0071.1794.4035 | Does the client's command-line parser ignore case in option names, and does it accept `name=value`? (owner item O7) | M1: the behaviour is in code, not data. M2: OL records only the option names and help text. M3: launching the client with trial arguments was not available in this session, and a launch shows only accept or reject, not the rule. | The client's own argument-registration and argument-parsing routines only. |
| BIN-2 | 2026-10-05 | 4906.0071.1794.4035 | (a) Which leading characters make an argv element an option? (b) When is the one-letter form looked up? (c) When does `?` show the usage text? (d) Per option: does it take no value, a required value or an optional value, and is the `name=value` form honoured? (e) What happens to unknown options and to elements that are not options? | M1 and M2 as for BIN-1. M3 cannot tell "accepted" from "silently ignored": the original ignores unknown and malformed options without any message (§10.1), so a launch test shows nothing for most cases. | The same parsing routine; the option-registration routines, read only for each option's value mode and one-letter form. The code that **uses** option values (network, login, launcher, patcher) was **not** examined (decision 0004). |
| BIN-3 | 2026-10-05 | 4906.0071.1794.4035 | Review re-check of BIN-1/BIN-2 (all 34 options, modes, one-letter forms, every §10.1 rule), which also found how wide the option-token character test is | As for BIN-2 | The same parsing and option-registration routines, by the review session; nothing from network, login or launcher code; no listing kept |
| BIN-4 | 2026-10-07 | 4906.0071.1794.4035 | (a) Where the client puts its preferences file, plugins, music files and screenshots, and under which names; (b) whether the client itself reads `ProjectVersion`, `Product.DocumentFolder`, `Launcher.DocumentFolder`, `[Launcher] GameClientType` or any built-in `raw/` path; (c) how `[Display] WindowedResolution` is packed; (d) how the 0xFFFF0003 label length is encoded and what the client does with the label; (e) how the 0xFFFF0001 trailing list depends on the record's `stamp`, and whether the client interprets its u64 values or 0xFFFF0004 / 0xFFFF0008; (f) the value forms of `--debug`, `--wfilelog` and `--wprintlog`; (g) repeated options and the doubled-introducer test (open items of §10.1) | M1: these are run-time behaviours of the code, not data (the one data part, the log-type names, was then read with M1, P62). M2: no published source covers them. M3: the folder and screenshot behaviour would need the client to run and write files, which a dirty-room session on this machine cannot do; window packing would need several resized runs, and the parser rules cannot be told apart by launch tests (§10.1). | The client's application start-up (preferences-file and folder setup), its screenshot, music-file and plugin-folder routines, the window-size preference code, the log-type parser, the option dispatch and parsing routines, and the archive library's metadata-record loaders. Nothing from network, login, launcher, patcher or anti-tamper code (option handlers for network and login options were skipped); no listing kept |

No disassembly listing, pseudo-code, symbol or client string leaves the dirty scratch area; the facts below are stated as behaviour.

<a id="s1-4"></a>
### 1.4 Provenance table

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| P1 | Archive names are `client_<key>.dat`. Aux archives are `client_<key>_aux_<n>.datx` | Observed in user install 2026-10-03 and 2026-10-05; LC; NOTES dat-format §1 | Confirmed |
| P2 | Archive inventory in §3.2 (the 29 archives, including `client_cell_15.dat` and `client_map_15.dat`, which the notes did not list). Sizes are a dated snapshot (B2) | Observed in user install 2026-10-03 (B1) and 2026-10-05 (B2, all 29) | Confirmed (corrects NOTES) |
| P3 | No archive reaches 4 GiB. The largest is 4,287,627,264 bytes | Observed in user install 2026-10-05 (all 29) | Confirmed |
| P4 | Header bytes 0x14C–0x153 identify the archive family, kind and subset (region number or folded language code) | Values observed in user install 2026-10-05 (all 29); meaning per DC §3.4 (BIN there) | Confirmed (values); meaning from DC |
| P5 | Header 0x174 = 112 in every archive. Header 0x178 = 3 in general/gamelogic/anim/mesh/sound/surface/highres/local and **0** in every cell and map archive | Observed in user install 2026-10-03 (A, B1) and 2026-10-05 (B2, all 29); NOTES dat-format §2 said "observed as 3" | Confirmed (corrects NOTES: cell/map are 0) |
| P6 | The 16-byte value at 0x17C and the u32 at 0x18C did **not** change between A and B1. Two aux archives have zeros there (also in B2) | Observed in user install 2026-10-03 (diff of A and B1), 2026-10-05 (B2) | Confirmed |
| P7 | Housekeeping subfiles 0xFFFF0001, 0xFFFF0003, 0xFFFF0004, 0xFFFF0005 and 0xFFFF0008 exist, with the layouts and presence in §9.2 | Observed in user install 2026-10-03 and 2026-10-05 (all 29); AC-DRW (0xFFFF0001 = "iteration" record in AC) | Confirmed (layouts), Reported (AC analogy) |
| P8 | 0xFFFF0003 carries a pair of 16-byte stamps that is identical in every archive that has the record, and that changed between A and B1 | Observed in user install 2026-10-03; identity re-checked 2026-10-05 (B2, 27 archives) | Confirmed |
| P9 | 0xFFFF0005 carries a 16-byte stamp shared by a base archive and its aux archives, plus the member index and the member count | Observed in user install 2026-10-03 and 2026-10-05 (all 29) | Confirmed |
| P10 | The 64-bit client is `x64/lotroclient64.exe`. A 32-bit `lotroclient.exe` and the launcher `LotroLauncher.exe` sit in the install root | Observed in user install 2026-10-03; OL; NOTES engine-client-re §1 | Confirmed |
| P11 | The PE version resource gives a four-part file version and a product-version string ending in a channel word | Observed in user install 2026-10-03, re-checked 2026-10-05 (all three executables) | Confirmed |
| P12 | `ProjectVersion` is a 3-byte ASCII file whose content is `120` in A, B1 and B2, with no line terminator | Observed in user install 2026-10-03 and 2026-10-05 | Confirmed |
| P13 | `lotro.launcherconfig` is a .NET appSettings XML file. Element and key names are in §6 | Observed in user install 2026-10-03; OL; NOTES network §1.1 | Confirmed |
| P14 | Launcher-config lookup order: `<gametype>.launcherconfig`, then any `*.launcherconfig`, then `TurbineLauncher.exe.config`, then (DDO) `DNDLauncher.exe.config` | OL; NOTES network §1.1 | Reported |
| P15 | Steam app id 212500. `installdir` is `Lord of the Rings Online`. Depots and acf keys are in §5.3 | Observed in user install 2026-10-03 and 2026-10-05 (`appmanifest_212500.acf`); EL | Confirmed |
| P16 | `libraryfolders.vdf` structure, its locations under `~/.local/share/Steam` (`steamapps/` and `config/`, identical copies), and `~/.steam/steam` being a symlink to it | Observed on user machine 2026-10-03, copies re-compared 2026-10-05; OS | Confirmed |
| P17 | The Proton prefix lives in `<library>/steamapps/compatdata/212500/pfx`, in the library **that holds the game**. On the user machine that library is the default root (`~/.local/share/Steam`) on 2026-10-05; the only other library lists no apps. That the prefix follows the game into a non-default library is Steam's convention, not observed | Observed on user machine 2026-10-03 and 2026-10-05 (default-root case only) | Confirmed (default-root case); Reported (non-default library) |
| P18 | The settings folder inside a Steam Proton prefix is `pfx/drive_c/users/steamuser/Documents/The Lord of the Rings Online/` | Observed on user machine 2026-10-03 and 2026-10-05; OL | Confirmed |
| P19 | A settings folder can also appear directly as `$HOME/The Lord of the Rings Online/` when a Wine prefix maps Documents to the home directory | Reported by the first draft as seen on 2026-10-03; the folder is **absent** on 2026-10-05, and no prefix on the machine maps Documents to the home directory (both prefixes have a real `Documents` directory). Mapping behaviour is Wine's (from memory) | Reported |
| P20 | `UserPreferences.ini` is UTF-8, INI sections, `Key=Value`. Key names are in §8.3. Line ends are CRLF in the file the official client and launcher write, LF in a file under a third-party launcher's prefix (P45) | Observed on user machine 2026-10-03 and 2026-10-05 (both files on the machine); OL; NOTES engine-client-re §2 | Confirmed |
| P21 | Keys may appear lower-cased (e.g. `graphicscore`), because third-party launchers rewrite them | Reported by the first draft for a copy seen on 2026-10-03 that no longer exists; neither preferences file on the machine has a lower-cased key on 2026-10-05. OL writes through a case-folding INI writer | Reported |
| P22 | `PluginData/<account>/<world>/PluginOptions.xml` exists | Observed on user machine 2026-10-03 and 2026-10-05 | Confirmed |
| P23 | `Plugins/`, `Music/` (`.abc`), `ui/skins/<Skin>/SkinDefinition.xml`, screenshots, chat logs in the settings folder | NOTES engine-client-re §2, §6, §7; wiki pages via Wayback; BIN-4 for `Plugins`, `Music` and screenshots (P58). On 2026-10-05 neither settings folder on the machine has a `Plugins/` folder | BIN (`Plugins`, `Music`, screenshots); Reported (skins, chat logs); Confirmed (`PluginData/` and `lotro.keymap`, P22, P38) |
| P24 | Windows registry: `…\Uninstall\Steam App 212500` → `InstallLocation` | EL | Reported |
| P25 | Windows registry: `SOFTWARE\[WOW6432Node\]StandingStoneGames\The Lord of the Rings Online` and `SOFTWARE\WOW6432Node\Turbine\The Lord of the Rings Online` | KD | Reported |
| P26 | The install carries an Inno Setup uninstall log (`unins000.dat`) whose AppId is `12bbe590-c890-11d9-9669-0800200c9a66`. Inno Setup registers uninstall keys as `<AppId>_is1` | Observed in user install 2026-10-03 (AppId); Inno Setup convention (from memory) | Confirmed (AppId), Reported (key name) |
| P27 | Steam registry: `HKCU\Software\Valve\Steam` `SteamPath`; `HKLM\SOFTWARE\WOW6432Node\Valve\Steam` `InstallPath`; `HKCU\Software\Valve\Steam\Apps\212500` with `Installed`/`Running`/`Updating` DWORDs | Observed 2026-10-03 in the Proton prefix registry files; OS (InstallPath) | Confirmed |
| P28 | Standalone default install path `C:\Program Files (x86)\StandingStoneGames\The Lord of the Rings Online` | KD | Reported |
| P29 | OneLauncher search roots, prefix layouts, its built-in prefix path and its per-game config file (§5.6) | OL | Reported |
| P30 | The set of 34 client command-line options and their meanings (§10) | OL (help text the client prints, captured 2024-07-10); NOTES network §1.7; the option names, one-letter forms and value modes read from the client (BIN-1, BIN-2) | Reported (meanings), BIN (names, one-letter forms, value modes) |
| P31 | Launch-argument templates put option names in argv that the help text does not list, and one launcher appends an element with a leading space | NOTES network §1.4; OL | Reported |
| P32 | `--prefs <file>` also moves the settings folder to the parent of `<file>`. `--cwd` does not work in practice | OL; BIN-4 (the settings folder is the directory of the preferences file, P57) | BIN (first sentence); Reported (`--cwd`) |
| P33 | Language archive names are the labels of the client's language enumeration: `English`, `EN_GB`, `DE`, `FR` | DC §2.1 (labels from the data there); `English` observed in user install 2026-10-03 and 2026-10-05; OL (`DE`, `FR`) | Confirmed (`English` file), DC (labels) |
| P34 | Iteration record decoding (§9.3): runs and singles; the run and single counts sum to the record's count; the trailing list consumes the rest exactly | Observed in user install 2026-10-03 (A, B1) and 2026-10-05 (B2): all 29 archives parse with zero bytes left over; AC-DRW (base layout) | Confirmed |
| P35 | Iteration census (§9.3, invariants 9–10): every non-zero directory-entry iteration of **every member** of a split set (or of an unsplit archive) is an iteration listed in the **base** archive's 0xFFFF0001 record, and is at most that record's `HI` − 1. Here `HI` is always the **base** archive's highest listed value, also for aux members. An archive's highest entry iteration **equals** that `HI` − 1 in only 13 of the 29 archives: anim, cell_1–5, general, local_English, map_1, map_5, highres_aux_3, mesh_aux_1 and sound_aux_1 (sound_aux_1's own list is empty). Counted per split set (highest entry iteration over all members), equality holds in 13 of the 22 sets: the ten unsplit archives above plus highres, mesh and sound, whose highest entry sits in an aux member. An aux member's own list, when not empty, is a subset of the base's list and does not cover the member's own entries | Census of every directory entry of all 29 archives, user install 2026-10-05 (B2), equality count re-run against the base `HI` 2026-10-06 (B2); agrees with DC §5.2 and §7.3 | Confirmed 2026-10-06 (corrects the first draft, which generalised "= `HI` − 1" from two archives, and the third revision, which compared each archive with its own list) |
| P36 | Header 0x174 is the archive's "eng" datpack version and 0x178 its "game" datpack version. The client's log reports both numbers for every archive it opens, under those two labels, and the values match the header bytes (112 and 3, or 112 and 0 for region archives); for the executable it reports −1 for both, i.e. no restriction | LOG (2026-10-05: 15 archives logged with game 3, 14 with game 0); header bytes, user install 2026-10-05 (all 29); DC §3.3 (BIN: container version and game-data version) | Confirmed |
| P37 | Command-line parsing rules of the original (§10.1) (BIN-3 re-check: the option-token test uses only the low 8 bits of the first UTF-16 code unit): `-` and `/` introduce options; a doubled introducer selects the long form; the one-letter form is tried only for a single introducer and a one-character name, case-sensitively, then falls back to the long form; long names ignore case; `name=value` is honoured only by options that allow it (`--account` in this build), is an error for options without a value, and makes the element unknown otherwise; `?` shows usage only after the name matched nothing; unknown options and non-option elements are ignored silently; a required value is taken from the next element whatever it starts with | BIN-1, BIN-2, BIN-3 | BIN |
| P38 | The per-user folder also holds a `CEF/` subfolder (in-game store browser cache: `CachePath`, `UserData`). The client's own run-time **logs** are written outside it, to `AppData/Local/The Lord of the Rings Online/` (`lotroclient64.log`, and the launcher/patcher logs) | Observed on user machine 2026-10-05 | Confirmed |
| P39 | `UserPreferences.ini` carries `[Launcher] GameClientType` (observed value 3 on a 64-bit install) and a numbered `[User_N]` block with `UserName`, `WorldName_N`, `LastPlayed_N`, `UserFileName_N`, `UserAckTime_N` | Observed on user machine 2026-10-05 | Confirmed |
| P40 | `UserPreferences.ini` and `lotro.keymap` as written by the official launcher and client are UTF-8 **without** a BOM, CRLF line ends, trailing newline, ASCII content. Why the 2026-10-03 draft saw a BOM is not known; a third-party launcher is one possible cause, but the only third-party-launcher preferences file on the machine has no BOM either | Observed on user machine 2026-10-05 (no BOM in any preferences or key-map file present) | Confirmed (no BOM); Reported (cause) |
| P41 | The current Steam `appmanifest_212500.acf` lists exactly two `InstalledDepots`: 212501 and 212502. Redistributables ship as a separate Steam app (the library's `apps` list also has 228980). `buildid` observed `12250101` | Observed in user install 2026-10-05 | Confirmed (corrects the first draft's depot list) |
| P42 | The install also holds `browser/` and `patches/` directories (named in `uninstall_manifest.txt`); `forward/` holds forward-patch iteration data during patching; `bindat/`, `tmp/`, `temp/` and `downloading/` are patcher working directories, empty when idle | Observed in user install 2026-10-05 (listing, `uninstall_manifest.txt`, `downloading.txt`) | Confirmed |
| P43 | Snapshot B2 (§1.1): the 2026-10-04 data patch changed six sizes (§3.2) and two aux iteration records: `highres_aux_3` from 288 bytes and 30 runs to 284 bytes and 29 runs, `mesh_aux_1` from 3,196 to 3,192 bytes (652 runs). `general` kept `N` 10168 and `HI` 20167 | Observed in user install 2026-10-05; B1 values from the first draft and DC §7.2 | Confirmed |
| P44 | A third-party launcher on the user machine keeps per-game JSON configs at `~/.config/lotro-launcher/games/<id>/config.json` and its own Proton-style prefix at `~/.local/share/lotro-launcher/prefix` (§5.6.5) | Observed on user machine 2026-10-05 (key names and value types only; no paths or account data copied) | Confirmed |
| P45 | A second `UserPreferences.ini`, under that third-party launcher's prefix: LF line ends (no CR), no BOM, a trailing blank line, no `[Launcher]` or `[User_N]` section, an extra `[General]` section with key `LastWorld`, and `WindowedResolution=83756426` | Observed on user machine 2026-10-05 | Confirmed |
| P46 | Housekeeping record sizes: 0xFFFF0005 is exactly 24 bytes, 0xFFFF0003 exactly 37 bytes, 0xFFFF0004 12 bytes, 0xFFFF0008 16 bytes in every archive that has them; 0xFFFF0001 sizes in §9.2.1 | Census, user install 2026-10-05 (all 29) | Confirmed |
| P47 | Every VDF and ACF file under the Steam root uses only two backslash sequences, `\\` and `\"`; none contains `\n` or `\t` | Census of `libraryfolders.vdf` (both copies), all 24 `appmanifest_*.acf` and `config/config.vdf` on the user machine 2026-10-05. All 24 manifests are in the default root's `steamapps/`; the second library named in `libraryfolders.vdf` has an empty `steamapps/` and lists no apps | Confirmed |
| P48 | Imported corrections: each 0xFFFF0001 trailing entry is a u32 iteration and a u64 value; the client's archive library never reads 0xFFFF0004; any archive (not only shared-data ones) may have aux members; regions are the keys of the RegionID DID map; the key-map companion files named in the help text are not read by this build | DC §2.3, §2.4, §7.3, §15 items 2–6; KM §13 item 1 | as marked in DC and KM (BIN, Confirmed) |
| P49 | The client contains no `raw/` path of its own. Every path under `raw/` that the data names is in the language archive: UTF-16 strings in string tables (0x25…, 86 occurrences) and in the language archive's 0x0E… record (2), and ASCII strings in 0x22… records (13). Each is relative to the install directory and starts `raw/<lang>/`. In this install they name 96 distinct files: 85 `.bik` videos (under `chargen/class_videos`, `chargen/race_videos`, `introcinematic`, `logo`, `vignette_videos`) and 11 `.jpg` images (under `logo`). All 96 exist under `raw/en/`; the one file there that no string names is a `.bik` under `vignette_videos` (§3.6) | BIN-4 (no such path in the client, ASCII or UTF-16); census of `client_local_English.dat`, `client_general.dat` and `client_gamelogic.dat` (2026-10-07); file listing of `raw/` (2026-10-07) | Confirmed 2026-10-07 (data and files), BIN (client) |
| P50 | Lutris keeps one YAML file per game in `<XDG config>/lutris/games/` (falling back to `<XDG data>/lutris/games/` when `<XDG config>/lutris` does not exist); for Wine games the `game` section has `exe`, `args`, `working_dir`, `prefix` and `arch`; a relative `exe` is relative to the game directory | LUT | Reported |
| P51 | Steam roots that Lutris searches on Linux include `~/.local/share/Steam`, `~/.steam/steam`, `~/.steam`, `~/.steam/debian-installation`, `~/snap/steam/common/.local/share/Steam`, `~/.var/app/com.valvesoftware.Steam/.local/share/Steam` and the older Flatpak location `~/.var/app/com.valvesoftware.Steam/data/Steam` | LUT. On the user machine only `~/.local/share/Steam` and `~/.steam/steam` exist (2026-10-07) | Reported (Flatpak, Snap, Debian forms); Confirmed (the two present) |
| P52 | `libraryfolders.vdf` may use a top-level key in any letter case (older files: `LibraryFolders`) and numbered entries whose value is a plain path string instead of a block; current files may also carry a `contentstatsid` entry that is not a library | LUT (its parser accepts both forms and ignores `contentstatsid`) | Reported |
| P53 | Depot 212501 lists the shared archives (`general`, `gamelogic`, `anim`, `mesh`, `sound`, `surface`, `highres`, `cell_*`, `map_*` and the aux parts `sound_aux_1`, `surface_aux_1`, `surface_aux_2`, `highres_aux_1`, `highres_aux_2`); depot 212502 lists `client_local_English.dat`, the `raw/en/` media and the launcher's `en/` resources, i.e. the English language content. `client_mesh_aux_1.datx` and `client_highres_aux_3.datx` are in neither depot: they were created on the machine by the patcher (they are also the two archives with an all-zero 0x17C, DC §3.3) | DEP (both cached manifests, 2026-10-07); header bytes (P6) | Confirmed 2026-10-07 |
| P54 | The Inno Setup uninstall log stores the AppId **without** braces (`12bbe590-c890-11d9-9669-0800200c9a66`); Inno Setup names the uninstall key `<AppId>_is1` | `unins000.dat` header, user install 2026-10-07; INNO | Confirmed (AppId form), Reported (key rule) |
| P55 | `lotro-launcher` is the project owner's own GPL-3 launcher (https://github.com/solarbaron/lotro-launcher), installed as version 0.1.7. Its per-game `config.json` keys are `id`, `gameType` (`LOTRO`, `LOTRO_Preview`, `DDO`, `DDO_Preview`), `gameDirectory`, `settingsDirectory`, `clientType` (`Win64`, `Win32`, …), `highResEnabled`, `locale`, `newsfeedUrl`. Its Wine settings (JSON names `prefixMode` = `Builtin` or `User`, `userPrefixPath`, `userWineExecutable`, `debugLevel`, `dxvkEnabled`, `dxvkVersion`, `esyncEnabled`, `fsyncEnabled`) exist in the program but are not written to any file by the published source | LL; the installed config files (key names only, P44) | Confirmed (package, installed keys), Reported (source) |
| P56 | The game client never reads `ProjectVersion`, `Product.DocumentFolder`, `Launcher.DocumentFolder` or `[Launcher] GameClientType`: none of these names occurs in the client, in ASCII or UTF-16 | BIN-4 | BIN |
| P57 | The client's preferences file is the `--prefs` value if given; otherwise `UserPreferences.ini` in the process's **current directory** if that file exists; otherwise `<Documents>\The Lord of the Rings Online\UserPreferences.ini`, where `<Documents>` is the shell's personal-documents folder and the folder name is fixed in the client. The settings folder is the directory that holds the preferences file. The run-time log folder is `<local application data>\The Lord of the Rings Online\` | BIN-4; LOG (log folder, P38) | BIN; Confirmed (log folder) |
| P58 | Relative to the settings folder: plugins in `Plugins`, player music in `Music` (a name is tried as `<name>.abc`, then `<name>.txt`), screenshots directly in the folder as `ScreenShot_<YYYY-MM-DD_HHMMSS>_<n>.jpg` with the first free `n` from 0 to 99 | BIN-4 | BIN |
| P59 | `[Display] WindowedResolution` = (client-area width in pixels × 65536) + client-area height in pixels | BIN-4; the two observed values (P45, §8.3) decode to 2560 × 1418 and 1278 × 1418 | BIN |
| P60 | 0xFFFF0003's label length is a variable-length count of 1, 2 or 4 bytes; the client marks the version stamp as invalid when the label contains the ASCII text `INVALID_VERSION` | BIN-4 | BIN; Confirmed (every observed label is 4 bytes with a 1-byte count, P46) |
| P61 | 0xFFFF0001: the trailing list exists only when the record's `stamp` is 2; `stamp` 1 ends after the run list; `stamp` ≥ 3 is refused; the client copies each u64 verbatim; no game-client code reads 0xFFFF0004; 0xFFFF0008 is the archive's product identifier | DC §7.3, §7.5 (BIN there); BIN-4 | BIN; Confirmed (every 0xFFFF0001 has `stamp` 2, 2026-10-05) |
| P62 | `--wfilelog` and `--wprintlog` are first tried as a string of `0`/`1` digits (one per bit; whether that attempt can succeed is OPEN, §10.1) and otherwise read as a comma-separated list of names; a name is a label of the game data's "LogType" enumeration (EMAPPER DID map label "LogType", 0x230000B5 in this install) and its value *v* sets bit *v* − 1 | BIN-4; EMAPPER DID map 0x28000001 decoded 2026-10-07 (key 0x29 → "LogType" → 0x230000B5; key 0x4A → "Languages" → 0x230000B1, matching DC P38) | BIN; Confirmed (label and DID) |
| P63 | `--debug` is read as an unsigned 32-bit number in C notation (decimal, `0x` hexadecimal, or leading-`0` octal); it sets every one of the 32 debug flags to the matching bit | BIN-4 | BIN |
| P64 | Each option occurrence is handed to the option's handler at once, in command-line order. The handlers read for `--prefs`, `--language` and `--debug` replace the earlier value, so the last occurrence wins | BIN-4 | BIN |
| P65 | The doubled-introducer test compares the second UTF-16 code unit with the first as whole 16-bit units | BIN-4 | BIN |
| P66 | Every `installdir` value in the 24 app manifests on the user machine is one path component made of ASCII letters, digits, space, `-` and `_` | Census of all 24 `appmanifest_*.acf`, 2026-10-07 | Confirmed |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Install directory** (`GAME_DIR`) | The directory that directly holds `client_general.dat`. |
| **Archive key** | The `<key>` part of `client_<key>.dat`, e.g. `general`, `cell_3`, `local_English`. |
| **Split set** | A base archive `client_<key>.dat` plus its aux archives `client_<key>_aux_1.datx … _aux_<n>.datx`, read together as one logical archive. An archive without aux members is a split set of one. |
| **Region** | The numeric suffix of `cell_<r>` and `map_<r>`. |
| **Settings folder** (`PREFS_DIR`) | The per-user folder in the user's Documents that holds `UserPreferences.ini`, plugin data and so on. |
| **Prefix** | A Wine/Proton directory tree that contains `drive_c/`. Proton nests it one level deeper, under `pfx/`. |
| **Housekeeping record** | A subfile whose DID is in 0xFFFF0000–0xFFFFFFFF. |
| **Option token** | A command-line element whose first character is `-` or `/` (§10.1). |

All file and directory name comparisons in this spec are **ASCII case-insensitive**. The game is a Windows program and its data may be copied onto a case-sensitive file system with any case. On a case-sensitive file system, resolve each path component by listing its parent directory and matching case-insensitively. If more than one entry matches, take the exact-case one if present; otherwise take the lexicographically smallest and log a warning. For archive names the "exact case" is the spelling of §3.1: lower-case `client_`, key and extension, and the language label as the enumeration writes it (e.g. `client_local_English.dat`).

---

<a id="s3"></a>
## 3. Install directory contents

<a id="s3-1"></a>
### 3.1 Archive name grammar

| Pattern | Kind | Notes |
|---|---|---|
| `client_<word>.dat` | base archive | `<word>` is lower-case letters (general, gamelogic, anim, mesh, sound, surface, highres) |
| `client_local_<Lang>.dat` | language archive | `<Lang>` keeps the case it is written in (see §3.5) |
| `client_cell_<r>.dat`, `client_map_<r>.dat` | region archives | `<r>` is decimal with no leading zeros |
| `client_<key>_aux_<n>.datx` | aux member of a split set | `<key>` is any of the keys above; `<n>` is decimal ≥ 1. Note the **`.datx`** extension |

Anything else that starts with `client_` is ignored with a debug log. Other `*.dat` files in the install (for example the uninstaller's `unins000.dat`) are not archives (DC §2.1).

Aux members are possible for every archive, including region and language archives: the original client applies the same set logic to all of them (DC §2.3, BIN there). In the observed install only shared-data archives have aux members (Confirmed 2026-10-05).

<a id="s3-2"></a>
### 3.2 Observed inventory (version B)

Snapshot **B2**, Confirmed (user install 2026-10-05, all 29 archives). Sizes change with patches; never hard-code them. The header columns are explained in §9.1 and DC §3.3–§3.4. "Set" is the split-set member index and count from record 0xFFFF0005 (§9.2.4).

| File | Size (bytes) | Block size (0x144) | 0x14C | 0x150 | 0x178 | Set |
|---|---|---|---|---|---|---|
| client_general.dat | 781,189,120 | 1024 | 0x01030000 | 0 | 3 | 0 of 1 |
| client_gamelogic.dat | 1,052,770,304 | 256 | 0x01010000 | 0 | 3 | 0 of 1 |
| client_anim.dat | 734,003,200 | 24000 | 0x01040000 | 0 | 3 | 0 of 1 |
| client_mesh.dat | 4,286,578,688 | 17408 | 0x01020000 | 0 | 3 | 0 of 2 |
| client_mesh_aux_1.datx | 1,673,527,296 | 17408 | 0x01020000 | 0 | 3 | 1 of 2 |
| client_sound.dat | 4,287,627,264 | 17408 | 0x01070000 | 0 | 3 | 0 of 2 |
| client_sound_aux_1.datx | 3,901,751,296 | 17408 | 0x01070000 | 0 | 3 | 1 of 2 |
| client_surface.dat | 4,287,627,264 | 4096 | 0x01090000 | 0 | 3 | 0 of 3 |
| client_surface_aux_1.datx | 4,287,627,264 | 4096 | 0x01090000 | 0 | 3 | 1 of 3 |
| client_surface_aux_2.datx | 2,643,460,096 | 4096 | 0x01090000 | 0 | 3 | 2 of 3 |
| client_highres.dat | 4,287,627,264 | 40960 | 0x01080000 | 0 | 3 | 0 of 4 |
| client_highres_aux_1.datx | 4,287,627,264 | 40960 | 0x01080000 | 0 | 3 | 1 of 4 |
| client_highres_aux_2.datx | 4,286,578,688 | 40960 | 0x01080000 | 0 | 3 | 2 of 4 |
| client_highres_aux_3.datx | 564,133,888 | 40960 | 0x01080000 | 0 | 3 | 3 of 4 |
| client_local_English.dat | 1,905,757,352 | 256 | 0x03030000 | 2 | 3 | 0 of 1 |
| client_cell_1.dat … _5.dat | 416–641 MB | 256 | 0x02030000 | 1…5 | 0 | 0 of 1 |
| client_cell_14.dat | 65,011,712 | 256 | 0x02030000 | 14 (0x0E) | 0 | 0 of 1 |
| client_cell_15.dat | 1,048,576 | 256 | 0x02030000 | 15 (0x0F) | 0 | 0 of 1 |
| client_map_1.dat … _5.dat | 22–23 MB | 256 | 0x020A0000 | 1…5 | 0 | 0 of 1 |
| client_map_14.dat | 23,068,672 | 256 | 0x020A0000 | 14 | 0 | 0 of 1 |
| client_map_15.dat | 1,048,576 | 256 | 0x020A0000 | 15 | 0 | 0 of 1 |

Sizes in B1 (2026-10-03) that differ from B2: anim 738,197,504; mesh_aux_1 1,672,478,720; sound_aux_1 3,902,799,872; surface_aux_2 2,640,314,368; highres_aux_3 566,231,040; local_English 1,906,171,856. All other sizes are equal in B1 and B2.

Observations (Confirmed, B2, all 29):

- Read as a u32, the most significant byte of 0x14C is the dataset type (**1** = shared game data, **2** = region data, **3** = language data) and the next byte is the archive kind; the low 16 bits are the variant number, 0 everywhere. The full meaning, including the client's table of kinds, is in DC §3.4.
- Every aux member has the same 0x14C, 0x150 and block size as its base.
- For a language archive 0x150 is the language code folded to 16 bits (DC §3.4): `local_English` has 2. Other labels the enumeration defines would carry EN_GB 0x1001, FR 0x1003, DE 0x1004 (DC; not observed as files).
- Version A had the same set of files, with different sizes for some of them.
- Regions are the keys of the RegionID DID map (DC §2.4): 1, 2, 3, 4, 5, 6, 14, 15 in this install. Region 6 has no archive files. Region 15 archives are 1 MiB placeholders in B1 and B2.

<a id="s3-3"></a>
### 3.3 Required and optional archives

This classification is a **project decision**, informed by the facts above. LC treats `anim` as optional and `highres` as optional (Reported). The original client fails to start when region 1 cannot be opened, but tolerates other missing regions (DC §2.4).

| Tier | Archives | If missing |
|---|---|---|
| **Core** | `general`, `gamelogic`, at least one `local_<Lang>` | The directory is **not** a usable install (§4) |
| **Content** | `mesh`, `surface`, `sound` (each with all its aux members), `anim`, every `cell_<r>` with its matching `map_<r>` | Usable, but flagged *incomplete*. The client warns, and subfile lookups that would land in the missing archive fail with a "missing archive" error, not a crash |
| **Optional** | `highres` with its aux members | Usable. High-resolution lookups fall back as if `--HighResOutOfDate` were given (§10) |

Region archives come in pairs: if `cell_<r>` exists, `map_<r>` should exist too (Confirmed for every observed region). An unpaired region archive is recorded but flagged.

<a id="s3-4"></a>
### 3.4 Discovering split-set members

Do not hard-code aux counts. They differ between versions and grow as content is added. For each base archive found:

1. Probe `client_<key>_aux_1.datx`, `_aux_2.datx`, … in order. Stop at the first index that does not exist, or at index 16, whichever comes first. Also list the directory for `client_<key>_aux_*.datx`; any match not reached by the probe (an index after a gap, an index above 16, or a malformed index) is reported as a stray file and is not opened. (The original client finds aux files both ways, DC §2.3.)
2. Read record 0xFFFF0005 from the base and from each member found (§9.2.4). The stamps must all be equal. Each member's index must equal its `<n>` (the base is index 0). Every member must report the same set count.
3. If the set count is larger than the number of files found, the set is **incomplete**. Record the missing indices.
4. If an aux file exists whose index is ≥ the set count, or whose stamp differs, it is a **stale leftover**. Exclude it from the set and log a warning. Patches can leave such files behind; see `backup/` in §3.6. Never delete it (the original client may delete such files in some modes, DC §2.3; we never write to the install).
5. If 0xFFFF0005 cannot be read (damaged archive), fall back to "every consecutive aux file found is a member" and mark the set *unverified*.

Invariant (Confirmed in A, B1 and B2): in a healthy install, step 2 holds for all four multi-member split sets and for every single-member archive (index 0 of 1).

<a id="s3-5"></a>
### 3.5 Language archives

- The language name is whatever follows `client_local_` and comes before `.dat`. It is the label of the language in the game's language enumeration: `English`, `EN_GB`, `FR`, `DE` (P33). Only `English` is present as a file (Confirmed 2026-10-05). Keep the name exactly as written, because it is also the value passed with `--language` (§10).
- An install may hold several languages. The default is the one named by `--language`. Otherwise use `English` if it is present. Otherwise use the first in lexicographic order.

<a id="s3-6"></a>
### 3.6 Other files in the install directory

All Confirmed (file listing of user install 2026-10-03, re-checked 2026-10-05) unless marked otherwise. Our client reads none of these at run time except where noted. They are listed so that validation and diagnostics can recognise them.

| Path (relative to `GAME_DIR`) | What it is |
|---|---|
| `x64/` | The 64-bit client: `lotroclient64.exe`, `lotrobrowserhelper64.exe`, and 64-bit copies of the middleware DLLs. The 64-bit client binary lives **only** here |
| `lotroclient.exe` | 32-bit client (legacy; its servers were shut down in Aug 2025 per NOTES; Reported) |
| `LotroLauncher.exe` | Official launcher (32-bit) |
| `lotro.launcherconfig` | Local launcher bootstrap config (§6) |
| `ProjectVersion` | 3-byte ASCII number (§7) |
| `patchcache.bin` | Patcher state. Contains the absolute install path as UTF-16. Never parse or write it |
| `unins000.dat`, `unins000.exe`, `uninstall_manifest.txt` | Inno Setup uninstaller files and a list of the folder/file patterns the uninstaller removes |
| `raw/<lang>/…` | Loose media outside the archives: `chargen/{class_videos,race_videos}/*.bik`, `introcinematic/*.bik`, `logo/*.bik` and `logo/*.jpg`, `vignette_videos/*.bik`. `<lang>` is lower-case `en`, `de`, `fr`; in this install `de` and `fr` hold only `logo/*.jpg` (Confirmed 2026-10-07). The client finds these files only through paths named in the language archive (§3.6.1, P49) |
| `en/`, `de/`, `fr/`, `themes/`, `locales/`, `browser/`, `*.png`, Qt and CEF files | Launcher UI and embedded browser resources |
| `backup/` | Previous copies of binaries saved by the patcher. **Never** treat it as an install. Skip it while searching (§5.8) |
| `bindat/`, `forward/`, `tmp/`, `temp/`, `downloading/`, `patches/` | Patcher working directories, empty or near-empty when idle (P42). `forward/` receives forward-patch iteration data during a patch; `downloading/` holds working files during the initial dat download (per its own `downloading.txt`) |
| `troubleshooting/`, `_CommonRedist/`, `3rdparty/`, `DXSETUP.exe`, `*.cab` | Redistributable installers |
| `icon.ico` | Product icon (OL uses it in its UI; Reported) |

The `uninstall_manifest.txt` file lists the folders and glob patterns the uninstaller removes; on this install they are `en fr de backup bindat browser forward locales patches raw themes tmp troubleshooting downloading x64` plus `*.bin *.dat *.datx *.dll *.log *.pak *.xml Awesomium*.exe lotro*.exe ProjectVersion TurbineServices.config` (Confirmed 2026-10-05). It is a convenient allow-list of what the publisher considers part of the install.

<a id="s3-6-1"></a>
#### 3.6.1 How the client uses `raw/`

The client has no built-in path into `raw/` (BIN-4, P49). Every file it opens there is named by
a string in the **language archive** of the selected language: string tables (0x25…), 0x22…
records and the language archive's 0x0E… record hold paths relative to the install directory,
of the form `raw/<lang>/<folder>/<file>`, with `/` as the separator. In this install the
English archive names 96 files, all present: 85 Bink videos (`.bik`) and 11 JPEG images
(`.jpg`) (Confirmed 2026-10-07). No other kind of file under `raw/` is named. So the answer
to the former OPEN is: the client reads from `raw/` only what the language data names, and in
this install that is videos and images only.

Rules for our client: never scan `raw/` to decide what exists; resolve a `raw/…` path only
when a decoded record names it, relative to `GAME_DIR` and case-insensitively (§2). A named file
that is missing is a warning for that one asset. The `de` and `fr` folders (logo images only) are
named by no installed archive (Confirmed 2026-10-07).

<a id="s3-7"></a>
### 3.7 Game-type identification

DDO uses the same engine and the same archive naming, so `client_general.dat` alone does not prove the directory is a LOTRO install (Reported: OL, PL). Decide the game type in this order:

1. The launcher config's file stem (`lotro.launcherconfig` → LOTRO; `ddo.launcherconfig` → DDO).
2. Otherwise, the `DataCenter.GameName` value in whichever launcher config was found (§6): `LOTRO` (historically also `LOTROEU`) or `DDO`.
3. Otherwise, the client binary present: `x64/lotroclient64.exe` or `lotroclient.exe` → LOTRO; a `dndclient*.exe` → DDO (PL; Reported).
4. Otherwise *unknown*. Our client accepts *unknown* with a warning.

Our client targets LOTRO. A DDO install is reported as "found, unsupported" and is not opened.

---

<a id="s4"></a>
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

The validation result is a structured report. A lone boolean is not enough. The report holds: core OK or not; the game type; per archive its path, size, header fields (§9.1) and status (ok, bad magic, size mismatch, unreadable); per split set its status (complete, incomplete with missing indices, unverified, with any stale members or stray files); languages found; and warnings.

---

<a id="s5"></a>
## 5. Locating installs

<a id="s5-1"></a>
### 5.1 Search order

Candidates are gathered from every source below in the order given, de-duplicated (§5.8), and validated (§4). The **first usable install in this order is the default**. All usable installs are returned so a UI can offer a choice.

1. **Explicit**: a `--game-dir` option of our own client or tools, if given. Nothing else is searched.
2. **Environment**: `TESSERA_GAME_DIR`, if set and non-empty. Nothing else is searched.
3. **Steam** (§5.3 Linux/macOS, §5.4 Windows).
4. **Third-party launcher configs**: OneLauncher (§5.6.2), then the `lotro-launcher` configs (§5.6.5).
5. **Standalone installer** paths and registry (§5.5).
6. **Wine/Proton/Lutris/CrossOver prefixes** (§5.6).
7. **Bounded scan** of the user's home games folders (§5.6.4).

If 1 or 2 names a directory that is not a usable install, report that as an error. Do not silently fall back to searching: the user asked for that directory.

<a id="s5-2"></a>
### 5.2 Paths inside a prefix

When a source yields a Windows path (registry value, `--prefs` argument, launcher setting, a path read from a log) and we run on Linux or macOS, translate it as follows:

- `C:\…` maps to `<prefix>/drive_c/…`.
- Other drive letters, including `Z:`, are resolved through the `<prefix>/dosdevices/<letter>:` symlinks when present (Confirmed on both prefixes on the machine: Steam's has `c:`, `s:` → the Steam root, `z:` → `/`; the third-party launcher's has `c:`, `x:` → the home directory, `z:` → `/`). Without a symlink, `Z:\…` maps to `/…` (Wine's default; Reported, from memory); any other letter is unresolvable, which is logged and skipped.
- Backslashes become `/`, and each component is matched case-insensitively (§2).

The client logs its own install path through such a mapping (for example under `S:/` on the Steam prefix), so a Windows-style path must never be assumed to be under `drive_c` (Confirmed 2026-10-05, LOG).

<a id="s5-3"></a>
### 5.3 Steam on Linux and macOS

**Steam roots** (try each; a root counts only if it exists):

| Root | Mark |
|---|---|
| `~/.local/share/Steam` | Confirmed (user machine) |
| `~/.steam/steam` (normally a symlink to the above) | Confirmed (symlink observed) |
| `~/.steam/root`, `~/.steam` | Reported (OL searches `~/.steam/steamapps`) |
| `~/.var/app/com.valvesoftware.Steam/.local/share/Steam` (Flatpak) | Reported (LUT, P51) |
| `~/.var/app/com.valvesoftware.Steam/data/Steam` (older Flatpak layout) | Reported (LUT, P51) |
| `~/snap/steam/common/.local/share/Steam` (Snap) | Reported (LUT, P51) |
| `~/.steam/debian-installation` (Debian's packaged Steam) | Reported (LUT, P51) |
| `~/Library/Application Support/Steam` (macOS) | Reported (OS) |

> **OPEN:** Confirm the Flatpak, Snap and Debian roots on a machine that uses them. They are now sourced from a published launcher's search list (LUT, P51) but not observed: neither exists on the user machine (2026-10-07). Needs the owner or another machine.

**Library list.** For each root, read `<root>/steamapps/libraryfolders.vdf`. If that file is absent, read `<root>/config/libraryfolders.vdf`. On the user machine both exist and are byte-identical (Confirmed 2026-10-05). The root itself is always a library, even if the file is missing.

**libraryfolders.vdf structure** (Confirmed; key names are needed for interoperability):

- Top-level key `libraryfolders`, whose value is a block.
- Inside it, numbered keys `"0"`, `"1"`, …, each a block with these entries:
  - `path`: the library root, a native path;
  - `label`, `contentid`, `totalsize`, `update_clean_bytes_tally`, `time_last_update_verified`;
  - `apps`: a block mapping an app-id string to a size string.
- Presence of `"212500"` in `apps` means the library *should* hold the game. The size may be `"0"` even when the game is installed (observed 2026-10-03 only; on 2026-10-05 the 212500 entry has a non-zero size, so the zero case is not reproducible now). **Use presence only, never the size.** Even when 212500 is not listed, still probe each library for the manifest (step below). The list can be stale.

**Older and transitional forms** (Reported, LUT, P52). Older files spell the top-level key `LibraryFolders`; the key comparison is case-insensitive anyway. In older files a numbered entry's value is a plain path string, not a block: treat that string as `path`. Some files carry a non-numbered entry `contentstatsid` beside the numbered ones: ignore every entry whose key is not a decimal number. None of these forms is present on the user machine (Confirmed 2026-10-05: one current-format file).

**App manifest.** In each library, look for `<library>/steamapps/appmanifest_212500.acf` (Confirmed). Its top-level key is `AppState`. Keys used:

| Key | Use | Observed value (2026-10-05) |
|---|---|---|
| `appid` | must equal `212500` | `212500` |
| `installdir` | folder name under `steamapps/common/`. Accept it only when it is one plain path component (no `/`, `\`, `:` or control character; not empty, `.` or `..`; not ending in `.` or a space; not a Windows device name); otherwise ignore it with a warning (§13, P66) | `Lord of the Rings Online` (**no leading "The"**) |
| `StateFlags` | bitmask; value 4 = fully installed (Steam convention; Reported) | `4` |
| `buildid` | Steam build number, a coarse version label (§9.5) | `12250101` |
| `InstalledDepots` | block of depot-id → {`manifest`, `size`} | **212501 and 212502 only** (P41). Redistributables are a separate Steam app, id 228980, which also appears in `libraryfolders.vdf`'s `apps` list |
| `UserConfig` → `language`, `MountedConfig` → `language` | Steam language choice | `english` |

The game directory is `<library>/steamapps/common/<installdir>`. If the manifest is missing but `apps` lists 212500, also try `<library>/steamapps/common/Lord of the Rings Online`.

**Depot contents** (Confirmed 2026-10-07 from Steam's cached depot manifests, DEP, P53). Depot 212501 holds the shared archives, including the high-resolution set as far as Steam ships it; depot 212502 holds the English language content (`client_local_English.dat`, `raw/en/…`, the launcher's `en/` resources). Two aux parts of this install (`client_mesh_aux_1.datx`, `client_highres_aux_3.datx`) are in neither depot: the launcher's patcher created them on the machine. Which depots carry the other languages was not observed. Still do not rely on depot ids: the install is identified by its files (§4), and the patcher adds files that no depot lists.

**Proton prefix of a Steam install** (Confirmed 2026-10-05): the compatdata directory of the game is `<library>/steamapps/compatdata/212500/`, in the library that holds the game. Inside it:

- `pfx/` is the prefix; its user is `steamuser`, so the settings folder is under `pfx/drive_c/users/steamuser/Documents/` (§8.1);
- `version` holds the Proton tool name as text (observed: a GE-Proton release);
- `pfx/dosdevices/` holds the drive-letter symlinks used by §5.2.

**VDF text syntax** (the subset these files use; Confirmed by observation, OS for the general shape):

- Tokens are double-quoted strings, `{` and `}`.
- A key token is followed either by a quoted value or by a `{ … }` block.
- Whitespace (spaces, tabs, CR, LF) separates tokens.
- A backslash inside a quoted string makes the next character literal: `\\` is one backslash and `\"` is a quote. These are the only two sequences in any VDF or ACF file on the user machine (P47); any other `\x` also reads as the character `x` (so `\n` reads as the letter `n`, never a newline).
- `//` starts a comment that runs to the end of the line (Reported, from memory).
- Keys are compared case-insensitively.

The parser must reject unbalanced braces and unterminated strings with an error, never a panic. It must limit nesting depth (16 is plenty) and file size (1 MiB is plenty).

<a id="s5-4"></a>
### 5.4 Steam on Windows

1. Find the Steam root from registry value `SteamPath` under `HKCU\Software\Valve\Steam`. Otherwise use `InstallPath` under `HKLM\SOFTWARE\WOW6432Node\Valve\Steam`, then the same key without `WOW6432Node` (Confirmed in prefix registry; OS).
2. From there, follow §5.3 (library list → manifest → `steamapps\common\<installdir>`).
3. In addition, read `InstallLocation` from `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Steam App 212500`, and from the same path under `SOFTWARE\WOW6432Node\…` (EL; Reported).
4. Fallback: `C:\Program Files (x86)\Steam\steamapps\common\Lord of the Rings Online` (path shape Confirmed via `installdir`).

`HKCU\Software\Valve\Steam\Apps\212500` holds DWORDs `Installed`, `Running` and `Updating` (Confirmed in the prefix registry). If `Updating` = 1, warn that Steam is patching the game at this moment.

<a id="s5-5"></a>
### 5.5 Standalone installer (Windows, or inside a prefix)

| Source | Detail | Mark |
|---|---|---|
| Default path | `C:\Program Files (x86)\StandingStoneGames\The Lord of the Rings Online` | Reported (KD) |
| Older default path | `C:\Program Files (x86)\Turbine\The Lord of the Rings Online` | Reported (inferred from the KD registry key name) |
| Registry | `HKLM\SOFTWARE\StandingStoneGames\The Lord of the Rings Online`, `HKLM\SOFTWARE\WOW6432Node\StandingStoneGames\The Lord of the Rings Online`, `HKLM\SOFTWARE\WOW6432Node\Turbine\The Lord of the Rings Online`; try value names `InstallLocation`, `GameFolder`, `Install Location` | Reported (KD) |
| Inno Setup uninstall key | `HKLM\SOFTWARE\[WOW6432Node\]Microsoft\Windows\CurrentVersion\Uninstall\<AppId>_is1`, with value `InstallLocation`. The uninstall log stores the AppId without braces, `12bbe590-c890-11d9-9669-0800200c9a66`, so the key is `12bbe590-c890-11d9-9669-0800200c9a66_is1` (P54). Also try `HKCU\…` with the same path (Inno writes there for per-user installs; INNO) | Confirmed (AppId form, 2026-10-07), Reported (key rule) |
| Program Files scan | Walk `C:\Program Files` and `C:\Program Files (x86)` under the §5.8 limits | Reported (OL, PL) |

Whether the Inno key name has braces is answered (no braces, P54). Look the key up case-insensitively anyway, and also accept a braced form `{12bbe590-…}_is1`; it costs nothing (a braced form was not observed).

> **OPEN:** Which of the KD value names (`InstallLocation`, `GameFolder`, `Install Location`) really exist under the publisher keys. They can only be read from a Windows registry with a standalone install, which is not available (the user's install is Steam's, and its Proton prefix registry has no publisher key, Confirmed 2026-10-07). Look them up with a case-insensitive match over the key's values and accept any of them. Needs the owner or another machine.
> **OPEN:** The historical Codemasters (EU) default install path. No published source found on 2026-10-07; the client and data carry no trace of it. Needs a historical install or a reliable published source. Until then, the Program Files scan of this table finds such an install.

<a id="s5-6"></a>
### 5.6 Wine, Proton, OneLauncher, Lutris, CrossOver

<a id="s5-6-1"></a>
#### 5.6.1 Prefix discovery

A directory `X` is a prefix if `X/drive_c` exists. It is a Proton-style wrapper if `X/pfx/drive_c` exists; then the prefix is `X/pfx` (OL; Confirmed for Steam Proton). In the third-party launcher's prefix (§5.6.5) `pfx` is a symlink back to `X` itself, and `X/drive_c` also exists (Confirmed 2026-10-05); after canonicalisation (§5.8) both readings are the same prefix. For each prefix found, search `drive_c/Program Files` and `drive_c/Program Files (x86)` under the §5.8 limits.

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
| `lotro-launcher` built-in prefix | `<XDG data>/lotro-launcher/prefix` (normally `~/.local/share/lotro-launcher/prefix`). Confirmed 2026-10-05 (§5.6.5) |

OL only scans the compatdata folder of the default Steam root (Reported, OL), so it would miss the prefix of a game installed in another library. That case was not observed: on the user machine the game, its manifest and `compatdata/212500` are all in the default root (Confirmed 2026-10-05). Searching the compatdata folder of **every** library is a project decision.

<a id="s5-6-2"></a>
#### 5.6.2 OneLauncher configs (Reported, OL)

OneLauncher keeps one TOML file per configured game at `<XDG data>/onelauncher/games/<game-id>/config.toml` (normally `~/.local/share/onelauncher/games/*/config.toml`). Top-level keys of interest:

- `game_directory`: the install directory;
- `game_settings_directory`: a custom settings folder, optional;
- `game_type`: `LOTRO` or `DDO`;
- `client_type`: `WIN64`, `WIN32` or `WIN32Legacy`;
- `high_res_enabled`;
- the table `[wine]` with `user_prefix_path`.

Use `game_directory` as a candidate, and `game_settings_directory` as the settings folder for that candidate. Parse only these keys and ignore the rest. Never write the file.

<a id="s5-6-3"></a>
#### 5.6.3 Lutris

Lutris keeps one YAML file per game (Reported, LUT, P50):

- **Location:** `<XDG config>/lutris/games/*.yml` (normally `~/.config/lutris/games/`). When `<XDG config>/lutris` does not exist, Lutris uses `<XDG data>/lutris/games/` instead (normally `~/.local/share/lutris/games/`). Search both.
- **Keys used** (all in the top-level `game` mapping, for games run with Wine): `exe` (the program to start, absolute or relative to the game directory), `working_dir` (directory the program starts in), `prefix` (the Wine prefix), `args`. Ignore every other key.

Rules: read each file (cap 64 KiB; a YAML parser limited to plain mappings and scalars; reject anything else without panicking). Add `prefix` as a prefix (§5.6.1). Take the directory of `exe` as a candidate, and its parent when that directory is named `x64`; take `working_dir` as a candidate too. Translate Windows-style paths through the prefix (§5.2). Validate every candidate (§4). Never write these files.

> **OPEN:** Confirm the Lutris location and keys on a machine with a Lutris-managed install (none on the user machine, 2026-10-07). Needs the owner or another machine. The `~/Games/*/` prefix scan remains as a fallback.

<a id="s5-6-4"></a>
#### 5.6.4 Bounded home scan

As a last resort, scan `~/games`, `~/Games` and each Steam `steamapps/common` folder for usable installs, to a depth of 3 (OL).

<a id="s5-6-5"></a>
#### 5.6.5 `lotro-launcher` configs (Confirmed 2026-10-05)

A third-party launcher found on the user machine keeps its files under directories named `lotro-launcher` (the directory names are interoperability identifiers of that program, not names of ours). Observed layout:

| Path | Content |
|---|---|
| `<XDG config>/lotro-launcher/config.json` (normally `~/.config/lotro-launcher/config.json`) | Launcher-wide settings (UI locale, log verbosity and similar). Not used by us |
| `<XDG config>/lotro-launcher/games/<id>/config.json` | One JSON object per configured game; `<id>` observed `lotro` |
| `<XDG config>/lotro-launcher/games/<id>/accounts.json` | Account data. **Never open it** |
| `<XDG config>/lotro-launcher/logs/` | The launcher's own logs. Not read |
| `<XDG data>/lotro-launcher/prefix/` | The launcher's own Proton-style prefix: `drive_c/`, `dosdevices/`, a `version` file naming a GE-Proton release, and `pfx` as a symlink back to the prefix itself (§5.6.1). Under `drive_c/users/` it has `Public`, a real `steamuser` directory, and an entry named after the host user that is a **symlink to `steamuser`**, so both names reach the same folder |

Keys of a per-game `config.json` (Confirmed: names and value types; JSON, no BOM):

| Key | Type | Use | Observed |
|---|---|---|---|
| `gameDirectory` | string | install candidate (native path) | the Steam install of §5.3 |
| `settingsDirectory` | string | settings folder for that candidate (native path) | `<prefix>/drive_c/users/<host user>/Documents/The Lord of the Rings Online` inside the launcher's own prefix, **not** the Steam prefix. The `<host user>` component is the symlink to `steamuser` described above, so the canonical folder is under `steamuser` |
| `gameType` | string | game type hint (§3.7) | `LOTRO` |
| `clientType` | string | which client executable the launcher starts | `Win64` |
| `highResEnabled` | boolean | whether high-resolution data is in use | `true` |
| `locale` | string | language selection | `en` |
| `id`, `newsfeedUrl` | string | not used | — |

Rules: read each `games/*/config.json` (cap 64 KiB, reject anything that is not a JSON object), use `gameDirectory` as a candidate and `settingsDirectory` as that candidate's settings-folder override (§8.1 step 2), ignore every other key, and never write any of these files.

So the same install can have **two** settings folders on one machine, one per launcher prefix (Confirmed: the Steam prefix and this launcher's prefix each hold a `UserPreferences.ini`). §8.1 decides which one our client uses.

**Origin** (Confirmed 2026-10-07, LL, P55): `lotro-launcher` is the project owner's own GPL-3 launcher (https://github.com/solarbaron/lotro-launcher), installed as version 0.1.7. Its per-game `gameType` values are `LOTRO`, `LOTRO_Preview`, `DDO` and `DDO_Preview` (project decision: treat a `*_Preview` value as its base game for §3.7).

**Custom prefix** (Reported, LL source at commit f880b07): the program has Wine settings, among them a prefix mode (`Builtin` or `User`) and a user prefix path (JSON names `prefixMode`, `userPrefixPath`), but that version writes them to no file. So no persisted custom-prefix key exists today, and the built-in prefix of §5.6.1 is the only one to search. If a per-game `config.json` ever carries `userPrefixPath` as a string, add it as a prefix (§5.6.1); this costs nothing and needs no other change.

<a id="s5-7"></a>
### 5.7 Security and privacy of discovery

- Discovery reads only metadata files, directory listings and the first 0x400 bytes of archives. It must not send anything over a network.
- Registry and file reads that fail (no access, missing) are skipped silently at debug log level.
- Files known to hold account data (`accounts.json` of §5.6.5, launcher logs) are never opened.

<a id="s5-8"></a>
### 5.8 Search limits and de-duplication

- Maximum recursion depth is 5 below a search root (OL), and 3 for the §5.6.4 scan.
- Skip directories whose name starts with `.` (except an explicitly named prefix root), and directories named `dosdevices`, `backup` or `compatdata` (case-insensitive), in **every** walk. `dosdevices` contains drive symlinks that would loop back to `/`; compatdata prefixes are enumerated separately (§5.6.1).
- Do not follow directory symlinks while recursing (this also stops the self-referencing `pfx` link of §5.6.5). Do follow them when they are an explicit candidate, e.g. `~/.steam/steam`.
- Cap the directories visited at 20,000 **per walk root** (each `Program Files` folder of each prefix, each home-scan root), and log when the cap is hit. The total work is bounded by the number of roots times the cap.
- Two candidates are the same install if their canonical (symlink-resolved) paths are equal. Keep the first one found and record every source that named it.

---

<a id="s6"></a>
## 6. `lotro.launcherconfig`

Format (Confirmed 2026-10-05): UTF-8 **with BOM**, CRLF line ends, XML declaration, root element `configuration`. The element names present are:

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

The game client reads neither `Product.DocumentFolder` nor `Launcher.DocumentFolder`: neither name occurs in it (BIN-4, P56). It uses the fixed folder name `The Lord of the Rings Online` (§8.1, P57). Both keys belong to the official launcher.

> **OPEN:** Where the official launcher resolves `Launcher.DocumentFolder` (value `Launcher`). No `Launcher/` subfolder was present in either settings folder on the user machine, and the launcher's run-time logs go to `%LOCALAPPDATA%\The Lord of the Rings Online\` (§8.2). Only the launcher uses it, and the launcher is excluded from M4 (decision 0011), so this stays open; our client does not need it.

---

<a id="s7"></a>
## 7. `ProjectVersion`

Confirmed: a file of exactly 3 bytes, ASCII digits `120`, with no line terminator. It is identical in A, B1 and B2, so it does **not** identify a content version.

Our parser reads at most 64 bytes, trims ASCII whitespace, and accepts 1–10 ASCII digits as an unsigned integer. Anything else is reported as *malformed*, which is not an error. The value is recorded in the fingerprint report (§9.4) as metadata only.

The game client never reads `ProjectVersion`: the name occurs nowhere in it (BIN-4, P56). The file is listed in the uninstaller's manifest (§3.6), so it belongs to the launcher and patcher side.

> **OPEN:** What `ProjectVersion` counts. It is `120`, which is **not** either datpack version (the archives are eng 112 / game 3, §9.1, P36). Only the launcher or patcher can use it, and both are excluded from M4 (decision 0011). Our client records it as metadata only (above), so nothing depends on the answer.

---

<a id="s8"></a>
## 8. Per-user settings folder

<a id="s8-1"></a>
### 8.1 Location

**What the official client does** (BIN-4, P57, P58). It decides on a preferences **file** first, and the settings folder is the directory that holds that file:

- the `--prefs` value, if given (§10);
- otherwise `UserPreferences.ini` in the process's current directory, if that file exists there;
- otherwise `<Documents>\The Lord of the Rings Online\UserPreferences.ini`, where `<Documents>` is the shell's personal-documents folder. The folder name is fixed in the client; it does not read the launcher config (§6).

Plugins (`Plugins`), player music (`Music`) and screenshots (§8.2) are then placed relative to that folder. The run-time logs go elsewhere, to `<local application data>\The Lord of the Rings Online\` (§8.2).

**Our client.** Let `NAME` be `The Lord of the Rings Online`, the name the official client uses. If the launcher config's `Product.DocumentFolder` (§6) holds a different value, log it at info level; it does not change `NAME`. Resolve the settings folder in the order of the steps below. The step labels are stable identifiers that code and other specs cite; step 1a sits between steps 1 and 2.

- **Step 1.** If our client is started with `--prefs <file>`, the folder is the parent directory of `<file>` (P32; BIN). The preferences file is `<file>` itself, which need not be called `UserPreferences.ini`.
- **Step 1a.** If our client's current working directory holds a file named `UserPreferences.ini` (case-insensitive, §2), the folder is that directory, as in the official client (P57). Project decision: this step is skipped when the working directory is the filesystem root or the user's home directory, where such a file would more likely be an accident than a portable setup.
- **Step 2.** A per-install override from the source that found the install: OneLauncher `game_settings_directory` (§5.6.2) or `lotro-launcher` `settingsDirectory` (§5.6.5).
- **Step 3.** Native Windows: `<Documents>/NAME`, where `<Documents>` is the user's known Documents folder. It may be redirected (OneDrive, roaming profiles), so use the shell's known-folder lookup, not `%USERPROFILE%\Documents`.
- **Step 4.** Install inside a prefix (Steam Proton, Wine, Lutris, a launcher's own prefix): `<prefix>/drive_c/users/<user>/Documents/NAME`. For a Steam compatdata prefix `<user>` is `steamuser` (Confirmed). For other prefixes try the host user name first, then every other entry under `drive_c/users/` except `Public`, in lexicographic order. In the `lotro-launcher` prefix the host-user entry is a symlink to `steamuser`, so both names lead to the same folder (Confirmed 2026-10-05, §5.6.5). If `Documents` is a symlink into the host home, follow it (P19; Reported).
- **Step 5.** Native Linux run of our client against a Steam/Proton install: use the prefix folder from step 4, so that plugins and preferences are shared with the official client.
- **Step 6.** Otherwise: `$XDG_DOCUMENTS_DIR/NAME` if set, else `~/Documents/NAME`, else `~/NAME` (P19).

**De-duplication.** Settings-folder candidates are de-duplicated by canonical (symlink-resolved) path, exactly as installs are in §5.8: when two candidates resolve to the same folder, keep the first one in the order above and drop the later one. A path that does not exist is compared by the canonical path of its nearest existing ancestor followed by the remaining components. This applies within step 4 (the host-user symlink case) and across steps (for example a step-2 override that names the same folder that step 4 finds).

When several distinct candidates exist, prefer the one that already contains a `UserPreferences.ini`. When steps 1–5 (including 1a) yield no existing folder, the folder our client **proposes** (for creating its own files) is the first step-6 path (`$XDG_DOCUMENTS_DIR/NAME`, else `~/Documents/NAME`), never a path inside the install or a prefix.

<a id="s8-2"></a>
### 8.2 Contents

Observed contents (Confirmed 2026-10-05): the Steam-prefix settings folder holds `CEF/`, `PluginData/`, `UserPreferences.ini` and `lotro.keymap`; the `lotro-launcher` prefix's settings folder holds `PluginData/` and `UserPreferences.ini`.

| Path (relative to settings folder) | Content | Mark |
|---|---|---|
| `UserPreferences.ini` | Client and launcher preferences (§8.3). The file name is fixed when `--prefs` is not given (OL) | Confirmed |
| `lotro.keymap` | Key bindings, written by the client; format in `keymap.md` §12. `--keymap` names an alternative file to load (§10). The meta-key companion files that the help text mentions are not read by build 4906.0071.1794.4035 (KM K31) | Confirmed (presence) |
| `Plugins/` | Lua plugins: `Plugins/<Author>/<Name>.plugin` descriptor files plus package folders; also `.plugincompendium`, `.skincompendium`, `.musiccompendium` manager files. Absent in both settings folders on the machine (2026-10-05). The client builds the folder path as the settings folder plus `Plugins` (P58) | BIN (folder name); Reported (contents) |
| `PluginData/<account>/<world>/…` | Saved plugin data. Observed: `PluginOptions.xml` at the world level, under several worlds (P22) | Confirmed |
| `PluginData/…/*.plugindata` | Data saved by plugins per scope (account, server, character); scope folder names are owned by `lua-plugin-api.md` §10.3 | Reported |
| `CEF/` | In-game store browser (CEF) cache and user data: `CachePath/`, `UserData/`. Not read by our client | Confirmed |
| `Music/*.abc`, `Music/*.txt` | Player-music files for `/play`. A requested name is looked up as `Music/<name>.abc` first, then `Music/<name>.txt` (P58) | BIN |
| `ui/skins/<Skin>/SkinDefinition.xml` + TGA images | Custom UI skins, read at client start | Reported |
| `ScreenShot_<YYYY-MM-DD_HHMMSS>_<n>.jpg` | Screenshots, written directly into the settings folder (no sub-folder). The time stamp is the local time of the shot; `n` is the smallest of 0 … 99 for which no such file exists yet (P58). Not read by our client | BIN |
| chat logs | Written by the client | Reported |

**Run-time logs are not in this folder.** The client, launcher and patcher write their own logs to `<prefix>/drive_c/users/<user>/AppData/Local/The Lord of the Rings Online/` (i.e. `%LOCALAPPDATA%\The Lord of the Rings Online\`): `lotroclient64.log`, `LotroLauncher*.log`, `PatchClient.log`, `TTEPatchClient.log` (Confirmed 2026-10-05, P38). Our client never reads these; they are listed so diagnostics can point a user at them. Do not parse or copy them (the launcher log contains account and login detail).

Screenshots are answered above (BIN-4): the client writes them as JPEG files directly into the settings folder, not into the prefix's `Pictures/Screenshots/` folder, which is therefore not the client's target.

> **OPEN:** The chat-log file names and folder. The client binary contains no chat-log file name or `.txt` log pattern (BIN-4, 2026-10-07), so the name comes from data, from UI script, or from code that was not found; and no chat log exists on the machine. Needs an owner observation: turn chat logging on in the client's options, chat briefly, and record the new file's folder (relative to the settings folder) and the shape of its name (date format, no text).

<a id="s8-3"></a>
### 8.3 `UserPreferences.ini`

**Format** (Confirmed 2026-10-05, both files on the machine): UTF-8 **without** a BOM, `[Section]` headers, `Key=Value` lines with no spaces around `=`, ASCII content. The file the official launcher and client write uses CRLF line ends and a trailing newline (P40); the file under the `lotro-launcher` prefix uses LF line ends and ends with an empty line (P45). Values are untyped text. Observed value shapes: `True`/`False` (also lower-case `true`/`false` in `[Launcher]`); decimals with two places (`0.30`, `1.00`); integers; `WxH` resolutions (`2880x1620`); a packed integer (`WindowedResolution`, below); enumeration words (`FullScreenWindowed`, `D3D11`, `Anisotropic`); ISO-8601 timestamps in `[User_N]` (`2026-10-04T22:11:23`); hyphenated GUID-like device identifiers in `[Voice]`; free text with spaces (`[Sound] FocusBehavior`, `SoundFeatures`).

**Reading rules:**

- Accept a leading UTF-8 BOM or none, and CRLF or LF line ends (a lone CR is not a line end).
- Section and key names are compared **case-insensitively**; lower-cased keys are reported in files rewritten by third-party launchers (P21).
- If a key repeats within a section, the last one wins.
- Blank lines, and lines without `=` outside a section header, are ignored.
- Cap the file at 1 MiB.

**Writing rules** (when our client saves preferences):

- Preserve every section and key it does not own, in the original order, with the original case.
- Write UTF-8 **without** a BOM, CRLF line ends and a trailing newline, to match what the official client writes (P40).
- Never touch `[Launcher]`, `[User_N]` or `[General]`.

> **OPEN:** Whether our client shares this file with the official client, or keeps its own file and only imports from it. This is a project decision; until it is made, our client treats the official file as **read-only** and writes its own.

**Sections and keys observed** (Confirmed; key names only):

| Section | Keys |
|---|---|
| `[Launcher]` (written by the official launcher) | AllowMultipleInstances, FirstRun, UseHighResolutionGameData, WindowGeometry, PreCache, SelectLastPlayedWorld, **GameClientType** (observed `3` on a 64-bit install; selects which client executable the launcher starts, P39) |
| `[User_N]` (launcher; numbered per account) | UserName, WorldName_N, LastPlayed_N (ISO-8601), UserFileName_N and UserAckTime_N (EULA/ToS acknowledgement records) (P39) |
| `[General]` (only in the `lotro-launcher` prefix's file, P45) | LastWorld |
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
| `[UI]` | CharSelectSortType, FloatyTextOnTop, FloatyTextOpacity, FloatyTextScale, HotspotVisualizationColorOverride_Boon, HotspotVisualizationColorOverride_Harm, RadarMovementSmoothing, WebstoreExternalWindow, UIScale_Cursor, UIScale_Global, UIScale_Pregame, MapQuestListXScreenPercentage, MapQuestListYScreenPercentage, MountAppearanceShowPlayerMounted, Bag1Columns … Bag6Columns, BankRepositoryAccountItemDisplayShowText, BankRepositoryCharacterItemDisplayShowText, BankRepositorySharedHousingItemDisplayShowText, BankRepositoryStandardHousingItemDisplayShowText, StatTreeShowAllStats |
| `[Voice]` | DefaultGroupVolume, DeviceCapture, DevicePlayback, HandsFree, MicCaptureThreshold, MicVolume, NetworkLatencyThresh, VoiceVolume |

Known values (Reported unless marked):

- `[Render] GraphicsCore` is one of `D3D9`, `D3D10`, `D3D11`, `OpenGL` (`D3D11` Confirmed in both files).
- `[Display] ScreenMode` includes `FullScreen` and `FullScreenWindowed` (both Confirmed).
- `[Display] Resolution` is a `WxH` string (`2880x1620` and `1920x1080` Confirmed), but `[Display] WindowedResolution` is a single unsigned integer, not a `WxH` string. Observed (Confirmed): `167773578` (0x0A00058A) with `Resolution=2880x1620`, and `83756426` (0x04FE058A) with `Resolution=1920x1080`.
- **`WindowedResolution` packing** (BIN-4, P59): the value is the window's client-area **width** in pixels times 65536 plus its client-area **height** in pixels (width in the high 16 bits, height in the low 16 bits). The client stores a new value when the windowed client area changes size. The observed values decode to 2560 × 1418 (with `Resolution=2880x1620`) and 1278 × 1418 (with `Resolution=1920x1080`). The second is narrower than its `Resolution` but **taller** (1418 > 1080), so the stored value is **not bounded by `Resolution`** in either dimension. A reader accepts any width and height from 0 to 65535 as stored and does not reject or clamp the value because it exceeds `Resolution`. Worked example (synthetic): width 1600 and height 900 give 1600 × 65536 + 900 = 104,858,500 (0x06400384).
- `[Render] D3DVersionPromptedForAtStartup` = `11` is present in both preferences files on the machine (Confirmed 2026-10-05). OL writes this value when it creates a file (Reported).
- The launcher records the client type in `[Launcher] GameClientType`, observed `3` on this 64-bit install (P39). It selects among the WIN64 / WIN32 / WIN32Legacy client executables.

> **OPEN:** The full key list, value ranges and defaults. The client registers each preference in the code of the subsystem that owns it, as `Section.Key` with a type and a default (BIN-4); there is no single table, and transcribing the registrations would copy a bulk table out of the client (decision 0012). Keys appear in the file only after the client has saved them. A writer must therefore preserve every key it does not own (above). Needs a decision on whether a key list is wanted at all; until then this stays open.
> **OPEN:** The `GameClientType` enumeration (which integer is WIN64, WIN32 and WIN32Legacy). Only `3` on a 64-bit install was observed. The game client never reads the key (BIN-4, P56); only the launcher does, and the launcher is out of M4 scope ([decision 0011](../decisions/0011-standing-m4-authorisation.md)). Treat `GameClientType` as opaque: read it, do not depend on the specific value. Needs the owner (switch the client type in the official launcher and note the stored value) if it is ever needed.

---

<a id="s9"></a>
## 9. Version fingerprint

<a id="s9-1"></a>
### 9.1 Header fields used

All offsets are absolute file offsets. All integers are little-endian. Only these fields are needed here; [`dat-container.md`](dat-container.md) §3 owns the header and gives the meaning of every field.

| Offset | Size | Type | Meaning | Changes between A and B? |
|---|---|---|---|---|
| 0x000 | 256 | bytes | version string area; empty (first byte 0) in every observed archive | no |
| 0x100 | 4 | bytes | `00 50 4C 00` in every observed archive | no |
| 0x140 | 4 | u32 | magic, must be 0x00005442 | no |
| 0x144 | 4 | u32 | block size (§3.2) | no |
| 0x148 | 4 | u32 | file length, must equal the real length | yes (layout) |
| 0x14C | 4 | u32 | archive variant, kind and dataset type (§3.2; DC §3.4) | no |
| 0x150 | 4 | u32 | region number or folded language code (§3.2; DC §3.4) | no |
| 0x160 | 4 | u32 | root directory node offset | yes (layout) |
| 0x170 | 4 | u32 | 0x28000000 in `general`, 0 elsewhere (master DID map, DC §3.3) | no |
| 0x174 | 4 | u32 | **"eng" datpack version** (container version, DC §3.3); 112 in all archives (P36) | no |
| 0x178 | 4 | u32 | **"game" datpack version** (game data version, DC §3.3); 3 (shared-data and language families) or 0 (region family) (P36) | no |
| 0x17C | 16 | bytes | a 16-byte identifier, identical in 27 of 29 archives; all zero in `highres_aux_3` and `mesh_aux_1` (DC §3.3: opaque, never compared by the client) | no |
| 0x18C | 4 | u32 | 1100 in shared-data and language archives and in `cell_1`/`map_1`; 17139 in the other region archives; 0 where 0x17C is zero | no |
| 0x190 | 12 | 3 × u32 | 104, 32, 8 in every archive | no |
| 0x19C | 4 | u32 | total free payload bytes (DC §3.3) | yes |

Fields marked "layout" change whenever the patcher moves data. Two installs with the same content can differ there. They are **not** part of the content fingerprint. The values in the last column compare A with B1; all values except sizes and layout fields are the same in B2 (Confirmed 2026-10-05).

The client's own log reports the two datpack versions for every archive it opens, and the numbers equal the bytes at 0x174 and 0x178 exactly; for the executable itself it reports −1 for both, meaning it accepts any value (P36, LOG). `ProjectVersion` (120, §7) is a different number and is not either datpack version.

<a id="s9-2"></a>
### 9.2 Housekeeping records

These are ordinary subfiles found by DID lookup in the archive's directory (DC). All the entries observed were stored uncompressed, but readers must honour the compression flag as for any subfile. Integers are little-endian. "stamp" means 16 opaque bytes, compared bytewise, never interpreted. All layouts are Confirmed (user install 2026-10-03, A and B1; 2026-10-05, B2, all 29).

**Which archives carry which record** (Confirmed 2026-10-05):

| Record | Present in |
|---|---|
| 0xFFFF0001 | every archive |
| 0xFFFF0003 | every archive except `highres_aux_3`, `mesh_aux_1` |
| 0xFFFF0004 | `general`, `gamelogic`, `anim`, `mesh`, `sound`, `surface`, `highres`, `local_*`, `cell_1`, `map_1` |
| 0xFFFF0005 | every archive |
| 0xFFFF0008 | every base (`.dat`) archive; no `.datx` |

<a id="s9-2-1"></a>
#### 9.2.1 0xFFFF0001: iteration record

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | i32 | `N`: number of iterations listed (≥ 0) |
| 4 | var | i32 tokens | run list, decoded as in §9.3 |
| after runs | 4 | u32 | `K`: number of trailing entries (observed 11 when `N` > 0, and 0 when `N` = 0) |
| +4 | 12 × K | — | `K` entries, each a u32 iteration number followed by a u64 opaque value (DC §7.3) |

Observed sizes (Confirmed, B2, all 29): 148 bytes (one run; cell_3–5, cell_14, cell_15, map_3–5, map_14, map_15), 152 bytes (a single then a run; cell_2, map_2), 156 bytes (two runs; the eight shared-data and language bases, cell_1, map_1), 284 bytes (`highres_aux_3`: 29 runs, of which 22 are singles), 3,192 bytes (`mesh_aux_1`: 652 runs, of which 541 are singles), and 8 bytes (`N` = 0, `K` = 0: `highres_aux_1`, `highres_aux_2`, `sound_aux_1`, `surface_aux_1`, `surface_aux_2`). In B1, `highres_aux_3` was 288 bytes with 30 runs and `mesh_aux_1` 3,196 bytes (P43). Record sizes are a snapshot; never hard-code them.

**`stamp` rule** (BIN, P61, from DC §7.3): `K` and the trailing entries are present only when the directory entry's `stamp` (DC §5.2) is 2. With `stamp` 1 the record ends right after the run list; the client refuses `stamp` 3 or more. Every 0xFFFF0001 in the install has `stamp` 2 (Confirmed 2026-10-05). A reader that sees `stamp` 1 decodes the runs only and treats `K` as 0; for `stamp` ≥ 3 it reports the record as malformed (§9.3).

> **OPEN:** What the u64 in each trailing entry means. It is not a Windows FILETIME. The client copies it verbatim and no game-client code interprets it (BIN-4, P61); its 64 bits look uniformly random (66 entries of six archives, 2026-10-07). It is written by the patcher, which decision 0011 excludes from M4, so the client cannot answer this. Not used in the fingerprint; nothing depends on it.

<a id="s9-2-2"></a>
#### 9.2.2 0xFFFF0003: install version stamp

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 16 | stamp | stamp S1 |
| 16 | 16 | stamp | stamp S2 |
| 32 | 1, 2 or 4 | count | label length `L`, a variable-length count (below); observed always 1 byte, value 4 |
| 32 + size of the count | L | ASCII | a short decimal label: `5758` in shared-data/language archives and in `cell_1`/`map_1`; another value shared by each other `cell_<r>`/`map_<r>` pair |

**Label length** (BIN-4, P60). The count is read like this (first byte `a`):

| First byte `a` | Count size | Value |
|---|---|---|
| 0x00–0x7F | 1 | `a` |
| 0x80–0xBF | 2 | (`a` − 0x80) × 256 + the second byte |
| 0xC0–0xFF | 4 | (`a` − 0xC0) × 2²⁴ + the second byte × 2¹⁶ + bytes 3 and 4 read as a little-endian u16 |

This is the VLE of [`records.md`](records.md) §1.2 without its 5-byte `0xE0` form: here `0xE0` starts a 4-byte count like any other byte from 0xC0. Worked example (synthetic): bytes `80 85` give 133; bytes `C0 01 34 12` give 0x00011234 = 70,196.

**What the client does with the label** (BIN-4, P60): it stores it with the two stamps and marks the archive's version stamp **invalid** when the label contains the ASCII text `INVALID_VERSION` anywhere (a case-sensitive substring test; the text is an interoperability marker, quoted for that reason). No observed label contains it.

Size observed: exactly 37 bytes in all 27 archives that carry it (P46). The S1/S2 pair is **the same in every archive that has the record**, and it changed between A and B1 (P8).

Reader rule: decode `L` as above. The record is well-formed when 32 + the count size + `L` equals the record size and every label byte is in 0x21–0x7E. Otherwise it is malformed, which is reported and treated as if the record were absent. For every record in the install (count size 1, `L` = 4, size 37) this gives the same result as the earlier rule "`L` < 0x80 and 33 + `L` = size", so fingerprints (§9.4) do not change. A label containing `INVALID_VERSION` is well-formed but is reported as an invalid version stamp (warning).

> **OPEN:** What the decimal label counts (`5758` here; it did not occur in the client's log or binary). Needs a second content version to compare, or a published source.

<a id="s9-2-3"></a>
#### 9.2.3 0xFFFF0004

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | i32 | `M` (> 0) |
| 4 | 4 | i32 | −`M` |
| 8 | 4 | i32 | 1 |

Exactly 12 bytes wherever present (P46). Unchanged between A and B1. No code of the game client loads it (DC §7.5, BIN there; P61). Not used in the fingerprint.

> **OPEN:** Meaning. No code of the game client uses it; the archives are written by the patcher, which is excluded from M4 (decision 0011), so the client cannot answer this (DC §7.5). Nothing depends on it.

<a id="s9-2-4"></a>
#### 9.2.4 0xFFFF0005: split-set membership

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 16 | stamp | set stamp: equal for all members of one split set; changed between A and B1 |
| 16 | 4 | u32 | member index (0 for the `.dat` base, `n` for `_aux_<n>`) |
| 20 | 4 | u32 | member count of the set (1 for unsplit archives) |

Size: exactly 24 bytes in all 29 archives (Confirmed 2026-10-05, P46). Reader rule: a record shorter than 24 bytes is malformed; a longer one is accepted, its first 24 bytes are used, and the extra length is logged at debug level.

<a id="s9-2-5"></a>
#### 9.2.5 0xFFFF0008

16 bytes: one stamp, identical in every archive that has it, in A, B1 and B2. Not used in the fingerprint.

Meaning (BIN, DC §7.5, P61): the archive's **product identifier**, a GUID that the archive library stores and returns on request. Opening an archive does not compare it with anything. A reader treats it as opaque.

<a id="s9-3"></a>
### 9.3 Decoding the iteration run list

Read the i32 `N`. Keep a counter `remaining` = `N`. While `remaining` > 0:

- Read an i32 token `t`.
- If `t` < 0, it is a **run**: its length is −`t`; read the next i32 as `start`. The run covers the iterations `start` … `start` + length − 1. Subtract the length from `remaining`.
- If `t` ≥ 0, it is a **single** iteration `t`. Subtract 1 from `remaining`.

Then read `K` and the `K` trailing entries.

The record is **malformed** (report it, do not panic) when any of these happen:

- `N` < 0;
- `t` = −2,147,483,648 (its length cannot be represented as a positive i32);
- a run's `start` < 0, or `start` + length − 1 > 2,147,483,647;
- `remaining` would go below 0;
- the record ends early;
- `K` > 64;
- bytes remain after the trailing entries;
- the directory entry's `stamp` is 3 or more (§9.2.1). With `stamp` 1 there is no `K` and no trailing list; the record ends after the runs.

Bound the number of tokens by the record size (at most (size − 8) / 4).

Derived values:

- `N`, the iteration count;
- `HI`, the highest iteration: the largest of `start` + length − 1 over all runs and of the singles, or none when `N` = 0;
- `RUNS`, the number of runs plus singles.

Observed (Confirmed, B2, all 29; A and B1 where stated):

- Every list with `N` > 0 starts at iteration 1. The eight shared-data and language bases and `cell_1`/`map_1` have two runs: one starting at 1, and a second that starts exactly 10000 above the last iteration of the first run (example: `general` has 1…3630 and 13630…20167). `cell_2`/`map_2` have the single 1 followed by a run starting at 10001, the same pattern with a first run of length 1. Regions 3, 4, 5, 14 and 15 have one run starting at 1.
- `highres_aux_3` and `mesh_aux_1` have many runs and singles.
- `N` and `HI` grew from A to B1 in every archive with `N` > 0. Example: `general` went from `N` 9770, `HI` 19769 to `N` 10168, `HI` 20167 (unchanged in B2).
- `K` = 11 whenever `N` > 0, and the trailing list holds the `K` highest listed iterations in ascending order. Decoders must still accept any `K` from 0 to 64.

<a id="s9-3-1"></a>
#### 9.3.1 Worked example (synthetic)

A synthetic record of 52 bytes:

| Offset | Bytes (hex) | Read as | Effect |
|---|---|---|---|
| 0 | `06 00 00 00` | `N` = 6 | `remaining` = 6 |
| 4 | `FD FF FF FF` | `t` = −3 | a run of length 3 … |
| 8 | `01 00 00 00` | `start` = 1 | … covering 1, 2, 3; `remaining` = 3 |
| 12 | `0A 00 00 00` | `t` = 10 | single 10; `remaining` = 2 |
| 16 | `FE FF FF FF` | `t` = −2 | a run of length 2 … |
| 20 | `1A 27 00 00` | `start` = 10010 | … covering 10010, 10011; `remaining` = 0, stop |
| 24 | `02 00 00 00` | `K` = 2 | |
| 28 | `1A 27 00 00` + 8 bytes | entry 1 | iteration 10010, opaque u64 |
| 40 | `1B 27 00 00` + 8 bytes | entry 2 | iteration 10011, opaque u64 |

Result: `N` = 6, `HI` = 10011, `RUNS` = 3, no bytes left over, and the trailing list holds the two highest listed iterations in ascending order.

Malformed variants: the same record with `N` = 2 (the first run would take `remaining` to −1); the same record cut after byte 22 (ends early); the same record with one extra byte at the end (bytes remain).

<a id="s9-4"></a>
### 9.4 Fingerprint algorithm

The fingerprint has three levels. Levels 0 and 1 are lists of per-archive tuples, sorted by file name in lower case, and are hashed. Level 2 is a diagnostic report and is not hashed.

**Level 0, schema identity** (header only; cheap). Per archive whose header could be read: lower-case file name; 0x14C; 0x150; 0x174; 0x178; block size (0x144); the 0x17C bytes; 0x18C. Use this level to check that an install is the kind of data our decoders expect (e.g. 0x174 = 112). It is the same across content updates (P6).

**Level 1, content version** (one directory lookup per housekeeping record). Per archive listed in level 0:

- lower-case file name;
- `N` and `HI` from 0xFFFF0001;
- the set stamp, index and count from 0xFFFF0005;
- the S1/S2 pair and the decimal label from 0xFFFF0003, if the record is present and well-formed.

Also one install-wide value: the S1/S2 pair, which must be identical across archives (invariant 6). It is *uniform* (one pair), *mixed* (pairs differ) or *none* (no archive has a well-formed record).

**Level 2, deep** (optional; walks every directory). Per archive: the number of directory entries, the highest entry timestamp and the highest entry iteration. Per split set (an unsplit archive is a set of one), with `L` the base archive's 0xFFFF0001 list and `HI` its highest value:

- **unlisted**: the number of entries, in any member, whose iteration is non-zero and not in `L`;
- **too new**: the number of entries, in any member, whose iteration is ≥ `HI` (or non-zero when `L` is empty).

In a healthy install both numbers are 0 for every set (invariant 9, Confirmed B2). A non-zero number is reported as "inconsistent iteration state", for example after an interrupted patch. (That an interrupted patch produces this state is an inference, Reported; the original client records the iteration that last wrote each entry, DC §5.2.) Level 2 never makes an install unusable.

<a id="s9-4-1"></a>
#### 9.4.1 Serialisation (project convention)

The serialisation is our own convention. It must stay stable, because caches key on it; any change is a breaking change of every cache.

- Text is UTF-8 (in practice ASCII). Every line, including the last, ends with a single LF (0x0A). Fields are separated by a single space.
- Lines are sorted in ascending byte order of the lower-cased file name.
- Integers are unsigned decimal with no leading zeros (header words are written as their u32 value, e.g. 0x01030000 is `16973824`).
- Stamps and the 0x17C bytes are 32 lower-case hex digits, in byte order.
- An unavailable value is written as `-`.

**Level-0 line**, one per archive whose header was readable (others are left out): `name 0x14C 0x150 0x174 0x178 blocksize 0x17C 0x18C` (eight fields). An install with no readable header gives empty text.

**Level-1 lines**, one per level-0 archive: `name N HI setstamp setindex setcount`, followed by ` S1 S2 label` only when 0xFFFF0003 is present and well-formed. Unavailable values: 0xFFFF0001 missing or malformed → `N` and `HI` are `-`; `N` = 0 → `HI` is `-`; 0xFFFF0005 missing or malformed → the three set fields are `-`; an empty label → `-`. After the archive lines comes one final line: `install S1 S2` (uniform), `install mixed`, or `install none`.

**Hash.** The SHA-256 of the text bytes, as 64 lower-case hex digits. The **short fingerprint** is its first 16 hex digits.

<a id="s9-4-2"></a>
#### 9.4.2 Worked example (synthetic)

Two archives, with made-up stamp values: `client_general.dat` (0x14C 0x01030000, 0x150 0, 0x174 112, 0x178 3, block size 1024, 0x17C bytes 00 01 02 … 0F, 0x18C 1100; the §9.3.1 record, so `N` 6 and `HI` 10011; set stamp 16 × `AA`, index 0, count 1; 0xFFFF0003 with S1 16 × `11`, S2 16 × `22`, label `1234`) and `client_mesh_aux_1.datx` (0x14C 0x01020000, 0x150 0, 0x174 112, 0x178 3, block size 17408, 0x17C all zero, 0x18C 0; an empty 0xFFFF0001 list; set stamp 16 × `BB`, index 1, count 2; no 0xFFFF0003).

Level-0 text (two lines, 160 bytes):

| Line |
|---|
| `client_general.dat 16973824 0 112 3 1024 000102030405060708090a0b0c0d0e0f 1100` |
| `client_mesh_aux_1.datx 16908288 0 112 3 17408 00000000000000000000000000000000 0` |

SHA-256 `b68924f7b0bddbf7fa2b802a10e979f11df08898be36047a33f08f7de0373d5d`, short fingerprint `b68924f7b0bddbf7`.

Level-1 text (three lines, 273 bytes):

| Line |
|---|
| `client_general.dat 6 10011 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 0 1 11111111111111111111111111111111 22222222222222222222222222222222 1234` |
| `client_mesh_aux_1.datx 0 - bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 1 2` |
| `install 11111111111111111111111111111111 22222222222222222222222222222222` |

SHA-256 `97e17f1a64385463806e064b46ff24a7ec954b3ffff812c56d3c43c8051fc888`, short fingerprint `97e17f1a64385463`.

**Uses:**

- Real-data tests record the short level-1 fingerprint they ran against.
- Caches derived from archives (indexes, decoded assets) are keyed by it.
- Two installs with equal level-1 fingerprints are treated as content-identical (Reported as an inference; it held for the two observed copies of version A).

<a id="s9-5"></a>
### 9.5 Build labels that are not part of the fingerprint

- **Client file version**: the PE version resource of `x64/lotroclient64.exe`. Its fixed-info file version has the form `AAAA.BBBB.CCCC.DDDD`, observed `4906.0071.1794.4035` (B1, B2) and `4601.0070.2401.4020` (A). The product-version string adds a channel word, observed `lotro_live` for the clients and `lotro` for the launcher.
  - `lotroclient.exe` and `LotroLauncher.exe` carry the same file version as the 64-bit client (Confirmed).
  - Reading the PE resource section is a bounded parse of a public file format. Our code never executes the client.
  - Show this label to users. Do not use it to choose decoders. It does **not** identify the content: B1 and B2 share it.
- **Steam `buildid`** from the app manifest (§5.3). Steam-only; coarse.
- **`ProjectVersion`** (§7). Unchanged across the observed versions.

> **OPEN:** Whether the first group of the file version (`4906`) encodes the content update number (it rose from 4601 to 4906 between A and B). The client's own log only repeats the four groups with a build-type word (`4906.0071.1794.4035.RELEASE`, LOG, 2026-10-07). Needs more client versions to compare; nothing depends on it.

<a id="s9-6"></a>
### 9.6 Invariants

Each of these can be checked on real data and on synthetic fixtures. All of them hold in B2 for all 29 archives (Confirmed 2026-10-05); 1–8 also held in A and B1.

1. Every archive has magic 0x5442 at 0x140, and 0x148 equals the file length.
2. Every archive is smaller than 2³² bytes.
3. 0x174 = 112 everywhere. 0x178 = 0 exactly for archives whose 0x14C dataset-type byte is 2.
4. Aux members share their base's 0x14C, 0x150 and 0x144.
5. For each split set, all 0xFFFF0005 stamps are equal, the indices are exactly 0…count−1, and every member reports the same count.
6. The 0xFFFF0003 S1/S2 pair is identical in every archive that carries the record.
7. Every 0xFFFF0001 record decodes with no bytes left over. The sum of run lengths and singles equals `N`. The trailing list holds the `K` highest listed iterations in ascending order.
8. Region archives come in `cell_<r>`/`map_<r>` pairs with equal 0x150 = `r`, and equal 0xFFFF0003 labels.
9. For every split set, with `L` the base archive's 0xFFFF0001 list and `HI` its highest value: every non-zero directory-entry iteration in every member is in `L`, and is at most `HI` − 1. `HI` here is the base archive's value even when the member is an aux archive, whose own list may be empty or smaller. (Observed: an archive's highest entry iteration **equals** this `HI` − 1 in 13 of the 29 archives, or in 13 of the 22 split sets taking the highest over all members, P35; equality is not an invariant.)
10. An aux member whose own 0xFFFF0001 list is not empty lists only iterations that are also in its base's list; its own list does not cover its own entries (`highres_aux_3`, `mesh_aux_1`).
11. Record sizes: 0xFFFF0005 is 24 bytes, 0xFFFF0003 37 bytes, 0xFFFF0004 12 bytes, 0xFFFF0008 16 bytes.

---

<a id="s10"></a>
## 10. Client command line

Our client accepts the option set of the original 64-bit client, so third-party launchers can start it without changes. The table restates the help text recorded by OL (P30) in our own words. The option names, one-letter forms and value modes were read from the client (BIN-1, BIN-2) for build 4906.0071.1794.4035; its option-registration routines register these 34 options and no others were found. "Mode" is the original's value mode (§10.1): **S** switch (no value), **R** required value, **O** optional value, **I** value, also accepted in the `name=value` form. "Ours" says what our client does with each option. Options that would need a feature we do not have are accepted and ignored with an info log.

| Short | Long | Mode | Argument | Meaning | Ours |
|---|---|---|---|---|---|
| `-a` | `--account` | I | string | Account (subscription) name to log in with. Launchers pass the subscription name, not the login user name | use |
| | `--authserverurl` | R | URL | Auth service URL, used to refresh the login ticket | use (net) |
| | `--chatserver` | R | host:port | Chat server to use | use (net) |
| | `--connspeed` | R | 0.0–640.0 | Connection speed in kbit/s; 0 = probe | accept |
| `-r` | `--create` | R | name | Character name to create or play | use |
| | `--cwd` | R | dir | Change directory at start-up (reported not to work in the original; launchers change directory themselves) | use (works in ours) |
| | `--debug` | R | 32-bit bit field | Which debug outputs are on | map to log filters |
| `-f` | `--franchise` | R | string | Franchise name | accept |
| | `--gametype` | R | string | Game type, used for support URLs | accept; also a game-type hint (§3.7) |
| | `--glsticket` | R | registry key | Load the login ticket from a registry key | accept (Windows only) |
| `-z` | `--glsticketdirect` | R | string | Login ticket data given directly | use (net); **never log the value** |
| | `--glsticketlifetime` | R | seconds | Ticket lifetime | use (net) |
| | `--HighResOutOfDate` | S | none | The high-resolution archive was not updated; do not switch to very high texture detail | use (§3.3) |
| `-h` | `--host` | R | host or IP | Server to connect to (also see `--port`) | use (net) |
| | `--keymap` | R | file | Key-map file to load instead of `lotro.keymap`. The help text also mentions meta-key companion files, which this build does not read (KM K31) | use: an original-format key-map file, imported per `keymap.md` §12.14 |
| | `--language` | R | string | Language to run in. Matches the `<Lang>` of `client_local_<Lang>.dat` (§3.5) | use |
| | `--loadfile` | R | file | Load file for an auto-generated character | accept |
| `-m` | `--mps` | R | file (`.wc`) | Names a `.wc` file describing a monster-play session to enter directly | accept |
| | `--outport` | R | 1–65535 | Outgoing network port | use (net) |
| `-p` | `--port` | R | 1–65535 | Server port | use (net) |
| | `--prefs` | R | file | Preferences file; also moves the settings folder to the file's parent (§8.1) | use |
| | `--remotemouse` | S | none | Adjusts mouse handling for a client driven through a remote-desktop link | accept |
| | `--resource-set` | R | list | Comma-separated resource sets; the last is the default | accept |
| | `--rodat` | O | (value) | No-op kept for compatibility; launchers pass `--rodat on` | accept (always read-only) |
| | `--safe` | S | none | Starts with conservative fallback display settings instead of the saved ones | use |
| `-s` | `--specify` | R | race,class | Race/class for a character to create | accept |
| | `--surveyurl` | R | URL | URL for quest surveys | accept |
| | `--usechat` | S | none | Connect to a chat server | use (net) |
| | `--useexiturl` | S | none | Open a browser at an exit URL | accept, never open |
| `-u` | `--user` | R | name | Character name to play | use |
| | `--voicenetdelay` | R | int (ms) | Voice network delay threshold | accept |
| | `--voiceoff` | S | none | Disable voice chat | accept |
| | `--wfilelog` | R | bit string or names | File logging for event types; names are comma-separated labels of the data's "LogType" enumeration and OR'd (§10.1) | map to log filters |
| | `--wprintlog` | R | bit string or names | Print logging, same argument form | map to log filters |

Older or template-only option names (Reported: NOTES network §1.4, §1.7), all to be **accepted and ignored**: `--dbwlib`, `--nodbwlib`, `--usetdm`, `--webjournalurl`, `--bugurl`, `--supporturl`, `--supportserviceurl`, `--crashreceiver`, `--DefaultUploadThrottleMbps`. Tessera's own options must not reuse any name in the table or in this list (compared ignoring case); new ones should use the `--tessera-` prefix. `--game-dir` is also ours.

<a id="s10-0"></a>
**Parsing rules (Tessera).** These follow the original (§10.1) except where a rule says otherwise.

1. Trim leading and trailing ASCII whitespace from every argv element. At least one launcher appends `" --HighResOutOfDate"` with a leading space as a single element (P31; Reported). *Difference:* the original does not trim; if such an element reaches its parser unchanged, it is not an option token there and is silently ignored.
2. An element is an option token when its first character is `-` (U+002D) or `/` (U+002F), compared as whole characters. *Difference:* the original tests only the low 8 bits of the first UTF-16 code unit, so a non-ASCII first character whose low byte is 0x2D or 0x2F (for example U+FF2D or U+012F) also makes an option token there (BIN-3). Tessera does not copy this; no launcher depends on it. If the second character is the same as the first, both are removed (the **doubled** form); otherwise only the first is removed (the **single** form). The rest is the name text.
3. If the name text contains `=`, the part before the first `=` is the name and the part after it is the **inline value**.
4. Lookup: with the single form and a name of exactly one character, look the character up among the one-letter forms, **case-sensitively** (`-a` is `--account`, `-A` is not). If that finds nothing, and in every other case, look the name up among the long names **ignoring ASCII case** (`--highresoutofdate` is `--HighResOutOfDate`). The doubled form never uses the one-letter forms: `--a` looks for a long option named `a`.
5. Values, by mode:
   - **S**: no value. An inline value is an **error**: report it naming the option, show the usage text and do not start (same as the original, which fails the parse). This catches mistakes such as `--safe=false`.
   - **R**: the inline value if given; otherwise the next element **verbatim, whatever it starts with** (`--prefs -x.ini` gives the value `-x.ini`; on Linux `--prefs /home/u/p.ini` gives a path, not an option). It is an error, as in the original, when there is no next element or it is empty. *Difference:* the original does not honour an inline value for R options; it treats the whole element as an unknown option and ignores it. Tessera accepts it (a deliberate superset; no launcher can rely on the original's behaviour there).
   - **O**: the inline value if given (superset, as for R); otherwise the next element if it is not an option token; otherwise an empty value.
   - **I**: the inline value if given; otherwise the next element if it is not an option token; otherwise the option gets no value. Not an error at parse time (a missing account name is reported later, by the code that needs it).
6. If the name matched no option and starts with `?` (`-?`, `/?`, `--?`), show the usage text and do not start, with a non-zero exit status. A bare `?` is not an option token (rule 9).
7. Unknown options are a warning, never fatal, and never take a value. *Difference:* the original ignores them without a message.
8. A token consisting only of introducers (`-`, `/`, `--`, `//`) is ignored with a warning.
9. An element that is not an option token and is not consumed as a value is ignored with a warning (the original ignores it silently). Empty elements are skipped.
10. Repeated options: the last one wins.
11. Values for `--glsticketdirect`, `--account` and `--glsticket` are secrets. Redact them in every log line.
12. Path values (`--prefs`, `--keymap`, `--cwd`, `--loadfile`) may be Windows paths. Translate them with §5.2 when running natively on Linux or macOS.

<a id="s10-1"></a>
### 10.1 How the original parser behaves (owner item O7)

Facts about how build 4906.0071.1794.4035 reads its own command line (BIN-1, BIN-2), stated as behaviour.

- **Option tokens.** An element whose first character is `-` or `/` is an option token. Every other element is not. The test looks only at the low 8 bits of the first UTF-16 code unit, so a non-ASCII first character with low byte 0x2D or 0x2F also qualifies (BIN-3).
- **Introducers.** One leading introducer character is removed. If the next character is the same character, it is removed too, so `-x`, `--x`, `/x` and `//x` all name option `x`, while `-/x` names option `/x`.
- **`name=value`.** The name text is split at the first `=` into a name and an inline value.
- **One-letter or long form.** The one-letter form is tried only when there was a single introducer and the name is exactly one character; that comparison is exact, so it is case-sensitive. If it finds nothing, or in every other case, the long names are searched ignoring case. A doubled introducer or a longer name (`--a`, `-account`) therefore always uses the long names.
- **Value modes.** Every option has one of four modes (column "Mode" in §10):
  - *switch* (S): set when named. Given an inline value, the client reports that the option was given a parameter it does not take, and the parse fails.
  - *required value* (R): the next element is taken as the value whatever its first character is. An empty element, or the end of the command line, makes the client report that the option needs a parameter, and the parse fails. An inline value is **not** honoured: the whole text (for example `prefs=x`) is looked up as a name, matches nothing and is ignored as an unknown option.
  - *optional value* (O, only `--rodat`): the next element is the value unless it is an option token or the command line ends; then the value is empty and the next token is processed normally. An inline value is not honoured (as for R).
  - *value with inline form* (I, only `--account`): the inline value is accepted. Without one, the next element is the value unless it is an option token; if it is, the option receives no value and parsing continues with that token. At the end of the command line it receives an empty value. Neither case fails the parse.
- **`?`.** Only after a name matched no option (in either form), the client checks the first character of the name (after the introducers): if it is `?`, it shows its usage text and the parse fails. So `-?`, `/?` and `--?` show usage; a bare `?` is not an option token and is ignored.
- **Unknown options and stray elements.** An option token whose name matches nothing (and does not start with `?`) is accepted silently and takes no value. An element that is not an option token and is not consumed as a value is also accepted silently and ignored. An empty element is skipped.
- **Failure.** When the parse fails, the client shows its usage text.

This answers owner item O7: long option names are matched ignoring case, and the `name=value` form is honoured only by `--account` (and its one-letter form `-a=…`); for switches it is an error, and for every other option the element is silently ignored.

Further behaviour of the original (BIN-4, 2026-10-07):

- **Repeated options** (P64). The parser hands each option occurrence to that option's handler at once, in command-line order; nothing is collected first. The handlers of `--prefs`, `--language` and `--debug` (the ones read) replace the earlier value, so the **last** occurrence wins, as in Tessera (§10 rule 10). Handlers of network and login options were not read (decision 0004).
- **Doubled introducer** (P65). The check for a doubled introducer compares the second UTF-16 code unit with the first as whole 16-bit units. So for an element whose first unit is a non-ASCII look-alike (BIN-3), only an identical second unit counts as doubled; the name text is the rest after removing one unit (single form) or two (doubled form). Tessera does not treat such elements as option tokens at all (§10 rule 2), so this has no effect on it.
- **`--debug`** (P63). The value is read as an unsigned 32-bit number in C notation: decimal, hexadecimal with a `0x` prefix, or octal with a leading `0`. Reading stops at the first character that does not fit, and text that does not start with a number gives 0. The client then sets each of its 32 debug flags to the matching bit of the number: set bits switch flags on, clear bits switch them off. Tessera maps the bits to its own log filters (§10 table); which original output each bit controls was not traced.
- **`--wfilelog` and `--wprintlog`** (P62). Each value is first tried as a string of `0` and `1` characters, character *i* (counting from 0 at the start) giving bit *i* of the mask; any other character makes this attempt fail (see the OPEN below). A decimal or hexadecimal number, as the help text suggests, is not read as a number. Otherwise the value is split at commas, and each part is looked up as a **label of the game data's "LogType" enumeration**: the enum mapper that the EMAPPER DID map (`records.md` §8.2) names `LogType` (0x230000B5 in this install, Confirmed 2026-10-07). A label with value *v* > 0 sets bit *v* − 1 of the mask; value 0 sets nothing; an unknown label is logged as an error and skipped. The names are therefore data, read at run time, and are not listed here. The client applies these options after its data is loaded, because it needs the enumeration.

> **OPEN:** It is not settled whether the original client ever takes a `--wfilelog` or `--wprintlog` value made only of `0` and `1` characters as a bit string, or whether such a value always goes on to the comma-separated label reading (where, unless the enumeration happens to have such a label, it is logged as an unknown label and sets nothing). Settling it needs a closer BIN read of the bit-string test or an M3 run of the client with such a value. **Reader rule for Tessera:** accept the bit-string form as specified above: a value made only of `0` and `1` characters is a bit string, character *i* giving bit *i*; any other value is a comma-separated label list.

> **OPEN:** Which output each `--debug` bit controls. Each bit is tested where its output is produced, across the whole client; tracing them was not attempted. Tessera's mapping to its own log filters does not depend on the answer.

<a id="s10-2"></a>
### 10.2 Worked examples (synthetic)

Each row is a complete command line (argv elements separated by spaces; the account name `user1` and file names are made up).

| Command line | Original | Tessera |
|---|---|---|
| `--HighResOutOfDate` | switch set | same |
| `--highresoutofdate` | switch set (long names ignore case) | same |
| `-a user1` | account = `user1` | same |
| `-A user1` | `-A` matches no one-letter form, then no long name `A`: ignored; `user1` ignored | same result, with two warnings |
| `--a user1` | doubled form: long name `a` does not exist: ignored; `user1` ignored | same result, with two warnings |
| `--account=user1` or `-a=user1` | account = `user1` | same |
| `--language=English` | whole element unknown: ignored, language not set | language = `English` (superset, rule 5) |
| `--safe=1` | error, parse fails, usage shown | error, usage shown, client does not start |
| `--prefs -x.ini` | prefs = `-x.ini` | same |
| `--port` (last element) | error: needs a parameter; parse fails | same |
| `--rodat --safe` | rodat = empty; switch `safe` set | same |
| `--rodat on` | rodat = `on` | same |
| `--account --safe` | account gets no value; `safe` set | same |
| `-?`, `/?`, `--?` | usage shown, parse fails | usage shown, client does not start |
| `?` | not an option token: ignored | ignored with a warning |
| `/language English`, `//safe` | language = `English`; `safe` set | same |
| `-/safe` | single form, name `/safe`: unknown, ignored | ignored with a warning |
| `" --HighResOutOfDate"` (one element) | not an option token: ignored (if it reaches the parser unchanged) | trimmed, switch set (rule 1) |

---

<a id="s11"></a>
## 11. Summary of open questions

All the `> **OPEN:**` callouts above, collected (revision of 2026-10-07):

- §5.3 confirm the Flatpak, Snap and Debian Steam roots (sourced, not observed; needs another machine).
- §5.5 the KD registry value names (needs a Windows standalone install); the Codemasters path (needs a historical source).
- §5.6.3 confirm the Lutris location and keys (sourced, not observed; needs another machine).
- §6 where the official launcher resolves `Launcher.DocumentFolder` (launcher only; excluded from M4; not needed).
- §7 what `ProjectVersion` counts (launcher/patcher only; excluded from M4; not needed).
- §8.2 the chat-log file names and folder (needs an owner observation).
- §8.3 whether our client shares `UserPreferences.ini` (project decision); the full preference key list (bulk table, decision needed); the `GameClientType` integers (launcher only; owner observation if ever needed).
- §9.2 the trailing iteration u64 and 0xFFFF0004 (patcher only; excluded from M4); what the 0xFFFF0003 decimal label counts (needs a second content version).
- §9.5 the file-version semantics (needs more client versions).
- §10.1 whether the original ever reads a `--wfilelog`/`--wprintlog` value of `0` and `1` characters as a bit string (needs a closer BIN read or an M3 run; Tessera accepts the bit-string form).
- §10.1 which output each `--debug` bit controls (not traced).

Answered in the revision of 2026-10-07: the use of `raw/` (§3.6.1); the legacy `libraryfolders.vdf` forms (§5.3, Reported); depot 212502 (§5.3); the Inno key brace form (§5.5); the origin and prefix keys of `lotro-launcher` (§5.6.5); the client's preferences file and settings folder (§8.1); the screenshot names (§8.2); the `WindowedResolution` packing (§8.3); the 0xFFFF0003 length encoding and the `INVALID_VERSION` marker (§9.2.2); the 0xFFFF0001 `stamp` rule and 0xFFFF0008 (§9.2.1, §9.2.5); the `--wfilelog`/`--wprintlog` names and the `--debug` value form, repeated options, the doubled-introducer test (§10.1); the implementation question on `installdir` (§13).

Answered in the 2026-10-05 revisions (no longer open): aux members for region and language archives (§3.1); the 0x14C kind byte, DE/FR language codes and regions (§3.2, from DC); the roles of the patcher directories (§3.6); the run-time log folder (§8.2); the meaning of 0x170, 0x174, 0x178, 0x17C, 0x18C and 0x190–0x19F (§9.1, from DC and LOG); the option introducers, one-letter dispatch, `?`, `name=value` and case handling (§10.1, O7); every implementation question of §13.

---

<a id="s12"></a>
## 12. Change log

| Date | Change |
|---|---|
| 2026-10-03 | First draft. Corrects the research notes: cell/map region 15 exists; 0x178 is 0 in cell/map archives; the 0x17C identifier and 0x18C do not change with content updates; the Steam folder is `Lord of the Rings Online` (no "The"). Adds the housekeeping-record layouts and the fingerprint. |
| 2026-10-05 | Moved to Review. Answered owner item O7 from the offline client's parser (BIN-1). Named header fields 0x174/0x178 as the eng/game datpack versions from the client's own log (P36). Confirmed the run-time log folder, the `CEF/` cache and PluginData worlds (P38); added `[Launcher] GameClientType` and the `[User_N]` keys (P39). Corrected P20: the official client and launcher write `UserPreferences.ini` and `lotro.keymap` without a BOM (P40). Corrected the Steam depot list (P41). Documented `browser/`, `patches/`, the patcher working directories and `uninstall_manifest.txt` (P42). |
| 2026-10-05 (2nd revision, after review) | **§10, §10.1 rewritten as behaviour (BIN-2):** introducers are `-` and `/`; the one-letter form is used only for a single introducer and a one-character name, falling back to the long names; `?` applies only after the name matched nothing; per-option value modes added (column "Mode"); **corrected** the O7 answer: `name=value` is honoured only by `--account`, is an error for switches and makes the element an ignored unknown option otherwise; **corrected** the missing-value rule: a required value is taken from the next element whatever it starts with; unknown options and stray elements are ignored silently by the original. Tessera's rules restated (§10 rules 1–12) with the differences marked; removed library-routine names and internal data-structure descriptions; paraphrased log and help text. **Implementers re-check** `apps/client` argument parsing (rules 2, 5, 6, 8, 9). **§9:** P35 and invariant 9 restated from a census of every directory entry of all 29 archives; invariants 10–11 added; level 2 gets a concrete criterion; level-0/level-1 serialisation pinned (§9.4.1) exactly as `crates/install` implements it, plus the new rule that a malformed 0xFFFF0003 (label bytes outside 0x21–0x7E) counts as absent; added §9.3 overflow checks for runs; synthetic examples §9.3.1 and §9.4.2 with hashes. **Stale observations:** versions redefined as A, B1, B2 (§1.1) after the 2026-10-04 data patch; §3.2 sizes and §9.2.1 record sizes are B2 snapshots (P43). **Provenance:** P19, P21, P23 (`Plugins/`) downgraded to Reported (not present 2026-10-05); P40's cause clause Reported; P44–P48 added; BIN uses logged in §1.3. **Coverage:** the `lotro-launcher` configs and prefix (§5.6.5, P44), a second preferences file with LF line ends and a `[General]` section (P45), LF accepted by the reader. **Imported** the corrections recorded in `dat-container.md` §15 items 1–6 (§3.1, §3.2, §3.5, §9.1, §9.2.1, §9.2.3) and `keymap.md` §13 item 1 (§8.2, `--keymap` row). **Minor:** duplicate `FrillTerrainColor` removed; the two Proton paragraphs of §5.3 merged; second `WindowedResolution` observation added; anchors added to every section; §13 answered. |
| 2026-10-06 (3rd revision, after re-review) | **Counts:** P35 and invariant 9 now say 10 of the **24** archives with `N` > 0 (29 minus the five empty aux members); P47 now says **24** `appmanifest_*.acf`, all in the default root, the second library's `steamapps/` being empty. **Stale observations:** P17 re-marked (default-root case Confirmed 2026-10-05, non-default library Reported); §5.6.1 no longer claims OL's default-root-only compatdata scan misses a prefix on the user machine (not observed; every-library search kept as a project decision); §5.3 dates the zero-size `apps` entry to 2026-10-03 only. **§8.1 step 4 and §5.6.5:** the host-user entry under the `lotro-launcher` prefix's `drive_c/users/` is a symlink to `steamuser` (Confirmed 2026-10-05); settings-folder candidates are now de-duplicated by canonical path, as installs are in §5.8. **Non-blocking notes:** §10 rule 2 and §10.1 record that the original's option-token test uses only the low byte of the first UTF-16 code unit (BIN-3, the review's re-check, logged in §1.3), with a new OPEN on the doubled-introducer case; the `--safe`, `--remotemouse` and `--mps` meanings reworded; §8.3 notes `D3DVersionPromptedForAtStartup` = 11 in both preferences files. **Implementers re-check** `crates/install` settings-folder resolution (§8.1 de-duplication). |
| 2026-10-06 (4th revision, after re-review) | **P35 and invariant 9:** `HI` is stated to be the base archive's value for every member; the equality count corrected to 13 of 29 archives (adding highres_aux_3, mesh_aux_1, sound_aux_1) and 13 of 22 split sets, from a fresh read-only walk of all 29 archives (2026-10-06). The invariant itself is unchanged and held in all 29. **§1.3:** BIN-3 now sits inside the BIN table. **§12:** rows put in date order. No implementer action (the equality count is an observation, not a rule). |
| 2026-10-07 (5th revision) | Status Revised (awaits review). Works through every OPEN with M1, M2 and M4 (new BIN use **BIN-4**, §1.3) and new sources DEP, LL, LUT, INNO; provenance rows P49–P66; P23 and P32 re-marked. **Closed:** the use of `raw/` (new §3.6.1: the client has no built-in path; the language archive names 96 files, all `.bik` or `.jpg`; `vignette_videos` added to §3.6); the legacy and transitional `libraryfolders.vdf` forms (Reported); depot 212502 = English language content, and the two locally created aux parts (§5.3); the Inno key has no braces (§5.5); `lotro-launcher` origin, game types and the absence of a persisted prefix key (§5.6.5); the official client's preferences-file rule (`--prefs`, then the current directory, then Documents with a fixed folder name) and the folders derived from it (§8.1, P57, P58); screenshot names (§8.2); `WindowedResolution` = width × 65536 + height (§8.3); the 0xFFFF0003 label count encoding and the `INVALID_VERSION` marker (§9.2.2); the 0xFFFF0001 `stamp` rule (§9.2.1, §9.3) and the meaning of 0xFFFF0008 (§9.2.5); `--wfilelog`/`--wprintlog` names come from the data's "LogType" enumeration, the `--debug` value form, repeated options (last wins) and the doubled-introducer test (§10.1); the implementer question on `installdir` (§13, §5.3). **Narrowed** (still OPEN, each now says what would settle it): Steam roots and Lutris (another machine), KD value names and the Codemasters path, `Launcher.DocumentFolder`, `ProjectVersion`, `GameClientType` (launcher-only, never read by the client), chat-log names (owner observation), the preference key list (decision needed), the iteration u64 and 0xFFFF0004 (patcher-only), the 0xFFFF0003 label and file-version semantics, `--debug` bit meanings. **Implementers re-check** `crates/install`: §8.1 (`NAME` is fixed; new step 1a for a `UserPreferences.ini` in the working directory), §9.2.2 (the label length is a 1-, 2- or 4-byte count; same results on all observed data, fingerprints unchanged), §9.2.1 and §9.3 (`stamp` 1 has no trailing list, `stamp` ≥ 3 is malformed), §5.3 (`installdir` rule as implemented, plus `contentstatsid` and plain-string entries in `libraryfolders.vdf`), §5.6.3 (new Lutris source), and `apps/client` for the `--wfilelog`/`--wprintlog` value form. |
| 2026-10-07 (6th revision, after review) | Answers the review of 2026-10-07 (status still Revised, awaits re-review). **Blocking issue, §10.1:** the parenthesis on the `--wfilelog`/`--wprintlog` bit-string attempt is removed from the bullet; the unknown is now a `> **OPEN:**` callout worded as behaviour (whether a value made only of `0` and `1` characters is ever taken as a bit string), with a reader rule (Tessera accepts the bit-string form as specified) and an entry in §11; P62 points to it. **Note a, §8.1:** the steps are now a bulleted list with bold labels (Step 1, Step 1a, Step 2 … Step 6), so that step 1a renders. The labels are kept rather than renumbered because code and this spec cite them (for example "§8.1 steps 4–5", "step 2", "the first step-6 path" in §13); the order and content of the steps are unchanged. **Note b, §8.3:** the `WindowedResolution` observations now state that the stored value is not bounded by `Resolution` (1278 × 1418 is taller than 1920 × 1080), with a reader rule: accept any 16-bit width and height as stored. No new BIN use, no new data read. **Implementer action:** none required; `apps/client` may check its `--wfilelog`/`--wprintlog` handling against the §10.1 reader rule (unchanged behaviour). |

---

<a id="s13"></a>
## 13. Open items raised during implementation (`crates/install`, 2026-10-04)

The implementer chose the behaviour below where the spec was silent. All seven are answered (2026-10-05); the answers are now part of the sections named.

1. **§9.4 serialisation of level 0 and of absent values in level 1.** Answered: the implemented serialisation is adopted unchanged and pinned in §9.4.1 (with the §9.4.2 example). Added only: a 0xFFFF0003 record whose label has bytes outside 0x21–0x7E is malformed and treated as absent (§9.2.2).
2. **§5.8 directory cap: global or per walk root?** Answered: per walk root, as implemented (§5.8).
3. **§5.8 skipping `compatdata` only inside `drive_c`.** Answered: skip `dosdevices`, `backup` and `compatdata` in every walk, as implemented (§5.8).
4. **§5.3 VDF escapes: does Steam write `\n` or `\t`?** Answered: no. Only `\\` and `\"` occur in any VDF or ACF file on the machine (P47); the "next character is literal" rule stands (§5.3).
5. **§9.2.4 size of 0xFFFF0005.** Answered: exactly 24 bytes in all 29 archives (P46). Readers require at least 24 and ignore extra bytes with a debug log, as implemented (§9.2.4).
6. **§8.1 folder to propose when no candidate exists.** Answered: the first step-6 path, never a path in the install or a prefix, as implemented (§8.1).
7. **§2 "exact case" for archive names.** Answered: the canonical spelling of §3.1, as implemented (§2).

> **OPEN:** (implementation, 2026-10-06, engineering choice, §5.3) The spec says `installdir` is a folder name under `steamapps/common/` but not which values count as one. The implementation accepts a value only when it is exactly one plain path component on every platform: it refuses `/`, `\`, `:` (so `C:` and `C:x`, which Windows reads as drive-relative paths, cannot escape the library), control characters, names ending in `.` or a space (Windows strips those, so `.. ` would become `..`), and Windows device names (`CON`, `PRN`, `AUX`, `NUL`, `CONIN$`, `CONOUT$`, `COM0`–`COM9`, `LPT0`–`LPT9`, the superscript-digit forms, with or without an extension). A refused value is ignored with a warning, as before. Please confirm, or name valid Steam folder names this would refuse.
>
> **Answered (2026-10-07):** Confirmed as written. All 24 `installdir` values on the user machine are single components built from ASCII letters, digits, space, `-` and `_` (P66), and none is refused by the rule. A folder name that Windows cannot hold (one containing `\`, `/` or `:`, ending in `.` or a space, or naming a device) cannot be a Steam install folder that also works on Windows, so the rule refuses nothing valid. Also refuse the empty value, `.` and `..` (the rule above already does if it treats them as non-plain components). §5.3 now says so.

---

<a id="s14"></a>
## 14. Corrections to other specs

These specs are owned elsewhere; the corrections are recorded here, not made there.

1. **`walker.md` §1** (`--keymap` for Tessera's own TOML keymap): this clashes with the original's `--keymap` option (§10), which `keymap.md` §12.14 now gives its original meaning. Rename Tessera's option to `--tessera-keymap`, as proposed in `keymap.md` §13 item 2. The other walker options (`--residency-mib`, `--view-blocks`, `--object-distance`, `--gpu-mib`, `--texture-cap`, `--interiors`) do not clash with any original name.
2. **`dat-container.md` §15 item 1 and §7.3** describe this spec's former P35 ("highest entry iteration = highest listed iteration − 1"). That statement is withdrawn here (P35, invariant 9); no change is needed there.
