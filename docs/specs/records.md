<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Record encodings inside subfile payloads

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | 2026-10-05: **passed**; re-review 2026-10-05: **passed**, unchanged. Full record moved to the review log (decision 0018). |
| **Companion spec** | `docs/specs/dat-container.md` (archive header, block/extent layer, B-tree directory, zlib, DID routing). This spec starts where that one ends: with a fully reassembled and, if needed, decompressed subfile payload. |

## Purpose

This spec gives an implementer of the `formats` crate what they need to decode these records from the bytes of a subfile payload:

- the shared primitive encodings (§1);
- what the high byte of a DID means, the DB object type the client registers for each prefix, and what each record starts with (§1.6, §2);
- localized string tables, DID prefix 0x25 (§3), and the StringInfo value that points into them (§4);
- render surfaces (textures and icons), DID prefix 0x41 (§5);
- the master property dictionary, DID 0x34000000 (§6, now a pointer to `property-system.md`);
- enum mappers, prefix 0x23 (§7), and DID maps, prefix 0x28 (§8);
- sniffing the audio inside sound records, prefix 0x0A (§9).

§10 lists every place where the research notes and the observed data disagree. §11 collects the open questions.

## Provenance

How sources are marked:

- **Confirmed**: the layout was decoded on the user's own install on 2026-10-03, using a throwaway probe in the dirty-room scratchpad. A Confirmed fact comes with what was decoded and how much of it.
- **Reported**: the fact comes only from published third-party material and was not, or could not be, checked against data.

Source abbreviations:

| Key | Source |
|---|---|
| OBS | Observed in user install 2026-10-03. Steam install, client data current on that date. Every 0x25, 0x23 and 0x28 record and the 0x34000000 record were decoded in full. All 1,408,052 headers of 0x41 records were read, and 6 surfaces were rendered and inspected by eye. Every 0x0A record had its header and trailer read. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.0, Apache-2.0 per its distribution's LICENSE. Studied as a binary: decompiled to recover facts only. |
| JT | jtauber/lotro Python scripts. No licence; facts only. |
| KD | koniecdev/LotroKoniecDev knowledge base and models. Facts only. |
| EP | Endevir LOTRO-Enchanced-text-patcher. Facts only. |
| MER | Middle-earth-Revenge master-property-viewer. Facts only. |
| DU | Middle-earth-Revenge DATUnpacker. Facts only. |
| D3D | Public Direct3D 9 `D3DFORMAT` enumeration and the public S3TC/DXTn block-compression description. These are general references. |
| OBS6 | Observed in user install 2026-10-06 (re-check for D3): all **11** prefix-0x08 records, only in `client_general.dat`, read with the header of [`walker.md`](walker.md) §11.5.1. Each record's size is the one §11.5.3 gives for its controller type (161, 89, 36, 94 bytes for types 1–4); the first u32 is a 0x47 DID (0x47000002 or 0x47001A7C). |
| BIN | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (SHA-256 prefix d22363eb9aff0fb5), read in the dirty room on 2026-10-05. **Question:** which DB object type the client registers for each DID range (a start-up table pairing a DID range with a type code and a cache). **Why M1–M3 were not enough:** the data pairs only some type codes with prefixes (`property-system.md` §9 lists the codes that stored references exercise); no published source has the full table; M3 cannot show it. Only offline start-up code was read. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| P1 | Little-endian fixed-width integers and IEEE-754 floats | LC, JT; OBS | Confirmed |
| P2 | VLE layout, all three forms, including byte order of the 4-byte form | LC; OBS (1-, 2- and 4-byte forms all seen) | Confirmed. The 0xE0 escape form is Reported only. |
| P3 | VLE values are always written in the shortest form | OBS (about 470k VLEs checked) | Confirmed |
| P4 | TSize = 1 hint byte + VLE count | LC, KD; OBS | Confirmed |
| P5 | The TSize hint byte indexes a table of hash-bucket counts, and elements are stored in bucket order | OBS. Derived by this spec writer; no source states it. | Confirmed for indices 0–12 |
| P6 | Pascal string (VLE length, Latin-1) | LC; OBS | Confirmed |
| P7 | Prefixed UTF-16 string (VLE count of UTF-16 code units, then UTF-16LE) | LC, KD; OBS | Confirmed |
| P8 | u8-length ASCII string | LC; OBS (0x28 labels) | Confirmed |
| P9 | Name hash used for string-table argument IDs | LC (hash routine); OBS (argument IDs match the hash of the argument-name strings stored beside them; 11,280 of 16,212 sampled IDs match a published name list) | Confirmed |
| P10 | The record types of this spec begin with their own DID; other types differ (§1.6) | LC, JT, DU; OBS (every record decoded; prologue census of 8 archives, 2026-10-05) | Confirmed. Corrected 2026-10-05: the earlier claim "records begin with their own DID" holds for most but not all prefixes. |
| D1 | DID high-byte type table (§2) | LC routing, research notes; OBS (per-archive prefix census of all archives) | Mixed. Per row in §2. |
| D2 | DB object type registered for each DID range (§2.1, §2.2) | BIN (the client's start-up table of DID ranges); OBS (labels from the DBOType enum mapper; every decoded prefix agrees with its type) | BIN |
| D3 | Prefix 0x08 (camera states) is decoded in [`walker.md`](walker.md) §11.5; its prologue is the camera entity's 0x47 DID, not its own DID (§1.6, §2.1) | `walker.md` W9, W25, W-I1, W-I2; OBS6 | Confirmed (layout owned by `walker.md`) |
| S1 | String-table layout (§3) | LC, KD, EP; OBS (282,152 tables, 806,052 entries, zero leftover bytes) | Confirmed |
| S2 | StringInfo layout (§4) | LC; OBS (6,352 enum display values and 626 property-definition defaults) | Confirmed |
| R1 | 0x41 surface header (§5) | LC, JT, DU; OBS (all 1,408,052 headers) | Confirmed |
| R2 | Format codes 20, 21, 28, 500, DXT1, DXT3, DXT5 | LC, JT, D3D; OBS | Confirmed. These are the only codes present. |
| R3 | Pixel byte order and row order of 20 and 21; DXT blocks are standard S3TC | JT, D3D; OBS (rendered, colours and orientation checked by eye) | Confirmed |
| R4 | Embedded JPEG has width = height = 0 in the header | OBS | Confirmed (all 65,706) |
| M1 | 0x34000000 layout (moved to `property-system.md` §2) | LC, MER; OBS (2,534,594 bytes decoded to the last byte) | Confirmed |
| M2 | Property type codes 1–22 and their value encodings | LC; OBS (all 22 codes present; every default/min/max value decoded) | Confirmed; see `property-system.md` §3–§4 |
| E1 | EnumMapper layout (§7) | LC; OBS (735 records, zero leftover bytes) | Confirmed |
| E2 | DataIdMap layout (§8) | LC; OBS (24 records, zero leftover bytes) | Confirmed |
| A1 | Sound record envelope: DID, size, audio, u32, f32 | LC; OBS (all 44,337 records) | Confirmed |
| A2 | Audio payload magics: RIFF/WAVE, OggS, RIFF/DMAP | LC (RIFF and OggS); OBS (DMAP found by this spec writer) | Confirmed |

---

<a id="s1"></a>
## 1. Primitive encodings

All multi-byte values are **little-endian**. This spec writes integers as `u8`, `u16`, `u32`, `u64` (unsigned) and `i32` (signed two's complement), and floats as `f32`/`f64` (IEEE-754 binary32/binary64). Offsets are in bytes from the start of the decompressed payload. Every field is packed: there is no alignment padding unless a layout says so.

<a id="s1-1"></a>
### 1.1 Fixed-width values (Confirmed)

| Name | Size | Meaning |
|---|---|---|
| `u8`/`u16`/`u32`/`u64` | 1/2/4/8 | Unsigned, little-endian |
| `i32` | 4 | Signed, little-endian |
| `f32`/`f64` | 4/8 | IEEE-754, little-endian byte order |
| `bool8` | 1 | Boolean. Only 0 and 1 are valid. Readers report any other value as malformed. |
| `DID` | 4 | `u32` data ID (§2) |

<a id="s1-2"></a>
### 1.2 VLE: variable-length unsigned integer

A VLE is 1, 2, 4 or 5 bytes. The first byte `a` selects the form:

| First byte `a` | Total size | Value |
|---|---|---|
| `0x00`–`0x7F` | 1 | `a` |
| `0x80`–`0xBF` | 2 | `(a & 0x3F) << 8` OR the second byte. Range 0–0x3FFF. |
| `0xE0` exactly | 5 | The next 4 bytes as a little-endian `u32` |
| `0xC0`–`0xDF`, `0xE1`–`0xFF` | 4 | `(a & 0x3F) << 24` OR `(second byte) << 16` OR (bytes 3 and 4 read as a little-endian `u16`). Range up to 0x3FFFFFFF. |

Notes:

- In the 4-byte form, the low 16 bits are stored **little-endian** after a big-endian-looking high part. Confirmed: the 4-byte VLE `C0 00 9B 6B` in 0x34000000 decodes to 27,547. Reading the u16 big-endian would give 39,787, and the record would then fail to parse.
- The 2-byte form keeps 6 value bits of `a` (bit 6 is 0 in that range), so `(a & 0x7F)` and `(a & 0x3F)` give the same result.
- A 4-byte-form first byte of `0xE0` would collide with the escape, so values whose top 6 bits are 0x20 cannot use the 4-byte form.

> **OPEN:** Whether the writer ever emits the `0xE0` escape, and for which values (presumably ≥ 0x20000000, or any value with bits 30–31 set), has not been seen in data. Readers must accept it; writers should use it only for values the 4-byte form cannot carry.

Invariants (Confirmed, about 470k VLEs in 0x25 and 0x23 records):

- **V1**: the shortest form is always used. No 2-byte VLE holds a value below 0x80, and no 4-byte VLE holds a value below 0x4000.
- **V2**: a reader that reaches end-of-buffer in the middle of a VLE reports truncation and does not guess.

<a id="s1-3"></a>
### 1.3 TSize: hashed-collection header

A **TSize** is the count prefix of a serialized hash table or hash set. It is 1 + VLE bytes:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 1 | u8 | **Bucket-count index** ("hint") |
| 1 | 1–5 | VLE | Element count *n* |

A reader needs only *n*. The hint describes how the writer's hash table was sized, and it fixes the order of the elements that follow.

**Bucket-count table** (Confirmed by this spec writer from element order, §1.3.1). Every value is prime:

| Hint | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Buckets *B* | 11 | 23 | 47 | 89 | 191 | 383 | 761 | 1531 | 3067 | 6143 | 12281 | 24571 | 49139 |

<a id="s1-3-1"></a>
#### 1.3.1 Element order (Confirmed)

Take the elements of a TSize collection in stored order. Their u32 keys, taken modulo *B*[hint], form a **non-decreasing** sequence. In other words, the elements are written bucket by bucket, in ascending bucket number. This held for:

- every observed string table, enum mapper and DID map;
- the name list of 0x34000000;
- its definition list.

One exception: an enum mapper whose hint is 10. Its order fits *B* = 12281 and does not fit 12289, which is why the table says 12281.

> **OPEN:** The order of elements that share a bucket (probably insertion order) has not been determined. This matters only for a byte-exact writer.

The hint is **not** a pure function of *n*:

- In string tables, the hint is the smallest index whose *B* ≥ *n*.
- In DID maps, a hint of 1 is used even for *n* = 0..9, and hint 4 (*B* = 191) appears with *n* = 314 and 315.
- In 0x34000000, *n* = 27,547 has hint 11 while *n* = 27,546 has hint 12.

A writer that wants a byte-exact round trip must keep the original hint. A writer producing new data may choose any hint ≥ the smallest one that fits, as long as it writes the elements in the matching bucket order.

> **OPEN:** Bucket counts for hint indices ≥ 13 have not been observed.

Invariant **T1**: hint ≤ 12 in all observed data. A reader that does not need ordering must not reject other hints. It may warn.

<a id="s1-4"></a>
### 1.4 Strings

| Name | Layout | Encoding | Mark |
|---|---|---|---|
| **Pascal string** | VLE byte length *L*, then *L* bytes | ISO-8859-1 (Latin-1). Each byte maps to the code point of the same value. Not NUL-terminated. | Confirmed (property names, enum labels) |
| **UTF-16 string** | VLE **code-unit** count *N*, then 2·*N* bytes | UTF-16LE. *N* counts 16-bit units, not bytes and not scalar values. Not NUL-terminated. | Confirmed (string tables, StringInfo literals) |
| **Short ASCII string** | `u8` length *L*, then *L* bytes | ASCII | Confirmed (DID-map labels) |

Notes on UTF-16 strings:

- They may contain unpaired surrogates. A lossless reader keeps them, for example as WTF-16 or as raw units.
- The longest observed string is 5,959 units.
- LC rejects lengths above 20,000. That is a reasonable defensive cap, but data does not require it.

Text inside string-table pieces stays as stored. Line breaks show up as the two characters backslash and `n`, and markup tags appear in the text. Interpreting them belongs to the UI layer and is not covered here.

> **OPEN:** The full escape and markup grammar inside string pieces (`\n`, `<rgb=…>`, `#1:{…}` plural and article selectors) is not specified yet.

<a id="s1-5"></a>
### 1.5 Name hash (Confirmed)

Some records refer to identifier names by a 28-bit hash. To compute it over the bytes of an ASCII name:

1. Start with a 32-bit accumulator *h* = 0.
2. For each byte *c*:
   1. Set *h* = (*h* shifted left 4) + *c*, keeping 32 bits.
   2. Let *g* = *h* AND 0xF0000000.
   3. Set *h* = *h* XOR (*g* shifted right 24).
3. The result is *h* AND 0x0FFFFFFF.

This is the classic ELF/PJW hash, except that the high nibble is not cleared inside the loop. The final mask takes care of it.

Invariant **H1**: every argument ID in a string table is < 0x10000000 (Confirmed). When a string-table entry carries argument names (§3.3), each name hashes to the matching argument ID (Confirmed on every such entry).

<a id="s1-6"></a>
### 1.6 Record prologue (Confirmed; corrected 2026-10-05)

Every record type **decoded in this spec** (0x0A, 0x23, 0x25, 0x28, 0x34, 0x41) starts with `u32` = its own DID. A mismatch between that value and the DID that was looked up means the record is corrupt or was routed to the wrong archive.

This is **not** true of every record type. The earlier statement "every record begins with its own DID" (P10) was wrong. Census of the first 8 bytes of every record of `client_general.dat`, `client_gamelogic.dat`, `client_anim.dat`, `client_mesh.dat`, `client_sound.dat`, `client_local_English.dat`, `client_cell_1.dat` and `client_map_1.dat` (2026-10-05; the same split as `dat-container.md` §6.2):

| Prologue | Prefixes | What precedes or replaces the DID |
|---|---|---|
| **D**: own DID at offset 0 | 0x01, 0x04, 0x06, 0x0A, 0x0F, 0x12, 0x14, 0x16–0x19, 0x1D, 0x20–0x23, 0x25, 0x27, 0x28, 0x2A, 0x2B, 0x2D, 0x30–0x35, 0x40, 0x41, 0x42, 0x51, 0x56, 0x70, 0x76, 0x78, 0x79, 0x80; most of 0x0E | nothing |
| **xD**: one u32, then own DID | 0x02 (u32 0, `scenery.md` §5.1), 0x11 (u32 0, `scenery.md` §4.3), 0x15 (u32 version 1, `environment.md` §5.1.1), 0x1A (u32 0, `environment.md` §5.2), 0x1F (u32 flag word, `setup.md` §5.1), 0x47 (u32 flag word, `setup.md` §4.1) | the u32 named |
| **–**: no own DID in the first 8 bytes | 0x05 (f32 clip duration first, `havok-data.md` §2.3), 0x07 (u32 table kind first, `script-table.md` §2.1), 0x0C (no prefix at all, `script-table.md`), 0x66 (VLE vertex count first, `collision.md` §4.1), 0x08 (camera entity's 0x47 DID first, [`walker.md`](walker.md) §11.5.1), 0x39, 0x43, 0x44, 0x58, 3 of 0x0E (compression dictionaries), 0xFF (archive metadata) | type-specific; layouts of 0x39, 0x43, 0x44, 0x58 OPEN |

A decoder therefore checks the DID only where its record's spec places it; the container layer must not require it (`dat-container.md` §6.2).

---

<a id="s2"></a>
## 2. DID high-byte type table

A DID is a `u32`. Its **high byte** (bits 24–31) is the record type, and the low 24 bits are a serial number. Landblock-family DIDs (high byte 0x80) carry more structure in byte 2 (§2.2).

The "Archive (count)" column lists the observed population in each archive of the user's install. Routing (which archive to search) is defined in `dat-container.md`. This table gives the **meaning**.

Status codes:

- **C**: record content was decoded in this spec, or in the spec named in parentheses.
- **BIN**: only the DB type is known (column "DB type"); the layout is not specified.
- **L**: the meaning is confirmed by a label in a DID map (§8) that points at DIDs of this prefix.
- **R**: Reported only.

<a id="s2-1"></a>
### 2.1 Table

The **DB type** column is the DB object type the client registers for the DID range (BIN:
the client's start-up table of DID ranges, read on 2026-10-05; see Provenance BIN). The code
is not the prefix (0x41 holds type 0x05, and prefixes 0x16 and 0x17 hold types 0x17 and
0x16). Labels are data: the DB-object-type enum mapper ("DBOType" in the EMAPPER DID map,
base 0x230000F0, `property-system.md` §9); they are quoted for interoperability. Where a range
is narrower than the whole prefix it is given. "Prologue" says what the record starts with
(§1.6): **D** own DID; **xD** one u32, then own DID; **–** no own DID in the first 8 bytes.

| Prefix | Archive (count, 2026-10-03) | DB type (BIN) | Meaning | Prologue | Status |
|---|---|---|---|---|---|
| 0x01 | general (1,057) | 0x52 CELLMESH | Cell geometry: physics mesh, cell volume, portals, wall polygons, embedded render mesh ([`collision.md`](collision.md) §5; **not** setups, [`setup.md`](setup.md) §2.2) | D | C (collision.md) |
| 0x02 | general (3,402) | 0x3C SCENE | Scene records: procedural placement nodes ([`scenery.md`](scenery.md) §5) | xD | C (scenery.md) |
| 0x04 | general (28,555) | 0x0B SETUP | Setups: embedded Havok skeleton tagfile, body with attachment lists, collision volumes, part list ([`setup.md`](setup.md) §6, S1/S6, 28,555 of 28,555 decoded). A DID-map label ending in "_Setup" points into this range. | D | C (setup.md), L |
| 0x05 | anim (25,395) | 0x0F DBANIMATOR | Animation clips ([`havok-data.md`](havok-data.md) §2.3) | – | C (havok-data.md) |
| 0x06 | mesh (37,484), mesh_aux_1 (10,079) | 0x38 RENDER_MESH | Meshes ([`mesh.md`](mesh.md)) | D | C (mesh.md) |
| 0x07 | gamelogic (7,526) | 0x2A SCRIPTTABLE | Script tables ([`script-table.md`](script-table.md) §2) | – | C (script-table.md) |
| 0x08 | general (11) | 0x0D CAMERASTATE | Camera states: header with the camera entity's 0x47 DID, clip distances and flags, then a controller body by type ([`walker.md`](walker.md) §11.5, 11 of 11 decoded) | – | C (walker.md) |
| 0x0A | sound (12,613), sound_aux_1 (2,531), local_English (29,193) | 0x40 SOUND | Audio records (§9). Localized voice-over is in local. | D | C |
| 0x0C | gamelogic (28,992) | 0x15 SCRIPTLET | Scriptlets ([`script-table.md`](script-table.md)) | – | C (script-table.md) |
| 0x0D | none | 0x08 MOVIE | Registered; no record in this install | | BIN |
| 0x0E | general (7), local (1) | by DID: 0x0E000002 0x1D SEGMENT_TABLE; 0x0E000004 0x19 PLACES_TABLE; 0x0E000006–0x0E00FFFF 0x44 MAPNOTE_DESC; 0x0E010000–0x0E01FFFF 0x01 COMPRESSION_DICTIONARY; 0x0E100001 0x18 FARVIEWENTITIES; 0x0E100003 0x3D STRING_TOKEN_MAPPER; 0x0E100009 0x58 FALLBACK_MAPPER_TABLE; 0x0E100010–0x0E100030 0x06 MONITOREDPROPERTIES; 0x0E200000–0x0E20001F 0x14 NAME_FILTER_TABLE; 0x0E400000–0x0E4FFFFF 0x2D QUEST_MAPNOTE_DESC; 0x0EBADA55 0x22 TABOO_TABLE | Singleton tables, e.g. place names (0x0E000004), map notes (0x0E000006), the compression dictionaries 0x0E010000–2 (label in DID map), a name-filter table 0x0E200000 (label) | D (4 of 7 in general, the local one); – (3) | R, L, BIN |
| 0x0F | general (7) | 0x4F REGION | Region descriptors ([`environment.md`](environment.md) §3; labelled per region in DID map 0x28000008) | D | C (environment.md), L |
| 0x11 | general (5) | 0x4A SCENE_DESC | Scene-choice tables, region slot 1 ([`scenery.md`](scenery.md) §4.3) | xD | C (scenery.md) |
| 0x12 | general (5) | 0x1C TERRAIN_DESC | Terrain-scene tables, slot 2 ([`scenery.md`](scenery.md) §4.2) | D | C (scenery.md) |
| 0x14 | general (5) | 0x13 ENCOUNTER_DESC | Encounter tables, slot 3; layout OPEN ([`environment.md`](environment.md) §4) | D | BIN |
| 0x15 | general (1) | 0x3F SKY_DESC | Sky description, slot 4 ([`environment.md`](environment.md) §5.1) | xD (u32 version 1) | C (environment.md) |
| 0x16 | general (5) | 0x17 WATER_DESC | Water tables, slot 5 ([`water.md`](water.md) §4) | D | C (water.md) |
| 0x17 | general (5) | 0x16 FOG_DESC | Fog-zone tables, slot 6 ([`water.md`](water.md) §5) | D | C (water.md) |
| 0x18 | general (5) | 0x21 PROPERTY_DESC | Block-map property descriptors, slot 7 ([`landblock.md`](landblock.md) §5.2) | D | C (landblock.md) |
| 0x19 | general (5) | 0x35 TERRAIN_TYPE_TABLE | Terrain type tables, slot 8 ([`terrain-texturing.md`](terrain-texturing.md) §3) | D | C (terrain-texturing.md) |
| 0x1A | general (963) | 0x3A DAY_DESC | Day descriptions ([`environment.md`](environment.md) §5.2) | xD | C (environment.md) |
| 0x1D | general (3), local (1) | 0x2C KEYMAP | Key maps (labelled in DID map 0x2800000C) | D | L, BIN |
| 0x1F | general (29,134) | 0x2B VISUAL_DESC | Held-item and object visual templates ([`setup.md`](setup.md) §5) | xD (u32 flags) | C (setup.md) |
| 0x20 | general (4,201) | 0x4E APPEARANCE | Appearance / wardrobe records ([`setup.md`](setup.md) §8, [`appearance.md`](appearance.md)) | D | C (setup.md) |
| 0x21 | general (20) | 0x43 UI_SCENE | UI scenes; layout OPEN | D | BIN |
| 0x22 | **local (302) only** | 0x28 UI_LAYOUT | UI layouts | D | R, BIN |
| 0x23 | general (735) | 0x4D ENUM_MAPPER | **EnumMapper** (§7) | D | C |
| 0x24 | none | 0x31 LAND_BLOCK_TERRAIN_LIST | Registered; no record in this install | | BIN |
| 0x25 | local (282,152) | 0x20 STRING_TABLE (range 0x25000000–0x26FFFFFF) | **String tables** (§3) | D | C |
| 0x26 | none observed | 0x20 STRING_TABLE (same range) | Part of the string-table range | | R, BIN |
| 0x27 | cell_1..5 (≈100 each), cell_14 (13) | 0x30 RENDER_IMPOSTER_TABLE | Per-region imposter tables; serials 0x27RR…, stepping by 0x1B; layout OPEN | D | BIN |
| 0x28 | general (24) | 0x49 DID_MAPPER | **DataIdMap** (§8) | D | C |
| 0x2A | general (45,544) | 0x12 SOUNDINFO | Sound-info records (also `property-system.md` §9); layout OPEN | D | BIN |
| 0x2B | general (510) | 0x51 RENDERMATERIAL | Shader records ([`material.md`](material.md) §7; labels name render materials) | D | C (material.md), L |
| 0x2D | general (3) (range 0x2D000000–0x2D0FFFFF) | 0x62 COMPUTESHADER | Compute shaders (a DXBC container) | D | BIN |
| 0x30 | general (24,091), local (3) | 0x1F MATERIALMODIFIER | Materials ([`material.md`](material.md) §6) | D | C (material.md) |
| 0x31 | general (13,781) | 0x36 MATERIALINSTANCE | Surface records: shader and material links ([`material.md`](material.md) §4) | D | C (material.md) |
| 0x32 | general (1) | 0x3B RENDER_LOD_CLASSES | LOD-class distance table ([`environment.md`](environment.md) §9.1) | D | C (environment.md) |
| 0x33 | general (24) | 0x26 IMAGE_COMPOSITOR | Compositors (DID-map labels such as "…_Face_Compositor") | D | L, BIN |
| 0x34 | gamelogic (1: 0x34000000) | 0x25 MASTER_PROPERTY | **Master property dictionary** (§6) | D | C |
| 0x35 | general (1) | 0x02 GAME_TIME | Game time ([`environment.md`](environment.md) §6) | D | C (environment.md) |
| 0x39 | general (11,155) | 0x1E PSDESC | Particle-system descriptors; layout OPEN | – | BIN |
| 0x40 | general (81,135), local (1) | 0x0A RENDERTEXTURE | Texture lists / mip chains ([`texture-list.md`](texture-list.md)) | D | C (texture-list.md) |
| 0x41 | highres (15,371), highres_aux_1..3 (16,885 / 38,556 / 7,169), surface (443,694), surface_aux_1..2 (545,619 / 340,535), local (223) | 0x05 RENDERSURFACE | **Render surfaces** (§5) | D | C |
| 0x42 | general (3), local (45) (range 0x42000000–0x42000FFF) | 0x4C FONT | Fonts (DID map 0x2800000B labels font names pointing here) | D | L, BIN |
| 0x43 | gamelogic (1) (range 0x43001000–0x43FFFFFF) | 0x50 SCRIPTCHANNELS | Script channels; layout OPEN | – | BIN |
| 0x44 | general (254) | 0x56 FORKPSDESC | Particle systems (texture names, particle templates); layout OPEN | – | BIN |
| 0x46 | none | 0x1A ENTITYGROUP | Registered; no record in this install | | BIN |
| 0x47 | gamelogic (40,643) | 0x39 ENTITYDESC | Entity descriptors ([`setup.md`](setup.md) §4) | xD (u32 flags) | C (setup.md) |
| 0x51 | general (2) | 0x24 ACTIONMAP | Action maps (labelled in DID map 0x2800000A) | D | L, BIN |
| 0x53 | none | 0x47 CONTENTDB | Registered; no record in this install | | BIN |
| 0x56 | gamelogic (1: 0x56000000) | 0x3E WLIB | Compiled script library ([`wlib.md`](wlib.md)) | D | C (wlib.md), L |
| 0x58 | local (1) (range 0x58000000–0x58000001) | 0x46 STRING_STATE | String state; layout OPEN | – | BIN |
| 0x66 | general (16,819) | 0x0E PHYSICS_MESH | Stand-alone collision meshes named by setups ([`collision.md`](collision.md) §4) | – (VLE count first) | C (collision.md) |
| 0x70–0x77 | gamelogic (383,332 at 0x70, 11 at 0x76) | 0x45 WSTATE | Script-state objects and singletons ([`wstate.md`](wstate.md)) | D | C (wstate.md), L |
| 0x78–0x7F | gamelogic (1,616 at 0x78, 383,332 at 0x79), local (2) | 0x32 DBPROPERTIES | Property sets; 0x79 + low 24 bits = the property set of the 0x70 object (`property-system.md` §5–§6) | D | C (property-system.md), L |
| 0x80 | cell_R, map_R | per kind, §2.2 | Landblock family | D | C ([`landblock.md`](landblock.md)) |
| 0x90–0x9F | none | registered three times: 0x64 KYNPATHDATA, 0x07 LBO, 0x59 HKX | Not archive records in this install (run-time or tool ranges) | | BIN |
| 0xFF | every archive (2–5 each: 0xFFFF0001, 3, 4, 5, 8) | none registered | Archive metadata records; see `dat-container.md` §7 | – | OPEN here |

The former OPEN of this section (status of 0x40 and 0x78/0x79) is resolved: they are C.

The one 0x0E record in local is 0x0EBADA55, a text filter list (TABOO_TABLE). It is mentioned
only so that a census does not flag it as an anomaly.

<a id="s2-2"></a>
### 2.2 Landblock family (prefix 0x80)

DID = `0x80` | (`K` << 20) | (`R` << 16) | (`X` << 8) | `Y`, where:

- `R` is the region: 1–5, or 14 (0xE). Archives for region 15 exist but contain only metadata.
- `X`, `Y` are landblock coordinates.

| K nibble | Archive | DB type (BIN) | Meaning | Count per full region | Status |
|---|---|---|---|---|---|
| 0 | cell_R | 0x11 LANDBLOCKDATA | Landblock data: heights, terrain surface layers, collision trees | 65,025 | C (`landblock.md` §3) |
| 1 | map_R | 0x2F LANDBLOCKBLOCKMAP | Block map: per-landblock (key, value) pairs | 65,025 | C (`landblock.md` §5) |
| 2 | cell_R | 0x2E LANDBLOCKINFO | Landblock info (cells, static entities, links, weenies) | sparse | C (`landblock.md` §4) |
| 4 | cell_R | 0x29 LANDBLOCKPROPERTIES | Landblock properties (minimap tiles, light maps) | sparse | C (`landblock.md` §6) |
| 5 | cell_R | 0x41 DISTANT_LANDSCAPE_SECTOR | Distant-landscape sector, one per 3 × 3 landblocks | sparse | Framing C; most of the layout OPEN (`landblock.md` §7) |
| 6 | none | 0x63 KYNLANDBLOCK | Registered (navigation); no record in this install | | BIN |

Invariant **L1** (Confirmed): `X` and `Y` each range over **0–254**, giving 255 × 255 = 65,025 blocks per region. They do not range over 0–255.

---

<a id="s3"></a>
## 3. String tables (prefix 0x25, `client_local_<Locale>.dat`)

A string table maps a 32-bit **token** to a localized message. A message is a list of literal **pieces** with **argument slots** between them.

<a id="s3-1"></a>
### 3.1 Table layout (Confirmed)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | DID | Table DID (0x25xxxxxx) |
| 4 | 4 | u32 | Table flag. Observed: 1 in 282,151 tables, 0 in one table. LC accepts 0, 1 and 2. |
| 8 | 2–6 | TSize | Entry count *n* (§1.3) |
| … | var | Entry × *n* | §3.2 |

The table ends exactly after the last entry. There are no trailing bytes.

> **OPEN:** The meaning of the table flag at offset 4.

<a id="s3-2"></a>
### 3.2 Entry layout (Confirmed)

| Field | Size | Type | Meaning |
|---|---|---|---|
| token | 4 | u32 | Message key, unique within the table |
| token_high | 4 | u32 | Always 0. KD treats token and token_high together as one u64 key. |
| piece_count *p* | 4 | u32 | Number of literal pieces. Observed 0–12. |
| pieces | var | UTF-16 string × *p* | Literal text pieces, in order |
| arg_count *a* | 4 | u32 | Number of argument slots |
| arg_ids | 4·*a* | u32 × *a* | Argument IDs. Each is the name hash (§1.5) of the argument's variable name. |
| name_group_count *g* | 1 | u8 | Number of argument-name groups. Observed: 0 or 1. |
| name groups | var | group × *g* | §3.3 |

<a id="s3-3"></a>
### 3.3 Argument-name group (Confirmed)

| Field | Size | Type | Meaning |
|---|---|---|---|
| name_count *m* | 4 | u32 | Number of names |
| names | var | UTF-16 string × *m* | Variable names, one for each argument ID in the same order |

Sources disagree on the type of the byte before the groups. LC reads it as a `bool8` "has names" flag; KD reads it as a group count and writes back a list of groups. Both readings give the same result on observed data, because every value is 0 or 1. Readers should treat the byte as a count, and loop over that many groups.

<a id="s3-4"></a>
### 3.4 Rendering model

Pieces and arguments alternate, starting and ending with a piece: piece 0, argument 0, piece 1, argument 1, …, piece *p*−1. An empty string marks an empty edge piece. A message that begins with an argument has `""` as piece 0.

When a caller supplies argument values keyed by variable-name hash, the message is rendered by concatenating the pieces and replacing each slot with the value for its `arg_id`. The same `arg_id` may appear more than once.

<a id="s3-5"></a>
### 3.5 Invariants

- **ST1** (Confirmed, every table): parsing consumes the payload exactly.
- **ST2** (Confirmed, every entry): `token_high` = 0.
- **ST3** (Confirmed, every entry): *a* = max(*p* − 1, 0).
- **ST4** (Confirmed, 7 of 806,052 entries carry names): when *g* = 1, *m* = *a*, and hash(names[i]) = arg_ids[i].
- **ST5** (Confirmed): tokens are unique within a table, and entries are in bucket order (§1.3.1). They are **not** sorted by token.
- **ST6** (Confirmed): every token < 0x10000000. LC labels tokens as hashes; how they are generated is unknown, and readers must not rely on this invariant.
- **ST7**: a StringInfo (§4) may point at a (table, token) pair that does not exist. 458 of 6,348 enum display references are unresolved in this install. A missing token is a lookup miss, not a parse error.

Observed population: 282,152 tables (0x25000000–0x2505B8B2, sparse) and 806,052 entries. All records were stored **uncompressed**.

---

<a id="s4"></a>
## 4. StringInfo (localizable text value)

StringInfo is the value encoding for "a piece of user-visible text". It appears in enum mappers (§7), in property values of type 13 (`property-system.md` §4.8), and in other records. It is recursive.

<a id="s4-1"></a>
### 4.1 Layout (Confirmed)

| Field | Type | Present when | Meaning |
|---|---|---|---|
| is_literal | bool8 | always | 1 = inline text, 0 = string-table reference |
| literal | UTF-16 string | is_literal = 1 | Inline text |
| token | u32 | is_literal = 0 | Entry token (§3.2) |
| table | DID | is_literal = 0 | String-table DID (0x25…) |
| has_args | bool8 | always | 1 = argument block follows; 0 = terminator follows |
| *(terminator)* | 2 bytes | has_args = 0 | Must be `01 00` |
| label_a, label_b, label_c | Pascal string × 3 | has_args = 1 | Three auxiliary strings |
| arg_count *k* | VLE | has_args = 1 | Number of argument values |
| arg × *k* | §4.2 | has_args = 1 | Argument values |

<a id="s4-2"></a>
### 4.2 Argument value

| Field | Type | Meaning |
|---|---|---|
| kind | u8 | 1 = nested StringInfo, 2 = f32, 4 = integer. No other value is valid. |
| var_id | u32 | Name hash (§1.5) of the variable this value fills |
| marker | u8 | Present only when kind ≠ 1. Must be 1. |
| value | per kind | kind 1: StringInfo (recursive); kind 2: f32; kind 4: VLE |

Readers must bound recursion depth. A depth limit of 16 is suggested; the depth seen in data is not known.

Invariants **SI1** (Confirmed over 6,352 enum values and 626 property defaults, all parsed exactly):

- the terminator is always `01 00`;
- no argument kind other than 1, 2 or 4 appears.

> **OPEN:** The meaning of the three Pascal strings in the argument block, and of the terminator bytes.

> **OPEN:** Only 4 of the 6,352 enum display values were literals. Argument blocks were not seen in enum mappers, so the argument path (§4.2) is Reported (LC) rather than exercised. (2026-10-04: none of the 1,110,502 StringInfo values in property sets has an argument block either; see `property-system.md` §4.8.)

---

<a id="s5"></a>
## 5. Render surfaces (prefix 0x41)

A render surface is one image: a texture level, an icon or a UI bitmap. It holds a **single image level**. Mip chains are separate 0x41 records listed by a 0x40 record.

<a id="s5-1"></a>
### 5.1 Header (Confirmed, 1,408,052 records)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | DID | Surface DID |
| 4 | 4 | u32 | **Category** (§5.4). Ignored by all known readers. |
| 8 | 4 | u32 | Width in pixels (0 for JPEG) |
| 12 | 4 | u32 | Height in pixels (0 for JPEG) |
| 16 | 4 | u32 | Format code (§5.2) |
| 20 | 4 | u32 | Data length *L* in bytes |
| 24 | *L* | bytes | Image data |

Invariant **RS1** (Confirmed, all records): 24 + *L* = payload length exactly.

<a id="s5-2"></a>
### 5.2 Format codes

The codes are Direct3D 9 `D3DFORMAT` values; the FourCC values are four ASCII bytes read as a little-endian u32.

| Code | Hex | Name | Expected *L* | Observed count | Status |
|---|---|---|---|---|---|
| 20 | 0x14 | R8G8B8 | W·H·3 | 19,705 | Confirmed |
| 21 | 0x15 | A8R8G8B8 | W·H·4 | 49,243 | Confirmed |
| 28 | 0x1C | A8 (one 8-bit channel) | W·H | 659,208 | Confirmed |
| 500 | 0x1F4 | Embedded JPEG (not a D3D code) | any | 65,706 | Confirmed |
| 0x31545844 | "DXT1" | BC1 | ⌈W/4⌉·⌈H/4⌉·8 | 346,604 | Confirmed |
| 0x33545844 | "DXT3" | BC2 | ⌈W/4⌉·⌈H/4⌉·16 | 206,910 | Confirmed |
| 0x35545844 | "DXT5" | BC3 | ⌈W/4⌉·⌈H/4⌉·16 | 60,676 | Confirmed |

No other codes occur in this install. Readers report any other code as "unsupported format", not as corruption.

Invariant **RS2** (Confirmed, all records): for every code except 500, *L* equals the "Expected *L*" column.

Observed dimension ranges:

| Format | Width | Height |
|---|---|---|
| DXT1 | 4–4096 | 4–4096 |
| DXT3 | 4–2048 | 4–2048 |
| DXT5 | 4–4096 | 4–2048 |
| 20 | 1–2048 | 1–2048 |
| 21 | 1–2048 | 1–1024 |
| 28 | 5–1024 | 5–1024 |

Most dimensions are powers of two, but not all, so readers must not require it. DXT dimensions were always multiples of 4 in this install. Readers should still use the ⌈·/4⌉ formula.

<a id="s5-3"></a>
### 5.3 Pixel data

All of the following are Confirmed by rendering samples and checking colours and orientation by eye.

- **Row order**: top row first, rows left to right, no row padding.
- **20 (R8G8B8)**: 3 bytes per pixel, stored in the order **B, G, R**.
- **21 (A8R8G8B8)**: 4 bytes per pixel, stored **B, G, R, A** (a little-endian u32 `0xAARRGGBB`). Alpha is straight, not premultiplied: transparent icon backgrounds have alpha 0.
- **28 (A8)**: 1 byte per pixel, single channel.
  > **OPEN:** whether consumers treat the A8 channel as alpha, luminance or a mask. This is a rendering question.
- **DXT1/3/5**: standard BC1/BC2/BC3 4×4 blocks in row-major block order, as in the public S3TC description. Do not flip V.
- **500 (JPEG)**: the *L* bytes are a complete JFIF stream starting `FF D8 FF`. Width and height in the header are 0. Take the dimensions from the JPEG.

> **OPEN:** Whether BC1 surfaces use the 1-bit-alpha mode (c0 ≤ c1) meaningfully, as cut-out alpha or as black.

<a id="s5-4"></a>
### 5.4 Category field (offset 4)

Observed values: 0, 1, 2, 4, 5, 6, 8, 10, 11, 13, 14, 15. 0 is the most common.

| Value | Observed pattern |
|---|---|
| 0 | Most common for every format |
| 15 | Every JPEG (65,706 of 65,706) and most A8R8G8B8 (36,382). The samples were UI icons and map images. |
| 4 | An R8G8B8 sample was a tangent-space normal map |
| 10 | Common for DXT3, DXT5 and A8R8G8B8 |
| 13 | Only with A8 (95 records) |

> **OPEN:** What the category field means (likely texture usage: diffuse / normal / UI / …). Decoding does not need it.

<a id="s5-5"></a>
### 5.5 Other observations (Confirmed)

- No 0x41 DID appears in more than one archive. The highres and surface families never hold two versions of the same DID.
- Both compressed and uncompressed storage occur for every format, including JPEG.
- Localized surfaces in `client_local_English.dat` (223) are 221 JPEGs and 2 A8R8G8B8, all with category 15.

---

<a id="s6"></a>
## 6. Master property dictionary (moved)

The master property dictionary (DID 0x34000000), the property type codes and their value encodings now live in **`docs/specs/property-system.md`**, together with property-set records, the +0x09000000 rule and the root DID map. The old anchors map as follows:

| Old section | New location |
|---|---|
| §6.1 Record layout | `property-system.md` §2.1 |
| §6.2 Definition layout | `property-system.md` §2.2–§2.4 |
| §6.3 Property type codes | `property-system.md` §3 |
| §6.4 Value encodings | `property-system.md` §4 |
| §6.5 Invariants MP1–MP5 | `property-system.md` §2.5. MP5 is corrected there: type-20 definitions carry a DB-object-type code. |

<a id="s6-1"></a><a id="s6-2"></a><a id="s6-3"></a><a id="s6-4"></a><a id="s6-5"></a>
These anchors are kept so that existing `Spec-Ref` trailers and code comments that cite `records.md` §6 still resolve. New work should cite `property-system.md`.

---

<a id="s7"></a>
## 7. EnumMapper (prefix 0x23, general)

<a id="s7-1"></a>
### 7.1 Layout (Confirmed, 735 records, zero leftover bytes)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | did | DID | 0x23… |
| 2 | base | DID | Parent enum whose entries are inherited, or 0. Observed: 0 in 650 records; a 0x23 DID in 85. |
| 3 | name_count *n* | TSize | |
| 4 | names | (u32 key, Pascal string) × *n* | Internal symbolic names, in bucket order |
| 5 | display_count *m* | TSize | |
| 6 | displays | (u32 key, StringInfo) × *m* | Localized display text for some keys |

Looking up a key:

1. Search this mapper.
2. If the key is not found and `base` ≠ 0, search the base mapper, and so on up the chain.

Readers must detect cycles in the base chain and limit its depth.

Invariants:

- **EM1**: parsing consumes the payload exactly.
- **EM2**: *m* ≤ *n* (Confirmed). Display keys are a subset of name keys (Reported).
- **EM3**: the largest observed mapper has 19,893 entries and hint 10.

Observed totals: 77,925 names and 6,352 display values.

---

<a id="s8"></a>
## 8. DataIdMap (prefix 0x28, general)

A DID map gives named, stable handles to DIDs. Example: the root map's "STRINGTABLE" label resolves to the DID map of named string tables.

<a id="s8-1"></a>
### 8.1 Layout (Confirmed, 24 records, zero leftover bytes)

The record is `did` (DID) followed by one or more **groups** until the payload ends. Each group has this layout:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | map_count *n* | TSize | |
| 2 | map | (u32 key, DID value) × *n* | |
| 3 | label_count *n′* | TSize | Must equal *n* |
| 4 | labels | (u32 key, short ASCII string) × *n′* | Human-readable label for each key |

Each group joins its map and its labels on `key`. In every observed group, the label keys are the same set as the map keys, and in the same order.

Every record in this install has **exactly two groups**:

- The first group is the main table.
- The second group usually has 0 elements, sometimes 1–3.

A reader should merge all groups. Within a group, a key that has no label is skipped (Reported).

> **OPEN:** What distinguishes the second group: platform-specific, debug or deprecated entries? A reader that merges both groups loses nothing.

<a id="s8-2"></a>
### 8.2 Root map

0x28000000 is the root. Its labels name the other DID maps, and resolving them gives the "L" rows of §2.1. The labels are interoperability identifiers that the client looks up by name, for example:

| Label | Resolves to |
|---|---|
| STRINGTABLE | 0x28000004, named string tables |
| EMAPPER | 0x28000001, named enum mappers |
| FONT | 0x2800000B |
| UILAYOUT | 0x28000003 |
| WEENIECONTENT | 0x2800000E |

Implementations read labels at run time and must not hard-code their DIDs.

Invariant **DM1** (Confirmed): every value in a map is either 0 (an "undefined" sentinel entry with key 0) or a DID whose prefix appears in §2.1.

---

<a id="s9"></a>
## 9. Sound payloads (prefix 0x0A)

<a id="s9-1"></a>
### 9.1 Record envelope (Confirmed, all 44,337 records in sound, sound_aux_1 and local_English)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | DID | 0x0A… |
| 4 | 4 | u32 | Audio length *S* |
| 8 | *S* | bytes | Audio container (§9.2) |
| 8+*S* | 4 | u32 | Unknown. Observed values 2, 3, 7. |
| 12+*S* | 4 | f32 | Unknown. Observed always 0.0. |

Invariant **SN1** (Confirmed): payload length = *S* + 16. Every observed 0x0A record is stored uncompressed.

How the trailer u32 relates to the container:

| Trailer u32 | Containers observed with it |
|---|---|
| 2 | Ogg and one WAV |
| 3 | Ogg and WAV |
| 7 | Only the DMAP records |

So the trailer is not a codec ID.

> **OPEN:** The meaning of the two trailer fields.

> **OPEN:** LC also describes a "sound info" record (DID, flags, 5 × f32, u32 sound DID, optional Pascal name). No 0x0A record in this install has that shape. **Partly answered (2026-10-05):** sound-info records are prefix 0x2A (DB type 0x12 SOUNDINFO, §2.1, BIN; also `property-system.md` §9). Their layout is not specified here.

<a id="s9-2"></a>
### 9.2 Container sniffing (Confirmed)

Classify the audio by its first bytes at payload offset 8:

| Bytes at +8 | Form type at +16 | Container | Count | Notes |
|---|---|---|---|---|
| `OggS` | | Ogg (Vorbis) | 40,874 | |
| `RIFF` | `WAVE` | RIFF WAVE | 3,455 | The first chunk is usually `fmt ` (PCM, format tag 1). Some files start with `JUNK` or `bext` chunks, so **walk the chunks** to find `fmt ` and `data` rather than assuming fixed offsets. RIFF chunk sizes are padded to even lengths. |
| `RIFF` | `DMAP` | DirectMusic audio-path configuration (not audio) | 8 | 0x0A000FE4–0x0A000FEB, which are the targets of DID map 0x28000005 |

Anything else should be reported as "unknown audio container". This install has no such records.

---

<a id="s10"></a>
## 10. Discrepancies with the research notes

These were found against `research/notes/dat-format.md` (DF) and `research/notes/data-semantics.md` (DS).

| # | Note says | Observed (2026-10-03) | Resolution |
|---|---|---|---|
| X1 | DF §6: "0x22 (EnumMapper)" | 0x23 records decode as enum mappers; 0x22 records exist only in local | DF is wrong. 0x23 = EnumMapper, 0x22 = UI layouts (DS is right). |
| X2 | DF §6: 0x22 is in general, then local | No 0x22 in general; 302 in local | Fact corrected; routing still tries both |
| X3 | DF §7 string table: "u64 token", then "u8 nArgStringGroups, each {u32 n; …}". DS §4.5: "bool hasNames" | The byte is 0 or 1. The token's upper u32 is always 0. | The two readings agree on all data; §3.3 adopts "count" |
| X4 | DF/DS: TSize hint is a "bucket-count hint" with unknown meaning | Hint indexes a prime bucket table; elements are stored in bucket order | New fact (§1.3) |
| X5 | DS §4.1: 0x34000000 is "skip 8, TSize names, skip 2, TSize defs" | Exactly right; consumes the whole record | Confirmed. The skipped bytes are zero in this install. |
| X6 | DS §4.1: the definition's `data` is "enum DID / bitfield enum DID" | Also 0x28 (DID map) for type 8, and 0x34000000 for type 10 | Extended (`property-system.md` §3) |
| X7 | DS §6: "a region has 256 × 256 landblocks" | 255 × 255 (X, Y ∈ 0..254) | Corrected (L1) |
| X8 | DS §1: 0x2701/0x2702 cell DIDs are an old-client artefact | 0x27RR… DIDs are present in today's cell_1..5 and cell_14 | Corrected; meaning OPEN |
| X9 | DS/DF list cell/map regions 1–5, 14 | cell_15 and map_15 archives also exist, holding only metadata | Noted |
| X10 | DS §13: font type ID unknown | 0x42 (DID map FONT labels point there) | New (L) |
| X11 | DS §1: 0x0A holds "SoundInfo records plus raw OGG/WAV" | Every 0x0A record has the raw envelope. 8 are DirectMusic DMAP, not audio. | Corrected (§9) |
| X12 | DF §6: 0x30 and 0x40 are general-only | 3 × 0x30 and 1 × 0x40 also live in local | Noted; routing should fall back to local |
| X13 | DS §7.4: 0x41 layout lists only formats | The JPEG header has width = height = 0, and each record is one level with no mips | New (§5) |
| X14 | DF §5: subfile payloads "usually" have a class index at +4 | True for some types only. For 0x41, +4 is a category; for 0x25 a flag; for 0x23 a base DID. | Type-specific |
| X15 | DS §1 lists the 0x41 search order across archives | No DID is duplicated between those archives in this install, so order does not change the result today | Noted |
| X16 | P10 / §1.6 (this spec, 2026-10-03): every record begins with its own DID | 0x02, 0x11, 0x15, 0x1A, 0x1F and 0x47 begin with a u32 before the DID; 0x05, 0x07, 0x08, 0x0C, 0x39, 0x43, 0x44, 0x58, 0x66 and some 0x0E have no DID at the start | Corrected (§1.6, 2026-10-05) |
| X17 | `environment.md` first look: kinds of 0x11, 0x12, 0x16, 0x17, 0x19, 0x02, 0x32 by content | The client's registration gives SCENE_DESC, TERRAIN_DESC, WATER_DESC, FOG_DESC, TERRAIN_TYPE_TABLE, SCENE, RENDER_LOD_CLASSES | Corrected (§2.1) |

---

<a id="s11"></a>
## 11. Open questions (summary)

1. VLE `0xE0` escape usage (§1.2).
2. Order within a hash bucket; bucket counts for hint ≥ 13 (§1.3).
3. String-piece markup and escape grammar (§1.4).
4. String-table flag at +4 (§3.1).
5. StringInfo auxiliary strings and terminator meaning; argument path is unexercised (§4).
6. Surface category field (§5.4); A8 semantics; BC1 1-bit alpha (§5.3).
7. 0x34000000 reserved and separator bytes; definition fields a/b/c/d; flag bits; subset list; colour channel order (moved to `property-system.md` §13).
8. DID-map second group (§8.1).
9. Sound trailer fields; where the sound-info records live (§9.1).
10. *(Answered 2026-10-05: the kind of every prefix is now known from the client's DID-range registration, §2.1, and most are decoded in other specs.)* Layouts still OPEN: 0x14, 0x21, 0x27, 0x2A, 0x39, 0x43, 0x44, 0x58 and most of landblock K = 5 (§2). (0x08 is decoded in [`walker.md`](walker.md) §11.5, 2026-10-06.)

<a id="s12"></a>
## 12. Validation guidance for implementers

Real-data tests (skipped when no install is configured) should assert the **invariants** above, not the observed counts, because counts change with every game patch. Synthetic fixtures should cover:

- every VLE form, including `0xE0` and truncation at each byte;
- TSize with hint 0 and with an unusual hint;
- a string table with *p* = 0, with *p* = 1, and with an argument-name group;
- a nested StringInfo up to the depth limit, plus one level past it;
- one surface of each format code, with a wrong *L* to check rejection;
- a property definition with default, minimum and maximum present, for each value type;
- a DID map with two groups and with mismatched counts;
- a sound record of each container kind, plus a truncated one.

<a id="s13"></a>
## 13. Open items raised by the `formats` implementation (2026-10-04)

These came up while implementing `crates/formats` from this spec alone. Each records the conservative choice the implementation made, so a spec writer can confirm or correct it.

> **OPEN:** BC1/BC2/BC3 palette rounding. §5.3 says "standard S3TC" but does not fix the rounding of the 1/3, 2/3 (colour) and 1/7, 1/5 (alpha) interpolants, which differs between decoders by ±1. The implementation rounds to nearest, `(w0·a + w1·b + d/2) / d`, and expands 5/6-bit endpoints by bit replication. DXT3 and DXT5 colour blocks always use four-colour mode, whatever the order of c0 and c1.

> **OPEN:** A8 (code 28) to RGBA conversion. Until §5.3's A8 question is answered, `Surface::to_rgba8` maps A8 to `(255, 255, 255, a)` and leaves the raw channel available to callers.

> **OPEN:** Property type 21 values are read as `bool8` (0 or 1 only, §1.1), so any other byte is a parse error. §6.4 says "u8 (0/1)", which may mean other values are possible but unobserved. Please confirm whether readers should reject them.
>
> **Resolved (2026-10-04):** only 0 and 1 occur, in 6,258,467 set values and 2,003 dictionary values. Strict `bool8` is safe. See `property-system.md` §4.5.

> **OPEN:** §6.2 says the two child-PID copies are always equal, but not what a reader should do when they differ. The implementation keeps the first copy and logs at debug level.
>
> **Resolved (2026-10-04):** keeping the first copy is the specified behaviour; copies never differ in observed data. See `property-system.md` §2.2.

> **OPEN:** §4.1 has a 2-byte terminator only when `has_args` = 0. The implementation reads nothing after the argument values when `has_args` = 1. Because that path has not been exercised on data (§4 OPEN), please confirm that no terminator follows the argument block.

> **OPEN:** Property type 17 (Position) has no stored encoding in §6.4. A definition of type 17 with a default, minimum or maximum flag set makes the dictionary decoder fail with "unsupported property type", since it cannot skip a value of unknown length. Observed data has no such definition.
>
> **Resolved (2026-10-04):** the position encoding is now specified in `property-system.md` §4.6.

> **OPEN:** A 0x41 JPEG surface (code 500) whose header width or height is non-zero is accepted. The implementation logs it and takes the dimensions from the JPEG stream. §5.1 does not say whether this should be an error.

> **OPEN (implementation choice, 2026-10-06):** before decoding an embedded JPEG, the implementation walks its marker segments to the frame header to bound the decoder's memory (progressive frames need coefficient buffers for the whole image). It rejects as malformed a stream that has a marker code 0x00, 0x01, 0xD0–0xD7 or 0xD8 before the frame header (JPEG decoders step over these in different ways, so a crafted stream could show the pre-scan a decoy frame), and a stream whose first frame header's width and height differ from the frame the decoder reads. A real JFIF stream has none of these before its frame header. §5.3 does not say whether such streams occur; please confirm none do.

> **OPEN:** MP5 (§6.5) does not hold for type 20 (DID reference). Every type-20 definition in this install (1,849 of 1,849) has a non-zero `data` field below 0x01000000. Example: PID 16 has `data` = 0x45. These values look like a code for the expected DID type (perhaps a DID-type index, but not the §2 high-byte prefix itself), not a zero. Found by `crates/formats/tests/real_data.rs` on 2026-10-04. The spec writer should determine what this value means and correct MP5 and the §6.3 table.
>
> **Resolved (2026-10-04):** the value is a DB-object-type code, labelled by the enum mapper that the EMAPPER DID map names "DBOType". 0x45 is the game-object (WState) type, whose references point at 0x70 DIDs. MP5 is corrected in `property-system.md` §2.5, and the code table is in §9.

<a id="s14"></a>
## 14. Change log

| Date | Change |
|---|---|
| before 2026-10-05 | First version 2026-10-03 and later edits; no change log was kept (see the repository history). |
| 2026-10-05 | Provenance: BIN row and D2; P10 corrected. §1.6 rewritten: record prologues per prefix (census of 8 archives); the claim that every record starts with its own DID is withdrawn (false for 0x05, 0x07, 0x0C, 0x66 and others). §2.1 rewritten: DB type of every prefix from the client's DID-range registration (adds the 0x15/0x1A/0x35 pairings, 0x66 = PHYSICS_MESH, 0x2A = SOUNDINFO, 0x16/0x17 = WATER_DESC/FOG_DESC, and all others), prologue column, statuses updated to the specs that decode each prefix (0x40, 0x78/0x79 now C as the former OPEN asked). §2.2: DB type per landblock kind; K = 5 is the distant-landscape sector; K = 6 registered but absent. §9.1 OPEN partly answered (sound info = 0x2A). §10: X16, X17. §11 item 10 updated. This change log added. |
| 2026-10-06 | Alignment with [`walker.md`](walker.md) §11.5, no other change; status stays Accepted. §2.1: the 0x08 row no longer says "layout OPEN"; it points to `walker.md` §11.5, status BIN → C (walker.md). §1.6: the "–" row names the 0x08 prologue (camera entity's 0x47 DID) and drops 0x08 from the OPEN layouts. §11 item 10: 0x08 removed from the OPEN list. Provenance: OBS6 and D3 added (11 of 11 records re-checked against `walker.md` §11.5.1 and the per-type record sizes). |
