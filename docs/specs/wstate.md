<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# WState: object-state records (DID 0x70000000–0x77FFFFFF)

| | |
|---|---|
| **Status** | Accepted (re-review 2026-10-07 of the OPEN-closing revision and its repair, see §17; previously Accepted 2026-10-05, Revised 2026-10-07). |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review sessions (spec-writer agent, 2026-10-05; spec-reviewer, a different session from the writer, 2026-10-07) |
| **Review record** | Claude review session (spec-writer agent), review 2026-10-05: changes requested (blocking: §10 network-form bullet under the `docs/LEGAL.md` §9.3 gate; wrong item, embedded-object, root-reference and small-set counts; minor: unscoped counts, an unknown without OPEN in §10, the class-134 ordering field). Claude review session (spec-writer agent), re-review 2026-10-05 after the repair round: **passed, Accepted**. Network-form content withdrawn (row X1 narrowed, §10 states stored form only); counts corrected and scoped; bundle-version OPEN added; class-134 ordering answered by N8. Re-review spot checks on the install with an independent decoder over all 383,343 records (all decode exactly): items 5,832,867, embedded objects 5,789,528, script objects 2,817,016; root referenced in 139 records and unreferenced in 383,204; small sets 90,232 / 930 / 674 (91,836) with header byte 2 = 0 in all and class-18 hints 0: 85,225, 1: 4,857, 2–8: 150, hint 1 in all 105 and 134 sets; class-18 sets in bucket order by value mod B (8,096 of 8,096); class-105 sets by whole value mod B (108 of 108; low/high half alone break 56/60); bucket fields of 131/134/138 all < B (792 + 43 + 9,213) and ascending in all 677 collections; 43 of 43 (134) and 383 of 792 (131) bucket fields equal a label; labels contiguous in 381,901, non-ascending in 9; import hint 2 in 383,341; value and reference counts of §5.2 and §8.1. First-round checks still stand. Remaining OPENs (§3.2, §5.4, §6, §7, §10, §14) are precise and non-blocking. Claude review session (spec-reviewer, a different session from the writer), review 2026-10-07 of the OPEN-closing revision: **changes requested** (small, precise fixes; nothing in the revision falls under a closed gate, no code or pseudo-code, provenance rows H6–H8, B5, N9–N11 and the change log are present, no section renumbered). Re-checked in an independent copy of the decompiler project (build 4906.0071.1794.4035, address ranges only, nothing from network, login, launcher or anti-tamper code) and confirmed: fields 4 and 5 are read into the dependency list as its excluded-flags and required-flags masks and the insertion routine rejects a dependency whose flags miss a required bit or hit an excluded bit (H6); field 6 is kept with the record and, together with the library's trailing byte, makes the item reader create script objects without the slot-0 call (H7); the archive-record builder registers bundle version 5 for its stream, sets the chunk-512 reader mode and passes the record boolean (B5); the per-class hash routines: strings by the 28-bit name hash over UTF-16 units with the 0xFFFFFFFF → 0xFFFFFFFE substitution, positions 160/161/162 by region·2²⁴ + X·2¹⁶ + Y·2⁸ + instance + cell, StringInfo by a stored u32 field (the token, per OBS4) or, for a literal, by the text's name hash; key equality with the 0.0002 tolerance (160: x, y, z and orientation; 162: x, y, z; 161: none); class-1980 collection layouts and buckets (N9, N10). Not re-read: H8 (property-set dependency), N11 (class-72 default) and the absence of the chunk-128 check on the archive path; they stay BIN-only as marked. Data spot checks (independent probes, 2026-10-07): fields 4/5/6 are 0/0/1 in all 383,343 object records and import flags are only 0 (317,632) and 16 (43,339); the bucket field of every class-131/134/138 element recomputed from the spec text alone: 9,211 + 792 + 22 string keys and 21 position keys (class 160, with X ≠ Y, so the X/Y order is tested) all match, plus 2 StringInfo keys of class 138 not recomputed; no 0x7F property-set record exists for the 0x76 records (H8 premise). Findings: (1) §5.4 class 1980 (N10): "the client fails the load when the hint is 23 or more" is incomplete: with the stream's strict-check flag set, a hint whose bucket count exceeds 50,000 (hint 13 or more) also fails; state it and add an OPEN on whether archive-record streams set that flag (same finding as `wlib.md` §4.3). (2) §5.4 bucket of a string key (N9): the client's hash stops at the first code unit 0, so a string containing U+0000 hashes only its prefix; say so (a writer adding such a key would otherwise compute a different bucket). (3) §14 third OPEN: "for M3/M4" reads as research methods; say "milestones M3/M4". Claude review session (spec-reviewer, a different session from the writer), re-review 2026-10-07 after the repair round: **passed, Accepted**. All findings resolved: (1) §5.4 class 1980, §3.2 (import list), N12, §14 and §15 give both failure conditions of the TSize hint check and settle (BIN3, `wlib.md` W33) that archive loading never sets the strict-check flag, so hints 13–22 are accepted there; (2) §5.4 and new row N13: the string hash stops at the first zero code unit, with a writer test; (3) §14: "milestones M3 and M4". Re-checked in an independent copy of the decompiler project (build 4906.0071.1794.4035, address ranges only, nothing from network, login, launcher or anti-tamper code): the two-condition check in the import-list reader and in the three class-1980 readers; the string hash routine ends at the first zero unit, gives 0 for an empty string and caches its result with the 0xFFFFFFFF sentinel; the generic archive-object loader that calls the record's serializer and the bundle reader (bundle version 5, chunk-512 mode) never set the strict bit, and neither do the stream's mode-setting routines. Data spot checks (independent probes, 2026-10-07): import hints 2 in 383,341 records and 6 and 7 in one each, all 99,863 non-empty lists in bucket order by DID; all 383,343 records walk; the two class-1980 instances hold three empty collections with hint 1 each; none of the 10,025 string keys (9,211 + 792 + 22) contains a zero code unit or is empty, and their buckets plus those of the 21 position keys still recompute from the spec text. Minor, not blocking: in the provenance table rows N12 and N13 sit between N10 and N11 (order only). Remaining OPENs (§3.2 writing tool, §5.4 field meanings, §6/§7 semantics) are precise and non-blocking. |
| **Implements into** | `crates/wstate` (`ObjectState`, `GameObject`, area/region/dungeon resolution); `apps/dattool` / `apps/explorer` object views; `crates/world` and `apps/client` consume §11 |
| **Companion specs** | `docs/specs/wlib.md` (the class library every record is decoded with); `docs/specs/records.md` [§1](records.md#s1) (VLE, TSize and bucket order, strings, name hash), [§4](records.md#s4) (StringInfo layout); `docs/specs/property-system.md` [§4](property-system.md#s4) (value encodings, position §4.6), [§5](property-system.md#s5) (property sets), [§6.2](property-system.md#s6-2) (the +0x09000000 rule), [§8](property-system.md#s8) (DID maps), [§9](property-system.md#s9) (DB-object-type codes); `docs/specs/string-table.md` [§9](string-table.md#s9) (rendering a StringInfo); [`landblock.md`](landblock.md) and [`coordinates.md`](coordinates.md) (M3: block maps, landblock info, map coordinates). This spec starts with a fully reassembled, decompressed record payload. |

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

§9 reports the whole-install decode run, §10 notes an older file form (Reported only) and
the versioned bundle reader, §12 gives implementation guidance, §13 lists disagreements, §14 the open questions, §15
test guidance with a worked synthetic example, §16 corrections to other specs and §17 the
change log.

<a id="provenance"></a>
## Provenance

How sources are marked:

- **Confirmed**: decoded on the user's own install on 2026-10-04 with a throwaway probe in
  the dirty-room scratchpad. Each Confirmed fact says what was decoded and how much.
- **Reported**: comes only from published third-party material and was not, or could not
  be, checked against data.
- **BIN**: read from the client executable under method M4 (decision 0011); "BIN only"
  marks facts the data cannot show.

| Key | Source |
|---|---|
| OBS | Observed in the user's install, 2026-10-04 (Steam install, client data current on that date). **Every** object-state record in `client_gamelogic.dat` was decoded: 383,332 records with prefix 0x70 and 11 with prefix 0x76 (no other prefix in 0x70–0x77 exists). 383,342 of 383,343 decode exactly with zero leftover bytes; the remaining one stops at one undecoded native class (§9). Property sets, string tables, block-map descriptors and DID maps were decoded as needed for §11. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.0 (Apache-2.0 per its distribution's LICENSE), studied as a decompiled binary for facts only; and LotroCompanion `lotro-tools` (area, territory, region and dungeon loaders), read for facts only. |
| MK | Mirarkitty `lotro_extractor` documentation (Apache-2.0). Facts only. |
| DS | `research/notes/data-semantics.md` §5.2, §6.7. |
| ER | `research/notes/engine-client-re.md` §9 (client memory observations). |
| OBS2 | Observed in the same install on 2026-10-05 with new throwaway probes: every object record decoded again (383,343 of 383,343 exactly, with the codec of §5.4 for class 3053), the per-record class tables checked against item order, the import lists checked against the records they name, and the reference census of §8 repeated with the two reference-bearing classes found under BIN. |
| LC2 | The class-index constants of three released builds of the LC library (11.0, 11.4, 11.6; dated 2023-11, 2024-08, 2025-05), facts only. |
| OBS3 | Observed in the same install on 2026-10-05 (repair pass): a new, independent throwaway decoder written from this spec and `property-system.md` decoded **all** 383,343 object records exactly, and re-counted every figure this repair changes: items, markers, labels, attribute values, references, root references, small sets and the bucket fields of classes 131, 134 and 138. Unless a count says otherwise, it covers all 383,343 records. |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, compiled 2026-09-27), disassembled on 2026-10-05 in the dirty room, address ranges only: the object-state record's load routine, the dependency-list reader, the bundle header, class-table and item readers, the native-class registration table and the per-class serializers and reference visitors, and the post-load reference check. **Questions:** what the reserved header fields and the import flags are; how the per-record class table is read; which native classes exist, which of them hold references, and how classes 3053, 72, 192 and 1980 are laid out; how the cases left open by the implementation are treated. **Why M1–M3 were not enough:** class 3053 occurs in one record only and its layout could not be inferred safely from one instance; whether a container holds references is a property of the client's code that coincidental integer values in the data cannot settle; reserved fields that are always 0 cannot be explained by data; no published source covers these (M2) and the running client does not expose them (M3). Nothing from network, login, launcher or anti-tamper code was read. **Repair pass (same build, 2026-10-05):** the serializers of the small sets (classes 18, 105, 134) and the shared serializer of the reference-keyed maps (131, 138), with its element routine, were read. **Questions:** how the small-set header is built; which field orders a class-134 set; what the second u32 of a 131, 134 or 138 element is. **Why M1–M3 were not enough:** in the data the third header byte is always 0, and in class 134 both u32 fields happen to be in bucket order in all five sets with more than one element, so the data cannot decide; no published source covers them. The search for where the bundle version number comes from was stopped before it reached code that may serve network traffic (§10). |
| BIN2 | **M4 (decision 0011), second pass.** Same build 4906.0071.1794.4035 (file version checked again on 2026-10-07), read on 2026-10-07 in the dirty room with a headless decompiler and a disassembler, by address range only: the object record's serializer and its dependency-list structure (constructor, insertion routine, the record's "collect dependencies" method); the routine that builds objects from an archive record and the reading context it sets up; the item reader and the two object-creation routines it chooses between; the per-class hash and equality routines of the native-class registry (classes 160–162, 199, 225); the serializers of classes 72 and 1980 and their element routines; the class-192 serializer; the link-section reader and the record's tear-down routine, plus a bounded byte-pattern search of the code for other readers of the record's link and available-property fields. **Questions:** what the reserved header fields mean; which bundle version and reader mode archive records are read with; how the bucket number of a class-131, 134 or 138 key is computed; the element layouts of class 1980; what the class-72 fields default to; who interprets the link and available-property sections. **Why M1–M3 were not enough:** the reserved fields are 0, 0 and 1 in every record; the version is not stored; the bucket of a key depends on a hash routine that the data can only confirm, not reveal; class 1980 holds no elements; no published source covers any of this (M2); the running client does not expose it (M3). The bundle-version search was kept to the archive-loading path, which sets the version itself (§10). Nothing from network, login, launcher or anti-tamper code was read. |
| OBS4 | Observed in the same install on 2026-10-07 with throwaway probes in the dirty-room scratchpad (all 383,343 object records dumped again and the 6,856 records that can hold classes 131, 134 or 138 decoded): the bucket field of **every** element of classes 131, 134 and 138 (10,048 elements in 677 collections) recomputed from the referenced item with the rules of §5.4; the class-192 type identifiers counted; the flag bytes of the position keys counted. |
| BIN3 | **M4 (decision 0011), repair pass after the 2026-10-07 review.** Same build 4906.0071.1794.4035 (`x64/lotroclient64.exe`), read on 2026-10-07 in the dirty room with a headless decompiler, by address range only: the TSize hint check in the import-list reader and in the three class-1980 collection readers; the routine that reads an archive record into its object and the routine that reads an object record's bundle (the streams they set up and the flags they give them); the reading stream's mode-setting routines; a search of the whole executable for code that sets the stream's strict-check flag; the string-hash routine's end condition. **Questions:** when the hint check fails and whether archive loading enables its strict condition; where the string hash stops. **Why M1–M3 were not enough:** the data contain only hints the client accepted and no key string with a zero code unit, and say nothing about the reading context; no published source covers either (M2); the running client does not expose them (M3). Nothing from network, login, launcher or anti-tamper code was read; the routines found to set the flag lie outside archive loading and were not followed further. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| H1 | Record header and section order (§3) | LC, DS; OBS, OBS3 (383,343 records exact) | Confirmed |
| H2 | Offset-4 class index = class of the root item (§3.1) | LC, DS; OBS, OBS3 (383,343 of 383,343) | Confirmed |
| H3 | Import list = dependency list of (DB-object type, DID, flags); flag 16 marks DIDs that appear as DID items (§3.2) | LC (layout); OBS (counts match exactly); BIN (flags field is a VLE) | Layout Confirmed; meaning of flag 16 Confirmed; flag 0 mostly explained (H4) |
| H4 | 17,473 of the 18,368 flag-0 imports whose DID occurs nowhere in the record are class-wide dependencies on two well-known objects named in the WEENIECONTENT map (§3.2) | OBS2 | Confirmed |
| H5 | Reserved fields: two VLEs that belong to the dependency list, then a boolean (0/1) (§3.1) | BIN; OBS | Confirmed (BIN + OBS) for layout; meaning: H6, H7 |
| H6 | The two VLEs are the dependency list's flag masks: the first excludes, the second requires flag bits of dependencies added later; both 0 = no filter (§3.1) | BIN2; OBS (0 and 0 in all 383,343 records) | Confirmed (BIN + OBS) for the values; the effect is BIN only |
| H7 | The boolean, together with the library's trailing byte, makes the client build the record's script objects without running their class's slot-0 initializer (§3.1) | BIN2; OBS (1 in all 383,343 records; the library byte is 1) | Confirmed (BIN + OBS) for the values; the effect is BIN only |
| H8 | Besides the import list, the client treats the record's property set (DID + 0x09000000) as a dependency with flags 16, except for 0x76 records (§3.2) | BIN2 | BIN only |
| B5 | Archive records are read with bundle version 5, in the attribute-wise mode that takes attribute lists from library chunk 512, and without the post-load reference check (§10) | BIN2; OBS (every record decodes exactly with the newest form) | Confirmed (BIN + OBS) |
| N9 | The bucket number of a class-131, 134 or 138 element is a per-class hash of the referenced item mod *B*: strings (225) by the 28-bit name hash of their UTF-16 code units, positions (160, 161, 162) by a sum of their integer fields, StringInfos (199) by their token or, for a literal, the name hash of its text (§5.4) | BIN2; OBS4 (all 10,048 elements reproduce: string keys 9,211 in class 138, 792 in 131 and 22 in 134; position keys 21 in 134; StringInfo table-reference keys 2 in 138) | Confirmed (BIN + OBS) for strings, positions with region, landblock and cell, and StringInfo references; BIN only for literal StringInfos and for absent position fields |
| N10 | Element layouts of the three collections of class 1980 (§5.4) | BIN2 | BIN only (no element occurs) |
| N12 | The TSize hint check of the import list (§3.2) and of the three class-1980 collections (§5.4): a hint of 23 or more always fails; a hint whose bucket count exceeds 50,000 (hint 13 or more) fails only when the reading stream's strict-check flag is set, and archive loading never sets it (same check as `wlib.md` §4.3, W33) | BIN3 | BIN only (observed import hints are 2, 6 and 7, and every 1980 hint is 1) |
| N13 | The string hash of N9 stops at the first zero code unit (§5.4) | BIN3; OBS (none of the 10,025 string keys contains a zero code unit, re-checked 2026-10-07) | BIN only |
| N11 | A new class-72 value is (0, 0, 0) followed by the client's shared identity rotation (1, 0, 0, 0) (§5.4) | BIN2; OBS (the four-float group is (1, 0, −0, 0) in all 92) | Confirmed (BIN + OBS) |
| B1 | Bundle layout: labels, class table, items (§4) | LC, DS; OBS | Confirmed |
| B2 | The per-record class table repeats the library's attribute list exactly (§4.2) | OBS, OBS3 (1,381,857 of 1,381,857 entries in all records) | Confirmed. Derived by this spec writer. |
| B3 | Class-table contents = script classes instantiated in the bundle plus the root class of every DID item's target; order = hash-bucket order (11 buckets, more for large tables), newest first within a bucket (§4.2) | OBS2 (contents 383,343 of 383,343; order 383,341 of 383,343 directly and the other 2 with the growth rule) | Confirmed. Derived by this spec writer. |
| B4 | On a duplicate class-table entry the first one wins (§4.2) | BIN | BIN only |
| I1 | Item markers 0 (embedded object) and 0x20000000 (DID) (§5.1) | LC; OBS | Confirmed. Markers 0x10000000 and 0x08000000 are Reported (never observed). |
| I2 | Script-object body = attribute values in library-list order (§5.2) | LC; OBS | Confirmed |
| I3 | Reference value 0xFFFFFFFF = null; otherwise a label of the same bundle (§5.2, §8) | OBS, OBS3 (7,294,111 reference attributes in all records: no other value occurs) | Confirmed. Derived by this spec writer. |
| R2 | Keys of class 131 and the first field of class 134 elements are references; with them every non-root item of every record is referenced (§8.1, §8.3) | BIN (reference visitors); OBS2 (792 + 43 values, all labels; 835 of 835 previously unreferenced items resolved) | Confirmed (BIN + OBS) |
| N1 | Native codecs for classes 11, 13, 17, 18, 23, 25, 35, 37, 39, 57, 97, 104, 105, 117, 134, 138, 160, 161, 166, 175, 199, 225, 415, 2479 (§5.3) | LC; OBS | Confirmed (per-class counts in §5.3) |
| N2 | Native codecs for classes 72, 111, 131, 192, 427, 1942, 1980 (§5.3, §5.4) | OBS | Confirmed sizes and layouts; field meanings mostly OPEN. Derived by this spec writer; neither LC nor MK decodes them. |
| N3 | Native codecs for classes 407, 2567, 3103, 3461, 3740 | LC | Reported (never observed). Class 101 is not a native class of this build (N6). |
| N4 | Small-set header: one u32 whose low 24 bits are the count and whose high 8 bits are a TSize-style bucket hint; elements of 18 and 105 in bucket order by value (§5.3) | OBS, OBS3 (91,836 sets); BIN (repair pass: the 24-bit count and the 64-bit remainder for 105) | Confirmed (BIN + OBS); counts of 65,536 or more are BIN only |
| N8 | The second u32 of each element of classes 131, 134 and 138 is the element's **bucket number** in the writer's hash table, and elements are stored in ascending bucket number; the following byte is a flag (§5.3, §8.1) | BIN (repair pass); OBS3 (792 + 43 + 9,213 elements, all below the bucket count; all 677 collections ascending) | Confirmed (BIN + OBS). Corrects the first reading as a plain value (§13). |
| N5 | Native class 3053 layout (§5.4) | BIN; OBS2 (13 of 13 instances decode exactly; the record then ends exactly) | Confirmed (BIN + OBS) |
| N6 | The client registers 258 native classes, all undefined in the library; which of them hold references and where (§5.3, §8.1) | BIN | BIN only for unobserved classes; Confirmed (BIN + OBS) for 131 and 134 (R2) |
| N7 | Layouts of 72 (3 + 4 floats), 192 (optional typed object), 1980 (three hashed collections) and the 2479 filter (eight optional fields) (§5.4) | BIN; OBS | Confirmed (BIN + OBS) for the observed bytes |
| L1 | Link-section layout (§6) | LC; OBS (580,922 links) | Layout Confirmed; meaning OPEN |
| P1 | Available-property section layout (§7) | LC; OBS | Layout Confirmed; meaning OPEN |
| G1 | Area / territory / region / dungeon kinds, their properties and their links (§11) | LC (`lotro-tools`), DS §6.7; OBS (all records of each kind) | Confirmed |
| G2 | Dungeon map position stored in the dungeon's object state (§11.4) | LC (`lotro-tools`); OBS (1,303 dungeons) | Confirmed |
| G3 | Region number 6 has an entry in the REGION DID map and a region record (0x0F prefix), but no cell or map archive is shipped (§11.3) | OBS2 | Confirmed |
| G4 | The well-known root classes kept their indices across LC builds 2023-11 … 2025-05 (§11.1) | LC2; OBS | Reported (stability); Confirmed for this install |
| X1 | Pre-live file encoding (§10). *(The network-form part of this row was withdrawn on 2026-10-05: out of scope until the `docs/LEGAL.md` §9.3 gate.)* | LC | Reported |

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
### 3.1 Header and sections (Confirmed, all 383,343 records)

| Order | Field | Type | Meaning / observed |
|---|---|---|---|
| 1 | did | DID | Equals the DID looked up (all) |
| 2 | root_class | u32 | Class index of the root item (§4.3). Always < the library's class count. |
| 3 | imports | TSize + import × *n* | Dependency list (§3.2) |
| 4 | reserved_a (excluded-flags mask) | VLE | Always 0. Part of the dependency list (H5, H6). |
| 5 | reserved_b (required-flags mask) | VLE | Always 0. Part of the dependency list (H5, H6). |
| 6 | reserved_flag (skip initializers) | bool8 | Always 1. The client fails the load on 2 or more (H5, H7). |
| 7 | bundle_size | u32 | Byte length of the bundle. Never 0 in this install. The client fails the load when it exceeds the bytes left. |
| 8 | bundle | *bundle_size* bytes | §4 |
| 9 | has_links | bool8 | 1 in 192,723 records |
| 10 | links | u32 count + link × count | Present only when `has_links` = 1 (§6) |
| 11 | has_available | bool8 | 1 in 199,295 records |
| 12 | available | TSize + (u32 pid, u32 pid echo) × *n* | Present only when `has_available` = 1 (§7) |
| — | *(end)* | | Zero leftover bytes in all 383,343 records (OBS2, OBS3) |

LC names fields 4–6 "always 0", "always 0" and an unnamed boolean. BIN: the client reads
fields 3–5 as one structure, the dependency list (a hashed table of imports followed by two
VLE values), and field 6 as a boolean of the object record. It keeps the bundle as raw
bytes after reading it and decodes it on demand, which is the model §12 recommends. The
field names `reserved_a`, `reserved_b` and `reserved_flag` are kept because existing code
uses them.

What the fields do (BIN2):

- **Fields 4 and 5: flag masks of the dependency list (H6).** When a dependency (a DID with
  flags, as in §3.2) is added to the list at run time, the client accepts it only if its
  flags share no bit with field 4 and contain every bit of field 5; otherwise the addition is
  ignored. Both are 0 here, which accepts everything. The masks are read after the imports
  and do not filter the stored list.
- **Field 6: skip initializers (H7).** When the client builds objects from the record's
  bundle (§10) and both this boolean and the class library's trailing byte
  (`wlib.md` §3.2) are 1, it creates each script object without running the function in
  slot 0 of its class's virtual-function table (`wlib.md` §6.4); the stored attribute values
  are the object's whole state. When either is 0, it runs that function for each new script
  object before filling in the stored values, and fails if the class has none. Both are 1
  in this install.

A reader that does not run scripts needs none of this. It keeps the three values for
byte-exact rewriting, accepts any VLE values (a warning for non-zero masks is allowed), and
rejects a boolean ≥ 2.

When `bundle_size` is 0, LC skips the bundle and the record has no items; this does not
occur here, but readers must accept it (the object then has no state, only a class).

<a id="s3-2"></a>
### 3.2 Import list (Confirmed layout)

A TSize count *n*, then *n* entries (9 bytes each for every observed entry):

| Field | Type | Meaning |
|---|---|---|
| dbo_type | u32 | DB-object-type code (`property-system.md` §9) of the referenced record |
| did | DID | The referenced record |
| flags | VLE | 16 or 0 observed. The client reads a VLE (BIN); both observed values are one byte long, so the bytes equal the earlier u8 reading. |

Observed (99,863 records have a non-empty list; *n* up to 1,767):

| dbo_type | DID prefix | flags | Entries | Notes |
|---|---|---|---|---|
| 0x4E (appearance) | 0x20 | 0 | 296,906 | Every one of these DIDs also occurs as a u32 elsewhere in the same record |
| 0x45 (game object) | 0x70 | 16 | 43,335 | Exactly the DID items (§5.1) of the bundle |
| 0x45 | 0x76 | 16 | 4 | Likewise (singletons importing other singletons) |
| 0x45 | 0x70 | 0 | 20,726 | 2,358 of these DIDs occur elsewhere in the record and 18,368 do not; of 301 sampled entries of the second kind, 273 do not occur in the object's property set either |

Facts:

- The TSize hint is 2 in 383,341 records (6 and 7 in the two records with the longest
  lists), and the entries are in **bucket order by DID** (`records.md` §1.3.1) in all
  99,863 non-empty lists.
- The flag-16 entries and the DID items correspond one to one: 43,339 of each, and each
  DID item's DID is a flag-16 import of its record.
- LC rejects any dbo_type other than 0x45 and 0x4E; Tessera should accept others and warn.
- The client checks the TSize hint as for class 1980 (§5.4, N12): a hint of 23 or more fails
  the load; a hint from 13 to 22 is accepted, because archive loading never enables the
  strict-check condition that would also reject it.

The import list is not needed to decode the bundle. It is useful as a **prefetch list**: the
records an object depends on.

Flag-0 game-object imports whose DID occurs nowhere else in the record (18,368 entries,
OBS2):

- 17,473 of them are **class-wide dependencies**: every record of a given root class imports
  the same DID, and the DID is one of two well-known objects named in the WEENIECONTENT map
  (0x2800000E, `property-system.md` §8.1; 0x70000214 and 0x7000EAA6 here). For example every
  record of root class 734 (14,475 records) imports 0x70000214. Neither DID occurs in the
  importing records, their property sets or the class library's bytes; the dependency
  follows from the object's class, not from anything stored in the record.
- The other 895 come from three records: 0x7000025B (root class 1486) imports 395 records
  (all of one class), 0x7001A638 (root class 2534) imports 498 records (497 of one class),
  and 0x70034C39 (root class 3720) imports 2. Those DIDs occur nowhere in the importing
  records or their property sets, and the targets do not name the importing records.

Flag 16 marks exactly the DIDs of the bundle's DID items (above); nothing else carries it.

How the client uses the list (H8, BIN2): when it collects the records an object depends on,
it takes every import as stored, whatever its flags and whether or not the bundle names the
DID, and adds one more dependency: the object's property set (DID + 0x09000000,
`property-system.md` §6.2) with flags 16. It does not add one for a 0x76 record, which has
no property set (§2). So a prefetching reader loads every import and the property set.

> **OPEN:** Why the three records of the last bullet import 895 records that they never
> name. The client treats them like any other import (above), so the reason lies with the
> tool that wrote the records; not needed to decode.

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
| class_index | u16 | A script class (the client reads it as signed 16-bit) |
| attribute_count *a* | u16 | |
| attributes | (u32 name hash, u8 type code) × *a* | |

<a id="s4-2"></a>
### 4.2 The per-record class table (Confirmed)

- It lists each **script** class whose instances occur in the bundle, once (no duplicates
  in any record). Native classes never appear in it.
- Each entry's attribute list equals the class library's attribute list for that class
  (`wlib.md` §8), with the same hashes and types **in the same order** (1,381,857 entries
  in all records, no exception). Classes without attributes appear with *a* = 0.
- In 38,413 records the table also lists script classes that have no instance in the
  bundle (38,612 such entries; 38,486 of them are classes with attributes, 126 without).
  **Each of them is the root class of a record named by one of the bundle's DID items**
  (B3). Readers ignore such entries for decoding.
- Every instantiated script class is listed (383,343 of 383,343 records), and every listed
  class is defined in the library (1,381,857 of 1,381,857).

**Contents rule (B3, Confirmed in 383,343 of 383,343 records).** The set of classes in the
table is exactly: the class of every embedded script object, plus the root class (offset 4)
of the target of every DID item.

**Order rule (B3).** The table is written from a hash table keyed by class index:

1. Walk the items in stored order. Each embedded script object contributes its class, each
   DID item the root class of its target; a class is inserted the first time it is met.
2. The table has *B* buckets, with *B* taken from the TSize bucket-count table
   (`records.md` §1.3): it starts at 11. When an insertion would make the count exceed
   2 × *B*, *B* becomes the next value of that table and the existing entries are
   redistributed, each new bucket keeping the relative order its entries had in the written
   sequence.
3. A new class goes to the **front** of bucket (class index mod *B*).
4. The table is written bucket by bucket in ascending bucket number, each bucket front to
   back.

So the class indices modulo *B* never decrease, and within a bucket the most recently
first-met class comes first. This reproduces the stored order in 383,341 records with
*B* = 11 (all tables of at most 22 classes); the two larger tables (32 classes with *B* = 23,
110 classes with *B* = 89) are reproduced by step 2. Only byte-exact writers need it.

**Duplicates (B4, BIN only).** If a class occurs twice, the client keeps the first entry
and discards the later one. No record has a duplicate.

Consequences for a reader:

- The bundle is **self-describing** for script objects: it can be decoded with the
  class table alone, without the library. A reader should still check each entry against
  the library and report a mismatch, because a mismatch means the library and the record
  come from different builds.
- If a script object's class is missing from the table, use the library's list (and warn).

Reported (LC) and BIN: older bundle encodings store each attribute **name** as a string
instead of the u32 hash; the client hashes such names with the name hash of `records.md`
§1.5 (with the result 0xFFFFFFFF replaced by 0xFFFFFFFE). The current files use the hash
form only (§10).

<a id="s4-3"></a>
### 4.3 Labels (Confirmed)

- Labels are unique within a record; the first label is always **0**.
- Labels are ascending in all but 9 records, and contiguous 0 … *n* − 1 in 381,901
  records. In the other 1,442 records some numbers are skipped (all 383,343 records, OBS3). Labels are therefore **names**, not
  positions: a reader maps label → item index.
- Item 0 (label 0) is the **root item**. It is always an embedded script object whose class
  equals `root_class` (§3.1) (383,343 of 383,343).

---

<a id="s5"></a>
## 5. Items

<a id="s5-1"></a>
### 5.1 Item marker (Confirmed for 0 and 0x20000000)

Each item starts with a u32 marker:

| Marker | Item | Then | Observed |
|---|---|---|---|
| 0x00000000 | Embedded object | u16 class index, then the body (§5.2 if the class is defined in the library, else §5.3) | 5,789,528 (all records) |
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

Observed attribute values (all 383,343 records, OBS3): 7,882,705 integers, 7,294,111
references, 1,221,626 floats (no NaN), 1,051,396 64-bit integers, 340,986 code-195 doubles, no
code-131 values. Of the references, 4,455,856 are null and 2,838,255 are labels present in
the same bundle. **No reference attribute holds a DID or any other value.** Links to other records
go through DID items (§5.1), which a reference can point at.

<a id="s5-3"></a>
### 5.3 Native classes and their codecs

The class index of a native object selects its codec. These indices are engine constants:
they are undefined in the class library (`wlib.md` §6.3) and LC has used the same numbers
across many client builds (Reported). Codec names are Tessera's own.

The client registers **258** native classes in this build (N6, BIN), every one an undefined
library entry; 34 of them occur in the data (this table). The registry is not reproduced
here; a class index that is undefined in the library and has no codec in this table is
handled by §5.5 whether or not the client knows it. Class 101 (a Reported LC codec) is
**not** registered in this build.

Common layouts used in the table:

- **hashed list** = TSize count *n* (`records.md` §1.3) followed by *n* elements; the
  elements are in bucket order by key where checked.
- **counted list** = u32 count *n* followed by *n* elements, in stored order.
- **small set** = a u32 header, then *n* elements. The header's low 24 bits are the count
  *n* and its high 8 bits are a TSize-style bucket hint *h* (`records.md` §1.3); in the
  stored bytes the count fills bytes 0–2 (little-endian) and the hint is byte 3 (N4, BIN).
  The elements are in ascending order of their bucket number, which the table gives per
  class. Observed (OBS3, all 91,836 sets: 90,232 of class 18, 930 of 105 and 674 of 134):
  byte 2 is 0 in every set (no count reaches 65,536); the hint is 0–8 in class 18
  (0: 85,225; 1: 4,857; 2 to 8: 150) and 1 in every set of 105 and 134; bucket order holds
  in every set with two or more elements (8,096 of class 18, 108 of 105, 5 of 134). The
  client clamps a hint beyond its bucket-count table to the table's last entry (BIN); a
  reader needs only *n*.

| Class | Codec | Body | Values are references? | Instances (records) | Mark |
|---|---|---|---|---|---|
| 11 | `map_u32` | hashed list of (u32 key, u32 value) | no | 155,041 (53,272) | Confirmed; bucket order by key in all |
| 13 | `multimap_u32` | as 11; keys may repeat | no | 115,685 (115,685) | Confirmed; bucket order in all |
| 35 | `refmap_u32` | as 11 | **yes** (64 null) | 269,163 (164,928) | Confirmed; bucket order in all |
| 37 | `refmultimap_u32` | as 13 | **yes** | 20,808 (20,808) | Confirmed; bucket order in all |
| 17, 25 | `array_u32` | counted list of u32 | no (integers, DIDs, float bit patterns) | 137,960 (62,125) / 387,414 (54,497) | Confirmed |
| 176, 182 | `refarray` | counted list of u32 | **yes** (176: 5,173 null) | 150,107 (58,593) / 314,721 (169,254) | Confirmed |
| 18 | `set_u32` | small set of u32; bucket = value mod *B* | no | 90,232 (31,361) | Confirmed |
| 105 | `set_u64` | small set of u64; bucket = the whole 64-bit value mod *B* (BIN; either 32-bit half alone breaks the stored order in 56 or 60 of the 108 sets with two or more elements) | no | 930 (263) | Confirmed |
| 134 | `ref_set_triple` (formerly `set_triple`) | small set of (u32 reference, u32 bucket, u8 flag); bucket = the second field (§5.4, N8); the flag was 0 in all 43 elements | **yes, first field** (R2) | 674 (184) | Confirmed (BIN + OBS) |
| 23 | `map_u32_u64` | hashed list of (u32 key, u64 value) | no | 148 (147) | Confirmed |
| 97 | `map_u64_u32` | hashed list of (u64 key, u32 value) | no (floats, PIDs, DIDs) | 7 (5) | Confirmed |
| 117 | `refmap_u64` | hashed list of (u64 key, u32 value); bucket order by the u64 key | **yes** | 725 (169) | Confirmed |
| 138 | `refpair_map` | hashed list of (u32 key reference, u32 bucket, bool8 flag = 0, u32 value reference); ascending by bucket (§5.4, N8) | **yes, first and last field** | 2 (2) | Confirmed (9,213 elements) |
| 131 | `refkey_map_tagged` (formerly `map_u32_tagged`) | hashed list of (u32 key reference, u32 bucket, bool8 flag = 0, u32 value); ascending by bucket (§5.4, N8) | **yes, the key** (R2); the value (last field) is a 0x70 DID in all 792 elements | 1 (1) | Confirmed (792 elements, BIN + OBS) |
| 104 | `array_u64` | counted list of u64 | no | 2,252 (2,252) | Confirmed |
| 111 | `array_u64` | counted list of u64 | no | 2 (2) | Confirmed |
| 39 | `property` | one property entry **without** echo: u32 PID then the value (`property-system.md` §5.2, array-element form). PID 0 = null entry with no value (BIN). | no | 862,882 (35,408) | Confirmed |
| 166 | `property_set` | a property set without the leading DID: TSize, then entries with echo (`property-system.md` §5) | no | 292,684 (151,744) | Confirmed |
| 57 | `bitset` | VLE bit count *b*, then ⌈*b*/8⌉ bytes; bit *i* is bit (*i* mod 8) of byte ⌊*i*/8⌋ | no | 381 (144) | Confirmed |
| 160, 161 | `position` | a position value (`property-system.md` §4.6) | no | 15,629 (9,670) / 4,628 (1,487) | Confirmed (§5.4) |
| 175 | `weighted_table` | counted list of (u32 value, u32 weight, u32 unknown), then u32 that is 0 | no | 8,131 (8,111) | Confirmed; trailing u32 = 0 in all |
| 199 | `string_info` | StringInfo (`records.md` §4) | no | 88,199 (36,287) | Confirmed; no argument blocks |
| 225 | `string` | UTF-16 string (`records.md` §1.4) | no | 10,094 (90) | Confirmed |
| 72 | `frame` | 3 × f32, then 4 × f32 (§5.4) | no | 92 (20) | Confirmed (BIN + OBS) |
| 192 | `typed_object` (formerly `sphere`) | optional object with a 16-byte type identifier (§5.4) | no | 380 (380) | Confirmed (BIN + OBS) |
| 427 | `scene_info` | variable (§5.4) | no | 1,172 (1,172) | Confirmed |
| 1942 | `barter_entry` | 49 bytes (§5.4) | no | 42,214 (3,449) | Confirmed size |
| 1980 | `triple_table` (formerly `opaque6`) | three hashed collections (§5.4) | no | 2 (2) | Confirmed (BIN + OBS) for the observed, empty form; element layouts BIN only (N10) |
| 415 | `options_profile` | variable (§5.4) | no | 138 (138) | Confirmed for the observed form |
| 2479 | `quest_target_locations` | variable (§5.4) | no | 1 (1) | Confirmed |
| 3053 | `nested_string_table` | variable (§5.4) | no | 13 (1) | Confirmed (BIN + OBS) |
| 407 | `compressed_bitset` | u32 size, then that many bytes: u32 0, u32 unpacked size, zlib stream of a bitset | no | 0 | Reported (LC) |
| 2567 | `vault_item` | u64 instance ID, property set (as 166), one property (as 39) | no | 0 | Reported (LC) |
| 3103 | `vault` | hashed list of (u32 id, UTF-16 string), then u32 capacity, u32 used | no | 0 | Reported (LC) |
| 3461 | `triple_u32` | 3 × u32 | no | 0 | Reported (LC) |
| 3740 | `u32` | 1 × u32 | no | 0 | Reported (LC) |

Instance counts (and record counts in parentheses) cover all 383,343 records (OBS2),
including record 0x70022DBA, which now decodes completely (§9); the first version of this
table excluded that record. The StringInfo split of the first version (71,210 table
references, 8,192 literals) counted the records without it.

Class 101 (`multimap_u64_u32`, counted list of (u64 key, u32 value), Reported LC) was removed
from the table: this client does not register it. A reader may keep the codec for older
data.

**Reference-bearing native classes that do not occur in this install (N6, BIN only).** The
client also visits references in these registered classes; their layouts are unknown, so
§5.5 applies if one ever occurs:

| Where the reference sits | Classes |
|---|---|
| The first u32 of the body (a single reference) | 12, 14, 19, 24, 26, 29, 36, 38, 98, 100, 106, 110, 112, 114, 118, 120, 132, 135, 137, 139, 141, 183, 3262 |
| The value of each map element (like 35) | 119 |
| The first field of each element (like 131 and 134) | 136, 3261 |
| The first and last field of each element (like 138) | 140 |
| The second u32 of each 8-byte element of a list | 2238, 3891 |
| The third u32 of each 16-byte element of a list | 3309 |

<a id="s5-4"></a>
### 5.4 Details of the less regular codecs

**Reference-keyed collections (131, 134, 138; N8).** These three classes are hash
collections whose key is a reference to another item of the bundle, hashed by the
**content** of the referenced item (a string, a position, ...) rather than by the label. So
that a reader does not have to recompute that hash, every key is stored with its bucket:

| Field | Type | Meaning |
|---|---|---|
| key | u32 | Reference (§8.1): a label of this bundle |
| bucket | u32 | The key's bucket number in the writer's table: 0 ≤ bucket < *B*, where *B* is the bucket count of the collection's hint (`records.md` §1.3) |
| flag | u8 | 0 in every stored element. The client fails the load on 2 or more (BIN). |
| value | u32 | 131: a plain u32 (a 0x70 DID in all 792 elements); 138: a reference. Class 134 is a set and has no value field. |

Facts (BIN, repair pass; OBS3):

- Elements are stored in **ascending bucket number** (1 of 1 collection of class 131, 674 of
  674 sets of 134, 2 of 2 collections of 138), and every bucket number is below *B*
  (792 + 43 + 9,213 elements). A run of elements with the same bucket keeps its stored order.
- On load the client places each element in the bucket given by the stored field; it does not
  recompute the hash and does not compare the field with *B*.
- Because the bucket numbers are small, they often equal some label of the bundle (all 43
  bucket fields of class 134 and 383 of the 792 of class 131 do). They are **not**
  references.

Reader rule: keep the bucket field for byte-exact rewriting and do not interpret it. A bucket
number ≥ *B*, or a bucket sequence that decreases, is reported as a warning; decoding
continues, since the field is not needed to read the collection.

<a id="s5-4-1"></a>
**Bucket of a key (N9, BIN2; Confirmed on all 10,048 stored elements, OBS4).** The writer's
bucket number is *H* mod *B*, where *B* is the bucket count of the collection's hint
(`records.md` §1.3; hints above 12 in `wlib.md` §7.1) and *H* is a hash of the **referenced
item's content**, chosen by that item's class:

| Key item class | *H* | Confirmed on |
|---|---|---|
| 225 (string) | The 28-bit name hash of `records.md` §1.5, computed over the string's UTF-16 code units (each unit, 0–65535, takes the place of a byte). The hash **stops at the first zero code unit** (N13): a string that contains U+0000 hashes only the units before it, so all strings that agree up to their first U+0000 share a hash. The empty string, and a string that starts with U+0000, give 0. | 10,025 keys (classes 131, 134, 138; none contains U+0000) |
| 160, 161, 162 (position) | (region × 2²⁴ + landblock X × 2¹⁶ + landblock Y × 2⁸ + instance + cell) mod 2³², with each absent field at the value `property-system.md` §4.6 gives it (region 0, X and Y 0xFF, instance 0, cell 0xFFFF). Offsets and orientation do not enter. | 21 keys of class 134, all with region, landblock and cell present; the absent-field values are BIN only |
| 199 (StringInfo) | A table reference: its token (`records.md` §4.1). A literal: the name hash of its text, as for class 225. | 2 table-reference keys of class 138; literals BIN only |

No other native class has a hash routine in this build (BIN2), so no other kind of item can
be a key. The client caches a string's hash; the result 0xFFFFFFFF would be replaced by
0xFFFFFFFE, which a 28-bit hash never produces.

Key equality, which a writer needs to avoid duplicate keys (BIN2): two strings are equal when
their code units are equal; two StringInfos when their kind and token (or text) are equal;
positions of class 161 when region, landblock X and Y, instance and cell are equal; class 162
also compares x, y and z, and class 160 also x, y, z and the four orientation components,
each within an absolute difference of 0.0002.

With these rules a writer can add elements: compute the bucket, insert the element after the
existing elements of that bucket (the client keeps the stored order within a bucket when it
reads), and write the collection in ascending bucket order.

**Position (160, 161).** Same encoding as the property value (`property-system.md` §4.6),
including the flag rules found there, which hold here too: bit 0x40 is set exactly when
the region byte is present and equals 1 (5,906 of 5,906; clear in all 14,351 others), and
bit 0x80 exactly when the cell field is present and equals 0 (18,306 of 18,306). Observed
flag bytes: 0x9B, 0x8B, 0xBB, 0xCB, 0xDB, 0xFB, 0x3B, 0x1B, 0x03, 0x7B, 0x43, 0x5B, 0x0B. A
zero flag byte (no position) did not occur.

**Frame (72), 28 bytes.** The client writes a three-float vector, then a four-float group,
with two separate routines (BIN). Observed: the three vary (x ∈ {0, ±2.5, 2, ...},
y ∈ {−4.5, −7, 0, −9.5, ...}, z ∈ {0, 0.05}); the four were (1, 0, −0, 0) in all 92. A new
class-72 value starts as (0, 0, 0) followed by the client's shared identity-rotation
constant (1, 0, 0, 0), the scalar-first identity that the client uses wherever an
orientation defaults (N11, BIN2). So the frame is an **offset (x, y, z) followed by a
rotation (w, x, y, z), scalar first**, as for positions (`property-system.md` §4.6), in
units of `coordinates.md`. What the frame is relative to is OPEN (§14).

**Typed object (192).** An optional, polymorphic object (BIN):

| Field | Type | Meaning |
|---|---|---|
| present | bool8 | 0 = no object, nothing follows; 1 = an object follows; 2 or more is malformed |
| type_id | 16 bytes | Present when `present` = 1. Identifier of the object's type (a GUID-style class identifier) |
| body | per type | Present when `present` = 1. The object's own serialization |

All 380 instances are present and carry the same type identifier; for that type the body is
16 bytes: three f32 (0 in all 380) and one f32 (15 distinct positive values), 33 bytes in
all. A centre offset and a radius fit; that reading is not confirmed. The body length of
other types is not known. Reader rule: when `present` = 1, keep the identifier and read a
16-byte body; if the record then fails to decode exactly, report "unsupported type in
class 192" (the identifier tells which), as for an unknown native class (§5.5). Readers
must not rely on the identifier's value, which this spec does not reproduce.

**Triple table (1980), formerly "opaque6".** Three TSize-prefixed hashed collections in a
row (BIN). Both instances are `01 00` three times: three empty collections with hint 1.
Element layouts (N10, BIN2; no element occurs in this install):

| Collection | Element | Bytes | Bucket |
|---|---|---|---|
| 1 | u32 key, u32 value | 8 | key mod *B* |
| 2 | u64 key, u32 value | 12 | (low 32 bits of the key + high 32 bits) mod 2³², then mod *B* |
| 3 | u64 key, StringInfo value (as class 199) | 8 + StringInfo | as collection 2 |

For each collection the client checks the TSize hint (N12, BIN3) and fails the load when
either condition holds: the hint is 23 or more (beyond its 23-entry bucket-count table,
`wlib.md` §7.1); or the reading stream's strict-check flag is set and the bucket count for
the hint is more than 50,000, which is every hint from 13 to 22. Object-record bundles are
read on a stream that never has the strict-check flag (§10, `wlib.md` §4.3), so a reader of
archive data rejects only hints of 23 or more and accepts hints 13 to 22. The client also
checks the count against the bytes left before allocating. A reader decodes non-empty collections
with these layouts; the field meanings are OPEN (§14).

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
   by one u32 for each of its set bits, in ascending bit order from 0x01 to **0x80** (BIN:
   the client reads a u32 for bit 0x80 too; the data never set it).

LC uses this item as the table of world positions of NPCs and quest targets (Reported). It
is not needed for M3.

**Nested string table (3053).** 13 instances, all in record 0x70022DBA (items 9,121 to
9,133). Layout (N5, BIN; all 13 decode exactly):

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | outer | TSize + outer element × *n* | Hashed by key (observed hint 1, *n* 1–3) |
| 2 | list_a | u32 count + u32 × count | Observed values are keys of the outer table |
| 3 | list_b | u32 count + u32 × count | Likewise |

Outer element: u32 key, u32 count *m*, then *m* × (UTF-16 string, inner table).

Inner table: TSize + inner element × *k* (observed hint 1, *k* 1–3). Inner element: u32 key,
u32 count *c*, then *c* × three UTF-16 strings.

Observed strings are zero to five characters long. Field meanings are OPEN; nothing in M3
or M4 needs them.

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

No record of this install needs this path any more (§9). It remains the rule for future
data, for classes the table of §5.3 does not cover, and for the open cases of class 192
and 1980 (§5.4).

---

<a id="s6"></a>
## 6. Link section (Confirmed layout)

Present when `has_links` = 1, in which case the u32 count is at least 1 (192,723 records;
up to 9 links each; 580,922 links in total).

| Field | Type | Observed |
|---|---|---|
| direction | bool8 | 0 (419,091) or 1 (161,831); the client fails the load on 2 or more (BIN) |
| link_type | u32 | 8, 12, 13, 2, 1, 19, 11, 7, and 0x10000007 … 0x10000019 (table below) |
| limit | u32 | 1, 99, 250, 100, 4, 25, 20, 3, 0, 200, 999, 10, 5, 275, 350, 150 |
| flag | u32 | 0 (560,618) or 1 (20,304) |
| pids | TSize + (u32 pid, u32 pid echo) × *m* | *m* = 0, 1, 2 or 3; echo equals pid in all 206,924 pairs; every PID is defined in the dictionary. Same structure as the §7 list (the client uses one reader for both, BIN). |
| terminator | u8 | 0 when link_type = 13 (120,606 of 120,606), else 3. The client stores it as a plain byte without a range check (BIN). |

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
> does not appear in the landblock link-type list. The client keeps each link as stored
> with the record, and the record's own routines do not interpret it; a bounded search
> found no other reader (BIN2, §14).

---

<a id="s7"></a>
## 7. Available-property section (Confirmed layout)

Present when `has_available` = 1 (199,295 records): a TSize count *n* (0–57; hint 1 in all
2,619 sampled lists) followed by *n* pairs (u32 pid, u32 pid echo). The echo equals the pid
in every pair (about 7 million), every PID is defined in the dictionary, and pairs are in
bucket order by PID. LC rejects a pair whose echo differs; the client stores both words
without comparing them (BIN). A reader should warn on a mismatch.

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
| `refpair_map` (138) | Key (first field) and value (last field) of each element |
| `refkey_map_tagged` (131) | Keys (first field; not the bucket field and not the value) |
| `ref_set_triple` (134) | First field of each element (not the bucket field) |

The list follows the client's own reference visitors (N6, BIN), which also name the
unobserved classes of §5.3.

A reference is either **0xFFFFFFFF (null)** or a label of the same bundle. Observed in all
383,343 records (OBS3; labels / null): attributes 2,838,255 / 4,455,856; 176 elements
1,096,735 / 5,173; 182 elements 581,959 / 0; 35 values 997,043 / 64; 37 values 82,291 / 0;
117 values 518 / 0; 138 fields 18,426 / 0; 131 keys 792 / 0 (every one a string item, class
225); 134 first fields 43 / 0 (positions, class 160, and strings). No reference anywhere held
an absent label. (The first version gave the counts of the 383,342 records without
0x70022DBA; the differences are exactly that record's references.)

Plain containers (11, 13, 17, 25, 97, ...) and the non-reference fields above hold integers,
PIDs, DIDs, float bit patterns or bucket numbers. Their values sometimes equal a label by
coincidence (all 43 bucket fields of class 134 and 383 of the 792 bucket fields of class 131
do, §5.4); they are **not** references.

<a id="s8-2"></a>
### 8.2 Resolution algorithm (in words)

1. Build a map from label to item index (labels are names, §4.3).
2. A reference value resolves to the item with that label; 0xFFFFFFFF resolves to "none".
3. An item that is a DID item stands for **another object record**. Resolving through it
   loads that record (lazily). DID items always point at existing 0x70/0x76 records here.
4. The graph may contain cycles: in 139 records some item refers back to the root (OBS3,
   counting exactly the references of §8.1).
   Resolution must be lazy or keep a visited set; it must never expand references
   recursively without one.

<a id="s8-3"></a>
### 8.3 Shape of the graph (Confirmed)

- With the references of §8.1, **every** non-root item of every record is referenced at
  least once (383,343 of 383,343 records; OBS2, OBS3). The 835 items that the first version
  of this spec found unreferenced (814 strings of class 225 and 21 positions of class 160, in
  30 records) are the targets of 131 keys and 134 first fields.
- The root is referenced by no item in 383,204 records, so there it is the only entry point.
- In the other 139 records the root is referenced too (cycles, above). The first version's
  count of 140 included record 0x7001A638 only because three class-131 bucket fields equal
  to 0 (the root's label) were counted as references; they are not (§5.4).
- An item can be referenced many times (up to 324 times in one record); shared sub-objects
  are normal.

---

<a id="s9"></a>
## 9. Whole-install statistics (Confirmed, 2026-10-04)

| Measure | Value |
|---|---|
| Object records | 383,343 (0x70: 383,332; 0x76: 11) |
| Decoded exactly (record and bundle consumed to the last byte) | **383,343 (100%)** with the 3053 codec of §5.4 (OBS2). Without it, 383,342; record 0x70022DBA (root class 3044) then stops at item 9,121 of 9,135. Its last item (9,134) is a `set_u32`, after which the bundle and the record end exactly. |
| Records with trailing bytes | 0 |
| Decoded with only the native codecs published by LC | 378,316 (98.69%). The other 5,027 fail on native classes 1942 (3,449 records), 427 (1,172), 192 (380), 72 (20), 1980 (2), 111 (2), 131 (1) and 3053 (1). |
| Items | 5,832,867 in all records (sum of every `item_count`, OBS3): 5,789,528 embedded objects (2,817,016 of them script objects, the rest the native instances of §5.3) and 43,339 DID items. Without record 0x70022DBA (9,135 items): 5,823,732. |
| Distinct root classes | 869 in 0x70 and 11 in 0x76; 877 in all, because classes 1547, 1556 and 1603 occur in both (`wlib.md` §7) |
| Distinct script classes instantiated | 1,169 |
| Distinct native classes instantiated | 34 (the 33 of the first version plus 3053) |

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
## 10. Other encodings (older file forms)

These forms are **not** used by the files of the current client. They are recorded so the
model stays general; none was checked against data. This spec describes the stored form in
the archives only.

- *Withdrawn (2026-10-05):* a bullet that described how object state is encoded in the
  official game protocol. That is out of scope until the `docs/LEGAL.md` §9.3 gate is passed
  (decision 0004), and no maintainer approval for it as T2 design input is recorded, as for
  `property-system.md` §10. Tessera's own protocol (M7) defines its own encoding.
- **Pre-live form** (older clients; Reported, LC): no import list, no reserved fields, no
  link and available-property flags (both sections always present); the bundle starts with
  a u32 label count and items are flag words that carry a package ID, a native type and a
  slot count, with each slot as (u32 value, u32 type code). Tessera does not support it.
- **Versioned bundle reader** (BIN): before bundle version 5 the class table names
  attributes by string (hashed on load, §4.2); before version 4 an embedded object's class
  index is followed by a second field, both u16 (versions 2–3) or both u32 (earlier);
  versions 2 and later read the per-record class table. The version is not stored in the
  record: it is a property of the reading context. The current files decode exactly with the
  newest form (version 5 or later) and Tessera reads only that form.

<a id="s10-1"></a>
**How the client reads archive records (B5, BIN2).** The routine that builds objects from an
object record's bundle sets up its own reading context and does not take the version from
elsewhere:

- it registers bundle version **5** for the object-graph stream itself, so archive records are
  always read with the newest form above (attribute hashes, no second class-index field);
- it reads in the attribute-wise mode that takes each script class's attribute list from
  library chunk 512 (`wlib.md` §3.3, §8.4), never from chunk 256;
- it passes the record's boolean (§3.1, field 6) to the object-creation step (the
  skip-initializer rule of §3.1);
- it does not enable the post-load reference-type check of library chunk 128
  (`wlib.md` §4.3).

So a reader of archive records needs no version parameter: the form is fixed at version 5.
A bundle that does not decode with that form is malformed. The older forms remain Reported
for other reading contexts only, which Tessera does not support (the search stayed on the
archive-loading path, decision 0004).

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
observation for this install, useful as a fast hint, and must not be hard-coded. (Indices of
other well-known kinds did not change across LC builds from 2023-11 to 2025-05, G4, so a
change is unlikely but possible.)

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

Region number 6 (G3): the REGION DID map (0x28000008, `property-system.md` §8.1) has a key 6
with its own label and a region-description record (prefix 0x0F; 0x0F000009 here), like
regions 1–5 and 14. One region record (`Area_RegionID` 6) and one dungeon position use it.
The install ships no cell or map archive for region 6, so no landblock carries it. A reader
treats region 6 as a valid region number without terrain. (The REGION map also has a key
15 whose 0x0F record is missing, while small region-15 archives exist.)

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
- **Reference typing.** The client checks a reference's target class only when the library
  has chunk 128 (`wlib.md` §4.3); this install has none, so the client loads references
  without a type check and a Tessera reader needs none either.
- **Writers.** A byte-exact writer must keep every TSize hint (imports, maps, sets,
  available properties), the small-set header, the bucket fields of classes 131, 134 and
  138 (§5.4), the label numbers, the class-table order and its extra entries.
- **Small-set bounds.** The count of a small set is 24 bits wide (§5.3); check it against
  the bytes left (4 bytes per element for 18, 8 for 105, 9 for 134) before allocating.

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
| LC | The import flags field is a byte. | It is a VLE (BIN); all observed values encode as one byte (§3.2). |
| LC | Class 134 is a set and class 131 a map of plain values. | Their first field and key are references (§8.1). |
| LC; first version of this spec | The second u32 of a 131, 134 or 138 element is a value. | It is the element's bucket number (§5.4, N8). The value of 131 is the last field. |
| LC; first version of this spec | A small set starts with a u16 count, a zero byte and a hint byte. | The first three bytes are one 24-bit count (§5.3, N4); the bytes agree for every count below 65,536. |
| LC | Native class 101 exists. | Not registered by this client (§5.3). |
| LC | A 2479 filter has at most seven optional fields. | Eight: bit 0x80 also carries a u32 (§5.4). |

---

<a id="s14"></a>
## 14. Open questions

Closed on 2026-10-05 (kept here so earlier references still resolve):

- *Order and extra entries of the per-record class table* — §4.2 contents and order rules.
- *Native class 3053* — layout in §5.4; every record now decodes. (The first version said
  "one instance"; there are 13 items in that one record.)
- *The 835 unreferenced items* — referenced by 131 keys and 134 first fields (§8.1, §8.3).
- *Region number 6* — a defined region without shipped terrain (§11.3).
- *Flag-0 imports* — mostly class-wide dependencies (§3.2); a narrow OPEN remains there.
- *Area class-index stability* — Reported stable (G4); discovery by data stays the rule.

Closed on 2026-10-07 (BIN2, OBS4):

- *Reserved header fields* — the two VLEs are the dependency list's excluded-flags and
  required-flags masks; the boolean is the skip-initializer switch (§3.1, H6, H7).
- *Content hash of 131/134/138 keys* — per key class in §5.4 (N9), reproduced on all 10,048
  stored elements.
- *Bundle version* — archive records are always read as version 5, in the chunk-512 mode,
  without the chunk-128 check (§10, B5).
- *Element layouts of class 1980* — §5.4 (N10); field meanings stay open below.
- *Frame (72) components* — an offset followed by a scalar-first rotation (§5.4, N11); what it
  is relative to stays open below.

Closed on 2026-10-07 in the repair pass (BIN3):

- *Strict TSize hint check on the archive path* — archive loading never sets the strict-check
  flag, so only hints of 23 or more fail there (§3.2, §5.4, N12).

Still open:

> **OPEN:** Why records 0x7000025B, 0x7001A638 and 0x70034C39 import 895 records that they
> never name (§3.2). The client treats them like any other import (H8), so the answer lies
> with the tool that wrote the records, not with the client; no M4 reading can settle it.

> **OPEN:** Field meanings inside `frame` (72: what the offset and rotation are relative
> to), `typed_object` (192: what the observed type is, beyond its layout), `scene_info`
> (427), `barter_entry` (1942), `triple_table` (1980: what the three collections map) and
> `nested_string_table` (3053) (§5.4). None is needed to decode, or for milestones M3 and M4.

> **OPEN:** Semantics of the link section (§6: direction, limit, flag, terminator, kind 13)
> and of the available-property section (§7). BIN2 read the section reader (one element is
> a boolean, three u32, the PID list and the trailing byte, stored as read) and found no
> other code in the record's own routines that interprets them; a bounded search for other
> readers found none. Not needed to decode.

Answers to the implementation OPEN of 2026-10-04 (choices `tessera-wstate` made where the
spec was silent):

| Choice | Answer |
|---|---|
| `property` (39) with PID 0 is a null entry with no value; an unknown PID fails the record | **Confirmed (BIN).** The client treats PID 0 exactly so and fails the load on a PID it cannot resolve. PID 0 does not occur in the data. |
| `options_profile` (415), Reported form: u16 and six u32 once after all groups; *t1* > 1 and unknown slot kinds rejected | **Accepted as Tessera's rule.** Only the observed form occurs (138 of 138); the alternative form stays Reported and was not checked against the client. |
| 2479 filter flag with bit 0x80 rejected | **Change it:** bit 0x80 carries a u32 like the other seven bits (§5.4, BIN). The part-2 count is a count, as implemented. |
| Class listed twice in a per-record class table: the first entry wins | **Confirmed (BIN, B4).** |
| A reference to an absent label fails the record; partial records may refer to labels of undecoded items | **Accepted as Tessera's rule.** No record has such a reference. |
| A bad item marker fails the record | **Accepted.** |

Answers to the count reconciliation of 2026-10-04 (all confirmed by OBS2):

- The old §5.3 and §9 figures **excluded** record 0x70022DBA. Both now cover all records:
  class 199: 88,199; 17: 137,960; 176: 150,107. *Corrected 2026-10-05 (OBS3):* the item total
  is **5,832,867** (5,789,528 embedded objects), not 5,832,853; the earlier figure counted
  only the 9,121 items of 0x70022DBA that decoded before the 3053 codec existed, not its
  9,135.
- 877 distinct root classes in all: classes 1547, 1556 and 1603 root both a 0x76 singleton
  and ordinary 0x70 records (§9).
- A scene-info entry is 41 bytes with a table-reference StringInfo; with a literal
  StringInfo of an empty string it is 34 bytes. Bounding counts with 34 is right.

Answer to the real-data-test note of 2026-10-04: resolving every area record found by its
property signature covers every area a block map can name, so it satisfies §15. A test that
reads the block-map `Area_DID`s through `landblock.md` is preferred when convenient; both
are acceptable.

---

<a id="s15"></a>
## 15. Test guidance

Synthetic fixtures (generated by our own code; no game data):

- a synthetic class library (`wlib.md` §14) defining class 0x0123 with one reference
  attribute (name hash 0x0ABCDEF0) and the worked example below;
- one fixture per codec of §5.3 with 0, 1 and several elements, including null references
  in `refarray` and `refmap_u32`, a `small set` whose elements are in bucket order for its
  hint, and a position for each flag bit;
- a `nested_string_table` (3053) with two outer keys, one of them with an inner table of two
  keys; a `typed_object` (192) that is absent (one byte 0) and one that is present; a
  `triple_table` (1980) with three empty collections; the same with the first collection's
  hint set to 13 and to 22 (must decode, §5.4) and to 23 (must fail); a 2479 filter with
  bit 0x80 set;
- a `refkey_map_tagged` (131) whose key points at a string item and a `ref_set_triple`
  (134) whose first field points at a position: both targets count as referenced; a 131
  element whose bucket field equals the root's label: the root must **not** count as
  referenced; a 134 set with a bucket field ≥ *B* and one whose bucket fields decrease: both
  decode, with a warning (§5.4); a writer test that adds a string key containing U+0000
  (for example "ab", U+0000, "c") and checks that its bucket equals the bucket of "ab"
  (§5.4, N13);
- small-set headers (§15.2): a set of class 18 in bucket order for hint 1; a header whose
  count needs the third byte (for example 70,000 elements claimed with too few bytes left
  must fail as "count larger than the remaining bytes", not be read as 4,464);
- a record with a DID item and a matching flag-16 import; a record whose root references
  itself (cycle); a record whose class table lists an extra class;
- malformed variants: bad marker; unknown native class at item 2 (expect a partial result
  with items 0 and 1); a reference to an absent label; a label list with a duplicate; a
  count larger than the remaining bytes (must fail without allocating); trailing bytes in
  the bundle and after the available-property section; a per-record class table that
  disagrees with the library (warning, decode by the table).

Real-data tests (skipped without `TESSERA_GAME_DIR`):

- decode every 0x70 and 0x76 record; assert zero trailing bytes for every fully decoded
  record and that the only partial decodes are unknown-native-class cases (with the 3053
  codec there are none in this install); print the counts of §9 (do not assert them);
- for every record, rebuild the class-table set from the items (§4.2 contents rule) and
  assert it matches; report how many stored orders the order rule reproduces (do not
  assert the count);
- assert that every non-root item of every fully decoded record is referenced (§8.3),
  counting the references of §8.1;
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

<a id="s15-2"></a>
### 15.2 Worked synthetic example: small sets

A class-18 set (`set_u32`) with hint 1 (*B* = 23) holding 5, 30 and 47. Buckets: 5 mod 23 = 5,
30 mod 23 = 7, 47 mod 23 = 1, so the stored order is 47, 5, 30. 16 bytes:

| Bytes | Meaning |
|---|---|
| `03 00 00 01` | header u32 0x01000003: count 3 (low 24 bits), hint 1 (high byte) |
| `2F 00 00 00` | 47 (bucket 1) |
| `05 00 00 00` | 5 (bucket 5) |
| `1E 00 00 00` | 30 (bucket 7) |

A class-134 set (`ref_set_triple`) with hint 0 (*B* = 11) holding references to labels 3 and
1, whose writer placed them in buckets 4 and 9. 22 bytes:

| Bytes | Meaning |
|---|---|
| `02 00 00 00` | header: count 2, hint 0 |
| `03 00 00 00` `04 00 00 00` `00` | reference to label 3, bucket 4, flag 0 |
| `01 00 00 00` `09 00 00 00` `00` | reference to label 1, bucket 9, flag 0 |

Labels 3 and 1 count as referenced; the bucket values 4 and 9 do not refer to anything, even if
the bundle has items with those labels. A header `70 11 01 02` claims 70,000 elements
(0x011170) with hint 2.

---

<a id="s16"></a>
## 16. Corrections to other specs

Recorded here instead of editing those specs:

| Spec | Section | Note |
|---|---|---|
| `records.md` | §1.3.1 OPEN (order within a bucket) | In the per-record class tables of object state (a hash table with the same bucket counts as TSize), entries that share a bucket are written **newest first** (reverse order of insertion), and the table grows to the next bucket count when an insertion would exceed twice the bucket count (§4.2). Whether every TSize collection follows the same rule was not checked. |
| `property-system.md` | §8.3 (dangling REGION entry) | The dangling REGION entry is key 15; key 6 resolves to an existing 0x0F record (§11.3). For information; nothing there is wrong. |

---

<a id="s17"></a>
## 17. Change log

| Date | Change |
|---|---|
| 2026-10-04 | First version (Review). |
| 2026-10-05 | OBS2 and M4 pass (BIN row). §3.1: reserved fields belong to the dependency list (two VLEs) plus a boolean. §3.2: import flags are a VLE; flag-0 imports explained (class-wide dependencies), narrow OPEN left. §4.2: contents and order rules of the per-record class table; duplicate rule; older attribute-name form. §5.3: counts cover all records; 131 and 134 hold references (renamed); 101 removed; 72, 192 (renamed `typed_object`), 1980 (renamed `triple_table`) and 3053 (`nested_string_table`) decoded; list of reference-bearing unobserved natives. §5.4: layouts for 72, 192, 1980, 3053; 2479 filter bit 0x80. §6, §7: client checks. §8: 131 keys and 134 first fields are references; no unreferenced items remain. §9: every record decodes exactly; counts reconciled. §10: versioned bundle reader. §11.1, §11.3: index stability, region 6. §12: reference typing. §13, §14 updated; implementation OPENs answered. §15: new tests. New §16, §17. Section numbers 1–15 unchanged. |
| 2026-10-05 | Repair after review (changes requested), with a new independent whole-install decode (OBS3) and an M4 repair pass (BIN row). **§10 network bullet withdrawn** (out of scope until the `docs/LEGAL.md` §9.3 gate, as in `property-system.md` §10); row X1 narrowed to the pre-live file form; Purpose updated; the bundle-version unknown is now an OPEN (§10, §14). **Counts corrected:** items 5,832,867 and embedded objects 5,789,528 (§9, §5.1, §14); root referenced in 139 records, not 140, and unreferenced in 383,204 (§8.2, §8.3); 91,836 small sets (N4, §5.3). **Scopes fixed:** §3.1 heading and end row, §3.2 import hint (383,341), §4.2 and B2 (1,381,857 entries), §4.3 (381,901 contiguous, 383,343 roots), H1, H2, I3, §5.2 and §8.1 now count all 383,343 records. **New facts (BIN + OBS):** the small-set header is one u32 with a 24-bit count and the hint in the high byte (N4); class 18 and 105 sets are ordered by value mod *B* (105 on the whole 64-bit value); the second u32 of a 131, 134 or 138 element is its bucket number and elements are stored in ascending bucket (new row N8, new paragraph in §5.4, rows of §5.3 and §8.1 reworded; this answers which field orders a 134 set). §3.2: a speculative remark replaced by the observed fact. §12: writer and bounds notes. §13: two rows. §14: two OPENs added. §15: new tests; new §15.2 worked example. No section renumbered. |
| 2026-10-07 | OPEN-closing revision (second M4 pass; new provenance keys BIN2 and OBS4; rows H6–H8, B5, N9–N11). Status moved to Revised, waiting for review. §3.1: fields 4 and 5 are the dependency list's excluded-flags and required-flags masks, field 6 the skip-initializer switch (with `wlib.md` §3.2); OPEN closed. §3.2: the client also treats the property set as a dependency; OPEN narrowed to the writing tool. §5.3: row 1980 points at the new element layouts. §5.4: new paragraph (anchor `s5-4-1`) with the bucket hash of 131/134/138 keys per key class and the key-equality rules, so a writer can add elements; frame (72) is an offset plus a scalar-first rotation; element layouts of the three 1980 collections. §6: OPEN states what BIN2 found. §10: new paragraph (anchor `s10-1`): archive records are read as bundle version 5, in the chunk-512 mode, without the chunk-128 check; OPEN closed. §14: five items closed; three narrow OPENs left. No section renumbered. |
| 2026-10-07 | Repair after the 2026-10-07 review (changes requested), with a third M4 pass (new provenance key BIN3, new rows N12 and N13). **§5.4, class 1980:** the TSize hint check has a second failure condition (strict-check flag set and bucket count above 50,000, i.e. hint 13 or more); archive loading never sets that flag, so archive readers reject only hint 23 or more; settled, not left OPEN. §3.2: the import list's reader has the same check (new bullet). **§5.4, string keys:** the hash stops at the first zero code unit; none of the 10,025 string keys in this install contains one. §14: the third OPEN now says "milestones M3 and M4" (not research methods); one item closed. §15: tests for 1980 hints 13, 22 and 23 and for a string key containing U+0000. No section renumbered. |
