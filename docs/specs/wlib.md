<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# WLib: the script class library (DID 0x56000000)

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
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

The other chunks (bytecode, messages, method tables) are described only far enough to skip
them safely. Executing script bytecode is out of scope.

<a id="provenance"></a>
## Provenance

How sources are marked:

- **Confirmed**: decoded on the user's own install on 2026-10-04 with a throwaway probe in
  the dirty-room scratchpad. Each Confirmed fact says what was decoded and how much.
- **Reported**: comes only from published third-party material and was not, or could not
  be, checked against data.

| Key | Source |
|---|---|
| OBS | Observed in the user's install, 2026-10-04 (Steam install, client data current on that date). The single 0x56 record (0x56000000, 3,420,458 bytes, stored uncompressed) was decoded **to the last byte**: all 6 chunks, all 5,822 class-table entries (2,129 defined), all 1,108 attribute lists (7,855 attribute entries), all 5,822 parent entries. The library was then used to decode all 383,343 object-state records (`wstate.md` §9), which cross-checks §6–§9 against 5.8 million serialized instances. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.0 (Apache-2.0 per its distribution's LICENSE), studied as a decompiled binary for facts only. |
| MK | Mirarkitty `lotro_extractor` documentation (Apache-2.0), `docs/wlib.md`. Facts only. |
| DS | `research/notes/data-semantics.md` §5.1. |
| ER | `research/notes/engine-client-re.md` §9 (client memory observations: slot type codes, native package flag). |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| W1 | Record header: DID, stop code, version, one extra u32 at version 38 (§3.1) | LC, MK, DS; OBS | Confirmed for version 38 (the only version present) |
| W2 | Chunk framing: marker 0xFEDC1234, type, size, payload; stop code followed by one byte 1 (§3.2) | LC, MK; OBS (6 chunks + stop, zero leftover) | Confirmed |
| W3 | Chunk type numbering (§3.3) | LC; OBS for types 1, 2, 16, 64, 512, 1024 | Confirmed for the six present types; Reported for the rest |
| W4 | Class-table entry layout (§6.1) | LC, MK; OBS (5,822 entries) | Confirmed |
| W5 | Instance size = 8 × slot count; reference-slot list = exactly the slots typed "reference" (§6.2) | OBS (2,129 of 2,129 classes) | Confirmed. Derived by this spec writer. |
| W6 | Value type codes 1, 2, 3, 130, 131, 195 (§5) | LC, ER; OBS | Confirmed |
| W7 | 64-bit types take two consecutive slots (§5, §6.2) | OBS (166 of 166 such attributes) | Confirmed. Derived by this spec writer. |
| W8 | Chunk 64 lists the classes that have a singleton record at 0x76000000 + class index (§7) | OBS (11 of 11, both directions) | Confirmed. Derived by this spec writer; LC calls the chunk "unknown". |
| W9 | Attribute-list layout (§8.1) | LC, MK, DS; OBS | Confirmed |
| W10 | Attribute lists are already **flattened**: a class's list contains every inherited attribute (§8.3) | OBS (542 of 542 classes whose ancestor has a list) | Confirmed. Corrects DS, MK and ARCHITECTURE §7.4 (see §12). |
| W11 | Instances serialize attribute values in attribute-list order (§8.4) | LC; OBS (1,381,854 class tables in object-state records equal the list, in order; 383,342 records decode exactly) | Confirmed |
| W12 | Attribute IDs are name hashes per `records.md` §1.5 (§8.2) | LC, MK; OBS (1,038 of 1,038 hashes whose candidate name is known from a third-party list reproduce exactly) | Confirmed |
| W13 | Parent-map layout and the "no parent" value 0xFFFFFFFF (§9) | LC, MK; OBS | Confirmed |
| W14 | Undefined class-table entries are the native classes that object-state records decode with built-in codecs (§6.3) | OBS (all 33 native class indices seen in object state are undefined; every defined class index seen decodes as a script object) | Confirmed |
| W15 | Older container versions 16, 30, 36 (no per-chunk size at 16; 16 skipped bytes at 36) | LC, MK, DS | Reported |

---

<a id="s1"></a>
## 1. Concepts

| Term | Meaning |
|---|---|
| **Class index** | A u16-range integer naming a class. The class table (§6) has one entry per index, 0 to *N* − 1 (*N* = 5,822 here). Indices are build-specific: they are stable within one install, but a reader must not assume they are stable across client updates, except for the native classes listed in `wstate.md` §5.3. |
| **Script class** | A class whose class-table entry is **defined** (§6). Its instances are serialized as attribute values in the order of its attribute list (§8). 2,129 here. |
| **Native class** | A class index whose class-table entry is **undefined**. Its instances are engine containers (hash maps, arrays, strings, positions, property sets, ...) with a built-in codec chosen by class index (`wstate.md` §5.3). |
| **Slot** | A 32-bit cell of a script-class instance. A class with *S* slots has an in-memory instance of 8·*S* bytes (a value plus a type tag per slot, Reported ER). 64-bit values take two slots. |
| **Attribute** | A named, typed member of a script class: (name hash, first slot index, type code). |
| **Name hash** | The 28-bit hash of an identifier (`records.md` §1.5). Attribute names themselves are **not** in the data; only their hashes are. |
| **Singleton class** | A class with exactly one instance, stored as its own object-state record at 0x76000000 + class index (§7). |

---

<a id="s2"></a>
## 2. Locating the library

- DID **0x56000000**, archive `client_gamelogic.dat` (routing as for every 0x56 DID,
  `dat-container.md`). It is the only 0x56 record in this install (census of all
  gamelogic entries, OBS).
- The DID map labelled **WLIB** (0x2800000D, `property-system.md` §8.1) names it: label
  "Client" → 0x56000000. Its second label, "ClientPDB" → 0x56000011, is dangling (no such
  record; presumably a debug build of the library with symbols). A reader should look the
  library up through the WLIB map's "Client" label and fall back to 0x56000000.
- There is no property set for the library (0x5F000000 does not exist).

---

<a id="s3"></a>
## 3. Container layout

<a id="s3-1"></a>
### 3.1 Header (Confirmed)

| Offset | Size | Type | Field | Observed |
|---|---|---|---|---|
| 0 | 4 | DID | did | 0x56000000 |
| 4 | 4 | u32 | stop_code | 0x1A015342 |
| 8 | 4 | u32 | version | 38 |
| 12 | 4 | u32 | extra (present only when version = 38) | 0x1E4A5AA4 |

Reported (LC, MK, DS) for other versions: version 36 has 16 header bytes after the version
instead of 4; versions 16 and 30 have no extra field. Readers should accept 30, 36 and 38,
and reject other versions with a clear error. Only 38 has been seen.

> **OPEN:** The meaning of `stop_code` (it is used only as the end marker of §3.2) and of
> the version-38 extra word (a checksum or build stamp?). Neither is needed to decode.

<a id="s3-2"></a>
### 3.2 Chunk stream (Confirmed)

After the header, the record is a sequence of elements until the end of the payload. Each
element starts with a u32 *m*:

- *m* = **0xFEDC1234**: a chunk. It continues with u32 *type* and, at versions 30 and 38,
  u32 *size* followed by *size* bytes of chunk payload. (At version 16 there is no size
  field and the chunk body follows directly, Reported; such a reader must understand every
  chunk type to find the next one.)
- *m* = **stop_code**: the end marker. It is followed by one byte that must be 1 (LC says
  this byte is absent in the sister game's data, Reported).
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

<a id="s3-3"></a>
### 3.3 Chunk types

| Type | Name used here | Status |
|---|---|---|
| 1 | bytecode | Present (§4.1) |
| 2 | messages | Present (§4.2) |
| 3 | import table | Reported (LC), absent |
| 4 | export table (class and function names) | Reported (LC), absent |
| 5 | synthetic package table | Reported (LC), absent |
| 6 | virtual table | Reported (LC), absent |
| 7 | engine function table | Reported (LC), absent |
| 8 | reference variables | Reported (LC), absent |
| 16 | class table | Present (§6) |
| 64 | singleton classes | Present (§7) |
| 256 | function locations | Reported (LC), absent |
| 257 | (unnamed; LC expects a single u32 0) | Reported, absent |
| 258 | source line numbers | Reported (LC), absent |
| 259 | frame information | Reported (LC), absent |
| 512 | attribute lists | Present (§8) |
| 1024 | parent map | Present (§9) |

A reader skips any chunk it does not need by its size field. Chunk types 3, 4 and 258 would
carry class, function and source-file **names**; the shipped library has none of them, so
class and attribute names are not recoverable from this install (§8.2).

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
| 131 | Double, "signed" variant (LC name) | 2 | 8 (f64, low word first) | 3 |
| 195 | Double | 2 | 8 (f64, low word first) | 47 |

Any other code is malformed. Bit 0x80 marks the 64-bit kinds. Reported (LC, network form
only): code 4 is accepted where 2 is expected and 132 where 130 is expected.

> **OPEN:** The difference between codes 131 and 195. Both decode as IEEE binary64 values.

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
| defined | bool8 | always | 1 = script class (fields below follow); 0 = native class or unused index (nothing follows) |
| class_index | u16 | defined | Equals the entry position *i* (2,129 of 2,129) |
| slot_count *S* | u16 | defined | Number of 32-bit slots |
| instance_size | u32 | defined | Always 8 × *S* (2,129 of 2,129) |
| slot table | (u32 default, u32 type code) × *S* | defined | Default value and type code (§5) of each slot, in slot order |
| ref_slot_count *R* | VLE | defined | |
| ref_slots | u16 × *R* | defined | Slot indices of reference type |
| method_count *M* | VLE | defined | |
| method_offsets | u32 × *M* | defined | Bytecode offsets (§4.1); 0xFFFFFFFF = no body |
| index_count *K* | VLE | defined | |
| method_indices | u16 × *K* | defined | Indices into `method_offsets`; 0xFFFF = none |

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

> **OPEN:** The exact meaning of `method_offsets` and `method_indices` (virtual-method
> table and its index map, presumably). Not needed to decode object state.

<a id="s6-3"></a>
### 6.3 Undefined entries (Confirmed)

3,693 entries are undefined. They include every **native class** that object-state records
use (33 distinct indices observed, `wstate.md` §5.3), and unused indices. Rule for
decoders: an embedded object whose class index has a defined entry is a script object;
every other class index must be handled by a native codec (`wstate.md` §5.3) or the object
cannot be decoded.

---

<a id="s7"></a>
## 7. Singleton classes (chunk type 64, Confirmed)

| Field | Type | Observed |
|---|---|---|
| count *n* | u32 | 11 |
| class indices | u32 × *n* | 387, 497, 1053, 1547, 1556, 1603, 1629, 1630, 1783, 3431, 3432 (stored in that order: 3432, 1783, 1629, 497, 387, 1630, 1556, 1547, 1603, 1053, 3431) |

Each listed class *c* is defined and has exactly one object-state record, at DID
**0x76000000 + *c***, whose class index (offset 4) is *c*. Conversely, every 0x76 record in
the install (11) is such a singleton. All 11 classes descend from class 486 in the parent
map. The list order has no evident meaning.

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
record repeats the list for each class it uses (`wstate.md` §4.2), and all 1,381,854 such
repetitions equal this chunk's list exactly, in order.

Reported (LC): the in-memory/network form instead uses slot order (`wstate.md` §10).

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
  (282 cases), so a reader resolves the map after reading it completely, not in one pass.
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

<a id="s11"></a>
## 11. Invariants (all Confirmed on this install)

| # | Invariant |
|---|---|
| L1 | Each chunk payload is consumed exactly; the record ends right after the stop code's byte 1. |
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

---

<a id="s13"></a>
## 13. Open questions

> **OPEN:** Meaning of the stop code and the version-38 extra header word (§3.1).

> **OPEN:** Difference between value type codes 131 and 195 (§5).

> **OPEN:** Semantics of the method tables (§6.1). Needed only for a bytecode interpreter,
> which is not planned.

> **OPEN:** Class names. The shipped library has no export or line-number chunks, so names
> are not in the data. Tessera identifies classes by index and by the property signature
> of their objects (`wstate.md` §11), never by name.

> **OPEN:** Whether class indices of the classes used for areas, territories, regions and
> dungeons (`wstate.md` §11) change between client builds. Readers must not hard-code them;
> `wstate.md` §11.1 gives a data-driven way to find them.

> **OPEN (implementation, 2026-10-04):** The spec does not say how to treat these cases;
> `tessera-wstate` rejects them as malformed (none occurs in the install): a parent-map
> entry ≥ the class count, a class that is its own parent, and a chunk type 16, 64, 512 or
> 1024 that occurs twice. A version-36 header is read as four u32 words after the version
> (§3.1 says "16 header bytes"; their meaning is unknown). Please confirm or correct.

---

<a id="s14"></a>
## 14. Test guidance

Synthetic fixtures (generated by our own code, never copied from the install):

- a minimal library: header with version 38, one class-table chunk with three entries
  (undefined; a root class with one integer and one reference attribute; a child class
  that repeats the root's two attributes and adds a 64-bit integer), an attribute-list
  chunk in bucket order, a parent map with 0xFFFFFFFF for roots, a singleton chunk with one
  index, an unknown chunk type that must be skipped by size, the stop code and byte 1;
- malformed variants: a wrong chunk marker; a stop code followed by a byte other than 1;
  a class-table count larger than the remaining bytes (must fail without allocating);
  a defined entry whose `class_index` differs from its position; an attribute-list class
  that is undefined; an attribute whose type disagrees with the slot table; a 64-bit
  attribute whose second slot has a different type; a parent-map cycle;
- version 30 (no extra word) must parse; version 16 must either parse (if implemented) or
  fail with "unsupported version".

Real-data test (skipped without `TESSERA_GAME_DIR`): load 0x56000000 and assert invariants
L1–L9. Report counts (entries, defined classes, attribute lists) in the test output; do not
assert the exact counts, which change with client updates.
