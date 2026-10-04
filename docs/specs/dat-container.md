<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# DAT archive container (`.dat` / `.datx`)

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-03 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/dat` (container, extents, directory, zlib, archive set, DID routing), `apps/dattool` |

---

<a id="s0"></a>
## 0. Purpose

This spec describes the on-disk container used by the game's data archives (`client_*.dat`
and `client_*_aux_N.datx`) closely enough that an implementer can:

- open every archive in a current install read-only and validate its file header;
- walk the directory B-tree, look up a record by its 32-bit data identifier (DID), and
  enumerate all records in DID order;
- reassemble a record's stored bytes from its extents, including fragmented records;
- inflate compressed records;
- decide which archive(s) of the set to search for a given DID;
- check a set of invariants that held on every archive of a real install.

It covers the container only. What is inside a record (meshes, textures, string tables and so
on) belongs in other specs. Writing archives is out of scope. Facts about the write side
appear only where a reader needs them to explain what it sees.

All values in this spec that were observed in a user's install are **observations**.
They are not constants to hard-code. Code must read them from the archive at run time.

---

<a id="s1"></a>
## 1. Provenance

Legend:
- **C**: Confirmed, observed in user install 2026-10-03 (read-only probes over all
  29 archives, with full B-tree walks).
- **R**: Reported, from published sources only, not observed (or not observable) in this install.

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
| OBS | Our own black-box observation of the user's install, 2026-10-03 (probes in the spec writer's scratch area; no data committed) |

| # | Fact (section) | Sources | Mark |
|---|---|---|---|
| P1 | Archive file names, `.datx` extension for aux parts ([§2](#s2)) | LC, OBS | C |
| P2 | Header area at 0x000–0x0FF is a NUL-padded version string; empty in this install ([§3.1](#s3-1)) | CHZ, OBS | C (empty observed) |
| P3 | Journal area at 0x100 begins `00 50 4C 00`, rest zero ([§3.2](#s3-2)) | CHZ, JT, OBS | C |
| P4 | Magic 0x00005442 at 0x140 ([§3.3](#s3-3)) | LC, JT, CHZ, ACE, MOG, OBS | C |
| P5 | 0x144 is the minimum allocation size (called "block size" in sources) ([§3.3](#s3-3), [§4.4](#s4-4)) | LC, JT, DO (name); OBS (meaning) | C, meaning corrected |
| P6 | 0x148 equals the actual file length ([§3.3](#s3-3)) | LC, JT, DO, OBS | C |
| P7 | 0x14C split into dataset type (byte 0x14F) and archive kind (byte 0x14E) ([§3.4](#s3-4)) | CHZ, ACE (type enum for sibling engine); OBS | C (values); meaning partly inferred |
| P8 | 0x150 is the region number for cell/map archives ([§3.4](#s3-4)) | CHZ (sibling: "subset = region"), OBS | C |
| P9 | 0x154/0x15C/0x1A0/0x19C describe the free-space table ([§4.5](#s4-5)) | DO (partly), OBS | C, **corrects** DO's chain model |
| P10 | 0x160/0x1A4 are root node offset and root extent size ([§3.3](#s3-3)) | LC, JT, DO, OBS | C |
| P11 | 0x170 is a "master map" DID in `general` only ([§3.3](#s3-3)) | ACE, CHZ (sibling), OBS (value present, DID exists) | C (value), R (meaning) |
| P12 | 0x174 = 112 everywhere (engine pack version); 111 = legacy layout ([§3.3](#s3-3), [§4.3](#s4-3)) | LC, KD, OBS | C (112), R (111 meaning) |
| P13 | 0x178 = 3 for portal and local archives, 0 for cell and map ([§3.3](#s3-3)) | LC, OBS | C |
| P14 | 0x17C 16-byte identifier, 0x18C u32 ([§3.3](#s3-3)) | CHZ, ACE (sibling names), OBS | C (values), R (meaning) |
| P15 | 0x190/0x194/0x198 = 104/32/8 ([§3.3](#s3-3)) | OBS | C (values), meaning inferred |
| P16 | 8-byte allocation header; modern layout with `{fragCount, 0}` ([§4.1](#s4-1)) | LC, OBS | C |
| P17 | Fragment table sits at the **end** of the first extent ([§4.2](#s4-2)) | OBS | C, **corrects/clarifies** LC-derived note |
| P18 | Legacy chained layout (header second word ≠ 0) ([§4.3](#s4-3)) | LC, DO | R (never observed) |
| P19 | Allocation sizing rule `max(minAlloc, roundUp4(len+8))` ([§4.4](#s4-4)) | OBS | C |
| P20 | Exact tiling of the file by allocations ([§4.6](#s4-6)) | OBS | C |
| P21 | Directory node: 62 child pointers {size, offset}, count at payload +496, 61 × 32-byte entries, payload 2452 bytes ([§5.1](#s5-1)) | LC, JT, OBS | C |
| P22 | Entry layout and field meanings ([§5.2](#s5-2)) | LC, DO, JT, MOG, OBS | C (layout); meanings as marked |
| P23 | Flag values 2 (stored) and 3 (zlib) only ([§5.3](#s5-3)) | LC, DO, MOG, OBS | C |
| P24 | B-tree min 30 / max 61 entries per non-root node, uniform leaf depth ([§5.4](#s5-4)) | OBS | C |
| P25 | Lookup and in-order traversal ([§5.5](#s5-5), [§5.6](#s5-6)) | LC, JT, OBS | C |
| P26 | Compressed payload = u32 inflated length + zlib stream ending exactly at stored length ([§6.1](#s6-1)) | LC, JT, MOG, OBS | C |
| P27 | Payload DID echo, by type ([§6.2](#s6-2)) | LC, MOG, OBS | C, **corrects** "always at +0" |
| P28 | Reserved metadata DIDs 0xFFFF00xx, incl. set-membership record 0xFFFF0005 ([§7](#s7)) | OBS (and an AC2 hint for 0xFFFF0001) | C |
| P29 | DID → archive routing ([§8](#s8)) | LC, OBS | C (observed table), R (LC order) |
| P30 | No DID appears in two archives of an install ([§8.3](#s8-3)) | OBS | C |
| P31 | 32-bit offsets, aux split near 4 GiB ([§9](#s9)) | LC, DO, OBS | C (sizes), R/inferred (policy) |

---

<a id="s2"></a>
## 2. Archive set and naming

<a id="s2-1"></a>
### 2.1 File names

- Archives sit directly in the install's root directory (not in `x64/`). **C**
- The base archive of a kind is named `client_<kind>.dat`. **C**
- Overflow parts of the same kind are named `client_<kind>_aux_<N>.datx`, with N = 1, 2, 3, ….
  The extension really is `.datx`. **C**
- Localised archives are named `client_local_<Language>.dat`, e.g. `client_local_English.dat`. **C**
- Other `*.dat` files in the install, such as `icudtl.dat` and an uninstaller's `unins000.dat`,
  are **not** archives. They do not carry the magic at 0x140. A locator must select files by the
  `client_` prefix and verify the magic. **C**

<a id="s2-2"></a>
### 2.2 Archives present in the observed install

Kinds and parts observed on 2026-10-03 (29 archives):

| Kind | Files | Notes |
|---|---|---|
| general | `client_general.dat` | |
| gamelogic | `client_gamelogic.dat` | |
| local | `client_local_English.dat` | one per installed language (R: LC enumerates languages) |
| mesh | `client_mesh.dat`, `client_mesh_aux_1.datx` | |
| surface | `client_surface.dat`, `client_surface_aux_1.datx`, `client_surface_aux_2.datx` | |
| highres | `client_highres.dat`, `client_highres_aux_1.datx` … `_aux_3.datx` | optional in LC (R) |
| sound | `client_sound.dat`, `client_sound_aux_1.datx` | |
| anim | `client_anim.dat` | optional in LC (R) |
| cell | `client_cell_{1,2,3,4,5,14,15}.dat` | region number in file name |
| map | `client_map_{1,2,3,4,5,14,15}.dat` | region number in file name |

- `cell_15` and `map_15` exist but are 1 MiB placeholders that hold only metadata records
  (§7). **C**
- An install is valid for LC if `client_general.dat` exists. **R**

> **OPEN:** The full set of language names used in `client_local_<Language>.dat` is not
> observed. Only `English` is present here.

<a id="s2-3"></a>
### 2.3 Set membership

The parts of one kind form a **set**. Each archive in a set carries a membership record at
DID 0xFFFF0005 ([§7.2](#s7-2)). That record gives a 16-byte set identifier, shared by all
parts, plus the part's index (0 for the `.dat` base) and the set's part count. **C**

A reader should discover aux parts in two ways: by the file-name pattern, and by checking the
membership records. It should report a missing part (index < count but no file present) as
an error for that set, not crash.

---

<a id="s3"></a>
## 3. File header

All integers are little-endian. "u32" is unsigned 32-bit. Offsets are absolute file offsets.
The header occupies 0x000–0x3FF. The first allocation starts at 0x400. **C**

<a id="s3-1"></a>
### 3.1 Version string area (0x000–0x0FF)

| Offset | Size | Type | Meaning | Mark |
|---|---|---|---|---|
| 0x000 | 256 | bytes | NUL-padded ASCII version string. **All zero** (empty) in all 29 observed archives. | C (empty), R (meaning) |

<a id="s3-2"></a>
### 3.2 Journal area (0x100–0x13F)

| Offset | Size | Type | Meaning | Mark |
|---|---|---|---|---|
| 0x100 | 64 | bytes | Transaction journal. In every observed archive it is exactly `00 50 4C 00` followed by 60 zero bytes, which sources describe as the "empty journal" state. | C (bytes), R (meaning) |

> **OPEN:** The journal's non-empty format (written while a patch is in progress, if the
> archive is opened "journalled") is unknown. A read-only reader should warn, not fail, if the
> 64 bytes are not the empty pattern. It may then be reading an archive that the launcher was
> interrupted while patching.

<a id="s3-3"></a>
### 3.3 Superblock (0x140–0x1A7)

Each column "Observed" summarises all 29 archives (per-archive values are in [§3.5](#s3-5)).

| Offset | Size | Type | Meaning | Observed | Mark |
|---|---|---|---|---|---|
| 0x140 | 4 | u32 | Magic. Must be 0x00005442 (bytes `42 54 00 00`). | all | C |
| 0x144 | 4 | u32 | Minimum allocation size ("block size"), see [§4.4](#s4-4) | 0x100, 0x400, 0x1000, 0x4400, 0x5DC0, 0xA000 | C |
| 0x148 | 4 | u32 | File size in bytes; equals the real file length | equal in all 29 | C |
| 0x14C | 4 | u32 | Dataset descriptor, see [§3.4](#s3-4) | | C |
| 0x150 | 4 | u32 | Subset: region number for cell/map, 2 for local, 0 otherwise ([§3.4](#s3-4)) | | C |
| 0x154 | 4 | u32 | Offset of the free-space table extent ([§4.5](#s4-5)) | | C |
| 0x158 | 4 | u32 | Unused here (sources: free-list tail) | 0 in all | C (value) |
| 0x15C | 4 | u32 | Number of free extents listed in the free-space table | 1 … 84 | C |
| 0x160 | 4 | u32 | Offset of the root directory node allocation | | C |
| 0x164 | 12 | 3 × u32 | Unused here (sibling engine: LRU bookkeeping) | 0 in all | C (value), R (meaning) |
| 0x170 | 4 | u32 | "Master map" DID. Non-zero only in `general` (0x28000000, and that DID exists in `general`); 0 elsewhere | | C (value), R (meaning) |
| 0x174 | 4 | u32 | Engine/container format version | 112 in all | C |
| 0x178 | 4 | u32 | Game data version | 3 for portal-type and local archives; 0 for cell and map | C (values), R (meaning) |
| 0x17C | 16 | bytes | Identifier (GUID-like). One value is shared by 27 archives. All zero in the two most recently created aux parts (`mesh_aux_1`, `highres_aux_3`). | | C (values), R (meaning: "major version id") |
| 0x18C | 4 | u32 | Minor version counter | 1100 or 17139; 0 where 0x17C is zero | C (values), R (meaning) |
| 0x190 | 4 | u32 | Constant 104. Equals the superblock length 0x1A8 − 0x140. | 104 in all | C (value), meaning inferred |
| 0x194 | 4 | u32 | Constant 32. Equals the directory entry size ([§5.2](#s5-2)). | 32 in all | C (value), meaning inferred |
| 0x198 | 4 | u32 | Constant 8. Equals the child-pointer / allocation-header size. | 8 in all | C (value), meaning inferred |
| 0x19C | 4 | u32 | Total free payload bytes = sum over listed free extents of (extent size − 8) | equal in all 29 | C |
| 0x1A0 | 4 | u32 | Size in bytes of the free-space table extent at 0x154 | | C |
| 0x1A4 | 4 | u32 | Size in bytes of the root node allocation at 0x160 | 2460 when 0x144 < 2460, else equal to 0x144 | C |
| 0x1A8 | 600 | bytes | Padding to 0x400 | all zero in all 29 | C |

Notes:
- Sources call 0x154 the "free-list head" or "journal offset". Observation shows that it
  points to a **table** of free extents, not to the head of a chain ([§4.5](#s4-5)). **C**
- The 0x1A4 relationship is a consequence of the allocation rule ([§4.4](#s4-4)), not a
  separate requirement. A reader must take the root size from 0x1A4 and not assume 2460.

> **OPEN:** Meaning of 0x17C/0x18C in this engine. The identifier is constant across the
> install except for newly created aux parts, so it is not a per-file ID. The two values of
> 0x18C (1100, 17139) split along the same lines as the presence of metadata record
> 0xFFFF0004 ([§7](#s7)).

> **OPEN:** Whether 0x190/0x194/0x198 are self-describing layout sizes (104/32/8) and
> could differ in other versions. Until known, a validator should warn if they differ from
> 104/32/8 and refuse to parse directory nodes if 0x194 ≠ 32.

<a id="s3-4"></a>
### 3.4 Dataset descriptor (0x14C) and subset (0x150)

Read 0x14C as four bytes:

| Byte offset | Meaning | Values observed | Mark |
|---|---|---|---|
| 0x14C | always 0 | 0 | C |
| 0x14D | always 0 | 0 | C |
| 0x14E | archive kind within the dataset type | see table below | C (values), meaning inferred |
| 0x14F | dataset type: 1 = portal-type, 2 = cell/map, 3 = local | 1, 2, 3 | C (values); meaning from the sibling engine's enum (Portal = 1, Cell = 2, Local = 3) |

| Archive (all parts) | 0x14F type | 0x14E kind | 0x150 subset |
|---|---|---|---|
| gamelogic | 1 | 0x01 | 0 |
| mesh (+aux) | 1 | 0x02 | 0 |
| general | 1 | 0x03 | 0 |
| anim | 1 | 0x04 | 0 |
| sound (+aux) | 1 | 0x07 | 0 |
| highres (+aux) | 1 | 0x08 | 0 |
| surface (+aux) | 1 | 0x09 | 0 |
| cell_R | 2 | 0x03 | R (1, 2, 3, 4, 5, 14, 15) |
| map_R | 2 | 0x0A | R (1, 2, 3, 4, 5, 14, 15) |
| local_English | 3 | 0x03 | 2 |

- Aux parts carry the same descriptor as their base. **C**
- 0x150 equals the number in the file name for every cell and map archive. **C**

> **OPEN:** Is 0x150 = 2 for `local_English` a language code? A second language install is
> needed to check. Kind values 0x05 and 0x06 are not used by any observed archive.

<a id="s3-5"></a>
### 3.5 Per-archive header values (observed 2026-10-03)

Hex unless noted. "G" = 0x17C identifier is non-zero (the shared value), "0" = all zero.

| Archive | 0x144 | 0x148 (= length) | 0x14C | 0x150 | 0x154 | 0x15C | 0x160 | 0x178 | 0x17C | 0x18C | 0x19C | 0x1A0 | 0x1A4 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| anim | 5DC0 | 2C000000 | 01040000 | 0 | 2BE0376C | 2 | 02E38D0C | 3 | G | 44C | 1AEE150 | 5DC0 | 5DC0 |
| cell_1 | 100 | 18D00000 | 02030000 | 1 | 0E887388 | 1 | 0CB4650C | 0 | G | 44C | 997FC | 5EA4 | 99C |
| cell_2 | 100 | 1A100000 | 02030000 | 2 | 089DE25C | 21 | 0E7D4054 | 0 | G | 42F3 | 1801E8 | 5EF8 | 99C |
| cell_3 | 100 | 1C500000 | 02030000 | 3 | 12E88CEC | 54 | 12792420 | 0 | G | 42F3 | 175510 | 3F44 | 99C |
| cell_4 | 100 | 26300000 | 02030000 | 4 | 0BCDFB00 | 1E | 1A06DD00 | 0 | G | 42F3 | 5497C | 5FB8 | 99C |
| cell_5 | 100 | 19B00000 | 02030000 | 5 | 0000119C | 1 | 1186C3D4 | 0 | G | 42F3 | FF3B4 | 1F60 | 99C |
| cell_14 | 100 | 03E00000 | 02030000 | E | 03D72A10 | 1E | 02B4BDB8 | 0 | G | 42F3 | C3500 | 1F60 | 99C |
| cell_15 | 100 | 00100000 | 02030000 | F | 0000119C | 2 | 00000400 | 0 | G | 42F3 | FCEF4 | 1F60 | 99C |
| gamelogic | 100 | 3EC00000 | 01010000 | 0 | 2E848440 | 1 | 03674844 | 3 | G | 44C | A60C4 | 52088 | 99C |
| general | 400 | 2E900000 | 01030000 | 0 | 0DA3C7F4 | 1 | 0E6CB36C | 3 | G | 44C | 831F8 | 8A94 | 99C |
| highres | A000 | FF900000 | 01080000 | 0 | FF632818 | 3D | 1A8B6F94 | 3 | G | 44C | 2EC220 | A000 | A000 |
| highres_aux_1 | A000 | FF900000 | 01080000 | 0 | FF619B34 | 2 | 14508B8C | 3 | G | 44C | 2EDFF0 | A000 | A000 |
| highres_aux_2 | A000 | FF800000 | 01080000 | 0 | 950943D4 | 1 | 241DE1EC | 3 | G | 44C | 1F324C | A000 | A000 |
| highres_aux_3 | A000 | 21C00000 | 01080000 | 0 | 0000A400 | 1 | 0562E9E8 | 3 | 0 | 0 | 291A3C | A000 | A000 |
| local_English | 100 | 719DDFD0 | 03030000 | 2 | 4CD92DA4 | 1 | 4DC3F7B0 | 3 | G | 44C | B965C | 10588 | 99C |
| map_1 | 100 | 01600000 | 020A0000 | 1 | 014F6184 | 25 | 0009E364 | 0 | G | 44C | 131818 | 1F60 | 99C |
| map_2 | 100 | 01600000 | 020A0000 | 2 | 014F6084 | 15 | 0133459C | 0 | G | 42F3 | 113FA4 | 1F60 | 99C |
| map_3 | 100 | 01600000 | 020A0000 | 3 | 014F6084 | 6 | 0133459C | 0 | G | 42F3 | 10AFF8 | 1F60 | 99C |
| map_4 | 100 | 01600000 | 020A0000 | 4 | 014F6084 | 17 | 0009E364 | 0 | G | 42F3 | 1470CC | 1F60 | 99C |
| map_5 | 100 | 01500000 | 020A0000 | 5 | 0000119C | 1 | 000A03C4 | 0 | G | 42F3 | 56AE8 | 1F60 | 99C |
| map_14 | 100 | 01600000 | 020A0000 | E | 014F6084 | 2 | 0133459C | 0 | G | 42F3 | 10800C | 1F60 | 99C |
| map_15 | 100 | 00100000 | 020A0000 | F | 0000119C | 2 | 00000400 | 0 | G | 42F3 | FCEF4 | 1F60 | 99C |
| mesh | 4400 | FF800000 | 01020000 | 0 | EAC3DF10 | 4 | 07916FF4 | 3 | G | 44C | D29D0 | 4400 | 4400 |
| mesh_aux_1 | 4400 | 63B00000 | 01020000 | 0 | 00004800 | 1 | 160EABF0 | 3 | 0 | 0 | 1B28C8 | 4400 | 4400 |
| sound | 4400 | FF900000 | 01070000 | 0 | FF736F08 | 1 | 0AFA73A4 | 3 | G | 44C | 1D2514 | 4400 | 4400 |
| sound_aux_1 | 4400 | E8A00000 | 01070000 | 0 | 70876A98 | 1 | C13CC54C | 3 | G | 44C | 1D0864 | 4400 | 4400 |
| surface | 1000 | FF900000 | 01090000 | 0 | 5FD70C30 | 1 | 2F934404 | 3 | G | 44C | 131F10 | 1E960 | 1000 |
| surface_aux_1 | 1000 | FF900000 | 01090000 | 0 | C7B998E8 | 1 | 208FB18C | 3 | G | 44C | 1316B8 | 14AFC | 1000 |
| surface_aux_2 | 1000 | 9D600000 | 01090000 | 0 | 26F6CE90 | 1 | 2710B000 | 3 | G | 44C | 549F0 | 1F60 | 1000 |

Common to all 29: 0x140 = 5442, 0x158 = 0, 0x164–0x16C = 0, 0x174 = 0x70 (112),
0x190 = 0x68, 0x194 = 0x20, 0x198 = 8. 0x170 = 0x28000000 in `general`, 0 elsewhere.

- Every file length is a multiple of 1 MiB except `local_English` (0x719DDFD0). **C**
- The largest files are 0xFF900000 bytes (4 GiB − 7 MiB). **C**

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
| Record (first extent) | entry +8 | entry +24 ([§5.2](#s5-2)) |
| Record fragment | fragment table pair `.offset` | pair `.length` |
| Free-space table | header 0x154 | header 0x1A0 |
| Free extent | free table pair `.offset` | pair `.size` |

<a id="s4-1"></a>
### 4.1 Allocation header (8 bytes)

Directory nodes, record first extents, and the free-space table each begin with an
8-byte header. Fragments and free extents do **not** (§4.2, §4.5).

| Offset | Size | Type | Meaning (modern layout) | Mark |
|---|---|---|---|---|
| +0 | 4 | u32 | `fragCount`: number of extra fragments that continue this allocation's payload | C |
| +4 | 4 | u32 | 0 in the modern layout. Non-zero selects the legacy chained layout ([§4.3](#s4-3)). | C (always 0 observed) |

Observed: all 113,118 directory nodes and the 29 free-space tables have `{0, 0}`. All
3,838,957 record first extents have word +4 = 0. **C**

<a id="s4-2"></a>
### 4.2 Modern layout: first extent plus fragment table

Let `O` be the allocation offset, `A` its size (from the table above), `F` = `fragCount`, and
`L` the logical payload length (for records: entry +12).

Layout of the first extent:

| Range | Content |
|---|---|
| `O … O+8` | allocation header `{F, 0}` |
| `O+8 … O+A−8F` | first payload bytes, capacity `A − 8 − 8F` |
| `O+A−8F … O+A` | fragment table: `F` pairs of `{u32 length, u32 offset}`, in payload order |

Each fragment is a raw byte range `[offset, offset+length)` that continues the payload. It
has no header of its own. **C**

Plain-words algorithm to reassemble a payload of length `L`:

1. Read the 8-byte header at `O`. If word +4 is not 0, go to [§4.3](#s4-3).
2. Let `F` = word +0. Check that `8 + 8F ≤ A` (reject otherwise).
3. Take `min(A − 8 − 8F, L)` bytes starting at `O+8`. These are the first payload bytes.
4. If more is needed, read the `F` pairs from the last `8F` bytes of the first extent
   (pair 0 first). For each pair in order, append `min(length, bytes still needed)` bytes
   from `offset`. Stop when `L` bytes are collected.
5. If all pairs are used and fewer than `L` bytes were collected, the record is corrupt.

Observations (**C**):
- The pair order was proven by fully inflating every compressed record with `F ≥ 2`
  (13 records in `mesh`, up to F = 19). Each yielded exactly the stated inflated length.
- The first extent's capacity plus all fragment lengths is ≥ `L`. The surplus is usually
  0–3 bytes; it is larger when a reused extent was bigger than needed. Fragment lengths
  are multiples of 4 and ≥ the minimum allocation size.
- `F` distribution over the whole install: 0 for all but 282 records. `general` 5
  (all F = 1), `local_English` 261 (F from 1 to 31; 22 records have F = 31), `mesh` 15
  (F up to 19), `sound_aux_1` 1. No other archive has fragmented records. Directory nodes
  are never fragmented.
- When `F > 0`, the entry's allocated size (+24) is the size of the **first extent only**
  and is smaller than `L + 8`.

> **OPEN:** Is 31 a hard maximum for `F`? A reader should accept any `F` with
> `8 + 8F ≤ A`, but may warn above 31.

<a id="s4-3"></a>
### 4.3 Legacy chained layout (Reported only)

Sources describe an older layout. It is selected when header word +4 ≠ 0, or when header
0x174 = 111. **R**
- Word +0 is the size of the next block and word +4 its offset.
- Each block contributes its size − 8 payload bytes.
- The chain is stored **tail-first**: the blocks fill the payload buffer from the end
  backwards, and the block whose next-size is 0 supplies the start of the buffer.
- One source caps the walk at 1000 steps.

**Not observed:** 0 of 3,838,957 records and 0 of 113,118 nodes in this install use it, and
0x174 is 112 everywhere. **C** (absence).

> **OPEN:** The exact byte accounting of the legacy layout cannot be confirmed against
> data. Implementers should treat it as an optional, feature-gated path with a step limit,
> and report "unsupported legacy layout" by default.

<a id="s4-4"></a>
### 4.4 Allocation sizing rule

Header 0x144 is the **minimum allocation size**. It is not a fixed block size: allocations are
not multiples of it, and offsets are aligned only to 4. **C** Fresh allocations follow:

> allocated size = max(0x144 value, (payload length + 8) rounded up to a multiple of 4)

Evidence (**C**):
- Unfragmented record entries: 3,790,044 match the rule exactly. 48,631 are larger than
  the rule gives ("slack", a reused larger extent). None are smaller. The other 282 are
  fragmented.
- Directory nodes are 2452 + 8 = 2460 bytes, or 0x144 when that is larger. Some nodes are
  larger still (reused extents). In `general`, 29 of 8,007 nodes are 2464–3468 bytes.
- All record offsets are multiples of 4.

Readers must not depend on the rule. It only explains the data. They must always use the
size given by the pointer or entry.

<a id="s4-5"></a>
### 4.5 Free-space table

| Item | Location | Content |
|---|---|---|
| Table allocation | offset = header 0x154, size = header 0x1A0 | 8-byte header `{0, 0}`, then header 0x15C pairs `{u32 size, u32 offset}` |
| Free extents | each pair | raw stale bytes (no header, no chain pointer) |

Observed (**C**, all 29 archives):
- Table header is `{0, 0}`. The word after the last pair is 0.
- Pairs are sorted by ascending offset. They are not sorted by size.
- Every free extent's size is ≥ the minimum allocation size.
- Header 0x19C equals the sum over pairs of (size − 8).
- Free extents contain stale data. Their first 8 bytes are **not** a chain link.
- The last free extent often, but not always, ends at end-of-file. It is the growth reserve.

Correction: one source describes the free list as a chain of extents, each starting with
`{nextSize, nextOffset | 1}`. That does not match this install. **C** (corrects DO). That
model may apply to older versions. **R**

> **OPEN:** The capacity of the free table (its size 0x1A0 versus the number of pairs it
> holds), and what the writer does when it fills, are unknown. A reader needs only the
> first 0x15C pairs and must check `8 + 8 × count ≤ size`.

<a id="s4-6"></a>
### 4.6 Tiling

In every observed archive, the union of the following covers `[0x400, file length)`
**exactly**, with no overlaps and no gaps:
- all directory node allocations (pointer sizes),
- all record first extents (entry +8, entry +24),
- all record fragments,
- the free-space table (0x154, 0x1A0),
- all free extents.

**C** (29/29 archives). This is the strongest available consistency check ([§10](#s10),
I-20).

---

<a id="s5"></a>
## 5. Directory B-tree

<a id="s5-1"></a>
### 5.1 Node layout

A node is an allocation (normally 2460 bytes, see §4.4). After its 8-byte allocation header
comes a fixed **2452-byte payload**. Offsets below are relative to the payload start
(allocation offset + 8).

| Offset | Size | Type | Meaning | Mark |
|---|---|---|---|---|
| 0 | 496 | 62 × {u32 size, u32 offset} | Child pointers. `size` is the child node's allocation size; `offset` is its absolute offset. | C |
| 496 | 4 | u32 | `n`: number of valid entries, 0 < n ≤ 61 (root of a non-empty archive: n ≥ 1) | C |
| 500 | 1952 | 61 × 32-byte entry | Entries; only the first `n` are valid ([§5.2](#s5-2)) | C |

Total payload 496 + 4 + 1952 = 2452. **C**

Rules:
- A node is a **leaf** if child pointer 0 is `{0, 0}`. In a leaf, pointers 0…n are all
  `{0, 0}`. **C**
- In an internal node, pointers 0…n are all non-zero (both size and offset). **C**
- Pointers n+1…61 and entries n…60 are **stale**: they may hold old non-zero data
  (2,801 nodes in the install have non-zero stale pointers). Readers must ignore them. **C**

<a id="s5-2"></a>
### 5.2 Directory entry (32 bytes)

Field names below are this spec's own.

| Offset | Size | Type | Name | Meaning | Mark |
|---|---|---|---|---|---|
| +0 | 2 | u16 | `flags` | Storage flags ([§5.3](#s5-3)) | C |
| +2 | 2 | u16 | `stamp` | Small version number of the record. Not used for reading (see OPEN below). | C (values), R/inferred (meaning) |
| +4 | 4 | u32 | `did` | Data identifier; the B-tree key | C |
| +8 | 4 | u32 | `offset` | Absolute offset of the record's first extent (its allocation header) | C |
| +12 | 4 | u32 | `length` | Stored payload length in bytes. For compressed records this includes the 4-byte inflated-length prefix. | C |
| +16 | 4 | u32 | `time` | Unix time (seconds, UTC) of the last write | C (range 2007-01-10 … 2026-10-03 matches release history and the patch date) |
| +20 | 4 | u32 | `iteration` | Per-record revision counter, increased by patches | C (values 0 … 106,600 across the install), R (meaning) |
| +24 | 4 | u32 | `allocSize` | Size of the first extent allocation ([§4](#s4)) | C |
| +28 | 4 | u32 | reserved | Always 0 | C (0 in all 3,838,957 entries) |

Observations on `stamp` (**C**): across the whole install it takes exactly these values: 1, 2, 7, 8,
11, 12, 14, 20, 21, 22, 27, 28, 29, 31, 32, 33, 38, 39. The earliest `time` per value rises with the value. In `general`, the
earliest `time` for 7 is 2007-01, for 20 it is 2008-10, for 38 it is 2012-10, and for 39 it
is 2018-10. Metadata records (§7) use 1 or 2.

> **OPEN:** Meaning of `stamp`. It behaves like "data-format generation when the record was
> written". The container does not need it to read data. It may be needed by record decoders.

<a id="s5-3"></a>
### 5.3 Flags

| Bit | Meaning | Mark |
|---|---|---|
| 0 | 1 = payload is zlib-compressed ([§6.1](#s6-1)) | C |
| 1 | Always set in observed data | C (value), meaning unknown |
| 2–15 | Always 0 in observed data | C |

Observed values: only **2** (stored) and **3** (compressed). **C** Sources report 0 and 1 as
possible in other versions or in sibling games, and one tool rejects values > 3. **R**

Compression by archive (**C**):

| Archive | Compressed (flags 3) | Stored (flags 2) |
|---|---|---|
| general, gamelogic, local_English, sound, sound_aux_1, map_* | 0 | all |
| anim | 22,245 | 3,155 |
| cell_1 … cell_5, cell_14 | 97.7–99.8 % | rest |
| mesh, mesh_aux_1 | 37,402 / 10,052 | 87 / 29 |
| surface, surface_aux_1, surface_aux_2 | 327,614 / 469,884 / 274,583 | 116,085 / 75,738 / 65,955 |
| highres, aux_1, aux_2, aux_3 | 13,148 / 15,446 / 37,785 / 7,016 | 2,228 / 1,442 / 774 / 155 |

The same DID type can be stored either way within one archive. A reader must honour the
per-entry flag and must not assume a per-type or per-archive rule. **C**

> **OPEN:** The meaning of bit 1.

<a id="s5-4"></a>
### 5.4 Tree shape (observed)

- Keys (DIDs) are unique within an archive. An in-order traversal yields strictly
  ascending DIDs. **C** (29/29)
- Every non-root node holds between **30 and 61** entries. **C**
- The root holds 1 … 58 entries (1 in most cell archives, 4 in the metadata-only `cell_15`/`map_15`). **C**
- All leaves are at the same depth. The maximum depth is 3 (root at depth 0), so there are
  at most 4 levels. **C**
- Total observed: 113,118 nodes, 3,838,957 entries.

<a id="s5-5"></a>
### 5.5 Lookup by DID

1. Start at the root node (header 0x160 / 0x1A4).
2. Binary-search (or linearly scan) the node's `n` valid entries for the DID.
3. If found, return that entry. Internal nodes hold real entries too, so a match can occur
   above leaf level.
4. If not found, let `i` be the number of entries whose DID is less than the target. If
   the node is a leaf, the DID is absent from this archive. Otherwise descend into child
   pointer `i` and repeat from step 2.
5. Bound the descent depth (e.g. 16 levels) and fail with a corruption error if it is
   exceeded.

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

Observed (**C**):
- Zlib header bytes are `78 DA` or `78 9C`. Both occur within one archive.
- Every compressed record that was fully inflated produced exactly the prefixed length,
  ended its stream (with a valid Adler-32 trailer), and left **no trailing bytes**. Records
  fully inflated: all compressed records of `cell_14` (69,195), `anim` (22,245),
  `mesh_aux_1` (10,052), `highres_aux_3` (7,016), and every multi-fragment compressed
  record of `mesh`. For all other compressed records the first 64 inflated bytes were
  checked.
- Largest inflated length: 17,984,776 bytes (`mesh`). Highest compression ratio:
  about 975:1 (`surface`).

A reader must cap the inflated length (configurable, with a default comfortably above
32 MiB) and must allow ratios of at least 1000:1. It must reject a stream that produces more
or fewer bytes than the prefix states.

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

Sources say the u32 after the echoed DID is a "class / type index". **R** In observed data that
dword is a small integer for some types (e.g. 0x04, 0x20, 0x2A, 0x30), and for others it
looks like a structured value. It belongs to the record-format specs, not to this one.

> **OPEN:** The meaning of the leading u32 for the "+4" types (it is 0 for 0x02 and 0x1A and
> 0x00000201 for 0x1F, for example). This is for the record-format specs.

---

<a id="s7"></a>
## 7. Reserved metadata records (DID 0xFFFF00xx)

<a id="s7-1"></a>
### 7.1 Presence

Every archive contains a few DIDs whose high 16 bits are 0xFFFF. They sort last in the
tree. All are stored uncompressed with `flags` = 2, `stamp` = 1 or 2, and `iteration` = 0. **C**

| DID | Present in | Length | Observed shape | Mark |
|---|---|---|---|---|
| 0xFFFF0001 | all 29 | 148–156 in base archives; 8 (all zero) in the older aux parts; 288 (`highres_aux_3`) and 3196 (`mesh_aux_1`) in the two newest aux parts | u32 values, including a value and its negation | C (shape), meaning OPEN |
| 0xFFFF0003 | all but the 2 newest aux parts | 37 | 32 bytes (two 16-byte identifiers) + 1-byte length 4 + 4 ASCII digits | C (shape), meaning OPEN |
| 0xFFFF0004 | the 10 archives with 0x18C = 1100 that are not aux parts | 12 | u32 N, i32 −N, u32 1 | C (shape), meaning OPEN |
| 0xFFFF0005 | all 29 | 24 | set membership ([§7.2](#s7-2)) | C |
| 0xFFFF0008 | all 22 base archives, no aux part | 16 | 16-byte identifier, the same in every archive | C (shape), meaning OPEN |

These records are archive metadata, not game content. A traversal that lists game records
should be able to filter `did ≥ 0xFFFF0000`.

<a id="s7-2"></a>
### 7.2 Set membership record (0xFFFF0005)

| Offset | Size | Type | Meaning | Mark |
|---|---|---|---|---|
| 0 | 16 | bytes | Set identifier, identical in all parts of one kind and different between kinds | C |
| 16 | 4 | u32 | Part index: 0 for the `.dat` base, N for `_aux_N.datx` | C |
| 20 | 4 | u32 | Number of parts in the set | C |

Observed counts: mesh 2, sound 2, surface 3, highres 4, every other archive 1. Each cell and
map archive has its own identifier (count 1). **C**

> **OPEN:** Meanings of 0xFFFF0001, 0003, 0004 and 0008. A historical sibling engine kept a
> version record at 0xFFFF0001. **R**

---

<a id="s8"></a>
## 8. DID → archive routing

<a id="s8-1"></a>
### 8.1 Observed placement (2026-10-03)

High byte of DID → archive(s) holding records with that high byte, with record counts.
(0xFFFF metadata is excluded.) **C**

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
| 06 | mesh, mesh_aux_1 |
| 0A | sound (12,613), sound_aux_1 (2,531), local (29,193) |
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
  those classes.
- Every gamelogic DID 0x70xxxxxx has a companion 0x79xxxxxx = DID + 0x09000000 (383,332
  pairs, exact one-to-one match). Sources report this offset as a fixed property-record
  relationship. **R** (meaning), **C** (match).
- Within a multi-part set, DID ranges of the parts **interleave**: each part spans almost
  the whole DID range of its type. A DID cannot be routed to a part by range. Each part
  must be searched.

<a id="s8-2"></a>
### 8.2 Reported routing order (LC)

LC tries archive kinds in a fixed order per high byte. **R**

| DID | Order |
|---|---|
| 0x01, 0x04, 0x0E000004–0x0E4FFFFF, 0x0F, 0x18, 0x1F, 0x20, 0x23, 0x28, 0x2A, 0x2B, 0x30, 0x31, 0x40 | general |
| 0x06 | mesh |
| 0x07, 0x34000000, 0x47, 0x56, 0x70–0x77 | gamelogic |
| 0x0A | sound, sound_aux_1, local |
| 0x22 | general, local |
| 0x25–0x26 | local |
| 0x41 | highres, highres_aux_1…3, surface, surface_aux_1…2, local |
| 0x78–0x7F | gamelogic, local |
| 0x80RRxxxx | cell/map by region as in §8.1 |

The LC table is incomplete compared with §8.1 (for example 0x02, 0x05, 0x0C, 0x1A, 0x39,
0x42 and 0x66 are missing).

<a id="s8-3"></a>
### 8.3 Routing rule for implementers

- No DID occurs in more than one archive of the observed install. Every pair of archives
  was intersected, with 0xFFFF metadata excluded. **C** So the order of the search affects
  only speed, not the result.
- The recommended rule: a routing table maps the high byte (plus the region nibble for
  0x27/0x80) to an ordered list of archive kinds. The lookup tries each part of each listed
  kind. If none has the DID, the reader may optionally fall back to searching every open
  archive. Implementers should keep the table as data (not code) so it can be updated
  without a code change, and should seed it from §8.1.
- If a future install does contain the same DID in two archives, prefer the copy with the
  higher `iteration`, then the later `time`, and log a warning. **R/inferred**

> **OPEN:** Whether the official client gives the `local` archive precedence over the
> general archives for shared high bytes (0x0A, 0x41, …). It cannot matter for this install,
> because no DID is duplicated.

---

<a id="s9"></a>
## 9. Limits

| Limit | Value | Mark |
|---|---|---|
| Offsets and sizes | u32, so an archive file is < 4 GiB | C (field widths), R (DO note about ">4 GB" requiring a format change) |
| Largest observed archive | 0xFF900000 bytes | C |
| Aux part creation | when a base reaches about 4 GiB − 7…8 MiB, new data goes to `_aux_N.datx` | C (sizes), inferred (policy) |
| Entries per node | 61 max; 30 min except root | C |
| Children per node | n + 1 ≤ 62 | C |
| Tree depth | ≤ 3 observed (4 levels) | C |
| Largest stored record | 40,916,068 bytes (`sound`) | C |
| Largest inflated record | 17,984,776 bytes | C |
| Fragments per record | ≤ 31 observed | C |
| File growth unit | 1 MiB (file lengths are multiples of 1 MiB except one) | C (lengths); R (KD: "grows by ~1 MiB") |

A reader must open archives read-only and must not lock them exclusively. The launcher may
rewrite them while the client is not running. **R** A reader must never write to the install
(project rule).

---

<a id="s10"></a>
## 10. Validator invariants

Every item held on all 29 archives on 2026-10-03 unless marked. "Error" means the archive is
unusable. "Warn" means it is readable but unexpected.

| ID | Check | Severity |
|---|---|---|
| I-1 | File length ≥ 0x400 and ≥ 0x1A8 | Error |
| I-2 | u32 at 0x140 = 0x00005442 | Error |
| I-3 | u32 at 0x148 = file length | Warn (possible concurrent patching) |
| I-4 | 0x144 > 0 | Error |
| I-5 | 0x174 = 112 (else warn; 111 means legacy layout, §4.3) | Warn |
| I-6 | 0x190/0x194/0x198 = 104/32/8 | Warn; Error if 0x194 ≠ 32 |
| I-7 | 0x100…0x13F is the empty journal pattern | Warn |
| I-8 | Root: 0x160 ≥ 0x400, 0x1A4 ≥ 2460, 0x160 + 0x1A4 ≤ length | Error |
| I-9 | Every node allocation header = `{0, 0}` | Error |
| I-10 | Every node has 1 ≤ n ≤ 61; non-root nodes have n ≥ 30 | Error (n out of 1…61); Warn (n < 30) |
| I-11 | Leaf iff child 0 is `{0,0}`; internal nodes have non-zero children 0…n; leaf children 0…n zero | Error |
| I-12 | Child pointers satisfy size ≥ 2460 and offset + size ≤ length | Error |
| I-13 | No node offset is visited twice; depth ≤ 16 | Error |
| I-14 | All leaves at the same depth | Warn |
| I-15 | In-order DIDs strictly ascending (implies unique) | Error |
| I-16 | Entry reserved (+28) = 0 | Warn |
| I-17 | Entry flags ∈ {2, 3} | Warn (Error if bits other than 0–1 set) |
| I-18 | Record first extent: offset ≥ 0x400, offset + allocSize ≤ length, header word +4 = 0, 8 + 8F ≤ allocSize; if F = 0 then allocSize ≥ length + 8 | Error |
| I-19 | Fragments in bounds; first capacity + Σ fragment lengths ≥ length | Error |
| I-20 | Tiling: nodes, record extents, fragments, the free table and free extents cover [0x400, length) exactly, with no overlap | Warn (Error on overlap) |
| I-21 | Free table header `{0,0}`; 8 + 8·count ≤ 0x1A0; pairs in bounds, sorted by offset; each size ≥ 0x144 | Warn |
| I-22 | 0x19C = Σ (free size − 8) | Warn |
| I-23 | Compressed record: inflated length = prefix, stream ends exactly at `length` | Error (for that record) |
| I-24 | 0xFFFF0005 exists; its part index matches the file name (0 for `.dat`, N for `_aux_N`) and index < count | Warn |
| I-25 | Across the open set, no DID (< 0xFFFF0000) appears in two archives | Warn |

---

<a id="s11"></a>
## 11. Observed statistics (2026-10-03)

<a id="s11-1"></a>
### 11.1 Full walks of `client_general.dat` and `client_local_English.dat`

Every payload was reassembled in full. Neither archive contains compressed records.

| Metric | general | local_English |
|---|---|---|
| Min allocation (0x144) | 1024 | 256 |
| Entries | 261,483 | 311,929 |
| Nodes (per depth 0/1/2/3) | 8,007 (1/5/210/7,791) | 8,722 (1/6/228/8,487) |
| Root entry count | 4 | 5 |
| Non-root entries per node | 30 … 61 | 30 … 61 |
| Nodes with allocation ≠ 2460 | 29 | 0 |
| Strictly sorted / unique | yes / yes | yes / yes |
| Flags distribution | 2: 261,483 | 2: 311,929 |
| `fragCount` distribution | 0: 261,478; 1: 5 | 0: 311,668; 1: 40; 2: 22; 3: 18; 4: 15; 5: 15; 6: 13; 7: 6; 8: 4; 9–30: 106; 31: 22 |
| Legacy chained headers | 0 | 0 |
| Payload begins with own DID (+0) | 199,731 | 311,923 |
| DID at +4 instead | 33,505 (types 02, 11, 15, 1A, 1F) | 0 |
| No echo | 28,247 (types 08, 39, 44, 66, part of 0E, FF) | 6 (0x58000000 and 5 × FF) |
| Free extents (0x15C) | 1 (0x83200 bytes, at end of file) | 1 (0xB9664 bytes, at end of file) |
| Tiling gaps / overlaps | 0 / 0 | 0 / 0 |
| `time` range | 2007-01-10 … 2026-10-03 | 2007-01-10 … 2026-10-03 |
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
| anim | 25,400 | 700 | 2 | 22,245 | 0 | 05 |
| cell_1 | 94,877 | 2,829 | 3 | 93,323 | 0 | 27, 80 |
| cell_2 | 98,921 | 2,922 | 3 | 97,503 | 0 | 27, 80 |
| cell_3 | 88,015 | 2,630 | 3 | 86,484 | 0 | 27, 80 |
| cell_4 | 96,370 | 2,687 | 3 | 94,139 | 0 | 27, 80 |
| cell_5 | 86,381 | 2,543 | 3 | 85,139 | 0 | 27, 80 |
| cell_14 | 69,363 | 2,215 | 3 | 69,195 | 0 | 27, 80 |
| cell_15 | 4 | 1 | 0 | 0 | 0 | (FF only) |
| gamelogic | 845,460 | 22,164 | 3 | 0 | 0 | 07, 0C, 34, 43, 47, 56, 70, 76, 78, 79 |
| general | 261,483 | 8,007 | 3 | 0 | 5 | see §11.1 |
| highres | 15,376 | 447 | 2 | 13,148 | 0 | 41 |
| highres_aux_1 | 16,888 | 482 | 2 | 15,446 | 0 | 41 |
| highres_aux_2 | 38,559 | 1,051 | 2 | 37,785 | 0 | 41 |
| highres_aux_3 | 7,171 | 228 | 2 | 7,016 | 0 | 41 |
| local_English | 311,929 | 8,722 | 3 | 0 | 261 | see §11.1 |
| map_1 … map_5, map_14 | 65,029–65,030 each | 2,036–2,167 | 2–3 | 0 | 0 | 80 |
| map_15 | 4 | 1 | 0 | 0 | 0 | (FF only) |
| mesh | 37,489 | 1,049 | 2 | 37,402 | 15 | 06 |
| mesh_aux_1 | 10,081 | 288 | 2 | 10,052 | 0 | 06 |
| sound | 12,618 | 414 | 2 | 0 | 0 | 0A |
| sound_aux_1 | 2,534 | 81 | 2 | 0 | 1 | 0A |
| surface | 443,699 | 13,896 | 3 | 327,614 | 0 | 41 |
| surface_aux_1 | 545,622 | 17,192 | 3 | 469,884 | 0 | 41 |
| surface_aux_2 | 340,538 | 9,908 | 3 | 274,583 | 0 | 41 |

Totals: 3,838,957 entries, 113,118 nodes. Zero errors across invariants I-1…I-25 (I-23
was checked in full only for the archives listed in §6.1).

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
4. **Free space** is a sorted table of `{size, offset}` pairs held in one allocation
   (0x154 / 0x1A0, count at 0x15C). It is not a linked chain. 0x19C is the total free
   payload. 0x158 is unused (0).
5. **0x14C** is not a single version number. Byte 0x14F is the dataset type and byte 0x14E
   the archive kind. 0x150 is the region (cell/map) or 2 (local).
6. **Node allocation size** is not always 2460. It equals 0x144 when that is larger, and it
   can be larger still for reused extents. The payload is always 2452.
7. **Compression** is not used at all in `general`, `gamelogic`, `local_English`, `sound*`
   or `map_*` in this install. Only flag values 2 and 3 occur.
8. **DID echo** is not universal. Six types put the DID at +4, and about ten types have no
   echo (§6.2).
9. **0x190/0x194/0x198** (104/32/8) and the **0xFFFF0005 set-membership record** are new
   findings.

---

<a id="s13"></a>
## 13. Change log

| Date | Change |
|---|---|
| 2026-10-03 | First version, written from research notes and confirmed or corrected by full B-tree walks of all 29 archives in the user's install. |

---

<a id="s14"></a>
## 14. Implementer notes and OPEN items (`crates/dat`, 2026-10-04)

Choices the clean-room implementation had to make where this spec is silent. Each is
conservative; spec writers should confirm or correct them.

> **OPEN:** Legacy chained layout ([§4.3](#s4-3)). Implemented behind the runtime switch
> `ArchiveOptions::legacy_layout` (off by default; such allocations fail with
> "unsupported legacy layout"). The implementation selects the legacy path **only** when
> allocation header word +4 ≠ 0. Header 0x174 = 111 alone does not switch, because an
> allocation whose header is `{0, 0}` reads identically in both models. When the final block
> (next-size 0) holds more data bytes than are still needed, the implementation takes the
> **first** `remaining` data bytes of that block. Neither choice is confirmed by data.

> **OPEN:** Lower bound for fragment and block offsets. The reader rejects any fragment,
> legacy block, record extent or node whose range starts below 0x400 or ends past the file
> length. I-19 only says "in bounds"; please confirm that 0x400 is the intended lower bound.

> **OPEN:** A stored length (entry +12) larger than the file length is rejected before any
> allocation is made, since non-overlapping extents cannot supply it. Configurable caps:
> stored records 256 MiB, inflated records 128 MiB (defaults).

> **OPEN:** Empty root. §5.1 says 0 < n for the root of a non-empty archive. The reader
> accepts a leaf root with n = 0 as an empty archive (validator: I-10 warning) and rejects
> an interior node with n = 0. Can an archive with no records (not even §7 metadata) exist?

> **OPEN:** Fragmented directory nodes. Never observed ([§4.2](#s4-2)). The reader would
> reassemble a node payload through the generic modern-layout path; the validator reports a
> non-`{0, 0}` node header as I-9 error regardless.

> **OPEN:** Duplicate DIDs across archives ([§8.3](#s8-3)). `ArchiveSet::locate` returns the
> first copy in routing order and does not compare `iteration`/`time` (that would require
> searching every candidate on every lookup). Duplicates are reported by the set validator
> (I-25). Revisit if a real install ever shows duplicates.

> **OPEN:** Routing order for shared high bytes. The default table (kept as text in
> `crates/dat/src/routing.rs`) puts the archive with the most records of a high byte first,
> e.g. `surface` before `highres` for 0x41 and `local` before `general` for 0x22 and 0x42.
> That differs from the LC order in [§8.2](#s8-2). Order affects speed only while no DID
> is duplicated. Metadata DIDs (0xFFFF00xx) have no rule: they exist in every archive and
> must be read per archive, not through the set.

> **OPEN:** Leaf strictness. The reader refuses a leaf whose child pointers 1…n are not all
> zero (I-11) instead of ignoring them. Stale pointers beyond n are ignored, as specified.
