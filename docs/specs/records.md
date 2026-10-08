<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Record encodings inside subfile payloads

| | |
|---|---|
| **Status** | **Accepted** (2026-10-08: re-review of the 2026-10-07 OPEN-closing revision and its 2026-10-08 review fixes passed; previously Accepted 2026-10-05 and 2026-10-06). Sections and anchors of the earlier Accepted text are unchanged; implementers re-check code built on the sections the 2026-10-07 change-log row lists under "Behaviour changes". |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | 2026-10-05: **passed**; re-review 2026-10-05: **passed**, unchanged. 2026-10-07 revision: 2026-10-08 **changes requested** (three items); re-review of the fixes 2026-10-08 (Claude review session, separate from the writer's): **passed**, Accepted. Details in the review log (decision 0018). |
| **Companion spec** | `docs/specs/dat-container.md` (archive header, block/extent layer, B-tree directory, zlib, DID routing). This spec starts where that one ends: with a fully reassembled and, if needed, decompressed subfile payload. |

## Purpose

This spec gives an implementer of the `formats` crate what they need to decode these records from the bytes of a subfile payload:

- the shared primitive encodings (§1);
- what the high byte of a DID means, the DB object type the client registers for each prefix, and what each record starts with (§1.6, §2);
- localized string tables, DID prefix 0x25 (§3), and the StringInfo value that points into them (§4);
- render surfaces (textures and icons), DID prefix 0x41 (§5);
- the master property dictionary, DID 0x34000000 (§6, now a pointer to `property-system.md`);
- enum mappers, prefix 0x23 (§7), and DID maps, prefix 0x28 (§8);
- sniffing the audio inside sound records, prefix 0x0A (§9);
- the framing of encounter tables, prefix 0x14 (§15).

§10 lists every place where the research notes, this spec's earlier text and the observed data or client disagree. §11 collects the open questions.

## Provenance

How sources are marked:

- **Confirmed**: the layout was decoded on the user's own install on 2026-10-03, using a throwaway probe in the dirty-room scratchpad. A Confirmed fact comes with what was decoded and how much of it.
- **Reported**: the fact comes only from published third-party material and was not, or could not be, checked against data.
- **BIN**: read from the client executable under method M4 (decision 0011; rows BIN and BIN2). A fact that rests on BIN alone is marked BIN; one also checked against data is marked Confirmed with both sources.

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
| BIN2 | **M4 under decision 0011, second use, 2026-10-07.** Same build 4906.0071.1794.4035, Ghidra headless decompiler on a read-only copy of the shared analysis project, single functions only. **Questions:** (a) how the client reads a 0x41 record: the shared record header, which types carry a second header word, and the size check (§5); (b) what the client does with the surface category, with JPEG surfaces and with block-compressed data when it turns a surface into a texture (§5.3, §5.4); (c) how a 0x25 record's word at offset 4 is read, and whether the entry lookup and renderer read it (§3.1); (d) how the client looks a key up in a DID map: which of the record's tables, in which order, and whether by key or by label (§8); (e) the hint limit of the generic TSize reader (§1.3). **Why M1–M3 were not enough:** the data shows these values but not what reads them (the category and the string-table word are unlabelled numbers, the second DID-map group is never needed to tell keys apart); no published source describes the client's readers; the client cannot be run here (M3). Only offline record readers, the DID-map lookup and the surface-to-texture routine were read; nothing about network, login, keys, launcher or anti-tamper. Listings stay in the dirty scratch area. |
| OBS7 | Observed in user install 2026-10-07 by throwaway probes in the dirty-room scratchpad: the block data of **all 346,604** DXT1 surfaces (568,358,856 blocks) and the colour blocks of all 267,586 DXT3 and DXT5 surfaces (1,542,008,703 blocks); a marker walk of **all 65,706** JPEG streams from the start-of-image marker to the first frame header (segment kinds, the position of every further start-of-image byte pair, precision and component count of the first frame), plus the end of each stream; all 24 DID maps (both groups, and whether each target DID exists in `client_general.dat`, `client_gamelogic.dat` or `client_local_English.dat`); all 5 prefix-0x14 records parsed to the last byte. |
| PS, ST, SND, MR, PT, TT, FN, TL, DC, KM, MD, UL | Companion specs, cited by section: `property-system.md` (its BIN rows C1–C16 rest on M4), `string-table.md`, `sound.md`, `misc-records.md`, `particles.md`, `terrain-texturing.md`, `fonts.md`, `texture-list.md`, `dat-container.md`, `keymap.md`, `map-data.md`, `ui-layout.md`. A fact taken from one of them keeps the mark it has there. |
| D3D10 | The public Direct3D 10 block-compression description (BC1–BC3 decoding, including BC1's three-colour mode), a general reference. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| P1 | Little-endian fixed-width integers and IEEE-754 floats | LC, JT; OBS | Confirmed |
| P2 | VLE layout, all three forms, including byte order of the 4-byte form; there is **no** 0xE0 escape: every first byte 0xC0–0xFF starts the 4-byte form; values of 0x40000000 or more cannot be written (§1.2) | LC (three forms); OBS (1-, 2- and 4-byte forms all seen); PS C1 (BIN: the client's one VLE routine for stored formats); PS OBS3 (no VLE in the property data starts with 0xE0 or more) | Confirmed for the forms seen; BIN for first bytes 0xE0–0xFF and for the writer limit. Corrected 2026-10-07: LC's 5-byte escape is not used by the client. |
| P3 | VLE values are always written in the shortest form | OBS (about 470k VLEs checked) | Confirmed |
| P4 | TSize = 1 hint byte + VLE count | LC, KD; OBS | Confirmed |
| P5 | The TSize hint byte indexes a table of hash-bucket counts, and elements are stored in bucket order | OBS. Derived by this spec writer; no source states it. | Confirmed for indices 0–12 |
| P5a | The bucket-count table has 23 entries (hints 13–22 added in §1.3); entries that share a bucket keep their stored order on reading and are written in that order; a hint of 23 or more makes the client's reader fail (§1.3) | PS C2, C3 (BIN); BIN2 (e) (the generic TSize reader compares the hint with the 23-entry table and fails the stream when it is not below) | BIN (no hint above 12 occurs) |
| P11 | Escape, markup and grammar inside string pieces (§1.4) | ST §4, §7 (Confirmed (BIN + OBS) there) | See `string-table.md` |
| P6 | Pascal string (VLE length, Latin-1) | LC; OBS | Confirmed |
| P7 | Prefixed UTF-16 string (VLE count of UTF-16 code units, then UTF-16LE) | LC, KD; OBS | Confirmed |
| P8 | u8-length ASCII string | LC; OBS (0x28 labels) | Confirmed |
| P9 | Name hash used for string-table argument IDs | LC (hash routine); OBS (argument IDs match the hash of the argument-name strings stored beside them; 11,280 of 16,212 sampled IDs match a published name list) | Confirmed |
| P10 | The record types of this spec begin with their own DID; other types differ (§1.6) | LC, JT, DU; OBS (every record decoded; prologue census of 8 archives, 2026-10-05) | Confirmed. Corrected 2026-10-05: the earlier claim "records begin with their own DID" holds for most but not all prefixes. |
| D1 | DID high-byte type table (§2) | LC routing, research notes; OBS (per-archive prefix census of all archives) | Mixed. Per row in §2. |
| D2 | DB object type registered for each DID range (§2.1, §2.2) | BIN (the client's start-up table of DID ranges); OBS (labels from the DBOType enum mapper; every decoded prefix agrees with its type) | BIN |
| D3 | Prefix 0x08 (camera states) is decoded in [`walker.md`](walker.md) §11.5; its prologue is the camera entity's 0x47 DID, not its own DID (§1.6, §2.1) | `walker.md` W9, W25, W-I1, W-I2; OBS6 | Confirmed (layout owned by `walker.md`) |
| D4 | Layouts of prefixes 0x0E (most DIDs), 0x1D, 0x21, 0x22, 0x27, 0x2A, 0x2D, 0x33, 0x39, 0x42, 0x43, 0x44, 0x51, 0x58 and of the 0xFFFF00xx records are specified in companion specs (§1.6, §2.1) | MR, SND, PT, ST, FN, KM, MD, UL, DC (each Confirmed there on every record of its type) | See the spec named in each row |
| S1 | String-table layout (§3) | LC, KD, EP; OBS (282,152 tables, 806,052 entries, zero leftover bytes) | Confirmed |
| S2 | StringInfo layout (§4) | LC; OBS (6,352 enum display values and 626 property-definition defaults) | Confirmed for the values that occur. Superseded for the flag byte and the argument block by S5. |
| S3 | The table word at offset 4 is read right after the DID and kept with the table; the entry lookup and the message renderer do not read it (§3.1) | BIN2 (c) (the string-table record reader; the lookup and render routine of `string-table.md` §9.2); OBS (1 in 282,151 tables, 0 in one) | BIN; meaning OPEN |
| S4 | Entry fields: `token_high` is a redirect DID (0 = none); piece count observed 1–12; the byte before the name groups is a flag, 0 or 1, any other value a stream error (§3.2, §3.3) | ST G34, D1, G41 (BIN there; OBS: redirect 0 and flag 0 or 1 in all 806,052 entries) | Confirmed for the values that occur; BIN for the rest |
| S5 | StringInfo stored form: first byte 1 = literal, any other value = reference (2 shows an authoring string); the second flag byte announces three authoring strings; an argument table (TSize of u32 key, u16 kind, value) **always** follows; the former "terminator `01 00`" is the empty argument table (§4) | PS §17.4, C4, C5, C11–C16 (BIN there); OBS (every StringInfo of this spec ends in `01 00`) | Confirmed for the values that occur; BIN for the rest |
| R1 | 0x41 surface header (§5) | LC, JT, DU; OBS (all 1,408,052 headers) | Confirmed |
| R2 | Format codes 20, 21, 28, 500, DXT1, DXT3, DXT5 | LC, JT, D3D; OBS | Confirmed. These are the only codes present. |
| R3 | Pixel byte order and row order of 20 and 21; DXT blocks are standard S3TC | JT, D3D; OBS (rendered, colours and orientation checked by eye) | Confirmed |
| R4 | Embedded JPEG has width = height = 0 in the header | OBS | Confirmed (all 65,706) |
| R5 | The client reads a surface as: the shared record header (own DID, then the category word, which only the 0x41 range's registration enables), width, height, format, data length, data; it fails the record when the length is not width × height × bits-per-pixel / 8 for a non-JPEG format (§5.1) | BIN2 (a); OBS (RS1, RS2 hold for all 1,408,052) | BIN; consistent with every record |
| R6 | Block-compressed data reaches the graphics device unchanged (the texture build copies it as stored, with the surface's own format code), so BC1/BC2/BC3 texels are decoded by the device; BC1 three-colour blocks decode index 3 to transparent black (§5.3) | BIN2 (b); D3D10; OBS7 (block census) | BIN for the client path; decoding rule general reference; census Confirmed |
| R7 | The client describes format 28 as an alpha-only format (8 bits, alpha mask 0xFF, no colour masks); its two traced consumers read the byte as opacity (terrain masks) and as coverage (glyph atlases) (§5.3) | BIN2 (a); TT §5.1; FN §4.1 | BIN / Confirmed by those specs |
| R8 | JPEG surfaces: the client decodes the stream with an embedded JPEG library, takes width and height from the stream (the header values are not used), requires an image of three components and builds an R8G8B8 image from it; every stored stream is a baseline or progressive JPEG stream of three 8-bit components with nothing but length-prefixed segments between its start marker and its first frame header (§5.3.2) | BIN2 (b) (surface-to-texture routine; the library's own message strings); OBS7 (all 65,706 streams) | BIN; census Confirmed |
| R9 | The category word is tested for 13 and 15 when a surface is loaded and for 15 when a texture is built from an uncompressed surface; no other value is tested in the routines read (§5.4) | BIN2 (a), (b); TL §7.2 and FN §4.1 (13 = glyph atlases, 15 = UI images and every JPEG) | BIN; meaning OPEN |
| M1 | 0x34000000 layout (moved to `property-system.md` §2) | LC, MER; OBS (2,534,594 bytes decoded to the last byte) | Confirmed |
| M2 | Property type codes 1–22 and their value encodings | LC; OBS (all 22 codes present; every default/min/max value decoded) | Confirmed; see `property-system.md` §3–§4 |
| E1 | EnumMapper layout (§7) | LC; OBS (735 records, zero leftover bytes) | Confirmed |
| E2 | DataIdMap layout (§8) | LC; OBS (24 records, zero leftover bytes) | Confirmed |
| E3 | Enum-mapper lookup: names and display texts are looked up independently along the base chain (§7.1) | PS §17.5 (BIN there) | BIN |
| E4 | DID maps: second-group census; the second group's keys never occur in the first; the client looks a key up in both key-to-DID tables of a loaded map, one after the other, at both steps of a root-then-sub-map lookup; the client addresses maps by key number, and the label strings do not occur in its executable (§8.1, §8.2) | OBS7 (24 of 24 records); BIN2 (d) (the DID-map lookup routines; the executable searched for the root labels as ASCII and as UTF-16) | Confirmed / BIN |
| A1 | Sound record envelope: DID, size, audio, u32, u32 (§9.1) | LC; OBS (all 44,337 records); SND A1 (the trailer is a storage-format word and a reserved word, BIN there) | Confirmed. Corrected 2026-10-07: the last word is read as a u32, not an f32. |
| A2 | Audio payload magics: RIFF/WAVE, OggS, RIFF/DMAP | LC (RIFF and OggS); OBS (DMAP found by this spec writer) | Confirmed |
| A3 | Sound-info records are prefix 0x2A; layout in `sound.md` §4 (§9.1) | SND I1 (Confirmed there, 45,544 of 45,544) | Confirmed (by `sound.md`) |
| N1 | Prefix 0x14 framing: DID, two u32, u32 entry count, entries of (u32 key, Pascal name, u32 slot count, 21-byte slots) (§15) | OBS7 (5 of 5 records parsed to the last byte, 70 slots) | Confirmed framing; field roles OPEN |

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

A VLE is 1, 2 or 4 bytes. The first byte `a` selects the form:

| First byte `a` | Total size | Value |
|---|---|---|
| `0x00`–`0x7F` | 1 | `a` |
| `0x80`–`0xBF` | 2 | `(a & 0x3F) << 8` OR the second byte. Range 0–0x3FFF. |
| `0xC0`–`0xFF` (0xE0 included) | 4 | `(a & 0x3F) << 24` OR `(second byte) << 16` OR (bytes 3 and 4 read as a little-endian `u16`). Range up to 0x3FFFFFFF. |

Notes:

- In the 4-byte form, the low 16 bits are stored **little-endian** after a big-endian-looking high part. Confirmed: the 4-byte VLE `C0 00 9B 6B` in 0x34000000 decodes to 27,547. Reading the u16 big-endian would give 39,787, and the record would then fail to parse.
- The 2-byte form keeps 6 value bits of `a` (bit 6 is 0 in that range), so `(a & 0x7F)` and `(a & 0x3F)` give the same result.
- **There is no 5-byte escape** (P2, BIN). The client reads and writes every VLE of the stored formats (TSize counts, Pascal and UTF-16 string lengths, and the others of `property-system.md` §17.1) with one routine, in which a first byte of 0xE0 is an ordinary 4-byte-form byte (value bits 0x20 in the top six). LC describes `0xE0` as an escape followed by a full `u32`; the client does not, and a reader that follows LC misreads every value 0x20000000–0x20FFFFFF. No VLE in the data starts with 0xE0 or more.
- **Writer rule** (BIN): the 1-byte form for 0–0x7F, the 2-byte form for 0x80–0x3FFF, the 4-byte form for 0x4000–0x3FFFFFFF. A value of 0x40000000 or more cannot be represented; a writer refuses it.

Invariants (Confirmed, about 470k VLEs in 0x25 and 0x23 records):

- **V1**: the shortest form is always used. No 2-byte VLE holds a value below 0x80, and no 4-byte VLE holds a value below 0x4000.
- **V2**: a reader that reaches end-of-buffer in the middle of a VLE reports truncation and does not guess.
- **V3** (BIN): a reader decodes first bytes 0xC0–0xFF, 0xE0 included, with the 4-byte form; it never consumes a fifth byte.

<a id="s1-3"></a>
### 1.3 TSize: hashed-collection header

A **TSize** is the count prefix of a serialized hash table or hash set. It is 1 + VLE bytes:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 1 | u8 | **Bucket-count index** ("hint") |
| 1 | 1, 2 or 4 | VLE | Element count *n* |

A reader needs only *n*. The hint describes how the writer's hash table was sized, and it fixes the order of the elements that follow.

**Bucket-count table** (hints 0–12 Confirmed by this spec writer from element order, §1.3.1; hints 13–22 BIN, P5a). Every value is prime:

| Hint | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Buckets *B* | 11 | 23 | 47 | 89 | 191 | 383 | 761 | 1531 | 3067 | 6143 | 12281 | 24571 | 49139 |

| Hint | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 22 |
|---|---|---|---|---|---|---|---|---|---|---|
| Buckets *B* | 98299 | 196597 | 393209 | 786431 | 1572853 | 3145721 | 6291449 | 12582893 | 25165813 | 50331599 |

The table ends at hint 22. The client's reader fails a collection whose hint is 23 or more (BIN2 (e)).

<a id="s1-3-1"></a>
#### 1.3.1 Element order (Confirmed)

Take the elements of a TSize collection in stored order. Their u32 keys, taken modulo *B*[hint], form a **non-decreasing** sequence. In other words, the elements are written bucket by bucket, in ascending bucket number. This held for:

- every observed string table, enum mapper and DID map;
- the name list of 0x34000000;
- its definition list.

One exception: an enum mapper whose hint is 10. Its order fits *B* = 12281 and does not fit 12289, which is why the table says 12281.

**Within a bucket** (BIN, P5a): when the client reads a collection it appends each element to the end of its bucket, so elements that share a bucket keep their stored order; when it writes, it emits the buckets in ascending order and each bucket's elements in that order. A byte-exact writer therefore keeps the stored order inside each bucket; a writer producing new data may use any order inside a bucket (the client reads it back unchanged).

The hint is **not** a pure function of *n*:

- In string tables, the hint is the smallest index whose *B* ≥ *n*.
- In DID maps, a hint of 1 is used even for *n* = 0..9, and hint 4 (*B* = 191) appears with *n* = 314 and 315.
- In 0x34000000, *n* = 27,547 has hint 11 while *n* = 27,546 has hint 12.

A writer that wants a byte-exact round trip must keep the original hint. A writer producing new data may choose any hint ≥ the smallest one that fits, as long as it writes the elements in the matching bucket order. It should stay at hint ≤ 12, because no stored collection uses more.

Invariant **T1** (revised 2026-10-07): hint ≤ 12 in all observed data (Confirmed). A reader accepts every hint 0–22 and reports a hint of 23 or more as malformed, as the client does (BIN, P5a). It never sizes anything by the hint (`property-system.md` §17.2 adds a count bound per collection type). A reader that does not need ordering may warn about elements that are not in bucket order, but decodes them.

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

Text inside string-table pieces stays as stored. Line breaks show up as the two characters backslash and `n`, and markup tags appear in the text. Interpreting them is not part of decoding (P11): the grammar of variables, labels, selectors, directives and escapes is [`string-table.md`](string-table.md) §4–§6, the rendering pipeline (whitespace, display cut, escape decoding) is its §7, and the UI-text markup (colour, emphasis, paragraphs, lists, links) is its §4.7–§4.8 and §7.8.

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
| **–**: no own DID in the first 8 bytes | 0x05 (f32 clip duration first, `havok-data.md` §2.3), 0x07 (u32 table kind first, `script-table.md` §2.1), 0x0C (no prefix at all, `script-table.md`), 0x66 (VLE vertex count first, `collision.md` §4.1), 0x08 (camera entity's 0x47 DID first, [`walker.md`](walker.md) §11.5.1), 0x39 (f32 time step first, [`particles.md`](particles.md) §4.1), 0x43 (a TSize channel table first, [`misc-records.md`](misc-records.md) §7.2), 0x44 (u32 image length first, [`particles.md`](particles.md) §6.1), 0x58 (u32 first, [`string-table.md`](string-table.md) §10.3), 3 of 0x0E (compression dictionaries, two 16-byte identifiers first, [`misc-records.md`](misc-records.md) §11), 0xFF (archive metadata, [`dat-container.md`](dat-container.md) §7) | type-specific; each layout is in the spec named (D4) |

A decoder therefore checks the DID only where its record's spec places it; the container layer must not require it (`dat-container.md` §6.2).

Shared header (BIN2 (a), R5): most record readers of the client start with one shared header reader. It reads the own DID and then, only when the record's DB type is registered with a "second header word" flag, one more u32. Among the registered DID ranges only the 0x41 range sets that flag; for 0x41 the word is the surface category (§5.4). For every other type the fields after the DID belong to the type's own layout.

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
| 0x0E | general (7), local (1) | by DID: 0x0E000002 0x1D SEGMENT_TABLE; 0x0E000004 0x19 PLACES_TABLE; 0x0E000006–0x0E00FFFF 0x44 MAPNOTE_DESC; 0x0E010000–0x0E01FFFF 0x01 COMPRESSION_DICTIONARY; 0x0E100001 0x18 FARVIEWENTITIES; 0x0E100003 0x3D STRING_TOKEN_MAPPER; 0x0E100009 0x58 FALLBACK_MAPPER_TABLE; 0x0E100010–0x0E100030 0x06 MONITOREDPROPERTIES; 0x0E200000–0x0E20001F 0x14 NAME_FILTER_TABLE; 0x0E400000–0x0E4FFFFF 0x2D QUEST_MAPNOTE_DESC; 0x0EBADA55 0x22 TABOO_TABLE | Singleton tables: named places 0x0E000004 and map notes 0x0E000006 ([`map-data.md`](map-data.md) §3–§4), dungeon quest notes 0x0E400000 (`map-data.md` §5), the name-filter table 0x0E200000 and the taboo table 0x0EBADA55 ([`misc-records.md`](misc-records.md) §9–§10), the compression dictionaries 0x0E010000–2 (stored envelope only, `misc-records.md` §11); the other registered DIDs have no record in this install | D (4 of 7 in general, the local one); – (3) | C (map-data.md, misc-records.md), L, BIN |
| 0x0F | general (7) | 0x4F REGION | Region descriptors ([`environment.md`](environment.md) §3; labelled per region in DID map 0x28000008) | D | C (environment.md), L |
| 0x11 | general (5) | 0x4A SCENE_DESC | Scene-choice tables, region slot 1 ([`scenery.md`](scenery.md) §4.3) | xD | C (scenery.md) |
| 0x12 | general (5) | 0x1C TERRAIN_DESC | Terrain-scene tables, slot 2 ([`scenery.md`](scenery.md) §4.2) | D | C (scenery.md) |
| 0x14 | general (5) | 0x13 ENCOUNTER_DESC | Encounter tables, slot 3 ([`environment.md`](environment.md) §4); framing §15, field roles OPEN | D | C (framing, §15), BIN |
| 0x15 | general (1) | 0x3F SKY_DESC | Sky description, slot 4 ([`environment.md`](environment.md) §5.1) | xD (u32 version 1) | C (environment.md) |
| 0x16 | general (5) | 0x17 WATER_DESC | Water tables, slot 5 ([`water.md`](water.md) §4) | D | C (water.md) |
| 0x17 | general (5) | 0x16 FOG_DESC | Fog-zone tables, slot 6 ([`water.md`](water.md) §5) | D | C (water.md) |
| 0x18 | general (5) | 0x21 PROPERTY_DESC | Block-map property descriptors, slot 7 ([`landblock.md`](landblock.md) §5.2) | D | C (landblock.md) |
| 0x19 | general (5) | 0x35 TERRAIN_TYPE_TABLE | Terrain type tables, slot 8 ([`terrain-texturing.md`](terrain-texturing.md) §3) | D | C (terrain-texturing.md) |
| 0x1A | general (963) | 0x3A DAY_DESC | Day descriptions ([`environment.md`](environment.md) §5.2) | xD | C (environment.md) |
| 0x1D | general (3), local (1) | 0x2C KEYMAP | Key maps (labelled in DID map 0x2800000C; [`keymap.md`](keymap.md) §12.9) | D | C (keymap.md), L |
| 0x1F | general (29,134) | 0x2B VISUAL_DESC | Held-item and object visual templates ([`setup.md`](setup.md) §5) | xD (u32 flags) | C (setup.md) |
| 0x20 | general (4,201) | 0x4E APPEARANCE | Appearance / wardrobe records ([`setup.md`](setup.md) §8, [`appearance.md`](appearance.md)) | D | C (setup.md) |
| 0x21 | general (20) | 0x43 UI_SCENE | UI scenes ([`misc-records.md`](misc-records.md) §4) | D | C (misc-records.md) |
| 0x22 | **local (302) only** | 0x28 UI_LAYOUT | UI layouts ([`ui-layout.md`](ui-layout.md) §4) | D | C (ui-layout.md) |
| 0x23 | general (735) | 0x4D ENUM_MAPPER | **EnumMapper** (§7) | D | C |
| 0x24 | none | 0x31 LAND_BLOCK_TERRAIN_LIST | Registered; no record in this install | | BIN |
| 0x25 | local (282,152) | 0x20 STRING_TABLE (range 0x25000000–0x26FFFFFF) | **String tables** (§3) | D | C |
| 0x26 | none observed | 0x20 STRING_TABLE (same range) | Part of the string-table range | | R, BIN |
| 0x27 | cell_1..5 (≈100 each), cell_14 (13) | 0x30 RENDER_IMPOSTER_TABLE | Per-region imposter tables; DIDs 0x27RRXXYY ([`misc-records.md`](misc-records.md) §5) | D | C (misc-records.md) |
| 0x28 | general (24) | 0x49 DID_MAPPER | **DataIdMap** (§8) | D | C |
| 0x2A | general (45,544) | 0x12 SOUNDINFO | Sound-info records ([`sound.md`](sound.md) §4; also `property-system.md` §9) | D | C (sound.md) |
| 0x2B | general (510) | 0x51 RENDERMATERIAL | Shader records ([`material.md`](material.md) §7; labels name render materials) | D | C (material.md), L |
| 0x2D | general (3) (range 0x2D000000–0x2D0FFFFF) | 0x62 COMPUTESHADER | Compute shaders (a DXBC container; [`misc-records.md`](misc-records.md) §6) | D | C (misc-records.md) |
| 0x30 | general (24,091), local (3) | 0x1F MATERIALMODIFIER | Materials ([`material.md`](material.md) §6) | D | C (material.md) |
| 0x31 | general (13,781) | 0x36 MATERIALINSTANCE | Surface records: shader and material links ([`material.md`](material.md) §4) | D | C (material.md) |
| 0x32 | general (1) | 0x3B RENDER_LOD_CLASSES | LOD-class distance table ([`environment.md`](environment.md) §9.1) | D | C (environment.md) |
| 0x33 | general (24) | 0x26 IMAGE_COMPOSITOR | Image compositors ([`misc-records.md`](misc-records.md) §3; DID-map labels such as "…_Face_Compositor") | D | C (misc-records.md), L |
| 0x34 | gamelogic (1: 0x34000000) | 0x25 MASTER_PROPERTY | **Master property dictionary** (§6) | D | C |
| 0x35 | general (1) | 0x02 GAME_TIME | Game time ([`environment.md`](environment.md) §6) | D | C (environment.md) |
| 0x39 | general (11,155) | 0x1E PSDESC | Particle-system descriptors ([`particles.md`](particles.md) §4) | – | C (particles.md) |
| 0x40 | general (81,135), local (1) | 0x0A RENDERTEXTURE | Texture lists / mip chains ([`texture-list.md`](texture-list.md)) | D | C (texture-list.md) |
| 0x41 | highres (15,371), highres_aux_1..3 (16,885 / 38,556 / 7,169), surface (443,694), surface_aux_1..2 (545,619 / 340,535), local (223) | 0x05 RENDERSURFACE | **Render surfaces** (§5) | D | C |
| 0x42 | general (3), local (45) (range 0x42000000–0x42000FFF) | 0x4C FONT | Fonts ([`fonts.md`](fonts.md) §3; DID map 0x2800000B labels font names pointing here) | D | C (fonts.md), L |
| 0x43 | gamelogic (1) (range 0x43001000–0x43FFFFFF) | 0x50 SCRIPTCHANNELS | Script channels ([`misc-records.md`](misc-records.md) §7) | – | C (misc-records.md) |
| 0x44 | general (254) | 0x56 FORKPSDESC | Fork-system descriptors ([`particles.md`](particles.md) §6) | – | C (particles.md) |
| 0x46 | none | 0x1A ENTITYGROUP | Registered; no record in this install | | BIN |
| 0x47 | gamelogic (40,643) | 0x39 ENTITYDESC | Entity descriptors ([`setup.md`](setup.md) §4) | xD (u32 flags) | C (setup.md) |
| 0x51 | general (2) | 0x24 ACTIONMAP | Action maps (labelled in DID map 0x2800000A; [`keymap.md`](keymap.md) §12.10) | D | C (keymap.md), L |
| 0x53 | none | 0x47 CONTENTDB | Registered; no record in this install | | BIN |
| 0x56 | gamelogic (1: 0x56000000) | 0x3E WLIB | Compiled script library ([`wlib.md`](wlib.md)) | D | C (wlib.md), L |
| 0x58 | local (1) (range 0x58000000–0x58000001) | 0x46 STRING_STATE | Locale record ([`string-table.md`](string-table.md) §10.3) | – | C (string-table.md) |
| 0x66 | general (16,819) | 0x0E PHYSICS_MESH | Stand-alone collision meshes named by setups ([`collision.md`](collision.md) §4) | – (VLE count first) | C (collision.md) |
| 0x70–0x77 | gamelogic (383,332 at 0x70, 11 at 0x76) | 0x45 WSTATE | Script-state objects and singletons ([`wstate.md`](wstate.md)) | D | C (wstate.md), L |
| 0x78–0x7F | gamelogic (1,616 at 0x78, 383,332 at 0x79), local (2) | 0x32 DBPROPERTIES | Property sets; 0x79 + low 24 bits = the property set of the 0x70 object (`property-system.md` §5–§6) | D | C (property-system.md), L |
| 0x80 | cell_R, map_R | per kind, §2.2 | Landblock family | D | C ([`landblock.md`](landblock.md)) |
| 0x90–0x9F | none | registered three times: 0x64 KYNPATHDATA, 0x07 LBO, 0x59 HKX | Not archive records in this install (run-time or tool ranges) | | BIN |
| 0xFF | every archive (2–5 each: 0xFFFF0001, 3, 4, 5, 8) | none registered | Archive metadata records ([`dat-container.md`](dat-container.md) §7) | – | C (dat-container.md) |

The former OPENs of this section are resolved: 0x40 and 0x78/0x79 are C (2026-10-05); the layouts of 0x14 (framing), 0x21, 0x27, 0x2A, 0x39, 0x43, 0x44, 0x58 and 0xFF are given here or in the spec named in each row (2026-10-07).

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
| 5 | cell_R | 0x41 DISTANT_LANDSCAPE_SECTOR | Distant-landscape sector, one per 3 × 3 landblocks | sparse | C as far as `landblock.md` §7 decodes it (its open fields are tracked there) |
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
| 8 | 2, 3 or 5 | TSize | Entry count *n* (§1.3) |
| … | var | Entry × *n* | §3.2 |

The table ends exactly after the last entry. There are no trailing bytes.

What the client does with the table flag (BIN2 (c), S3): its string-table reader reads the u32 right after the DID (this type has no second shared-header word, §1.6), keeps it with the loaded table and writes it back unchanged; the entry lookup and the message renderer (`string-table.md` §9.2) do not read it. The one table with flag 0 holds five plain single-piece entries and renders like any other (`string-table.md` §11). Readers keep the value for rewriting and do not interpret it; writers producing new tables write 1.

> **OPEN:** The meaning of the table flag at offset 4 (non-blocking). The client's string-table reader stores it, but neither the entry lookup nor the renderer reads it; any reader of it lies outside the routines read so far. The one table with flag 0 shows no visible difference in this install.

<a id="s3-2"></a>
### 3.2 Entry layout (Confirmed)

| Field | Size | Type | Meaning |
|---|---|---|---|
| token | 4 | u32 | Message key, unique within the table |
| token_high | 4 | u32 | **Redirect DID** (S4): 0 = none; otherwise the client renders the same token from that table instead (`string-table.md` §9.2). Always 0 in this install. KD treats token and token_high together as one u64 key, which gives the same result while the field is 0. |
| piece_count *p* | 4 | u32 | Number of literal pieces. Observed 1–12 (no entry has 0; readers still accept 0). |
| pieces | var | UTF-16 string × *p* | Literal text pieces, in order |
| arg_count *a* | 4 | u32 | Number of argument slots |
| arg_ids | 4·*a* | u32 × *a* | Argument IDs. Each is the name hash (§1.5) of the argument's variable name. |
| has_names *g* | 1 | u8 | Argument-name flag: 0 = no group, 1 = one group follows; any other value is malformed (S4). Observed: 0 or 1. |
| name group | var | group × *g* | §3.3 |

<a id="s3-3"></a>
### 3.3 Argument-name group (Confirmed)

| Field | Size | Type | Meaning |
|---|---|---|---|
| name_count *m* | 4 | u32 | Number of names |
| names | var | UTF-16 string × *m* | Variable names, one for each argument ID in the same order |

Sources disagreed on the type of the byte before the group: LC reads it as a `bool8` "has names" flag, KD as a group count. The client reads it as a flag (S4, BIN in `string-table.md` G41): 0 = no group, 1 = one group, and any other value fails the record. Readers do the same and reject values above 1 (revised 2026-10-07; the earlier text said to loop over a count, which gives the same result for 0 and 1).

<a id="s3-4"></a>
### 3.4 Rendering model

Pieces and arguments alternate, starting and ending with a piece: piece 0, argument 0, piece 1, argument 1, …, piece *p*−1. An empty string marks an empty edge piece. A message that begins with an argument has `""` as piece 0.

When a caller supplies argument values keyed by variable-name hash, the message template is assembled by concatenating the pieces and replacing each slot with the value for its `arg_id`. The same `arg_id` may appear more than once. Assembly is only the first step: the full rendering (grammar evaluation, whitespace, display cut, escapes) is [`string-table.md`](string-table.md) §7.

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
### 4.1 Layout (revised 2026-10-07; S2, S5)

This is the stored form as the client reads it (`property-system.md` §17.4, BIN there). The 2026-10-03 layout, which followed LC, gave the same result on every stored value, because every stored StringInfo has a first byte of 0 or 1, a flag byte of 0 and an empty argument table; it was wrong for every other input.

| Field | Type | Present when | Meaning |
|---|---|---|---|
| mode | u8 | always | 1 = inline text. **Any other value** = string-table reference. 0 is the normal reference; 2 means "show the second authoring string instead of looking the token up"; 3–255 resolve like 0 (`string-table.md` §9.0). The client does not check this byte like a `bool8`; readers accept every value, warn on values other than 0 and 1, and keep the byte. |
| literal | UTF-16 string | mode = 1 | Inline text |
| token | u32 | mode ≠ 1 | Entry token (§3.2) |
| table | DID | mode ≠ 1 | String-table DID (0x25…) |
| has_authoring | bool8 | always | 1 = three authoring strings follow; 0 = none. Any other value is malformed. Earlier revisions called this byte `has_args`; it has nothing to do with arguments. |
| token_name, source_text, comment | Pascal string × 3 | has_authoring = 1 | Authoring strings: a symbolic token name, the source-language text (shown only for mode 2), an authoring comment (never shown) |
| argument table | TSize, then entries | **always** | Hint byte and VLE count (§1.3), then per entry a u32 key (the variable's name hash, §1.5), a u16 argument kind and the value of that kind (§4.2) |

The two bytes `01 00` that every stored StringInfo ends with are the argument table, empty, with hint 1. There is no separate terminator, and nothing follows the argument table (this answers the §13 question about bytes after an argument block).

A writer producing new data writes mode 0 or 1, has_authoring 0, and an argument table (`01 00` when empty).

<a id="s4-2"></a>
### 4.2 Argument value

The value of each argument-table entry is specified in [`property-system.md`](property-system.md) §17.4.1 (BIN there): a 6-byte common header (u16 kind, u32 variable ID), then by kind: 1 a nested StringInfo; 2 an f64 and a u16 decimal-place count; 3 the same plus a u8 format-flag set and a u16 radix; 4 an i64; 5 an i64, a u8 format-flag set and a u16 radix; 6 and 7 stored like 4 and 5. Any other kind carries no value bytes at all. How values are turned into text is `string-table.md` §9.4. LC's reading (a VLE count with no hint byte, a u8 kind before the hash, an f32 or a VLE value) does not match the stored form.

Readers must bound recursion depth. A depth limit of 16 is suggested (`string-table.md` §7.9 uses it); no stored value nests, since none has an argument.

Invariants **SI1** (Confirmed over 6,352 enum values and 626 property defaults, all parsed exactly; `property-system.md` OBS3 extends it to all 1,110,503 StringInfos of property sets and the dictionary):

- mode is 0 or 1, has_authoring is 0, and the argument table is empty, stored as `01 00`;
- only 4 of the 6,352 enum display values are literals.

The former OPENs of this section are answered: the "three Pascal strings" are the authoring strings above, the "terminator" is the empty argument table, and the argument path is specified from the client (BIN) although no stored value exercises it.

---

<a id="s5"></a>
## 5. Render surfaces (prefix 0x41)

A render surface is one image: a texture level, an icon or a UI bitmap. It holds a **single image level**. Mip chains are separate 0x41 records listed by a 0x40 record.

<a id="s5-1"></a>
### 5.1 Header (Confirmed, 1,408,052 records)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | DID | Surface DID |
| 4 | 4 | u32 | **Category** (§5.4): the second word of the client's shared record header, which only this type carries (§1.6) |
| 8 | 4 | u32 | Width in pixels (0 for JPEG) |
| 12 | 4 | u32 | Height in pixels (0 for JPEG) |
| 16 | 4 | u32 | Format code (§5.2) |
| 20 | 4 | u32 | Data length *L* in bytes |
| 24 | *L* | bytes | Image data |

Invariant **RS1** (Confirmed, all records): 24 + *L* = payload length exactly.

How the client reads the header (BIN2 (a), R5): the shared header reader takes the DID and the category; the surface reader then takes width, height, format and *L*, describes the format (bits per pixel, channel masks, compressed or not) and, **for every format except 500**, fails the record unless *L* = (bits per pixel × height × width) / 8, computed in 32-bit unsigned arithmetic with the division truncating. Bits per pixel: 24 for code 20, 32 for code 21, 8 for code 28, 4 for DXT1, 8 for DXT3 and DXT5. A code the client does not know is described with 0 bits per pixel, so any such record with *L* > 0 fails. Then the *L* data bytes are read. Width and height are not checked for JPEG (§5.3).

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

Most dimensions are powers of two, but not all, so readers must not require it. DXT dimensions were always multiples of 4 in this install (Confirmed, invariant **RS3**). Readers should still use the ⌈·/4⌉ formula. For DXT dimensions that are multiples of 4 it equals the client's rule (*L* = W·H·4/8 for DXT1, W·H·8/8 for DXT3/5); for any other DXT dimension the two differ, and the client would reject the block-padded length, so a reader may also reject a DXT surface whose width or height is not a multiple of 4 (none exists).

<a id="s5-3"></a>
### 5.3 Pixel data

All of the following are Confirmed by rendering samples and checking colours and orientation by eye.

- **Row order**: top row first, rows left to right, no row padding.
- **20 (R8G8B8)**: 3 bytes per pixel, stored in the order **B, G, R**.
- **21 (A8R8G8B8)**: 4 bytes per pixel, stored **B, G, R, A** (a little-endian u32 `0xAARRGGBB`). Alpha is straight, not premultiplied: transparent icon backgrounds have alpha 0.
- **28 (A8)**: 1 byte per pixel, single channel. The channel is **alpha** (R7): the client's own description of code 28 is an alpha-only format (8 bits, alpha mask 0xFF, no colour channels, BIN2 (a)), and both traced consumers read the byte as an opacity value *v* / 255: terrain layer masks ([`terrain-texturing.md`](terrain-texturing.md) §5.1) and font glyph atlases, where it is ink coverage ([`fonts.md`](fonts.md) §4.1). A generic conversion to RGBA gives (255, 255, 255, *v*), the "white with alpha" view of `texture-list.md` §7.3; consumers that need the raw byte read it directly.
- **DXT1/3/5**: standard BC1/BC2/BC3 4×4 blocks in row-major block order, as in the public S3TC description. Do not flip V. See §5.3.1 for how the client decodes them.
- **500 (JPEG)**: the *L* bytes are a complete JPEG stream starting `FF D8 FF` (64,405 of the 65,706 carry a JFIF APP0 segment; the others start with another application segment). Width and height in the header are 0. Take the dimensions from the JPEG. See §5.3.2.

<a id="s5-3-1"></a>
#### 5.3.1 Block-compressed surfaces (R6)

- **Who decodes** (BIN2 (b)): the client never decodes BC1/BC2/BC3 blocks itself on the paths read. When it builds a texture from a surface it creates the device texture with the surface's own format code and copies compressed data unchanged; the graphics device decodes the blocks. So the reference decoder is the device's, as defined by the public block-compression description (D3D10), not a routine of the client.
- **BC1 three-colour mode**: in a BC1 block whose first endpoint (as a u16) is not greater than the second (c0 ≤ c1), index 2 is the midpoint of the two endpoints and index 3 is **transparent black** (0, 0, 0, 0). Decoders produce exactly that; they do not treat index 3 as opaque black. In this install (OBS7) 67,098,804 of the 568,358,856 DXT1 blocks are in three-colour mode (30,105,879 of them with c0 = c1), and 1,181,234 blocks in 1,429 surfaces actually use index 3. Whether a material then discards those texels (cut-out) or blends them is the material's business (`material.md`), not the surface's.
- **BC2/BC3 colour blocks** always use the four-colour mode, whatever the order of c0 and c1 (D3D10). 58,402 of the 1,542,008,703 DXT3/DXT5 colour blocks have c0 < c1; they decode in four-colour mode.
- **Rounding**: the public description defines the interpolants as 2/3·c0 + 1/3·c1, 1/3·c0 + 2/3·c1 and (three-colour mode) 1/2·c0 + 1/2·c1, and the BC3 alpha interpolants in sevenths and fifths, and it does not fix one rounding of them: devices are allowed to deviate slightly from the exact fractions. Since the client leaves decoding to the device, there is no client rounding to match. Tessera's rule: round to nearest from the exact fractions, with 5- and 6-bit endpoints expanded to 8 bits by bit replication (the choice `crates/formats` already made, §13). Real-data tests that compare against another decoder allow ±1 per 8-bit channel.

<a id="s5-3-2"></a>
#### 5.3.2 JPEG surfaces (R8)

- **Decoding in the client** (BIN2 (b)): the client decodes the stream with an embedded JPEG library (the library's own message strings are present in the executable). It takes width and height from the decoded image; **the header's width and height are not used** and are not checked when the record is loaded (§5.1). The image must have exactly three components: a stream that reads as any other component count fails, and the surface yields no texture. The decoded pixels become an R8G8B8 image (code 20), 3 bytes per pixel.
- **Tessera rule**: a JPEG surface whose header width or height is non-zero is accepted, the value is ignored (a debug log is enough), and the dimensions come from the stream. A stream that decodes to other than three components is reported as unsupported.
- **Stream census** (OBS7, all 65,706): every stream starts with `FF D8`, ends with the end-of-image marker `FF D9` as its last two bytes, and has its first frame header after nothing but length-prefixed marker segments: no stand-alone marker (`FF 00`, `FF 01`, `FF D0`–`FF D7`, a second `FF D8`) and no stray byte occurs between the start marker and the first frame header. The first frame header is baseline (`FF C0`, 65,591) or progressive (`FF C2`, 115); no other coding process occurs. Every frame has 8-bit precision and **three components** (65,706 of 65,706), so the client's three-component rule never fails on stored data. 2,498 streams contain further start-of-image and frame-header byte pairs, all of them inside APP1 or APP13 application segments (embedded thumbnails), which a segment walk skips. So a pre-scan that walks segments by their lengths reaches the same frame header the decoder uses, and the conservative rejections of `crates/formats` (§13) reject nothing in this install.

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

What the client does with the category (BIN2 (a), (b); R9). It is read as the second word of the shared record header (§1.6). Two places test it:

- When a surface record is loaded, the client sets an internal flag on the surface when the category is 13 or 15. What that flag changes was not traced.
- When a texture is built from an **uncompressed** surface (codes 20, 21, 28, and JPEG after decoding) whose category is 15, the client may build it in a different pixel format chosen by the graphics back end, under a renderer setting. Surfaces of every other category, and block-compressed surfaces, are always built in their own format.

No other category value is tested in the routines read. 13 is used only by font glyph atlases (`fonts.md` §4.1) and 15 by UI images and by every JPEG (`texture-list.md` §7.2, which also says that a texture list never mixes categories).

> **OPEN:** What the category values mean beyond the two tests above (non-blocking; decoding and drawing do not need them). The values other than 13 and 15 have no traced effect in the client, and the effect of the load-time flag for 13 and 15 was not traced. A Tessera reader keeps the value and gives it no meaning.

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

The names and the display texts are two separate lists, and each is looked up along the chain on its own (E3, `property-system.md` §17.5): a key whose name is in this mapper but whose display text is only in a base mapper gets that base display text, and the other way round.

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

Every record in this install has **exactly two groups** (Confirmed, OBS7, E4):

- The first group is the main table.
- The second group is empty in 17 of the 24 records. It holds 1 entry in 0x28000000, 0x28000002, 0x2800000D, 0x2800000E and 0x28000011, 2 in 0x2800000C, and 122 in 0x28000001 (the enum-mapper map). (The first version of this spec said "0, sometimes 1–3"; the 122-entry group was missed.)
- No key of a second group occurs in the first group of the same record (129 of 129).
- 77 of the 129 second-group targets exist in `client_general.dat`, `client_gamelogic.dat` or `client_local_English.dat` (0x28000001's 122 targets are 119 enum-mapper DIDs, prefix 0x23, and 3 DID-map DIDs, prefix 0x28; 73 of those enum mappers and all 3 DID maps exist, 76 of 122. The 77th is the one 0x70 target of 0x2800000E); the others name records that this install does not ship.

How the client uses the groups (BIN2 (d), E4): a loaded DID map holds two key → DID tables. To look a key up, the client searches one table and, if the key is not there, the other; the first hit wins. It does this at both steps of a two-step lookup (root map key → sub-map DID, then sub-map key → target DID). So entries of either group are equally visible to the client. The order of the two searches matters only for a key present in both groups, which no record has; which group is searched first was not traced.

Reader rule: merge both groups into one key → DID table (and one key → label table). If a key occurs in both groups, keep the first group's entry and warn (this never happens in this install). Within a group, a key that has no label is still a valid map entry; the label is only a name for it. A missing target is a lookup miss, not a parse error.

The former OPEN of this section ("what distinguishes the second group") is answered as far as the client is concerned: nothing; it looks in both. Why an authoring tool put an entry in the second group is not visible in the client and is not needed.

<a id="s8-2"></a>
### 8.2 Root map

0x28000000 is the root. Its labels name the other DID maps, and resolving them gives the "L" rows of §2.1. Examples (key, label, target in this install):

| Root key | Label | Resolves to |
|---|---|---|
| 0x12 | STRINGTABLE | 0x28000004, named string tables (its key 0x0B, `Language`, is the locale record 0x58000000, `string-table.md` §10.3) |
| 0x07 | EMAPPER | 0x28000001, named enum mappers |
| 0x10 | FONT | 0x2800000B |
| 0x04 | UILAYOUT | 0x28000003 |
| 0x0A | WEENIECONTENT | 0x2800000E |

**Keys and labels** (BIN2 (d), E4): the client itself addresses DID maps **by key number**: its lookups take a root key and a sub-map key, and none of the root labels (for example `STRINGTABLE`, `EMAPPER`, `UNIQUEDB`, `WEENIECONTENT`, `ACTIONMAP`) occurs in its executable, as ASCII or as UTF-16. The labels are data for tools. Both are stable within an install; Tessera may resolve a map by label (as the other specs describe) or by key, but must read the target DIDs at run time and must not hard-code them.

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
| 8+*S* | 4 | u32 | **Storage format** ([`sound.md`](sound.md) §3.2): 2 = keep the data as stored (Ogg decoded while playing), 3 = use as PCM (an Ogg payload is decoded in full when the record loads), 7 = not audio (the DMAP records). Observed 2, 3, 7. |
| 12+*S* | 4 | u32 | **Reserved word**: 0 in every record. The client reads it as a 32-bit word and keeps it (sound.md §3.1). Writers store 0. |

Invariant **SN1** (Confirmed): payload length = *S* + 16. Every observed 0x0A record is stored uncompressed.

How the trailer u32 relates to the container:

| Trailer u32 | Containers observed with it |
|---|---|
| 2 | Ogg and one WAV |
| 3 | Ogg and WAV |
| 7 | Only the DMAP records |

So the trailer is not a codec ID: it is a playback hint (A1, from `sound.md` §3.1–§3.2). Always classify the container by its magic bytes (§9.2). The earlier reading of the last word as an f32 0.0 is the same four zero bytes; it is a u32. Whether the reserved word has any meaning is tracked in `sound.md` §3.1 (no playback code reads it).

LC also describes a "sound info" record (DID, flags, 5 × f32, u32 sound DID, optional Pascal name). No 0x0A record has that shape: sound-info records are prefix 0x2A (DB type 0x12 SOUNDINFO, §2.1), and their layout is [`sound.md`](sound.md) §4 (A3; the name is present in all 45,544, not optional).

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

These were found against `research/notes/dat-format.md` (DF) and `research/notes/data-semantics.md` (DS); X16 and X18–X22 correct this spec's own earlier text.

| # | Note says | Observed (2026-10-03) | Resolution |
|---|---|---|---|
| X1 | DF §6: "0x22 (EnumMapper)" | 0x23 records decode as enum mappers; 0x22 records exist only in local | DF is wrong. 0x23 = EnumMapper, 0x22 = UI layouts (DS is right). |
| X2 | DF §6: 0x22 is in general, then local | No 0x22 in general; 302 in local | Fact corrected; routing still tries both |
| X3 | DF §7 string table: "u64 token", then "u8 nArgStringGroups, each {u32 n; …}". DS §4.5: "bool hasNames" | The byte is 0 or 1. The token's upper u32 is always 0. | The two readings agree on all data. **Superseded by X22** (2026-10-08): §3.3 now reads the byte as a flag and treats values above 1 as malformed |
| X4 | DF/DS: TSize hint is a "bucket-count hint" with unknown meaning | Hint indexes a prime bucket table; elements are stored in bucket order | New fact (§1.3) |
| X5 | DS §4.1: 0x34000000 is "skip 8, TSize names, skip 2, TSize defs" | Exactly right; consumes the whole record | Confirmed. The skipped bytes are zero in this install. |
| X6 | DS §4.1: the definition's `data` is "enum DID / bitfield enum DID" | Also 0x28 (DID map) for type 8, and 0x34000000 for type 10 | Extended (`property-system.md` §3) |
| X7 | DS §6: "a region has 256 × 256 landblocks" | 255 × 255 (X, Y ∈ 0..254) | Corrected (L1) |
| X8 | DS §1: 0x2701/0x2702 cell DIDs are an old-client artefact | 0x27RR… DIDs are present in today's cell_1..5 and cell_14 | Corrected: they are render imposter tables (DB type 0x30, `misc-records.md` §5) |
| X9 | DS/DF list cell/map regions 1–5, 14 | cell_15 and map_15 archives also exist, holding only metadata | Noted |
| X10 | DS §13: font type ID unknown | 0x42 (DID map FONT labels point there) | New (L) |
| X11 | DS §1: 0x0A holds "SoundInfo records plus raw OGG/WAV" | Every 0x0A record has the raw envelope. 8 are DirectMusic DMAP, not audio. | Corrected (§9) |
| X12 | DF §6: 0x30 and 0x40 are general-only | 3 × 0x30 and 1 × 0x40 also live in local | Noted; routing should fall back to local |
| X13 | DS §7.4: 0x41 layout lists only formats | The JPEG header has width = height = 0, and each record is one level with no mips | New (§5) |
| X14 | DF §5: subfile payloads "usually" have a class index at +4 | True for some types only. For 0x41, +4 is a category; for 0x25 a flag; for 0x23 a base DID. | Type-specific |
| X15 | DS §1 lists the 0x41 search order across archives | No DID is duplicated between those archives in this install, so order does not change the result today | Noted |
| X16 | P10 / §1.6 (this spec, 2026-10-03): every record begins with its own DID | 0x02, 0x11, 0x15, 0x1A, 0x1F and 0x47 begin with a u32 before the DID; 0x05, 0x07, 0x08, 0x0C, 0x39, 0x43, 0x44, 0x58, 0x66 and some 0x0E have no DID at the start | Corrected (§1.6, 2026-10-05) |
| X17 | `environment.md` first look: kinds of 0x11, 0x12, 0x16, 0x17, 0x19, 0x02, 0x32 by content | The client's registration gives SCENE_DESC, TERRAIN_DESC, WATER_DESC, FOG_DESC, TERRAIN_TYPE_TABLE, SCENE, RENDER_LOD_CLASSES | Corrected (§2.1) |
| X18 | §1.2 (this spec, after LC): a VLE first byte of 0xE0 is a 5-byte escape | The client has no escape; 0xE0 starts the 4-byte form (P2, BIN in `property-system.md` §17.1) | Corrected (§1.2, 2026-10-07) |
| X19 | §4 (this spec, after LC): `has_args` flag, terminator `01 00`, argument kinds 1/2/4 with a u8 kind and a marker byte | The flag announces authoring strings; `01 00` is an empty argument table that always follows; seven argument kinds with a u16 kind and a 6-byte header (S5) | Corrected (§4, 2026-10-07) |
| X20 | §8.1 (this spec): the second DID-map group holds 0–3 entries | 0x28000001's second group holds 122 entries (OBS7) | Corrected (§8.1, 2026-10-07) |
| X21 | §9.1 (this spec, after LC): the last word of a 0x0A record is an f32 | It is a reserved u32 (0 in all); the word before it is the storage format (A1) | Corrected (§9.1, 2026-10-07) |
| X22 | §3.3 (this spec, after KD): read the name-group byte as a count | The client reads it as a flag and fails values above 1 (S4) | Corrected (§3.3, 2026-10-07) |

---

<a id="s11"></a>
## 11. Open questions (summary)

Remaining (2026-10-07):

1. String-table flag at +4: meaning; the client keeps it but its lookup and renderer do not read it (§3.1). Non-blocking.
2. Surface category: meaning beyond the two tests the client makes (13/15 at load, 15 when building a texture from an uncompressed surface) (§5.4). Non-blocking.
3. Encounter tables (0x14): roles of the header words and of the 17 bytes of each slot (§15). Not needed offline.
4. Moved elsewhere, not tracked here: 0x34000000 reserved bytes and definition fields (`property-system.md` §13); the reserved word of 0x0A records (`sound.md` §3.1); the open fields of landblock kind 5 (`landblock.md` §7).

Answered on 2026-10-07 (kept for traceability, with the section that now holds the answer): VLE `0xE0` (no escape, §1.2); order within a hash bucket and bucket counts for hints 13–22 (§1.3); string-piece grammar (§1.4 → `string-table.md`); StringInfo authoring strings, the "terminator" and the argument path (§4); A8 semantics and BC1 three-colour mode (§5.3, §5.3.1); DID-map second group (§8.1); 0x0A trailer and sound-info records (§9.1); the layouts of 0x14 (framing, §15), 0x21, 0x27, 0x2A, 0x39, 0x43, 0x44, 0x58 and 0xFF (§2.1, D4). (0x08 was answered on 2026-10-06, `walker.md` §11.5.)

<a id="s12"></a>
## 12. Validation guidance for implementers

Real-data tests (skipped when no install is configured) should assert the **invariants** above, not the observed counts, because counts change with every game patch. Synthetic fixtures should cover:

- every VLE form, a first byte of `0xE0` decoded as the 4-byte form (V3), and truncation at each byte;
- TSize with hint 0, with hint 22 (accepted) and with hint 23 (malformed);
- a string table with *p* = 0, with *p* = 1, with an argument-name group, and with a name-group flag of 2 (malformed);
- a StringInfo of mode 2, one with authoring strings, one with each argument kind 1–7 and one with an unknown kind (no value bytes); a nested StringInfo up to the depth limit, plus one level past it;
- one surface of each format code, with a wrong *L* to check rejection; a JPEG surface with a non-zero header width (accepted, ignored); a BC1 block in three-colour mode using index 3 (decodes to (0, 0, 0, 0));
- a property definition with default, minimum and maximum present, for each value type;
- a DID map with two groups, a key found only in the second group, and mismatched counts;
- a sound record of each container kind, plus a truncated one;
- an encounter table of the §15 framing with zero entries, and one truncated inside a slot.

<a id="s13"></a>
## 13. Open items raised by the `formats` implementation (2026-10-04)

These came up while implementing `crates/formats` from this spec alone. Each records the conservative choice the implementation made, so a spec writer can confirm or correct it. All are answered; the questions are kept, relabelled, so that the history stays readable.

> **Question (2026-10-04):** BC1/BC2/BC3 palette rounding. §5.3 says "standard S3TC" but does not fix the rounding of the 1/3, 2/3 (colour) and 1/7, 1/5 (alpha) interpolants, which differs between decoders by ±1. The implementation rounds to nearest, `(w0·a + w1·b + d/2) / d`, and expands 5/6-bit endpoints by bit replication. DXT3 and DXT5 colour blocks always use four-colour mode, whatever the order of c0 and c1.
>
> **Answered (2026-10-07):** the client does not decode blocks itself; the graphics device does, and the public description lets devices differ slightly in rounding, so there is no client rounding to match. The implementation's choices (round to nearest, bit-replicated endpoints, four-colour mode for DXT3/DXT5) are the specified Tessera rule (§5.3.1, R6).

> **Question (2026-10-04):** A8 (code 28) to RGBA conversion. Until §5.3's A8 question is answered, `Surface::to_rgba8` maps A8 to `(255, 255, 255, a)` and leaves the raw channel available to callers.
>
> **Answered (2026-10-07):** the A8 byte is alpha (R7, §5.3). `(255, 255, 255, a)` is the specified generic conversion; keeping the raw channel available is right.

> **Question (2026-10-04):** Property type 21 values are read as `bool8` (0 or 1 only, §1.1), so any other byte is a parse error. §6.4 says "u8 (0/1)", which may mean other values are possible but unobserved. Please confirm whether readers should reject them.
>
> **Resolved (2026-10-04):** only 0 and 1 occur, in 6,258,467 set values and 2,003 dictionary values. Strict `bool8` is safe. See `property-system.md` §4.5.

> **Question (2026-10-04):** §6.2 says the two child-PID copies are always equal, but not what a reader should do when they differ. The implementation keeps the first copy and logs at debug level.
>
> **Resolved (2026-10-04):** keeping the first copy is the specified behaviour; copies never differ in observed data. See `property-system.md` §2.2.

> **Question (2026-10-04):** §4.1 has a 2-byte terminator only when `has_args` = 0. The implementation reads nothing after the argument values when `has_args` = 1. Because that path has not been exercised on data (§4 OPEN), please confirm that no terminator follows the argument block.
>
> **Answered (2026-10-07):** the layout was different from what §4.1 said: the flag byte announces three authoring strings, and an argument table always follows it, whatever the flag; the old "terminator" is that table when empty. Nothing follows the argument table (§4.1, S5). The decoder must be changed to the §4.1 layout; on stored data the result is the same.

> **Question (2026-10-04):** Property type 17 (Position) has no stored encoding in §6.4. A definition of type 17 with a default, minimum or maximum flag set makes the dictionary decoder fail with "unsupported property type", since it cannot skip a value of unknown length. Observed data has no such definition.
>
> **Resolved (2026-10-04):** the position encoding is now specified in `property-system.md` §4.6.

> **Question (2026-10-04):** A 0x41 JPEG surface (code 500) whose header width or height is non-zero is accepted. The implementation logs it and takes the dimensions from the JPEG stream. §5.1 does not say whether this should be an error.
>
> **Answered (2026-10-07):** not an error. The client ignores the header width and height of a JPEG surface and takes both from the stream (§5.3.2, R8). Keep the implementation's behaviour.

> **Question (implementation choice, 2026-10-06):** before decoding an embedded JPEG, the implementation walks its marker segments to the frame header to bound the decoder's memory (progressive frames need coefficient buffers for the whole image). It rejects as malformed a stream that has a marker code 0x00, 0x01, 0xD0–0xD7 or 0xD8 before the frame header (JPEG decoders step over these in different ways, so a crafted stream could show the pre-scan a decoy frame), and a stream whose first frame header's width and height differ from the frame the decoder reads. A real JFIF stream has none of these before its frame header. §5.3 does not say whether such streams occur; please confirm none do.
>
> **Answered (2026-10-07):** none occur. All 65,706 stored streams have only length-prefixed segments between the start marker and the first frame header; the further start and frame markers that some streams contain sit inside application segments, which the walk skips (§5.3.2, OBS7). The rejections reject nothing in this install and stay as Tessera's hostile-input rule.

> **Question (2026-10-04):** MP5 (§6.5) does not hold for type 20 (DID reference). Every type-20 definition in this install (1,849 of 1,849) has a non-zero `data` field below 0x01000000. Example: PID 16 has `data` = 0x45. These values look like a code for the expected DID type (perhaps a DID-type index, but not the §2 high-byte prefix itself), not a zero. Found by `crates/formats/tests/real_data.rs` on 2026-10-04. The spec writer should determine what this value means and correct MP5 and the §6.3 table.
>
> **Resolved (2026-10-04):** the value is a DB-object-type code, labelled by the enum mapper that the EMAPPER DID map names "DBOType". 0x45 is the game-object (WState) type, whose references point at 0x70 DIDs. MP5 is corrected in `property-system.md` §2.5, and the code table is in §9.

<a id="s14"></a>
## 14. Change log

| Date | Change |
|---|---|
| before 2026-10-05 | First version 2026-10-03 and later edits; no change log was kept (see the repository history). |
| 2026-10-05 | Provenance: BIN row and D2; P10 corrected. §1.6 rewritten: record prologues per prefix (census of 8 archives); the claim that every record starts with its own DID is withdrawn (false for 0x05, 0x07, 0x0C, 0x66 and others). §2.1 rewritten: DB type of every prefix from the client's DID-range registration (adds the 0x15/0x1A/0x35 pairings, 0x66 = PHYSICS_MESH, 0x2A = SOUNDINFO, 0x16/0x17 = WATER_DESC/FOG_DESC, and all others), prologue column, statuses updated to the specs that decode each prefix (0x40, 0x78/0x79 now C as the former OPEN asked). §2.2: DB type per landblock kind; K = 5 is the distant-landscape sector; K = 6 registered but absent. §9.1 OPEN partly answered (sound info = 0x2A). §10: X16, X17. §11 item 10 updated. This change log added. |
| 2026-10-06 | Alignment with [`walker.md`](walker.md) §11.5, no other change; status stays Accepted. §2.1: the 0x08 row no longer says "layout OPEN"; it points to `walker.md` §11.5, status BIN → C (walker.md). §1.6: the "–" row names the 0x08 prologue (camera entity's 0x47 DID) and drops 0x08 from the OPEN layouts. §11 item 10: 0x08 removed from the OPEN list. Provenance: OBS6 and D3 added (11 of 11 records re-checked against `walker.md` §11.5.1 and the per-type record sizes). |
| 2026-10-07 | OPEN-closing revision; status Accepted → **Revised**. Provenance: sources BIN2, OBS7, the companion-spec row and D3D10; rows P2 and A1 corrected; P5a, P11, D4, S3–S5, R5–R9, E3, E4, A3, N1 added; S2 marked superseded in part. **Behaviour changes (implementers re-check):** §1.2 (no 0xE0 escape: 0xC0–0xFF is always the 4-byte form; V3; writer limit), §1.3 (bucket counts for hints 13–22; hint ≥ 23 malformed; order within a bucket; T1 revised), §3.2–§3.3 (`token_high` is a redirect DID; the name-group byte is a flag and values above 1 are malformed), §4.1–§4.2 (StringInfo stored form: mode byte, authoring flag and strings, an argument table that always follows; argument values per `property-system.md` §17.4.1), §5.1 (the client's size rule; RS3), §7.1 (names and display texts looked up independently along the base chain), §8.1 (second-group census; merge both groups, a label-less key stays valid), §9.1 (the trailer is a storage-format u32 and a reserved u32). **New facts without a behaviour change:** §1.4 (grammar → `string-table.md`), §1.6 (shared header; prologue sources for 0x39, 0x43, 0x44, 0x58, 0x0E, 0xFF), §2.1 (rows 0x0E, 0x14, 0x1D, 0x21, 0x22, 0x27, 0x2A, 0x2D, 0x33, 0x39, 0x42, 0x43, 0x44, 0x51, 0x58, 0xFF now point to the specs that decode them), §2.2 (kind 5 defers to `landblock.md` §7), §3.1 (what the client does with the table flag), §5.3 with new §5.3.1 (A8 is alpha; block data decoded by the device; BC1 three-colour index 3 is transparent black; rounding rule) and §5.3.2 (JPEG decoding and stream census), §5.4 (the two category tests), §8.2 (root keys; the client addresses maps by key; labels are tool data), §10 (X8 resolved; X18–X22), §11 and §12 rewritten, §13 questions answered and relabelled, new §15 (encounter-table framing). OPENs closed: §1.2, §1.3 (two), §1.4, §4 (two), §5.3 (two), §8.1, §9.1 (two), §13 (five: rounding, A8, terminator, JPEG header, JPEG markers). OPENs kept and narrowed: §3.1 table flag, §5.4 category. New OPEN: §15 field roles. |
| 2026-10-08 | Review fixes, no behaviour change; status stays **Revised**. §8.1 bullet 4: the second-group target breakdown of 0x28000001 corrected (119 enum-mapper and 3 DID-map targets; 73 and 3 exist; total 77 of 129 unchanged; re-checked against OBS7 data). §1.3: the unsourced statement about a reader mode that refuses bucket counts above 50,000 removed; the advice to stay at hint ≤ 12 stands on the data alone. §10 X3: resolution marked superseded by X22. |

<a id="s15"></a>
## 15. Encounter tables (prefix 0x14, DB type 0x13 ENCOUNTER_DESC)

Region descriptors name one encounter table each in slot 3 ([`environment.md`](environment.md) §3–§4). The encounter pass itself is out of scope offline (`scenery.md` §7.7); this section only lets a reader walk the bytes, so that a census or a viewer can show the records.

<a id="s15-1"></a>
### 15.1 Framing (Confirmed, N1: 5 of 5 records parsed to the last byte, 70 slots)

| Order | Field | Type | Observed |
|---|---|---|---|
| 1 | did | DID | Own DID (prologue D, §1.6) |
| 2 | word_a | u32 | 0 in all 5 |
| 3 | word_b | u32 | 6 in all 5; `environment.md` §4 identifies it as the block-map key the table reads (`landblock.md` §5.6) |
| 4 | entry_count *n* | u32 | 16 in 0x14000000, 1 in the other four |
| 5 | entries | entry × *n* | Below |

Entry:

| Order | Field | Type | Observed |
|---|---|---|---|
| 1 | key | u32 | 0, 1, …, *n* − 1 in stored order |
| 2 | name | Pascal string (§1.4) | An authoring name; entry 0 of every table carries the same name |
| 3 | slot_count *m* | u32 | 1, 5, 7, 12 or 13 |
| 4 | slots | slot × *m* | Below |

Slot (21 bytes):

| Offset | Size | Type | Observed |
|---|---|---|---|
| 0 | 4 | u32 | Slot key: 0, 1, …, *m* − 1 in stored order |
| 4 | 17 | bytes | The same 17 bytes in all 70 slots: `00 00 00 00 00 00 80 3F 01 00 00 00 00 00 00 00 00` |

The counts are plain u32 values, not TSize headers (no hint byte), and the record ends exactly after the last slot.

Invariants **EN1** (Confirmed): parsing consumes the payload exactly; entry keys and slot keys run from 0 upwards in stored order.

> **OPEN:** The roles of `word_a`, of `word_b` beyond `environment.md` §4's reading, and of the 17 bytes of a slot. The bytes `00 00 80 3F` at slot offsets 8–11 read as the f32 1.0 if the 17 bytes begin with one u32 field, but the data has no variation that would fix the field boundaries, and the client's reader for this type was not located in the routines read for this revision (BIN2). Not needed offline; a reader keeps the 17 bytes raw.
