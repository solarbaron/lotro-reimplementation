# Property system: dictionary, values and property sets

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Companion specs** | `docs/specs/records.md`: primitives (§1: VLE, TSize and bucket order, strings), DID high-byte table (§2), StringInfo (§4), EnumMapper (§7), DataIdMap layout (§8). `docs/specs/dat-container.md`: archives and DID routing. This spec starts with a fully reassembled, decompressed record payload. |

## Purpose

Nearly all gameplay data is stored as **properties**: (property ID, value) pairs whose value type comes from one global dictionary. This spec gives an implementer of the `formats` and `props` crates what they need to:

- decode the master property dictionary, DID 0x34000000 (§2). This content was moved here from `records.md` §6, which now only points here;
- know every property type code and its stored value encoding (§3, §4);
- decode property-set records, DID prefixes 0x78–0x7F, including nested structs and arrays (§5);
- find the property set of a game object with the "+0x09000000" rule (§6);
- turn raw values into readable ones: enum labels, bit labels, StringInfo text, property names, DID-reference types (§7);
- navigate the root DID map to the named sub-maps (§8);
- understand the DB-object-type codes that type-20 definitions carry (§9).

§10 describes, as Reported only, the network form of property sets. §11 reports the whole-install validation run. §12 lists disagreements with the research notes, with `records.md` and with published tools. §13 collects the open questions, and §14 gives test guidance.

## Provenance

How sources are marked:

- **Confirmed**: decoded on the user's own install on 2026-10-04 with a throwaway probe in the dirty-room scratchpad. Each Confirmed fact says what was decoded and how much of it.
- **Reported**: comes only from published third-party material and was not, or could not be, checked against data.
- **Analogue**: a name or meaning suggested by the Asheron's Call lineage of the engine. It is useful vocabulary and nothing more.

Source abbreviations:

| Key | Source |
|---|---|
| OBS | Observed in the user's install, 2026-10-04 (Steam install, client data current on that date). Every 0x78 and 0x79 record in `client_gamelogic.dat` and `client_local_English.dat` was decoded in full (384,950 records, 47,680,867 property values), as were the 0x34000000 record, every 0x23 enum mapper and every 0x28 DID map. A census found no 0x7A–0x7F record in any archive. |
| OBS-R | `records.md` observations of 2026-10-03, re-run unchanged on 2026-10-04 for the dictionary. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.x, Apache-2.0 per its distribution's LICENSE. Studied as a binary, decompiled for facts only. |
| MK | Mirarkitty `lotro_extractor` (Apache-2.0) and `lotro_mesh_viewer` (MIT). Their property-set reader is a port of LC's. Facts and field names only. |
| ACE | ACEmulator `ACE.DatLoader` (AGPL-3.0), an Asheron's Call data loader. Field names only (Analogue). |
| DS | `research/notes/data-semantics.md` §4. |
| ER | `research/notes/engine-client-re.md` §8–§9 (client memory and cache observations). |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| M1 | 0x34000000 record layout (§2.1) | LC, MK, DS; OBS-R (2,534,594 bytes, decoded to the last byte) | Confirmed |
| M2 | Definition layout (§2.2) | LC, MK; OBS-R (27,546 definitions) | Confirmed |
| M3 | Descriptive names of definition fields (group, provider, patch flags, prediction timeout, inheritance, dat-file type, propagation, caching) | MK, ACE | Analogue / Reported. The layout itself is Confirmed. |
| M4 | A type-20 definition's `data` field is a DB-object-type code (§9) | OBS (7,390,369 DID values checked against the code); ER (cache type IDs match the same numbering) | Confirmed. This corrects `records.md` MP5. |
| T1 | Type codes 1–22 (§3) | LC, DS, ER; OBS | Confirmed |
| V1 | Value encodings of codes 1, 6–16, 18–22 (§4) | LC, MK; OBS (values in sets and in dictionary defaults) | Confirmed |
| V2 | Codes 2 and 3 (subtypes 1 and 3) | LC; OBS (14 and 129 values in sets; 44 waveform defaults) | Confirmed. Waveform subtypes 0 and 10 are Reported. |
| V3 | Codes 4 and 5 | LC; OBS (dictionary defaults only, 4 each) | Confirmed in defaults; no value occurs in any set |
| V4 | Position (code 17) layout (§4.6) | LC, MK; OBS (6,115 values, all consumed exactly, all fields plausible) | Confirmed. Flag 0x04 (instance) is Reported, because it never occurs. |
| V5 | Struct value header is a TSize-style bucket hint plus count, and fields are stored in bucket order (§4.10) | OBS (2,799,313 structs). Derived by this spec writer. LC and MK skip the first byte without interpreting it. | Confirmed |
| V6 | Array value: u32 count, then elements with a single PID (§4.11) | LC, MK, DS; OBS (1,386,117 arrays) | Confirmed |
| V7 | Bit *i* of a bitfield is labelled by enum key *i* + 1 (§7.3) | LC; OBS (1,062,289 set bits, all labelled) | Confirmed |
| S1 | Property-set record layout (§5) | LC, MK, DS; OBS (384,950 of 384,950 records exact, zero trailing bytes) | Confirmed |
| S2 | Top-level properties are in bucket order (§5.4) | OBS (384,950 records) | Confirmed |
| S3 | Null entry (PID 0) | LC | Reported. Never observed. |
| R1 | Object property set at DID + 0x09000000 (§6.2) | LC, DS; OBS (the serial sets of 0x70 and 0x79 are identical, 383,332 each) | Confirmed |
| D1 | Root DID map labels and sub-maps (§8) | OBS (all 24 DID maps) | Confirmed |
| N1 | Network property-set encoding (§10) | LC, DS | Reported |

---

<a id="s1"></a>
## 1. Concepts

| Term | Meaning |
|---|---|
| **PID** | Property ID, a `u32`. Two families occur: values below 0x10000000, and 0x1000xxxx (bit 28 set). PID 0 means "invalid". |
| **Definition** | The dictionary entry for a PID: type code, a type-dependent `data` reference, flags, optional default/min/max, and a child list (§2.2). |
| **Value** | The stored encoding of one property, chosen by the definition's type code (§4). A reader cannot skip a value without the dictionary. |
| **Property set** | A collection of (PID, value) pairs, with each PID at most once (§5). |
| **Property-set record** | A record whose whole payload is one property set: DID prefixes 0x78–0x7F (§6). |
| **Embedded property set** | The same encoding appearing inside another record type (landblock info, UI layouts, entity descriptors, and others). Each of those container specs points here. |

Decoding is **dictionary-driven**. A client loads 0x34000000 once at start-up. After that, every property set in every record is decoded by one generic routine that looks up each PID's type code. Nothing is hard-coded per record type.

---

<a id="s2"></a>
## 2. Master property dictionary (DID 0x34000000, gamelogic)

This record lists every PID with its name, value type and constraints. In this install it is stored **uncompressed** and is 2,534,594 bytes long. (Content moved from `records.md` §6.)

<a id="s2-1"></a>
### 2.1 Record layout (Confirmed)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | did | DID | 0x34000000 |
| 2 | reserved | 8 bytes | All zero (observed) |
| 3 | name_count *n* | TSize | Observed *n* = 27,547, hint 11 |
| 4 | names | (u32 pid, Pascal string name) × *n* | PID → name, in bucket order (`records.md` §1.3.1) |
| 5 | separator | 2 bytes | Observed `00 00` |
| 6 | def_count *d* | TSize | Observed *d* = 27,546, hint 12 |
| 7 | definitions | (u32 pid, definition) × *d* | §2.2, in bucket order by pid |
| 8 | *(end)* | | Observed: zero leftover bytes |

Names are internal identifiers in Latin-1. They are not localized and are not shown to players.

> **OPEN:** The meaning of the 8 reserved bytes and of the 2-byte separator.

<a id="s2-2"></a>
### 2.2 Definition layout (Confirmed)

The "Analogue name" column gives the names MK and ACE use. They are guesses from the engine lineage, not facts about LOTRO data.

| Field | Size | Type | Meaning | Analogue name |
|---|---|---|---|---|
| pid_echo | 4 | u32 | Equals the pid that precedes the definition | property name |
| type | 4 | u32 | Property type code 1–22 (§3) | property type |
| field_a | 4 | u32 | Unknown. 353 distinct values. 25,914 definitions hold a value that is itself a defined PID, 75 hold 0. | property group |
| field_b | 4 | u32 | Unknown. Observed 3, 4, 6, 7, 8, 9; 4 in 23,331 definitions. | property provider |
| data | 4 | u32 | Type-dependent reference (§3), else 0 | data |
| field_c | 4 | u32 | 0, or 16 (19 definitions) | patch flags |
| flags | 4 | u32 | Bit 0x800: default present. Bit 0x1000: minimum present. Bit 0x2000: maximum present. Other bits are listed in §2.3. Bits 16–31 were always 0. | |
| default | var | value | If flags & 0x800 and type ∉ {10, 11, 12} | |
| minimum | var | value | If flags & 0x1000 and type ∉ {10, 11, 12} | |
| maximum | var | value | If flags & 0x2000 and type ∉ {10, 11, 12} | |
| marker | 4 | u32 | Always 0x3FC00000 (the bit pattern of f32 1.5) | prediction timeout (f32) |
| field_d | 5 | 5 × u8 | Five small bytes (§2.3). The most common pattern is `02 02 04 00 01`. | inheritance, dat-file type, propagation, caching, (unnamed) |
| child_count *c* | 1–5 | VLE | Observed max 95 | |
| children | 8·*c* | (u32, u32) × *c* | Child PID written twice. Both copies were equal in every definition. | available properties |
| subset_count *s* | 4 | u32 | Non-zero in 8 definitions, all of type 11 | |
| subset | 4·*s* | u32 × *s* | PIDs, each also in `children` | required properties (guess) |
| terminator | 4 | u32 | Always 0 | |

Default, minimum and maximum are encoded per §4, using the definition's own type. LC skips them for types 10, 11 and 12. This install has no definition of those types with bit 0x800, 0x1000 or 0x2000 set, so that rule is Reported only.

If the two copies of a child PID ever differ, readers should keep the first and report the record as suspicious. This never happens in observed data.

<a id="s2-3"></a>
### 2.3 Field statistics (Confirmed, all 27,546 definitions)

| Field | Observed values (count) |
|---|---|
| flags bits | 0x800 (12,435), 0x8 (4,424), 0x200 (300), 0x1 (268), 0x1000 (215), 0x20 (192), 0x2000 (128), 0x40 (83), 0x400 (77), 0x8000 (31), 0x10 (13), 0x80 (5), 0x4000 (5), 0x2 (4), 0x4 (3) |
| field_d byte 0 | 2 (27,336), 0 (123), 1 (87) |
| field_d byte 1 | 2 (25,404), 1 (1,636), 0 (506) |
| field_d byte 2 | 4 (15,835), 3 (7,115), 6 (2,330), 2 (884), 8 (727), 5 (337), 1 (264), 0 (42), 7 (12) |
| field_d byte 3 | 0 (27,528), 1 (18) |
| field_d byte 4 | 1 (all) |
| Definitions with children, by type | 12 (2,664), 11 (1,106), 21 (7), 9 (4), 20 (2) |

Only 0x800, 0x1000 and 0x2000 affect parsing. The other flag bits and the field_d bytes are metadata. A reader keeps them, but nothing in decoding depends on them.

> **OPEN:** The meanings of field_a, field_b, field_c, field_d, the flag bits other than 0x800/0x1000/0x2000, and the subset list. The enum mappers that would label several of them are named in the EMAPPER DID map ("PropertyGroup", "PropertyProvider", "PatchFlags", "PropertyType"), but their target DIDs are **not present** in this install (§8.3).

<a id="s2-4"></a>
### 2.4 Children

- **Type 12 (array).** Usually one child, the element property (2,630 array definitions). 45 array definitions have no children, and 34 have 2–11.
- **Type 11 (struct).** The children are the possible fields. 49 of the 1,155 struct definitions have no children.
- **A definition with no children is "open".** Any PID may appear as a field or element. Every one of the 1,218,341 struct fields and 821,097 array elements in observed data that is not listed as a child belongs to an open definition. No closed definition ever contains an unlisted PID (§5.6).

<a id="s2-5"></a>
### 2.5 Invariants (Confirmed)

- **MP1**: the record is consumed exactly.
- **MP2**: every definition's `pid_echo` equals its key, `marker` = 0x3FC00000, and `terminator` = 0.
- **MP3**: every PID with a definition also has a name. Exactly one named PID has no definition: PID 0, whose name means "invalid".
- **MP4**: PIDs fall in two families: < 0x10000000 (2,402 names) and 0x1000xxxx (25,145 names).
- **MP5** (corrected; it replaces `records.md` MP5): the `data` field depends on the type:
  - type 8: high byte 0x23 (enum mapper) or 0x28 (DID map);
  - types 14, 18, 22: high byte 0x23;
  - type 10: 0x34000000;
  - type 20: a **DB-object-type code** below 0x100, non-zero in all 1,849 type-20 definitions (§9);
  - every other type: 0.
- **MP6**: every subset PID also appears in `children`.

---

<a id="s3"></a>
## 3. Property type codes (Confirmed)

Names are descriptive. "Defs" counts definitions in 0x34000000. "Values in sets" counts values of that type found anywhere in the 384,950 property-set records, nested values included.

| Code | Kind | `data` field holds | Defs | Values in sets |
|---|---|---|---|---|
| 1 | string | 0 | 280 | 134,569 |
| 2 | string token | 0 | 4 | 14 |
| 3 | waveform (animated scalar) | 0 | 78 | 129 |
| 4 | timestamp | 0 | 133 | 0 |
| 5 | tri-state | 0 | 5 | 0 |
| 6 | vector3 | 0 | 126 | 155,833 |
| 7 | instance ID (64-bit) | 0 | 496 | 20,164 |
| 8 | enum value | Enum mapper 0x23… (2,452) or DID map 0x28… (19) | 2,471 | 7,598,377 |
| 9 | float | 0 | 5,203 | 13,969,701 |
| 10 | property ID | 0x34000000 | 428 | 1,195,555 |
| 11 | struct | 0 | 1,155 | 2,799,313 |
| 12 | array | 0 | 2,709 | 1,386,117 |
| 13 | StringInfo | 0 | 626 | 1,110,502 |
| 14 | bitfield, 64 bits | Enum mapper 0x23… (labels the bits) | 56 | 358,604 |
| 15 | int | 0 | 6,321 | 4,349,712 |
| 16 | colour | 0 | 109 | 1,893 |
| 17 | position | 0 | 46 | 6,115 |
| 18 | bitfield, 32 bits | Enum mapper 0x23… | 261 | 907,579 |
| 19 | int64 | 0 | 262 | 298 |
| 20 | DID reference | DB-object-type code (§9) | 1,849 | 7,390,369 |
| 21 | boolean | 0 | 4,860 | 6,258,467 |
| 22 | bitfield, any width | Enum mapper 0x23… | 68 | 37,556 |

LC also lists a different code numbering for another game on the same engine. That numbering is out of scope. Readers report a type code outside 1–22 as malformed.

---

<a id="s4"></a>
## 4. Value encodings (stored form)

These encodings apply both to dictionary defaults, minimums and maximums (§2.2) and to values in property sets (§5). The "Status" column says where each was confirmed.

<a id="s4-1"></a>
### 4.1 Summary

| Code | Stored encoding | Status |
|---|---|---|
| 1 | Pascal string: VLE byte length, then Latin-1 bytes (`records.md` §1.4) | Confirmed (31 defaults; 134,569 set values, 47 of them containing bytes ≥ 0x80) |
| 2 | u32 token | Confirmed (14 set values) |
| 3 | Waveform (§4.2) | Confirmed for subtypes 1 and 3 |
| 4 | f64 | Confirmed in defaults only (4) |
| 5 | u8 | Confirmed in defaults only (4) |
| 6 | 3 × f32 (x, y, z) | Confirmed |
| 7 | u64 | Confirmed |
| 8 | u32 enum key (§7.1) | Confirmed |
| 9 | f32. No NaN or infinity among 13,969,701 set values. | Confirmed |
| 10 | u32 PID (§7.4) | Confirmed (set values only) |
| 11 | Struct header and fields (§4.10) | Confirmed (set values only) |
| 12 | Array count and elements (§4.11) | Confirmed (set values only) |
| 13 | StringInfo (`records.md` §4; §4.8 here) | Confirmed |
| 14 | u64 bit mask (§7.3) | Confirmed |
| 15 | u32, interpreted as signed int32 | Confirmed |
| 16 | 4 × u8 (§4.7) | Confirmed (length); channel order OPEN |
| 17 | Position (§4.6) | Confirmed (set values only) |
| 18 | u32 bit mask (§7.3) | Confirmed |
| 19 | u64 | Confirmed |
| 20 | u32 DID; 0 means "none" (§4.9) | Confirmed |
| 21 | u8: 0 = false, 1 = true (§4.5) | Confirmed |
| 22 | Variable-width bitfield (§4.4) | Confirmed |

<a id="s4-2"></a>
### 4.2 Waveform (code 3)

| Field | Type | Present when |
|---|---|---|
| subtype | u32 | always |
| v0 | f32 | subtype = 1 |
| v0…v9 | 10 × f32 | subtype ≥ 2 and ≠ 10 |
| v0…v9, extra, flag, *k*, pairs | 10 × f32, f32, bool8, u32 *k*, 2·*k* × f32 | subtype = 10 |
| *(nothing more)* | | subtype = 0 |

Observed: subtype 1 in 41 defaults and all 129 set values; subtype 3 (ten floats) in 3 defaults. Subtypes 0 and 10 are Reported (LC).

> **OPEN:** The meaning of the subtypes and of the ten floats (likely waveform shape, amplitude, phase, frequency and similar). LC notes that an older client version always stored ten floats. That variant is out of scope.

<a id="s4-3"></a>
### 4.3 Fixed-width scalars

Codes 2, 7, 8, 10, 14, 15, 18, 19 and 20 are plain little-endian integers, of 4 bytes or 8 bytes as listed in §4.1. Code 4 is an f64 timestamp; its epoch and unit are not specified here.

> **OPEN:** The timestamp epoch and unit (code 4) and the meaning of the three tri-state values (code 5). No set value of either type exists in this install.

<a id="s4-4"></a>
### 4.4 Variable-width bitfield (code 22)

| Field | Type | Meaning |
|---|---|---|
| bit_count *b* | VLE | Number of bits |
| bytes | ⌈*b*/8⌉ × u8 | Bit *i* is bit (*i* mod 8) of byte ⌊*i*/8⌋, least significant bit first |

Observed *b* ranges from 0 to 957, and 52 is the most common. In all 12,446 sampled values where *b* is not a multiple of 8, the unused high bits of the last byte were zero. Writers must zero them. Readers ignore them.

<a id="s4-5"></a>
### 4.5 Boolean (code 21)

One byte. All 6,258,467 set values and all 2,003 boolean default/minimum/maximum values in the dictionary were 0 or 1 (3,618,608 false, 2,639,859 true). Readers may treat any other byte as malformed, in line with `bool8` in `records.md` §1.1. This settles the `records.md` §13 question.

<a id="s4-6"></a>
### 4.6 Position (code 17)

A position begins with a flag byte. Each payload bit adds a field, and fields appear **in ascending bit order**:

| Flag bit | Field | Type | Observed in (of 6,115) |
|---|---|---|---|
| 0x01 | region | u8 | 6,102 |
| 0x02 | landblock X, landblock Y | u8, u8 | 6,102 |
| 0x04 | instance | u16 | 0 (Reported) |
| 0x08 | cell | u16 | 5,435 |
| 0x10 | x, y, z | 3 × f32 | 5,445 |
| 0x20 | orientation quaternion | 4 × f32 | 2,641 |
| 0x40 | *(no payload)* | | 2,108 |
| 0x80 | *(no payload)* | | 5,266 |

Facts about the flags (Confirmed unless marked):

- A flag byte of 0 is a position with no fields at all (Reported, LC; never observed).
- **Bits 0x40 and 0x80 carry no payload.** Every position decodes exactly when they are given no bytes, and the decoded fields are plausible.
- Bit 0x40 is set exactly when the region byte is present and equals 1 (2,108 of 2,108).
- Bit 0x80 is set exactly when the cell field is present and equals 0 (5,266 of 5,266). No position with a non-zero cell has it.
- Observed regions: 1, 2, 3, 4, 5 and 14. These match the landblock regions in `records.md` §2.2. Landblock X and Y match that section's 0–254 range.
- When the region and landblock are present and the cell is 0 (outdoors), x and y are block-local coordinates, always observed within 0–160. When the cell is non-zero (indoors), x and y fell outside 0–160 in 140 of 169 values, so they are not block-local in the same sense. Positions with no landblock (flags 0x10 or 0x30, 13 values) hold small relative offsets, which can be negative.
- Every sampled quaternion has unit length (433 of 433, to 3 decimals). The component order has not been determined. LC reads them as w, x, y, z.

> **OPEN:** What bits 0x40 and 0x80 mean beyond the correlations above ("region is the default region" and "outdoors"?). Writers should set them the same way: 0x40 when region = 1, 0x80 when cell = 0. Also OPEN: the quaternion component order, the units of x, y, z, and whether the orientation follows the same convention as entity placement in landblock records.

<a id="s4-7"></a>
### 4.7 Colour (code 16)

Four bytes. In 585 sampled set values the **fourth byte was always 0xFF**, so it is alpha. The first three bytes vary freely.

> **OPEN:** Whether the first three bytes are R, G, B or B, G, R. The little-endian D3D convention 0xAARRGGBB would give B, G, R, A in memory, which matches alpha last. ER reports that the client widens a colour to four floats in memory, in RGBA order.

<a id="s4-8"></a>
### 4.8 StringInfo (code 13)

The encoding is in `records.md` §4. Property-set observations (Confirmed, 1,110,502 values):

- 65,496 values are literals and 1,045,006 are string-table references.
- **No value has an argument block** (has_args = 1). The argument path in `records.md` §4.2 is still unexercised by data.
- Every reference names a 0x25 table, except 2 values whose table DID and token are both 0. Treat (0, 0) as "no text".
- 812,288 references resolve to an existing (table, token) pair in `client_local_English.dat`, and 232,718 (22.3%) do not. As in `records.md` ST7, a missing token is a lookup miss, not a parse error.

<a id="s4-9"></a>
### 4.9 DID reference (code 20)

A u32 DID. The value 0 means "no reference". It occurs 1,475,154 times.

The definition's `data` field names the **DB-object type** that the reference is expected to point to (§9). For all but 2,272 of the non-zero values, the DID's high byte is the prefix associated with that code. The exception is two icon-reference definitions declared with the game-object code (0x45) whose values are render surfaces (0x41). So readers **must not reject** a DID whose prefix disagrees with the declared code. They may warn.

<a id="s4-10"></a>
### 4.10 Struct (code 11)

| Field | Type | Meaning |
|---|---|---|
| hint | u8 | Bucket-count index, as in a TSize hint (`records.md` §1.3) |
| field_count *f* | u8 (see note) | Number of fields |
| fields | property entry × *f* | Each entry is **PID, PID echo, value** (§5.2) |

Confirmed facts (all 2,799,313 structs):

- The hint byte is 0 (2,784,714), 1 (9,615) or 2 (4,984).
- Fields are in **bucket order** by PID modulo the bucket count of the hint (2,799,307 of 2,799,307 structs checked in gamelogic).
- *f* ranged over 0–62. Hint 0 was seen with *f* = 0–20, hint 1 with 23–46, and hint 2 with 48–62. So in every struct, *f* ≤ 2·*B*[hint].
- A field PID never repeats within one struct.

The header is therefore a TSize whose count happens always to fit in one byte. LC and MK read it as "skip a byte, then a u8 count", which gives the same result on all observed data.

> **OPEN:** Whether a struct with 128 or more fields stores the count as a multi-byte VLE (TSize reading) or not. No struct in this install has more than 62 fields. Readers should decode the count as a VLE: that is byte-identical for counts below 128 and correct if the TSize reading holds.

<a id="s4-11"></a>
### 4.11 Array (code 12)

| Field | Type | Meaning |
|---|---|---|
| element_count *e* | u32 | Number of elements |
| elements | element entry × *e* | Each entry is **PID, value**, with no echo (§5.2) |

Confirmed facts (all 1,386,117 arrays):

- *e* ranged from 0 to 5,211. The most common sizes were 1 (572,261 arrays), 3 and 2.
- **Each element carries its own PID**, and its value is decoded by that PID's type. Elements of one array need not share a PID: 65,197 arrays mix element PIDs. 64,014 of them belong to closed definitions that list several children, and 1,183 to open definitions (§2.4).
- Element order is the stored order. No hashing applies, and duplicate element PIDs are normal.

---

<a id="s5"></a>
## 5. Property sets

<a id="s5-1"></a>
### 5.1 Property-set record layout (Confirmed, 384,950 records)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | did | DID | The record's own DID (0x78… or 0x79…) |
| 2 | count *n* | TSize | Number of property entries (`records.md` §1.3) |
| 3 | entries | property entry × *n* | §5.2, each with PID echo |
| 4 | *(end)* | | Zero trailing bytes in every record |

An embedded property set (in another record type) is the same structure without the leading DID: a TSize followed by *n* entries.

<a id="s5-2"></a>
### 5.2 Entries

There are two entry forms. Which one is used depends only on where the entry sits:

| Context | Entry layout |
|---|---|
| Top level of a set; field of a struct | u32 PID, u32 PID echo (must equal PID), value |
| Element of an array | u32 PID, value |

The value is decoded per §4, using the type from the PID's definition. A struct or array value contains further entries, so decoding is recursive.

**Null entry** (Reported, LC): if the first u32 of an entry is 0, the entry is empty. It has no echo and no value, and it still counts toward its enclosing count. No null entry occurs in this install.

<a id="s5-3"></a>
### 5.3 Decoding algorithm (in words)

1. Read the DID (record form only), then the TSize count *n*.
2. For each of the *n* entries, read an entry in the "with echo" form:
   1. Read the PID. If it is 0, the entry is null; go to the next entry.
   2. Read the echo. If it differs from the PID, the record is malformed.
   3. Look up the PID in the dictionary. An unknown PID makes the record undecodable, because the value length is unknown. Report it as such; do not guess.
   4. Decode the value by the definition's type. For a struct, read the struct header and then *f* entries in the "with echo" form. For an array, read the u32 count and then *e* entries in the "no echo" form.
3. After the last entry, the payload must be exhausted. Leftover bytes mean the record is malformed or the dictionary does not match the data.

Defensive limits for hostile input:

- **Nesting depth.** Observed values sit at most 9 levels below the top level (depth histogram in §11). A limit of 32 is suggested.
- **Array count.** Bound each count by the remaining payload: every element needs at least 5 bytes (a PID and a 1-byte value), so *e* cannot exceed remaining/5. Also apply an absolute cap, for example 1,000,000.
- **Struct field count and TSize count.** Apply the same remaining-bytes bound, with at least 9 bytes per entry (PID, echo, a 1-byte value).

<a id="s5-4"></a>
### 5.4 Order and hint (Confirmed)

- **PS-O1**: the top-level entries of every record are in bucket order: PID modulo *B*[hint] is non-decreasing (384,950 of 384,950 records).
- **PS-O2**: struct fields follow the same rule with the struct's own hint (§4.10).
- **PS-H1**: in every set, *n* ≤ 2·*B*[hint]. Observed hints by top-level count: 0 for *n* = 0–22, 1 for 2–46, 2 for 23–94, 3 for 76–117, 4 for 106–217, 5 for 385–467, and 6 for 801–811. The largest *n* is 811.
- **PS-H2**: the hint is **not** a function of *n*. 230,285 records use a hint smaller than "smallest *B* ≥ *n*", and 1,147 use a larger one. A byte-exact writer must keep the original hint. A writer producing new data should pick the smallest hint with 2·*B* ≥ *n* (which matches every observed struct) and write the entries in that hint's bucket order.

Hint counts over all records: 0 (126,430), 1 (46,027), 2 (97,173), 3 (74,764), 4 (3,627), 5 (36,791), 6 (138).

<a id="s5-5"></a>
### 5.5 Uniqueness

- **PS-U1**: a PID appears at most once among the top-level entries of a set (Confirmed: no duplicates in 32,676,318 top-level entries).
- **PS-U2**: a PID appears at most once among the fields of a struct (Confirmed).
- Array elements may repeat a PID (§4.11).

A reader that meets a duplicate top-level or struct PID should report the record as malformed. If it must continue, it keeps the last value, since that is what inserting into a map does.

<a id="s5-6"></a>
### 5.6 Children as a schema (Confirmed)

- **PS-C1**: when a struct definition lists children, every field of every value of that struct is one of them (8,158,713 fields checked).
- **PS-C2**: when an array definition lists children, every element PID is one of them (4,806,398 elements checked).
- When the definition lists no children, any PID may appear (§2.4).

Readers must not reject unlisted PIDs in open definitions. For closed definitions, an unlisted PID can be logged as a schema warning but must still be decoded, since its type comes from its own definition.

<a id="s5-7"></a>
### 5.7 Other observations

- 255 records hold an empty set (*n* = 0).
- Values were found at every depth from 0 to 9. Depth 0 holds 32,676,318 values, and 954 values sit at depth 9.
- Both PID families appear at every level: 45,133,521 values use 0x1000xxxx PIDs and 2,547,346 use PIDs below 0x10000000.

---

<a id="s6"></a>
## 6. Where property sets live

<a id="s6-1"></a>
### 6.1 DID ranges (Confirmed census, every archive)

| Prefix | Archive (count) | Meaning |
|---|---|---|
| 0x78 | gamelogic (1,616: 0x78000001–0x7800CD78, sparse); local (2) | **Stand-alone property sets** ("DB properties"). Many are reached through DID maps: CONTENT_LAYER, PHYSICSCONTROLLER, SCRIPTCALLBACK, DBPC, UNIQUEDB (§8). |
| 0x79 | gamelogic (383,332) | **Object property sets**: the properties of the 0x70 object with the same serial (§6.2) |
| 0x7A–0x7F | none | Not present in any archive |

The two 0x78 records in `client_local_English.dat` do not exist in gamelogic. Routing (`dat-container.md`) must therefore try gamelogic and then local for the 0x78–0x7F range, as it already does.

All 384,950 records decode with the layout in §5.1.

<a id="s6-2"></a>
### 6.2 The +0x09000000 rule (Confirmed)

The property set of a game object with DID *d* is at DID *d* + 0x09000000.

- For 0x70 objects (gamelogic, 383,332 records), this gives 0x79 records. The serial sets are **identical**: every 0x70 serial has a 0x79 partner, and every 0x79 has a 0x70 partner.
- LC and DS describe the rule for the whole game-object range 0x70–0x77, which would map to 0x79–0x80. Of those, only 0x70 and 0x76 occur. The 11 records with prefix 0x76 have **no** 0x7F partner, since no 0x7F record exists. A reader that applies the rule to a 0x76 DID gets a lookup miss, which it treats as "no property set".
- 0x78 records are stand-alone. No 0x6F record exists.

A reader implementing the rule should apply it to prefixes 0x70–0x77 only, treat a missing target as "no properties", and never apply it to the result again.

> **OPEN:** Whether 0x76 objects keep their properties somewhere else (perhaps inside the 0x76 record itself). Their layout is not covered by any spec yet.

<a id="s6-3"></a>
### 6.3 Embedded property sets (Reported)

DS lists embedded property sets (§5.1 without the DID) in landblock info (prefix 0x80, K = 2), UI layouts (0x22), script tables (0x07), entity descriptors (0x47, after a 16-byte header), block-map descriptors (0x18) and appearance records (0x20). Their placement belongs to the specs for those records, and the encoding is the one here. DS also reports a single-PID "property list" form in 0x18 records: a u32 count followed by entries in the no-echo form.

---

<a id="s7"></a>
## 7. Resolving values to readable form

None of this is needed to **decode** a set. It is needed to **display** one, as in the explorer, and by gameplay code.

<a id="s7-1"></a>
### 7.1 Enum values (code 8)

The definition's `data` names the mapper:

- **0x23… (EnumMapper, `records.md` §7):** look up the u32 key in the mapper and then up its base chain. The mapper gives a symbolic name and, for some keys, a StringInfo display text. In a one-third sample, 2,535,620 values resolved and 46 did not. An unresolved key is shown as its number.
- **0x28… (DID map, `records.md` §8):** the value is a **key** in that DID map. The map's label for the key is the display name, and its DID is the target. Two maps are used this way: 0x28000012 (CONTENT_LAYER, 1,552 values) and 0x28000008 (REGION, 11 values).

<a id="s7-2"></a>
### 7.2 Bitfields (codes 14, 18, 22)

Labels come from the mapper named by the definition's `data` (always 0x23…).

<a id="s7-3"></a>
### 7.3 Bit-to-key rule (Confirmed)

**Bit *i* (counting from 0 at the least significant bit, or from byte 0 bit 0 for code 22) is labelled by enum key *i* + 1.** Of 1,062,289 set bits in a half sample (858,662 of code 18, 179,300 of code 14, 24,327 of code 22), every one had a label under this rule. The alternatives "key = *i*" and "key = 2^*i*" each leave thousands unlabelled.

<a id="s7-4"></a>
### 7.4 Property-ID values (code 10)

The value is a PID; display it by its dictionary name. In observed data, 705,015 values are 0 (none), and the other 490,540 are all defined PIDs.

<a id="s7-5"></a>
### 7.5 StringInfo values (code 13)

Resolve as in `records.md` §4 against the string tables of the selected locale (string-table spec / `tessera-text`). (0, 0) means no text (§4.8).

<a id="s7-6"></a>
### 7.6 DID references (code 20)

Display the DID together with the declared type (§9). A viewer can open the target through normal routing.

---

<a id="s8"></a>
## 8. DID maps: the root map and its sub-maps

The layout is in `records.md` §8. This section records what the root map contains (Confirmed, all 24 DID maps decoded).

<a id="s8-1"></a>
### 8.1 Root map 0x28000000, first group

Labels are interoperability identifiers that the client looks up **by name**. Implementations read them at run time and must never hard-code the DIDs below, which are the values observed in this install.

| Root key | Label | Sub-map | Sub-map entries (groups) | Main target prefixes |
|---|---|---|---|---|
| 0x0 | UNDEFINED | 0 | | sentinel |
| 0x1 | MUSIC | 0x28000005 | 9 (9 + 0) | 0x0A (8). These are the DirectMusic DMAP records of `records.md` §9.2. |
| 0x3 | COMPRESSIONDICTIONARY | 0x28000010 | 4 (4 + 0) | 0x0E (3) |
| 0x4 | UILAYOUT | 0x28000003 | 236 (236 + 0) | 0x22 (232), 0x41 (2), 0x2A (1) |
| 0x5 | PRELOADRESOURCES | 0x2800000F | 24 (24 + 0) | 0x22 (9), 0x04 (7), 0x33 (7) |
| 0x6 | ACTIONMAP | 0x2800000A | 3 (3 + 0) | 0x51 (2) |
| 0x7 | EMAPPER | 0x28000001 | 436 (314 + 122) | 0x23 (428), 0x28 (7) |
| 0x8 | KEYMAP | 0x2800000C | 7 (5 + 2) | 0x1D (6) |
| 0x9 | CONTENT_LAYER | 0x28000012 | 1,661 (1,661 + 0) | 0x78 (1,660) |
| 0xA | WEENIECONTENT | 0x2800000E | 316 (315 + 1) | 0x70 (314), 0x07 (1) |
| 0xB | DBPC | 0x28000011 | 10 (9 + 1) | 0x78 (9) |
| 0xD | SCRIPTCALLBACK | 0x28000007 | 13 (13 + 0) | 0x78 (12) |
| 0xE | WLIB | 0x2800000D | 3 (2 + 1) | 0x56 (2) |
| 0xF | UNIQUEDB | 0x28000002 | 389 (388 + 1) | 0x2B (254), 0x78 (36), 0x41 (26), 0x31 (14), 0x2A (11), 0x39 (10) |
| 0x10 | FONT | 0x2800000B | 5 (5 + 0) | 0x42 (4) |
| 0x11 | REGION | 0x28000008 | 9 (9 + 0) | 0x0F (8) |
| 0x12 | STRINGTABLE | 0x28000004 | 179 (179 + 0) | 0x25 (177), 0x58 (1) |
| 0x19 | PHYSICSCONTROLLER | 0x28000030 | 7 (7 + 0) | 0x78 (6) |
| 0x10000002 | RIPCORDS | 0x2800002C | 292 (292 + 0) | 0x70 (291) |
| 0x10000003 | COMBATEFFECTS | 0x2800002E | 27 (27 + 0) | 0x70 (26) |
| 0x10000004 | BATTLESTATS | 0x28000031 | 14 (14 + 0) | 0x70 (13) |
| 0x10000005 | VOCATIONS | 0x28000032 | 8 (8 + 0) | 0x70 (7) |
| 0x10000006 | PROFESSIONS | 0x28000033 | 11 (11 + 0) | 0x70 (10) |

Every DID map, root included, has exactly one entry with DID 0: key 0, the "undefined" sentinel. That is why each target count is at most the entry count minus one (the prefix column lists only the main prefixes).

Sub-maps whose targets are 0x70 (game objects) reach their properties through §6.2. Sub-maps whose targets are 0x78 hand out stand-alone property sets directly.

<a id="s8-2"></a>
### 8.2 Root map, second group

The root's second group has one entry, "XBOX360_WORLDVIEWER", whose target 0x28000024 does **not** exist in this install. DID map 0x28000025 exists but is not referenced by the root. It is reached through the EMAPPER map's second group, as a DID map used for enum-like lookup.

<a id="s8-3"></a>
### 8.3 Second groups in general (Confirmed)

The second groups are not all dead entries. EMAPPER's second group has 122 entries, and 76 of them resolve to existing mappers. These include the DB-object-type mapper used in §9 ("DBOType" → 0x2300007D). The other 46 point at DIDs that do not exist, among them the "PropertyType", "PropertyGroup", "PropertyProvider" and "PatchFlags" mappers. The second-group entries of UNIQUEDB, KEYMAP, WLIB, WEENIECONTENT and DBPC are all dangling.

Dangling targets per map, counting both groups: EMAPPER 46, UNIQUEDB 28, MUSIC 8, UILAYOUT 2, KEYMAP 2, root 1, REGION 1, WLIB 1, DBPC 1.

Readers merge both groups (as `records.md` §8.1 says) and treat a dangling target as a lookup miss. This refines the `records.md` §8.1 OPEN: second groups hold entries that the client build may not ship, such as tool, server or other-platform data. They are still not useless.

---

<a id="s9"></a>
## 9. DB-object-type codes (Confirmed)

A type-20 definition's `data` field (§4.9) holds a code from the **DB-object-type** enumeration. That enumeration is in the data: it is the enum mapper labelled "DBOType" in the EMAPPER DID map (0x2300007D in this install, which has no entries of its own and inherits all 75 from its base mapper 0x230000F0). A client must read the code labels from there at run time. The table below records only what this install showed: which DID prefix the referenced values actually carry for each code.

| Code | Descriptive kind | DID prefix of referenced values (count) |
|---|---|---|
| 0x05 | Render surface | 0x41 (610,521) |
| 0x0B | Setup | 0x04 (26) |
| 0x12 | Sound info | 0x2A (35,095) |
| 0x15 | Scriptlet | 0x0C (88,794) |
| 0x1E | Particle-system descriptor (presumed) | 0x39 (272) |
| 0x20 | String table | 0x25 (1) |
| 0x28 | UI layout | 0x22 (544,130) |
| 0x2A | Script table | 0x07 (429,655) |
| 0x32 | DB property set | 0x78 (2,626) |
| 0x39 | Entity descriptor | 0x47 (204,005) |
| 0x43 | UI scene | 0x21 (103) |
| 0x45 | Game object (WState) | 0x70 (3,153,984); 0x41 (2,272, see §4.9) |
| 0x4C | Font | 0x42 (6) |
| 0x4D | Enum mapper | 0x23 (5) |
| 0x4E | Appearance | 0x20 (843,720) |

Codes that type-20 definitions declare but no stored value exercised: 0x08, 0x0A, 0x0D, 0x0F, 0x1A, 0x1F, 0x26, 0x36, 0x38, 0x3A, 0x44, 0x50, 0x51, 0x56. Their labels can be read from the mapper.

This gives new evidence for `records.md` §2.1. Prefix 0x2A ("unknown" there) holds **sound-info** records, 0x0C holds **scriptlets**, 0x21 holds **UI scenes**, and 0x39 is a descriptor type labelled with a particle-system-like name. The same numbering matches the cache type IDs that ER reports for landblock data (17), landblock info (46), landblock block map (47), landblock properties (41), entity groups (26) and navigation landblocks (100). Each of those is also a code in the mapper.

> **OPEN:** A full code → prefix table for the codes not exercised above. It would let the routing table in `dat-container.md` be cross-checked against data. Deriving it needs either more reference kinds or the archives' own type metadata.

---

<a id="s10"></a>
## 10. Network / in-memory form (Reported, LC and DS)

These facts matter only for the M7 protocol work. They are recorded here so that the property model is described in one place. None of them were checked.

| Element | Disk form (§5) | Network form |
|---|---|---|
| Set count | TSize | VLE |
| PID | u32, with echo where §5.2 says | **Packed PID.** Read byte *a*. If *a* = 0x80, a u32 PID follows. Otherwise read byte *b* and form *c* = (*a* << 8) \| *b*: if bit 0x4000 of *c* is set, PID = 0x10000000 \| (*c* & 0x3FFF), else PID = *c*. No echo. |
| Int (15) | u32 | VLE |
| Enum (8), property ID (10) | u32 | u8 family marker (0 or 16), then VLE. Marker 16 means OR in 0x10000000. |
| Struct (11) | hint + count | u8 field count (LC rejects counts ≥ 128) |
| Position (17) | §4.6 | A different flagged layout, to be specified in the protocol spec |
| Everything else | §4 | Same as disk |

---

<a id="s11"></a>
## 11. Validation run (2026-10-04)

A throwaway Python probe in the dirty-room scratchpad decoded every 0x78–0x7F record in every archive that has them. It worked from the layouts in this spec and the 0x34000000 dictionary.

| Measure | Result |
|---|---|
| Records attempted | 384,950 (gamelogic 0x79: 383,332; gamelogic 0x78: 1,616; local 0x78: 2) |
| Decoded with zero trailing bytes | **384,950 (100.000%)** |
| Records with trailing bytes | 0 |
| Records failing to decode | 0 |
| Property types responsible for failures | none |
| Unknown PIDs met | 0 |
| PID echo mismatches | 0 |
| Null (PID 0) entries | 0 |
| Self-DID mismatches | 0 |
| Property values decoded | 47,680,867 (32,676,318 top-level) |
| Records in bucket order | 384,950 of 384,950 |

Depth histogram, where depth 0 is the top level: 0: 32,676,318; 1: 4,385,622; 2: 7,725,625; 3: 1,398,880; 4: 830,115; 5: 497,019; 6: 98,245; 7: 65,147; 8: 2,942; 9: 954.

The M1 exit criterion (≥ 99.5% clean) is met with no exceptions, so no record has to be listed with a responsible property type.

---

<a id="s12"></a>
## 12. Discrepancies and corrections

| # | Source says | Observed | Resolution |
|---|---|---|---|
| X1 | `records.md` MP5: type-20 `data` = 0 | Every type-20 definition has a non-zero code < 0x100 | It is a DB-object-type code (§9). MP5 corrected in §2.5. |
| X2 | LC, MK, DS: struct value is "skip one byte, then u8 field count" | The skipped byte is a bucket-count hint (0–2), and fields are in bucket order for it | §4.10. Readers that skip it still decode correctly. |
| X3 | `records.md` §6.4: type 17 encoding "to be specified" | Flagged layout confirmed, including two payload-less bits | §4.6 |
| X4 | `records.md` §13: is type 21 strictly 0/1? | Only 0/1 in 6.26 M set values and 2,003 dictionary values | Strict `bool8` is safe (§4.5) |
| X5 | DS/LC: the +0x09000000 rule covers 0x70–0x77 | Only 0x70 → 0x79 pairs exist; 0x76 has no partners | §6.2 |
| X6 | ER: dictionary has 27,268 entries | 27,547 names / 27,546 definitions today | Counts change with patches; ER described an older build |
| X7 | `records.md` §2.1: 0x0C "possibly entity/prop data"; 0x2A, 0x21 and 0x39 unknown | Type-20 references declared as scriptlet, sound info, UI scene and particle-system-like codes point at exactly these prefixes | §9. `records.md` §2.1 should be updated by its owner. |
| X8 | `records.md` §8.1 OPEN: the second DID-map group may be useless | EMAPPER's second group holds 76 live mappers | §8.3 |

---

<a id="s13"></a>
## 13. Open questions (summary)

1. Dictionary reserved bytes, separator, field_a/b/c/d, extra flag bits and the subset list (§2.1, §2.3).
2. Struct field count ≥ 128: VLE or not (§4.10).
3. Waveform subtype meanings; subtypes 0 and 10 unexercised (§4.2).
4. Timestamp epoch and unit; tri-state meaning (§4.3).
5. Position bits 0x40 and 0x80, the quaternion order, and the instance field (bit 0x04, unexercised) (§4.6).
6. Colour channel order (§4.7).
7. StringInfo argument blocks: none in 1.1 M property values (§4.8; `records.md` §4).
8. The 0x76 objects' properties (§6.2).
9. DB-object-type codes not exercised by data (§9).
10. The null entry (PID 0) is Reported only (§5.2).

---

<a id="s14"></a>
## 14. Validation guidance for implementers

Real-data tests (skipped without an install) should assert the **invariants**, not the counts:

- every 0x78/0x79 record decodes with zero trailing bytes;
- PS-O1 holds, and PS-U1, PS-C1 and PS-C2 hold;
- every 0x70 DID's +0x09000000 partner exists.

Synthetic fixtures, built by our own code with a synthetic dictionary, should cover:

- one value of each type code 1–22, including every waveform subtype and every position flag bit (with 0x40 and 0x80 carrying no bytes);
- a struct with hint 0 and with hint 1, with fields in the matching bucket order, and an open struct with an unlisted field;
- an array with zero elements, one with mixed element PIDs, and one whose count exceeds the remaining bytes (must be rejected without allocating);
- an echo mismatch, an unknown PID, a null entry, trailing bytes, and nesting one level past the depth limit;
- a variable-width bitfield with *b* not a multiple of 8;
- a 0x70 DID with and without a 0x79 partner, and a 0x76 DID (no partner).

<a id="s14-1"></a>
### 14.1 Worked synthetic example

Suppose the dictionary defines these synthetic PIDs:

- 0x10000BBB, type 15 (int);
- 0x10000AAA, type 11 (struct) with one child, 0x10000CCC;
- 0x10000CCC, type 21 (boolean).

A stand-alone set 0x78000ABC holding int = 7 and a struct with one boolean field = true is 37 bytes:

| Bytes | Meaning |
|---|---|
| `BC 0A 00 78` | DID 0x78000ABC |
| `00 02` | TSize: hint 0 (*B* = 11), *n* = 2 |
| `BB 0B 00 10` `BB 0B 00 10` | PID 0x10000BBB and echo (0x10000BBB mod 11 = 3) |
| `07 00 00 00` | int 7 |
| `AA 0A 00 10` `AA 0A 00 10` | PID 0x10000AAA and echo (mod 11 = 5, so it comes after the bucket-3 entry) |
| `00 01` | struct header: hint 0, one field |
| `CC 0C 00 10` `CC 0C 00 10` | field PID 0x10000CCC and echo |
| `01` | true |

If 0x10000DDD were an array of type-15 elements with PID 0x10000123, the value "[5, 6]" would follow the PID and echo as `02 00 00 00`, then `23 01 00 10 05 00 00 00`, then `23 01 00 10 06 00 00 00`. Array elements have no echo.

---

<a id="s15"></a>
## 15. Open items raised by the `formats` implementation (2026-10-04)

These came up while implementing property-set decoding in `crates/formats` from this spec alone. Each records the conservative choice the implementation made, so a spec writer can confirm or correct it. The whole-install run of `crates/formats/tests/m1_real_data.rs` reproduces §11 exactly (384,950 of 384,950 records clean, 47,680,867 values, the same depth histogram), so none of these choices is exercised by current data.

> **OPEN:** §5.3 suggests bounding counts with at least 9 bytes per entry (5 per array element). A null entry (§5.2, Reported) is a bare 4-byte zero PID, so a set or array made of null entries would be rejected by that bound. The implementation bounds every count with 4 bytes per entry, which still rejects hostile counts before allocating. Please confirm whether null entries can occur in bulk, or whether the 9/5-byte bounds are safe.

> **OPEN:** §5.2 describes the null entry for "an entry" without saying whether it applies to array elements (the no-echo form). The implementation treats a zero PID as a null entry in all three contexts (top level, struct field, array element). Null entries are counted but not stored, so their positions are lost; a byte-exact writer would need them.

> **OPEN:** §5.5 says a duplicate top-level or struct PID makes the record malformed. The implementation rejects such records outright (it keeps entries in a list, so "keep the last value" does not arise at this layer). Higher layers that build maps may relax this if the spec allows it.

> **OPEN:** §6.2 states the +0x09000000 rule for prefixes 0x70–0x77, but 0x77 + 0x09000000 lands on prefix 0x80, which holds landblock records (a probe found a real 0x80 heightmap at the target of a hypothetical 0x77 DID). The implementation applies the rule to 0x70–0x76 only and returns "no property set" for 0x77, so its result is always in 0x79–0x7F. No 0x77 object exists in the current install. Please correct §6.2 (the rule must not produce a DID outside 0x79–0x7F) or say where 0x77 objects keep their properties.

> **OPEN:** §6.1 counts 0x7A–0x7F as part of the property-set range, but `records.md` §2.1 does not list those prefixes, so the implementation's DID classifier reports them as unknown (never observed) while its property-set helpers accept them. A 0x76 object's +0x09000000 target (0x7F) is therefore a property-set DID of unknown kind. Please say whether §2.1 should list 0x7A–0x7F as a reserved property-set range.

---

<a id="s16"></a>
## 16. Open items raised by the `props` implementation (2026-10-04)

These came up while implementing `crates/props` (registry, typed values, labels, DID-map navigation) from this spec and `records.md` alone. Each records the conservative choice the implementation made. On the install of 2026-10-04 the real-data test (`crates/props/tests/real_data.rs`) resolved 50,000 random 0x70 objects (6,171,402 values): every value named, 99.998% of enum values labelled, every set bit labelled, every non-zero PID value named, and all 29 DB-object-type codes that type-20 definitions declare labelled through "DBOType".

> **OPEN:** `records.md` §7.1 says to search the base chain when a key "is not found", but a mapper has two lists (names and display texts), and a derived mapper may name a key that only its base has a display text for, or the other way round. The implementation looks the name and the display text up **independently**: each comes from the first mapper in the chain that has one for the key. Please confirm whether the client does the same, or stops at the first mapper that has the key in either list.

> **OPEN:** §8.3 says some EMAPPER entries target DID maps (0x28000025 and others: 7 of 435 non-null targets in this install) that are "used for enum-like lookup". The implementation's enum-mapper lookup by name returns "no mapper" for those, since §7.1 only describes DID-map-backed enums through a type-8 definition's `data` field. Please say how such an EMAPPER-named DID map is used as an enum (keys = enum keys, labels = names?), so that a name-based lookup can return either form.

> **OPEN:** §7.1 (DID-map enums) does not say whether key 0, the "UNDEFINED" sentinel that every DID map holds with DID 0, should be shown as a label or as "none". The implementation shows the map's label for it like any other key and reports the target as the null DID.

> **OPEN:** §7.3 gives the bit-to-key rule for bitfields but not what a viewer should do with a set bit whose key *i* + 1 is missing from the mapper chain (none in observed data). The implementation reports such a bit as unlabelled by index (`bit<i>`), and treats the bitfield as only partly resolved.
