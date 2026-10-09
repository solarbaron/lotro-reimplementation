<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Map data: named places, map notes, travel networks and neighbourhoods

| | |
|---|---|
| **Status** | Accepted (re-review of the repair of the 2026-10-09 OPEN sweep passed, 2026-10-09) |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Spec reviewer (separate dirty-room session, AI-assisted), not the author; re-review by a second separate dirty-room reviewer session, AI-assisted, not the author |
| **Review record** | Re-review 2026-10-07: **passed**, Accepted. OPEN-closing revision 2026-10-08: changes requested (four items), then repaired. Re-review of the repair 2026-10-08 by a separate dirty-room reviewer session (AI-assisted, not the author): **passed**, Accepted; three non-blocking notes. Details in the review log (decision 0018). OPEN sweep 2026-10-09 (spec writer, dirty room): normative text of §2.3, §2.4, §7, §9.7 and §11 changed (rows MD44–MD48); status Revised until a separate session reviews it. Review of the OPEN sweep 2026-10-09 by a separate dirty-room reviewer session (AI-assisted, not the author): **changes requested**. Spot checks, read-only, own probe: MD44 (2,733 `WeenieType` `Landmark` sets with `MapNote_Type`, all class 1210, 2,731 with `Landmark_Music`, one class-1995 set without), MD45 (3,434 / 3,419 / 15, 8 DIDs), MD46 (`tessera-dattool stat`: 1,616 + 2 = 1,618; the 2 local sets hold no map struct), MD47 (27 top-level fields, entry counts 37 / 10 / 14 / 3 / 5 / 6, 21 of 37, all types and mappers, icons 0x41, storage modifier defined but not an entry field) and MD48 (types; every field in all 341 + 7 records) all reproduce; no new BIN fact; no code, pseudo-code, copied prose or game text. Blocking: (1) §12 MI14 still requires every §2.4 edge to resolve, but §2.4 now has 15 `QuestEvent_LandmarkDID` values that name no record; restate MI14 (exception or count) so it holds on this install. (2) §6.2 still says landmarks (class 1210) carry `Landmark_Music`, contradicting MD44 (2 of 2,733 lack it). (3) §2.4 "a reader treats such a value as no landmark" is a reader rule with no provenance (MD45 covers only the counts) and what the client does with a dangling value is not stated; mark it as a Tessera rule (and list it with the §15 rules) or add an OPEN. (4) §9.7 interval units "seconds by their size, not confirmed" is a guess in normative text; give a source or mark it `> **OPEN:**`. Non-blocking: §2.2 still shows `HousingControl` class as "(singleton)" while §9.7 gives class 1184; §2.3 could say the `Landmark` label is read through the `WeenieType` mapper named by the dictionary (0x230000C9 here). Repair 2026-10-09 (spec writer, dirty room, own read-only probe over the OBS3 dump): (1) MI14 now excepts `QuestEvent_LandmarkDID`, whose dangling values are warnings; (2) §6.2 gives the landmark-only properties with their count (2,731 of 2,733, MD49); (3) §2.4 marks "no landmark" as a Tessera rule, adds an OPEN on the client's handling (§15 item 11); (4) §9.7 interval unit moved to an OPEN with values per field (MD49) and published weekly upkeep (MD50, Reported) (§15 item 12); non-blocking: §2.2 class 1184, §2.3 mapper. Status Revised until a separate session re-reviews it. Re-review of the repair 2026-10-09 by a separate dirty-room reviewer session (AI-assisted, neither the author nor the earlier reviewer): **passed**, Accepted. Checked the whole revision since the last Accepted text (OPEN sweep and repair) against CLEAN_ROOM.md §5.3: every new fact has a row (MD44–MD50, keys OBS3 and PUB), no code or pseudo-code, no copied prose, no game text; all four blocking items and both non-blocking notes are fixed (MI14 exception, §6.2 counts, §2.4 Tessera rule with an OPEN and §15 item 11, §9.7 unit as an OPEN and §15 item 12, §2.2 class 1184, §2.3 mapper); the two §14.4 implementer notes are really answered. Spot checks with an own read-only probe on the install (fresh read, not the OBS3 dump): MD44 (2,733 `WeenieType` `Landmark` sets with `MapNote_Type`, all class 1210; one class-1995 set without), MD49 (2,731 class-1210 sets carry `Landmark_Music` and both override booleans, 2 carry none; `WeenieType` enum of mapper 0x230000C9; intervals 604,800 / 86,400 / 86,400, type int, no mapper), MD47 (class 1184; 37 type entries, 10 tier entries; the storage modifier is a float the tier entry does not allow and no tier carries), MD48 (UI element enum of mapper 0x230000D1, world property a PID, the disable flag a bool; the layout field is a DID whose 348 values all have prefix 0x22, a UI layout); MD50 matches the published sources. No new BIN fact, so no analysis-project check was needed. Non-blocking: `quests.md` §5 counts the 2,705 + 729 `QuestEvent_LandmarkDID` values as landmark references without the 15 that name no record (MD45 here); a §16 note for `quests.md` would keep the two specs aligned. |
| **Implements into** | `crates/formats` (decoders for the three singleton tables of §3–§5), `crates/world` (place lookup §3.4, map-note index §4.6, travel and housing graphs §8–§9), `apps/explorer` (world-map overlays: places, notes, links, travel routes, neighbourhoods), `apps/client` (walker place lookup), `crates/net` sandbox backend (travel and housing data for a local server) |
| **Companion specs** | [`records.md`](records.md) (**RC**: §1.2 VLE, §1.3 TSize and bucket order, §1.4 strings, §1.5 name hash, §2.1 DID table, §4 StringInfo); [`property-system.md`](property-system.md) (**PS**: §4.6 positions, §4.8 StringInfo, §5 property sets and list/table entry forms, §6.2 the +0x09000000 rule, §7.3 bit-to-key rule, §8.1 DID maps, §17.4 StringInfo reader); [`wstate.md`](wstate.md) (**WS**: object-state header and root class, §11 areas, territories, regions, dungeons); [`landblock.md`](landblock.md) (**LB**: §4.6 links, §6 landblock properties and minimap tiles); [`entity-descriptor.md`](entity-descriptor.md) (**ED**: SED types, §4.5.1 content layers); [`game-panels.md`](game-panels.md) (**GP**: §8 minimap, §9 world map, map georeference); [`coordinates.md`](coordinates.md) (**CO**: §5–§8 frames and map coordinates, §9 `/loc` text); [`string-table.md`](string-table.md) (**ST**: §9 rendering a StringInfo) |

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
| OBS | Observed in the user's install (Steam, data current on 2026-10-06/07) with throwaway read-only probes in the dirty-room scratch area, using the repository's decoders as tools. **Full censuses:** every byte of 0x0E000004, 0x0E000006 and 0x0E400000 (all rows decoded, zero bytes left over); every 0x70 record header (root class) and every 0x79 property set of the classes named in this spec, and of every 0x70 and 0x78 set when looking for references to them (inbound-reference scan over all 383,332 object property sets and 1,616 stand-alone sets, plus the bundles of all object-state records); all 49,636 landblock-info records (SEDs, links, weenies); the DID maps UNIQUEDB 0x28000002, WEENIECONTENT 0x2800000E, CONTENT_LAYER 0x28000012 and RIPCORDS 0x2800002C; the enum mappers named in §6.3 and §9. **2026-10-08** (new read-only probe, all records of each kind): the UNIQUEDB entries 0x1000001B and 0x10000013 and the two property sets they name, every element of their note-type arrays, the `extra` set of all 13,485 map-note rows (every world-event state), and the content-layer test of §4.8.2 on every row for *L* = 0 and *L* = 1. **2026-10-08, second probe** (read-only): the dictionary entries of all 53 `Discount_Travel_…` properties; a byte scan of every section of the client executable for the property IDs of MD38; a full instruction sweep of the class library's code block (record 0x56000000, by `wlib-bytecode.md` §3.2: 362,178 instructions, ending exactly at the block's end) for integer immediates equal to the property IDs of MD41, with the bodies, classes and static call graph involved; the `UIElementID` labels (mapper chain from 0x230000D1) of the element IDs named in MD38–MD40. |
| BIN | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (SHA-256 prefix d22363eb9aff0fb5, the build of the other world specs), read in the dirty room on 2026-10-07 in a copy of the shared analysis project, by address range. **Questions:** (q1) the stored layout of a map-note row: field order and identity of the three DIDs, whether the byte after the type is padding or a value, the exact condition for the link block, and when the trailing property set is read; (q2) which rows the client rejects as invalid, and how it indexes rows after loading; (q3) the places-table reader and lookup: entry order, the key used for hashing, case handling, and which DID the two u32 fields are; (q4) the dungeon quest-note row and table readers and how rows are grouped; (q5) how the client finds the three tables. **Why M1–M3 were not enough:** M1 decodes every row exactly but cannot tell padding from an always-zero value, `type = 1` from "bit 0 set" (no row has bit 0 with another bit), "area" from "dungeon" in the places table (both fields are equal in every record), or the client's validity and indexing rules (they leave no trace in data); no published source states them; M3 cannot isolate them. **Read:** the map-note base, row and quest-row serializers and their reset and validity routines, the two table serializers, the places-table entry serializer, its two name lookups and its debug dump, the map-note manager's two load routines, its add-note routine and its per-zone list lookup. No network, login, launcher or anti-tamper code. **Second use, 2026-10-07 (revision after review, same build, same method, by address range):** (q6) what the client's "valid position" test checks, for the row's own position and for `link_position`; (q7) what its "valid text" test of a map-note row checks; (q8) which lower-case mapping the places-table lookup applies to code units, and under which locale; (q9) the unit of `min_range` and `max_range` and the test that uses them. **Why M1–M3 were not enough for (q6)–(q9):** every position, text and key in the data already passes or is ASCII (no invalid position, no unresolved text, no non-ASCII key), so the data cannot show the rule or its edge cases; no published source states them; the owner's in-game observations cannot isolate them. **Read (second use):** the position class's validity routine and the orientation check it calls, the StringInfo validity routine, the start-up routine that selects the C-runtime locale from the game language, the map-note manager's ranged-note query and the radar's note-update routine with the range predicate it calls. The meaning of the C-runtime functions named by import is taken from their public documentation. **Third use, 2026-10-08 (OPEN-closing session; same build, SHA-256 prefix d22363eb9aff0fb5 checked again; Ghidra headless decompiler on a read-only copy of the analysis project, single functions by address):** (q10) what the client does with `flag_b`, and where the version that gates `extra` comes from; (q11) which check the valid-text test applies to each kind of StringInfo argument; (q12) how the client selects, filters and draws table notes on the radar and in the map window: content layers, `discovery`, `type` filters, icons, `level`, and the world-event states in `extra`; and which dungeon quest notes the radar fetches; (q13) which position the radar measures ranges from, and whether the map window applies the range test; (q14) whether the client reads an entity's own map-note properties for a table row; (q15) whether the client's code reads the travel-mode, hop, segment, discount and sell-multiplier properties of §8 directly. **Why M1–M3 were not enough for (q10)–(q15):** these are client rules with no trace in any record (`flag_b` is 0 everywhere, no record stores a version, no StringInfo has arguments, the selection and drawing rules combine data with player and UI state); no published source states them; observing the client could show that a note appears but not the rule, the reference position or the filter that decides it. **Read (third use):** the map-note row class (its serialiser, reset, validity routines and per-row link test), the stream's version query and the version table compiled into the client, the StringInfo validity routine, the StringInfo argument-value factory and the five value classes' check routines, the map-note manager's start-up, its two icon-table loaders, its post-filing hook and its region, dungeon and zone queries, its world-event state evaluator, the radar's note update, note placement, ranged-note update, type filter, content-layer source and reference-position update (with the user option that selects the position record), the map window's note pass and its map-node relation, and a search of the whole executable, as instruction operands, for the property IDs of §6.1, of the `TravelMode_…`, `TravelHop_…` and `TravelSegment_…` properties, of the four properties named in §8.8 and of the three instancing properties of §9.3 (the `Discount_Travel_…` properties were not in this search; the fourth use covers them). No network, login, launcher or anti-tamper code. **Fourth use, 2026-10-08 (revision after the review of the same date; same build, SHA-256 prefix checked again; same method, single functions by address):** (q16) what the client does at the 13 instructions whose operand equals a `Discount_Travel_…` property ID, and where the travel window takes its destinations and prices from; (q17) which region and landblock rectangle the radar's area query uses, and what it queries while its recorded position is invalid; (q18) how the map window relates an area to a map node when two nodes list the area or none does. **Why M1–M3 were not enough for (q16)–(q18):** an operand equal to a property ID may be a UI element ID or a property use, which only the code shows; the radar's rectangle and the map window's registration rule leave no trace in data (no area is listed twice, no row lies in an unlisted area); no published source states them; observing the client shows which notes appear, not the rectangle or rule behind them. **Read (fourth use):** the 13 instructions and the routines around them; the travel window's constructor, its property-change registration and handler, its refresh routines and its two calls into the script machine; the radar's note update with the rectangle routine, the georeference setter and the position normaliser it calls; the map window's area-to-node registration, its area lookup and its node-ancestry test. No network, login, launcher or anti-tamper code. |
| LC | LotroCompanion `lotro-tools` (places, map-note, quest-note, travel and housing loaders), read for facts only. Marked **Reported** where only LC supports a fact. |
| PUB | Published player documentation, consulted 2026-10-09 for a gameplay fact only: the lotro-wiki page "House" and the publisher's "LOTRO Housing FAQs" help article (as summarised by a web search; both pages refused a direct fetch). Marked **Reported**. |
| OBS3 | Observed in the user's install on 2026-10-09 (OPEN sweep, spec writer), read-only; archives unchanged since 2026-10-04 (file times). A dump of **every** 0x70 record with its 0x79 set and of the 0x78 sets of `client_gamelogic.dat`, made on 2026-10-07 with the repository's own decoders, walked at every nesting depth; the property dictionary for types; `tessera-dattool stat` for the 0x78 count of every archive. |
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
| MD7 | Map-note row layout, field identities, the boolean byte, the link block read when bit 0 of `type` is set, the trailing property set (§4.2–§4.3) | OBS (every row decodes exactly; link block present in exactly the 2,754 rows with bit 0); BIN (q1) | Confirmed (layout); BIN (byte checked as boolean); version gate MD29 |
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
| MD27 | `min_range` and `max_range` are distances in world units; the radar hides a note whose two ranges both exceed 0.0002 in magnitude when its straight-line distance from the radar's reference position (MD35) is above `max_range` or below `min_range`, and draws a shown one at the rim in the player's horizontal direction to it (§4.7) | BIN (q9, q13) | BIN |
| MD28 | `flag_b` is stored and copied with the row, but neither the radar nor the map window reads it; it has no display effect (§4.2) | BIN (q10); OBS (0 in all 13,485 rows) | BIN |
| MD29 | The version that gates `extra` is the client's own current version for this record kind, taken from a table compiled into the executable (2 in this build); records carry no version, so archive rows always have `extra` (§4.2) | BIN (q10); OBS (every row decodes exactly only with `extra`) | Confirmed |
| MD30 | Valid-text test per argument: a nested-StringInfo argument passes when it passes the same test; numeric arguments always pass; an entry without a value is skipped; a mode-2 reference passes on token ≠ 0 and table ≠ 0 alone (§4.4.2) | BIN (q11) | BIN (no StringInfo here has arguments or mode 2) |
| MD31 | The client's note queries: a region and landblock range (ranged notes of the region, the notes filed in each landblock), or a zone (its filed notes), each extended by the notes filed under every zone a returned link row leads into; every returned note passes the discovery check and the content-layer test of §4.8.2 with *L* = the player's `Volume_ContentLayer` (§4.8.1, §4.8.2) | BIN (q12); OBS (12,620 rows pass the layer test at *L* = 0 and at *L* = 1) | BIN (rule); Confirmed (counts) |
| MD32 | Radar and map-window filters: `type` against the window's note-type filter, minus the current private encounter's disallowed types on the radar; on outdoor maps, the map node listing the note's `area` and its ancestors; neither reads `level` (§4.8.3, §4.8.4) | BIN (q12) | BIN |
| MD33 | Note icons come from two UI property sets named by UNIQUEDB keys 0x1000001B (`UI_MapNoteIcons_Map`, map window) and 0x10000013 (`UI_RadarNoteIcons_Map`, radar), looked up by the exact `type` value; no entry means no icon; ranged notes on the radar use the entry of bit 45 (§4.8.5) | BIN (q12); OBS (0x78000001: 46 entries, 0x78000002: 47; every entry a single bit, no duplicate; every non-link row's type has an entry except the 11 rows of bit 62) | Confirmed (data); BIN (rule) |
| MD34 | A row whose `extra` holds world-event map-note states is shown only while one state's condition holds; the first such state replaces the icons and the text it carries (§4.8.6) | BIN (q12); OBS (663 rows with states, 849 states, each with exactly one condition form) | Confirmed (data); BIN (rule) |
| MD35 | The radar's reference position is the player's position, recorded by the radar when the player moves; the map window applies no range test (§4.7) | BIN (q13) | BIN |
| MD36 | For table rows the client takes every map-note value from the row; it does not consult the entity's map-note properties (§6.2) | BIN (q12, q14: the display copy and the selection read only row fields) | BIN |
| MD37 | The radar fetches the dungeon quest notes (§5) of each zone that a selected link row leads into (§5.3) | BIN (q12) | BIN |
| MD38 | No instruction of the client executable has as an operand the property ID of a `TravelMode_…` or `TravelHop_…` property, of `Travel_DiscountArray`, `TravelWebSellMultiplier`, `TravelDestinationCost` or `TravelDestinationAllowPaperItems`, or of the three instancing properties of §9.3; none of these IDs occurs anywhere in the file as a 32-bit value. The operands equal to `TravelSegment_…` IDs are UI element IDs of equal value. Of the 53 `Discount_Travel_…` properties, 10 occur as operands of 13 instructions: 12 of them are UI element IDs of equal value (9 child-element lookups, a UI-message test, a switch that picks element IDs, the value of an element-valued UI property), and one is the property use of MD39 (§8.5, §8.6, §8.8, §9.3) | BIN (q15: operand search for every ID listed except the `Discount_Travel_…` ones; q16: the 13 `Discount_Travel_…` instructions); OBS (byte scan of the executable; element labels) | BIN (operand uses); Confirmed (byte scan) |
| MD39 | The travel window (top-level UI element `Travel_Window`) registers with the local player for change notification of `Discount_Travel_Special`, `Advancement_Level`, `Travel_BypassUsageChecks`, `WE_Ettenmoors_Freep_Population_Tracker`, `WE_Ettenmoors_Freep_Population_Cap` and `Shapeshift_MeshIID`, among others; when one changes it rebuilds its destination list and buttons, without looking at which property changed or at its value (§8.8) | BIN (q16); OBS (property and element labels) | BIN |
| MD40 | The travel window takes its destination list, and per destination the values it shows (among them the cost shown in `Travel_Currency_DestinationCost`, replaced by `Travel_Currency_DestinationCost_Free` when the cost is 0, and the minimum level shown in `Travel_Currency_MinLevel`), from two script functions of the class library that it starts by function ID: 0x01CC0001 (the list) and 0x01CC0003 (one destination); both are functions of class 460 here (`wlib-bytecode.md` §6.4) (§8.8) | BIN (q16); OBS (both IDs resolve to bodies of class 460) | BIN |
| MD41 | The class library's code pushes as 32-bit integer immediates (`wlib-bytecode.md` §5.1, op 0x01, type 2) the IDs of `Travel_DiscountArray`, `TravelWebSellMultiplier` and `Travel_BypassUsageChecks`, once each, in three bodies of class 1507 that class 460 inherits (slots 10, 27, 36 here); `TravelDestinationAllowPaperItems` once (class 1499, the destination class of §8.3, slot 86) and `TravelDestinationCost` once (classes 2115 and 2122, slot 119). The static calls of function 0x01CC0003 reach the three class-1507 bodies. No `Discount_Travel_…`, `TravelMode_…`, `TravelHop_…`, `TravelSegment_…` or instancing ID occurs as an immediate (§8.5, §8.6, §8.8, §9.3) | OBS (every instruction of the code block; the static call graph; class indices and slots are this install's) | Confirmed |
| MD42 | The radar's area query uses the region of its recorded position and the landblock rectangle from the block offsets (*bx*, *by*) of its map georeference to the block that contains the point *W*/*s* east and *H*/*s* north of block (*bx*, *by*)'s south-west corner (*W* × *H* the pixel size of `MinimapMapImage`, *s* the georeference scale, 1.0 when \|*s*\| ≤ 0.0002), found with CO §4's normaliser without clamping; the radar queries only while it holds a georeference; with region 0 the result is empty (§4.8.1) | BIN (q17); OBS (element label) | BIN |
| MD43 | The map window registers each area of a map node's `UI_Map_AreaDIDs_Array` once: an area already registered keeps its first node and the client reports the conflict; an area that no node lists, or a node whose element is 0, maps to the element `WorldMap`, the active element of the root node (§4.8.4) | BIN (q18); OBS (element label; no area is listed by two nodes, GP §9.4) | BIN (rule); Confirmed (no duplicate here) |
| MD44 | Landmarks are exactly the property sets whose `WeenieType` has the label `Landmark` and that carry `MapNote_Type`: 2,733, all of class 1210; `Landmark_Music` is in 2,731 of them. The only other set with `WeenieType` `Landmark` (class 1995 here) has no `MapNote_Type` (§2.3) | OBS3 (every 0x79 set) | Confirmed 2026-10-09 |
| MD45 | `QuestEvent_LandmarkDID` values in quest records, any depth: 3,434; 3,419 name landmarks; 15 (8 distinct DIDs) name no record of the install (§2.4) | OBS3 | Confirmed 2026-10-09 |
| MD46 | Stand-alone property sets (0x78): 1,616 in `client_gamelogic.dat`, 2 in `client_local_English.dat`, 1,618 in all; the 1,300 map sets are all in the first (§7) | OBS3 (`tessera-dattool stat` over all 29 archives); implementation census 2026-10-07 | Confirmed 2026-10-09 |
| MD47 | `HousingControl` labels and value types (§9.7); `HousingControl_HouseValueTier_StoragePriceModifier` is defined in the dictionary but is not an allowed field of the tier entry and occurs in no set | OBS3 (dictionary; the one record) | Confirmed 2026-10-09 |
| MD48 | Ripcord field types: `RipCord_WorldProperty`, `RipCord_WorldPropertyTableProperty`, `RipCord_WorldPropertyArrayElement` are PIDs; `RipCord_UIElement_ID` an enum of mapper 0x230000D1; `RipCord_UIElement_Layout` a DID; `RipCord_Name`, `RipCord_Help` strings; `RipCord_DisableEditingInAdminPanel` a bool in all 348 (§11) | OBS3 (dictionary; all 348 ripcord records) | Confirmed 2026-10-09 |
| MD49 | In class 1210, `Landmark_Music`, `Landmark_OverrideMountedCombat_Allow` and `Landmark_OverrideMountedCombat_Disallow` occur together in the same 2,731 sets; the other 2 sets carry none of the three (§6.2). `WeenieType` is an enum of mapper 0x230000C9 (§2.3). `HousingControl`: `HousingControl_UpkeepInterval` = 604,800, `HousingControl_PurchasePersonalHouseInterval` = `HousingControl_PurchaseKinshipHouseInterval` = 86,400, all of dictionary type int with no mapper (§9.7) | OBS3 (dictionary; every class-1210 set; the one `HousingControl` record) | Confirmed 2026-10-09 |
| MD50 | House upkeep is due weekly (§9.7) | PUB | Reported |

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
| `HousingControl` | 0x7000022C | 1184 | Housing prices, upkeep, limits (§9.7) |

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
| Landmark (map-note entity) | `WeenieType` label `Landmark` together with `MapNote_Type` (all 2,733; MD44). The label is read through the enum mapper the dictionary names for `WeenieType` (0x230000C9 here, MD49; PS §7.1). `Landmark_Music` is in 2,731 of them, so it is not a sufficient test | 1210 | 2,733 |
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
| quest (class 1398) | `QuestEvent_LandmarkDID` (in objective and loot structures) | landmark (3,419 of 3,434 values; the other 15, 8 distinct DIDs, name no record, MD45; see the note below the table) | 3,434 |
| map-note row (§4) | entity slot | landmark, NPC, item and other entity classes | 13,485 |
| area | `Area_Territory` → territory `Area_Region` → region | WS §11.2 | 675, 86 |
| dungeon | `Dungeon_MapData`, `Dungeon_MiniMapData` | 0x78 set with `UI_Map_GameMap` | 1,303 + 1,303 |

`QuestEvent_LandmarkDID` is the one edge of this table with values that name no record (MD45).
**Tessera rule** (not a fact about the client): a reader treats such a value as "no landmark",
keeps the quest record, and reports the value as a warning (MI14).

> **OPEN:** what the original client does with a `QuestEvent_LandmarkDID` value that names no
> record (for example in the quest's objective display or tracking). The data cannot show it
> and no published source states it; until it is specified the Tessera rule above applies.

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
2 or more (BIN q1); otherwise the row ends after `discovery`. That version is not stored in the
archives: the client takes it from a table of current record-kind versions compiled into its own
executable, where this record kind has version 2 in the build of the provenance table (MD29). So
every row read from the archives has `extra` (all rows of this install decode exactly with it),
and readers always read it. A writer always writes it.

`flag_b` (MD28): the client checks it as a boolean when reading and copies it with the row, but
neither the radar nor the map window (§4.8) reads it, so it has no visible effect. Readers keep
it for rewriting and otherwise ignore it.

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
- it is a **reference** of mode 2 (PS §17.4 step 1): the token is not 0 and the table DID is not
  0; nothing is looked up;
- it is any other **reference**: the token is not 0, the table DID is not 0, the string table
  with that DID exists in the active locale's archives, the table holds the token, and every
  argument of the argument table (if any) passes its own check (below).

So a reference fails when it is null (token 0 or table 0, PS §4.8), when its table is missing,
or when the token is missing from the active locale's table. An entry whose text is empty still
passes: the result of rendering is never looked at. In this install all 10,731 non-link texts
are references with no arguments that resolve in the English tables; 87 of them resolve to an
entry with empty text, and pass.

**Argument check** (MD30, BIN q11), by the entry's argument kind (PS §17.4.1):

| Kind | Check |
|---|---|
| 1 (nested StringInfo) | Passes when the nested StringInfo passes this whole test, arguments included (recursively, up to the nesting limit of PS §5.3) |
| 2–7 (floats and integers) | Always passes |
| 0, 8–65,535 (entry without a value) | Skipped: it neither passes nor fails the text |

This is the deep form of the validity test of PS §17.4.2. No StringInfo in this install has
arguments or mode 2, so only the null and resolving cases occur.

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

How the radar and the map window use these lists is §4.8.

<a id="s4-7"></a>
### 4.7 Range test in the radar (BIN, MD27)

`min_range` and `max_range` are distances in world units (CO §3). The client's radar (GP §8)
applies them to the notes it places (BIN q9):

1. A note is **ranged** when |`min_range`| > 0.0002 and |`max_range`| > 0.0002. Other notes are
   not range-tested.
2. For a ranged note, the radar takes the straight-line three-dimensional distance *d* between
   the note's position and the radar's reference position, both expressed in one world frame
   (CO §5–§7). The **reference position** is the player's position as the radar last recorded it
   (MD35, BIN q13). The radar records the player's position when the radar is set up and refreshes
   the record as the player moves, at the latest once the player has moved farther than a small
   threshold distance of the radar's own from the recorded position. The client keeps two
   records of the player's position; a user option for smoother radar movement (on by default)
   selects which of them the radar uses. Both are the player's position. While the recorded
   position is invalid (§4.4.1 tests 1–4), ranged notes are not range-tested.
3. The note is hidden when *d*² > `max_range`² or *d*² < `min_range`², computed in f32. So it
   shows when `min_range` ≤ *d* ≤ `max_range`, both ends included.
4. A shown ranged note is drawn at the rim of the radar, in the horizontal direction from the
   player to the note, and the radar passes the horizontal distance to the note's label.

With the stored values, ranged notes show between 80 and 800 units (811 rows), 80 and 640 units
(23 rows), and so on (§4.5). This ties in with §4.6 step 3: the notes with both ranges non-zero
are the ones kept in the separate list of ranged notes.

The **map window** (the world-map panel, GP §9) applies no range test: its note pass (§4.8.4)
reads neither range, so ranged notes appear there like any other note (MD35). In the explorer,
which has no player, Tessera uses the camera's position as the reference.

<a id="s4-8"></a>
### 4.8 How the client selects and draws notes (BIN, MD31–MD34)

Two views of the client use the map-note table: the radar (GP §8) and the map window (GP §9).
Both ask the map-note manager for notes with the queries of §4.8.1, which apply the tests of
§4.8.2, and then filter and draw the result (§4.8.3, §4.8.4) with the icons of §4.8.5 and the
world-event states of §4.8.6. Neither view reads `level` or `flag_b`: **`level` has no effect
on what either view shows** (its only use is the validity check of §4.4).

<a id="s4-8-1"></a>
#### 4.8.1 Queries (MD31)

1. **Area query**: a region *R* and an inclusive rectangle of landblocks (X from *x*₀ to *x*₁, Y
   from *y*₀ to *y*₁), normally the landblocks the view covers. The result is, in this order:
   every ranged note (§4.6 step 3) whose position lies in region *R*, whatever its landblock;
   the notes filed under (*R*, landblock) for each landblock of the rectangle (§4.6 step 2); then,
   for each distinct non-zero `link_target` of the link rows among those landblock notes, the
   notes filed under that zone (§4.6 step 1).
2. **Zone query**: a zone DID *Z*. The result is the notes filed under *Z*, then, for each
   distinct `link_target` of the link rows among them other than *Z*, the notes filed under that
   zone.

The extension through link rows is one level deep: the notes added for a linked zone are not
themselves followed. A note enters the result only when it passes both tests of §4.8.2; link rows
are tested and returned like other notes.

**Radar** (MD42). The radar queries from its recorded position (§4.7), and only while it holds a
map georeference (the one with which it places notes, GP §8.6.1); without one its result is
empty.

- **Recorded position outdoors (cell 0) or invalid** (§4.4.1 tests 1–4): an area query with
  *R* = the recorded position's region and this rectangle:
  1. *x*₀ = *bx* and *y*₀ = *by*, the block offsets of the georeference. Outdoors this is the
     georeference the radar takes from its tile grid (GP §8.6.1), normally the grid's
     south-west block.
  2. Take the position in block (*bx*, *by*) of the georeference's region with block-local offset
     (*W*/*s*, *H*/*s*), in f32, where *W* and *H* are the width and height in pixels of the
     element `MinimapMapImage` and *s* is the georeference scale in pixels per world unit (1.0
     instead when |*s*| ≤ 0.0002). Normalise it with CO §4 (C22) without clamping. *x*₁ and *y*₁
     are its block X and Y. When the normaliser fails, the block it leaves is used.
  3. The rectangle is *x*₀ … *x*₁ × *y*₀ … *y*₁, both ends included; an empty range gives no
     landblock notes.

  With *R* = 0 the query returns nothing (no row has region 0, MI8), so a recorded position that
  is invalid through its region shows no notes. Example: `MinimapMapImage` at its layout size
  2048 × 1536 (GP §8.1) and the tile scale 1.25 give offsets of 1638.4 and 1228.8 units, so
  *x*₁ = *bx* + 10 and *y*₁ = *by* + 7: a rectangle of 11 × 8 blocks with the georeference block at
  its south-west corner, larger than the 3 × 3 tile grid. The radar resizes `MinimapMapImage` to
  each dungeon image it shows (GP §8.5.2), so after a visit indoors the rectangle follows that
  image's size.
- **Recorded position in a cell:** a zone query for the dungeon it is showing (GP §8.5), when
  that dungeon is non-zero.

> **OPEN:** the rectangle for an invalid recorded position whose region is not 0 depends on the
> georeference the radar last set; after an indoor georeference its block offsets were not traced.
> **Tessera rule:** while the recorded position is invalid, the radar shows no table notes.

The map window makes an area query for an outdoor map node and a zone query for a dungeon map.

<a id="s4-8-2"></a>
#### 4.8.2 Discovery and content layers (MD31)

- **Discovery.** A note with `discovery` = 0 passes. A note with a non-zero `discovery` passes only
  when a player exists and the client's game logic reports that the player has discovered that
  key. Without a player such notes fail.
- **Content layer.** Let *L* be the player's `Volume_ContentLayer` (enum; 0 when absent or when
  there is no player), as in GP §8.4. The note passes when its `layers` value is an array and
  either the array is empty or some element *e* satisfies *e* = *L*, *e* = 2 (`All`), or *e* = 1
  (`InstanceZero`) with *L* = 0. Elements whose value type is neither enum nor property ID are
  ignored. A `layers` value
  that is not an array (or a null property) fails.

This layer rule is the map-note manager's own; it differs from the SED test of ED §4.5.1 (no
instance number; `InstanceZero` matches the layer 0 as well as 1). In this install 12,620 of the
13,485 rows pass for *L* = 0 and the same 12,620 for *L* = 1; the other 865 name only named layers
(0x1000xxxx).

<a id="s4-8-3"></a>
#### 4.8.3 Radar (MD32)

For each note of its query result the radar:

1. **Link rows** are not drawn. The radar collects the zones they lead into and fetches the
   dungeon quest notes of those zones (§5.3).
2. **Type filter.** Any other note is drawn only when its `type` shares a bit with the radar's
   note-type filter: a 64-bit mask kept by the radar (the user's radar note settings, GP §8.8)
   from which the bits of `PrivateEncounterTemplate_DisallowedMapNoteType` of the private
   encounter the player is in (`instances.md`) are removed.
3. **Placement.** A note whose two ranges both exceed 0.0002 in magnitude is a ranged note and
   follows §4.7; any other note is placed at its own position on the radar.
4. **Icon.** The images of §4.8.5 from the radar's table for the note's `type`; for a ranged note
   the entry for the value with only bit 45 set is used instead of the note's own `type`.
5. **World-event states** (§4.8.6) are applied last.

<a id="s4-8-4"></a>
#### 4.8.4 Map window (MD32)

1. The window queries as §4.8.1 says, with *L* of §4.8.2.
2. A note is kept only when its `type` shares a bit with the window's note-type filter (a 64-bit
   mask held by the window, set from the user's map filter controls, GP §9).
3. On an **outdoor map node** a note is kept only when the map node whose
   `UI_Map_AreaDIDs_Array` lists the note's `area` (GP §9.1) is the node being viewed or one of
   its descendants (the client walks parent links from the note's node towards the root). For a
   note with no `area` the client derives the area from the note's position instead (no row here
   lacks an `area`). The client registers each area once, when it builds its node tree (MD43):
   if a second node lists an area already registered, the area keeps the node registered first
   and the client reports the conflict; its registration order is that of its internal node
   table, not the tree order. No area is listed by two nodes in this install (GP §9.4). An area
   that no node lists maps to the **root node** (its active element `WorldMap`), so such a note is
   kept only while the root map is viewed.
   On a **dungeon map** every selected note is kept.

   **Tessera rule:** when two nodes list one area, the first in depth-first tree order (children
   in array order) keeps it, and a warning is logged.
4. Kept notes are drawn at their positions with the images of §4.8.5 from the window's table,
   then §4.8.6 is applied. The window does not read `min_range` or `max_range` (§4.7).

<a id="s4-8-5"></a>
#### 4.8.5 Icon tables (MD33)

Each view has an icon table: a stand-alone property set (0x78) found through UNIQUEDB (§2.1).

| View | UNIQUEDB key | Label | DID here | Entries here |
|---|---|---|---|---|
| Map window | 0x1000001B | `UI_MapNoteIcons_Map` | 0x78000001 | 46 |
| Radar | 0x10000013 | `UI_RadarNoteIcons_Map` | 0x78000002 | 47 |

Each set holds one property, `UI_Map_MapNoteTypeArray`, an array of structs:

| Field | Type | Present here | Meaning |
|---|---|---|---|
| `UI_Map_MapNoteType` | bitfield 64 | all | The `type` value the entry serves; matched against the whole 64-bit value |
| `UI_Map_MapNoteOnscreenImageStruct` | struct of `UI_Map_MapNoteImage`, `UI_Map_MapNoteSelectedImage`, `UI_Map_MapNoteHighlightImage` (DIDs) | all | Images of the note's normal, selected and highlighted states |
| `UI_Map_MapNoteOffscreenImageStruct` | struct of the same three DIDs | 14 of each table | The same three images for the note's off-screen marker |
| `UI_Map_MapNoteMenuImage` | DID | all | A further image; not used when notes are placed |
| `UI_Map_MapNoteRelativeZLevel` | int | 7 of each table | Added to the drawing level of the note's icon (absent = 0) |

The client reads the table at start-up, before the map-note table, keyed by `UI_Map_MapNoteType`
(the first entry wins for a repeated value; none is repeated here). For a note it looks up the
`type` value: an entry gives its images; no entry gives empty images, so the note shows no icon.
In this install every entry has exactly one bit set: bits 1, 16–23, 25–60 and 63 in both tables,
and bit 24 in the radar's only. Every non-link row's `type` has an entry in both tables except
bit 62 (11 rows); link rows (bit 0) have none.

<a id="s4-8-6"></a>
#### 4.8.6 World-event states (MD34)

A row whose `extra` holds `MapNote_GameSpecific` with a `WorldEvent_MapNoteStateArray` (§4.5)
depends on world state:

1. The client evaluates the states in array order. The condition of a state is the first present
   of `WorldEvent_Condition`, `WorldEvent_AllConditionList` and `WorldEvent_AnyConditionList`; their
   meaning is that of `quests.md` §14 (world events).
2. At the **first state whose condition holds**, the note is shown, and each of the following that
   the state carries replaces the corresponding value: `MapNote_Icon` the normal image,
   `MapNote_SelectIcon` the selected image, `MapNote_MouseOverIcon` the highlighted image,
   `MapNote_OffScreenIcon` the off-screen marker's normal image, and `MapNote_StringInfo` the row's
   `text`.
3. When no state's condition holds, or the array is empty, the note is **hidden**.
4. The manager indexes such rows by the world properties that their conditions name (through each
   named world event's `WorldEvent_WorldPropertyName`) and evaluates them again when one of those
   properties changes.

The other fields that occur in `MapNote_GameSpecific` (`Entity_ContentLayers`, `MapNote_Enabled`,
`Usage_QuestRequirements`) are not read by these paths. Census: 663 rows have a state array (485
with one state, 170 with two, 8 with three); each of the 849 states has exactly one condition form
(`WorldEvent_Condition` 364, `WorldEvent_AllConditionList` 473, `WorldEvent_AnyConditionList` 12);
659 states carry all four icons and 11 all but `MapNote_OffScreenIcon`; 843 carry
`MapNote_StringInfo`; 6 carry only a condition.

> **OPEN:** (a) the initial contents of the radar's and the map window's note-type filters and how
> the filter controls change them (`UI_Map_MapNoteFilterProperty`,
> `UI_Map_IncludeCommonMapNoteTypes`, `UI_Map_MapNoteDisplayType`, GP §8.8); (b) which discovery
> keys a player has discovered (player state supplied by the server; the client only asks its game
> logic); (c) what the row's `icon` field changes (it is copied with the note; its use was not
> traced). (a) and (c) can be settled by observing the original client (owner); (b) is server
> state. The former item (d), the default map node for unlisted areas, is answered in §4.8.4 step 3
> (MD43). **Tessera rule:** §14.1.

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
- The radar (§4.8.3 step 1) fetches the rows filed under each zone that a link row of its query
  result leads into, keeping those that pass the content-layer test of §4.8.2; the discovery check
  does not apply to them (MD37, BIN q12).

> **OPEN:** which of the fetched quest notes the client finally shows, and with which icon: that
> depends on the player's quests and on matching `QuestDispenser_RoleConstraint` and the role
> names, which are player state supplied by the server. Observing the original client (owner) can
> show the result for given quests, not the rule.

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
entity's. Of the 2,733 landmarks (class 1210), 2,731 also carry `Landmark_Music` (enum) and
the two `Landmark_OverrideMountedCombat_…` booleans, always all three together; the other 2
carry none of them (MD44, MD49). A reader treats all three as optional. Landmarks are named as
objective targets by quests (`QuestEvent_LandmarkDID`, §2.4).

For table rows the client never consults the entity's map-note properties (MD36, BIN q12, q14):
the note the views select and draw (§4.8) is built from the row's fields alone. So the 21 + 13 +
232 + 60 + 50 differences above have no effect on what the client shows; the row wins. (Where the
differences come from lies in the world-building process, outside both the data and the client.)

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

- 1,300 of the 1,616 stand-alone property sets (0x78) of `client_gamelogic.dat` hold
  `UI_Map_GameMap` (the install has 1,618 0x78 sets in all: 2 more in `client_local_English.dat`,
  neither a map set; MD46) (fields
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
> landblock is continued (LB §4.6 leaves the link ids and block pair OPEN), where a segment ends
> (the next segment's head, or a node without a further link), and whether the
> `Entity_ContentLayers` of SegmentNode SEDs (42,824) prune paths. The client executable does not
> read `TravelSegment_HeadName` or `TravelSegment_SubSegmentArray` by property ID (MD38), and the
> class library's code does not name them either (MD41), so the rule is server behaviour or lives
> in other data-driven logic; it cannot be settled from the client binary. Observing a ride in
> the original client (owner) can show the path taken.

<a id="s8-5-2"></a>
#### 8.5.2 Hop (class 1501 here; Confirmed)

`TravelHop_Delay` (float; 0 in 10, 3 in 1) and, in 9, `TravelHop_SceneID` (a scene record,
class 1468). A hop is a pause or a scene between two parts of a route.

> **OPEN:** the unit of `TravelHop_Delay` (seconds is likely) and what the scene record does.
> Neither the client executable (MD38) nor the class library's code (MD41) names
> `TravelHop_Delay` or `TravelHop_SceneID` by property ID; both are taken to be server-side
> travel rules. Timing a hop in the original client (owner) can
> settle the unit.

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

> **OPEN:** units of the five floats (metres and degrees per second are likely) and how the
> rider is moved along a path with them. Neither the client executable (MD38) nor the class
> library's code (MD41) names any `TravelMode_…` property by property ID, so the movement is
> driven from outside both (server or other data-driven logic). Observing a ride in the original
> client (owner) can bound the speeds and turn rates.

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
- **The travel window takes its values from script code** (BIN, MD40). The client's travel
  window (the stable-master panel, top-level element `Travel_Window`) asks the class library's
  script code for the destinations of a travel web (function 0x01CC0001) and, per destination,
  for the values it shows (function 0x01CC0003); `wlib-bytecode.md` §6.4 gives how such a
  function is started. The cost element (`Travel_Currency_DestinationCost`) shows the returned
  cost; when the cost is 0 the window shows `Travel_Currency_DestinationCost_Free` instead. The
  minimum-level text (`Travel_Currency_MinLevel`) shows a returned level (a fixed text instead
  when that level is below 2).
- **The script code names the price properties** (OBS, MD41). The class library pushes the
  property IDs of `Travel_DiscountArray`, `TravelWebSellMultiplier` and `Travel_BypassUsageChecks`
  in three bodies that the static calls of function 0x01CC0003 reach, and the IDs of
  `TravelDestinationAllowPaperItems` (in code of the destination class) and `TravelDestinationCost`
  elsewhere. The client executable itself names none of the four price properties (MD38).
- **The window follows the player's discounts** (BIN, MD39). It rebuilds its list when the local
  player's `Discount_Travel_Special`, `Advancement_Level`, `Travel_BypassUsageChecks`, two
  Ettenmoors population properties or `Shapeshift_MeshIID` change, among others; it does not read
  those values itself. No other `Discount_Travel_…` property is named by the client executable
  except as a UI element ID of equal value (MD38), and none is named by the class library (MD41).

So the price the original client shows comes from the class library's script code, whose call
graph names the discount array and the sell multiplier. That code is client-side data in the
user's install, not server software.

> **OPEN:** the price formula: how the discounts (`Travel_DiscountArray` names PIDs of float
> properties, all labelled `Discount_Travel_…`, 46 distinct) and `TravelWebSellMultiplier` change
> `TravelDestinationCost`, what `TravelDestinationAllowPaperItems` allows, and whether the script
> computes the price or passes on a value it obtains through a native function. It can be derived
> by reading function 0x01CC0003 and the bodies it reaches with `wlib-bytecode.md`, once the native
> functions they call are specified (`wlib-bytecode.md` §7 leaves them open). Whether the server
> charges the price the window shows is server state. Also open: how the 26 routes listed in
> `TravelHomeLocationsArray` are meant, and the meaning of the `Usage_QuestStatus` and
> `Usage_Operator` values. A local server needs its own rule until then (§14.3). Prices shown by
> the original client's stable-master panel (owner) can test a candidate formula.

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
> `RolloutStep` drive the number of live copies). It is server behaviour: neither the client
> executable (MD38) nor the class library's code (MD41) names these properties by property ID,
> and it cannot be observed from one client. A
> local server needs its own rule (§14.3).

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

`HousingControl` (one record, class 1184 here) carries the price and upkeep tables a server
needs (MD47; value types from the dictionary, every field present in every entry unless a count
is given):

| Label | Type | Content |
|---|---|---|
| `HousingControl_HouseTypeInfoArray` | array of `HousingControl_HouseTypeInfoEntry` (37) | `HousingControl_HouseType` (enum, mapper 0x23000257), `HousingControl_HousePrice` (int), `HousingControl_HouseUpkeep` (int), `HousingControl_Icon`, `…_Icon16`, `…_Icon32`, `…_Icon_Large`, `…_Icon_Panorama` (DIDs → 0x41), `HousingControl_SkillDeco_Mount_Tokens` (int, 21 of 37) |
| `HousingControl_HouseValueTierArray` | array of `HousingControl_HouseValueTierEntry` (10) | `HousingControl_HouseValueTier` (enum, mapper 0x23000259), `HousingControl_HouseValueTier_PriceModifier` (float), `HousingControl_HouseValueTier_UpkeepPriceModifier` (float). The dictionary also defines `HousingControl_HouseValueTier_StoragePriceModifier` (float), but it is not an allowed field of the entry and no set carries it |
| `HousingControl_HookRotationArray` | array of `HousingControl_HookRotationEntry` (14) | `HousingControl_HookCategory` (64-bit bitfield, mapper 0x23000264), `HousingControl_RotationType` (enum, mapper 0x23000271) |
| `HousingControl_HousingGroupInfo_Personal`, `…_Kinship` | arrays of `HousingControl_HousingGroupInfo_Struct` (3, 5) | `HousingControl_HousingGroupInfo_Group` (enum, mapper 0x2300025A), `…_DefaultPermission` and `…_CantChangePermission` (32-bit bitfields, mapper 0x2300025B) |
| `HousingControl_PermissionTypeIcons_List` | array of `HousingControl_PermissionTypeIcons_Struct` (6) | `HousingControl_PermissionType` (32-bit bitfield, mapper 0x2300025B), `…_PermissionTypeIcon`, `…_PermissionTypeActiveIcon`, `…_PermissionTypeInactiveIcon` (DIDs → 0x41) |
| `HousingControl_UpkeepInterval`, `HousingControl_PurchasePersonalHouseInterval`, `HousingControl_PurchaseKinshipHouseInterval` | int | Intervals: `HousingControl_UpkeepInterval` 604,800 here, both purchase intervals 86,400 here (MD49); unit OPEN (below) |
| `HousingControl_MaxPlayersPerNeighborhoodInstance`, `HousingControl_MinLevelToBuyHouse`, `HousingControl_InitialUpkeepCredits`, `HousingControl_MaxUpkeepCredits`, `HousingControl_Permissions_MaxEntries` | int | Limits |
| `HousingControl_HouseRestoralUpkeepMultiplier`, `HousingControl_MaximumRestoralFeeMultiplier`, `HousingControl_PremiumAbandonRefundPercent`, `HousingControl_PremiumForeclosureRefundPercent`, `HousingControl_MithrilToWritConversionRate`, `HousingControl_AbsoluteBankingRestriction` | float | Factors and rates |
| `HousingControl_ForSaleSignDataID`, `HousingControl_StorageKeeperDataID`, `HousingControl_MountSkillDecoTokenDataID`, `HousingControl_PetSkillDecoTokenDataID`, `HousingControl_PremiumWritDataID` | DID → 0x70 | Objects the system uses |
| `HousingControl_PremiumWritIcon` | DID → 0x41 | Icon |
| `HousingControl_NeighborhoodContentLayer` | enum of DID map 0x28000012 | A content layer; the same named layer appears in 108 map-note layer lists |

Values are data and are read at run time.

> **OPEN:** the unit of the three `HousingControl_…Interval` values and what each one times
> (the upkeep period; a waiting time between house purchases is a guess from the names only).
> No record and no published source states the unit. Published player guides say upkeep is due
> weekly (MD50, Reported), which 604,800 would match if the unit were seconds; that is a
> consistency, not a source, and the purchase intervals have no such reference.

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
`RipCord_WorldPropertyArrayElement`, both PIDs), a category (`RipCord_Category`, enum mapper
0x2300030B), a name and help string (`RipCord_Name`, `RipCord_Help`, Latin-1 strings), a UI
element in one layout (`RipCord_UIElement_ID`, an enum of mapper 0x230000D1;
`RipCord_UIElement_Layout`, a DID of a UI layout), and `RipCord_DisableEditingInAdminPanel`
(bool). Every field is in every one of the 348 records, the two class-3166 fields in its 7
(MD48). They are administrator
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
| MI14 | Every DID edge of §2.4 points to a record of the listed kind, except `QuestEvent_LandmarkDID`: its values point to a landmark or name no record (15 values, 8 distinct DIDs here, MD45); a value naming no record is a warning (Tessera rule of §2.4), a value naming a record of another kind is a violation. |
| MI15 | Every position of the three tables, and every `link_position`, passes the position test of §4.4.1. Places: no name contains a code unit 0 (§3.3.1). |
| MI16 | Icon tables (§4.8.5): both UNIQUEDB keys resolve to a property set holding `UI_Map_MapNoteTypeArray`; every element has `UI_Map_MapNoteType`, `UI_Map_MapNoteOnscreenImageStruct` and `UI_Map_MapNoteMenuImage`; no `UI_Map_MapNoteType` value repeats within a table (a repeat is a warning; the first entry wins). |
| MI17 | World-event states (§4.8.6): every state of a `WorldEvent_MapNoteStateArray` has exactly one of `WorldEvent_Condition`, `WorldEvent_AllConditionList`, `WorldEvent_AnyConditionList` (a state with none is a warning; with several, the first in that order is used). |

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
| `00 00 00 00 20 00 00 00` | `type` with only bit 37 set, 2³⁷ (key 0x26) |
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
- Map notes (§4): select and draw valid rows as the map window does (§4.8.1, §4.8.4), with the
  content-layer test of §4.8.2 for the layer being viewed (*L* = 0 by default), the icons of
  §4.8.5 from the map window's table and `text` rendered by ST §9; `level` and `flag_b` play no
  part. **Tessera rules** for what §4.8 leaves to player and UI state (the OPEN of §4.8.6): the
  type filter starts with every bit set and is a user toggle per `type` bit (labels of §6.3);
  rows with a non-zero `discovery` are shown and marked "discovery"; rows with world-event states
  are shown with their own values and marked "world event" unless the explorer offers a world
  state to evaluate §4.8.6 with; a row's `icon` field is ignored. Draw link rows (which have no
  icon entry) as arrows from the row's position to the `link_target` zone's map, at
  `link_position`.
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
  Discounts and sell multipliers are OPEN (§8.8: the original takes the shown price from
  class-library script code); **Tessera rule:** ignore them until that formula is specified.
- Housing: one neighbourhood per `Neighborhood` record, created on first entry, holding the
  template's houses; entrance and exit by the template's place keys. Prices per §9.7 (Reported).

<a id="s14-4"></a>
### 14.4 Implementer notes

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07; engineering choices where the spec is silent, recorded
> for review):** (a) The places-table entry count is bounded by remaining bytes / 10 (smallest
> entry: a 1-byte name count, a 1-byte position, two u32 slots). (b) The dungeon quest-note row
> count is bounded by remaining bytes / 19 (a 1-byte position, three DIDs, a null `layers`
> property, an empty set). (c) The ripcord fields whose value type §11 does not give
> (`RipCord_WorldPropertyTableProperty`, `RipCord_WorldPropertyArrayElement`,
> `RipCord_UIElement_ID`, `RipCord_UIElement_Layout`) are read as integer bit patterns. Which
> dictionary types do they have? (d) No typed view of `HousingControl` (§9.7) is implemented:
> the value-tier modifier labels are elided (`…_PriceModifier` and the others) and the value
> types of prices, upkeep and modifiers are not given. What are the full labels and types?
>
> *Answer:* (a) and (b) are confirmed: both bounds follow from the smallest possible entry and
> are safe allocation limits. (c) The four fields are PIDs (the two class-3166 fields), an enum
> of mapper 0x230000D1 and a DID; §11 now gives every ripcord field type (MD48). (d) §9.7 now
> has a table of every `HousingControl` label with its type (MD47); the stated
> `…_StoragePriceModifier` was wrong: it is defined but never stored.

> **OPEN:** (implementer, 2026-10-09; engineering choices where the spec is silent, recorded
> for review) (a) The landmark rule of §2.3 needs the `WeenieType` key labelled `Landmark`;
> `crates/formats/src/gameplay/map.rs` (`classify_map_kind`, `MapWeenieKeys`) classifies no set
> as a landmark when the mapper chain lacks that label, with no fallback to `Landmark_Music`.
> (b) The `HousingControl` and ripcord views (§9.7, §11) read a field of another value type as
> absent instead of failing the record. Confirm or replace.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07; real-data census differs from the spec):** (a) §2.3
> landmarks: recognising them by `MapNote_Type` and `Landmark_Music` finds 2,731 sets, while
> §2.3 counts 2,733 landmark records. Measured: 2,731 sets carry `Landmark_Music`, all of them
> with `MapNote_Type`; the 2,731 recognised records share one root class (taken from the
> records, not from §2.3), and of the 16,720 sets that carry `MapNote_Type` without
> `Landmark_Music`, 2 belong to that class. Records of the class that carry neither property
> were not searched (the test reads root classes only of records it already selected), so
> the 2 + 2,731 = 2,733 match is consistent with §2.3 but not a full class census. Which
> property rule recognises every landmark without the class index (`MapNote_Type` alone also
> occurs on other entity classes)? (b) §7: the install has 1,618 stand-alone (0x78) sets,
> not 1,616; the 1,300 sets holding `UI_Map_GameMap` match. Which count is meant? (c) §2.4
> `QuestEvent_LandmarkDID`: all values in quest records (any depth) number 3,434; 3,419 name
> landmarks, and 15 (8 distinct DIDs) name no record in the install. Does the 3,419 leave out
> references to missing records, and should a reader treat such a value as "no landmark"?
> **Tessera choice** until answered: the census reports them; they are not an MI14 failure.
> Every other census count of §2.4, §3.5, §4.5, §5.3, §7, §8 (including the SegmentNode counts
> of §8.5.1), §9 and §11 that the test checks reproduces.
>
> *Answer:* (a) Class 1210 has exactly 2,733 records, all with `MapNote_Type`; 2 lack
> `Landmark_Music`. The property rule that finds all 2,733 and nothing else is `WeenieType`
> label `Landmark` together with `MapNote_Type` (MD44; one other set has that `WeenieType` but
> no `MapNote_Type`); §2.3 now states it. (b) Both: 1,616 is the count of
> `client_gamelogic.dat`, the install has 2 more in `client_local_English.dat` (MD46); §7 now
> says so. (c) The 3,419 counted only values that name a landmark; the 15 dangling values (8
> DIDs) were left out. §2.4 now gives 3,434 values with the split (MD45). Treating a missing
> target as "no landmark" is a **Tessera rule** (§2.4), the same as the Tessera choice above,
> and MI14 reports such values as warnings; what the original client does with them is OPEN
> (§2.4, §15 item 11). *(Wording revised 2026-10-09 after review.)*

---

<a id="s15"></a>
## 15. Open questions (summary)

Closed on 2026-10-08 (former items 1, 4, 12 and 13 of the previous list, and most of 2): `flag_b` and the version
gate (§4.2), the argument check (§4.4.2), the radar's reference position and the map window's
range behaviour (§4.7), how the views select, filter and draw notes (§4.8), and the entity's
properties versus the row (§6.2). Also closed in the revision after review (2026-10-08): the
default map node for unlisted areas (§4.8.4, MD43). Remaining:

1. §4.8.6: the initial note-type filters and the filter controls; the discovered state; the row's `icon` field (owner; discovered state is server state).
2. §5.3: which fetched dungeon quest notes the client shows (server-side quest state).
3. §8.5.1: the walking rule along SegmentNode links, and segment ends (server or data-driven logic; owner can observe a ride).
4. §8.5.2, §8.6: units and use of hop delay, hop scene and travel-mode values (same).
5. §8.8: the price formula (discounts, sell multiplier, paper items), which the original takes from class-library script code (readable with `wlib-bytecode.md` once its native functions are specified); routes listed as home locations and usage-requirement values (server rules).
6. §9.3: neighbourhood instancing rule (server).
7. §9.6: how `House_…` links name houses and hooks (waits for LB §4.6).
8. §11: which ripcords gate travel or map features (server administration).
9. §3.3.1: the lower-case table for non-ASCII code units under the client's locales (operating system).
10. §4.8.1: the radar's area rectangle for an invalid recorded position with a non-zero region, after an indoor georeference (Tessera rule given).
11. §2.4: what the client does with a `QuestEvent_LandmarkDID` value that names no record (Tessera rule given: "no landmark", a warning under MI14).
12. §9.7: the unit of the three `HousingControl_…Interval` values and what each times (server rules; upkeep is reported weekly, MD50).

---

<a id="s16"></a>
## 16. Notes for other specs

- `records.md` §2.1, prefix 0x0E: the layouts of 0x0E000004, 0x0E000006 and 0x0E400000 are now
  given here (Confirmed); the UNIQUEDB labels are `PlacesTable`, `MapNotes` and
  `DungeonQuestMapNotes`; the `SegmentTable` entry (0x0E000002) names a record that does not
  exist in this install.
- `game-panels.md` §8.8: §4.6 here gives how the client files notes after loading, §4.7 the
  radar's range test and reference position, and §4.8 how the radar and the map window select,
  filter and draw table notes (content layers, discovery, type filters, the two icon tables,
  world-event states). The filter defaults and controls remain OPEN (§4.8.6 here).
  §4.8.1 here gives the radar's area-query rectangle, which is derived from the georeference and
  the size of `MinimapMapImage` and is larger than the 3 × 3 tile grid of §8.4.1 (MD42); §4.8.4
  gives the map window's area-to-node registration, with the root node as the default (MD43),
  which bears on the §9.4 OPEN about areas in no node.
- `instances.md`: `PrivateEncounterTemplate_DisallowedMapNoteType` removes note types from the
  radar's filter while the player is in that private encounter (§4.8.3 here).
- `property-system.md` §17.4.2: the argument check of the deep validity test is given per kind
  in §4.4.2 here.
- `landblock.md` §4.6: links of type 9 join SegmentNode SEDs (§8.5.1 here), which bears on the
  OPEN about link ends.
- `wlib-bytecode.md` §6.4 and §10: the travel window starts class-460 functions 0x01CC0001 and
  0x01CC0003 by function ID; the second reaches the class-1507 bodies that name
  `Travel_DiscountArray` and `TravelWebSellMultiplier` (MD40, MD41, §8.8 here).
- `ui-layout.md` / `game-panels.md`: the travel window's elements named in §8.8 here
  (`Travel_Window`, `Travel_Currency_DestinationCost`, `Travel_Currency_DestinationCost_Free`,
  `Travel_Currency_MinLevel`) belong to a panel no spec yet describes.

---

<a id="changelog"></a>
## Change log

| Date | Change | By |
|---|---|---|
| 2026-10-07 | First version: singleton tables (§3–§5), map-note properties (§6), map images (§7), travel (§8), housing (§9), place consumers (§10), ripcords (§11). Status Review. | Spec writer (dirty room) |
| 2026-10-07 | Revision after review (changes requested). Header: review issues moved below the header table, which they had split, with a resolution note. Provenance: BIN second use with questions q6–q9; rows MD24–MD27 added; MD4, MD8 and MD10 revised; unused **Probable** mark removed. §3.2: debug-dump labels paraphrased. §3.3: the full 16-bit lowered unit enters the hash; new §3.3.1 (per-unit, locale-dependent lower-casing, key ends at unit 0, Tessera rule, OPEN for non-ASCII). §3.4: lookup cites §3.3.1; the sentence on an untraced routine dropped. §3.5: code-unit census (296,009 units, none 0, five 0x1F). §4.2: units of the ranges. §4.4: new §4.4.1 (position test) and §4.4.2 (text test, OPEN for arguments). §4.6: link entry needs both positions valid; 2,590 of 2,754 link rows qualify. New §4.7 (radar range test). MI8 revised, MI15 added; §15 items 2 and 11–13; §16 note for GP §8.8. Re-measured over all records with a new probe: entry and row counts with 0 bytes left over (9,025, 13,485, 5,638), the flag-byte and region histograms of §3.5, §4.5 and §5.3, interior places (2,330) and orientations (7,351), the link-position flag bytes of §4.3, all eleven (`min_range`, `max_range`) pairs of §4.5, and level 0 exactly in the link rows; all reproduce, no count changed. Status stays Review. | Spec writer (dirty room) |
| 2026-10-07 | Second re-measurement after review, over all records (no text change outside this row): every position of the three tables and every `link_position` passes §4.4.1 (13,485, 2,754, 9,025, 5,638; largest \|w² + x² + y² + z² − 1\| 2.4 · 10⁻⁷); the 10,731 non-link texts are references without arguments that resolve in the English tables, 87 of them to empty text (§4.4.2); 2,590 link rows get a zone entry and 164 lead into their own dungeon (§4.6); the eleven range pairs (12,611 / 811 / 23 / 40 in eight pairs, §4.5, §4.7); place names: 296,009 code units, 296,004 in 0x20–0x7F and five 0x1F, none 0, 168 with upper-case letters, no two equal ignoring case; 412 link rows whose `link_position` shares the row's landblock (§4.3); the §5.3 flag-byte and region lines. All reproduce; no count changed. Status stays Review. | Spec writer (dirty room) |
| 2026-10-08 | OPEN-closing revision (Status Revised). Provenance: OBS extended (icon tables, world-event states, layer-test counts); BIN third use with questions q10–q15; MD7 and MD27 revised; rows MD28–MD38 added. §4.2: the version gating `extra` comes from the client's own version table (2 here), `flag_b` has no display effect; OPEN closed. §4.4.2: mode-2 references and the argument check per kind; OPEN closed. §4.6: OPEN moved to §4.8 and narrowed. §4.7: the reference position is the player's position as the radar records it; the map window applies no range test; OPEN closed. New §4.8 (queries with link extension, discovery check, the map-note content-layer rule, radar and map-window filters, the two icon tables, world-event states; `level` unused by both views) with a narrower OPEN (filter defaults and controls, discovered state, the row's `icon` field, default map node). §5.3: the radar fetches quest notes of linked zones; OPEN made precise. §6.2: the client takes table notes from the row only; OPEN closed. §8.5.1, §8.5.2, §8.6, §8.8, §9.3: OPENs made precise (MD38: the client's code reads none of those properties). MI16 and MI17 added. §13.2 notation of the `type` value. §14.1 explorer rules follow §4.8. §15 and §16 updated. | Spec writer (dirty room) |
| 2026-10-08 | Revision after the review of the OPEN-closing revision (changes requested; status stays Revised). Blocking item: the third BIN use's search text now lists the properties actually searched; BIN fourth use (q16–q18) and a second OBS probe added. MD38 narrowed and re-stated: no operand and no 32-bit value anywhere in the executable for the `TravelMode_…`, `TravelHop_…`, four §8.8 and three instancing IDs; the 13 instructions with `Discount_Travel_…` operands classified (12 UI element IDs, one property use). New MD39 (the travel window watches the player's `Discount_Travel_Special` and other properties and rebuilds on change), MD40 (the travel window takes destinations and shown cost and level from class-library functions 0x01CC0001 and 0x01CC0003), MD41 (OBS, full instruction sweep: the class library names `Travel_DiscountArray`, `TravelWebSellMultiplier`, `Travel_BypassUsageChecks`, `TravelDestinationAllowPaperItems` and `TravelDestinationCost`; function 0x01CC0003 reaches the first three; no `Discount_Travel_…`, travel-mode, hop, segment or instancing ID). §8.8: three new bullets, the "server rules" conclusion replaced and the OPEN restated as the price formula in class-library code; §8.5.1, §8.5.2, §8.6, §9.3 cite MD41 beside MD38; §14.3 and §15 item 5 follow. Minor items: §4.8.1 radar query rewritten with the rectangle (MD42: georeference block offsets, `MinimapMapImage` size over the scale, CO §4 normaliser), the georeference gate, the empty result for region 0, and a narrow OPEN with a Tessera rule (§15 item 10); §4.8.4 step 3: first registered node keeps a repeated area, no repeat here, Tessera rule for repeats, unlisted areas map to the root node (MD43), so former OPEN (d) of §4.8.6 is closed. Editorial: §4.7 step 2 sentence rephrased and re-wrapped. §16 notes for GP, `wlib-bytecode.md` and the UI specs. | Spec writer (dirty room) |
| 2026-10-08 | Re-review of the revision after review: passed; status Accepted. No text change outside the header and this row; three non-blocking notes in the review log. | Spec reviewer (dirty room) |
| 2026-10-09 | OPEN sweep (status Revised, waiting for review). New provenance key OBS3 and rows MD44–MD48. Answered both implementer notes of §14.4: landmark recognition rule (`WeenieType` `Landmark` with `MapNote_Type`, §2.3, MD44); landmark references with 15 dangling values (§2.4, MD45); 0x78 count per archive (§7, MD46); `HousingControl` labels and types, with the correction that no storage price modifier is stored (§9.7, MD47); ripcord field types (§11, MD48); the two allocation bounds confirmed. Still OPEN: locale lower-casing, the radar's invalid-position rectangle, note filters and discovery, quest-note selection, travel paths, hop and mode units, travel price, instancing, house links, ripcord gating. | Spec writer (dirty room) |
| 2026-10-09 | Repair after the review of the OPEN sweep (changes requested; status stays Revised). Blocking: (1) MI14 excepts `QuestEvent_LandmarkDID`, whose values may name no record (warning), naming another kind stays a violation; (2) §6.2 no longer says every landmark carries `Landmark_Music`: 2,731 of 2,733 carry it and both `Landmark_OverrideMountedCombat_…` booleans, all three together, the other 2 none (MD49); (3) §2.4 "no landmark" marked as a Tessera rule with an OPEN on the client's handling; §14.4 answer (c) reworded to match; §15 item 11; (4) §9.7 interval unit moved from a guess to an OPEN, values given per field (MD49), published weekly upkeep added as Reported (new tag PUB, MD50); §15 item 12. Non-blocking: §2.2 `HousingControl` class 1184; §2.3 names the `WeenieType` mapper (0x230000C9 here). New rows MD49 (OBS3, own read-only probe over the 2026-10-07 dump) and MD50 (Reported). | Spec writer (dirty room) |
