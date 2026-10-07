<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# WLib: the script class library (DID 0x56000000)

| | |
|---|---|
| **Status** | Accepted (re-review 2026-10-07 of the OPEN-closing revision and its repair, see §16; previously Accepted 2026-10-05, Revised 2026-10-07). |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review sessions (spec-writer agent, 2026-10-05; spec-reviewer, a different session from the writer, 2026-10-07) |
| **Review record** | Claude review session (spec-writer agent), review 2026-10-05: changes requested (five findings: §14 trailing-byte test contradicted §3.2/§10; network-form remarks at the end of §5 and §8.4 fell under the `docs/LEGAL.md` §9.3 gate; §9 count scope; speculation in §2; no reader rule for chunks 32 and 256). Claude review session (spec-writer agent), re-review 2026-10-05 after the repair round: **passed, Accepted**. All five findings resolved (network-form content withdrawn; §14 consistent with §3.2 and §10; W25 scoped; §2 reduced to the "Client" label; §10 reader rules for chunks 32 and 256 backed by BIN row W24). Re-review spot checks on the install with an independent decoder: header 0x1A015342 / 38 / 0x1E4A5AA4; chunk set 1, 2, 16, 64, 512, 1024 and trailing byte 1; parent index > child index in 296 of 5,822 entries and 282 defined classes (W25); 1,381,857 per-record class-table entries equal the library lists in all 383,343 object records (W11). Earlier first-round checks (L2–L6, attribute-list census, flattening, parent map, singletons, method tables, root classes) still stand. Remaining OPENs (§3.1 extra word, §3.2 boolean byte, chunks 32/256/128) are precise and non-blocking. Claude review session (spec-reviewer, a different session from the writer), review 2026-10-07 of the OPEN-closing revision: **changes requested** (small, precise fixes; nothing in the revision falls under a closed gate, no code or pseudo-code, provenance rows W26–W32 and the change log are present, no section renumbered). Re-checked in an independent copy of the decompiler project (build 4906.0071.1794.4035, address ranges only, nothing from network, login, launcher or anti-tamper code) and confirmed: signature, version 38 and the stored extra word (kept as the library's first field, written back on save); chunk dispatch (32 and 64 share the small-set reader into separate fields, 256 and 512 share the attribute-list reader into separate fields; 4, 8, 2048, 4096, 8192, 16384 skipped by size; anything else fails); the small-set header (low 24 bits count, high byte hint clamped to index 22, insertion by value mod *B*); the 23-entry bucket table with indices 13–22 as listed in §7.1; chunk 128 layout (TSize, u32 class, u32 count checked against the bytes left, triples u16/u8/i16); the trailing byte read as a separate boolean (≥ 2 fails); the library looked up through the DID-map layer with map 0xE and key 2; the skip-initializer switch (record boolean AND library byte) and the slot-0 call otherwise; the archive path's reader mode (stream flag selecting chunk 512, never 256) and the chunk-64/0x76 path being taken only outside that mode (W26, W27, W28, W30, W31). Data spot checks (independent probes, 2026-10-07): WLIB map 0x2800000D has keys 0, 2 → 0x56000000 and 3 → 0x56000011, and 0x56000011 is in no archive; header 0x1A015342 / 38 / 0x1E4A5AA4, six chunks, trailing byte 1, record ends there; chunk 64 header 0x0000000B with the 11 listed indices in bucket order mod 11; the extra word matches no CRC-32 or Adler-32 of any chunk payload, whole chunk or the record after the header, and occurs once in the record; the 11 0x76 records are exactly 0x76000000 + the chunk-64 classes. Findings: (1) §4.3 and §15 (W28): the client's TSize hint check has a second failure condition that the spec omits: when the stream's strict-check flag is set, a hint whose bucket count exceeds 50,000 (hint 13 or more) also fails; state it and add an OPEN on whether archive-record streams set that flag (the same reader shape is in `wstate.md` §5.4, class 1980). (2) §2 and W26: "neither label string occurs in the executable" is true for key 3's label but not for "Client", which occurs inside other strings and once as a stand-alone string in the version resource; re-scope the claim to "no lookup uses either label". Also say in the change log why the key-3 label is back after the 2026-10-05 repair removed it as unnecessary (or refer to the entry by key only). Claude review session (spec-reviewer, a different session from the writer), re-review 2026-10-07 after the repair round: **passed, Accepted**. All findings resolved: (1) §4.3, W33, §13, §14 and §15 now give both failure conditions of the TSize hint check (hint 23 or more always; bucket count above 50,000, i.e. hint 13 or more, only with the stream's strict-check flag) and settle, with BIN3, that the archive path never sets that flag; (2) §2 and W26 now say that no lookup uses either label and the client asks by key; (3) key 3's label is no longer reproduced anywhere in the spec, and the §16 change log says why (for the same reason this review record now refers to it as "key 3's label"). Re-checked in an independent copy of the decompiler project (build 4906.0071.1794.4035, address ranges only, nothing from network, login, launcher or anti-tamper code): the two-condition check (strict condition tested only in plain read mode) in the chunk-128 reader, the import-list reader and the three class-1980 readers; the generic archive-object loaders give their stream only the attribute-wise and chunk-512 mode bits before calling the record's or library's serializer; the object-record bundle reader registers bundle version 5 and adds only the chunk-512 mode bit; the stream's mode-setting and version-registration routines never set the strict bit; an executable-wide scan for code that sets it found only wrapper, blob pack/unpack and type-registry routines, none of them in the vtables of the object-record or library classes or on their load path (not followed further). Data spot checks (independent probes, 2026-10-07): import hints 2 in 383,341 records and 6 and 7 in one each, all 99,863 non-empty lists in bucket order; the library has no chunk 128 (chunks 1, 2, 16, 64, 512, 1024, trailing byte 1); the two class-1980 instances hold three empty collections with hint 1 each; none of the 10,025 string keys of classes 131, 134 and 138 contains a zero code unit or is empty, and all their buckets still recompute from the spec text. Remaining OPENs (§3.1 extra word; users of chunk 32 and of the chunk-256 reader mode, stopped under decision 0004) are precise and non-blocking. |
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
| BIN2 | **M4 (decision 0011), second pass.** Same build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, compiled 2026-09-27; file version checked again on 2026-10-07), read on 2026-10-07 in the dirty room with a headless decompiler and a disassembler, by address range only: the library's per-chunk readers for types 32, 64, 128 and 256; every read of the loaded library's fields through the client's pointer to it; the routine that asks the DID-map layer for the library; the object-creation and object-graph reading routines that consult the library's chunks; the post-load check. **Questions:** what the extra header word and the trailing byte do; what chunks 32, 64, 128 and 256 hold and who reads them; which WLIB map entry the client asks for and whether anything asks for the second one. **Why M1–M3 were not enough:** the library exists in one version; chunks 32, 128 and 256 do not occur, and the extra word and the trailing byte have one value each, so the data cannot show how they are used; no published source covers them (M2); the running client does not expose them (M3). Two searches were stopped where they reached code that may serve network traffic (decision 0004): the users of chunk 32 and of the reader mode that selects chunk 256 (§3.3). Nothing from network, login, launcher or anti-tamper code was read. |
| OBS4 | Observed in the same install on 2026-10-07 with throwaway probes in the dirty-room scratchpad: the WLIB DID map (0x2800000D) decoded in full; chunk 64's header bytes and element order; a search for the extra header word in the library's bytes and checksum trials (CRC-32 and Adler-32 of each chunk and of the whole record). |
| BIN3 | **M4 (decision 0011), repair pass after the 2026-10-07 review.** Same build 4906.0071.1794.4035 (`x64/lotroclient64.exe`), read on 2026-10-07 in the dirty room with a headless decompiler, by address range only: the chunk-128 reader's TSize check; the routine that reads an archive record into its object (the stream it sets up and the flags it gives that stream); the routine that reads an object record's bundle (its own stream and flags); the reading stream's mode-setting routines; and a search of the whole executable for code that sets the reading stream's strict-check flag. **Question:** when the TSize hint check fails, and whether the archive-record path enables the strict-check condition. **Why M1–M3 were not enough:** the data contain only hints the client accepted and say nothing about the reading context; no published source covers it (M2); the running client does not expose it (M3). Nothing from network, login, launcher or anti-tamper code was read; the routines found to set the flag lie outside archive loading and were not followed further. |

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
| W26 | The client finds the library by asking the DID-map layer for map 0xE (WLIB) and key **2** (label "Client"); no code asks for key 3 (DID 0x56000011, absent from every archive); no lookup uses either entry's label: the client asks by key (§2) | BIN2; OBS4 (map entries) | Confirmed (BIN + OBS) |
| W27 | Chunks 32 and 64 are **small sets** of class indices: one u32 header (low 24 bits count, high 8 bits bucket hint), then the elements in bucket order by value mod *B* (§7) | BIN2 (reader); OBS4 (chunk 64: header 0x0000000B, hint 0, 11 of 11 elements in bucket order mod 11) | Confirmed (BIN + OBS) for chunk 64; BIN only for chunk 32 |
| W28 | Element layout of chunk 128 and the conditions under which its check runs (§4.3) | BIN2 | BIN only |
| W29 | A class in chunk 64 has its new objects made from its 0x76 record, both when a script creates an object of that class and when the object table resolves a 0x76 DID (§7) | BIN2 | BIN only (no script runs in Tessera's offline client; the 0x76 records are Confirmed, OBS) |
| W30 | The trailing boolean byte, together with a boolean of each object record, decides whether script objects read from archives are created without running their class's slot-0 function (§3.2; `wstate.md` §3.1) | BIN2; OBS (the byte is 1 here, and the record boolean is 1 in all 383,343 records) | Confirmed (BIN + OBS) for the values; the effect is BIN only |
| W31 | Chunk 256 replaces chunk 512's lists only in a second reader mode that the client does not use for archive records; the archive-record reader uses chunk 512 (§3.3, §8.4) | BIN2 | BIN only |
| W32 | The extra header word is stored and never read by this build; it is not a CRC-32 or Adler-32 of a chunk or of the record and does not occur elsewhere in the library (§3.1) | BIN2; OBS4 | Confirmed (negative result) |
| W33 | The TSize hint check of chunk 128 (and of the other hashed collections that use the same check, `wstate.md` §3.2 and §5.4) has two failure conditions: a hint of 23 or more always; a hint whose bucket count exceeds 50,000 (hint 13 or more) only when the reading stream's strict-check flag is set. The archive-record path never sets that flag (§4.3) | BIN3 | BIN only (no chunk 128 occurs; the strictness of a reading context is not visible in data) |

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
| **Singleton class** | A class listed in chunk 64 (§7). It has one object-state record of its own at 0x76000000 + class index, which the client uses as the starting state of new objects of that class (W29); it may also root ordinary 0x70 records (W22). The name "singleton" is kept because existing code uses it; "prototype class" describes the client's use better. |
| **Function ID** | How native code names a script function: the declaring class index (15 bits) times 65,536, plus the function's number within that class (16 bits) (§6.4). Interop identifier; not needed to decode state. |

---

<a id="s2"></a>
## 2. Locating the library

- DID **0x56000000**, archive `client_gamelogic.dat` (routing as for every 0x56 DID,
  `dat-container.md`). It is the only 0x56 record in this install (census of all
  gamelogic entries, OBS).
- The DID map labelled **WLIB** (0x2800000D, `property-system.md` §8.1) names it. The map
  has three entries (OBS4):

  | Group | Key | Label | DID | In the archives |
  |---|---|---|---|---|
  | first | 0 | (the "undefined" sentinel every DID map has) | 0 | — |
  | first | **2** | "Client" | 0x56000000 | yes |
  | second | 3 | (not needed; see below) | 0x56000011 | **no** (dangling) |

- The client looks the library up **by key, not by label** (W26, BIN2): it asks the DID-map
  layer for map 0xE (the WLIB map's code, `property-system.md` §8.1) and key 2. If the map
  has no such key, it gets the null DID and no library is loaded. No code in this build asks
  for key 3, and no lookup uses either entry's label: the client asks by key. The "Client"
  label is given only so that a reader can recognise the entry in a map listing; nothing
  needs key 3's label, so this spec refers to that entry by its key only.
- A reader looks the library up as the client does (WLIB map, key 2) and may fall back to
  0x56000000. It ignores key 3: its record is not shipped, nothing in the client reads it,
  and the data do not say what it would hold.
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

What is known about the extra word (W32): the client keeps it as the first field of the
loaded library and writes it back unchanged when it saves one. No code in this build reads it
afterwards: every access to the loaded library through the client's pointer to it touches
other fields, and the value does not occur as a constant in the code. In the data it is not a
CRC-32 or Adler-32 of any chunk payload, of any whole chunk, or of the record after the
header, and its bytes occur nowhere else in the library. A reader keeps it for byte-exact
rewriting and gives it no meaning.

> **OPEN:** What the extra word identifies (for example a build or source stamp of the
> library). No reader exists in this client, so nothing depends on it.

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

What the boolean byte controls (W30, BIN2). The client keeps it with the library record and
uses it as a global switch for one decision made when it builds objects from object-state
records (`wstate.md` §3.1, field 6, and §10):

- Normally the client creates a script object by running the function in **slot 0** of its
  class's virtual-function table (§6.4) and then fills in the stored attribute values. If
  that slot has no body (0xFFFFFFFF, 468 classes here) the creation fails with a "missing
  function" error.
- When **both** this byte and the object record's own boolean are 1, the client creates the
  record's script objects without running slot 0; the stored values are the whole state.
- Both are 1 in this install (the byte here; the record boolean in all 383,343 records,
  `wstate.md` §3.1), so archive objects are created without running any script.

A Tessera reader that does not execute scripts needs nothing from the byte except the range
check (0 or 1).

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
| 32 | accepted-class set | parsed with the same reader as type 64 (a small set of class indices, §7.1) and kept in a set of its own, apart from chunk 64's (W24, W27). Used only by one serializer outside object-state loading (below) | no |
| 64 | singleton (prototype) classes | parsed (§7) | yes |
| 128 | reference-type constraints | parsed: a TSize-prefixed hashed collection keyed by class index holding, per class, a list of (slot, type code, required class) triples (§4.3, W18, W28) | no |
| 256 | alternative attribute lists | parsed with the same reader as type 512 (§8.1 layout) into a table of its own, apart from chunk 512's (W24). Used instead of chunk 512 only by a second reader mode that archive loading does not use (below, W31) | no |
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

Who reads chunks 32 and 256 (BIN2):

- **Chunk 256.** The client's object-graph reader has two attribute-wise modes, chosen by
  the stream it reads from. The mode used for archive object records (`wstate.md` §10) takes
  each script class's attribute list, and so the serialization order, from **chunk 512**.
  The other mode takes them from chunk 256 instead and, in addition, makes the objects of
  classes listed in chunk 64 from their 0x76 records (§7). The archive-record path never
  selects that mode.
- **Chunk 32.** One serializer outside object-state loading, which holds a script object
  together with its class index, accepts that class only if it is in chunk 32's set, and
  only when its stream is in a strict-check mode; it reports an error otherwise. No
  object-state reader consults chunk 32.

> **OPEN:** Which data the second reader mode and the chunk-32 serializer serve. Both
> searches stopped at code that may serve network traffic (decision 0004), and neither chunk
> occurs in this install. Archive readers are not affected: §10 gives the reader rule for
> chunks 32 and 256.

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

Layout (W28, BIN2):

| Field | Type | Meaning |
|---|---|---|
| class_count *n* | TSize (`records.md` §1.3) | Number of constrained classes. Hint check below the tables (W33). The client also fails the load when *n* exceeds the bytes left. |
| classes | class entry × *n* | Bucket order by class index |

Class entry:

| Field | Type | Meaning |
|---|---|---|
| class_index | u32 | The constrained class (key) |
| triple_count *t* | u32 | Checked against the bytes left before allocating |
| triples | triple × *t* | 5 bytes each |

Triple:

| Field | Type | Meaning |
|---|---|---|
| slot | u16 | Slot index in the class's instance (§6.1) |
| type | u8 | Declared value type code (§5) |
| required_class | i16 | Class index that a reference in this slot must point at (itself or a descendant) |

The minimum size of a class entry is 8 bytes plus 5 per triple.

Hint check of `class_count` (W33, BIN3). The client fails the load when either condition
holds:

1. the hint is 23 or more (beyond its 23-entry bucket-count table, §7.1);
2. the reading stream's **strict-check flag** is set, and the bucket count for the hint is
   more than 50,000, which is the case for every hint from 13 to 22 (the bucket count for
   hint 13 is 98,299, §7.1).

The second condition does not apply on the archive-record path: the routine that reads an
archive record into its object sets up its own stream with other flags, and so does the
routine that reads an object record's bundle (`wstate.md` §10); no code on either path, and
none of the stream's own mode-setting routines, sets the strict-check flag (BIN3).
The code found to set it builds streams outside archive loading. So a reader of archive data
rejects only hints of 23 or more and accepts hints 13 to 22. The same two-condition check is
used by the hashed collections of `wstate.md` §3.2 and §5.4 (class 1980).

What the check does (W18). For each script object of a loaded bundle whose class has an
entry, and for each triple:

- the slot's type must equal the declared type. An untyped number matches a declared integer
  or float of the same width: 4 matches 2 or 3, 132 matches 130 or 131, but nothing matches
  195 except 195;
- when the type is 1 (reference) and the value is not null, the referenced object's class
  must be `required_class` or one of its descendants (parent map, §9).

The load fails if a check fails. The check runs only when the reading context enables it;
the path that builds objects from archive records (`wstate.md` §10) does not (BIN2). The
chunk is absent from this install in any case, so a Tessera reader needs no check. A reader
that meets the chunk parses it with this layout (a payload that does not parse exactly is
malformed) and may ignore its content.

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
- **Slot 0** is the class's initializer: the client runs `method_offsets[0]` of class *C*
  when it creates a new object of *C* from scratch, and the creation fails when that slot is
  0xFFFFFFFF. Objects built from archive records skip it when the switch of §3.2 is on, and
  objects of classes in chunk 64 start from their 0x76 record instead (§7.2) (BIN2).

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

<a id="s7-1"></a>
### 7.1 Layout: a small set (W27, Confirmed BIN + OBS; revised 2026-10-07)

| Field | Type | Meaning | Observed |
|---|---|---|---|
| header | u32 | Low 24 bits: element count *n*. High 8 bits: bucket hint *h*. | 0x0000000B: *n* = 11, *h* = 0 |
| class indices | u32 × *n* | In bucket order: value mod *B* is non-decreasing, where *B* is the bucket count for *h* | 3432, 1783, 1629, 497, 387, 1630, 1556, 1547, 1603, 1053, 3431 |

This is the same "small set" header as object-state classes 18, 105 and 134
(`wstate.md` §5.3). Observed order: the values modulo 11 are 0, 1, 1, 2, 2, 2, 5, 7, 8, 8, 10
(11 of 11 in bucket order); within a bucket the stored order is kept. The first version of
this spec read the header as a plain u32 count, which gives the same result whenever the hint
is 0, as here.

Reader rules (BIN2):

- the count is the low 24 bits; check it against the bytes left (4 per element) before
  allocating;
- a hint beyond the client's bucket-count table is clamped to its last entry (index 22);
  the table continues `records.md` §1.3 after index 12 with 98299, 196597, 393209, 786431,
  1572853, 3145721, 6291449, 12582893, 25165813 and 50331599 (indices 13–22, BIN2). A
  reader needs only *n*;
- the client inserts each element in the bucket (value mod *B*) and keeps the stored order
  within a bucket. A byte-exact writer keeps the header and the order; a writer producing
  new data writes the elements in bucket order for the hint it chooses.

Chunk 32 has the same layout (§3.3).

<a id="s7-2"></a>
### 7.2 Meaning

Each listed class *c* is defined and has exactly one object-state record at DID
**0x76000000 + *c***, whose class index (offset 4) is *c*. Conversely, every 0x76 record in
the install (11) is such a singleton. All 11 classes descend from class 486 in the parent
map.

How the client uses the list (W29, BIN2): the 0x76 record is the **starting state of new
objects** of a listed class.

- When a script creates an object of class *c*, the client checks the list. For a listed
  class it builds the new object graph from record 0x76000000 + *c* (decoded anew each time,
  as for any archive record, `wstate.md` §10). For other classes it makes a blank instance
  from the class table's slot defaults (§6.1) and runs the class's slot-0 function (§6.4).
- When the client's object table resolves a game-object DID in the 0x76 range, it loads the
  record only if its class is listed.
- The second object-graph reader mode of §3.3 also makes the objects of listed classes from
  their 0x76 records. The archive-record mode does not.

"Singleton" describes the 0x76 record, not the class: classes 1547, 1556 and 1603 also root
ordinary 0x70 records (1, 2 and 1 records; W22).

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
repetitions (in all 383,343 records) equal this chunk's list exactly, in order. The client's
archive-record reader takes the lists from this chunk, never from chunk 256 (W31, §3.3).

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
5. Decode the singleton list (§7.1: small-set header, 24-bit count).
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
| Chunk 256 present (alternative attribute lists) | Decode it with the §8.1 layout (a payload that does not parse, or leaves bytes over, is malformed) and keep it as a separate table. Do **not** merge it into chunk 512's lists and do not let it replace them: the serialization order of archive records (§8.4) comes from chunk 512 only, as in the client's archive-record reader (W24, W31). A warning is optional. |
| Chunk 32 present (accepted-class set) | Decode it with the §7.1 layout (small-set header with a 24-bit count, then that many u32; same parse rule) and keep it as a separate set. Do not add its classes to the singleton list, which comes from chunk 64 only (W24). No object-state reader uses it (§3.3); a warning is optional. |
| Chunk 128 present | Decode it with the §4.3 layout (a payload that does not parse exactly is malformed). Running its check is optional; the client's archive-record path does not (§4.3). |
| Chunk 256, 32 or 128 present twice | Malformed (reject), as for the four chunks above. |
| Small-set hint of chunk 32 or 64 beyond the bucket-count table | Clamp it, as the client does (§7.1); not malformed. |

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
| L10 | The elements of chunk 64 are in bucket order for its header's hint (§7.1). |

A reader reports a violation of L1–L7 as malformed data. L8, L9 and L10 are cross-checks: a
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
| LC; first version of this spec | Chunk 64 is a u32 count followed by the class indices. | The first u32 is a small-set header: a 24-bit count and a bucket hint in the high byte (§7.1). The readings agree whenever the hint is 0, as in this install. |
| `combat-data.md` §13.1 | The WLIB map's second entry is, by its position, the server-side class library. | Nothing supports that reading: the entry (key 3, DID 0x56000011) is never requested by the client, its record is in no archive, and nothing in the data or the client says what it holds (§2). |

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

Closed on 2026-10-07 (BIN2, OBS4):

- *Boolean byte after the signature* — the global switch that lets archive objects be built
  without running their slot-0 initializer (§3.2, W30).
- *Element layout of chunk 128* — §4.3 (W28); the check also needs the reading context to
  enable it, which archive loading does not.
- *Which code reads chunks 32 and 256* — chunk 256 only a second reader mode that archive
  loading does not use; chunk 32 one serializer outside object-state loading (§3.3, W31).
  What those serve stays OPEN below.
- *Chunk 64 order* — bucket order of a small set (§7.1, W27); the "no evident meaning" of
  the first version is withdrawn.
- *Dangling second WLIB entry* — key 3, 0x56000011, never requested (§2, W26).

Closed on 2026-10-07 in the repair pass (BIN3):

- *Strict TSize hint check on the archive path* — the archive-record path never sets the
  strict-check flag, so only hints of 23 or more fail there (§4.3, W33).

Still open:

> **OPEN:** What the version-38 extra header word identifies (§3.1). No code in this build
> reads it; it is not needed to decode.

> **OPEN:** Which data the second object-graph reader mode (the user of chunk 256) and the
> chunk-32 serializer serve (§3.3). Both searches stopped at code that may serve network
> traffic (decision 0004). Neither chunk occurs in this install, and archive readers do not
> depend on the answer.

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
  it must load (a warning is allowed), and the attribute list (and so the serialization
  order) must still be chunk 512's; a chunk 256 whose payload is truncated must fail; a
  library with a chunk 32 listing a class must load, and the singleton list must still be
  chunk 64's alone;
- small-set headers (§7.1): a singleton chunk with header `03 00 00 01` (count 3, hint 1,
  *B* = 23) and the values 47, 5, 30 in that order (buckets 1, 5, 7) must give three
  classes; a header whose hint byte is 0xFF must be clamped, not rejected; a header claiming
  70,000 elements with too few bytes left must fail as "count larger than the remaining
  bytes" (not be read as 4,464 with the third byte ignored);
- a chunk 128 payload of 20 bytes (TSize with hint 0 and count 1; class index; triple count
  2; two 5-byte triples) must parse exactly; the same payload cut by one byte must fail;
  the same payload with hint 13 or hint 22 must parse (archive data are read without the
  strict check, §4.3); with hint 23 or 0xFF it must fail;
- method-table checks (§6.4) on the synthetic classes: a child class whose table extends its
  parent's and whose `method_indices` name the new slots.

Real-data test (skipped without `TESSERA_GAME_DIR`): load 0x56000000 and assert invariants
L1–L9. Report counts (entries, defined classes, attribute lists) in the test output; do not
assert the exact counts, which change with client updates.

---

<a id="s15"></a>
## 15. Corrections to other specs

None of the facts above contradicts a locked spec. Notes for other spec owners:

- `records.md` §1.3.1 OPEN (order within a bucket): see `wstate.md` §16 for the rule
  observed in object-state class tables.
- `records.md` §1.3 OPEN (bucket counts for hints ≥ 13): the client's table has 23 entries;
  indices 13–22 are listed in §7.1 (BIN2). In the TSize readers seen (§4.3; `wstate.md`
  §3.2 and §5.4) a hint of 23 or more fails the load, and a hint of 13 or more also fails
  when the reading stream is in strict-check mode, which archive loading never uses (W33).
  The small-set reader clamps the hint to 22 instead (§7.1).
- `combat-data.md` §13.1 describes the dangling second WLIB entry (key 3) as the server-side
  class library; nothing in the client or the data supports that reading (§2, §12).
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
| 2026-10-07 | OPEN-closing revision (second M4 pass, new provenance keys BIN2 and OBS4; rows W26–W32). Status moved to Revised, waiting for review. §1: "singleton class" also explained as prototype. §2: the WLIB map's three entries tabulated; the client asks for map 0xE, **key 2**; the dangling key 3 (0x56000011) is never requested. §3.1: what is known about the extra word (stored, never read, not a checksum); OPEN narrowed. §3.2: the trailing boolean's effect (with the record boolean of `wstate.md` §3.1, objects built from archives skip the slot-0 initializer); OPEN closed. §3.3: rows 32, 64, 128 and 256 rewritten; who reads chunks 32 and 256; OPEN narrowed to what that code serves (search stopped under decision 0004). §4.3: chunk 128 element layout and the condition under which its check runs. §6.4: slot 0 is the initializer. **§7 split into §7.1 and §7.2 (new anchors `s7-1`, `s7-2`; the `s7` anchor is unchanged): chunk 64 (and 32) is a small set whose u32 header holds a 24-bit count and a bucket hint; the stored order is bucket order; bucket counts for hints 13–22; the 0x76 records are the starting state of new objects of the listed classes.** Implementers re-check the chunk-64 reader: the earlier "u32 count" reading is only right when the hint byte is 0. §8.4: archive reader uses chunk 512 only. §10: rows for chunks 32, 128 and 256 and for the small-set hint. §11: new cross-check L10. §12: two rows. §13: five items closed, two narrow OPENs left. §14: tests for the small-set header and chunk 128. §15: notes for `records.md` §1.3 and `combat-data.md` §13.1. No section renumbered. |
| 2026-10-07 | Repair after the 2026-10-07 review (changes requested), with a third M4 pass (new provenance key BIN3, new row W33). **§4.3:** the TSize hint check of chunk 128 has a second failure condition (strict-check flag set and bucket count above 50,000, i.e. hint 13 or more); the archive-record path never sets that flag, so archive readers reject only hint 23 or more; settled, not left OPEN. §15 and §14 follow (new test for hints 13, 22, 23). **§2, W26:** the claim that neither label string occurs in the executable is withdrawn (it held for one label only); the spec now says that no lookup uses either label and the client asks by key. Key 3's label is no longer reproduced (§2, §12, §13, §15, and the previous change-log row): nothing needs it, as the 2026-10-05 repair found, and the correction to `combat-data.md` §13.1 rests on the client never requesting key 3, not on the label. No section renumbered. |
