<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Miscellaneous small records: compositors, UI scenes, imposter tables, compute shaders, script channels, text filters, compression dictionaries, bare property sets and metadata objects

| | |
|---|---|
| **Status** | Accepted (re-review of the 2026-10-09 repair, 2026-10-09: passed). History: OPEN sweep of 2026-10-09 (Revised); review 2026-10-09: changes requested; repaired 2026-10-09 (§20 last row) |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review sessions (spec-writer agent), none of them an author session: first and second review (2026-10-07), review of the 2026-10-08 revision |
| **Review record** | Second review 2026-10-07: **passed**, Accepted. Revision of 2026-10-08: reviewed 2026-10-08, **passed**, Accepted. Full records in the review log (decision 0018). OPEN sweep 2026-10-09 (spec writer, dirty room): normative text of §4.2, §12.2, §13 (introduction), §13.1, §13.5, §13.7 changed (row U2 and new rows X1–X5); status Revised until a separate session reviews it. Review of the 2026-10-09 revision (separate review session, not an author session, 2026-10-09): **changes requested**. Checked: provenance for every new fact (keys BC and OBS3, rows U2, X1–X5), no code or transliterated pseudo-code, no game text, consistency with `combat-data.md` §5.3 (mapper 0x23000046, key 8 `Multiply`), `wlib-bytecode.md` and `property-system.md`, and the closed OPENs. Spot checks, read-only, with an independent probe over the repository decoders: U2 (bit sums of the flag-word census: 0x010 in 3,026 of 3,051) reproduced; X1 (419 members; 237 / 120 / 21 / 9 / 32; 4 referrer-only objects) reproduced over all 383,332 0x79 sets; X3 (all eleven field types, the `UI_Map_FogOfWar` default false, the three §13.1 enum mappers) reproduced; X5 (4,112 matches in 795 object records, none found among the decoded values) reproduced; X4 read again in the class-library disassembly (class 1341 functions 9 and 12). **Blocking:** (1) X2 and the §13.7 answer say the control's number list names all 123 members and 35 other singletons. Observed: of the 93 structs of `Version_VersionNumberDataList`, 92 carry `Version_SingletonList` and one carries `AccountVersion_SingletonList` instead; the `Version_SingletonList` arrays name 155 distinct DIDs, covering 121 of the 123 members; the other 2 members are the 2 DIDs of `Version_AlwaysRunOnPlayers`; the control as a whole names 158 distinct DIDs (123 members and 35 others). Correct X2, the §13.7 answer and the §13.7 description of the structs. (2) §13.1 step 1 says *d* = 0 when *v* = 1. The bytecode (class 1341 function 9) changes *d* only when *v* > 1 or *v* < 1, so for *v* = 1 (and for NaN) *d* stays *v*: a `Multiply` by exactly 1 shows 100 with the increase template. Correct the step. Recommended in the same change: say that int and int64 values are shown unscaled whatever the flag (their cases of the value-type switch do not consult it), instead of leaving flag-with-non-float unstated. (3) The §13 introduction, edited in this revision, still says each family has one root class and every family except GameCallback has one control singleton named in WEENIECONTENT. That contradicts §13.5 (six root classes, no control; none of the 316 WEENIECONTENT labels is a property-response control), §13.6 (110 root classes) and §13.7 (31 member root classes). State root classes and controls per family (controls for §13.1–§13.4 and §13.7; none for §13.5 and §13.6). **Not blocking:** provenance rows are ordered X1, X2, X4, X5, X3; the header "Implements into" still says one fuzz target per prefix while the §19 answer (8) accepts one per spec. Repair of 2026-10-09 (spec writer, dirty room; every blocking figure re-measured before the change): (1) X2, the §13.7 description of the structs and its answer corrected; (2) §13.1 step 1 and row X4 corrected for *v* = 1 and NaN, and int and int64 values stated as unscaled; (3) the §13 introduction states root classes and controls per family (new row X6). Not-blocking items done too (rows in order X1–X6; "Implements into" says one fuzz target per spec). Status Revised until a separate session re-reviews it. Re-review of the repair (separate review session, neither an author session nor the first reviewer, 2026-10-09): **passed**, status Revised → Accepted. Checked against CLEAN_ROOM.md §5.3: every new or changed fact (rows U2, X1–X6, keys BC and OBS3) has a provenance row and a mark; no code, no pseudo-code and nothing that follows the bytecode listing (§13.1 states the rule as conditions and results, not as the listing's branch structure); no game text (template texts referred to by role only); consistent with §13.1–§13.7, §14 (W1), §16 MR11, `wlib.md` §6, `wlib-bytecode.md` and `combat-data.md` §5.3; the three blocking items of the first review are fixed and the closed OPENs (§4.2, §13.5, §13.7, §19 implementer notes, the scale part of §13.1, the 0x70 part of §12.2) are answered in the normative text. Spot checks, read-only (install archives unchanged since 2026-10-04; the class library and the WEENIECONTENT map extracted with `tessera-dattool` and parsed by the reviewer's own probes; the families counted by the reviewer's own probe over the 2026-10-07 object-record dump): X2 reproduced exactly (control class 1623; 93 structs, 92 `Version_SingletonList` and 1 `AccountVersion_SingletonList`, 93 distinct numbers; 155 distinct singleton-list DIDs = 121 members + 34 others; the 2 `Version_AlwaysRunOnPlayers` DIDs are members and in no number list; 158 in all = 123 + 35; none of the 35 carries `Version_*`, 33 have an empty set); X6 reproduced exactly (5,211 / 1, 675 / 1, 337 / 1, 85 / 1, 419 / 6, 110 / 110, 123 / 31; every control in a class of its own outside its members' classes and without the member property; WEENIECONTENT 316 entries, 314 0x70 entries naming 313 distinct objects, none a property-response or game-callback member or naming one at any depth); X4 read again in the disassembly (class 1341 function 9: the flag picks one of two template keys before the value-type switch; int and int64 cases ignore it; for float with the flag, `Multiply` changes *d* only when *v* > 1 or *v* < 1, then *d* × 100 in single precision widened to double; function 12: increase template when *v* ≥ 1 with the flag and a float, else decrease); X1 member and class counts reproduced. **Not blocking (for the next revision):** (a) the §13 introduction says a control "lacks the property by which its members are recognised"; that holds for §13.1–§13.4, but the Version control carries `Version_*` properties and is set apart by its control-only properties, as X6 and §13.7 say; the sentence could say so. (b) Row X4 cites "class 1392 function 95" and "class 1391 function 89", which are virtual-table slot numbers (function numbers 16 and 9), while the class 1341 numbers are function numbers (function 48 is slot 41); use one term. |
| **Implements into** | `crates/formats` (decoders for prefixes 0x33, 0x21, 0x27, 0x2D, 0x43, 0x0E200000, 0x0EBADA55, and the stored envelope of 0x0E010000–0x0E010002; fuzz targets that together reach every decoder of this spec, one per spec being enough, §19 answer (8)), `crates/material` (the compositor image composition of §3.6), `crates/props` (typed views of §12 and §13), `apps/explorer` (compositor and imposter previews, typed metadata views, the PID → metadata index of §13.1), `apps/dattool` (`info` and `validate` for these prefixes) |
| **Companion specs** | [`records.md`](records.md) §1 (VLE, TSize and bucket order, strings), §1.6 (prologues), §2.1 (DID → DB type table, whose BIN rows this spec closes); [`property-system.md`](property-system.md) §4 (value encodings), §5 (property sets), §8 (DID maps), §9 (DB-object-type codes); [`entity-descriptor.md`](entity-descriptor.md) §3 (the static entity descriptor reused by UI scenes); [`wstate.md`](wstate.md) §2, §3 (object records and the WEENIECONTENT map); [`script-table.md`](script-table.md) (script nodes 27 GameCallback, 33 ModifyCompositorColor, 44 ModifyCompositorLayer, 53 TextureCompositor); [`appearance.md`](appearance.md) §3.3–§3.4 (face compositing OPEN, character-creation controls); [`ui-layout.md`](ui-layout.md) §9.6.2 (the 2D compositor's per-pixel arithmetic, reused here); [`texture-list.md`](texture-list.md) §2.2; [`string-table.md`](string-table.md) §10.3 (prefix 0x58). |

---

<a id="purpose"></a>
## Purpose

This spec covers the low-volume record types that were still marked BIN or OPEN in
`records.md` §2.1, and the "metadata" object families inside the property-set and
object-state ranges. With it an implementer can:

- decode every record of prefixes 0x33 (image compositors), 0x21 (UI scenes), 0x27
  (imposter tables), 0x2D (compute shaders) and 0x43 (script channels) to the last byte,
  and the text-filter singletons 0x0E200000 and 0x0EBADA55 (§3–§7, §9, §10);
- **compose a face or caparison texture** from a compositor record, a per-layer selection
  and a set of colours, with the client's arithmetic (§3.6), for the explorer's compositor
  preview and, later, for avatars (`appearance.md` §3.3);
- map imposter atlas cells to texture coordinates for an imposter preview (§5.4);
- show typed views of the 0x78 bare property sets (§12) and of the metadata objects in the
  0x70 range: property metadata (display names, percentage flag, auction category,
  allowed operators) indexed by PID, error categories, tutorial hints, the movie library,
  property responses, game callbacks and version objects (§13);
- resolve the well-known "control" singletons through the WEENIECONTENT DID map (§14) and
  follow the references between all of these records (§15).

The compression dictionaries (0x0E010000–0x0E010002) are described in **stored form only**
(§11). Their use belongs to the network protocol, which was behind the M8 gate when this
spec was written (decision 0004) and since 2026-10-11 belongs to M8's own specs
(decision 0034). The locale record 0x58000000 is fully specified in
`string-table.md` §10.3 and is only cross-referenced here (§8).

Game text is never reproduced: records are named by DID, fields by property label or by
enum-mapper and DID-map labels, which are quoted for interoperability (they are how a
client finds the records at run time).

---

<a id="provenance"></a>
## Provenance

How sources are marked:

- **Confirmed**: decoded on the user's own install with throwaway probes in the dirty-room
  scratch directory; each fact says how many records were checked. Unless a row says
  otherwise, **every** record of the type was decoded and the decode consumed it to the
  last byte.
- **Reported**: from published sources only.
- **BIN**: read from the client executable under method M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)).
  "Confirmed (BIN + OBS)" marks a fact that the data shows and the client's reader settles.

| Key | Source |
|---|---|
| OBS | Observed in the user's install on 2026-10-06/07 (Steam install, client data current on that date). Every record of prefixes 0x21 (20), 0x27 (507), 0x2D (3), 0x33 (24), 0x43 (1), 0x0E (8: 7 in `client_general.dat`, 1 in `client_local_English.dat`), 0x58 (1) and 0x78 (1,618: 1,616 in gamelogic, 2 in the English archive) was read; the property sets of all 383,343 object records (0x70 and 0x76, set at DID + 0x09000000) and of all 40,643 entity descriptors (0x47) were scanned for the references of §15; all 24 DID maps and all 735 enum mappers were decoded for labels; the 5,954 distinct non-zero DIDs named by compositors were looked up (the 2,977 distinct `image_did` and `mask_did` values all exist and their surface headers were read; none of the 2,977 distinct non-zero `legacy_did` values exists; the two sets do not overlap), and the 600 texture lists named by imposter tables were looked up and their surface headers read. Re-measured on 2026-10-07 after the first review: all 24 compositors (payloads read again through `tessera-dattool cat`, byte-identical to the first read) and the surface headers of the 0x33000017 overlay options; all 7,526 script tables (0x07) and 28,992 scriptlets (0x0C) byte-scanned for script nodes 33 and 44 (218 and 199 found, the counts of `script-table.md`). The probes used the project's own decoders read-only plus independent Python parsers written from the hypotheses of this spec. Re-measured a second time on 2026-10-07 in a separate session, from scratch: all 24 compositors (payloads read through `tessera-dattool cat`), the surface headers of all 2,977 distinct `image_did` and `mask_did` values, the directory entries of all 30 client archives (3,838,957 entries, 1,408,052 of them 0x41) for the 2,977 distinct non-zero `legacy_did` values, all 7,526 script tables for nodes 33 and 44, and all 40,643 entity descriptors for `AppearanceUI_CompositorFile`; every count of §1–§3 and C1–C9 reproduced. **Added 2026-10-08:** the top-level surfaces of both atlases of all 300 non-empty imposter tables decoded (DXT5 alpha and DXT1 colour) for the cell-margin and atlas-content checks (I7); the name-filter and taboo records re-read and parsed with independent parsers (NF2, TB2, TB3); a property-set walk that descends into structs and arrays over all 40,643 entity descriptors (0x47), all 0x79 sets and all 1,618 0x78 sets, plus a raw 4-byte scan of every record of the general, gamelogic, English, cell and map archives for the 1,618 0x78 DIDs (PS2). |
| BIN | **M4 (decision 0011).** Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5 (verified on 2026-10-07), read in the dirty room on 2026-10-06/07 with Ghidra (headless, read-only copy of the shared project). **Code read:** the type-code getters and virtual tables of the DB objects for codes 0x26, 0x43, 0x30, 0x62, 0x14 and 0x22; for each, the record serialiser and the dependency collector; for 0x26 also the post-load image step, the composition routine, the entity-side compositor instance (creation, layer and colour changes) and the three CPU pixel routines it uses (copy, colour multiply through an 8-bit mask, opacity mix); second use, 2026-10-07 (revision after review, same build): the job set-up that turns opacity and colour floats into bytes (the float-to-integer conversions and how their results are stored), the sub-rectangle set-up of the image class that places an option on the canvas, the replace routine's size handling, the reflection registration of script node 33 and the stored-form colour reader it uses; third use, 2026-10-07 (verification of the second use, same build): the opacity conversion and its byte store in the job set-up, the colour packing in the composition routine and the source of its three components, the opacity skip test, the sub-rectangle set-up, and the stored-form colour reader and its scale constant; all matched the second use; for 0x43 the serialiser of its element class; for SCRIPTCHANNELS (0x50) the reflection registration of its four classes (field names, field kinds, field offsets). **Questions:** what each stored field is and in which order the client reads it; which DIDs the client loads as dependencies; why the first DID of each compositor image pair never exists in the archives; how layers, selections and colours combine into the composed image; what the trailing boolean of a compositor means; the element layout of imposter tables, name-filter and taboo tables; the field roles of script channels; (second use) what the client does with an opacity or colour component outside 0–1 and with an option placed past the canvas edge. **Why M1–M3 were not enough:** the data shows field sizes but not roles; one DID of every compositor pair is absent from every archive and the data cannot say whether it is read, used or ignored; the composition arithmetic is not in the data and a frame capture (M3) cannot separate layers, masks and colours or show rounding; no published source covers these formats (M2). For the second use: no opacity outside 0–1 occurs in the data (all 199 node-44 opacities are 1.0), so neither data nor a capture of the shipped content can show the out-of-range behaviour a hostile-input reader needs, and the only off-canvas placements are 8 options of one face compositor, where a capture would have to isolate one layer of one character's face texture. **Not read:** anything that handles network traffic, login, keys, the launcher or anti-tamper; in particular the compression dictionaries' reader and users were **not** examined (§11). **Fourth use, 2026-10-08** (same build, SHA-256 prefix as above; headless Ghidra on an own copy of the shared project, deleted afterwards; plus a byte search of the executable for type-code getters). **Code read:** the imposter-table serialiser and its element serialiser (field → member mapping), the distant-sprite vertex builder of the sector manager, the sprite material builder (which atlas goes to which texture parameter), the sprite painter's draw and set-up; the UI-scene record serialiser and constructor, the UI-scene element creation and its name lookup, the camera element's set-up, the UI 3D-view render call (camera selection, fog override) and the copy of the render state into the fog shader constants; the script-channel lookup, transition lookup, play request, queueing, replacement and stop handling; the name-filter table serialiser, its rule-set serialiser, every name-filter check (per character, per name), the vowel test and the multi-letter collapse; the taboo-table serialiser, its list matcher and the pattern-match wrapper (library identified by its error strings). **Questions:** what the imposter entry floats are; which atlas is a normal map and how its channels are read; where the 1/32 cell margin lies; what the UI-scene trailer does and how a viewport picks its camera; how priority, interruptible, exclusive and transitions decide what plays on a channel; what every name-filter field does; what the taboo header numbers mean, how lists are hashed and which pattern dialect applies. **Why M1–M3 were not enough:** the data gives only the numbers (all UI-scene trailers are identical defaults, imposter floats are unlabeled, rule-set numbers and taboo header numbers have no names); the rules are code; no published source describes them; a frame capture (M3) cannot separate quad size from anchor offset, nor show a channel's queue or a filter's rule. The DXBC programs of shader records 0x2B00000E, 0x2B0003B9 and 0x2B0006E5 (the three records whose programs declare the distant-sprite constants) were disassembled with a public shader disassembler to read the sprite quad and normal decoding. |
| BC | The client library's bytecode (record 0x56000000, chunk type 1), read on 2026-10-09 in the dirty room with a throwaway disassembler that follows [`wlib-bytecode.md`](wlib-bytecode.md). This is data in the user's install (method M1); the meaning of native functions it calls is taken from `wlib-bytecode.md` where given and otherwise inferred from use, as stated per fact. |
| SPEC | Facts already stated, with their own provenance, in the companion specs named in the header; used here only as cross-references. |
| OBS3 | Observed in the user's install on 2026-10-09 (OPEN sweep, spec writer), read-only; archives unchanged since 2026-10-04 (file times). A dump of **every** 0x70 record with its 0x79 set, made on 2026-10-07 with the repository's own decoders, walked at every depth, plus the property dictionary for types. |
| GEN | General references, not project-specific: the public DXBC shader-container format of Direct3D 10/11 (container magic, chunk table, program-version token); Direct3D surface-format names. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| C1 | Compositor layout: layer list, option list, colour entries, trailing boolean (§3.2) | OBS (24 of 24 records, zero bytes left); BIN (serialiser: field order and types; the boolean must be 0 or 1) | Confirmed (BIN + OBS) |
| C2 | Layer keys are keys of the ImageCompositorLayerNames enum mapper, colour keys of ImageCompositorColorNames; compositor names (used by scripts and controls, not stored in the record) of TextureCompositorNames (§3.3) | OBS (166 of 166 layer keys and 1,926 of 1,926 colour keys resolve) | Confirmed |
| C3 | The first DID of every option and colour entry is read and **discarded** on load; it is never a dependency and is never loaded. It is 0 exactly in the colour entries whose `mask_did` is 0 and non-zero everywhere else (§3.2, §3.4) | BIN (the reader overwrites it with the null DID; the dependency collector and the loader skip it); OBS (3,737 first DIDs = 1,811 options + 1,926 colour entries; 121 are 0, exactly the 121 colour entries with `mask_did` 0; the other 3,616, 2,977 distinct, are absent from every archive) | Confirmed (BIN + OBS) |
| C4 | The second DID of an option is a 32-bit or 24-bit colour image; that of a colour entry an 8-bit alpha mask of the same size, or 0 (§3.4) | OBS (1,811 option images: 1,700 A8R8G8B8, 111 R8G8B8; 1,805 masks: all A8, each the size of its option image; 121 zero) | Confirmed |
| C5 | Composition algorithm: canvas from the base option, per-layer selection and opacity, colour multiply through masks, opacity mix of overlays, preconditions (§3.6) | BIN (composition routine, instance defaults, layer/colour setters) | BIN |
| C6 | Pixel arithmetic of the three operations (§3.6.3) | BIN (the 24-bit copy routine, the masked-multiply routine for a 32-bit destination and an 8-bit mask, the mix routines for a 32-bit source into 32- and 24-bit destinations, and the job set-up that converts opacity and colour to bytes) | BIN |
| C7 | Trailing boolean: selects DXT3 (1) or DXT1 (0) when the composed image is block-compressed; the composition path of this build never requests compression (§3.5) | BIN; OBS (1 in the 3 non-avatar compositors, 0 in the 21 face compositors) | BIN (use); Confirmed (values) |
| C8 | Character-creation controls tie an integer property (option index) and a layer key to a compositor record (§3.7) | OBS (218 `AppearanceUI_CompositorControl` structs in 0x47 entity descriptors) | Confirmed |
| C9 | Out-of-range inputs and off-canvas placements: opacity and colour bytes come from 32-bit truncating conversions whose results are not clamped (opacity keeps the low 8 bits; colour components are shifted and OR-ed into one word); node-33 colours are stored as 4 bytes and are always 0–1; placements are clipped to the canvas by a sub-rectangle rule (§3.6.1, §3.6.2, §3.6.3) | BIN (job set-up, sub-rectangle set-up, replace routine, node-33 registration, stored-form colour reader); OBS (199 of 199 node-44 opacities 1.0; 218 of 218 node-33 colours have alpha 255, two distinct values; 8 off-canvas options, all in 0x33000017) | BIN (behaviour); Confirmed (values) |
| C10 | Pairing of `AC_Color_*` properties with compositor colour keys (§3.7) | SPEC (`appearance.md` §3.5, row A21: all 218 node-33 instances of the species script tables) | Confirmed (by reference) |
| U1 | UI scene layout: a count, that many static entity descriptors, then a colour, a boolean and two 32-bit fields (§4.1) | OBS (20 of 20 records, 3,051 descriptors, zero bytes left); BIN (scene serialiser; element serialiser with its flag bits) | Confirmed (BIN + OBS) |
| U2 | UI scene descriptor flag census and type census (§4.2) | OBS (3,051 of 3,051); 0x010 row corrected 2026-10-09 to 3,026 (the sum of the observed flag words that hold the bit; reproduced by the implementation's real-data census of 2026-10-07) | Confirmed |
| U3 | UI scene trailer = fog override of the 3D view: flag, fog colour, fog start and end distances (§4.1) | BIN (fourth use: serialiser, constructor defaults, the 3D-view render call, the copy into the fog constants); OBS (flag 0 in 20 of 20) | BIN (meaning); Confirmed (values) |
| U4 | A viewport's camera is the scene element whose UISceneEntity key equals the viewport's camera value (§4.2) | BIN (element creation and name lookup, 3D-view render call) | BIN |
| I1 | Imposter table layout: hashed table of (imposter-name key → atlas cell, two floats), two texture-list DIDs, two floats (§5.2) | OBS (507 of 507, zero bytes left); BIN (serialiser, element reader with defaults, dependency collector) | Confirmed (BIN + OBS) |
| I2 | Table DIDs: 0x27, region, then the 27-landblock super-block coordinates (§5.1) | OBS (507 of 507) | Confirmed |
| I3 | Atlas geometry: column/row counts from the entry count, cell size 128 texels at the top level, stored cell size = 31/32 of the cell step (§5.4) | OBS (300 of 300 non-empty tables: texture sizes and stored floats agree) | Confirmed (OBS) |
| I4 | Entry keys are keys of the RenderImposterNames enum mapper (§5.3) | OBS (816 distinct keys, all resolve) | Confirmed |
| I5 | `value_a` × sprite scale is the width and height of the sprite quad, `value_b` × sprite scale the height of its lower edge above the anchor (§5.2) | BIN (element serialiser: stored order = member order; sprite builder; sprite vertex program); OBS (`value_a` > 0 in 12,406 of 12,406) | BIN |
| I6 | The first atlas is the colour atlas (alpha = coverage), the second a tangent-space normal atlas; channel mapping (§5.3) | BIN (material builder binds them to parameters 3000 and 3011; sprite pixel program) | BIN |
| I7 | The 1/32 margin is at the right and bottom of each cell; texture coordinates of a cell (§5.4) | BIN (sprite builder); OBS (the last 4 texel columns and rows of all 12,406 used cells have alpha 0 in the top-level colour atlas) | Confirmed (BIN + OBS) |
| CS1 | Compute shader: u32 byte count + a DXBC compute-shader blob of exactly that size (§6) | OBS (3 of 3); BIN (serialiser: count then raw bytes, count checked against the bytes left); GEN | Confirmed (BIN + OBS) |
| SC1 | Script-channel record layout (§7) | OBS (1 of 1, zero bytes left); BIN (reflection registration: classes, fields, kinds, order) | Confirmed (BIN + OBS) |
| SC2 | Script keys are keys of the FX enum mapper; channel keys of ScriptChannels (§7.3) | OBS (346 distinct script keys all resolve in FX; channel keys 0x10000001, 0x10000003, 0x10000005 resolve in ScriptChannels) | Confirmed |
| SC3 | Channel arbitration: exclusive channels hold one current script per entity; priority and interruptible decide replacement; refused requests are dropped or queued by priority; the queue head starts when the current script ends (§7.4) | BIN (fourth use) | BIN |
| SC4 | Transition choice: exact (from, to), then (any, to), then (from, any), then (any, any), within the channel's own list (§7.4) | BIN (fourth use) | BIN |
| NF1 | Name-filter table layout (§9) | OBS (1 of 1, zero bytes left); BIN (serialiser and element reader) | Confirmed (BIN + OBS) |
| NF2 | Meaning of every rule-set field: character-type masks, run and vowel limits, multi-letter groups, extra vowel characters, forbidden first characters, character classes, taboo mode, languages (§9) | BIN (fourth use: rule-set serialiser and all checks); OBS (class lists and language keys, 3 of 3 rule sets) | BIN (meaning); Confirmed (values) |
| NF3 | Per-character and per-name checks, vowel set (§9.1, §9.2) | BIN (fourth use) | BIN |
| TB1 | Taboo table layout (§10) | OBS (1 of 1, zero bytes left); BIN (serialiser: list headers, then a string-keyed hashed table of string lists) | Confirmed (BIN + OBS) |
| TB2 | Header `category` [`a`] = result category (1–5), `contexts` [`b`] = bit mask of contexts (§10) | BIN (fourth use: list matcher) | BIN |
| TB3 | String hash of the list table (§10) | BIN (matcher's lookup); OBS (the 8 keys lie in non-decreasing bucket order for hint 1, bucket count 23) | Confirmed (BIN + OBS) |
| TB4 | Pattern dialect and matching procedure (§10.1) | BIN (fourth use: match wrapper; library identified by its error strings) | BIN |
| CD1 | Compression-dictionary stored envelope (§11) | OBS (3 of 3) | Confirmed (envelope only; body and use not researched, M8) |
| PS1 | 0x78 census by family and by DID-map label (§12) | OBS (1,618 of 1,618 decode with the generic reader) | Confirmed |
| PS2 | Typed referrers of the 0x78 families, including struct members (§12.2) | OBS (2026-10-08: all 0x47, 0x78 and 0x79 property sets walked into structs and arrays; raw scan of the non-asset archives) | Confirmed |
| M1 | PropertyMetaData family: properties, census, uniqueness by PID, control list (§13.1) | OBS (5,211 of 5,211 objects; control list 5,211 of 5,211) | Confirmed |
| M2 | ErrorCategory family and the error control (§13.2) | OBS (675 of 675) | Confirmed |
| M3 | TutorialHint family and control (§13.3) | OBS (337 of 337) | Confirmed |
| M4 | MovieLibrary family and control (§13.4) | OBS (85 of 85) | Confirmed |
| M5 | PropertyResponse / PropertyResponseCallback family (§13.5) | OBS (419 objects in 6 classes) | Confirmed (census); behaviour server-side, out of scope |
| M6 | GameCallback family (§13.6) | OBS (110 objects; references from 0x07 and 0x0C records) | Confirmed |
| M7 | Version family (§13.7) | OBS (124 objects in 32 classes) | Confirmed (census); behaviour server-side, out of scope |
| W1 | WEENIECONTENT DID map census (§14) | OBS (316 entries) | Confirmed |
| G1 | Typed references into every record type of this spec (§15) | OBS (all property sets of 0x47, 0x70/0x76 (+0x09000000) and 0x78; all DID maps) | Confirmed |
| X1 | Property-response members: 419 objects in six root classes found by the label rule of §13.5 (237 effect lists, 120 world-event lists, 32 callback-only, 21 map-property, 9 calculate-property arrays) | OBS3 (every 0x79 set); implementation census 2026-10-07 | Confirmed 2026-10-09 |
| X2 | Version family: 123 members in 31 root classes plus the `VersionSystem` control (class 1623 here). Its `Version_VersionNumberDataList` holds 93 structs, each with a distinct `Version_Number`: 92 carry `Version_SingletonList`, one carries `AccountVersion_SingletonList` (one DID) instead. The `Version_SingletonList` arrays name 155 distinct DIDs: 121 members and 34 other objects; the two members not among them are the two DIDs of `Version_AlwaysRunOnPlayers`, which no number list names. The control names 158 distinct DIDs in all: the 123 members and 35 other 0x70 objects (34 + the `AccountVersion_SingletonList` entry), none of which carries `Version_*` properties (33 of the 35 have an empty property set) (§13.7) | OBS3 (re-measured 2026-10-09 in the repair, after the review's count) | Confirmed 2026-10-09 |
| X3 | Value types of the `UI_Map_GameMap` fields and of the `AppearanceUI_*` control fields (§12.2, §3.7) | OBS3 (dictionary) | Confirmed 2026-10-09 |
| X4 | Percentage display of modifier values: the modifier-description code asks the metadata control for `PropertyMetaData_DisplayAsPercentage`; with the flag, a float value *v* is shown as *d* × 100, where for `Multiply` *d* = *v* − 1 when *v* > 1, 1 − *v* when *v* < 1 and *v* itself otherwise (*v* = 1 or NaN), with an increase text when *v* ≥ 1 and a decrease text otherwise; for other operators *d* = *v*; int and int64 values are shown unscaled whatever the flag (§13.1) | BC (class 1341 functions 9, 12 and 48; class 1392 function 95; class 1391 function 89 here); the *v* = 1, NaN and int cases read again in functions 9 and 12 on 2026-10-09 (repair) | Confirmed (bytecode) 2026-10-09 |
| X5 | Of the 4,112 raw four-byte matches of 0x78 DIDs in 795 object records, none is a decoded value of the record (§12.2) | OBS3 (all 383,332 0x70 records raw-scanned; the 795 with matches decoded with `tessera-wstate` and every decoded value compared, 2026-10-09) | Confirmed 2026-10-09 |
| X6 | Root classes and controls per §13 family: property metadata 5,211 objects in 1 class, control `PropertyMetaDataControl`; error categories 675 in 1, `ErrorControl`; tutorial hints 337 in 1, `TutorialHintControl`; movie library 85 in 1, `MovieLibraryControl`; property responses 419 in 6, no control; game callbacks 110 in 110, no control; Version 123 in 31, `VersionSystem`. Each control is a separate object of its own class and lacks its family's identifying member property (`PropertyMetaData_Property`, `ErrorCategory_BriefString`, `TutorialHint_Name`, `MovieLibrary_CategoryType`; for Version the control-only properties of §13.7 set it apart). None of the 313 distinct 0x70 objects named in WEENIECONTENT is a property-response or game-callback member or names one at any depth (§13) | OBS3 (every 0x79 set; the WEENIECONTENT map of W1, 2026-10-09 repair) | Confirmed 2026-10-09 |

---

<a id="s1"></a>
## 1. Conventions

- Integers and floats are little-endian (`records.md` §1.1). `DID` is a u32.
- **VLE**, **TSize** (one bucket-hint byte, then a VLE count), the bucket-count table and
  **bucket order** are those of `records.md` §1.2–§1.3.1. "Hashed table" below means: a
  TSize, then *n* elements, written in bucket order of their u32 key modulo the bucket
  count of the hint. A reader needs only *n*; a byte-exact writer keeps the hint.
- **Pascal string**: VLE byte count then Latin-1 bytes; **UTF-16 string**: VLE unit count
  then UTF-16LE (`records.md` §1.4).
- **bool8**: one byte, 0 or 1. Wherever this spec says "bool8 (BIN)", the client's reader
  fails the record on any value ≥ 2; Tessera rejects the record too.
- "Embedded property set" is the form of `property-system.md` §5.1 (TSize count, entries
  with PID echo, no DID).
- The client's readers of UI scenes, imposter tables, compute shaders, name-filter and
  taboo tables check, before allocating, that a stored count does not exceed the bytes left
  in the record; the compositor reader instead stops at the first read past the end (BIN).
  Tessera readers check every count against the bytes left with the minimum element size
  and report truncation.
- Hashed tables read by the client fail the load when the hint is above 22 (BIN, imposter,
  name-filter and taboo readers; the client's bucket-count table has at least 23 entries,
  `records.md` §1.3 lists the first 13). Observed hints in this spec's records are 0, 1
  and 4. **Scope:** for the record types of this spec, a Tessera reader rejects a hint
  above 22 (MR12). This is a per-type exception to `records.md` invariant T1 ("must not
  reject other hints"), the same rule that `property-system.md` §5.3 already applies to
  property sets; hints 13–22 are still accepted. For the script-channel record (§7) the
  client's check was not read; Tessera applies the same rule there because the client's
  bucket table has no entry for a larger hint. §18 proposes narrowing T1 accordingly.

---

<a id="s2"></a>
## 2. Census and DB types (Confirmed)

| DID (range) | Archive (count) | DB type (code, label) | Prologue (`records.md` §1.6) | Section |
|---|---|---|---|---|
| 0x33000000–0x3300001E (sparse) | general (24) | 0x26 IMAGE_COMPOSITOR | D | §3 |
| 0x21000000–0x2100001F (sparse) | general (20) | 0x43 UI_SCENE | D | §4 |
| 0x27RRXXYY | cell_1 (100), cell_2 (94), cell_3 (100), cell_4 (100), cell_5 (100), cell_14 (13) = 507 | 0x30 RENDER_IMPOSTER_TABLE | D | §5 |
| 0x2D000000–0x2D000002 | general (3) | 0x62 COMPUTESHADER | D | §6 |
| 0x43001000 | gamelogic (1) | 0x50 SCRIPTCHANNELS | – (no DID) | §7 |
| 0x58000000 | local_English (1) | 0x46 STRING_STATE | – | §8 → `string-table.md` §10.3 |
| 0x0E200000 | general (1) | 0x14 NAME_FILTER_TABLE | D | §9 |
| 0x0EBADA55 | local_English (1) | 0x22 TABOO_TABLE | D | §10 |
| 0x0E010000–0x0E010002 | general (3) | 0x01 COMPRESSION_DICTIONARY | – | §11 (stored form only) |
| 0x78000001–… | gamelogic (1,616), local_English (2) | 0x32 DBPROPERTIES | D | §12 |
| 0x70… metadata objects | gamelogic (5,211 + 675 + 337 + 85 + 419 + 110 + 124, plus their controls) | 0x45 WSTATE | D | §13 |

DB type codes and labels are those of `records.md` §2.1 (BIN) and the DBOType enum mapper
(`property-system.md` §9). How each type is found at run time (DID map labels):

| Record | DID map (root key, label) | Entry label (interoperability) |
|---|---|---|
| 7 face compositors 0x33000000–0x33000006 | PRELOADRESOURCES (0x2800000F) | `Av_<race>_<sex>_Face_Compositor` |
| Compute shaders, script channels, name-filter table, taboo table | UNIQUEDB (0x28000002) | `ComputeShader_HDAmbientOcclusion` (0x2D000000), `ComputeShader_WaterSimulation` (0x2D000001), `ComputeShader_WaterInteraction` (0x2D000002), `ScriptChannels` (0x43001000), `NameFilterTable` (0x0E200000), `TabooTable` (0x0EBADA55) |
| Compression dictionaries | COMPRESSIONDICTIONARY (0x28000010) | three labels, keys 0x10000001–0x10000003 (§11) |
| Locale record | STRINGTABLE (0x28000004) | `Language` (`string-table.md` §10.3) |
| 0x78 sets | UNIQUEDB, DBPC (0x28000011), SCRIPTCALLBACK (0x28000007), CONTENT_LAYER (0x28000012), PHYSICSCONTROLLER (0x28000030) and others | §12 |
| Metadata controls | WEENIECONTENT (0x2800000E) | §14 |

The other 17 compositors, the UI scenes and the imposter tables are not labelled in any
DID map; they are reached through typed properties (§15) or, for imposter tables, by
computing the DID (§5.1).

---

<a id="s3"></a>
## 3. Image compositors (prefix 0x33, DB type 0x26)

<a id="s3-1"></a>
### 3.1 Model

A compositor describes a layered 2D image: an ordered list of **layers**; each layer offers
numbered **options** (alternative images placed at a pixel position); each option can be
tinted through **colour entries**, each pairing a named colour with an 8-bit mask. An
entity that uses a compositor holds, per layer, a selected option number and an opacity,
and per colour name an RGB colour (§3.6). The result is one texture (a face texture for
the 21 avatar compositors, a warg or caparison texture for the other 3).

<a id="s3-2"></a>
### 3.2 Layout (Confirmed (BIN + OBS), C1; 24 of 24 records)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `did` | DID | Own DID |
| 2 | `layer_count` *L* | u32 | Number of layers. Observed 2–11 (2: 4 records, 3: 1, 7: 11, 8: 2, 9: 2, 11: 4); 166 layers in all. |
| 3 | layers | *L* × layer | In **drawing order**: layer 0 is the base (§3.6) |
| 4 | `compress_with_alpha` | bool8 (BIN) | §3.5. 0 in 21 records, 1 in 3 (0x33000009, 0x3300000E, 0x3300000F). |
| — | *(end)* | | Zero bytes left in all 24 |

Layer:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `layer_key` | u32 | Key in the enum mapper labelled ImageCompositorLayerNames (EMAPPER key 0x43; 0x230000DE here). Unique within a record. |
| 2 | `option_count` *n* | u32 | Observed 1–40; 1,811 options in all |
| 3 | options | *n* × option | Stored order |

Option:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `index` | u32 | The option's number, matched against the selection (§3.6). Observed 1 … *n* in stored order in 165 of 166 layers; in the other layer (0x3300000E, layer 0x10000015) the numbers are 1–39 and then 999. Readers must not assume contiguity. |
| 2 | `legacy_did` | DID | Read and discarded (§3.4). Non-zero in all 1,811 options |
| 3 | `image_did` | DID | The option's image, a render surface (0x41) |
| 4 | `x` | u32 | Left edge of the image on the canvas, in texels. Observed 0–476. |
| 5 | `y` | u32 | Top edge (row 0 at the top), in texels. Observed 0–701. |
| 6 | `colour_count` *c* | u32 | 0 (307 options), 1 (1,082), 2 (422) |
| 7 | colour entries | *c* × colour entry | Stored order = application order (§3.6.2) |

Colour entry (12 bytes):

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `colour_key` | u32 | Key in the enum mapper labelled ImageCompositorColorNames (EMAPPER key 0x42; 0x230000DF here) |
| 2 | `legacy_did` | DID | Read and discarded (§3.4). 0 exactly when `mask_did` is 0 (121 entries), non-zero in the other 1,805 |
| 3 | `mask_did` | DID | An 8-bit alpha surface the size of the option image, or 0 = no mask (the colour applies to the whole option image) |

Option size: 24 + 12·*c* bytes. Record size: 9 + Σ (8 + Σ option sizes).

<a id="s3-3"></a>
### 3.3 Keys and names (Confirmed, C2)

Three enum mappers belong to compositors; they are found by label in the EMAPPER DID map
(`property-system.md` §8), never by fixed DID:

| EMAPPER label (key) | Used for | Example keys (labels) |
|---|---|---|
| TextureCompositorNames (0x48) | The name under which an entity holds a compositor instance (script nodes and controls, §3.7); not stored in the 0x33 record | 0x10000001 `Avatar_Face` |
| ImageCompositorLayerNames (0x43) | `layer_key` | 0x1000000B `Avatar_Face_Base`, 0x10000004 `Avatar_Face_Eyes`, 0x10000013 `Beorning_Face_Base` |
| ImageCompositorColorNames (0x42) | `colour_key` | 0x1000000B `Avatar_Face_SkinColor`, 0x1000000A `Avatar_Face_Eyes_EyeColor` |

All 166 layer keys and all 1,926 colour keys of this install resolve. Observed pairings of
layer and colour key (each colour key occurs only under particular layers) are data, not
rules; a reader accepts any key.

<a id="s3-4"></a>
### 3.4 The two DIDs of each pair (Confirmed (BIN + OBS), C3, C4)

- **`legacy_did`** (first DID of every option and colour entry): the client reads it and
  immediately replaces it with the null DID; consequently it is never collected as a
  dependency and never loaded. There are 3,737 such DIDs (1,811 options + 1,926 colour
  entries). In colour entries `legacy_did` is 0 exactly when `mask_did` is 0 (121 entries)
  and non-zero otherwise; in options it is never 0. None of the 3,616 non-zero values
  (1,811 + 1,805; 2,977 distinct) exists in any archive of this install. Tessera keeps the
  value for byte-exact rewriting and otherwise ignores it; a reader must not reject a 0 or
  a non-zero value in either position. (The client's load step would, for a non-null first DID, use that
  image's own placement rectangle instead of `x`, `y` for layers other than 0; because the
  value is always nulled first, that path is dead in this build.)
- **`image_did`**: always present. Base-layer images are R8G8B8 (111) or A8R8G8B8 (46);
  images of the other layers are A8R8G8B8 (1,654). All layer-0 options of a record have
  the same width and height and sit at (0, 0) (24 of 24 records).
- **`mask_did`**: A8 surfaces, each exactly the size of its option image (1,805 of 1,805),
  or 0 (121 entries, all with colour key 0x10000007 under layer key 0x10000003).
- The client loads every non-null `image_did` and `mask_did` as dependencies of the
  compositor (BIN). Failing to load one only logs an error; the record stays usable and
  composition may then fail (§3.6.1, P3) or treat a missing mask as "no mask"
  (§3.6.2 step 3.3).

<a id="s3-5"></a>
### 3.5 The trailing boolean (C7)

When the client turns a composed image into a texture it can block-compress it; the boolean
then chooses DXT3 (1) over DXT1 (0) (BIN). The composition path of this build always
requests an uncompressed texture, so the boolean has no visible effect (BIN). The three
records with 1 are the non-avatar compositors, whose bases are 1024 × 1024 A8R8G8B8 (four
face compositors with 0 also have A8R8G8B8 bases). Tessera stores the
flag and, if it ever compresses composed images, uses it the same way.

<a id="s3-6"></a>
### 3.6 Composition (BIN, C5, C6, C9)

<a id="s3-6-1"></a>
#### 3.6.1 Inputs and preconditions

Inputs:

- the compositor record;
- a **selection list** with exactly one entry per layer, in layer order; an entry is
  (u32 option number, f32 opacity). An entity's compositor instance starts with every
  entry at (0, 1.0) (BIN); script node 44 ModifyCompositorLayer (`script-table.md`) sets
  the entry of the layer whose `layer_key` equals the node's `LayerName` to (its
  `ImageNumber`, its `Opacity`) (BIN). The setter stores the f32 as read, without any
  range check, so an opacity may be any f32 value, including negative values, values
  above 1, infinities and NaN (§3.6.2 and §3.6.3 say what each does). Observed: `Opacity`
  1.0 and `ImageNumber` 0 (102 nodes) or 1 (97) in all 199 node-44 instances;
- a **colour map** from colour key to four floats (R, G, B, A); script node 33
  ModifyCompositorColor sets one entry (BIN). Only R, G and B are used. Node 33 stores its
  `Color` as a 4-byte colour (`property-system.md` §4.7: bytes B, G, R, A), and the
  client turns each byte *b* into the float *b* × *k*, where *k* is the single-precision
  value nearest 1/255 (bit pattern 0x3B808081) (BIN). Colours that come from data are
  therefore always within 0–1, and for every byte value *b* the conversion of §3.6.3 gives
  back exactly *b* (all 256 values checked). Observed: 218 node-33 instances with two
  distinct colours, alpha 255 in both. The colour setter stores the four floats without
  a range check (BIN); values outside 0–1 can reach it only from callers other than node
  33 (none traced), and §3.6.3 defines them anyway.

Composition fails when any of the following holds; the client releases the instance's
previous composed texture before composing, so after a failure the instance has none (BIN):

- **P1**: some option of the record has `image_did` = 0 (never in this install), or the
  record's images have not been requested yet;
- **P2**: the selection list's length differs from the layer count;
- **P3**: a layer other than 0 has a non-zero selected number, and some option stored
  before the first option with that `index`, or that option itself, has no loaded image
  (if no option has that `index`, the layer passes and is skipped in §3.6.2);
- **P4**: a loaded layer-0 option image differs in size from the first layer-0 option's
  image, or such an option is not at (0, 0).

A record with zero layers composes to "no image" without failing.

<a id="s3-6-2"></a>
#### 3.6.2 Steps

1. **Base option.** In layer 0, take the option whose `index` equals the selected number;
   if the number is 0 or no option matches, take the first stored option of layer 0.
2. **Canvas.** Create a canvas of the base option's image size. (The client creates it in
   a working format chosen by its renderer; Tessera uses RGBA8.) Its initial content
   matters only when layer 0 is skipped; Tessera starts from transparent black.
3. For each layer *i* = 0, 1, …, *L* − 1 in order:
   1. Pick the option: for *i* = 0 the base option of step 1; for *i* > 0 the option whose
      `index` equals the selected number. Skip the layer if the number is 0 or no option
      matches.
   2. Skip the layer if its opacity is not greater than 0.0002 (this applies to layer 0
      too). A NaN opacity is not greater, so it skips the layer; so does any negative
      value or −∞. Any opacity greater than 0.0002, however large, passes.
   3. **Tinted copy.** Make a temporary image *T* the size of the option image:
      - If the option has no colour entries, *T* is a copy of the option image.
      - Otherwise process the colour entries in stored order. An entry whose mask is
        loaded but differs in size from the option image is skipped; an entry whose mask
        is 0 or failed to load counts as having no mask. For the first entry
        not skipped, *T* is first set to a copy of the option image. For every entry not
        skipped, take its colour from the colour map (white (1, 1, 1) if the key is
        absent); if all of R, G, B are within 0.0002 of 1.0 the entry changes nothing;
        otherwise **multiply** *T* by the colour through the entry's mask (§3.6.3; without
        a mask, over the whole image).
      - If every entry was skipped, *T* is a plain copy of the option image.
   4. **Place.** The target rectangle is (`x`, `y`) to (`x` + width − 1, `y` + height − 1)
      of the canvas, reduced to the canvas by the placement rule below. For layer 0,
      **replace** the rectangle with *T*. For other layers, **mix** *T* into the rectangle
      with the opacity byte *o* of §3.6.3.
4. The canvas is the composed image. The client then makes a texture from it, with a full
   mip chain when a global renderer flag asks for one (BIN).

**Placement rule (BIN, C9).** The client clips; it never rejects a placement. With *W* ×
*H* the canvas size and *w* × *h* the size of *T*:

1. Take `x` and `y` as **signed** 32-bit integers (a stored value ≥ 0x80000000 is
   negative). Compute left = `x`, top = `y`, right = `x` + *w* and bottom = `y` + *h* in
   32-bit two's-complement arithmetic (a sum that overflows wraps around). Right and
   bottom are exclusive.
2. If left < 0, set left = 0; if top < 0, set top = 0.
3. If right > *W*, set right = *W*; if bottom > *H*, set bottom = *H*.
4. If right < 1, set right = 1; if bottom < 1, set bottom = 1.
5. If left ≥ right, set left = right − 1; if top ≥ bottom, set top = bottom − 1.
6. The operation covers min(*w*, right − left) columns and min(*h*, bottom − top) rows:
   texel (*i*, *j*) of *T* (from its top-left corner) goes to canvas texel (left + *i*,
   top + *j*). The rest of *T* is dropped.

For in-range placements this is plain clipping at the right and bottom edges. The rule
has two odd cases that Tessera reproduces as stated: with a negative `x` (or `y`), *T* is
still drawn from its own first column (row) at canvas column (row) 0, so the part beyond
`x` + *w* is what gets dropped, not the part left of the canvas (and if `x` + *w* ≤ 0, one
column is still drawn); and an option that starts at or past the right (bottom) edge
has its first column (row) drawn into the canvas's last column (row). Observed: placements past the canvas edge occur only in 8 options of 0x33000017,
layer 0x10000012, and only at the bottom: their last rows reach row **310** (option 5:
`y` 219, height 92) of a 256-row, 512-column canvas. No placement in this install has a
negative coordinate or starts outside the canvas.

<a id="s3-6-3"></a>
#### 3.6.3 Pixel arithmetic (BIN, C6)

All values are bytes 0–255; inc(*x*) = 0 if *x* = 0, else *x* + 1; div256(*x*) = *x* ÷ 256
rounded toward zero (*x* may be negative). *D* is the destination texel, *S* the source
texel, channels c ∈ {R, G, B}. Masked multiply and mix **never change** the destination's
alpha.

| Operation | Per texel |
|---|---|
| **Replace** (base layer, and the copy into *T*) | *D* = *S* (all four bytes). **Tessera rule:** a 24-bit source gives alpha 255. |
| **Masked multiply** (colour *C*, mask byte *M*; opacity is always 1.0 here) | *k* = inc(*M*); without a mask *k* = 256. If *k* = 0, unchanged. *X*.c = (inc(*C*.c) × *D*.c) ÷ 256 (integer division); if *k* = 256, *D*.c = *X*.c; else *D*.c = *D*.c − div256((*D*.c − *X*.c) × *k*). |
| **Mix** (overlay, opacity byte *o*) | If *o* = 0, unchanged. *a* = *S*.A if *o* = 255, else (*S*.A × (*o* + 1)) ÷ 256. If *a* = 0, unchanged. *k* = *a* + 1; if *k* = 256, *D*.c = *S*.c; else *D*.c = *D*.c − div256((*D*.c − *S*.c) × *k*). |

**Float-to-byte conversions (BIN, C9).** Both use conv(*v*): multiply the f32 *v* by
255.0 in single precision (round to nearest even), then truncate toward zero to a signed
32-bit integer. If the product is NaN or lies outside −2³¹ … 2³¹ − 1, conv(*v*) is −2³¹
(the processor's "integer indefinite" result). Nothing is clamped:

- **Opacity byte** *o* = the low 8 bits of conv(opacity) (the integer modulo 256). For
  0.0002 < opacity ≤ 1 this is trunc(opacity × 255). Above 1 it wraps: 1.5 gives 126, 2.0
  gives 254, and an opacity whose product is 256 (for example 1.004) gives 0, so the layer
  then changes nothing; +∞ and any opacity whose product is 2³¹ or more give 0. Values that are not greater
  than 0.0002 never get here (§3.6.2 step 3.2). The masked multiply always uses opacity
  1.0, so its opacity byte is 255.
- **Colour bytes**: the client forms one 32-bit word as (conv(R) shifted left 16) OR
  (conv(G) shifted left 8) OR conv(B) OR 0xFF000000, in 32-bit two's-complement
  arithmetic (bits shifted past bit 31 are lost), without masking the three integers to
  8 bits first. *C*.R, *C*.G and *C*.B are bits 16–23, 8–15 and 0–7 of that word. For
  components in 0–1 this gives *C*.c = trunc(component × 255). Out of range, a component
  spills into its neighbours: for example B = 1.2 gives conv = 306, so *C*.B = 50 and bit 0
  of *C*.G is also set; a component with conv < 0 (product −1 or below) sets every bit
  above its own byte, so B = −0.5 (conv −127) gives *C*.B = 129 and *C*.G = *C*.R = 255. Colours from data are always within 0–1 (§3.6.1),
  so this case needs input from outside the archives.

**Tessera rule:** implement both conversions exactly as stated, including the wrap and
the spill, so that any colour map and selection list (hostile or not) gives the client's
result. A float-to-integer conversion that saturates or maps NaN to 0 does not match
and must not be used here.

The mix equals the "Mix colour" operation and the masked multiply the "Multiply" operation of
`ui-layout.md` §9.6.2 with the mask byte as the source alpha (the same client routines).
Overlay images are all A8R8G8B8 in this install; other source formats are not specified
here.

<a id="s3-7"></a>
### 3.7 Who drives a compositor (Confirmed, C8)

- **Script nodes** (`script-table.md`): node 53 TextureCompositor attaches a compositor
  record (`ImageCompositorFile`, type-20 DID code 0x26) to an entity under a
  TextureCompositorNames key (`CompositorName`), with `MeshType` and `MaterialType` saying
  where the texture goes; nodes 44 and 33 change the selection and colour map of a named
  instance (BIN: an unknown compositor name logs an error and changes nothing; a second
  attach under the same name is refused).
- **Character-creation controls** on avatar entity descriptors (0x47; `appearance.md`
  §3.4): 218 `AppearanceUI_CompositorControl` structs, each with
  `AppearanceUI_CompositorFile` (a 0x33 DID), `AppearanceUI_CompositorLayerName` (a layer
  key), `AppearanceUI_ControlPropertyName` (the integer property, such as `AC_Texture_Eyes`,
  whose value is the selected option number of that layer) and
  `AppearanceUI_MinControlValue` (0 or 1: whether option 0, "none", is offered).

For an explorer preview: select option `index` 1 in every layer (or the avatar entity's
`AC_Texture_*` values through its controls), opacity 1.0, and either white or the avatar's
`AC_Color_*` colours.

**Colour pairing (C10).** The pairing of `AC_Color_*` properties with colour keys is not
stored in the compositor or in the character-creation controls; it is made by the node-33
instances of each species' script table, whose stored `Color` is replaced at run time by
the property that drives it. The resulting table (which property feeds which colour key of
which compositor name) is in `appearance.md` §3.5, Confirmed there over all 218 node-33
instances. A viewer that has the avatar's script table follows those nodes; one that has
not uses that table, and otherwise offers a colour picker per colour key.

---

<a id="s4"></a>
## 4. UI scenes (prefix 0x21, DB type 0x43)

<a id="s4-1"></a>
### 4.1 Layout (Confirmed (BIN + OBS), U1; 20 of 20 records)

| Order | Field | Type | Meaning / observed |
|---|---|---|---|
| 1 | `did` | DID | Own DID |
| 2 | `entity_count` *n* | u32 | 2–732; 3,051 in all |
| 3 | entities | *n* × static entity descriptor | Exactly the layout of `entity-descriptor.md` §3.1 (flags word, then optional fields in that spec's stream order). The client's element reader has the same fields, flag bits and order (BIN). |
| 4 | `fog_colour` | u32 | Fog colour, 0xAARRGGBB; the client converts each byte to a float by × 1/255 (the stored-form colour reader of C9). 0 in all 20. |
| 5 | `fog_override` | bool8 (BIN) | 0 in all 20 |
| 6 | `fog_start` | f32 | Fog start distance, world units. −1.0 (0xBF800000) in all 20, which is also the client's constructor default. |
| 7 | `fog_end` | f32 | Fog end distance, world units. −1.0 in all 20 (constructor default −1.0). |
| — | *(end)* | | Zero bytes left in all 20 |

**Use (U3, BIN).** The four trailer fields override the distance fog of the 3D view that
shows the scene. When a viewport draws its scene, it passes its own fog settings (the
viewport properties `UICore_Viewport_UseFog`, `…_FogColor`, `…_FogNearDistance` and
`…_FogFarDistance`, `ui-widgets.md` §12). When the scene's `fog_override` is 1, the client
replaces all four by the scene's values: fog on, `fog_colour`, `fog_start`, `fog_end`. When it
is 0 the viewport's settings stand. The pair then feeds the linear distance fog of
`lighting.md` §10 (start = near, end − start = range). The client's constructor defaults are
`fog_override` 0, colour 0 (all four components 0.0) and both distances −1.0. Since every
record of this install stores `fog_override` = 0, the trailer has no visible effect here;
a viewer may ignore it, a renderer that supports viewport fog applies the rule.

<a id="s4-2"></a>
### 4.2 Descriptor census (Confirmed, U2; 3,051 descriptors)

UI scene descriptors use fewer bits than landblock descriptors. The invariant
`entity-descriptor.md` ED1 ("0x002, 0x004 and 0x080 always set") does **not** hold here.

| Bit | Field | Present in |
|---|---|---|
| 0x001 | own DID | 0 |
| 0x002 | `type` | 3,051 (all): 7 Physics 2,842; 9 Light 120; 27 Camera 82; 2 Decal 7 (labels from the enum mapper of `entity-descriptor.md` §4.1) |
| 0x004 | `iid` | 323 |
| 0x008 | `object` (a 0x47 entity descriptor) | 2,969, all 0x47 |
| 0x010 | position + rotation | 3,026 (corrected 2026-10-09; the five flag words 0x18A, 0x10A, 0x102, 0x182, 0x18E, 25 descriptors, lack it) |
| 0x020, 0x040, 0x200, 0x400 | | 0 |
| 0x080 | properties | 2,099 |
| 0x100 | name | 3,041 |

Flag words observed: 0x19A (1,867), 0x11A (761), 0x19E (159), 0x11E (151), 0x192 (41),
0x112 (25), 0x18A (15), 0x9A (10), 0x116 (7), 0x10A (5), 0x196 (5), 0x102 (3), 0x182 (1),
0x18E (1).

> **Answered (2026-10-09; was OPEN):** the bit table and the flag-word census disagree for bit 0x010. Five of the
> observed words (0x18A (15), 0x10A (5), 0x102 (3), 0x182 (1), 0x18E (1); 25 descriptors)
> lack 0x010, so the word census implies 3,026 descriptors with position + rotation, not
> 3,051. Every other row of the table matches the word census (0x004 323, 0x008 2,969,
> 0x080 2,099, 0x100 3,041). The implementation (2026-10-07, `crates/formats` real-data
> test) reproduces the word census exactly and counts 3,026; a clean-room code review
> confirmed the arithmetic. Proposed correction for the spec writer: the 0x010 row should
> read 3,026.
>
> *Answer:* accepted. The bit row was wrong; the flag-word census is right, and the row now
> reads 3,026 (U2).

Properties found in UI scene descriptors include `CameraFOV` (47), `CameraFarDistance`
(19) on cameras, `LightRange`, `LightDiffuse`, `LightDiffuseIntensity`, `LightFalloff`,
`KeyLight` on lights, and `Render_AdjustableScale`/`Physics_AdjustableScale` (1,537 each)
on objects. Names (bit 0x100) are the scene's own element names; some match keys of the
UISceneEntity enum mapper (EMAPPER key 0x78), for example camera names. How UI code picks
a camera or light by name is UI behaviour; the camera rule is given below.

**Scene elements and camera selection (U4, BIN).**

1. When a scene is loaded for a viewport, the client creates one scene element per
   descriptor of type 2 (Decal), 7 (Physics), 9 (Light) or 27 (Camera), in stored order.
   Descriptors of other types create nothing (none occur in this install).
2. Each element gets an identifier: its descriptor name (bit 0x100) looked up as a label of
   the enum mapper with EMAPPER key 0x78 (UISceneEntity; 0x230000B9 here). A name that is
   not a label of that mapper gives no usable identifier.
3. The viewport's `UICore_Viewport_camera` value (an enum of the same mapper,
   `ui-widgets.md` §12) selects the camera: the 3D view uses the **first** element, in
   creation order, whose identifier equals that value, and takes the view from it.
4. Nothing is drawn when the viewport's camera value is 0, when no element matches, or when
   the matched element provides no camera.
5. A camera element reads `CameraFOV` from its descriptor's properties; a float value in
   1–100 is used, other values are ignored.

So the camera is chosen by name, not by type. A viewer without a viewport (the explorer)
uses the first type-27 descriptor whose name is a UISceneEntity label, or else the first
type-27 descriptor.

---

<a id="s5"></a>
## 5. Imposter tables (prefix 0x27, DB type 0x30)

<a id="s5-1"></a>
### 5.1 DIDs (Confirmed, I2)

DID = 0x27000000 | (*R* << 16) | (*X* << 8) | *Y*, with *R* the region (1–5, 14) and *X*, *Y*
multiples of 27 (0, 27, …, 243): one table per square of 27 × 27 landblocks, with *X*, *Y*
the landblock coordinates (`records.md` §2.2) of its lower corner. Regions 1, 3, 4, 5 have
all 100 squares, region 2 has 94, region 14 has 13. Nothing in the data names these DIDs;
a reader computes the DID of the square that contains a landblock: *X* = 27 × ⌊*x* / 27⌋,
likewise for *Y*.

<a id="s5-2"></a>
### 5.2 Layout (Confirmed (BIN + OBS), I1; 507 of 507 records)

| Order | Field | Type | Meaning / observed |
|---|---|---|---|
| 1 | `did` | DID | Own DID |
| 2 | entries | hashed table (TSize) of entry | Keyed by `name_key`; bucket hint 1 in all 507; 0–128 entries (12,406 in all); bucket order by key mod 23 in all 289 tables with two or more entries |
| 3 | `colour_atlas` | DID | A texture list (0x40), or 0 |
| 4 | `second_atlas` | DID | A texture list (0x40), or 0 |
| 5 | `cell_u` | f32 | Width of one atlas cell in texture coordinates (§5.4) |
| 6 | `cell_v` | f32 | Height of one atlas cell (§5.4) |
| — | *(end)* | | Zero bytes left in all 507 |

Entry (14 bytes):

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `name_key` | u32 | Imposter name: a key of the enum mapper labelled RenderImposterNames (EMAPPER key 0x65; 0x230000E7 here). 816 distinct keys, all resolve. |
| 2 | `column` | u8 | Atlas column |
| 3 | `row` | u8 | Atlas row |
| 4 | `value_a` | f32 | Client default 1.0 (BIN). Observed 0.40–166.2, always positive. |
| 5 | `value_b` | f32 | Client default 0.0 (BIN). Observed −29.3 to 0.144. |

The entry's cell is (`column`, `row`); in all 12,406 entries the *i*-th stored entry has
`column` = *i* mod 16 and `row` = ⌊*i* / 16⌋. The client loads both texture lists as
dependencies (BIN).

Empty tables: 207 have no entries; 204 of them have both DIDs and both floats 0, 3 have
two texture-list DIDs and both floats 0.

**Entry floats (I5, BIN).** The client keeps the entry members in stored order and the
distant-sprite builder (`distant-landscape.md` §4.8, §5.6) multiplies both floats by the
sprite's `scale`:

- *S* = `value_a` × `scale` is the **size** of the sprite quad: its width and its height,
  in world units (`coordinates.md` §3);
- *B* = `value_b` × `scale` is the **height of the quad's lower edge above the sprite
  position** (negative: the quad reaches below the position; observed `value_b` from −29.3
  to 0.144, so most quads start slightly below the anchor).

The quad is a camera-facing billboard centred horizontally on the sprite position. With
*d* the unit vector from the sprite position to the eye, *R* = the horizontal unit vector
(−*d*.y, *d*.x, 0) / |(−*d*.y, *d*.x)| in the world frame of `coordinates.md` §3 (the viewer's
right on screen) and *U* = *d* × *R* (unit, pointing upward), its corners are

| Corner | Position | Texture coordinates (§5.4) |
|---|---|---|
| top left | P − (*S*/2)·*R* + (*B* + *S*)·*U* | (*u*₀, *v*₀) |
| bottom left | P − (*S*/2)·*R* + *B*·*U* | (*u*₀, *v*₁) |
| top right | P + (*S*/2)·*R* + (*B* + *S*)·*U* | (*u*₁, *v*₀) |
| bottom right | P + (*S*/2)·*R* + *B*·*U* | (*u*₁, *v*₁) |

where P is the sprite position. The quad turns with the view in both yaw and pitch (*U* is
perpendicular to *d*, not world-vertical). The registry's `Render_ImposterName` (an enum on
entities, mapper as above) names the same imposter keys on placed objects; how those are
drawn is `mesh.md` §8.7.

<a id="s5-3"></a>
### 5.3 The two atlases (Confirmed, I3; 300 non-empty tables)

Both texture lists are mip chains of 4 levels (`texture-list.md` §2.2, kind 2). The first
list's surfaces are DXT5 and the second's DXT1 in all 300 tables; both have the same size.
The registry defines `Render_ImposterSourceColorImage` and
`Render_ImposterSourceNormalImage`, matching the roles below.

**Roles (I6, BIN).** The client binds `colour_atlas` to texture parameter 3000 (the colour
texture) and `second_atlas` to parameter 3011 (the normal texture) of the sprite material
(`material.md` §8); when `second_atlas` is 0 it uses a sprite program without a normal
texture. The sprite pixel program reads them as follows:

- colour atlas: RGB is the albedo; A is the coverage, multiplied by the sprite's fade
  factor; the texel is discarded when that product is below the alpha-test threshold plus
  0.001;
- normal atlas: a tangent-space normal *n* = 2·(R, G, B) − 1 (each channel 0–1), in the
  quad's frame: R along *R* (§5.2; along −*R* for a mirrored sprite, §5.4), G along *U*, B
  along −*d* (away from the viewer). The A channel is not read.

The program lights the texel with the fake directional light, the ambient and the
hemispheric ambient (using *n*·*U* for the hemisphere) and applies distance fog; those
inputs are `lighting.md` §4.2 and §10.

<a id="s5-4"></a>
### 5.4 Atlas geometry (Confirmed (OBS), I3)

For a table with *n* ≥ 1 entries:

- columns *C* = the smallest power of two ≥ *n*, at most 16;
- rows *R* = the smallest power of two ≥ ⌈*n* / 16⌉;
- the top-level atlas surfaces are 128·*C* × 128·*R* texels (300 of 300);
- `cell_u` = (31/32) / *C* and `cell_v` = (31/32) / *R* (300 of 300; e.g. *n* = 19 gives
  *C* = 16, *R* = 2, `cell_u` = 0.060546875, `cell_v` = 0.484375).

**Cell texture coordinates (I7, Confirmed (BIN + OBS)).** The client derives the cell step
from the stored size: step_u = `cell_u` + `cell_u` / 31 (= 1 / *C*), step_v = `cell_v` + `cell_v` / 31 (= 1 / *R*). Cell
(`column`, `row`) covers

- *u*₀ = `column` × step_u, *u*₁ = *u*₀ + `cell_u`;
- *v*₀ = `row` × step_v, *v*₁ = *v*₀ + `cell_v`;

so the unused 1/32 of each cell step is at the **right and bottom** of the cell. In the data,
the last 4 texel columns and the last 4 texel rows of every used cell (12,406 of 12,406) have
alpha 0 in the top-level colour atlas, while the first columns and rows are partly covered.
The atlases' samplers use wrap addressing (`material.md` §7.5, address code 1, in all
permutations of the three sprite shader records).

**Mirrored sprites (BIN).** A sector sprite can be flagged as mirrored
(`distant-landscape.md` §5.6). For a mirrored sprite the client swaps the two *u* values
between the left and right corners and subtracts 1 from both: left corners get
*u*₁ − 1, right corners *u*₀ − 1. With wrap addressing this samples the same cell flipped
horizontally, and the negative *u* tells the program to negate the normal's R axis (§5.3).
The client also has a vertical flip of the same kind (*v* values swapped and reduced by 1),
applied when the colour atlas's texture object carries a renderer-internal flag and the
graphics device reports a capability; neither comes from the data. Tessera rule: never apply
the vertical flip.

---

<a id="s6"></a>
## 6. Compute shaders (prefix 0x2D, DB type 0x62) (Confirmed (BIN + OBS), CS1)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `did` | DID | Own DID |
| 2 | `size` *s* | u32 | Byte count of the blob. The client fails the load if *s* exceeds the bytes left (BIN). |
| 3 | `blob` | *s* bytes | A DXBC container (GEN): magic `DXBC`, 16-byte checksum, u32 1, u32 total size (= *s*), u32 chunk count, chunk offsets |
| — | *(end)* | | Record size = 8 + *s* in all 3 |

Observed: all three blobs have 5 chunks (RDEF, ISGN, OSGN, SHEX, STAT), program type 5
(compute) and shader model 5.0; sizes 20,780, 2,532 and 1,664 bytes. Their labels (§2)
say what they compute. Tessera does not execute DXBC; the explorer shows the chunk list.
The shaders' behaviour belongs to the specs of the effects that use them.

---

<a id="s7"></a>
## 7. Script channels (prefix 0x43, DB type 0x50)

<a id="s7-1"></a>
### 7.1 Model

The client's script system plays FX scripts on named **channels**. A channel lists the
scripts that may play on it with a priority and an "interruptible" flag, optional
transition rules, and whether the channel is exclusive. The record is written by the
client's reflection-based serialiser (BIN): object fields in registration order, maps as
hashed tables, arrays as VLE-counted lists. It has **no DID prologue**.

<a id="s7-2"></a>
### 7.2 Layout (Confirmed (BIN + OBS), SC1; 1 of 1 record, 3,285 bytes, zero left)

| Order | Field | Type | Meaning / observed |
|---|---|---|---|
| 1 | channels | hashed table (TSize) of (u32 `channel_key`, channel) | Hint 0, 4 channels |
| 2 | `global_transitions` | VLE count + transition × count | 0 here |

Channel:

| Order | Field | Type | Meaning / observed |
|---|---|---|---|
| 1 | scripts | hashed table (TSize) of (u32 `script_key`, script info) | 296, 34, 13 and 2 entries (hints 4, 1, 0, 0) |
| 2 | transitions | VLE count + transition × count | 0, 0, 12, 0 |
| 3 | `exclusive` | bool8 | 1 in all 4 |

Script info (5 bytes): u32 `priority` (observed 0, 1, 2, 10000), bool8 `interruptible`
(1 in 343 entries, 0 in 2).

Transition (12 bytes): u32 `from_script`, u32 `to_script`, u32 `run_script` (field roles
from the client's field names, BIN). Observed in the 12 transitions: `from_script` 0
(presumably "any"), `to_script` a script of the same channel, `run_script` one fixed
script.

<a id="s7-3"></a>
### 7.3 Keys (Confirmed, SC2)

- `channel_key`: 1 (the base mapper's default) or a key of the enum mapper labelled
  ScriptChannels (EMAPPER key 0x7C; observed 0x10000001, 0x10000003, 0x10000005).
- `script_key`, `from_script` (if not 0), `to_script`, `run_script`: keys of the enum
  mapper labelled FX (EMAPPER, second label group, key 0x1F). All 346 distinct script keys
  resolve in FX.
- The record is named by the `ScriptChannels` label of UNIQUEDB and by the type-20
  property `ScriptTable_ScriptChannels` (one 0x47 entity descriptor names it).

The playback rules are §7.4.

<a id="s7-4"></a>
### 7.4 Playback rules (BIN, SC3, SC4)

The client arbitrates per entity and per channel. Each entity keeps, for every channel it
has used, a **current** script (with the priority and interruptible flag it was admitted
with) and a **queue** of waiting requests ordered by priority.

**Finding the channel.** A script started by code may carry its own channel and script
info; otherwise the client looks the script key up in the record: the channel whose
`scripts` table holds the key, and that entry's script info. A script found in no channel
is not managed: it plays at once.

**A play request** (script *N* with priority *p*ₙ and interruptible *i*ₙ, on channel *K*):

1. If channel *K* has `exclusive` = 0, *N* plays at once; nothing is recorded.
2. If the entity has no current script on *K*, *N* becomes current and plays.
3. Otherwise, with *C* the current script (priority *p*꜀, interruptible *i*꜀), *N* is
   **refused** when *p*ₙ < *p*꜀, or *p*ₙ = *p*꜀ and *i*꜀ = 0. (The client also has a hold
   state set by code and a forced request; a held channel refuses, a forced request is never
   refused. Neither is in the record.)
4. A refused request is either dropped or queued, by a **queue mode** that comes with the
   request (not from this record): mode 3 never queues; mode 1 queues only when *p*ₙ is
   greater than the priority of the first queued entry (or the queue is empty); every other
   mode always queues. A queued request is inserted in front of the first entry *E* with a
   strictly lower priority (so equal priorities keep arrival order), or appended when
   there is none. After an insertion in front of *E*, every entry behind *E* (not *E*
   itself) that was queued with mode 1 is stopped and removed. The requester is told
   "queued" only for an insertion in front of an existing entry; a dropped or appended
   request is reported as refused (an appended entry nevertheless stays in the queue).
5. A request that is not refused **replaces** *C*: *C* is stopped and the client looks for a
   transition from *C* to *N* (below). Without a transition *N* becomes current and plays.
   With one, the channel has no current script and *N* is reported as waiting; the
   transition's `run_script` is expected to play next, and when it starts the channel
   records it as the running transition. In both cases every queued entry with mode 1 is
   then stopped and removed.

**Transition choice (SC4).** Only when *C* and *N* are on the same channel and both keys are
non-zero. In the channel's own `transitions` list, in stored order, the client takes the
first entry with (`from_script`, `to_script`) = (*C*, *N*); failing that the first with
(0, *N*); then (*C*, 0); then (0, 0). So 0 means "any" in either position. The record's
`global_transitions` list is not consulted by this lookup.

**When a script stops** (a script of an exclusive channel): a pending or running
transition on that channel is cancelled first (its script is stopped). Then, if the
stopped script is the current one and the queue is not empty, the first queued entry is
removed and goes through step 5 against the stopped script, and is started when it becomes
current; with an empty queue the channel is left without a current script. A stopped script
that was only queued is removed from the queue.

> **OPEN:** (1) which queue mode each caller passes (the value travels with the request
> from the code that starts the script; script-table nodes were not traced for it); (2)
> what admits *N* after a transition's `run_script` ends (the hand-over after a transition
> was not traced past the point where *N* is reported as waiting); (3) whether anything
> reads `global_transitions` (empty in this install). Tessera rule until settled: treat every
> request as mode 0, and start *N* when the transition script ends.

---

<a id="s8"></a>
## 8. Locale record (prefix 0x58, DB type 0x46)

Fully specified in [`string-table.md`](string-table.md) §10.3 (layout, field use, BIN
provenance). Nothing is added here. Its row in `records.md` §2.1 should point there (§18).

---

<a id="s9"></a>
## 9. Name-filter table (0x0E200000, DB type 0x14) (Confirmed (BIN + OBS), NF1)

The table holds per-"rule set" parameters for validating character and other names. Layout
(1 of 1 record, 2,685 bytes, zero left):

| Order | Field | Type | Meaning / observed |
|---|---|---|---|
| 1 | `did` | DID | 0x0E200000 |
| 2 | rule sets | hashed table (TSize) of (u32 `rule_key`, rule set) | Hint 1, 3 rule sets. Keys 1, 2 and 0x10000003: keys of the enum mapper labelled NameRules (EMAPPER key 0x02). |

Rule set (field order and meanings BIN, NF2; values OBS, 3 of 3 rule sets). The former
neutral names are kept in brackets for implementers who used them.

| Order | Field | Type | Meaning | Observed |
|---|---|---|---|---|
| 1 | `letter_classes` [`n1`] | u32 | Bit mask of character classes (row 11) in which letters are allowed (§9.1) | 6935, 2833, 6935 |
| 2 | `digit_classes` [`n2`] | u32 | Bit mask of classes in which decimal digits are allowed | 0, 2817, 0 |
| 3 | `printable_classes` [`n3`] | u32 | Bit mask of classes in which any printable character is allowed | 5368, 5374, 5368 |
| 4 | `max_repeat` [`n4`] | u32 | Most identical characters allowed in a row; 0 = no limit (§9.2) | 2 in all |
| 5 | `max_vowel_run` [`n5`] | u32 | Most vowels (other than `extra_vowels`) allowed in a row; 0 = no limit (§9.2) | 4 in all |
| 6 | `vowel_within` [`n6`] | u32 | A vowel must occur among the first `vowel_within` letters; 0 = no check (§9.2) | 6 in all |
| 7 | `max_consonant_run` [`n7`] | u32 | Every `max_consonant_run` letters in a row must contain a vowel, i.e. at most `max_consonant_run` − 1 non-vowels in a row; 0 = no check (§9.2) | 7 in all |
| 8 | `letter_groups` [`string_list_a`] | u32 count + UTF-16 string × count | Letter sequences that count as one letter for `vowel_within` and `max_consonant_run` (§9.2) | 4 strings of 2, 2, 2 and 3 units in each rule set |
| 9 | `extra_vowels` [`string_a`] | UTF-16 string | Characters that count as vowels for `vowel_within` and `max_consonant_run` and break a vowel run for `max_vowel_run` (§9.2) | 3 units in each rule set |
| 10 | `forbidden_first` [`string_b`] | UTF-16 string | Characters a name may not start with (§9.2) | empty in all 3 |
| 11 | character classes | hashed table (TSize) of (u32 `class_key`, UTF-16 string `allowed`, u32 count + UTF-16 string × count `excluded_ranges`) | One entry per context class; `allowed` lists characters always allowed in that class; each `excluded_ranges` string gives an inclusive range by its first two units (first, last) (§9.1) | Hint 1, 13 entries in each rule set; keys are the single bits 1, 2, 4, …, 4096; `allowed` 0–62 units; 12 classes have one 2-unit range (first ≤ last in all 36), class 16 has none, in each rule set |
| 12 | `taboo_with_rules` [`flag_a`] | bool8 (BIN) | Selects how the taboo check is run for this rule set (§10.1) | 1, 0, 1 |
| 13 | `languages` [`u32_list`] | u32 count + u32 × count | Keys of the enum mapper labelled Languages (EMAPPER key 0x4A; 0x230000B1 here). The client's UI tests whether a language value held by its data layer (by the mapper, the client language) is in this list | rule 1: 2, 0x10000004, 0x10000003, 0x10000001; rule 2: none; rule 0x10000003: 0x10000004, 0x10000001, 0x10000003 (all resolve) |
| 14 | `flag_b` | bool8 (BIN) | A per-rule-set switch that the client hands to its UI; meaning OPEN (§9.3) | 0 in all |

The strings are data and are not reproduced here.

**Context classes.** The single-bit class keys are **contexts**: the code that checks a
string passes one context bit *m*. The same bit selects the character class of §9.1 and the
taboo lists of §10.1 that apply.

<a id="s9-1"></a>
### 9.1 Character check (BIN, NF3)

Character *c* is allowed in context *m* under rule set *r* when all of these hold:

1. *c* is not `[` (U+005B) or `]` (U+005D);
2. and either
   - *c* passes the **type test**: (`letter_classes` AND *m*) ≠ 0 and *c* is a letter, or
     (`digit_classes` AND *m*) ≠ 0 and *c* is a decimal digit, or
     (`printable_classes` AND *m*) ≠ 0 and *c* is printable; **and** *c* lies in none of
     the `excluded_ranges` of the class whose key equals *m*;
   - or *c* occurs in the `allowed` string of the class whose key equals *m*.

The class is looked up by the exact key *m*; for a value of *m* that is not a key, there
are no ranges and no `allowed` characters. An unknown rule-set key allows no character.
Letter, digit and printable are the C runtime's wide-character classifications on the
client's platform (Tessera: Unicode letter categories L*, decimal digits Nd, and "not a
control, format, surrogate or unassigned code point" for printable).

A **string check** in context *m* rejects an empty string and any string with a
character that fails the character check.

<a id="s9-2"></a>
### 9.2 Name check (BIN, NF3)

The client checks a name against rule set *r* on a lower-cased copy of the name (C runtime
lower-casing; Tessera: Unicode simple lower-case mapping). An unknown rule-set key passes
every step. Steps, in this order; the first failing step rejects the name:

1. **Repeats.** If `max_repeat` ≠ 0: no character may occur more than `max_repeat` times
   in a row.
2. **Collapsed form.** For steps 3 and 5 the client builds a collapsed copy: walking the name
   from the start, at each position it keeps the character there and advances by the length
   of the first `letter_groups` string (in stored order) that matches the name at that
   position, compared case-insensitively over that length (cut at the name's end); with no
   match it advances by one. So each letter group shrinks to its first character.
3. **Early vowel.** If `vowel_within` ≠ 0: among the first min(`vowel_within`, length)
   characters of the collapsed copy at least one must be a vowel or occur in
   `extra_vowels`.
4. **Vowel run.** If `max_vowel_run` ≠ 0: on the lower-cased name (not collapsed), no more
   than `max_vowel_run` characters in a row may be vowels that are not in `extra_vowels`
   (a character in `extra_vowels`, or any non-vowel, ends the run).
5. **Consonant run.** If `max_consonant_run` = *k* ≠ 0: on the collapsed copy of length
   *n*, if *n* < *k* it must contain at least one vowel-or-`extra_vowels` character;
   otherwise no *k* characters in a row may all be neither vowels nor in `extra_vowels`.
6. **First character.** If the name is not empty, its first character must not occur in
   `forbidden_first`.

**Vowels** are exactly these code points (a fixed set of the client's vowel test, decision
0012): U+0041, U+0045, U+0049, U+004F, U+0055, U+0061, U+0065, U+0069, U+006F, U+0075,
U+00C0, U+00C1, U+00C4, U+00C8, U+00C9, U+00D6, U+00DA, U+00DC, U+00E0, U+00E1, U+00E4,
U+00E8, U+00E9, U+00F6, U+00F9, U+00FC. The set is not symmetric between upper and lower
case (for example U+00F9 is a vowel but U+00FA is not); since the checks run on the
lower-cased name, a character counts as a vowel when its lower-cased form is in the set.

Tessera rule: an empty `letter_groups` string is skipped (the client would not advance).

<a id="s9-3"></a>
### 9.3 Open points

> **OPEN:** (1) which context bit and which rule-set key the client's name-entry code
> passes for character names and for other names, and in which order it runs the string
> check (§9.1), the name check (§9.2) and the taboo check (§10.1) (the callers were not
> traced); (2) what `flag_b` switches in the client's UI. Name validation in the original
> is repeated server-side; Tessera needs these rules only for its own server (M7), which may
> pick the context and order itself.

---

<a id="s10"></a>
## 10. Taboo table (0x0EBADA55, DB type 0x22) (Confirmed (BIN + OBS), TB1)

A localized word filter. Layout (1 of 1 record, English archive, 385,598 bytes, zero left):

| Order | Field | Type | Meaning / observed |
|---|---|---|---|
| 1 | `did` | DID | 0x0EBADA55 |
| 2 | `list_count` *k* | u32 | 8 |
| 3 | list headers | *k* × (Pascal string `list_name`, u32 `category` [`a`], u32 `contexts` [`b`]) | Names are source-file names; (`category`, `contexts`) observed (4, 0x7FFFFFFF), (4, 0x1387), (3, 0x1387), (3, 0x1387), (3, 0x1004), (2, 0x303), (1, 0x1387), (5, 0x7FFFFFFF) |
| 4 | lists | hashed table (TSize) keyed by Pascal string, value u32 count + UTF-16 string × count | Hint 1, 8 entries; each key equals one header's `list_name`; 39 to 6,708 strings per list, 17,612 in all |

The strings are filter patterns (regular expressions, §10.1). None is reproduced here.

**Header fields (TB2, BIN).**

- `contexts`: bit mask of the contexts (§9) in which the list applies; a list is used for a
  check in context *m* when (`contexts` AND *m*) ≠ 0. 0x7FFFFFFF applies everywhere.
- `category`: what a match in this list means; the check returns one of five distinct
  result categories for `category` 1, 2, 3, 4 and 5. A match in a list with `category` 0 is
  ignored (the scan goes on); a match in a list with any other value stops the scan and
  counts as no match.

**String hash (TB3).** The table is bucketed (`records.md` §1.3) by this hash of the key's
bytes: start with *h* = 0; for each byte *b*, taken as a **signed** 8-bit value, set
*h* = (16·*h* + *b*) mod 2³²; then, if any of the top four bits of *h* is set, set
*h* = (*h* XOR (top four bits shifted right by 24)) AND 0x0FFFFFFF. A final 0xFFFFFFFF
becomes 0xFFFFFFFE. The bucket is *h* mod the bucket count. The 8 stored keys are in
non-decreasing bucket order for hint 1 (bucket count 23). A reader does not need the hash;
a byte-exact writer does.

<a id="s10-1"></a>
### 10.1 Matching (BIN, TB4)

**Pattern dialect.** The client compiles each pattern with the PCRE2 library (identified by
its error texts), passing pattern and subject as 16-bit code-unit strings, with the caseless
option (option value 0x8) and no other option; UTF mode is not requested. A pattern matches
when a search anywhere in the subject succeeds (not anchored). Without UTF mode, caseless
matching folds only code points below 256 (the library's character tables). A pattern that
fails to compile never matches; an empty pattern or an empty subject never matches.

**Checking one string *t* in context *m* with rule set *r*** (the matcher of the rule-set
variant below):

1. An empty *t* matches nothing.
2. For each list header whose `contexts` shares a bit with *m*, the client takes that list
   and builds the subject: *t* lower-cased, keeping only the characters that pass the
   character check of §9.1 for (*r*, *m*).
3. The patterns of the list are tried in stored order; the first match decides for that
   list: with `category` 0 the scan continues with the next header, otherwise it stops and
   the result is the list's category (1–5) or no match (other values).

**Variants.** When the name-filter rule set *r* in use has `taboo_with_rules` = 1, the client
runs the rule-set variant: it cuts the input into pieces with a tokenizer that was not
traced and checks each piece as above. Otherwise it runs a variant without the
character filter, which was not traced in detail.

> **OPEN:** (1) the order in which the client walks the list headers (stored order is
> likely, the reader's list insertion was not checked); (2) the tokenizer of the rule-set
> variant and the details of the other variant; (3) which context bits the client passes
> for chat and for names. Needed only for a chat or name filter (M7 and later).

---

<a id="s11"></a>
## 11. Compression dictionaries (0x0E010000–0x0E010002, DB type 0x01) — stored form only

**Gate (until 2026-10-11).** The three labels of these records (COMPRESSIONDICTIONARY DID map 0x28000010,
keys 0x10000001–0x10000003) name directions of client–server traffic. Their use belongs to
the official network protocol, which was behind the M8 gate when this section was
written (decision 0004) and since 2026-10-11 belongs to M8's own specs (decision 0034 §2).
This section gives only what a container tool needs to list and validate
the records; nothing here describes how the dictionaries are applied, and no BIN reading
was done for them. The entries' directory timestamps date from 2007.

Stored envelope (Confirmed, CD1; 3 of 3):

| Offset | Size | Type | Meaning / observed |
|---|---|---|---|
| 0 | 16 | bytes | An identifier, the same in all three records |
| 16 | 16 | bytes | A second identifier; 0x0E010001 and 0x0E010002 share one value, 0x0E010000 has another |
| 32 | 4 | u32 | 0 in 0x0E010000; 0x0E010001 in 0x0E010001 and 0x0E010002 (a DID link) |
| 36 | 4 | u32 | 1, 1, 0 |
| 40 | 4 | u32 | *m* = number of bytes that follow (218,203; 214,909; 0) |
| 44 | *m* | bytes | Body (not specified) |

The records have no DID prologue (`records.md` §1.6). A validator checks that the record
is exactly 44 + *m* bytes long.

> **OPEN (M8):** the body layout and the meaning of every field above. Not researched
> before decision 0034 (2026-10-11); now research for M8 within its §2 scope.

---

<a id="s12"></a>
## 12. Bare property sets (prefix 0x78, DB type 0x32)

<a id="s12-1"></a>
### 12.1 Layout

A 0x78 record is a property-set record: own DID, then an embedded property set
(`property-system.md` §5–§6). All 1,618 decode with the generic reader; 9 are empty
(0x78000151 in the English archive is the 6-byte DID + `00 00`). There is no per-family
layout: what a set means is given by the properties it carries and by who names it.

<a id="s12-2"></a>
### 12.2 Families (Confirmed, PS1; 1,616 gamelogic sets, 1,608 non-empty)

Families by the properties they carry (each set counted once, first matching row):

| Family | Sets | Characteristic properties | Named by | Spec |
|---|---|---|---|---|
| Game maps | 1,300 | `UI_Map_GameMap` (struct: `UI_Map_MapImage` (0x41), `UI_Map_Scale`, `UI_Map_PixelOffsetX/Y`, `UI_Map_BlockOffsetX/Y`, `UI_Map_FogOfWar`, `UI_Map_FogOfWar_Color`); 21 of them also `Landblock_LightMap` | `Dungeon_MapData` and `Dungeon_MiniMapData` (1,303 references each, 0x79 sets of dungeon objects), `Area_MapData` (1) | map spec (not yet written); `wstate.md` §11.4 |
| Local sky | 147 | `Environment_LocalSky`; 7 also `InteriorFogNear/Far/Color` (two further sets carry only `Environment_PlayerIndoorAmbianceOverride` and are counted under Other) | `Monitor_VolumePropertiesToApply` in `Entity_Monitors` structs of 0x47 descriptors (156 references; all 147 sets) | `environment.md` §7, `sound.md` §10.3.4 |
| Footstep / surface sounds | 38 | `SND_Footstep_Surfaces`; 19 also `Physics_SurfacePropertiesToApply`, `Content_IAmMadeOf`; 18 `Base_Surface_Type` | `Physics_SurfacePropertiesToApply` (52 in 0x47, 19 in 0x78; 20 of the 38 sets) | `sound.md` |
| Ambient music | 25 | `Ambient_MusicRegion`, `Ambient_MusicType` | `Monitor_VolumePropertiesToApply` (25 references; all 25 sets) | `sound.md` §10.3.4 |
| Content-layer defaults | 13 | `World_PresentTimeOfDay`, `World_IsNight` | CONTENT_LAYER DID map 0x28000012 (1,660 entries, 1,626 of them to 0x7800001C) | `entity-descriptor.md` §4.5.1 |
| Volumes | 9 | `Volume_IsLiquid`, `PathMap_*` | `Monitor_VolumePropertiesToApply` (145 references; all 9 sets) | `collision.md`, `water.md` §8 |
| Other | 76 | singletons: UI tables, option lists, script-callback parameter sets, physics-controller tuning, character-creation presets (`Chargen_AvatarPresetsArray`, 8), bone sliders (5), stat trees (5) … | UNIQUEDB (36 labels to 0x78), DBPC (9), SCRIPTCALLBACK (12), PHYSICSCONTROLLER (6); typed: `Monitor_VolumePropertiesToApply` (56), `UIViewport_LightingScene` (19), `AppearanceUI_PresetsFile` (12), `AppearanceUI_BoneSliderDefinitionsFile` (12) | per subject |

For the explorer: show a 0x78 set with the generic property view, title it with the DID-map
label(s) that point at it (a set may have several, e.g. 0x78000007 is both
`GameplayOptionList` and `Preferences_Gameplay_Option_List`), and offer a typed view for the
game-map family (image with the offsets and scale).

**Referrers (PS2, Confirmed 2026-10-08).** The "Named by" column counts typed references in
all property sets of 0x47 descriptors, 0x79 sets and 0x78 sets, **including struct members
and array elements** (the first census did not count 0x78 targets inside 0x47 descriptors,
which is why the monitor references were missing). Every set of the local-sky, ambient-music and
volume families is named by at least one `Monitor_VolumePropertiesToApply`. A raw 4-byte
scan of every record of the general, gamelogic, English, cell and map archives finds the
same 0x47 counts exactly.

**Raw matches in object records (Confirmed, X5).** The raw scan finds 4,112 four-byte values
equal to the DID of one of the 1,618 0x78 sets inside 795 object records (0x70). Every one of
these records was decoded with `wstate.md`, and none of the 4,112 is a decoded integer, DID,
key or reference of the record: they are byte patterns that span field boundaries or lie inside
other values (most at unaligned offsets, some at a regular stride inside large native maps). So
no object record refers to a 0x78 set.

> **OPEN:** the raw scan also finds such values inside script tables (0x07), cells (0x01) and
> landblock records (0x80); those records were not decoded for this question, so whether
> those matches are references or chance byte patterns is not known. A viewer lists only the
> typed referrers.

---

<a id="s13"></a>
## 13. Metadata objects in the object-state range (0x70, DB type 0x45)

These are ordinary object records (`wstate.md` §3) whose payload of interest is their
property set at DID + 0x09000000 (`property-system.md` §6.2). Root classes and controls
differ per family (X6; corrected 2026-10-09):

| Family | Members | Root classes of the members | Control (WEENIECONTENT label, §14) |
|---|---|---|---|
| Property metadata (§13.1) | 5,211 | 1 | `PropertyMetaDataControl` |
| Error categories (§13.2) | 675 | 1 | `ErrorControl` |
| Tutorial hints (§13.3) | 337 | 1 | `TutorialHintControl` |
| Movie library (§13.4) | 85 | 1 | `MovieLibraryControl` |
| Property responses (§13.5) | 419 | 6 | none |
| Game callbacks (§13.6) | 110 | 110 (one each) | none |
| Version (§13.7) | 123 | 31 | `VersionSystem` |

Where there is a control, it is a separate singleton object of its own class, named in
WEENIECONTENT, whose property set lists the members (how completely is stated per family);
it lacks the property by which its members are recognised. The property-response and game-callback
families have no control: no WEENIECONTENT entry is one of their members or names one, and
they are found by their properties (§13.5 rule, §13.6 `GameCallback_ReturnValue`). Class
indices and counts are observations of this build; families are recognised by the control
and by the properties, not by class number.

<a id="s13-1"></a>
### 13.1 Property metadata (Confirmed, M1; 5,211 objects)

Control: WEENIECONTENT label `PropertyMetaDataControl` (0x70000214 here). Its set holds
`PropertyMetaData_PropertyMetaDataList`, an array of 5,211 DIDs that is exactly the set of
property-metadata objects (no duplicates, none missing). Every effect object (root class
734 here) imports the control as a class-wide dependency (`wstate.md` §3.2).

Properties of each object:

| Property | Type | Present | Meaning / observed |
|---|---|---|---|
| `PropertyMetaData_Property` | PID (type 10) | 5,211 | The property described. **Unique** across all objects; every value is defined in the dictionary. Build a PID → object index from it. |
| `PropertyMetaData_Name` | StringInfo | 5,211 | Display name: a string-table reference, token 228870261 in all, in one of 5,188 different tables |
| `PropertyMetaData_DisplayAsPercentage` | bool | 2,094 (all true) | Show the value as a percentage |
| `PropertyMetaData_AuctionPropModCategory` | enum (mapper 0x230002A3) | 5,211 | Grouping category; 44 values (most common labels: EffectOverride 1,142, Misc 887, Damage 614, Virtues 397) |
| `PropertyMetaData_AllowedOperatorList` | array of `PropertyMetaData_AllowedOperator` (enum, mapper 0x23000046) | 5,211 | 1–5 operators per property: Add 4,498, Set 4,415, Subtract 4,048, Multiply 2,474, Divide 539, Or 44, ArrayAdd 12 |
| `PropertyMetaData_ExaminationMod` | struct | 5,211 | Always holds `PropertyMetaData_ExaminationMod_PropertyOp` (same enum; Invalid 5,109, Add 97, Multiply 3, Subtract 2); in a few objects also values of other properties |
| `PropertyMetaData_HideIdentityExaminationMod` | bool | 5,211 | true in 5,210 |
| `PropertyMetaData_ForwardedValueTarget` | PID | 364 | A property that receives this one's value (all defined) |
| `PropertyMetaData_ForwardedValue_RequiredWeenieType` | enum (mapper 0x230000C9) | 352 | Player 183, Agent 168, DistantBattleUnit 1 |

Types of the 5,211 described properties: Float 3,152, Array 1,159, Int 625, Bool 152,
DidRef 51, Enum 44, Bitfield32 24, Bitfield64 2, String 1, BitfieldAny 1. That is, the
metadata covers mostly gameplay statistics and modifiers, not all 27,546 dictionary
entries.

Explorer use: in any property view, if the PID has a metadata object, show its display
name next to the internal name, group by auction category, and mark percentage values.

**Percentage display (Confirmed (bytecode), X4; settles the scale part of the former OPEN).**
The client library's code that writes the description of a modifier entry (`combat-data.md`
§5; class 1341 here, also reached from the relic class 2219) asks the metadata control (class
1392 here) for the modified property's `PropertyMetaData_DisplayAsPercentage` (false when the
property has no metadata object). When the flag is true and the entry's value *v* is a float:

1. operator `Multiply` (key 8 of mapper 0x23000046): *d* = *v* − 1 when *v* > 1 and 1 − *v*
   when *v* < 1 (single precision); in every other case (*v* exactly 1, or NaN) *d* stays *v*
   (corrected 2026-10-09). The text template is the "increase" form when *v* ≥ 1 and the
   "decrease" form otherwise (so NaN takes the decrease form);
2. any other operator: *d* = *v*;
3. the number shown is *d* × 100, multiplied in single precision and then widened to double for
   formatting.

So a stored 0.05 with `Add` shows as 5, a `Multiply` by 1.05 as 5 (increase), one by 0.9 as
10 (decrease) and one by exactly 1 as 100 (increase), each in a percentage template. Without
the flag a float is shown unscaled. Int and int64 values are shown unscaled whatever the flag:
the code's cases for those value types do not consult it. (The flag is also tested once before
the value type is examined, to choose between two template keys for every value type.) The
template texts are string-table entries chosen by key and are not reproduced here.

> **OPEN:** the operator semantics of `AllowedOperator` and `ExaminationMod` (the percentage
> scale is settled above, X4). Not in the data. A scan of the client's native code
> (2026-10-08, BIN fourth use) finds no immediate use of the property ids of
> `…_AllowedOperatorList` or `…_Property`; the id of `…_ExaminationMod` occurs only in the
> examination window's code. The class library names neither `…_AllowedOperatorList` nor
> `…_ExaminationMod` by a 32-bit literal (BC, 2026-10-09). An owner observation or a reading of
> the examination window's native code would settle them.

<a id="s13-2"></a>
### 13.2 Error categories (Confirmed, M2; 675 objects)

Each object has `ErrorCategory_BriefString` (StringInfo, all 675), and optionally
`ErrorCategory_Sound` (a sound-info DID, 0x2A…; 673, all the same DID),
`ErrorCategory_Script` (a key of the FX enum mapper; 2) and `ErrorCategory_Icon` (a render surface; 1).

Control: WEENIECONTENT label `ErrorControl` (0x70000222 here) with
`ErrorControl_DefaultIcon` (0x41 DID), `ErrorControl_DefaultCategory` (one category DID)
and `ErrorControl_CategoryArray`: 675 structs, each `ErrorControl_CategoryErrorTypeArray`
(keys of the enum mapper labelled ErrorType; 950 keys in all, each in one struct only) and
`ErrorControl_CategoryDID`. The 675 structs name 674 distinct categories (one twice); the
remaining category is the default. To classify an error: find the struct listing its
ErrorType key; if none, use the default category.

<a id="s13-3"></a>
### 13.3 Tutorial hints (Confirmed, M3; 337 objects)

Each object: `TutorialHint_Name` (StringInfo), `TutorialHint_Type` (enum, mapper
0x23000127; 337 distinct values), `TutorialHint_HintList` and `TutorialHint_MPHintList`
(arrays of 3 `TutorialHint_Section` structs, each `TutorialHint_Text` (StringInfo) and
`TutorialHint_Image` (0x41 DID or 0)), `TutorialHint_AutoShow` (true),
`TutorialHint_NeverShowAgainDefault` (true), `TutorialHint_UseMPOverrides` (true in 16);
optional conditions `TutorialHint_ShowOnlyWhenServerMaxLevelAtOrAbove` (int, 98),
`TutorialHint_ShowOnlyWhenWorldProperty` (PID, 4), `TutorialHint_ShowOnlyToSubscribers`
(1), `TutorialHint_ShowOnlyWhenWebstoreEnabled` (1).

Control: `TutorialHintControl` (0x70000220 here): `TutorialHintControl_HintArray` (331
DIDs), `TutorialHintControl_MPHintArray` (50), `TutorialHintControl_SessionHintArray` (4),
`TutorialHintControl_SkillHintArray` (empty) and `TutorialHintControl_LevelHintsArray` (20
structs of level and hint type). The arrays together name all 337 objects (385 references).

<a id="s13-4"></a>
### 13.4 Movie library (Confirmed, M4; 85 objects)

Each object: `MovieLibrary_CategoryType` (enum, mapper 0x23000262), `MovieLibrary_Description`,
`MovieLibrary_Tooltip`, `MovieLibrary_Filename` (StringInfo each; the file name is
localized text), `MovieLibrary_Icon` (0x41, the same in all), `MovieLibrary_SortPriority`
(int), and in 65 objects `MovieLibrary_MovieID` (enum, mapper 0x23000261) and
`MovieLibrary_QuestDID` (0x70 quest). Control: `MovieLibraryControl` (0x7000B566 here) with
`MovieLibrary_CategoryList` (48 category keys, display order) and `MovieLibrary_MovieList`
(85 DIDs). Other references: `Keyframe_MovieTrack` (65), `RaceTable_RaceSelect_Movie_DID`
(15), `AdvTable_CharGen_ClassSelect_Movie_DID` (12). The movie files themselves are not in
the archives (prefix 0x0D has no records, `records.md` §2.1).

<a id="s13-5"></a>
### 13.5 Property responses (Confirmed census, M5; 419 objects)

Six root classes (X1). **Members** are the objects whose property set carries, at top level,
any property whose label starts with `PropertyResponse` (that is `PropertyResponse_…` or
`PropertyResponseCallback_…`) other than the two referrer properties
`PropertyResponse_Callback` and `PropertyResponse_EntityPropertyResponseMap`; this finds 419.
By the first characteristic array present: 237 carry `PropertyResponseCallback_AppliedEffect_Array`
(effect lists, with `PropertyResponseCallback_UseTargetLevelForSpellcraft`,
`…_OnTrueResponseType`, `…_OnFalseResponseType`, `…_AIHint`, `…_Drama` in some); 120 carry
`PropertyResponse_WorldEventResponse_List`; 21 `PropertyResponse_MapProperty_Array`/`_EntityType`;
9 `PropertyResponse_CalculateProperty_Array`/`_EntityType`; none has only
`PropertyResponse_Filter_Array`; the other 32 carry only `PropertyResponseCallback_*` callback
settings (`_OnTrueResponseType`, `_OnFalseResponseType`, `_AIHint`, `_Drama`) and no array. They are referenced by `PropertyResponse_Callback` (2,034),
`Area_PropertyResponseMapDID` (640), `PrivateEncounterTemplate_PropertyResponseMapDID`
(606), `PropertyResponse_EntityPropertyResponseMap` (4) and
`WorldEventGenerator_ResponseCallback` (3). This is server-side gameplay logic; Tessera
decodes and shows it generically and gives it meaning only in its own server work (M7).
Four monster objects also carry `PropertyResponse_*` properties; they are not part of the
family.

> **Answered (2026-10-09; was OPEN, census correction proposed for the spec writer):** the breakdown above leaves
> out 32 members. Measured by the implementation (2026-10-07, `crates/formats` real-data
> test): the five characteristic arrays (first present wins) find 387 objects (237 effect
> lists, 120 world-event lists, 21 map-property and 9 calculate-property arrays, none with
> only a filter array). The other 32 carry no `PropertyResponseCallback_AppliedEffect_Array`
> and none of the other arrays, only `PropertyResponseCallback_*` callback settings
> (`_OnTrueResponseType`, `_OnFalseResponseType`, `_AIHint`, `_Drama`). Recognition rule
> used by Tessera, by label like `Version_*` (§13.7) and `PathMap_*` (§12.2): a member is an
> object whose set carries, at top level, any property whose label starts with
> `PropertyResponse` (`PropertyResponse_` or `PropertyResponseCallback_`), except the two
> referrer properties named above (`PropertyResponse_Callback`,
> `PropertyResponse_EntityPropertyResponseMap`). That finds exactly 419. The four monster
> objects carry only `PropertyResponse_EntityPropertyResponseMap` (a referrer) and are
> not members. Proposed: say "six root classes: 237 effect lists, 32 callback-only
> objects, 120 world-event lists, 21 map-property, 9 calculate-property arrays" and state
> the rule.
>
> *Answer:* accepted and re-measured (X1): the paragraph above now states the label rule and
> the full breakdown.

<a id="s13-6"></a>
### 13.6 Game callbacks (Confirmed, M6; 110 objects)

110 objects, each of its own root class (all descendants of one library class), each with
exactly one property `GameCallback_ReturnValue`, a PID naming the `GCParam_*` property in
which the callback's result is returned (7 distinct: a boolean 82, a script value 19, an
instance ID 5, and one each of an enum, a StringInfo, a float and an integer); one object
also has `GCParam_PEMsgType`. They are the targets of script node 27 GameCallback
(`script-table.md`, its `GameScript_Package` DID): their DIDs occur in script tables (0x07)
and scriptlets (0x0C), in two object records, and in no property set. The callback's behaviour is code, not
data.

<a id="s13-7"></a>
### 13.7 Version objects (Confirmed census, M7; 124 objects)

124 objects in 32 root classes carry `Version_*` properties: 123 **members** in 31 root classes
and the control (X2). Members describe migrations (quest lists, trait resets,
skill untraining, chat-type additions, property renames with `Version_GOP_UpdateArray`, item
conversions …). Control: `VersionSystem` (0x7000021D here) with
`Version_VersionNumberDataList`, `Version_AlwaysRunOnPlayers` (2 DIDs) and `Version_Number`
(the current update). The data list holds 93 structs, each with a `Version_Number` (an enum of
content updates; 93 distinct values) and one DID array: `Version_SingletonList` in 92 structs,
`AccountVersion_SingletonList` (one DID) in the other one. The `Version_SingletonList` arrays
name 155 distinct DIDs: 121 of the 123 members and 34 other objects. The remaining 2 members
are exactly the 2 DIDs of `Version_AlwaysRunOnPlayers`, which no number list names. In all the
control names 158 distinct DIDs: all 123 members plus 35 other 0x70 objects (the 34
and the account entry) that carry no `Version_*` properties and are therefore not members
(X2; corrected 2026-10-09). A reader that wants every member from the control alone takes the
union of the `Version_SingletonList` arrays and `Version_AlwaysRunOnPlayers`, and filters by
the `Version_*` rule above. The family's objects describe server-side migrations of
character data. Tessera shows them generically; no client behaviour depends on them.

> **Answered (2026-10-09; was OPEN):** whether the 124 include the `VersionSystem` control. The §13 introduction says
> Version has no control, this section names one. Measured by the implementation: 123
> objects carry `Version_*` properties and lack the control-only properties
> (`Version_VersionNumberDataList`, `Version_AlwaysRunOnPlayers`); with the control that is
> 124. The implementation leaves the control out of the family; its real-data test pins
> 123 members plus the control. Proposed: reword the §13 introduction or this section so
> both agree on whether `VersionSystem` is a control and whether it is counted.
>
> *Answer:* the 124 include the control. `VersionSystem` is the family's control (it is named
> in WEENIECONTENT, §14, and between its number list and `Version_AlwaysRunOnPlayers` it names
> all 123 members, along with 35 objects that are not members; corrected 2026-10-09, see the
> paragraph above); the §13 introduction is corrected, and this section now says 123 members
> plus the control, as the implementation counts (X2).

---

<a id="s14"></a>
## 14. The WEENIECONTENT DID map (0x2800000E) (Confirmed, W1)

Root key 0x0A of the root DID map (`property-system.md` §8). 316 entries: the null entry,
one script table (`SharedUISelection_ScriptTable`, 0x07000000) and **314 object records**
(all 0x70, 263 distinct root classes). By label: 109 contain "Control", 38 "Dialog", 33
"Directory", 13 "Manager", 5 "Table"; 23 point at skill objects. Labels used by this spec:

| Label | Points at (here) | Used in |
|---|---|---|
| `PropertyMetaDataControl` | 0x70000214 | §13.1 |
| `ErrorControl` | 0x70000222 | §13.2 |
| `TutorialHintControl` | 0x70000220 | §13.3 |
| `MovieLibraryControl` | 0x7000B566 | §13.4 |
| `VersionSystem` | 0x7000021D | §13.7 |

A client resolves these by label at run time; the DIDs are observations of this install.
The other singletons belong to the specs of their subsystems.

---

<a id="s15"></a>
## 15. Reference graph (Confirmed, G1)

Typed references into this spec's records, found in all property sets of 0x47 entity
descriptors, 0x70/0x76 objects (their 0x79 sets) and 0x78 sets, plus DID maps (counts are
references, not distinct targets):

| Target | Referenced by | Count |
|---|---|---|
| 0x33 compositor | `AppearanceUI_CompositorFile` in 0x47 avatar entities | 218 |
| | script node 53 `ImageCompositorFile` (0x07 tables; `script-table.md`) | 39 nodes |
| | PRELOADRESOURCES labels | 7 |
| 0x41 surfaces | compositor `image_did`, `mask_did` | 1,811 + 1,805 |
| 0x21 UI scene | `UICore_Viewport_uiscene` in UI layouts (0x22; `ui-layout.md`) | 20 |
| | `UIViewport_UISceneOverride` in 0x47 | 32 |
| | `MonsterPlay_SessionCreate_UIScene` in 0x79 | 95 |
| | `RaceTable_UIScene` in 0x79 | 8 |
| 0x47 entity descriptors | UI scene descriptors (bit 0x008) | 2,969 |
| 0x40 texture lists | imposter tables | 600 |
| 0x27 imposter table | computed from landblock coordinates (§5.1) | — |
| 0x43 script channels | `ScriptTable_ScriptChannels` in one 0x47; UNIQUEDB | 1 + 1 |
| 0x2D, 0x0E200000, 0x0EBADA55 | UNIQUEDB labels | 3 + 1 + 1 |
| 0x78 sets | §12.2; among them `Monitor_VolumePropertiesToApply` in `Entity_Monitors` structs of 0x47 (a struct member, PS2) | 383 monitor references |
| PropertyMetaData objects | control list | 5,211 |
| ErrorCategory objects | error control | 675 + 1 default |
| TutorialHint objects | tutorial control | 385 |
| MovieLibrary objects | movie control and 3 other properties | 85 + 92 |
| PropertyResponse objects | 5 properties (§13.5) | 3,287 |
| GameCallback objects | script tables and scriptlets (raw DIDs in script nodes) | 6,245 + 737 occurrences |

The registry property `UI_NPC_Default_UIScene` (type-20 code 0x43) is defined but holds no
value in any scanned set.

---

<a id="s16"></a>
## 16. Invariants

Each is Confirmed on all records of the type in this install unless marked BIN (a client
check that a Tessera reader must apply).

- **MR1** (0x33): the record parses exactly with §3.2; trailing byte ∈ {0, 1} (BIN check).
- **MR2** (0x33): layer keys are unique within a record; every `image_did` is non-zero;
  every non-zero `mask_did` names an A8 surface of its option image's size; every option's
  `legacy_did` is non-zero, and a colour entry's `legacy_did` is 0 exactly when its
  `mask_did` is 0 (observations, not reader checks: a reader accepts any value).
- **MR3** (0x33): all layer-0 options have equal size and `x` = `y` = 0 (else composition
  fails, §3.6.1 P4).
- **MR4** (0x21): count + descriptors + 13 trailing bytes consume the record exactly; every
  descriptor decodes with `entity-descriptor.md` §3.1 with no bit ≥ 0x800 set.
- **MR5** (0x27): DID low bytes are multiples of 27; hint 1; entries in bucket order by key
  mod 23; the *i*-th entry's cell is (*i* mod 16, ⌊*i*/16⌋); for *n* ≥ 1 both DIDs are
  non-zero and `cell_u`·*C* = `cell_v`·*R* = 31/32 (§5.4).
- **MR6** (0x2D): record size = 8 + `size`; blob starts with `DXBC` and its own total-size
  field equals `size`.
- **MR7** (0x43): parses exactly with §7.2; every bool8 ∈ {0, 1}.
- **MR8** (0x0E200000, 0x0EBADA55): parse exactly with §9 and §10; taboo hashed-table keys
  equal the header names one to one.
- **MR9** (0x0E010000–2): record length = 44 + the u32 at offset 40.
- **MR10** (0x78): every set decodes with the generic property-set reader.
- **MR11** (§13.1): `PropertyMetaData_Property` values are unique and the control list
  equals the family.
- **MR12** (BIN): a reader of the record types of this spec rejects a hashed-table hint
  above 22 and any count larger than the bytes left. For these types this is an exception
  to `records.md` T1 (§1; §18).
- **MR13** (0x27, I7): in the top-level colour atlas of every non-empty table, the last 4
  texel columns and the last 4 texel rows of each used cell have alpha 0.
- **MR14** (0x0E200000): in every rule set, the class keys are the 13 single bits 1 … 4096;
  every `excluded_ranges` string has exactly 2 units with first ≤ last; `languages` values
  resolve in the Languages mapper.
- **MR15** (0x0EBADA55, TB3): the list table's keys are in non-decreasing order of
  (hash of §10 mod the bucket count of the stored hint).

---

<a id="s17"></a>
## 17. Worked synthetic examples

These byte strings are made up for tests; they are not taken from the install.

<a id="s17-1"></a>
### 17.1 A two-layer compositor (85 bytes)

```
99 00 00 33   02 00 00 00
01 00 00 10   01 00 00 00
   01 00 00 00  01 AA AA 41  01 BB BB 41  00 00 00 00  00 00 00 00  01 00 00 00
      03 00 00 10  02 AA AA 41  02 BB BB 41
02 00 00 10   01 00 00 00
   01 00 00 00  03 AA AA 41  03 BB BB 41  10 00 00 00  20 00 00 00  00 00 00 00
00
```

Decodes as: DID 0x33000099; 2 layers. Layer key 0x10000001 with one option: index 1,
legacy DID 0x41AAAA01 (ignored), image 0x41BBBB01 at (0, 0), one colour entry (colour key
0x10000003, legacy 0x41AAAA02, mask 0x41BBBB02). Layer key 0x10000002 with one option: index
1, image 0x41BBBB03 at (16, 32), no colour entries. Trailing boolean 0. Dependencies:
0x41BBBB01, 0x41BBBB02, 0x41BBBB03 (not the 0x41AAAA… DIDs).

<a id="s17-2"></a>
### 17.2 Composition arithmetic

Base texel *D* = (R 200, G 150, B 100, A 255); colour (0.5, 0.25, 1.0) gives *C* = (127, 63,
255); mask byte 128 gives *k* = 129. Masked multiply:

- R: *X* = (128 × 200) ÷ 256 = 100; 200 − div256((200 − 100) × 129) = 200 − 50 = **150**;
- G: *X* = (64 × 150) ÷ 256 = 37; 150 − div256(113 × 129) = 150 − 56 = **94**;
- B: *X* = (256 × 100) ÷ 256 = 100; 100 − 0 = **100**; A stays 255.

Then an overlay texel *S* = (R 10, G 20, B 30, A 200) mixed at opacity 0.5: *o* =
trunc(127.5) = 127; *a* = (200 × 128) ÷ 256 = 100; *k* = 101:

- R: 150 − div256(140 × 101) = 150 − 55 = **95**;
- G: 94 − div256(74 × 101) = 94 − 29 = **65**;
- B: 100 − div256(70 × 101) = 100 − 27 = **73**; A stays 255.

<a id="s17-3"></a>
### 17.3 An imposter table with two entries (50 bytes)

```
1B 00 01 27   01 02
05 00 00 00  00 00  00 00 80 3F  00 00 00 00
1E 00 00 00  01 00  00 00 20 41  00 00 80 BF
10 00 00 40   11 00 00 40   00 00 F8 3E   00 00 78 3F
```

DID 0x2701001B (region 1, square *X* = 0, *Y* = 27); TSize hint 1, 2 entries. Key 5 (bucket
5 of 23) at cell (0, 0), 1.0, 0.0; key 30 (bucket 7) at cell (1, 0), 10.0, −1.0. Atlases
0x40000010 and 0x40000011. *n* = 2 gives *C* = 2, *R* = 1: `cell_u` = 0.484375 (0x3EF80000),
`cell_v` = 0.96875 (0x3F780000). The second entry's preview rectangle is *u* 0.5–0.984375,
*v* 0–0.96875.

<a id="s17-4"></a>
### 17.4 A script-channel record (32 bytes)

```
00 01   01 00 00 00
   00 01   AA 0A 00 10  02 00 00 00  01
   01   00 00 00 00  AA 0A 00 10  BB 0B 00 10
   01
00
```

One channel (key 1) with one script (key 0x10000AAA, priority 2, interruptible), one
transition (from any, to 0x10000AAA, run 0x10000BBB), exclusive; no global transitions.

---

<a id="s18"></a>
## 18. Corrections and additions to other specs

These are proposals for the owners of those specs; this session edits only this file.

| Spec | Place | Proposed change |
|---|---|---|
| `records.md` | §2.1 rows 0x21, 0x27, 0x2D, 0x33, 0x43, 0x58, 0x0E | Status C with a pointer to this spec (0x58 to `string-table.md` §10.3); 0x27 "layout OPEN" → §5; 0x0E: the NAME_FILTER_TABLE and TABOO_TABLE layouts are §9–§10, compression dictionaries §11 (stored envelope only, gated) |
| `records.md` | §1.6 prologue table | 0x43 and 0x58: "layout OPEN" → this spec §7 and `string-table.md` §10.3; the 0x0E compression dictionaries begin with two 16-byte identifiers (§11) |
| `entity-descriptor.md` | ED1 | Holds for landblock descriptors only; UI scene descriptors (§4.2) lack 0x004 and 0x080 in many cases. A shared reader must not require them. |
| `records.md` | §1.3 invariant T1 ("must not reject other hints") | Narrow T1: hints 13–22 must be accepted; a hint above 22 is rejected wherever the client's reader is known to reject it, which so far is every property set (`property-system.md` §5.3) and the imposter, name-filter and taboo tables (this spec §1, MR12). Until T1 is revised, MR12 and `property-system.md` §5.3 are per-type exceptions to it. |
| `appearance.md` | §3.3 OPEN (0x33 compositor layout) | Answered by §3 of this spec; the `AC_Color_*` → colour-key pairing is `appearance.md` §3.5 itself (§3.7, C10) |
| `distant-landscape.md` | §4.8 "Size … Inferred" | BIN now (I5, §5.2): `value_a` × `scale` is the quad's width and height, `value_b` × `scale` the height of its lower edge above the sprite position; corners as in §5.2 |
| `distant-landscape.md` | §4.8 `distance` OPEN; §5.6 per-sprite flag OPEN | From the sprite vertex program (fourth BIN use): the sprite's `distance` reaches the program, which fades the sprite in over 10 world units starting at `distance` × (second component of the program constant `c_DistantSpriteConfig`) − 25 from the eye, times the far-draw fade; the per-sprite flag computed in §5.6 is the horizontal **mirror** flag of §5.4 here |
| `ui-widgets.md` | §12 OPEN (how a Viewport draws) | Partly answered: camera selection by UISceneEntity key and the scene fog override (§4.1, §4.2 here) |
| `sound.md` | §10.3.4 | The 383 `Monitor_VolumePropertiesToApply` references name 0x78 sets of the families of §12.2 here (156 local sky, 25 ambient music, 145 volume, 56 other, 1 empty set) |

---

<a id="s19"></a>
## 19. Open questions

> **OPEN:** (implementer, 2026-10-09; engineering choices, recorded for review)
> `crates/formats/src/gameplay/misc.rs`: (1) `modifier_value_display` (§13.1) gives no number
> for a value that is not a float, int or int64. (2) `VersionSystemView::member_candidates`
> (§13.7) is the union of the `Version_SingletonList` arrays and `Version_AlwaysRunOnPlayers`
> and leaves the `AccountVersion_SingletonList` entry out (its object is not a member here).
> (3) The typed views of §3.7 and §12.2 read a field of another value type as absent. Confirm
> or replace.

> **OPEN:** script-channel queue modes per caller, the hand-over after a transition script,
> and any use of `global_transitions` (§7.4).

> **OPEN:** which context bit and rule-set key the name-entry code passes, the order of the
> name checks, and the meaning of `flag_b` (§9.3).

> **OPEN:** taboo list walking order, the tokenizer and the variant without rule set, and
> the context bits used for chat and names (§10.1).

> **OPEN (M8):** compression-dictionary body and field meanings (§11); research for M8
> within decision 0034 §2.

> **OPEN:** whether the raw 0x78 DID matches in 0x07, 0x01 and 0x80 records are
> references (§12.2; the 0x70 matches are not, X5).

> **OPEN:** operator semantics of property metadata (the percentage scale is settled, §13.1, X4); needs the
> data-driven code or one owner observation (§13.1).

> **Answered (2026-10-09; was OPEN, implementer notes, `crates/formats/src/gameplay/misc.rs`, 2026-10-07):** choices
> made where the spec is silent; confirm or correct.
> (1) Placement: the composition (§3.6) and the typed views of §12 and §13 (bare-set
> families, metadata classifier and views) live in `crates/formats/src/gameplay/misc.rs`
> next to the decoders, instead of `crates/material` and `crates/props` as "Implements
> into" says. They are pure functions over decoded records and RGBA8 buffers and need
> nothing from those crates; the gameplay record specs share this one `gameplay` module.
> `crates/material` and `crates/props` can wrap them.
> (2) §3.6.1 does not say what happens when layer 0 has no options or the base option's
> image is not loaded (a load failure, not P1); Tessera fails composition
> (`CompositionFailure::NoBaseImage`). For P4, when the first layer-0 option's image is
> not loaded, the first loaded layer-0 image is the size reference.
> (3) "Within 0.0002 of 1.0" (§3.6.2 step 3.3) is taken as inclusive (|c − 1| ≤ 0.0002).
> (4) Images passed to composition are RGBA8 as the surface decoder gives them; an A8
> mask's byte is read from the alpha channel; an image whose buffer does not match its
> size counts as not loaded.
> (5) The value types of `UI_Map_*` fields (§12.2), `AppearanceUI_CompositorLayerName`,
> `AppearanceUI_ControlPropertyName`, `AppearanceUI_MinControlValue` (§3.7) and several
> §13 enum fields are not given; the views accept any integer-like value as a key and any
> numeric value as a number.
> (6) MR2 (repeated layer keys, null DIDs) is reported by `first_unusual_field`, never
> rejected; node 44 addresses the first layer with a repeated key.
> (7) The `PathMap_*` volume signature (§12.2) is any property whose dictionary label
> starts with `PathMap_`; `Version_*` (§13.7) and the property-response members (§13.5
> OPEN) likewise.
> (8) Fuzzing: instead of one fuzz target per prefix, one target
> (`fuzz/fuzz_targets/formats_gameplay_misc.rs`) takes a selector byte and reaches every
> decoder of this spec plus composition, placement and the pixel arithmetic. This is the
> convention of all the gameplay record modules (one target per spec).
> (9) A DXBC container (§6) may have chunk-table entries that share an offset or point
> back into the header; the chunk list is sized from the table, so such a container is
> accepted.
>
> *Answer:* (1) Confirmed: "Implements into" names likely homes, not a requirement; pure
> functions in the gameplay module are fine. (2), (3), (4), (6), (9) Confirmed as Tessera
> decisions: the client's handling of a missing base image, of the tolerance boundary and of
> overlapping chunk entries was not traced, none of these cases occurs in this install, and the
> chosen behaviour is safe for hostile input. (5) The dictionary types (X3): in `UI_Map_GameMap`,
> `UI_Map_MapImage` is a DID (→ 0x41), `UI_Map_Scale` a float, `UI_Map_PixelOffsetX`/`Y` and
> `UI_Map_BlockOffsetX`/`Y` ints, `UI_Map_FogOfWar` a bool (default false) and
> `UI_Map_FogOfWar_Color` a colour; `AppearanceUI_CompositorLayerName` is an enum of mapper
> 0x230000DE, `AppearanceUI_ControlPropertyName` a PID and `AppearanceUI_MinControlValue` an
> int. Every §13 field the tables call "enum" is dictionary type 8 with the mapper given there.
> (7) Confirmed: label-prefix signatures are data-driven and agree with the censuses (§12.2,
> §13.5 now states its rule, §13.7). (8) Confirmed: one target per spec that reaches every
> decoder meets the project's fuzzing rule; it is not a format matter.

---

<a id="s20"></a>
## 20. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review). Layouts of 0x33, 0x21, 0x27, 0x2D, 0x43, 0x0E200000, 0x0EBADA55 Confirmed (BIN + OBS) over all records; compositor composition (BIN); compression-dictionary envelope (stored form only, no BIN, gated); 0x78 family census; metadata families PropertyMetaData, ErrorCategory, TutorialHint, MovieLibrary, PropertyResponse, GameCallback, Version; WEENIECONTENT census; reference graph. |
| 2026-10-07 | Revision after the first review (status stays Review). Re-measured over all 24 compositors (payloads re-read, byte-identical), the surface headers of the overlay options of 0x33000017, and all 7,526 script tables and 28,992 scriptlets for nodes 33/44; second BIN use under decision 0011 for the out-of-range and placement questions. (1) §3.2, §3.4, C3, MR2: 3,737 first DIDs, of which 121 are 0, exactly the colour entries with `mask_did` 0; the 3,616 non-zero values (2,977 distinct) exist in no archive. (2) §3.6.2: the off-canvas options reach row 310, not 307; the client's placement rule (clip, never reject, with its two odd cases) replaces the Tessera rule and closes the clipping OPEN (removed from §19). (3) §3.6.1, §3.6.3, new row C9: opacity and colour conversions defined for every f32 value (low 8 bits of a 32-bit truncation; colour components OR-ed unmasked; NaN skips the layer); node-33 colours are stored as bytes and always 0–1; observed opacities all 1.0. (4) §1, MR12, §18: hint rule scoped as a per-type exception to `records.md` T1, with a proposal to narrow T1. (5) OBS row: 5,954 distinct non-zero compositor DIDs (2,977 with headers, 2,977 absent). BIN row extended with the second use. No other count in §1–§3 changed (layer histogram, options, colour counts, x/y ranges, index exception, formats, masks, flags, key resolution all re-confirmed). |
| 2026-10-07 | Second revision after the first review (status stays Review). Every count disputed by the review re-measured from scratch over all records in a separate session: 3,737 first DIDs, 121 of them 0 (all colour entries with `mask_did` 0, colour key 0x10000007 under layer key 0x10000003), 3,616 non-zero (2,977 distinct), none among the 3,838,957 directory entries of the 30 client archives; off-canvas options: 8, all in 0x33000017 layer 0x10000012, last row 310 (option 5: `y` 219, height 92) of the 256-row canvas, none past column 511, none negative or starting outside; 5,954 distinct non-zero compositor DIDs (2,977 with headers, 2,977 absent, no overlap). The other counts of §1–§3 (layer histogram, 166 layers, 1,811 options, colour counts 307/1,082/422, `x` 0–476, `y` 0–701, the `index` exception, base and overlay formats, 1,805 A8 masks of matching size, flag values, 199 node-44 and 218 node-33 instances with the stated values, 218 `AppearanceUI_CompositorFile` references) re-confirmed; no further correction needed. The BIN facts of C9 (opacity and colour conversions, the opacity skip, the placement rule, the stored-form colour scale) re-read and confirmed (BIN row, third use). Review issues 1–5 stay resolved as in the previous row. |
| 2026-10-08 | Revision answering the OPEN callouts (status Accepted → Revised). Fourth BIN use (decision 0011) and new OBS measurements. (1) §3.7, C10: the `AC_Color_*` → colour-key pairing is referenced from `appearance.md` §3.5 (OPEN closed). (2) §4.1, U3: the UI-scene trailer is a fog override of the 3D view; fields renamed `fog_colour`, `fog_override`, `fog_start`, `fog_end` (formerly `colour`, `flag`, `value_a`, `value_b`; the last two are f32, formerly listed as u32 bit patterns). (3) §4.2, U4: camera chosen by UISceneEntity key equal to the viewport's camera value; element types created; `CameraFOV` range. (4) §5.2, I5: `value_a`/`value_b` are the sprite quad's size and lower-edge height (× sprite scale), with the corner table. (5) §5.3, I6: atlas roles (parameters 3000 and 3011) and the normal channel mapping. (6) §5.4, I7, MR13: margin at the right and bottom (BIN + OBS over 12,406 cells); mirrored sprites; I3's OPEN mark removed. (7) New §7.4 (SC3, SC4): channel arbitration, queueing, transition precedence, stop handling; narrowed OPEN. (8) §9 (NF2, NF3, MR14): every rule-set field named and explained (former names in brackets), new §9.1 character check, §9.2 name check with the vowel set, §9.3 narrowed OPEN; correction: class 16 has no range (12 of 13 classes have one). (9) §10 (TB2–TB4, MR15): header fields renamed `category`/`contexts` (formerly `a`/`b`) and explained, the list-table string hash, new §10.1 matching (PCRE2, caseless, unanchored); narrowed OPEN. (10) §12.2, §15, PS2: referrers found inside `Entity_Monitors` structs (`Monitor_VolumePropertiesToApply`) for all local-sky, ambient-music and volume sets; correction: the 19 `Physics_SurfacePropertiesToApply` references are in 0x78 sets, not 0x79, and 52 more are in 0x47; narrowed OPEN on raw matches. (11) §13.1: OPEN made precise (where the rule is not, what would settle it). (12) §16 MR13–MR15, §18 four new proposal rows, §19 rewritten. |
| 2026-10-08 | Review of the 2026-10-08 revision (separate review session): passed; status Revised → Accepted. No content change. Record in the review log. |
| 2026-10-09 | OPEN sweep (spec writer, dirty room; status Revised, waiting for review). New provenance key OBS3 and rows X1–X3; row U2 corrected. Accepted the implementation's census corrections: §4.2 bit 0x010 is in 3,026 descriptors; §13.5 states the member rule and the 32 callback-only objects (X1); §13.7 and the §13 introduction: the Version family has the `VersionSystem` control, 123 members plus the control (X2). §19 implementer notes (1)–(9) confirmed, with the value types of (5) (X3). Still OPEN: script-channel queue modes, name-entry context and order, taboo walking and tokenizer, compression dictionaries (gated), raw 0x78 DID matches in other records, operator semantics of property metadata. §13.1 percentage display read from the class library (new key BC, row X4); the operator part of that callout stays open. §12.2: the raw 0x78 matches in object records are not references (X5); those in 0x07, 0x01 and 0x80 records stay open. |
| 2026-10-09 | Repair after the review of the OPEN sweep (spec writer, dirty room; status stays Revised, waiting for re-review). Each blocking figure re-measured (OBS3 dump, class-library bytecode) before the change. (1) Row X2, §13.7 and its answer: the 93 number-list structs (92 `Version_SingletonList`, 1 `AccountVersion_SingletonList`), 155 distinct DIDs in the singleton lists covering 121 members, the 2 others in `Version_AlwaysRunOnPlayers`, 158 distinct DIDs in all (123 members, 35 others); the earlier "names all 123 members" was wrong. (2) §13.1 step 1 and row X4: for `Multiply`, *d* stays *v* when *v* = 1 or NaN (a `Multiply` by 1 shows 100, increase template); int and int64 values are unscaled whatever the flag. (3) §13 introduction: table of members, root classes and controls per family; property responses and game callbacks have no control (new row X6). Not blocking: provenance rows ordered X1–X6; "Implements into" says one fuzz target per spec is enough. |
| 2026-10-11 | Policy wording only, after [decision 0034](../decisions/0034-official-server-compatibility.md); no fact changed, nothing re-measured, status unchanged. "Purpose", provenance row CD1, §11 and the OPEN callouts of §11 and §19 on the compression dictionaries no longer say they are behind the M8 gate: the owner set that gate aside on 2026-10-11, and the dictionaries' body and field meanings are now M8 research within decision 0034 §2. The statement that no BIN reading was done for them stays true. |
