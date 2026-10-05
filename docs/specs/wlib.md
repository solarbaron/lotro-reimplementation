<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# WLib: the script class library (DID 0x56000000)

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Claude review session (spec-writer agent), review 2026-10-05: changes requested (five findings: §14 trailing-byte test contradicted §3.2/§10; network-form remarks at the end of §5 and §8.4 fell under the `docs/LEGAL.md` §9.3 gate; §9 count scope; speculation in §2; no reader rule for chunks 32 and 256). Claude review session (spec-writer agent), re-review 2026-10-05 after the repair round: **passed, Accepted**. All five findings resolved (network-form content withdrawn; §14 consistent with §3.2 and §10; W25 scoped; §2 reduced to the "Client" label; §10 reader rules for chunks 32 and 256 backed by BIN row W24). Re-review spot checks on the install with an independent decoder: header 0x1A015342 / 38 / 0x1E4A5AA4; chunk set 1, 2, 16, 64, 512, 1024 and trailing byte 1; parent index > child index in 296 of 5,822 entries and 282 defined classes (W25); 1,381,857 per-record class-table entries equal the library lists in all 383,343 object records (W11). Earlier first-round checks (L2–L6, attribute-list census, flattening, parent map, singletons, method tables, root classes) still stand. Remaining OPENs (§3.1 extra word, §3.2 boolean byte, chunks 32/256/128) are precise and non-blocking. |
| **Implements into** | `crates/wstate` (`ClassLibrary`); `apps/dattool` / `apps/explorer` class views |
| **Companion specs** | `docs/specs/records.md` [§1](records.md#s1) (fixed-width values, VLE, TSize and bucket order, strings, name hash §1.5), [§2](records.md#s2) (DID high-byte table); `docs/specs/property-system.md` [§8](property-system.md#s8) (DID maps; the WLIB map); `docs/specs/wstate.md` (object-state records, which are decoded with this library); `docs/specs/dat-container.md` (routing: prefix 0x56 lives in gamelogic). This spec starts with the fully reassembled payload of record 0x56000000. |

---

<a id="purpose"></a>
## Purpose

Game objects such as items, quests, skills, areas and dungeons are stored as instances of
**script classes** (`wstate.md`). The layout of every script class (which attributes it
has, of which value type, in which order they are serialized, and which class it derives
from) is stored in one record, the **class library** ("WLib") at DID 0x56000000 in
`client_gamelogic.dat`. This spec gives an implementer of `tessera-wstate` what they need
to:

- walk the chunked container of the WLib record (§3);
- read the class table: which class indices are script classes, their slot layout and
  defaults (§6);
- read each class's attribute list, which fixes the **serialization order** of its
  attribute values in object-state records (§8);
- read the parent map (§9) and the singleton-class list (§7);
- build a `ClassLibrary` and check it (§10, §11).

The other chunks (bytecode, messages) and the method tables are described far enough to
skip them safely and to know what they mean (§4, §6.4). Executing script bytecode is out of
scope.

<a id="provenance"></a>
## Provenance

How sources are marked:

- **Confirmed**: decoded on the user's own install on 2026-10-04 with a throwaway probe in
  the dirty-room scratchpad. Each Confirmed fact says what was decoded and how much.
- **Reported**: comes only from published third-party material and was not, or could not
  be, checked against data.
- **BIN**: read from the client executable under method M4 (decision 0011). A fact marked
  "Confirmed (BIN + OBS)" was read from the code and also holds in the data; "BIN only"
  means the data cannot show it (for example, how a value the data never contains is
  treated).

| Key | Source |
|---|---|
| OBS | Observed in the user's install, 2026-10-04 (Steam install, client data current on that date). The single 0x56 record (0x56000000, 3,420,458 bytes, stored uncompressed) was decoded **to the last byte**: all 6 chunks, all 5,822 class-table entries (2,129 defined), all 1,108 attribute lists (7,855 attribute entries), all 5,822 parent entries. The library was then used to decode all 383,343 object-state records (`wstate.md` §9), which cross-checks §6–§9 against 5.8 million serialized instances. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.0 (Apache-2.0 per its distribution's LICENSE), studied as a decompiled binary for facts only. |
| MK | Mirarkitty `lotro_extractor` documentation (Apache-2.0), `docs/wlib.md`. Facts only. |
| DS | `research/notes/data-semantics.md` §5.1. |
| ER | `research/notes/engine-client-re.md` §9 (client memory observations: slot type codes, native package flag). |
| OBS2 | Observed in the same install on 2026-10-05: the library re-decoded to the last byte with a new throwaway probe (same counts as OBS), plus the method-table cross-checks of §6.4. |
| LC2 | The class-index constants of three released builds of the LC library (versions 11.0, 11.4 and 11.6, dated 2023-11, 2024-08 and 2025-05), read for facts only. |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, compiled 2026-09-27), disassembled on 2026-10-05 in the dirty room, address ranges only: the library record's load routine, its chunk dispatcher and the per-chunk readers, the class-table entry reader, and the script machine's function-call and value-type checks. **Questions:** what the header words and the trailing byte are and which values the client accepts; which chunk types the client knows; what the method tables mean; how value codes 131 and 195 differ. **Why M1–M3 were not enough:** the data contain one library at one version with one value of each header word, so they cannot show what is checked or meant; the method tables are offsets into bytecode whose meaning only the code that uses them shows; no published source describes them (M2); the running client (M3) does not expose them. Nothing from network, login, launcher or anti-tamper code was read. **Repair pass (same build, 2026-10-05):** the chunk dispatcher's jump table and the library object's set-up and tear-down routines were re-read. **Question:** where the client keeps chunks 32 and 256 relative to chunks 64 and 512 (merged, replaced or separate). **Why M1–M3 were not enough:** neither chunk occurs in the data, and no published source describes them. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| W1 | Record header: DID, stop code, version, one extra u32 at version 38 (§3.1) | LC, MK, DS; OBS | Confirmed for version 38 (the only version present) |
| W2 | Chunk framing: marker 0xFEDC1234, type, size, payload; stop code followed by one byte (1 here; a boolean, W16) (§3.2) | LC, MK; OBS (6 chunks + stop, zero leftover) | Confirmed |
| W3 | Chunk type numbering (§3.3) | LC; OBS for types 1, 2, 16, 64, 512, 1024; BIN (W17) | Confirmed for the six present types; LC's names for absent types Reported; the client's handling of each type per W17 |
| W4 | Class-table entry layout (§6.1) | LC, MK; OBS (5,822 entries) | Confirmed |
| W5 | Instance size = 8 × slot count; reference-slot list = exactly the slots typed "reference" (§6.2) | OBS (2,129 of 2,129 classes) | Confirmed. Derived by this spec writer. |
| W6 | Value type codes 1, 2, 3, 130, 131, 195 (§5) | LC, ER; OBS | Confirmed (meanings and codes 4, 132: W19) |
| W7 | 64-bit types take two consecutive slots (§5, §6.2) | OBS (166 of 166 such attributes) | Confirmed. Derived by this spec writer. |
| W8 | Chunk 64 lists the classes that have a singleton record at 0x76000000 + class index (§7) | OBS (11 of 11, both directions) | Confirmed. Derived by this spec writer; LC calls the chunk "unknown". |
| W9 | Attribute-list layout (§8.1) | LC, MK, DS; OBS | Confirmed |
| W10 | Attribute lists are already **flattened**: a class's list contains every inherited attribute (§8.3) | OBS (542 of 542 classes whose ancestor has a list) | Confirmed. Corrects DS, MK and ARCHITECTURE §7.4 (see §12). |
| W11 | Instances serialize attribute values in attribute-list order (§8.4) | LC; OBS, OBS2 (all 1,381,857 class-table entries of all 383,343 object-state records equal the list, in order; all 383,343 records decode exactly) | Confirmed |
| W12 | Attribute IDs are name hashes per `records.md` §1.5 (§8.2) | LC, MK; OBS (1,038 of 1,038 hashes whose candidate name is known from a third-party list reproduce exactly) | Confirmed |
| W13 | Parent-map layout and the "no parent" value 0xFFFFFFFF (§9) | LC, MK; OBS | Confirmed |
| W14 | Undefined class-table entries are the native classes that object-state records decode with built-in codecs (§6.3) | OBS, OBS2 (all 34 native class indices seen in object state are undefined; every defined class index seen decodes as a script object); BIN (all 258 registered native classes are undefined entries) | Confirmed |
| W15 | Older container versions 16, 30, 36 (no per-chunk size at 16; 16 skipped bytes at 36) | LC, MK, DS | Reported. The current client accepts only version 38 (W16). |
| W16 | Load order and checks: signature 0x1A015342 right after the DID, version must be 38, then one stored word, then the chunk stream; the trailing byte is a separate boolean field (0 or 1) (§3.1, §3.2) | BIN; OBS (one record) | Confirmed (BIN + OBS) for the observed values; acceptance of other values is BIN only |
| W17 | Chunk types the client parses, skips or rejects (§3.3) | BIN | BIN only for the types absent from the data; Confirmed (BIN + OBS) for the six present types |
| W18 | Chunk 128 holds per-class reference-type constraints used to check references after an object is loaded; it is absent here, so no check runs (§3.3, §4.3) | BIN | BIN only |
| W19 | Valid slot type codes are 1, 2, 3, 4, 130, 131, 132, 195; 4 and 132 are untyped numbers that match an integer or float declaration; bit 0x40 marks a time stamp (195 = time in seconds, −1.0 = unset; 131 = plain double) (§5) | BIN; OBS (defaults: 47 of 47 code-195 attributes default to −1.0; codes 4 and 132 never occur in the library) | Confirmed (BIN + OBS) |
| W20 | Method tables: `method_offsets` is the class's virtual-function table (bytecode offsets); `method_indices` maps each function the class declares to a slot of that table; a function ID is (declaring class index × 65,536 + function number) (§6.4) | BIN; OBS2 (cross-checks over all 2,129 classes) | Confirmed (BIN + OBS) |
| W21 | `class_index` and `method_indices` values are read as signed 16-bit (0xFFFF = −1); `instance_size` is the byte length of the slot block (§6.1) | BIN; OBS | Confirmed (BIN + OBS) |
| W22 | Three singleton classes (1547, 1556, 1603) also root ordinary 0x70 records (1, 2 and 1 records) (§7) | OBS2 | Confirmed |
| W23 | Class indices of the well-known object kinds were the same in LC builds from 2023-11 to 2025-05; all 36 distinct indices named there are root classes of records in this install (§13) | LC2; OBS2 (root-class census) | Reported (stability across builds); Confirmed that the indices are in use here |
| W24 | The client keeps chunk 32 and chunk 256 in their own places, apart from chunk 64 and chunk 512: neither is merged into nor replaces the other (§3.3, §10) | BIN (repair pass) | BIN only (neither chunk occurs in the data) |
| W25 | A parent index is larger than its child's index in 282 defined classes and in 296 of all 5,822 entries (§9) | OBS2 (re-counted 2026-10-05) | Confirmed |

---

<a id="s1"></a>
## 1. Concepts

| Term | Meaning |
|---|---|
| **Class index** | A u16-range integer naming a class. The class table (§6) has one entry per index, 0 to *N* − 1 (*N* = 5,822 here). Indices are build-specific: they are stable within one install, but a reader must not assume they are stable across client updates, except for the native classes listed in `wstate.md` §5.3. |
| **Script class** | A class whose class-table entry is **defined** (§6). Its instances are serialized as attribute values in the order of its attribute list (§8). 2,129 here. |
| **Native class** | A class index whose class-table entry is **undefined**. Its instances are engine containers (hash maps, arrays, strings, positions, property sets, ...) with a built-in codec chosen by class index (`wstate.md` §5.3). |
| **Slot** | A 32-bit cell of a script-class instance. A class with *S* slots has an in-memory instance of 8·*S* bytes (a value plus a type tag per slot, Confirmed BIN). 64-bit values take two slots. |
| **Attribute** | A named, typed member of a script class: (name hash, first slot index, type code). |
| **Name hash** | The 28-bit hash of an identifier (`records.md` §1.5). Attribute names themselves are **not** in the data; only their hashes are. |
| **Singleton class** | A class listed in chunk 64 (§7). It has one object-state record of its own at 0x76000000 + class index; it may also root ordinary 0x70 records (W22). |
| **Function ID** | How native code names a script function: the declaring class index (15 bits) times 65,536, plus the function's number within that class (16 bits) (§6.4). Interop identifier; not needed to decode state. |

---

<a id="s2"></a>
## 2. Locating the library

- DID **0x56000000**, archive `client_gamelogic.dat` (routing as for every 0x56 DID,
  `dat-container.md`). It is the only 0x56 record in this install (census of all
  gamelogic entries, OBS).
- The DID map labelled **WLIB** (0x2800000D, `property-system.md` §8.1) names it: label
  "Client" → 0x56000000. This label is the only one a reader needs (interoperability
  identifier). The map's other entry points at a DID that no archive of this install
  contains; readers ignore it. A reader should look the library up through the WLIB map's
  "Client" label and fall back to 0x56000000.
- There is no property set for the library (0x5F000000 does not exist).

---

<a id="s3"></a>
## 3. Container layout

<a id="s3-1"></a>
### 3.1 Header (Confirmed)

| Offset | Size | Type | Field | Observed |
|---|---|---|---|---|
| 0 | 4 | DID | did | 0x56000000 |
| 4 | 4 | u32 | stop_code (the library **signature**) | 0x1A015342 |
| 8 | 4 | u32 | version | 38 |
| 12 | 4 | u32 | extra (present only when version = 38) | 0x1E4A5AA4 |

What the client does (W16, BIN): after the generic DID it reads the signature and fails the
load unless it equals **0x1A015342**; it then reads the version and fails unless it equals
**38**; it then reads the extra word and keeps it with the library. The same signature value
ends the chunk stream (§3.2). The field keeps its name `stop_code` here because existing code
uses it; "signature" is the better description.

Reported (LC, MK, DS) for other versions: version 36 has 16 header bytes after the version
instead of 4; versions 16 and 30 have no extra field. The current client rejects them. Tessera
accepts 38; it may keep reading 30 and 36 as LC describes (they cannot be tested against any
data), and rejects every other version with a clear error.

> **OPEN:** What the extra word means. The client stores it and does not use it while
> loading; no reader of it was found. It is not needed to decode.

<a id="s3-2"></a>
### 3.2 Chunk stream (Confirmed)

After the header, the record is a sequence of elements until the end of the payload. Each
element starts with a u32 *m*:

- *m* = **0xFEDC1234**: a chunk. It continues with u32 *type* and, at versions 30 and 38,
  u32 *size* followed by *size* bytes of chunk payload. (At version 16 there is no size
  field and the chunk body follows directly, Reported; such a reader must understand every
  chunk type to find the next one.)
- *m* = **stop_code**: the end of the chunk stream. The record then has one more field, a
  **boolean byte** (0 or 1) that belongs to the library record rather than to the chunk
  stream. The client fails the load when it is 2 or more (W16, BIN). It is 1 here. LC says
  this byte is absent in the sister game's data (Reported).
- Anything else: malformed.

Observed: six chunks, then the stop code and its byte 1, then exactly the end of the
payload.

| Order | Type | Size (bytes) | Content | § |
|---|---|---|---|---|
| 1 | 1 | 1,919,908 | Bytecode | §4.1 |
| 2 | 2 | 439,827 | Messages | §4.2 |
| 3 | 16 | 975,654 | Class table | §6 |
| 4 | 64 | 48 | Singleton classes | §7 |
| 5 | 512 | 61,636 | Attribute lists | §8 |
| 6 | 1024 | 23,292 | Parent map | §9 |
| — | stop | 1 | `01` | |

Every chunk's payload is consumed exactly by the layouts in §4–§9 (zero leftover bytes in
each).

The client reads the size field of every chunk before it looks at the type, at version 38
(BIN). Chunks may come in any order; the client dispatches each by type.

> **OPEN:** What the boolean byte after the signature controls. It is not needed to decode.

<a id="s3-3"></a>
### 3.3 Chunk types

The type values are single bits. The table gives the meaning used here, what the current
client does with the chunk (W17, BIN), and whether it is present.

| Type | Name used here | Client (BIN) | Present here |
|---|---|---|---|
| 1 | bytecode | parsed (§4.1) | yes |
| 2 | messages | parsed (§4.2) | yes |
| 4 | (LC: export table of class and function names) | **skipped** by size | no |
| 8 | (LC: reference variables) | **skipped** by size | no |
| 16 | class table | parsed (§6) | yes |
| 32 | second class-index list | parsed with the same reader as type 64 (u32 count, then that many u32) and kept in a list of its own, apart from chunk 64's (W24) | no |
| 64 | singleton classes | parsed (§7) | yes |
| 128 | reference-type constraints | parsed: a TSize-prefixed hashed collection keyed by class index; its content is, per class, a list of (slot, type code, required class) triples (W18); the serialized element layout is OPEN | no |
| 256 | second attribute-list chunk | parsed with the same reader as type 512 (§8.1 layout) into a table of its own, apart from chunk 512's (W24) | no |
| 512 | attribute lists | parsed (§8) | yes |
| 1024 | parent map | parsed (§9) | yes |
| 2048, 4096, 8192, 16384 | (unknown) | **skipped** by size | no |
| any other value | | the load **fails** | |

LC's numbering of further chunk types (3 import table, 5 synthetic package table, 6 virtual
table, 7 engine function table, 257, 258 source line numbers, 259 frame information) is
Reported only; the current client rejects all of them, so they belong to other builds or the
sister game.

A Tessera reader skips any chunk it does not need by its size field. It should accept the
types the client accepts and report other types as malformed (a warning is enough if it
can still skip them by size). The shipped library carries no names, so class and attribute
names are not recoverable from this install (§8.2).

> **OPEN:** The meaning of chunk 32 (a list of class indices like chunk 64), the meaning of
> chunk 256 (attribute lists like chunk 512) and which client code uses either, and the
> exact element layout of chunk 128. None of them occurs in this install. §10 gives the
> reader rule for chunks 32 and 256 until this is answered.

---

<a id="s4"></a>
## 4. Chunks that a state decoder can skip

<a id="s4-1"></a>
### 4.1 Bytecode (type 1, Confirmed layout)

u32 length *L* (1,919,904), then *L* bytes of compiled script code. The method tables of
§6.1 hold byte offsets into this block (all 124,804 non-null offsets are < *L*). The
instruction set is not specified and is out of scope.

<a id="s4-2"></a>
### 4.2 Messages (type 2, Confirmed layout)

| Field | Type | Observed |
|---|---|---|
| message_count *n* | u32 | 4,525 |
| messages | UTF-16 string (`records.md` §1.4) × *n* | |
| array_count *m* | u32 | 0 |
| arrays | (u32 count, count × u32) × *m* (Reported LC) | none |

The message texts are not interpreted by this spec and are not shown to players by Tessera.

<a id="s4-3"></a>
### 4.3 Reference-type constraints (type 128, BIN only)

When this chunk is present, the client checks every script object after an object-state
bundle is loaded:

- each listed attribute's slot type must match the declared type. An untyped number matches
  a declared integer or float of the same width: 4 matches 2 or 3, 132 matches 130 or 131,
  but nothing matches 195 except 195;
- each non-null reference must point at an object whose class is the required class or one
  of its descendants (parent map, §9).

The load fails if a check fails. The chunk is absent from this install, so the client
performs no such check here and a Tessera reader needs none.

---

<a id="s5"></a>
## 5. Value type codes (Confirmed)

The same codes are used in the class table's slot types (§6.1), in attribute lists (§8.1)
and in object-state class tables (`wstate.md` §4.2).

| Code | Kind | Slots | Serialized size in object state | Count in attribute lists |
|---|---|---|---|---|
| 1 | Reference to another item of the same object-state record (or null) | 1 | 4 (u32) | 3,721 |
| 2 | Integer (32-bit; signedness not encoded) | 1 | 4 (u32) | 2,648 |
| 3 | Float | 1 | 4 (f32) | 1,320 |
| 130 | 64-bit integer | 2 | 8 (u64, low word first) | 116 |
| 131 | Double (plain number) | 2 | 8 (f64, low word first) | 3 |
| 195 | **Time stamp**: a double holding a time in seconds; −1.0 means "unset" | 2 | 8 (f64, low word first) | 47 |
| 4 | Untyped 32-bit number (BIN only) | 1 | never in files | 0 |
| 132 | Untyped 64-bit number (BIN only) | 2 | never in files | 0 |

The code is a small bit field (W19, BIN):

- the low four bits give the base kind: 1 reference, 2 integer, 3 float, 4 untyped number;
- bit **0x80** marks a 64-bit kind that takes two slots;
- bit **0x40** (only in 195) marks a time value. The script machine produces 195 values for
  clock readings and elapsed times, and uses −1.0 when there is no clock; plain arithmetic
  results are 131. This matches the data: every code-195 slot defaults to −1.0 (47 of 47
  attributes, §6.2), every code-131 slot to 0.

The client accepts exactly the eight codes of the table in a slot. Codes 4 and 132 appear only
in values built at run time (an untyped number matches a declared integer or float of the
same width, §4.3); they never occur in the library or in object-state records. Any other code
in a file is malformed.

Both 131 and 195 decode as IEEE binary64. A reader that shows values may print a 195 value
of −1.0 as "unset".

---

<a id="s6"></a>
## 6. Class table (chunk type 16)

<a id="s6-1"></a>
### 6.1 Layout (Confirmed)

| Field | Type | Meaning |
|---|---|---|
| entry_count *N* | VLE | Number of class indices (5,822 here). Entry *i* describes class index *i*. |
| entries | entry × *N* | In class-index order |

Each entry:

| Field | Type | Present when | Meaning |
|---|---|---|---|
| defined | bool8 | always | 1 = script class (fields below follow); 0 = native class or unused index (nothing follows). The client fails the load on 2 or more (BIN). |
| class_index | i16 | defined | Equals the entry position *i* (2,129 of 2,129). The client reads it as signed (W21). |
| slot_count *S* | u16 | defined | Number of 32-bit slots |
| instance_size | u32 | defined | Byte length of the slot table that follows (the client reads the slot table as one length-prefixed block, W21). Always 8 × *S* (2,129 of 2,129). |
| slot table | (u32 default, u32 type code) × *S* | defined | Default value and type code (§5) of each slot, in slot order. It is the initial image of an instance. |
| ref_slot_count *R* | VLE | defined | |
| ref_slots | u16 × *R* | defined | Slot indices of reference type |
| method_count *M* | VLE | defined | |
| method_offsets | u32 × *M* | defined | The class's **virtual-function table**: byte offsets into the bytecode (§4.1); 0xFFFFFFFF = no body (§6.4) |
| index_count *K* | VLE | defined | |
| method_indices | i16 × *K* | defined | For each function the class itself declares (by function number), its slot in `method_offsets`; 0xFFFF (−1) = none (§6.4) |

The client checks each VLE count against the bytes left before it allocates (at least one
byte per element) and fails the load otherwise (BIN). Tessera's tighter bounds of §10 also
hold for all valid data.

<a id="s6-2"></a>
### 6.2 Facts about the slot table (Confirmed, all 2,129 defined classes)

- *S* ranges from 0 (1,021 classes) upwards. The most common non-zero counts are 1 (322),
  2 (177) and 3 (142).
- A 64-bit attribute (codes 130, 131, 195) occupies slots *s* and *s* + 1, and **both**
  slots carry the attribute's type code (166 of 166).
- Defaults by type: reference slots default to **0xFFFFFFFF** (the null reference, 3,721
  of 3,721); integer, float and 64-bit-integer slots default to 0. A code-195 double has
  default words (low 0, high 0xBFF00000), that is −1.0, in 47 of 47. Code 131 defaults to 0.
- `ref_slots` lists exactly the slots whose type is 1, in ascending order (2,129 of
  2,129). It is redundant with the slot table and can serve as a check.
- `method_offsets` is never empty. The first element is 0xFFFFFFFF in 468 classes, and in
  exactly those classes `method_indices` starts with 0xFFFF. Every non-0xFFFF method index
  is < *M*. A class's method table is at least as long as its parent's (1,496 of 1,496).

The slot table alone does not give attribute names or the serialization order; §8 does.

The method tables are explained in §6.4.

<a id="s6-3"></a>
### 6.3 Undefined entries (Confirmed)

3,693 entries are undefined. They include every **native class** that object-state records
use (34 distinct indices observed, `wstate.md` §5.3), and unused indices. Rule for
decoders: an embedded object whose class index has a defined entry is a script object;
every other class index must be handled by a native codec (`wstate.md` §5.3) or the object
cannot be decoded.

The client knows a fixed set of native classes (258 class indices in this build, all
undefined in the library; W14 and `wstate.md` §5.3). The other 3,435 undefined indices are
unused.

<a id="s6-4"></a>
### 6.4 Method tables (W20, Confirmed BIN + OBS)

Needed only by a bytecode interpreter; a state decoder skips them.

- `method_offsets` is the class's **virtual-function table**. Slot *v* holds the bytecode
  offset of the function bound to slot *v* for this class, or 0xFFFFFFFF when the slot has
  no body.
- `method_indices` belongs to the functions the class itself declares. Entry *f* gives the
  virtual-table slot of the class's function number *f*, or −1 when that function has no
  slot.
- A **function ID** names a function as (declaring class index × 65,536 + function number
  *f*). The declaring class index has 15 bits; the client masks it with 0x7FFF.
- **Static call** of function ID (*P*, *f*): slot *v* = `method_indices[f]` of class *P*,
  then the body is at `method_offsets[v]` of class *P*.
- **Virtual call** of (*P*, *f*) on an object of class *C*: *v* is taken from class *P* as
  above, and the body is at `method_offsets[v]` of class *C*. A slot index outside *C*'s table,
  a −1 index or a 0xFFFFFFFF offset is a "missing function" error.

Cross-checks over all 2,129 classes (OBS2):

- every non-negative entry of `method_indices` is < *M* and the entries are unique within a
  class (2,129 of 2,129);
- for every class with a defined parent (1,496), the slots beyond the parent's table length
  are all named by the class's own `method_indices` (1,496 of 1,496), and in 1,487 of them
  every slot whose offset differs from the parent's is also named by the class's own
  `method_indices` (overrides). Nine classes differ from the parent in a slot they do not
  name.
- `method_offsets[0]` is 0xFFFFFFFF exactly when `method_indices[0]` is −1 (468 classes).

---

<a id="s7"></a>
## 7. Singleton classes (chunk type 64, Confirmed)

| Field | Type | Observed |
|---|---|---|
| count *n* | u32 | 11 |
| class indices | u32 × *n* | 387, 497, 1053, 1547, 1556, 1603, 1629, 1630, 1783, 3431, 3432 (stored in that order: 3432, 1783, 1629, 497, 387, 1630, 1556, 1547, 1603, 1053, 3431) |

Each listed class *c* is defined and has exactly one object-state record at DID
**0x76000000 + *c***, whose class index (offset 4) is *c*. Conversely, every 0x76 record in
the install (11) is such a singleton. All 11 classes descend from class 486 in the parent
map. The list order has no evident meaning.

"Singleton" describes the 0x76 record, not the class: classes 1547, 1556 and 1603 also root
ordinary 0x70 records (1, 2 and 1 records; W22). The client reads this chunk with the same
reader as chunk 32 (§3.3).

---

<a id="s8"></a>
## 8. Attribute lists (chunk type 512)

<a id="s8-1"></a>
### 8.1 Layout (Confirmed)

| Field | Type | Meaning |
|---|---|---|
| class_count *n* | TSize | Observed hint 7 (*B* = 1,531), *n* = 1,108 |
| classes | class entry × *n* | In **bucket order by class index** (`records.md` §1.3.1) |

Class entry:

| Field | Type | Meaning |
|---|---|---|
| class_index | u32 | A defined class (1,108 of 1,108) |
| attribute_count *a* | TSize | Observed hints 0 (978 lists), 1 (120), 2 (10); *a* from 1 to 86 |
| attributes | attribute × *a* | In **bucket order by name hash** (1,108 of 1,108 lists) |

Attribute:

| Field | Type | Meaning |
|---|---|---|
| name_hash | u32 | Name hash of the attribute (§8.2) |
| slot | u16 | First slot index of the attribute |
| type | u8 | Value type code (§5) |

A defined class with no entry in this chunk has **no attributes** (1,021 classes; all of
them have *S* = 0). Every class with an entry has *S* ≥ 1.

<a id="s8-2"></a>
### 8.2 Attribute identity (Confirmed)

- Within one class, name hashes are unique (1,108 of 1,108).
- All 2,554 distinct hashes are below 0x10000000, as the 28-bit name hash requires.
- Where a candidate name is known from a published third-party list (1,038 of the 2,554
  hashes), hashing it with `records.md` §1.5 reproduces the stored value in every case.
- Names are **not** stored anywhere in this install's library. Tessera shows attributes as
  hashes (for example `attr_0B3FA2E`) and may attach names that it can verify by hashing a
  candidate string. It must not ship third-party name lists (ARCHITECTURE §7.4).

<a id="s8-3"></a>
### 8.3 Inheritance is already flattened (Confirmed)

For each class *c* with an attribute list, let *p* be the nearest ancestor (§9) that has an
attribute list. In all 542 such classes:

- *c*'s list contains **every** entry of *p*'s list, with the same hash, slot and type;
- *c*'s own additional attributes (167 classes add some, 375 add none) occupy the slots
  after *p*'s last slot;
- the slots of *c*'s list cover 0 … *S*(*c*) − 1 exactly once (counting 64-bit attributes as
  two slots), for all 1,108 lists.

So a class's list is its **complete** attribute set. A reader must **not** prepend or append
the parent's list: doing so would read inherited values twice.

<a id="s8-4"></a>
### 8.4 Serialization order (Confirmed)

An instance of class *c* in an object-state record stores its attribute values **in the
order of *c*'s list as stored in this chunk** (bucket order by name hash), not in slot
order. The two orders differ in 668 of the 1,108 classes. Evidence: every object-state
record repeats the list for each class it uses (`wstate.md` §4.2), and all 1,381,857 such
repetitions (in all 383,343 records) equal this chunk's list exactly, in order.

This spec describes the stored form in the archives only.

---

<a id="s9"></a>
## 9. Parent map (chunk type 1024, Confirmed)

| Field | Type | Meaning |
|---|---|---|
| count | u32 | Equals the class-table entry count *N* (5,822) |
| parents | u32 × count | Entry *i* = parent class index of class *i*; **0xFFFFFFFF = no parent** |

Facts:

- 1,497 defined classes have a parent and 632 are roots. The parent of a defined class is
  itself defined, with one exception (class 488, whose parent 223 is undefined).
- 35 undefined indices also have parents. For example, the native position classes form a
  chain 160 → 163 → 161 (`wstate.md` §5.3). Native-class parents do not affect decoding.
- Depth (number of ancestors) reaches 17. A parent index can be larger than its child's
  (282 of the 2,129 defined classes; 296 of all 5,822 entries, counting the undefined
  indices that have parents; W25), so a reader resolves the map after reading it
  completely, not in one pass.
- No cycles exist. Readers must still guard against them (stop after *N* steps).
- Every root class of an object-state record in prefix 0x70 descends from class 485
  (383,332 of 383,332); see `wstate.md` §11 for the classes used to resolve areas.

Reported (MK): some tools treat 0 as "no parent". In this install no entry is 0, and class
0 is undefined, so treating 0 as "no parent" too is harmless.

---

<a id="s10"></a>
## 10. Building the class library

In words:

1. Read the header (§3.1) and walk the chunk stream (§3.2), keeping the payloads of types
   16, 64, 512 and 1024. Missing any of the four is an error.
2. Decode the class table (§6.1) into an array indexed by class index; undefined entries
   are empty.
3. Decode the attribute lists (§8.1). For each class entry, check that the class is
   defined and store the list **as stored** (this order is the serialization order). Check
   each attribute: its type equals the slot table's type at `slot` (and at `slot` + 1 for
   64-bit types).
4. Decode the parent map (§9) into the same index space.
5. Decode the singleton list (§7).
6. Derived views (computed once): ancestor chain of each class; "is-a" queries; for each
   class, the attribute list (no flattening needed, §8.3); the set of reference attributes.

Counts read from the data must be bounded by the bytes left before allocating: at least 1
byte per class-table entry, 6 bytes per attribute-list class entry, 7 bytes per attribute,
4 bytes per parent entry.

Cases the data never contain, decided here (answers the implementation OPEN of 2026-10-04):

| Case | Rule |
|---|---|
| A parent-map entry ≥ the class count (other than 0xFFFFFFFF) | Malformed (reject). |
| A class that is its own parent, or any cycle | Malformed (reject). The client never checks for cycles; it never meets one. |
| Chunk type 16, 64, 512 or 1024 present twice | Malformed (reject). The client would re-read the second copy over the first; no data does this. |
| A chunk type outside §3.3's accepted set | Malformed; a reader may instead warn and skip it by size. |
| Version ≠ 38 | Unsupported. Reading 30 and 36 per LC (W15) is optional; the version-36 header is then the version followed by four u32 words whose meaning is unknown (LC's "16 bytes"). |
| Trailing boolean byte ≥ 2 | Malformed. 0 is valid. |
| Chunk 256 present (second attribute-list chunk) | Decode it with the §8.1 layout (a payload that does not parse, or leaves bytes over, is malformed) and keep it as a separate table. Do **not** merge it into chunk 512's lists and do not let it replace them: the serialization order of §8.4 comes from chunk 512 only, as in the client, which stores the two apart (W24). Report a warning that the chunk's meaning is unknown (§13). |
| Chunk 32 present (second class-index list) | Decode it with the §7 layout (u32 count, then that many u32; same parse rule) and keep it as a separate list. Do not add its classes to the singleton list, which comes from chunk 64 only (W24). Report a warning that the chunk's meaning is unknown (§13). |
| Chunk 256 or 32 present twice | Malformed (reject), as for the four chunks above. |

<a id="s11"></a>
## 11. Invariants (all Confirmed on this install)

| # | Invariant |
|---|---|
| L1 | Each chunk payload is consumed exactly; the record ends right after the boolean byte that follows the stop code (§3.2). |
| L2 | Class-table entry *i*, when defined, has `class_index` = *i*. |
| L3 | `instance_size` = 8 × `slot_count`. |
| L4 | `ref_slots` = the ascending list of slots typed 1. |
| L5 | Every class in an attribute list is defined; within a list, hashes are unique; slot ranges tile 0 … *S* − 1. |
| L6 | An attribute's type equals the slot-table type of each slot it occupies. |
| L7 | Parent-map length = class-table length. |
| L8 | A class's attribute list contains the list of its nearest listed ancestor (§8.3). |
| L9 | Every singleton class index is defined and has a record at 0x76000000 + index. |

A reader reports a violation of L1–L7 as malformed data. L8 and L9 are cross-checks: a
violation is a warning, since only the stored list matters for decoding.

---

<a id="s12"></a>
## 12. Disagreements with other sources

| Source | Claim | Finding |
|---|---|---|
| DS §5.1, MK `docs/wlib.md`, ARCHITECTURE §7.4 | "Attributes are not flattened from the parent class"; ARCHITECTURE adds that the library must compute the flattened view. | The stored list of a derived class **already contains** the inherited attributes (§8.3). MK's practical advice (decode with the class's own list only, never walk parents) is right; the reason given is not. ARCHITECTURE §7.4 should say "the attribute list is complete; do not merge parents". |
| MK `docs/wlib.md`, ER | Chunk marker written as 0xB8C577F4. | The marker is 0xFEDC1234 (the same value read as a signed 32-bit integer is −19,131,852). DS already notes this. |
| DS §5.1 | CLASS_DEFS pair is "(u32 default, u32 type)". | Confirmed; the pairs are the per-slot table, one per slot, and 64-bit attributes contribute two (§6.2). |
| LC | Chunk 64 "unknown". | It is the singleton-class list (§7). |
| LC | Chunk types 3, 5, 6, 7, 257, 258, 259 exist. | The current client rejects them; it knows 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024 and 2048–16384 (§3.3). |
| LC | The byte after the stop code must be 1. | It is a boolean; 0 is valid too (§3.2). |
| LC | Code 131 is a "signed" double. | 131 is a plain double; 195 is a time stamp (§5). |

---

<a id="s13"></a>
## 13. Open questions

Closed on 2026-10-05 (kept here so earlier references still resolve):

- *Stop code* — it is the library signature (§3.1). The extra word stays OPEN below.
- *Codes 131 and 195* — 131 is a plain double, 195 a time stamp (§5).
- *Method tables* — virtual-function table and its index map (§6.4).
- *Class names* — not an open question: the shipped library carries no names (no type-4
  chunk, which the client skips anyway) and the client needs none. Tessera identifies
  classes by index and by the property signature of their objects (`wstate.md` §11), never
  by name.
- *Class-index stability* — seven well-known root classes (quests and deeds, monsters,
  NPCs, titles, emotes, relics, legendary titles) carry the same indices in all three LC
  builds (2023-11, 2024-08, 2025-05), and every one of the 36 distinct indices named by the
  2023-11 build (items, effects, skills and others) is the root class of records in this
  install (W23; the kinds were not re-verified by property signature). Indices therefore
  change rarely if at all. Readers still must
  not hard-code them; `wstate.md` §11.1 gives a data-driven way to find them.
- *Implementation OPEN of 2026-10-04* — answered by the table in §10: the implemented
  rejections are the rule; the version-36 reading is acceptable as optional, unverifiable
  support.

Still open:

> **OPEN:** Meaning of the version-38 extra header word (§3.1) and of the boolean byte after
> the signature (§3.2). Neither is needed to decode.

> **OPEN:** Meaning of chunks 32 and 256 (and which client code reads them), and the element
> layout of chunk 128 (§3.3, §4.3). None occurs in this install; §10 gives the interim reader
> rule for 32 and 256.

---

<a id="s14"></a>
## 14. Test guidance

Synthetic fixtures (generated by our own code, never copied from the install):

- a minimal library: header with version 38, one class-table chunk with three entries
  (undefined; a root class with one integer and one reference attribute; a child class
  that repeats the root's two attributes and adds a 64-bit integer), an attribute-list
  chunk in bucket order, a parent map with 0xFFFFFFFF for roots, a singleton chunk with one
  index, an unknown chunk type that must be skipped by size, the stop code and byte 1;
- malformed variants: a wrong chunk marker; a stop code followed by a byte of 2 or more
  (0 and 1 are both valid, §3.2, §10); a class-table count larger than the remaining bytes (must fail without allocating);
  a defined entry whose `class_index` differs from its position; an attribute-list class
  that is undefined; an attribute whose type disagrees with the slot table; a 64-bit
  attribute whose second slot has a different type; a parent-map cycle;
- version 30 (no extra word) must parse if the optional support is implemented, else fail
  with "unsupported version"; version 16 must either parse (if implemented) or fail with
  "unsupported version";
- the trailing boolean byte 0 must parse; 2 must fail;
- a chunk of type 4 or 2048 must be skipped by size; a chunk of type 3 must be reported
  (error or warning, §10);
- a library with an extra chunk 256 whose list for the root class differs from chunk 512's:
  it must load with a warning, and the attribute list (and so the serialization order) must
  still be chunk 512's; a chunk 256 whose payload is truncated must fail; a library with a
  chunk 32 listing a class must load with a warning, and the singleton list must still be
  chunk 64's alone;
- method-table checks (§6.4) on the synthetic classes: a child class whose table extends its
  parent's and whose `method_indices` name the new slots.

Real-data test (skipped without `TESSERA_GAME_DIR`): load 0x56000000 and assert invariants
L1–L9. Report counts (entries, defined classes, attribute lists) in the test output; do not
assert the exact counts, which change with client updates.

---

<a id="s15"></a>
## 15. Corrections to other specs

None of the facts above contradicts a locked spec. Two notes for other spec owners:

- `records.md` §1.3.1 OPEN (order within a bucket): see `wstate.md` §16 for the rule
  observed in object-state class tables.
- ARCHITECTURE §7.4 should say "the attribute list is complete; do not merge parents"
  (§12; unchanged from the first version of this spec).

---

<a id="s16"></a>
## 16. Change log

| Date | Change |
|---|---|
| 2026-10-04 | First version (Review). |
| 2026-10-05 | M4 pass (BIN row). §3.1: `stop_code` is the signature; the client accepts only version 38. §3.2: the trailing byte is a boolean (0 valid). §3.3: chunk table rewritten with the client's parse/skip/reject behaviour; chunks 32, 128, 256 described. New §4.3 (reference-type constraints). §5: codes 4 and 132 added; 131 = plain double, 195 = time stamp. §6.1: signed fields, slot-block length, method-table meaning; new §6.4. §7: singleton classes may root 0x70 records. §10: rules for the malformed cases of the implementation OPEN. §13 reorganised: five OPENs closed, two narrow ones left. New §15, §16. Section numbers 1–14 unchanged. |
| 2026-10-05 | Repair after review (changes requested). **Withdrawn** (out of scope until the `docs/LEGAL.md` §9.3 gate, as in `property-system.md` §10): the LC remark on the network form at the end of §5 and the slot-order remark at the end of §8.4; the spec now states that it covers the stored form only. §14: the malformed trailing byte is "2 or more", matching §3.2 and §10; tests for chunks 256 and 32 added. §9: the 282 parent-after-child cases are scoped to defined classes, with 296 over all entries (new row W25). §2: the speculative remark and the second WLIB label removed; only the "Client" label is needed. §3.3, §10, §13: chunks 32 and 256 are stored apart from 64 and 512 (new row W24, BIN repair pass); §10 gives a reader rule for each, and the OPEN on their meaning is widened. W11 and §8.4 counts now cover all 383,343 records (1,381,857 class-table entries). No section renumbered. |
