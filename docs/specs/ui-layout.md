<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# UI layout records (prefix 0x22): element trees, states, media items, inheritance, layout and native panels

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Fifth review: Claude review session (spec-writer agent), review 2026-10-06: **passed; status moved to Accepted**. The one fourth-review item is repaired. §4.7 now says "observed lengths ≤ 50 (media paths ≤ 48)". §11 has a Pascal-string census row. The third-review change-log row no longer states a maximum of 47, and a fourth-review change-log row records the correction. Every §5.3 item passes: only functional facts; no code or pseudo-code; no copied prose; no game text, keys or hostnames; every fact has a provenance row and a mark; byte layouts are complete; invariants are testable; all 14 §16 items have an in-section `> **OPEN:**` callout; no closed gate. The spec is implementable. Re-check of the disputed counts: all 302 records were re-extracted read-only from the install on 2026-10-06 (2,610,590 bytes, byte-identical, dictionary 0x34000000 identical). A decoder written independently from §4, §6.1, `property-system.md` §2, §4 and §5 and `records.md` §1 and §4 decodes every record with zero leftover bytes. It counts 11,377 Pascal strings (11,365 Image paths, 2 Movie paths, 10 in property sets), the longest 50 bytes. Twelve strings are longer than 47 (ten Image paths of 48, one property-set string of 49, one of 50). Movie paths are 28 and 47 bytes, 11 Image paths are non-empty, and every length is below 128. Further spot checks, all matched: the §5.1 design-size histogram, with 151 of 3,144 top-level elements outside their design area; the §5.2 ID counts (13,035 distinct, one used 88 times, 69 engine-level; states 9,470 / 3,507; types 6,484 / 1,320); §5.5 read-order numbering in all 302 top-level and 3,774 non-empty child tables; §5.6 5,665 cross-layout and 2,952 same-layout references; the §5.9 entry, PID and value-type census and the top five state properties; the §6.2 Image census and draw modes; the §6.3 Coloured-image census; the §6.4 Animation census (2,252 frames, 2–109 per item, durations, draw modes, loop); the §6.5 hot spots; the §6.6 Fade depths 18 / 16 / 2; the §6.7 Pause pairs; the §6.8 State and Message items; the §11 record sizes, per-record maxima, 13,970 elements without states and the engine widget-type counts. Earlier records follow. Fourth review: Claude review session (spec-writer agent), review 2026-10-06: **changes requested** (one item). All three third-review items are repaired. §9.1 now states only that a panel is created from a (layout DID, top-level element ID) pair and points to the OPEN callouts. U27 no longer claims how the pair is chosen. §6.1 gives the Movie and Image path length as a VLE (REC §1), 1 byte for every observed path. §16 item 9 allows "not at all", matching §3.2. Every other §5.3 item still passes, and the spec is implementable. Remaining issue, which a spot check over all 302 records contradicts (§5.3, "each fact ... marked Confirmed"): §4.7 "Defensive limits" says "Pascal strings: observed lengths ≤ 47". That is false. The records hold 11,377 Pascal strings (11,367 media paths plus 10 strings inside property sets), and the longest is 50 bytes. 12 strings are longer than 47: ten of 48, one of 49 and one of 50. The ten Image paths of 48 bytes are the longest media paths (Movie paths are 28 and 47). The third review record above wrongly repeated the "47 or shorter" figure, and the change log row inherited it. Fix: say "observed lengths ≤ 50 (media paths ≤ 48)" in §4.7, and correct the 2026-10-06 change-log row if it is restated. Every length is still below 128, so the §6.1 statement that the VLE is 1 byte holds and needs no change. Spot checks on the 302 records extracted read-only from the install on 2026-10-06 (2,610,590 bytes) all matched: the §5.8 edge census for all four edges; the §5.4 element and state flag census; §5.3 (118 z-levels, 12,911 zeros; `has_position` 16,199 / 222, of which 209 typed and 12 based have a zero rectangle); the §6.1 per-kind counts and their element / state split (12,998); the §11 media-per-block rows; 2,852 empty states; all state tables at hint 1 and every element table at the minimal hint; the §5.7 default-state table (883 / 237 / 21 / 71 = 1,212, none of the 21 an override child); the §7.1 chain depths 7,076 / 1,494 / 44 with 3 failures; 11,365 Image and 2 Movie items, every one carrying its path. Earlier records follow. Third review: Claude review session (spec-writer agent), review 2026-10-06: **changes requested** (one blocking item). Both earlier issues are repaired: §5.7 now gives the correct default-state census, and §9.1 has an OPEN callout, so every §16 item has an in-section callout. Blocking, §5.3 "unknowns are marked OPEN, not guessed": §9.1, first sentence, says game code creates a panel "usually through the UILAYOUT DID map". That is stated as a BIN fact ("All of §9.1: BIN, U27"), but §3.2 says none of the map's use was traced in the client (U23), and the new §9.1 callout lists "directly, or through a `UILAYOUT` map entry" as unknown. Delete "usually through the UILAYOUT DID map" (or replace it with a pointer to the OPEN), so that §9.1 states only that a panel is created from a (layout DID, top-level element ID) pair. Non-blocking: in §6.1, "(1-byte length prefix)" for the Movie and Image path strings can be read as a u8 length. REC §1 defines a VLE length, and it is 1 byte only for lengths below 128 (all observed paths are 47 bytes or shorter), so say "VLE length; 1 byte for every observed path". Also, §16 item 9 ("how game code finds layouts through the maps") assumes the maps are used, while the §3.2 callout allows "not at all"; align the wording. Everything else passes the §5.3 checklist and is implementable: grammar, limits, merge, layout formulas, states, media player, invariants and the §14 examples. I recomputed the §14.1 offsets (158 bytes) and every §14.2 and §14.3 result, including the 32-bit aspect pass. Spot checks, re-extracted from the install read-only on 2026-10-06: all 302 records, 2,610,590 bytes, byte-identical to the earlier extraction. These match: the §5.7 default-state table (883 / 237 / 21 / 71 = 1,212, by an independent resolver), with none of the 21 an override child; the §5.8 edge census for all four edges; the §5.4 flag census for elements and states; §5.3 (118 z-levels, 12,911 zeros; `has_position` 16,199 / 222, of which 209 typed and 12 based have a zero rectangle); the §6.1 per-kind and element/state media counts (12,998); the §11 media-per-block rows; 2,852 empty states (§9.4); state tables all hint 1 and element tables at the minimal hint (§4.2); the §7.1 chain depths 7,076 / 1,494 / 44, with 3 failures. Earlier records follow. Re-review: Claude review session (spec-writer agent), review 2026-10-05: changes requested. Every earlier issue is repaired, and every §5.3 item passes except one census fact that a spot check over all 302 records contradicts. §5.7 says 258 based elements get their default state only through the base chain, so that it "exists after resolution". In fact it exists after resolution for 237 of them, and for 21 based elements it is defined nowhere, so 92 default states (71 + 21) have no effect, not 71. These spot checks matched: all 302 records were re-extracted from the install, byte-identical; the §7.3 override census (97 + 13, 82 / 28, 8 of 21, 7); U30 (223 / 9, 3 states); the ObjectMode census; the §10 zero and 0x25 breakdown; 5,555 / 13,036; 83,161 instances; 131 Text without a font; `has_position` 222 / 209 / 12; the §5.2 ID counts. Earlier record: Claude review session (spec-writer agent), review 2026-10-05: changes requested (unmarked unknowns need `> **OPEN:**` callouts; provenance rows missing for §3.2, §8.5, §9.1, §9.2, §9.8; §7.3 step 6 base references of overriding/inherited based children; minor: aspect-pass float width, stale §15 rows, §10 0x25 row wording). Grammar and censuses re-verified independently on all 302 records. |
| **Implements into** | `crates/formats` (decoder for prefix 0x22 records with a fuzz target, §4), `crates/ui` (descriptor resolution §7, run-time layout §8, element tree, states and media player §9, drawing of element images §9.6), `apps/explorer` (layout browser: tree, states, media, resolved view), `apps/client` (native panels, M5) |
| **Companion specs** | [`records.md`](records.md) §1 (VLE, TSize and bucket order, Pascal strings), §2 (DID prefixes), §5 (render surfaces 0x41), §7 (enum mappers), §8 (DID maps). [`property-system.md`](property-system.md) §4–§5 (property values and embedded property sets), §8 (root DID map). [`dat-container.md`](dat-container.md) (routing; prefix 0x22 lives in `client_local_<Locale>.dat`). [`fonts.md`](fonts.md) §7–§9 (text layout, outline, font choice; written in parallel). [`string-table.md`](string-table.md) (StringInfo text). [`lua-plugin-api.md`](lua-plugin-api.md) (plugin controls, blend-mode values). [`install-layout.md`](install-layout.md) (install directory, the `raw/` folder). `skin-definition.md` (user skins; not yet written). |

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
  questions (§16).

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
| BIN | The client executable of the user's install, `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`, change list 711794, compiled 2026-09-27), disassembled in the dirty room under the standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md), method M4. **Questions:** (a) the exact grammar and field roles of the record (the deserialisers of the layout, element, state and media-item objects, and the factory that creates media items by type); (b) what each media-item kind does when it runs; (c) how a base element is resolved and merged; (d) the edge modes, the design size and the run-time layout of an element; (e) how elements are created, ordered among siblings, switched between states and drawn; (f) the button state choice. **Revision questions (same build, 2026-10-05):** (g) which base reference survives when a derived child overrides an inherited child, and when that child's own base is resolved (the element merge, the resolver and the creation routine); (h) the float width of the aspect pass and the exact bounds of the keep-inside clamp (the run-time layout routine); (i) the screen root's creation; (j) whether the client loads PID 0x3CD (`UICore_Element_DrawAfterChildren`) as an immediate operand. **Why M1–M3 were not enough:** the data gives values but not their meaning; the only published reader (LC) gives no semantics and misreads several fields (§15); no public description of the layout engine exists; the original client cannot be observed in this session (M3), and observation could not give the layout formulas anyway. Only offline UI code was read: no network, login, launcher, patcher or anti-tamper code. |
| LC | LotroCompanion `delta-lotro-dat-utils` 11.4 (in the `lotro-tools` distribution; Apache-2.0 per its distribution), class `UILayoutLoader`, read as Java bytecode for facts: field order, the enum-mapper DIDs it uses, its media-type numbers. |
| ER | `research/notes/engine-client-re.md` §6 (element IDs come from enum 0x230000D1; user skins). |
| AC | Asheron's Call analogue (ACE `LayoutDesc`/`ElementDesc` readers, the DatReaderWriter schema, a published client structure listing). Vocabulary only: the same engine family has layout, element and state descriptors, edge fields and incorporation flags. Marked **Analogue**. |
| PS | [`property-system.md`](property-system.md) (embedded property sets, value encodings, root DID map). |
| REC | [`records.md`](records.md) (VLE, TSize, strings, enum mappers, DID maps, render surfaces). |
| FO | [`fonts.md`](fonts.md) (font choice, text drawing). |

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
| U13 | Sibling order: z-level descending, then read order ascending; lower z is in front (§9.3) | BIN (insertion comparator, hit test); OBS (backgrounds carry positive z, title bars, close buttons and tooltips negative z) | BIN (list order, hit test); Probable (drawing order, OPEN in §9.3) |
| U14 | State switching, property layering and media-list replacement (§9.4) | BIN | BIN |
| U15 | Media player: sequential items, waiting items, result codes (§9.5) | BIN | BIN |
| U16 | Element images: two layers, drawn at native size, tiled over the element rectangle and clipped (§9.6) | BIN (draw routine); OBS (element and image sizes) | BIN |
| U17 | Button state choice (§9.7) | BIN | BIN |
| U18 | All references from layouts resolve; target record types (§10) | OBS | Confirmed |
| U19 | Media colour is u32 0xAARRGGBB (bytes B, G, R, A) (§6.9) | BIN (colour serialiser); OBS (alpha byte patterns) | Confirmed |
| U20 | Design-size, element, state, media and property census (§11) | OBS | Confirmed |
| U21 | Layouts are stored only in the locale archive under prefix 0x22 (§3.1) | OBS (302 records in `client_local_English.dat`, none in `client_general.dat`) | Confirmed |
| U22 | The client loads a layout as DB-object type 0x28 (§3.1) | BIN (the resolver requests each base layout with type code 0x28) | BIN |
| U23 | Contents of the `UILAYOUT` and `PRELOADRESOURCES` DID maps, and which layouts each kind of reference reaches (§3.2) | OBS (both maps read in full; every layout checked against the maps, base references and layout-valued properties) | Confirmed (contents); how game code uses the maps is OPEN (§3.2) |
| U24 | Override children: the inherited child's type and base reference survive, the derived child's are ignored, and the merged child's own base is resolved only when the child is created (§7.3 step 6) | BIN (element merge, resolver, creation routine); OBS (110 override sites, census in §7.3) | BIN (rule), Confirmed (census) |
| U25 | Aspect pass in 32-bit floats; keep-inside clamp bounds are the parent's width and height, not width − 1 and height − 1 (§8.4 steps 3 and 5) | BIN (run-time layout routine) | BIN |
| U26 | Screen root: element ID 0x13, type Field, edges (5, 5, 5, 5), design size = screen size or 1024 × 768 (§8.5) | BIN (screen-root creation routine) | BIN; the resize trigger is OPEN (§8.5) |
| U27 | Root panels, modal-dialog container and the dialog override properties (§9.1) | BIN (panel creation, dialog framework) | BIN |
| U28 | Creation order of an element tree: resolve, user skin, factory, registration, children in table order with post-creation before attachment, post-creation of a top-level element after attachment (§9.2) | BIN (creation routine and its two callers) | BIN; the per-user saved-data lookup in step 5 is OPEN |
| U29 | User-skin hooks: geometry override between resolution and creation, image substitution when an image is set by DID (§9.8) | BIN (creation callers apply the active skin to the resolved descriptor; the merge honours a skin-override marker and a per-field mask) | BIN |
| U30 | `UICore_Element_DrawAfterChildren` (PID 0x3CD) is set on 232 elements and 3 states; no client instruction loads 0x3CD as an immediate (§9.6) | OBS (census); BIN (search of the code section) | Confirmed (census); effect OPEN (§9.6) |

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

> **OPEN:** how game code turns a panel into a layout DID. Unknown: (1) whether it looks up the
> `UILAYOUT` map by entry key, by label or not at all (DIDs could also come from game-object
> properties or from code); (2) whether the client loads the 9 layouts of `PRELOADRESOURCES`
> ahead of use or the map serves another purpose; (3) how the one layout reached by none of the
> three reference kinds above is opened. None of this was traced in the client (U23). Tessera
> can open any panel by (layout DID, top-level element ID) without the answer (§9.1).

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
  during inheritance (§7) and sets it when a user skin overrides an element's geometry (BIN).
  In state blocks it is always 0.

> **OPEN:** whether anything reads `has_position` after creation. The descriptor loader, the
> merge of §7.2 and the user-skin override are the only uses found; the run-time layout of §8.4
> does not read it. Readers outside the layout code (widget classes, saved panel positions,
> §9.2 step 5) were not searched. Until answered, Tessera treats the flag as informational and
> lays out elements with `has_position` = 0 like any other.

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

Most frequent other engine-level properties (named only): tooltip layout, ID, entry and on/off
(`UICore_Element_tooltip_*`), text justification and margins (`UICore_Text_*_justification`,
`UICore_Text_*_margin`, see FO §7.4), text colours and outline (FO §8), `UICore_Element_hide`,
`UICore_Element_ShouldEraseBackground`, `UICore_Element_activatable`,
`UICore_Element_DrawAfterChildren`, list-box entry templates (`UICore_ListBox_entry_templates`,
each a layout DID plus element ID), meter images and directions (`UICore_Meter_*`).

> **OPEN:** the behaviour of the remaining ~190 engine-level UI properties (tooltips,
> list boxes, trees, meters, scroll bars, menus, browsers, viewports) beyond their names and
> value types. Each widget class reads its own set; trace per widget as M5 panels need them.

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
surface, so the element fades together with its children) from *start* to *end* over
*duration* seconds. 18 of the 36 Fade items sit on top-level elements, 16 on depth-1 and 2 on
depth-2 elements.

> **OPEN:** which non-root elements own a drawing surface (so that a Fade on them is visible).

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

> **OPEN:** what the client does when the screen (window or render target) size changes. It
> is not known which code reacts to the change, whether it resizes the screen root (so that
> its children are laid out again by §8.4 because the root's size changed) or lays out each root
> panel directly, and whether modal-dialog containers (§9.1) follow. The expected result, every
> root panel laid out again from its layout's design size to the new screen size, is inferred
> from §8.3 and §8.4, not traced.

---

<a id="s9"></a>
## 9. Building native panels (BIN)

<a id="s9-1"></a>
### 9.1 Root panels

Game code creates a panel from a (layout DID, top-level element ID) pair. How it chooses the
pair is OPEN (see the callout at the end of this section and §3.2). The generic steps are:

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

For Tessera's walker (M5) the data-driven part is: create a root panel from a (layout, element)
pair and run §9.2–§9.6. (All of §9.1: BIN, U27.)

> **OPEN:** which panels the client creates at start-up, in which order, and how game code
> chooses the (layout DID, top-level element ID) pair for each (directly, or through a
> `UILAYOUT` map entry, see the OPEN in §3.2). This is game code outside the layout records; no
> record in the install lists the start-up panels. Tessera chooses its own panel set for M5 and
> does not need this to render a panel. Settle per panel by tracing the code that opens it, when
> a milestone needs that panel's exact behaviour.

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
   looks the element ID up in a per-element table held by the UI manager. If an entry exists
   and holds at least one of the four keys 0x0E, 0x18, 0x1E and 0x32, the client calls a
   widget hook with the value true before the children are created (BIN, U28).

> **OPEN:** the table of step 5: where it comes from (probably the user's saved panel
> positions and sizes, the client's own UI-layout save files), what the four keys mean, and
> what the widget hook does with it (probably: the panel's position or size comes from the user,
> not from the layout). Tessera can skip this step until its own saved-panel feature exists.

6. Push the element's ID on the ID path. For each child descriptor of the resolved descriptor,
   in table order: create the child (steps 1–6, recursively), run the child's post-creation
   (step 8), then attach the child to the element (§9.3), which lays the child out (§8.4).
   Pop the ID.
7. A **top-level** element created inside a given parent is then attached to it (laid out,
   §8.4). A root panel has no parent at this point (§9.1).
8. **Post-creation** (for a top-level element after step 7, for a child before it is
   attached): start the element's own media list (the media of its own block, §9.5), mark the
   element as having finished creation, then switch it to `default_state` (§9.4).

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
  that child's own children the same way.
- **Drawing** therefore goes from the start of the list to its end: higher z-levels behind,
  lower in front. This is inferred (Probable): the hit-test order and the data (§5.3) agree with
  it, but the drawing traversal was not traced.

> **OPEN:** the client's drawing traversal of the child list. Confirm that children are drawn
> from the start of the list to its end (higher z-level first, then lower read order first), that
> a child is drawn with all its descendants before its next sibling, and whether hidden children
> or children outside the parent's rectangle are skipped or clipped. A frame capture of one panel
> with overlapping siblings of different z-levels would also settle the first point (M3).

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

An element has two image layers, drawn in this order before its children (but see the OPEN on
`UICore_Element_DrawAfterChildren` below):

1. **Base image** (set by Image and Animation items): the surface drawn with its draw mode.
2. **Overlay** (set by Coloured image items): the colour fill, or the surface tinted by the
   colour, with its draw mode.

Each layer is drawn at the surface's **native pixel size**, anchored at the element's top-left
corner (plus a run-time image offset that is (0, 0) unless code sets it), **repeated (tiled)**
horizontally and vertically to cover the element's rectangle, and clipped to the rectangle. No
layer is ever stretched. Both layers are drawn with the element's image opacity
(`UICore_Element_alphablendmod`, default 1). When the element fits inside the surface and the
image offset is (0, 0), the client draws the top-left part of the surface once; otherwise it
tiles.

Data agreement: of 10,350 Image items on typed elements, the surface size equals the element
size in 6,127 cases, is smaller on at least one axis in 3,000 (tiled "middle" pieces of
frames, for example a 2-pixel-wide image on a 474-pixel-wide edge) and is larger in 1,223
(clipped).

Draw modes map to the renderer's blend states: 1 Colorize, 2 Normal, 3 Multiply,
4 Alphablend, 5 Overlay, 6 Grayscale, 7 Screen; 8 None means "no layer".

> **OPEN:** the effect of `UICore_Element_DrawAfterChildren` (PID 0x3CD, Bool). It is set on
> 232 elements (223 true, 9 false) and 3 states (all true) (OBS, U30). Its name suggests that the
> element's two image layers are drawn after its children instead of before, but that is not
> established: no instruction in the client loads 0x3CD as an immediate operand and no data
> table of the executable holds it, so the reader (if any) was not found (BIN, U30). Unknown:
> whether the client honours it, whether it moves only the image layers or also text, and
> whether a state can switch it. Until answered, Tessera draws image layers before children and
> records which elements set the property.

> **OPEN:** the exact blend equations of draw modes 1, 3, 5, 6 and 7 (and whether Normal
> ignores the image's alpha). They are render-state facts of the client's graphics module,
> not traced here. Owner frame captures of one panel per mode would settle them.

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

When a user skin is active, the client applies it at two points: after resolving a descriptor
and before creating the element, a skin entry for the element (matched by element ID and the
path of IDs from the root) can override its position and size (and then sets `has_position`);
and when an image is set by DID, the skin can substitute its own image for the surface (through
an art-asset table) (BIN, U29). The merge of §7.2–§7.3 has a separate mode for skin overrides,
selected by a marker that descriptors read from layout records never carry: in that mode only
the fields marked in the override are copied. Layout resolution always clears the marker, so it
never affects §7. The skin file format and matching rules belong to
`skin-definition.md`.

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
| LC `UILayoutLoader` and its summary in the research notes (Reported sources only) | Movie item ends with a u32; kind 6 "animate value"; kind 13 "scale" (two floats); the u32 after the rectangle is a colour (notes) or skipped (LC); media count is a u8 | Movie ends with a bool8 (LC fails on the only record that has Movie items); kind 6 fades the opacity; kind 13 is a random wait in seconds; the u32 is the z-level; the count is a VLE (identical on data, all counts < 128). | BIN, OBS |

---

<a id="s16"></a>
## 16. Open questions (summary)

Every item has an in-section `> **OPEN:**` callout; this list only collects them.

1. Archive version source for the Image item's file path (§6.1).
2. Jump item index arithmetic (§6.8); unused in data.
3. Which code assigns an element a reference box (§8.3).
4. Blend equations of the draw modes (§9.6); owner frame captures could settle them.
5. Drawing traversal of the child list (§9.3): inferred from hit testing and data (Probable),
   not traced.
6. Behaviour of the remaining engine-level properties and of each widget class (§5.9); trace
   per widget as panels need it.
7. Object modes None, Tile and CanvasResize beyond the layout rule of §8.4 step 1 (§8.4).
8. Which panels the client opens at start-up and how game code picks each panel's layout and
   element (game code; §9.1).
9. Whether and how game code uses the `UILAYOUT` and `PRELOADRESOURCES` maps to find layouts
   (by entry key, by label or not at all), and how the one unreferenced layout is opened (§3.2).
10. Readers of `has_position` outside the layout code (§5.3).
11. Which non-root elements own a drawing surface, so that a Fade on them is visible (§6.6).
12. What the client does when the screen size changes (§8.5).
13. The per-element saved-data table consulted at creation, its four keys and its hook (§9.2
    step 5).
14. The effect of `UICore_Element_DrawAfterChildren`, whose reader was not found (§9.6).

None of the owner-observation items O1–O17 concerns UI layouts.

---

## Change log

| Date | Change |
|---|---|
| 2026-10-05 | First complete draft: record grammar with census over all 302 records, media-item variants, field meanings and enum mappers, base-element resolution, design-time and run-time layout with edge modes 0–6, sibling order, states, media player, image drawing, button states, references and failure categories (OBS + BIN, client 4906.0071.1794.4035). |
| 2026-10-05 | Revision after review (changes requested). §7.3 step 6: an override merges the derived child into the inherited child without resolving either; the inherited child's type and base reference survive, the derived child's are ignored; the merged child is resolved at creation (BIN), with an override census and worked example §14.5, invariant UL15. §8.4 step 3: aspect pass stated in 32-bit floats (BIN). §8.4 step 5: the clamp bounds *Nw* and *Nh* confirmed as the client's exact rule (BIN). New in-section OPEN callouts in §3.2, §5.3, §8.4 step 1 (object modes), §8.5, §9.2 step 5 (with the traced lookup keys), §9.3 and §9.6 (DrawAfterChildren census and search result); §16 lists all OPENs. Provenance rows U21–U30 for §3.1–§3.2, §7.3, §8.4, §8.5, §9.1, §9.2, §9.6 and §9.8; U13 mark split; BIN row lists the revision questions. §10: the 0x25 count split into 5,362 StringInfo and 5 DID references, and the 1,324 zero references broken down. §15: the 0x2A, 0x21 and colour rows reworded as agreements; the 0x22 row quotes the current status. |
| 2026-10-06 | Revision after re-review (changes requested). §5.7: default-state census corrected and put in a table: 883 on the element, 237 based and present after resolution, 21 based and defined nowhere (none an override child, all resolve), 71 typed and undefined; 92 default states have no effect (re-probed over all 302 records). §9.1: the start-up panel question is now an in-section OPEN callout, so every §16 item has one; §16 item 8 reworded to match. |
| 2026-10-06 | Revision after third review (changes requested). §9.1: removed the unsupported claim that panels are usually created through the `UILAYOUT` map; the section now states only that a panel is created from a (layout DID, top-level element ID) pair and points to the OPEN callouts in §9.1 and §3.2. §6.1: Movie and Image body sizes now state the path's VLE length (REC §1), 1 byte for every observed path (all shorter than 128 bytes; see the 2026-10-06 fourth-review row for the corrected maximum). §16 item 9 reworded to allow that the maps are not used at all, matching §3.2. |
| 2026-10-06 | Revision after fourth review (changes requested). §4.7: the observed Pascal-string maximum corrected from 47 to 50 bytes (media paths ≤ 48), re-measured over all 302 records freshly extracted read-only from the install (2,610,590 bytes, byte-identical to the review extraction): 11,377 Pascal strings (11,367 media paths, 10 inside property sets), twelve longer than 47 (ten Image paths of 48, one property-set string of 49, one of 50); Movie paths 28 and 47. The third-review change-log row no longer states "all 47 bytes or shorter". §11: new census row for Pascal strings. The other §4 counts were re-measured over all records and stand: nesting depth ≤ 7; media per block 0–5 (elements) and 0–10 (states); table sizes up to 450 (top level), 339 (children), 0–139 (states); state tables all hint 1, element tables and property sets at the minimal hint (one element set of 23 entries at hint 1); 16,421 element and 12,977 state blocks with the §4.3 and §4.4 equalities; 7,804 typed / 8,617 based; 12,998 media items with kind = tag. §6.1 unchanged (every length VLE is 1 byte). |
