<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Fonts: font records, glyph atlases, text layout and how the client picks a font

| | |
|---|---|
| **Status** | Accepted (revision 7, 2026-10-06: accepted after revision for decisions 0014/0015). Revision 6 was Accepted on 2026-10-06 |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | **Revision 7 review: Claude review session (spec-writer agent), 2026-10-06: passed, Accepted (accepted after revision for decisions 0014/0015).** Every item of `CLEAN_ROOM.md` §5.3 holds. Revision 7 changes no fact about the original; the reworded and added paragraphs are marked Tessera Decision, Tessera rule or Tessera default and cite the new provenance tag UR, which is declared the source of no fact about the original. They agree with the accepted decision 0015: §4.1 and §8.4 with UR §3 (atlases decoded on the workers into CPU coverage buffers and never uploaded; one exact CPU path for all text, outline then fill with underlines; caret and selection as rectangle fills, the blink repainting only the caret's rectangle); §6.3 and Q3 with UR §2.2 (whole-surface resampling, premultiplied bilinear with clamp-to-edge, no mip chain, `ui_scale_filter`; game text never re-rasterised at the scaled size); Q6 with UR Decision 1.1 and 1.6 and the Consequences (32-bit 0xAARRGGBB straight-alpha canvases starting transparent black, text and art in one canvas, the final blend exact only for alpha 0 and 255 at 1 : 1); §11 item 4 with UR §3 (Tessera fonts rasterised at 1 : 1 into the same coverage form). Decision 0014 cites nothing in this spec, and nothing here conflicts with it (the text hit tests of §6.3 item 3 are those UR §2.4 relies on). No code, no copied prose, no game text, keys or hostnames; section numbers are unchanged; Q3 and Q6 stay OPEN with precise questions and suggested methods and block nothing in M5, because the Tessera defaults are complete. Spot checks (owner's install, read-only, 2026-10-06, fresh extraction by a new probe over all records): (1) F1: 48 records with prefix 0x42, 45 in `client_local_English.dat` and 3 in `client_general.dat`, all stored uncompressed; (2) F2/FN1/FN2: payload = 36 + 11·*n* and offset 0 = DID, 48/48, 28,315 glyphs; (3) F4/FN3, FN5, FN6: line height = largest *H*, width field = largest *W*, 0 ≤ *T*, *T* + *H* ≤ line height, advances 0–37, baseline < line height, 48/48; (4) F8/FN4 and F5/FN7: codes strictly ascending from U+0020 with `?` present; *bx* = *by* with borders 4/3/2/1/0 in 32/8/3/4/1 records and a zero background DID only with zero borders; (5) F6/F7/FN8/FN10: all 95 atlases found, all category 13 and format 28, the 47 background atlases the size of their foreground, and the packing rule exact in 48/48 atlases with 703 row breaks. Non-blocking notes for a later revision: the Q3 default says "at scale 1 the canvas is copied exactly", where UR §2.2 also requires no rotation in the entry's transform; §11 item 4 names Tessera fonts of item 2 only, where UR §3 names items 2–3 (§8.4 already says "§11"); the notes of the revision-6 review (FN26's "never with 1" should read "never with the default (1, 1, 1)"; the fourth cell in three §14 rows) and of the revision-4 review (§7.5 step 1 with `UICore_Text_one_line`; the §1 legend date) are still open. **Revision 7 (spec writer, 2026-10-06): submitted for review.** It applies the accepted decision record 0015 (UI rendering): the Tessera Decisions of §6.3, §8.4 and §11 item 4 are reworded to the chosen design (exact CPU compositing into one canvas per surface, no GPU text path, Tessera fonts at 1 : 1), and Q3 and Q6 carry the Tessera defaults that 0015 sets (change log). No fact about the original is changed. **Revision 6 re-review: Claude review session (spec-writer agent), 2026-10-06: passed, Accepted.** The blocking item of the revision-5 review is resolved. §6.3 item 1, F35 and FN26 now name the scale: the nearest display node at or above the element, (1, 1, 1) only when none. Item 3 names the conversion. Both agree with `ui-behaviour.md` §4.1 step 3 and §9.2.1 (revision 3). BIN spot check (decision 0011, build 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5): one shared conversion routine, called from many text routines, does four things: it fetches the element's scale through the same accessor as the point-in-element test; it divides *x* by the *x* component and *y* by the *y* component with no tolerance test; it ignores *z*; and it truncates and adds the horizontal and vertical scroll offsets. §13.12 recomputes to (20, 15), and to (31, 21) for the wrong reading. Every other §5.3 item passes. Non-blocking notes for a later revision: FN26's phrase "never with 1 unless no such node exists" should read "never with the default (1, 1, 1) unless …", because a node can have scale 1. Several §14 rows, including the new one, have a fourth cell that the three-column header does not have. **Revision 6 (spec writer, 2026-10-06): submitted for review.** It answers the one blocking item of the revision-5 review below: §6.3 items 1 and 3 and F35 now name the scale that text hit tests divide by (the nearest display node at or above the element; (1, 1, 1) only when there is none), from BIN question 25 (change log). **Revision 5 review: Claude review session (spec-writer agent), 2026-10-06: changes requested** (one blocking item). Blocking (§5.3 "each fact ... marked Confirmed" and "unknowns are marked OPEN"; §6.3 items 1 and 3, F35): item 1 says "an element without a display node has the scale (1, 1, 1)", and item 3 divides mouse positions by "the scale". BIN spot check (decision 0011, same build): the element's scale accessor (the one the point-in-element test of `ui-behaviour.md` §4.1 calls) returns the scale of the element's own display node, otherwise that of the nearest ancestor with one (following parents), and (1, 1, 1) only when none exists. Read literally, item 1 makes a text element inside a scaled panel, which has no node of its own, divide by 1. That maps hit points wrongly at every scale other than 1, and it contradicts F35 ("the nearest display node") and `ui-behaviour.md` §9.2.1 (*ex*, *ey*). Reword item 1 to the nearest display node at or above the element, and say in item 3 which scale the three text hit-test routines divide by. If they use a different accessor, add an OPEN. Every other §5.3 item passes. The changes to §6.3 item 4, §7.6, F35, F41, Q3, Q13 and §14 are functional facts with BIN questions 23–24 stated. §7.6 is now the only statement of the truncation rule and agrees with `ui-behaviour.md` §10.1. The census is reconciled, section numbers are kept, and there is no game text. Spot checks (owner's install, read-only, 2026-10-06; all 48 font records freshly extracted; layouts byte-identical to the earlier extraction): (1) F1: 48 records with prefix 0x42, 3 in `client_general.dat` and 45 in `client_local_English.dat`. (2) F2/FN1/FN2: payload = 36 + 11·*n* and offset 0 = DID, 48/48. (3) F4/FN3: offset 4 = largest glyph height and offset 8 = largest width, 48/48. (4) F8/FN4, FN6, FN7: codes strictly ascending from U+0020 with `?` present; baseline < line height; *bx* = *by*, with a zero background DID only together with zero borders; 48/48. (5) F41, F40, F34, F44: `truncate_text_to_fit` 325 values (319 + 1 + 5 null, as §7.6); bullet image 15 values (two DIDs, 11 and 4); bullet spacing 2 values (2 and 4); `extra_leading` once (0.0); the settings folder holds `UserPreferences.ini`, `lotro.keymap`, `PluginData` and a browser cache, the three `UIScale_*` keys are in `[UI]`, and no key names a font: all match. **Revision 5 (spec writer, 2026-10-06): submitted for review.** It answers the M5 completeness critique items B2 and B3 for this spec (change log). Earlier records follow. Claude review session (spec-writer agent), review of revision 4, 2026-10-06: **passed**, Status moved to Accepted. Every item of `CLEAN_ROOM.md` §5.3 holds for the new material (§6.3, §6.1/F10, §7.5, §7.6, §8.4–§8.6, §9.1 rule 5 and the panel-font census, §9.3 local file form and game objects, §9.6, FN20–FN25, §13.8–§13.11, §14, §15): functional facts only; no code or listing-shaped text; the names used (`UIScale_*`, `GOP_UIScale_*`, `UIScaleDefinition_*`, `GetUIGroupScale`, the `UICore_Text_*` and `UICore_Element_*` properties, `ChatOptionFontMenu`, the XML element and attribute names of §9.3) are stored property, enum or element labels, plugin-API names or file-format names needed for interoperability; no game text, assets, keys or hostnames (string-table entries are counted, never quoted; settings values are not given); every new fact has a row (F35–F46) with a mark, Probable and Inferred parts carry OPENs (Q2, Q3, Q13 residue, Q16, Q17), and BIN questions 13–22 state build, question and the M1–M3 justification, with no network, login, launcher or anti-tamper code; section numbers of revision 3 are unchanged (new material in new subsections; §8.4 keeps its topic); invariants are testable; no closed gate is touched. All arithmetic of §13.8–§13.11 re-checked (compositing incl. partial alpha k = 162 → (161, 120, 60, 102), colour packing 0xFF7F3FFF, underline rows and columns, both truncation cases incl. cut indices and content extents, list-item advance and bullet position), and FN21, FN22, FN24 follow from §8.4, §8.5 and §7.6. Spot checks on the install (2026-10-06, all 302 layouts re-extracted read-only and byte-identical to the writer's copies): F40 15 `UICore_Text_bullet_image` values (two distinct 0x41 DIDs) and 2 `UICore_Text_bullet_spacing` (2 and 4); F41 325 `UICore_Text_truncate_text_to_fit` (320 naming three string-table entries, 5 null) and 271 `UICore_Text_auto_tooltip_truncated_text` (246 true); F42 one `UICore_Text_set_tag_font_by_property` = 0 and no other by-property value; F46 781 non-empty element-level font arrays with first entries 272/107/50/63/65/31 (588), 2,038 elements based on 0x22000002, 2,018 with a template font, inherited 177/696/403/369/76/63 (1,784), `TrajanPro18` in 16 of 75 templates; F45 all nine game objects are 101 bytes and hold one `GOP_ChatTextTypeFontColorStruct`; F44 the settings folder holds `UserPreferences.ini`, `lotro.keymap`, `PluginData` and a browser cache, with the three `UIScale_*` keys in `[UI]` and no chat-font key. BIN checks (same build): the XML element names of §9.3 and `GetUIGroupScale` occur in the executable; the underline routine computes ⌊(line height + leading)/20⌋ + 1 (at least 1), extends the right end by −*B* and fills an inclusive rectangle through a helper that overwrites, with byte → float → byte colour conversion preserving all 256 values (F38); the drawing loop adds *X* to the pen only for the first non-newline character of a line that carries the list-item flag, with or without a bullet image (F40). Non-blocking remarks for a later revision: (i) §7.5 step 1 says the flagged character "starts a line", which holds only with wrapping on; with `UICore_Text_one_line` it can sit mid-line, where the stated rules give *X* in the layout advance but not in the drawing pen (that is the client's behaviour; one sentence would make it explicit); (ii) the §1 legend dates Confirmed observations 2026-10-05, while the revision-4 OBS facts are from 2026-10-06 (the OBS row states this). Earlier records: Claude review session (spec-writer agent), re-review of revision 3, 2026-10-06: **passed**, Status moved to Accepted. Every item of `CLEAN_ROOM.md` §5.3 holds: functional facts only; no code or listing-shaped text (the one fenced block is the byte dump of the synthetic record of §13.1); identifiers are stored property, enum, DID-map and element labels used for interoperability; no game text, assets, keys or hostnames; every fact has a provenance row with a mark, and BIN questions 1–12 carry build, question and the M1–M3 justification; byte layouts give offset, size, type, meaning and units; FN1–FN19 and the §13 examples are testable (all §13 arithmetic re-checked); unknowns are OPEN (Q1–Q3, Q6–Q11, Q13–Q15); no closed gate is touched. Issues (a) and (b) and the legend item are fixed: §7.3, F28, FN18 and §13.6 now agree on the wrap-off line count, and F10 matches §6.1. Spot checks on the install (2026-10-06, all records): 48 font records (3 general, 45 local), all of size 36 + 11·*n*, 28,315 glyphs; borders 4/3/2/1/0 in 32/8/3/4/1 records with *bx* = *by*; the 95 atlases are format 28, category 13, foreground and background of equal size; packing rule holds with 703 row breaks; 122 inkless glyphs (U+0020 48, U+00A0 48, U+F00D 8, U+F000 17, U+00AF 1), 28,193 inked, 27,563 tight; foreground ink inside the bordered boxes in all 48 and outside the plain boxes only in 0x42000027 (3,259 pixels); 0x420000FD background all zero; 0x42000168–0x4200016B background equal to foreground. BIN check of question 12 (same build): the final-newline line is appended after the character loop with no test of the wrap-off flag. Earlier record: Claude review session (spec-writer agent), re-review of revision 2, 2026-10-05: changes requested. All five earlier items are fixed and the data spot checks agree (48 records, 95 atlases, 28,193 inked and 27,563 tight glyphs, the counts of `UICore_Text_one_line`, `UICore_Text_outline`, `UICore_Text_extra_leading` and both justification properties, the locale font, and the BIN break-range constants). Two remaining issues: (a) FN18 says that text with wrapping off has exactly one line, but §7.3 says that a final newline still adds the §7.1 case-2 line in that mode; §7.3 itself says both "the whole text is one line" and "adds the extra line". (b) F10 says "layout clamps it to 0–255", but §6.1 says that layout uses the values unclamped and only the drawing pass clamps. Earlier record: Claude review session (spec-writer agent), review 2026-10-05: changes requested. (1) §4.3: the count of glyphs with ink is wrong. (2) No rule and no OPEN for the metrics of an empty line (§7.1). (3) "Used" extent and the per-axis meaning of the far-side values are unclear in §7.4. (4) Probable statements without OPEN callouts: §7.3 wrap flag, §8.1 outline-flag default, §9.1 tag slot for markup, §9.3 font for each chat line. (5) The rules for consecutive spaces at a break and for the indent on wrapped lines are not stated (§7.2–§7.3). All five addressed in revision 2 (2026-10-05, see change log); awaiting re-review. Revision 3 (2026-10-06) answers (a), (b) and the minor legend item; awaiting re-review. |
| **Implements into** | `crates/formats` (font-record decoder, §3; fuzz target), `crates/material` or a new text/UI crate (glyph-atlas loading, §4; glyph lookup, §5; placement, line layout and outline rendering, §6–§8), the M5 UI crate (`tessera-ui` in the roadmap: font selection for layout elements, §9.1–§9.2), the chat panel of `apps/client` (§9.3), `crates/script` (plugin `Font` values, §9.4), `apps/explorer` (a font view: atlas with glyph boxes, metrics table), `apps/dattool` (optional font summary), `fuzz/` |
| **Companion specs** | [`records.md`](records.md) §1 (VLE, TSize, strings), §5 (render surfaces 0x41), §8 (DID maps). [`dat-container.md`](dat-container.md) §8 (routing of prefix 0x42: local, then general). [`property-system.md`](property-system.md) §4 (value encodings), §5 (property sets), §7.3 / V7 (bit *i* of a bitfield = enum key *i* + 1), §8 (root DID map), §9 (DB-object code 0x4C = font). [`texture-list.md`](texture-list.md) §6.2 (font atlases; **completed** here). [`lua-plugin-api.md`](lua-plugin-api.md) §14.1 and Appendix A.2 (plugin `Font` values; **corrected** here). [`string-table.md`](string-table.md) §4 (UTF-16 text, markup syntax). `ui-layout.md` (UI layout records, element inheritance and states; written in parallel). Tessera decision record (Accepted 2026-10-06): [`0015-ui-rendering.md`](../decisions/0015-ui-rendering.md) (UI paint and composition; **UR**), cited by §4.1, §6.3, §8.4 and §11 (revision 7). |

---

<a id="s0"></a>
## 0. Purpose

Every piece of text the original client draws (panels, tooltips, chat, quest text, floating
names) uses **bitmap fonts stored in the data**: one record per font at one pixel size,
plus one or two single-channel glyph images. This spec lets an implementer:

- decode every font record (prefix 0x42) to the last byte and load its glyph atlases (§3–§4);
- find a glyph for a character, including the client's substitution rule for missing
  characters (§5);
- place glyphs on a line, lay out lines, break them and align them the way the client does
  (§6–§7), including list-item bullets (§7.5) and truncation to fit (§7.6);
- know how a UI scale affects text (§6.3);
- draw text with and without the outline style, with underlines, and composite it with the
  client's exact arithmetic (§8);
- choose the font a given piece of text uses: UI layout elements, the quest text size
  buttons, chat by chat type, plugin labels and the client's diagnostic overlay (§9);
- decide where Tessera needs fonts of its own (§11).

Answer to the M5 roadmap question "decode the font records or document that fallback fonts
are required": **the records are fully decodable** (§3); the data fonts cover Latin, Greek
and Cyrillic script but no CJK script, and the client itself never falls back to another
font (§5, §11).

All counts are observations of one install on one date and change with patches. Fonts,
their DIDs and every table named here are **data** and must be read at run time; tests
assert the invariants of §12, not the counts.

Names of properties, enum labels, DID-map labels and UI element labels appear because they
are stored in the data and a reader looks values up by them (interoperability). Font family
names appear only as the labels of the plugin API's `Font` enum (`lua-plugin-api.md`
Appendix A.2), which are interoperability identifiers.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: observed in the user's install on 2026-10-05 by throwaway probes in the
  dirty-room scratchpad, over **all** records of the kind unless a sample is stated.
- **BIN**: read from the client executable under decision 0011 (method M4); see the BIN row.
  "Confirmed (BIN)" means the fact rests on the executable only; "Confirmed (BIN + OBS)"
  means the executable's rule and every record of the data agree.
- **Probable**: supported by names, statistics or a partial trace, not by an exact rule.
- **Inferred**: used only as a qualifier of one part of a row (F33, F34): the match between
  a stored record field and a value the client holds at run time is deduced from field
  order, not read from the code that loads the record. Every Inferred part has an OPEN
  callout (Q2).
- **Reported**: from published material only.

| Tag | Source |
|---|---|
| OBS | User install, Steam, data current on 2026-10-05. Read in full: all 48 records with prefix 0x42 (`client_general.dat` 3, `client_local_English.dat` 45); all 95 glyph-atlas surfaces they name (`client_surface*.dat`, headers and pixels); all 302 UI layouts (0x22); all 24 DID maps (0x28); the property dictionary 0x34000000; the enum mappers 0x23000026, 0x23000177, 0x2300018E, 0x230000D1, 0x230002A8/0x230002A9; the two property sets 0x7800000B and 0x78000006 in full; a scan of every record of `client_general.dat`, `client_local_English.dat` and `client_gamelogic.dat` (including all 384,950 property sets) for the property IDs of §9 stored with their echo. **Revision 4 (2026-10-06):** all 302 UI layouts again (census of `UICore_Text_truncate_text_to_fit`, `UICore_Text_auto_tooltip_truncated_text`, `UICore_Text_bullet_image`, `UICore_Text_bullet_spacing`, the "by property" font properties, and the first entry of every `UICore_Text_fonts` array, with inheritance from the text-style library); the nine game objects of §9.3 (0x70035169 …, read in full); the owner's settings folder inside the Steam prefix (`install-layout.md` §8.1), read only: file names and the key names of `UserPreferences.ini`, no values beyond the three `UIScale_*` keys. |
| BIN | Client executable `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`), read with a disassembler (objdump on selected address ranges, plus small scripts that search the code for constants and call sites) on 2026-10-05. **Questions:** (1) how the client reads the font record and what each field means; (2) how a glyph is found, what happens to a missing character, and whether there is kerning; (3) how glyphs are placed and lines laid out, broken and aligned; (4) how the outline style is drawn and with which atlas; (5) how a UI text element, the quest text, chat and the diagnostic overlay choose their font. **Why M1–M3 were not enough:** the data gives the numbers but not their roles (for example, which trailer value is the baseline and which two are borders, whether the 5 metric bytes are bearings or offsets) and contains no rule for substitution, line breaking, outline drawing or font choice; no published source describes the LOTRO record correctly (PUB-RU below disagrees with the data); M3 (watching the running client) can show results but not the rules, and is not available in this session. Only offline text and UI code was read; nothing about network, login, launcher, keys or anti-tamper. **Revision 2 (same build, 2026-10-05), further questions:** (6) the metrics of a line without characters and of text that ends in a newline; (7) what a line break consumes, and whether the negative-bearing indent applies to wrapped lines; (8) the exact extents that horizontal and vertical justification compare; (9) the defaults of the wrap and outline flags and of the font slots; (10) which characters take the tag font slot; (11) whether each chat line is drawn in its chat type's font. **Why M1–M3 were not enough:** the data holds only property values (for example 246 `UICore_Text_one_line` values), not what the client does with them; these are layout and drawing rules that exist only in code, and M3 is not available. Read: the UI text element's property handler, constructor, character-list builder, line layout, alignment and drawing routines, and the chat-log routines that append a line. The break ranges of §7.3 are stored in the code as a 16-bit start plus a span (for example U+4E00 and 0x51BF), which is why a search for the range end values as immediates does not find them. **Revision 3 (same build, 2026-10-06), further question:** (12) whether the extra line after a final newline is also added when wrapping is off. **Why M1–M3 were not enough:** as for questions 6–11 (a layout rule with no trace in the data). Read: the end of the same line-layout routine; the final-newline check runs after the character loop and does not test the wrap-off flag. **Revision 4 (same build, 2026-10-06), further questions:** (13) how a UI scale reaches text (Q3); (14) the exact arithmetic that composites a glyph's coverage and colour into the paint canvas (Q6); (15) what the underline, bold and italic markup flags do when drawing; (16) where a list-item bullet is drawn and which character carries its extra width (Q15); (17) how `UICore_Text_truncate_text_to_fit` shortens text (Q13); (18) which object a "by property" font source reads (Q7); (19) what reads `UI_Options_Menu_FontIndex` (Q11); (20) whether the chat-type table has a local file form (Q9); (21) whether the nine chat-type game objects are named by the client (Q10); (22) whether the client draws any other text style (shadow) or uses system fonts. **Why M1–M3 were not enough:** the data holds only the property values; scaling, compositing, underline geometry, bullet placement and truncation exist only in code, and M3 (screenshots of the running client) is not available in this session. Read: the text element's paint and draw routines, its three mouse hit-test routines, the glyph blit, the two-operand image combiner and its A8-onto-32-bit pixel loop, the fill-rectangle helper, the character-list builder and markup-tag reader, the line layout (list-item path), the truncation routines, the font-from-struct routine and its per-slot property table, the gameplay-options XML writer, and the client's GDI imports. **Revision 5 (same build, 2026-10-06), further questions:** (23) how the UI scaler and the resize routine size a scaled panel, and which box the element geometry, hit tests and clamps use (Q3 context; answered in `ui-behaviour.md` §9.2–§9.2.1, BIN); (24) where the text widget clears its truncation switch. **Why M1–M3 were not enough:** as for questions 13–22. Read: the UI-scaler routine and its resize-in-Stretch helper, the base resize routine, the displayed-box update, and the text widget's property-change handler. **Revision 6 (same build, SHA-256 prefix d22363eb9aff0fb5, 2026-10-06), further question:** (25) which scale the text element's mouse hit tests divide by when the element has no display node of its own (answers the revision-5 review). **Why M1–M3 were not enough:** the scale lookup is code only; the data holds no display nodes, and M3 is not available in this session. Read: the text widget's mouse-position conversion step (the only use of the element scale lookup in the text widget code), the element's scale lookup in every UI element class (one class family carries an identical copy), and the ancestor search it calls. |
| PUB-AC | Asheron's Call font record as decoded by the ACE server project and the DatReaderWriter library (both read for facts only): DID, max height, max width, count, 11-byte glyph descriptors, two border counts, a baseline offset, foreground and background surface DIDs. **Reported for AC only**; used as a naming hint. The LOTRO header and trailer match it; the LOTRO glyph entry stores its fields in a different order (§3.2). |
| PUB-RU | The "mojo" font converter of the Elanor translation launcher (GPL repository, read for facts only), which writes LOTRO font records from bitmap-font files. Its writer puts 5 extra bytes after a 16-byte header and a 15-byte trailer. That arithmetic gives the right total size (36 + 11·*n*) but assigns every metric to the wrong character; observation contradicts it (F3). Superseded. |
| SPEC | `lua-plugin-api.md` P12 (font enum values are the 48 font DIDs), `texture-list.md` T9, `records.md` §5.4 (category 13 occurs only on 95 A8 surfaces), `property-system.md` §9 (code 0x4C, 6 values), `string-table.md` §4.7–§4.8 (markup kinds) and §10.3 (locale record 0x58000000: a font DID, then a 32-bit value "read by UI text"). |
| UR | Decision record 0015 (Accepted 2026-10-06). It decides Tessera's behaviour only and is the source of no fact about the original; the paragraphs that cite it are marked Tessera Decision or Tessera default (revision 7). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| F1 | Fonts are records with prefix 0x42; 48 exist; 3 in general and 45 in local; DB-object code 0x4C (§3.1) | OBS; SPEC; BIN (the font loader asks for code 0x4C) | Confirmed |
| F2 | Record = 16-byte header, *n* 11-byte glyph entries, 20-byte trailer; size = 36 + 11·*n* (§3.2) | OBS (48 of 48, zero leftover bytes); BIN (the client's reader takes exactly these fields in this order) | Confirmed (BIN + OBS) |
| F3 | Glyph entry order: width, height, left bearing, right bearing, top offset, then code, atlas x, atlas y (§3.2) | OBS (atlas x-step rule holds for all 27,564 same-row neighbouring pairs, 0 exceptions; the opposite grouping fails); BIN (the glyph lookup reads the code at entry offset 5 and the metrics at 0–4) | Confirmed (BIN + OBS) |
| F4 | Header: line height = largest glyph height; width field = largest glyph width (§3.3) | OBS (48 of 48); BIN (line height feeds layout) | Confirmed |
| F5 | Trailer: horizontal and vertical outline border, baseline, foreground atlas DID, background atlas DID or 0 (§3.3) | OBS (48 of 48; flat-bottomed letters end on the baseline in 47 records); BIN (border and baseline use, §6–§8) | Confirmed (BIN + OBS) |
| F6 | Atlas packing: rows, border spacing, greedy row breaks, row height (§4.2) | OBS (48 of 48 atlases, 703 of 703 row breaks) | Confirmed |
| F7 | Atlases are A8 surfaces of category 13; foreground = glyph coverage, background = outline coverage; same size (§4.1) | OBS (95 of 95; pixel census); BIN (outline pass reads the background atlas) | Confirmed |
| F8 | Codes are UTF-16 code units, strictly ascending, first U+0020; every font has `?` (§3.3, §5) | OBS (48 of 48) | Confirmed |
| F9 | Lookup by code unit; a missing code is replaced by `?` from the same font; if that is missing too, nothing is drawn and layout advances 1 pixel (§5) | BIN | Confirmed (BIN) |
| F10 | Pen advance = left bearing + width + right bearing; line layout and the drawing pen use the three values unclamped; the per-character sum clamped to 0–255 is stored for the underline extent and the truncation measure only; no kerning (§6.1; corrected in revision 4) | BIN; OBS (all advances 0–37) | Confirmed (BIN) |
| F11 | Glyph box at pen + left bearing, line top + line baseline − font baseline + top offset (§6.2) | BIN | Confirmed (BIN) |
| F12 | Line height, line baseline and leading rules (§7.1) | BIN | Confirmed (BIN) |
| F13 | Indent of a line whose first glyph has a negative left bearing: 1 − bearing, on every line (first line, after a newline, after a wrap) (§7.2) | BIN | Confirmed (BIN) |
| F14 | Break opportunities and the characters a line may not start with (§7.3) | BIN | Confirmed (BIN) |
| F15 | Justification values and offsets; the extents compared on each axis (§7.4) | BIN; OBS (enum 0x23000177; the layouts use 2 (Top) once horizontally and 4 (Right) 12 times and 5 (Left) twice vertically) | Confirmed (BIN) |
| F16 | Two passes: all outlines, then all fills; outline from the background atlas grown by the borders, or eight 1-pixel-offset stamps when there is no background atlas; default outline colour opaque black (§8) | BIN | Confirmed (BIN) |
| F17 | Two font slots per text element (normal, tag), each set from an array, a struct field, a single value or a property indirection; tag slot falls back to the normal one (§9.1) | BIN; OBS (property census) | Confirmed (BIN) |
| F18 | Font-array index; the quest panels' increase/decrease buttons step a session index starting at 0 (§9.2) | BIN; OBS (ladders in layouts 0x22000025, 0x22000043, 0x22000067, 0x22000842) | Confirmed (BIN) |
| F19 | Chat: per chat type a size step and a colour; size step → font DID through the UNIQUEDB label `UI_ChatFonts`; every chat line is appended in the font of its chat type's size step and in its colour (§9.3) | OBS (0x7800000B, 0x78000006); BIN (both tables are built by the client; three chat-log append routines set colour and both font slots before appending) | Confirmed (BIN) |
| F20 | FONT DID map: four named fonts; only key 2 is used, by render-diagnostic overlays (§9.5) | OBS (0x2800000B); BIN | Confirmed (BIN) |
| F21 | Surface category 13 is given to images whose source path is under a `ui\fonts` folder or whose name starts with `font_`; 15 to images under `surfaces\ui\`; both tests ignore case (§14) | BIN; OBS (category 13 = exactly the 95 atlases) | Confirmed (BIN + OBS) |
| F22 | Text-style library layout 0x22000002 (77 templates); 2,038 elements inherit from it (§9.1) | OBS | Confirmed |
| F23 | Glyph coverage per family; no CJK, no U+FFFD (§10) | OBS | Confirmed |
| F24 | `UI_NPC_LogFont`, `GOP_TextType*` and `PreprocOptions_FontGen_*` have no values in the data; the client code names none of the first two groups (§9.6) | OBS (scan); BIN (immediate search) | Confirmed |
| F25 | Line width = pen position after the line's last character minus that character's trailing value *T* (§7.2) | BIN | Confirmed (BIN) |
| F26 | Metrics of a line that holds only a newline, of the extra line after a final newline, and of empty text; the last line's leading is dropped unless `UICore_Text_last_line_leading` is set (§7.1) | BIN; OBS (`UICore_Text_last_line_leading` has no value in any layout) | Confirmed (BIN) |
| F27 | A break consumes exactly one space, tab or carriage return, never after a newline; other spaces of a run stay on their lines (§7.3) | BIN | Confirmed (BIN) |
| F28 | Wrapping is switched off by `UICore_Text_one_line`; in that mode neither the width nor a newline ends a line, so all characters are on the first line; a final newline still adds the extra line of §7.1 case 2 (§7.3) | BIN; OBS (246 true, 23 false values) | Confirmed (BIN) |
| F29 | A word wider than the line is broken before the glyph that would overflow (§7.3, closes Q5) | BIN | Confirmed (BIN) |
| F30 | Newline, tab and carriage return take the `?` substitute in layout; only the newline is skipped when drawing (§5, closes Q4 for text elements) | BIN; OBS (no font has a glyph below U+0020) | Confirmed (BIN) |
| F31 | Outline flag and wrap-off flag default to off (§7.3, §8.1) | BIN (text-element constructor) | Confirmed (BIN) |
| F32 | Only text inside link markup takes the tag font slot and tag colour; a link's own colour wins (§9.1) | BIN (character-list builder) | Confirmed (BIN) |
| F33 | Both font slots of a new text element start with the locale record's font (0x42000028 in English); a font DID of 0 or one that does not resolve leaves the slot's font unchanged (§9.1, closes Q12) | BIN; OBS (0x58000000 font field, `string-table.md` §10.3) | Confirmed (BIN); link from record field to locale-object field Inferred (as in `string-table.md` §10.3) |
| F34 | Leading factor *f* = locale leading value + `UICore_Text_extra_leading`, raised to 0 if negative (§7.1, narrows Q2) | BIN; OBS (locale value 0 in English; `UICore_Text_extra_leading` occurs once, 0.0) | Confirmed (BIN); record field Inferred |
| F35 | UI scale: text is laid out and painted at the fonts' own pixel sizes in the element's unscaled coordinates (its layout box); the scale vector of the nearest display node at or above the element (its own node, else the nearest ancestor's, else (1, 1, 1)) is applied when the painted result is shown (the displayed box); mouse positions are always divided by that scale, *x* by *x* and *y* by *y*, for text hit tests (items 1 and 3 corrected in revision 6, BIN question 25); no font is swapped and no metric is scaled for a scale; element geometry under a scale is `ui-behaviour.md` §9.2.1 (§6.3; item 4 corrected in revision 5) | BIN; OBS (`UIScale_*` keys in the settings file; `GOP_UIScale_*` and `UIScaleDefinition_*` properties) | Confirmed (BIN); display filter OPEN (Q3) |
| F36 | Glyph and outline compositing arithmetic onto a 32-bit ARGB canvas, including the colour-alpha factor and the transparent-destination rule (§8.4) | BIN | Confirmed (BIN) |
| F37 | Colour properties (four floats) are packed to ARGB bytes by multiplying by 255 and truncating (§8.4) | BIN | Confirmed (BIN) |
| F38 | Underline (`<u>`): drawn in the fill pass before the glyph; thickness ⌊(line height + leading) / 20⌋ + 1, at least 1; rows from baseline + *t* to baseline + 2*t* − 1; columns from the pen to pen + advance (− *B* when *B* < 0), inclusive; overwrites the canvas (§8.5) | BIN | Confirmed (BIN); colour source partly OPEN (Q16) |
| F39 | Bold (`<b>`) and italic (`<i>`) set character flags that no text routine tests: no visible effect; no shadow style exists; the text element never calls the system font functions (§8.6) | BIN (flag census of the builder, layout, paint and blit; GDI imports and their callers); OBS (no shadow property for text in the dictionary) | Confirmed (BIN); "no game text in a system font" Probable |
| F40 | List items: the first character after `<li>` carries the bullet flag and an extra width = bullet image width (0 without image) + `UICore_Text_bullet_spacing`; layout adds it to that character's overflow test and advance; drawing puts the bullet image at the line start with its bottom row on the baseline (§7.5, closes Q15) | BIN; OBS (15 elements with a bullet image, 2 with a spacing: 2 and 4) | Confirmed (BIN) |
| F41 | Truncation: suffix text from `UICore_Text_truncate_text_to_fit` (a StringInfo), measured once; the line that is cut, the cut index rule, the measured extents, the draw-time switch to the suffix; `UICore_Text_auto_tooltip_truncated_text` marks the element as truncated; the switch is cleared whenever `UICore_Text_editable` or `UICore_Text_selectable` becomes true (§7.6, closes Q13; this spec is the only statement of the rule) | BIN (revision 5: the clearing); OBS (325 values on 324 elements and 1 state: 319 name one of two entries of one string table, 1 an entry of a second table, 5 null; 271 auto-tooltip values, 246 true; re-counted 2026-10-06) | Confirmed (BIN); where the switch is set Probable |
| F42 | "By property" font sources read a member of the element's own font-struct property; the tag slot falls back to the normal struct; `UICore_Text_set_font_by_property` sets both slots (§9.1, closes Q7) | BIN (per-slot property table and font-from-struct routine); OBS (one value in the data: `UICore_Text_set_tag_font_by_property` = 0) | Confirmed (BIN) |
| F43 | `UI_Options_Menu_FontIndex` is read from a menu item when the item is chosen (§9.6, narrows Q11) | BIN (about ten routines read it as an enum property of a UI element) | Confirmed (BIN); what the handler then sets OPEN |
| F44 | The chat-type table can be written to and read from an XML options file (sections Aliases, Layouts, Options, ChatColors; one ChatFont element per entry with TextType, Color, Size); the settings folder holds no chat-font key (§9.3, narrows Q9) | BIN (writer and reader routines, element names); OBS (settings folder 2026-10-06) | Confirmed (BIN); file location and value encoding OPEN |
| F45 | The nine game objects with a chat-type struct hold nothing else, and the client code names none of their DIDs (§9.3, narrows Q10) | OBS (all nine, 101 bytes each); BIN (immediate search) | Confirmed |
| F46 | Fonts most used by native panels: first entries of the layouts' font arrays and fonts inherited from the text-style library (§9.1) | OBS | Confirmed |

---

<a id="s2"></a>
## 2. Overview

| Piece | Where | § |
|---|---|---|
| Font record (metrics, glyph table, atlas DIDs) | prefix 0x42; `client_local_English.dat` (45) and `client_general.dat` (3) | §3 |
| Glyph atlases | two 0x41 render surfaces per font (only one for the font without an outline atlas), A8, category 13 | §4 |
| Font choice for UI text | UI layout records 0x22: element properties `UICore_Text_fonts` and relatives, often inherited from the text-style library 0x22000002 | §9.1–§9.2 |
| Font choice for chat | property set named `UI_ChatFonts` (UNIQUEDB) and the gameplay-option defaults (`GameplayOptionDefaults`) | §9.3 |
| Plugin labels | plugin `Font` values are font DIDs | §9.4 |
| Diagnostic overlay | DID map `FONT` (root key 0x10) | §9.5 |

The client rasterises text at the font's own pixel size; there are no scalable outlines in
the data. Bold styles are separate records; there is no italic; the only styles the renderer
adds are the outline and the underline (§8). The `<b>` and `<i>` markup tags change nothing
visible (§8.6).

---

<a id="s3"></a>
## 3. Font record (prefix 0x42)

<a id="s3-1"></a>
### 3.1 Location and identity (Confirmed)

- 48 records: 0x42000027, 0x42000028 and 0x420000FD in `client_general.dat`; the other 45
  in `client_local_English.dat`. No DID occurs twice. Routing follows `dat-container.md` §8
  (local first, then general, for prefix 0x42).
- All 48 are stored uncompressed in this install; readers must accept either storage form
  (`dat-container.md`).
- The DB-object code for fonts is 0x4C (`property-system.md` §9). Every property whose value
  is a font DID declares that code (§9).
- Font DIDs are stable identifiers: the plugin API exposes all 48 by name
  (`lua-plugin-api.md` Appendix A.2), and the client wires those names to these DIDs (BIN).
  The directory time stamps of the current iterations are from 2007–2008 for 32 records and
  from 2018–2019 for 16 (0x420000F0–0x420000FA, 0x420000FD, 0x42000168–0x4200016B).

> **OPEN:** Installs with another language archive (`client_local_DE.dat`,
> `client_local_FR.dat`) were not available. Whether they carry their own 0x42 records with
> the same DIDs is unknown; a reader must take whatever the routed archives hold.

<a id="s3-2"></a>
### 3.2 Byte layout (Confirmed, 48 of 48 records, zero leftover bytes)

All integers little-endian. The payload is what the container returns for the DID
(`dat-container.md`), starting with the DID itself.

**Header (16 bytes)**

| Offset | Size | Type | Meaning | Units |
|---|---|---|---|---|
| 0 | 4 | u32 (DID) | The record's own DID (0x42xxxxxx) | |
| 4 | 4 | u32 | **Line height**: the height of the line box. Equals the largest glyph height in the record (F4). | pixels |
| 8 | 4 | u32 | **Widest glyph**: the largest glyph width in the record (F4) | pixels |
| 12 | 4 | u32 | **Glyph count** *n* | entries |

**Glyph entries (*n* × 11 bytes), starting at offset 16**

Entry *i* starts at offset 16 + 11·*i*.

| Offset in entry | Size | Type | Meaning | Units |
|---|---|---|---|---|
| 0 | 1 | u8 | **W**: width of the glyph box | pixels |
| 1 | 1 | u8 | **H**: height of the glyph box | pixels |
| 2 | 1 | i8 | **A**: left bearing, the horizontal offset from the pen position to the left edge of the box; may be negative | pixels |
| 3 | 1 | i8 | **B**: right bearing, the space from the right edge of the box to the next pen position; may be negative | pixels |
| 4 | 1 | i8 | **T**: top offset, the distance from the top of the line box down to the top of the glyph box. The client reads it as signed; all observed values are 0–31. | pixels |
| 5 | 2 | u16 | **Code**: the UTF-16 code unit this glyph draws | |
| 7 | 2 | u16 | **X**: left column of the glyph box in the atlases | pixels |
| 9 | 2 | u16 | **Y**: top row of the glyph box in the atlases | pixels |

**Trailer (20 bytes), at offset 16 + 11·*n***

| Offset in trailer | Size | Type | Meaning | Units |
|---|---|---|---|---|
| 0 | 4 | u32 | **Horizontal outline border** *bx*: how far the background (outline) image of a glyph extends left and right of its box; also half the horizontal gap between boxes in the atlas | pixels |
| 4 | 4 | u32 | **Vertical outline border** *by*: the same, above and below | pixels |
| 8 | 4 | u32 | **Baseline**: distance from the top of the line box down to the baseline | pixels |
| 12 | 4 | u32 (DID) | **Foreground atlas**: 0x41 surface with the glyph coverage | |
| 16 | 4 | u32 (DID) | **Background atlas**: 0x41 surface with the outline coverage, or **0** when the font has none | |

Total size: 36 + 11·*n* bytes. Observed sizes: 2,137 (*n* = 191), 2,500 (224), 5,503 (497),
7,417 (671), 7,516 (680) and 15,645 (1,419).

The header and trailer have the same shape as the Asheron's Call font record (PUB-AC); the
glyph entry does not: AC stores the code and position first, LOTRO stores the five metric
bytes first (F3). A reader built from the AC layout assigns every metric to the wrong field.

<a id="s3-3"></a>
### 3.3 Field facts (Confirmed, all 48 records)

- Glyph codes are **strictly ascending** and therefore unique. The first code is always
  U+0020. Every font contains U+003F `?` (§5 depends on it).
- 0 ≤ *T* and *T* + *H* ≤ line height for every glyph.
- Baseline < line height. The flat-bottomed capitals `H I E Z` end exactly on the baseline
  (*T* + *H* = baseline) in 47 fonts; round letters and digits may overshoot it by one
  pixel. The exception is FixedSys15 (0x42000027), whose glyphs all fill a full 8 × 15 cell
  that ends 3 pixels below the baseline.
- *bx* = *by* in every record. Values: 4 (32 records), 3 (8), 2 (3), 1 (4), 0 (1, the
  record without a background atlas).
- 0 ≤ *A* + *W* + *B* ≤ 37 for every glyph; *A* ranges −7…12, *B* −17…11, *H* 1…36,
  *W* 0…41. *W* = 0 occurs only for code U+F00D in the eight `Verdana*` records.
- 28,315 glyph entries in total.
- The space U+0020 always has *H* = line height, *T* = 0, *A* = 0, *B* = 1, and an empty
  (all-zero) box in the atlas.

<a id="s3-4"></a>
### 3.4 Decoding (in words) and limits for hostile input

1. Need at least 36 bytes. Read the DID, line height, widest-glyph width and count *n*.
2. Check that the payload length equals 36 + 11·*n* exactly before allocating anything;
   otherwise report the record as malformed. (This bounds *n* by the payload.)
3. Read the *n* entries in the order of §3.2. Readers should report, as warnings, entries
   that break §3.3 (codes not ascending, *T* + *H* above the line height, a box outside the
   atlas, §4.3) but should still decode them.
4. Read the trailer. A background DID of 0 means "no background atlas". Any other value is
   a surface DID to be resolved through the archive set.
5. A suggested hard cap: *n* ≤ 65,536 (a code is 16 bits, so no valid record exceeds it).

---

<a id="s4"></a>
## 4. Glyph atlases

<a id="s4-1"></a>
### 4.1 Surfaces (Confirmed, 95 of 95)

- Each font names a foreground atlas; 47 also name a background atlas. All 95 exist, all are
  render surfaces (`records.md` §5) with format code 28 (A8, one byte per pixel) and
  category 13. No other surface in the install has category 13 (`records.md` §5.4).
- The foreground and background atlas of a font have the same width and height. Widths are
  128, 256, 512 or 1024; heights are not powers of two (123–728).
- Pixels are rows top first (`records.md` §5.3). The byte is **coverage**: 0 = no ink,
  255 = full ink.
- **Foreground**: the glyph shapes. Each glyph's ink lies inside its box (*X*, *Y*,
  *W* × *H*).
- **Background**: the same glyphs grown outwards by up to the border (a soft halo used for
  the outline style, §8). Its ink lies inside the **bordered box**
  (*X* − *bx*, *Y* − *by*, *W* + 2·*bx*, *H* + 2·*by*).

**Tessera Decision (UR §3).** The atlases are decoded once, on the asset workers, into CPU-side
coverage buffers (one byte per pixel) that the UI painter shares; they are read only by the CPU
compositing of §8 and are never uploaded to the GPU.

<a id="s4-2"></a>
### 4.2 Packing rule (Confirmed; needed only to build synthetic fixtures)

The atlases of all 48 fonts were packed by the same rule:

1. Glyphs are placed in code order, left to right, in rows.
2. The first glyph of each row has *X* = *bx*; the first row has *Y* = *by*.
3. Within a row, the next glyph's *X* = previous *X* + previous *W* + 2·*bx*.
4. A new row starts exactly when the next glyph would not fit, that is when its *X* + *W* +
   *bx* would exceed the atlas width.
5. The next row's *Y* = this row's *Y* + the largest *H* in this row + 2·*by*.
6. The atlas height = last row's *Y* + its largest *H* + *by*.

Consequently no two bordered boxes overlap and every bordered box lies inside the atlas.

<a id="s4-3"></a>
### 4.3 Observed exceptions (Confirmed)

| Font | What differs | Effect |
|---|---|---|
| 0x4200002A (`TrajanProBold16`) | Background DID 0; borders 0 | Outline style uses the stamped outline of §8.3 |
| 0x420000FD (`VerdanaBold16`) | Background atlas exists but every pixel is 0 | Outline style draws nothing visible |
| 0x42000168–0x4200016B (`BookAntiqua26/28/32/36`) | Borders 1; background atlas is a byte-for-byte copy of the foreground | The "outline" is the glyph itself drawn in the outline colour under the fill |
| 0x42000027 (`FixedSys15`) | Foreground ink also fills part of the border ring (3,259 faint pixels), none outside the bordered box | Draws as it is; no special case |
| 0x42000028 (`LucidaConsole12`) | Background halo is wider than its 2-pixel border (1,620 pixels outside the bordered boxes) | Those pixels are never sampled |

Of the 28,315 glyphs, 122 have no ink: U+0020 (48), U+00A0 (48), U+F00D (8, `Verdana*`),
U+F000 (17, `BookAntiqua*`) and U+00AF (1, FixedSys15). The other 28,193 have ink, and for
27,563 of them the glyph box is exactly the tight bounding box of the ink.

---

<a id="s5"></a>
## 5. Finding a glyph (Confirmed (BIN))

Text is a sequence of UTF-16 code units (`string-table.md`). The client looks up **each
code unit** on its own:

1. Find the entry whose code equals the unit. (The client builds a table indexed by
   code − smallest code, holding the entry index or "none"; any lookup structure with the
   same result is fine.)
2. If there is none and the unit is not `?` (U+003F), look up `?` instead. The substituted
   glyph is drawn and advances like a real `?`.
3. If `?` is missing too, there is no glyph: nothing is drawn. The line layout then treats
   the character as a blank glyph with *W* = *H* = 1, *A* = *B* = *T* = 0 (advance 1 pixel),
   while the drawing pass advances 0. This cannot happen with the fonts of this install
   (every font has `?`).

Consequences:

- The client **never** takes a glyph from another font. Any character a font lacks shows
  as `?` (for example all CJK text, §10).
- A character outside the Basic Multilingual Plane is a surrogate pair, so it shows as two
  `?`.
- No font has a glyph below U+0020, so every control character gets the `?` substitute of
  step 2 when a text element builds its character list. The newline (U+000A) is
  special only in two places: it ends the line (§7.3), and the drawing pass skips it (it
  is not drawn and does not move the drawing pen). In **layout** it keeps the `?` metrics,
  so its advance counts towards the width of the line it ends (§7.2, example §13.6).
- A tab (U+0009) or carriage return (U+000D) that reaches a text element is laid out
  **and drawn** as `?` (both are break opportunities, §7.3). Whether text from a given
  source can still contain tabs at that point (for example after the string-table
  whitespace handling of `string-table.md`) is a question for that source, not for the
  font code.

---

<a id="s6"></a>
## 6. Placing glyphs

<a id="s6-1"></a>
### 6.1 Advance and kerning (Confirmed (BIN))

- The pen moves right by *A* + *W* + *B* after each glyph. Line layout (§7) and the drawing
  pen both use the three values as they are (the drawing pen takes the sum from the glyph
  itself; a character without any glyph, §5 step 3, advances the drawing pen by 0). The
  client also stores the sum per character clamped to 0–255 (a negative sum becomes 0);
  that stored value is used only for the underline extent (§8.5) and for measuring
  truncation (§7.6). All observed advances are 0–37, so the clamp never acts.
- There is **no kerning**: no pair table exists in the data and the client adds nothing that
  depends on the neighbouring glyph. Overlap between neighbours comes only from negative
  bearings.

<a id="s6-2"></a>
### 6.2 Glyph box on screen (Confirmed (BIN))

Let a line have its top at *y*_line and its baseline *L* pixels below that (§7.1), and let
the pen be at *x*. A glyph of a font with baseline *b* is drawn:

- left edge *x* + *A*, top edge *y*_line + *L* − *b* + *T*;
- size *W* × *H*, sampled 1 : 1 from the foreground atlas at (*X*, *Y*).

So glyphs of different fonts on one line share the line's baseline. All positions are whole
pixels; glyphs are never scaled or filtered during layout and drawing. A UI scale acts
later, on the painted result (§6.3).

<a id="s6-3"></a>
### 6.3 UI scale (Confirmed (BIN); display filter OPEN)

The original offers UI scale options (the settings-file keys `UIScale_Global`,
`UIScale_Cursor`, `UIScale_Pregame` in section `[UI]`, and per-panel-group gameplay options
`GOP_UIScale_*` bounded by `UIScaleDefinition_*` records; plugins can read a group's scale
through `GetUIGroupScale`; those names are interoperability identifiers). For text the
client behaves as follows:

1. Every UI element has a **scale** (x, y, z): the scale vector of the nearest **display
   node** at or above the element. That is the element's own display node if it has one
   (root panels, the tooltip and drag-icon panels); otherwise the display node of the
   nearest ancestor that has one, found by following parents; (1, 1, 1) only when neither the
   element nor any ancestor has a display node. A text element inside a scaled panel
   normally has no node of its own, so its scale is the panel's scale. This is the
   (*ex*, *ey*) of `ui-behaviour.md` §9.2.1, the same scale that sets the element's displayed
   size (item 4) and that the point-in-element test of `ui-behaviour.md` §4.1 uses
   (corrected in revision 6; BIN question 25).
2. A text element lays out and paints its text exactly as §5–§8 describe, at the fonts'
   own pixel sizes and in the element's own **unscaled** pixel coordinates. Neither the
   layout, nor the glyph blit, nor the font choice reads the scale: no larger font is
   chosen for a larger scale, and no metric is multiplied.
3. The scale is applied when the painted element is shown. The text element's mouse hit
   tests (character under the mouse, link under the mouse, selection) all convert a mouse
   position relative to the element's displayed box into text coordinates through one
   step: divide *x* by the scale's *x* and *y* by the scale's *y*, where "the scale" is the
   element's scale of item 1 (32-bit floats), truncate each towards zero, and add the
   element's horizontal and vertical scroll offsets. The step always divides; it has no
   tolerance test (unlike the alpha test of `ui-behaviour.md` §4.1 step 3, which skips the
   division when every component is within 0.0002 of 1). The *z* component is not used.
4. Element geometry uses the same factor, outside the text code; `ui-behaviour.md` §9.2 and
   §9.2.1 are the single statement of it (corrected in revision 5). In short: every element
   has a **layout box** (`ui-layout.md` §8, unscaled) and a **displayed box**; the UI scaler
   resizes a scaled panel in object mode Stretch, which keeps the panel's layout box at its
   unscaled size and sets its scale vector, so a text element inside a scaled panel keeps
   its layout box, in which this spec lays out text, whatever the UI scale. The displayed
   size of every element under a scale is trunc(max(0, (size + 1) × scale − 1)) on each axis
   (the formula revision 4 quoted for children that follow their parent's scale). The earlier sentence "the scaler sets a
   panel's own size to its screen size ÷ scale" is withdrawn: the scaler requests the size
   unscaled size × scale, and that request changes the scale vector, not the layout box.

So scaled text is a magnified or reduced image of the 1 : 1 text, not text re-rendered at
another size. **Tessera Decision (UR Decision 1, §2.2):** paint the text of a scaled panel
at 1 : 1 into the CPU canvas of the surface the element is drawn into (§8.4), together with
the panel's art, and let the GPU compose that surface scaled onto its displayed box; do not
pick larger fonts and do not re-rasterise text at the scaled size.

> **OPEN (Q3, narrowed):** the filter used when the scaled result is shown (nearest or
> linear) and whether the whole panel or each element is resampled. Suggested method: M3,
> an owner screenshot of the chat window at UI scale 1.0, 1.5 and 0.5 (the last for
> smoothing of a reduced panel). **Tessera default (UR §2.2)** until it is answered: each
> surface is resampled as a whole, not element by element; at scale 1 the canvas is copied
> exactly; otherwise a bilinear filter on premultiplied texels with clamp-to-edge, and no
> mip chain; Tessera's setting `ui_scale_filter = linear | nearest` selects the filter.

---

<a id="s7"></a>
## 7. Laying out lines (Confirmed (BIN))

A text element lays out its whole text in one pass over its characters, in text order.
Every character carries the font of the slot it falls in (§9.1) and that font's glyph
for it (§5, including the `?` substitute). "Available width" is the element's width
minus its left and right text margins (`UICore_Text_left_margin`,
`UICore_Text_right_margin`; where their values come from belongs to `ui-layout.md`).

<a id="s7-1"></a>
### 7.1 Line metrics

For each line, over the characters assigned to it (§7.3 says which ones):

- **Line height** = the largest font line height (header offset 4; the client keeps it
  per character limited to 0–255) among the characters on the line.
- **Line baseline** *L* = the largest font baseline (trailer offset 8) among the characters
  on the line.
- **Leading** = the line height × *f*, computed in single precision and truncated towards
  zero to an integer.
- The next line's top = this line's top + line height + leading.

**Leading factor.** *f* = *f*₀ + *E*, raised to 0 if the sum is negative, where:

- *E* is the element's Float property `UICore_Text_extra_leading` (0.0 when the element
  has none; it occurs once in the layouts, with value 0.0);
- *f*₀ is a locale value the element copies when it is created: the client's locale
  object provides it, and by the field order of `string-table.md` §10.3 it is the 32-bit
  field that follows the font DID in the locale record (0x58000000 here), read as an IEEE
  single-precision number. That field is 0 in the English record, so *f*₀ = 0.0 and *f*
  = *E* in this install.

> **OPEN (Q2, narrowed):** the match between that locale-record field (and the font DID
> before it, §9.1) and the locale values the text element copies is Inferred from field
> order (as in `string-table.md` §10.3), not read from the record loader. Only the value 0
> is observed for *f*₀, so nothing visible depends on it in this install.

**Text height** = the sum of (line height + leading) over all lines, **minus the last
line's leading**, unless the element's Bool property `UICore_Text_last_line_leading` is
true (default false; no UI layout of this install sets it). The client also zeroes the
last line's stored leading in that case.

**Lines without visible characters.** Line metrics come only from the characters on the
line, so the following cases are fixed by the client as stated:

1. **Blank line** (a line that holds only a newline, for example between two consecutive
   newlines): the newline is a character of that line, so the line takes the newline's
   font: line height and baseline of that font, leading = that height × *f*. Its width
   follows §7.2 with the `?` metrics of that font (with *B* ≥ 0: *A* + *W* of `?`, plus an
   indent if that *A* is negative). The newline's font is the font of the slot it
   falls in: the normal slot, or the tag slot inside link markup (§9.1).
2. **Text that ends in a newline** gets one extra, final line after that newline. It has
   no characters, width 0, line height = the line height of the final newline's font,
   **baseline 0 and leading 0**. Its height counts in the text height. Because its leading
   is 0, removing "the last line's leading" removes nothing, so the line before it keeps
   its leading.
3. **Empty text** (no characters) has exactly one line whose width, height, baseline and
   leading are all 0. The text height is 0.
4. A line with no characters can also arise from the forced-break edge case of §7.3 (the
   first glyph of the text is wider than the available width). It has width 0, height 0,
   baseline 0 and leading 0.

The newline that ends a line also counts towards that line's height and baseline (it is
the line's last character), so a newline in a larger font makes the line it ends taller.

<a id="s7-2"></a>
### 7.2 Line start, indent and line width

- A line is drawn starting at the text area's left edge plus the alignment offset of §7.4
  plus an **indent** that is normally 0.
- If the **first character of the line** has a negative left bearing *A*, the indent is
  1 − *A*. The glyph's ink then starts one pixel right of the line start instead of being
  cut off. This applies to **every** line: the first line of the text, a line after a
  newline, a line after a wrap (the first character after the consumed space, §7.3), and
  a line after a forced break. If that first character is a space (*A* = 0), there is no
  indent.
- **Pen.** Within the line, a pen starts at the indent and moves right by *A* + *W* + *B*
  for every character on the line, including a newline that ends the line (with the `?`
  metrics, §5). For the first character with negative *A* this puts the pen at
  1 + *W* + *B* after it.
- **Trailing value *T*.** Each character gets a value *T*, computed in text order:
  *T* = *B* when the previous character's *T* is 0 or more (or there is no previous
  character); otherwise *T* = *B* + the smaller of 0 and (previous *T* + *W*). The
  "previous character" is the previous character of the text even across a line break
  (*T* is not reset when a line starts); before the first character it is 0.
- **Line width** = the pen position after the line's last character − that character's
  *T*. With the usual *B* ≥ 0 this is the right edge of the last glyph box (pen before it
  + *A* + *W*). The width includes the indent and the contribution of a newline that ends
  the line; a space consumed by a wrap (§7.3) belongs to neither line.

List-item markup (`<li>`) adds an extra amount to one character and draws a bullet; §7.5
gives the rules (closes the former Q15).

<a id="s7-3"></a>
### 7.3 Breaking lines

**Wrapping switch.** The element's Bool property `UICore_Text_one_line` switches wrapping
off (default false, that is wrapping on; the layouts hold 246 true and 23 false values).
With wrapping off, neither the available width nor a newline ends a line: **every
character of the text is on the first line**. Newlines are then still not drawn but still
count with their `?` advance and towards that line's height and baseline. The check for a
final newline (§7.1 case 2) runs after all characters are placed, in both modes, so:

- text that does **not** end in a newline has exactly **one** line;
- text that **ends** in a newline has exactly **two** lines: the first holds every
  character (including that final newline), the second is the extra line of §7.1 case 2
  (width 0, line height of the final newline's font, baseline 0, leading 0), and the text
  height counts both;
- empty text has the single all-zero line of §7.1 case 3.

The text height then follows §7.1 unchanged (example §13.6).

With wrapping on, the client goes through the characters in order and, for each
character *c*, applies these rules:

1. **Newline.** If the character before *c* is a newline, the line ends before *c*,
   whatever the width. The newline stays on the line it ends.
2. **Overflow test.** *c* **overflows** when the pen before *c* (in the line's own
   coordinates, indent included) + *A* + *W* of *c* is **greater than** the available
   width.
3. **Break opportunity.** A break is possible **before** *c*, where *p* is the character
   before it, when
   - *c* is a space (U+0020), tab (U+0009) or carriage return (U+000D); or
   - *p* is a hyphen-minus (U+002D); or
   - *c* or *p* is in one of the "break anywhere" ranges U+3040–U+312F, U+3190–U+319F,
     U+4E00–U+9FBF, U+F900–U+FAFF;

   and none of these forbids it:
   - *c* is one of `!` `)` `,` `.` `?` (U+0021, U+0029, U+002C, U+002E, U+003F), U+3001,
     U+3002, U+30FC, U+FF01, U+FF09, U+FF1F, U+FF70, U+FF9E or U+FF9F;
   - *p* is `(` (U+0028) or U+FF08.

   The first character of the text has no *p* and is never a break opportunity. The client
   remembers the **latest** opportunity on the current line; an opportunity before *c* is
   recorded before *c*'s overflow is acted on.
4. **Breaking on overflow.** If *c* overflows:
   - and the current line has a break opportunity (possibly the one before *c* itself),
     the line ends at the latest one;
   - and the line has none, the line ends **just before *c*** (a forced break inside the
     word). This closes the former Q5: an over-long word is split before the glyph that
     would pass the edge. Edge case: if *c* is the very first character of the text, the
     first line is then an empty line (§7.1 case 4) and *c* starts the second line.
5. **No re-test.** The characters that move to the new line (from the break up to and
   including *c*) are not tested again; testing resumes with the character after *c*.
   So a single glyph wider than the available width stands alone on its line and extends
   past the edge, and a moved word that is already too wide is split at the next
   character.

> **OPEN (implementation, M5 `tessera-ui::text`):** rule 4's edge case names only the first
> character of the text. Whether the same empty line arises when the first character of any
> later line (after a newline, or after a wrap that consumed a space) is wider than the
> available width and the line has no opportunity is not stated. Tessera applies rule 4
> literally (the line ends just before *c*, giving an empty line of §7.1 case 4) on every
> line until answered.
>
> Two related readings Tessera applies until answered (review of the M5 implementation):
> (a) an opportunity before the **first character of a line** (a space after a newline, or
> the second space of a run after a wrap consumed the first) is not recorded, as for the first
> character of the text: breaking there would move nothing, give an empty line and consume a
> space that "spaces at the start of the next line stay" and "the spaces after it start the
> next line" keep. (b) When the empty line of rule 4 follows a wrap that consumed a space,
> that line keeps §7.1 case 4 (height, baseline and leading 0) and the consumed space counts
> towards the height and baseline of the line holding *c*, the next line with characters.
> Which reading the client uses for (a) and (b) is not stated.

**What a break consumes.** When a line ends at an opportunity before a space, tab or
carriage return, exactly **that one character** is consumed: it belongs to neither line,
is not drawn, and its advance is not counted (the next line's pen starts without it; the
indent rule of §7.2 looks at the character after it). Nothing else is consumed:

- every space is an opportunity of its own, so in a run of spaces the line ends before the
  **last space reached** when the overflow happened. The earlier spaces of the run stay at
  the end of the line before (they count in its width up to the last one's *T*); if the
  overflow happened on a space, the spaces after it start the next line and are laid out
  as blank advances;
- after a newline nothing is consumed: spaces at the start of the next line stay;
- at a hyphen opportunity nothing is consumed: the hyphen ends the line;
- after a forced break nothing is consumed.

The consumed character, and every other character that moves to the next line, counts
towards the **next** line's height and baseline (§7.1). This matters only when fonts are
mixed.

<a id="s7-4"></a>
### 7.4 Alignment

The horizontal and vertical justification properties (`UICore_Text_horizontal_justification`,
`UICore_Text_vertical_justification`) use enum mapper 0x23000177: 1 Bottom, 2 Top,
3 Center, 4 Right, 5 Left (0 Undef). An element without the property uses **5 (Left)**
horizontally and **2 (Top)** vertically. The client reads the value of **both axes
through the same table**; the enum labels do not matter:

| Value | Offset on either axis |
|---|---|
| 1 (Bottom) or 4 (Right) | far side: *F* = available − used. Value 1 on the horizontal axis right-aligns; value 4 on the vertical axis bottom-aligns. |
| 3 (Center) | ⌊available / 2⌋ − ⌊used / 2⌋ (each halved separately, rounding down) |
| any other value (0, 2, 5, others) | 0 (near side: left or top) |

The layouts hold, at element and state level, horizontal values 0 (5), 2 (1), 3 (1,003),
4 (241), 5 (789) and vertical values 0 (7), 1 (40), 2 (108), 3 (1,269), 4 (12), 5 (2), so
"crossed" values do occur.

**Horizontal (per line).**

- used = the line's width (§7.2, indent included);
- content width = the widest line width + left margin + right margin;
- available = the larger of the element's width and the content width, minus the left and
  right margins;
- the line's first character is drawn at pen x = left margin + offset + indent.

**Vertical (the block of lines).**

- content height = text height (§7.1) + top margin + bottom margin;
- used = the larger of the content height and the line height of the normal slot's font
  (0 if that slot has no font);
- available = the larger of the element's height and the content height, minus the top and
  bottom margins;
- the first line's top = top margin + offset; each next line follows §7.1.

Note that the vertical "used" extent **includes** the top and bottom margins while
"available" excludes them. This is the client's rule and must be kept for fidelity: with
margins *t* and *b*, element height *H* and a content height that fits, bottom-aligned
text puts its first line top at *H* − text height − *t* − 2*b*, and centred text at
*t* + ⌊(*H* − *t* − *b*) / 2⌋ − ⌊(text height + *t* + *b*) / 2⌋ (example §13.7).

All values are 32-bit integers. The client halves "available" and "used" as unsigned
numbers, so a negative available extent (margins larger than the element) gives a
meaningless offset. **Tessera Decision:** treat a negative available extent as 0.

With `UICore_Text_truncate_text_to_fit`, the content extents are replaced after layout as
§7.6 describes (closes the former Q13).

<a id="s7-5"></a>
### 7.5 List items and the bullet (Confirmed (BIN))

The markup reader (`string-table.md` §4.7) reports `<p>` with a "break" flag and `<li>` with
the "break" flag plus a "list item" flag. A tag with the break flag becomes a newline
character in the character list (§7.3 rule 1 then ends the line), except where the element
option of `string-table.md` §4.7 drops it at the very start of the text or right after a
newline. Then:

1. **Which character.** The flags of a tag apply to the **next character emitted** after
   it, and only to that one. After an `<li>` that became a newline, that is the first
   character after the newline, so it starts a line. That character carries the list-item
   flag and an **extra amount** *X*:
   *X* = the width of the element's `UICore_Text_bullet_image` (a 0x41 image DID; 0 when
   the element has none) + the element's Int property `UICore_Text_bullet_spacing` (0 when
   absent).
2. **Layout.** For a character with the list-item flag, *X* is added to the overflow test
   of §7.3 rule 2 (pen + *X* + *A* + *W* compared with the available width) and to the
   pen advance of that character (the pen moves by *X* + *A* + *W* + *B*). The bullet does
   not enter the line metrics of §7.1 (line height and baseline come only from the
   characters' fonts).
3. **Drawing.** When the first character of a drawn line carries the list-item flag:
   - if the element has a bullet image, the image is drawn at its own size (blended like
     other UI art), its left edge at the line start (left margin + alignment offset +
     indent, §7.2), its top at line top + *L* − image height (*L* = line baseline, §7.1),
     so its bottom row is the bottom row of flat-bottomed capitals. This happens in each
     drawing pass, so with the outline flag on (§8.1) the bullet is drawn twice at the same
     place;
   - then the pen moves right by *X* before the glyph is drawn, with or without an image.
4. Closing `</li>` turns the list state off; it inserts nothing.

Data (OBS): 15 text elements carry a bullet image (two distinct images), two carry a
spacing (values 2 and 4).

<a id="s7-6"></a>
### 7.6 Truncation to fit (Confirmed (BIN); enabling condition Probable)

This section is the **only** statement of the truncation rule; `ui-behaviour.md` §10 refers
here and adds the UI-level effects (the truncated-text tooltip, UB §10.2, and the element's
mouse-visible flag while cut, UB §4.3).

`UICore_Text_truncate_text_to_fit` is a **StringInfo** property (`string-table.md`): the
**suffix** shown where the text is cut. Census (OBS, re-counted 2026-10-06): 325 values in the
layouts, 324 on elements and 1 on a state; 319 name one of two entries of one string table,
1 names an entry of a second table (320 values, three distinct entries), and 5 are null
(table and token 0). An element truncates when it has this property with a resolvable text;
the text widget clears its truncation switch whenever `UICore_Text_editable` or
`UICore_Text_selectable` becomes true (BIN, revision 5). Whether anything switches it on
again while the element stays editable or selectable is part of the OPEN below; Tessera never
truncates editable or selectable text.

> **OPEN (Q13, residue):** where and under which exact condition the client sets the
> truncation switch (the setting routine's callers were not traced). Use "property present,
> text resolves to at least one character, element neither editable nor selectable" until
> answered.

1. **Suffix.** When the element's text is (re)built, the suffix text is built as a
   character list of its own with the element's font slots and styles (§9.1), and laid out
   alone with wrapping off and an unlimited width (with the element's leading factor,
   §7.1). This gives the **suffix width** *S* (its line width, §7.2) and height.
2. **Lay out** the element's text normally (§7.1–§7.4), giving the content width and
   height, both including the margins.
3. **Does it fit?** With wrapping off (`UICore_Text_one_line`) compare the content width
   with the element's width; with wrapping on compare the content height with the
   element's height. If the content is not greater, nothing is truncated (stop here).
4. If `UICore_Text_auto_tooltip_truncated_text` is true, the element is marked
   "truncated" (a state flag that the tooltip logic reads; the tooltip is
   `ui-behaviour.md` §10.2).
5. **The line to cut.** With wrapping off it is line 0. With wrapping on: let
   *H*ₐ = element height − top margin − bottom margin − 1; add up (line height + leading)
   from line 0 onwards; the line to cut is the **last line whose running sum is still
   ≤ *H*ₐ**; if line 0 alone exceeds *H*ₐ, it is line 0.
6. **The cut index.** Budget *R* = element width − left margin − right margin − *S* −
   the cut line's indent (§7.2). Walk the characters of the cut line from its first one,
   using each character's stored advance (the clamped value of §6.1):
   - while *R* is **greater than** the character's advance and the character is **not the
     last character of the line**, subtract the advance from *R* and move past it;
   - otherwise subtract its advance from *R* and stop.
   The cut index is the index of the first character not moved past. Exception: if the
   cut line is the last line of the text, *R* is still greater than 0 after the stop, and
   the character where the walk stopped is not a newline, the cut index is the end of the
   text (nothing is cut).
7. **Extents after the cut.** Width of the cut line = its indent + the sum of the stored
   advances of its characters before the cut index + *S*. With wrapping off the content
   width becomes that + left and right margins. With wrapping on the content width becomes
   the widest of lines 0 … cut line (with the cut line's new width) + margins, and the
   content height the sum of (line height + leading) of lines 0 … cut line + top and
   bottom margins. Alignment (§7.4) uses the cut line's new width for that line.
8. **Drawing.** Characters are drawn in order up to the cut index; there the drawing
   switches to the suffix's characters, which continue on the **same line** from the
   current pen position (no line advance happens inside the suffix), each in its own font
   and colour. Nothing after the cut index is drawn, including later lines.

When the cut index is the end of the text (the exception of step 6), the drawing never
reaches it, so no suffix is drawn, although the cut line's width of step 7 still includes
*S*. With wrapping on, the cut line can be the last line of the text only when line 0
alone exceeds *H*ₐ and the text has a single line: the sum of step 5 includes every
leading, so if every line fitted *H*ₐ the content height would be below the element height
and step 3 would not cut. (Example: §13.10.)

---

<a id="s8"></a>
## 8. Drawing text (Confirmed (BIN) unless marked)

<a id="s8-1"></a>
### 8.1 Passes

1. If the element's outline flag is set, draw the **outline** of every glyph of every line
   (§8.2 or §8.3).
2. Then draw the **fill** of every glyph of every line: the glyph box of §6.2 from the
   foreground atlas, coloured with the glyph's text colour. In this pass, an underlined
   character first gets its underline (§8.5), then its glyph.

Both passes walk the lines and characters in text order and repeat the line positions of
§7 (alignment, indent, list-item bullet of §7.5). Because all outlines come first, an
outline never covers a neighbouring glyph's fill.

The outline flag is the Bool property `UICore_Text_outline` (705 true and 26 false values
in the layouts). When no element, template or state in the element's chain supplies it,
the flag is **off** (a new text element starts with it cleared; Confirmed (BIN)). The
outline colour is the Colour property `UICore_Text_font_outline_color` of the element (or
of its active state); without it the colour is **opaque black**.

<a id="s8-2"></a>
### 8.2 Outline from the background atlas

When the font has a background atlas, each glyph's outline is the **bordered box**: left
edge *x* + *A* − *bx*, top edge as in §6.2 minus *by*, size (*W* + 2·*bx*) × (*H* + 2·*by*),
sampled from the background atlas at (*X* − *bx*, *Y* − *by*), coloured with the outline
colour.

(The client extends the destination downwards by *bx* rather than *by*; since *bx* = *by*
in every record this has no effect. Readers should use *by*.)

<a id="s8-3"></a>
### 8.3 Stamped outline (no background atlas)

When the background DID is 0, the outline is made from the **foreground** glyph: it is drawn
eight times in the outline colour, offset by one pixel in each direction (dx, dy ∈ {−1, 0, 1},
except (0, 0)), in the order dy = −1, 0, 1 and, within each, dx = −1, 0, 1. Then the fill
pass draws the glyph normally on top.

<a id="s8-4"></a>
### 8.4 Colour and compositing (Confirmed (BIN))

**Where.** A text element paints into the canvas that the UI paint pass hands it, after the
element's own background art (the base element paint runs first), once per dirty
rectangle, clipped to that rectangle and to the element. The glyphs are composited **on
the CPU** into that canvas: the colour is applied while each glyph is composited, not
later. How the canvas reaches the screen (and at which UI scale, §6.3) is outside the text
code.

> **OPEN (Q6, narrowed):** the format of the UI paint canvas (the rules below are those for
> format 21) and how the finished canvas is blended onto the screen. Suggested method: M4
> on the UI paint pass that creates the canvas, or M3 (frame capture). **Tessera rule (UR
> Decision 1.1, 1.6):** every canvas is 32-bit 0xAARRGGBB with straight alpha (the A8R8G8B8
> layout of format 21, so the rules below apply as written), starting as transparent black;
> text and art are composited into the same canvas, and the GPU composes the finished canvas
> with the surface's opacity. The final blend is exact only for alpha 0 and 255 at scale 1 until the
> original's blend is known.

**Colours.** A Colour property holds four floats, red, green, blue, alpha (0.0–1.0). The
client packs it into a 32-bit ARGB value by multiplying each component by 255 and
truncating towards zero (so 0.999 gives 254). Text colours from markup are already ARGB
(`string-table.md` §4.7). The default outline colour (0, 0, 0, 1) packs to 0xFF000000.

**Compositing one pass of one glyph.** Source: the atlas coverage byte *c* (0–255) of each
pixel of the box (foreground atlas for the fill and for the stamped outline, background
atlas for the outline of §8.2). Colour: the pass colour, ARGB, with colour bytes
*C*ᵣ, *C*ᵍ, *C*ᵇ and alpha *α*. Destination: the canvas pixel, here a 32-bit canvas with
alpha (surface format 21, A8R8G8B8, bytes *D*ₐ, *D*ᵣ, *D*ᵍ, *D*ᵇ); the client has separate
routines for canvases of format 20, 22 and 32, which were not read. For every pixel of the
clipped box:

1. **Effective coverage** *s*: if *α* = 255, *s* = *c*; if *α* = 0, *s* = 0; otherwise
   *s* = ⌊*c* · (*α* + 1) / 256⌋.
2. If *s* = 0, the pixel is left unchanged.
3. Let *a* = *s* + 1.
4. If *a* = 256 (full coverage), or *D*ₐ = 0 (transparent destination), the pixel becomes
   alpha **= *c*** (the raw coverage, not *s*) and colour (*C*ᵣ, *C*ᵍ, *C*ᵇ).
5. If *D*ₐ = 255 (opaque destination): each colour channel becomes
   *D* − trunc((*D* − *C*) · *a* / 256), and the alpha stays 255.
6. Otherwise (0 < *D*ₐ < 255): let *t* = *D*ₐ + 1 and
   *k* = *t* + *a* − ⌊*t* · *a* / 256⌋. Each colour channel becomes
   *D* − trunc((*D* − *C*) · *a* / *k*), and the alpha becomes *k* − 1.

"trunc" rounds towards zero (the products can be negative); all arithmetic is in 32-bit
signed integers. Channel results are bytes (they stay within 0–255 by construction).

Consequences: an opaque text colour (*α* = 255, the usual case) uses the coverage directly;
a translucent text colour reduces the coverage only where the canvas already has alpha;
where the canvas is transparent the written alpha is the raw coverage even for a
translucent colour. Examples: §13.8.

**Tessera Decision (UR Decision 1, §3):** reproduce these rules exactly, on the CPU, for all
text: the outline pass, then the fill pass with underlines, composited into the surface's
canvas inside each damaged rectangle. There is no GPU text path (the earlier allowance of a
GPU path that matches only the opaque and transparent destination cases is withdrawn, because
UR chose exact CPU compositing). Tessera's own fallback fonts (§11) go through the same path.

**Not covered here:** where the text colours come from (the colour properties have the
same four-way structure as the font properties of §9.1: `UICore_Text_font_colors`,
`UICore_Text_font_color_struct`, `UICore_Text_font_color`,
`UICore_Text_set_normal_font_color_by_property`, and the `tag_` equivalents), the
selection highlight and the text cursor of editable text. **Tessera rule (UR §3):** the caret
and the selection highlight are drawn as rectangle fills into the same canvas; the caret's
blink repaints only the caret's rectangle.

<a id="s8-5"></a>
### 8.5 Underline (Confirmed (BIN); colour source partly OPEN)

Characters inside `<u>` … `</u>` (`string-table.md` §4.7) carry an underline flag. In the
fill pass (§8.1), before such a character's glyph, the client fills a rectangle:

- **Thickness** *t* = ⌊(line height + leading) / 20⌋ + 1, and at least 1, using the line
  height and leading of the character's line (§7.1).
- **Rows**: from (line top + *L*) + *t* to (line top + *L*) + 2*t* − 1, inclusive, where
  *L* is the line baseline (§7.1); that is, one thickness below the baseline row.
- **Columns**: from the pen position before the character to pen + the character's stored
  advance (§6.1), **inclusive**; when the glyph's right bearing *B* is negative, the right
  end moves right by −*B*. Because the right end is inclusive, the underlines of
  neighbouring characters overlap by one column and form a continuous line.
- **Pixels**: the rectangle (clipped to the paint rectangle and the element) is
  **overwritten** with the underline colour (no blending; the colour's alpha byte is
  written as it is).
- **Colour**: a second per-character colour. Inside a link that supplies its own colour
  (§9.1) both the text colour and the underline colour are that colour.

> **OPEN (Q16):** for text outside such links, which property supplies the underline colour
> (it is a second colour field of the element's text style, not traced to a property).
> Until answered, use the character's text colour.

Example: §13.9.

<a id="s8-6"></a>
### 8.6 Bold, italic, shadow and system fonts (Confirmed (BIN))

- `<b>` and `<i>` set character flags, but no routine of the text element (character-list
  builder, line layout, paint, glyph blit) tests them: **bold and italic markup has no
  visible effect**. Bold text in the UI comes only from choosing a bold font record
  (`TrajanProBold*`, `BookAntiquaBold*`, `VerdanaBold16`).
- There is **no shadow style** for text: the drawing has only the outline and fill passes
  (§8.1) and the underline; the property dictionary has no text-shadow property (OBS).
- The client imports Windows font-creation and text-drawing functions. They are used by a
  separate drawing component that works on Windows device contexts; none of the text
  element's routines (character-list builder, layout, paint, glyph blit) reaches them.
  Probable (BIN): no game UI text is drawn with a system font; together with §5 this means
  the original never shows a glyph from outside the font records. The embedded web browser
  (the client starts a browser helper and keeps a browser cache in the settings folder,
  OBS) renders web pages with its own font engine; that is outside this spec.

> **OPEN (Q17):** whether any text drawer other than the UI text element (for example the
> 3-D floating names and damage numbers) uses another code path or system fonts. Suggested
> method: M4 on the floating-text renderer.

---

<a id="s9"></a>
## 9. How a font is chosen

<a id="s9-1"></a>
### 9.1 UI layout text elements (Confirmed (BIN) for the mechanism; counts OBS)

A text element has two **font slots**:

- **normal**: for ordinary text, including text inside colour, emphasis, paragraph and
  list markup (`string-table.md` §4.7): those tags change the colour or set layout flags
  but never switch the font slot;
- **tag**: for the label text of **link markup** only (`string-table.md` §4.8: a tag whose
  kind the client's link registry recognises, up to its closing tag). Those characters
  take the tag slot's font **and** the tag colour (`UICore_Text_tag_font_color` and
  relatives); if the link itself supplies a colour, the link's colour wins over the tag
  colour. Confirmed (BIN): the character-list builder switches to the tag style exactly
  while a recognised link is open, and only there.

Order in which the client reads a tag (Confirmed (BIN)): a tag while a link is open closes
the link; otherwise the tag is first offered to the link registry (a recognised kind opens
a link); only if the registry does not recognise it, and the element's Bool property
`UICore_Text_parse_markup_tags` is true, is it read as colour, emphasis, paragraph or list
markup; anything else is shown literally in the normal slot. So link recognition does not
depend on `UICore_Text_parse_markup_tags` (see §14 for the note to `string-table.md`).

**Initial font.** When a text element is created, **both** slots are loaded with the
**locale default font**: the font DID held by the client's locale object, which by the
field order of `string-table.md` §10.3 is the font DID of the locale record (0x58000000;
0x42000028 `LucidaConsole12` in the English record). Properties then replace it. So an
element that gets no font from any source in its inheritance chain draws its text in the
locale default font (this closes Q12). The record-to-object field match is Inferred, as
in `string-table.md` §10.3 (OPEN Q2 covers both fields; the record holds exactly one font
DID, and the client's locale-object field is loaded as a font, so no other candidate
exists in the record).

Each slot can be filled from four kinds of property. All font-valued properties declare
DB-object code 0x4C.

| Slot | Array of fonts | Struct holding one font | Single font | By property |
|---|---|---|---|---|
| normal | `UICore_Text_fonts` (array of `UICore_Text_font`) | `UICore_Text_font_struct` (field `UICore_Text_font`) | `UICore_Text_font` | `UICore_Text_set_normal_font_by_property` |
| tag | `UICore_Text_tag_fonts` (array of `UICore_Text_font`) | `UICore_Text_tag_font_struct` (field `UICore_Text_tag_font`) | `UICore_Text_tag_font` | `UICore_Text_set_tag_font_by_property` |

Rules:

1. From an **array**, the slot takes the entry at the element's current **font index**
   (§9.2; 0 unless a panel changes it). An index at or past the end of the array changes
   nothing.
2. If the tag slot has no property of its own, it uses the normal slot's property.
3. The single-font property is what the slot finally holds: choosing from an array or a
   struct writes the chosen DID into `UICore_Text_font` (normal) or `UICore_Text_tag_font`
   (tag), and the element loads that font.
4. Setting a slot to the DID **0**, or to a DID that does not resolve to a font record,
   **leaves the slot's current font in place** (initially the locale default font); for
   an unresolvable DID the client only writes a diagnostic. (The client's "null DID"
   compared here is a zero-initialised value that the code never writes, that is 0.) A
   slot can be without any font only if the locale default font itself cannot be loaded.
   Text in a slot without a font is laid out with the 1 × 1 blank glyph of §5 step 3
   (advance 1 pixel per character, line height 0) and is not drawn.
5. "By property" (Confirmed (BIN), closes the former Q7): the value is a PropertyId that
   names a **member of the element's own font-struct property**: `UICore_Text_font_struct`
   for the normal slot, `UICore_Text_tag_font_struct` for the tag slot (the tag slot falls
   back to the normal struct when the element has no tag struct). The member is read
   instead of the default member `UICore_Text_font`; it must hold a DID, which is then set
   as in rule 3 and rule 4. If the struct or the member is missing, or the member is not a
   DID, the slot is unchanged. `UICore_Text_set_normal_font_by_property` and
   `UICore_Text_set_tag_font_by_property` act on one slot each;
   `UICore_Text_set_font_by_property` acts on both. The client remembers the PropertyId
   and uses it again when it re-applies fonts. In the data the only value is one
   `UICore_Text_set_tag_font_by_property` = 0, which names no member and changes nothing.

> **OPEN (implementation, M5 `tessera-ui::text`):** the order in which the array, struct and
> by-property sources of one slot are applied when an element (after inheritance) has more
> than one of them, and whether rule 2 (the tag slot uses the normal slot's properties) also
> covers `UICore_Text_set_normal_font_by_property`. Rule 3 implies that a chosen array or
> struct entry replaces the single font. Tessera applies, per slot: the single font, then
> the struct (through the by-property member when one is set), then the array entry; and
> the tag slot without properties of its own takes all of the normal slot's sources.
>
> Also open: whether a source that changes nothing counts as "a property of its own" for
> rule 2. Tessera does not count a by-property value of 0 (rule 5: it "names no member and
> changes nothing", so it also does not stop the default struct member from being read), but
> does count a single font of DID 0 (`UICore_Text_tag_font` = 0, four values in the data) and a
> non-zero by-property PropertyId whose member is missing or not a DID: with only those, the
> tag slot keeps the locale default font instead of taking the normal slot's.

Element states may carry the same properties (52 state-level `UICore_Text_fonts`); how a
state's properties override the element's belongs to `ui-layout.md`.

**Where the values come from (OBS):**

- 835 `UICore_Text_fonts` arrays (783 on elements, 52 on states) with 0 (16), 1 (800),
  2 (3), 5 (1), 6 (5) or 7 (10) entries; 293 `UICore_Text_tag_fonts` arrays (0, 1 or 6
  entries); 6 `UICore_Text_font_struct` (in two layouts, all naming 0x42000029);
  6 `UICore_Text_tag_font` (4 of them 0).
- The layouts name 37 distinct font DIDs, all present. Eleven fonts are named by no layout:
  0x42000027 and 0x42000028 (§9.5; 0x42000028 is also the English locale default font
  above), 0x420000F7 and eight `BookAntiqua*` sizes (0x420000F0, F2, F3, F5, F6, F8, F9,
  FA); apart from those two uses they are reachable only through the plugin API (§9.4).
- **Text-style library.** The UI layout labelled `text` in the UILAYOUT DID map
  (0x22000002 in this install) holds 77 template elements: named text styles (for example
  `buttonfont`, `titlebarfont`, `fieldlabelsmall`, `floatynamefont`, `radarfont`), each
  with a font array, usually one entry, and usually the outline flag. 2,038 of the 16,421
  elements of all layouts name this layout and one of these templates as their base and so
  inherit its font (inheritance rules: `ui-layout.md`). Which elements use a given
  template is read from the base references, never from the template labels.
- **Fonts of the native panels (observation, revision 4).** Counting the first entry of
  every element-level `UICore_Text_fonts` array (781 non-empty arrays) and, separately,
  the font that the 2,018 based elements whose template has a font array inherit from the
  text-style library:

  | Font (DID in this install) | Line height | First entry of an own array | Inherited from a template |
  |---|---|---|---|
  | `Verdana14` (0x42000001) | 14 | 272 | 177 |
  | `TrajanPro14` (0x4200000C) | 14 | 107 | 696 |
  | `TrajanPro16` (0x4200000B) | 16 | 50 | 403 |
  | `TrajanPro18` (0x42000009) | 18 | 63 | 369 |
  | `Verdana12` (0x42000008) | 12 | 65 | 76 |
  | `TrajanPro20` (0x42000000) | 20 | 31 | 63 |

  These six cover 75 % of the own arrays (588 of 781) and 88 % of the inherited fonts
  (1,784 of 2,018); the text-style library's most common template font is `TrajanPro18`
  (16 of its 75 templates with a font array). So native panels mostly draw `TrajanPro` at
  14–20 pixels and `Verdana` at 12–14, with chat in `Verdana` 14–23 (§9.3). These are
  observations of one install for checklists, not values to build in: a panel's font is
  always read from its layout.

<a id="s9-2"></a>
### 9.2 Font ladders and the quest text size (Confirmed (BIN))

Some elements carry several fonts of one family in one array. Observed ladders (heights
from §3; DIDs of this install):

| Elements | Entries |
|---|---|
| Quest text in the quest journal, quest dispenser and objective panels (layouts 0x22000025, 0x22000043, 0x22000067, 0x22000842; 10 elements) | 7: heights 20, 22, 24, 26, 28, 32, 36 (`BookAntiqua20/22/24/26/28/32/36`) |
| Quest dispenser twist name | 6: heights 22–36 (the same ladder without the first step) |
| Chat window text, input line and log template (layout 0x22000034) | 6: `Verdana14/16/18/20/22/23`, in the order of the chat size steps (§9.3); the same list as tag fonts |
| Examination name | 5: heights 20, 18, 15, 14, 13 (`TrajanPro*`, descending) |
| Quest title and category title | 2: heights 15, 14 (descending) |

The quest dispenser and the quest journal panels have **increase** and **decrease** font
size buttons (elements `Quest_IncreaseFontSize`, `Quest_DecreaseFontSize`,
`Quest_Bestow_IncreaseFontSize`, `Quest_Bestow_DecreaseFontSize`). For the dispenser:

1. The client keeps one font index for the session. It starts at **0** and is not saved.
2. Increase asks for index + 1, decrease for index − 1.
3. The panel offers the requested index to each of its book-text elements (the font
   example, intro, mission, twist name, twist description and objective texts): an element
   whose array is long enough takes that entry for **both** slots and lays its text out
   again; the others keep their font.
4. If at least one element took the index, it becomes the session index; otherwise nothing
   changes. So the index stays within 0 … (longest array − 1), here 0–6, and a negative
   request is refused.
5. When the panel is filled for a new quest it applies the session index again.

The journal panel uses the same per-element rule for its own text elements.

> **OPEN (Q8):** whether the journal panel shares the dispenser's session index, and how the
> descending ladders (examination name, quest titles) choose their entry. They look like
> shrink-to-fit fallbacks; no code path that picks them was found.

<a id="s9-3"></a>
### 9.3 Chat (Confirmed (BIN))

Two tables decide a chat line's font and colour. Both are read at run time.

**Size step → font.** The UNIQUEDB DID map (root label `UNIQUEDB`) entry with label
`UI_ChatFonts` (key 0x10000011) names a property set (0x7800000B, in the local archive)
holding `UI_ChatFont_Array`: a list of `UI_ChatFont_Struct`, each with
`UI_ChatFont_FontEnum` (enum mapper 0x23000026: 0 Small, 1 Medium, 2 Large, 3 XLarge,
4 XXLarge, 5 XXXLarge) and `UI_ChatFont_FontDataID` (font DID). The client builds the map
size step → font DID from it and skips entries whose DID is 0 (or whose DID field is not a DID). This install:

| Step | Font |
|---|---|
| 0 Small | 0x42000001 (`Verdana14`) |
| 1 Medium | 0x42000018 (`Verdana16`) |
| 2 Large | 0x42000019 (`Verdana18`) |
| 3 XLarge | 0x42000016 (`Verdana20`) |
| 4 XXLarge | 0x4200001A (`Verdana22`) |
| 5 XXXLarge | 0x4200001B (`Verdana23`) |

**Chat type → size step and colour.** The gameplay options hold
`GOP_ChatTextTypeFontColorArray`: a list of `GOP_ChatTextTypeFontColorStruct`, each with
`GOP_ChatTextType` (a 64-bit bitfield over the chat-type enum 0x230002A8; one bit set; bit
*i* = chat type *i* + 1, `property-system.md` V7), `GOP_ChatTextTypeFont` (size step) and
`GOP_ChatTextTypeColor`. The client builds the map chat type → (size step, colour) from
it. The defaults are the property set labelled `GameplayOptionDefaults` (UNIQUEDB key 0x68,
also labelled `Preferences_Gameplay_Option_Defaults` under key 0x10000012, and DBPC key 8;
0x78000006 in gamelogic): 43 entries, 41 at Medium and 2 at XLarge (chat types 0x18
`Global` and 0x21 `EventBroadcast`). A player's own choices replace the defaults.

**Local file form (revision 4).** The client can write the chat-type table to, and read it
from, an XML options file (BIN: one writer and one reader routine). Among the element
names the writer uses are `Aliases`, `Layouts`, `Options` and `ChatColors`; `ChatColors`
holds one `ChatFont` element per entry of `GOP_ChatTextTypeFontColorArray`,
with the attributes `TextType`, `Color` and `Size` (the chat type, the colour and the size
step of §9.3). These names are interoperability identifiers. The owner's settings folder
(OBS, 2026-10-06) holds `UserPreferences.ini`, `lotro.keymap`, `PluginData` and a browser
cache only, and `UserPreferences.ini` has no chat-font key, so the live table is not kept
in a local file in this install.

> **OPEN (Q9, narrowed):** where the live table is kept (for example with the character
> on the server, which this project does not study), the folder and file name of the XML
> options file, and the text encoding of its `TextType`, `Color` and `Size` values.
> Suggested method: M3 (save a UI layout or options profile in the original client and
> read the written file), M4 on the writer routine for the value encoding.

**Each chat line in its own font (Confirmed (BIN)).** Every routine found that adds a line
to a chat log (three) does the following, in this order:

1. Look up the line's chat type in the chat-type map. The chat type is given as a 64-bit
   mask; the client checks its set bits from bit 0 upwards and takes the (size step,
   colour) of the **last** set bit that has an entry. If no bit has an entry, the line
   gets size step **0** (Small) and colour **opaque white**.
2. Look up the size step in the size-step map. If the step has no entry, the font DID is
   0, which leaves the element's current font in place (§9.1 rule 4).
3. Set the chat text element's `UICore_Text_font_color` to the colour and **both** of its
   font slots (normal and tag) to the font.
4. Append the line. An appended line keeps the font, colour and tag style that the
   element had at that moment; changing them for the next line does not change lines
   already in the log.

One of the three routines first appends the same line, with the colour set but **without**
a font change, to a second text element that it finds through a property of the chat
window.

> **OPEN (Q14):** what that second element is, and therefore in which font its copy of the
> line appears.

So each chat line is laid out and drawn in the font of its own chat type's size step,
including the label text of links inside it (both slots hold the same font), and lines of
different types in one log can have different heights. The chat window's text elements
also carry the same six fonts in the same order (§9.2); that array only gives the
element's initial font, which every append replaces.

Nine game objects (0x70035169, 0x70035199–0x7003519B, 0x70040072, 0x70047DC0–0x70047DC3)
each carry one `GOP_ChatTextTypeFontColorStruct` (Medium, one chat type, one colour) and no
other property (OBS: all nine read in full, 101 bytes each); the client code does not name
any of their DIDs (BIN, immediate search). They play no part in the chat-font rules above.

> **OPEN (Q10, narrowed):** which data record refers to them and why. Nothing in the font
> choice depends on them.

<a id="s9-4"></a>
### 9.4 Plugin labels

The plugin API's `Turbine.UI.Lotro.Font` values are font DIDs (`lua-plugin-api.md` §14.1,
Appendix A.2); the client wires the 48 names to the 48 DIDs of §3.1 (BIN). A plugin label
uses the font given to it; outline drawing is §8. (The default font of a new plugin label is
an OPEN of `lua-plugin-api.md` §13.4.)

<a id="s9-5"></a>
### 9.5 The FONT DID map and the diagnostic overlay (Confirmed (BIN))

The root DID map's `FONT` entry (root key 0x10; 0x2800000B here) lists four named fonts:

| Key | Label | Font (this install) |
|---|---|---|
| 1 | `Lucida_Console_12` | 0x42000028 |
| 2 | `FixedSys_15` | 0x42000027 |
| 3 | `Default_Korean` | 0x42000029 |
| 4 | `Default_Japanese` | 0x42000029 |

Only key 2 is used by this client build: its render-diagnostic overlays (visualisation
modes with on-screen statistics) draw the codes U+0020–U+007F from `FixedSys_15`,
which the client copies into a 256 × 256 texture. No other use of this map was found. The
two "default" entries point to 0x42000029, which has no Hangul, Kana or CJK glyphs (§10).

**Tessera Decision:** Tessera's own overlays may keep their built-in font; using
`FixedSys_15` through this map is optional.

<a id="s9-6"></a>
### 9.6 Defined but unused

These properties exist in the dictionary but have no value anywhere in the data
(Confirmed), and the client code does not name the first two groups (BIN):

- `UI_NPC_LogFont` (font DID);
- `GOP_TextTypeFont`, `GOP_TextTypeFontColorStruct`, `GOP_TextTypeFontColorArray`,
  `GOP_TextType`, `GOP_TextTypeColor` (an older text-type table);
- `PreprocOptions_FontGen_*` (font name, heights, antialiasing mode from enum 0x2300018E,
  background border pixels, "want background image", character ranges, generated file
  name): the settings of the tool that produced the font records. They explain the
  trailer's border fields (§3.2) and the optional background atlas but play no part at run
  time.

`UI_Options_Menu_FontIndex` (enum 0x23000026, the size steps of §9.3) has no value in any
property set or layout. The client reads it as a property of a **UI element** in about ten
routines; the one read in detail runs when a menu item is chosen and reads the item's value
(BIN). So the size step is attached to menu items at run time, not stored in the data.

> **OPEN (Q11, narrowed):** which chat types a chosen size step is then applied to (one
> type, a group, or all), and which panel builds those menus. Suggested method: M4 on the
> menu-selection routine, M3 in the original client.

---

<a id="s10"></a>
## 10. Census (Confirmed)

Records with identical code lists form six families:

| Family (plugin enum labels) | Records | Glyphs | Line heights | Border | Coverage |
|---|---|---|---|---|---|
| `TrajanPro*`, `TrajanProBold*` | 19: 0x42000000, 02, 07, 09, 0A, 0B, 0C, 0D, 11, 12, 13, 14, 15, 24, 25, 26, 2A, 35, 0x420000F7 | 497 | 13–36 | 4 (0x4200002A: 0) | Basic Latin, Latin-1, most of Latin Extended-A, a few Latin Extended-B, spacing modifiers, 4 Greek, punctuation, euro sign, letterlike and maths symbols, 2 ligatures, and 164 private-use codes (U+E0xx, U+F6xx, U+F7xx) |
| `BookAntiqua*`, `BookAntiquaBold*` | 17: 0x4200000E, 0F, 10, 0x420000F0–F6, F8, F9, FA, 0x42000168–16B | 671 | 12–36 | 4 (0x42000168–16B: 1) | Latin-1, Latin Extended-A, part of Extended-B, Greek (73), Cyrillic (94), box drawing and geometric shapes, symbols |
| `Verdana10…23` | 8: 0x42000001, 08, 16, 17, 18, 19, 1A, 1B | 680 | 10–23 | 3 | Latin-1, Latin Extended-A, part of Extended-B, Greek (73), Cyrillic (94), Latin Extended Additional (96) |
| `LucidaConsole12`, `VerdanaBold16` | 2: 0x42000028, 0x420000FD | 191 | 12, 16 | 2 | U+0020–U+007E and U+00A0–U+00FF only |
| `FixedSys15` | 1: 0x42000027 | 224 | 15 | 2 | Exactly the printable part of Windows code page 1252: U+0020–U+007F, U+00A0–U+00FF, the 27 non-Latin-1 characters of that code page, and the five unassigned bytes 0x81, 0x8D, 0x8F, 0x90, 0x9D as code points; monospace 8 × 15 cells, advance 9 |
| `Arial12` | 1: 0x42000029 | 1,419 | 12 | 4 | Latin, Greek (73), Cyrillic (118), Hebrew (52), Arabic (208) with presentation forms (256), symbols, up to U+FFFC |

No font contains U+FFFD, a tab glyph, or any Hangul, Kana or CJK ideograph. The line height
of every record equals the number at the end of its plugin enum label.

---

<a id="s11"></a>
## 11. Fallback fonts: when Tessera needs its own

1. **Game text needs no fallback for fidelity.** The original substitutes `?` from the same
   font (§5) and never switches fonts. A faithful renderer does the same.
2. **Tessera's own text needs its own font.** Text that is not drawn by a game layout:
   start-up and install-detection screens, error dialogs, Tessera's settings, the plugin
   manager window when it has no game layout, and debug overlays (`walker.md` already uses a
   built-in 8 × 8 font for its overlay). These must work before or without a game install.
3. **Optional extended coverage (Tessera Decision, default off):** a user may want plugin
   or chat text in scripts the game fonts lack (CJK, for example) shown as real glyphs
   instead of `?`. If offered, it must be an opt-in that leaves the default rendering
   identical to the original.
4. **Scaling:** the data fonts exist only at fixed pixel sizes, and the original never
   swaps fonts for a UI scale: it scales the painted 1 : 1 text (§6.3). **Tessera Decision
   (UR §3):** Tessera's own fonts (item 2), when they draw into a game-UI surface, are
   rasterised on the CPU by a pure-Rust rasteriser at 1 : 1 into the same one-byte coverage
   form as the game atlases, and then take exactly the compositing path of §8.4, so plugin
   text in a Tessera font blends like game text and is scaled with its surface. Rasterising
   them at the scaled size, which an earlier version of this item allowed, is not done:
   there is one text path.
5. **Licence rule:** any font Tessera ships must be original or under a licence compatible
   with the project (for example the SIL Open Font License). Game fonts and atlases are read
   from the user's install at run time and are never shipped or extracted
   (`docs/LEGAL.md` §11).

---

<a id="s12"></a>
## 12. Invariants

Confirmed over all 48 records and 95 atlases of the install unless stated. Tests should
check them on real data and build synthetic records that satisfy them.

| # | Invariant |
|---|---|
| FN1 | Payload length = 36 + 11·*n*, where *n* = header offset 12. |
| FN2 | Header offset 0 equals the record's DID. |
| FN3 | Header offset 4 = max *H*; header offset 8 = max *W* over all entries. |
| FN4 | Codes strictly ascending; first code U+0020; U+003F present. |
| FN5 | For every entry: *H* ≥ 1, *T* ≥ 0, *T* + *H* ≤ line height, 0 ≤ *A* + *W* + *B* ≤ 255. |
| FN6 | Baseline < line height. |
| FN7 | *bx* = *by*. A background DID of 0 occurs only with *bx* = *by* = 0. |
| FN8 | The foreground DID resolves to a 0x41 surface of format 28 and category 13; a non-zero background DID resolves to one with the same width and height. |
| FN9 | Every bordered box (*X* − *bx*, *Y* − *by*, *W* + 2*bx*, *H* + 2*by*) lies inside the atlas, and no two overlap. |
| FN10 | Packing (§4.2): *X* of a row's first glyph = *bx*; the first row's *Y* = *by*; next *X* = *X* + *W* + 2*bx* within a row; row step = largest *H* + 2*by*; atlas height = last row's *Y* + its largest *H* + *by*. |
| FN11 | Foreground ink lies inside the bordered boxes (all 48); inside the plain boxes in 47 (not 0x42000027). |
| FN12 | Every non-zero font DID named by a UI layout (37 distinct), by `UI_ChatFonts` (6) or by the `FONT` DID map (3) exists as a 0x42 record; 0 means "leave the slot's font unchanged" (§9.1 rule 4). |
| FN13 | The `UI_ChatFonts` array has exactly one entry per size step 0–5. |
| FN14 | Every `GOP_ChatTextType` value in `GameplayOptionDefaults` has exactly one bit set. |

Layout invariants (§7), for synthetic tests of a layout implementation:

| # | Invariant |
|---|---|
| FN15 | For every line, width = pen after its last character − that character's *T* (§7.2); when every *B* ≥ 0 this is the right edge of the last glyph box. |
| FN16 | Text height = Σ (line height + leading) over all lines − the last line's leading (flag `UICore_Text_last_line_leading` off). |
| FN17 | The characters of all lines, in order, plus the characters consumed by wraps, give back the whole text; each wrap consumes at most one character, and only a space, tab or carriage return. |
| FN18 | Text with *k* newlines and wrapping off has exactly one line if it does not end in a newline, and exactly two lines if it does (all characters on the first, the second being the §7.1 case-2 line); with wrapping on and no line wider than the available width it has *k* + 1 lines, and if the text ends in a newline its last line has width 0, baseline 0, leading 0 and the height of the final newline's font. |
| FN19 | Empty text has one line with all metrics 0 and text height 0. |

Drawing and truncation invariants (§6.3, §7.5–§8.6), for synthetic tests:

| # | Invariant |
|---|---|
| FN20 | Compositing (§8.4): with an opaque colour, coverage 0 leaves the pixel unchanged, coverage 255 writes the colour with alpha 255, and over a transparent pixel the written alpha equals the coverage. |
| FN21 | Compositing over an opaque pixel keeps alpha 255, and each channel moves from *D* towards *C* by trunc((*C* − *D*) · (*s* + 1) / 256), never past *C*. |
| FN22 | Underline thickness is ⌊(line height + leading) / 20⌋ + 1 ≥ 1; its first row is one thickness below the baseline row; it spans advance + 1 columns (more when *B* < 0). |
| FN23 | A list-item character's pen advance exceeds its plain advance by exactly *X* = bullet image width + bullet spacing; line heights do not change. |
| FN24 | Truncation: if the content fits (step 3 of §7.6), nothing is cut and no suffix is drawn; otherwise the drawn width of the cut line ≤ element width − margins whenever every advance is ≥ 0 and *S* ≤ element width − margins − cut-line indent. |
| FN25 | Layout, line breaks and glyph positions of a text element are identical at every UI scale (§6.3); only the displayed image changes. |
| FN26 | A text element without a display node of its own converts a mouse position with the scale of the nearest display node above it, never with 1 unless no such node exists (§6.3 items 1 and 3, §13.12). With every scale 1 the conversion is the identity plus the scroll offsets. |

---

<a id="s13"></a>
## 13. Synthetic worked examples (for tests)

<a id="s13-1"></a>
### 13.1 A four-glyph record

A synthetic font, DID 0x42AB0001: line height 10, widest glyph 6, four glyphs, borders 1,
baseline 8, foreground atlas 0x41AB0010 and background atlas 0x41AB0011 (both 32 × 12, A8,
category 13).

| Glyph | W | H | A | B | T | Code | X | Y |
|---|---|---|---|---|---|---|---|---|
| space | 3 | 10 | 0 | 1 | 0 | U+0020 | 1 | 1 |
| `?` | 4 | 7 | 0 | 1 | 1 | U+003F | 6 | 1 |
| `A` | 6 | 7 | 0 | 1 | 1 | U+0041 | 12 | 1 |
| `j` | 3 | 8 | −2 | 1 | 2 | U+006A | 20 | 1 |

Its 80 bytes, in hexadecimal (rows of 16):

```
01 00 ab 42 0a 00 00 00 06 00 00 00 04 00 00 00
03 0a 00 01 00 20 00 01 00 01 00 04 07 00 01 01
3f 00 06 00 01 00 06 07 00 01 01 41 00 0c 00 01
00 03 08 fe 01 02 6a 00 14 00 01 00 01 00 00 00
01 00 00 00 08 00 00 00 10 00 ab 41 11 00 ab 41
```

Checks: 36 + 11 × 4 = 80 (FN1); packing gives X = 1, 1 + 3 + 2 = 6, 6 + 4 + 2 = 12,
12 + 6 + 2 = 20, and the last bordered box ends at column 24 ≤ 32 (FN10); atlas height
1 + 10 + 1 = 12; `A` and `?` end on the baseline (1 + 7 = 8); `j` descends 2 below it.

<a id="s13-2"></a>
### 13.2 Laying out "jA ?" on one line

Line height 10, line baseline 8, leading 0, line top at y = 0.

| Step | Pen before | Box (left, top, W × H) | Advance |
|---|---|---|---|
| line start | — | first glyph `j` has *A* = −2, so indent = 1 − (−2) = 3 | pen = 3 |
| `j` | 3 | (3 − 2 = 1, 0 + 8 − 8 + 2 = 2, 3 × 8) | −2 + 3 + 1 = 2 |
| `A` | 5 | (5, 1, 6 × 7) | 7 |
| space | 12 | (12, 0, 3 × 10), empty | 4 |
| `?` | 16 | (16, 1, 4 × 7) | 5 |

Final pen position = 21. Line width = 21 − *T* of `?` = 21 − 1 = **20**, the right edge
of the `?` box (16 + 0 + 4). (*T* of each character here is its *B* = 1, because no *T*
is negative.)

<a id="s13-3"></a>
### 13.3 Substitution and outline

- "é" (U+00E9) is not in the font: it is drawn as `?` with the `?` metrics (§5).
- With the outline flag set and the background atlas present, `A` of §13.2 first gets the
  outline box (4, 0, 8 × 9) sampled from the background atlas at (11, 0) in the outline
  colour; after all outlines, its fill box (5, 1, 6 × 7) from the foreground atlas at
  (12, 1).
- With a background DID of 0, the outline of `A` is the foreground box (5, 1, 6 × 7) drawn in
  the outline colour at the eight positions (4, 0), (5, 0), (6, 0), (4, 1), (6, 1), (4, 2),
  (5, 2), (6, 2), in that order.

<a id="s13-4"></a>
### 13.4 Breaking

Available width 10, text "A A A" in the font of §13.1, *f* = 0.

- First space: 7 + 0 + 3 = 10, not greater than 10, so it fits; the break opportunity
  before it is recorded; pen 11.
- Second `A`: 11 + 0 + 6 = 17 > 10, it overflows; it is not an opportunity itself, so the
  line ends at the recorded opportunity before the space, and the space is consumed.
  Line 1 = "A", width 7 − 1 = **6**.
- Line 2 starts with the second `A` at pen 0 (no indent, *A* = 0); the same happens again
  at the third `A`. Result: three lines "A", "A", "A", each width 6, height 10, text
  height 30.

**Runs of spaces.** Available width 13, text "A", three spaces, "A":

- `A` pen 0 → 7; space 1: 7 + 3 = 10 ≤ 13, opportunity recorded, pen 11;
- space 2: 11 + 3 = 14 > 13, overflows; it is itself an opportunity, so the line ends
  before it and space 2 is consumed. Line 1 = "A" + space 1, width 11 − 1 = **10** (space 1
  stays and counts);
- line 2 starts with space 3 (pen 0 → 4, *A* = 0 so no indent), then `A` (4 + 6 = 10 ≤ 13,
  fits). Line 2 = space 3 + "A", width 11 − 1 = **10**; the space is laid out as a blank
  advance.

**Over-long word.** Available width 10, text "AAA" (no opportunity): the second `A`
overflows (7 + 6 = 13 > 10) and the line has no opportunity, so the line ends just before
it: lines "A", "A", "A", width 6 each.

Break opportunities: in "well-known" a break is possible before `k` (after the hyphen); in
"( x" none before the space (it follows `(`); in "x, y" none before the comma but one before
the space; between two CJK ideographs always, but never before U+3002.

<a id="s13-5"></a>
### 13.5 Choosing fonts (values of this install)

- Chat type `Global` (enum value 0x18; bit 23 of `GOP_ChatTextType`): default size step 3
  (XLarge) → `UI_ChatFonts` step 3 → 0x42000016.
- Quest dispenser: two presses of "increase" from the start index 0 → index 2 → the 7-entry
  ladder gives its third entry (height 24) and the twist-name ladder its third (height 26).
  At index 6 the 7-entry ladders take height 36 while the twist name keeps the entry it had.
- A chat line whose chat type has no entry in the chat-type map: size step 0 → 0x42000001,
  colour opaque white. A line of type `Global` followed by a line of a Medium type: the
  first line stays in 0x42000016 (line height 20), the second is in 0x42000018 (line
  height 16).
- A text element with no font property anywhere in its chain: both slots hold the locale
  default font, 0x42000028 with the English locale record.
- Element with normal slot font *N* and tag slot font *G*, markup on, text "ab", then an
  `<rgb=…>` span "cd", then a recognised link whose label is "ef": `a b c d` are laid out
  in *N* (`c d` in the span's colour), `e f` in *G* in the tag colour, or in the link's own
  colour if it supplies one.

<a id="s13-6"></a>
### 13.6 Blank lines and a final newline

Font of §13.1, wrapping on, wide element, *f* = 0.2 (`UICore_Text_extra_leading` = 0.2,
locale value 0), so the leading of a 10-pixel line is 2 (10 × 0.2, truncated).

Text "A", newline, newline, "A":

| Line | Characters | Pen after last | Width | Height | Baseline | Leading | Top |
|---|---|---|---|---|---|---|---|
| 1 | `A`, newline (`?` metrics) | 7 + 5 = 12 | 12 − 1 = 11 | 10 | 8 | 2 | 0 |
| 2 | newline | 5 | 5 − 1 = 4 | 10 | 8 | 2 | 12 |
| 3 | `A` | 7 | 6 | 10 | 8 | (2, dropped) | 24 |

Text height = 12 + 12 + 10 = 34. Neither newline is drawn.

Text "A", newline: line 1 as above (width 11, height 10, leading 2); line 2 is the extra
line: width 0, height 10, baseline 0, leading 0, top 12. Text height = 12 + 10 = 22 (line
1 keeps its leading because the dropped last leading is line 2's 0).

Empty text: one line, all metrics 0; text height 0.

With `UICore_Text_one_line` = true, text "A", newline, "A" is one line: pen 7 + 5 + 7 = 19,
width 18; the newline is not drawn, so the second `A` is drawn at pen 7 while the width
still counts the newline's 5.

With `UICore_Text_one_line` = true, text "A", newline has two lines, exactly as with
wrapping on: line 1 holds `A` and the newline (width 11, height 10, baseline 8, leading
2, top 0); line 2 is the §7.1 case-2 line (width 0, height 10, baseline 0, leading 0, top
12). Text height = 12 + 10 = 22. Text "A", newline, "A", newline with wrapping off also
has two lines: line 1 holds all four characters (pen 7 + 5 + 7 + 5 = 24, width 23), line 2
is the case-2 line.

<a id="s13-7"></a>
### 13.7 Alignment with margins

Element 40 × 30, margins left 2, right 2, top 3, bottom 1; one line "A" in the font of
§13.1 (width 6, text height 10), and that font in the normal slot (line height 10).

Horizontal: content width = 6 + 2 + 2 = 10; available = max(40, 10) − 4 = 36; used = 6.

| Value | Offset | Pen x of `A` |
|---|---|---|
| 4 or 1 | 36 − 6 = 30 | 2 + 30 = 32 (box ends at 38 = 40 − right margin) |
| 3 | 18 − 3 = 15 | 17 |
| 0, 2, 5 | 0 | 2 |

Vertical: content height = 10 + 3 + 1 = 14; available = max(30, 14) − 4 = 26; used =
max(14, 10) = 14.

| Value | Offset | Line top | Top of the `A` box |
|---|---|---|---|
| 1 or 4 | 26 − 14 = 12 | 3 + 12 = 15 (= 30 − 10 − 3 − 2·1) | 15 + 8 − 8 + 1 = 16 |
| 3 | 13 − 7 = 6 | 9 | 10 |
| 0, 2, 5 | 0 | 3 | 4 |

<a id="s13-8"></a>
### 13.8 Compositing arithmetic (§8.4)

Text colour *C* = (R 200, G 100, B 0), opaque (*α* = 255). Pixels written as
(A, R, G, B):

| Destination before | Coverage *c* | *s*, *a* | Rule | Destination after |
|---|---|---|---|---|
| (0, 0, 0, 0) | 128 | 128, 129 | step 4 (transparent) | (128, 200, 100, 0) |
| (255, 0, 0, 255) | 128 | 128, 129 | step 5: R 0 − trunc(−200·129/256) = 100; G 0 − trunc(−100·129/256) = 50; B 255 − trunc(255·129/256) = 127 | (255, 100, 50, 127) |
| (255, 0, 0, 255) | 0 | 0 | step 2 | unchanged |
| (255, 0, 0, 255) | 255 | 255, 256 | step 4 (full) | (255, 200, 100, 0) |

Partial alpha: destination (128, 200, 100, 0), colour (0, 0, 255) opaque, *c* = 64: *s* = 64,
*a* = 65, *t* = 129, ⌊129 · 65 / 256⌋ = 32, *k* = 129 + 65 − 32 = 162. R = 200 −
trunc(200 · 65 / 162) = 120; G = 100 − trunc(100 · 65 / 162) = 60; B = 0 −
trunc(−255 · 65 / 162) = 102; A = 161. Result (161, 120, 60, 102).

Translucent colour: white with *α* = 127 and *c* = 255 gives *s* = ⌊255 · 128 / 256⌋ = 127,
*a* = 128. Over (255, 0, 0, 0) the result is (255, 127, 127, 127). Over a transparent pixel
the result is (255, 255, 255, 255): step 4 writes the raw coverage as alpha.

Colour packing: the Colour property (0.5, 0.25, 1.0, 1.0) packs to A 255, R 127, G 63,
B 255, that is 0xFF7F3FFF.

<a id="s13-9"></a>
### 13.9 Underline (§8.5)

Font of §13.1, line top 0, line height 10, baseline *L* = 8, leading 0: *t* = ⌊10 / 20⌋ + 1
= 1. In "jA ?" of §13.2 with `A` underlined: `A` has pen 5 and advance 7, so the underline
covers row 9 (8 + 1 to 8 + 2 − 1), columns 5 to 12. The `A` box ends at row 7.

With leading 2 (*f* = 0.2): *t* = ⌊12 / 20⌋ + 1 = 1. A 20-pixel line with leading 2:
*t* = ⌊22 / 20⌋ + 1 = 2, rows *L* + 2 to *L* + 3. A glyph with advance 5 and *B* = −1 at
pen 10: columns 10 to 16.

<a id="s13-10"></a>
### 13.10 Truncation (§7.6)

Font of §13.1, margins 0, *f* = 0. Suffix text "?": laid out alone, pen 5, width 5 − 1 =
*S* = 4.

**Wrapping off**, element width 30, text "AAAAA" (width 35 − 1 = 34 > 30, so it is cut).
*R* = 30 − 4 − 0 = 26. First `A`: 26 > 7, not last, *R* = 19. Second: *R* = 12. Third: *R* =
5. Fourth: 5 is not greater than 7, so the walk stops (*R* = −2). Cut index 3. Cut-line
width = 0 + 21 + 4 = 25; content width 25. Drawn: `A` at pens 0, 7, 14, then the suffix `?`
at pen 21 (box at (21, 1), 4 × 7). With element width 35 the text fits and nothing is cut.

**Wrapping on**, element 30 × 25, text "AAA AAA AAA" (three lines "AAA", each width 20 and
height 10; the two spaces are consumed by wraps), suffix "??" (*S* = 10 − 1 = 9). Content
height 30 > 25, so it is cut. *H*ₐ = 25 − 0 − 0 − 1 = 24; running sums 10, 20, 30: the cut
line is line 1. *R* = 30 − 9 − 0 = 21: first `A` of line 1, 21 > 7, *R* = 14; second,
14 > 7, *R* = 7; third, 7 is not greater than 7, stop. The cut index is the third `A` of
line 1. Drawn: line 0 "AAA", then on line 1 "AA" and the suffix "??" from pen 14; line 2
is not drawn. Content width = max(20, 14 + 9) = 23, content height 20.

<a id="s13-11"></a>
### 13.11 List item with a bullet (§7.5)

Font of §13.1, an element with a 5 × 4 bullet image and `UICore_Text_bullet_spacing` = 2,
so *X* = 7. Text `<li>A`, with the opening break dropped at the start of the text (§7.5),
margins 0, left alignment. `A` carries the list-item flag.

- Layout: overflow test 0 + 7 + 0 + 6 = 13; pen after `A` = 7 + 7 = 14; line width
  14 − 1 = 13; line height 10, baseline 8 (the bullet does not change them).
- Drawing: bullet image at (0, 8 − 4 = 4), rows 4 to 7, columns 0 to 4; then the pen
  moves to 7 and `A` is drawn at (7, 1), 6 × 7.

<a id="s13-12"></a>
### 13.12 Mouse position in a text element of a scaled panel (§6.3)

A root panel has a display node with scale (1.5, 1.5, 1); a Text element inside it has no
node of its own, so its scale (§6.3 item 1) is (1.5, 1.5, 1). Its horizontal and vertical
scroll offsets are 0 and 5. The pointer is at (31, 16) relative to the Text element's
displayed box.

- *x* = trunc(31 ÷ 1.5) + 0 = trunc(20.67) = 20; *y* = trunc(16 ÷ 1.5) + 5 = 10 + 5 = 15.
  The text coordinates (20, 15) go to the character, link and selection lookups.
- Reading the element's scale as (1, 1, 1) because it has no node of its own would give
  (31, 21), which is wrong (FN26).

---

<a id="s14"></a>
## 14. Corrections and additions to other specs

| Spec | Statement | Correction or addition |
|---|---|---|
| `lua-plugin-api.md` §14.1 | Offset 4 is the "nominal point size"; layout after offset 8 OPEN | Offset 4 is the **line height in pixels** (the largest glyph height); it happens to equal the number in the enum label. The full layout is §3.2 here; the OPEN is answered. |
| `texture-list.md` §6.2 | "The 48th ends with one such DID followed by a value that is not a surface DID"; font layout OPEN | The 48th (0x4200002A) has background DID **0**: it has no outline atlas (§4.3). The full layout is §3 here. |
| `records.md` §5.4 | Meaning of the category field OPEN; 13 "only with A8 (95 records)" | 13 = **font glyph atlas** (exactly the 95 atlases of §4); BIN: the client's source-asset rule gives 13 to images under a `ui\fonts` folder or named `font_…`, and 15 to images under `surfaces\ui\` (UI art), ignoring case. Other values are not covered here. |
| `records.md` §2.1 (row 0x42) | Fonts identified through the `FONT` DID-map labels only | Layout, atlases and use are now specified here (§3–§9). |
| `property-system.md` §9 | DB-object code 0x4C, 6 values | Those 6 values are the `UI_ChatFont_FontDataID` entries of `UI_ChatFonts` (§9.3). UI layouts hold many more font DIDs (§9.1); they are not counted there because UI layouts are not property-set records. |
| `string-table.md` §10.3 | Locale record fields "a font (0x42 prefix)" and "unknown (read by UI text)" | Addition (BIN): the locale object's font DID is the **initial font of both slots of every UI text element** (§9.1); the next 32-bit value is the **base leading factor** *f*₀ of every UI text element, used as an IEEE single (§7.1). This fits the field order assumed there and does not change its Inferred mark. |
| `string-table.md` §4.7 and G33 | "Markup is interpreted by UI text elements whose markup option is on … In other elements every character is shown literally." | Refinement (BIN): `UICore_Text_parse_markup_tags` gates only colour, emphasis, paragraph and list markup. A tag is first offered to the link registry, and a recognised link kind is handled whether or not that property is set (§9.1). |
| `string-table.md` §4.7 | Bold "turns bold on / off", italic "turns italic on / off"; OPEN "whether a list item draws a bullet glyph and how deep the indent is" | Addition (BIN, revision 4): bold and italic set flags that no text routine tests, so they have no visible effect in UI text elements (§8.6). Underline draws the rectangle of §8.5. The list-item OPEN is answered by §7.5: the bullet is the element's `UICore_Text_bullet_image`, and the extra amount is its width + `UICore_Text_bullet_spacing`, applied to the first character of the item. The element option that drops the break of `<p>` and `<li>` at the very start of the text also drops it right after a newline (§7.5). |
| `ui-behaviour.md` §10.1 (truncation stated a second time, with line heights only and the fit rule OPEN; census 318 + 1 + 5) | Two statements of the truncation rule | **Applied there** (UB revision 2): UB §10.1 now refers to §7.6 here and keeps only the UI-level effects; the census is reconciled to 325 values (§7.6). | BIN, OBS |
| this spec, revision 5 | §6.3 item 1: "an element without a display node has the scale (1, 1, 1)"; item 3: positions are divided by "the scale" | Corrected in revision 6 (BIN question 25): the scale is that of the nearest display node at or above the element, (1, 1, 1) only when there is none; text hit tests always divide by its *x* and *y*. Agrees with `ui-behaviour.md` §4.1 step 3 and §9.2.1. | BIN |
| this spec, revision 4 | §6.3 item 4: "the scaler component sets a panel's own size to its screen size ÷ scale" | Corrected in revision 5: the scaler requests unscaled size × scale and resizes in Stretch, keeping the layout box; see `ui-behaviour.md` §9.2.1. | BIN |
| this spec, revision 3 | §6.1 and F10: "the drawing pass uses the same sum stored per character and clamped to 0–255" | Corrected in revision 4: the drawing pen uses the glyph's unclamped *A* + *W* + *B* (0 when there is no glyph); the clamped per-character value serves only the underline extent and the truncation measure. No visible difference with the fonts of this install (all advances 0–37). |
| this spec, revision 3 | §8.4: "renders an element's text into an image that belongs to the element" | Corrected in revision 4: the element paints into the canvas the UI paint pass gives it, after its own background art; whose canvas it is stays OPEN (Q6). |
| this spec, revision 1 | §7.2 "line width … equals the final pen position"; §9.1 rule 4 "a DID that is 0 or does not resolve leaves the slot without a font … zero width"; §13.2 width 21; §13.4 width 7 | Corrected in revision 2: line width = final pen − *T* of the last character (§13.2: 20, §13.4: 6); a DID of 0 or an unresolvable DID leaves the slot's font unchanged, and text without any font advances 1 pixel per character. |

---

<a id="s15"></a>
## 15. Open questions

| # | Question | Where | Suggested method |
|---|---|---|---|
| Q1 | Font records in other language archives (DE, FR) | §3.1 | M1 on an install with those archives |
| Q2 | Narrowed in revision 2: *f* = *f*₀ + `UICore_Text_extra_leading` (BIN). Still open: confirm from the locale-record loader that the text element's default font and *f*₀ are the record's font DID and the field after it (only *f*₀ = 0 is observed) | §7.1 | M4 (locale-record loader), M1 on other language archives |
| Q3 | **Narrowed in revision 4 (BIN):** text is painted 1 : 1 and the painted result is scaled; no font is swapped (§6.3). Still open: the filter used when the scaled result is shown, and whether each element or the whole panel is resampled. The geometry of a scaled panel (layout box kept, displayed box scaled) is settled in `ui-behaviour.md` §9.2.1 (revision 5). Tessera default until answered: UR §2.2 (whole-surface premultiplied bilinear, no mips; revision 7) | §6.3 | M3 (owner screenshot at 1.0, 1.5 and 0.5), M4 on the UI display path |
| Q4 | **Closed in revision 2 (BIN):** a tab that reaches a text element is laid out and drawn as `?` and is a break opportunity (§5, §7.3). Whether a given text source (plugin `SetText`, string-table whitespace handling) still passes tabs through is outside this spec. | §5 | — |
| Q5 | **Closed in revision 2 (BIN):** forced break before the glyph that would overflow; no re-test of moved characters (§7.3) | §7.3 | — |
| Q6 | **Narrowed in revision 4 (BIN):** the colour is applied while glyphs are composited on the CPU, with the exact arithmetic of §8.4 for a format-21 canvas. Still open: the format of the UI paint canvas and how it is blended onto the screen. Tessera rule until answered: UR Decision 1.1 (32-bit 0xAARRGGBB canvases, A8R8G8B8 layout; revision 7) | §8.4 | M4 (the UI paint pass), M3 (frame capture) |
| Q7 | **Closed in revision 4 (BIN):** a member of the element's own font-struct property (§9.1 rule 5) | §9.1 | — |
| Q8 | Quest journal index sharing; entry choice of the descending ladders | §9.2 | M4, M3 |
| Q9 | **Narrowed in revision 4:** not in a local file of this install; the client can export and import the table as XML (§9.3). Still open: where the live table is kept, the XML file's location and the value encoding | §9.3 | M3 (save a UI layout or options profile, change a chat size, look at written files), M4 on the XML writer |
| Q10 | **Narrowed in revision 4:** they hold nothing else and the client does not name them (§9.3). Still open: which record refers to them | §9.3 | M1 (scan all records for their DIDs) |
| Q11 | **Narrowed in revision 4 (BIN):** read from a chosen menu item (§9.6). Still open: which chat types the chosen size step is applied to, and which panel builds the menus (`ChatOptionFontMenu`) | §9.6 | M4 on the menu-selection routine, M3 |
| Q12 | **Closed in revision 2 (BIN):** both slots start with the locale default font (0x42000028 in English); 0 or unresolvable DIDs leave it in place (§9.1) | §9.1 | — |
| Q13 | **Closed in revision 4 (BIN):** §7.6, the only statement of the rule (`ui-behaviour.md` §10.1 refers here). Revision 5 (BIN): the switch is cleared whenever the element becomes editable or selectable. Still Probable: where and under which condition the switch is set | §7.6 | M4 (the routine that sets the switch and its callers) |
| Q14 | The second text element that one chat-log routine appends each line to without a font change | §9.3 | M4, `ui-layout.md` (chat window layout 0x22000034) |
| Q15 | **Closed in revision 4 (BIN):** §7.5 | §7.5 | — |
| Q16 | The property that supplies the underline colour of text outside a coloured link | §8.5 | M4 (the style set-up of the text element), M3 |
| Q17 | Whether floating names, damage numbers or other text drawers outside the UI text element use another path or system fonts | §8.6 | M4 |

Owner observations that would close Q3, Q9 and Q16 cheaply (not in the current owner
checklist O1–O17; none of O1–O17 concerns fonts):

- a screenshot of the chat window at UI scale 1.0, at 1.5 and at 0.5, to see whether glyph
  edges are resampled with a linear or a nearest filter and whether a reduced panel is
  smoothed (owner observation 1 of decision 0015);
- a screenshot of underlined text (for example a quest heading) next to plain text of
  another colour, to compare the underline colour with the text colour;
- whether the quest text size chosen with the dispenser's buttons survives a client restart
  (§9.2 predicts it does not);
- after changing one chat type's font size in the options, which files in the settings
  folder change.

---

## Change log

| Date | Change |
|---|---|
| 2026-10-05 | First version: record layout and census of all 48 fonts and 95 atlases; glyph lookup, placement, line layout, breaking, alignment and outline drawing from the client (BIN); font choice for UI layouts, quest text, chat, plugins and the diagnostic overlay; fallback-font guidance; corrections and additions to `lua-plugin-api.md`, `texture-list.md`, `records.md` and `property-system.md`. |
| 2026-10-05 | Revision 2, answering the review of 2026-10-05 (new M4 questions 6–11 in the BIN row; facts F25–F34). §4.3: glyphs with ink are 28,193, not 28,151 (122 inkless listed with counts). §5: newline, tab and carriage return take the `?` substitute; only the newline is skipped when drawing (closes Q4). §6.1: layout uses unclamped bearings. §7.1: leading factor *f* = *f*₀ + `UICore_Text_extra_leading` (narrows Q2); text height drops the last line's leading unless `UICore_Text_last_line_leading`; rules for blank lines, a final newline, empty text. §7.2: indent applies to every line; line width = final pen − trailing value *T* (corrects revision 1). §7.3: wrap switch is `UICore_Text_one_line` (Confirmed, default off; one-line mode ignores newlines); rule order; forced break inside over-long words (closes Q5); exactly one space/tab/CR consumed per wrap, runs of spaces. §7.2: Q15 (list-item bullet). §7.4: defaults Left/Top; both values 1 and 4 give the far side on either axis; exact used/available extents per axis, including the vertical margin rule; Q13. §8.1: outline flag defaults to off. §9.1: only link markup uses the tag slot, a link's own colour wins; slots start with the locale default font (closes Q12); a DID of 0 or an unresolvable DID leaves the font unchanged (corrects rule 4). §9.3: per-line chat font Confirmed (BIN) with the fallbacks for unmapped types and steps; Q14. §12: FN15–FN19. §13: §13.2 and §13.4 widths corrected; new §13.4 cases, §13.5 bullets, §13.6, §13.7. §14: notes to `string-table.md` §10.3 and §4.7. |
| 2026-10-06 | Revision 3, answering the re-review of 2026-10-05 (BIN question 12). §7.3: with wrapping off all characters are on the first line and a final newline still adds the §7.1 case-2 line, so such text has two lines (replaces the contradictory "the whole text is one line"); FN18 and F28 agree with it; §13.6: two wrap-off examples. F10: line layout uses unclamped bearings, only the drawing pass clamps (matches §6.1). §1 legend: defines the qualifier Inferred used in F33 and F34. |
| 2026-10-06 | Revision 4 (BIN questions 13–22; facts F35–F46; Status back to Review). New §6.3 (UI scale: 1 : 1 painting, scaled result, hit tests divide by the scale; narrows Q3). §6.1 and F10 corrected (the drawing pen uses unclamped advances). New §7.5 (list items and the bullet; closes Q15) and §7.6 (truncation to fit; closes Q13). §8.1: underline in the fill pass. §8.4 rewritten: CPU compositing arithmetic, colour packing, paint canvas (narrows Q6). New §8.5 (underline geometry; Q16 for its colour source) and §8.6 (bold and italic have no effect; no shadow; no system fonts). §9.1 rule 5: "by property" reads a member of the element's font struct (closes Q7); census of the fonts native panels use. §9.3: XML export form of the chat table and the settings-folder observation (narrows Q9); the nine game objects (narrows Q10). §9.6: `UI_Options_Menu_FontIndex` is read from menu items (narrows Q11). §11 item 4. §12: FN20–FN25. §13.8–§13.11. §14: notes to `string-table.md` §4.7 and corrections of revision 3. §15 updated. |
| 2026-10-06 | Revision 5 (BIN questions 23–24; status Accepted → Review; answers the M5 completeness critique items B2 and B3). §7.6 declared the only statement of the truncation rule (`ui-behaviour.md` §10.1 now refers here); census reconciled and stated in full (325 values on 324 elements and 1 state: 319 + 1 naming three entries, 5 null); the clearing of the switch for editable or selectable text is now BIN (F41); step 4 points to the tooltip in UB §10.2. §6.3 item 4 corrected: the scaler requests unscaled size × scale and resizes in object mode Stretch, which keeps the layout box and changes the scale vector; the "(size + 1) × scale − 1" formula is the displayed-size formula of `ui-behaviour.md` §9.2.1; F35 updated. Q3 and Q13 rows updated; §14 rows for UB §10.1 and for revision 4's §6.3 item 4. |
| 2026-10-06 | Revision 6 (BIN question 25; answers the revision-5 review, one blocking item; status stays Review). §6.3 item 1 corrected: an element's scale is the scale vector of the nearest display node at or above it (own node, else the nearest ancestor's by following parents), (1, 1, 1) only when none exists; it is the (*ex*, *ey*) of `ui-behaviour.md` §9.2.1. §6.3 item 3: the text hit tests convert through one step that always divides *x* and *y* by that scale (no 0.0002 tolerance, *z* unused), truncates and adds the scroll offsets. F35 updated; new FN26 and §13.12; §14 row for the revision-5 wording. Section numbers unchanged. |
| 2026-10-06 | Revision 7 (status Accepted → Revised, awaiting review; applies the accepted decision record [0015](../decisions/0015-ui-rendering.md) (**UR**); no new research; no fact about the original changed; section numbers unchanged). **Reworded Tessera Decisions:** §6.3 (text painted at 1 : 1 into the CPU canvas of its surface, the surface composed scaled by the GPU; no re-rasterising), §8.4 (exact CPU compositing for all text; the allowance of a GPU text path is withdrawn), §11 item 4 (Tessera's own fonts are rasterised at 1 : 1 into the same coverage form and take the §8.4 path; rasterising them at the scaled size is no longer allowed). **Tessera rules added next to OPENs:** Q3 (whole-surface premultiplied bilinear resampling, exact copy at scale 1, no mips, setting `ui_scale_filter`; UR §2.2), Q6 (32-bit 0xAARRGGBB straight-alpha canvases shared by text and art; UR Decision 1.1, 1.6); both §15 rows updated, and the owner screenshot now also asks for scale 0.5. **Cross-references added:** §4.1 (atlases decoded on the workers into CPU coverage buffers, never uploaded; UR §3), §8.4 "Not covered here" (caret and selection drawn as rectangle fills; UR §3). §1: provenance tag UR; header companion entry. |
