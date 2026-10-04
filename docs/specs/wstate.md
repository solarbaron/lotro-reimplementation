<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# WState: object-state records (DID 0x70000000–0x77FFFFFF)

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/wstate` (`ObjectState`, `GameObject`, area/region/dungeon resolution); `apps/dattool` / `apps/explorer` object views; `crates/world` and `apps/client` consume §11 |
| **Companion specs** | `docs/specs/wlib.md` (the class library every record is decoded with); `docs/specs/records.md` [§1](records.md#s1) (VLE, TSize and bucket order, strings, name hash), [§4](records.md#s4) (StringInfo layout); `docs/specs/property-system.md` [§4](property-system.md#s4) (value encodings, position §4.6), [§5](property-system.md#s5) (property sets), [§6.2](property-system.md#s6-2) (the +0x09000000 rule), [§8](property-system.md#s8) (DID maps), [§9](property-system.md#s9) (DB-object-type codes); `docs/specs/string-table.md` [§9](string-table.md#s9) (rendering a StringInfo); `landblock.md` and `coordinates.md` (M3, in preparation: block maps, landblock info, map coordinates). This spec starts with a fully reassembled, decompressed record payload. |

---

<a id="purpose"></a>
## Purpose

Every game object in the data (item, quest, skill, effect, NPC, area, dungeon, ...) has an
**object-state record** ("WState") in prefix 0x70, plus, at DID + 0x09000000, a property
set (`property-system.md` §6.2). The object-state record says **what class the object is**
and holds the object's script-level state: a small graph of script-class instances and
engine containers. This spec gives an implementer of `tessera-wstate` what they need to:

- read the record header, including the root class index at offset 4 and the dependency
  list (§3);
- decode the serialized object graph ("bundle"): reference labels, the per-record class
  table, items, script objects and the native container codecs (§4, §5);
- skip or read the trailing link and available-property sections (§6, §7);
- resolve references between items and to other records (§8);
- for M3: identify area, territory, region and dungeon records and resolve them **by name**,
  including the dungeon map positions and the region-number table (§11).

§9 reports the whole-install decode run, §10 summarizes other encodings (Reported only),
§12 gives implementation guidance, §13 lists disagreements, §14 the open questions and §15
test guidance with a worked synthetic example.

<a id="provenance"></a>
## Provenance

How sources are marked:

- **Confirmed**: decoded on the user's own install on 2026-10-04 with a throwaway probe in
  the dirty-room scratchpad. Each Confirmed fact says what was decoded and how much.
- **Reported**: comes only from published third-party material and was not, or could not
  be, checked against data.

| Key | Source |
|---|---|
| OBS | Observed in the user's install, 2026-10-04 (Steam install, client data current on that date). **Every** object-state record in `client_gamelogic.dat` was decoded: 383,332 records with prefix 0x70 and 11 with prefix 0x76 (no other prefix in 0x70–0x77 exists). 383,342 of 383,343 decode exactly with zero leftover bytes; the remaining one stops at one undecoded native class (§9). Property sets, string tables, block-map descriptors and DID maps were decoded as needed for §11. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.0 (Apache-2.0 per its distribution's LICENSE), studied as a decompiled binary for facts only; and LotroCompanion `lotro-tools` (area, territory, region and dungeon loaders), read for facts only. |
| MK | Mirarkitty `lotro_extractor` documentation (Apache-2.0). Facts only. |
| DS | `research/notes/data-semantics.md` §5.2, §6.7. |
| ER | `research/notes/engine-client-re.md` §9 (client memory observations). |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| H1 | Record header and section order (§3) | LC, DS; OBS (383,342 records exact) | Confirmed |
| H2 | Offset-4 class index = class of the root item (§3.1) | LC, DS; OBS (383,342 of 383,342) | Confirmed |
| H3 | Import list = dependency list of (DB-object type, DID, flags); flag 16 marks DIDs that appear as DID items (§3.2) | LC (layout); OBS (counts match exactly) | Layout Confirmed; meaning of flag 16 Confirmed; flag 0 partly OPEN |
| B1 | Bundle layout: labels, class table, items (§4) | LC, DS; OBS | Confirmed |
| B2 | The per-record class table repeats the library's attribute list exactly (§4.2) | OBS (1,381,854 of 1,381,854) | Confirmed. Derived by this spec writer. |
| I1 | Item markers 0 (embedded object) and 0x20000000 (DID) (§5.1) | LC; OBS | Confirmed. Markers 0x10000000 and 0x08000000 are Reported (never observed). |
| I2 | Script-object body = attribute values in library-list order (§5.2) | LC; OBS | Confirmed |
| I3 | Reference value 0xFFFFFFFF = null; otherwise a label of the same bundle (§5.2, §8) | OBS (7,293,936 reference attributes: no other value occurs) | Confirmed. Derived by this spec writer. |
| N1 | Native codecs for classes 11, 13, 17, 18, 23, 25, 35, 37, 39, 57, 97, 104, 105, 117, 134, 138, 160, 161, 166, 175, 199, 225, 415, 2479 (§5.3) | LC; OBS | Confirmed (per-class counts in §5.3) |
| N2 | Native codecs for classes 72, 111, 131, 192, 427, 1942, 1980 (§5.3, §5.4) | OBS | Confirmed sizes and layouts; field meanings mostly OPEN. Derived by this spec writer; neither LC nor MK decodes them. |
| N3 | Native codecs for classes 101, 407, 2567, 3103, 3461, 3740 | LC | Reported (never observed) |
| N4 | The u16 after the count of a 32-bit set is a bucket hint in its high byte (§5.3) | OBS | Confirmed. Derived by this spec writer. |
| L1 | Link-section layout (§6) | LC; OBS (580,922 links) | Layout Confirmed; meaning OPEN |
| P1 | Available-property section layout (§7) | LC; OBS | Layout Confirmed; meaning OPEN |
| G1 | Area / territory / region / dungeon kinds, their properties and their links (§11) | LC (`lotro-tools`), DS §6.7; OBS (all records of each kind) | Confirmed |
| G2 | Dungeon map position stored in the dungeon's object state (§11.4) | LC (`lotro-tools`); OBS (1,303 dungeons) | Confirmed |
| X1 | Network and pre-live encodings (§10) | LC | Reported |

---

<a id="s1"></a>
## 1. Concepts

| Term | Meaning |
|---|---|
| **Object record** | One record with a DID in 0x70000000–0x77FFFFFF. Its payload is laid out as in §3. |
| **Root class** | The class index at payload offset 4. It identifies the kind of object (item, quest, area, ...). |
| **Bundle** | The length-prefixed block inside the record that holds the serialized object graph (§4). |
| **Item** | One element of the bundle's object graph: either a DID (a pointer to another object record) or an **embedded object** (§5). |
| **Label** | The u32 number that names an item inside its bundle. References between items use labels, not positions. |
| **Root item** | The first item. It is always an embedded script object of the root class. |
| **Script object** | An embedded object whose class is defined in the class library (`wlib.md` §6). Its body is a list of attribute values. |
| **Native object** | An embedded object whose class is undefined in the library. Its body is decoded by a built-in codec chosen by class index (§5.3). |
| **Reference** | A 32-bit value that holds a label of the same bundle, or 0xFFFFFFFF for null. Reference attributes and the values of reference containers are references. |

---

<a id="s2"></a>
## 2. Where object records live (Confirmed)

| Prefix | Archive (count) | Meaning |
|---|---|---|
| 0x70 | gamelogic (383,332) | Game objects. Each has a property set at DID + 0x09000000 (prefix 0x79; `property-system.md` §6.2). |
| 0x76 | gamelogic (11) | **Singletons**: DID = 0x76000000 + class index, for each class in the library's singleton list (`wlib.md` §7). They have no property set. |
| 0x71–0x75, 0x77 | none | Not present in any archive |

All object records are routed to gamelogic (`dat-container.md`). Many well-known objects
are reached through the DID map labelled **WEENIECONTENT** (0x2800000E,
`property-system.md` §8.1), whose labels point almost only at 0x70 records (§5.4 and §11.3
use two of them).

---

<a id="s3"></a>
## 3. Record layout

<a id="s3-1"></a>
### 3.1 Header and sections (Confirmed, 383,342 records)

| Order | Field | Type | Meaning / observed |
|---|---|---|---|
| 1 | did | DID | Equals the DID looked up (all) |
| 2 | root_class | u32 | Class index of the root item (§4.3). Always < the library's class count. |
| 3 | imports | TSize + import × *n* | Dependency list (§3.2) |
| 4 | reserved_a | VLE | Always 0 |
| 5 | reserved_b | VLE | Always 0 |
| 6 | reserved_flag | bool8 | Always 1 |
| 7 | bundle_size | u32 | Byte length of the bundle. Never 0 in this install. |
| 8 | bundle | *bundle_size* bytes | §4 |
| 9 | has_links | bool8 | 1 in 192,723 records |
| 10 | links | u32 count + link × count | Present only when `has_links` = 1 (§6) |
| 11 | has_available | bool8 | 1 in 199,295 records |
| 12 | available | TSize + (u32 pid, u32 pid echo) × *n* | Present only when `has_available` = 1 (§7) |
| — | *(end)* | | Zero leftover bytes in all 383,342 decoded records |

LC names fields 4–6 "always 0", "always 0" and an unnamed boolean; their meaning is OPEN.
A reader should accept other values and warn.

When `bundle_size` is 0, LC skips the bundle and the record has no items; this does not
occur here, but readers must accept it (the object then has no state, only a class).

<a id="s3-2"></a>
### 3.2 Import list (Confirmed layout)

A TSize count *n*, then *n* entries of 9 bytes:

| Field | Type | Meaning |
|---|---|---|
| dbo_type | u32 | DB-object-type code (`property-system.md` §9) of the referenced record |
| did | DID | The referenced record |
| flags | u8 | 16 or 0 |

Observed (99,863 records have a non-empty list; *n* up to 1,767):

| dbo_type | DID prefix | flags | Entries | Notes |
|---|---|---|---|---|
| 0x4E (appearance) | 0x20 | 0 | 296,906 | Every one of these DIDs also occurs as a u32 elsewhere in the same record |
| 0x45 (game object) | 0x70 | 16 | 43,335 | Exactly the DID items (§5.1) of the bundle |
| 0x45 | 0x76 | 16 | 4 | Likewise (singletons importing other singletons) |
| 0x45 | 0x70 | 0 | 20,726 | 2,358 of these DIDs occur elsewhere in the record and 18,368 do not; of 301 sampled entries of the second kind, 273 do not occur in the object's property set either |

Facts:

- The TSize hint is 2 in 383,340 records (6 and 7 in the two records with the longest
  lists), and the entries are in **bucket order by DID** (`records.md` §1.3.1) in all
  99,863 non-empty lists.
- The flag-16 entries and the DID items correspond one to one: 43,339 of each, and each
  DID item's DID is a flag-16 import of its record.
- LC rejects any dbo_type other than 0x45 and 0x4E; Tessera should accept others and warn.

The import list is not needed to decode the bundle. It is useful as a **prefetch list**: the
records an object depends on.

> **OPEN:** What a flag-0 game-object import refers to when its DID occurs nowhere in the
> record or the property set (§3.2 table), and whether flag 16 means "embedded DID
> item" in general or something wider.

---

<a id="s4"></a>
## 4. The bundle

<a id="s4-1"></a>
### 4.1 Layout (Confirmed)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | item_count *n* | VLE | Number of items (1 to 57,284 observed) |
| 2 | labels | u32 × *n* | Label of item 0, item 1, ... |
| 3 | class_count *c* | u16 | Number of class-table entries |
| 4 | class table | class entry × *c* | §4.2 |
| 5 | items | item × *n* | §5, in label-list order |
| — | *(end)* | | The bundle ends exactly after item *n* − 1 (all decoded records) |

Class entry:

| Field | Type | Meaning |
|---|---|---|
| class_index | u16 | A script class |
| attribute_count *a* | u16 | |
| attributes | (u32 name hash, u8 type code) × *a* | |

<a id="s4-2"></a>
### 4.2 The per-record class table (Confirmed)

- It lists each **script** class whose instances occur in the bundle, once (no duplicates
  in any record). Native classes never appear in it.
- Each entry's attribute list equals the class library's attribute list for that class
  (`wlib.md` §8), with the same hashes and types **in the same order** (1,381,854 entries,
  no exception). Classes without attributes appear with *a* = 0.
- In 38,413 records the table also lists script classes that have no instance in the
  bundle (38,612 such entries; 38,486 of them are classes with attributes, 126 without).
  Readers ignore such entries.
- Every instantiated script class is listed (383,342 of 383,342 records), and every listed
  class is defined in the library (1,381,854 of 1,381,854).
- The order of entries is neither ascending nor first-use order.

Consequences for a reader:

- The bundle is **self-describing** for script objects: it can be decoded with the
  class table alone, without the library. A reader should still check each entry against
  the library and report a mismatch, because a mismatch means the library and the record
  come from different builds.
- If a script object's class is missing from the table, use the library's list (and warn).

> **OPEN:** The rule that orders the class table and the reason for the extra entries.

<a id="s4-3"></a>
### 4.3 Labels (Confirmed)

- Labels are unique within a record; the first label is always **0**.
- Labels are ascending in all but 9 records, and contiguous 0 … *n* − 1 in 381,900
  records. In 1,442 records some numbers are skipped. Labels are therefore **names**, not
  positions: a reader maps label → item index.
- Item 0 (label 0) is the **root item**. It is always an embedded script object whose class
  equals `root_class` (§3.1) (383,342 of 383,342).

---

<a id="s5"></a>
## 5. Items

<a id="s5-1"></a>
### 5.1 Item marker (Confirmed for 0 and 0x20000000)

Each item starts with a u32 marker:

| Marker | Item | Then | Observed |
|---|---|---|---|
| 0x00000000 | Embedded object | u16 class index, then the body (§5.2 if the class is defined in the library, else §5.3) | 5,780,393 |
| 0x20000000 | DID item | DID of another object record | 43,339 (all prefix 0x70 or 0x76, all existing) |
| 0x10000000 | Embedded object (as for 0) | | Reported (LC); never observed. ER reports this bit as the in-memory "native" flag. |
| 0x08000000 | Instance ID | u64 | Reported (LC); never observed |
| anything else | | | Malformed |

A DID item is never the root item.

<a id="s5-2"></a>
### 5.2 Script-object body (Confirmed)

For each attribute of the class, **in the order of the class's attribute list**
(`wlib.md` §8.4, equal to §4.2), one value:

| Type code | Value | Size |
|---|---|---|
| 1 (reference) | u32: a label of this bundle, or 0xFFFFFFFF = null | 4 |
| 2 (integer) | u32 | 4 |
| 3 (float) | f32 | 4 |
| 130 (64-bit integer) | u64, low 32-bit word first | 8 |
| 131, 195 (double) | f64, low 32-bit word first | 8 |

There is no padding and no per-value tag. A class with no attributes has an empty body.

Observed attribute values (383,342 records): 7,882,705 integers, 7,293,936 references,
1,221,626 floats (no NaN), 1,051,396 64-bit integers, 340,986 code-195 doubles, no code-131
values. Of the references, 4,455,856 are null and 2,838,080 are labels present in the same
bundle. **No reference attribute holds a DID or any other value.** Links to other records
go through DID items (§5.1), which a reference can point at.

<a id="s5-3"></a>
### 5.3 Native classes and their codecs

The class index of a native object selects its codec. These indices are engine constants:
they are undefined in the class library (`wlib.md` §6.3) and LC has used the same numbers
across many client builds (Reported). Codec names are Tessera's own.

Common layouts used in the table:

- **hashed list** = TSize count *n* (`records.md` §1.3) followed by *n* elements; the
  elements are in bucket order by key where checked.
- **counted list** = u32 count *n* followed by *n* elements, in stored order.
- **small set** = u16 count *n*, one byte that is 0, one byte that is a TSize-style bucket
  hint, then *n* elements in bucket order by element (N4: hint byte observed 0–8 and the
  other byte 0 in all 91,835 sets; bucket order held in 1,442 of 1,442 sampled non-empty
  sets).

| Class | Codec | Body | Values are references? | Instances (records) | Mark |
|---|---|---|---|---|---|
| 11 | `map_u32` | hashed list of (u32 key, u32 value) | no | 155,033 (53,271) | Confirmed; bucket order by key in all |
| 13 | `multimap_u32` | as 11; keys may repeat | no | 115,685 (115,685) | Confirmed; bucket order in all |
| 35 | `refmap_u32` | as 11 | **yes** (64 null) | 269,159 (164,927) | Confirmed; bucket order in all |
| 37 | `refmultimap_u32` | as 13 | **yes** | 20,808 (20,808) | Confirmed; bucket order in all |
| 17, 25 | `array_u32` | counted list of u32 | no (integers, DIDs, float bit patterns) | 137,832 / 387,414 | Confirmed |
| 176, 182 | `refarray` | counted list of u32 | **yes** (176: 5,173 null) | 150,030 / 314,721 | Confirmed |
| 18 | `set_u32` | small set of u32 | no | 90,231 (31,360) | Confirmed |
| 105 | `set_u64` | small set of u64 | no | 930 (263) | Confirmed |
| 134 | `set_triple` | small set of (u32, u32, u8); the u8 was 0 in all sampled | no | 674 (184) | Confirmed layout |
| 23 | `map_u32_u64` | hashed list of (u32 key, u64 value) | no | 148 (147) | Confirmed |
| 97 | `map_u64_u32` | hashed list of (u64 key, u32 value) | no (floats, PIDs, DIDs) | 7 (5) | Confirmed |
| 117 | `refmap_u64` | hashed list of (u64 key, u32 value); bucket order by the u64 key | **yes** | 725 (169) | Confirmed |
| 138 | `refpair_map` | hashed list of (u32 ref, u32 value, bool8 = 0, u32 ref) | **yes, first and last field** | 2 (2) | Confirmed (9,213 elements) |
| 131 | `map_u32_tagged` | hashed list of (u32 key, u32 value, bool8 = 0, u32 DID) | no (last field is a 0x70 DID) | 1 (1) | Confirmed (792 elements); new |
| 104 | `array_u64` | counted list of u64 | no | 2,252 (2,252) | Confirmed |
| 111 | `array_u64` | counted list of u64 | no | 2 (2) | Confirmed; new |
| 101 | `multimap_u64_u32` | counted list of (u64 key, u32 value) | no | 0 | Reported (LC) |
| 39 | `property` | one property entry **without** echo: u32 PID then the value (`property-system.md` §5.2, array-element form) | no | 862,882 (35,408) | Confirmed |
| 166 | `property_set` | a property set without the leading DID: TSize, then entries with echo (`property-system.md` §5) | no | 292,684 (151,744) | Confirmed |
| 57 | `bitset` | VLE bit count *b*, then ⌈*b*/8⌉ bytes; bit *i* is bit (*i* mod 8) of byte ⌊*i*/8⌋ | no | 381 (144) | Confirmed |
| 160, 161 | `position` | a position value (`property-system.md` §4.6) | no | 15,629 / 4,628 | Confirmed (§5.4) |
| 175 | `weighted_table` | counted list of (u32 value, u32 weight, u32 unknown), then u32 that is 0 | no | 8,110 (8,110) | Confirmed; trailing u32 = 0 in all |
| 199 | `string_info` | StringInfo (`records.md` §4) | no | 79,402 (36,286) | Confirmed: 71,210 table references, 8,192 literals, no argument blocks |
| 225 | `string` | UTF-16 string (`records.md` §1.4) | no | 10,094 (90) | Confirmed |
| 72 | `frame` | 7 × f32 (§5.4) | no | 92 (20) | Confirmed size; new |
| 192 | `sphere` | 33 bytes (§5.4) | no | 380 (380) | Confirmed size; new |
| 427 | `scene_info` | variable (§5.4) | no | 1,172 (1,172) | Confirmed; new |
| 1942 | `barter_entry` | 49 bytes (§5.4) | no | 42,214 (3,449) | Confirmed size; new |
| 1980 | `opaque6` | 6 bytes (§5.4) | no | 2 (2) | Confirmed size only; new |
| 415 | `options_profile` | variable (§5.4) | no | 138 (138) | Confirmed for the observed form |
| 2479 | `quest_target_locations` | variable (§5.4) | no | 1 (1) | Confirmed |
| 3053 | (unknown) | not decoded (§5.5) | ? | 1 (1) | OPEN |
| 407 | `compressed_bitset` | u32 size, then that many bytes: u32 0, u32 unpacked size, zlib stream of a bitset | no | 0 | Reported (LC) |
| 2567 | `vault_item` | u64 instance ID, property set (as 166), one property (as 39) | no | 0 | Reported (LC) |
| 3103 | `vault` | hashed list of (u32 id, UTF-16 string), then u32 capacity, u32 used | no | 0 | Reported (LC) |
| 3461 | `triple_u32` | 3 × u32 | no | 0 | Reported (LC) |
| 3740 | `u32` | 1 × u32 | no | 0 | Reported (LC) |

Instance counts include the 9,121 items read from the one partially decoded record (§9).

<a id="s5-4"></a>
### 5.4 Details of the less regular codecs

**Position (160, 161).** Same encoding as the property value (`property-system.md` §4.6),
including the flag rules found there, which hold here too: bit 0x40 is set exactly when
the region byte is present and equals 1 (5,906 of 5,906; clear in all 14,351 others), and
bit 0x80 exactly when the cell field is present and equals 0 (18,306 of 18,306). Observed
flag bytes: 0x9B, 0x8B, 0xBB, 0xCB, 0xDB, 0xFB, 0x3B, 0x1B, 0x03, 0x7B, 0x43, 0x5B, 0x0B. A
zero flag byte (no position) did not occur.

**Frame (72), 28 bytes.** Seven f32: three that vary (observed x ∈ {0, ±2.5, 2, ...},
y ∈ {−4.5, −7, 0, −9.5, ...}, z ∈ {0, 0.05}) and four that were (1, 0, −0, 0) in all 92.
A plausible reading is a position followed by a unit quaternion with the scalar first; this
is this spec writer's interpretation, not Confirmed (the constant (1, 0, 0, 0) is consistent
with it).

**Sphere (192), 33 bytes.** u8 (1 in all 380), 16 bytes (identical in all 380), 3 × f32
(0 in all 380), f32 (15 distinct positive values). Read as: flag, 16-byte identifier,
centre offset, radius. Only the sizes and the constancy are Confirmed.

**Scene info (427).**

| Field | Type | Observed |
|---|---|---|
| title | StringInfo | table reference |
| owner | DID | The DID of the record that contains the item (1,172 of 1,172) |
| entry_count *k* | u32 | 6 (1,171 records), 1 (1 record) |
| entries | entry × *k* | |

Entry: u32 sentinel 0xFCFCFCFC (all), u32 index (1 … *k* in order, all), f32, f32, u32,
StringInfo, u32, u32, bool8 (0 in 7,026 entries, 1 in 1). With a plain StringInfo an entry is
41 bytes.

**Barter entry (1942), 49 bytes.** In all 42,214:

| Offset | Size | Observed |
|---|---|---|
| 0 | 4 | DID of the containing record (all) |
| 4 | 4 | A 0x70 DID (42,207, all existing) or 0 (7) |
| 8 | 4 | Small integer (1 in 41,424) |
| 12 | 1 | 0 or 1 |
| 13 | 4 | 0, except in the 7 entries whose offset-4 DID is 0: then a 0x70 DID |
| 17 | 4 | 0, except in those 7 entries: then 1–5 |
| 21 | 8 × 3 | Three pairs (u32 0x70 DID or 0, u32 quantity); the first DID is always set |
| 45 | 4 | 0 (except 3 entries) |

Field meanings are OPEN.

**Opaque6 (1980), 6 bytes.** Two instances, both `01 00 01 00 01 00`. OPEN.

**Options profile (415), observed form.** u32 (320 in all), u32 (0), u32 set count *s* (6
in all); then *s* × (u32 84, then 84 slot records); then u8 *t1*, u8 *t2*; then, because
*t1* = 1 and *t2* = 2 in all 138, 24 bytes. A slot record is u32 kind followed by: nothing
for kind 0; u64 + u32 for kind 2; u32 for kinds 6, 7 and 9 (Reported LC). Only kind 0
occurs. Reported (LC): when *t1* is 0 or 1 and *t2* ≠ 2, *t2* groups follow instead of the
24 bytes, each u32, u8, u8 count *e*, *e* × (u32, 4 × f32, u8), then u16 and 6 × u32. The
observed form is 2,078 bytes.

**Quest target locations (2479).** One instance, in the record named by the WEENIECONTENT
map label "QuestEventTargetLocationControl" (0x70019558 here); decodes exactly with LC's
layout:

1. hashed list of (u32 DID, u32 count *p*, *p* × (position, u32, u8)) — 50,115 DIDs;
2. u32 that is 7, then 7 × hashed list of (u32, counted list of u32);
3. hashed list of (u32 DID, u32 count *p*, *p* × (position, u32, u8, counted list of u32
   content-layer IDs)) — 34,083 DIDs;
4. hashed list of (u32 quest DID, hashed list of (u32 objective index, u32 condition count,
   per condition: u32 entry count, per entry: u32 DID, position, f32 radius, Pascal string,
   u32 *g*, *g* × filter, Pascal string)) — 20,012 quests. A filter is a flag byte followed
   by one u32 for each of its set bits 0x01 … 0x40 (bit 0x80 never set).

LC uses this item as the table of world positions of NPCs and quest targets (Reported). It
is not needed for M3.

<a id="s5-5"></a>
### 5.5 Unknown native classes

A native body has no length prefix, so an unknown native class makes the **rest of the
bundle** undecodable. The reader then:

1. keeps every item decoded so far (the root item is item 0 and is always a script object,
   so the object's class and root attributes survive);
2. records "unknown native class *C* at item *i* of *n*" and marks the object partially
   decoded;
3. continues with the next record. It does not attempt to resynchronize inside the bundle;
   `bundle_size` lets it skip to the link section, which it may still decode.

This replaces ARCHITECTURE §7.4's "decode unknown native classes as opaque byte ranges",
which is not possible without a length.

---

<a id="s6"></a>
## 6. Link section (Confirmed layout)

Present when `has_links` = 1, in which case the u32 count is at least 1 (192,723 records;
up to 9 links each; 580,922 links in total).

| Field | Type | Observed |
|---|---|---|
| direction | u8 | 0 (419,091) or 1 (161,831) |
| link_type | u32 | 8, 12, 13, 2, 1, 19, 11, 7, and 0x10000007 … 0x10000019 (table below) |
| limit | u32 | 1, 99, 250, 100, 4, 25, 20, 3, 0, 200, 999, 10, 5, 275, 350, 150 |
| flag | u32 | 0 (560,618) or 1 (20,304) |
| pids | TSize + (u32 pid, u32 pid echo) × *m* | *m* = 0, 1, 2 or 3; echo equals pid in all 206,924 pairs; every PID is defined in the dictionary |
| terminator | u8 | 0 when link_type = 13 (120,606 of 120,606), else 3 |

| link_type | Count |
|---|---|
| 8 | 191,938 |
| 12 | 142,613 |
| 13 | 120,606 |
| 2 | 36,569 |
| 1 | 36,533 |
| 0x10000019 | 30,181 |
| 19 | 20,178 |
| 0x1000000C, 0x10000013, 0x1000000B | 746, 744, 658 |
| 11, 7 | 112, 36 |
| 0x1000000D, 0x10000007, 0x1000000F, 0x10000010, 0x10000011, 0x10000012 | 3, 1, 1, 1, 1, 1 |

The link_type values coincide with the link-type codes of landblock links (DS §6.3;
`landblock.md`): for example 1 and 2 are patrol kinds, 8 generator, 12 script, 19 remote
detector, 0x10000019 phasing. Several types appear as a pair of entries with direction 0
and 1 and the same limit (for example type 13 with limit 99). The section reads like a
**schema of which link kinds the object may take part in**, with a per-kind limit and the
properties a link of that kind carries.

> **OPEN:** Meaning of direction, limit, flag and terminator, and the code-13 kind, which
> does not appear in the landblock link-type list.

---

<a id="s7"></a>
## 7. Available-property section (Confirmed layout)

Present when `has_available` = 1 (199,295 records): a TSize count *n* (0–57; hint 1 in all
2,619 sampled lists) followed by *n* pairs (u32 pid, u32 pid echo). The echo equals the pid
in every pair (about 7 million), every PID is defined in the dictionary, and pairs are in
bucket order by PID. LC rejects a pair whose echo differs.

In a 3,145-record sample, the listed PIDs overlap the object's own property set in 3,041
records, are a subset of it in 14 and disjoint from it in 90.

> **OPEN:** What the list means (properties the object exposes, accepts, or computes?).
> It is not needed for M3; readers keep it as a PID list.

---

<a id="s8"></a>
## 8. Object graph and reference resolution

<a id="s8-1"></a>
### 8.1 Reference-bearing values (Confirmed)

| Where | Which values are references |
|---|---|
| Script object | Attributes of type 1 |
| `refmap_u32` (35), `refmultimap_u32` (37), `refmap_u64` (117) | Map values (not keys) |
| `refarray` (176, 182) | Elements |
| `refpair_map` (138) | First and last field of each element |

A reference is either **0xFFFFFFFF (null)** or a label of the same bundle. Observed:
attributes 2,838,080 labels / 4,455,856 null; 176 elements 1,087,874 / 5,173; 182
elements 581,959 / 0; 35 values 996,945 / 64; 37 values 82,291 / 0; 117 values 518 / 0; 138
fields 18,426 / 0. No reference anywhere held an absent label.

Plain containers (11, 13, 17, 25, 97, ...) hold integers, PIDs, DIDs or float bit patterns.
Their values sometimes equal a label by coincidence; they are **not** references.

<a id="s8-2"></a>
### 8.2 Resolution algorithm (in words)

1. Build a map from label to item index (labels are names, §4.3).
2. A reference value resolves to the item with that label; 0xFFFFFFFF resolves to "none".
3. An item that is a DID item stands for **another object record**. Resolving through it
   loads that record (lazily). DID items always point at existing 0x70/0x76 records here.
4. The graph may contain cycles: in 140 records some item refers back to the root.
   Resolution must be lazy or keep a visited set; it must never expand references
   recursively without one.

<a id="s8-3"></a>
### 8.3 Shape of the graph (Confirmed)

- In 383,172 records every item except the root is referenced at least once, and the root
  is referenced by none, so the root is the only entry point.
- In 140 records the root is referenced too (cycles, above).
- In 30 records, 835 further items are referenced by nothing found by §8.1 (814 strings
  of class 225 and 21 positions of class 160).
- An item can be referenced many times (up to 324 times in one record); shared sub-objects
  are normal.

> **OPEN:** Who refers to the 835 unreferenced items of §8.3 (perhaps a plain container
> whose values are references after all, or script code).

---

<a id="s9"></a>
## 9. Whole-install statistics (Confirmed, 2026-10-04)

| Measure | Value |
|---|---|
| Object records | 383,343 (0x70: 383,332; 0x76: 11) |
| Decoded exactly (record and bundle consumed to the last byte) | **383,342 (99.9997%)** |
| Records with trailing bytes | 0 |
| Partially decoded | 1: record 0x70022DBA (root class 3044), stops at item 9,121 of 9,135 on native class 3053 |
| Decoded with only the native codecs published by LC | 378,316 (98.69%). The other 5,027 fail on native classes 1942 (3,449 records), 427 (1,172), 192 (380), 72 (20), 1980 (2), 111 (2), 131 (1) and 3053 (1). |
| Items | 5,823,732 (5,780,393 embedded objects, 43,339 DID items) |
| Distinct root classes | 869 in 0x70, 11 in 0x76 |
| Distinct script classes instantiated | 1,169 |
| Distinct native classes instantiated | 33 |

Record payload sizes: 243,441 records are below 1,000 bytes and 117,397 between 1,000 and
1,999; the largest is 29,944,338 bytes. Bundles: 302,607 are below 1,000 bytes; the largest
bundle holds 57,284 items.

Most common root classes (counts of 0x70 records; class indices are build-specific):

| Root class | Records | Reported kind (LC) |
|---|---|---|
| 796 | 52,753 | Item; LC's label for this class is "chisel" |
| 795 | 41,881 | Clothing item |
| 799 | 23,204 | Weapon |
| 1398 | 20,807 | Quest or deed |
| 1723 | 20,161 | Monster |
| 802 | 19,720 | Jewellery |
| 1724 | 16,605 | NPC |
| 1237 | 15,224 | Loot table |
| 734 | 14,475 | Effect |
| 827 | 13,247 | Skill |

For M3, §11 lists the classes that matter and how to find them without hard-coding.

---

<a id="s10"></a>
## 10. Other encodings (Reported only, LC)

These forms are **not** used by the files of the current client. They are recorded so the
model stays general; none was checked.

- **Network form** (object state received from a server): each script-attribute value is
  followed by a u32 type code (64-bit values as two value + code pairs), attributes are
  read in slot order rather than list order, positions and property sets use their network
  forms (`property-system.md` §10). Relevant to M7 only.
- **Pre-live form** (older clients): no import list, no reserved fields, no link and
  available-property flags (both sections always present); the bundle starts with a u32
  label count and items are flag words that carry a package ID, a native type and a slot
  count, with each slot as (u32 value, u32 type code). Tessera does not support it.

---

<a id="s11"></a>
## 11. Areas, territories, regions and dungeons (M3)

The world walker shows the name of the area under the camera and needs dungeon names and
map positions. Landblock data names areas and dungeons by DID (`landblock.md`: the block map
gives an `Area_DID` per landblock; landblock-info cells give a `Dungeon_DID`). This section
turns those DIDs into names and hierarchy.

<a id="s11-1"></a>
### 11.1 Kinds (Confirmed)

Each kind is identified by the **properties** of its property set (property names come
from the dictionary in the user's data, `property-system.md` §2). The class index is an
observation for this install, useful as a fast hint, and must not be hard-coded.

| Kind | Identifying properties (top level of the 0x79 set) | Root class here | Records | Object state |
|---|---|---|---|---|
| Area | `Area_Name`, `Area_Territory`, `Area_Icon` | 690 | 675 | Root object with no attributes |
| Territory | `Area_Name`, `Area_Region` | 692 | 86 | Root object with one reference attribute (hash 0x0D87F6C3) → `array_u32` of DIDs (§11.5) |
| Region | `Area_Name`, `Area_RegionID` | 180 | 5 | Root object with no attributes |
| Dungeon | `Dungeon_Name`, `Dungeon_MapData`, `Dungeon_ParentDungeon` | 691 | 1,303 | Root object with one reference attribute (hash 0x00B3FA2E) → position or null (§11.4) |
| Region control | `Region_Map_List` | 2395 | 1 | Root object with one reference attribute (hash 0x072EA140) → region table (§11.3) |

In the scan of all 383,332 property sets, each identifying property occurred only in
records of the listed class, except `Area_Name`, which areas, territories and regions share
(for example `Area_Territory`: 675 records, all class 690; `Dungeon_Name`: 1,303, all class
691). All five classes share the ancestor chain
486 → 89 → 485 → 484 (`wlib.md` §9).

Data-driven discovery of the class indices, for a reader that wants the fast hint:

- **area class** = the root class of any `Area_DID` target in a block-map descriptor;
- **territory class** = the root class of any area's `Area_Territory` target;
- **region class** = the root class of any territory's `Area_Region` target;
- **dungeon class** = the root class of any `Dungeon_DID` target in landblock info;
- **region control** = the record named by the WEENIECONTENT DID map (0x2800000E) label
  "RegionControl" (0x7001819C here).

<a id="s11-2"></a>
### 11.2 Resolving an area DID to names (Confirmed chain)

Given an area DID *a* (for example from a block map):

1. Load the property set at *a* + 0x09000000. If it has `Area_Territory`, *a* is an area.
   (Optionally check the root class at offset 4 of record *a*.)
2. The **area name** is the StringInfo value of `Area_Name`, rendered per
   `string-table.md` §9 in the user's language.
3. `Area_Territory` is a territory DID *t*. Its property set holds the **territory name**
   (`Area_Name`) and `Area_Region`, a region DID *r*.
4. The property set of *r* holds the **region name** (`Area_Name`) and `Area_RegionID`, an
   integer region number. That property's definition labels its values through the DID map
   **REGION** (0x28000008, `property-system.md` §8.1).

Observed: 675 of 675 areas point to a territory record, 86 of 86 territories to a region
record. All 675 + 86 + 5 names are string-table references that resolve in
`client_local_English.dat`. All 684 non-zero `Area_DID` values found in the five block-map
descriptor records (0x18) are area records; one value is 0 (no area).

The object-state records of areas and regions carry no attributes, so the name chain
needs only the root class and the property sets. Other area properties (icon, music,
map data) are listed by DS §6.7.

<a id="s11-3"></a>
### 11.3 Region numbers and the region-control record (Confirmed)

The landblock region number *R* in landblock DIDs (`records.md` §2.2) is a small integer
(1–5 and 14 here). Two sources name these numbers:

- **Region records** (§11.1): `Area_RegionID` values 1, 2, 4, 5 and 6. No region record has
  3 or 14.
- **The region-control record** (0x7001819C here, found by its WEENIECONTENT label):
  - its property set has `Region_Map_List`, an array of structs `{Region_ID, Region_Name}`
    with 6 entries, for region numbers 1, 2, 3, 4, 14 and 5 (in that stored order);
  - its object state has a root attribute (hash 0x072EA140) referencing a `refmap_u32`
    (class 35) that maps the region numbers 1, 2, 3, 4, 5, 14 to `string_info` items; these
    StringInfo values equal the `Region_Name` values of the property set.

For regions 1, 2 and 5 the region-control name and the region record's `Area_Name` are the
same StringInfo; for region 4 they differ (different tokens). A reader that needs a name for
a landblock region number should use the region-control table, which covers every region
number that has landblocks.

> **OPEN:** What region number 6 (one region record, one dungeon position, no landblocks
> in any archive) stands for.

<a id="s11-4"></a>
### 11.4 Dungeons (Confirmed)

Given a dungeon DID *d* (for example a landblock-info cell's `Dungeon_DID`):

- the **dungeon name** is `Dungeon_Name` (StringInfo) in the property set at
  *d* + 0x09000000 (1,303 of 1,303 resolve);
- `Dungeon_ParentDungeon` is 0 (1,277) or another dungeon DID (26, all dungeon records);
- `Dungeon_MapData` is a 0x78 property-set DID (1,303 of 1,303) holding the map UI
  properties (DS §6.7);
- the **map position**: the root object's reference attribute with hash **0x00B3FA2E**
  points to a `position` item (997 dungeons) or is null (306). LC uses this position to
  place the dungeon on the world map (Reported).

Observed dungeon positions (997): region byte 2 (415), 1 (307), 4 (112), 3 (84), 5 (76),
14 (2), 6 (1); cell 0, that is outdoors, in 935 and an indoor cell in 62. Flag bytes 0xBB
(598), 0xFB (260), 0x9B (51), 0x3B (35), 0xDB (26), 0x7B (16), 0x1B (6), 0x5B (5); all
carry region, landblock, cell and x/y/z, and most an orientation. Converting such a
position to map coordinates is specified in `coordinates.md`.

<a id="s11-5"></a>
### 11.5 Territory object state (Confirmed)

The territory root attribute (hash 0x0D87F6C3) references an `array_u32` (class 25) of
0x70 DIDs: empty in 71 territories, 1 element in 13, 2 in 1, 7 in 1. All 22 listed DIDs
are records of root class 1398 (quests or deeds, Reported LC). Not needed for M3.

---

<a id="s12"></a>
## 12. Implementation guidance

- **Two decode depths.** Most callers need only the root class (offset 4) and the property
  set. Offer a header-only read (8 bytes, plus the import list if wanted) and a full bundle
  decode on demand. Records up to 30 MB exist, so never decode bundles eagerly in bulk.
- **Bounds.** Treat input as hostile. Every count is checked against the bytes left before
  allocating, using the smallest element size: 9 bytes per import, 4 per label, 4 per
  class-table entry plus 5 per attribute, 4 per item (marker), and per codec the fixed
  element size (for example 8 for `map_u32`, 13 for `refpair_map`). The bundle is parsed
  from its own `bundle_size` slice, so it can never read into the link section.
- **Recursion.** Items are flat; only native property values (39, 166) and StringInfo (199)
  nest, with the depth limits of `property-system.md` §5.3 and `records.md` §4.2.
- **Errors.** One error type per crate (`thiserror`), with distinct cases for: bad marker,
  unknown native class (partial result, §5.5), attribute-list mismatch with the library
  (warning), reference to an absent label, trailing bytes, truncated input.
- **Library mismatch.** If the per-record class table disagrees with the library, decode
  with the per-record table (it describes the bytes) and warn.
- **Writers.** A byte-exact writer must keep every TSize hint (imports, maps, sets,
  available properties), the label numbers, the class-table order and its extra entries.

---

<a id="s13"></a>
## 13. Disagreements with other sources

| Source | Claim | Finding |
|---|---|---|
| ARCHITECTURE §7.4 | Unknown native classes "decode as opaque byte ranges with a warning rather than failing the whole object". | Native bodies have no length; the rest of the bundle is lost, but the root item and the sections after the bundle survive (§5.5). |
| ARCHITECTURE §7.4, DS §5.1 | Attributes are not flattened; a library must compute the flattened view. | The attribute lists are already complete (`wlib.md` §8.3); the per-record class table confirms it. |
| DS §5.2 | Native table lists class 17/25/176/182 as one "AArray" family with 176 and 182 holding references. | Confirmed, and 35, 37, 117 and 138 also hold references (§8.1). |
| LC | Reference attributes that hold values ≥ 0x70000000 are DIDs. | No reference attribute holds a DID in this install; DID links go through DID items (§5.1). The threshold rule is harmless but unused. |
| LC, DS §5.2 | Native codec list (32 class indices). | Seven more native classes occur (72, 111, 131, 192, 427, 1942, 1980) and one remains undecoded (3053). |
| DS §5.2 | Imports have dboType 69 → 0x70/0x76 and 78 → 0x20. | Confirmed; these are the DB-object-type codes 0x45 and 0x4E of `property-system.md` §9. |

---

<a id="s14"></a>
## 14. Open questions

> **OPEN:** Meaning of the reserved header fields (§3.1) and of flag-0 game-object imports
> (§3.2).

> **OPEN:** Order and extra entries of the per-record class table (§4.2).

> **OPEN:** Native class 3053 (one instance, in 0x70022DBA). Its body begins with two
> bytes 1, a u32 2, a u32 9 and then many one- and two-character UTF-16 strings; the
> layout was not worked out. Not needed for M3.

> **OPEN:** Field meanings inside `frame` (72), `sphere` (192), `scene_info` (427),
> `barter_entry` (1942) and `opaque6` (1980) (§5.4).

> **OPEN:** Semantics of the link section (§6) and of the available-property section (§7).

> **OPEN:** The 835 unreferenced items (§8.3).

> **OPEN:** Region number 6 (§11.3).

> **OPEN:** Whether the five area-related class indices of §11.1 change between client
> builds (use the data-driven discovery of §11.1 either way).

> **OPEN (implementation, 2026-10-04):** Choices `tessera-wstate` made where this spec is
> silent; please confirm or correct:
> - `property` (39) with PID 0 is read as a null entry with no value (the array-element
>   form of `property-system.md` §5.2); a PID missing from the dictionary fails the record.
> - `options_profile` (415), Reported form: the u16 and the six u32 are read **once, after
>   all** *t2* groups (§5.4 can also be read as "after each group"). *t1* > 1 with any
>   *t2*, and slot kinds other than 0, 2, 6, 7, 9, are rejected (the record fails, since
>   the body length is then unknown).
> - `quest_target_locations` (2479): a filter flag byte with bit 0x80 is rejected. The
>   count before the seven part-2 tables is read as a count (7 observed).
> - A class listed twice in a per-record class table: the first entry describes the bytes.
> - A reference to an absent label fails the record (§12 lists it as an error case);
>   references are checked against the label list, so a partial record (§5.5) can still
>   refer to labels of items that were not decoded.
> - A bad item marker fails the record (no partial result), unlike an unknown native class.

> **OPEN (implementation review, 2026-10-04):** Counts to reconcile (spec writer):
> - §5.3 says its instance counts include the 9,121 items of the partially decoded record
>   0x70022DBA, and §9 gives 5,823,732 items. The `tessera-wstate` census, which counts
>   those items, finds 5,832,853 items (for example class 199: 88,199; 17: 137,960;
>   176: 150,107), so the §5.3 and §9 figures appear to **exclude** that record.
> - §9 "869 in 0x70, 11 in 0x76" root classes: the census finds 877 distinct root classes
>   in all, so the two sets appear to overlap by 3 classes.
> - §5.4 gives 41 bytes for a scene-info entry "with a plain StringInfo". With a literal
>   StringInfo (`records.md` §4.1) the smallest entry is 34 bytes; the implementation
>   bounds counts with 34.

> **OPEN (implementation, 2026-10-04):** §15 asks the real-data test to resolve "every
> block-map `Area_DID`". No clean-room spec for the block-map descriptors (0x18) or block
> maps exists yet (`landblock.md` is in preparation), so the `tessera-wstate` real-data
> test resolves **every area record** found by its property signature (§11.1) instead,
> which includes every area a block map can name. Switch to the block-map list once
> `landblock.md` lands.

---

<a id="s15"></a>
## 15. Test guidance

Synthetic fixtures (generated by our own code; no game data):

- a synthetic class library (`wlib.md` §14) defining class 0x0123 with one reference
  attribute (name hash 0x0ABCDEF0) and the worked example below;
- one fixture per codec of §5.3 with 0, 1 and several elements, including null references
  in `refarray` and `refmap_u32`, a `small set` whose elements are in bucket order for its
  hint, and a position for each flag bit;
- a record with a DID item and a matching flag-16 import; a record whose root references
  itself (cycle); a record whose class table lists an extra class;
- malformed variants: bad marker; unknown native class at item 2 (expect a partial result
  with items 0 and 1); a reference to an absent label; a label list with a duplicate; a
  count larger than the remaining bytes (must fail without allocating); trailing bytes in
  the bundle and after the available-property section; a per-record class table that
  disagrees with the library (warning, decode by the table).

Real-data tests (skipped without `TESSERA_GAME_DIR`):

- decode every 0x70 and 0x76 record; assert zero trailing bytes for every fully decoded
  record and that the only partial decodes are unknown-native-class cases; print the
  counts of §9 (do not assert them);
- for every block-map `Area_DID`, resolve area → territory → region names; assert each step
  finds the identifying properties of §11.1 and each name StringInfo resolves;
- for every dungeon record, check the position attribute is null or a position.

<a id="s15-1"></a>
### 15.1 Worked synthetic example

A synthetic object 0x70000ABC of class 0x0123 whose single reference attribute points at a
string item "Hi" (class 225), with no imports, links or available properties. 60 bytes:

| Bytes | Meaning |
|---|---|
| `BC 0A 00 70` | DID 0x70000ABC |
| `23 01 00 00` | root class 0x0123 |
| `02 00` | imports: TSize hint 2, *n* = 0 |
| `00` `00` `01` | reserved_a = 0, reserved_b = 0, reserved_flag = 1 |
| `29 00 00 00` | bundle_size = 41 |
| `02` | bundle: item_count = 2 |
| `00 00 00 00` `01 00 00 00` | labels 0 and 1 |
| `01 00` | class_count = 1 |
| `23 01` `01 00` `F0 DE BC 0A` `01` | class 0x0123, 1 attribute: hash 0x0ABCDEF0, type 1 (reference) |
| `00 00 00 00` `23 01` `01 00 00 00` | item 0: marker 0, class 0x0123, attribute = label 1 |
| `00 00 00 00` `E1 00` `02 48 00 69 00` | item 1: marker 0, class 225 (string), UTF-16 length 2, "Hi" |
| `00` | has_links = 0 |
| `00` | has_available = 0 |

Decoding gives a root object whose attribute 0x0ABCDEF0 resolves to the string "Hi". If the
attribute were `FF FF FF FF` instead, it would be null and item 1 would be an unreferenced
item (§8.3).
