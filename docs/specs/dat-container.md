<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# DAT archive container (`.dat` / `.datx`)

| | |
|---|---|
| **Status** | Accepted (re-review of 2026-10-07 passed; the second revision of 2026-10-07 is the Accepted text) |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Re-review 2026-10-07 of the second revision of 2026-10-07: **passed**, Accepted. Full record moved to the review log (decision 0018). |
| **Implements into** | `crates/dat` (container, extents, directory, zlib, archive set, DID routing), `apps/dattool` |
| **Companion specs** | [`install-layout.md`](install-layout.md) (finding the install, version fingerprint, the 0xFFFF0001 run-list decoder), [`records.md`](records.md) (record prologues §1.6, DID type table §2, DID maps §8), [`property-system.md`](property-system.md) (DID maps and enum mappers) |

---

<a id="s0"></a>
## 0. Purpose

This spec describes the on-disk container used by the game's data archives (`client_*.dat`
and `client_*_aux_N.datx`) closely enough that an implementer can:

- open every archive in a current install read-only and validate its file header;
- walk the directory B-tree, look up a record by its 32-bit data identifier (DID), and
  enumerate all records in DID order;
- reassemble a record's stored bytes in both storage layouts (the extent layout with a
  fragment table, and the older chained-block layout), including fragmented records;
- inflate compressed records with the same acceptance rules as the original client;
- recognise an archive that is in the middle of an update (pending journal, invalid free
  table) and the side files the original client may leave next to an archive;
- decide which archive(s) of the set to search for a given DID, in the original client's
  order;
- check a set of invariants that held on every archive of a real install.

It covers the container only. What is inside a record (meshes, textures, string tables and so
on) belongs in other specs. Writing archives is out of scope. Facts about the write side
appear where a reader needs them to explain what it sees, or to judge whether an archive is
consistent.

All values in this spec that were observed in a user's install are **observations**.
They are not constants to hard-code. Code must read them from the archive at run time. The
numbers that define an algorithm (checksum seeds, limits) are functional facts recorded with
BIN provenance under [decision 0012](../decisions/0012-numeric-constants.md).

---

<a id="s1"></a>
## 1. Provenance

Legend:
- **C**: Confirmed, observed in the user's install with read-only probes. Two full
  observations exist:
  - **2026-10-03**: full B-tree walks of all 29 archives (the first version of this spec);
  - **2026-10-05**: the same walks repeated after the launcher re-applied a content patch on
    2026-10-04, plus a full inflation of **every** compressed record (1,740,962) and a
    census of every entry, node, fragment, free extent and metadata record.
  Unless a fact says otherwise, it held on both dates. Per-archive values in §3.5 and §11
  are those of 2026-10-05.
- **R**: Reported, from published sources only, not observed (or not observable) in this install.
- **BIN**: from method M4 (see the BIN source row). A fact marked **BIN** alone rests only on
  the client's code; **BIN + C** means the code and the data agree.

Source tags:

| Tag | Source |
|---|---|
| LC | LotroCompanion `delta-lotro-dat-utils` jar, facts extracted from compiled bytecode by a spec writer (https://sourceforge.net/projects/lotrocompanion/) |
| JT | jtauber/lotro, `dat.py` (2014) (https://github.com/jtauber/lotro) |
| DO | themissingdocs DatOptimize / LOTRODefrag tools (https://www.themissingdocs.net/?page_id=258), studied as permitted by the author's page |
| MOG | Mogwai-TheFurry/DatExplorer, a reader for a sibling game's archives (https://github.com/Mogwai-TheFurry/DatExplorer) |
| CHZ | Chorizite/DatReaderWriter, for an older sibling engine (https://github.com/Chorizite/DatReaderWriter) |
| ACE | ACEmulator ACE.DatLoader, for an older sibling engine (https://github.com/ACEmulator/ACE) |
| KD | koniecdev/LotroKoniecDev knowledge base (observations of `datexport` version numbers) |
| AT1 | Adobe Type 1 Font Format specification (published), for the checksum cipher constants in [§3.6](#s3-6) |
| OBS | Our own black-box observation of the user's install, 2026-10-03 and 2026-10-05 (probes in the spec writer's scratch area; no data committed) |
| LOG | The official client's own log file `lotroclient64.log` (under `%LOCALAPPDATA%\The Lord of the Rings Online\` in the Steam Proton prefix), written by its run of 2026-10-04 and read read-only on 2026-10-07 (method M3). Only the lines that report each archive's datpack versions were read. |
| EM | Game data read in the install on 2026-10-05: the "Languages" enumeration (EMAPPER DID map label, enum mapper 0x230000B1 with base 0x23000186) and the "RegionID" DID map (0x28000008), decoded with [`records.md`](records.md) §7–§8 |
| BIN | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md): disassembly of the offline client `x64/lotroclient64.exe`, **client build 4906.0071.1794.4035**, read on 2026-10-05 in a dirty-room spec-writing session (Q17, Q18 and the Q14 re-check on 2026-10-06; Q19–Q22 on 2026-10-07; the Q20 re-read, Q23 and Q24 in the repair session of 2026-10-07), address ranges only. **Scope:** the client's own archive library (file open and header handling, directory B-tree, extent and chained reads, free-space handling, the journal and overlay side files, compression, archive-set handling, metadata records) and the archive-selection code that maps a DID to an archive. **Not read:** any network, login, patch-protocol, launcher, patcher (`PatchClient.dll`) or anti-tamper code. The questions and why M1–M3 could not answer them are listed in the table below. |

BIN questions:

| # | Question | Why M1–M3 were not enough |
|---|---|---|
| Q1 | Meaning of entry flag bit 1 | Every multi-block record in the data has bit 1 set, and single-block records read the same in either layout, so the data cannot show what the bit selects. |
| Q2 | Non-empty journal format and behaviour; meaning of the 64 bytes at 0x100 and of 0x164–0x16C | No journal exists in the install. Producing one would require interrupting the official patcher mid-write, which is unsafe and outside M3. |
| Q3 | Free-table capacity, relocation, the 0x154 value 0xFFFFFFFF, and 0x158 | Write-side behaviour; the data shows only end states. |
| Q4 | Header fields 0x17C–0x19B | Values are constant or nearly so; their use is not observable. |
| Q5 | Exact byte accounting of the chained (legacy) layout | Only single-block chained allocations occur in the data. |
| Q6 | Meaning of the entry fields `stamp`, `time` and `iteration`, and whether an update always raises `iteration` | Only one patch level is available on the machine; write rules are not observable. |
| Q7 | Archive search order, local-archive precedence, and what happens to duplicate DIDs | No DID is duplicated in the install, so the order has no visible effect. |
| Q8 | Compression edge cases, and the writer's compression policy (which length its 80 % rule measures) | Every compressed record in the install is well formed (1,740,962 checked), so the client's tolerance is not observable. The data shows which records ended up compressed, not the threshold that decided it. |
| Q9 | Language and region archive naming; the 0x150 subset values | Only the English language archive is installed. |
| Q10 | Meaning of the metadata records 0xFFFF00xx | Their contents are opaque identifiers and counters. |
| Q11 | Empty root nodes, fragmented directory nodes, leaf strictness, maximum fragment count | These cases do not occur in the data. |
| Q12 | The DID value that lookup treats as "no DID" | Nothing in an archive declares it, and the data cannot show what a lookup does with any particular key. The data only hints at it (never-used entry slots, [§5.1](#s5-1)). Asked on 2026-10-05 (revision). |
| Q13 | The older resource-identifier encoding (0x14E and 0x14F both zero) and what the client does with it | No archive in the install uses it. Asked on 2026-10-05 (revision). |
| Q14 | How a set is opened: part order, identifier rules, part-count check, deletion of mismatched files | Every set in the install is complete and consistent. Observing the failure paths would need missing or mismatched aux files in the install (writing to the install is forbidden) and a run of the client, which deletes files on that path. Re-read on 2026-10-05 (revision); identifier rules re-checked on 2026-10-06. |
| Q15 | Access and sharing mode in which the archive library opens a file | M3 would need the official client running next to a probe, which is not possible in a dirty-room session on this machine. Even then it could not show the mode that is not in use. Re-read on 2026-10-05 (revision). |
| Q16 | When the writer grows a file, by how much, and when an archive counts as full | These are write-side rules. The data shows only the resulting lengths (multiples of 1 MiB), from which the constants cannot be recovered. Re-read on 2026-10-05 (revision). |
| Q17 | How the client loads a part's 0xFFFF0005 record: stamp, length, compression, and what happens when the record is absent or unusable | No archive in the install has an absent, short, compressed or wrongly stamped membership record, and producing one would mean writing to the install. Asked on 2026-10-06 (answer to the implementer notes of [§14](#s14)). |
| Q18 | When and how the client reads the version-112 free-space table, and what it checks | Every table in the install is consistent, and the client reads it only on the write side, which cannot be run against the install. Asked on 2026-10-06 (answer to the implementer notes of [§14](#s14)). |
| Q19 | The exact file name of an archive set's overlay | No overlay exists in the install, and making the client create one would mean letting it write next to the archives. Asked on 2026-10-07 (OPEN of [§2.5](#s2-5)). |
| Q20 | Which datpack (container and game-data) versions the game client expects when it opens each archive family, and how the comparison with the file's values decides | The log (M3) shows the expected values of one run but not where they come from or whether another configuration changes them; the data shows only the stamped values. Asked on 2026-10-07 (OPEN of [§3.7](#s3-7)); the comparison rule re-read on 2026-10-07 after the review of that date. |
| Q21 | The configured maximum archive length, and the length of a newly created archive | Write-side settings that leave no trace in the data except the final lengths. Asked on 2026-10-07 (OPEN of [§4.7](#s4-7)). |
| Q22 | Whether any code of the game client reads 0xFFFF0004, the u64 values of the iteration list, or the product identifier 0xFFFF0008; how the iteration list's trailing part depends on the record's `stamp` | The records' contents are opaque numbers; the data cannot show whether or how they are used. Asked on 2026-10-07 (OPEN of [§7.5](#s7-5)). |
| Q23 | Which 0x174 and 0x178 a newly created archive gets, and where a set takes them from when it creates an aux part | Every archive in the install was written by the patcher; making the game client create one would mean letting it write to the install. Asked on 2026-10-07 (review of 2026-10-07, note b). |
| Q24 | In which directory the client looks for a set's overlay | No overlay exists in the install, and the lookup leaves no trace without one. Asked on 2026-10-07 (review of 2026-10-07, note c). |

| # | Fact (section) | Sources | Mark |
|---|---|---|---|
| P1 | Archive file names, `.datx` extension for aux parts ([§2](#s2)) | LC, OBS, BIN | C |
| P2 | Area 0x000–0x0FF: written with a fixed text banner when the client creates an archive, never read; all zero in every observed archive ([§3.1](#s3-1)) | CHZ, OBS, BIN | C (zero), BIN (use) |
| P3 | Area 0x100–0x13F: the client writes the 5 bytes `00 50 4C 00 00` at 0x100 whenever it flushes an archive and never reads the area ([§3.2](#s3-2)) | CHZ, JT, OBS, BIN | C (bytes), BIN (use) |
| P4 | Magic 0x00005442 at 0x140, checked on every open ([§3.3](#s3-3)) | LC, JT, CHZ, ACE, MOG, OBS, BIN | C |
| P5 | 0x144 is the minimum allocation size; it must be a multiple of 4 ([§3.3](#s3-3), [§4.4](#s4-4)) | LC, JT, DO (name); OBS (meaning); BIN (multiple of 4, per-kind defaults) | C, BIN |
| P6 | 0x148 equals the actual file length ([§3.3](#s3-3)) | LC, JT, DO, OBS | C |
| P7 | 0x14C–0x153 form the archive's 64-bit resource identifier: variant (u16), kind (byte 0x14E), dataset type (byte 0x14F), subset (u32 0x150) ([§3.4](#s3-4)) | CHZ, ACE (sibling names); OBS; BIN | C (values), BIN (meaning) |
| P8 | Subset 0x150 = region number for cell/map, the folded language code for language archives, 0 for portal archives ([§3.4](#s3-4)) | CHZ (sibling: "subset = region"), OBS, EM, BIN | C, BIN |
| P9 | 0x154/0x15C/0x1A0/0x19C describe the free-space table in version 112; the chain description of older sources applies to version 111 ([§4.5](#s4-5)) | DO, OBS, BIN | C, BIN |
| P10 | 0x160/0x1A4 are root node offset and root allocation size ([§3.3](#s3-3)) | LC, JT, DO, OBS, BIN | C |
| P11 | 0x170 holds the master DID map in `general` only; it is the root DID map of `records.md` §8.2 ([§3.3](#s3-3)) | ACE, CHZ (sibling), OBS | C (value and identity), R (client use) |
| P12 | 0x174 = 112 everywhere; 111 = older layout; the client treats 111 and 112 as compatible ([§3.3](#s3-3), [§3.7](#s3-7)) | LC, KD, OBS, BIN | C (112), BIN (rule) |
| P13 | 0x178 = 3 for portal and language archives, 0 for cell and map ([§3.3](#s3-3)) | LC, OBS | C (values) |
| P14 | 0x17C–0x18F is a 20-byte version stamp (16-byte identifier, u32 counter) that the archive library stores and returns but does not check ([§3.3](#s3-3)) | CHZ, ACE (sibling names), OBS, BIN | C (values), BIN (use) |
| P15 | 0x190/0x194/0x198 = 104/32/8 are written at creation and forced to 104/32/8 in memory on every open; never validated ([§3.3](#s3-3)) | OBS, BIN | C (values), BIN (use) |
| P16 | 8-byte allocation header; extent layout `{fragCount, 0}` ([§4.1](#s4-1)) | LC, OBS, BIN | C |
| P17 | Fragment table sits at the **end** of the first extent ([§4.2](#s4-2)) | OBS, BIN | C, **corrects/clarifies** LC-derived note |
| P18 | Chained layout: algorithm, where it applies, free tag ([§4.3](#s4-3)) | LC, DO (outline); BIN (algorithm) | BIN; C (only single-block chains occur) |
| P19 | Allocation sizing rule `max(minAlloc, roundUp4(len+8))`, best fit, split only if the rest ≥ minAlloc ([§4.4](#s4-4)) | OBS, BIN | C, BIN |
| P20 | Exact tiling of the file by allocations ([§4.6](#s4-6)) | OBS | C (29/29, both dates) |
| P21 | Directory node: 62 child pointers {size, offset}, count at payload +496, 61 × 32-byte entries, payload 2452 bytes ([§5.1](#s5-1)) | LC, JT, OBS, BIN | C |
| P22 | Entry layout and field meanings ([§5.2](#s5-2)) | LC, DO, JT, MOG, OBS, BIN | C (layout), BIN (meanings) |
| P23 | Flag bit 0 = zlib, bit 1 = extent layout; values 0, 2, 3 observed ([§5.3](#s5-3)) | LC, DO, MOG, OBS, BIN | C, BIN; **corrects** "only 2 and 3" |
| P24 | B-tree of minimum degree 31: 30–61 entries per non-root node, uniform leaf depth ([§5.4](#s5-4)) | OBS, BIN | C, BIN |
| P25 | Lookup and in-order traversal; leaf test is child 0's offset = 0 ([§5.5](#s5-5), [§5.6](#s5-6)) | LC, JT, OBS, BIN | C, BIN |
| P26 | Compressed payload = u32 inflated length + zlib stream ending exactly at stored length; the client's acceptance rules ([§6.1](#s6-1)) | LC, JT, MOG, OBS, BIN | C (all 1,740,962), BIN (rules) |
| P27 | Payload DID echo, by type ([§6.2](#s6-2)) | LC, MOG, OBS; `records.md` §1.6 | C |
| P28 | Reserved metadata DIDs 0xFFFF00xx, their roles and versions ([§7](#s7)) | OBS, BIN (and an AC2 hint for 0xFFFF0001) | C (shapes), BIN (roles) |
| P29 | DID → archive routing as the client does it ([§8](#s8)) | LC, OBS, BIN | C (placement), BIN (order) |
| P30 | No DID appears in two archives of an install ([§8.3](#s8-3)) | OBS | C (both dates) |
| P31 | 32-bit offsets; aux part creation when no part accepts a write ([§9](#s9)) | LC, DO, OBS, BIN | C (sizes), BIN (policy) |
| P32 | Pending journal: header 0x16C, the `.jrnl` side file, its format, checksum and replay ([§3.6](#s3-6)) | BIN, AT1 (cipher constants) | BIN |
| P33 | 0x158 is the chained free list's tail (version 111); in version 112 it has no function and may hold a stale value ([§3.3](#s3-3), [§4.5](#s4-5)) | DO (name), OBS, BIN | C (0x119C in four archives on 2026-10-05), BIN |
| P34 | 0x154 = 0xFFFFFFFF marks the free table as invalid; the client refuses to open such a version-112 archive ([§4.5](#s4-5)) | BIN | BIN |
| P35 | Free-table capacity `(0x1A0 − 16) / 8` pairs, terminator pair, relocation size `max(0x144, 8 × count′ + 8024)` ([§4.5](#s4-5)) | OBS, BIN | C, BIN |
| P36 | Writer limit on extents per record: at most `0x144 / 8` (so F ≤ 0x144/8 − 1) ([§4.2](#s4-2)) | OBS, BIN | C (never exceeded; reached in `local_English`), BIN |
| P37 | Archive kinds and default minimum allocation sizes of the client's archive table ([§3.4](#s3-4)) | OBS, BIN | C (every observed 0x144 matches), BIN |
| P38 | Language archive file name `client_local_<label>.dat` from the "Languages" enumeration; subset = folded code ([§2.1](#s2-1), [§3.4](#s3-4)) | EM, BIN | C (English), BIN |
| P39 | Region archives are opened for every key of the "RegionID" DID map; only region 1 is required ([§2.4](#s2-4)) | EM, BIN | C (keys), BIN |
| P40 | `stamp` (+2) is the record's data version, returned with the payload and checked by the code that decodes the record ([§5.2](#s5-2)) | OBS, BIN | C (values), BIN |
| P41 | `time` is set to the current Unix time when a record is written, unless the writer supplies one ([§5.2](#s5-2)) | OBS, BIN | C (10,677 records written within 33 s on 2026-10-04), BIN |
| P42 | `iteration` is the patch iteration that last wrote the record; a write with a lower iteration than the stored one is skipped ([§5.2](#s5-2)) | OBS, BIN | C (every non-zero value is in the base archive's iteration list), BIN |
| P43 | Within a set: overlay first, then parts in index order; the first part holding the DID wins ([§8.3](#s8-3)) | BIN | BIN |
| P44 | Records with flags 0 are chained-layout leftovers from archives created as version 111 and converted to 112 ([§5.3](#s5-3), [§4.5](#s4-5)) | OBS, BIN | C (8 records), BIN (inferred history) |
| P45 | Overlay side file and its precedence ([§2.5](#s2-5)) | BIN | BIN |
| P46 | Compression writer policy: only payloads > 16 bytes. The zlib stream (entry `length` − 4) is kept only if 10 × its length ≤ 8 × the original length, or if the original minus the stream length is ≥ 512,000 bytes ([§6.1](#s6-1)) | OBS, BIN (Q8) | C (all 1,740,962 satisfy it with the stream length; 17 only through the 512,000 clause; 2026-10-05), BIN |
| P47 | The "no DID" value is 0: lookup of DID 0 reports "absent" without searching; new nodes fill unused entry slots with DID 0 ([§5.1](#s5-1), [§5.5](#s5-5)) | OBS, BIN (Q12) | BIN (value and rule); C (186,823 unused entry slots are all zero; no entry has DID 0; 2026-10-05) |
| P48 | Older identifier encoding: recognised when 0x14E and 0x14F are both zero and the u32 at 0x14C is non-zero; converted in memory only ([§3.4](#s3-4)) | BIN (Q13) | BIN only (no such archive in the install) |
| P49 | Set opening: discovery by pattern, byte-wise name order, identifier and index checks (all-zero identifier allowed only on the first part loaded; while the set's identifier is all zero the part's identifier is adopted without an index check), part-count check, deletion of the set's files on failure ([§2.3](#s2-3), [§7.2](#s7-2)) | BIN (Q14, identifier rules re-checked 2026-10-06), OBS | BIN only for the rules; C for the membership values it checks ([§7.2](#s7-2)) and for "no all-zero identifier in 29 of 29 archives" (2026-10-05) |
| P50 | Access and sharing: read-write mode opens with no sharing, read-only mode opens for reading with read sharing ([§3.7](#s3-7), [§9](#s9)) | BIN (Q15) | BIN only. The flags per mode were re-read at the file-open routine on 2026-10-05; that the game client uses the read-write mode comes from the first BIN reading of its archive initialisation. |
| P51 | File growth: *need* formula, growth by whole MiB, the full test with 8 MiB headroom, metadata exemption ([§4.7](#s4-7)) | BIN (Q16) | BIN only; C (file lengths are multiples of 1 MiB except `local_English`) |
| P52 | Loading 0xFFFF0005: ordinary record read (a compressed copy is inflated), `stamp` must be 1, the first 24 bytes are used and further bytes ignored, a shorter record leaves the missing fields at their defaults; the open does not check whether the load worked; defaults are an all-zero identifier, index 0, count 1 ([§7.2](#s7-2)) | BIN (Q17) | BIN only (no such record in the install); C for "every membership record has `stamp` 1, 24 bytes and flag bit 0 clear" (29 of 29, 2026-10-06) |
| P53 | Version-112 free table: read only when the writer first allocates or frees space; offset 0 with size 0 means an empty free list; the allocation header is skipped, pairs are read up to the first pair whose size is 0, and the one check is that 8 × pairs read + Σ (size − 8) equals 8 × 0x15C + 0x19C ([§4.5](#s4-5)) | BIN (Q18), OBS | BIN; C (29 of 29 tables: offset and size non-zero, the first pair with size 0 is the pair at index 0x15C and is `{0, 0}`, and the check holds; 2026-10-06) |
| P54 | The overlay of a set is the base archive's full file name with `.ovl` appended, in the base's directory (for example `client_general.dat.ovl`) ([§2.5](#s2-5)) | BIN (Q19) | BIN only (no overlay in the install, 2026-10-07) |
| P55 | The game client opens every archive with expected datpack versions −1 (container) and −1 (game data), so it compares neither 0x174 nor 0x178; a negative expected value disables each comparison; either failed comparison refuses the archive, and the refusal is waived when the expected container version and 0x174 are 111 and 112 in either order ([§3.7](#s3-7)) | BIN (Q20, rule re-read 2026-10-07), LOG | BIN; C (29 of 29 archives logged with expected values −1 and −1 in the run of 2026-10-04, read 2026-10-07) |
| P56 | The game client opens its archives with a configured maximum length of 0xFFFFFFFF; an archive created by the archive library starts 1 MiB (0x100000) long, with the minimum allocation size of its kind ([§4.7](#s4-7)) | BIN (Q21), OBS | BIN; C (`cell_15` and `map_15` are exactly 1 MiB, 2026-10-05) |
| P57 | 0xFFFF0001's trailing list exists only when the record's `stamp` is 2; the u64 of each entry is copied verbatim and not interpreted by any code found; no code of the game client reads 0xFFFF0004; 0xFFFF0008 is read and written only through the archive library's product-identifier accessors, and the open does not check it ([§7.3](#s7-3), [§7.5](#s7-5)) | BIN (Q22), OBS | BIN; C (every 0xFFFF0001 has `stamp` 2, 2026-10-05) |
| P58 | Creation stamps 0x174 = expected container version if ≥ 1, else 111, and 0x178 = expected game data version unchanged; a set creating an aux part takes the expected versions from its overlay or first part (that archive's own 0x174 and 0x178) ([§4.7](#s4-7)) | BIN (Q23) | BIN only (no archive created by the game client in the install, 2026-10-07) |
| P59 | The overlay is looked for in the directory the opener names for the set, or the current directory when none is named; the separate overlay-directory parameter is not used by the set-level open and is empty in the game client's call ([§2.5](#s2-5)) | BIN (Q24) | BIN only (no overlay in the install, 2026-10-07) |

---

<a id="s2"></a>
## 2. Archive set and naming

<a id="s2-1"></a>
### 2.1 File names

- Archives sit directly in the install's root directory (not in `x64/`). **C**
- A file name is `client_` + the archive's name + `.dat`. The names come from a fixed table
  in the client ([§3.4](#s3-4)): `general`, `gamelogic`, `anim`, `mesh`, `sound`, `surface`,
  `highres` (portal archives), `cell_<r>` and `map_<r>` (region archives, `<r>` in decimal),
  and `local_<Language>` (language archive). **C**, **BIN**
- Overflow parts of an archive are named `client_<name>_aux_<N>.datx`, with N = 1, 2, 3, ….
  The extension really is `.datx`. **C**, **BIN**
- `<Language>` is the **label** of the language code in the game's "Languages" enumeration
  (EMAPPER DID map label "Languages"). The client looks the label up at run time. Labels in
  this install: `English` (code 2), `EN_GB` (0x10000001), `FR` (0x10000003), `DE`
  (0x10000004). Only `client_local_English.dat` is present. **C** (labels, EM), **BIN** (rule)
- Other `*.dat` files in the install, such as `icudtl.dat` and an uninstaller's `unins000.dat`,
  are **not** archives. They do not carry the magic at 0x140. A locator must select files by the
  `client_` prefix and verify the magic. **C**
- That rule only filters out names the client never opens. A file whose name is one the client
  **does** open (a base name of [§2.4](#s2-4), or a name matching a set's aux pattern of
  [§2.3](#s2-3) step 1) but which is too short or lacks the magic is a broken archive, not a
  foreign file: the client fails that archive's set ([§2.3](#s2-3)). Report it as an error
  (I-1 or I-2) for that set. A `client_<x>.dat` name that is not one of these stays a warning.
  **BIN** (Q14)
- Name matching. The original client runs on a case-insensitive file system, so letter case in
  file names carries no meaning there (**R**, platform). Its aux search counts **every** name
  matching `client_<name>_aux_*.datx`, whatever stands in place of `*` (for example `_aux_01`
  or `_aux_x`), and checks each such part by its position in byte-wise name order (**BIN**,
  Q14). A reader that accepts only the canonical form `_aux_<N>` (N decimal, no leading zero)
  should report any other matching name as a warning that says the client would count it as
  a part.
- Side files that the original client may create next to an archive are not archives a reader
  should open as part of the set: `<archive file name>.jrnl` ([§3.6](#s3-6)) and an overlay
  ([§2.5](#s2-5)). **BIN**

<a id="s2-2"></a>
### 2.2 Archives present in the observed install

Kinds and parts observed on 2026-10-05 (29 archives; the same files as on 2026-10-03):

| Kind | Files | Notes |
|---|---|---|
| general | `client_general.dat` | |
| gamelogic | `client_gamelogic.dat` | |
| local | `client_local_English.dat` | one per installed language |
| mesh | `client_mesh.dat`, `client_mesh_aux_1.datx` | |
| surface | `client_surface.dat`, `client_surface_aux_1.datx`, `client_surface_aux_2.datx` | |
| highres | `client_highres.dat`, `client_highres_aux_1.datx` … `_aux_3.datx` | optional in LC (R) |
| sound | `client_sound.dat`, `client_sound_aux_1.datx` | |
| anim | `client_anim.dat` | optional in LC (R) |
| cell | `client_cell_{1,2,3,4,5,14,15}.dat` | region number in file name |
| map | `client_map_{1,2,3,4,5,14,15}.dat` | region number in file name |

- `cell_15` and `map_15` are 1 MiB placeholders that hold only metadata records ([§7](#s7)). **C**
- An install is valid for LC if `client_general.dat` exists. **R**
- The RegionID DID map also lists region 6, for which no archive exists in this install
  ([§2.4](#s2-4)). **C**

<a id="s2-3"></a>
### 2.3 Set membership

The parts of one archive form a **set**. Each archive in a set carries a membership record at
DID 0xFFFF0005 ([§7.2](#s7-2)). That record gives a 16-byte set identifier, shared by all
parts, plus the part's index (0 for the `.dat` base) and the set's part count. **C**

The client opens a set like this (**BIN**, P49):

1. It opens the base first. It then lists the files in the same directory whose names match
   `client_<name>_aux_*.datx`. The number of files found is the number of aux parts it will
   try to open.
2. In the normal open it sorts the names found by plain byte-wise comparison and opens them
   in that order. (In the mode that creates or re-stamps parts it instead opens
   `client_<name>_aux_1.datx`, `_aux_2`, … by name, as many as were found, and logs a missing
   file as an error.) Byte-wise order equals index order only while a set has at most 9 aux
   parts: `_aux_10` would sort before `_aux_2`. No set in the install has more than 3.
3. For each part, in opening order, it reads 0xFFFF0005 and compares the part with what has
   been loaded so far:
   - a part whose set identifier is all zero is accepted only if it is the first part loaded
     (in practice the base). Anywhere else it is an error;
   - while the set's identifier is still all zero, the part's identifier is adopted as the
     set's, without an index check. The set's identifier starts out all zero, so this is how
     the base's identifier becomes the set's (so the base's own part index is never checked);
     if the base carried zero, it is also how the first aux part's identifier becomes the
     set's;
   - otherwise the part's identifier must equal the set's, and its part index must equal its
     position in the opening order (base 0, first aux 1, …). A mismatch is logged as a stamp
     mismatch and the part fails.

   Consequence: a set of any part count is accepted with an all-zero identifier on the base,
   provided every aux part carries the same non-zero identifier and the second and later aux
   parts carry their positions as part index. In that case the first aux part's index is not
   checked. An all-zero identifier on an aux part always fails. (Re-checked against the client
   on 2026-10-06; the "all zero" comparison is against a 16-byte all-zero constant.) No archive
   in the install has an all-zero identifier (29 of 29 non-zero, 2026-10-05). **BIN**, **C**
4. In the normal open, if any aux part fails to open or fails step 3, the client closes the
   set and deletes the base file and every aux file it found, logging each one as a deleted
   mismatched file. The open then fails without reaching step 5. A read-only reader must
   never delete or modify anything. It reports such files and the set's failure.
5. After all parts are loaded it compares the number of parts with the count in the base's
   0xFFFF0005 record. If they differ, it logs an incorrect part count and the open fails. The
   message speaks of finding fewer parts, but the test is for inequality.

Every archive in the original client goes through this set logic, including region and
language archives, so any archive could in principle have aux parts. Only portal archives have
them in this install. **BIN**, **C**

A reader should discover aux parts both by pattern and by name, order them by the number `N`
in the file name (not by byte-wise name order), and check the membership records as in step 3.
A pattern match without a canonical number `N` ([§2.1](#s2-1), for example `_aux_01.datx` or
`_aux_x.datx`) gets no place in that order: the reader does not open it as a part and reports it
as a warning. Because the client counts it, the reader also reports, as the client's step 5 would
see it, the part count that includes such files.
It should report a missing part (index < count but no file present) or a count mismatch as an
error for that set, not crash.

How a part's 0xFFFF0005 record is loaded, and the values a part has when the record is absent
or unusable, is in [§7.2](#s7-2). Only the base's part count enters step 5. The client never
compares the count fields of aux parts.

Recommended severities for a reader. The rule is: **Error** when records of the set cannot be
read, which includes a part that is missing and a number of parts that differs from the base's
count (a part may then be absent, or a file the reader does not know may hold records); **Warn**
when every file is readable and present but the membership metadata disagrees, even where the
client would refuse the set. A read-only reader can still serve every record in that case.

| Situation | Client ([§2.3](#s2-3), [§7.2](#s7-2)) | Reader severity |
|---|---|---|
| Base or aux file with a name the client opens, too short or without the magic | the set fails (an aux part also triggers step 4) | Error (I-1, I-2) for the set |
| File with valid magic that cannot be opened (I-4, I-6 entry size, I-8, read error) | the set fails | Error under that invariant |
| Part missing (index < base count, no file) or number of parts ≠ base count | step 5 fails | Error for the set |
| Part index ≠ its position, identifier disagrees, all-zero identifier on an aux part | step 3 fails | Warn (I-24) |
| 0xFFFF0005 absent, `stamp` ≠ 1, or shorter than 24 bytes | defaults are used, step 3 or 5 then decides | Warn (I-24) |
| Aux part's count differs from the base's | not checked | Warn (I-24) |
| Declared count above a reader's own limit | (no limit in the client) | the reader's resource limit; Error is acceptable |

Region archives other than region 1 that fail are tolerated by the client ([§2.4](#s2-4)). The
severity is still reported for that archive, and the other sets stay usable.

<a id="s2-4"></a>
### 2.4 Which archives the client opens

**BIN**, with the data it consults read in **EM**:

- **Portal archives**: one set per entry of the client's archive table whose dataset type is 1
  ([§3.4](#s3-4)).
- **Region archives**: one `cell_<r>` and one `map_<r>` set for every non-zero key `r` of the
  "RegionID" DID map (EMAPPER DID map label "RegionID"; 0x28000008 in this install). Keys in
  this install: 1, 2, 3, 4, 5, 6, 14, 15. A region archive that fails to open is tolerated,
  except for region 1, whose failure makes the client's archive initialisation fail. Region 6
  has no files in this install and is skipped that way. The table also has a region kind
  0x0D (dataset type 2), which this client never opens.
- **Language archive**: `client_local_<label>.dat` for the selected language code
  ([§2.1](#s2-1)).
- The client checks that the dataset type and kind in each archive's resource identifier
  ([§3.4](#s3-4)) are a pair from its table (unless the opener asks it to skip the check), and
  that all archives of one family carry the same variant number. A failed check is reported
  and the archive is refused. A reader should also check that the identifier agrees with the
  file name.

<a id="s2-5"></a>
### 2.5 Overlay file

The original client can keep an **overlay** next to an archive set: a separate archive in this
same container format, with the extension `.ovl`. **BIN**

- When the overlay exists, reads of the set search the overlay **before** all parts, and
  writes go to the overlay. The client also keeps, per overlay, a list of DIDs marked deleted;
  such a DID is reported as absent even if a part still holds it.
- When the client next opens the set in a mode that allows it, it merges the overlay's records
  into the parts and then stops using the overlay.
- No overlay exists in the observed install. **C** (absence)

A read-only reader may ignore an overlay but should report its presence, because the set's
records may then be out of date.

**File name** (**BIN**, Q19, P54). The overlay's name is the base archive's **full** file name,
extension included, with `.ovl` appended, in the same directory as the base. Examples:
`client_general.dat.ovl`, `client_cell_14.dat.ovl`, `client_local_English.dat.ovl`. The name
the client builds for a set is `client_` + the archive's name + `.dat` ([§2.1](#s2-1)); the aux
parts take the same name without the `.dat` extension as the stem of `_aux_<N>.datx`, but the
overlay keeps it. There is one overlay per set, never one per aux part. A reader looks for this
exact name (letter case as in [§2.1](#s2-1)) when it reports an overlay. (This replaces the
OPEN of the Accepted version, which guessed `client_general.ovl`.)

**Directory** (**BIN**, Q24, P59). The client looks for the overlay in the directory that the
opener names for the set, which is the directory of the archives themselves; when the opener names
no directory, it uses the process's current directory. The archive library's open parameters also
carry a separate overlay directory, but the set-level open does not use it, and the game client
passes it empty. So for the game client the overlay sits next to the base archive.

> **OPEN:** The set-level open also tries the overlay's bare file name relative to the current
> directory when it is not found next to the archives; the conditions under which that second
> location is used were not traced. **Reader rule:** look for the overlay only next to the base
> archive; do not search the current directory.

---

<a id="s3"></a>
## 3. File header

All integers are little-endian. "u32" is unsigned 32-bit. Offsets are absolute file offsets.
The header occupies 0x000–0x3FF. The first allocation starts at 0x400. **C**

<a id="s3-1"></a>
### 3.1 Version string area (0x000–0x0FF)

| Offset | Size | Type | Meaning | Mark |
|---|---|---|---|---|
| 0x000 | 256 | bytes | Free text. When the client creates an archive it writes a fixed ASCII banner here, zero-padded, with byte 0x0FF set to 0x1A. It never reads the area. **All zero** in all 29 observed archives. | C (zero), BIN (use) |

A reader must not interpret this area.

<a id="s3-2"></a>
### 3.2 Transaction marker area (0x100–0x13F)

| Offset | Size | Type | Meaning | Mark |
|---|---|---|---|---|
| 0x100 | 5 | bytes | `00 50 4C 00 00`. The client writes exactly these 5 bytes here every time it flushes an archive that it has open for writing. It never reads them. | C (bytes), BIN (use) |
| 0x105 | 59 | bytes | Not used by the client after creation. All zero in every observed archive. | C (value), BIN |

Older sources call this area the "transaction journal" and describe `00 50 4C 00` as the empty
journal. In this client the real journal is a separate file ([§3.6](#s3-6)); this area does not
record whether an update is pending. A reader should not warn about its contents. A validator
may report a value other than the observed bytes as information only.

<a id="s3-3"></a>
### 3.3 Superblock (0x140–0x1A7)

The client reads and writes the superblock as one 0x68-byte block. Each column "Observed"
summarises all 29 archives (per-archive values are in [§3.5](#s3-5)).

| Offset | Size | Type | Meaning | Observed | Mark |
|---|---|---|---|---|---|
| 0x140 | 4 | u32 | Magic. Must be 0x00005442 (bytes `42 54 00 00`). The client refuses any other value. | all | C, BIN |
| 0x144 | 4 | u32 | Minimum allocation size ("block size"), see [§4.4](#s4-4). Must be a multiple of 4 (the client refuses to create an archive otherwise). | 0x100, 0x400, 0x1000, 0x4400, 0x5DC0, 0xA000 | C, BIN |
| 0x148 | 4 | u32 | Length of the managed file in bytes; equals the real file length | equal in all 29 | C |
| 0x14C | 8 | — | Resource identifier, see [§3.4](#s3-4) | | C, BIN |
| 0x154 | 4 | u32 | Version 112: offset of the free-space table allocation. Version 111: offset of the first block of the free chain. 0xFFFFFFFF: free table invalid ([§4.5](#s4-5)). | | C, BIN |
| 0x158 | 4 | u32 | Version 111: offset of the last block of the free chain. Version 112: no function; may keep a stale value from an earlier version-111 life of the archive ([§4.5](#s4-5)). | 0 in 25 archives; 0x119C in `cell_5`, `cell_15`, `map_5`, `map_15` (2026-10-05; 0 in all on 2026-10-03) | C, BIN |
| 0x15C | 4 | u32 | Number of free extents (version 112: pairs in the table; version 111: blocks in the chain) | 1 … 91 | C, BIN |
| 0x160 | 4 | u32 | Offset of the root directory node allocation | | C |
| 0x164 | 8 | 2 × u32 | No function in this client (never read or written except as part of the whole superblock) | 0 in all | C (value), BIN |
| 0x16C | 4 | u32 | Pending-journal checksum. 0 = no pending journal ([§3.6](#s3-6)) | 0 in all | C (value), BIN |
| 0x170 | 4 | u32 | Master DID map. Non-zero only in `general`, where it is 0x28000000, the root DID map of `records.md` §8.2; 0 elsewhere | | C (value), R (client use) |
| 0x174 | 4 | u32 | Container version: 112 current layout, 111 older layout ([§3.7](#s3-7)) | 112 in all | C, BIN |
| 0x178 | 4 | u32 | Game data version | 3 for portal-type and language archives; 0 for cell and map | C (values), BIN (compared on open) |
| 0x17C | 16 | bytes | Version stamp, part 1: a 16-byte identifier. One value is shared by 27 archives. All zero in the two most recently created aux parts (`mesh_aux_1`, `highres_aux_3`). | | C (values), BIN (opaque) |
| 0x18C | 4 | u32 | Version stamp, part 2: a counter belonging to the identifier at 0x17C | 1100 or 17139; 0 where 0x17C is zero | C (values), BIN (opaque) |
| 0x190 | 4 | u32 | Superblock length 104 | 104 in all | C, BIN |
| 0x194 | 4 | u32 | Directory entry size 32 | 32 in all | C, BIN |
| 0x198 | 4 | u32 | Allocation-header and child-pointer size 8 | 8 in all | C, BIN |
| 0x19C | 4 | u32 | Total free payload bytes = sum over free extents of (extent size − 8) | equal in all 29 | C |
| 0x1A0 | 4 | u32 | Version 112: size of the free-space table allocation. Version 111: size of the first free block. | | C, BIN |
| 0x1A4 | 4 | u32 | Size in bytes of the root node allocation at 0x160 | 2460 when 0x144 < 2460, else equal to 0x144 | C |
| 0x1A8 | 600 | bytes | Padding to 0x400 | all zero in all 29 | C |

Notes:
- **0x17C–0x18F** form one 20-byte stamp. The client stores it and returns it on request, and
  copies it when it writes the header. It does not compare it with anything when it opens an
  archive. A reader treats it as opaque. **BIN**
- **0x190, 0x194, 0x198** are written as 104, 32 and 8 when an archive is created. After every
  header read the client overwrites its in-memory copy with 104, 32 and 8, whatever the file
  holds, and it never compares them. They are therefore not self-describing in practice: a
  reader must use the fixed sizes of this spec and must not refuse an archive because these
  fields differ. A validator may warn. **BIN**
- **0x164–0x16B** are preserved, but not used, by the client. **BIN**
- Sources call 0x154 the "free-list head" or "journal offset". In version 112 it points to a
  **table** of free extents ([§4.5](#s4-5)). **C**, **BIN**
- The 0x1A4 relationship is a consequence of the allocation rule ([§4.4](#s4-4)), not a
  separate requirement. A reader must take the root size from 0x1A4 and not assume 2460.

<a id="s3-4"></a>
### 3.4 Resource identifier (0x14C–0x153)

The client treats the 8 bytes at 0x14C as one 64-bit **resource identifier** that names the
archive's family, kind and subset. **BIN**, **C**

| Offset | Size | Type | Meaning | Values observed | Mark |
|---|---|---|---|---|---|
| 0x14C | 2 | u16 | Variant number. The client can register several archives of the same family, kind and subset with different variants and searches them in ascending variant order ([§8.2](#s8-2)). All archives of one family must carry the same variant. | 0 in all | C (value), BIN |
| 0x14E | 1 | u8 | Archive kind (table below) | see below | C, BIN |
| 0x14F | 1 | u8 | Dataset type: 1 = portal, 2 = region, 3 = language | 1, 2, 3 | C, BIN |
| 0x150 | 4 | u32 | Subset: portal 0; region archives the region number; language archives the folded language code (below) | 0, 1–5, 14, 15, 2 | C, BIN |

The client's archive table (**BIN**; the names are the file-name parts of [§2.1](#s2-1) and
are needed for interoperability). "Default minimum allocation" is the 0x144 value the client
uses when it creates an archive of that kind. Every observed archive's 0x144 equals its kind's
default. **C**

| Dataset type (0x14F) | Kind (0x14E) | Name | Default minimum allocation | Present in install |
|---|---|---|---|---|
| 1 | 0x01 | gamelogic | 0x100 | yes |
| 1 | 0x02 | mesh | 0x4400 | yes |
| 1 | 0x03 | general | 0x400 | yes |
| 1 | 0x04 | anim | 0x5DC0 | yes |
| 1 | 0x07 | sound | 0x4400 | yes |
| 1 | 0x08 | highres | 0xA000 | yes |
| 1 | 0x09 | surface | 0x1000 | yes |
| 2 | 0x03 | cell | 0x100 | yes |
| 2 | 0x0A | map | 0x100 | yes |
| 2 | 0x0D | — (no file name is needed: never opened) | 0x1000 | no (not opened by this client) |
| 3 | 0x03 | local | 0x100 | yes |

Kind values 0x05, 0x06, 0x0B and 0x0C do not occur in the table and are not used by this
client build. **BIN**

**Folded language code.** For a language archive, the subset is the 32-bit language code
folded to 16 bits: the high 16 bits OR-ed into the low 16 bits, giving a value below 0x10000.
For the codes of [§2.1](#s2-1): English 2 → 2; EN_GB 0x10000001 → 0x1001; FR 0x10000003 →
0x1003; DE 0x10000004 → 0x1004. **BIN** (rule), **C** (English = 2)

Worked example (synthetic): an `EN_GB` language archive carries, at 0x14C–0x153, the bytes
`00 00 03 03 01 10 00 00` (variant 0, kind 3, type 3, subset 0x1001). A `cell_14` archive
carries `00 00 03 02 0E 00 00 00`.

- Aux parts carry the same identifier as their base. **C**
- 0x150 equals the number in the file name for every cell and map archive. **C**

<a id="s3-4-1"></a>
#### 3.4.1 Older identifier encoding

The client also accepts an older encoding of the identifier, in which bytes 0x14E and 0x14F
are both zero (**BIN** only, P48; no archive in the install uses it). It recognises the
encoding when 0x14E and 0x14F are both zero and the u32 at 0x14C is not zero. Let *t* be the
u16 at 0x14C, which in this encoding is the dataset type. The client converts as follows:

| Condition | Dataset type | Kind | Subset | Variant |
|---|---|---|---|---|
| *t* = 1 or *t* = 4 | *t* | byte at 0x150 | 0 | 0 |
| any other *t* | *t* | byte at 0x152 | u16 at 0x150 | 0 |

- The conversion happens in memory when the archive is opened. The file is not changed by it.
- The converted identifier then goes through the table check of [§2.4](#s2-4). Type 4 has no
  row in the table above, so an archive converted to type 4 is refused (unless the opener
  skips the check).
- If the whole u32 at 0x14C is zero as well, nothing is converted. The identifier then has
  dataset type 0, which fails the table check, and the client refuses the archive unless the
  opener skips the check.

Reader rule: apply the same conversion in memory, report the archive as using the older
encoding (Info), never write the converted value back, and then check the converted
identifier against the file name as for any archive.

Worked examples (synthetic): bytes `01 00 00 00 03 00 00 00` at 0x14C–0x153 convert to type 1,
kind 3, subset 0 (`general`). Bytes `02 00 00 00 0E 00 03 00` convert to type 2, kind 3,
subset 14 (`cell_14`). Bytes `03 00 00 00 02 00 03 00` convert to type 3, kind 3, subset 2
(the English language archive).

<a id="s3-5"></a>
### 3.5 Per-archive header values (observed 2026-10-05)

Hex unless noted. "G" = 0x17C identifier is non-zero (the shared value), "0" = all zero.

| Archive | 0x144 | 0x148 (= length) | 0x14C (u32) | 0x150 | 0x154 | 0x158 | 0x15C | 0x160 | 0x178 | 0x17C | 0x18C | 0x19C | 0x1A0 | 0x1A4 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| anim | 5DC0 | 2BC00000 | 01040000 | 0 | 2BA0265C | 0 | 2 | 02E38D0C | 3 | G | 44C | 1607020 | 5DC0 | 5DC0 |
| cell_1 | 100 | 18D00000 | 02030000 | 1 | 0C56FFD8 | 0 | 3D | 0CB773D4 | 0 | G | 44C | CC6F0 | 5E1C | 99C |
| cell_2 | 100 | 1A100000 | 02030000 | 2 | 04D6D224 | 0 | 2C | 0E7D3BD4 | 0 | G | 42F3 | 184E94 | 5E2C | 99C |
| cell_3 | 100 | 1C500000 | 02030000 | 3 | 12E873E0 | 0 | 5B | 12792418 | 0 | G | 42F3 | 173F30 | 3F44 | 99C |
| cell_4 | 100 | 26300000 | 02030000 | 4 | 03582DE8 | 0 | 10 | 1969BDC8 | 0 | G | 42F3 | 61A4C | 5E3C | 99C |
| cell_5 | 100 | 19B00000 | 02030000 | 5 | 0000119C | 119C | 1 | 1186C4D0 | 0 | G | 42F3 | FF3B8 | 1F60 | 99C |
| cell_14 | 100 | 03E00000 | 02030000 | E | 03D72A10 | 0 | 1E | 02B4BDB8 | 0 | G | 42F3 | C34D0 | 1F60 | 99C |
| cell_15 | 100 | 00100000 | 02030000 | F | 0000129C | 119C | 2 | 00000400 | 0 | G | 42F3 | FCEEC | 1F68 | 99C |
| gamelogic | 100 | 3EC00000 | 01010000 | 0 | 0EE6A194 | 0 | 1 | 035D6C3C | 3 | G | 44C | 107D00 | 5A968 | 99C |
| general | 400 | 2E900000 | 01030000 | 0 | 221EECEC | 0 | 1 | 0E63DE34 | 3 | G | 44C | 8A4B4 | 8518 | 99C |
| highres | A000 | FF900000 | 01080000 | 0 | FF62A094 | 0 | 1B | 1A8B6F94 | 3 | G | 44C | 2EAB98 | A000 | A000 |
| highres_aux_1 | A000 | FF900000 | 01080000 | 0 | FF61ACE4 | 0 | 1 | 144F187C | 3 | G | 44C | 2EBF70 | A000 | A000 |
| highres_aux_2 | A000 | FF800000 | 01080000 | 0 | 8E64BD80 | 0 | 1 | 24295C98 | 3 | G | 44C | 1EF1E8 | A000 | A000 |
| highres_aux_3 | A000 | 21A00000 | 01080000 | 0 | 0000A400 | 0 | 1 | 054532C4 | 3 | 0 | 0 | 2BD160 | A000 | A000 |
| local_English | 100 | 71978CA8 | 03030000 | 2 | 4C52C7FC | 0 | 1 | 4DC5260C | 3 | G | 44C | 6B5E8 | 128F8 | 99C |
| map_1 | 100 | 01600000 | 020A0000 | 1 | 014F6184 | 0 | 25 | 0009E364 | 0 | G | 44C | 131818 | 1F60 | 99C |
| map_2 | 100 | 01600000 | 020A0000 | 2 | 014F6084 | 0 | 16 | 0133459C | 0 | G | 42F3 | 113F9C | 1F60 | 99C |
| map_3 | 100 | 01600000 | 020A0000 | 3 | 014F6084 | 0 | 7 | 0133459C | 0 | G | 42F3 | 10AFF0 | 1F60 | 99C |
| map_4 | 100 | 01600000 | 020A0000 | 4 | 014F6084 | 0 | 12 | 0009E364 | 0 | G | 42F3 | 1448BC | 1F60 | 99C |
| map_5 | 100 | 01500000 | 020A0000 | 5 | 0000119C | 119C | 1 | 000A03C4 | 0 | G | 42F3 | 56AE8 | 1F60 | 99C |
| map_14 | 100 | 01600000 | 020A0000 | E | 014F6084 | 0 | 1 | 0133459C | 0 | G | 42F3 | 108014 | 1F60 | 99C |
| map_15 | 100 | 00100000 | 020A0000 | F | 0000129C | 119C | 2 | 00000400 | 0 | G | 42F3 | FCEEC | 1F68 | 99C |
| mesh | 4400 | FF800000 | 01020000 | 0 | E25FC934 | 0 | 2 | 07912188 | 3 | G | 44C | D052C | 4400 | 4400 |
| mesh_aux_1 | 4400 | 63C00000 | 01020000 | 0 | 00004800 | 0 | 1 | 1613FA7C | 3 | 0 | 0 | 16C9A4 | 4400 | 4400 |
| sound | 4400 | FF900000 | 01070000 | 0 | FF73A94C | 0 | 2 | 0AFA73A4 | 3 | G | 44C | 1D28E8 | 4400 | 4400 |
| sound_aux_1 | 4400 | E8900000 | 01070000 | 0 | 667431A4 | 0 | 2 | C62BEFC8 | 3 | G | 44C | DC0B0 | 4400 | 4400 |
| surface | 1000 | FF900000 | 01090000 | 0 | F0B137DC | 0 | 3 | 2F8D0EF0 | 3 | G | 44C | 131DF0 | 1DE78 | 1000 |
| surface_aux_1 | 1000 | FF900000 | 01090000 | 0 | 2504C044 | 0 | 3 | 20952150 | 3 | G | 44C | 131E84 | 18FB0 | 1000 |
| surface_aux_2 | 1000 | 9D900000 | 01090000 | 0 | 114698A0 | 0 | 1 | 22EB2924 | 3 | G | 44C | EDEC8 | 3FCC | 1000 |

Common to all 29: 0x140 = 5442, 0x164–0x16F = 0, 0x174 = 0x70 (112), 0x190 = 0x68,
0x194 = 0x20, 0x198 = 8. 0x170 = 0x28000000 in `general`, 0 elsewhere. The 16-byte value
marked G is the same in all 27 archives that have one.

- Every file length is a multiple of 1 MiB except `local_English` (0x71978CA8). **C**
- The largest files are 0xFF900000 bytes (4 GiB − 7 MiB). **C**
- Layout fields (lengths, 0x154–0x160, 0x19C, 0x1A0) differ from the 2026-10-03 observation
  for most archives, because the content was re-applied on 2026-10-04 with different placement.
  The entry totals per set, the identity fields 0x14C–0x150, 0x174, 0x178 and 0x17C–0x18F did
  not change. **C**

<a id="s3-6"></a>
### 3.6 Pending journal (0x16C and the `.jrnl` file)

The client can perform a set of writes as one **journalled transaction**. The redo log is a
separate file whose name is the archive's full file name with `.jrnl` appended (for example
`client_general.dat.jrnl`), in the same directory. **BIN**

**Commit sequence used by the writer** (shown so that a reader can interpret the states):

1. Write the complete journal file and flush it.
2. Store the journal's checksum (below) in header 0x16C and flush the header. From now on
   0x16C ≠ 0 means "a transaction may be half-applied; the journal holds every byte it will
   write".
3. Apply the writes to the archive and flush.
4. Set 0x16C to 0, flush, and delete the journal file.

So **0x16C = 0** means the archive is consistent, and **0x16C ≠ 0** means an update was
interrupted between steps 2 and 4. All observed archives have 0x16C = 0 and no journal file.
**C**, **BIN**

**Journal file layout** (all little-endian):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 8 | u64 | Length the archive must have after the transaction. If bit 63 is set, only the low 32 bits count. |
| 8 | … | records | Zero or more records, back to back, until the end of the file |

Record:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| +0 | 4 | u32 | Data length *n* (may be 0) |
| +4 | 4 | u32 | Unused |
| +8 | 8 | u64 | Absolute archive offset where the data goes |
| +16 | *n* | bytes | Data |

**Checksum.** The checksum covers every byte of the journal file in file order: the 8-byte
header, then for each record its 16-byte header followed by its *n* data bytes. It is computed
with the running cipher of the Type 1 font encryption (AT1), keeping the sum of the cipher
bytes:

1. Start with a 16-bit key *r* = 55665 and a 32-bit sum *s* = 0. The constants are
   *c1* = 52845 and *c2* = 22719.
2. For each byte *b*: the cipher byte is *b* XOR (*r* shifted right by 8). Add the cipher byte
   to *s* (modulo 2³²). Set *r* = ((cipher byte + *r*) × *c1* + *c2*) modulo 65536.
3. If *s* is 0 at the end, use 1. The result is the value stored in 0x16C.

Test vectors (synthetic): the empty file gives 1; the single byte 0x00 gives 0xD9; the bytes
`00 01 02 03` give 0x239. A 28-byte journal with header 0x0000000000100000 and one record
writing the 4 bytes `41 42 43 44` at offset 0x400 (bytes
`00 00 10 00 00 00 00 00 04 00 00 00 00 00 00 00 00 04 00 00 00 00 00 00 41 42 43 44`) gives
0xF31. **BIN** (algorithm), AT1 (constants)

**Validation and replay by the client.** When the client opens an archive for writing **with
journalling enabled** and finds 0x16C ≠ 0, it:

1. requires the journal file to exist;
2. reads it completely: the header, then records until the end of the file; any short read or
   leftover byte is an error;
3. requires the computed checksum to equal 0x16C;
4. extends the archive to the header's length if that is larger than the current length;
5. writes the records' data at their offsets, where a later record's bytes replace an earlier
   record's bytes wherever the two overlap, except that the 4 bytes at archive offset 0x16C
   keep their current value;
6. sets 0x16C to 0, flushes, and deletes the journal.

If any check fails, the open fails. When journalling is enabled and 0x16C = 0, a leftover
journal file is deleted. The game client itself opens its archives **without** journalling,
so it does not replay; journalled opens are made by other users of the same library. **BIN**

**What a read-only reader does:**
- 0x16C = 0: nothing to do. A journal file next to the archive is stale and must be ignored.
- 0x16C ≠ 0: the archive may be half-updated. The default is to refuse the archive with a clear
  "pending update" error. A reader may instead offer an option to apply the journal **in
  memory**: validate it as in steps 1–3, then present a view in which the journal's records
  replace the archive bytes at their offsets (later records win; bytes 0x16C–0x16F keep the
  file's value), with logical length = max(file length, header length). It must never write
  the replay to disk.

<a id="s3-7"></a>
### 3.7 Checks the client makes when it opens an archive

| Check | Client behaviour | Recommended reader behaviour | Mark |
|---|---|---|---|
| Magic at 0x140 | refuses | Error | BIN, C |
| 0x174 version | 111 and 112 are interchangeable: a version-111 archive is read with the chained layout everywhere ([§4.3](#s4-3)); opened for writing in a particular mode it is converted to 112 in place. Compared with the opener's expected container version, when that is not negative: a difference refuses the archive, unless the expected value and 0x174 are 111 and 112 in either order (see "Expected datpack versions" below). Whether a value other than 111 or 112 is accepted when no comparison is made was not shown (OPEN below). | Accept 112; accept 111 with the chained layout; Warn and refuse other values | BIN, C (112) |
| 0x178 game data version | compared with the opener's expected game data version, when that is not negative and the opener does not ask to skip it; a difference refuses the archive, unless the expected container version and 0x174 are 111 and 112 in either order, which waives the refusal even though the game data versions differ | Warn if not 3 (portal, language) or 0 (region) | BIN, C |
| Resource identifier | the older encoding is converted first ([§3.4.1](#s3-4-1)); then type and kind must be a pair from the client's table; one variant per family; the check is skipped only on request | Convert the older encoding (Info); Warn on a mismatch with the file name | BIN, C |
| 0x154 = 0xFFFFFFFF (version 112) | refuses (free table invalid) | Warn; read records normally; skip free-space checks | BIN |
| 0x16C ≠ 0 | journalled writable opens replay ([§3.6](#s3-6)); other opens ignore it | Error by default ([§3.6](#s3-6)) | BIN |
| 0xFFFF0001 present and decodable | required when the opener asks for the iteration list, which the game client does | Warn | BIN, C |
| 0xFFFF0003 present | warns only | Info | BIN, C |
| 0x148 versus file length | used as the end of the managed area | Warn on mismatch | C |
| 0x190–0x198 | ignored ([§3.3](#s3-3)) | Warn on values other than 104/32/8 | BIN |
| Version string, 0x100 marker | ignored | ignore | BIN |

The archive library opens a file in one of two modes (**BIN**, P50):
- **read-write**: read and write access, and no sharing, so no other process can open the file
  while it is held;
- **read-only**: read access only, and other processes may open the file for reading.

The game client opens its archives in the read-write mode unless it is configured to be
read-only. A reader of ours opens read-only and allows sharing. It must expect the open to fail
while the official client runs, and report that clearly.

**Expected datpack versions** (**BIN**, Q20, P55; **C** by LOG). The opener passes an expected
container version (compared with 0x174), an expected game data version (compared with 0x178) and
a request to skip the game-data comparison. A negative expected value disables that comparison.
The rule, re-read on 2026-10-07:

1. **Container comparison fails** when the expected container version is not negative and differs
   from 0x174.
2. **Game-data comparison fails** when the expected game data version is not negative, the opener
   has not asked to skip it, and it differs from 0x178.
3. If **either** comparison fails, the archive is refused. Either one alone is enough.
4. **Waiver:** the refusal of step 3 does not happen when the expected container version and 0x174
   are 111 and 112 in either order (expected 111 with 0x174 = 112, or expected 112 with
   0x174 = 111). The waiver applies even when the game-data comparison failed. It does not apply
   when the two container versions are equal: expected 112 with 0x174 = 112 and a different 0x178
   is refused.

Worked cases (synthetic): expected (112, 3) against a file with (112, 0) is refused; expected
(111, 3) against (112, 0) is accepted; expected (−1, 3) against (112, 0) is refused; expected
(−1, −1) against any pair is not compared.

- The game client's archive initialisation passes −1 for both values for every archive family
  (portal, region, language). The client's own log of its run of 2026-10-04 reports the expected
  values −1 and −1 for each of the 29 archives it opened (read 2026-10-07). So the game client
  compares neither 0x174 nor 0x178 with an expected value when it opens an archive.

- The client keeps separate expected-value slots per family (the portal and language families
  share one container-version slot, the region family has its own; the portal and region families
  share one game-data slot, the language family has its own). All of them start at −1. A code path
  that stores other values into them exists, but the game client's run does not take it (the log
  shows −1), and it was not traced further.
- 0x174 still selects how the free space is kept (version 111 chain, version 112 table,
  [§4.5](#s4-5)); that is a layout decision, not a version check.

> **OPEN:** Whether the client opens an archive whose 0x174 is neither 111 nor 112 when no
> expected container version is given (the game client's case) was not shown. **Reader rule:**
> refuse such an archive with a clear message (the 0x174 row of the table above), whatever the client does;
> no archive in the install has such a value (29 of 29 hold 112, 2026-10-05).

Reader rule (unchanged): accept 112 and 111; for 0x178, warn when the value is not 3 (portal and
language) or 0 (region). The warning reflects what the install holds, not a client refusal.

---

<a id="s4"></a>
## 4. Allocation (extent) layer

Every object in the file after the header is an **allocation**: a directory node, a
record's stored bytes, the free-space table, or a free extent. An allocation is a contiguous
byte range `[offset, offset + size)`. Its size is always known from whoever points at it:

| Allocation | Offset from | Size from |
|---|---|---|
| Root node | header 0x160 | header 0x1A4 |
| Child node | parent child-pointer `.offset` | parent child-pointer `.size` |
| Record (first extent or first block) | entry +8 | entry +24 ([§5.2](#s5-2)) |
| Record fragment (extent layout) | fragment table pair `.offset` | pair `.length` |
| Next block (chained layout) | previous block's header word +4 | previous block's header word +0 |
| Free-space table | header 0x154 | header 0x1A0 |
| Free extent | free table pair `.offset` | pair `.size` |

Every allocation lies inside `[0x400, file length)`. The header ends at 0x3FF and the writer
places the first free extent of a new archive at 0x400. **C**, **BIN**

<a id="s4-1"></a>
### 4.1 Allocation header (8 bytes)

Directory nodes, records (first extent or every block) and the free-space table begin with an
8-byte header. Fragments of the extent layout and free extents of version 112 do **not**
([§4.2](#s4-2), [§4.5](#s4-5)).

The header has two readings. Which one applies is **not** decided by the header itself:

| Allocation | Layout used | Mark |
|---|---|---|
| Directory node (any archive) | chained ([§4.3](#s4-3)) | BIN, C |
| Record in a version-112 archive, entry flag bit 1 set | extent ([§4.2](#s4-2)) | BIN, C |
| Record in a version-112 archive, entry flag bit 1 clear | chained ([§4.3](#s4-3)) | BIN, C |
| Any allocation in a version-111 archive | chained | BIN |
| Free-space table | its own layout ([§4.5](#s4-5)); header `{0, 0}` | C |

| Offset | Size | Type | Extent layout | Chained layout |
|---|---|---|---|---|
| +0 | 4 | u32 | `fragCount` F: number of extra fragments | size of the next block, 0 if none |
| +4 | 4 | u32 | 0 (not read by the client) | offset of the next block, 0 if none; bit 0 set = free tag (error) |

Observed (**C**, 2026-10-05): all 112,267 directory nodes and the 29 free-space tables have
`{0, 0}`. Every record with flag bit 1 set has word +4 = 0. The 8 records with flag bit 1
clear have `{0, 0}` (a single block). A header of `{0, 0}` reads identically in both
layouts.

<a id="s4-2"></a>
### 4.2 Extent layout: first extent plus fragment table

Let `O` be the allocation offset, `A` its size (from the table above), `F` = `fragCount`, and
`L` the logical payload length (for records: entry +12).

Layout of the first extent:

| Range | Content |
|---|---|
| `O … O+8` | allocation header `{F, 0}` |
| `O+8 … O+A−8F` | first payload bytes, capacity `A − 8 − 8F` |
| `O+A−8F … O+A` | fragment table: `F` pairs of `{u32 length, u32 offset}`, in payload order |

Each fragment is a raw byte range `[offset, offset+length)` that continues the payload. It
has no header of its own. **C**, **BIN**

Plain-words algorithm to reassemble a payload of length `L`:

1. Read the 8-byte header at `O`. Let `F` = word +0. (Word +4 is not consulted.)
2. Check that `8 + 8F ≤ A` (reject otherwise).
3. Take `min(A − 8 − 8F, L)` bytes starting at `O+8`. These are the first payload bytes.
4. If more is needed, read the `F` pairs from the last `8F` bytes of the first extent
   (pair 0 first). For each pair in order, append `min(length, bytes still needed)` bytes
   from `offset`. Stop when `L` bytes are collected.
5. If all pairs are used and fewer than `L` bytes were collected, the record is corrupt.

Worked example (synthetic): `A` = 32, `L` = 20, header `{1, 0}`, pair `{8, 0x2000}` at
`O+24`. Step 3 takes 16 bytes from `O+8 … O+24`; step 4 takes 4 bytes from 0x2000.

Limits:
- The writer allocates at most `0x144 / 8` extents for one record (first extent included), so
  `F ≤ 0x144 / 8 − 1`: 31 for a minimum allocation of 0x100, 127 for 0x400, 511 for 0x1000,
  2175 for 0x4400. This keeps the fragment table inside a minimum-size first extent. **BIN**
- The client's reader enforces no maximum and does not check `8 + 8F ≤ A`. A reader must check
  step 2 and may warn when `F` exceeds `0x144 / 8 − 1`. **BIN**

Observations (**C**, 2026-10-05):
- 256 records are fragmented: `general` 3 (F = 1), `local_English` 204 (F 1–31; 11 records
  have F = 31, the writer's limit for that archive), `mesh` 39 (F 1–2), `sound_aux_1` 2
  (F = 1), `surface` 8 (F = 1). All 256 reassemble: the 47 compressed ones inflate to exactly
  their prefixed length, and the 209 stored ones begin with their own DID.
- On 2026-10-03 the distribution was different (282 fragmented records, up to F = 19 in
  `mesh`), because the same content was then placed differently.
- The first extent's capacity plus all fragment lengths is ≥ `L`. Fragment lengths are
  multiples of 4 and ≥ the minimum allocation size.
- When `F > 0`, the entry's allocated size (+24) is the size of the **first extent only**
  and is smaller than `L + 8`.
- No archive exceeds its writer limit for `F`.

<a id="s4-3"></a>
### 4.3 Chained layout

An allocation in the chained layout is a chain of **blocks**. Each block starts with the
8-byte header `{next size, next offset}` and carries `size − 8` payload bytes, where `size` is
the block's own size as given by whoever points at it (the entry or node pointer for the
first block, the previous block's header for the others). The chain is stored
**tail-first**: the first block holds the **end** of the payload. **BIN**

Plain-words algorithm to read a payload of length `L` (for a directory node, `L` = 2452):

1. Let `remaining` = `L`. The current block is `{size, offset}` from the entry or pointer.
2. While the current offset is not 0 and `remaining` > 0:
   1. Read the block's 8-byte header.
   2. Let `c` = min(`size − 8`, `remaining`). Subtract `c` from `remaining`.
   3. Copy the block's **first** `c` payload bytes (from block offset +8) to payload positions
      `remaining … remaining + c`.
   4. If the header's next offset has bit 0 set, the archive is corrupt (that bit tags free
      blocks). Otherwise the next block is `{header word +0, header word +4}`.
3. If the chain ends while `remaining` > 0, the payload is incomplete. The client does not
   detect this; a reader must report it as corrupt.

The client sets no limit on the number of blocks. A reader must bound the walk (for example
to 1000 blocks, as one source does), detect revisited offsets, and treat a block whose size is
8 or less as corrupt (it would contribute no payload).

Worked example (synthetic): `L` = 10. The entry points to block X `{size 12, offset 0x400}`
whose header is `{16, 0x40C}` and whose 4 payload bytes are `47 48 49 4A`. Block Y at 0x40C,
size 16, has header `{0, 0}` and payload `41 42 43 44 45 46 ?? ??`. Step 2 for X: `c` = 4,
`remaining` = 6, payload[6…10] = `47 48 49 4A`. For Y: `c` = min(8, 6) = 6, `remaining` = 0,
payload[0…6] = `41 42 43 44 45 46`. Result: `41 … 4A`.

Where it applies ([§4.1](#s4-1)): every directory node, records whose flag bit 1 is clear, and
everything in a version-111 archive. **BIN**

Observed (**C**): every directory node and all 8 records with flag bit 1 clear are single
blocks with header `{0, 0}`. No multi-block chain occurs in the install, so the algorithm above
rests on BIN alone for chains longer than one block.

<a id="s4-4"></a>
### 4.4 Allocation sizing rule

Header 0x144 is the **minimum allocation size**. It is not a fixed block size: allocations are
not multiples of it, and offsets are aligned only to 4. **C** The writer follows these rules
(**BIN**, **C**):

> allocated size = max(0x144 value, (payload length + 8) rounded up to a multiple of 4)

- It takes the **smallest** free extent that is large enough. If what is left over is at least
  0x144 bytes, the rest stays free as a new extent; otherwise the whole extent is used, which
  creates "slack".
- If no single free extent is large enough, it builds the record from several extents (up to
  the limit of [§4.2](#s4-2)), taking the largest free extents first, and grows the file for
  the remainder.

Evidence (**C**, 2026-10-05):
- Unfragmented record entries: 3,780,330 match the rule exactly. 58,371 are larger than the
  rule gives (slack). None are smaller. The other 256 are fragmented.
- Directory nodes are 2452 + 8 = 2460 bytes, or 0x144 when that is larger. 340 nodes are
  larger still (reused extents); in `general`, 25 of 7,983 nodes are 2504–3480 bytes.
- All record offsets are multiples of 4.

Readers must not depend on the rule. It only explains the data. They must always use the
size given by the pointer or entry.

<a id="s4-5"></a>
### 4.5 Free space

**Version 112: free-space table.**

| Item | Location | Content |
|---|---|---|
| Table allocation | offset = header 0x154, size = header 0x1A0 | 8-byte header `{0, 0}`, then header 0x15C pairs `{u32 size, u32 offset}`, then a terminating pair `{0, 0}` |
| Free extents | each pair | raw stale bytes (no header, no chain pointer) |

- Capacity: the table can hold `(0x1A0 − 16) / 8` pairs (header and terminator take 16
  bytes). A reader needs only the first 0x15C pairs and must check
  `16 + 8 × count ≤ size`. **BIN**, **C**
- Pairs are sorted by ascending offset. They are not sorted by size. **C**, **BIN**
- When the writer saves the table and `16 + 8 × count` is **not** smaller than the current
  size, it frees the old table extent and allocates a new one of size
  max(0x144, 8 × count′ + 8024), where count′ is the number of free extents after the old table
  was freed. This explains the common table sizes 0x1F60 and 0x1F68 (one or two free extents
  at the time). **BIN**, **C**
- While the writer holds unsaved changes to the free list, it writes the header with
  **0x154 = 0xFFFFFFFF**; it restores the real offset when it saves the table. A version-112
  archive whose 0x154 is 0xFFFFFFFF was therefore not closed cleanly. The client refuses to open
  it. A reader can still read every record (the directory does not depend on the free list) but
  must not read the table and must skip I-20…I-22. **BIN**

How the client reads the table (**BIN**, Q18, P53):
- It does not read the table to open an archive or to read records. It loads the table the
  first time the writer needs to allocate or free space. Opening checks only the
  0xFFFFFFFF marker above.
- **No table.** Offset (0x154) 0 **and** size (0x1A0) 0 mean "no table yet": the free list is
  empty and nothing is read or checked. The writer is in this state right after it creates an
  archive, and right after it converts a version-111 archive. The next save writes a table
  (the relocation rule above, since 16 + 8 × count is never below a size of 0). No archive in
  the install is in this state.
- **Otherwise** it reads the table's (0x1A0 − 8) bytes after the 8-byte allocation header. It
  never looks at the header itself. It takes pairs one after another and stops at the first
  pair whose **size** is 0. It does not stop at the count in 0x15C.
- Its one consistency test: 8 × (number of pairs taken) + Σ (size − 8) over those pairs must
  equal 8 × 0x15C + 0x19C, which is the same as "the sizes sum to 8 × 0x15C + 0x19C". If the
  test fails, the write that needed the table fails. No test of bounds, order or minimum size
  was found in the loader.

Reader rules that follow:
- 0x154 = 0 and 0x1A0 = 0 in version 112: no table. Report Info, skip I-21, and run I-20 and
  I-22 with an empty free list. 0x15C or 0x19C non-zero in that state: Warn (I-22).
- Exactly one of 0x154 and 0x1A0 zero: the table is unusable. Warn (I-21), and skip the I-20
  and I-22 checks that depend on it.
- Read pairs up to the first pair with size 0, as the client does. That pair must be the one
  at index 0x15C, and it should be `{0, 0}`. A zero size earlier, or none within the table:
  Warn (I-21).

Observed (**C**, all 29 archives, 2026-10-05):
- Table header `{0, 0}`; terminating pair `{0, 0}`; `16 + 8 × count` < 0x1A0.
- Every free extent's size is ≥ the minimum allocation size.
- Header 0x19C equals the sum over pairs of (size − 8).
- Free extents contain stale data. Their first 8 bytes are **not** a chain link.
- The last free extent often, but not always, ends at end-of-file.
- (2026-10-06, 29 of 29) 0x154 and 0x1A0 non-zero; the first pair with size 0 is the pair at
  index 0x15C; the client's consistency test holds.

**Version 111: free chain** (**BIN**; matches DO).
- 0x154 is the first free block, 0x158 the last, 0x15C the number of blocks, 0x1A0 the size
  of the first block, 0x19C the total of (size − 8).
- Each free block begins with `{u32 next size, u32 next offset}`; the next offset carries bit 0
  set as a "free" tag, which the reader masks off. The chain ends at next size 0.

**Conversion.** When a version-111 archive is opened for writing in the converting mode, the
writer walks the chain, moves every block into the table, checks that
`0x19C + 8 × 0x15C` is unchanged, sets 0x174 to 112 and rewrites the header. It does not clear
0x158. **BIN** The four archives with 0x158 = 0x119C ([§3.5](#s3-5)) are consistent with this
history: they still hold two records in the chained layout (flags 0, [§5.3](#s5-3)), and 0x119C
is an earlier location of their free table. **C** (values), inferred (history)

<a id="s4-6"></a>
### 4.6 Tiling

In every observed archive, the union of the following covers `[0x400, file length)`
**exactly**, with no overlaps and no gaps:
- all directory node allocations (pointer sizes),
- all record first extents (entry +8, entry +24),
- all record fragments,
- the free-space table (0x154, 0x1A0),
- all free extents.

**C** (29/29 archives, on both dates). This is the strongest available consistency check
([§10](#s10), I-20).

<a id="s4-7"></a>
### 4.7 File growth and the size limit (writer facts)

These rules explain archive lengths and aux parts. A reader needs none of them. **BIN** (P51)

- For each record write the writer computes *need* = record length +
  50 × max(2452, 0x144). For example, with 0x144 = 0x100 and a record of 1,000 bytes,
  *need* = 1,000 + 50 × 2452 = 123,600 bytes.
- An archive is "full" when its configured maximum length, minus its current length (0x148),
  plus its free bytes (0x19C), minus 8 MiB (0x800000), is smaller than *need*. The first three
  terms are summed modulo 2³². If that sum is below 8 MiB, the subtraction wraps and the test
  does not report the archive as full. The 8 MiB headroom, and so this test, does not apply
  to metadata records (DID ≥ 0xFFFF0000). A full archive refuses the write.
- If the free bytes are not more than *need*, the writer grows the file by
  (*need* − free bytes + 1 MiB) rounded down to a multiple of 1 MiB (0x100000). In the example,
  with 20,000 free bytes, the growth is (123,600 − 20,000 + 1,048,576) rounded down = 1 MiB.
  If a maximum length is configured (not 0) and the grown length would exceed it, the archive
  is full and refuses the write. New space becomes a free extent.
- A set writes a record to the first part, in index order, that accepts it. If no part
  accepts it, the set creates the next aux part (`_aux_<count>.datx`) and writes there.

**Configured maximum length** (**BIN**, Q21, P56). The maximum is a u32 set per archive by the
opener. A value of 0 acts like 2³² in the full test (the sum is taken modulo 2³²) and disables the
check on growth.

- The game client's archive initialisation sets the maximum to **0xFFFFFFFF** for every archive
  it opens.
- Worked consequence (synthetic numbers): with the maximum 0xFFFFFFFF, an archive of length
  0xFF800000 with 1,000,000 free bytes gives 0xFFFFFFFF − 0xFF800000 + 1,000,000 = 9,388,607;
  minus 8 MiB (8,388,608) that leaves 999,999, so the archive is full for any *need* above
  999,999 bytes. The same archive with no free bytes sums to 8,388,607, below 8 MiB: the
  subtraction wraps and the test does not report it as full. An archive of length 0xFF900000
  wraps the same way whenever its free bytes are at most 0x100000.
- The observed archives (largest 0xFF900000) were written by the launcher's patcher, not by the
  game client. The patcher's own maximum was not read: the patcher is excluded from M4
  (decision 0011). So the observed lengths are not evidence for the game client's value.

**New archives** (**BIN**, Q21, P56). When the archive library creates an archive, the file
starts **1 MiB** (0x100000) long, its minimum allocation size (0x144) is the default of its kind
([§3.4](#s3-4)), and its first free extent starts at 0x400 ([§4](#s4)). The two metadata-only
placeholders `cell_15` and `map_15` are exactly 1 MiB long (**C**, 2026-10-05), as such a
newly created archive would be.

**Versions stamped at creation** (**BIN**, Q23, P58). The creating opener's expected datpack
versions ([§3.7](#s3-7)) decide the new archive's versions:

- 0x174 = the expected container version when it is 1 or more, otherwise 111. The new archive
  then keeps its free space as a version-111 chain or a version-112 table to match
  ([§4.5](#s4-5)).
- 0x178 = the expected game data version as given, with no substitution.
- 0x190/0x194/0x198 = 104/32/8 ([§3.3](#s3-3)).

When a set creates its next aux part (the last bullet of the list at the start of this section), it does not use the
values the set was opened with. It takes the expected versions from the set's overlay when one is
in use, otherwise from its first part, and those are that archive's own 0x174 and 0x178. So a new
aux part carries the same 0x174 and 0x178 as its base: 112 and 3 for the sets of the observed
install. Only a set with no part at all would fall back to −1 and −1, and so to 0x174 = 111.

A stand-alone archive created with the game client's own expected values (−1 and −1,
[§3.7](#s3-7)) would get 0x174 = 111 and 0x178 = 0xFFFFFFFF.

> **OPEN:** Whether the game client ever creates an archive other than an aux part was not
> traced. Every archive in the install was written by the patcher, and all hold 0x174 = 112.
> **Reader rule:** treat 0x178 = 0xFFFFFFFF like any other value outside 3 and 0 (Warn,
> [§3.7](#s3-7)), and accept 0x174 = 111 with the chained layout.

---

<a id="s5"></a>
## 5. Directory B-tree

<a id="s5-1"></a>
### 5.1 Node layout

A node is an allocation (normally 2460 bytes, see §4.4) read with the chained layout
([§4.3](#s4-3)): in practice one block whose 8-byte header is `{0, 0}`, followed by a
**2452-byte payload**. Offsets below are relative to the payload start (allocation
offset + 8).

| Offset | Size | Type | Meaning | Mark |
|---|---|---|---|---|
| 0 | 496 | 62 × {u32 size, u32 offset} | Child pointers. `size` is the child node's allocation size; `offset` is its absolute offset. | C |
| 496 | 4 | u32 | `n`: number of valid entries, 0 ≤ n ≤ 61 | C, BIN |
| 500 | 1952 | 61 × 32-byte entry | Entries; only the first `n` are valid ([§5.2](#s5-2)) | C |

Total payload 496 + 4 + 1952 = 2452. **C**

Rules:
- A node is a **leaf** if child pointer 0's **offset** is 0. That is the only test the client
  makes. In every observed leaf, child pointers 0…n are all `{0, 0}`. **BIN**, **C**
- In an internal node, pointers 0…n are all non-zero (both size and offset). **C**
- When it loads a node, the client refuses it if `n` ≥ 62, or if any child pointer 0…n has
  bit 0 of its offset set. It does not check the other pointers. **BIN**; no observed node
  violates either rule. **C**
- Pointers n+1…61 and entries n…60 are **stale**: they may hold old non-zero data
  (2,778 nodes in the install have non-zero stale pointers). Readers must ignore them. **C**
- When the writer creates a node, it fills every entry slot with zeros, including the DID
  field, which receives the "no DID" value 0 ([§5.5](#s5-5)). **BIN** Of the 3,009,330 entry
  slots n…60 in the install, 186,823 are still entirely zero; the rest hold stale entries.
  **C** (2026-10-05)

<a id="s5-2"></a>
### 5.2 Directory entry (32 bytes)

Field names below are this spec's own.

| Offset | Size | Type | Name | Meaning | Mark |
|---|---|---|---|---|---|
| +0 | 2 | u16 | `flags` | Storage flags ([§5.3](#s5-3)) | C, BIN |
| +2 | 2 | u16 | `stamp` | The record's data version (below) | C (values), BIN |
| +4 | 4 | u32 | `did` | Data identifier; the B-tree key | C |
| +8 | 4 | u32 | `offset` | Absolute offset of the record's first extent or first block | C |
| +12 | 4 | u32 | `length` | Stored payload length in bytes. For compressed records this includes the 4-byte inflated-length prefix. | C |
| +16 | 4 | u32 | `time` | Unix time (seconds, UTC) at which the record was written into this archive | C, BIN |
| +20 | 4 | u32 | `iteration` | The patch iteration that last wrote the record (below) | C, BIN |
| +24 | 4 | u32 | `allocSize` | Size of the first extent or first block ([§4](#s4)) | C |
| +28 | 4 | u32 | reserved | Always 0 | C (0 in all 3,838,957 entries) |

**`stamp` (data version).** The archive library stores the 16-bit value the writer supplies and
returns it with the payload when the record is read. The code that decodes a record may check
it. Example: the client's loader for the archive's version-stamp record 0xFFFF0003 requires the
value 1, and its decoder for the iteration list 0xFFFF0001 accepts values up to 2. The
container does not interpret it. **BIN**

Observed values (**C**, both dates): 1, 2, 7, 8, 11, 12, 14, 20, 21, 22, 27, 28, 29, 31, 32,
33, 38, 39. Metadata records use 1 or 2. Most record types carry many of the values from 7 to
39, and the earliest `time` per value rises with the value (in `general`: 7 from 2007-01,
20 from 2008-10, 38 from 2012-10, 39 from 2018-10). The 10,677 game records rewritten on
2026-10-04 kept a range of values (7 … 39), so the value travels with the record content; it is
not set by the local writer.

**`time`.** When the writer supplies 0, the archive library stores the current Unix time.
**BIN** The 10,677 game records written by the 2026-10-04 update all carry times within 33
seconds of each other (2026-10-04 20:08:20–20:08:53 UTC), the moment the update ran on the
user's machine. Older records carry the times of the original build. A `time` change alone
therefore says that a record was rewritten, not that its content changed. **C**

**`iteration`.** Each content update applies a sequence of numbered **iterations**. A record's
`iteration` is the number of the iteration that last wrote it; 0 means it was never written by
an update.
- The base archive of each set lists the iterations it has applied in record 0xFFFF0001
  ([§7](#s7)). Every non-zero entry `iteration` in every part of the set is a member of that
  list, and every one is lower than the list's highest value. **C** (all 3,838,840 game
  records, 2026-10-05)
- When a record is written with a non-zero iteration that is **lower** than the iteration
  already stored for that DID, the archive library skips the write (unless the writer forces
  it), for every game record and for 0xFFFF0006, 0xFFFF0007 and 0xFFFF0009. A write with the
  same or a higher iteration replaces the record. So `iteration` never decreases for a DID.
  **BIN**
- The records rewritten on 2026-10-04 carry many different iterations (for example 77,225 …
  77,249 in `gamelogic`), one per patch step that last touched them. **C**

Guidance for comparing two installs (`tessera-dattool diff`): a higher `iteration` or a
different `length` means the content changed. Equal `iteration` and `length` do not prove equal
content, because an iteration can be re-applied; only a content hash proves it. A change of
`time` or of the physical placement alone is a rewrite, not a content change.

<a id="s5-3"></a>
### 5.3 Flags

| Bit | Meaning | Mark |
|---|---|---|
| 0 | 1 = payload is zlib-compressed ([§6.1](#s6-1)) | C, BIN |
| 1 | 1 = record uses the extent layout ([§4.2](#s4-2)); 0 = chained layout ([§4.3](#s4-3)) | BIN, C |
| 2–15 | Not used by the client's reader; always 0 in observed data | BIN, C |

Observed values (**C**, 2026-10-05): **2** (stored) and **3** (compressed) for every record
except 8: the records 0xFFFF0003 and 0xFFFF0008 of `cell_5`, `cell_15`, `map_5` and `map_15`
have flags **0** (stored, chained layout, single block). On 2026-10-03 those archives were
present with flags 2 for these records, so both forms occur in real installs. Sources report 0
and 1 as possible values and one tool rejects values > 3. **R** Value 1 (compressed, chained) is
valid for the client but not observed.

A reader must choose the layout from bit 1 and must not infer it from the allocation header.

Compression by archive (**C**, 2026-10-05):

| Archive | Compressed (flags 3) | Stored (flags 2 / 0) |
|---|---|---|
| general, gamelogic, local_English, sound, sound_aux_1, map_1…4, map_14 | 0 | all / 0 |
| map_5, map_15, cell_15 | 0 | all but 2 / 2 |
| anim | 22,245 | 3,155 / 0 |
| cell_1 … cell_4, cell_14 | 97.7–99.8 % | rest / 0 |
| cell_5 | 85,139 | 1,240 / 2 |
| mesh, mesh_aux_1 | 37,402 / 10,052 | 87 / 29 |
| surface, surface_aux_1, surface_aux_2 | 327,483 / 464,271 / 280,331 | 113,558 / 77,615 / 66,601 |
| highres, aux_1, aux_2, aux_3 | 13,233 / 13,101 / 40,053 / 7,008 | 2,214 / 1,478 / 752 / 155 |

The same DID type can be stored either way within one archive. A reader must honour the
per-entry flag and must not assume a per-type or per-archive rule. **C** The writer's choice is
explained in [§6.1](#s6-1).

<a id="s5-4"></a>
### 5.4 Tree shape

- The tree is a B-tree of minimum degree 31: at most 61 entries per node and, outside the
  root, at least 30. The writer rebalances when a child would drop below the minimum. **BIN**
- Keys (DIDs) are unique within an archive. An in-order traversal yields strictly
  ascending DIDs (compared as unsigned 32-bit values). **C** (29/29), **BIN**
- Every non-root node holds between **30 and 61** entries. **C**
- The root holds 1 … 58 entries here (1 in most cell archives, 4 in the metadata-only
  `cell_15`/`map_15`). A newly created archive has a root leaf with **no entries** (n = 0)
  until its first record is written. **C**, **BIN**
- All leaves are at the same depth. The maximum depth is 3 (root at depth 0), so there are
  at most 4 levels. **C**
- Total observed on 2026-10-05: 112,267 nodes, 3,838,957 entries (2026-10-03: 113,118 nodes,
  the same entry count).

<a id="s5-5"></a>
### 5.5 Lookup by DID

1. Start at the root node (header 0x160 / 0x1A4).
2. Binary-search the node's `n` valid entries for the DID, comparing as unsigned 32-bit
   values.
3. If found, return that entry. Internal nodes hold real entries too, so a match can occur
   above leaf level.
4. If not found, let `i` be the number of entries whose DID is less than the target. If
   the node is a leaf (child pointer 0's offset is 0), the DID is absent from this archive.
   Otherwise descend into child pointer `i` and repeat from step 2.
5. Bound the descent depth (e.g. 16 levels) and fail with a corruption error if it is
   exceeded. (The client has no bound; ours is a safety measure.)

The client's "no DID" value is **0**. Its lookup reports DID 0 as absent without searching the
tree, and no entry in the install has DID 0. A reader must do the same: a lookup of DID 0
returns "absent", and an entry with DID 0 is reported (I-33). **BIN** (value and rule), **C**
(no entry has DID 0 in any of the 29 archives, 2026-10-05) (P47)

<a id="s5-6"></a>
### 5.6 In-order traversal

For a node with `n` entries, visit in this order: child 0, entry 0, child 1, entry 1, …,
entry n−1, child n. For a leaf, just visit entries 0…n−1. This yields every record exactly
once, in ascending DID order. A hostile-input-safe implementation:
- tracks the set of node offsets already visited and rejects a revisit (cycle);
- bounds the depth;
- checks every child pointer `{size, offset}` against the file length and against
  `size ≥ 2460` before reading.

---

<a id="s6"></a>
## 6. Record payload

<a id="s6-1"></a>
### 6.1 Compression

If `flags` bit 0 is set, the reassembled stored payload (`length` bytes) is:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | u32 | Inflated length in bytes |
| 4 | length − 4 | bytes | One zlib stream (RFC 1950: 2-byte header, deflate data, Adler-32 trailer) |

How the client inflates (**BIN**; it links zlib 1.2.13):
- A compressed record whose stored `length` is below 16 is refused.
- It allocates an output buffer of exactly the prefixed length and inflates the stream into
  it with zlib's one-shot decompression.
- It refuses the record if the stream needs more output than the prefix allows, if the
  stored bytes end before the stream does, or if the stream is invalid (including a bad
  Adler-32).
- It accepts bytes after the end of the stream (they are ignored), and it does **not** check
  that the stream filled the whole buffer.

Observed (**C**, 2026-10-05, every one of the 1,740,962 compressed records in all 29
archives fully inflated):
- Every stream produced exactly the prefixed length, ended with a valid Adler-32 trailer, and
  left **no trailing bytes**.
- Zlib header bytes are `78 9C` (1,055,024) or `78 DA` (685,938). Both occur within one
  archive.
- The smallest compressed record is 19 bytes stored. The largest inflated length is
  17,984,776 bytes (`mesh`). The highest ratio is about 979:1 (`surface`).

Reader requirements:
- Reject: `length` < 16 (Error for the record); an inflated length above a configurable cap
  (default comfortably above 32 MiB); a stream that would produce more bytes than the prefix; a
  stream that is invalid or ends after the stored bytes.
- Allow ratios of at least 1000:1.
- A stream that ends before filling the prefixed length is accepted by the client, but the
  missing bytes are then undefined. It never occurs in data; a reader must report it as
  corrupt.
- Bytes after the end of the stream are accepted by the client. They never occur in data; a
  reader should accept them and warn.

Writer policy, which explains why one type is stored both ways (**BIN**, **C**, P46). Let *U*
be the original (uncompressed) payload length and *Z* the length of the zlib stream the
compressor produced. *Z* excludes the 4-byte inflated-length prefix, so for a stored record
*Z* = entry `length` − 4 and *U* = the prefix value.

1. A payload is compressed only if *U* > 16.
2. The compressed form is kept if 10 × *Z* ≤ 8 × *U* (the stream is at most 80 % of the
   original), or if *U* − *Z* ≥ 512,000 bytes. Otherwise the record is stored uncompressed.
3. The client's own compressor uses zlib's default level (header `78 9C`).

Every compressed record in the install satisfies the rule measured this way (17 of them only
through the 512,000-byte clause). Measured with the entry `length` (prefix included) instead,
5,758 of them would fail it, so the 4-byte prefix must not be counted. Every compressed
record's inflated length is at least its stored length. **C** (all 1,740,962, 2026-10-05)

Worked example (synthetic): *U* = 1,000 and *Z* = 800 gives 8,000 ≤ 8,000, so the record is
compressed, with entry `length` 804. Testing the entry `length` instead (8,040 > 8,000) would
wrongly predict a stored record.

<a id="s6-2"></a>
### 6.2 DID echo and type prefix

After decompression (or directly, for stored records), most record types repeat their own
DID. The location depends on the type, so the container layer must **not** require an echo.
Per high byte of the DID, observed in this install (**C**, all records checked):

| Echo position | DID high bytes |
|---|---|
| u32 at +0 equals the DID | 01, 04, 06, 0A, 0F, 12, 14, 16–19, 1D, 20, 21, 22, 23, 25, 27, 28, 2A, 2B, 2D, 30–35, 40, 41, 42, 51, 56, 70, 76, 78, 79, 80; most of 0E |
| u32 at +4 equals the DID (another u32 precedes it) | 02, 11, 15, 1A, 1F, 47 |
| no echo at +0 or +4 | 05, 07, 08, 0C, 39, 43, 44, 58, 66, FF; some of 0E (0x0E01xxxx) |

What the leading u32 of the "+4" types means, and what the other types start with, is
specified per type in [`records.md`](records.md) §1.6 and the record specs it names (for
example 0x02 and 0x11: a u32 0; 0x15: a u32 version 1; 0x1F and 0x47: a u32 flag word).

---

<a id="s7"></a>
## 7. Reserved metadata records (DID 0xFFFF00xx)

<a id="s7-1"></a>
### 7.1 Presence and roles

Every archive contains a few DIDs whose high 16 bits are 0xFFFF. They sort last in the
tree. All are stored uncompressed with `iteration` = 0. **C** The client's archive library
reserves 0xFFFF0000–0xFFFF0009 for its own use. **BIN**

| DID | Present in | Length | `stamp` | Role | Mark |
|---|---|---|---|---|---|
| 0xFFFF0001 | all 29 | 148–156 in base archives; 8 in the older aux parts; 284 (`highres_aux_3`) and 3,192 (`mesh_aux_1`) in the two newest aux parts (288 and 3,196 on 2026-10-03) | 2 | **Iteration list**: the iterations applied to the archive ([§7.3](#s7-3)). Loaded when the archive is opened by the game client; an archive without it fails to open there. | C (shape), BIN (role) |
| 0xFFFF0003 | all but the 2 newest aux parts | 37 | 1 (required) | **Version stamp record**: two 16-byte identifiers and a short ASCII version name ([§7.4](#s7-4)) | C (shape), BIN (role) |
| 0xFFFF0004 | `general`, `gamelogic`, `anim`, `mesh`, `sound`, `surface`, `highres`, `local_English`, `cell_1`, `map_1` | 12 | 2 | u32 N, i32 −N, u32 1. Not referenced by the client's archive library. | C (shape) |
| 0xFFFF0005 | all 29 | 24 | 1 | **Set membership** ([§7.2](#s7-2)) | C, BIN |
| 0xFFFF0008 | all 22 base archives, no aux part | 16 | 1 | A 16-byte GUID identifying the product or data set, identical in every archive ([§7.5](#s7-5)) | C (shape), BIN (layout) |
| 0xFFFF0000, 0002, 0006, 0007, 0009 | none | — | — | Reserved by the library; 0006, 0007 and 0009 have loaders in the client | BIN |

The 8 metadata records with `flags` 0 are listed in [§5.3](#s5-3).

These records are archive metadata, not game content. A traversal that lists game records
should be able to filter `did ≥ 0xFFFF0000`. They must be read **per archive**: the client
refuses to look up 0xFFFF0005 through a set, because every part has its own. **BIN**

<a id="s7-2"></a>
### 7.2 Set membership record (0xFFFF0005)

| Offset | Size | Type | Meaning | Mark |
|---|---|---|---|---|
| 0 | 16 | bytes | Set identifier, identical in all parts of one set and different between sets. All zero is allowed on the base (part 0) of a set of any size; the aux parts then carry one shared non-zero identifier, which the set adopts. All zero on an aux part is never allowed. Rules in [§2.3](#s2-3) step 3 | C, BIN |
| 16 | 4 | u32 | Part index: 0 for the `.dat` base, N for `_aux_N.datx` | C, BIN |
| 20 | 4 | u32 | Number of parts in the set | C, BIN |

Observed counts: mesh 2, sound 2, surface 3, highres 4, every other archive 1. Each cell and
map archive has its own identifier (count 1). **C** How the client checks the record is in
[§2.3](#s2-3).

How the client loads the record of each part (**BIN**, Q17, P52):
- It reads 0xFFFF0005 from that part alone with its ordinary record read, so a compressed copy
  is inflated by the rules of [§6.1](#s6-1). Compression is allowed, though never observed.
- The record's `stamp` must be 1. Otherwise the record is ignored.
- It reads the three fields in order: the 16-byte identifier, then the index, then the count.
  Bytes after the first 24 are ignored, so a longer record is accepted. If the record ends
  early, every field from the first incomplete one onwards keeps its default.
- Defaults, in force before the load and whenever the record is absent, ignored or short: an
  all-zero identifier, part index 0, part count 1.
- Opening the part does not check whether the load worked. An unusable record therefore acts
  as "all zero, index 0, count 1" in [§2.3](#s2-3). A one-part set is accepted that way. On an
  aux part the all-zero identifier fails step 3. On the base of a multi-part set the count 1
  fails step 5.

Reader rules: read the record like any other record. Use the first 24 bytes, and treat a
record that is absent, has `stamp` ≠ 1 or has fewer than 24 bytes as absent (I-24 warning,
then the defaults above). A reader may cap the length it inflates for this record. A record
above that cap is valid for the client, so report it as a warning (I-24), not as a corrupt
archive.

<a id="s7-3"></a>
### 7.3 Iteration list (0xFFFF0001)

The layout and the run-list decoder are specified in [`install-layout.md`](install-layout.md)
§9.2.1 and §9.3. Additional facts (**BIN**, **C**):
- The trailing list holds `K` entries of a u32 iteration and a **u64** value.
- The trailing part (`K` and its entries) is present only when the record's `stamp` is 2. With
  `stamp` 1 the record ends after the run list. The client refuses a `stamp` of 3 or more (the
  load fails, and the game client's open then fails, [§3.7](#s3-7)). Every 0xFFFF0001 in the
  install has `stamp` 2. **BIN** (Q22, P57), **C**
- The client copies each entry's u64 verbatim (8 bytes, no conversion) and no code of the game
  client that was found interprets it. **BIN** (Q22)
- Only the base archive's list describes the set: entry iterations in aux parts are members of
  the base's list ([§5.2](#s5-2)). Aux parts have empty lists (N = 0) or lists that do not
  cover their own entries (`highres_aux_3`, `mesh_aux_1`).
- The highest listed iteration is never used by an entry. On 2026-10-05, counting all parts
  of a set, the highest entry iteration was exactly one below it in 13 of the 20 sets that
  hold game records, and lower in the other 7 (for example `gamelogic` 77,249 against 77,252,
  `map_14` 78 against 201, `surface` 106,600 against 106,616). Counting only the base archive
  the relation holds in 10 of 20 (`highres` alone: 56,024 against 63,224).

<a id="s7-4"></a>
### 7.4 Version stamp record (0xFFFF0003)

The layout is in [`install-layout.md`](install-layout.md) §9.2.2: two 16-byte identifiers, a
1-byte length and that many ASCII characters. The client loads it into the archive's version
stamp when it opens the archive, requires `stamp` = 1, and only warns when the record is
missing. For a set it uses the record of the first part, in index order, that has one.
**BIN**

<a id="s7-5"></a>
### 7.5 Product identifier (0xFFFF0008)

A GUID in the usual little-endian GUID serialisation: u32, u16, u16, then 8 bytes. The value is
identical in all 22 base archives. **C**, **BIN** (layout)

The archive library has a "get product identifier" and a "set product identifier" operation for
this record; the setter writes it with `stamp` 1. Opening an archive does not read or compare it,
and no caller of the getter that compares it with anything was found. A reader treats it as an
opaque product identifier. **BIN** (Q22, P57)

**What is still unknown** (Q22, read 2026-10-07):

- 0xFFFF0004: the 32-bit value 0xFFFF0004 appears only in the archive library's list of reserved
  DIDs; no code of the game client loads or writes a record by that DID. The archives are
  written by the launcher's patcher, which decision 0011 excludes from M4, so whatever uses the
  record was not examined.
- The u64 of the iteration list's trailing entries: copied verbatim by the client ([§7.3](#s7-3)).
  Observed values look uniformly random (all 64 bits used, no shared prefix between consecutive
  iterations; 2026-10-07, the 66 trailing entries of six archives).

> **OPEN:** The meaning of 0xFFFF0004 (u32 N, i32 −N, u32 1) and of the iteration list's u64
> values. Both are written by the patcher, so M4 on the game client cannot answer them; the
> patcher is excluded by decision 0011. Readers must not depend on either. Needs a maintainer
> decision to extend M4 to the patcher, or a published source.

---

<a id="s8"></a>
## 8. DID → archive routing

<a id="s8-1"></a>
### 8.1 Observed placement (2026-10-05)

High byte of DID → archive(s) holding records with that high byte, with record counts.
(0xFFFF metadata is excluded.) The placement is identical to 2026-10-03 except that records
moved between the parts of the `surface`, `highres` and `sound` sets. **C**

| High byte(s) | Archive(s) (count) |
|---|---|
| 01, 02, 04, 08, 0F, 11, 12, 14–19, 1A, 1F, 20, 21, 23, 28, 2A, 2B, 2D, 31, 32, 33, 35, 39, 44, 51, 66 | general only |
| 0E | general (7: 0x0E000004, 0x0E000006, 0x0E01000x, 0x0E200000, 0x0E400000), local (1) |
| 1D | general (3), local (1) |
| 30 | general (24,091), local (3) |
| 40 | general (81,135), local (1) |
| 42 | general (3), local (45) |
| 07, 0C, 34, 43, 47, 56, 70, 76, 79 | gamelogic only |
| 78 | gamelogic (1,616), local (2) |
| 05 | anim |
| 06 | mesh (37,484), mesh_aux_1 (10,079) |
| 0A | sound (12,621), sound_aux_1 (2,523), local (29,193) |
| 22 | local only (302) |
| 25 | local only (282,152) |
| 58 | local only (1) |
| 41 | surface, surface_aux_1, surface_aux_2, highres, highres_aux_1…3, local (223) |
| 27 0R xxxx | cell_R (13–100 per region) |
| 80 0R xxxx, 80 2R xxxx, 80 4R xxxx, 80 5R xxxx | cell_R |
| 80 1R xxxx | map_R |

Notes (**C**):
- For cell/map DIDs the second byte is `(class << 4) | R`, where R is the region (1–5, 0xE,
  0xF). Each region's archive holds 65,025 (= 255 × 255) DIDs of class 0, and its map archive
  holds 65,025 DIDs of class 1. The low 16 bits stay within 0x00–0xFE in each byte for
  those classes. The client takes the region from bits 16–19 of the DID. **BIN**
- Every gamelogic DID 0x70xxxxxx has a companion 0x79xxxxxx = DID + 0x09000000 (383,332
  pairs, exact one-to-one match). Sources report this offset as a fixed property-record
  relationship. **R** (meaning), **C** (match).
- Within a multi-part set, DID ranges of the parts **interleave**: each part spans almost
  the whole DID range of its type. A DID cannot be routed to a part by range. Each part
  must be searched.

<a id="s8-2"></a>
### 8.2 How the client selects the archive

The client registers every DID range with a record type and a **home** (**BIN**). The home is
a dataset type (portal, region or language) plus an archive kind ([§3.4](#s3-4)). To find a
DID it:

1. Determines the record type from the DID's range.
2. If the home is a portal archive: first searches the **high-resolution** set (type 1,
   kind 8), if that set is open. If it holds the DID, it is used.
3. Searches the home: for a portal type, the portal set of the home kind; for a regional type,
   the region set of the home kind for the region in DID bits 16–19; for a language type, the
   language set of the selected language.
4. If the DID is still not found, searches the **language** set (type 3, kind 3) as a
   fallback.
5. Otherwise the DID is absent.

"Searches a set" means: try every registered variant of that (type, kind, subset) in
ascending variant order, and within each set use the order of [§8.3](#s8-3). The first set
that holds the DID wins; `iteration` and `time` are not compared. **BIN**

Home of each DID range that holds records in the install (kind from the client's
registration, **BIN**; placement agrees, **C**). Where the registration covers several high
bytes as one range, the table names the range and says which of its high bytes occur:

| High byte(s) | Home | Archives searched, in order |
|---|---|---|
| 05 | portal, kind 4 | highres → anim → local |
| 06 | portal, kind 2 | highres → mesh (+aux) → local |
| 07, 0C, 34, 43, 47, 56; the ranges 70–77 (only 70 and 76 occur) and 78–7F (only 78 and 79 occur) | portal, kind 1 | highres → gamelogic → local |
| 0A | portal, kind 7 | highres → sound (+aux) → local |
| 41 | portal, kind 9 | highres (+aux) → surface (+aux) → local |
| 01, 02, 04, 08, 0E, 0F, 11, 12, 14–1A, 1D, 1F, 20, 21, 23, 28, 2A, 2B, 2D, 30–33, 35, 39, 40, 42, 44, 51, 66 | portal, kind 3 | highres → general → local |
| 22, 25 (registered as the range 25–26; only 25 occurs), 58 | language, kind 3 | local |
| 27, 80 0R, 80 2R, 80 4R, 80 5R | region, kind 3 | cell_R → local |
| 80 1R | region, kind 0x0A | map_R → local |

The registration also has ranges with no record in this install, for example high bytes
0x0D, 0x24, 0x46, 0x53 and 0x90–0x9F, and the region class 0x806R (kind 0x0D, never opened).
They are not tabulated here. A reader that meets a DID outside the table uses the fallback of
[§8.3](#s8-3). **BIN**

So the language archive is a **fallback**, not an override: records such as the localized
0x0A sounds and 0x41 textures are found there only because their home archives do not hold
them. The search order matters only if a DID is duplicated, which never happens in the
install.

LC's routing table (Reported, 2026-10-03 version of this spec) tries `highres` before
`surface` for 0x41 and `general` before `local` for 0x22; it agrees with the client except
that the client never searches `general` for 0x22, and it lacks several prefixes (0x02, 0x05,
0x0C, 0x1A, 0x39, 0x42, 0x66).

<a id="s8-3"></a>
### 8.3 Routing rule for implementers

- No DID occurs in more than one archive of the observed install, on either date. Every pair
  of archives was intersected, with 0xFFFF metadata excluded. **C**
- Implement the order of [§8.2](#s8-2): high-resolution set (portal types only), home set,
  language set. Keep the home table as data (not code), seeded from §8.2, so that it can be
  updated without a code change. A reader may fall back to searching every open archive when
  the table has no entry, and should log that.
- Within one set, search the overlay first if one is open ([§2.5](#s2-5)), then the parts in
  index order (base, `_aux_1`, `_aux_2`, …). The first part holding the DID is
  authoritative; a copy in a later part is stale (the writer can leave one behind when an
  earlier part gains free space). **BIN**
- If DIDs are duplicated across archives, use the first copy in this order and report the
  duplicate (I-25). Do not compare `iteration` or `time`.
- Metadata DIDs (0xFFFF00xx) have no route: read them per archive.

---

<a id="s9"></a>
## 9. Limits

| Limit | Value | Mark |
|---|---|---|
| Offsets and sizes | u32, so an archive file is < 4 GiB | C (field widths), R (DO note about ">4 GB" requiring a format change) |
| Largest observed archive | 0xFF900000 bytes | C |
| Aux part creation | when no existing part accepts a write ([§4.7](#s4-7)) | BIN, C (sizes) |
| Entries per node | 61 max; 30 min except root; the client refuses n ≥ 62 | C, BIN |
| Children per node | n + 1 ≤ 62 | C |
| Tree depth | ≤ 3 observed (4 levels); the client has no limit | C, BIN |
| Largest stored record | 40,916,068 bytes (`sound`); largest in `gamelogic` 29,944,338 | C |
| Largest inflated record | 17,984,776 bytes | C |
| Smallest compressed record | 19 bytes stored; the client refuses < 16 | C, BIN |
| Fragments per record | writer limit `0x144 / 8 − 1`; ≤ 31 observed | BIN, C |
| File growth unit | 1 MiB ([§4.7](#s4-7)); file lengths are multiples of 1 MiB except `local_English` | BIN, C |

A reader must open archives read-only and must not lock them exclusively. The official client
opens them in the read-write mode of [§3.7](#s3-7), with no sharing, while it runs (**BIN**,
P50), and the launcher rewrites them while the client is not running (**R**). A reader must
never write to the install (project rule).

---

<a id="s10"></a>
## 10. Validator invariants

Every item held on all 29 archives on 2026-10-05 unless marked. "Error" means the archive is
unusable. "Warn" means it is readable but unexpected. "Info" means it is reported only on
request.

| ID | Check | Severity |
|---|---|---|
| I-1 | File length ≥ 0x400 and ≥ 0x1A8 | Error |
| I-2 | u32 at 0x140 = 0x00005442 | Error |
| I-3 | u32 at 0x148 = file length | Warn (possible concurrent patching) |
| I-4 | 0x144 > 0 and a multiple of 4 | Error |
| I-5 | 0x174 = 112; 111 is readable with the chained layout ([§3.7](#s3-7)) | Warn for 111; Error for other values |
| I-6 | 0x190/0x194/0x198 = 104/32/8 | Warn (never Error: the client ignores them) |
| I-7 | 0x100…0x104 = `00 50 4C 00 00`, 0x105…0x13F zero | Info |
| I-8 | Root: 0x160 ≥ 0x400, 0x1A4 ≥ 2460, 0x160 + 0x1A4 ≤ length | Error |
| I-9 | Every node allocation header = `{0, 0}` (single block) | Error (a multi-block node is legal for the client but never observed; see [§14](#s14)) |
| I-10 | Every node has n ≤ 61; non-root nodes have n ≥ 30; the root has n ≥ 1 | Error (n ≥ 62); Warn (n < 30 in a non-root node; n = 0 in the root) |
| I-11 | Leaf iff child 0's offset = 0; internal nodes have non-zero children 0…n; leaf children 1…n zero | Error (internal child zero); Warn (leaf child non-zero) |
| I-12 | Child pointers satisfy size ≥ 2460 and offset + size ≤ length | Error |
| I-13 | No node offset is visited twice; depth ≤ 16 | Error |
| I-14 | All leaves at the same depth | Warn |
| I-15 | In-order DIDs strictly ascending (unsigned; implies unique) | Error |
| I-16 | Entry reserved (+28) = 0 | Warn |
| I-17 | Entry flags ∈ {0, 1, 2, 3}, that is, bits 2–15 clear | Warn when any of bits 2–15 is set. Values 0, 2 and 3 pass silently. Value 1 (compressed, chained layout) is legal for the client but never observed: Info only, no warning |
| I-18 | Record first extent or block: offset ≥ 0x400, offset + allocSize ≤ length; for flag bit 1 set: 8 + 8F ≤ allocSize, and if F = 0 then allocSize ≥ length + 8 | Error |
| I-19 | Fragments and chained blocks in bounds; the payload can be completed | Error |
| I-20 | Tiling: nodes, record extents, fragments, the free table and free extents cover [0x400, length) exactly, with no overlap | Warn (Error on overlap); skipped when 0x154 = 0xFFFFFFFF |
| I-21 | Free table header `{0,0}`; 16 + 8·count ≤ 0x1A0; the first pair with size 0 is the pair at index count and is `{0,0}` ([§4.5](#s4-5)); pairs in bounds, sorted by offset; each size ≥ 0x144 | Warn; skipped when 0x154 = 0xFFFFFFFF; not applicable (Info) when 0x154 = 0 and 0x1A0 = 0 (no table) |
| I-22 | 0x19C = Σ (free size − 8); with no table (0x154 = 0 and 0x1A0 = 0), 0x15C = 0 and 0x19C = 0 | Warn; skipped when 0x154 = 0xFFFFFFFF |
| I-23 | Compressed record: length ≥ 16, inflated length = prefix, stream ends within `length` with a valid trailer, no trailing bytes | Error for that record (trailing bytes: Warn) |
| I-24 | 0xFFFF0005 exists; its part index matches the file name (0 for `.dat`, N for `_aux_N`) and index < count; all parts share the identifier, except that the base may carry an all-zero identifier while the aux parts share one non-zero identifier ([§2.3](#s2-3) step 3; such a set is valid and gives no warning); an aux part with an all-zero identifier is a warning. A record that is absent, has `stamp` ≠ 1 or is shorter than 24 bytes counts as absent; a compressed or longer record is valid ([§7.2](#s7-2)) | Warn |
| I-25 | Across the archives the client opens together (every portal and region set plus the **one** selected language set, [§2.4](#s2-4)), no DID (< 0xFFFF0000) appears in two archives. Archives of other languages are validated on their own and are not part of this check | Warn |
| I-26 | 0x16C = 0 (no pending journal) | Error by default ([§3.6](#s3-6)) |
| I-27 | 0x154 ≠ 0xFFFFFFFF (free table valid) | Warn |
| I-28 | Child pointers 0…n have bit 0 of the offset clear | Error |
| I-29 | F ≤ 0x144 / 8 − 1 | Warn |
| I-30 | 0xFFFF0001 exists and decodes; every non-zero entry `iteration` of the set is in the base archive's list | Warn |
| I-31 | 0xFFFF0003, where present, has `stamp` = 1 | Warn |
| I-32 | For records with flag bit 1 set, allocation header word +4 = 0 (the client does not read it) | Warn |
| I-33 | No entry has DID 0, the "no DID" value ([§5.5](#s5-5)); such an entry is unreachable by lookup | Warn |

All of I-1…I-33 held on 2026-10-05. The root n = 0 case of I-10 does not occur, and the 8
records with flags 0 satisfy I-17. I-23 was checked in full for every compressed record.

---

<a id="s11"></a>
## 11. Observed statistics (2026-10-05)

<a id="s11-1"></a>
### 11.1 Full walks of `client_general.dat` and `client_local_English.dat`

Every payload was reassembled in full. Neither archive contains compressed records.

| Metric | general | local_English |
|---|---|---|
| Min allocation (0x144) | 1024 | 256 |
| Entries | 261,483 | 311,929 |
| Nodes (per depth 0/1/2/3) | 7,983 (1/5/210/7,767) | 8,654 (1/6/227/8,420) |
| Root entry count | 4 | 5 |
| Non-root entries per node | 30 … 61 | 30 … 61 |
| Nodes with allocation ≠ 2460 | 25 | 4 |
| Strictly sorted / unique | yes / yes | yes / yes |
| Flags distribution | 2: 261,483 | 2: 311,929 |
| `fragCount` distribution | 0: 261,480; 1: 3 | 0: 311,725; 1: 43; 2: 19; 3: 14; 4: 13; 5: 9; 6: 14; 7: 5; 8: 3; 9–30: 73; 31: 11 |
| Chained multi-block allocations | 0 | 0 |
| Payload begins with own DID (+0) | 199,731 | 311,923 |
| DID at +4 instead | 33,505 (types 02, 11, 15, 1A, 1F) | 0 |
| No echo | 28,247 (types 08, 39, 44, 66, part of 0E, FF) | 6 (0x58000000 and 5 × FF) |
| Free extents (0x15C) | 1 (0x8A4BC bytes, ends at end of file) | 1 (0x6B5F0 bytes, ends at end of file) |
| Tiling gaps / overlaps | 0 / 0 | 0 / 0 |
| `time` range (game records) | 2007-01-10 … 2026-10-04 | 2007-01-10 … 2026-10-04 |
| Largest stored record | 2,981,586 | 6,114,043 |

DID high-byte histogram, `general`: 01:1057, 02:3402, 04:28555, 08:11, 0E:7, 0F:7, 11:5,
12:5, 14:5, 15:1, 16:5, 17:5, 18:5, 19:5, 1A:963, 1D:3, 1F:29134, 20:4201, 21:20, 23:735,
28:24, 2A:45544, 2B:510, 2D:3, 30:24091, 31:13781, 32:1, 33:24, 35:1, 39:11155, 40:81135,
42:3, 44:254, 51:2, 66:16819, FF:5.

DID high-byte histogram, `local_English`: 0A:29193, 0E:1, 1D:1, 22:302, 25:282152, 30:3,
40:1, 41:223, 42:45, 58:1, 78:2, FF:5.

<a id="s11-2"></a>
### 11.2 All archives (structural walk)

| Archive | Entries | Nodes | Depth | Compressed | Fragmented | High bytes present |
|---|---|---|---|---|---|---|
| anim | 25,400 | 689 | 2 | 22,245 | 0 | 05 |
| cell_1 | 94,877 | 2,759 | 3 | 93,323 | 0 | 27, 80 |
| cell_2 | 98,921 | 2,915 | 3 | 97,503 | 0 | 27, 80 |
| cell_3 | 88,015 | 2,631 | 3 | 86,484 | 0 | 27, 80 |
| cell_4 | 96,370 | 2,669 | 3 | 94,139 | 0 | 27, 80 |
| cell_5 | 86,381 | 2,543 | 3 | 85,139 | 0 | 27, 80 |
| cell_14 | 69,363 | 2,215 | 3 | 69,195 | 0 | 27, 80 |
| cell_15 | 4 | 1 | 0 | 0 | 0 | (FF only) |
| gamelogic | 845,460 | 21,772 | 3 | 0 | 0 | 07, 0C, 34, 43, 47, 56, 70, 76, 78, 79 |
| general | 261,483 | 7,983 | 3 | 0 | 3 | see §11.1 |
| highres | 15,447 | 450 | 2 | 13,233 | 0 | 41 |
| highres_aux_1 | 14,579 | 423 | 2 | 13,101 | 0 | 41 |
| highres_aux_2 | 40,805 | 1,098 | 2 | 40,053 | 0 | 41 |
| highres_aux_3 | 7,163 | 228 | 2 | 7,008 | 0 | 41 |
| local_English | 311,929 | 8,654 | 3 | 0 | 204 | see §11.1 |
| map_1 … map_5, map_14 | 65,029–65,030 each | 2,036–2,167 | 2–3 | 0 | 0 | 80 |
| map_15 | 4 | 1 | 0 | 0 | 0 | (FF only) |
| mesh | 37,489 | 1,030 | 2 | 37,402 | 39 | 06 |
| mesh_aux_1 | 10,081 | 287 | 2 | 10,052 | 0 | 06 |
| sound | 12,626 | 414 | 2 | 0 | 0 | 0A |
| sound_aux_1 | 2,526 | 80 | 2 | 0 | 2 | 0A |
| surface | 441,041 | 13,813 | 3 | 327,483 | 8 | 41 |
| surface_aux_1 | 541,886 | 16,920 | 3 | 464,271 | 0 | 41 |
| surface_aux_2 | 346,932 | 10,029 | 3 | 280,331 | 0 | 41 |

Totals: 3,838,957 entries (3,838,840 game records and 117 metadata records), 112,267 nodes,
1,740,962 compressed records, 256 fragmented records. Zero errors across I-1…I-33.

Per-set totals did not change between 2026-10-03 and 2026-10-05 (for example surface
1,329,859 and highres 77,994 entries), while the split between parts did.

---

<a id="s12"></a>
## 12. Corrections to earlier research notes

These replace facts recorded in the dirty-room notes. Spec readers do not need those notes.
The list exists for reviewers.

1. **Fragment table location.** The `{length, offset}` pairs of a fragmented record are at
   the **end** of the first extent, not right after the allocation header. The payload
   always starts at allocation offset + 8.
2. **Entry +24** is the size of the first extent only. It can be smaller than `length + 8`
   when the record is fragmented.
3. **0x144** is a minimum allocation size, not a fixed block size for chaining.
4. **Free space** in version 112 is a sorted table of `{size, offset}` pairs held in one
   allocation (0x154 / 0x1A0, count at 0x15C). 0x19C is the total free payload. The chain model
   applies to version 111, where 0x158 is the chain's tail.
5. **0x14C** is not a single version number. Bytes 0x14C–0x153 form a resource identifier:
   variant, kind (0x14E), dataset type (0x14F), subset (0x150).
6. **Node allocation size** is not always 2460. It equals 0x144 when that is larger, and it
   can be larger still for reused extents. The payload is always 2452.
7. **Compression** is not used at all in `general`, `gamelogic`, `local_English`, `sound*`
   or `map_*` in this install.
8. **DID echo** is not universal ([§6.2](#s6-2)).
9. **0x190/0x194/0x198** (104/32/8) and the **0xFFFF0005 set-membership record** are new
   findings; the three sizes are ignored by the client.
10. **Legacy layout selection.** The chained layout is chosen by entry flag bit 1 (records) or
    always (directory nodes, version-111 archives), not by header word +4 ≠ 0.
11. **The journal** is a separate `.jrnl` file guarded by header 0x16C. The 64 bytes at 0x100
    are a write-only marker, not a journal.
12. **Flags** take the values 0, 2 and 3, not only 2 and 3.

---

<a id="s13"></a>
## 13. Change log

| Date | Change |
|---|---|
| 2026-10-03 | First version, written from research notes and confirmed or corrected by full B-tree walks of all 29 archives in the user's install. |
| 2026-10-05 | Revised with method M4 (BIN, client build 4906.0071.1794.4035, questions Q1–Q11) and a full re-observation after the 2026-10-04 content re-application, including inflation of every compressed record. Resolved the 22 OPEN items of the first version (answers in place and in [§14](#s14)). New: flag bit 1 selects the layout (§5.3); chained-layout algorithm (§4.3); pending journal and `.jrnl` format (§3.6); client open checks (§3.7); resource identifier, archive table and language/region naming (§2.1, §2.4, §3.4); free-table capacity, relocation and invalid marker (§4.5); writer rules (§4.4, §4.7, §6.1); meanings of `stamp`, `time`, `iteration` (§5.2); metadata record roles (§7); the client's routing order (§8.2–§8.3); invariants I-26…I-32; I-6, I-7, I-9, I-10, I-11, I-17, I-18 changed. Corrected: 0x158 is not always 0; flags 0 occurs; 0x190–0x198 must not cause a refusal (I-6); 0x100 area is not a journal. Updated all per-archive observations to 2026-10-05. Provenance rows P32–P46 added. |
| 2026-10-05 (revision after review) | Answers the review of 2026-10-05. §6.1: the writer's 80 % rule measures the zlib stream (*Z* = `length` − 4), not the stored length; census re-run (0 failures, 17 through the 512,000-byte clause, 5,758 failures if the prefix were counted); worked example added. §5.5: the "no DID" value is 0 (new BIN question Q12, P47); §5.1 notes that new nodes zero their slots; new invariant I-33. §3.4.1 (new): the older identifier encoding, its conversion and a reader rule (Q13, P48), replacing the former note. §2.3 rewritten after re-reading the set-opening code (Q14, P49): discovery by pattern, byte-wise name order, the exact identifier rules, the part-count test (inequality, not only "fewer"), and deletion of the set's files when an aux part fails in the normal open. §3.7 and §9: access and sharing per mode (Q15, P50). §4.7: provenance (Q16, P51), the modular arithmetic of the full test, the "no maximum" case and a worked example. I-17 now says value 1 gives no warning. §8.2: table heading and cells name only ranges that hold records, with the occurring high bytes of multi-byte ranges; 0x13, 0x1B and 0x1C (no records, and no registration was found for them) removed from the kind-3 row; absent registered ranges listed. The client-table name of kind 0x0D removed (§2.4, §3.4). Q8 extended to the writer policy. |
| 2026-10-06 | Answers the re-review of 2026-10-05. §7.2: the set-identifier row no longer says all zero is "allowed only for a one-part set"; it now agrees with §2.3 (all zero allowed on the base of a set of any size, never on an aux part). §2.3 step 3: re-checked against the client (Q14) and the consequence stated (the base's identifier is adopted the same way; with an all-zero base the first aux part's identifier is adopted and its index is not checked); observation added that no archive has an all-zero identifier. I-24: the "all parts share the identifier" clause now exempts an all-zero base and says an all-zero aux identifier warns. P49 updated. |
| 2026-10-06 (second revision) | Answers the re-review of 2026-10-06. Provenance: removed the note "identifier rules re-checked on 2026-10-06" from BIN questions Q15 and Q16, where it had been added by mistake (it belongs to Q14 alone). Answers the seven implementer notes of [§14](#s14) (audit of 2026-10-06) with two new BIN questions, both read on 2026-10-06 under decision 0011 (address ranges of the archive library only): Q17, how a part's 0xFFFF0005 is loaded (P52), and Q18, how the free table is loaded (P53). New text: §2.1 (names the client opens without the magic are errors; the aux pattern counts any match), §2.3 (aux counts are never compared; reader severity table), §4.5 (the client reads the table only on the write side; offset 0 with size 0 means no table; pairs read up to the first zero size; the sum test), §7.2 (load rules and defaults of 0xFFFF0005). Invariants changed: I-21 (terminator position, no-table case), I-22 (no-table case), I-24 (what counts as absent), I-25 (which archives it covers). Probe of 29 archives on 2026-10-06: every free table passes the client's test with its first zero-size pair at index 0x15C, and every 0xFFFF0005 has `stamp` 1, 24 bytes and flag bit 0 clear. |
| 2026-10-07 | Revision (status Revised, awaits review). Closes three of the four OPEN items with method M4 (BIN questions Q19–Q22, client build 4906.0071.1794.4035, archive library and archive initialisation only) and the client's own log (new source tag LOG). **§2.5:** the overlay file name is the base's full file name plus `.ovl` (`client_general.dat.ovl`), not `client_general.ovl` (P54). **§3.7:** the game client opens every archive with expected datpack versions −1 and −1, so it compares neither 0x174 nor 0x178 (P55, confirmed by the log for all 29 archives); reader rule unchanged. **§4.7:** the game client's configured maximum length is 0xFFFFFFFF, with a worked example of the modular full test; a newly created archive starts 1 MiB long (P56). **§7.3:** the trailing list exists only for `stamp` 2, `stamp` ≥ 3 is refused, the u64 is copied verbatim (P57). **§7.5:** the product-identifier accessors and the absence of any reader of 0xFFFF0004 in the game client (P57); the OPEN now names only what the patcher holds. Non-blocking notes of the re-review of 2026-10-06 applied: I-24 punctuation; the §2.3 severity rule now covers "number of parts ≠ base count"; §2.3 says how a reader orders (does not order) a non-canonical aux match. **No implementer action** is required by the closed OPENs (reader rules unchanged); `crates/dat` may name the overlay it reports by the §2.5 rule. |
| 2026-10-07 (second revision) | Answers the review of 2026-10-07 (status still Revised, awaits re-review). BIN reads in the repair session of 2026-10-07 (client build 4906.0071.1794.4035, archive library only): the Q20 comparison re-read, and new questions Q23 and Q24. **Blocking issue, §3.7:** "Expected datpack versions" now states the rule as numbered steps: either comparison alone refuses, and the refusal is waived when the expected container version and 0x174 are 111 and 112 in either order, even if the game-data comparison failed; worked cases added. The 0x174 and 0x178 rows of the §3.7 table carry the same rule and waiver (P55 updated). **Note a:** "any value is accepted at open" replaced by "compares neither … with an expected value"; new OPEN with a reader rule for a 0x174 other than 111 or 112. **Note b, §4.7:** new paragraph "Versions stamped at creation" (P58): 0x174 = expected container version if ≥ 1, else 111; 0x178 = expected game data version unchanged; a set creating an aux part takes the expected versions from its overlay or first part, so the new part matches its base (112/3 here); new OPEN, with a reader rule, on whether the game client ever creates a stand-alone archive. **Note c, §2.5:** new paragraph "Directory" (P59): the overlay is looked for in the set's directory, or the current directory when none is named; new OPEN, with a reader rule, on a second lookup in the current directory. **Implementer action:** check `crates/dat` against the reader rules of the new OPENs in §2.5, §3.7 and §4.7; the earlier reader rules are unchanged. |

---

<a id="s14"></a>
## 14. Implementer notes and OPEN items (`crates/dat`, 2026-10-04)

Choices the clean-room implementation had to make where the first version of this spec was
silent. Each is answered below (2026-10-05).

> **Answered:** Legacy chained layout ([§4.3](#s4-3)). The client selects the chained layout
> **per allocation kind**, not from header word +4: directory nodes always, records when
> entry flag bit 1 is clear, and everything in a version-111 archive. The implementation's
> rule (word +4 ≠ 0) gives the same bytes for all data seen so far, but a record with flags 0
> and a header such as `{X ≠ 0, 0}` would be misread as fragmented; switch on flag bit 1.
> When a block holds more payload than is still needed, the **first** `remaining` bytes of
> its payload are used, which is what the implementation does. The first block of a chain
> holds the end of the payload. Keep the runtime switch, the step limit and the "unsupported"
> default for version-111 archives if desired; flags-0 records in version-112 archives must be
> readable (8 exist in this install, all single blocks).

> **Answered:** Lower bound for fragment and block offsets. 0x400 is correct: the header
> occupies 0x000–0x3FF and every allocation of the tiling lies at or above 0x400
> ([§4](#s4)). The client itself does not check bounds; rejecting is the safe choice.

> **Answered:** A stored length larger than the file length cannot be supplied by
> non-overlapping extents; rejecting it early is correct. The caps (stored 256 MiB, inflated
> 128 MiB) are above every observed value (stored 40,916,068; inflated 17,984,776). The
> client has no cap.

> **Answered:** Empty root. Yes: a newly created archive has a root leaf with n = 0 until
> its first record is written ([§5.4](#s5-4)). Accept it as an empty archive (I-10 warning).
> An interior node with n = 0 cannot be produced by the writer; rejecting it is correct.

> **Answered:** Fragmented directory nodes. They do not exist: directory nodes are read with
> the **chained** layout, and node header word +0 is the size of a next block, never a
> fragment count. Do **not** apply the extent-layout path to nodes. A node with header
> `{0, 0}` is a single block. A non-zero header would be a multi-block chain (legal for the
> client, never observed); keeping I-9 as an error is acceptable.

> **Answered:** Duplicate DIDs across archives. The client returns the first copy in its
> search order and never compares `iteration` or `time` ([§8.2](#s8-2)). Returning the first
> copy in routing order is therefore correct, provided the routing order is the client's.

> **Answered:** Routing order for shared high bytes. The client's order is: high-resolution
> set (only for portal types), then the type's home set, then the language set
> ([§8.2](#s8-2)). For 0x41 that is `highres` (+aux) → `surface` (+aux) → `local`; for 0x0A
> `highres` → `sound` (+aux) → `local`; for 0x42 `highres` → `general` → `local`; for 0x22
> only `local`. (The `highres` probe for non-texture types never finds anything in this
> install but is part of the client's order.) The current
> default table (most records first: `surface` before `highres`, `local` before `general`)
> differs; it gives the same result today only because no DID is duplicated. Change it to the
> client's order.

> **Answered:** Leaf strictness. The client decides "leaf" from child 0's offset alone and
> never looks at a leaf's other pointers. No observed leaf has a non-zero pointer 1…n.
> Refusing such a leaf is stricter than the client; a warning (I-11) and reading it as a leaf
> matches the client.

Engineering choices made by `crates/dat` while fixing the audit of 2026-10-06, where this spec
is silent. They are recorded here for review; none of them changes a format fact. Each is
answered below (2026-10-06, with BIN questions Q17 and Q18).

> **Implementer note:** Set-level severities (I-24 and open failures). The implementation reports a file
> named like an archive but without the magic (too short, or wrong magic) as a **warning**
> (§2.1: not an archive), and a file whose magic is valid but that cannot be opened (I-4,
> I-6 entry size, I-8, or a read error) as an **error** under that invariant, since every DID
> it holds is lost. A missing part, a part count that differs between parts and a declared
> count above 64 are **errors** for the set (§2.3); a wrong part index, a disagreeing
> identifier, an all-zero identifier on an aux part and an absent or unreadable 0xFFFF0005
> stay I-24 **warnings**. Is that split the intended reading of §2.3 and §10?
>
> **Answered:** Partly. The general rule and a table are now in [§2.3](#s2-3): Error when
> records of the set cannot be read, Warn when every file is readable but the membership
> metadata disagrees. Two changes follow from it. (1) A file with a name the client opens
> (a base name of [§2.4](#s2-4), or a match of a set's aux pattern) that is too short or lacks
> the magic is **not** a foreign file. The client fails that set, so report an **Error** (I-1,
> I-2) for the set. Only `client_<x>` names that the client never opens stay warnings
> ([§2.1](#s2-1)). (2) A part count that differs between parts is a **Warn** (I-24), not an
> Error: the client compares only the base's count with the number of parts and never reads
> an aux part's count ([§2.3](#s2-3)). A missing part and "number of parts ≠ base count" stay
> Errors. The cap of 64 parts is the reader's own resource limit, and Error is acceptable for
> it. The rest of the split (index, identifier, all-zero aux identifier, absent or unreadable
> 0xFFFF0005 as I-24 warnings) is the intended reading.

> **Implementer note:** Size cap for 0xFFFF0005. The record is read with a stored and inflated cap of
> 1,024 bytes (24 needed) and must decode to at least 24 bytes, or it is corrupt. A
> compressed 0xFFFF0005 (never observed, §7.1) is still decoded. Should a compressed or
> longer-than-24-byte membership record be refused outright?
>
> **Answered:** Do not refuse either case ([§7.2](#s7-2), BIN Q17). The client reads the
> record with its ordinary read, so a compressed copy is inflated and is valid. It uses the
> first 24 bytes and ignores the rest, so a longer record is valid too. It ignores the record
> when `stamp` ≠ 1. When the record is shorter than 24 bytes, the fields that are not complete
> keep their defaults (all-zero identifier, index 0, count 1). It never fails the open because
> of the load itself. Treat absent, `stamp` ≠ 1 and shorter than 24 bytes as "absent" (I-24
> warning, defaults). A stored or inflated cap of 1,024 bytes is a fine resource limit, but a
> record above it must give an I-24 warning, not "corrupt": the client accepts it.

> **Implementer note:** Validator work on overlapping records (I-20, I-23). With full inflation (I-23)
> the validator inflates a compressed record only if none of its extents overlaps bytes
> already inflated for another record or itself; a skipped record gets an I-23 warning (the
> overlap itself is the I-20 error). Out-of-range fragments are reported once per record
> (I-19, "N of F fragments out of range"), not once per fragment. The reader keeps
> per-record caps only (§9); callers that read every record are told to budget the
> aggregate. Acceptable?
>
> **Answered:** Acceptable. The client never checks tiling or overlap and has no aggregate
> limit, so these are validator choices with no client behaviour to match. Skipping inflation
> of a record whose extents overlap is a safe way to bound the work. Report the overlap under
> I-20 (Error) and the skipped inflation under I-23 (Warn). One I-19 finding per record is
> fine.

> **Implementer note:** Language selection in the set. The set routes the language set (§8.2) to the
> archives of one language only: the one requested by the caller, else `English`, else the
> lexicographically first label (install-layout §3.5); a requested label that is absent is
> a warning and the default is used. Archives of other languages stay open and are
> validated, but `locate` never searches them (no cross-locale fallback, string-table
> §10.1) and they take no part in I-25. Is excluding them from I-25 right?
>
> **Answered:** Yes. The client opens exactly one language set, the one for its selected
> language ([§2.4](#s2-4)), together with the portal and region sets. Archives of other
> languages are never open alongside them, so a DID they share with the selected language
> has no effect on the client. Including them could report duplicates that do not matter.
> (Only one language archive is installed here, so whether they share DIDs is not observed.)
> I-25 now says which archives it
> covers ([§10](#s10)). The fallback order (requested, else `English`, else the first label)
> belongs to [`install-layout.md`](install-layout.md) §3.5 and is not a container fact.

> **Implementer note:** File-name grammar. Archive names are matched ASCII case-insensitively against
> install-layout §3.1 (aux parts only as `.datx`, `<r>` and `<N>` without leading zeros, an
> unknown key word kept as an "other" archive). When two files differ only in case, the
> canonical spelling is opened (else the lexicographically smallest name) and the other is
> reported as a warning (install-layout §2). For a language archive the canonical label is
> not known to the container layer, so two spellings of one label resolve to the smallest
> name. Acceptable?
>
> **Answered:** Acceptable, with one addition ([§2.1](#s2-1)). The client runs on a
> case-insensitive file system, so case-insensitive matching is right, and keeping one file
> for names that differ only in case is a reasonable rule where the host file system allows
> both. The addition: the client's aux search counts **every** name matching
> `client_<name>_aux_*.datx` (for example `_aux_01.datx` or `_aux_x.datx`) and checks it by
> position. Report such a non-canonical match as a warning that the client would count it as
> a part. For two spellings of one language label, choosing the smallest name is fine. A
> caller that knows the label from the Languages enumeration can ask for the canonical
> spelling.

> **Implementer note:** Free-space table check (§4.5, I-21). The reader now requires
> `16 + 8 × count ≤ size` (header, pairs and terminating pair) and has no special case for a
> zero-sized table, so an archive without a table gets an I-21 warning. The terminator's
> value and the 0x154 = 0xFFFFFFFF case (§4.5, I-27) are not yet checked by the validator.
>
> **Answered:** BIN Q18, now in [§4.5](#s4-5) and I-21, I-22. Offset 0 **and** size 0 is the
> client's "no table" state (empty free list, nothing read). Report it as Info, not an I-21
> warning, and check I-20 and I-22 with an empty list (0x15C and 0x19C should then be 0).
> Exactly one of the two zero is a broken table (I-21 warning). Otherwise the client reads
> pairs up to the first pair with size 0, not up to 0x15C. The validator should check that
> this pair is the one at index 0x15C and is `{0, 0}`, and should apply the client's one test:
> the sizes of the pairs read sum to 8 × 0x15C + 0x19C. The 0x154 = 0xFFFFFFFF case remains
> I-27 (Warn) and skips I-20…I-22. Keep `16 + 8 × count ≤ size` as a reader bound.

> **Implementer note:** Unopenable archives in tools (`apps/dattool`, audit of 2026-10-06). For a
> directory input, `dattool validate` counts a file it cannot open as an **error** when its
> name is one of the archive kinds of §2.1 (`general`, `gamelogic`, `local_<Lang>`, `mesh`,
> `surface`, `highres`, `sound`, `anim`, `cell_<r>`, `map_<r>` and their aux parts), also when
> it is too short or lacks the magic (I-1, I-2: an interrupted patch leaves such a file); an
> unknown `client_<x>` name stays a warning. `info`, `stat`, `snapshot` and `diff` treat such
> an archive as an input that could not be read completely (status 1, or 2 for `diff`). This
> is stricter than the set's own warning for a recognised name without the magic (note
> above). Should a recognised name without the magic be an error everywhere?
>
> **Answered:** Yes. A recognised name (any archive kind of [§2.1](#s2-1) and its aux parts,
> including a match of the aux pattern) that is too short or lacks the magic is an **Error**
> (I-1, I-2) everywhere, in the set and in every tool. The client fails that set, so the
> tools' current behaviour is right and the set's warning should be raised to match
> ([§2.3](#s2-3) table). Unknown `client_<x>` names stay warnings.

---

<a id="s15"></a>
## 15. Corrections to other specs

These are recorded here instead of editing the other specs, for their authors to apply.

1. **`install-layout.md` P35 and §9.6 invariant 9** ("highest directory-entry iteration =
   highest listed iteration − 1", stated for `general` and `map_5`): still true for those two,
   but not a general rule. Counting only the base archive it holds in 10 of the 20 sets with
   game records on 2026-10-05 (for example `highres` 56,024 against 63,224, `map_14` 78 against
   201). The general facts are in [§5.2](#s5-2) and [§7.3](#s7-3): every non-zero entry
   iteration of a set is a member of the base archive's list and below its highest value.
2. **`install-layout.md` §3.2 OPEN** (the second byte of 0x14C; the 0x150 codes for DE and FR;
   the meaning of regions): answered in [§3.4](#s3-4) and [§2.4](#s2-4). Byte 0x14E is the
   archive kind; language archives carry the folded language code (DE 0x1004, FR 0x1003,
   EN_GB 0x1001, English 2); regions are the keys of the RegionID DID map.
3. **`install-layout.md` §3.5**: language names are the labels of the Languages enumeration;
   the install lists `EN_GB` in addition to `English`, `DE`, `FR`.
4. **`install-layout.md` §3.1 OPEN** (aux members for region and language archives): every
   archive goes through the client's set logic, so aux members are possible for any archive
   ([§2.3](#s2-3)); none exist in this install.
5. **`install-layout.md` §9.2.1**: each trailing entry of 0xFFFF0001 is a u32 iteration and a
   u64 value (the meaning of the u64 remains open). §9.2.3: the client's archive library never
   reads 0xFFFF0004.
6. **`install-layout.md` §9.1 OPEN** (meaning of 0x170, 0x17C, 0x18C, 0x190–0x19F): answered in
   [§3.3](#s3-3).
7. **`records.md` §2.1, row 0xFF** ("OPEN here"): the roles of the metadata records are now in
   [§7](#s7).
