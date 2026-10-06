<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# UI layout records (prefix 0x22): element trees, states, media items, inheritance, layout and native panels

| | |
|---|---|
| **Status** | Revised (2026-10-06: applies the accepted decisions 0014 and 0015, see the change log; review pending). Previously Accepted (M5 consolidation revision, reviewed 2026-10-06) |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | **M5 consolidation revision review: Claude review session (spec-writer agent), 2026-10-06: passed; status moved to Accepted.** Every `CLEAN_ROOM.md` §5.3 item holds for the changed material (§5.3, §8.5, §9.2 steps 5 and 8, §9.3, §9.8, §14.6, §15, §16, §17.2–§17.5, U28, U29, U33, U42, U45, UL21). It is functional facts only, with no code, listing structure or addresses; U45's "immediates" describes evidence, not behaviour. The labels are data identifiers under the §17 note, there is no game text, every new fact has a row with a mark, and no gate is touched. B1, B3, B4, B5 and B6 are resolved consistently with `ui-behaviour.md` §4.3, §9.2.1, §9.4 and `skin-definition.md` SK26, SK33, SK34 and D23. The withdrawn Tessera click-through rule no longer appears anywhere. Spot checks (owner's install, read-only, 2026-10-06; fresh extraction byte-identical to the earlier one; reviewer's own resolver): (1) U45/UL21: `containerdisplay` (0x10000052 → 0x22000074) has 11 top-level elements, including `PlayerBag1_Template` … `PlayerBag6_Template` (0x10003A9B, …9D, …9F, …A1, …A3, …A5). Bag 1 is based on `InventoryBag_Template` and bags 2–6 on `PlayerBagOver1_Template`. All six resolve to `UIScaler`, and no element of the 302 layouts is based on any of them: match. (2) §17.2–§17.3 P15: resolved root (599, 164, 197, 183), edges 0, flag byte 0, z 0, `save_loc` only; 61 elements, 2 Text, 7 string refs, 3 fonts, 47 art surfaces, 67 states; game types `UIScaler`, `InventoryDisplay`, `InventoryToolbar` and `BuyNowButton` 1 each: match. (3) The second §17.2 table re-run for all 15 panels: every cell matches. (4) §5.3: `has_position` is 1 on 16,199 elements and 0 on 222, and 221 of the 222 have a zero rectangle: match. (5) §11: Dragbar 135 and ResizeBar 52 elements. BIN (same build): the hit test reads exactly the mouse-visible and blocks-clicks bits (§9.3, U33): match. Non-blocking: P9's string-ref count 51 includes one null (0, 0) reference (50 without), and no other panel has one. Say in the note under the table whether null counts. The checklist `m5-visual.md` P15 rows still need the maintainer's update, as the change log says. **M5 consolidation revision (spec writer, 2026-10-06): submitted for review.** It answers the M5 completeness critique items B1, B3, B4, B5 and B6 for this spec (change log). Earlier records follow. M5-revision re-review: Claude review session (spec-writer agent), review 2026-10-06: **passed; status moved to Accepted**. All three items of the M5-revision review are repaired. (1) §17.2 second table: an independent resolver over the 302 records, replacing a base property by the derived property with the same PID (§7.2 step 3), gives for all 15 panels exactly the String-refs, Fonts and Art-surface cells now in the table (P4 11, P10 4, P12 15 string refs; P2 6 fonts; art surfaces including cursor images: P1 167, P2 13, P3 150, P4 313, P5 572, P6 36, P7 58, P8 106, P9 122, P10 25, P11 12, P12 93, P13 77, P14 16, P15 19); the text-element and fallback-type columns also match again. The note under the table now defines each count and matches how the cells are measured. (2) Art surfaces: the cells changed to include Cursor-item images (§6.5), consistent with the note. (3) §9.6.3 opening: now says the element's opacity is not applied and that with an image the compositor job takes *C*.A as its opacity; consistent with the table and the image paragraph. New fact checked (§17.4 P6 row, U42): the resolved `MainMenu_ButtonField` holds eight Buttons with left, middle and right frame-piece children plus `MainMenu_CloseBox`; `UICore_Element_hide` (PID 0x381) is true on exactly `MainMenu_EndCinematic_Button` and `MainMenu_LeaveMPButton`; the hidden LeaveMP button's stored slot (y 196) lies between the visible Exit (165) and Return (227) buttons, and the hidden EndCinematic button shares its stored rectangle with the visible Options button. The new OPEN is in §17.5 item (6) and §16 item 15. Every §5.3 item passes: only functional facts; no code or listing structure; no copied prose; no game text, keys or hostnames; U42 updated and marked; invariants testable; every unknown has an in-section `> **OPEN:**` callout listed in §16; no closed gate. The spec is implementable for M5. Earlier records follow. M5-revision review: Claude review session (spec-writer agent), review 2026-10-06: **changes requested** (two blocking items, one minor). Blocking 1 (§5.3 "each fact ... marked Confirmed"; U42, §17.2 second table): an independent resolver over the 302 records (§7.2 step 3: a derived property **replaces** the base property with the same PID) contradicts four cells. "String refs (distinct)" is 11, not 12, for P4; 4, not 5, for P10; 15, not 16, for P12. "Fonts (distinct)" is 6, not 8, for P2. The counting probe appended the base and derived property entries instead of replacing them by PID, so it counted values that the resolved panel never uses: P2 `UICore_Text_fonts` and `UICore_Text_tag_fonts` of the base; P4 and P12 base `UICore_Text_entry` values; P10 one base `UICore_Element_tooltip_entry`. Fix: recount every String-refs and Fonts cell by §7.2 step 3. The checklist P2-T row inherits the 8. Blocking 2 (same row): the "Art surfaces" note says the count covers media items of elements and states, animation frames and DID-valued properties. The cells exclude the images of Cursor items (kind 10, §6.5). With them the counts are P1 167, P2 13, P4 313, P5 572, P7 58, P8 106, P9 122, P10 25, P11 12, P12 93, P13 77, P14 16; P3, P6 and P15 are unchanged. Fix: say "excluding cursor images (§6.5)" in the note, or change the cells and the checklist A rows to match. Minor (not blocking): §9.6.3 opens with "the effective opacity of the overlay draw is always 255". The image case then says "the opacity is *o* = *C*.A". Say that the element's opacity is not applied (255) and that the compositor job takes *C*.A as its opacity. Everything else passes. Each §5.3 item: only functional facts; the per-pixel arithmetic is in words, with numeric constants allowed under decision 0012, and has no code or listing structure; no copied prose; no game text, keys or hostnames; `ui/layouts`, `.layout` and the XML node and attribute names are file-format interop identifiers; provenance rows U31–U44 have marks; invariants UL16–UL20 are testable; new unknowns have in-section `> **OPEN:**` callouts and §16 lists them; no closed gate. The spec is implementable once the counts are fixed. Spot checks, all matching: (1) U23 and U31 data side: root DID-map index 4 is `UILAYOUT`; it has 236 entries, 232 of them 0x22; all 100 immediate (key, element) pairs at open-panel call sites name a top-level element of the keyed layout; `UILAYOUT` and `UIElementID` labels are unique. (2) U32: 46 of the 48 top-level elements of `gameplayactivatables` resolve with PID 0x10000F07 true. (3) §5.9: PIDs 0x3CD 232 + 3 (true on 223 elements), 0x695 194 + 1, 0x476 41 + 1, 0x475 1 + 0 (true on 192, 34, 1), 0x10000F07 61 + 0, 0xBD0 1, 0x6E5 1. (4) U42, §17.2 first table and second-table element, text-element, state and fallback-type columns for all 15 panels: key, layout DID, root ID, type, resolved rectangle, edges, flag byte, z-level and save properties. (5) U44: the DrawAfterChildren elements in P6 (8), P7 (45) and P12 (4) are all Buttons. P6 text buttons have left, middle and right frame pieces as children. All §17.3 cells I recomputed by §8.4 match (P1, P2, P3, P6, P11 at all three sizes). §14.6–§14.8 recomputed by hand: every result matches, including the Colorize working and the 481 × 406 saved-location example. UL18–UL20 hold. BIN cross-check of U36: the draw-mode converter maps 1–8 to seven operations plus "none", and 0 and values above 8 to the same operation as draw mode 2. Earlier records follow. M5 revision (2026-10-06): submitted for review. Section numbers of the Accepted text are unchanged; new material is in §9.6.1–§9.6.3, §9.9, §14.6–§14.8, §17 and rows U31–U44. Earlier records follow. Fifth review: Claude review session (spec-writer agent), review 2026-10-06: **passed; status moved to Accepted**. The one fourth-review item is repaired. §4.7 now says "observed lengths ≤ 50 (media paths ≤ 48)". §11 has a Pascal-string census row. The third-review change-log row no longer states a maximum of 47, and a fourth-review change-log row records the correction. Every §5.3 item passes: only functional facts; no code or pseudo-code; no copied prose; no game text, keys or hostnames; every fact has a provenance row and a mark; byte layouts are complete; invariants are testable; all 14 §16 items have an in-section `> **OPEN:**` callout; no closed gate. The spec is implementable. Re-check of the disputed counts: all 302 records were re-extracted read-only from the install on 2026-10-06 (2,610,590 bytes, byte-identical, dictionary 0x34000000 identical). A decoder written independently from §4, §6.1, `property-system.md` §2, §4 and §5 and `records.md` §1 and §4 decodes every record with zero leftover bytes. It counts 11,377 Pascal strings (11,365 Image paths, 2 Movie paths, 10 in property sets), the longest 50 bytes. Twelve strings are longer than 47 (ten Image paths of 48, one property-set string of 49, one of 50). Movie paths are 28 and 47 bytes, 11 Image paths are non-empty, and every length is below 128. Further spot checks, all matched: the §5.1 design-size histogram, with 151 of 3,144 top-level elements outside their design area; the §5.2 ID counts (13,035 distinct, one used 88 times, 69 engine-level; states 9,470 / 3,507; types 6,484 / 1,320); §5.5 read-order numbering in all 302 top-level and 3,774 non-empty child tables; §5.6 5,665 cross-layout and 2,952 same-layout references; the §5.9 entry, PID and value-type census and the top five state properties; the §6.2 Image census and draw modes; the §6.3 Coloured-image census; the §6.4 Animation census (2,252 frames, 2–109 per item, durations, draw modes, loop); the §6.5 hot spots; the §6.6 Fade depths 18 / 16 / 2; the §6.7 Pause pairs; the §6.8 State and Message items; the §11 record sizes, per-record maxima, 13,970 elements without states and the engine widget-type counts. Earlier records follow. Fourth review: Claude review session (spec-writer agent), review 2026-10-06: **changes requested** (one item). All three third-review items are repaired. §9.1 now states only that a panel is created from a (layout DID, top-level element ID) pair and points to the OPEN callouts. U27 no longer claims how the pair is chosen. §6.1 gives the Movie and Image path length as a VLE (REC §1), 1 byte for every observed path. §16 item 9 allows "not at all", matching §3.2. Every other §5.3 item still passes, and the spec is implementable. Remaining issue, which a spot check over all 302 records contradicts (§5.3, "each fact ... marked Confirmed"): §4.7 "Defensive limits" says "Pascal strings: observed lengths ≤ 47". That is false. The records hold 11,377 Pascal strings (11,367 media paths plus 10 strings inside property sets), and the longest is 50 bytes. 12 strings are longer than 47: ten of 48, one of 49 and one of 50. The ten Image paths of 48 bytes are the longest media paths (Movie paths are 28 and 47). The third review record above wrongly repeated the "47 or shorter" figure, and the change log row inherited it. Fix: say "observed lengths ≤ 50 (media paths ≤ 48)" in §4.7, and correct the 2026-10-06 change-log row if it is restated. Every length is still below 128, so the §6.1 statement that the VLE is 1 byte holds and needs no change. Spot checks on the 302 records extracted read-only from the install on 2026-10-06 (2,610,590 bytes) all matched: the §5.8 edge census for all four edges; the §5.4 element and state flag census; §5.3 (118 z-levels, 12,911 zeros; `has_position` 16,199 / 222, of which 209 typed and 12 based have a zero rectangle); the §6.1 per-kind counts and their element / state split (12,998); the §11 media-per-block rows; 2,852 empty states; all state tables at hint 1 and every element table at the minimal hint; the §5.7 default-state table (883 / 237 / 21 / 71 = 1,212, none of the 21 an override child); the §7.1 chain depths 7,076 / 1,494 / 44 with 3 failures; 11,365 Image and 2 Movie items, every one carrying its path. Earlier records follow. Third review: Claude review session (spec-writer agent), review 2026-10-06: **changes requested** (one blocking item). Both earlier issues are repaired: §5.7 now gives the correct default-state census, and §9.1 has an OPEN callout, so every §16 item has an in-section callout. Blocking, §5.3 "unknowns are marked OPEN, not guessed": §9.1, first sentence, says game code creates a panel "usually through the UILAYOUT DID map". That is stated as a BIN fact ("All of §9.1: BIN, U27"), but §3.2 says none of the map's use was traced in the client (U23), and the new §9.1 callout lists "directly, or through a `UILAYOUT` map entry" as unknown. Delete "usually through the UILAYOUT DID map" (or replace it with a pointer to the OPEN), so that §9.1 states only that a panel is created from a (layout DID, top-level element ID) pair. Non-blocking: in §6.1, "(1-byte length prefix)" for the Movie and Image path strings can be read as a u8 length. REC §1 defines a VLE length, and it is 1 byte only for lengths below 128 (all observed paths are 47 bytes or shorter), so say "VLE length; 1 byte for every observed path". Also, §16 item 9 ("how game code finds layouts through the maps") assumes the maps are used, while the §3.2 callout allows "not at all"; align the wording. Everything else passes the §5.3 checklist and is implementable: grammar, limits, merge, layout formulas, states, media player, invariants and the §14 examples. I recomputed the §14.1 offsets (158 bytes) and every §14.2 and §14.3 result, including the 32-bit aspect pass. Spot checks, re-extracted from the install read-only on 2026-10-06: all 302 records, 2,610,590 bytes, byte-identical to the earlier extraction. These match: the §5.7 default-state table (883 / 237 / 21 / 71 = 1,212, by an independent resolver), with none of the 21 an override child; the §5.8 edge census for all four edges; the §5.4 flag census for elements and states; §5.3 (118 z-levels, 12,911 zeros; `has_position` 16,199 / 222, of which 209 typed and 12 based have a zero rectangle); the §6.1 per-kind and element/state media counts (12,998); the §11 media-per-block rows; 2,852 empty states (§9.4); state tables all hint 1 and element tables at the minimal hint (§4.2); the §7.1 chain depths 7,076 / 1,494 / 44, with 3 failures. Earlier records follow. Re-review: Claude review session (spec-writer agent), review 2026-10-05: changes requested. Every earlier issue is repaired, and every §5.3 item passes except one census fact that a spot check over all 302 records contradicts. §5.7 says 258 based elements get their default state only through the base chain, so that it "exists after resolution". In fact it exists after resolution for 237 of them, and for 21 based elements it is defined nowhere, so 92 default states (71 + 21) have no effect, not 71. These spot checks matched: all 302 records were re-extracted from the install, byte-identical; the §7.3 override census (97 + 13, 82 / 28, 8 of 21, 7); U30 (223 / 9, 3 states); the ObjectMode census; the §10 zero and 0x25 breakdown; 5,555 / 13,036; 83,161 instances; 131 Text without a font; `has_position` 222 / 209 / 12; the §5.2 ID counts. Earlier record: Claude review session (spec-writer agent), review 2026-10-05: changes requested (unmarked unknowns need `> **OPEN:**` callouts; provenance rows missing for §3.2, §8.5, §9.1, §9.2, §9.8; §7.3 step 6 base references of overriding/inherited based children; minor: aspect-pass float width, stale §15 rows, §10 0x25 row wording). Grammar and censuses re-verified independently on all 302 records. |
| **Implements into** | `crates/formats` (decoder for prefix 0x22 records with a fuzz target, §4), `crates/ui` (descriptor resolution §7, run-time layout §8, element tree, states and media player §9, drawing of element images §9.6), `apps/explorer` (layout browser: tree, states, media, resolved view), `apps/client` (native panels, M5) |
| **Companion specs** | [`records.md`](records.md) §1 (VLE, TSize and bucket order, Pascal strings), §2 (DID prefixes), §5 (render surfaces 0x41), §7 (enum mappers), §8 (DID maps). [`property-system.md`](property-system.md) §4–§5 (property values and embedded property sets), §8 (root DID map). [`dat-container.md`](dat-container.md) (routing; prefix 0x22 lives in `client_local_<Locale>.dat`). [`fonts.md`](fonts.md) §7–§9 (text layout, outline, font choice; written in parallel). [`string-table.md`](string-table.md) (StringInfo text). [`lua-plugin-api.md`](lua-plugin-api.md) (plugin controls, blend-mode values). [`install-layout.md`](install-layout.md) (install directory, the `raw/` folder). [`skin-definition.md`](skin-definition.md) (user skins, §9.8). [`ui-behaviour.md`](ui-behaviour.md) (run-time behaviour: hit flags §4.3, hover §4.4, message delivery §12.1, UI scale and the displayed box §9.2; called **UB** below). |

---

<a id="s0"></a>
## 0. Purpose

The original client builds its native panels (toolbar, chat, map, character sheet, dialogs,
tooltips, …) from **UI layout records**, DID prefix 0x22. A layout holds a forest of
**elements**. Each element has a design rectangle, a z-level, an anchoring mode for each of its
four edges, a property set, a list of **media items** (images, animations, fades, sounds, …),
a table of named **states** that override its properties and media, and child elements. An
element may instead name a **base element** in the same or another layout and inherit from it.

This spec gives:

- the complete byte layout of the record, decoded over all 302 records of this install with
  zero leftover bytes (§4), and every media-item variant (§6);
- the meaning of every field, with the enum mappers that label element IDs, state IDs,
  element types, draw modes and UI messages (§5);
- how the client resolves base-element inheritance (§7);
- the client's layout rules: design size, the seven edge modes, z-order and sibling order
  (§8);
- how the client turns a layout into a live panel: creation, states, the media player, image
  drawing, the button state machine (§9);
- every reference from layouts to other records, with a resolution census (§10), the full
  census (§11), invariants (§12), the failure categories needed for ROADMAP M5 exit criterion 1
  (§13), worked synthetic examples (§14), corrections to and agreements with other specs (§15) and the open
  questions (§16);
- how game code names and opens panels (§3.2, §9.1), the per-pixel equations of the draw modes
  (§9.6.1–§9.6.3), saved panel locations (§9.9), and a catalogue of 15 native panels that the
  walker can show with sandbox data, for ROADMAP M5 exit criterion 2 (§17; the matching
  checklist is [`m5-visual.md`](../checklists/m5-visual.md)).

Behaviour that belongs to individual game panels (what a quest log does with its list, which
panel opens when) is code in the original client, not data. This spec describes the generic,
data-driven part that every panel shares, and says where game code takes over.

All counts are observations of one install on one date. Tests assert the invariants, not the
counts.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in the user's install on 2026-10-05 with throwaway probes in the dirty-room scratchpad: **all** 302 records with prefix 0x22 (`client_local_English.dat`, 2,610,590 bytes, all stored uncompressed) decoded to the last byte; all 735 enum mappers, all 24 DID maps and the property dictionary 0x34000000 read; every DID referenced from a layout (27,512 non-zero references) checked for existence; the headers of all 5,115 render surfaces named by image items read; the 102 string tables named by layout text checked for the referenced tokens; the 13 file paths named by media items checked under the install directory (read-only). |
| BIN | The client executable of the user's install, `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`, change list 711794, compiled 2026-09-27), disassembled in the dirty room under the standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md), method M4. **Questions:** (a) the exact grammar and field roles of the record (the deserialisers of the layout, element, state and media-item objects, and the factory that creates media items by type); (b) what each media-item kind does when it runs; (c) how a base element is resolved and merged; (d) the edge modes, the design size and the run-time layout of an element; (e) how elements are created, ordered among siblings, switched between states and drawn; (f) the button state choice. **Revision questions (same build, 2026-10-05):** (g) which base reference survives when a derived child overrides an inherited child, and when that child's own base is resolved (the element merge, the resolver and the creation routine); (h) the float width of the aspect pass and the exact bounds of the keep-inside clamp (the run-time layout routine); (i) the screen root's creation; (j) whether the client loads PID 0x3CD (`UICore_Element_DrawAfterChildren`) as an immediate operand. **M5 revision questions (same build, 2026-10-06):** (k) how game code names the layout and element of a panel it opens (the framework's open-panel entry, the element manager's create-by-key entry and their callers); (l) what the per-element table consulted at creation (§9.2 step 5) holds; (m) the drawing traversal of an element and its children, and who reads PID 0x3CD (the element's property-change handler and draw routine); (n) how draw modes are converted and which per-pixel routine each one selects (the draw-mode converter, the element image draw, the 2D compositor and its routine tables); (o) how saved panel locations are stored, converted and applied (the apply routine, its two callers and the layout-file reader and writer); (p) which top-level elements of the activatables layout become panels. Only the routines named here and their direct callees were read. **Why M1–M3 were not enough:** the data names panels and states but not which code opens them; draw-mode names do not define arithmetic; saved locations are not in the install; the original client cannot be observed in this session. **Why M1–M3 were not enough (first questions):** the data gives values but not their meaning; the only published reader (LC) gives no semantics and misreads several fields (§15); no public description of the layout engine exists; the original client cannot be observed in this session (M3), and observation could not give the layout formulas anyway. Only offline UI code was read: no network, login, launcher, patcher or anti-tamper code. **Consolidation questions (same build, 2026-10-06):** (q) how the original opens its bag windows (the call sites of the open-panel entry with the `containerdisplay` key and the routine that constructs the bag windows); (r) what the hit test reads and how the listener hook relates to it (answered in UB §4.3, BIN; this spec now refers there). **Why M1–M3 were not enough:** the data does not say which code opens a layout or which flags code writes. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.4 (in the `lotro-tools` distribution; Apache-2.0 per its distribution), class `UILayoutLoader`, read as Java bytecode for facts: field order, the enum-mapper DIDs it uses, its media-type numbers. |
| ER | `research/notes/engine-client-re.md` §6 (element IDs come from enum 0x230000D1; user skins). |
| AC | Asheron's Call analogue (ACE `LayoutDesc`/`ElementDesc` readers, the DatReaderWriter schema, a published client structure listing). Vocabulary only: the same engine family has layout, element and state descriptors, edge fields and incorporation flags. Marked **Analogue**. |
| PS | [`property-system.md`](property-system.md) (embedded property sets, value encodings, root DID map). |
| REC | [`records.md`](records.md) (VLE, TSize, strings, enum mappers, DID maps, render surfaces). |
| FO | [`fonts.md`](fonts.md) (font choice, text drawing). |
| DEC | Accepted Tessera decisions [0014](../decisions/0014-ui-input-bridge.md) (UI input bridge, **IB**) and [0015](../decisions/0015-ui-rendering.md) (UI rendering, **UR**). They fix **Tessera's** behaviour where the original's is unknown or where Tessera departs from it; they are not sources of facts about the original. Statements taken from them are marked "Tessera rule" with the decision and its section. |

Marks: **Confirmed** = checked over every record of the kind (OBS), or a BIN fact the data
agrees with on every record; **BIN** = rests on the disassembly only (no data can show it);
**Probable** = supported by data or BIN but not proven in every case; **Reported** =
third-party only; **Analogue** = vocabulary from the related engine.

| # | Fact | Sources | Mark |
|---|---|---|---|
| U1 | Record header: DID, design width, design height, element table (§4.1) | BIN (layout deserialiser); OBS (302 of 302) | Confirmed |
| U2 | Keyed tables (TSize + entries of key + body) for elements, children and states (§4.2) | BIN (table serialiser); OBS (all tables in bucket order, keys unique) | Confirmed |
| U3 | State block shared by elements and states: state ID, flag byte, five i32, bool8, property set, media list (§4.3) | BIN; OBS (16,421 element blocks and 12,977 state blocks, zero leftover) | Confirmed |
| U4 | Element tail: read order, element ID, type, base element, base layout, default state, four edge modes, states, children (§4.4) | BIN; OBS; LC (same order) | Confirmed |
| U5 | Media list count is a VLE; each item is a u32 kind tag followed by the item's own body, which repeats the kind (§4.6) | BIN (list serialiser, item factory); OBS (12,998 items) | Confirmed |
| U6 | Media item bodies for kinds 1, 3–13; kind 2 and kinds above 13 are not created (§6) | BIN (factory and the twelve item serialisers); OBS (kinds 1, 4, 5, 6, 8, 9, 10, 11, 12, 13 present and exact) | Confirmed (kinds 3 and 7: BIN only) |
| U7 | The u32 after the rectangle is the **z-level**, a signed integer; LC skips it and the research notes call it a colour (§5.3) | BIN; OBS (118 distinct values, 12,911 zeros, no colour pattern) | Confirmed |
| U8 | Flag byte: bit 0 = pass state changes to children, bit 1 = keep inside the parent (§5.4) | BIN (serialiser; state switch; run-time layout clamp) | BIN |
| U9 | Element-ID, state-ID, element-type, draw-mode and UI-message enums (§5.2) | LC (first three DIDs); BIN (draw-mode mapping); OBS (all values resolve; EMAPPER labels) | Confirmed |
| U10 | Edge modes 0–6 and the design-time and run-time layout formulas (§8) | BIN | BIN (OBS: value census) |
| U11 | Design size = record header width × height, used for panels placed on the screen (§8.3) | BIN | BIN |
| U12 | Base-element resolution and merge rules (§7) | BIN; OBS (8,614 of 8,617 references resolve, depth ≤ 3, no cycles) | BIN (rules), Confirmed (census) |
| U13 | Sibling order: z-level descending, then read order ascending; lower z is in front (§9.3) | BIN (insertion comparator, hit test; M5 revision: the child-drawing routine walks the list from its start); OBS (backgrounds carry positive z, title bars, close buttons and tooltips negative z) | BIN (list order, hit test, drawing order) |
| U14 | State switching, property layering and media-list replacement (§9.4) | BIN | BIN |
| U15 | Media player: sequential items, waiting items, result codes (§9.5) | BIN | BIN |
| U16 | Element images: two layers, drawn at native size, tiled over the element rectangle and clipped (§9.6); the image opacity reaches the base layer only (M5 revision) | BIN (draw routine; M5: the image-layer draw passes the opacity only with the base layer) ; OBS (element and image sizes) | BIN |
| U17 | Button state choice (§9.7) | BIN | BIN |
| U18 | All references from layouts resolve; target record types (§10) | OBS | Confirmed |
| U19 | Media colour is u32 0xAARRGGBB (bytes B, G, R, A) (§6.9) | BIN (colour serialiser); OBS (alpha byte patterns) | Confirmed |
| U20 | Design-size, element, state, media and property census (§11) | OBS | Confirmed |
| U21 | Layouts are stored only in the locale archive under prefix 0x22 (§3.1) | OBS (302 records in `client_local_English.dat`, none in `client_general.dat`) | Confirmed |
| U22 | The client loads a layout as DB-object type 0x28 (§3.1) | BIN (the resolver requests each base layout with type code 0x28) | BIN |
| U23 | Contents of the `UILAYOUT` and `PRELOADRESOURCES` DID maps, and which layouts each kind of reference reaches (§3.2) | OBS (both maps read in full; every layout checked against the maps, base references and layout-valued properties); BIN (M5: game code names layouts by `UILAYOUT` entry key, U31) | Confirmed (contents); BIN (use of `UILAYOUT`); `PRELOADRESOURCES` OPEN (§3.2) |
| U24 | Override children: the inherited child's type and base reference survive, the derived child's are ignored, and the merged child's own base is resolved only when the child is created (§7.3 step 6) | BIN (element merge, resolver, creation routine); OBS (110 override sites, census in §7.3) | BIN (rule), Confirmed (census) |
| U25 | Aspect pass in 32-bit floats; keep-inside clamp bounds are the parent's width and height, not width − 1 and height − 1 (§8.4 steps 3 and 5) | BIN (run-time layout routine) | BIN |
| U26 | Screen root: element ID 0x13, type Field, edges (5, 5, 5, 5), design size = screen size or 1024 × 768 (§8.5) | BIN (screen-root creation routine) | BIN; the resize trigger is OPEN (§8.5) |
| U27 | Root panels, modal-dialog container and the dialog override properties (§9.1); the three ways game code creates panels and elements (M5 revision) | BIN (panel creation, dialog framework, the open-panel and create-by-key entries) | BIN |
| U28 | Creation order of an element tree: resolve, user skin, factory, registration, children in table order with post-creation before attachment, post-creation of a top-level element after attachment (§9.2); step 5 is the mouse-listener check, not saved data (M5 revision); post-creation ends by recomputing the mouse-visible flag (consolidation) | BIN (creation routine and its two callers; M5: the listener registry and its registration routine; consolidation: the post-creation routine) | BIN |
| U29 | User-skin hooks: geometry override between resolution and creation, applied to a per-build resolved copy, writing the rectangle, the four edge modes (0 unless protected) and the scaler origin but not `has_position`; image substitution by the base-image setter (§9.8) | BIN (creation callers apply the active skin to the resolved descriptor; the merge honours a skin-override marker and a per-field mask); SK33, SK34, SK38 (two independent readings) | BIN |
| U30 | `UICore_Element_DrawAfterChildren` (PID 0x3CD) is set on 232 elements and 3 states. The client reaches it through a subtraction chain in the element's property-change handler (so no instruction holds 0x3CD as an immediate); when true the element draws its children first and its own content after them (§9.6) | OBS (census); BIN (property-change handler, element draw routine; M5 revision) | Confirmed (census); BIN (effect) |
| U31 | Game code names a panel's layout by its `UILAYOUT` DID-map entry key, which the client turns into a layout DID through root DID-map index 4; panels are opened by (entry key, top-level element ID) (§3.2, §9.1) | BIN (open-panel entry, element-manager create-by-key entry, the root-map lookup and its 167 callers, 21 of which use index 4); OBS (index 4 is `UILAYOUT` in the root map; all 114 call sites that pass both values as immediates (100 to the open-panel entry, 14 to a window-class helper that calls it) name a top-level element of the keyed layout) | BIN (mechanism), Confirmed (pairs resolve) |
| U32 | The activatables layout: game code creates, as root panels, every top-level element whose resolved `UI_Element_CreateAsRootElement` is true, skipping those whose `UI_HideWhenField_HideWhenMonsterPlayer` is true while a game condition holds (Probable: monster play), and records `UI_Element_IsActivatablePanel` and `UI_Element_IsContextualPanel` for each (§9.1) | BIN (the loop over the layout's top-level table); OBS (46 of the 48 top-level elements of that layout carry the property after resolution) | BIN (rule), Confirmed (census) |
| U33 | The per-element table of §9.2 step 5 is the UI manager's registry of game-code message listeners, keyed by element ID then by UI message; keys 0x0E, 0x18, 0x1E, 0x32 are the mouse-button messages, and the hook sets the element's **mouse-visible** flag (UB §4.3) through the same setter that the type rules use; the hit test reads mouse-visible and blocks clicks (§9.2, §9.3); the registry's handlers are called at the top of a message's walk (UB §12.1) | BIN (registry lookup in the creation routine, the registration routine and its 33 callers, the hook, the hit test; consolidation: the flag setters, the top-of-chain handler); OBS (labels of the four keys in `UIElementMessage`) | BIN |
| U34 | Drawing traversal: an element draws only when visible, clipped to its box; children are drawn from the start of the child list to its end, each clipped to its own rectangle within the parent's clip; DrawAfterChildren reverses the element's own content and its children (§9.3, §9.6) | BIN (element draw routine, child-drawing routine, property-change handler) | BIN |
| U35 | `UICore_Element_clip_to_parent` (PID 0xBD0) is read per child by the child-drawing routine and selects which rectangle of the child is clipped (§9.3) | BIN; OBS (set on 1 element) | BIN (read); effect OPEN |
| U36 | Draw-mode conversion: draw modes 1–8 map to the compositor operations of §9.6.1; other values map to the Normal operation | BIN (converter called wherever an Image, Animation or Coloured-image item sets a layer) | BIN |
| U37 | Per-pixel equations of the compositor operations for 32-bit ARGB sources and destinations (§9.6.2, §9.6.3) | BIN (the compositor's static routine tables and the routines for 32-bit ARGB) | BIN |
| U38 | Base-layer Grayscale draws nothing (no routine); Grayscale occurs only on Coloured-image items in data (§9.6.1) | BIN (empty table entries); OBS (§6.2, §6.3 census) | BIN, Confirmed (census) |
| U39 | A Coloured-image item with an image draws the image's alpha filled with the item's colour, with the colour's alpha as opacity (§9.6.3) | BIN (compositor job set-up and the overlay routines) | BIN |
| U40 | Saved panel locations: `UICore_Element_save_loc`, `UICore_Element_save_size`, `UICore_Element_save_visibility` select what a saved entry restores; entries are keyed by layout identifier and element ID and hold two corners in normalized screen coordinates plus a visibility byte; the conversion to pixels and the order of application (§9.9) | BIN (property-change handler, apply routine and its two callers, normalized-to-pixel conversion) | BIN |
| U41 | The client's UI-layout file serialisation: `Layout` elements with an `ID` attribute (`0x%08X`), `Element` children with `ID`, `X`, `Y`, `Width`, `Height`; coordinates are screen fractions with the origin at the top left; folder `ui/layouts`, extension `.layout` (§9.9) | BIN (writer and reader of the XML form; strings in the executable) | BIN |
| U42 | The 15 panels of §17: layout, root element, how the client opens each, geometry at three screen sizes, element, text, state, string, font and art counts, the main-menu buttons hidden by the layout (P15 replaced by the `containerdisplay` bag window in the consolidation revision, its row re-measured 2026-10-06 with the same resolver) | OBS (layouts resolved by §7 over all their elements, 2026-10-06; string, font and art counts recounted 2026-10-06 with property replacement by PID, §7.2 step 3, and with cursor images); BIN (open path per panel, U31–U32) | Confirmed (data), BIN (open path) |
| U43 | The UI's 2D compositor runs on the CPU over the panel's drawing surface (the routines are integer per-pixel loops over 32-bit pixels) | BIN | BIN; how panel surfaces reach the screen is OPEN (§9.6.1) |
| U44 | In `DrawAfterChildren` buttons of the main menu, options and plugin manager panels the button draws its own label text and its frame pieces are children, so the label lands on top of the frame (§9.6) | OBS (element trees of §17 panels P6, P7, P12) | Confirmed (data agreement) |
| U45 | The original's bag windows are the six top-level elements `PlayerBag1_Template` … `PlayerBag6_Template` of the `containerdisplay` layout: one game routine constructs six bag windows, passing the six element IDs as immediates, and each bag window opens the `containerdisplay` key with its element ID through the open-panel entry; no call site opens `backpack` (§17.2 P15) | BIN (the open-panel call site with the `containerdisplay` key, the bag-window constructor and its six callers); OBS (element labels, resolved trees) | BIN (open path), Confirmed (data) |
| U46 | Per panel of §17, the non-root elements whose resolved `UICore_Element_ObjectMode` is set, by mode, how deep they nest and how many the layout hides; no state block of these panels sets the property (§17.2, UL22) | OBS (2026-10-06: every element instance of the 15 instantiated panels, resolved by §7 with property replacement by PID; the per-panel element counts equal those of §17.2) | Confirmed |
| U47 | (a) In the 15 instantiated panels of §17, 159 children without an object mode of their own sit under a non-root element in Stretch mode; steps 2–6 of §8.4 at the parent's design size give back the design box for 158 of them; the exception is P10's `Panel_Label_Title` (295 × 20 at x = 0, edges (1, 5, 1, 5)) inside the 350 pixels wide `Panel_TitleBar` (§8.4 OPEN (scope)). (b) The portraits: `PlayerPortrait` (P4) has object mode 3, no media and no children; `OpponentPortrait` (P5) has object mode 1 and one Image item with draw mode 2, and its child `Target_Image` (no media) has mode 1 (§17.4) | OBS (2026-10-06: the same instantiated panels as U46, read-only probes; (a) counts every such child, with the edge rules of §8.4 step 2) | Confirmed |

---

<a id="s2"></a>
## 2. Concepts

| Term | Meaning |
|---|---|
| **Layout** | One 0x22 record: a design size and a keyed table of **top-level elements**. |
| **Element descriptor** | The stored description of one element (§4.3 + §4.4). It holds either a **type** (a widget class) or a **base reference** (element ID + layout DID), never both and never neither. |
| **State descriptor** | The block of §4.3. An element descriptor begins with one (its *own* block, state ID 0); each named **state** of an element is another one. |
| **Media item** | One entry of a state block's media list (§6): set an image, play an animation, fade, play a sound, wait, switch state, … |
| **Resolved descriptor** | The element descriptor after its base chain has been merged in (§7). Only resolved descriptors are instantiated. |
| **Element** (run time) | A live widget created from a resolved descriptor, with a parent, a current box, a current state, run-time properties and a running media list (§9). |
| **Design rectangle** | x, y, width, height of the descriptor, in pixels, relative to the parent's top-left corner (top-level elements: relative to the layout's design area). |
| **Box** | An inclusive rectangle (left, top, right, bottom); width = right − left + 1. The client works in boxes internally; §8 uses them. |

---

<a id="s3"></a>
## 3. Finding layouts and their labels

<a id="s3-1"></a>
### 3.1 Records

- Prefix 0x22, stored only in `client_local_<Locale>.dat` (302 records in the English
  archive, none in `client_general.dat`; REC §2, X1–X2). Every record in this install is stored
  uncompressed; readers must still accept compressed storage (`dat-container.md`).
- DB-object type code 0x28 (`property-system.md` §9, DID range 0x22000000–0x22FFFFFF); the
  client requests layouts with this code, for example when it loads the layout named by a base
  reference (§7.1) (BIN, U22).

<a id="s3-2"></a>
### 3.2 DID maps

- The root DID map label `UILAYOUT` (REC §8.2) names a DID map whose entries give stable keys
  and labels to layouts. In this install it has 236 entries: 232 name 0x22 records, the rest
  other types. It is the only index of layouts in the data, and the client loads layouts by DID.
- The root label `PRELOADRESOURCES` names a map with 9 layouts among its entries.
- Of the 302 layouts, 232 are in the UILAYOUT map, 177 are named as base layouts by other
  elements (§7), 72 are named by layout-valued properties (§10); one is reached by none of
  these.

Implementations read these maps at run time and must not hard-code the DIDs.

**How game code finds a layout (BIN, U31).** Game code names the layout of a panel by the
**entry key** of its `UILAYOUT` map entry (keys are 0x1000xxxx values; the map's labels, such as
`toolbar` or `chatwindow`, name the same entries). The client turns the key into a layout DID
with its general DID-map lookup, called with the key and the root-map index of `UILAYOUT`
(index 4 in the root map, REC §8.2), and then loads the record as DB-object type 0x28. A key
that the map does not hold gives no layout and no panel. The 232 entries that name layouts therefore act
as the client's panel-to-layout table; the 70 layouts outside the map are reached only through base
references and layout-valued properties (templates, tooltips, list-box entries).

Tessera resolves a panel by the map **label** of its entry (or by the key read from the same
entry), then the DID from the map, at run time. The labels and keys in §17 are observations of
this install.

> **OPEN:** (1) whether the client loads the 9 layouts of `PRELOADRESOURCES` ahead of use or the
> map serves another purpose (not traced); (2) how the one layout reached by none of the three
> reference kinds above is opened (no caller found). Neither affects §17.

<a id="s3-3"></a>
### 3.3 Enum mappers

The labels of the ID fields come from enum mappers (REC §7). Find each through the root map
label `EMAPPER` and its own label (labels are interoperability identifiers read from the
install):

| EMAPPER label | DID in this install | Base mapper (engine level) | Own labels | Used for |
|---|---|---|---|---|
| `UIElementID` | 0x230000D1 | 0x2300013B (44 labels incl. 0 `Undef`, keys up to 0x33) | 19,893 (keys 0x10000001–0x10004DB8) | Element IDs, table keys, base-element IDs, element-valued properties |
| `UIStateID` | 0x230000D2 | 0x2300017B (22 labels incl. `Undef`, keys up to 0x15) | 1,137 | State IDs, default states, State media items, state-valued properties |
| `UIElementType` | 0x230000CB | 0x230001BC (20 labels incl. `Undef`, keys up to 0x13) | 533 | Element types |
| `DrawModeType` | 0x230000CE | none | 9 (keys 0–8) | Draw modes of Image, Animation and Coloured-image items |
| `UIElementMessage` | 0x230000D0 | 0x230001AC (77 labels incl. `Undef`, keys up to 0x52) | 22 | Message media items, messages the client sends (§9.5) |

Keys below 0x10000000 are engine-level (the base mappers); keys 0x1000xxxx are game-level.
All 16,421 element IDs, 12,977 state IDs, 7,804 types, 1,212 default states, 8,617 base
element IDs, 11,919 draw modes (Image, Coloured image and Animation items), 86 State-item
state IDs and 31 message IDs in the layouts resolve to a label (OBS).

Engine-level widget types (base mapper, keys 1–19): GroupBox, ColorPicker, Scrollable, Menu,
Scrollbar, ComboBox, ListBox, Panel, Button, Tree, Viewport, Browser, Frame, Field, Dragbar,
Text, Meter, ResizeBar, Flash. Engine-level states include `normal`, `normal_rollover`,
`normal_pressed`, `highlight`, `highlight_rollover`, `highlight_pressed`, `ghosted`,
`ghosted_rollover`, `ghosted_pressed`, `ghosted_highlight` (keys in §9.7).

Draw modes (keys): 1 Colorize, 2 Normal, 3 Multiply, 4 Alphablend, 5 Overlay, 6 Grayscale,
7 Screen, 8 None, 0 Undef. The same numbers are the plugin API's `BlendMode` values
([`lua-plugin-api.md`](lua-plugin-api.md) Appendix A).

---

<a id="s4"></a>
## 4. Record layout (Confirmed, 302 of 302, zero leftover bytes)

All integers are little-endian. "VLE", "TSize" and "Pascal string" are as in REC §1.
`bool8` is one byte that must be 0 or 1: the client's reader rejects values ≥ 2 (BIN), and all
values in the data are 0 or 1. There is no padding anywhere.

<a id="s4-1"></a>
### 4.1 Record

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | did | u32 | The record's DID (0x22…) |
| 2 | design_width | u32 | Width in pixels of the design area of the top-level elements (§8.3) |
| 3 | design_height | u32 | Height in pixels of the design area |
| 4 | elements | keyed table of element entries (§4.2) | Top-level elements |
| 5 | *(end)* | | The payload ends exactly here |

<a id="s4-2"></a>
### 4.2 Keyed tables

Three tables use the same form: the top-level element table, each element's **children** and
each element's **states**.

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | hint | u8 | Bucket-count index (REC §1.3) |
| 2 | count *n* | VLE | Number of entries |
| 3 | entries | *n* × (u32 key, body) | Element tables: key = element ID, body = element descriptor (§4.3 then §4.4). State table: key = state ID, body = state descriptor (§4.3). |

Entries are stored in bucket order of their keys (REC §1.3.1) and keys are unique within a
table (Confirmed: all 302 top-level tables, 16,421 child tables, 16,421 state tables). The
order carries no meaning; the **read order** field (§5.5) gives the authored sibling order.

Observed hints (OBS): element tables use the smallest hint *h* with *n* ≤ 2·*B*[*h*]
(*B* = 11, 23, 47, 89, 191, 383, …): hints 0–5 for top-level tables (*n* up to 450), 0–4 for
child tables (*n* up to 339). The embedded property sets of element blocks follow the same
rule (all hint 0 except one set of 23 entries with hint 1). **State tables always use hint 1**,
for every *n* from 0 to 139.

<a id="s4-3"></a>
### 4.3 State descriptor block

This block starts every element descriptor and is the whole body of a state entry.

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | state_id | u32 | 0 in an element's own block (all 16,421); equal to the table key in a state entry (all 12,977) |
| 2 | flags | u8 | Bit 0: pass state changes to children. Bit 1: keep inside the parent. Bits 2–7: not read by the client (BIN); always 0 in data. §5.4 |
| 3 | x | i32 | Design rectangle left, pixels relative to the parent |
| 4 | y | i32 | Design rectangle top |
| 5 | width | i32 | Design width in pixels |
| 6 | height | i32 | Design height in pixels |
| 7 | z_level | i32 | Sibling stacking key (§5.3, §9.3) |
| 8 | has_position | bool8 | 1 when the descriptor specifies its position and size (§5.3) |
| 9 | properties | embedded property set: TSize + entries with PID echo (PS §5.1 without the DID) | Properties of the element or the state |
| 10 | media_count *m* | VLE | Number of media items. Observed 0–5 in element blocks, 0–10 in state blocks |
| 11 | media | *m* × media item (§4.6) | Media list, in execution order |

In a state entry, fields 3–8 are always zero (all 12,977); the client's text-format loader
forbids states that override position (BIN), and the binary state switch never reads them
(§9.4).

<a id="s4-4"></a>
### 4.4 Element descriptor tail (follows §4.3)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 12 | read_order | u32 | Authored position among siblings (§5.5) |
| 13 | element_id | u32 | Element ID (UIElementID). Equal to the table key in all 16,421 |
| 14 | type | u32 | Widget type (UIElementType); 0 when the element has a base reference |
| 15 | base_element | u32 | Element ID of the base element; 0 when none |
| 16 | base_layout | u32 | DID of the layout holding the base element (0x22…); 0 when none |
| 17 | default_state | u32 | State entered after creation (UIStateID); 0 = none (§5.7) |
| 18 | edge_left | u32 | Anchoring mode of the left edge (§5.8, §8) |
| 19 | edge_top | u32 | Anchoring mode of the top edge |
| 20 | edge_right | u32 | Anchoring mode of the right edge |
| 21 | edge_bottom | u32 | Anchoring mode of the bottom edge |
| 22 | states | keyed table (§4.2) of state entries | Named states |
| 23 | children | keyed table (§4.2) of element entries | Child elements |

Exactly one of `type` and (`base_element`, `base_layout`) is set in every element (7,804
typed, 8,617 based; never both, never neither). `base_element` ≠ 0 exactly when
`base_layout` ≠ 0.

<a id="s4-5"></a>
### 4.5 State entry

A state entry is the key (u32 state ID) followed by a state descriptor block (§4.3) whose
`state_id` equals the key.

<a id="s4-6"></a>
### 4.6 Media item

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | kind_tag | u32 | Media kind (§6). The client creates the item object from this value |
| 2 | kind | u32 | The same kind again, read by the item's own deserialiser. Equal to `kind_tag` in all 12,998 items |
| 3 | body | per kind (§6.1) | |

The client's factory creates objects for kinds 1 and 3–13 only. For kind 2 and for any other
value it creates nothing and skips the item's deserialiser, so the bytes of the body would be
read as the next item (BIN). Readers must therefore treat an unknown kind as a decoding
failure of the whole record. No unknown kind occurs in this install.

<a id="s4-7"></a>
### 4.7 Decoding in words, and limits for hostile input

1. Read the DID, the design width and height, then the top-level table header.
2. For each table entry, read the key, then the element descriptor: the state block (§4.3),
   the tail (§4.4), the state table (each entry: key, state block), then the child table
   (each entry: key, element descriptor, recursively).
3. For each media list, read the VLE count, then each item: the kind tag, the repeated kind,
   then the body selected by the kind tag (§6.1).
4. After the last top-level element the payload must be exhausted.

Defensive limits (suggestions; data never comes close):

- Element nesting: observed depth ≤ 7 below a top-level element; a limit of 64 is safe.
- Table counts: a state block is at least 29 bytes (4 + 1 + 20 + 1 + 2 + 1), an element
  descriptor at least 73 (block + 40 + two 2-byte table headers); so an element entry needs
  at least 77 bytes and a state entry at least 33. Bound counts by remaining bytes ÷ these
  sizes.
- Media counts: each item needs at least 12 bytes; animation frame counts at most remaining ÷ 4.
- Property sets: PS §5.3 limits.
- Pascal strings: observed lengths ≤ 50 (media paths ≤ 48); reject lengths above the remaining bytes.

---

<a id="s5"></a>
## 5. Field meanings

<a id="s5-1"></a>
### 5.1 Design size (Confirmed values; use BIN)

`design_width` × `design_height` is the size of the area in which the top-level elements of
the layout were designed. When the client places a top-level element of the layout directly
on the screen, it lays it out from this area to the current screen size (§8.3). Observed:
1024 × 768 (283 layouts), 1440 × 900 (8), 2560 × 1080 (5), 1280 × 960 (2), 1280 × 1024 (1),
1280 × 1600 (1), 127 × 127 (1), 0 × 0 (1). 151 of the 3,144 top-level elements extend outside
their layout's design area (negative position or beyond the size), which is legal.

<a id="s5-2"></a>
### 5.2 IDs

- **Element IDs** label elements (§3.3). They are unique only among siblings (table keys).
  13,035 distinct IDs occur on 16,421 elements; one ID is used by 88 elements. Engine-level IDs
  (below 0x10000000) appear on 69 elements: parts that engine widgets look up by ID inside
  their own subtree (dialog buttons and fields, context-menu parts, the scroll-bar `widget`).
- **State IDs** label states. 9,470 of the 12,977 states use engine-level IDs; 3,507 use
  game-level IDs.
- **Types** select the widget class that the client instantiates (§9.2): 6,484 typed elements
  use engine types, 1,320 use game types.

<a id="s5-3"></a>
### 5.3 Rectangle, z-level and `has_position`

- The rectangle is in pixels, relative to the parent element's top-left corner. Width and
  height were never negative.
- `z_level` is a **signed** sibling stacking key. Lower values are in front (§9.3). Observed:
  118 distinct values, 0 on 12,911 elements, the next most common −5, −1, 10, 1, −10, 1126,
  −500, −2, −1500. Typical use: backgrounds and frames positive, title bars, close buttons,
  overlays and tooltips negative.
- `has_position` (bool8) is 1 on 16,199 elements and 0 on 222. Of the 222, 221 have a zero
  rectangle (209 typed, 12 based); they are positioned by code. The client copies the flag
  during inheritance (§7). A user-skin override does **not** change it: the override writes
  only the rectangle, the edge modes and the scaler origin (§9.8; `skin-definition.md` §8.4,
  SK33, two independent BIN readings). In state blocks it is always 0.

> **OPEN:** whether anything reads `has_position` after creation. The descriptor loader and
> the merge of §7.2 are the only uses found; the run-time layout of §8.4 and the user-skin
> override do not read it, and the listener check of §9.2 step 5 reads only the element ID.
> Readers in widget classes and in the saved-location code of §9.9 were not searched. Until
> answered, Tessera treats the flag as informational and lays out elements with
> `has_position` = 0 like any other.

<a id="s5-4"></a>
### 5.4 Flag byte (BIN)

| Bit | Name used here | Effect |
|---|---|---|
| 0 | pass state to children | When the element switches to a state, the switch is passed to its children (§9.4). The bit on a state applies to switches into that state; the bit on the element's own block applies when the requested state is not defined (the element then goes to state 0 and passes 0 down). |
| 1 | keep inside parent | After the run-time layout, the element's box is clamped into its parent (§8.4 step 5). Only the element's own block is read for this; the bit on a state has no effect. |

Census: elements 0 (13,955), 1 (2,435), 2 (15), 3 (16); states 0 (6,900), 1 (6,041), 2 (5),
3 (31).

<a id="s5-5"></a>
### 5.5 Read order (Confirmed numbering; use BIN)

In every record the top-level elements are numbered 0 … *n* − 1 and the children of every
element 1 … *n*, each number used once among siblings (all 302 top-level tables, all 3,774
non-empty child tables). The number is the authored order of the siblings. The client uses it
to order siblings of equal z-level (§9.3), and offsets it when inherited and new children are
combined (§7.3 step 7).

<a id="s5-6"></a>
### 5.6 Base reference

`base_element` and `base_layout` name a **top-level** element of a layout (possibly the same
layout) from which this element inherits (§7). 5,665 references point into another layout and
2,952 into the same one.

<a id="s5-7"></a>
### 5.7 Default state

After an element is created, the client starts its own media list and then switches it to
`default_state` (§9.2 step 8). 1,212 elements set it (a non-zero value). Counted over all 302
records, each element taken as stored and, when it has a base, resolved by §7:

| Where the default state is defined | Elements |
|---|---|
| In the element's own state table | 883 |
| Based element, not in its own table, present after resolution (§7) | 237 |
| Based element, defined nowhere in the resolved descriptor | 21 |
| Typed element (no base), not in its own table | 71 |

None of the 21 based elements is an override child (§7.3 step 6), and all 21 resolve without a
failure. A switch to a state that the resolved element does not define becomes a switch to
state 0 (§9.4 step 1), so for 92 elements (71 + 21) the default state has no effect.
(Confirmed, OBS, U20; probe re-run 2026-10-06.)

<a id="s5-8"></a>
### 5.8 Edge modes (values Confirmed; behaviour BIN)

Each edge has a mode that says how the edge moves when the parent's size differs from the
design size. Summary (exact formulas in §8):

| Value | Left / top edge | Right / bottom edge |
|---|---|---|
| 0 | keep the current position (the design position at the first layout) | same |
| 1 | centre the element (width or height kept) | same |
| 2 | mirror: distance from the near side equals the opposite edge's distance from the far side | mirror (symmetric margins) |
| 3 | scale the coordinate by the parent's growth ratio | same |
| 4 | follow the parent's **far** side (move by the parent's growth) | keep the design position (follow the near side) |
| 5 | keep the design position (follow the near side) | follow the parent's **far** side |
| 6 | aspect mode: centre, scale by the other axis's ratio, then keep the design aspect ratio | same |

Census (OBS):

| Edge | 0 | 1 | 2 | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|---|
| left | 10,776 | 564 | 0 | 494 | 1,006 | 3,560 | 21 |
| top | 11,428 | 268 | 0 | 297 | 803 | 3,625 | 0 |
| right | 10,722 | 564 | 3 | 534 | 832 | 3,745 | 21 |
| bottom | 11,156 | 266 | 3 | 309 | 1,223 | 3,463 | 1 |

Common combinations (left, top, right, bottom): (0,0,0,0) 10,197; (5,5,5,5) stretch with the
parent 1,639; (5,0,5,0) 385; (5,5,5,4) stretch horizontally 380; (5,5,4,4) pinned to the top
left 374; (4,5,5,4) pinned to the top right 329; (3,3,3,3) proportional 288; (4,4,5,5) pinned
to the bottom right 206. Mode 6 occurs only as (6,5,6,5) (17, full-screen "scale parents" and
loading images), (5,5,6,5) (4), (6,5,5,5) (4) and (1,1,1,6) (1); mode 2 only as (5,5,2,2) (3).

<a id="s5-9"></a>
### 5.9 Properties

Element and state blocks carry ordinary embedded property sets (PS §5). All 16,421 element
sets and 12,977 state sets decode with the property dictionary. The dictionary names are
interoperability identifiers; they come from the install.

- 35,615 top-level property entries on elements, using 835 PIDs: 202 engine-level PIDs
  (below 0x10000000, names `UICore_…`, `UISkin_…`) with 30,412 entries and 633 game-level PIDs
  (0x1000xxxx) with 5,203 entries.
- 2,083 entries on states, using 44 PIDs (39 engine-level). The most common state properties
  are `UICore_Text_font_colors` (1,076), `UICore_Element_hide` (165), `UICore_Text_entry`
  (164), `UICore_Text_font_outline_color` (108) and `UICore_Meter_child_image` (90).
- Value types found anywhere in layout properties (counting nested values): Enum 15,130,
  Bool 11,381, DID 8,447, Int 5,940, StringInfo 5,478, Array 4,655, Struct 4,287,
  Colour 2,864, Float 821, PropertyId 630, Bitfield32 386, Bitfield64 99, Vector3 72,
  String 10. Nesting reaches depth 12.

Engine-level properties whose client behaviour this spec traced (others are named only):

| PID | Dictionary name | Type | Uses (elements + states) | Effect (BIN) |
|---|---|---|---|---|
| 0x00C0 | `UICore_Element_ObjectOpacity` | Float | 5 + 2 | Opacity of the element's own drawing surface (root panels and elements that code gives a surface): fades the element with all its children. Fade items animate it (§6.6). No effect on an element without its own surface |
| 0x06E5 | `UICore_Element_alphablendmod` | Float | 1 + 0 | Opacity factor of the element's two image layers, clamped to 0–1, default 1 (§9.6) |
| 0x0203 | `UICore_Element_ObjectMode` | Enum 0x2300017A: 0 None, 1 Stretch, 2 Tile, 3 CanvasResize | 446 + 3 | Surface mode, consulted on root panels; children of a Stretch parent are not re-laid out (§8.4) |
| 0x024F | `UICore_Element_notify_on_media_done` | Bool | 4 + 0 | When true, the element sends UI message 0x1B when its media list completes (§9.5) |
| 0x0B58 | `UICore_Element_IgnoreParentStateChange` | Bool | 1 + 0 | A child with this set ignores state changes passed down by its parent (§9.4) |
| 0x0025 | `UICore_Text_fonts` | Array of `UICore_Text_font` (0x06E9, DID 0x42…) | 783 + 52 | Fonts of a text element, chosen by font index (FO §9) |
| 0x0070 | `UICore_Text_entry` | StringInfo | 2,229 + 164 | The element's text (string-table reference) |
| 0x011B, 0x055E, 0x011C | `UICore_Dialog_UIElement_Override`, `UICore_Dialog_UILayout_Override`, `UICore_Dialog_modal` | Enum (element ID), DID (0x22), Bool | 0 in layouts | Read by the dialog framework from the dialog's own properties: the overrides replace the layout and element of the dialog; a modal dialog is created inside its own screen-sized container (§9.1) |
| 0x051F, 0x042A, 0x02F8, 0x07A8 | `UICore_Button_ghosted`, `UICore_Button_highlighted`, `UICore_Button_rollover`, `UICore_Button_supports_ghosted_highlight` | Bool | 8 + 3, 12 + 19, 965 + 47, 19 + 1 | Inputs of the button state choice (§9.7) |
| 0x03CD | `UICore_Element_DrawAfterChildren` | Bool | 232 + 3 | When true the element draws its children first and its own content after them (§9.3, §9.6). Read whenever the effective value changes, so a state can switch it |
| 0x0BD0 | `UICore_Element_clip_to_parent` | Bool | 1 + 0 | Read per child while the parent draws its children; selects which rectangle of the child is clipped (§9.3; OPEN there) |
| 0x0695, 0x0476, 0x0475 | `UICore_Element_save_loc`, `UICore_Element_save_size`, `UICore_Element_save_visibility` | Bool | 194 + 1, 41 + 1, 1 + 0 (true on 192, 34 and 1 elements) | Select which parts of a saved panel location are restored (§9.9) |
| 0x10000F07 | `UI_Element_CreateAsRootElement` (game level) | Bool | 61 + 0 | Read by the game code that opens the activatables layout: marks the top-level elements it creates as panels (§9.1) |

Most frequent other engine-level properties (named only): tooltip layout, ID, entry and on/off
(`UICore_Element_tooltip_*`), text justification and margins (`UICore_Text_*_justification`,
`UICore_Text_*_margin`, see FO §7.4), text colours and outline (FO §8), `UICore_Element_hide`,
`UICore_Element_ShouldEraseBackground`, `UICore_Element_activatable`,
`UICore_Element_DrawAfterChildren`, list-box entry templates (`UICore_ListBox_entry_templates`,
each a layout DID plus element ID), meter images and directions (`UICore_Meter_*`).

> **OPEN:** the behaviour of the remaining ~190 engine-level UI properties (tooltips,
> list boxes, trees, meters, scroll bars, menus, browsers, viewports) beyond their names and
> value types. Each widget class reads its own set; trace per widget as M5 panels need them.

<a id="s5-9-opacity"></a>
> **OPEN:** which descendants `UICore_Element_ObjectOpacity` reaches when they own a drawing
> surface of their own. The row above (BIN) states that the property fades the element's
> surface with all its children; that is traced for children drawn **into** that surface. For a
> descendant with its own surface (§6.6), `ui-behaviour.md` §11.1 step 2 (BIN) has auto-fade
> write the fade opacity into the element **and** into each child with its own surface, which
> would be needed only if such a child is composed on its own and is not reached by its
> ancestor's opacity; `lua-plugin-api.md` §13.19 leaves the same question OPEN for plugin
> windows. Fade items (§6.6) and the plugin setter write only the element's own property. Not
> traced. **Tessera rule** (DEC: UR Decision 1.7, "Opacity"): every surface is composed with
> its own `ObjectOpacity` only, and the ancestors' opacity is not multiplied in. So in Tessera
> a Fade item or `SetOpacity` on an element leaves its descendants that own surfaces unfaded
> (for example the minimap's player marker under a Fade on P1's root, `game-panels.md` §8.2
> step 3), while auto-fade fades them through UB §11.1 step 2.

<a id="s5-9-erase"></a>
> **OPEN:** the effect of `UICore_Element_ShouldEraseBackground` (named only above; it may make
> the erase of an update region, §9.6.1, conditional per element). **Tessera rule** (DEC: UR
> Decision 1.3): every update rectangle is erased to transparent black before it is drawn, and
> the property is ignored.

---

<a id="s6"></a>
## 6. Media items

<a id="s6-1"></a>
### 6.1 Bodies (follow the two u32 of §4.6)

Names in the first column are this spec's own; LC's names are given only where they disagree
in substance (§15).

| Kind | Name | Body (in order) | Body size | Count in data |
|---|---|---|---|---|
| 1 | Movie | u32 movie DID (DB-object type 0x08, `property-system.md` §9); Pascal string file path; bool8 fit_to_screen | 5 + path length + size of the path's VLE length (REC §1); that VLE is 1 byte for every observed path, giving 6 + path length | 2 (states) |
| 2 | — | not supported by the client | — | 0 |
| 3 | Jump | u32 target index; f32 probability | 8 | 0 |
| 4 | Image | u32 image DID; u32 draw mode; Pascal string file path (see note) | 8 + path length + size of the path's VLE length (REC §1); that VLE is 1 byte for every observed path, giving 9 + path length | 11,365 |
| 5 | Message | u32 UI message ID; f32 probability | 8 | 31 |
| 6 | Fade | f32 start opacity; f32 end opacity; f32 duration (seconds) | 12 | 36 |
| 7 | Hit mask | u32 image DID | 4 | 0 |
| 8 | Animation | f32 duration (seconds); u32 draw mode; bool8 loop; u32 frame count *k*; *k* × u32 frame DID | 13 + 4·*k* | 133 |
| 9 | Coloured image | u32 image DID; u32 draw mode; u32 colour 0xAARRGGBB | 12 | 421 |
| 10 | Cursor | u32 cursor image DID; u32 hotspot x; u32 hotspot y | 12 | 180 |
| 11 | Sound | u32 sound DID | 4 | 692 |
| 12 | State | u32 state ID; f32 probability | 8 | 86 |
| 13 | Pause | f32 minimum seconds; f32 maximum seconds | 8 | 52 |

Note on kind 4: the client reads the file-path string only when the archive reports version
≥ 2 for the item tag 0x4D444449; otherwise the item ends after the draw mode (BIN). The data
provides no version field, and every Image item in this install carries the string (the record
grammar fails without it). Readers read it unconditionally.

> **OPEN:** where the client's archive takes the version for tag 0x4D444449 from, and whether
> an older or newer build can produce Image items without the path.

Where items occur: Movie only in states; Image 2,985 element / 8,380 state; Message 7 / 24;
Fade 3 / 33; Animation 2 / 131; Coloured image 98 / 323; Cursor 178 / 2; Sound 5 / 687;
State 0 / 86; Pause 2 / 50.

Every item runs when the media player reaches it (§9.5). The result of running an item is
**done** (go on with the next item), **busy** (run it again next frame) or **state changed**
(stop this list; a new list has been installed).

<a id="s6-2"></a>
### 6.2 Image (kind 4)

- If the draw mode is 8 (None): remove the element's base image. Result: done.
- Otherwise, if the DID is 0 and the path is not empty: load the image from that file. The
  path is relative to the install directory, uses `/` separators and names a JPEG file in
  this install (11 items; all files exist). Result: done.
- Otherwise: set the element's base image to the DID (a render surface, prefix 0x41;
  REC §5) with the draw mode. A DID of 0 with an empty path gives an element without an image.
  Result: done. Before loading, the client lets an active user skin replace the surface (§9.8).

Census: 10,560 items with a 0x41 DID (all exist), 805 with DID 0 (794 of them with draw mode 8
and empty path: "clear image"). Draw modes: Alphablend 9,995, None 799, Normal 413, Colorize 76,
Screen 40, Overlay 35, Multiply 7. All referenced surfaces have category 15 (REC §5.4) and
formats A8R8G8B8 (code 21), R8G8B8 (20) or JPEG (500).

<a id="s6-3"></a>
### 6.3 Coloured image (kind 9)

Sets the element's **overlay** layer (§9.6): an image DID (0 = no image, a plain colour), a
draw mode and a colour.

- Draw mode 8 (None): remove the overlay. Result: done.
- Otherwise: overlay = (image or none, colour, draw mode). With no image the overlay fills the
  element's rectangle with the colour; with an image the image is tinted by the colour.

Census: DID 0 with draw mode None and colour 0 (154, "clear overlay"); DID 0 with another
draw mode and a colour (238, colour fills, e.g. 75 % black 0xBF000000); a 0x41 DID with a
colour (29). Draw modes: None 154, Grayscale 118, Normal 59, Alphablend 51, Multiply 29,
Colorize 7, Screen 3.

<a id="s6-4"></a>
### 6.4 Animation (kind 8)

A flip-book over the frame DIDs (all 0x41; 2,252 frames, all exist; 2–109 frames per item).

- On its first run the item records the start time.
- Each run: *t* = (now − start) ÷ duration, clamped to [0, 1]; if |duration| ≤ 0.0002, *t* = 0.
  The frame index is ⌊(*k* − 1)·*t* + 0.5⌋ (0-based). When the index differs from the last
  shown one, the element's base image is set to that frame with the item's draw mode.
- While *t* < 1 the result is busy. When *t* reaches 1 the start time is cleared; the result
  is busy again if `loop` is 1 (the animation restarts) and done otherwise.

Census: durations 1.0 (87), 1.1 (19), 2.0 (12), 0.5 (11), others; draw mode Alphablend 130,
Normal 3; loop 1 in 100, 0 in 33.

<a id="s6-5"></a>
### 6.5 Cursor (kind 10)

Sets the mouse cursor shown while the pointer is over the element: a 0x41 image and a hot spot
in pixels within that image. A DID of 0 restores the default cursor. Result: done. Census: 180
items, hot spots (16, 16) 157, (16, 14) 20, (0, 0) 3.

<a id="s6-6"></a>
### 6.6 Fade (kind 6)

Animates `UICore_Element_ObjectOpacity` (§5.9: the opacity of the element's own drawing
surface, so the element fades together with its children; for children that own a surface of
their own see the §5.9 opacity OPEN) from *start* to *end* over
*duration* seconds. 18 of the 36 Fade items sit on top-level elements, 16 on depth-1 and 2 on
depth-2 elements.

> **OPEN:** which non-root elements own a drawing surface (so that a Fade on them is visible).
> Partial answers from other specs: `lua-plugin-api.md` §13.19 (BIN) gives the client's rule
> for every element: it owns a surface while its `UICore_Element_ObjectMode` value is not 0
> (or it has no value and is a root panel) and it is shown or has
> `UICore_Element_KeepObjectWhenHidden` true; `ui-widgets.md` §4.5: an alpha-test meter asks
> for a surface at post-creation; `game-panels.md` §8.2 step 3: game code sets object mode 1
> on the minimap's player marker. §17.2 (U46) counts the non-root elements of the §17 panels
> that carry an object mode. Still open: whether that rule and the scope stated in §8.4 step 1
> describe the same set (§8.4 OPEN), and which descendants a Fade reaches when they own
> surfaces (§5.9 opacity OPEN). Tessera applies the LPA §13.19 rule to native and plugin
> elements alike (DEC: UR Decision 1.1).

1. On its first run: read the element's current opacity *c*. If the fade goes downwards
   (*start* > *end*) and *c* < *start*, or upwards (*start* < *end*) and *c* > *start*, the fade
   resumes part-way: the elapsed fraction is min(1, |*c* − *start*| ÷ |*end* − *start*|) and the
   start time is set back by that fraction of the duration. Otherwise the start time is now.
2. Each run: *t* = (now − start) ÷ duration clamped to [0, 1] (*t* = 1 if |duration| ≤ 0.0002);
   opacity = *start* + (*end* − *start*)·*t*.
3. Busy while *t* < 1; at *t* = 1 the start time is cleared and the result is done.

Observed triples include (1, 0, 0.5), (0, 1, 0.5), (0.3, 1, 0.3), (1, 0.3, 0.3).

<a id="s6-7"></a>
### 6.7 Pause (kind 13)

On its first run the item picks a duration uniformly between *minimum* and *maximum* (equal
values give a fixed wait) and returns busy until that much time has passed; then it clears its
timer and returns done. Observed pairs (1, 1) 32, (4, 4) 4, (8, 8) 4, (0.001, 0.001) 3,
(3, 3) 2, (0.2, 0.2) 2, (0.5, 0.2) 1 (minimum above maximum), (4.5, 4.5), (10, 10), (2.5, 2.5),
(3596, 3596).

<a id="s6-8"></a>
### 6.8 State, Message and Jump (kinds 12, 5, 3)

These items wait (result busy) until the element has finished its creation (§9.2 step 8).
Then each draws a uniform random number *r* in [0, 1] and acts only if *r* ≤ *probability*:

- **State**: switch the element to the state ID (§9.4). Result: state changed when it acted,
  done otherwise. 38 of the 86 items name state 0 (back to no state; from another state this
  restarts the element's own media list), 16 `normal`. Observed probabilities 1.0 (77) and 1/10, 1/9, …,
  1/2 (one each: a chain that picks one of several states with equal chance).
- **Message**: send the UI message ID (UIElementMessage) to the element's listeners (game code).
  Result: done. Probability 1.0 in all 31.
- **Jump** (unused in data): set the media player's position so that the list continues at the
  target index. Result: busy.

> **OPEN:** the exact index arithmetic of Jump (the client stores *target* − 1 as the current
> position before resuming). No item uses it.

<a id="s6-9"></a>
### 6.9 Sound, Movie, Hit mask (kinds 11, 1, 7)

- **Sound**: play the sound-info record with the DID (prefix 0x2A, DB-object type 0x12,
  `property-system.md` §9; 692 items, all exist)
  through the UI sound path, not positioned in the world. Result: done (the list does not wait).
- **Movie**: wait until the element has finished creation; then play the video named by the
  DID (0 in both items) or by the file path (relative to the install directory; Bink video files
  in this install). With `fit_to_screen` = 1 the video is scaled to the screen with its aspect
  ratio kept and centred. Result: busy until the video ends, then done.
- **Hit mask** (unused in data): set an image whose pixels decide where the element can be hit
  by the pointer (§9.3); DID 0 removes the mask. Result: done.

Colours in Coloured image items are one u32 = 0xAARRGGBB, stored little-endian (bytes B, G, R,
A); the client converts each channel to channel ÷ 255 (BIN).

---

<a id="s7"></a>
## 7. Resolving an element descriptor (base inheritance)

The client never instantiates a based descriptor directly. It first builds the resolved
descriptor (BIN).

<a id="s7-1"></a>
### 7.1 Chain

1. If the descriptor has a type (no base reference), it is already resolved.
2. Otherwise load the layout `base_layout` (DB type 0x28) and find `base_element` among its
   **top-level** elements (child elements are not searched). If the layout or the element is
   missing, resolution fails and the element is **not created** (§13).
3. Resolve that base descriptor first (recursively). The client keeps the list of
   (element ID, layout DID) pairs visited on this chain and fails on a repeat (cycle).
4. Merge the original descriptor into a copy of the resolved base (§7.2–§7.3).

Census: 8,617 based elements; chains of depth 1 (7,076), 2 (1,494), 3 (44); 3 references fail
(the named element does not exist in the named layout); no cycles.

<a id="s7-2"></a>
### 7.2 Merging a state block (base block *B*, derived block *D*)

Both blocks must have the same state ID (true for element blocks, both 0, and for states merged
by key). Then:

1. Copy from *D* into the result: the two flag bits, x, y, width, height, z-level and
   `has_position`. (The derived geometry always replaces the base geometry, including a zero
   rectangle.)
2. **Append** a copy of *D*'s media list after the base's media list (base items run first).
3. Merge the property sets: every property of *D* is added to the result, replacing a property
   with the same PID.

<a id="s7-3"></a>
### 7.3 Merging an element descriptor (base *B*, derived *D*)

Let *R* start as a copy of the resolved base, and remember its rectangle as the old box.

1. Merge the element's own blocks (§7.2).
2. If *D*'s element ID is not 0, it replaces *R*'s. *R* keeps its own type and base reference;
   *D*'s are never copied. When a based element is resolved, *R* is the resolved base, so the
   result is a typed descriptor with the type of the end of the chain. (In the recursive merge
   of step 6, *R* may be a based child; see there.)
3. Copy from *D*: default state, the four edge modes and the read order.
4. States: for each state of *D*, if *R* has a state with the same key, merge it (§7.2);
   otherwise add a copy.
5. Count the children of *R* whose keys *D* does not override; call the number *c*.
6. Children: for each child of *D*, if *R* has a child with the same key (an **override**),
   merge *D*'s child into *R*'s child with these same rules (§7.3, recursively); otherwise add
   a copy. In an override:
   - *R*'s child is the inherited child exactly as stored in the base layout: children are
     copied with their parent and never resolved during a merge, so it may itself be a based
     descriptor.
   - Neither child is resolved before or during the merge. The merge never copies a type or a
     base reference: the merged child keeps the **inherited** child's type, `base_element` and
     `base_layout`, and the overriding child's type and base reference are **ignored**, even
     when they name a different base. Everything else follows §7.2 and steps 2–8.
   - If the merged child still carries a base reference, it is resolved (§7.1) only when the
     element tree is created (§9.2 step 1): its base chain is loaded then and the merged child
     is merged into the resolved base like any based descriptor. Its media then run in the
     order: base chain, inherited child, overriding child.
7. Every child added in step 6 (new key) gets read order = its own read order + *c*, so new
   children sort after the inherited ones.
8. If *R*'s width or height now differs from the old box, every child of *R* whose key *D* does
   **not** override is re-laid out from the old box to the new one with the design-time rule of
   §8.2. Children that *D* overrides keep *D*'s geometry.

Census: 5,555 based elements change size relative to their base; their inherited children
(13,036) are re-laid out by step 8. No inherited child uses edge mode 2 or 6.

Override census (step 6). Count each stored child of a stored based element whose key matches
a child of the resolved base: 97 such children. A further 13 overrides happen one level deeper
(a child of an overriding child overrides a child of the inherited child), 110 override sites in
all. In every site both children are typed with the same type (82 sites) or both are based
(28 sites). In 8 of the 21 first-level based pairs the two children name different bases; the
derived child's base is ignored there (the result follows the inherited child's base). All 7
deeper based pairs name the same base. Worked example: §14.5.

If every top-level element of every layout were created with all inherited children, there
would be 83,161 element instances (59,224 of type Field, 6,945 Button, 6,923 Text, …): a
measure of how much the layouts share through templates.

---

<a id="s8"></a>
## 8. Layout

<a id="s8-1"></a>
### 8.1 Conventions

- Boxes are inclusive: (left, top, right, bottom), width = right − left + 1.
- An element's design box *D* is (x, y, x + width − 1, y + height − 1) from its resolved
  descriptor (after user-skin overrides, §9.8).
- *O* is the parent's design box ("old"), *N* the parent's current box ("new"):
  *Ow* = *Or* − *Ol* + 1, *Oh* = *Ob* − *Ot* + 1, *Nw*, *Nh* likewise.
- ΔW = *Nw* − *Ow*, ΔH = *Nh* − *Oh*.
- *Rx* = *Nw* ÷ *Ow* and *Ry* = *Nh* ÷ *Oh* as 32-bit floats (0 when the divisor is 0).
- *w* = *Dr* − *Dl* + 1, *h* = *Db* − *Dt* + 1.
- half(*n*) = *n* ÷ 2 rounded toward zero; trunc(*f*) = *f* rounded toward zero to an integer.
- Results are relative to the parent's top-left corner.

<a id="s8-2"></a>
### 8.2 Design-time rule (used only by inheritance, §7.3 step 8) (BIN)

The new box of the child is computed from its design box with the parent's old and new boxes:

| Mode | Left | Top | Right | Bottom |
|---|---|---|---|---|
| 0, 5 (left/top); 0, 4 (right/bottom) | *Dl* | *Dt* | *Dr* | *Db* |
| 1 | half(*Nw*) − half(*w*) | half(*Nh*) − half(*h*) | half(*Nw*) + half(*w*) − 1 | half(*Nh*) + half(*h*) − 1 |
| 3 | trunc(*Dl*·*Rx*) | trunc(*Dt*·*Ry*) | trunc(*Dr*·*Rx*) | trunc(*Db*·*Ry*) |
| 4 (left/top); 5 (right/bottom) | *Dl* + ΔW | *Dt* + ΔH | *Dr* + ΔW | *Db* + ΔH |
| 2, 6, any other | unchanged; the client reports an error and the step fails, but the other edges still apply | | | |

The child's new rectangle is (left, top, right − left + 1, bottom − top + 1), and its own
children are then re-laid out the same way, recursively, from its old box to its new one.

<a id="s8-3"></a>
### 8.3 Reference boxes at run time (BIN)

When an element is laid out at run time (§8.4), its *O* and *N* are:

| Case | *O* | *N* |
|---|---|---|
| The element is a **screen-level panel** (created as a root panel, §9.1, or flagged by code), or it has no parent | (0, 0, design_width − 1, design_height − 1) of the element's layout record | (0, 0, screen width − 1, screen height − 1) |
| The element has a code-assigned reference box | that box | the parent's current box |
| Otherwise | the parent's **design** box (from the parent's resolved descriptor) | the parent's **current** box |

> **OPEN:** which client code assigns a reference box to an element (the second row). Not
> needed for data-driven panels.

<a id="s8-4"></a>
### 8.4 Run-time rule (BIN)

The client lays an element out when it is attached to a parent, and lays all children of an
element out again whenever that element's size changes.

Tessera rule (DEC: IB D8, "Geometry used for hit tests"): hit tests read every element's
current geometry, so this re-layout takes effect at once for hit testing. An implementation
may defer it for speed only until the next hit test that would read an element it moves; before
any hit test every pending re-layout is completed. Painting (UR Decision 1.3) reads the same
geometry and changes no box that a hit test reads.

1. **Stretch parents.** If the element has no object mode of its own and its parent's object
   mode (`UICore_Element_ObjectMode`, consulted only on root panels and elements flagged by code;
   default CanvasResize) is Stretch, keep the element's current box and go to step 6 (no
   clamping).

> **OPEN:** what object modes None (0), Tile (2) and CanvasResize (3) do beyond this step:
> how the drawing surface of a root panel in each mode is sized, scaled or repeated when it is
> composed onto the screen. Known (BIN, from a diagnostic message of the client, not from traced
> code): Tile needs surface dimensions that are powers of two; otherwise the client logs the
> problem and switches the element to CanvasResize. Data use on elements: Stretch 301,
> CanvasResize 119, None 24, Tile 2. Not needed for panels drawn 1:1.
>
> **Tessera rules** until traced (DEC: UR Decision 1.1, 1.6, 1.7 and §2.1):
> - every surface's canvas has the size of the element's layout box (UB §9.2.1) and is
>   composed onto the element's displayed box, scaled by displayed box ÷ canvas on each axis
>   (1 : 1 when the two are equal); Stretch and CanvasResize differ only in what an element
>   resize does to the layout box (UB §9.2.1);
> - **Tile** with both canvas sides powers of two is **not** fitted to the box: the canvas is
>   repeated from the box's top-left corner over the displayed box at the scale of the nearest
>   display node at or above the element (UB §9.2.1), so unscaled it is an exact 1 : 1
>   repeat. An element below a Tile surface gets no surface of its own and is drawn into the
>   Tile surface's canvas as if its mode were 0;
> - **None** on a shown root panel that has no surface above it (for example a native
>   top-level panel whose layout value is 0): the panel is drawn through a Tessera stand-in
>   surface, sized and composed like CanvasResize but at opacity 1, because for the original
>   the element has no surface for its `ObjectOpacity`. A resize of such a panel follows the
>   no-surface rule of UB §9.2.1 step 3 (the layout box takes the size, the children are laid
>   out again). For every other rule the panel counts as having no surface.
>
> **OPEN (conflict, Tile canvas size):** the Tile bullet above takes its canvas size from UR
> §2.1 (the layout box, when both of its sides are powers of two). `lua-plugin-api.md` §20
> (BIN, fourth pass, correction to §5.9 here) says instead that a Tile surface of the original
> takes the size of the element's **base image** (or a size set by code), and falls back to
> CanvasResize unless both sides of that size are powers of two. The two do not agree, and the
> decision does not cite the LPA statement. The BIN statement is the fact about the original;
> the layout-box canvas stays Tessera's rule only until decision 0015 is revised to
> reconcile it (for plugin mode 2 the decision already uses the base image). It does not affect
> M5: no element of the §17 panels is in mode Tile (UL22), and the data hold 2 Tile elements
> in all.

<a id="s8-4-scope"></a>
> **OPEN:** two statements about object modes do not yet agree, and neither was re-traced.
> (1) Step 1 says the object mode is consulted only on root panels and elements flagged by
> code; `lua-plugin-api.md` §13.19 and its §20 correction to §5.9 (BIN) say that **any**
> non-zero object mode, from a layout or from `SetStretchMode`, gives the element its own
> surface, and §17.2 (U46) finds 92 non-root elements with a non-zero mode in the §17 panels.
> Whether the Stretch-parent test of step 1 also applies to a non-root parent that owns a
> surface by that rule is not known. (2) `ui-behaviour.md` §9.2.1 has a branch for the element
> resize of a surface-owning element in mode None (the layout box is not changed); by
> LPA §13.19 condition 1 an element in mode 0 owns no surface, so that branch would never be
> reached; `ui-behaviour.md` §9.2.1 records the same question as an OPEN, with the same Tessera
> rule. **Tessera rule** (DEC: UR Decision 1.1, §2.1): surface ownership follows
> LPA §13.19 for every element, native or plugin; for an element that owns a surface, its
> object mode decides an element resize as UB §9.2.1 says (Stretch: the layout box is kept and
> the children are not laid out again); an element in mode 0 resizes by the no-surface rule of
> UB §9.2.1 step 3.
>
> **Tessera rule for question (1)** (a Tessera choice, not a decision text; the decisions do
> not settle it): the Stretch-parent test of step 1 is applied only when the parent is a **root
> panel**. Step 1 also names "elements flagged by code"; which elements code flags is not known
> (§6.6 OPEN), and Tessera flags none. Whether that flag is the one by which an alpha-test
> meter asks for a surface at post-creation (`ui-widgets.md` §4.5) is not known; the two were
> traced separately. Tessera does not treat an alpha-test meter as a flagged element: it owns a
> surface (UW §4.5), but for step 1 and for the screen-level row of §8.3 it is laid out like
> any other non-root element. No alpha-test meter is in the §17 panels, so M5 does not depend
> on this. A child of a **non-root** element that owns a
> surface in Stretch mode (for example the children of `GamePlay_Radar` in P1) is laid out by
> steps 2–6 like the
> child of any other non-root element, both when it is attached and whenever its parent's
> layout box changes. Because an element resize of a Stretch surface keeps its layout box
> (UB §9.2.1), such a child is in practice laid out again only when the parent's layout box
> changes by another path (for example after a change of the parent's object mode), not
> through the parent's own element resize. Data that bear on the
> choice (OBS, 2026-10-06, the 15 instantiated panels of §17, U47): 159 children without an
> object mode of their own sit under a non-root Stretch element. For 158 of them, steps 2–6 at
> the parent's design size give back the child's design box, so the two readings agree. The
> one exception is P10's title label `Panel_Label_Title`, a 295 pixels wide element with
> centring edges (1, 5, 1, 5) at design x = 0 inside the 350 pixels wide Stretch element
> `Panel_TitleBar`. Laid out by its edges it sits at x = 28 (and is 294 wide, §8.4 step 2);
> kept by step 1 it would stay at x = 0. The authored centring edges point to layout by edges,
> which is why Tessera chose it. An owner screenshot of P10's title bar settles it: under this
> rule the title is centred in its bar.

2. **First pass**, each edge by its mode:

| Mode | Left | Top | Right | Bottom |
|---|---|---|---|---|
| 0 | *Dl* | *Dt* | *Dr* | *Db* |
| 1 | half(*Nw*) − half(*w*) | half(*Nh*) − half(*h*) | half(*Nw*) + half(*w*) − 1 | half(*Nh*) + half(*h*) − 1 |
| 2 | (*Or* − *Ol*) − *Dr* | (*Ob* − *Ot*) − *Db* | (*Nr* − *Nl*) − *Dl* | (*Nb* − *Nt*) − *Dt* |
| 3 | trunc(*Dl*·*Rx*) | trunc(*Dt*·*Ry*) | trunc(*Dr*·*Rx*) | trunc(*Db*·*Ry*) |
| 4 | *Dl* + ΔW | *Dt* + ΔH | *Dr* | *Db* |
| 5 | *Dl* | *Dt* | *Dr* + ΔW | *Db* + ΔH |
| 6 | trunc(half(*Nw*) − *w*·*Ry*·0.5) | trunc(half(*Nh*) − *h*·*Rx*·0.5) | trunc(half(*Nw*) + *w*·*Ry*·0.5) − 1 | trunc(half(*Nh*) + *h*·*Rx*·0.5) − 1 |
| other | *Dl* | *Dt* | *Dr* | *Db* |

   The products and sums of mode 6 are 32-bit float operations; half(*Nw*) and half(*Nh*) are
   integers converted to float.

3. **Aspect pass** (only for edges in mode 6). Let *A* = *w* ÷ *h* (1.0 if *h* = 0) and
   *B* = *h* ÷ *w* (1.0 if *w* = 0), from the design size. With *L*, *T*, *R*, *Bm* the results
   of the first pass, in this order:
   1. if the left mode is 6: *L* = trunc(*R* − (*Bm* − *T* + 1)·*A*);
   2. if the right mode is 6: *R* = trunc(*L* + (*Bm* − *T* + 1)·*A*) − 1 (using the new *L*);
   3. if the top mode is 6: *T* = trunc(*Bm* − (*R* − *L* + 1)·*B*);
   4. if the bottom mode is 6: *Bm* = trunc(*T* + (*R* − *L* + 1)·*B*) − 1 (using the new *T*).

   All arithmetic of this pass is in 32-bit floats, as in step 2 (BIN, U25): *A* and *B* are
   the quotient of the two design extents, each converted from integer to a 32-bit float and
   divided in 32 bits; the integer extent ((*Bm* − *T* + 1) or (*R* − *L* + 1)) is converted to
   a 32-bit float and multiplied by *A* or *B* in 32 bits; the edge it starts from (*R*, *L*,
   *Bm* or *T*) is converted to a 32-bit float and the product is subtracted from or added to
   it in 32 bits; the result is truncated toward zero, then 1 is subtracted for the right and
   bottom edges as integers. Using 64-bit floats can change results by one pixel.
4. **Mode 0 keeps the current position.** If the element's current box is not empty (width or
   height not 0) or the element has finished creation, every edge in mode 0 takes the value of
   the element's **current** box instead. (Elements in mode 0 therefore stay where code or the
   user put them; at their first layout the current box equals the design box.)
5. **Keep inside parent.** If the element's own flag bit 1 is set: *L* = max(*L*, 0),
   *T* = max(*T*, 0), *R* = min(*R*, *Nw*), *Bm* = min(*Bm*, *Nh*), with
   *Nw* = *Nr* − *Nl* + 1 and *Nh* = *Nb* − *Nt* + 1. This is the client's exact rule (BIN,
   U25): the bounds are the parent's width and height, not width − 1 and height − 1, so the
   inclusive right and bottom edges may end one pixel beyond the parent's last column and row.
   Implementations that match the client must keep this. The clamp only moves edges, so it can
   shrink the element; it never moves the element as a whole.
6. Set the element's size to (*R* − *L* + 1, *Bm* − *T* + 1) and its position to (*L*, *T*).
   When its size changed, its children are laid out again.

Notes:

- Modes 4 and 5 are mirror images: 4 means "this edge follows the parent side opposite to it",
  5 "this edge follows the parent side it faces". On the left and top edges 0 and 5 give the
  same first-pass value; on the right and bottom edges 0 and 4 do.
- Mode 2 uses the parent's design size for the left and top edges and its current size for the
  right and bottom edges. In data it occurs only on right and bottom edges, where it makes the
  far margin equal the near margin.
- Mode 6 on both horizontal edges with mode 5 on top and bottom (17 of the 26 elements that use
  mode 6) fills the parent's height and gives the element the width height × *A*, roughly
  centred: a "cover" fit of a wide loading image or a scaled full-screen container. With mode 6
  on one horizontal edge only ((5, 5, 6, 5) and (6, 5, 5, 5), 4 each), the other edge stays
  anchored and the element keeps its design aspect ratio from it: side panels of full-screen
  menus.

<a id="s8-5"></a>
### 8.5 Screen root (BIN)

The client creates one screen-root element: element ID 0x13 (engine label `Hollow`), type
Field, design size = the screen size (1024 × 768 if no renderer is available), edges
(5, 5, 5, 5), and lays panels into it (BIN, U26). Root panels laid out by §8.4 use the
screen-level reference boxes of §8.3, which read the current screen size.

**Screen size changes** (answered by `ui-behaviour.md` §9.4, BIN). The UI registers for the
render device's size-change notification; the handler resizes the screen root to the new
render size (1024 × 768 without a device). Because the root's size changed, §8.4 lays out every
root panel again from its layout's design size to the new screen size (§8.3), and modal-dialog
containers (§9.1), being screen-root children with edges (5, 5, 5, 5), follow. What game code
does on the accompanying notification is OPEN there.

Tessera rule (DEC: UR §2.6): the screen size is the render target's size in physical pixels.
The platform's display scale factor is **not** applied to it, because the original has no such
factor; larger panels come from `UIScale_Global` (`ui-behaviour.md` §9.1), as in the original.
On a screen resize a root panel whose size does not change is only moved (§8.4 step 6), so its
children are not laid out again.

---

<a id="s9"></a>
## 9. Building native panels (BIN)

<a id="s9-1"></a>
### 9.1 Root panels

Game code creates a panel from a (layout, top-level element ID) pair, where the layout is named
by its `UILAYOUT` entry key (§3.2, BIN U31). The generic steps are:

1. Load the layout and find the top-level element (top-level table only).
2. Create the element tree (§9.2 steps 1–6) without a parent, then run the panel's
   post-creation (§9.2 step 8).
3. Mark the panel as screen-level (§8.3) and as having its own object mode (§8.4 step 1).
4. Attach it to the screen root (§8.5); this lays it out from the layout's design size to the
   screen (§8.3, §8.4).
5. Show it if the screen root is shown.

A dialog whose `UICore_Dialog_modal` is true is instead created inside a new screen-sized
container element (a Field, edges 5, 5, 5, 5) that is itself screen-level; if the dialog has no
object mode, the client gives it CanvasResize. `UICore_Dialog_UILayout_Override` and
`UICore_Dialog_UIElement_Override` replace the layout DID and element ID that the dialog would
otherwise use.

Tessera rules for these dialogs:
- **Surface** (DEC: UR Decision 1.1, "Modal dialogs"): the dialog owns the drawing surface
  (it gets CanvasResize when it has no object mode, above). The screen-sized container, which
  code creates with no images and no text, gets no canvas; it stays in the tree for layout and
  hit testing. Whether the original gives the container a surface is not stated; the
  difference cannot be seen.
- **Input** (DEC: IB D5, "Who pushes"; `ui-behaviour.md` §5.6 OPEN): while a dialog of the
  dialog framework is the active panel, Tessera pushes the input map `DialogBoxes` (Tessera
  context `ui.dialog`) for it at priority 0. Which panels push that map in the original is
  not known.

For Tessera's walker (M5) the data-driven part is: create a root panel from a (layout, element)
pair and run §9.2–§9.6. (Steps and dialogs: BIN, U27.)

**How game code reaches these steps (BIN, U31, U32).** Three entry points exist. Callers name the layout by
its entry key; only a variant of the third form takes a layout DID (for layouts that a
property or another record names):

| Entry | Arguments | What it does | Callers seen |
|---|---|---|---|
| **Open panel** | (entry key, top-level element ID) | Maps the key to a layout DID (§3.2); if the map has no entry, nothing is created. Otherwise runs steps 1–5 above and returns the panel. Window classes of game code call it from their own set-up, with both values as immediates or with the key held by the window object | 100 call sites with both values as immediates, 14 more through one window-class helper; all 114 name a top-level element of the keyed layout |
| **Activatables loop** | none (one fixed key, the `gameplayactivatables` entry) | Loads that layout and walks its top-level table in table order. For each element whose resolved properties (§7) have `UI_Element_CreateAsRootElement` = true it opens a panel through **Open panel**, except when a game condition holds and the element's `UI_HideWhenField_HideWhenMonsterPlayer` is true (the condition routine was not read; Probable, from the property's name: the player plays a monster). For each panel it records `UI_Element_IsActivatablePanel` and `UI_Element_IsContextualPanel` (game behaviour beyond that is not traced) | 1 |
| **Create element** | (parent or none, entry key, element ID) | Maps the key to a layout and creates that top-level element inside the given parent (or without one) by §9.2. A sibling entry takes a layout DID instead | 247 call sites (219 with a key immediate); 29 for the DID form |

In this install 46 of the 48 top-level elements of the activatables layout have
`UI_Element_CreateAsRootElement` = true after resolution (most through their base, the movable
"drag box" template); they are the movable heads-up boxes (minimap, quest tracker, extra
shortcut bars, progress bars and similar; §17).

> **OPEN:** which of these panels the client opens at start-up and in which order, and which
> it opens only on demand (a key press, a menu button, a game event). This is game code outside
> the layout records; no record lists the start-up set. Tessera chooses its own panel set (§17)
> and does not need the answer to render a panel.

<a id="s9-2"></a>
### 9.2 Creating an element tree

The client keeps the **ID path** (the element IDs from the panel's top element down to the
current one) while it creates a tree; user skins match on it (§9.8). For an element
descriptor (top-level or child):

1. Resolve the descriptor (§7). On failure, do not create the element (and none of its
   children); creation of its siblings and parent continues.
2. Apply user-skin overrides to the resolved descriptor (§9.8).
3. Look up the widget factory registered for the descriptor's type. If none is registered, the
   element is not created. The engine types used by the layouts (GroupBox, ColorPicker, Menu,
   Scrollbar, ListBox, Panel, Button, Tree, Viewport, Browser, Field, Dragbar, Text, Meter,
   ResizeBar; §11) have engine widgets; game types 0x1000xxxx are registered by game code.
4. The factory creates the element. The element keeps a copy of its resolved descriptor; its
   position and size are set from the design rectangle and its z-level from `z_level`.
5. The element is registered by element ID (IDs are not unique across the UI). The client then
   looks the element ID up in the UI manager's **listener registry**: a table, keyed by element
   ID and then by UI message (`UIElementMessage`, §3.3), of the game-code handlers that asked to
   receive that message from elements with that ID. If the element's entry holds at least one
   of the four mouse-button messages 0x0E `Element_mouse_release`, 0x18 `Element_mouse_tap`,
   0x1E `Element_mouse_click` and 0x32 `Element_mouse_press`, the client sets the element's
   **mouse-visible** flag to true before its children are created (BIN, U28, U33).
   Registering a handler for one of these four messages later sets it on every live element
   with that ID. Mouse-visible is the flag the hit test and the hover update read (§9.3); it
   is written by these registrations, by the element class's type rule and by code, the last
   write wins, and post-creation (step 8) recomputes it from the type rule, so the value set
   here does not survive creation. `ui-behaviour.md` §4.3 is the single statement of this
   rule, including the type rules (Buttons, for example, are mouse-visible without any
   handler) and Tessera's rule; implement the flag from there. The registry's handlers are
   called when a message reaches the top of its walk (`ui-behaviour.md` §12.1).

   The earlier reading of this table as per-user saved panel data was wrong; saved panel
   locations are a separate mechanism (§9.9).

6. Push the element's ID on the ID path. For each child descriptor of the resolved descriptor,
   in table order: create the child (steps 1–6, recursively), run the child's post-creation
   (step 8), then attach the child to the element (§9.3), which lays the child out (§8.4).
   Pop the ID.
7. A **top-level** element created inside a given parent is then attached to it (laid out,
   §8.4). A root panel has no parent at this point (§9.1).
8. **Post-creation** (for a top-level element after step 7, for a child before it is
   attached): start the element's own media list (the media of its own block, §9.5), mark the
   element as having finished creation, then switch it to `default_state` (§9.4), report
   every effective property to the widget (§9.4 step 5) and recompute the element's
   mouse-visible flag from its type rule (`ui-behaviour.md` §4.3; BIN, U28).

Tessera note: unknown game types need a fallback widget (for example a plain Field that draws
images and text) so that their art still shows; record which types fell back.

<a id="s9-3"></a>
### 9.3 Sibling order, drawing order and hit testing

Each element keeps its children in one list ordered by **z-level descending**, and among equal
z-levels by **read order ascending**. A child is inserted by walking from the end of the list
backwards past every child that has a lower z-level, or the same z-level and a higher read
order.

- **Hit testing** walks the list from its end: the child with the lowest z-level (and, among
  equals, the highest read order) is tested first, and the first child that contains the point
  (and whose hit mask, if any, does not exclude it) receives it; the search then descends into
  that child's own children the same way. Only visible elements are tested. When no child takes
  the point, the element itself is the target only if its **mouse-visible** flag or its
  **blocks-clicks** flag (`UICore_Element_blocks_clicks`, or set by code) is on; otherwise the
  point passes to the elements behind it (BIN, U33). Boxes here are the elements' displayed
  boxes, which differ from the layout boxes of §8 only under a UI scale. The full procedure,
  the flag rule and the box model are `ui-behaviour.md` §4.1–§4.3 and §9.2.1.
  Tessera rules: hit tests read every element's current geometry as it stands when they run
  (DEC: IB D8, §8.4 note); the cases in which Tessera's drawn position and hit-test box differ
  (a rotated surface, a resize whose paint is still running, rounding under a UI scale) are
  recorded in UR §2.4.
- **Drawing** goes from the start of the list to its end: higher z-levels behind, lower in
  front, and among equal z-levels the lower read order first (BIN, U13, U34). Each child is
  drawn with all its descendants before its next sibling.

**Drawing one element (BIN, U34).** The client redraws a panel's drawing surface by update
regions (a list of rectangles that changed). Drawing an element:

1. If the element is not visible, it and its whole subtree are skipped.
2. Intersect each update rectangle with the element's clip rectangle (its own box in surface
   coordinates, already intersected with its ancestors' clips). If nothing remains, skip the
   element and its subtree.
3. If code attached a background object to the element, draw it first (the condition is a
   flag that code sets; not traced further).
4. If `UICore_Element_DrawAfterChildren` (§5.9) is **true**, draw the children now (step 6).
5. Draw the element's own content. For a plain element this is the two image layers of §9.6.
   Widget classes replace this step with their own drawing (270 of the 737 widget classes do),
   which is where text, meters and similar widget content are drawn; per class not traced.
6. If `UICore_Element_DrawAfterChildren` is **false** (the default), draw the children now:
   walk the child list from its start to its end; for each child compute its rectangle in
   surface coordinates (the parent's origin plus the child's position), intersect it with the
   parent's clip rectangle, skip the child if the result is empty, and otherwise draw the child
   by these same steps with the result as its clip.

So every element is clipped to its own box and to all its ancestors' boxes; nothing is drawn
outside a parent, and an element that lies wholly outside its parent is never drawn.

<a id="s9-3-nested"></a>
> **OPEN:** how an element that owns a surface of its own below a root panel (a non-zero
> object mode, §6.6, §8.4 OPEN; §17.2 counts them per panel) is composed into or over the
> surface of its parent: its clip, its order against the parent content drawn after it, and
> the opacity it takes (`lua-plugin-api.md` §13.18 OPEN (1), §13.19 OPEN; §5.9 opacity OPEN).
> The steps above are the traced rule for drawing into one surface. **Tessera rule** (DEC: UR
> Decision 1.7): such an element is composed as a separate entry at its place in the drawing
> order; parent content drawn after it inside its coverage region goes to a canvas above it;
> it is clipped by its nearest enclosing root panel and by the root panels above that, not by
> its other ancestors; it takes its own opacity only. One consequence for P1 is recorded in
> `game-panels.md` §8.7.2.

> **OPEN:** `UICore_Element_clip_to_parent` (PID 0xBD0, set on one element) is read per child in
> step 6 and, when true, makes the client take the child's rectangle from a second set of the
> child's fields; which rectangle that is was not established (U35). Tessera ignores the property
> until then.

<a id="s9-4"></a>
### 9.4 States at run time

Switching an element to state *s*:

1. Look up *s* in the element's resolved states table. If it is not there, the target becomes
   0 (no state).
2. If the target equals the current state, stop.
3. Make the target the current state.
4. **Pass to children:** if the state found has flag bit 0 set, or no state was found and the
   element's own block has bit 0 set, switch every child to the target the same way (so an
   undefined state reaches the children as 0), except children whose
   `UICore_Element_IgnoreParentStateChange` is true.
5. **Properties:** the element's effective properties are its own properties, overridden by
   the current state's properties, overridden by properties set at run time. The client
   recomputes them and reports each property whose effective value changed (or disappeared) to
   the widget, which reacts (text, colours, visibility, …).
6. **Media:** replace the running media list with the state's media list, or with the element's
   own media list when the state has no descriptor, and start it from the first item (§9.5).

Media effects are not undone when the state changes: an image or overlay set by one state stays
until another item replaces or clears it. 2,852 of the 12,977 states are empty (no properties,
no media).

<a id="s9-5"></a>
### 9.5 Media player

Each element runs at most one media list at a time.

1. Starting a list sets the position to the first item.
2. Running: from the current position, run items one after another. If an item returns done,
   go on to the next. If it returns busy or state changed, stop and register the element for an
   update next frame; the next update resumes at the same position (so a busy item is run
   again).
3. When the position passes the last item, the list is complete; if
   `UICore_Element_notify_on_media_done` is true the element sends UI message 0x1B (engine
   label `Element_media_done`) to its listeners.

So a list is a small script: instant items (Image, Coloured image, Cursor, Sound, Message,
Hit mask) apply in order within one frame; Fade, Animation, Pause and Movie block the script
until they finish; a State item hands over to another state's script.

<a id="s9-6"></a>
### 9.6 Drawing an element's images

An element has two image layers. They are part of the element's own content (§9.3 step 5), so
they are drawn before the element's children, or after them when
`UICore_Element_DrawAfterChildren` is true (§9.3 steps 4–6; BIN, U30, U34):

1. **Base image** (set by Image and Animation items): the surface drawn with its draw mode.
2. **Overlay** (set by Coloured image items): the colour fill, or the surface's alpha filled
   with the colour (§9.6.3), with its draw mode.

Each layer is drawn at the surface's **native pixel size**, anchored at the element's top-left
corner (plus a run-time image offset that is (0, 0) unless code sets it), **repeated (tiled)**
horizontally and vertically to cover the element's rectangle, and clipped to the rectangle. No
layer is ever stretched. When the element fits inside the surface and the image offset is
(0, 0), the client draws the top-left part of the surface once; otherwise it tiles.

The element's image opacity (`UICore_Element_alphablendmod`, clamped to 0–1, default 1) is
passed with the **base layer only** (M5 revision, BIN, U16): the overlay layer's draw takes the
item's colour and no opacity.

> **OPEN:** whether the client folds `UICore_Element_alphablendmod` into the overlay colour's
> alpha somewhere else (not found; the property is set on one element in this install).

Data agreement: of 10,350 Image items on typed elements, the surface size equals the element
size in 6,127 cases, is smaller on at least one axis in 3,000 (tiled "middle" pieces of
frames, for example a 2-pixel-wide image on a 474-pixel-wide edge) and is larger in 1,223
(clipped).

Data agreement for `UICore_Element_DrawAfterChildren` (OBS, U44): it is true on text buttons of
the main menu, options and plugin manager panels (§17 P6, P7, P12) whose frame pieces are child
elements and whose label is drawn by the button itself; with the property the label lands on
top of the frame.

<a id="s9-6-1"></a>
#### 9.6.1 Draw modes and compositor operations (BIN, U36, U38, U43)

The UI is composed on the CPU: each root panel owns a drawing surface (§8.4 step 1, §9.1;
Probable: 32-bit ARGB, the destination format whose routines keep an alpha channel), and element
layers are blended into it pixel by pixel by the client's 2D compositor.
When an Image, Animation or Coloured image item sets a layer, the client converts the draw mode
(`DrawModeType`, §3.3) into one of seven compositor operations:

| Draw mode | Label | Operation (this spec's name) | Base layer | Overlay without image | Overlay with image |
|---|---|---|---|---|---|
| 1 | Colorize | Colorize | §9.6.2 | §9.6.3 | not traced (unused) |
| 2 | Normal | Replace | §9.6.2 | §9.6.3 | not traced (unused) |
| 3 | Multiply | Multiply | §9.6.2 | §9.6.3 | §9.6.3 |
| 4 | Alphablend | Over | §9.6.2 | §9.6.3 | not traced (unused) |
| 5 | Overlay | Mix colour | §9.6.2 | not traced (unused) | not traced (unused) |
| 6 | Grayscale | Grayscale | **draws nothing** (the client has no routine) | §9.6.3 | not traced (unused) |
| 7 | Screen | Screen | §9.6.2 | not traced (unused) | §9.6.3 |
| 8 | None | (no layer) | the item clears the layer (§6.2, §6.3) | same | same |
| 0 or > 8 | — | Replace | as draw mode 2 | | |

"Unused" means no item of that kind uses the combination in this install (Image items use draw
modes 1, 2, 3, 4, 5, 7 and 8; Coloured image items without an image use 1, 2, 3, 4, 6 and 8, with
an image only 3 and 7; §6.2, §6.3). The names are the client's labels; the arithmetic is below
and does not follow the usual meaning of every name (draw mode 5 "Overlay" is a plain colour
mix that keeps the destination's alpha).

How the panel surfaces reach the screen is a separate step:

> **OPEN:** how a root panel's drawing surface is composed onto the frame (blend state,
> premultiplication, filtering) and what a newly created surface or an erased area holds before
> drawing (the erase fill colour is a run-time value, zero at start-up; Probable: transparent
> black (0, 0, 0, 0)). **Tessera rule** until a frame capture (M3) settles it (DEC: UR
> Decision 1.1, 1.6 and the "UI pipeline" and "Blending on stored values" rows of its §8): a
> new surface and an erased area hold transparent black; surfaces are composed on stored
> (not sRGB-decoded) values with premultiplied source-over, the straight-alpha canvas texels
> being premultiplied when they are read. At 1 : 1 and opacity 1 this equals straight
> source-over within ±1 per channel and replaces the frame exactly where the surface alpha is
> 255. A scaled or rotated surface is filtered bilinearly on premultiplied texels by default,
> with a Tessera setting for nearest (UR §2.2; `fonts.md` Q3). The surface's alpha is always
> used, which is what makes the minimap round
> (`game-panels.md` §8.7.1). (This replaces the earlier Tessera choice of straight source-over
> blending.)

<a id="s9-6-2"></a>
#### 9.6.2 Per-pixel arithmetic of the base layer (BIN, U37)

Notation, all integer arithmetic on bytes 0–255 unless stated:

- *D* = destination pixel (*D*.A, *D*.R, *D*.G, *D*.B), *S* = source pixel from the surface.
- inc(*x*) = 0 when *x* = 0, otherwise *x* + 1 (so 1–255 become 2–256).
- dec(*x*) = 0 when *x* = 0, otherwise *x* − 1.
- div256(*x*) = *x* ÷ 256 rounded **toward zero** (*x* may be negative).
- mix(*d*, *x*, *k*) = *d* − div256((*d* − *x*) × *k*), stored as its low 8 bits.
- *o* = opacity byte = trunc(`UICore_Element_alphablendmod` × 255.0), 32-bit float, rounded
  toward zero; 255 for the default 1.0.
- "Channels" means R, G and B; *D*.A is unchanged unless the operation says otherwise.

If *o* = 0, no operation changes any pixel. Otherwise, per pixel:

**Weight rules.**

- **W1** (Mix colour, Over, Screen): *a* = *S*.A if *o* = 255, else div256(*S*.A × (*o* + 1)).
  If *a* = 0 the pixel is unchanged; otherwise *k* = *a* + 1 (1 < *k* ≤ 256).
- **W2** (Multiply, Colorize): *k* = inc(*S*.A) if *o* = 255, else div256(inc(*S*.A) × (*o* + 1)).
  If *k* = 0 the pixel is unchanged.

**Operations.**

| Operation | Result |
|---|---|
| Replace | *D* = *S*, all four bytes, whatever *S*.A and *o* are (only *o* = 0 stops it) |
| Mix colour | W1; each channel *D*.c = mix(*D*.c, *S*.c, *k*); *D*.A unchanged |
| Over | W1; then: if *k* = 256 or *D*.A = 0, *D* = *S* (all four bytes, *S*'s own alpha); else if *D*.A = 255, each channel *D*.c = mix(*D*.c, *S*.c, *k*) and *D*.A stays 255; else let *q* = *D*.A + 1 and *r* = *q* + *k* − div256(*q* × *k*); each channel *D*.c = *D*.c − ((*D*.c − *S*.c) × *k* ÷ *r* rounded toward zero), and *D*.A = *r* − 1 (low 8 bits) |
| Multiply | W2; *X*.c = div256(inc(*S*.c) × *D*.c); each channel *D*.c = mix(*D*.c, *X*.c, *k*) |
| Screen | W1; *X*.c = *D*.c + *S*.c − div256(inc(*S*.c) × *D*.c); each channel *D*.c = mix(*D*.c, *X*.c, *k*) |
| Colorize | W2; *X* = the colour with the hue and saturation of *S* and the value of *D* (below); each channel *D*.c = mix(*D*.c, *X*.c, *k*) |
| Grayscale | (no base-layer routine; nothing is drawn) |

**Colorize colour *X*.** With *r*, *g*, *b* = inc(*S*.R), inc(*S*.G), inc(*S*.B):

1. *mx* = max(*r*, *g*, *b*), *mn* = min(*r*, *g*, *b*), *δ* = *mx* − *mn*.
2. If *δ* = 0: saturation *sat* = 0 and hue *h* = 0. Otherwise *sat* = (256 × *δ*) ÷ *mx* rounded
   toward zero, and the hue, in units of 1/4096 of a 60° sector:
   - if *mx* = *r*: *h* = (4096 × (*g* − *b*)) ÷ *δ* rounded toward zero, plus 24,576 when
     *g* − *b* < 0;
   - otherwise if *mx* = *g*: *h* = 8,192 + (4096 × (*b* − *r*)) ÷ *δ* rounded toward zero;
   - otherwise: *h* = 16,384 + (4096 × (*r* − *g*)) ÷ *δ* rounded toward zero.
3. *V* = max(inc(*D*.R), inc(*D*.G), inc(*D*.B)).
4. If *sat* = 0: *X*.R = *X*.G = *X*.B = *V*. Otherwise:
   - *c* = div256(*V* × *sat*), *m* = *V* − *c*;
   - sector *i* = *h* ÷ 4096 rounded toward zero (0–5);
   - *f* = *h* − 8,192 × ((*i* + 1) ÷ 2 rounded toward zero) (the signed distance from the
     nearest even sector boundary, −4096 ≤ *f* < 4096);
   - *t* = *f* × *c* ÷ 4096 rounded toward zero, then corrected by one toward the nearer integer
     when the remainder is greater than 2048 in magnitude (exactly ±2048 is not corrected);
   - (*X*.R, *X*.G, *X*.B) by sector: 0: (*V*, *m* + *t*, *m*); 1: (*m* − *t*, *V*, *m*);
     2: (*m*, *V*, *m* + *t*); 3: (*m*, *m* − *t*, *V*); 4: (*m* + *t*, *m*, *V*); 5: (*V*, *m*, *m* − *t*).
5. Finally *X*.c = dec(*X*.c) for each channel.

Sources without alpha (R8G8B8 surfaces) are read with *S*.A = 255 (BIN for Over; the other
operations have separate routines per source format that were not read, Probable).

> **OPEN:** the routines for source formats other than 32-bit ARGB and 24-bit RGB (JPEG images
> after decoding, alpha-only images), and for destination formats other than 32-bit ARGB. Not
> needed for the panels of §17 as long as Tessera decodes JPEG to opaque RGB and treats it like
> an R8G8B8 source. **Tessera rule** for the other sources (DEC: UR §4.2): every image is
> converted to 32-bit ARGB before it is drawn, and the drawing above uses those pixels;
> sources without alpha get alpha 255; alpha-only sources become black with their 8-bit value
> as alpha; channels of 1–6 bits are widened by bit replication and 10-bit channels keep
> their top 8 bits; DXTn surfaces are decompressed. The source format is kept beside the
> pixels for the overlay rule of §9.6.3.

<a id="s9-6-3"></a>
#### 9.6.3 Per-pixel arithmetic of the overlay layer (BIN, U37, U39)

*C* = the item's colour (§6.9). The element's opacity is not applied to the overlay draw (it
is drawn as if the element's opacity were 255). Without an image the colour's alpha enters as
the source alpha *a′* below; with an image the compositor job takes *C*.A as its opacity.

**Without an image** (item DID 0): the source is *C* with alpha *a′* = ((255 + 1) × (*C*.A + 1))
÷ 256 − 1 = *C*.A, the same for every pixel of the element's rectangle.

| Operation | Result |
|---|---|
| Replace | fill: *D* = (*a′*, *C*.R, *C*.G, *C*.B) |
| Over | as §9.6.2 Over with *S* = (*a′*, *C*.R, *C*.G, *C*.B) and *k* = *a′* + 1; nothing when *a′* = 0 |
| Multiply | *k* = inc(*a′*); as §9.6.2 Multiply with *S*.c = *C*.c |
| Colorize | *k* = inc(*a′*); as §9.6.2 Colorize with *S*.c = *C*.c |
| Grayscale | nothing when *a′* = 0; *k* = *a′* + 1; *V* = max(inc(*D*.R), inc(*D*.G), inc(*D*.B)); each channel *D*.c = mix(*D*.c, *V*, *k*). The colour's R, G and B are not used. Because mix keeps only the low 8 bits, a destination with a channel of 255 and *k* = 256 becomes black (*V* = 256 stored as 0); this is the client's behaviour |

**With an image** (item DID names a 32-bit ARGB surface; all 16 such surfaces in this install
are): per pixel the source is *S* = (*I*.A, *C*.R, *C*.G, *C*.B), where *I* is the image pixel:
the image contributes only its alpha, the colour supplies the channels. The opacity is
*o* = *C*.A, and the weight rules of §9.6.2 use inc(*o*) in place of (*o* + 1) (the same except
that *o* = 0 draws nothing). Multiply and Screen then follow §9.6.2. In this install every such
item has *C*.A = 255.

> **OPEN:** the overlay routines that data does not use (table above), and overlay images in
> formats other than 32-bit ARGB: for an alpha-only image the client has separate routines (not
> read); for any other format it draws nothing.

<a id="s9-6-3-formats"></a>
> **OPEN:** whether a render surface or user-skin image stored in a 32-bit format other than
> A8R8G8B8 (A8B8G8R8, the engine-specific R8G8B8A8 of `skin-definition.md` §7.4) reaches the
> compositor as "32-bit ARGB" after loading, and so draws as an overlay image. Not traced; in
> this install every overlay image is A8R8G8B8 (above). **Tessera rule** (DEC: UR §4.2): only
> A8R8G8B8 sources count as 32-bit ARGB here. An overlay with an image in any other format
> draws nothing, except an alpha-only image, whose 8-bit value is used as *I*.A.

<a id="s9-7"></a>
### 9.7 Button state choice (engine widget type Button)

The button widget picks its state from `UICore_Button_ghosted` (G), `UICore_Button_highlighted`
(H), `UICore_Button_rollover` (rollover enabled, E), `UICore_Button_supports_ghosted_highlight`
(S), whether a mouse button went down on it and is still held (P) and whether the pointer is
over it (O):

| Condition | H = false | H = true |
|---|---|---|
| not G: base | `normal` 0x0B | `highlight` 0x07 |
| not G: P, or (E and O) | `normal_rollover` 0x08 | `highlight_rollover` 0x11 |
| not G: P and O | `normal_pressed` 0x03 | `highlight_pressed` 0x01 |
| G: base | `ghosted` 0x12 | `ghosted` 0x12, or `ghosted_highlight` 0x13 if S |
| G: P, or (E and O) | `ghosted_rollover` 0x14 | `ghosted_rollover` 0x14, or `ghosted_highlight` 0x13 if S |
| G: P and O | `ghosted_pressed` 0x15 | `ghosted_pressed` 0x15 |

Later rows win over earlier ones. The button switches (§9.4) only if it defines the chosen state
in its resolved states table; otherwise it stays in its current state.

<a id="s9-8"></a>
### 9.8 User skins (hook only)

When a user skin is active, the client applies it at two points (BIN, U29):

1. After resolving a descriptor and before creating the element, a skin entry for the element
   (matched by element ID and the path of IDs from the root) is applied to a **per-build
   resolved copy** of the descriptor (never to the loaded layout or to a shared base). It
   replaces the position and size (unless protected by `UISkin_CannotBeMoved` or
   `UISkin_CannotBeResized`), sets all four edge modes to 0 unless the element carries
   `UISkin_CannotBeDetached`, and may set the two scaler-origin properties; it does **not**
   change `has_position` (§5.3), the z-level, the flag byte, the states or the media.
2. When an element's base image is set by DID (Image items, Animation frames, meter images,
   images set by game code), the skin can substitute its own image for the surface (through
   an art-asset table); Coloured-image overlays, hit masks and cursors are not substituted.

The rules are `skin-definition.md` §6.3, §6.8 and §8.2–§8.4. The merge of §7.2–§7.3 has a
separate mode for skin overrides, selected by a marker that descriptors read from layout records never carry: in that mode only
the fields marked in the override are copied. Layout resolution always clears the marker, so it
never affects §7. The skin file format and matching rules belong to
`skin-definition.md`.

<a id="s9-9"></a>
### 9.9 Saved panel locations (BIN, U40, U41)

Players move and resize panels; the client restores them through a mechanism outside the layout
records.

**Which elements.** Three Bool properties of an element select what a saved entry restores
(§5.9): `UICore_Element_save_loc` (position), `UICore_Element_save_size` (size) and
`UICore_Element_save_visibility` (shown or hidden). They are read through the element's
effective properties, so a state can change them. In this install they are true on 192, 34 and
1 elements; most are panel roots or the movable boxes of the activatables layout (§17).

**The saved entry.** Game code keeps a store of entries keyed by (an identifier of the
element's layout, element ID). An entry holds two corners as 32-bit floats in **normalized
screen coordinates** (*x* from −1 at the left edge to +1 at the right edge, *y* from +1 at the
top edge to −1 at the bottom edge): (*x0*, *y0*) for the top-left corner and (*x1*, *y1*) for
the bottom-right corner, plus a visibility byte.

**Applying an entry.** Game code asks the UI manager to apply the store to an element it has
created (two callers seen). If none of the three properties is true, nothing happens. If the
store has no entry for the element, nothing happens. Otherwise, with *W* × *H* the screen
size in pixels:

1. Convert each corner (*nx*, *ny*) to pixels in 32-bit floats: *px* = ((1/*W* + *nx*) + 1) × *W* × 0.5 and
   *py* = (1 − (1/*H* + *ny*)) × *H* × 0.5; then truncate toward zero. (The client subtracts
   the screen surface's origin, which is (0, 0).) This gives (*X0*, *Y0*) and (*X1*, *Y1*).
2. If `UICore_Element_save_visibility` is true: show or hide the element by the stored byte.
3. If `UICore_Element_save_loc` is true: move the element so that its top-left corner is at
   (*X0*, *Y0*).
4. If `UICore_Element_save_size` is true: set the element's size to
   (*X1* − *X0* + 1, *Y1* − *Y0* + 1).
5. Send UI message 0x2E (`Element_saved_location_applied`) to the element's listeners.

Position and size set here go through the element's ordinary move and resize, so edge mode 0
keeps them on later layouts (§8.4 step 4) and children are laid out again when the size changes.

**The file form.** The client can write and read the store as XML: one `Layout` element per
layout identifier, with an attribute `ID` written as `0x` and eight upper-case hexadecimal
digits, holding one `Element` element per entry with the attributes `ID`, `X`, `Y`, `Width` and
`Height`. The four numbers are fractions of the screen with the origin at the top-left corner:
`X` = (*x0* + 1) × 0.5, `Y` = (1 − *y0*) × 0.5, `Width` = (*x1* + 1) × 0.5 − `X`,
`Height` = (1 − *y1*) × 0.5 − `Y`, written as decimal numbers with six digits after the point.
The executable names a folder `ui/layouts` and an extension `.layout` for such files, and the
same node names appear inside the options-profile XML (a `Layouts` node). The visibility byte is
not among the attributes written. (These names are interoperability identifiers of the file
format.)

> **OPEN:** (1) where the client keeps the live store between sessions (no file of this kind
> exists in either settings folder on the owner's machine on 2026-10-06; the store may come
> from the server); (2) whether the layout identifier is the `UILAYOUT` entry key or the
> layout DID, and how the element `ID` attribute is written (the writer formats it through a
> general value formatter that was not read); (3) which panels game code applies the store to
> and when. Tessera keeps its own store of these entries in its own settings (normalized
> corners and visibility per panel), applies it by steps 1–5, and may import `.layout` files
> read-only once (2) is settled. Without a store every panel uses its layout geometry (§8).

---

<a id="s10"></a>
## 10. References from layouts (Confirmed)

Every non-zero DID referenced from a layout exists in the archives (27,512 references). Base
references are excluded here (§7.1). The 1,324 zero references are: Image items with DID 0
(805), Coloured image items with DID 0 (392), Movie items (2), StringInfo values whose table
and token are both 0 (115, all the same pair) and DID-valued properties set to 0 (10). Media
items count once per DID field, StringInfo values once per value (nested values included),
properties once per value (values inside arrays and structs included).

| Target prefix | Record type | Referenced from | References |
|---|---|---|---|
| 0x41 | Render surface (REC §5) | Image (10,560), Animation frames (2,252), Coloured image (29), Cursor (180) items; properties such as meter frames and images, map images, icons, cursors (2,139) | 15,160 |
| 0x25 | String table ([`string-table.md`](string-table.md)) | StringInfo values naming a non-zero table (text, tooltips, labels; 5,362) and DID-valued properties (5) | 5,367 |
| 0x22 | UI layout | Tooltip layouts (2,129), list-box entry templates (1,711), menu and tree templates, override layouts | 4,043 |
| 0x42 | Font ([`fonts.md`](fonts.md)) | `UICore_Text_font` inside font arrays and structs, `UICore_Text_tag_font` | 1,230 |
| 0x70 | Game object ([`wstate.md`](wstate.md)) | Game-level properties (map areas, travel NPCs, items) | 996 |
| 0x2A | Sound info (DB-object type 0x12, `property-system.md` §9) | Sound items (692), one credits property (2) | 694 |
| 0x21 | UI scene (DB-object type 0x43, `property-system.md` §9) | `UICore_Viewport_uiscene` | 20 |
| 0x40 | Texture list ([`texture-list.md`](texture-list.md)) | Viewport opacity mask property | 2 |

Text: 5,478 StringInfo values (5,477 string-table references, 1 literal). The 5,477 references
name 3,329 distinct (table, token) pairs: 3,326 resolve, 2 name a missing token (a lookup miss,
string-table.md), and 1 is the pair (0, 0), used by 115 values (no text). The other 5,362
references all name a 0x25 table.

Files: 13 media items name files by path (11 Image items, JPEG; 2 Movie items, Bink video), all
relative to the install directory and all present.

Fonts and text drawing: [`fonts.md`](fonts.md) §7–§9. Resolved over §7, 6,923 Text-type element
instances would exist if every layout were instantiated in full; 131 of them carry no font
property on the element or any of its states (FO §9.1 rule 4: such text is not drawn).

---

<a id="s11"></a>
## 11. Census of this install (Confirmed, 2026-10-05)

| Quantity | Value |
|---|---|
| Records | 302 (0x22000000–0x22000914, sparse), 107 to 239,417 bytes, 2,610,590 bytes in total, all uncompressed |
| Top-level elements | 3,144 (1 to 450 per record; 111 records have one) |
| Elements | 16,421 (up to 1,533 in one record); nesting depth up to 7 below a top-level element; up to 339 children of one element |
| Typed / based elements | 7,804 / 8,617 |
| States | 12,977 (0 to 139 per element; 13,970 elements have none) |
| Media items | 12,998 (3,280 in element blocks, 9,718 in state blocks); kinds in §6.1 |
| Engine widget types used | Field 4,796, Button 700, Text 346, ListBox 193, Dragbar 135, Meter 88, Panel 73, Tree 59, ResizeBar 52, GroupBox 13, Scrollbar 13, Browser 7, ColorPicker 4, Menu 4, Viewport 1 (Scrollable, ComboBox, Frame and Flash unused) |
| Media per block | elements: 0 (13,199), 1 (3,167), 2 (54), 5 (1); states: 0 (4,069), 1 (8,144), 2 (740), 3 (13), 4 (6), 5 (3), 6 (1), 10 (1) |
| Property entries | 35,615 on elements (835 PIDs), 2,083 on states (44 PIDs) |
| Element property-set hints | 0 in all but one set (hint 1, 23 entries); state sets always 0 |
| Distinct element IDs | 13,035 |
| Default states set | 1,212 |
| `has_position` = 0 | 222 elements |
| Pascal strings (re-measured 2026-10-06) | 11,377: 11,367 media file paths and 10 strings inside property sets. Longest 50 bytes; 12 longer than 47 (ten of 48, one of 49, one of 50). Media paths: Image up to 48 bytes (the ten of 48), Movie 28 and 47; 11,354 of the 11,365 Image paths are empty, so 13 media items name a file (§10). All lengths below 128, so every length VLE is 1 byte |

---

<a id="s12"></a>
## 12. Invariants

Confirmed on every record unless marked.

- **UL1** Parsing per §4 consumes every record exactly.
- **UL2** Every element's own block has `state_id` = 0; every state entry's block has
  `state_id` = its key.
- **UL3** Every element table key equals the element's `element_id`.
- **UL4** Every media item has `kind_tag` = `kind`, and the kind is one of 1, 3–13 (in data:
  1, 4, 5, 6, 8, 9, 10, 11, 12, 13).
- **UL5** Exactly one of `type` and (`base_element`, `base_layout`) is non-zero; `base_element`
  and `base_layout` are zero together.
- **UL6** State blocks have x = y = width = height = z_level = 0 and `has_position` = 0.
- **UL7** All bool8 fields are 0 or 1; flag bytes use only bits 0–1.
- **UL8** Keyed tables are in bucket order with unique keys; state tables use hint 1.
- **UL9** Read orders: top-level elements 0 … *n* − 1, children 1 … *n*, unique among siblings.
- **UL10** Edge modes are in 0–6.
- **UL11** Every element ID, state ID, type, default state, base element ID, draw mode and
  message ID resolves in its enum mapper chain (§3.3).
- **UL12** Every non-zero referenced DID exists (§10).
- **UL13** Every base layout DID is a 0x22 record of the install (all 177); the named base
  element is a top-level element of it in 8,614 of 8,617 cases (§13).
- **UL14** Width and height are never negative.
- **UL15** When a child of a based element overrides an inherited child (§7.3 step 6), both
  are typed with the same type or both are based (all 110 override sites). Observation, not a
  rule of the client: a decoder must accept other pairs and apply §7.3 step 6.
- **UL16** For every panel of §17, the `UILAYOUT` map label exists, its entry names a 0x22
  record of the install, the root element ID is a key of that record's top-level table, and the
  root resolves by §7 with every descendant resolving too (15 of 15 panels, 2026-10-06).
- **UL17** Draw-mode conversion (§9.6.1) is total: every u32 maps to exactly one operation (0 and
  values above 8 to Replace).
- **UL18** For all *d*, *x* in 0–256 and *k* = 256, mix(*d*, *x*, *k*) equals *x* modulo 256; the
  weight rules W1 and W2 (§9.6.2) only ever give 1 ≤ *k* ≤ 256 for a pixel they do not skip.
- **UL19** Over (§9.6.2) leaves *D* unchanged when *S*.A = 0, writes *S* exactly when
  *S*.A = 255 and *o* = 255, and keeps *D*.A = 255 when *D*.A was 255. Replace writes *S*
  exactly whenever *o* > 0. Multiply with *S* = (255, 255, 255, 255) and *o* = 255 leaves *D*
  unchanged.
- **UL20** Saved-location conversion (§9.9 step 1) maps *nx* = −1 to *px* = 0 and *ny* = +1 to
  *py* = 0 for every screen size.
- **UL21** (OBS) The layout labelled `containerdisplay` has the six top-level element labels
  `PlayerBag1_Template` … `PlayerBag6_Template`; each resolves (§7.1) to the game type
  `UIScaler`, and none of them is named by a base reference of another layout's element
  (§17.2 P15, U45).
- **UL22** (OBS) In the 15 instantiated panels of §17, exactly 92 non-root elements have a
  resolved `UICore_Element_ObjectMode`: 86 Stretch (1) and 6 CanvasResize (3), none None (0)
  and none Tile (2); no state block of these panels sets the property; no such element has
  more than 3 surfaced ancestors, where a surfaced ancestor is an ancestor with a non-zero
  resolved object mode, or the root (counted once, whatever its mode) (§17.2, U46).

<a id="s12-1"></a>
### 12.1 Hostile-input notes

- Reject a record whose payload does not end exactly after the last element, any bool8 above 1,
  any unknown media kind, and any count that the remaining bytes cannot hold (§4.7).
- A key/ID mismatch (UL2, UL3, UL4) never occurs; report it as malformed.
- Resolution (§7) must detect cycles and missing bases without recursion limits being reached
  by hostile chains; a depth limit of 32 is ample (observed 3).
- Layout arithmetic (§8) must not overflow: use 64-bit intermediates or saturating 32-bit
  arithmetic; treat NaN from 0 ÷ 0 as 0 as the client does.

---

<a id="s13"></a>
## 13. Failure categories (ROADMAP M5 exit criterion 1)

| Category | Records / elements in this install | Client behaviour (BIN) |
|---|---|---|
| Record does not decode | 0 of 302 | The layout cannot be loaded; elements using it as a base fail |
| Unknown media kind | 0 | Decoding fails (§4.6) |
| Base layout missing | 0 | The element is not created |
| Base element missing from the base layout's top level | 3 elements (in 2 records) | The element (and its subtree) is not created |
| Base chain cycle | 0 | The element is not created |
| Type without a registered widget | depends on the implementation | The element is not created (Tessera: fall back, §9.2) |
| Missing referenced record | 0 | — |
| Missing string-table token | 2 distinct pairs | Lookup miss; no text |

So every layout record decodes, and the only resolution failures are the 3 dangling base
references.

---

<a id="s14"></a>
## 14. Worked synthetic examples

All values are invented for illustration; none is taken from a record.

<a id="s14-1"></a>
### 14.1 A small record (158 bytes)

A layout 0x22FFF001, design size 800 × 600, with one Button element 0x10001234 at (10, 20),
100 × 30, z-level −5, flags 1, edges (5, 5, 4, 4), default state 0x0B, an Image item, and one
state 0x0B with another Image item:

| Offset | Bytes | Field |
|---|---|---|
| 0x00 | `01 F0 FF 22` | did 0x22FFF001 |
| 0x04 | `20 03 00 00` `58 02 00 00` | design 800 × 600 |
| 0x0C | `00 01` | top-level table: hint 0, 1 entry |
| 0x0E | `34 12 00 10` | key 0x10001234 |
| 0x12 | `00 00 00 00` | state_id 0 |
| 0x16 | `01` | flags: pass state to children |
| 0x17 | `0A 00 00 00` `14 00 00 00` `64 00 00 00` `1E 00 00 00` | x 10, y 20, width 100, height 30 |
| 0x27 | `FB FF FF FF` | z_level −5 |
| 0x2B | `01` | has_position |
| 0x2C | `00 00` | empty property set |
| 0x2E | `01` | 1 media item |
| 0x2F | `04 00 00 00` `04 00 00 00` `EF CD AB 41` `04 00 00 00` `00` | Image: DID 0x41ABCDEF, Alphablend, empty path |
| 0x40 | `00 00 00 00` | read_order 0 |
| 0x44 | `34 12 00 10` `09 00 00 00` | element_id, type 9 (Button) |
| 0x4C | `00 00 00 00` `00 00 00 00` | no base |
| 0x54 | `0B 00 00 00` | default_state 0x0B |
| 0x58 | `05 00 00 00` `05 00 00 00` `04 00 00 00` `04 00 00 00` | edges 5, 5, 4, 4 |
| 0x68 | `01 01` | state table: hint 1, 1 entry |
| 0x6A | `0B 00 00 00` `0B 00 00 00` | key 0x0B, state_id 0x0B |
| 0x72 | `00` + 20 zero bytes + `00` | flags 0, zero geometry and z, has_position 0 |
| 0x88 | `00 00` | empty property set |
| 0x8A | `01` | 1 media item |
| 0x8B | `04 00 00 00` `04 00 00 00` `F0 CD AB 41` `04 00 00 00` `00` | Image: DID 0x41ABCDF0 |
| 0x9C | `00 00` | children: hint 0, none |

Created as a root panel on a 1600 × 1200 screen: ΔW = 800, ΔH = 600; edges (5, 5, 4, 4) keep
the design box (10, 20, 109, 49), so the button stays at (10, 20), 100 × 30 (it is pinned to
the top-left corner). Post-creation shows image 0x41ABCDEF, then the switch to state 0x0B
replaces it with 0x41ABCDF0.

<a id="s14-2"></a>
### 14.2 Layout arithmetic

Design area 1024 × 768 placed on a 1920 × 1080 screen: *O* = (0, 0, 1023, 767),
*N* = (0, 0, 1919, 1079), ΔW = 896, ΔH = 312, *Rx* = 1.875, *Ry* = 1.40625.

| Element design rectangle | Edges | Result (x, y, width, height) |
|---|---|---|
| (824, 668, 200, 100) | 4, 4, 5, 5 | (1720, 980, 200, 100): pinned to the bottom-right corner |
| (0, 0, 1024, 40) | 5, 5, 5, 4 | (0, 0, 1920, 40): full-width bar |
| (362, 284, 300, 200) | 1, 1, 1, 1 | (810, 440, 300, 200): centred (half(1920) − half(300) = 810) |
| (100, 50, 200, 100) | 3, 3, 3, 3 | (187, 70, 374, 140): left trunc(187.5) = 187, right trunc(299 × 1.875) = 560, top trunc(70.31) = 70, bottom trunc(149 × 1.40625) = 209 |
| (0, 0, 1024, 768) | 6, 5, 6, 5 | first pass: left trunc(960 − 1024 × 1.40625 × 0.5) = 240, right trunc(960 + 720) − 1 = 1679, top 0, bottom 1079; aspect pass with *A* = 1024 ÷ 768: left trunc(1679 − 1080 × 1.3333) = 239, right trunc(239 + 1440) − 1 = 1678 → (239, 0, 1440, 1080) |

<a id="s14-3"></a>
### 14.3 Inheritance

Template *T* (typed, Button): rectangle (0, 0, 120, 30), media [Image 0x41000010], properties
{`UICore_Text_entry` = a, `UICore_Button_rollover` = true}, children {label L: (5, 5, 110, 20),
edges (5, 5, 5, 5), read order 1}, state 0x08 with media [Image 0x41000011].

Element *E* based on *T*: rectangle (40, 60, 200, 30), media [], properties
{`UICore_Text_entry` = b}, children {icon I: read order 1}, state 0x08 with media
[Sound 0x2A000005], edges (4, 5, 5, 4).

Resolved *E*: type Button; rectangle (40, 60, 200, 30); edges (4, 5, 5, 4); media
[Image 0x41000010]; properties {entry = b, rollover = true}; state 0x08 media
[Image 0x41000011, Sound 0x2A000005]; children: L re-laid out from *T*'s 120 × 30 to 200 × 30
with edges (5, 5, 5, 5): (5, 5, 190, 20); I with read order 1 + 1 = 2 (one inherited child).

<a id="s14-4"></a>
### 14.4 Button states

A button with `UICore_Button_rollover` = true, not ghosted, not highlighted: the pointer moves
over it → `normal_rollover` (0x08); the mouse button goes down → `normal_pressed` (0x03); the
pointer leaves while the button is held → `normal_rollover`; released outside → `normal`
(0x0B). Each switch happens only if the button defines that state.

<a id="s14-5"></a>
### 14.5 Overriding a based child (§7.3 step 6)

Layout 0x22FFF030 holds the top-level elements *X* (typed, Text, media [Image 0x41000030]) and
*Y* (typed, Text, media [Image 0x41000031]).

Template *P* (typed, Field, rectangle (0, 0, 100, 40)) has one child *K*, key 0x10000500: based
on (*X*, 0x22FFF030), rectangle (0, 0, 50, 20), media [Image 0x41000040], read order 1.

Element *E* (based on *P*, rectangle (0, 0, 100, 40), so no size change) has one child with the
same key 0x10000500: based on (*Y*, 0x22FFF030), rectangle (5, 5, 60, 20), media
[Sound 0x2A000007], read order 1.

Resolving *E* (§7.1–§7.3): *R* starts as a copy of *P*. The child key matches, so *E*'s child
is merged into *P*'s child *K* without resolving either. The merged child keeps *K*'s base
reference (*X*, 0x22FFF030); *E*'s child's base (*Y*) is ignored. It takes rectangle
(5, 5, 60, 20) and read order 1 from *E*'s child, and its media list is
[Image 0x41000040, Sound 0x2A000007].

Creating *E* (§9.2): the merged child is resolved when it is created. Its base *X* is typed, so
the result is: type Text, rectangle (5, 5, 60, 20), media
[Image 0x41000030, Image 0x41000040, Sound 0x2A000007]. The image shown after the media list
runs is 0x41000040; nothing from *Y* is used.

<a id="s14-6"></a>
### 14.6 Drawing order and `UICore_Element_DrawAfterChildren`

Element *P* has four children: *A* (z-level 10, read order 1), *B* (z 0, read order 2),
*C* (z 0, read order 1) and *E* (z −5, read order 3). The child list (§9.3) is *A*, *C*, *B*,
*E*. Drawing *P* with `UICore_Element_DrawAfterChildren` false: *P*'s own images, then *A*
(with its subtree), *C*, *B*, *E*; *E* ends up in front. A click inside all four goes to *E*
first (hit testing walks *E*, *B*, *C*, *A*), and to *P* itself only if no child takes it and
*P* is mouse-visible or blocks clicks (§9.3, `ui-behaviour.md` §4.3). With the property true on *P*: *A*, *C*, *B*, *E*
first, then *P*'s own content on top of them.

<a id="s14-7"></a>
### 14.7 Per-pixel arithmetic (§9.6.2, §9.6.3)

Bytes are written (A, R, G, B). All values invented.

| Operation | *D* | *S* (or colour) | *o* | Result | Working |
|---|---|---|---|---|---|
| Over | (128, 200, 100, 0) | (128, 0, 0, 255) | 255 | (192, 67, 34, 170) | *a* = 128, *k* = 129; *q* = 129, *r* = 129 + 129 − div256(16,641) = 193; R: 200 − (200 × 129 ÷ 193 → 133) = 67; G: 100 − 66 = 34; B: 0 − (−255 × 129 ÷ 193 → −170) = 170; A = 192 |
| Multiply | (255, 200, 100, 40) | (255, 128, 64, 255) | 255 | (255, 100, 25, 40) | *k* = 256; *X* = (div256(129 × 200), div256(65 × 100), div256(256 × 40)) = (100, 25, 40) |
| Multiply | same | same | 127 | (255, 150, 63, 40) | *k* = div256(256 × 128) = 128; R: 200 − div256(100 × 128) = 150; G: 100 − div256(75 × 128) = 63; B unchanged |
| Screen | (255, 200, 100, 40) | (191, 128, 64, 255) | 255 | (255, 221, 129, 201) | *a* = 191, *k* = 192; *X* = (200 + 128 − 100, 100 + 64 − 25, 40 + 255 − 40) = (228, 139, 255); R: 200 − div256(−28 × 192) = 221; G: 100 − div256(−39 × 192) = 129; B: 40 − div256(−215 × 192) = 201 |
| Colorize | (255, 100, 50, 20) | (255, 40, 120, 200) | 255 | (255, 20, 60, 100) | *r*, *g*, *b* = 41, 121, 201; *δ* = 160; *sat* = 203; *mx* = *b*: *h* = 16,384 + (4096 × −80 ÷ 160) = 14,336, sector 3; *V* = 101; *c* = 80, *m* = 21; *f* = −2048, *t* = −40; sector 3 gives (21, 61, 101), minus one each |
| Grayscale (overlay, no image) | (255, 200, 100, 40) | colour alpha 0xBF | — | (255, 200, 175, 160) | *a′* = 191, *k* = 192, *V* = 201; G: 100 − div256(−101 × 192) = 175; B: 40 − div256(−161 × 192) = 160; R: 200 − div256(−1 × 192) = 200 |
| Grayscale (overlay, no image) | (255, 255, 10, 10) | colour alpha 0xFF | — | (255, 0, 0, 0) | *k* = 256, *V* = 256, stored as 0 (§9.6.3) |

<a id="s14-8"></a>
### 14.8 Saved panel location (§9.9)

Screen 1920 × 1080; an entry with corners (−0.5, 0.5) and (0.0, −0.25); all three save
properties true. In 32-bit floats: *px0* = ((1/1920 − 0.5) + 1) × 1920 × 0.5 = 480.5,
*py0* = (1 − (1/1080 + 0.5)) × 1080 × 0.5 = 269.5, *px1* = 960.5, *py1* = 674.5. Truncated:
(480, 269) and (960, 674). The element is shown or hidden by the stored byte, moved to
(480, 269) and resized to 481 × 406. In the file form the same entry reads `X` = 0.250000,
`Y` = 0.250000, `Width` = 0.250000, `Height` = 0.375000.

---

<a id="s15"></a>
## 15. Corrections to, and agreements with, other specs and documents

Recorded here; the other files are not edited by this session. Rows marked **Agreement** need
no change in the target; they record the evidence this spec adds.

| Target | Statement there | Finding (this spec) | Basis |
|---|---|---|---|
| `records.md` §2.1, row 0x22 | "UI layouts", status R, BIN | Decoded completely: 302 of 302 records, zero leftover bytes (§4). Status C, spec `ui-layout.md`. | OBS, BIN |
| `records.md` §2.1, row 0x2A | Sound-info records (DB-object type 0x12); layout OPEN; status BIN | **Agreement.** Layouts use 0x2A DIDs as sound info: Sound items (692) pass them to the client's UI sound player, and all 694 references from layouts resolve (§10). This spec adds no layout facts for 0x2A records. | OBS, BIN |
| `records.md` §2.1, row 0x21 | UI scenes (DB-object type 0x43); layout OPEN; status BIN | **Agreement.** The only layout property naming 0x21 DIDs is `UICore_Viewport_uiscene` (20 references, all resolve), so these are the 3D scenes shown in viewport elements. This spec adds no layout facts for 0x21 records. | OBS |
| `docs/ARCHITECTURE.md` §15 | Layout records hold "rectangles, colours, per-state images"; UIData variants OPEN | The u32 after the rectangle is the **z-level**, not a colour (§5.3). Media variants are specified in §6. The font record type is 0x42 (`fonts.md`). | BIN, OBS |
| `property-system.md` §4.7 | Colour (code 16) is u32 0xAARRGGBB, bytes B, G, R, A (BIN; Confirmed) | **Agreement, with more evidence.** Under that order the 2,864 colour values in layout properties form the expected UI palette (pale gold, gold, yellow, orange and red text), while R, G, B, A would make most text blue or cyan; the colour of Coloured image items (§6.9) uses the same 0xAARRGGBB order. | OBS, BIN |
| `install-layout.md` §3.6 (OPEN "whether the client reads anything from `raw/` beyond videos and splash images") | `raw/<lang>/…` holds loose media | The UI layouts name exactly 13 files there by path relative to the install directory: 11 JPEG images (Image items, §6.2) and 2 Bink videos (Movie items, §6.9); the localized record names its own language folder. No other layout field refers to `raw/`. | OBS (all 13 exist), BIN |
| `lua-plugin-api.md` §14.2 (art, "see `ui-layout.md`, OPEN there") | Where native art comes from | Native art is render surfaces (0x41) named by layout media items and properties (§6, §10), drawn tiled at native size (§9.6); user skins replace them (§9.8). | BIN, OBS |
| `skin-definition.md` §14 OPEN-1 (saved panel positions; cites `ui-layout.md` §9.2 step 5 as the per-user saved panel data) | The client consults per-user saved panel data while it creates an element | **Correction of the cited fact.** §9.2 step 5 is the mouse-listener check, not saved data (U33). Saved locations are applied by game code after creation, through §9.9, and only for elements whose save properties are true; whether they then override a skin's geometry is still the owner test that OPEN-1 describes (the apply step moves and resizes the element after the skin-adjusted descriptor was built, so a saved entry would win; Probable). | BIN. Applied in `skin-definition.md` OPEN-1 (consolidation revision). |
| `skin-definition.md` §13 (correction of §5.3 and §9.8 here: the override does not set `has_position`, zeroes the edge modes, may set the scaler origin, acts on a per-build copy) | — | **Applied** in §5.3 and §9.8 (consolidation revision, U29). | BIN (SK33, SK34) |
| `ui-behaviour.md` §4.3 OPEN (how the mouse-button flag of §9.2 step 5 maps onto the hit-test flags) | — | **Answered there** (UB revision 2): the flag is mouse-visible; the "second flag" is blocks clicks; §9.2 step 5 and §9.3 here now refer to UB §4.3. | BIN |
| `skin-definition.md` §8.6 and D23 (bags = `containerdisplay` / `PlayerBag1_Template`) against §17.2 P15 here (`backpack` / `RealBackpackField`) | Two different bag panels | **Resolved** in favour of `containerdisplay` / `PlayerBag1_Template`, the bag window the original opens (U45); §17 here changed, SK D23 now names this catalogue. | BIN, OBS |
| `lua-plugin-api.md` Appendix A (`BlendMode` values equal the draw modes) | Blend-mode names and values | **Agreement, with arithmetic.** The draw-mode values are converted to the compositor operations of §9.6.1; §9.6.2 gives their arithmetic for native layers. Whether plugin controls use the same compositor path is for that spec to establish. | BIN |
| `ui-behaviour.md` §9.2.1 (element resize of a surface-owning element in mode None: the layout box is not changed) | — | **Recorded in UB §9.2.1 OPEN; agreement.** The conflict with `lua-plugin-api.md` §13.19 condition 1 (a mode-0 element owns no surface), raised by decision 0015 (spec question 14), is now an OPEN in `ui-behaviour.md` §9.2.1 (whether the original can reach the None branch at all), with the same Tessera rule as §8.4 OPEN (scope) here: an element in mode 0 resizes by the no-surface rule of UB §9.2.1 step 3. | BIN (both specs); DEC (UR §2.1, Decision 1.1) |
| `lua-plugin-api.md` §13.18, mode-1 row (children of a resized Stretch plugin control are not laid out again) | Earlier text gave §8.4 step 1 here as the reason | **Correction of the citation; applied in the working-tree `lua-plugin-api.md` §13.18, pending its review.** Under the Tessera choice of the §8.4 scope OPEN, step 1 applies only under a root-panel parent, so it is not the reason; the reason is that an element resize of a Stretch surface keeps the layout box (`ui-behaviour.md` §9.2.1). The result for the plugin control is unchanged. | DEC; Tessera choice |
| LC `UILayoutLoader` and its summary in the research notes (Reported sources only) | Movie item ends with a u32; kind 6 "animate value"; kind 13 "scale" (two floats); the u32 after the rectangle is a colour (notes) or skipped (LC); media count is a u8 | Movie ends with a bool8 (LC fails on the only record that has Movie items); kind 6 fades the opacity; kind 13 is a random wait in seconds; the u32 is the z-level; the count is a VLE (identical on data, all counts < 128). | BIN, OBS |

---

<a id="s16"></a>
## 16. Open questions (summary)

Every item has an in-section `> **OPEN:**` callout; this list only collects them.

1. Archive version source for the Image item's file path (§6.1).
2. Jump item index arithmetic (§6.8); unused in data.
3. Which code assigns an element a reference box (§8.3).
4. ~~Blend equations of the draw modes~~ (closed 2026-10-06, §9.6.1–§9.6.3). Remaining: how panel
   surfaces are composed onto the frame and what an erased area holds (§9.6.1; Tessera rule
   from decision 0015); the overlay opacity question (§9.6); routines for other source formats
   and for unused overlay combinations (§9.6.2, §9.6.3; Tessera rules from decision 0015).
5. ~~Drawing traversal of the child list~~ (closed 2026-10-06, §9.3). Remaining: the rectangle
   that `UICore_Element_clip_to_parent` selects (§9.3).
6. Behaviour of the remaining engine-level properties and of each widget class (§5.9); trace
   per widget as panels need it.
7. Object modes None, Tile and CanvasResize beyond the layout rule of §8.4 step 1 (§8.4;
   Tessera rules from decision 0015).
8. Which panels the client opens at start-up, in which order, and which only on demand (§9.1).
   How game code picks each panel's layout and element is closed (§9.1, U31).
9. `PRELOADRESOURCES` use and the one unreferenced layout (§3.2). The use of `UILAYOUT` is
   closed (§3.2, U31).
10. Readers of `has_position` outside the layout code (§5.3).
11. Which non-root elements own a drawing surface, so that a Fade on them is visible (§6.6;
    partial answers from `lua-plugin-api.md` §13.19, `ui-widgets.md` §4.5 and
    `game-panels.md` §8.2, and the per-panel census of §17.2).
12. ~~What the client does when the screen size changes~~ (closed 2026-10-06 by `ui-behaviour.md` §9.4; §8.5).
13. ~~The per-element table consulted at creation~~ (closed 2026-10-06: the listener registry,
    §9.2 step 5). Saved panel locations: the live store, the layout identifier and the element
    `ID` text of the file form, and which panels use them (§9.9).
14. ~~The effect of `UICore_Element_DrawAfterChildren`~~ (closed 2026-10-06, §9.3, §9.6).
15. Per-panel game behaviour that §17 lists as OPEN: the chat windows' element IDs, the
    drag-box frame state in normal play, map and minimap art chosen by code, the visible set of
    main-menu buttons (§17.5). The bag panel's open path is closed (U45, §17.2 P15).
16. Which descendants `UICore_Element_ObjectOpacity` (and so a Fade item) reaches when they own
    a surface of their own (§5.9; Tessera rule: own opacity only, decision 0015).
17. The effect of `UICore_Element_ShouldEraseBackground` (§5.9; Tessera rule: always erase,
    decision 0015).
18. The scope of the object-mode test of §8.4 step 1 against the surface rule of
    `lua-plugin-api.md` §13.19, and the unreachable None branch of `ui-behaviour.md` §9.2.1
    (§8.4; Tessera rules from decision 0015, and for question (1) Tessera's own choice: the
    Stretch-parent test only for root parents; an owner screenshot of P10's title bar settles
    it). Also: whether the alpha-test meter's surface flag (`ui-widgets.md` §4.5) is the
    "flagged by code" of §8.3 and §8.4 step 1 (Tessera: it is not).
19. How an element that owns a surface below a root panel is composed into or over its parent:
    clip, order, opacity (§9.3; Tessera rule from decision 0015).
20. Whether 32-bit images in a byte order other than A8R8G8B8 count as 32-bit ARGB for the
    overlay rule (§9.6.3; Tessera rule from decision 0015).
21. The canvas size of a native Tile surface: the layout box (decision 0015 §2.1) against the
    base image or a code-set size (`lua-plugin-api.md` §20, BIN) (§8.4; not relevant to M5).

None of the owner-observation items O1–O17 concerns UI layouts.

---

<a id="s17"></a>
## 17. Native panels for the walker (ROADMAP M5 exit criterion 2)

Fifteen panels that the walker can show with sandbox data. All counts and geometry are
observations of this install (2026-10-06, U42). The `UILAYOUT` labels, element-ID labels,
widget-type labels and property names are identifiers from the user's own data (enum mappers
and DID maps, §3.3) and are used only to find the records; the entry keys, layout DIDs and
element IDs next to them are this install's values and must be looked up at run time.

<a id="s17-1"></a>
### 17.1 Opening a panel in Tessera

1. Find the `UILAYOUT` entry by its label (§3.2) and take its layout DID. Find the root element
   ID by its label in the `UIElementID` mapper chain (§3.3); it must be a key of the layout's
   top-level table (UL16). In this install every `UILAYOUT` label and every `UIElementID` label
   is unique, so each lookup gives one value.
2. Create it as a root panel by §9.1 steps 1–5 (resolution §7, creation §9.2, layout §8 from
   the layout's design size to the screen).
3. Apply Tessera's own saved location for the panel, if any, by §9.9.
4. Elements whose type is a game-level widget type (0x1000xxxx) get Tessera's fallback widget
   (§9.2 note) that draws images and text; Tessera's own code fills the fields listed under
   "Data" from the sandbox.
5. Run the media player and states (§9.4–§9.5); buttons follow §9.7.

<a id="s17-2"></a>
### 17.2 Catalogue

"Open path" names the entry point of §9.1 that the original client uses (BIN, U31, U32):
**Open** = open-panel entry with both values as immediates at a call site; **Window** = a
window class of game code that calls **Open** (P10 and P13 as described in their rows; for
P15 the bag-window class, U45); **Loop** = the activatables loop; **Create** = the
create-element entry.

| # | Panel | `UILAYOUT` label (key → layout DID) | Root element label (ID) | Root type | Open path | Design rect (x, y, w, h); edges; flag byte; z | Save properties |
|---|---|---|---|---|---|---|---|
| P1 | Minimap | `gameplayactivatables` (0x10000051 → 0x22000012) | `GamePlay_RadarBox` (0x10000382); its child `GamePlay_Radar` is based on `MinimapWindow` (0x100001EF) of `radar` (0x1000003C → 0x2200003A) | `HiddenDragBox` (game) | Loop | (822, 12, 224, 280); (4, 5, 5, 4); 0; 0 | loc |
| P2 | Chat window | `chatwindow` (0x10000007 → 0x22000034) | `NewChatWindow1` (0x1000031E), based on `NewPrimaryChatWindowTemplate` | `ChatWindow` (game) | Create (every call site seen passes no parent and takes the element ID from a variable; OPEN which windows) | (5, 444, 313, 199); (5, 4, 4, 5); 0; 0 | loc, size |
| P3 | Toolbar | `toolbar` (0x10000042 → 0x22000033) | `ToolbarField` (0x10000B16) | `UIScaler` (game) | Open | (0, 568, 1024, 200); (1, 4, 1, 5); 2 (keep inside); 0 | — |
| P4 | Player vitals | `toolbar` (same) | `AvatarPanel` (0x10000B46) | `HiddenDragBox` (game) | Open | (2, 2, 280, 300); (5, 5, 4, 4); 0; 1 | loc |
| P5 | Target vitals | `toolbar` (same) | `OpponentPanel` (0x10000B56) | `HiddenDragBox` (game) | Open | (279, 2, 305, 300); (5, 5, 4, 4); 0; 0 | loc |
| P6 | Main menu | `mainmenu` (0x10000053 → 0x22000071) | `MainMenuField` (0x1000042A) | `UIScaler` (game) | Open | (431, 257, 233, 279); (1, 1, 1, 1); 1; −100 | — |
| P7 | Options | `options` (0x10000038 → 0x22000063) | `BaseOptionsField` (0x100004CB) | `UIScaler` (game) | Open | (224, 120, 577, 534); (0, 0, 0, 0); 0; 0 | loc |
| P8 | Social | `social` (0x10000073 → 0x2200004E) | `Social_Field` (0x10000ABE) | `UIScaler` (game) | Open | (262, 118, 587, 577); (0, 0, 0, 0); 1; 0 | loc |
| P9 | Quest log | `quest` (0x10000047 → 0x22000043) | `QuestPanel` (0x1000092C) | `UIScaler` (game) | Open | (9, 118, 770, 550); (0, 0, 0, 0); 0; 0 | loc, size |
| P10 | World map | `map` (0x10000041 → 0x22000041) | `MapPanel` (0x10004970) | `UIScalerPanel` (game) | Window (the key is held by the window object; `map` is the only layout with this element) | (0, 0, 1038, 802); (0, 0, 0, 0); 1; 0 | loc, size |
| P11 | Quest tracker | `gameplayactivatables` (same as P1) | `GamePlay_QuestTrackerBox` (0x1000038B) | `HiddenDragBox` (game) | Loop | (670, 223, 340, 550); (4, 5, 5, 4); 0; 0 | loc |
| P12 | Plugin manager | `pluginmanager` (0x100000DA → 0x2200087E) | `PluginManager` (0x10002D6A) | `UIScalerPanel` (game) | Open | (0, 0, 600, 400); (0, 0, 0, 0); 0; 0 | — |
| P13 | Character journal | `character_journal` (0x1000001C → 0x2200003D) | `CharacterJournalPanel` (0x1000284D) | `UIScalerPanel` (game) | Window (immediate pair) | (0, 0, 500, 580); (0, 0, 0, 0); 0; 0 | loc |
| P14 | Extra shortcut bar | `gameplayactivatables` (same as P1) | `GamePlay_Undocked_CtrlBar` (0x10000378) | `HiddenDragBox` (game) | Loop | (0, 650, 420, 55); (0, 0, 0, 0); 1; 0 | loc |
| P15 | Bag (the first of six bag windows) | `containerdisplay` (0x10000052 → 0x22000074) | `PlayerBag1_Template` (0x10003A9B), based on `InventoryBag_Template` of the same layout; the other five windows are `PlayerBag2_Template` … `PlayerBag6_Template` | `UIScaler` (game) | Window (a bag-window class opens the key with the element ID its creator passes; one game routine creates the six windows with the six IDs as immediates, U45) | (599, 164, 197, 183); (0, 0, 0, 0); 0; 0 | loc |

The rectangles, edges, flag bytes and z-levels are those of the resolved root (§7). The root
of every panel resolves, and so does every descendant (UL16).

| # | Elements (resolved instances) | Text elements | String refs (distinct) | Fonts (distinct) | Art surfaces (distinct 0x41) | States | Game widget types needing a fallback (count) |
|---|---|---|---|---|---|---|---|
| P1 | 51 | 3 | 11 | 2 | 167 | 166 | `HiddenDragBox` 1, `Radar` 1, `MapFogOfWar` 1, `TimeOfDayDisplay` 1, `MoodDisplay` 1, `TutorialHighlighter` 2, two housing buttons |
| P2 | 19 | 1 | 0 | 6 | 13 | 26 | `ChatWindow`, `ChatWindowTabListBox`, `PlayerChatTextEntry` (1 each) |
| P3 | 216 | 4 | 6 | 3 | 150 | 377 | `UIScaler` 1, `QuickslotBar` 1, `LevelProgressMeter` 1, `PipDisplay` 6, `AutoAttackButton` 1, `ToolbarQuickslotPageChanger` 1, `TutorialHighlighter` 23 |
| P4 | 63 | 5 | 11 | 3 | 313 | 374 | `HiddenDragBox` 1, `TargetViewport` 1, `PropertyDisplay` 3, `RankInsigniaDisplay` 1, `EffectDisplayList` 1, `HideWhenField` 2 |
| P5 | 62 | 6 | 10 | 3 | 572 | 510 | `HiddenDragBox` 1, `MultiVitalDisplay` 1, `TargetViewport` 1, `ClassDisplay` 1, `PropertyDisplay` 1, `RankInsigniaDisplay` 1, `EffectDisplayList` 1, `HideWhenField` 1, `TutorialHighlighter` 1, two medal icons |
| P6 | 66 | 1 | 18 | 3 | 36 | 344 | `UIScaler` 1 |
| P7 | 593 | 3 | 42 | 4 | 58 | 1,939 | `UIScaler` 1 |
| P8 | 881 | 61 | 185 | 8 | 106 | 2,594 | `UIScaler` 1, two lock icons |
| P9 | 204 | 16 | 51 | 14 | 122 | 739 | `UIScaler` 1, `QuestTreeDisplay` 1, `QuestDisplay` 1, `HideWhenField` 1 |
| P10 | 30 | 1 | 4 | 2 | 25 | 44 | `UIScalerPanel` 1 |
| P11 | 18 | 1 | 4 | 1 | 12 | 59 | `HiddenDragBox` 1, `QuestTracker` 1 |
| P12 | 167 | 10 | 15 | 8 | 93 | 319 | `UIScalerPanel` 1 |
| P13 | 97 | 12 | 10 | 5 | 77 | 224 | `UIScalerPanel` 1, `PedigreeDisplay` 4 |
| P14 | 22 | 1 | 5 | 1 | 16 | 68 | `HiddenDragBox` 1, `QuickslotBar` 1 |
| P15 | 61 | 2 | 7 | 3 | 47 | 67 | `UIScaler` 1, `InventoryDisplay` 1, `InventoryToolbar` 1, `BuyNowButton` 1 |

All counts are taken on the instantiated panel: every element resolved by §7, so that a
derived property replaces the base property with the same PID (§7.2 step 3) and a replaced base
value is not counted. "String refs" counts distinct StringInfo table references (table, token)
in any property of any element or state block, including values inside arrays and embedded
property sets; literal strings are not counted. "Fonts" counts distinct font DIDs (prefix 0x42)
in DID-valued properties, found the same way. "Art surfaces" counts the distinct render surfaces (prefix 0x41) named
anywhere in the panel: Image, Coloured-image and Cursor items (the cursor's image, §6.5) of
elements and states, animation frames, and DID-valued properties; not all are visible at once.
Button and Field counts dominate the element counts.

<a id="s17-2-modes"></a>
**Non-root elements with an object mode** (OBS, U46, UL22; asked for by decision 0015, which
gives each of them a surface of its own, §6.6, §9.3 OPEN). Counted on the same instantiated
panels; the root itself is not counted. "Surfaced ancestors" is the largest number of
ancestors above one such element that have a non-zero resolved object mode, or are the root
(the root counts once, whatever its mode; the same definition as UL22). In Tessera each
non-root one owns a surface and the root is drawn through its own surface or, in mode 0,
through the stand-in surface of §8.4 (DEC: UR Decision 1.1), so the column is also the number
of surfaces above the element. "Hidden by game code" uses the
normal-play rules of `game-panels.md` §3 (drag-box frame children §4, tutorial highlighters
§5, radar children §8.2).

| # | Count | Stretch (1) | CanvasResize (3) | Surfaced ancestors | Hidden by the layout (`UICore_Element_hide`) | Hidden by game code in normal play | Which elements |
|---|---|---|---|---|---|---|---|
| P1 | 18 | 17 | 1 | 2 | 0 | 6 | the drag bar (3) and title of the drag box; `GamePlay_Radar`; inside it `Minimap_Frame`, `MinimapPlayerMarker`, `RadarQuestObjective`, `AreaText_QuestHighlight`, the two labels, `TimeOfDay`, `Mood`, five buttons and two tutorial highlighters |
| P2 | 0 | | | | | | |
| P3 | 53 | 53 | 0 | 1 | 15 | 23 | all 23 tutorial highlighters; per shortcut slot (15) a new-info highlight and a darkening overlay, the overlays hidden by the layout |
| P4 | 4 | 2 | 2 | 2 | 0 | 2 | the drag bar (3) and title; `VitalsParent`; the portrait `PlayerPortrait` (3) inside it |
| P5 | 6 | 5 | 1 | 3 | 0 | 3 | the drag bar (3) and title; `VitalsParentOpponent`; the portrait `OpponentPortrait` and the Field `Target_Image` inside it; one tutorial highlighter |
| P6, P7, P9, P12, P13, P15 | 0 | | | | | | |
| P8 | 1 | 1 | 0 | 1 | 0 (its ancestor `GuildMembers_Field` is) | 0 | one header button inside `GuildMembers_Field` of the kinship page |
| P10 | 4 | 4 | 0 | 1 | 0 | 0 | the title bar, two toggle buttons and the resize-bar image |
| P11 | 3 | 2 | 1 | 1 | 0 | 2 | the drag bar (3) and title; `GamePlay_QuestTracker` |
| P14 | 3 | 2 | 1 | 1 | 0 | 2 | the drag bar (3) and title; `CtrlBar` |
| **All** | **92** | **86** | **6** | at most 3 | 15 | 38 | none in mode None (0) or Tile (2); no state block sets a mode |

These are data facts. Whether the original gives every one of these elements a surface of its
own is the §8.4 scope OPEN; in Tessera each of them that is shown owns one (DEC: UR Decision
1.1), so they bound where Tessera's surface contents are not byte-exact (UR Decision 1.7).

<a id="s17-3"></a>
### 17.3 Default geometry of the roots

Computed by §8.4 from each layout's design size (1024 × 768 for all fifteen layouts) to the
screen, with no saved location and no user skin. Values are (x, y, width, height) in screen
pixels.

| # | 1024 × 768 | 1920 × 1080 | 2560 × 1440 |
|---|---|---|---|
| P1 | (822, 12, 224, 280) | (1718, 12, 224, 280) | (2358, 12, 224, 280) |
| P2 | (5, 444, 313, 199) | (5, 756, 313, 199) | (5, 1116, 313, 199) |
| P3 | (0, 568, 1024, 200) | (448, 880, 1024, 200) | (768, 1240, 1024, 200) |
| P4 | (2, 2, 280, 300) | same | same |
| P5 | (279, 2, 305, 300) | same | same |
| P6 | (396, 245, 232, 278) | (844, 401, 232, 278) | (1164, 581, 232, 278) |
| P7 | (224, 120, 577, 534) | same | same |
| P8 | (262, 118, 587, 577) | same | same |
| P9 | (9, 118, 770, 550) | same | same |
| P10 | (0, 0, 1038, 802) | same | same |
| P11 | (670, 223, 340, 550) | (1566, 223, 340, 550) | (2206, 223, 340, 550) |
| P12 | (0, 0, 600, 400) | same | same |
| P13 | (0, 0, 500, 580) | same | same |
| P14 | (0, 650, 420, 55) | same | same |
| P15 | (599, 164, 197, 183) | same | same |

P6 shows the centring rule with an odd design size: at 1024 × 768, edges (1, 1, 1, 1) give
left = half(1024) − half(233) = 396 and right = half(1024) + half(233) − 1 = 627, so the width is
232, one pixel less than the design width (§8.4 step 2), even at the design screen size.

Roots with edges (0, 0, 0, 0) keep their design position at every screen size; in the original most of them carry save properties, so
in a player's client they usually sit where the player left them (§9.9), and game code may
also place some of them when it opens them (not traced). The checklist therefore compares
their sizes always and their positions only when the original has no saved location for them.

<a id="s17-4"></a>
### 17.4 Data each panel needs, and what the sandbox provides

| # | Data the panel shows | Sandbox (M5) |
|---|---|---|
| P1 | Player position and heading (marker), the minimap image of the current area, fog of war, time of day, mood (a gauge), housing shortcuts | Position, heading and world time: yes (walker). Minimap image: OPEN (chosen by game code; Tessera draws the frame without it until settled). Fog of war, mood, housing: not provided; those children are hidden by Tessera |
| P2 | Chat lines by channel, tabs, the text-entry line | Yes: Tessera's own system lines and local echo of typed text; one tab |
| P3 | Shortcut slots, experience meter, class resource pips, combat or non-combat state (default state `toolbar_noncombat`), menu buttons | Slots: Tessera's own actions or empty; meter: a fixed sandbox value; pips: hidden; state: non-combat; menu buttons open the other panels here |
| P4 | Name, level, morale and power meters, effects, a 3D portrait (`TargetViewport`) | Name, level and full meters: yes; effects: empty; portrait: optional (M4 figure rendered into the viewport; composed as external content by the Tessera rule of `ui-widgets.md` §12, DEC: UR Decision 1.7), otherwise empty. `PlayerPortrait` also has object mode 3 and no media (§17.2, U47), so it owns a CPU surface as well; how the two are ordered is the `ui-widgets.md` §12 OPEN and Tessera rule on surface-owning viewports |
| P5 | The selected target's name, level, vitals, class, portrait, effects | Yes when the player selects a sandbox figure or himself; hidden when nothing is selected (Tessera's choice; the original's rule is game code). Portrait (`OpponentPortrait`): optional, as for P4, composed as external content by the Tessera rule of `ui-widgets.md` §12 (DEC: UR Decision 1.7). It has object mode 1 and its own image (draw mode 2), and its child `Target_Image` also has mode 1 (§17.2, U47), so both own CPU surfaces as well; their order against the external content is the `ui-widgets.md` §12 OPEN and Tessera rule on surface-owning viewports |
| P6 | Fixed buttons. The button field holds eight text buttons (Buttons whose left, middle and right frame pieces are children) and a close box (a Button without text). Two of the text buttons resolve with `UICore_Element_hide` true, so six text buttons and the close box show by default; the six keep their stored positions, which leave the slot of one hidden button empty between two visible ones (the other hidden button shares its stored rectangle with a visible one) | Yes, the six default text buttons and the close box; the two hidden buttons stay hidden (Tessera choice until the OPEN in §17.5 item (6) is settled) |
| P7 | Option pages built by game code from the templates of the same layout | Partly: Tessera fills the pages with its own settings using those templates |
| P8 | Friend, guild, group and raid lists | Yes, as empty lists |
| P9 | Quest categories and entries | Yes, as an empty log |
| P10 | The map image of the region or area (chosen by game code from the many map elements of the same layout), notes, the player marker | Player marker: yes. Map choice: OPEN (game code and game data); until settled Tessera shows the panel without a map image |
| P11 | Tracked quests | Yes, empty |
| P12 | The user's plugins, their descriptions and load state | Yes (Tessera's plugin host) |
| P13 | Name, level, race, class, lineage displays | Name and level: yes; the rest: Tessera's sandbox profile or empty |
| P14 | Shortcut slots of an extra bar | Yes, empty |
| P15 | The bag's items (`InventoryDisplay`), a bag toolbar with a search entry (`InventoryToolbar`), the title, a store button (`BuyNowButton`), drag bar, resize bar and close box | Yes, as an empty bag: frame, title, drag bar, resize bar and close box work; the item list shows no items; the search entry accepts text and finds nothing; the store button is hidden (Tessera choice). Only bag 1 is required; bags 2–6 are the same layout with other roots |

<a id="s17-5"></a>
### 17.5 Behaviour that is game code (OPEN)

> **OPEN:** (1) the drag-box frame (`HiddenDragBox` roots of P1, P4, P5, P11, P14): which of its
> four states (`hidden`, `visible`, `dragging`, `visible_mouseover`) game code selects in normal
> play and in a layout-editing mode, and whether the frame children `HiddenDragBox_DragBox`,
> `HiddenDragBox_DragBar` and the title are hidden by code in normal play (the states only
> change the drag bar's image). Tessera's choice until an owner screenshot settles it: frame,
> bar and title hidden in normal play, shown in an edit mode. (2) Which chat windows game code
> creates (element IDs `NewChatWindow1` to `NewChatWindow9` exist) and where it attaches them.
> (3) ~~How the bag panel (P15) is opened~~ (closed: U45; the original's bags are the `containerdisplay` windows; the `backpack` layout's `RealBackpackField`, an engine Field of 19 elements, is opened by no call site and is not a Tessera target). (4) How the minimap (P1) and the world map (P10)
> choose their images. (5) The widget behaviour of the game types in the second table of §17.2
> beyond drawing their images and text. (6) Whether game code shows the two main-menu text
> buttons that the layout hides (P6, §17.4), hides any of the six others, or moves the visible
> buttons to close the gap; not traced. None of these blocks drawing the panels' art, geometry
> and text.

---

## Change log

| Date | Change |
|---|---|
| 2026-10-05 | First complete draft: record grammar with census over all 302 records, media-item variants, field meanings and enum mappers, base-element resolution, design-time and run-time layout with edge modes 0–6, sibling order, states, media player, image drawing, button states, references and failure categories (OBS + BIN, client 4906.0071.1794.4035). |
| 2026-10-05 | Revision after review (changes requested). §7.3 step 6: an override merges the derived child into the inherited child without resolving either; the inherited child's type and base reference survive, the derived child's are ignored; the merged child is resolved at creation (BIN), with an override census and worked example §14.5, invariant UL15. §8.4 step 3: aspect pass stated in 32-bit floats (BIN). §8.4 step 5: the clamp bounds *Nw* and *Nh* confirmed as the client's exact rule (BIN). New in-section OPEN callouts in §3.2, §5.3, §8.4 step 1 (object modes), §8.5, §9.2 step 5 (with the traced lookup keys), §9.3 and §9.6 (DrawAfterChildren census and search result); §16 lists all OPENs. Provenance rows U21–U30 for §3.1–§3.2, §7.3, §8.4, §8.5, §9.1, §9.2, §9.6 and §9.8; U13 mark split; BIN row lists the revision questions. §10: the 0x25 count split into 5,362 StringInfo and 5 DID references, and the 1,324 zero references broken down. §15: the 0x2A, 0x21 and colour rows reworded as agreements; the 0x22 row quotes the current status. |
| 2026-10-06 | Revision after re-review (changes requested). §5.7: default-state census corrected and put in a table: 883 on the element, 237 based and present after resolution, 21 based and defined nowhere (none an override child, all resolve), 71 typed and undefined; 92 default states have no effect (re-probed over all 302 records). §9.1: the start-up panel question is now an in-section OPEN callout, so every §16 item has one; §16 item 8 reworded to match. |
| 2026-10-06 | Revision after third review (changes requested). §9.1: removed the unsupported claim that panels are usually created through the `UILAYOUT` map; the section now states only that a panel is created from a (layout DID, top-level element ID) pair and points to the OPEN callouts in §9.1 and §3.2. §6.1: Movie and Image body sizes now state the path's VLE length (REC §1), 1 byte for every observed path (all shorter than 128 bytes; see the 2026-10-06 fourth-review row for the corrected maximum). §16 item 9 reworded to allow that the maps are not used at all, matching §3.2. |
| 2026-10-06 | Revision after fourth review (changes requested). §4.7: the observed Pascal-string maximum corrected from 47 to 50 bytes (media paths ≤ 48), re-measured over all 302 records freshly extracted read-only from the install (2,610,590 bytes, byte-identical to the review extraction): 11,377 Pascal strings (11,367 media paths, 10 inside property sets), twelve longer than 47 (ten Image paths of 48, one property-set string of 49, one of 50); Movie paths 28 and 47. The third-review change-log row no longer states "all 47 bytes or shorter". §11: new census row for Pascal strings. The other §4 counts were re-measured over all records and stand: nesting depth ≤ 7; media per block 0–5 (elements) and 0–10 (states); table sizes up to 450 (top level), 339 (children), 0–139 (states); state tables all hint 1, element tables and property sets at the minimal hint (one element set of 23 entries at hint 1); 16,421 element and 12,977 state blocks with the §4.3 and §4.4 equalities; 7,804 typed / 8,617 based; 12,998 media items with kind = tag. §6.1 unchanged (every length VLE is 1 byte). |
| 2026-10-06 | **M5 revision (status back to Review).** New BIN questions (k)–(p) on the same client build; provenance rows U31–U44; U13, U16, U23, U27, U28, U30 updated. **Closed OPENs:** how game code names layouts (§3.2: by `UILAYOUT` entry key through root-map index 4) and opens panels (§9.1: open-panel, activatables loop, create-element; 114 call sites with immediate pairs, all resolving); the §9.2 step 5 table (**corrected**: it is the message-listener registry; keys 0x0E, 0x18, 0x1E, 0x32 are the mouse-button messages and the hook sets the element's mouse-button flag that hit testing needs; it is not saved panel data); the drawing traversal (§9.3: visible elements only, clipped to their box and ancestors, children from the start of the list); `UICore_Element_DrawAfterChildren` (§9.3, §9.6: children first, then the element's own content; reached through a subtraction chain, so no immediate 0x3CD); the blend equations (new §9.6.1–§9.6.3: CPU compositor, draw-mode conversion table, per-pixel arithmetic of every operation data uses, Colorize as an HSV recombination, base-layer Grayscale draws nothing, overlay images contribute only their alpha). **Corrected:** §9.6 no longer says both layers use the image opacity (base layer only). **New:** §5.9 rows for PIDs 0x3CD, 0xBD0, 0x695, 0x476, 0x475 and the game-level `UI_Element_CreateAsRootElement`; §9.9 saved panel locations and their XML file form; invariants UL16–UL20; examples §14.6–§14.8; §15 rows for `skin-definition.md` OPEN-1 and `lua-plugin-api.md` Appendix A; §17 catalogue of 15 native panels (layout, root, open path, geometry at three screen sizes, counts, data and sandbox, game-code OPENs) for ROADMAP M5 exit criterion 2, with the checklist `docs/checklists/m5-visual.md`. §16 updated (items 4, 5, 13, 14 closed; 15 added). |
| 2026-10-06 | Revision after the M5-revision review (changes requested). §17.2 second table recounted on the instantiated panels by an independent re-run of the resolver that replaces a base property with the derived property of the same PID (§7.2 step 3); the earlier probe had appended base and derived property entries. **Corrected:** String refs P4 12 → 11, P10 5 → 4, P12 16 → 15; Fonts P2 8 → 6. Every other String-refs and Fonts cell re-measured and unchanged. Art-surface cells now include the images of Cursor items (§6.5), as the note always claimed: P1 167, P2 13, P4 313, P5 572, P7 58, P8 106, P9 122, P10 25, P11 12, P12 93, P13 77, P14 16 (P3, P6, P15 unchanged; the art counts without cursor images were unaffected by the replacement fix). The note under the table now defines each count. §9.6.3: the opening sentence no longer says the overlay opacity is always 255; it says the element's opacity is not applied, and that with an image the compositor job takes the colour's alpha as its opacity. §17.4 P6 row: the button field has eight text buttons and a close box; two text buttons are hidden by `UICore_Element_hide` in the layout, so six text buttons and the close box show by default (re-checked on the resolved tree). §17.5 item (6) and §16 item 15: whether game code changes that set is OPEN. U42 updated. Checklist `m5-visual.md` P2-T, P6-A and the A rows updated to match. |
| 2026-10-06 | **M5 consolidation revision** (status Accepted → Review; answers the M5 completeness critique items B1, B3, B4, B5 and B6 for this spec; BIN consolidation questions (q)–(r) on the same build). **Hit flags (B1):** §9.2 step 5 and §9.3 now say the hook sets the **mouse-visible** flag and that the hit test reads mouse-visible and blocks clicks; post-creation recomputes mouse-visible from the type rule (§9.2 step 8, U28), so the creation-time value does not survive; the Tessera rule "elements without a handler let clicks through" is withdrawn in favour of `ui-behaviour.md` §4.3, the single statement of the rule; §14.6 and U33 follow. **Box model (B3):** §9.3 notes that hit testing uses displayed boxes (UB §9.2.1). **Bag panel (B4):** P15 is now `containerdisplay` / `PlayerBag1_Template`, the window the original opens (U45, new); its catalogue, count, geometry and data rows are re-measured (61 elements, 2 text elements, 7 string refs, 3 fonts, 47 art surfaces, 67 states; (599, 164, 197, 183) at every screen size); §17.5 item (3) and §16 item 15 closed; UL21 added. **`has_position` (B5):** §5.3 and §9.8 corrected per `skin-definition.md` SK33/SK34 (the override does not set it, zeroes the edge modes unless protected, may set the scaler origin, acts on a per-build copy), U29 updated. **Stale references (B6):** §8.5 OPEN and §16 item 12 answered by UB §9.4; header companion list names `skin-definition.md` and `ui-behaviour.md`; §15 rows for SK OPEN-1 and the SK §13 correction marked applied, new rows for UB §4.3 and the bag contradiction. The checklist `m5-visual.md` P15 rows still name the old panel and need the maintainer's update. |
| 2026-10-06 | **Revision applying the accepted decisions 0014 (UI input bridge) and 0015 (UI rendering)** (status Accepted → Revised, review pending). No fact about the original is changed; the decisions fix only Tessera's behaviour. New provenance tag DEC. **Tessera rules now taken from the decisions:** §9.6.1 OPEN (composition onto the frame: premultiplied source-over on stored values, transparent-black initial content; **replaces** the earlier Tessera choice of straight source-over); §8.4 step-1 OPEN (canvas = layout box composed onto the displayed box; Tile with power-of-two sides repeated at the display-node scale, no surfaces below a Tile surface; a stand-in surface at opacity 1 for a shown mode-0 root panel); §9.6.2 OPEN (conversion of other source formats); §8.4 note (re-layout complete before any hit test, IB D8); §8.5 note (physical render size, no platform scale factor); §9.1 notes (modal dialog surface and container; `DialogBoxes` push); §9.3 hit-test note; §17.4 P4 portrait as external content. **New OPEN callouts with Tessera rules** (spec questions 1, 3, 5, 14 and 16 of decision 0015): §5.9 which descendants `ObjectOpacity` reaches when they own surfaces (own opacity only); §5.9 `ShouldEraseBackground` (always erase); §8.4 scope of the step-1 object-mode test against LPA §13.19, and the None branch of UB §9.2.1; §9.3 composition of non-root surfaces; §9.6.3 32-bit formats other than A8R8G8B8. §6.6 OPEN cites LPA §13.19, UW §4.5 and GP §8.2 step 3 as partial answers. **New data (spec question 7 of decision 0015):** §17.2 census of non-root elements with an object mode per panel (92 in the 15 panels: 86 Stretch, 6 CanvasResize), row U46, invariant UL22, measured 2026-10-06 on the instantiated panels with read-only probes. §15 row for the UB §9.2.1 conflict; §16 items 4, 7, 11 updated and items 16–20 added. Section numbers unchanged. |
| 2026-10-06 | **Fixes after the review of the decision-0014/0015 revision** (status stays Revised, review pending). No fact about the original is changed. **§8.4 OPEN (scope):** explicit Tessera choice for question (1): the Stretch-parent test of step 1 applies only to root parents, so the children of a non-root Stretch surface are laid out by their edges; supporting data in new row U47 (a) (159 such children in the §17 panels, the readings differ only for P10's `Panel_Label_Title`, x 28 against 0); the OPEN now points to the matching `ui-behaviour.md` §9.2.1 OPEN. **§8.4:** new OPEN on the conflict between the Tile canvas of decision 0015 §2.1 (the layout box) and the BIN statement of `lua-plugin-api.md` §20 (the base image or a code-set size); §16 item 21. **§15:** the UB §9.2.1 row now reads "recorded in UB §9.2.1 OPEN; agreement". **§17.4:** the P5 row gets the external-content portrait note; the P4 and P5 rows say that the portraits also own CPU surfaces (`PlayerPortrait` mode 3; `OpponentPortrait` mode 1 with an own image and a mode-1 child; U47 (b)) and point to the new `ui-widgets.md` §12 OPEN and Tessera rule. **UL22 and the §17.2 note:** one definition of "surfaced ancestors" (ancestors with a non-zero resolved object mode, plus the root); counts re-checked, unchanged. §16 item 18 updated. |
| 2026-10-06 | **Final-pass fixes** (status stays Revised, review pending). No fact about the original is changed. **§8.4 OPEN (scope):** one note that it is not known whether the flag by which an alpha-test meter asks for a surface (`ui-widgets.md` §4.5) is the "flagged by code" of §8.3 and §8.4 step 1, with the Tessera choice that it is not (the meter owns a surface but is laid out as a non-root element; no M5 panel affected); §16 item 18 extended to match. **§15:** new row recording the corrected citation in `lua-plugin-api.md` §13.18 (mode-1 row: the reason is UB §9.2.1, not §8.4 step 1), applied in the working-tree LPA, pending its review. |
