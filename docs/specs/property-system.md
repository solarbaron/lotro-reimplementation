<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Property system: dictionary, values and property sets

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent), 2026-10-05 |
| **Review record** | Re-review 2026-10-05, Claude review session (spec-writer agent): **accepted**. Every §5.3 checklist item passes after the repair: §10 is a stub; the third-party field names are gone; BIN questions 12–19 and rows C1–C10 cover the earlier unlogged M4 facts; one TSize rule (§17.2); reader rules for StringInfo (§17.4) and for disagreeing position flags (§4.6.1). Spot checks over the whole install with independent probes matched: OBS3 (VLE forms, every TSize hint, every StringInfo first byte, `has_args` and argument table, in all 384,950 sets and in 0x34000000); the field_a, field_b, field_d and flag histograms of §2.3; the families in MP4; the code-22 bit counts of §4.4; the zero counts in §4.9 and §7.4; and the (0, 0) StringInfos of §4.8. Non-blocking notes for a later revision: the old label `timeout` is still mentioned in §2.2 and in the change log; §17.4.1 does not say what a reader does when the value header's kind differs from the entry kind. Earlier record: review 2026-10-05, Claude review session (spec-writer agent): changes requested. Repaired by the spec writer on 2026-10-05 (change log, §18); not yet re-reviewed. Earlier findings: Blocking: §10 states official network-protocol structure without the maintainer approval that `docs/LEGAL.md` §9.3 requires; third-party (MK, ACE) identifier names in §2.2 and row M3; M4 uses behind §17.1, §17.2, §17.4 and several §2.3/§4.4/§7.1 facts have no provenance row; §17.4 hint rule contradicts §5.3/§17.2. Spot checks against the install (dictionary, all property sets, colours, positions, 0x70/0x79 pairing, defaults) all matched. |
| **Companion specs** | `docs/specs/records.md`: primitives (§1: VLE, TSize and bucket order, strings), DID high-byte table (§2), StringInfo (§4), EnumMapper (§7), DataIdMap layout (§8); this spec corrects several of them in §17. `docs/specs/dat-container.md`: archives and DID routing. `docs/specs/material.md` §5.3 (the float curve, which is the waveform value of §4.2). `docs/specs/coordinates.md` §5–§7 and §11 (frames, units and rotations of position values). `docs/specs/wstate.md` §2 (the 0x70 and 0x76 object records). This spec starts with a fully reassembled, decompressed record payload. |

## Purpose

Nearly all gameplay data is stored as **properties**: (property ID, value) pairs whose value type comes from one global dictionary. This spec gives an implementer of the `formats` and `props` crates what they need to:

- decode the master property dictionary, DID 0x34000000 (§2). This content was moved here from `records.md` §6, which now only points here;
- know every property type code and its stored value encoding, including the meaning of the stored bytes where the client defines one (§3, §4);
- decode property-set records, DID prefixes 0x78–0x7F, including nested structs, arrays and null entries (§5);
- find the property set of a game object (§6);
- turn raw values into readable ones the way the client does: enum labels, bit labels, StringInfo text, property names, DID-reference types (§7);
- navigate the root DID map to the named sub-maps (§8);
- understand the DB-object-type codes that type-20 definitions carry, and the DID range of each (§9).

§10 is withdrawn: it is out of scope until the `docs/LEGAL.md` §9.3 gate. §11 reports the whole-install validation runs. §12 lists disagreements with the research notes, with other specs and with published tools. §13 collects the open questions, §14 gives test guidance and worked synthetic examples, §15–§16 answer the questions raised by the implementations, §17 records corrections to other specs, and §18 is the change log.

## Provenance

How sources are marked:

- **Confirmed**: decoded on the user's own install with a throwaway probe in the dirty-room scratchpad (2026-10-04, re-run 2026-10-05). Each Confirmed fact says what was decoded and how much of it.
- **BIN**: read from the client executable under method M4 (row BIN below). A fact marked BIN alone rests only on the executable: no record in this install exercises it.
- **Reported**: comes only from published third-party material and was not, or could not be, checked against data.

Field names in this spec (`pid_echo`, `field_a` and so on) are this spec's own labels. They are not taken from any third-party source.

Source abbreviations:

| Key | Source |
|---|---|
| OBS | Observed in the user's install, 2026-10-04 (Steam install, client data current on that date). Every 0x78 and 0x79 record in `client_gamelogic.dat` and `client_local_English.dat` was decoded in full (384,950 records, 47,680,867 property values), as were the 0x34000000 record, every 0x23 enum mapper and every 0x28 DID map. A census found no 0x7A–0x7F record in any archive. |
| OBS2 | Observed in the same install on 2026-10-05 (data files unchanged since 2026-10-04). All 384,950 property-set records re-decoded (zero failures, zero trailing bytes); every colour value (1,893), every position value (6,115), every dictionary definition (27,546, including the order of all 3,783 child lists), every enum mapper (735) and every DID map (24) checked against the BIN facts below. |
| OBS3 | Observed in the same install on 2026-10-05 by the repair session, with a separate probe: every VLE, every TSize header and every StringInfo in all 384,950 property-set records and in the whole 0x34000000 record classified by form. Property sets: 4,494,108 one-byte VLEs, 38,278 two-byte VLEs, no four-byte VLE; TSize hints 0–6 only; all 1,110,502 StringInfos have a first byte of 0 (1,045,006) or 1 (65,496), `has_args` = 0, and an argument table with hint 1 and count 0. Dictionary: 55,131 one-byte VLEs and 2 four-byte VLEs (the name count and the definition count, both with first byte 0xC0); TSize hints 0, 1, 11 and 12 only; its one StringInfo has the same shape. No VLE anywhere starts with a byte of 0xE0 or more. |
| OBS-R | `records.md` observations of 2026-10-03, re-run unchanged on 2026-10-04 for the dictionary. |
| BIN | **M4, standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md).** The client executable of the user's install (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035, compiled 2026-09-27), read with a disassembler (objdump, address ranges only) on 2026-10-05. **Questions:** (1) what the reserved bytes and the separator of 0x34000000 are; (2) the roles of the definition fields and flag bits; (3) the waveform subtypes; (4) the timestamp and tri-state values; (5) the position flag bits 0x40 and 0x80 and the quaternion order; (6) the colour channel order; (7) how a struct with 128 or more fields is stored; (8) null entries, duplicate PIDs and count limits; (9) how the client finds a game object's property set and which DID ranges hold property sets; (10) the DID ranges of DB-object-type codes that no value exercises; (11) how enum and bit labels are resolved. **Why M1–M3 were not enough:** these are reader and writer rules for cases that never occur in the data (no null entry, no struct with 128 fields, no stored timestamp or tri-state value, no waveform subtype 0 or 10, no 0x77 object), or meanings that byte statistics cannot settle (channel order, flag meanings). No published source states them (M2), and no log or capture shows them (M3). **Second use, 2026-10-05 (repair session, same build, same method):** (12) which VLE forms the stored formats use, whether a 0xE0 escape exists, and which form the writer picks (§17.1); (13) the bucket counts for hints 13–22, the order of entries within a bucket when reading, and the order in which the writer emits entries (§17.2); (14) how a StringInfo's argument table is framed, whether `has_args` and the first byte are checked, which argument kinds exist and how each is stored (§17.4); (15) what the client uses field_d byte 2 for (§2.3); (16) whether the code-22 reader bounds the bit count before reading (§4.4); (17) how the client formats enum and property-ID values in its debug display (§7.1); (18) which tri-state value each of the client's text keywords gives (§4.3); (19) what the stored-form position reader does when bits 0x40 and 0x80 disagree with the payload (§4.6). **Why M1–M3 were not enough for (12)–(19):** the data never exercises these paths (no four-byte VLE in a property set, no hint above 12, no StringInfo with arguments, no malformed bitfield or position), or the question is about behaviour rather than bytes (field_d byte 2, display, text parsing). LC disagrees with the client on (12) and (14), so M2 could not settle them, and no log or capture shows them (M3). Branches of these routines that serve forms other than the stored form were skipped, and nothing from them is stated here (decision 0004). Listings and scripts stay in the dirty scratch area. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.x, Apache-2.0 per its distribution's LICENSE. Studied as a binary, decompiled for facts only. |
| MK | Mirarkitty `lotro_extractor` (Apache-2.0) and `lotro_mesh_viewer` (MIT). Their property-set reader is a port of LC's. Facts only. |
| DS | `research/notes/data-semantics.md` §4. |
| ER | `research/notes/engine-client-re.md` §8–§9 (client memory and cache observations). |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| M1 | 0x34000000 record layout (§2.1) | LC, MK, DS; OBS-R (2,534,594 bytes, decoded to the last byte) | Confirmed |
| M2 | Definition layout (§2.2) | LC, MK; OBS-R (27,546 definitions) | Confirmed |
| M3 | *(withdrawn 2026-10-05: third-party field names removed; §2.2 uses this spec's own labels)* | | |
| M4 | A type-20 definition's `data` field is a DB-object-type code (§9) | OBS (7,390,369 DID values checked against the code); ER (cache type IDs match the same numbering); BIN (the client's type registry, §9) | Confirmed. This corrects `records.md` MP5. |
| M5 | The 8 bytes after the dictionary's DID and the 2-byte separator are the own DID, base DID and empty display list of an **embedded enum mapper** that holds the name list (§2.1) | BIN; OBS2 (all three are zero; the separator decodes as an empty TSize) | Confirmed |
| M6 | Definition tail: four metadata bytes, then the child list as a **TSize collection** whose hint is the byte formerly counted as "field_d byte 4" (§2.2) | BIN; OBS2 (hint 1 in all 27,546 definitions; all 3,783 child lists in bucket order modulo 23) | Confirmed |
| M7 | Default, minimum and maximum are read for **every** type whose flag bit is set; there is no exemption for types 10–12 (§2.2) | BIN | BIN (no definition of types 10–12 has those bits) |
| M8 | Flag 0x10 marks an absolute timestamp (§2.3, §4.3) | BIN; OBS2 (all 13 definitions with 0x10 have type 4) | Confirmed |
| M9 | The value after the optional default/min/max is an f32 field (always 1.5), and the last u32 is a field (always 0); the client checks neither (§2.2) | BIN; OBS | Confirmed |
| T1 | Type codes 1–22 (§3) | LC, DS, ER; OBS; BIN (the client's value reader handles 1–22 and rejects 0) | Confirmed |
| V1 | Value encodings of codes 1, 6–16, 18–22 (§4) | LC, MK; OBS (values in sets and in dictionary defaults); BIN | Confirmed |
| V2 | Codes 2 and 3 (subtypes 1 and 3) | LC; OBS (14 and 129 values in sets; 44 waveform defaults); BIN | Confirmed |
| V3 | Codes 4 and 5 | LC; OBS (dictionary defaults only, 4 each); BIN | Confirmed in defaults; no value occurs in any set |
| V4 | Position (code 17) layout (§4.6) | LC, MK; OBS (6,115 values, all consumed exactly, all fields plausible); BIN | Confirmed. Flag 0x04 (instance) is BIN only, because it never occurs. |
| V5 | Struct value header is a TSize (hint byte, then VLE count) and fields are stored in bucket order (§4.10) | OBS (2,799,313 structs); BIN (the struct is read by the client's generic TSize collection reader) | Confirmed. Counts ≥ 128 are BIN only. |
| V6 | Array value: u32 count, then elements with a single PID (§4.11) | LC, MK, DS; OBS (1,386,117 arrays); BIN | Confirmed |
| V7 | Bit *i* of a bitfield is labelled by enum key *i* + 1 (§7.3) | LC; OBS (1,062,289 set bits, all labelled); BIN (two label routines walk the bits this way) | Confirmed |
| V8 | Waveform = the float curve of `material.md` §5.3: subtype 1 has one float; **every other subtype**, 0 included, has ten; subtype 10 adds period, loop byte and pairs; subtypes outside 1–10 evaluate to the base term (§4.2) | BIN; OBS (subtypes 1 and 3 only) | BIN for subtypes 0, 2 and 4–10; Confirmed for 1 and 3 |
| V9 | Timestamp: f64; the stored value f64 3.4028234663852886 × 10³⁸ means "no time"; other stored values are relative to the game clock unless the definition has flag 0x10 (§4.3) | BIN; OBS2 (all 4 stored timestamps are the "no time" value) | Confirmed for "no time"; the relative rule is BIN only |
| V10 | Tri-state: 0 = false, 1 = true, 255 = undefined (§4.3) | BIN; OBS2 (all 4 stored tri-states are 255) | Confirmed |
| V11 | Position flags are a function of the values: 0x40 exactly when region = 1, 0x80 exactly when the cell is 0, and each payload bit set exactly when its field differs from its "absent" value (§4.6) | BIN; OBS2 (6,115 of 6,115 values follow every rule) | Confirmed |
| V12 | Position orientation is stored (w, x, y, z) (§4.6) | BIN (the writer compares it with the identity (1, 0, 0, 0)); `coordinates.md` §11.1; OBS2 (2,614 of 2,641 orientations have zero second and third components) | Confirmed |
| V13 | Colour (code 16) is a u32 0xAARRGGBB stored little-endian: bytes **B, G, R, A** (§4.7) | BIN (the shared colour reader); OBS2 (byte 3 = 0xFF in all 1,893 set values; byte 2 ≥ byte 1 ≥ byte 0 in all 149 skin colours and all 234 hair colours) | Confirmed |
| V14 | Boolean bytes other than 0 and 1 make the client's reader fail (§4.5) | BIN; OBS | Confirmed |
| S1 | Property-set record layout (§5) | LC, MK, DS; OBS (384,950 of 384,950 records exact, zero trailing bytes) | Confirmed |
| S2 | Top-level properties are in bucket order (§5.4) | OBS (384,950 records); BIN (the writer emits bucket by bucket) | Confirmed |
| S3 | Null entry: second u32 (the PID) = 0, no value; 8 bytes in the table form, 4 in the list form; a null list element keeps its index (§5.2) | BIN; LC (list form only) | BIN. Never observed. |
| S4 | Duplicate PIDs are not rejected by the client; a lookup finds the first stored entry (§5.5) | BIN; OBS (no duplicates) | BIN |
| S5 | Count limits: every count ≤ remaining bytes; TSize hint ≤ 22 (§5.3) | BIN | BIN |
| R1 | Object property set at DID + 0x09000000 (§6.2) | LC, DS; OBS (the serial sets of 0x70 and 0x79 are identical, 383,332 each) | Confirmed |
| R2 | The client computes the property-set DID as 0x78000000 + 0x01000000 + (object DID with its top four bits cleared), only for DIDs of the game-object type (§6.2) | BIN; OBS | Confirmed for 0x70; BIN for the rest |
| R3 | Property-set records own the whole range 0x78000000–0x7FFFFFFF; game objects own 0x70000000–0x77FFFFFF (§6.1) | BIN | BIN for 0x7A–0x7F and 0x77 (no records) |
| D1 | Root DID map labels and sub-maps (§8) | OBS (all 24 DID maps) | Confirmed |
| D2 | DID range of every DB-object-type code that a type-20 definition declares (§9) | BIN; OBS (the 15 exercised codes) | Confirmed for 15 codes; BIN for 14 |
| E1 | An enum value is resolved by the DB type of the DID in the definition's `data`: enum mapper → name along the base chain; DID map → the map's label for the key (§7.1) | BIN; OBS (19 type-8 definitions name DID maps) | Confirmed |
| E2 | Names and display texts are looked up independently along an enum mapper's base chain (§7.1) | BIN | BIN |
| E3 | Bitfield display: an all-zero mask shows the name of key 0 when the mapper has one; unlabelled set bits are left out and the result is marked incomplete (§7.3) | BIN; OBS2 (372 of 385 bitfield definitions have a key-0 name in their mapper chain) | Confirmed |
| N1 | *(withdrawn 2026-10-05; §10 is out of scope until the `docs/LEGAL.md` §9.3 gate)* | | |
| C1 | The VLE of the stored formats has no 0xE0 escape: a first byte of 0xC0–0xFF starts the 4-byte form. The writer uses the 1-byte form up to 0x7F, the 2-byte form up to 0x3FFF and the 4-byte form above; values of 0x40000000 or more cannot be represented (§17.1). The same routine reads TSize counts, Pascal and UTF-16 string lengths and code-22 bit counts. | BIN (question 12); OBS3 (no VLE starts with 0xE0 or more; the only 4-byte VLEs start with 0xC0) | Confirmed for the forms that occur; BIN for the writer limits |
| C2 | The bucket-count table has 23 entries; the counts for hints 13–22 (§17.2) | BIN (question 13) | BIN (no hint above 12 occurs, OBS3) |
| C3 | Reading keeps the stored order of entries that share a bucket; writing emits the buckets in ascending order and each bucket's entries in that order; a hint of 23 or more, or a count above the remaining bytes, makes the reader fail (§17.2) | BIN (question 13); OBS (PS-O1, PS-O2, MP7) | Confirmed for bucket order; BIN for the order within a bucket and for the failures |
| C4 | StringInfo: first byte 1 means literal and any other byte a reference; `has_args` other than 0 or 1 makes the reader fail; the argument table follows in both cases (§17.4) | BIN (question 14); OBS3 (all 1,110,503 StringInfos have first byte 0 or 1, `has_args` 0 and the empty table `01 00`) | Confirmed for the values that occur; BIN for other bytes |
| C5 | Argument kinds 1–7, with 6 and 7 stored like 4 and 5; the stored layout of each kind; any other kind carries no value bytes (§17.4) | BIN (question 14) | BIN (no StringInfo in this install has an argument) |
| C6 | field_d byte 2 is read by the client's routines that change or remove a property of a live object, and some of its values make those routines refuse (§2.3) | BIN (question 15) | BIN |
| C7 | The code-22 reader sizes its storage from the bit count before it reads any bit bytes, without comparing the count with the remaining bytes (§4.4) | BIN (question 16) | BIN |
| C8 | The client's debug display of enum and property-ID values (§7.1) | BIN (question 17) | BIN |
| C9 | The client's text parser maps its three tri-state keywords to 1, 0 and 255 (§4.3) | BIN (question 18) | BIN |
| C10 | The stored-form position reader takes the region and the cell only from the payload bits 0x01 and 0x08, and ignores 0x40 and 0x80 (§4.6) | BIN (question 19); OBS2 (every stored position agrees) | Confirmed for agreeing input; BIN for disagreeing input, which never occurs |

---

<a id="s1"></a>
## 1. Concepts

| Term | Meaning |
|---|---|
| **PID** | Property ID, a `u32`. Two families occur: values below 0x10000000, and 0x1000xxxx (bit 28 set). PID 0 means "invalid" or "no property". |
| **Definition** | The dictionary entry for a PID: type code, a type-dependent `data` reference, flags, optional default/min/max, metadata and a child list (§2.2). |
| **Value** | The stored encoding of one property, chosen by the definition's type code (§4). A reader cannot skip a value without the dictionary. |
| **Property set** | A collection of (PID, value) pairs, with each PID at most once (§5). |
| **Property-set record** | A record whose whole payload is one property set: DID prefixes 0x78–0x7F (§6). |
| **Embedded property set** | The same encoding appearing inside another record type (landblock info, UI layouts, entity descriptors, and others). Each of those container specs points here. |
| **Table form / list form** | The two ways a set of properties is stored (§5.2). The table form is a TSize collection (`records.md` §1.3) whose entries are a u32 key followed by a property; it is used for top-level sets and struct values. The list form is a count followed by properties without keys; it is used for array values. |

Decoding is **dictionary-driven**. A client loads 0x34000000 once at start-up. After that, every property set in every record is decoded by one generic routine that looks up each PID's type code. Nothing is hard-coded per record type.

---

<a id="s2"></a>
## 2. Master property dictionary (DID 0x34000000, gamelogic)

This record lists every PID with its name, value type and constraints. In this install it is stored **uncompressed** and is 2,534,594 bytes long. (Content moved from `records.md` §6.)

<a id="s2-1"></a>
### 2.1 Record layout (Confirmed)

The record is the dictionary's own DID, then an **enum mapper** (`records.md` §7.1, embedded without change) that maps each PID to its name, then the table of definitions.

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | did | DID | 0x34000000 |
| 2 | names.did | DID | Own DID of the embedded name mapper. Observed 0. |
| 3 | names.base | DID | Base mapper of the embedded name mapper. Observed 0. |
| 4 | names.name_count *n* | TSize | Observed *n* = 27,547, hint 11 |
| 5 | names.names | (u32 pid, Pascal string name) × *n* | PID → name, in bucket order (`records.md` §1.3.1) |
| 6 | names.display_count | TSize | The mapper's display-text list. Observed empty: hint 0, count 0 (the bytes `00 00`). |
| 7 | names.displays | (u32 pid, StringInfo) × count | None observed |
| 8 | def_count *d* | TSize | Observed *d* = 27,546, hint 12 |
| 9 | definitions | (u32 pid, definition) × *d* | §2.2, in bucket order by pid |
| 10 | *(end)* | | Observed: zero leftover bytes |

Rows 2–7 were previously described as "8 reserved bytes, names, 2-byte separator". The bytes are the same; the client reads them with its enum-mapper reader (BIN), so a reader may reuse its `records.md` §7.1 decoder for rows 2–7. A non-zero embedded DID or base, or a non-empty display list, never occurs; a reader accepts them and may warn.

Names are internal identifiers in Latin-1. They are not localized and are not shown to players.

<a id="s2-2"></a>
### 2.2 Definition layout (Confirmed)

Each definition follows its u32 table key (the pid of row 9 above). The field labels are this spec's own; the meanings of the metadata fields are OPEN (§2.3).

| Field | Size | Type | Meaning |
|---|---|---|---|
| pid_echo | 4 | u32 | Equals the table key. The client stores it as the definition's own PID. |
| type | 4 | u32 | Property type code 1–22 (§3) |
| field_a | 4 | u32 | Metadata (§2.3) |
| field_b | 4 | u32 | Metadata (§2.3). A newly created definition starts with 4 (BIN). |
| data | 4 | u32 | Type-dependent reference (§3, MP5), else 0 |
| field_c | 4 | u32 | Metadata: 0, or 16 (19 definitions) |
| flags | 4 | u32 | Bit 0x800: default present. Bit 0x1000: minimum present. Bit 0x2000: maximum present. Bit 0x10: absolute timestamp (§4.3). Other bits in §2.3. Bits 16–31 were always 0. |
| default | var | value | Present if flags & 0x800 |
| minimum | var | value | Present if flags & 0x1000 |
| maximum | var | value | Present if flags & 0x2000 |
| field_e | 4 | f32 | Metadata: 1.5 in every definition (bit pattern 0x3FC00000); a newly created definition also starts with 1.5 (BIN). Called `timeout` in drafts before the 2026-10-05 repair. |
| field_d | 4 | 4 × u8 | Metadata bytes 0–3 (§2.3). Most common: `02 02 04 00`, which is also what a newly created definition starts with (BIN). |
| children | 1 + VLE + 8·*c* | TSize collection | The child list: hint byte, VLE count *c*, then *c* entries of (u32 key, u32 value), both the child PID, in bucket order of the hint (`records.md` §1.3.1). Observed: hint 1 in every definition, *c* ≤ 95. |
| subset_count *s* | 4 | u32 | Non-zero in 8 definitions, all of type 11 |
| subset | 4·*s* | u32 × *s* | PIDs, each also in `children`. Meaning OPEN (§2.3). |
| last | 4 | u32 | A field; 0 in every definition |

Default, minimum and maximum are encoded per §4, using the definition's own type. The client reads them **for every type** whose bit is set (BIN). LC skips them for types 10, 11 and 12; that exemption is wrong for this client, and readers must not apply it. (No definition of types 10–12 has any of the three bits in this install, so both readings give the same result today.)

Readers:

- report a `pid_echo` that differs from its table key as malformed (the client does not compare them, but no definition differs);
- accept any `field_e` and any `last` value (the client checks neither) and may warn;
- if the key and the value of a child entry ever differ, keep the key and report the record as suspicious (this never happens in observed data);
- bound the child count and the subset count by the remaining bytes (8 and 4 bytes per entry), as the client bounds every count by the remaining bytes (§5.3).

The children's TSize hint has the meaning of `records.md` §1.3; its 2026-10-04 description as "field_d byte 4, always 1" referred to the same byte.

<a id="s2-3"></a>
### 2.3 Field statistics and roles (Confirmed counts, all 27,546 definitions)

| Field | Observed values (count) |
|---|---|
| flags bits | 0x800 (12,435), 0x8 (4,424), 0x200 (300), 0x1 (268), 0x1000 (215), 0x20 (192), 0x2000 (128), 0x40 (83), 0x400 (77), 0x8000 (31), 0x10 (13), 0x80 (5), 0x4000 (5), 0x2 (4), 0x4 (3) |
| field_a | 353 distinct values. 25,914 definitions hold a number that is also a defined PID; the other 1,632 hold one of 21 numbers that are not (0 in 75 of them). One value serves up to 3,889 definitions of unrelated kinds, so the match with PIDs looks numeric only. |
| field_b | 4 (23,331), 3 (3,504), 6 (390), 7 (303), 9 (16), 8 (2) |
| field_c | 0 (27,527), 16 (19, all of type 20) |
| field_d byte 0 | 2 (27,336), 0 (123), 1 (87) |
| field_d byte 1 | 2 (25,404), 1 (1,636), 0 (506) |
| field_d byte 2 | 4 (15,835), 3 (7,115), 6 (2,330), 2 (884), 8 (727), 5 (337), 1 (264), 0 (42), 7 (12) |
| field_d byte 3 | 0 (27,528), 1 (18) |
| children hint | 1 (all) |
| Definitions with children, by type | 12 (2,664), 11 (1,106), 21 (7), 9 (4), 20 (2) |

Roles known from the client (BIN):

- **0x800, 0x1000, 0x2000** select the optional default, minimum and maximum (§2.2). They are the only bits that change the byte layout.
- **0x10** occurs only on timestamp definitions (13 of 13, OBS2) and makes the timestamp absolute (§4.3).
- **field_d byte 2** (values 0–8) is read by the client's routines that change or remove a property of a live game object, and for some values and some objects those routines refuse the operation and log a warning (C6). It plays no part in reading, writing or displaying values. An offline client has no use for it; what each value means is OPEN below.
- The client copies all other fields into memory and never uses them while reading or displaying values.

Nothing in decoding depends on the remaining fields. A reader keeps them, so that a writer can reproduce a record byte for byte.

> **OPEN (non-blocking):** What field_a, field_b, field_c (16 on 19 DID-reference definitions), field_d bytes 0–3, flag bits 0x1, 0x2, 0x4, 0x8, 0x20, 0x40, 0x80, 0x200, 0x400, 0x4000 and 0x8000, `field_e` and the subset list mean. The EMAPPER DID map names four enum mappers whose targets are **not present** in this install ("PropertyGroup", "PropertyProvider", "PatchFlags", "PropertyType", §8.3); whether any of them labels one of these fields is not known. The client's readers and value display never use these fields (BIN). The one use that was traced (field_d byte 2, C6) concerns which side, client or server, may change a property; following it further leads into server-side property management, where M4 is excluded (decisions 0004 and 0011). None of them affects decoding.

<a id="s2-4"></a>
### 2.4 Children

- **Type 12 (array).** Usually one child, the element property (2,630 array definitions). 45 array definitions have no children, and 34 have 2–11.
- **Type 11 (struct).** The children are the possible fields. 49 of the 1,155 struct definitions have no children.
- **A definition with no children is "open".** Any PID may appear as a field or element. Every one of the 1,218,341 struct fields and 821,097 array elements in observed data that is not listed as a child belongs to an open definition. No closed definition ever contains an unlisted PID (§5.6).

<a id="s2-5"></a>
### 2.5 Invariants (Confirmed)

- **MP1**: the record is consumed exactly.
- **MP2**: every definition's `pid_echo` equals its key, `field_e` = 1.5 (bit pattern 0x3FC00000), and `last` = 0. Only the first is a reader rule (§2.2).
- **MP3**: every PID with a definition also has a name. Exactly one named PID has no definition: PID 0, whose name means "invalid".
- **MP4**: PIDs fall in two families: < 0x10000000 (2,402 names) and 0x1000xxxx (25,145 names).
- **MP5** (corrected; it replaces `records.md` MP5): the `data` field depends on the type:
  - type 8: high byte 0x23 (enum mapper) or 0x28 (DID map);
  - types 14, 18, 22: high byte 0x23;
  - type 10: 0x34000000;
  - type 20: a **DB-object-type code** below 0x100, non-zero in all 1,849 type-20 definitions (§9);
  - every other type: 0.
- **MP6**: every subset PID also appears in `children`.
- **MP7**: the name list, the definition table and every child list are in bucket order of their own hint (OBS2: 3,783 of 3,783 child lists).

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

LC also lists a different code numbering for another game on the same engine. That numbering is out of scope. The client's value reader fails on type code 0 and handles exactly the codes 1–22 (BIN). Readers report a type code outside 1–22 as malformed.

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
| 3 | Waveform (§4.2) | Confirmed for subtypes 1 and 3; BIN for the others |
| 4 | f64 timestamp (§4.3) | Confirmed in defaults only (4) |
| 5 | u8 tri-state: 0, 1 or 255 (§4.3) | Confirmed in defaults only (4) |
| 6 | 3 × f32 (x, y, z) | Confirmed |
| 7 | u64 | Confirmed |
| 8 | u32 enum key (§7.1) | Confirmed |
| 9 | f32. No NaN or infinity among 13,969,701 set values. | Confirmed |
| 10 | u32 PID (§7.4) | Confirmed (set values only) |
| 11 | Struct: TSize header and fields in the table form (§4.10) | Confirmed (set values only) |
| 12 | Array: u32 count and elements in the list form (§4.11) | Confirmed (set values only) |
| 13 | StringInfo (`records.md` §4; §4.8 here) | Confirmed |
| 14 | u64 bit mask (§7.3) | Confirmed |
| 15 | u32, interpreted as signed int32 | Confirmed |
| 16 | u32 0xAARRGGBB, little-endian: bytes B, G, R, A (§4.7) | Confirmed |
| 17 | Position (§4.6) | Confirmed (set values only) |
| 18 | u32 bit mask (§7.3) | Confirmed |
| 19 | u64 | Confirmed |
| 20 | u32 DID; 0 means "none" (§4.9) | Confirmed |
| 21 | u8: 0 = false, 1 = true; anything else is malformed (§4.5) | Confirmed |
| 22 | Variable-width bitfield (§4.4) | Confirmed |

<a id="s4-2"></a>
### 4.2 Waveform (code 3)

A waveform value is the **float curve** of `material.md` §5.3: the client reads property waveforms, material curves and day-description curves with one routine (BIN). `material.md` §5.3.1 specifies how a curve is evaluated over time.

| Field | Type | Present when |
|---|---|---|
| subtype | u32 | always |
| v0 | f32 | subtype = 1 |
| v0…v9 | 10 × f32 | subtype ≠ 1 (0 included) |
| period | f32 | subtype = 10 |
| loop | u8 | subtype = 10. Must be 0 or 1; the client's reader fails on any other value. |
| *k* | u32 | subtype = 10. The client's reader fails if *k* exceeds the remaining bytes. |
| pairs | *k* × (f32, f32) | subtype = 10. Order per `material.md` §5.3. |

Facts (BIN unless marked):

- The reader accepts every subtype value. Subtype 1 carries one float; **every other subtype carries ten floats**. That includes subtype 0, which LC describes as having no floats (corrected, X11), and subtypes above 10.
- Observed (Confirmed): subtype 1 in 41 defaults and all 129 set values; subtype 3 (ten floats) in 3 defaults.
- The meanings of the subtypes are the curve modes of `material.md` §5.3.1 (1 constant, 2 linear ramp, 3 noise, 4 sine, 5 square, 6 rectified sine, 7 smooth noise, 8 fractal noise, 9 staircase, 10 keyframed).
- Subtypes 0 and above 10 do not occur in data. The client's evaluator returns the base term *B* = v0 + v1 · *t* of `material.md` §5.3.1 for them (after the same 3,600-second wrap of *t*), and Tessera does the same.

LC notes that an older client version always stored ten floats. That variant is out of scope.

<a id="s4-3"></a>
### 4.3 Fixed-width scalars, timestamps and tri-states

Codes 2, 7, 8, 10, 14, 15, 18, 19 and 20 are plain little-endian integers, of 4 bytes or 8 bytes as listed in §4.1.

**Timestamp (code 4).** An f64 on the client's game clock. Stored values follow these rules (BIN):

- The stored f64 3.4028234663852886 × 10³⁸ (bytes `00 00 00 E0 FF FF EF 47`, the largest f32 widened to f64) means **"no time"**. Writers store "no time" this way.
- If the definition has flag 0x10, any other stored value is the time itself.
- Otherwise a stored value is the time minus the game clock at the moment of writing, and a reader adds its own game clock.

All 4 timestamps stored in this install (dictionary defaults) are "no time" (OBS2), and no property set holds a timestamp. Tessera decision: decode the f64, map the "no time" value to "no time", and keep any other value as stored, together with the absolute flag.

> **OPEN (non-blocking, gated):** The epoch and unit of the game clock that relative timestamps are measured against. The client keeps that clock in step with the official server, so finding out falls under the official-server gate (decision 0004, `docs/LEGAL.md` §9.3). No stored value in this install needs it, and an offline client can use its own clock.

**Tri-state (code 5).** One byte: 0 = false, 1 = true, 255 = undefined (C9: the client's text parser turns its three keywords into exactly 1, 0 and 255; the keywords themselves are not needed and are not given here). All 4 stored tri-states (dictionary defaults) are 255 (OBS2). The client's binary reader stores any byte without checking it (BIN). Tessera keeps any other byte as its number and may warn; it is not malformed.

<a id="s4-4"></a>
### 4.4 Variable-width bitfield (code 22)

| Field | Type | Meaning |
|---|---|---|
| bit_count *b* | VLE | Number of bits |
| bytes | ⌈*b*/8⌉ × u8 | Bit *i* is bit (*i* mod 8) of byte ⌊*i*/8⌋, least significant bit first |

Observed *b* ranges from 0 to 957, and 52 is the most common. In all 12,446 sampled values where *b* is not a multiple of 8, the unused high bits of the last byte were zero. Writers must zero them. Readers ignore them; the client keeps them as read (BIN). The client's reader sizes its bit storage from *b* before it reads any bit byte and never compares *b* with the remaining bytes (C7), so a hostile *b* makes it allocate first and fail later. Tessera instead checks ⌈*b*/8⌉ against the remaining bytes before it allocates, and reports a larger value as malformed. *b* is a VLE (§17.1), so it is below 0x40000000.

<a id="s4-5"></a>
### 4.5 Boolean (code 21)

One byte. All 6,258,467 set values and all 2,003 boolean default/minimum/maximum values in the dictionary were 0 or 1 (3,618,608 false, 2,639,859 true). The client's reader fails on any other byte (BIN), so readers report it as malformed, in line with `bool8` in `records.md` §1.1. This settles the `records.md` §13 question.

<a id="s4-6"></a>
### 4.6 Position (code 17)

A position begins with a flag byte. Each payload bit adds a field, and fields appear **in ascending bit order**:

| Flag bit | Field | Type | Writer sets the bit exactly when (BIN) | Observed in (of 6,115) |
|---|---|---|---|---|
| 0x01 | region | u8 | region ≠ 0 | 6,102 |
| 0x02 | landblock X, landblock Y | u8, u8 | X ≠ 0xFF and Y ≠ 0xFF | 6,102 |
| 0x04 | instance | u16 | instance ≠ 0 | 0 (BIN only) |
| 0x08 | cell | u16 | cell ≠ 0xFFFF | 5,435 |
| 0x10 | x, y, z | 3 × f32 | (x, y, z) ≠ (0, 0, 0) | 5,445 |
| 0x20 | orientation (w, x, y, z) | 4 × f32 | orientation ≠ (1, 0, 0, 0) | 2,641 |
| 0x40 | *(no payload)* | | region = 1 | 2,108 |
| 0x80 | *(no payload)* | | cell = 0 (outdoors) | 5,266 |

Facts:

- **Bits 0x40 and 0x80 carry no payload in the stored form** (BIN; Confirmed: every position decodes exactly). The client's reader reads the region whenever 0x01 is set and the cell whenever 0x08 is set, whatever 0x40 and 0x80 say. Bit 0x40 means "region 1" and bit 0x80 means "cell 0, outdoors".
- **Every flag is a function of the values** (V11). All 6,115 stored positions follow every rule in the table (OBS2): a present region is never 0, a present landblock never has 0xFF, a present cell is never 0xFFFF, 0x40 is set exactly when the region is 1, 0x80 exactly when the cell is 0, a present (x, y, z) is never all zero, and a present orientation is never the identity.
- **Absent fields have the values that make the writer leave them out:** region 0 ("none"), landblock (0xFF, 0xFF) ("none"), instance 0, cell 0xFFFF ("none"), position (0, 0, 0), orientation identity. A flag byte of 0 is therefore a position with all of these (LC; never observed). A new position value in the client starts with exactly these values, and its reader leaves a field untouched when the field's bit is clear (BIN), which is why §4.6.1 gives absent fields these values.
- **Orientation order** is (w, x, y, z), scalar first (BIN: the writer compares with the identity (1, 0, 0, 0); Confirmed: 2,614 of 2,641 orientations have zero second and third components, the pattern of a turn about the vertical axis with the scalar first). Every orientation has unit length (2,641 of 2,641 within 0.001). The rotation convention is that of `coordinates.md` §11.
- **Frames and units** are those of `coordinates.md` §5–§7: with a landblock, (x, y, z) is block-local (x and y within 0–160 outdoors, z an absolute height); with a non-zero cell, the cell is an interior cell of that landblock and x, y may leave 0–160 (140 of 169 such values do). Positions with no landblock (flags 0x10 or 0x30, 13 values) hold small relative offsets, which can be negative.
- Observed regions: 1, 2, 3, 4, 5 and 14. These match the landblock regions in `records.md` §2.2. Landblock X and Y match that section's 0–254 range.

Writers compute the flag byte from the values with the rules in the table, including bits 0x40 and 0x80.

<a id="s4-6-1"></a>
#### 4.6.1 Reader rule for disagreeing flags

A reader (C10) takes every value **only from the payload bits** 0x01–0x20 and never from 0x40 or 0x80, as the client's stored-form reader does:

- 0x40 set with 0x01 clear: the region is absent, so it is 0 ("none"), not 1.
- 0x40 clear with 0x01 set and a region byte of 1: the region is 1.
- 0x80 set with 0x08 set and a non-zero cell: the cell is the stored value.
- 0x80 set with 0x08 clear: the cell is absent, so it is 0xFFFF ("none"), not 0.
- 0x80 clear with 0x08 set and a stored cell of 0: the cell is 0.

In every such case the value still decodes to the same number of bytes, so a disagreement is a **warning, not malformed input**. A reader that keeps a position for byte-exact rewriting keeps the original flag byte; a writer producing new data recomputes it from the values. No stored position in this install disagrees (V11).

<a id="s4-7"></a>
### 4.7 Colour (code 16)

A colour is a u32 whose value is 0xAARRGGBB, stored little-endian, so the four bytes in the record are **blue, green, red, alpha** (BIN; Confirmed). The client turns each channel into a float by dividing by 255 and keeps the four floats in the order red, green, blue, alpha; when writing, it multiplies each float by 255 and truncates. The same reader serializes colours in many other record types, and `environment.md` §5.6 gives the same layout for sky records.

Evidence in data (OBS2):

- Byte 3 is 0xFF in all 1,893 set values, so it is alpha. Dictionary defaults also use 127 and 76 in two semi-transparent tool colours.
- In all 149 skin colours of character appearances (PID 0x100001E0), byte 2 ≥ byte 1 ≥ byte 0, the red-dominant order of skin tones when byte 2 is red. The same holds for all 234 hair colours (PID 0x10000595) and for 112 of 119 lip colours (PID 0x100066D9). Under the opposite reading, all skin colours would be blue.

<a id="s4-8"></a>
### 4.8 StringInfo (code 13)

The encoding is in `records.md` §4, corrected by §17.4 here. Property-set observations (Confirmed, 1,110,502 values):

- 65,496 values are literals and 1,045,006 are string-table references.
- **No value has an argument block** (has_args = 1), and every argument table is empty (hint 1, count 0; OBS3). The argument path is still unexercised by data; its layout is in §17.4.1 (BIN only).
- Every reference names a 0x25 table, except 2 values whose table DID and token are both 0. Treat (0, 0) as "no text".
- 812,288 references resolve to an existing (table, token) pair in `client_local_English.dat`, and 232,718 (22.3%) do not. As in `records.md` ST7, a missing token is a lookup miss, not a parse error.

<a id="s4-9"></a>
### 4.9 DID reference (code 20)

A u32 DID. The value 0 means "no reference". It occurs 1,475,154 times.

The definition's `data` field names the **DB-object type** that the reference is expected to point to (§9). For all but 2,272 of the non-zero values, the DID lies in the DID range of that type. The exception is two icon-reference definitions declared with the game-object code (0x45) whose values are render surfaces (0x41). So readers **must not reject** a DID outside the declared type's range. They may warn.

<a id="s4-10"></a>
### 4.10 Struct (code 11)

A struct value is a property set in the table form (§5.2) without a leading DID:

| Field | Type | Meaning |
|---|---|---|
| hint | u8 | Bucket-count index of a TSize (`records.md` §1.3) |
| field_count *f* | VLE | Number of fields |
| fields | table entry × *f* | Each entry is a u32 key, then a property: u32 PID and value (§5.2) |

The client reads the header and the fields with the same TSize collection reader it uses for top-level sets (BIN), so **the count is a VLE**: a struct with 128 or more fields stores it in the 2- or 4-byte VLE form (`records.md` §1.2). Example: hint 4 and 130 fields give the header `04 80 82`.

Confirmed facts (all 2,799,313 structs):

- The hint byte is 0 (2,784,714), 1 (9,615) or 2 (4,984).
- Fields are in **bucket order** by PID modulo the bucket count of the hint (2,799,307 of 2,799,307 structs checked in gamelogic).
- *f* ranged over 0–62. Hint 0 was seen with *f* = 0–20, hint 1 with 23–46, and hint 2 with 48–62. So in every struct, *f* ≤ 2·*B*[hint].
- A field PID never repeats within one struct.

LC and MK read the header as "skip a byte, then a u8 count". That gives the same result on all observed data, but it is wrong for 128 or more fields.

<a id="s4-11"></a>
### 4.11 Array (code 12)

An array value is a property list in the list form (§5.2):

| Field | Type | Meaning |
|---|---|---|
| element_count *e* | u32 | Number of elements. The client's reader fails if *e* exceeds the remaining bytes (BIN). |
| elements | list entry × *e* | Each entry is a property: u32 PID and value (§5.2), with no key |

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
| 2 | count *n* | TSize | Number of entries (`records.md` §1.3) |
| 3 | entries | table entry × *n* | §5.2 |
| 4 | *(end)* | | Zero trailing bytes in every record |

An embedded property set (in another record type) is the same structure without the leading DID: a TSize followed by *n* table entries.

<a id="s5-2"></a>
### 5.2 Entries

A **property** is a u32 PID followed by its value, decoded per §4 with the type from the PID's definition. Properties are stored in two forms (BIN):

| Form | Used for | Entry layout |
|---|---|---|
| Table form | Top level of a set; fields of a struct | u32 key, then a property (u32 PID, value). In every observed entry the key equals the PID, which is why the pair looks like "PID, PID echo". |
| List form | Elements of an array | A property (u32 PID, value), with no key |

A struct or array value contains further entries, so decoding is recursive.

**Null property** (BIN; never observed): a property whose PID is 0 has **no value**.

- In the table form a null entry is the u32 key followed by a PID of 0: **8 bytes**. The key of a null entry is normally 0; a reader does not require it.
- In the list form a null element is a PID of 0 alone: **4 bytes**. It occupies its index in the array: later elements keep their positions.

LC reads a zero first u32 in the table form as a complete null entry of 4 bytes. That is wrong for this client (corrected, X9).

<a id="s5-3"></a>
### 5.3 Decoding algorithm (in words)

1. Read the DID (record form only), then the TSize: a hint byte and a VLE count *n*. Fail if the hint is above 22 (the client's bucket table has 23 entries, `records.md` §1.3, §17.2) or if *n* fails the count limit below. §17.2 states this hint and count rule once for every TSize in this spec.
2. For each of the *n* entries, read a table entry:
   1. Read the key, then the PID.
   2. If the PID is 0, the entry is null; go to the next entry.
   3. If the PID differs from the key, the record is malformed. (The client does not compare them, but no observed entry differs.)
   4. Look up the PID in the dictionary. An unknown PID makes the record undecodable, because the value length is unknown: the client's reader fails too (BIN). Report it as such; do not guess.
   5. Decode the value by the definition's type. For a struct, read the TSize header and then *f* table entries. For an array, read the u32 count and then *e* list entries; a list entry is a PID followed by its value, or a PID of 0 alone.
3. After the last entry, the payload must be exhausted. Leftover bytes mean the record is malformed or the dictionary does not match the data.

Defensive limits for hostile input:

- **Counts.** The client fails a collection whose count exceeds the number of remaining bytes (BIN). Tessera applies tighter bounds that still accept null entries: a table entry needs at least 8 bytes (key and PID), so a TSize or struct count cannot exceed remaining / 8; a list element needs at least 4 bytes (a PID), so an array count cannot exceed remaining / 4. Also apply an absolute cap, for example 1,000,000.
- **Hint.** Never size anything by the hint: the client's table runs up to 50,331,599 buckets for hint 22 (§17.2).
- **Nesting depth.** Observed values sit at most 9 levels below the top level (depth histogram in §11). The client has no limit of its own; a limit of 32 is suggested.

<a id="s5-4"></a>
### 5.4 Order and hint (Confirmed)

- **PS-O1**: the top-level entries of every record are in bucket order: PID modulo *B*[hint] is non-decreasing (384,950 of 384,950 records).
- **PS-O2**: struct fields follow the same rule with the struct's own hint (§4.10).
- **PS-H1**: in every set, *n* ≤ 2·*B*[hint]. Observed hints by top-level count: 0 for *n* = 0–22, 1 for 2–46, 2 for 23–94, 3 for 76–117, 4 for 106–217, 5 for 385–467, and 6 for 801–811. The largest *n* is 811.
- **PS-H2**: the hint is **not** a function of *n*. 230,285 records use a hint smaller than "smallest *B* ≥ *n*", and 1,147 use a larger one. The client's writer stores the index of its in-memory table's bucket count, which depends on the table's history (BIN). A byte-exact writer must keep the original hint. A writer producing new data should pick the smallest hint with 2·*B* ≥ *n* (which matches every observed struct) and write the entries in that hint's bucket order.

Hint counts over all records: 0 (126,430), 1 (46,027), 2 (97,173), 3 (74,764), 4 (3,627), 5 (36,791), 6 (138).

<a id="s5-5"></a>
### 5.5 Uniqueness

- **PS-U1**: a PID appears at most once among the top-level entries of a set (Confirmed: no duplicates in 32,676,318 top-level entries).
- **PS-U2**: a PID appears at most once among the fields of a struct (Confirmed).
- Array elements may repeat a PID (§4.11).

The client does not reject a repeated key: it stores both entries, and a lookup returns the one stored **first** (BIN; for entries in bucket order, which is the only form that occurs). A reader that meets a duplicate top-level or struct PID reports the record as malformed. If it must continue, it keeps the first value.

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

The client's type registry assigns the whole range 0x78000000–0x7FFFFFFF to property-set records (DB-object type 0x32) and 0x70000000–0x77FFFFFF to game objects (type 0x45) (BIN, §9).

| Prefix | Archive (count) | Meaning |
|---|---|---|
| 0x78 | gamelogic (1,616: 0x78000001–0x7800CD78, sparse); local (2) | **Stand-alone property sets** ("DB properties"). Many are reached through DID maps: CONTENT_LAYER, PHYSICSCONTROLLER, SCRIPTCALLBACK, DBPC, UNIQUEDB (§8). |
| 0x79 | gamelogic (383,332) | **Object property sets**: the properties of the 0x70 object with the same serial (§6.2) |
| 0x7A–0x7F | none | **Reserved for property sets** (BIN). They would hold the sets of game objects with prefixes 0x71–0x76 (§6.2). No record exists in this install. |

The two 0x78 records in `client_local_English.dat` do not exist in gamelogic. Routing (`dat-container.md`) must therefore try gamelogic and then local for the 0x78–0x7F range, as it already does.

All 384,950 records decode with the layout in §5.1.

<a id="s6-2"></a>
### 6.2 The +0x09000000 rule (Confirmed for 0x70)

The client finds the property set of a DID *d* as follows (BIN):

1. If *d* lies in the game-object range 0x70000000–0x77FFFFFF, the property-set DID is 0x78000000 + 0x01000000 + (*d* with its top four bits cleared). For prefixes 0x70–0x76 this equals *d* + 0x09000000: 0x70 → 0x79, 0x71 → 0x7A, …, 0x76 → 0x7F.
2. If *d* lies in the property-set range 0x78000000–0x7FFFFFFF, the set is the record *d* itself.
3. For any other DID there is no property set.

Facts:

- For 0x70 objects (gamelogic, 383,332 records) the rule gives 0x79 records, and the serial sets are **identical**: every 0x70 serial has a 0x79 partner, and every 0x79 has a 0x70 partner.
- The 11 records with prefix 0x76 are class-library singletons, which have no property set (`wstate.md` §2). Their 0x7F targets do not exist, and the lookup is a miss.
- For prefix 0x77 the formula gives a DID with prefix 0x80, outside the property-set range, which holds landblock records. No 0x77 object exists, so the client's behaviour there is never exercised. Tessera applies step 1 to prefixes 0x70–0x76 only and treats a 0x77 DID as having no property set.
- 0x78 records are stand-alone. No 0x6F record exists.

A reader treats a missing target as "no properties" and never applies the rule to its own result.

<a id="s6-3"></a>
### 6.3 Embedded property sets

DS lists embedded property sets (§5.1 without the DID) in landblock info (prefix 0x80, K = 2), UI layouts (0x22), script tables (0x07), entity descriptors (0x47, after a 16-byte header), block-map descriptors (0x18) and appearance records (0x20). The container specs now decode most of them with the encoding here (`landblock.md`, `entity-descriptor.md`, `setup.md`, `script-table.md`, `wstate.md`), and their placement belongs to those specs. DS also reports a single-PID "property list" form in 0x18 records: a u32 count followed by entries in the list form; null elements keep their index there too (§5.2).

---

<a id="s7"></a>
## 7. Resolving values to readable form

None of this is needed to **decode** a set. It is needed to **display** one, as in the explorer, and by gameplay code. The rules below are those of the client (BIN) unless marked.

<a id="s7-1"></a>
### 7.1 Enum values (code 8)

The definition's `data` holds a DID. The client resolves an enum value by the **DB-object type of that DID** (§9), not by where the DID came from:

- **Enum mapper (0x23…, `records.md` §7):** the key's **name** is looked up in the mapper and then up its base chain; the key's **display text** (a StringInfo) is looked up separately in the same way. Each lookup takes the first mapper in the chain that has the key **in that list**, so a name may come from one mapper and the display text from another (E2). In a one-third sample, 2,535,620 values resolved and 46 did not. An unresolved key is shown as its number.
- **DID map (0x28…, `records.md` §8):** the key's name is the map's **label** for the key. A DID-map enum has no display text, and the map's target DID plays no part in the display. Key 0 is treated like any other key: every DID map has key 0 with target DID 0, and its label is shown as the name (E1). Nineteen type-8 definitions name DID maps (0x28000001, 0x28000002, 0x28000007, 0x28000008, 0x28000011, 0x28000012, 0x28000025 and 0x28000030 in this install); values occur for 0x28000012 (CONTENT_LAYER, 1,552 values) and 0x28000008 (REGION, 11 values).
- **Any other DID:** the value has no name.

The client's debug display (C8) shows a named enum value or property-ID value (§7.4) in one of two ways, chosen by the caller: the name alone, or the name, a space, and the key in square brackets written as `0x` and eight upper-case hexadecimal digits. For a synthetic key 42 named `NAME` that is `NAME [0x0000002A]`. A key it cannot name is shown as an error text that contains the key in the same hexadecimal form. Tessera views may display values any way they like; this is recorded for fidelity only.

<a id="s7-2"></a>
### 7.2 Bitfields (codes 14, 18, 22)

Labels come from the mapper named by the definition's `data` (always 0x23…), with the lookups of §7.1.

<a id="s7-3"></a>
### 7.3 Bit-to-key rule (Confirmed)

**Bit *i* (counting from 0 at the least significant bit, or from byte 0 bit 0 for code 22) is labelled by enum key *i* + 1.** Of 1,062,289 set bits in a half sample (858,662 of code 18, 179,300 of code 14, 24,327 of code 22), every one had a label under this rule. The alternatives "key = *i*" and "key = 2^*i*" each leave thousands unlabelled. Two label routines of the client walk the bits in exactly this way (BIN).

Display rules of the client (E3):

- An all-zero mask is shown by the **name of key 0**, when the mapper chain has one (372 of the 385 bitfield definitions do).
- Otherwise the names of the set bits are listed in ascending bit order, separated by spaces.
- A set bit whose key has no name is **left out**, and the client marks the result as incomplete. None occurs in observed data. A Tessera viewer may instead show such a bit by its index (decision for debugging views).

<a id="s7-4"></a>
### 7.4 Property-ID values (code 10)

The value is a PID; display it by its dictionary name (the embedded name mapper of §2.1). In observed data, 705,015 values are 0 (none), and the other 490,540 are all defined PIDs.

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

The root's second group has one entry, "XBOX360_WORLDVIEWER", whose target 0x28000024 does **not** exist in this install. DID map 0x28000025 exists but is not referenced by the root. It is reached through the EMAPPER map's second group, as a DID map used for enum-like lookup (§8.3).

<a id="s8-3"></a>
### 8.3 Second groups in general (Confirmed)

The second groups are not all dead entries. EMAPPER's second group has 122 entries, and 76 of them resolve to existing mappers. These include the DB-object-type mapper used in §9 ("DBOType" → 0x2300007D). The other 46 point at DIDs that do not exist, among them the "PropertyType", "PropertyGroup", "PropertyProvider" and "PatchFlags" mappers. The second-group entries of UNIQUEDB, KEYMAP, WLIB, WEENIECONTENT and DBPC are all dangling.

Dangling targets per map, counting both groups: EMAPPER 46, UNIQUEDB 28, MUSIC 8, UILAYOUT 2, KEYMAP 2, root 1, REGION 1, WLIB 1, DBPC 1.

Readers merge both groups (as `records.md` §8.1 says) and treat a dangling target as a lookup miss. This refines the `records.md` §8.1 OPEN: second groups hold entries that the client build may not ship, such as tool, server or other-platform data. They are still not useless.

**EMAPPER entries that name DID maps** (7 of the 435 non-null targets: four in the first group, three in the second; for example 0x28000012, 0x28000030 and 0x28000025) are enum sources like the mappers. A name-based enum lookup returns the target DID, whatever its type, and its keys are then resolved by the type of that DID as in §7.1: a DID map gives the label of each key as its name, and no display text (BIN).

---

<a id="s9"></a>
## 9. DB-object-type codes

A type-20 definition's `data` field (§4.9) holds a code from the **DB-object-type** enumeration. That enumeration is in the data: it is the enum mapper labelled "DBOType" in the EMAPPER DID map (0x2300007D in this install, which has no entries of its own and inherits all 75 from its base mapper 0x230000F0). A client reads the code labels from there at run time.

The client also holds a registry that assigns each code a DID range (BIN). Ranges are format facts and may be used directly. The table covers every code that a type-20 definition declares (29 codes), with the DID prefix that the stored values actually carry where any value exists (OBS). The descriptive kinds are this spec's words.

<a id="s9-1"></a>
### 9.1 Codes declared by type-20 definitions

| Code | Descriptive kind | DID range (BIN) | DID prefix of referenced values (count, OBS) |
|---|---|---|---|
| 0x05 | Render surface | 0x41000000–0x41FFFFFF | 0x41 (610,521) |
| 0x08 | Movie | 0x0D000000–0x0DFFFFFF | none |
| 0x0A | Texture list (`texture-list.md`) | 0x40000000–0x40FFFFFF | none |
| 0x0B | Setup | 0x04000000–0x04FFFFFF | 0x04 (26) |
| 0x0D | Camera state | 0x08000000–0x08FFFFFF | none |
| 0x0F | Animation clip | 0x05000000–0x05FFFFFF | none |
| 0x12 | Sound info | 0x2A000000–0x2AFFFFFF | 0x2A (35,095) |
| 0x15 | Scriptlet | 0x0C000000–0x0CFFFFFF | 0x0C (88,794) |
| 0x1A | Entity group | 0x46000000–0x46FFFFFF | none (no 0x46 record exists) |
| 0x1E | Particle-system descriptor | 0x39000000–0x39FFFFFF | 0x39 (272) |
| 0x1F | Material (`material.md` §6) | 0x30000000–0x30FFFFFF | none |
| 0x20 | String table | 0x25000000–0x26FFFFFF | 0x25 (1) |
| 0x26 | Image compositor | 0x33000000–0x33FFFFFF | none |
| 0x28 | UI layout | 0x22000000–0x22FFFFFF | 0x22 (544,130) |
| 0x2A | Script table | 0x07000000–0x07FFFFFF | 0x07 (429,655) |
| 0x32 | DB property set | 0x78000000–0x7FFFFFFF | 0x78 (2,626) |
| 0x36 | Surface record (`material.md` §4) | 0x31000000–0x31FFFFFF | none |
| 0x38 | Mesh | 0x06000000–0x06FFFFFF | none |
| 0x39 | Entity descriptor | 0x47000000–0x47FFFFFF | 0x47 (204,005) |
| 0x3A | Day description (`environment.md`) | 0x1A000000–0x1AFFFFFF | none |
| 0x43 | UI scene | 0x21000000–0x21FFFFFF | 0x21 (103) |
| 0x44 | Map-note descriptor | 0x0E000006–0x0E00FFFF | none |
| 0x45 | Game object (WState) | 0x70000000–0x77FFFFFF | 0x70 (3,153,984); 0x41 (2,272, see §4.9) |
| 0x4C | Font | 0x42000000–0x42000FFF | 0x42 (6) |
| 0x4D | Enum mapper | 0x23000000–0x23FFFFFF | 0x23 (5) |
| 0x4E | Appearance | 0x20000000–0x20FFFFFF | 0x20 (843,720) |
| 0x50 | Script channels | 0x43001000–0x43FFFFFF | none |
| 0x51 | Shader record (`material.md` §7) | 0x2B000000–0x2BFFFFFF | none |
| 0x56 | Particle-system descriptor, second kind | 0x44000000–0x44FFFFFF | none |

Facts:

- For the 15 codes that stored values exercise, every value lies in the code's range except the 2,272 values of §4.9 (OBS).
- The dictionary itself (code 0x25, 0x34000000–0x34FFFFFF) and DID maps (code 0x49, 0x28000000–0x28FFFFFF) complete the codes this spec uses.
- The same numbering matches the cache type IDs that ER reports for landblock data (17), landblock info (46), landblock block map (47), landblock properties (41), entity groups (26) and navigation landblocks (100). Each of those is also a code in the mapper.

This gives new evidence for `records.md` §2.1 (§17.3).

---

<a id="s10"></a>
## 10. Withdrawn: other serialised forms

This section is **withdrawn** (2026-10-05). It used to describe how property sets are encoded in the official game protocol. That is out of scope until the `docs/LEGAL.md` §9.3 gate is passed (decision 0004), and no maintainer approval for it as T2 design input is recorded. Tessera's own protocol (M7) defines its own encoding and does not need it. The anchor is kept so that older references still resolve. Everything in this spec describes the stored form in the archives only.

---

<a id="s11"></a>
## 11. Validation runs (2026-10-04, re-run 2026-10-05)

A throwaway Python probe in the dirty-room scratchpad decoded every 0x78–0x7F record in every archive that has them. It worked from the layouts in this spec and the 0x34000000 dictionary. The 2026-10-05 re-run, with the table-form and list-form entries of §5.2 and the count and hint limits of §5.3, gave the same results: no record trips a limit.

| Measure | Result |
|---|---|
| Records attempted | 384,950 (gamelogic 0x79: 383,332; gamelogic 0x78: 1,616; local 0x78: 2) |
| Decoded with zero trailing bytes | **384,950 (100.000%)** |
| Records with trailing bytes | 0 |
| Records failing to decode | 0 |
| Property types responsible for failures | none |
| Unknown PIDs met | 0 |
| Key/PID mismatches | 0 |
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
| X2 | LC, MK, DS: struct value is "skip one byte, then u8 field count" | The skipped byte is a bucket-count hint (0–2), fields are in bucket order for it, and the client reads the count as a VLE (BIN) | §4.10. Readers that skip the byte still decode all observed data. |
| X3 | `records.md` §6.4: type 17 encoding "to be specified" | Flagged layout confirmed, including two payload-less bits | §4.6 |
| X4 | `records.md` §13: is type 21 strictly 0/1? | Only 0/1 in 6.26 M set values and 2,003 dictionary values; the client fails on anything else (BIN) | Strict `bool8` is safe (§4.5) |
| X5 | DS/LC: the +0x09000000 rule covers 0x70–0x77 | Only 0x70 → 0x79 pairs exist; 0x76 has no partners; 0x77 would leave the property-set range (BIN) | §6.2 |
| X6 | ER: dictionary has 27,268 entries | 27,547 names / 27,546 definitions today | Counts change with patches; ER described an older build |
| X7 | `records.md` §2.1: 0x0C "possibly entity/prop data"; 0x2A, 0x21 and 0x39 unknown | Type-20 references declared as scriptlet, sound info, UI scene and particle-system codes point at exactly these prefixes; the client's registry gives the same ranges (BIN) | §9. `records.md` §2.1 should be updated by its owner (§17.3). |
| X8 | `records.md` §8.1 OPEN: the second DID-map group may be useless | EMAPPER's second group holds 76 live mappers | §8.3 |
| X9 | LC (and this spec until 2026-10-04): a zero first u32 is a complete 4-byte null entry in every context | In the table form the client reads a key and then the PID; a null entry is 8 bytes there (BIN) | §5.2 |
| X10 | LC: default, minimum and maximum are skipped for types 10, 11 and 12 | The client reads them for every type (BIN) | §2.2 |
| X11 | LC: waveform subtype 0 carries no floats | Every subtype except 1 carries ten floats (BIN) | §4.2 |
| X12 | This spec until 2026-10-04: a duplicate PID keeps the last value | The client's lookup returns the first stored entry (BIN) | §5.5 |
| X13 | This spec until 2026-10-04: the f32 1.5 after the optional values is a fixed marker, and the dictionary has 8 reserved bytes and a 2-byte separator | They are fields of the definition and of an embedded enum mapper (BIN) | §2.1, §2.2 |
| X14 | `records.md` §1.2 (LC): VLE first byte 0xE0 is a 5-byte escape | The client's VLE has no escape (BIN) | §17.1 |
| X15 | `records.md` §4.1 (LC): StringInfo ends with a terminator `01 00`, or with three strings and a VLE count | The client reads an argument table (TSize) in both cases (BIN) | §17.4 |
| X16 | `records.md` §4.1: the StringInfo first byte is a `bool8` | The client treats 1 as literal and every other value as a reference, without a check (C4) | §17.4 reader rules |
| X17 | This spec until the 2026-10-05 repair: "accept any hint and require only the count" for StringInfo argument tables, against §5.3 | One TSize rule for every collection (hint 0–22, count bound by the smallest entry) | §17.2 |

---

<a id="s13"></a>
## 13. Open questions (summary)

1. Meanings of the definition metadata that does not affect decoding: field_a, field_b, field_c, field_d bytes 0, 1 and 3, most flag bits, `field_e` and the subset list (§2.3). Non-blocking; partly gated.
2. The game clock behind relative timestamps (§4.3). Server-defined; gated by decision 0004; no stored value needs it.
3. StringInfo arguments: none in 1.1 M property values. Their stored layout is now given (§17.4.1, BIN); the meaning of their fields and of the three strings, and the substitution rule, are open (§17.4; `records.md` §4).

Resolved in the 2026-10-05 repair: the stored layout of StringInfo argument values (§17.4.1), the reader rules for disagreeing position flags (§4.6) and for unusual StringInfo first bytes (§17.4), and one hint rule for every TSize (§17.2).

Resolved earlier on 2026-10-05: the dictionary prefix and separator (§2.1), the definition tail (§2.2), struct counts ≥ 128 (§4.10), waveform subtypes (§4.2), the tri-state values (§4.3), position bits 0x40/0x80, the quaternion order, the instance field and units (§4.6), colour channel order (§4.7), the 0x76 objects (§6.2), DB-object-type codes not exercised by data (§9), the null entry (§5.2), and all questions of §15 and §16.

---

<a id="s14"></a>
## 14. Validation guidance for implementers

Real-data tests (skipped without an install) should assert the **invariants**, not the counts:

- every 0x78/0x79 record decodes with zero trailing bytes;
- PS-O1 holds, and PS-U1, PS-C1 and PS-C2 hold;
- every 0x70 DID's §6.2 partner exists;
- every position value follows the writer rules of §4.6, and every colour's byte 3 is 0xFF;
- MP7 holds for the dictionary.

Synthetic fixtures, built by our own code with a synthetic dictionary, should cover:

- one value of each type code 1–22, including waveform subtypes 0 (ten floats), 1, 3 and 10, and every position flag bit (with 0x40 and 0x80 carrying no bytes);
- a waveform of subtype 10 with loop byte 2 (rejected) and one with a pair count larger than the remaining bytes (rejected without allocating);
- a struct with hint 0 and with hint 1, with fields in the matching bucket order, an open struct with an unlisted field, and a struct with 130 fields (2-byte VLE count);
- an array with zero elements, one with mixed element PIDs, one with a null element in the middle (later indices unchanged), and one whose count exceeds the remaining bytes (rejected without allocating);
- a table-form null entry (key 0, PID 0: 8 bytes), a key/PID mismatch, an unknown PID, a duplicate PID, trailing bytes, a TSize hint of 23 (rejected), and nesting one level past the depth limit;
- a TSize whose count is 0x4000 (4-byte VLE `C0 00 00 40`) and a VLE whose first byte is 0xE0 (read as the 4-byte form, value 0x20xxxxxx, never as a 5-byte escape);
- a StringInfo whose first byte is 2 (decoded as a reference, with a warning), one with `has_args` = 2 (rejected), the 32-byte argument example of §17.4, an argument of kind 9 (6 bytes, no value), a kind-1 argument holding a nested StringInfo, and an argument table whose count exceeds remaining / 6 (rejected without allocating);
- positions whose flags disagree with the payload (§4.6): 0x40 set with 0x01 clear (region 0, warning) and 0x80 set with a stored cell of 0x0123 (cell 0x0123, warning);
- a code-22 bitfield whose ⌈*b*/8⌉ exceeds the remaining bytes (rejected before allocating);
- a dictionary record with a non-empty embedded display list, and a definition of type 11 with the default bit set;
- a variable-width bitfield with *b* not a multiple of 8;
- a colour, a timestamp with the "no time" value, and a tri-state of 255;
- a 0x70 DID with and without a 0x79 partner, a 0x76 DID (no partner), a 0x77 DID (no property set) and a 0x7A DID (itself a property-set DID).

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
| `BB 0B 00 10` `BB 0B 00 10` | key 0x10000BBB and PID 0x10000BBB (0x10000BBB mod 11 = 3) |
| `07 00 00 00` | int 7 |
| `AA 0A 00 10` `AA 0A 00 10` | key and PID 0x10000AAA (mod 11 = 5, so it comes after the bucket-3 entry) |
| `00 01` | struct header: hint 0, one field |
| `CC 0C 00 10` `CC 0C 00 10` | field key and PID 0x10000CCC |
| `01` | true |

If 0x10000DDD were an array of type-15 elements with PID 0x10000123, the value "[5, 6]" would follow the key and PID as `02 00 00 00`, then `23 01 00 10 05 00 00 00`, then `23 01 00 10 06 00 00 00`. Array elements have no key.

<a id="s14-2"></a>
### 14.2 More worked synthetic examples

**Null entries.** In the set above, a null top-level entry would be the 8 bytes `00 00 00 00 00 00 00 00` (key 0, PID 0) and would count toward *n*. In the array, the value "[5, null, 6]" is `03 00 00 00`, `23 01 00 10 05 00 00 00`, `00 00 00 00`, `23 01 00 10 06 00 00 00`: the null element is 4 bytes and occupies index 1.

**Dictionary prefix.** A synthetic dictionary with two names and one definition begins `00 00 00 34` (DID), `00 00 00 00` (embedded mapper DID), `00 00 00 00` (its base), `00 02` (names: hint 0, two entries), the two (pid, name) entries in bucket order, `00 00` (empty display list), then `00 01` (definitions: hint 0, one entry) and the definition.

**Colour.** The bytes `40 80 C0 FF` are the u32 0xFFC08040: red 0xC0, green 0x80, blue 0x40, alpha 0xFF, or (0.753, 0.502, 0.251, 1.0) as floats.

**Position.** Region 1, landblock (0x10, 0x20), cell 0, (x, y, z) = (10.0, 20.0, 30.5), identity orientation. The flag byte is 0x01 + 0x02 + 0x08 + 0x10 + 0x40 + 0x80 = 0xDB, and the 18 bytes are `DB`, `01`, `10 20`, `00 00`, `00 00 20 41`, `00 00 A0 41`, `00 00 F4 41`.

**Timestamp "no time".** `00 00 00 E0 FF FF EF 47`.

**Property-set DIDs.** 0x70001234 → 0x79001234. 0x76000005 → 0x7F000005, which does not exist (a singleton). 0x77000001 → no property set. 0x7A000001 → itself.

---

<a id="s15"></a>
## 15. Questions raised by the `formats` implementation (2026-10-04), resolved 2026-10-05

These came up while implementing property-set decoding in `crates/formats` from this spec alone. The whole-install run of `crates/formats/tests/m1_real_data.rs` reproduces §11 exactly (384,950 of 384,950 records clean, 47,680,867 values, the same depth histogram), so none of these choices is exercised by current data.

> **Resolved question** (count bounds): §5.3 suggested at least 9 bytes per entry (5 per array element), but a Reported null entry was a bare 4-byte zero PID; the implementation bounds every count with 4 bytes per entry.
>
> **Resolution:** the null entry is 8 bytes in the table form and 4 bytes in the list form (§5.2, BIN), so the bounds are now 8 bytes per table entry and 4 bytes per list element (§5.3). Both accept null entries; the implementation's 4-byte bound is also safe but looser. The client itself only requires each count to be at most the remaining byte count.

> **Resolved question** (null entries in arrays): does the null entry apply to array elements, and are positions kept? The implementation treats a zero PID as null in all three contexts and does not store null entries.
>
> **Resolution:** yes for arrays: a list element whose PID is 0 has no value and keeps its index (§5.2, BIN). Top-level and struct entries are different: after the key, the PID is read, and only a PID of 0 makes the entry null (8 bytes). An implementation that reads a single zero u32 in the table form misframes the rest of the record. A byte-exact writer keeps null entries in place.

> **Resolved question** (duplicates): §5.5 said a duplicate top-level or struct PID makes the record malformed; the implementation rejects such records.
>
> **Resolution:** rejecting is correct for Tessera. The client stores both and returns the first stored on lookup, so a lenient reader keeps the first value (§5.5, BIN). The earlier "keep the last" advice was wrong (X12).

> **Resolved question** (0x77): the +0x09000000 rule maps 0x77 to prefix 0x80, which holds landblock records; the implementation applies the rule to 0x70–0x76 only.
>
> **Resolution:** that is the specified behaviour (§6.2). The client's formula gives the same DIDs as +0x09000000 and would also land in 0x80 for 0x77; no 0x77 object exists. The rule never produces a DID outside 0x79–0x7F when it is limited to 0x70–0x76.

> **Resolved question** (0x7A–0x7F): `records.md` §2.1 does not list those prefixes, so the DID classifier calls them unknown while the property-set helpers accept them.
>
> **Resolution:** the client's registry assigns the whole range 0x78000000–0x7FFFFFFF to property-set records (§6.1, §9, BIN). The classifier should report 0x7A–0x7F as property-set DIDs; `records.md` §2.1 should list them as such (§17.3).

---

<a id="s16"></a>
## 16. Questions raised by the `props` implementation (2026-10-04), resolved 2026-10-05

These came up while implementing `crates/props` (registry, typed values, labels, DID-map navigation) from this spec and `records.md` alone. On the install of 2026-10-04 the real-data test (`crates/props/tests/real_data.rs`) resolved 50,000 random 0x70 objects (6,171,402 values): every value named, 99.998% of enum values labelled, every set bit labelled, every non-zero PID value named, and all 29 DB-object-type codes that type-20 definitions declare labelled through "DBOType".

> **Resolved question** (base chain): a mapper has two lists; does the client look up names and display texts independently along the base chain, or stop at the first mapper that has the key in either list?
>
> **Resolution:** independently. Each lookup walks the chain and stops at the first mapper that has the key **in its own list** (§7.1, BIN). The implementation's behaviour matches the client.

> **Resolved question** (EMAPPER-named DID maps): how is a DID map named by EMAPPER used as an enum?
>
> **Resolution:** keys are enum keys and the map's labels are their names; there is no display text. The client resolves every enum DID by its DB-object type, so a name-based lookup through EMAPPER should return the target DID of either type, and key resolution then follows §7.1 (§8.3, BIN).

> **Resolved question** (key 0 of DID-map enums): show the "UNDEFINED" sentinel as a label or as "none"?
>
> **Resolution:** as its label, like any other key; its target DID is 0, which plays no part in the display (§7.1, BIN). The implementation's behaviour matches the client.

> **Resolved question** (unlabelled bits): what should a viewer do with a set bit whose key *i* + 1 has no name?
>
> **Resolution:** the client leaves such bits out of the list and marks the result as incomplete; it shows an all-zero mask by the name of key 0 when there is one (§7.3, BIN). Showing an unlabelled bit by its index, as the implementation does, is an allowed Tessera display decision for debugging views; a player-facing view should follow the client.

---

<a id="s17"></a>
## 17. Corrections to other specs

These specs are owned by other spec writers. The facts below were found while answering this spec's questions; their owners should fold them in. Until then, implementers of those specs follow the text here where it disagrees.

<a id="s17-1"></a>
### 17.1 `records.md` §1.2 (VLE)

The client reads and writes the VLEs of the stored formats in this spec with one routine (C1, BIN): the counts of TSize collections, the lengths of Pascal and UTF-16 strings, and the bit count of code-22 bitfields. That routine has **no 0xE0 escape**:

- a first byte below 0x80 is the 1-byte form;
- a first byte 0x80–0xBF starts the 2-byte form: (first byte & 0x3F) << 8, then the second byte;
- any first byte 0xC0–0xFF, 0xE0 included, starts the 4-byte form: (first byte & 0x3F) << 24, then the second byte << 16, then a little-endian u16.

The writer uses the 1-byte form up to 0x7F, the 2-byte form up to 0x3FFF and the 4-byte form above. Values of 0x40000000 and more cannot be represented, and a writer must refuse them. A reader that treats 0xE0 as a 5-byte escape misreads values 0x20000000–0x20FFFFFF. In this install no VLE starts with 0xE0 or more, and the only 4-byte VLEs are the two large counts of the dictionary, both with first byte 0xC0 (OBS3). This settles the `records.md` §1.2 OPEN.

<a id="s17-2"></a>
### 17.2 `records.md` §1.3 (TSize)

The client's bucket-count table has **23 entries** (C2, BIN). Hints 0–12 match `records.md` §1.3; hints 13–22 are 98,299; 196,597; 393,209; 786,431; 1,572,853; 3,145,721; 6,291,449; 12,582,893; 25,165,813; 50,331,599. When reading, the client sizes its table by the hint and inserts each entry after the previous one when they share a bucket, so entries within a bucket keep their stored order; when writing, it emits buckets in ascending order and each bucket's entries in that order (C3). The hint it writes is the index of its in-memory table's bucket count. This settles both `records.md` §1.3 OPENs.

**One rule for every TSize** in the formats of this spec (top-level sets, struct values, the dictionary's name list, display list and definition table, child lists, StringInfo argument tables, and embedded sets in other records):

1. The hint must be 0–22. A hint of 23 or more is malformed; the client's reader fails on it too (C3).
2. The count must not exceed the remaining bytes divided by the smallest possible entry of that collection: 8 bytes for property tables (key and PID of a null entry, §5.3); 5 bytes for the dictionary's name list (u32 PID and an empty Pascal string); 9 bytes for display lists (u32 key and the smallest StringInfo, which is 5 bytes: first byte, empty string length, `has_args` and the empty argument table); 8 bytes for child lists; 50 bytes for dictionary definitions (u32 key, seven u32 fields, `field_e`, `field_d`, an empty 2-byte child list, `subset_count` and `last`); 6 bytes for StringInfo argument tables (u32 key and u16 kind, §17.4). The client's own check is weaker (count ≤ remaining bytes), so this rule rejects nothing that could actually be decoded.
3. Any hint 0–22 is accepted with any count that passes step 2. The hint is not required to be the smallest hint that fits; a reader never sizes anything by it (§5.3) and keeps it for byte-exact rewriting.
4. Entries are not required to be in bucket order to be decoded. A reader may warn when they are not (PS-O1, PS-O2, MP7 hold for every observed collection).

<a id="s17-3"></a>
### 17.3 `records.md` §2.1 (DID high-byte table)

From the client's type registry (BIN, §9): 0x7A–0x7F belong to the property-set range 0x78000000–0x7FFFFFFF, and 0x77 to the game-object range; 0x26 belongs to the string-table range 0x25000000–0x26FFFFFF; and §9.1 gives the DID range of 29 DB-object types, including the prefixes 0x08, 0x0D, 0x1A, 0x21, 0x2A, 0x2B, 0x30, 0x31, 0x33, 0x39, 0x40, 0x43 and 0x44. The same registry covers the remaining prefixes of §2.1; that transcription belongs to the owner of `records.md`.

<a id="s17-4"></a>
### 17.4 `records.md` §4.1 (StringInfo)

The client reads a StringInfo in the stored form as follows (C4, C5, BIN). OBS3 shows which values occur: first byte 0 or 1, `has_args` 0, and an empty argument table, in all 1,110,503 StringInfos of the property sets and the dictionary.

1. **First byte.** 1 means a literal: a UTF-16 string follows (`records.md` §1.4). **Any other value** means a reference: a u32 token, then a u32 table DID. The client does not check this byte.
2. **`has_args` byte.** It must be 0 or 1; any other value makes the client's reader fail.
3. If `has_args` = 1, three Pascal strings.
4. **In both cases**, an argument table: a TSize collection (hint byte, VLE count; §17.2) whose entries are a u32 key (the variable's name hash), a u16 argument kind *K*, and then the value of kind *K* (§17.4.1).

The 2-byte "terminator `01 00`" of `records.md` §4.1 is therefore an **empty argument table** with hint 1 and count 0. LC's reading of the argument block (a VLE count with no hint byte, then per argument a u8 kind before the u32 hash) does not match the stored form.

**Reader rules** (Tessera):

- First byte: 0 and 1 are the normal values. A reader treats any other value as a reference, as the client does, warns, and keeps the byte so that a rewrite reproduces it. This corrects `records.md` §4.1, which calls the byte a `bool8`: it is not checked like one.
- `has_args` other than 0 or 1 is malformed.
- The argument table follows the TSize rule of §17.2 (hint 0–22, any such hint accepted; at most remaining / 6 entries).
- A non-empty argument table is decoded with §17.4.1. Nested StringInfos (kind 1) count toward the nesting limit of §5.3. Because the meaning of argument values and the way the client substitutes them into text are OPEN, a resolver that meets a StringInfo with `has_args` = 1 or with a non-empty table resolves the base text without substitution and reports the result as **"arguments not supported"**; it does not guess a substitution.

<a id="s17-4-1"></a>
#### 17.4.1 Argument values (BIN only)

Every value of kind 1–7 starts with a common 6-byte header; the kind-specific fields follow it. Offsets are from the start of the value, after the entry's u32 key and u16 kind.

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 2 | u16 | Kind again. The client stores this as the value's kind; writers repeat *K*. |
| 2 | 4 | u32 | Meaning OPEN |
| 6 | var | per kind | Table below |

| *K* | Fields after the header, in order | Value size (bytes) |
|---|---|---|
| 1 | A nested StringInfo, in this same layout (recursive) | 6 + StringInfo |
| 2 | u64, u16 | 16 |
| 3 | u64, u16, u8, u16 | 19 |
| 4 | u64 | 14 |
| 5 | u64, u8, u16 | 17 |
| 6 | Stored exactly like kind 4 | 14 |
| 7 | Stored exactly like kind 5 | 17 |
| 0, 8–65,535 | **No value bytes at all**, not even the header. The client creates no value and keeps the entry without one. | 0 |

A reader decodes unknown kinds the same way: the entry is 6 bytes (key and kind), it has no value, and the reader warns. All multi-byte fields are little-endian.

> **OPEN (for the owner of `records.md`, non-blocking):** The meaning of the three strings, of the header's u32 and of each kind's fields, and how the client substitutes argument values into the text. No stored StringInfo in this install has arguments (OBS3, §4.8).

**Worked synthetic example.** A reference to token 0x11 of table 0x25000001 with one kind-4 argument (key 0xAABBCCDD, header u32 0, u64 7) and `has_args` = 0 is 32 bytes: `00`, `11 00 00 00`, `01 00 00 25`, `00` (has_args), `01 01` (TSize: hint 1, one entry), `DD CC BB AA` (key), `04 00` (kind), `04 00` (header kind), `00 00 00 00` (header u32), `07 00 00 00 00 00 00 00` (u64). The same reference with no arguments is the 12 bytes `00`, `11 00 00 00`, `01 00 00 25`, `00`, `01 00`.

<a id="s17-5"></a>
### 17.5 `records.md` §7.1 (EnumMapper lookup)

"If the key is not found" applies to each list separately: names and display texts are looked up independently along the base chain (§7.1, BIN).

<a id="s17-6"></a>
### 17.6 Other specs

- `landblock.md` (OPEN on null elements in 0x18 descriptor palettes): if the palette is read in the list form, a null element keeps its slot, as that spec assumes (§5.2, BIN).
- `terrain-texturing.md` (colour order question pointing at §4.7): property colours are B, G, R, A (§4.7). The client's colour reader is shared by many record types; whether a given record field uses it is for that spec's owner to confirm.
- `material.md` §5.3 (float curve): the reader accepts any mode; every mode except 1 carries ten floats; a mode-10 loop byte of 2 or more and a pair count larger than the remaining bytes make the reader fail; the evaluator returns the base term *B* for modes outside 1–10 (§4.2, BIN).
- `coordinates.md` §11.1 and `environment.md` §5.6 agree with §4.6 and §4.7.

---

<a id="s18"></a>
## 18. Change log

| Date | Sections | Change |
|---|---|---|
| 2026-10-04 | all | First complete version (M1): dictionary, type codes, value encodings, property sets, +0x09000000 rule, DID maps, DB-object-type codes; validation run; implementation questions §15–§16 added by the clean room. |
| 2026-10-05 | header, provenance, §1, §2.1–§2.3, §2.5, §3, §4.1–§4.11, §5.2–§5.5, §6.1–§6.3, §7.1–§7.4, §8.2–§8.3, §9, §10, §11–§16; new §14.2, §17, §18 | Revised by the spec writer with M4 under decision 0011 (row BIN) and new whole-install checks (OBS2). Closed the OPENs on the dictionary prefix and separator, definition fields (structural part), waveform subtypes, timestamps and tri-states, position bits and orientation, colour channel order (B, G, R, A), struct counts, 0x76 objects and unexercised DB-object-type codes, and all §15–§16 questions. **Behaviour changes for implementers:** table-form null entries are 8 bytes (§5.2); count bounds are 8 and 4 bytes per entry (§5.3); TSize hints above 22 are rejected (§5.3); a lenient reader keeps the first of duplicate PIDs (§5.5); default/min/max are read for every type (§2.2); `timeout` and `last` are no longer checked (§2.2); waveform subtype 0 carries ten floats (§4.2); struct counts are VLEs (§4.10); 0x7A–0x7F are property-set DIDs (§6.1). Corrections to `records.md` and others in §17. |
| 2026-10-05 | header, Purpose, provenance (legend, OBS3, BIN questions 12–19, rows M3, N1 withdrawn, rows C1–C10), §2.2, §2.3, §2.5, §4.3, §4.4, §4.6 (new §4.6.1 anchor `s4-6-1`), §4.8, §5.3, §7.1, §10, §12 (X16, X17), §13, §14, §17.1, §17.2, §17.4 (new §17.4.1) | Repair after review (changes requested). **§10 withdrawn** to a stub: official-protocol encoding is out of scope until the `docs/LEGAL.md` §9.3 gate; the network remark in §4.6 and row N1 removed. **Third-party field names removed**: the "Analogue name" column of §2.2, row M3, the Analogue mark and source ACE are gone; the f32 field formerly called `timeout` is now `field_e`. **Provenance**: second M4 use logged as BIN questions 12–19, with fact rows C1–C10, and a new whole-install census OBS3 (no VLE starts with 0xE0 or more; all StringInfos have first byte 0/1, `has_args` 0 and an empty argument table). **Implementability**: one TSize rule for every collection (§17.2); StringInfo first byte other than 0/1 is a reference with a warning, `has_args` stays strict, argument values now have a stored layout (§17.4.1, BIN) and a resolver reports "arguments not supported" instead of guessing; reader rule for position flags that disagree with the payload (§4.6.1: payload bits win, warning only); code-22 bit counts are bounded before allocating (§4.4). |
