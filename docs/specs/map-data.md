<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Map data: named places, map notes, travel networks and neighbourhoods

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Spec reviewer (separate dirty-room session, AI-assisted), not the author; re-review by a second separate dirty-room reviewer session, AI-assisted, not the author |
| **Review record** | **Re-review 2026-10-07, `CLEAN_ROOM.md` §5.3: passed, status Accepted.** All nine checklist items hold: functional facts only; no code, no pseudo-code, nothing that follows a listing's structure; no copied prose, and labels used only as interoperability identifiers; no game text, keys or hostnames (the §3.2 debug-dump labels are now paraphrased, and the locale names of §3.3.1 are described, not quoted); every fact has a provenance row (MD24–MD27 cover §3.3.1, §4.4.1, §4.4.2 and §4.7); the BIN second use (q6–q9) names build, questions and why M1–M3 were not enough, within decision 0011; units are stated (§4.2); the invariants (MI8, MI15) are testable; unknowns are OPEN with Tessera rules; no closed gate. All six issues of the first review are resolved as the resolution note below says. Data re-checked over all records with the reviewer's own read-only probe (not the writer's): every position of the three tables and every `link_position` passes §4.4.1 (13,485, 9,025, 5,638 and 2,754; largest \|w² + x² + y² + z² − 1\| 2.4 · 10⁻⁷); all 10,731 non-link texts are references without arguments, all resolve in the English tables, 87 to empty text; the 2,754 link rows have null text and `level` 0, and `level` is 0 in no other row; 2,590 link rows qualify for a zone entry and 164 lead into their own dungeon, none has target 0; the eleven range pairs of §4.5 and no row with `min_range` − `max_range` > 0.0002; place names: 296,009 code units, maximum 0x7A, none 0, five 0x1F, 168 names with upper case, 9,025 distinct ignoring case, bucket order 9,024 of 9,024 under the lowered hash and 165 failures under the stored name; §4.3 link-position flag bytes and the 412 same-landblock rows (region, X and Y; 416 if the region is ignored). Five further Confirmed facts, all reproduced exactly: §4.5 (area, dungeon, entity classes, `layers` lengths and the 2,780 elements, flag bytes, regions, `icon`, `level`, the `type` bit counts, `flag_b`, `discovery`, `extra` 12,814/671 with 663 world-event arrays, 12,532 ascending pairs); §5.2 (5,460/178 entries, hint 0, 3,363 role constraints, 2,453 active-quest arrays, 16,124 quests and 16,121 role names, lengths 1–146, 720 of length 1); §5.3 (flag bytes, every row indoors and every `dungeon` of class 691, `area` 5,248/354/36, regions, entity classes, layer lengths); §3.5 (flag bytes, 7,351 orientations, no instance, regions, slot classes, 2,330 interior places); §4.1 (*n* = 13,485 with 0 bytes left, and the 53-byte minimum row). BIN spot check in a private copy of the analysis project (same build, decision 0011): the position test of §4.4.1 (the four CO §4 tests, finite x, y, z, finite orientation, f32 sum in stored order, tolerance 0.0002) and §3.3.1 (per-unit wide lower-casing with the full 16-bit unit in the hash, stop at unit 0, locale set for all categories from the game language with four locales and English as default) agree with the spec; q7 and q9 were not re-traced. Non-blocking notes for a later revision: §13.2 writes the `type` value as "1 << 37", which reads as code notation (say "bit 37 set, 2³⁷"); MD27's fact text does not mention §4.7 step 4 (rim placement, horizontal distance); the Tessera rules of §3.3.1, §4.4.2 and §4.7 carry no Decision mark (as in §14). *First review, 2026-10-07:* changes requested (issues below), with spot checks of MD1–MD3, MD5–MD7, MD9, MD12, MD13, MD16, MD18 and §13 that all agreed with the data. |
| **First review record** | 2026-10-07, `CLEAN_ROOM.md` §5.3: **changes requested**, status stayed Review. Spot checks over all records of the install (own read-only probes), all of which agree with the spec: MD1 and §2.2 (the UNIQUEDB and WEENIECONTENT entries, the payload sizes, no 0x0E000002); MD2, MD3 and MD5 (9,025 entries with 0 bytes left over, lower-cased-name bucket order holds for 9,024 of 9,024 pairs while the name as stored fails 165, `area` = `dungeon` everywhere, the flag, region and zone-class counts); MD6, MD7 and MD9 (13,485 rows with 0 bytes left over, the link block in exactly the 2,754 rows with bit 0, every census line of §4.3 and §4.5); MD12 and MD13 (5,638 rows, all interior, every `dungeon` of class 691, every census line of §5); MD16 and MD18 (the class counts of §2.3, the route destination and mode classes, the 18,878/3,058/234 actions, the whole §8.4.2 pattern table, the costs, and all 73 pads resolving). All 10,731 non-link row texts also resolve in the English string tables. Issues listed below the header table. |
| **Implements into** | `crates/formats` (decoders for the three singleton tables of §3–§5), `crates/world` (place lookup §3.4, map-note index §4.6, travel and housing graphs §8–§9), `apps/explorer` (world-map overlays: places, notes, links, travel routes, neighbourhoods), `apps/client` (walker place lookup), `crates/net` sandbox backend (travel and housing data for a local server) |
| **Companion specs** | [`records.md`](records.md) (**RC**: §1.2 VLE, §1.3 TSize and bucket order, §1.4 strings, §1.5 name hash, §2.1 DID table, §4 StringInfo); [`property-system.md`](property-system.md) (**PS**: §4.6 positions, §4.8 StringInfo, §5 property sets and list/table entry forms, §6.2 the +0x09000000 rule, §7.3 bit-to-key rule, §8.1 DID maps, §17.4 StringInfo reader); [`wstate.md`](wstate.md) (**WS**: object-state header and root class, §11 areas, territories, regions, dungeons); [`landblock.md`](landblock.md) (**LB**: §4.6 links, §6 landblock properties and minimap tiles); [`entity-descriptor.md`](entity-descriptor.md) (**ED**: SED types, §4.5.1 content layers); [`game-panels.md`](game-panels.md) (**GP**: §8 minimap, §9 world map, map georeference); [`coordinates.md`](coordinates.md) (**CO**: §5–§8 frames and map coordinates, §9 `/loc` text); [`string-table.md`](string-table.md) (**ST**: §9 rendering a StringInfo) |

> **Review issues (2026-10-07), as raised by the first reviewer (all resolved; accepted on re-review, see the review record above):**
> 1. **§4.4 and §4.6, "valid position" is not defined.** Step 1 of §4.4 and the filing rule for a
>    link row's `link_position` both depend on it, and so does MI8. State the test (if it is the
>    four-part test of CO §4, cite it) or add an OPEN.
> 2. **§4.4, "`text` does not render to a valid string" is ambiguous.** It could mean a null
>    reference, a token missing from the active locale's table, an empty result, or an empty
>    literal. Say which, or what is known from BIN, and mark the rest OPEN. MI8 depends on it.
> 3. **§3.2, the client's debug-dump labels are quoted.** The two quoted words are strings taken
>    from the client binary and are not needed for interoperability. Paraphrase them, for
>    example: "the client's debug dump of the table names the first field as the area and the
>    second as the dungeon".
> 4. **§4.2, no units for `min_range` and `max_range`.** §5.3 requires units. State them, or add
>    an explicit OPEN for the unit (the §4.6 OPEN covers only what the values limit).
> 5. **§3.3 and §3.4, lower-casing of code units outside ASCII is not specified.** BIN q3 says each
>    code unit is lower-cased, but not by which mapping (ASCII only, Latin-1, or full UTF-16).
>    State it, or mark it OPEN. A reader of other installs, or of hostile input, needs the rule
>    to place and find keys.
> 6. *(minor)* The sentence in §3.4 about "one further routine … note-shaped object … its caller
>    was not traced" describes the binary's internals, not a behaviour an implementer can use.
>    Rephrase it as a functional fact or drop it. The provenance legend defines a
>    **Probable** mark that no row uses; remove it or use it.
>
> **Resolution (spec writer, revision of 2026-10-07; awaiting re-review):**
> 1. §4.4.1 now states the position test (the four tests of CO §4, finite x, y, z, and a finite
>    orientation of unit length within 0.0002), from BIN q6; §4.6 says that a link row needs both
>    its own position and `link_position` to pass it. Checked on every position of the three
>    tables (MD24).
> 2. §4.4.2 now states the text test from BIN q7: a literal always passes, even an empty one; a
>    reference needs a non-zero token and table, the table in the active locale and the token in
>    it, and each argument must pass its own check. The text is never rendered, so an entry with
>    empty text passes (87 rows here). The per-kind argument rule is OPEN (MD25).
> 3. §3.2 paraphrases the debug dump.
> 4. §4.2 gives the unit (world units, CO §3); new §4.7 gives the radar's range test from BIN q9
>    (MD27).
> 5. New §3.3.1 states what BIN q8 shows: each code unit is lower-cased on its own by the C
>    runtime's locale-dependent mapping, under the locale the client selects at start-up from the
>    game language; the full 16-bit unit enters the hash; a code unit 0 ends the key. The exact
>    mapping of non-ASCII units is OPEN and a Tessera rule is given (MD26).
> 6. The §3.4 sentence is dropped. The unused **Probable** mark is removed from the legend.

> **Naming note (interoperability).** Property names (`MapNote_…`, `Travel…`, `House_…`,
> `Neighborhood…`, `HousingHook_…`, `UI_Map_…`, …), DID-map labels (`PlacesTable`, `MapNotes`,
> `DungeonQuestMapNotes`, `TravelWebDirectory`, …) and enum labels (`Link`, `Quest`, …) are
> identifiers read from the user's own data (the property dictionary 0x34000000, DID maps, enum
> mappers). They are used only to find records and properties at run time. Field names in the
> byte layouts below are Tessera's own. Class indices, DIDs and counts are this install's values
> (2026-10-06/07); they are observations, never constants. No game text, place name or other
> string content from the data is reproduced here.

---

<a id="purpose"></a>
## Purpose

This spec lets an implementer read the data behind the world map and the travel and housing
systems:

- the three **singleton tables** under DID prefix 0x0E that the client loads for maps: the
  named-places table (§3), the map-note table (§4) and the dungeon quest-note table (§5);
- the **map-note properties** of game objects and how the table rows relate to them (§6);
- the map links from areas and dungeons to their map images (§7, mostly by reference);
- the **travel network**: travel-web records, destinations, routes, route actions, travel
  modes, the stable-master NPCs and the directory singleton (§8);
- **neighbourhoods and housing**: neighbourhood instances, templates, house lists, houses,
  gates, housing hooks and the housing-control singleton (§9);
- every property whose string value names a place of §3 (§10), and the RIPCORDS records (§11).

With it the explorer can draw named places, landmarks, map notes, dungeon links, travel routes
and neighbourhood boundaries on the maps of GP §9; the walker can resolve a place name to a
position and name the area or dungeon it lies in (with WS §11 and CO §9); and a local server
can offer travel (routes, costs, destinations) and housing instances. Positions use the
encoding of PS §4.6 throughout. Gameplay rules that live only on the official server
(prices actually charged, discounts, quest gating) are out of reach of data and binary
analysis and are marked OPEN or Reported.

---

<a id="provenance"></a>
## Provenance

| Tag | Source |
|---|---|
| OBS | Observed in the user's install (Steam, data current on 2026-10-06/07) with throwaway read-only probes in the dirty-room scratch area, using the repository's decoders as tools. **Full censuses:** every byte of 0x0E000004, 0x0E000006 and 0x0E400000 (all rows decoded, zero bytes left over); every 0x70 record header (root class) and every 0x79 property set of the classes named in this spec, and of every 0x70 and 0x78 set when looking for references to them (inbound-reference scan over all 383,332 object property sets and 1,616 stand-alone sets, plus the bundles of all object-state records); all 49,636 landblock-info records (SEDs, links, weenies); the DID maps UNIQUEDB 0x28000002, WEENIECONTENT 0x2800000E, CONTENT_LAYER 0x28000012 and RIPCORDS 0x2800002C; the enum mappers named in §6.3 and §9. |
| BIN | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (SHA-256 prefix d22363eb9aff0fb5, the build of the other world specs), read in the dirty room on 2026-10-07 in a copy of the shared analysis project, by address range. **Questions:** (q1) the stored layout of a map-note row: field order and identity of the three DIDs, whether the byte after the type is padding or a value, the exact condition for the link block, and when the trailing property set is read; (q2) which rows the client rejects as invalid, and how it indexes rows after loading; (q3) the places-table reader and lookup: entry order, the key used for hashing, case handling, and which DID the two u32 fields are; (q4) the dungeon quest-note row and table readers and how rows are grouped; (q5) how the client finds the three tables. **Why M1–M3 were not enough:** M1 decodes every row exactly but cannot tell padding from an always-zero value, `type = 1` from "bit 0 set" (no row has bit 0 with another bit), "area" from "dungeon" in the places table (both fields are equal in every record), or the client's validity and indexing rules (they leave no trace in data); no published source states them; M3 cannot isolate them. **Read:** the map-note base, row and quest-row serializers and their reset and validity routines, the two table serializers, the places-table entry serializer, its two name lookups and its debug dump, the map-note manager's two load routines, its add-note routine and its per-zone list lookup. No network, login, launcher or anti-tamper code. **Second use, 2026-10-07 (revision after review, same build, same method, by address range):** (q6) what the client's "valid position" test checks, for the row's own position and for `link_position`; (q7) what its "valid text" test of a map-note row checks; (q8) which lower-case mapping the places-table lookup applies to code units, and under which locale; (q9) the unit of `min_range` and `max_range` and the test that uses them. **Why M1–M3 were not enough for (q6)–(q9):** every position, text and key in the data already passes or is ASCII (no invalid position, no unresolved text, no non-ASCII key), so the data cannot show the rule or its edge cases; no published source states them; the owner's in-game observations cannot isolate them. **Read (second use):** the position class's validity routine and the orientation check it calls, the StringInfo validity routine, the start-up routine that selects the C-runtime locale from the game language, the map-note manager's ranged-note query and the radar's note-update routine with the range predicate it calls. The meaning of the C-runtime functions named by import is taken from their public documentation. |
| LC | LotroCompanion `lotro-tools` (places, map-note, quest-note, travel and housing loaders), read for facts only. Marked **Reported** where only LC supports a fact. |
| RC, PS, WS, LB, ED, GP, CO, ST | The companion specs listed in the header. |

Marks: **Confirmed** = checked over every record, row or value of the kind (OBS), or a BIN fact
that the data agree with in every case; **BIN** = rests on the disassembly only; **Reported** =
third-party only.

| # | Fact | Sources | Mark |
|---|---|---|---|
| MD1 | The client finds the places, map-note and dungeon quest-note tables through UNIQUEDB entries (keys 0x9E, 0x1000001F, 0x1000000F; labels `PlacesTable`, `MapNotes`, `DungeonQuestMapNotes`) and loads them as DB types 0x19, 0x44, 0x2D (§2.1) | BIN (q5: the three loaders pass key, root key 0xF and type); OBS (the three UNIQUEDB entries name 0x0E000004, 0x0E000006, 0x0E400000; RC §2.1 type labels) | Confirmed |
| MD2 | Places table layout: DID, TSize, then entries {UTF-16 name, position, u32 area slot, u32 dungeon slot} (§3.1–§3.2) | OBS (9,025 entries, 0 bytes left); BIN (q3: entry serializer order and field identity) | Confirmed |
| MD3 | Places-table entries are in bucket order of the RC §1.5 hash of the **lower-cased** name modulo *B*[hint] (§3.3) | OBS (9,024 of 9,024 consecutive pairs; the hash of the name as stored fails on 165 pairs); BIN (q3: the lookup lower-cases each code unit before hashing) | Confirmed |
| MD4 | Place lookup is case-insensitive (mapping in MD26); one lookup returns the position, another the area slot (§3.4) | BIN (q3, q8) | BIN |
| MD5 | Place census: names, positions, area/dungeon slots (§3.5) | OBS | Confirmed |
| MD6 | Map-note table layout: DID, u32 row count, rows; the reader fails when the count exceeds the remaining bytes (§4.1) | OBS (13,485 rows, 0 bytes left); BIN (q1, q4: the table serializers) | Confirmed |
| MD7 | Map-note row layout, field identities, the boolean byte, the link block read when bit 0 of `type` is set, the trailing property set (§4.2–§4.3) | OBS (every row decodes exactly; link block present in exactly the 2,754 rows with bit 0); BIN (q1) | Confirmed (layout); BIN (byte checked as boolean, version gate) |
| MD8 | Validity rules: a valid position (MD24); `type` ≠ 0; for non-link rows a valid text (MD25), a non-zero level and `min_range` not above `max_range` by more than 0.0002 (§4.4) | BIN (q2, q6, q7); OBS (all rows satisfy them) | Confirmed |
| MD9 | Row census (§4.5) | OBS | Confirmed |
| MD10 | After loading, the client files each row by dungeon, else by (region, landblock) when either range is zero, else in a list of ranged notes; link rows whose target is non-zero and not their own dungeon, and whose two positions are valid, also register an entry under their target zone (§4.6) | BIN (q2, q6); OBS (2,590 of the 2,754 link rows qualify; the other 164 lead into their own dungeon) | BIN (rule); Confirmed (counts) |
| MD11 | Rows repeat the map-note properties of their entity, with a few differences; the content-layer list comes from the placement, not the entity (§6.2) | OBS | Confirmed |
| MD12 | Dungeon quest-note layout: rows of base fields, layer property and a property set (§5) | OBS (5,638 rows, 0 bytes left); BIN (q4) | Confirmed |
| MD13 | Every dungeon quest-note row lies in an interior cell and names a dungeon; the client groups rows by dungeon (§5.3) | OBS; BIN (q4) | Confirmed (data); BIN (grouping) |
| MD14 | Map-note property definitions and the bit-to-key labels of `MapNote_Type` and `MapNote_Level` (§6.1, §6.3) | OBS (dictionary, enum mappers 0x230000D7/0x230001A4 and 0x230000D6/0x230001A3) | Confirmed |
| MD15 | Dungeon and area map-data links: 1,300 stand-alone sets with `UI_Map_GameMap`, all reached from dungeons (and one area) (§7) | OBS | Confirmed |
| MD16 | Travel graph: classes, properties, reference targets and counts (§8.1–§8.7) | OBS (all records of the classes; inbound-reference scan) | Confirmed |
| MD17 | `TravelSegment_HeadName` names the `Segment_HeadName` of a SegmentNode SED (type 19) in landblock info (§8.5.1) | OBS (2,185 of 2,185 distinct values found on SegmentNode SEDs; none on links or other SED types) | Confirmed |
| MD18 | Route action patterns and their relation to swift travel (§8.4.2) | OBS | Confirmed |
| MD19 | `TravelDestinationCost` is a money amount in the smallest coin unit; route requirements use the generic `Usage_…` properties | LC | Reported |
| MD20 | Neighbourhood and housing graph: classes, properties, counts, references from landblock info (§9) | OBS | Confirmed |
| MD21 | House price and upkeep = type price (or upkeep) from the housing-control record × the house's value-tier modifier (§9.7) | LC | Reported |
| MD22 | Properties whose string values are place keys (§10) | OBS (every string value of every 0x79 and 0x78 set tested against the table) | Confirmed |
| MD23 | RIPCORDS map and records (§11) | OBS | Confirmed |
| MD24 | The client's valid-position test: region ≠ 0, landblock X ≠ 0xFF and Y ≠ 0xFF, cell ≠ 0xFFFF (the tests of CO §4), x, y, z finite, orientation components finite and \|w² + x² + y² + z² − 1\| ≤ 0.0002 in f32; applied to a map-note row's position (§4.4) and, for the link entry of §4.6, to both the row's position and `link_position` (§4.4.1) | BIN (q6); OBS (all 13,485 row positions, 2,754 link positions, 9,025 place positions and 5,638 quest-note positions pass; the largest \|w² + x² + y² + z² − 1\| is 2.4 · 10⁻⁷) | Confirmed (rule BIN; every stored position passes) |
| MD25 | The valid-text test of a non-link row: a literal passes whatever its content; a reference passes when token ≠ 0, table ≠ 0, the table exists in the active locale and holds the token, and every argument passes its own check; the text is not rendered (§4.4.2) | BIN (q7); OBS (all 10,731 non-link texts are references that resolve in the English tables, 87 of them to an entry with empty text; none has arguments) | Confirmed (rule BIN; every row passes) |
| MD26 | Places lookup lower-cases each UTF-16 code unit on its own with the C runtime's locale-dependent mapping, under the locale the client sets at start-up from the game language, adds the full 16-bit lowered unit into the hash, compares names case-insensitively under the same locale, and stops at a code unit 0 (§3.3.1) | BIN (q8; C-runtime behaviour from its public documentation); OBS (all 296,009 code units of the 9,025 names are below 0x80, none is 0) | BIN (ASCII part Confirmed by MD3) |
| MD27 | `min_range` and `max_range` are distances in world units; the radar hides a note whose two ranges both exceed 0.0002 in magnitude when its straight-line distance from the radar's reference position is above `max_range` or below `min_range` (§4.7) | BIN (q9) | BIN |

---

<a id="s1"></a>
## 1. Conventions

- Byte layouts use the primitives of RC §1: little-endian integers, `f32`, VLE (RC §1.2),
  TSize (RC §1.3), UTF-16 string = VLE code-unit count then UTF-16LE units (RC §1.4).
- **Position** means the flag-byte position value of PS §4.6, read with the reader rule of
  PS §4.6.1, wherever it appears (in a property or directly in a table row).
- **Property** (inline, outside a set) means the list form of PS §5.2: a u32 PID followed by the
  value decoded by that PID's dictionary type; a PID of 0 has no value.
- **Property set** (embedded) means PS §5.1 without the leading DID: a TSize then table entries.
- **StringInfo** means PS §17.4's stored form. "Null text" is a reference with token 0 and table
  0 (PS §4.8).
- A **class** is the root class at offset 4 of a 0x70 record (WS §3.1). Records are named by
  their kind and, for this install, their class index; implementers identify kinds by the
  properties listed, not by index (WS §11.1 explains why).
- **Properties of a 0x70 record** are those of its 0x79 set (PS §6.2).
- "Zone" means an area (class 690 here) or a dungeon (class 691), WS §11.1.

<a id="s2"></a>
## 2. Finding the records

<a id="s2-1"></a>
### 2.1 The three singleton tables (Confirmed, MD1)

| Table | UNIQUEDB key | Label | DB type (RC §2.1) | DID here | Payload here |
|---|---|---|---|---|---|
| Named places | 0x9E | `PlacesTable` | 0x19 | 0x0E000004 (general) | 953,316 bytes |
| Map notes | 0x1000001F | `MapNotes` | 0x44 | 0x0E000006 (general) | 1,469,008 bytes |
| Dungeon quest notes | 0x1000000F | `DungeonQuestMapNotes` | 0x2D | 0x0E400000 (general) | 1,037,249 bytes |

UNIQUEDB is the root DID map's key 0xF (PS §8.1; 0x28000002 here). The client asks for each
table by the UNIQUEDB key and the DB type (BIN q5). A reader looks the DID up in UNIQUEDB at run
time (by key or by label; both are stable identifiers in this install) and never hard-codes it.
UNIQUEDB also names 0x0E000002 (`SegmentTable`), which does not exist in this install.

<a id="s2-2"></a>
### 2.2 Directory singletons

Found through WEENIECONTENT (root key 0xA, PS §8.1) by label:

| Label | DID here | Class here | Content |
|---|---|---|---|
| `TravelWebDirectory` | 0x7000025A | 1509 | `TravelWebArray`: travel-web records (§8.7) |
| `NeighborhoodDirectory` | 0x7000B012 | 1954 | `NeighborhoodDirectory_NeighborhoodArray`: neighbourhoods (§9.1) |
| `HousingControl` | 0x7000022C | (singleton) | Housing prices, upkeep, limits (§9.7) |

<a id="s2-3"></a>
### 2.3 Kinds covered, and how to recognise them

| Kind (Tessera's name) | Identifying properties | Class here | Records |
|---|---|---|---|
| Travel-web record | `TravelHomeLocation`, `TravelDestinationRecordArray` | 1508 | 421 |
| Travel destination | `TravelDisplayName`, `TravelDestinationIsSwiftTravel` | 1499 | 768 |
| Travel route | `TravelRoute_Destination`, `TravelRoute_ActionArray` | 1505 | 5,008 |
| Travel segment | `TravelSegment_HeadName` or `TravelSegment_SubSegmentArray` | 1506 | 2,223 |
| Travel hop | `TravelHop_Delay` | 1501 | 11 |
| Travel pad | `TravelPad_Location` (and no other property) | 1502 | 73 |
| Travel mode | `TravelMode_PathNodeDistance` | 2130 | 8 |
| Travel versioning | `TravelVersioning_DestinationPairArray` | 2191 | 1 |
| Landmark (map-note entity) | `MapNote_Type`, `Landmark_Music` | 1210 | 2,733 |
| Neighbourhood | `Neighborhood_NeighborhoodTemplate` | 1185 | 3,248 |
| Neighbourhood template | `NeighborhoodTemplate_HouseList` | 1956 | 14 |
| House list | `HouseList_HouseArray` | 1952 | 14 |
| House | `House_NeighborhoodTemplate`, `House_Type` | 1181 | 227 |
| Neighbourhood gate | `NeighborhoodGate_NeighborhoodTemplate` | 1955 | 13 |
| Housing hook | `HousingHook_ID` | 1989 | 744 |
| Ripcord | `RipCord_WorldProperty` | 2801, 3166 | 341, 7 |
| Area, territory, region, dungeon | WS §11.1 | 690, 692, 180, 691 | 675, 86, 5, 1,303 |

Travel segments, hops and pads share the abstract ancestor class 1497 (WS §3.1 chain
1497 → 486 → 89 → 485 → 484). Landmarks and housing hooks are entity classes (chain 1733 → 787
→ 1726 → 488 → 223 → 89 → 485 → 484). Every other kind above derives from 486: directly, or
through 1620 → 1621 (travel versioning) or through 2801 (ripcord class 3166).

<a id="s2-4"></a>
### 2.4 Reference graph (Confirmed, OBS)

Every edge below is a DID-valued property (or a place-name string, marked "name") checked over
all records. "All" means every value of that property pointed to a record of the target kind.

| From | Property | To | Values |
|---|---|---|---|
| `TravelWebDirectory` | `TravelWebArray` / `TravelWebWC` | travel-web record | 375 |
| NPC (class 1724) | `TravelWebWC` | travel-web record | 437 |
| travel-web record | `TravelHomeLocation` | destination | 421 |
| travel-web record | `TravelHomeLocationsArray` / `TravelHomeLocation` | destination (775) or route (26) | 801 |
| travel-web record | `TravelDestinationRecordArray` / `TravelDestinationRecord` / `TravelDestinationRoute` | route | 5,199 |
| route | `TravelRoute_Destination` | destination | 5,008 |
| route | `TravelRoute_TravelMode` | travel mode | 5,008 |
| route | `TravelRoute_ActionArray` / `TravelRoute_Action` | segment (18,878), hop (3,058), pad (234) | 22,170 |
| segment | `TravelSegment_SubSegmentArray` / `TravelSegment_SubSegment` | segment | 135 |
| segment | `TravelSegment_HeadName` (name) | SegmentNode SED in landblock info (§8.5.1) | 2,191 |
| hop | `TravelHop_SceneID` | scene record (class 1468) | 9 |
| pad, NPC | `TravelPad_Location` (name) | place (§3) | 73, 432 |
| travel mode | `TravelMode_TravelEffectDID` | effect (class 734) | 7 |
| travel versioning | `TravelVersioning_SlowDestination`, `…_SwiftDestination` | destination | 11 + 11 |
| race table (860), advanced-start table (3335) | `Travel_PlayerHomeLocationArray` / `Travel_PlayerHomeLocation` | destination | 25, 162 |
| `NeighborhoodDirectory` | `NeighborhoodDirectory_NeighborhoodArray` / `…_Neighborhood` | neighbourhood | 3,248 |
| neighbourhood | `Neighborhood_NeighborhoodTemplate` | template | 3,248 |
| template | `NeighborhoodTemplate_HouseList` | house list | 14 |
| template | `NeighborhoodTemplate_SceneID` | scene record (class 1468) | 14 |
| template | `NeighborhoodTemplate_Telepad`, `…_BootPosition` (name) | place | 14 + 14 |
| house list | `HouseList_HouseArray` / `HouseList_House` | house | 227 |
| house | `House_NeighborhoodTemplate` | template | 227 |
| house | `House_CharacteristicArray` / `House_Characteristic` | trait records (classes 1477, 1483) | 471 |
| house | `House_Telepad` (name) | place | 227 |
| gate | `NeighborhoodGate_NeighborhoodTemplate` | template | 13 |
| gate | `NeighborhoodGate_Telepad` (name) | place | 13 |
| NPC | `HousingSystem_PurchaseTemplateDID` | template | 21 |
| landblock-info weenie | `HouseKeeper_HouseDID` | house | 454 (+3 on SEDs) |
| landblock-info weenie | `HousingSystem_NeighborhoodGateDID` | gate | 43 |
| landblock-info SED type 6 | the SED's object DID | housing hook | 32,338 SEDs |
| quest (class 1398) | `QuestEvent_LandmarkDID` (in objective and loot structures) | landmark | 3,419 |
| map-note row (§4) | entity slot | landmark, NPC, item and other entity classes | 13,485 |
| area | `Area_Territory` → territory `Area_Region` → region | WS §11.2 | 675, 86 |
| dungeon | `Dungeon_MapData`, `Dungeon_MiniMapData` | 0x78 set with `UI_Map_GameMap` | 1,303 + 1,303 |

The object-state bundles (WS §5) of these records repeat the same lists as `array_u32` or
`map_u32` natives (for example the 18,878 route-to-segment references), so a reader can build
the graph from the property sets alone. Landmarks are not placed by landblock info: no SED,
link or weenie names a landmark record. Their positions exist only in the map-note table (§4).

---

<a id="s3"></a>
## 3. Named places (0x0E000004, `PlacesTable`)

A hash table from a place key (a short identifier string) to a position and the zone the
position lies in. Many records name places by key (§10): travel pads, house and neighbourhood
entrances, recall and rally points, instance entry and exit points.

<a id="s3-1"></a>
### 3.1 Record layout (Confirmed, MD2)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | DID | Own DID |
| 4 | 1–6 | TSize | Hint byte and VLE entry count *n* (RC §1.3) |
| … | var | entry × *n* | §3.2, in bucket order (§3.3) |
| end | | | No bytes remain |

The client's TSize reader rejects a hint above 22 (as PS §17.2 states for every TSize).

<a id="s3-2"></a>
### 3.2 Entry layout (Confirmed, MD2)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `name` | UTF-16 string | The place key. Compared case-insensitively (§3.4). |
| 2 | `position` | Position | Where the place is (PS §4.6 frames) |
| 3 | `area` | u32 DID | Area slot: the zone DID the client reports as the place's area (BIN q3). 0 = none. |
| 4 | `dungeon` | u32 DID | Dungeon slot (BIN q3: the client's debug dump of the table names the first of the two fields as the area and the second as the dungeon). 0 = none. |

In this install `area` = `dungeon` in all 9,025 entries, and the value is 0 (6,836), a dungeon
record (2,071) or an area record (118). So the pair is "the zone of the place", written twice.

<a id="s3-3"></a>
### 3.3 Order of entries (Confirmed, MD3)

Let *B* be the bucket count of the hint (RC §1.3; hint 10 → 12,281 here). For each entry,
compute the RC §1.5 name hash over the UTF-16 code units of the name **after lower-casing each
unit** (§3.3.1). Each lowered unit takes the place of the byte *c* of RC §1.5 with its full
16-bit value, not truncated to 8 bits (BIN q8); for the ASCII names of this install the two
readings agree. The entries are stored with (hash mod *B*) non-decreasing. Entries of one bucket keep their stored
order when loaded (BIN q3: the loader appends an entry after the previous one when both fall in
the same bucket). A writer that adds places sorts by bucket and keeps the original hint for a
byte-exact copy, as RC §1.3 says.

<a id="s3-3-1"></a>
#### 3.3.1 Lower-casing and key end (BIN, MD26)

What the client does (BIN q8):

1. **Per code unit.** Each UTF-16 code unit is lower-cased on its own. Surrogate pairs are not
   combined, and no unit becomes more than one unit.
2. **Mapping.** The client uses the C runtime's wide-character lower-case function for the hash
   and the runtime's case-insensitive wide-string comparison for matching names (§3.4). Both
   follow the runtime's current locale. At start-up the client sets the runtime locale, all
   categories, from the game language: an English, UK-English, French or German locale, English
   for any language code it does not list. Under such a locale the mapping of non-ASCII units is
   the operating system's lower-casing for that locale. In every case the units A–Z (0x41–0x5A)
   map to a–z (0x61–0x7A) and every other ASCII unit is unchanged.
3. **Key end.** The hash and the comparison both stop at the first code unit 0. A stored name
   that contains a unit 0 is therefore keyed only by the units before it. No name in this install
   contains a unit 0; five contain one unit 0x1F, which lower-casing leaves unchanged.

**Tessera rule** (for keys outside ASCII, which this install does not have): map A–Z to a–z;
map any other code unit to the Unicode simple lower-case mapping of that unit when the result is a
single unit in the Basic Multilingual Plane, otherwise leave it unchanged; leave surrogate units
unchanged. Treat a unit 0 as the end of the key, as the client does, and warn when a stored name
contains one. A reader that finds a non-ASCII name out of bucket order (MI2) under this rule
reports a warning, not malformed input, because the writer's mapping may differ.

> **OPEN:** the exact lower-case table for non-ASCII code units under each of the client's
> locales (it belongs to the operating system or its compatibility layer, not to the data). Only
> keys with non-ASCII units are affected.

<a id="s3-4"></a>
### 3.4 Lookup (BIN, MD4)

To find a place by key *k*:

1. Compute the hash of §3.3 over *k* lower-cased (§3.3.1), up to its first unit 0.
2. Take the bucket (hash mod *B*) and compare *k* with each entry name of that bucket,
   ignoring case under the same mapping (§3.3.1; ASCII case folding is enough for every key in
   this install).
3. The first match is the place. No match means "unknown place": the client's lookups then
   return failure, and the caller keeps its own default.

The client has two lookups: one returns the position, one returns the `area` slot. Readers
that do not need the client's hash table may use any case-insensitive map from key to entry
(with the mapping of §3.3.1); keys are unique under case folding (§3.5).

<a id="s3-5"></a>
### 3.5 Census (Confirmed, MD5)

- *n* = 9,025, hint 10; no trailing bytes.
- Names: 4–69 code units (296,009 in all), all below 0x80 and none 0; five names contain one
  unit 0x1F each; 168 contain upper-case letters; no two names are equal ignoring case.
- Position flag bytes (PS §4.6): 0xBB 3,574; 0xFB 2,015; 0x3B 1,099; 0x9B 758; 0x7B 663;
  0xDB 348; 0x1B 312; 0x5B 256. Every position has region, landblock, cell and x/y/z; 7,351
  have an orientation; none has an instance. All 9,025 follow the flag rules of PS §4.6.
- Regions: 1 (3,282), 2 (2,917), 5 (1,039), 4 (944), 3 (668), 6 (109), 14 (63), 15 (3).
- Interior cell (cell ≠ 0): 2,330. By zone slot: outdoors 0 (6,655) or an area (40); indoors a
  dungeon (2,071), an area (78) or 0 (181).

---

<a id="s4"></a>
## 4. Map notes (0x0E000006, `MapNotes`)

One row per placed map note: a landmark, NPC, door, item or other entity that the maps mark,
with its position, zone, text, icon class and visibility level. Rows whose type is `Link` are
doors and portals between zones (§4.3).

<a id="s4-1"></a>
### 4.1 Record layout (Confirmed, MD6)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | DID | Own DID |
| 4 | 4 | u32 | Row count *n* (a plain u32, not a TSize) |
| 8 | var | row × *n* | §4.2 |
| end | | | No bytes remain |

The client fails the record when *n* exceeds the number of bytes left (BIN q1/q4); a reader
applies a tighter bound, *n* ≤ remaining / 53: the smallest possible row (§4.2) has a one-byte
position, a null `layers` property (4 bytes), an empty literal `text` (5 bytes), no link block
and an empty `extra`, 1 + 12 + 4 + 5 + 17 + 12 + 2 = 53 bytes.

<a id="s4-2"></a>
### 4.2 Row layout (Confirmed layout, MD7)

| Order | Field | Type | Present | Meaning |
|---|---|---|---|---|
| 1 | `position` | Position | always | Where the note is |
| 2 | `area` | u32 DID | always | The area (or, in 74 rows, a dungeon) the note lies in |
| 3 | `dungeon` | u32 DID | always | The dungeon the note lies in; 0 outdoors |
| 4 | `entity` | u32 DID | always | The 0x70 record the note marks (§6.2) |
| 5 | `layers` | property | always | Content-layer list: in every row PID 0x4CE `Entity_ContentLayers`, an array of `Entity_ContentLayer` enum values (ED §4.5.1) |
| 6 | `text` | StringInfo | always | Label; null text in link rows |
| 7 | `icon` | u32 DID | always | Icon override (0x41 surface) or 0 |
| 8 | `level` | u32 | always | `MapNote_Level` bitfield (§6.3) |
| 9 | `type` | u64 | always | `MapNote_Type` bitfield (§6.3) |
| 10 | `flag_b` | u8 | always | A boolean. The client's reader fails on a value above 1 (BIN). 0 in every row. |
| 11 | `link_position` | Position | only if bit 0 of `type` is set | Where the link leads |
| 12 | `link_target` | u32 DID | only if bit 0 of `type` is set | The zone the link leads into |
| 13 | `min_range` | f32 | always | `MapNote_MinRange`: a distance in world units (CO §3), compared with a straight-line distance (§4.7) |
| 14 | `max_range` | f32 | always | `MapNote_MaxRange`: a distance in world units (CO §3), as `min_range` |
| 15 | `discovery` | u32 | always | Discovery key: the `MapNote_Discoverable` enum value of the note (§6.2); 0 = none |
| 16 | `extra` | property set | see below | Game-specific properties (§4.5) |

`extra` is read only when the stream version the client associates with this record kind is
2 or more (BIN q1); otherwise the row ends after `discovery`. Every row of this install has
`extra` (all rows decode exactly with it), so readers always read it.

> **OPEN:** (a) The meaning of `flag_b` (BIN: stored as a boolean; never 1 here). (b) Where the
> client takes the version that gates `extra` for records read from the archives; only the
> "present" case occurs in data.

<a id="s4-3"></a>
### 4.3 Link rows (Confirmed, MD7)

A row is a link row when **bit 0** of `type` is set (BIN q1: the reader and the validity check
test that bit). In this install link rows have `type` = 1 exactly (2,754 rows), and in every one
of them `text` is null, `level`, `min_range`, `max_range` and `discovery` are 0, and the
`entity` is a door-like entity (classes 815 and 1722 here, which carry `MapNote_Linkable`).
`link_target` is a dungeon (2,685) or an area (69); `link_position` is an interior position
(flag bytes 0x3B 1,300; 0x7B 699; 0x1B 425; 0x5B 324) except 6 outdoor ones (0xFB 4, 0xBB 2).
In 412 link rows the target position lies in the same landblock as the row.

<a id="s4-4"></a>
### 4.4 Validity (BIN, Confirmed by data, MD8)

The client reports a row as invalid when:

1. its `position` fails the position test of §4.4.1, or
2. `type` is 0, or
3. bit 0 of `type` is clear (not a link) and any of:
   - `text` fails the text test of §4.4.2,
   - `level` is 0,
   - `min_range` exceeds `max_range` by more than 0.0002 (that is, `min_range` − `max_range`
     > 0.0002, in f32).

Link rows skip the checks of step 3. Step 1 does not look at `link_position`; §4.6 does. All
13,485 rows here pass. A reader logs invalid rows and keeps them out of the map display.

<a id="s4-4-1"></a>
#### 4.4.1 Position test (BIN, Confirmed by data, MD24)

A position is valid when all of these hold (BIN q6). Absent fields take the values of PS §4.6
("absent fields") before the test, so a position stored without a cell fails test 4 and one
stored without an orientation has the identity, which passes test 6.

1. region ≠ 0;
2. landblock X ≠ 0xFF;
3. landblock Y ≠ 0xFF;
4. cell ≠ 0xFFFF;
5. x, y and z are each finite (neither NaN nor infinite);
6. the orientation's four components are each finite, and
   |w² + x² + y² + z² − 1| ≤ 0.0002, with the sum formed in f32 in the order w, x, y, z.

Tests 1–4 are the invalid-position tests of CO §4 (C17). The instance is not tested. In this
install every position of the three tables passes: 13,485 row positions, 2,754 link positions,
9,025 place positions and 5,638 quest-note positions; the largest |w² + x² + y² + z² − 1| is
2.4 · 10⁻⁷.

<a id="s4-4-2"></a>
#### 4.4.2 Text test (BIN, Confirmed by data, MD25)

The client checks `text` without rendering it (BIN q7). The text passes when:

- it is a **literal**: always, whatever its content, the empty literal included;
- it is a **reference**: the token is not 0, the table DID is not 0, the string table with that
  DID exists in the active locale's archives, the table holds the token, and every argument of
  the argument block (if any) passes its own check.

So a reference fails when it is null (token 0 or table 0, PS §4.8), when its table is missing,
or when the token is missing from the active locale's table. An entry whose text is empty still
passes: the result of rendering is never looked at. In this install all 10,731 non-link texts
are references with no arguments that resolve in the English tables; 87 of them resolve to an
entry with empty text, and pass.

> **OPEN:** the check applied to each argument kind (PS §17.4.1). No StringInfo in this install
> has arguments. **Tessera rule:** an argument passes when it is itself a StringInfo that passes
> this test, or any other kind of value.

<a id="s4-5"></a>
### 4.5 Census (Confirmed, MD9)

- *n* = 13,485; no trailing bytes.
- Position flag bytes: 0xBB 4,552; 0x9B 2,657; 0xFB 2,251; 0xDB 1,342; 0x3B 1,218; 0x7B 1,018;
  0x1B 247; 0x5B 200. All follow PS §4.6; no instance. Regions: 1 (4,811), 2 (4,119), 4 (1,731),
  3 (1,436), 5 (1,314), 14 (68), 6 (6).
- `area` is never 0: an area (13,411) or a dungeon (74). `dungeon` is 0 in all 10,802 outdoor
  rows; of the 2,683 indoor rows, 2,581 name a dungeon and 102 have 0.
- `entity`: classes 1724 (5,487 rows), 1210 (3,217), 815 (1,674), 796 (1,234), 1722 (1,080),
  804 (473), 1170 (320); 5,698 distinct entities, 1,031 of them in more than one row.
- `layers`: list lengths 0 (10,844), 1 (2,515), 2 (117), 3 (7), 5 (2). Of the 2,780 elements,
  1,574 are 1 (`InstanceZero`), 202 are 2 (`All`) and 1,004 are named layers (0x1000xxxx).
- `text`: references only (no literal); null in exactly the link rows; no argument table.
- `icon`: 0 in 13,474 rows, a 0x41 DID in 11.
- `level`: 0x10000 (7,501), 0xF0000 (3,203), 0 (2,754, the link rows), 0x20000 (14), 0xE0000 (9),
  0xA0000 (4).
- `type`: exactly one bit set in every row. Bit 0 (2,754), bit 1 (4), and bits 20–62 for the
  rest; the most common are bits 42 (2,458), 37 (1,697), 57 (1,123), 44 (1,049).
- `flag_b`: 0 in every row.
- (`min_range`, `max_range`): (0, 0) in 12,611 rows, (80, 800) in 811, (80, 640) in 23, eight
  other pairs in 40; `min_range` ≤ `max_range` everywhere.
- `discovery`: non-zero in 2,379 rows.
- `extra`: empty (hint 0, count 0) in 12,814 rows; one entry, `MapNote_GameSpecific` (a struct),
  in 671. Inside it: `WorldEvent_MapNoteStateArray` (663; arrays of `WorldEvent_MapNoteState`
  structs with a `MapNote_StringInfo` and world-event conditions), and rarely
  `Entity_ContentLayers`, `MapNote_Enabled`, `Usage_QuestRequirements`.
- Rows are not sorted by region or landblock (12,532 of 13,484 consecutive pairs ascend).

<a id="s4-6"></a>
### 4.6 How the client indexes rows (BIN, MD10)

When the map-note manager loads the table it copies each row and files it in exactly one place:

1. If `dungeon` ≠ 0: in the list of that dungeon.
2. Otherwise, if |`max_range`| ≤ 0.0002 or |`min_range`| ≤ 0.0002: in a table keyed by the
   position's region, then by the landblock key *X* · 65,536 + *Y* (landblock X and Y of the
   position).
3. Otherwise (both ranges non-zero): in one list of ranged notes.

After filing, a link row also gets an entry (the row and its `link_position`) in the list of
the zone `link_target` when all of these hold (BIN q2, q6): `link_target` is non-zero,
`link_target` differs from the row's own `dungeon`, the row's own `position` passes §4.4.1, and
`link_position` passes §4.4.1. So every zone knows the doors that lead into it. In this install
2,590 of the 2,754 link rows get such an entry; the other 164 lead into their own dungeon.

> **OPEN:** how the maps use the lists afterwards: which `level` bits each map level shows,
> how `type` bits select icons and filters (GP §8.8, `UI_Map_MapNoteFilterProperty`), whether
> `min_range` and `max_range` limit anything besides the radar (§4.7), how `discovery` hides
> undiscovered notes, and whether the content-layer test of
> ED §4.5.1 applies to `layers`. **Tessera rule** until settled: §14.1.

<a id="s4-7"></a>
### 4.7 Range test in the radar (BIN, MD27)

`min_range` and `max_range` are distances in world units (CO §3). The client's radar (GP §8)
applies them to the notes it places (BIN q9):

1. A note is **ranged** when |`min_range`| > 0.0002 and |`max_range`| > 0.0002. Other notes are
   not range-tested.
2. For a ranged note, the radar takes the straight-line three-dimensional distance *d* between
   the note's position and the radar's reference position, both expressed in one world frame
   (CO §5–§7).
3. The note is hidden when *d*² > `max_range`² or *d*² < `min_range`², computed in f32. So it
   shows when `min_range` ≤ *d* ≤ `max_range`, both ends included.
4. A shown ranged note is drawn at the rim of the radar, in the horizontal direction from the
   player to the note, and the radar passes the horizontal distance to the note's label.

With the stored values, ranged notes show between 80 and 800 units (811 rows), 80 and 640 units
(23 rows), and so on (§4.5). This ties in with §4.6 step 3: the notes with both ranges non-zero
are the ones kept in the separate list of ranged notes.

> **OPEN:** (a) which position is the radar's reference (the player's, or the radar view's
> centre when they differ); (b) whether the world map applies the same test.
> **Tessera rule:** use the player's position (the camera's in the explorer).

---

<a id="s5"></a>
## 5. Dungeon quest notes (0x0E400000, `DungeonQuestMapNotes`)

<a id="s5-1"></a>
### 5.1 Layout (Confirmed, MD12)

The record is laid out like §4.1: own DID, u32 row count, rows, nothing left over. A row is:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `position` | Position | Where the note is |
| 2 | `area` | u32 DID | Area, or 0 |
| 3 | `dungeon` | u32 DID | Dungeon the note lies in |
| 4 | `entity` | u32 DID | The quest-giving or quest-target entity |
| 5 | `layers` | property | `Entity_ContentLayers`, as in §4.2 |
| 6 | `quest_info` | property set | Quest-dispenser properties (§5.2) |

The first four fields and `layers` are the same base layout as §4.2 fields 1–5 (BIN q4: the
same base reader).

<a id="s5-2"></a>
### 5.2 `quest_info` (Confirmed)

Every set has one or two entries (5,460 and 178 rows; hint 0):

| Property | Rows | Value |
|---|---|---|
| `QuestDispenser_RoleConstraint` | 3,363 | Pascal string: a role identifier of the entity |
| `QuestDispenser_ActiveQuests` | 2,453 | Array of `QuestDispenser_QuestRole` structs: `QuestDispenser_Quest` (DID of a quest record, class 1398; 16,124 values) and `QuestDispenser_RoleName` (Pascal string; 16,121 values). Array lengths 1–146 (720 have 1). |

<a id="s5-3"></a>
### 5.3 Census and use (MD13)

- *n* = 5,638. Every row is indoors (flag bytes 0x3B 2,163; 0x1B 1,818; 0x7B 919; 0x5B 738;
  all with a non-zero cell) and every `dungeon` is a dungeon record. `area` is 0 in 5,248 rows,
  an area in 354, a dungeon in 36. Regions: 2 (1,958), 1 (1,657), 4 (1,074), 5 (561), 3 (381),
  14 (7).
- `entity` classes: 1724 (2,936), 1170 (1,718), 796 (321), 1723 (317), 1210 (301), 1510 (45);
  2,612 distinct, 471 in more than one row.
- `layers` lengths: 0 (3,099), 1 (2,520), 2 (10), 4 (1), 5 (8).
- The client files each row with a non-zero `dungeon` in that dungeon's list (BIN q4). Outdoor
  quest markers do not come from this table.

> **OPEN:** how the client decides which quest notes to show (quest state of the player and
> role matching are server-side state).

---

<a id="s6"></a>
## 6. Map-note properties of game objects

<a id="s6-1"></a>
### 6.1 Definitions (Confirmed, MD14)

| Property | PID here | Type (PS §3) | Enum / DB source |
|---|---|---|---|
| `MapNote_Type` | 0x288 | bitfield 64 | mapper 0x230000D7 (base 0x230001A4) |
| `MapNote_Level` | 0x100 | bitfield 32 | mapper 0x230000D6 (base 0x230001A3) |
| `MapNote_StringInfo` | 0x6C2 | StringInfo | |
| `MapNote_Icon`, `MapNote_OffScreenIcon`, `MapNote_MouseOverIcon`, `MapNote_SelectIcon` | 0x56, 0x75D, 0x75E, 0x75F | DID | |
| `MapNote_MinRange`, `MapNote_MaxRange` | 0x4AA, 0x40C | float | |
| `MapNote_Discoverable` | 0x1EB | enum | mapper 0x2300006F |
| `MapNote_Enabled`, `MapNote_Linkable`, `MapNote_VisibleExternally` | 0x5EB, 0x3AC, 0x5EA | boolean | |
| `MapNote_GameSpecific` | 0x757 | struct (open) | |

<a id="s6-2"></a>
### 6.2 Rows versus entity properties (Confirmed, MD11)

The table rows are a compiled form of the entities' map-note properties, placed. Comparison of
all 10,731 non-link rows with the 0x79 set of their `entity`:

- `MapNote_Enabled` is true on the entity in all 10,731.
- `type` equals `MapNote_Type` in 10,710 (21 differ); `level` equals `MapNote_Level` in 10,709
  (13 differ, 9 entities lack it); `text` equals `MapNote_StringInfo` in 10,499 (232 differ).
- Ranges equal `MapNote_MinRange`/`MapNote_MaxRange` in 2,916 rows, differ in 60; in 7,755 the
  entity has neither and the row has (0, 0).
- `icon` is 0 when the entity has no `MapNote_Icon` (10,720), and non-zero in 11 rows whose
  entity has none.
- `discovery` equals the entity's non-zero `MapNote_Discoverable` in 2,329 rows and differs
  in 50.
- No entity carries `Entity_ContentLayers`; the rows' layer lists come from the placement.
- Link rows' entities (2,754) carry no `MapNote_Type` except 7.

The row values are therefore authoritative for maps; a reader does not override them with the
entity's. Landmarks (class 1210) also carry `Landmark_Music` (enum) and the
`Landmark_OverrideMountedCombat_…` booleans, and are named as objective targets by quests
(`QuestEvent_LandmarkDID`, §2.4).

> **OPEN:** why the 21 + 13 + 232 + 60 + 50 rows differ from their entity (probably per-placement
> overrides in the world-building data), and which value the client shows if it ever reads the
> entity's properties.

<a id="s6-3"></a>
### 6.3 Bits and labels (Confirmed, MD14)

Bit *i* of `MapNote_Type` or `MapNote_Level` is enum key *i* + 1 (PS §7.3). Labels come from the
mappers of §6.1 at run time:

- `MapNote_Type`: key 1 (bit 0) is `Link`, key 2 (bit 1) `Quest`; keys 0x11–0x40 (bits 16–63)
  are the note categories of the derived mapper (47 labels, key 0x3E unlabelled: icon classes
  such as vendors, trainers, stable-masters, points of interest).
- `MapNote_Level`: keys 0x11–0x14 (bits 16–19) are the four map levels of the derived mapper
  (labels `World`, `RegionZoom`, `Game`, `Region` in key order). Rows use bit 16 alone (7,501),
  bits 16–19 (3,203), 17 alone (14), 17+18+19 (9), 17+19 (4).

---

<a id="s7"></a>
## 7. Area and dungeon map images (Confirmed, MD15)

Map images and their georeference are covered elsewhere: outdoor minimap tiles in LB §6,
the world-map tree in GP §9, dungeon maps in GP §8.5, names in WS §11. Facts for overlays:

- 1,300 of the 1,616 stand-alone property sets (0x78) hold `UI_Map_GameMap` (fields
  `UI_Map_MapImage`, `UI_Map_BlockOffsetX/Y`, `UI_Map_PixelOffsetX/Y`, `UI_Map_Scale`, and in most
  `UI_Map_FogOfWar` and `UI_Map_FogOfWar_Color`); 21 of them also carry `Landblock_LightMap`
  and 3 `Landblock_CeilingLightMap`.
- Every one of the 1,300 is the `Dungeon_MapData` or `Dungeon_MiniMapData` of a dungeon, or the
  `Area_MapData` of the one area that has a non-zero value. `Dungeon_MapData` =
  `Dungeon_MiniMapData` in 1,301 of 1,303 dungeons; 1,298 distinct sets are named by each.
- Areas also carry `Area_Icon` (0x41; 665 of 675), `Area_SceneArray` (scene records, class 1468),
  `Area_VisitedAreaType` and `Ambient_MusicType`.
- The CONTENT_LAYER DID map (0x28000012) names 0x78 sets too, but those hold world-state
  properties (`World_IsNight`, `World_PresentTimeOfDay`), not maps.

To place a note, place or link on a map, convert its position with the map's georeference
(GP §8.6.1 for dungeon and minimap images, GP §9.3 for world-map nodes; CO §8 for map
coordinates).

---

<a id="s8"></a>
## 8. Travel network

<a id="s8-1"></a>
### 8.1 Model

- A **travel-web record** is one station of the network (a stable or a swift-travel point).
  Its home destination is where it is; its destination records list the routes it sells and
  their costs.
- A **destination** is a named arrival point; it says whether it is a swift-travel destination.
- A **route** leads to one destination, in one travel mode, through a list of actions:
  **segments** (ride a path), **hops** (a scene or delay) and **pads** (arrive at a place).
- Stable-master NPCs name their station with `TravelWebWC` and stand at `TravelPad_Location`.

<a id="s8-2"></a>
### 8.2 Travel-web record (class 1508 here; Confirmed)

| Property | Type | Present | Value |
|---|---|---|---|
| `TravelHomeLocation` | DID | 421 of 421 | Destination record: the station itself. 399 distinct; some stations share one. |
| `TravelHomeLocationsArray` | array of `TravelHomeLocation` | 394 | Other destinations (775 values) or routes (26) treated as the same station |
| `TravelDestinationRecordArray` | array of `TravelDestinationRecord` structs | 421 | 1–40 routes per station, 5,199 in all |
| ↳ `TravelDestinationRoute` | DID | 5,199 | A route record |
| ↳ `TravelDestinationCost` | int | 5,199 | Price, 0–19,200 here (29 are 0) |
| ↳ `TravelDestinationMinLevel` | int | 0 | Defined as a field of the struct; never stored |

5,005 of the 5,008 routes are sold by at least one station (at most 13 stations sell one
route). 12 route entries lead to the station's own home destination.

<a id="s8-3"></a>
### 8.3 Destination (class 1499 here; Confirmed)

| Property | Type | Present | Value |
|---|---|---|---|
| `TravelDisplayName` | StringInfo | 768 | Name shown in travel lists |
| `TravelDestinationIsSwiftTravel` | boolean | 768 | true 395, false 373 |
| `TravelDestinationAllowPaperItems` | boolean | 768 | true 763 |
| `MapNote_Enabled`, `MapNote_Level`, `MapNote_StringInfo`, `MapNote_Type` | | 2 | Map-note properties on two destinations |

Destinations carry no position; where a destination is comes from the routes' last pad or
segment (§8.4) and from the stations whose home it is. 2,229 routes end at a destination that
is some station's home.

<a id="s8-4"></a>
### 8.4 Route (class 1505 here; Confirmed)

<a id="s8-4-1"></a>
#### 8.4.1 Properties

| Property | Type | Present | Value |
|---|---|---|---|
| `Name` | StringInfo | 5,008 | Route name |
| `TravelRoute_Destination` | DID | 5,008 | Destination |
| `TravelRoute_TravelMode` | DID | 5,008 | Travel mode (8 distinct, §8.6) |
| `TravelRoute_ActionArray` | array of `TravelRoute_Action` DIDs | 5,008 | 1–15 actions, in travel order |
| `Usage_MinLevel` | int | 4,516 | Minimum character level, 10–150 |
| `Usage_AllowedWhileMounted` | boolean | 4,733 | Always true |
| `Usage_RequiresSubscriberOrUnsub` | boolean | 2,745 | Always true |
| `Usage_QuestRequirements` | array of 1 `Usage_QuestRequirement` struct | 178 | `Usage_QuestID` (quest DID), `Usage_QuestStatus` (enum), `Usage_Operator` (enum 1, 3 or 6) |
| `Usage_RequiredFaction` | struct | 41 | `Usage_RequiredFaction_DataID` (faction record, class 1889), `Usage_RequiredFaction_Tier` (enum 4–6) |
| `Usage_Permission` | DID | 5,008 | One shared usage-permission record |
| `UI_Examination_Tooltip_…` | | 5,008 | Tooltip layout and element (same in all) |

The dictionary also defines `TravelRoute_NonSubscriberTemporaryServiceRequired` (enum), never
stored here.

<a id="s8-4-2"></a>
#### 8.4.2 Action patterns (Confirmed, MD18)

Writing S for a segment, H for a hop, P for a pad and S+ for two or more segments in a row, the
action lists are:

| Pattern | Routes | Swift destination | Slow destination |
|---|---|---|---|
| S H S | 2,685 | 2,661 | 24 |
| S+ | 1,946 | 0 | 1,946 |
| H P | 233 | 170 | 63 |
| S+ H S+ | 66 | 0 | 66 |
| H S | 61 | 61 | 0 |
| S | 6 | 0 | 6 |
| S+ H S, S H S+, S H P, S+ H S+ H S, S+ H S+ H S+ | 4, 4, 1, 1, 1 | 0 | all |

So swift travel is a short ride, a hop and a short ride (or a hop to a pad); slow travel rides
a chain of segments, sometimes with a hop where the path changes mode or region.

<a id="s8-5"></a>
### 8.5 Route actions

<a id="s8-5-1"></a>
#### 8.5.1 Segment (class 1506 here; Confirmed, MD17)

| Property | Type | Present | Value |
|---|---|---|---|
| `TravelSegment_HeadName` | string | 2,191 | Key of the path's first node (below) |
| `TravelSegment_SubSegmentArray` | array of `TravelSegment_SubSegment` DIDs | 32 | 2–5 other segments, ridden in order |

A segment has one of the two, never both. Each of the 2,185 distinct head names is the
`Segment_HeadName` property (string, PID 0x5F2 here) of a **SegmentNode** SED (ED type 19) in
landblock info. Of the 55,645 SegmentNode SEDs, 2,260 carry `Segment_HeadName`, and 2,193 of
those carry a value that some segment uses; no link and no SED of another type carries one. The path continues from the head node through landblock-info links
of type 9 (`Segment`; LB §4.6): 44,259 such links join two SegmentNode SEDs of one landblock and
18,139 cross a landblock edge and have one end id of 0.

> **OPEN:** the walking rule along a path: which link end is "next", how a link that leaves the
> landblock is continued (LB §4.6 leaves the link ids and block pair OPEN), and where a segment
> ends (the next segment's head, or a node without a further link). SegmentNode SEDs also carry
> `Entity_ContentLayers` (42,824) and scale properties; whether layers prune paths is not known.

<a id="s8-5-2"></a>
#### 8.5.2 Hop (class 1501 here; Confirmed)

`TravelHop_Delay` (float; 0 in 10, 3 in 1) and, in 9, `TravelHop_SceneID` (a scene record,
class 1468). A hop is a pause or a scene between two parts of a route.

> **OPEN:** the unit of `TravelHop_Delay` (seconds is likely) and what the scene record does.

<a id="s8-5-3"></a>
#### 8.5.3 Pad (class 1502 here; Confirmed)

`TravelPad_Location`: a place key (§3). All 73 resolve. A pad ends the route at that place.

<a id="s8-6"></a>
### 8.6 Travel mode (class 2130 here; Confirmed)

Eight records, each named by routes (2,665, 669, 488, 477, 366, 202, 109, 32 routes):

| Property | Type | Present | Value here |
|---|---|---|---|
| `TravelMode_PathNodeDistance` | float | 8 | 3 in all |
| `TravelMode_TurnSpeed` | float | 8 | 200 in all |
| `TravelMode_TurnAcceleration` | float | 8 | 0.75 in all |
| `TravelMode_MoveAcceleration` | float | 8 | 1.5 in all |
| `TravelMode_WalkDistance` | float | 8 | 0 in all |
| `TravelMode_TravelEffectDID` | DID | 7 | One effect record (the same in all 7) |
| `TravelMode_ScriptPants` | enum (mapper 0x23000097) | 7 | A distinct value per mode |

LC labels the modes by mount kind (horse, boat, goat and others) by their DIDs (Reported); the
data distinguish them only by `TravelMode_ScriptPants`.

> **OPEN:** units of the five floats (metres and degrees per second are likely) and how a
> client moves the rider along a path with them.

<a id="s8-7"></a>
### 8.7 Stations, NPCs and other users (Confirmed)

- `TravelWebDirectory` lists 375 of the 421 stations. The other 46 are named only by NPCs.
  NPCs (class 1724) name 411 distinct stations through `TravelWebWC` (437 NPCs); every station
  is in the directory or named by an NPC.
- Stable-master NPC properties: `TravelWebWC` (437), `TravelPad_Location` (432; 421 resolve,
  11 do not), `Travel_DiscountArray` (435 NPCs, 1–3 property IDs each), `TravelWebSellMultiplier`
  (float, on 16,583 NPCs: 1 on 16,276, 0 on 307), `Travel_Stablemaster_MustBeDiscovered`
  (boolean, 431; true 59).
- `TravelVersioning` (class 2191, one record): 11 pairs (`TravelVersioning_SlowDestination`,
  `TravelVersioning_SwiftDestination`); in all 11 the first is a slow and the second a swift
  destination.
- Race tables (class 860, 8 records) and advanced-start tables (class 3335, 9 records) list
  `Travel_PlayerHomeLocationArray` destinations (3–4 and 18 each).

<a id="s8-8"></a>
### 8.8 Server-side meaning (Reported or OPEN)

- `TravelDestinationCost` is a money amount in the smallest coin unit (LC, MD19).
- Route requirements are the generic usage checks (`Usage_MinLevel`, quest and faction
  requirements, subscription) (LC).

> **OPEN:** how discounts (`Travel_DiscountArray` names PIDs of float properties, all labelled
> `Discount_Travel_…`, 46 distinct) and `TravelWebSellMultiplier` change the price; what
> `TravelDestinationAllowPaperItems` allows; how the 26 routes listed in
> `TravelHomeLocationsArray` are meant; the meaning of the `Usage_QuestStatus` and
> `Usage_Operator` values. These are server rules; a local server needs its own (§14.3).

---

<a id="s9"></a>
## 9. Neighbourhoods and housing

<a id="s9-1"></a>
### 9.1 Model

A **neighbourhood template** describes one housing area: the landblocks it covers, its houses,
its entrance and exit places. A **neighbourhood** is one named copy of a template (3,248 copies
of 14 templates). **Houses** are plots in a template; **gates** lead into a template;
**housing hooks** are the decoration points placed in house landblocks.

<a id="s9-2"></a>
### 9.2 Neighbourhood (class 1185 here; Confirmed)

`Neighborhood_Name` (StringInfo) and `Neighborhood_NeighborhoodTemplate` (template DID), in all
3,248. `NeighborhoodDirectory` lists all 3,248. Copies per template: 450, 400, 400, 350, 300,
275 (four templates), 225, 20, and 1 (three templates).

<a id="s9-3"></a>
### 9.3 Neighbourhood template (class 1956 here; Confirmed)

| Property | Type | Present | Value |
|---|---|---|---|
| `NeighborhoodTemplate_Name` | StringInfo | 14 | |
| `NeighborhoodTemplate_HouseList` | DID | 14 | House list |
| `NeighborhoodTemplate_Boundaries` | array of `NeighborhoodTemplate_AllowedLandblock` positions | 14 | 0–173 landblocks; each position has only region and landblock (flag bytes 0x03 or 0x43) |
| `NeighborhoodTemplate_Telepad` | string | 14 | Place key of the entrance (§3) |
| `NeighborhoodTemplate_BootPosition` | string | 14 | Place key where players are put out |
| `NeighborhoodTemplate_SceneID` | DID | 14 | Scene record (class 1468) |
| `NeighborhoodTemplate_InitialInstances` | int | 14 | 1–50 (10 in ten templates) |
| `NeighborhoodTemplate_OccupancyThresholdPercentage` | float | 14 | 0.7–1 |
| `NeighborhoodTemplate_RolloutStep` | int | 14 | 1–10 |

Boundary regions: 1 (eight templates), 2 (four), 3 (one, with 173 blocks); one template has no
boundary. The dictionary also defines `NeighborhoodTemplate_InstanceProperties` (struct),
never stored.

> **OPEN:** the instancing rule (how `InitialInstances`, `OccupancyThresholdPercentage` and
> `RolloutStep` drive the number of live copies) is server behaviour.

<a id="s9-4"></a>
### 9.4 House list and house (classes 1952 and 1181 here; Confirmed)

House list: `HouseList_HouseArray`, 1–30 house DIDs (227 in all, each house in exactly one list).
225 houses name the template whose list holds them; 2 houses sit in the list of a template
with a single neighbourhood but name another template.

| House property | Type | Present | Value |
|---|---|---|---|
| `House_Address` | StringInfo | 227 | Address text |
| `House_Description` | StringInfo | 227 | |
| `House_NeighborhoodTemplate` | DID | 227 | Template |
| `House_Telepad` | string | 227 | Place key of the plot (all resolve) |
| `House_Type` | enum (mapper 0x23000257) | 227 | 37 distinct values, 1–40 |
| `House_ValueTier` | enum (mapper 0x23000259) | 227 | 1, 3, 4, 6–11 |
| `House_IsKinshipHouse` | boolean | 227 | true 25 |
| `House_IsPremiumHouse` | boolean | 227 | true 88 |
| `House_CharacteristicArray` | array of `House_Characteristic` DIDs | 225 | 1–3 trait records each |

<a id="s9-5"></a>
### 9.5 Gate (class 1955 here; Confirmed)

`NeighborhoodGate_NeighborhoodTemplate` (template) and `NeighborhoodGate_Telepad` (place key), in
all 13. Gates are reached from landblock-info weenies through
`HousingSystem_NeighborhoodGateDID` (43 weenies); NPCs that sell houses name a template with
`HousingSystem_PurchaseTemplateDID` (21).

<a id="s9-6"></a>
### 9.6 Housing hooks (class 1989 here; Confirmed)

744 records, placed by 32,338 landblock-info SEDs of type 6 (Weenie) whose object DID is a hook
(ED §2). Properties: `HousingHook_ID` (enum, mapper 0x23000265), `HousingHook_HookCategory`
(bitfield 64, mapper 0x23000264, whose 38 labels name hook categories such as interior floors,
walls and yards; bit-to-key rule PS §7.3), `HousingHook_IsPropertyHook` (true in 153), `Render_Draw` false in all,
`Usage_MinUsageDist` (12 or 25), `Usage_HousingPermission_OwnerOnlyRequirement` (81). The
dictionary also defines `HousingHook_AllowableOffset` (vector), never stored. Landblock info
also holds links of the `House_…` types (LB §4.6: decoration, property driver, permission,
landmark, for-sale sign, storage).

> **OPEN:** how the `House_…` links relate to houses and hooks (the link ids are OPEN in
> LB §4.6).

<a id="s9-7"></a>
### 9.7 The housing-control record (Confirmed properties; Reported formula)

`HousingControl` carries the price and upkeep tables a server needs: `HousingControl_HouseTypeInfoArray`
(per `HousingControl_HouseType`: `HousingControl_HousePrice`, `HousingControl_HouseUpkeep` and
icons), `HousingControl_HouseValueTierArray` (per `HousingControl_HouseValueTier`:
`…_PriceModifier`, `…_UpkeepPriceModifier`, `…_StoragePriceModifier`), `HousingControl_UpkeepInterval`,
`HousingControl_MaxPlayersPerNeighborhoodInstance`, `HousingControl_MinLevelToBuyHouse`,
`HousingControl_NeighborhoodContentLayer` (a content layer; the same named layer appears in 108
map-note layer lists) and further limits. Values are data and are read at run time.

LC computes a house's price as the type's price times the tier's price modifier, truncated to
an integer, and upkeep likewise (MD21, Reported).

---

<a id="s10"></a>
## 10. Properties that name places (Confirmed, MD22)

Every string value of every 0x79 and 0x78 set was looked up in §3 (case-insensitively). These
properties hold place keys (hits / values; the remainder do not resolve):

| Property (path) | Resolve / values |
|---|---|
| `TravelPad_Location` | 494 / 505 |
| `House_Telepad` | 227 / 227 |
| `NeighborhoodTemplate_Telepad`, `NeighborhoodTemplate_BootPosition`, `NeighborhoodGate_Telepad` | 14, 14, 13 (all) |
| `Death_RallyPoint` | 388 / 388 |
| `Effect_Recall_Telepad` | 288 / 310 |
| `EffectGenerator_EffectDataList` / `…_EffectData_Destination` | 629 / 630 |
| `AffectEntity_PortalStormCommand_LocationList` / `…_Location` | 30 / 30 |
| `MonsterPlay_SessionStartLocation` | 95 / 95 |
| `PortalDestination` | 11 / 11 |
| `Quest_RoleArray` (and objective roles) / `Quest_Role` / `QuestDispenser_PortalDestination` | 18 + 18 (all) |
| `RaceTable_InitialHomeTelepad`, `RaceTable_StartingLocation` | 8, 8 (all) |
| `AdvTable_AdvancedCharacterStart_InitialHomeTelepad` | 9 / 9 (all differ from the key in case only) |
| `PrivateEncounterTemplate_StartingPosition`, `…_SuccessTeleportLocation`, `…_InstanceMilestone`, `…_DefaultExitLocation`, `…_ExitLocationIfEnteredThroughDoor`, `…_ExternalInstanceMilestone`, `…_DynamicTelepadInfo` / `…_DynamicTelepad`, the team-config and success-location array forms | all resolve (1,510; 1,248; 730; 15; 2; 50; 108; 6 + 6 + 12) |

`TravelSegment_HeadName` values are **not** place keys (§8.5.1). Role-constraint strings of quests
coincide with a place key in a handful of values and are not place references.

---

<a id="s11"></a>
## 11. RIPCORDS (Confirmed, MD23)

The root DID map's key 0x10000002 (`RIPCORDS`, 0x2800002C here; PS §8.1) has 292 entries: the
null sentinel and 291 records of two ripcord classes (284 of class 2801, 7 of class 3166; the
classes have 341 and 7 records in all). A ripcord record names a world property it switches
(`RipCord_WorldProperty`, a PID; class 3166 adds `RipCord_WorldPropertyTableProperty` and
`RipCord_WorldPropertyArrayElement`), a category (`RipCord_Category`, enum mapper 0x2300030B),
a name and help string (`RipCord_Name`, `RipCord_Help`, Latin-1 strings), and a UI element in
one layout (`RipCord_UIElement_ID`, `RipCord_UIElement_Layout`). They are administrator
switches over world state, not map or travel data; they are listed here because the map and
travel features read world properties that ripcords may switch.

> **OPEN:** which ripcords gate travel or map features.

---

<a id="s12"></a>
## 12. Invariants

Checked on this install unless marked; readers report violations as warnings, except where
"malformed" is said.

| ID | Invariant |
|---|---|
| MI1 | Each of the three tables decodes to its last byte; leftover bytes mean malformed. |
| MI2 | Places: entries are in bucket order of the lower-cased-name hash (§3.3); names are unique ignoring case. |
| MI3 | Places: `area` = `dungeon`; each is 0 or a zone record. |
| MI4 | Every position in the three tables obeys PS §4.6 (flag rules, no instance). |
| MI5 | Map notes: `flag_b` ≤ 1 (malformed otherwise, BIN). |
| MI6 | Map notes: the link block is present exactly when bit 0 of `type` is set. |
| MI7 | Map notes: `type` has exactly one bit set; link rows have `type` = 1, null text, `level` 0, ranges 0, `discovery` 0. |
| MI8 | Map notes: every row passes §4.4 (the position test of §4.4.1 for every row, the text test of §4.4.2 and the level and range checks for non-link rows); `area` ≠ 0; `dungeon` = 0 for every outdoor row. |
| MI9 | Map notes and quest notes: `layers` has PID `Entity_ContentLayers` and no element 0. |
| MI10 | Quest notes: every row is indoors and `dungeon` is a dungeon record. |
| MI11 | Travel: every route has a destination, a mode and at least one action; every action is a segment, hop or pad; every segment has a head name or sub-segments, not both; every head name names a SegmentNode SED's `Segment_HeadName`. |
| MI12 | Travel: every pad's `TravelPad_Location` resolves in §3. |
| MI13 | Housing: every house is in exactly one house list; every template's house list, scene and telepad keys resolve. |
| MI14 | Every DID edge of §2.4 points to a record of the listed kind. |
| MI15 | Every position of the three tables, and every `link_position`, passes the position test of §4.4.1. Places: no name contains a code unit 0 (§3.3.1). |

---

<a id="s13"></a>
## 13. Worked synthetic examples

All bytes are invented for illustration; DIDs 0x700000xx and table 0x25000001 are synthetic.

<a id="s13-1"></a>
### 13.1 Places table with two entries

Two places, keys "ab" and "B". Hint 0 → *B* = 11. Lower-cased hashes (RC §1.5): "ab" →
(0x61 · 16) + 0x62 = 0x672 = 1,650, bucket 1,650 mod 11 = 0; "b" → 0x62 = 98, bucket 10. So "ab"
is stored first.

| Bytes | Meaning |
|---|---|
| `04 00 00 0E` | own DID 0x0E000004 |
| `00 02` | TSize: hint 0, 2 entries |
| `02 61 00 62 00` | name "ab" (2 units) |
| `9B 02 10 20 00 00 00 00 80 3F 00 00 00 40 00 00 40 40` | position: flags 0x9B; region 2; landblock (0x10, 0x20); cell 0; x, y, z = 1, 2, 3 |
| `00 00 00 00 00 00 00 00` | area 0, dungeon 0 |
| `01 42 00` | name "B" (1 unit) |
| `1B 02 10 20 05 01 00 00 20 41 00 00 A0 41 00 00 F0 41` | position: flags 0x1B; region 2; landblock (0x10, 0x20); cell 0x0105; x, y, z = 10, 20, 30 |
| `30 00 00 70 30 00 00 70` | area and dungeon 0x70000030 |

66 bytes. Looking up "AB" lower-cases it to "ab", hashes to bucket 0, matches "ab" ignoring case
and returns the first position and area 0. Looking up "b" finds "B".

<a id="s13-2"></a>
### 13.2 A map-note row

| Bytes | Field |
|---|---|
| `9B 02 10 20 00 00 00 00 80 3F 00 00 00 40 00 00 40 40` | `position` (as above) |
| `10 00 00 70` `00 00 00 00` `20 00 00 70` | `area` 0x70000010, `dungeon` 0, `entity` 0x70000020 |
| `CE 04 00 00` `01 00 00 00` `64 05 00 00` `01 00 00 00` | `layers`: PID 0x4CE, array of 1: PID 0x564, value 1 |
| `00` `11 00 00 00` `01 00 00 25` `00` `01 00` | `text`: reference, token 0x11, table 0x25000001, no args, empty argument table |
| `00 00 00 00` | `icon` 0 |
| `00 00 01 00` | `level` 0x10000 (bit 16, key 0x11) |
| `00 00 00 00 20 00 00 00` | `type` 1 << 37 (key 0x26) |
| `00` | `flag_b` |
| `00 00 00 00` `00 00 00 00` `00 00 00 00` | `min_range` 0, `max_range` 0, `discovery` 0 |
| `00 00` | `extra`: empty set |

89 bytes. Because `dungeon` = 0 and a range is 0, the client files it under region 2, landblock
key 0x10 · 65,536 + 0x20 = 1,048,608 (§4.6).

<a id="s13-3"></a>
### 13.3 A link row

As §13.2 but: `type` = `01 00 00 00 00 00 00 00`; `text` = `00 00 00 00 00 00 00 00 00 00 01 00`
(null); `level` = 0; `layers` = `CE 04 00 00 00 00 00 00` (empty array); and after `flag_b` the
link block `1B 02 10 20 05 01 00 00 20 41 00 00 A0 41 00 00 F0 41` (`link_position`) and
`30 00 00 70` (`link_target` 0x70000030). 103 bytes. Besides its own filing, it adds an entry
to the list of zone 0x70000030 because that differs from its `dungeon` (0).

<a id="s13-4"></a>
### 13.4 A dungeon quest-note row

`position` (18 bytes, interior), `area` `00 00 00 00`, `dungeon` `30 00 00 70`, `entity`
`40 00 00 70`, `layers` `CE 04 00 00 00 00 00 00`, then the set `00 01` (hint 0, one entry),
key and PID of `QuestDispenser_RoleConstraint` (u32 each, from the dictionary), and the Pascal
string `03 61 62 63`. 52 bytes.

<a id="s13-5"></a>
### 13.5 A travel graph

Station W (travel-web record) has `TravelHomeLocation` = D1 and one `TravelDestinationRecord`
{`TravelDestinationRoute` = R, `TravelDestinationCost` = 500}. Route R has
`TravelRoute_Destination` = D2 (`TravelDestinationIsSwiftTravel` true), `TravelRoute_TravelMode`
= M and `TravelRoute_ActionArray` = [S1, H1, S2], where S1 and S2 have head names that are
`Segment_HeadName` values of SegmentNode SEDs near D1 and D2, and H1 is a hop with
`TravelHop_Delay` 0 and a scene. The explorer draws W at the place of its NPC's
`TravelPad_Location`, a swift-travel edge W → D2 labelled with R's name and the cost, and the
two short paths S1 and S2.

---

<a id="s14"></a>
## 14. Implementation guidance

<a id="s14-1"></a>
### 14.1 Explorer (Tessera rules where marked)

- Places (§3): list keys with region, landblock, map coordinates (CO §8) and zone name (WS
  §11.2/§11.4); "go to place" moves the camera to the position.
- Map notes (§4): **Tessera rule** until §4.6's OPEN is settled: draw every valid non-link row on
  the map node or dungeon map that contains its position (GP §9.4, §8.5), with a symbol chosen
  by the label of its single `type` bit and its `text` rendered by ST §9; show rows whose
  `layers` pass the ED §4.5.1 test for the content layer being viewed (layer 1 by default);
  ignore `level`, ranges and `discovery`. Draw link rows as arrows from the row's position to
  the `link_target` zone's map, at `link_position`.
- Dungeon quest notes (§5): optional layer on dungeon maps, labelled with the quest names of
  `QuestDispenser_ActiveQuests`.
- Travel (§8): stations at their NPCs' pads; edges per route to the destination's position
  (the last pad, or the end of the last segment when the path walk of §8.5.1 is settled; until
  then the position of the station whose home is the destination); swift and slow edges in
  different styles; the cost from the selling station.
- Neighbourhoods (§9): outline the template's boundary landblocks; mark entrance, exit and house
  plots by their place keys.

<a id="s14-2"></a>
### 14.2 Walker

- A "go to place" command takes a key and teleports to its position (§3.4).
- The `/loc` text and the area name come from CO §9 and WS §11; the places table adds no names
  for the player's position (its keys are identifiers, not display text).

<a id="s14-3"></a>
### 14.3 Local server (sandbox)

- Travel: offer, at a station, the routes of its `TravelDestinationRecordArray` whose usage
  requirements the character meets, at `TravelDestinationCost`; on purchase, move the character
  to the route's end (a pad's place, or the destination's station) after the hops' delays.
  Discounts and sell multipliers are OPEN (§8.8); **Tessera rule:** ignore them.
- Housing: one neighbourhood per `Neighborhood` record, created on first entry, holding the
  template's houses; entrance and exit by the template's place keys. Prices per §9.7 (Reported).

---

<a id="s15"></a>
## 15. Open questions (summary)

1. §4.2: meaning of `flag_b`; the version source that gates `extra`.
2. §4.6: how the maps use `level`, `type`, `discovery` and `layers` of notes, and whether ranges limit anything besides the radar (with GP §8.8).
3. §5.3: which dungeon quest notes the client shows.
4. §6.2: why some rows differ from their entity.
5. §8.5.1: the walking rule along SegmentNode links, and segment ends.
6. §8.5.2, §8.6: units and use of hop delay, hop scene and travel-mode values.
7. §8.8: discounts, sell multiplier, paper items, routes listed as home locations, usage-requirement values.
8. §9.3: neighbourhood instancing rule.
9. §9.6: how `House_…` links name houses and hooks.
10. §11: which ripcords gate travel or map features.
11. §3.3.1: the lower-case table for non-ASCII code units under the client's locales.
12. §4.4.2: the validity check applied to each StringInfo argument kind.
13. §4.7: which position is the radar's reference; whether the world map applies the range test.

---

<a id="s16"></a>
## 16. Notes for other specs

- `records.md` §2.1, prefix 0x0E: the layouts of 0x0E000004, 0x0E000006 and 0x0E400000 are now
  given here (Confirmed); the UNIQUEDB labels are `PlacesTable`, `MapNotes` and
  `DungeonQuestMapNotes`; the `SegmentTable` entry (0x0E000002) names a record that does not
  exist in this install.
- `game-panels.md` §8.8: §4.6 here gives how the client files notes after loading, and §4.7 the
  radar's range test for ranged notes; the other display rules remain OPEN there and here.
- `landblock.md` §4.6: links of type 9 join SegmentNode SEDs (§8.5.1 here), which bears on the
  OPEN about link ends.

---

<a id="changelog"></a>
## Change log

| Date | Change | By |
|---|---|---|
| 2026-10-07 | First version: singleton tables (§3–§5), map-note properties (§6), map images (§7), travel (§8), housing (§9), place consumers (§10), ripcords (§11). Status Review. | Spec writer (dirty room) |
| 2026-10-07 | Revision after review (changes requested). Header: review issues moved below the header table, which they had split, with a resolution note. Provenance: BIN second use with questions q6–q9; rows MD24–MD27 added; MD4, MD8 and MD10 revised; unused **Probable** mark removed. §3.2: debug-dump labels paraphrased. §3.3: the full 16-bit lowered unit enters the hash; new §3.3.1 (per-unit, locale-dependent lower-casing, key ends at unit 0, Tessera rule, OPEN for non-ASCII). §3.4: lookup cites §3.3.1; the sentence on an untraced routine dropped. §3.5: code-unit census (296,009 units, none 0, five 0x1F). §4.2: units of the ranges. §4.4: new §4.4.1 (position test) and §4.4.2 (text test, OPEN for arguments). §4.6: link entry needs both positions valid; 2,590 of 2,754 link rows qualify. New §4.7 (radar range test). MI8 revised, MI15 added; §15 items 2 and 11–13; §16 note for GP §8.8. Re-measured over all records with a new probe: entry and row counts with 0 bytes left over (9,025, 13,485, 5,638), the flag-byte and region histograms of §3.5, §4.5 and §5.3, interior places (2,330) and orientations (7,351), the link-position flag bytes of §4.3, all eleven (`min_range`, `max_range`) pairs of §4.5, and level 0 exactly in the link rows; all reproduce, no count changed. Status stays Review. | Spec writer (dirty room) |
| 2026-10-07 | Second re-measurement after review, over all records (no text change outside this row): every position of the three tables and every `link_position` passes §4.4.1 (13,485, 2,754, 9,025, 5,638; largest \|w² + x² + y² + z² − 1\| 2.4 · 10⁻⁷); the 10,731 non-link texts are references without arguments that resolve in the English tables, 87 of them to empty text (§4.4.2); 2,590 link rows get a zone entry and 164 lead into their own dungeon (§4.6); the eleven range pairs (12,611 / 811 / 23 / 40 in eight pairs, §4.5, §4.7); place names: 296,009 code units, 296,004 in 0x20–0x7F and five 0x1F, none 0, 168 with upper-case letters, no two equal ignoring case; 412 link rows whose `link_position` shares the row's landblock (§4.3); the §5.3 flag-byte and region lines. All reproduce; no count changed. Status stays Review. | Spec writer (dirty room) |
