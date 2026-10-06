<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# User UI skins: the skin folder, `SkinDefinition.xml`, and how a skin is applied

| | |
|---|---|
| **Status** | Accepted (consolidation revision, reviewed 2026-10-06) |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | **Consolidation revision review: Claude review session (spec-writer agent), 2026-10-06: passed; status moved to Accepted.** Every `CLEAN_ROOM.md` §5.3 item holds for the changed material (header, SK51, §8.4 scaler paragraph, §8.6, D23, §12.7 steps 2–3 and pass condition, §13 rows, OPEN-1). It is functional facts only, with no code, addresses or game text. The identifiers are data labels, SK51 has a mark, and no gate is touched. B4 is resolved: D23 and §12.7 run over the `ui-layout.md` §17.2 catalogue, whose P15 is the same root as the Bags row. B6 is resolved: OPEN-1 now matches `ui-layout.md` §9.2 step 5 and §9.9, §8.6 no longer calls the pair choice OPEN, and the §13 rows are marked applied. The §8.4 citation of `ui-behaviour.md` §9.2–§9.2.1 is consistent, and the review's suggestion (c) is done. Spot checks (owner's install, read-only, 2026-10-06; fresh extraction byte-identical to the earlier one; reviewer's own resolver and the base table 0x78000011): (1) §8.6, all seven rows: 51/19/216/63/62/61/97 elements; 58/12/105/176/184/45/72 distinct images; own/state Image items 11/99, 5/11, 100/133, 8/283, 10/330, 41/20 and 42/83; none outside the base table: match. (2) SK48, over the six panels the text names (target vitals excluded): 419 distinct images; 836 items (207 own, 629 states); 85 Normal and 751 Alphablend; no Coloured-image item or Animation frame in the base table: match. (3) SK51: all 15 catalogue panels name base-table images in Image items. P11 (0 own / 29 state items) and P14 (0 / 36) name them only in states, and the other 13 also in own blocks: match. (4) Bag roots: bag 1 is based on `InventoryBag_Template`, bags 2–6 on `PlayerBagOver1_Template`, and all six resolve to `UIScaler`: match. (5) Protection: `PlayerPortrait` (depth 2) and `OpponentPortrait` (depth 4), both `TargetViewport`, carry `UISkin_CannotBeDetached`, and no root does: match. Non-blocking: the earlier suggestions (a), (b) and (d) are still open. **Consolidation revision (spec writer, 2026-10-06): submitted for review.** It answers the M5 completeness critique items B4 and B6 for this spec and the review's non-blocking suggestion (c) (change log). Earlier records follow. **M5 revision review:** Claude review session (spec-writer agent), 2026-10-06: **passed**, status Accepted. Reviewed the revision of 2026-10-06 (§6.8–§6.11, §8.6, §12.7–§12.8, SK38–SK50, D20–D24, SI10–SI13, SP18–SP24; OPEN-2, 3, 6, 7, 8, 9 (a) and (c), 11 closed; section numbers 0–15 kept). Every `CLEAN_ROOM.md` §5.3 item holds: functional facts only; no code or listing-shaped steps (the XML in §12 is synthetic data, the scaler rule is arithmetic in words); identifiers are interoperability labels from the data or the plugin API; no game text, keys or hostnames; every section carries a provenance mark; the byte layouts are unchanged and complete; SI10–SI13 and SP18–SP24 are testable; unknowns are OPEN or stated as not traced, with Tessera's behaviour defined; no gate touched. An implementer can build the M5 skin work from this spec, `ui-layout.md` and `ui-behaviour.md` without dirty material. Spot checks (owner's install, fresh read-only extraction of the 302 layout records, the root, `EMAPPER`, `UNIQUEDB` and `UILAYOUT` maps, the element, type and state mapper chains and the base table; byte-identical to the writer's inputs; the reviewer used its own layout reader written from `ui-layout.md` §4, §6 and §7): (1) §8.6 table: all seven rows match (layout labels and DIDs, top-level labels, root kinds and bases, rectangles, edge modes, 51 / 19 / 216 / 63 / 62 / 61 / 97 elements built, 58 / 12 / 105 / 176 / 184 / 45 / 72 distinct images, own/state splits), as do the chat and bag bases, the toolbar default state and the resolved `UIScaler` type of all six bag roots; (2) SK48: 419 distinct images, 836 items (207 / 629; 85 Normal, 751 Alphablend), none outside the base table, no Coloured-image or Animation frame in it; (3) SI11/SI12: 38 roots, 3,380 Image items, no exception, 80 Colorize/Screen items only in `CharacterEquipmentPage_Page`; (4) SI13/SK42 data: `luaplugins` (key and DID as stated) has 13 top-level elements including the four named; (5) §6.9: 3,144 top-level elements resolved, 265 of the 419 shown elsewhere, 38 by more than one element of one panel; also §8.6 facts (both `GamePlay_RadarBox` builds, `ToolbarField` at depth 1 in `pvpgameplay`, the two portraits at depths 2 and 4 of type `TargetViewport`, the four alternatives' rectangles and counts), SK36 (8 protected elements in 7 records), SK14 (4,742 one-to-one 0x41 pairs) and the XSD/MUI dictionary statements. BIN (decision 0011, address ranges only): SK45 matches (origin read unless the caller asks to ignore it, truncated products with the old and new scale, clamp to screen minus new size then to 0, cached scale stored). **Non-blocking suggestions for the next revision:** (a) turn the two "not traced" items into `> **OPEN:**` callouts or D-rows: whether a failed skin-file load is retried before the next switch (§6.11), and whether top-level overrides reach the `luaplugins` controls (§6.8, SK42); move the Tessera-only behaviours of the §6.11 table (log once per activation, remember failed loads) into §10; (b) extend SK39 (or add a row) for the directly created game displays of the last §6.8 table row; (c) in §8.4 cite `ui-behaviour.md` §9.2 for which panels scale and how the scale is chosen (it agrees with §8.4 and §12.8), and say whether `UIScalerPanel` roots such as `CharacterJournalPanel` take the scaler origin; (d) extend the naming note to `UICore_Element_…`, `UI_ClassDisplay_Icon`, `MonsterPlay_Session_Skin`, the `luaplugins` element labels and the plugin API names of §6.8 (citing `lua-plugin-api.md` / `docs/ARCHITECTURE.md` §18); (e) §12.7 step 6: say what the test does when no further skinnable DID is left for *D*ₘ. History of the version accepted before this revision follows. Third review: Claude review session (spec-writer agent), 2026-10-06: **passed**, status Accepted. Every `CLEAN_ROOM.md` §5.3 item holds, and the spec can be implemented without dirty material. The one open item from the re-review is fixed: §6.7 and SK15 now give 6,187 distinct 0x41 DIDs for the four image kinds, 10 distinct 0x2A sound DIDs (692 references), and 6,197 in all. Spot checks on the owner's install (2026-10-06), with a fresh read-only extraction of all 302 layout records (16,421 elements), 735 enum mappers, 24 DID maps and 1,616 property sets: §6.7/SK15 (10,769 / 10,560 / 29 / 180 references; 5,122 / 5,105 / 16 / 7 distinct; 2,252 frame references, 1,066 distinct and 1,065 frame-only; 6,187 / 10 / 6,197); SK12/SK13 and §5.1–§5.2 (label resolution, 5,623 + 2 and 19,893 + 44 labels, key ranges, no ASCII-case collisions, no non-ASCII labels, longest 63 and 116); SK14 (4,742 pairs, both columns distinct, all 0x41, all named by Image items, 6 among Coloured-image DIDs, none Animation or Cursor); SK16 (378 pairs: 322 different / 4 same / 52 without a base entry, 13 shared replacements); SK36/SI9 (8 `UISkin_CannotBeDetached` elements in 7 records, depths 1–4, typed, none a base); 6,224 elements with non-zero edge modes; 218 scaler-origin elements; the §12.2 example label and key; OBS (no `ui/` folder, 25 `[UI]` keys, no skin key). Non-blocking suggestion for the next revision: in the naming note, cite the interoperability rationale (the *Google v. Oracle* row of `docs/LEGAL.md` §4.1, as `plugin-descriptor.md` does) and keep `docs/LEGAL.md` §10 for the naming rule only. Earlier re-review: Claude review session (spec-writer agent), review 2026-10-05: changes requested. All five earlier issues are repaired, and every §5.3 item passes except one census statement that a spot check over all 302 layout records contradicts: §6.7 says "All media kinds together name 6,187 distinct DIDs", but with the 10 Sound-item DIDs (prefix 0x2A) all media kinds name 6,197; 6,187 is the count for the four image kinds (Image, Coloured image, Cursor, Animation; all prefix 0x41). Matched on re-check: the §6.7 and SK15 per-kind census (10,769 / 5,122 / 4,742 / 9,877; 5,105, 16 (6), 7 (0); 2,252 / 1,066 / 1,065, none in the base table; 29 Coloured-image references, 12 in the base table; 11 file-path JPEG items), SK36 / SI9 (8 protected elements in 7 records, typed, depths 1–4, none a base), the 6,224 / 10,197 edge-mode split, the 218 scaler-origin elements, the §5.3 ID census, and the base and game-skin tables. BIN matched (build 4906.0071.1794.4035, address ranges only): SK34 (per-build resolved copy, discarded after the build; the matcher is called only from the two build paths), §8.3 matching, §8.4 (own-property-set protection lookups, the field writes, no `has_position`), §7.1 dispatch, §6.4 step 5, §7.4 (header fields, low-byte bit count, size formula, linear-size check for FourCC only) and §6.5 (fill values, truncation, the white strip). Earlier record: Claude review session (spec-writer agent), review 2026-10-05: changes requested (has_position vs `ui-layout.md` §5.3/§9.8; override applied to the resolved descriptor not stated; stale `ui-layout.md` cross-references, §2 anchoring values, OPEN-5/OPEN-10; DDS header offsets; placeholder texel values) |
| **Implements into** | `crates/ui` (`tessera-ui`: skin discovery, definition parser, image override and geometry override application), `crates/material` or `crates/assets` (decoding skin image files: TGA, JPEG, DDS), `crates/props` (reading the base and game skin property sets), `crates/install` (settings folder, `install-layout.md` §8), `crates/settings` (Tessera's own skin selection), `apps/client` (options entry), `fuzz/` (one target for the definition parser, one per image decoder) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (UI layout records 0x22; cited here: §17 the M5 native-panel catalogue, §4.4 edge-mode fields, §5.3 rectangle and `has_position`, §5.8 edge-mode values, §6.2–§6.5 media items that name images, §7 base inheritance, §8.4 run-time layout, §9.1–§9.2 building panels and element trees, §9.6 drawing an element's images, §9.8 the user-skin hook), `install-layout.md` §8 (settings folder), `records.md` §7–§8 (enum mappers, DID maps), `property-system.md` §4–§5 (property sets), `plugin-descriptor.md` §9 (compendium side files), `texture-list.md` / `material.md` (render surfaces 0x41), [`lua-plugin-api.md`](lua-plugin-api.md) §13.8 and §14.2 (plugin backgrounds and the game-styled plugin controls, §6.8 here) |

> **Naming note (interoperability).** The folder and file names (`ui`, `skins`,
> `SkinDefinition.xml`), the XML element and attribute names (`SkinName`, `Name`, `Mapping`,
> `ArtAssetID`, `FileName`, `PanelFile`, `ID`, `Element`, `X`, `Y`, `Width`, `Height`,
> `Detach`, `ScaleOriginX`, `ScaleOriginY`), the DID-map labels (`EMAPPER`, `UNIQUEDB`,
> `STRINGTABLE`, `ArtAssetID`, `UIElementID`, `BaseSkinMappings`, `MonsterPlaySkin`,
> `UI_Option`, `UILAYOUT`), the property names (`UISkin_…`, `GOP_UI_DefaultSkin`,
> `UIComponent_UIScaler_…`, `UICore_Meter_…`), the layout labels and element labels of §8.6
> and the enum labels quoted as examples are identifiers that existing skins and the user's own
> data use. They appear here only because a compatible reader cannot work without them: a
> skin names elements and art assets by exactly these labels, and the client finds layouts by
> them. The interoperability rationale is the *Google v. Oracle* row of `docs/LEGAL.md` §4.1
> and `docs/ARCHITECTURE.md` §15; `docs/LEGAL.md` §10 is cited for the naming rule only: these
> identifiers must not become Tessera product, crate or UI names. All descriptive text is our
> own; no third-party prose, schema comment, client message text or game text is reproduced.

---

<a id="s0"></a>
## 0. Purpose

A *user skin* is a folder of image files plus one XML file, written by players, that the
original client reads from the user's settings folder. A skin can do exactly two things:

1. **replace UI art**: an image that the UI would take from the archives is taken from a file
   in the skin folder instead (*mappings*, §4.4, §6);
2. **move and resize UI elements**: the rectangle of a named element in a named panel is
   replaced before the panel is built (*panel overrides*, §4.5, §8).

This spec lets an implementer build, without reading dirty material:

- discovery of skins below the settings folder, and the skin list the user chooses from (§3,
  §9.1);
- a parser that accepts and rejects `SkinDefinition.xml` files exactly as the original does,
  including its defaults, case rules, number parsing and duplicate handling (§4);
- the resolution of the skin's symbolic names against the user's own data: art-asset names
  and element names are labels of two enum mappers, and art-asset values map to render
  surfaces through a property-set record (§5, §6.1);
- the per-image precedence between a user skin, a data-defined *game skin* and the base art,
  and the fallback when a file is missing or unreadable (§6);
- the image file formats the original accepts, and how it decodes them (§7);
- the matching and application of geometry overrides to layout elements (§8);
- selection, activation and the moments at which skins are read (§9);
- which of the client's image paths a skin can reach, how a skin replaces art per element and
  per state, the image size rules and the complete failure behaviour (§6.8–§6.11);
- the skin identifiers (layout labels and top-level element IDs) of the native panels that
  M5 builds (minimap, chat, toolbar, vitals, bags, character panel) with their art census
  (§8.6), and a test procedure for ROADMAP M5 exit criterion 3 that uses only a synthetic skin
  generated by the test (§12.7);
- Tessera's own decisions where the original's behaviour is undefined, unsafe or bound to
  Windows (§10).

All counts are observations of one install on one date. Every DID, enum value and property
ID quoted is an observation that an implementation **resolves at run time** through the
labels named here; none is a constant to hard-code.

---

<a id="s1"></a>
## 1. Provenance

Legend: **Confirmed** = observed in the user's install on 2026-10-05 (rows SK38 and later:
2026-10-06) by read-only throwaway probes over **all** records of the kind; **Confirmed (BIN)** = read from the client
executable under decision 0011 and resting on it alone; **Reported** = published sources
only; **Decision** = Tessera's own choice (§10), not a fact about the original.

| Tag | Source |
|---|---|
| **OBS** | The owner's Steam install (data current on 2026-10-05), read in place: every enum mapper (0x23, 735 records) and DID map (0x28, 24) of `client_general.dat`; every stand-alone property set (0x78, 1,616) of `client_gamelogic.dat`; the property dictionary 0x34000000; every UI layout record (0x22, 302 records, 16,421 elements, parsed completely with the record grammar of `ui-layout.md` §4); the DID lists of every render-surface archive. Also the owner's settings folder inside the Proton prefix (`…/Documents/The Lord of the Rings Online/`): it has **no `ui/` folder**, so no user skin is installed, and its `UserPreferences.ini` has no skin-related key (the `[UI]` section holds 25 unrelated keys). On 2026-10-06 (M5 revision), re-extracted read-only: the `UILAYOUT` DID map (236 entries), the element, state, type and art-asset mapper chains, the base table and the game skin, and all 302 layout records; the panel trees of §8.6 were built with the resolution rules of `ui-layout.md` §7 and censused completely. |
| **BIN** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (changelist 711794, compiled 2026-09-27, SHA-256 prefix `d22363eb9aff0fb5`), read with capstone, lief and pefile in the dirty room on 2026-10-05, function by function (address ranges only). **Questions:** (1) where the client looks for skins, to what depth, and how it matches the file name; (2) the exact grammar its parser accepts: element and attribute names, case rules, defaults, number parsing, duplicate and error handling, and when a file is rejected; (3) how art-asset names and element names are resolved against data; (4) the per-image precedence between user skin, game skin and base art, and the fallback for missing files; (5) which image file formats it decodes and how; (6) how panel overrides are matched to layout elements and what they change; (7) how the skin list is built, how the selection is stored and when a skin is activated or re-read; (8) on which descriptor an override acts (before or after base inheritance, a shared or a per-build copy), which of its fields it writes (in particular whether it sets `has_position`), and from where the override code can be reached; (9) which fields of a DDS header the reader uses and how it sizes the pixel data; (10) the stored 8-bit texel values of the placeholder image. Questions 8–10 were added on 2026-10-05 in answer to the first review. **M5 revision questions (same build and hash, read on 2026-10-06):** (11) which code paths give an element an image through the substitution of §6.3 and which create images directly (Image, Animation, Coloured-image, Hit-mask and Cursor items; meter frame arrays and meter child images; images that game code sets at run time; plugin backgrounds); (12) what the refresh after a skin switch does to existing elements, and whether the switch or the option handler that calls it rebuilds panels; (13) when the user skin is deactivated; (14) how the UI scaler uses the scaler-origin properties that an override writes; (15) a second, independent reading of the duplicate-mapping check of §4.4; (16) from which layout the plugin API's game-styled controls are built. **Why M1–M3 were not enough:** the data holds the base art table and the name labels (M1, used throughout), but the parser rules, the precedence, the image decoders and the activation are code; this owner has no skin installed to observe (M1); the published documents are from 2007–2014, do not cover most of these rules and partly disagree with each other (M2); no running official client is available to this session (M3). Only the offline UI-skin, UI-layout instantiation, file-search, XML-reading and image-decoding code was read: nothing about network, login, keys, the launcher or anti-tamper. Numeric constants (§6.5, §7) are functional facts under [decision 0012](../decisions/0012-numeric-constants.md). No address, listing or symbol appears in this spec. |
| **DOC** | The publisher's original skinning overview and art-asset dictionary pages (2007), as republished by the community site at `lotrointerface.com/index.php?p=ui_skinning_1` and `…_2`, fetched 2026-10-05. Facts only; no text is reproduced. |
| **RES** | The project's resource catalogue (`research/resources.json`), entries for the 2007–2009 community forum thread on UI skinning (lotrointerface.com thread 11, a repost of the publisher's beta-forum guide with later developer clarifications) and the publisher's 2014 skinning art pack; that thread now needs a login and could not be re-read on 2026-10-05. |
| **XSD** | `lunarwtr/vscode-lotro-api` (MIT): `resources/xsds/lotro-skin.xsd` and `lotro-skin-panelfiles.xsd` (community schemas), and `resources/skinning/SkinDictionary.xml` (a community dump of panel and element names with geometry); used for facts and cross-checks only. |
| **ASSOC** | `lunarwtr/lotro-api-docs`, `skinning/skinning_element_asset_assocation.xml` (no licence; a community dump pairing element names, art-asset names and image DIDs); cross-check only. |
| **MUI** | `Felix-Helleckes/MithrilUI` (MIT, 2026): a skin generator with documentation of observed client behaviour (`docs/SKINNING.md`, `docs/SAFETY.md`, `docs/INSTALL.md`, `tools/validate.py`) and two name lists mined from shipping skins (`skin/dictionary.txt`, `docs/reference-element-ids.txt`). Community claims; facts only. Its validator (`tools/validate.py`) states that of two mappings of one art asset the last one wins (§4.4, SK46). |
| **OL** | `JuneStepp/OneLauncher` (GPL-3), `src/onelauncher/addon_manager_window.py`: a third-party manager's skin discovery and install folder. Facts only. |
| **LAY** | The UI layout record grammar of `ui-layout.md` §4 and §6 (element rectangle, edge modes, element ID, base reference, media items), applied by a throwaway probe to all 302 layout records (re-run 2026-10-05 for the media-kind census of §6.7); used here only to census data. |
| **ULS** | `ui-layout.md` (status Review) for the behaviour of the layout system around the skin hook: base inheritance (§7), element creation order (§9.2), run-time layout and edge modes (§5.8, §8.4), image drawing (§9.6), file-path image items (§6.2). Facts taken from there keep that spec's marks; where this spec's BIN reading disagrees, §13 records the correction. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| SK1 | Skins live below `<settings folder>/ui/skins/`; the settings folder is the folder of the preferences file (`install-layout.md` §8.1, including `--prefs`) | BIN; DOC; RES; MUI; OL | Confirmed (BIN) |
| SK2 | Discovery searches **every** folder below the skins root, at any depth, including the root itself, for files whose name equals `SkinDefinition.xml` ignoring ASCII case; each such file is one candidate skin, and its folder is the skin folder (§3.2) | BIN | Confirmed (BIN). DOC/MUI describe one folder per skin directly below the root; the deeper search is new. |
| SK3 | Skins are discovered and parsed once, when the client's UI starts; there is no reload path (§9.4) | BIN; DOC; MUI | Confirmed (BIN) |
| SK4 | The definition is parsed by a non-validating XML parser; a file that is not well-formed is rejected and the skin is not listed (§4.1) | BIN; DOC; MUI | Confirmed (BIN) |
| SK5 | The root element's name is not checked; element names are compared ignoring ASCII case; attribute names must match exactly (§4.1) | BIN | Confirmed (BIN) |
| SK6 | Recognised children of the root: `SkinName`, `Mapping`, `PanelFile`; everything else is ignored (§4.2) | BIN; DOC; XSD | Confirmed (BIN) |
| SK7 | `SkinName`: the last one with a non-empty `Name` sets the skin's name; none gives an empty name (§4.3) | BIN | Confirmed (BIN). DOC and XSD say "exactly one". |
| SK8 | `Mapping`: both attributes must be non-empty; the art-asset name must resolve; for a repeated art-asset value the **first** mapping is kept and later ones are rejected; `FileName` is narrowed to the system code page, and a name with any unrepresentable character has every non-ASCII character replaced by the text `<0xNNNN>` (§4.4) | BIN | Confirmed (BIN). MUI's validator claims the last one wins; a second BIN reading (SK46) confirms that the first wins, so OPEN-8 is closed. |
| SK9 | `PanelFile`: `ID` must be non-empty but its value is never looked up; its `Element` children form the override tree (§4.5) | BIN; OBS (only 1 of the 109 `PanelFile` IDs listed by XSD is an element label at all) | Confirmed (BIN) |
| SK10 | `Element` attributes, integer parsing (base auto-detected, partial numbers accepted, missing means 0) and the conditions under which an element override is kept or dropped (§4.6, §4.7) | BIN | Confirmed (BIN) |
| SK11 | A definition with no kept mapping and no kept panel override is rejected (§4.8) | BIN | Confirmed (BIN) |
| SK12 | Art-asset names are labels of the enum mapper that the `EMAPPER` DID map labels `ArtAssetID`; element names are labels of the mapper it labels `UIElementID`; lookup ignores ASCII case and follows the base chain (§5) | BIN; OBS | Confirmed |
| SK13 | Label census: 5,623 + 2 art-asset labels, 19,893 + 44 element labels; no two labels of either chain are equal ignoring ASCII case (§5.3) | OBS | Confirmed |
| SK14 | The base art table is the property set that `UNIQUEDB` labels `BaseSkinMappings`: an array of (art-asset value, render-surface DID) pairs, 4,742 entries, both columns unique, every DID present in the surface archives (§6.1) | OBS; BIN (the client reads it keyed by DID, first entry wins) | Confirmed |
| SK15 | Every one of the 4,742 base DIDs is referenced by at least one Image media item of the layouts; 4,742 of the 5,122 distinct images named by Image, Coloured-image and Cursor items are in the base table; no Animation frame DID (1,066 distinct) and no Cursor DID (7) is in it; the four image kinds name 6,187 distinct 0x41 DIDs and Sound items 10 distinct 0x2A DIDs, 6,197 in all (§6.7; re-checked 2026-10-06) | OBS (LAY) | Confirmed |
| SK16 | A *game skin* (`MonsterPlaySkin`, 378 pairs of art-asset value → replacement DID, with a skin ID) is a second, data-defined art layer (§6.2) | OBS; BIN | Confirmed |
| SK17 | Per-image precedence: user skin, then game skin, then base art; only images whose DID is in the base table are affected (§6.3) | BIN | Confirmed (BIN) |
| SK18 | Image path = skin folder + `FileName`; a drive-letter or doubled-separator `FileName` replaces the folder; `\` becomes `/`; the path is lower-cased before it is opened; the TGA and IFF readers copy at most 255 bytes of the path (§6.4) | BIN; DOC (paths relative to the skin folder) | Confirmed (BIN) |
| SK19 | A mapped file that cannot be read shows a fixed 32 × 32 placeholder that tiles as a grid; the base art is **not** used as a fallback; its texels are (76, 76, 102, 255) and (255, 255, 255, 255) as 8-bit R, G, B, A (§6.5) | BIN; RES (grid pattern) | Confirmed (BIN) |
| SK20 | Loaded skin images are cached by path and the cache is emptied whenever the active skin changes, so edited images are re-read on the next switch (§6.6) | BIN; RES (image edits can be reloaded) | Confirmed (BIN) |
| SK21 | Format dispatch on the lower-cased path: contains `.dds` → DDS; else contains `.jpg` → JPEG; else contains `.tga` → TGA; else contains `.iff` → IFF; else unreadable (§7.1) | BIN; DOC/XSD (TGA, JPEG) | Confirmed (BIN). DDS and IFF are new. |
| SK22 | TGA: RLE for types 9, 10, 11 and uncompressed otherwise; depths 8, 15, 16, 24, 32; both vertical origins; each scanline decoded independently (§7.2) | BIN; DOC (TGA with alpha) | Confirmed (BIN) |
| SK23 | JPEG: decoded by an embedded IJG libjpeg (2010 release family); opaque (§7.3) | BIN | Confirmed (BIN) |
| SK24 | DDS: `DDS ` magic, DXT1–DXT5 or one of the listed uncompressed layouts; top level only; the header fields read, the size computation and the linear-size check of §7.4 | BIN | Confirmed (BIN) |
| SK25 | Panel overrides are applied while the client builds UI elements, to the element's **resolved** descriptor (after base inheritance), matched by element ID from the root of the subtree being built down through every level (§8.2, §8.3) | BIN; ULS (`ui-layout.md` §9.2 step 2) | Confirmed (BIN) |
| SK26 | Applying an override sets X and Y unless the element is marked `UISkin_CannotBeMoved`, Width and Height unless `UISkin_CannotBeResized`, sets the four edge modes to 0 unless `UISkin_CannotBeDetached`, and sets the UI-scaler origin when `ScaleOriginX` or `ScaleOriginY` was given (§8.4) | BIN; OBS (8 elements carry `UISkin_CannotBeDetached`; none carries the other two) | Confirmed (BIN) |
| SK27 | `Detach` is parsed but its value is never used by this build (§8.5) | BIN; XSD (describes it as toolbar-only) | Confirmed (BIN) |
| SK28 | The skin list shown to the user is the "no skin" entry followed by all skin names sorted by UTF-16 code unit; activation picks the first discovered skin whose name matches ignoring ASCII case (§9.1, §9.3) | BIN | Confirmed (BIN) |
| SK29 | The selection is stored as the gameplay option `GOP_UI_DefaultSkin` (a text value: the skin's name, or the localised "no skin" text); it is not a `UserPreferences.ini` setting (§9.2) | OBS; BIN | Confirmed |
| SK30 | The selected skin is activated when the client creates its in-game UI and whenever the option is applied; switching takes effect at once (§9.3) | BIN; DOC | Confirmed (BIN) |
| SK31 | Third-party managers keep a `*.skincompendium` file inside a skin folder; the client never reads it | OL; BIN (`plugin-descriptor.md` §9) | Reported (location); Confirmed (BIN) (ignored) |
| SK32 | Published name lists agree with the install: 13,108 of 13,112 ASSOC (art-asset, image DID) pairs equal the base table; all 3,759 element names of XSD's dictionary and all 693 of MUI's are element labels; 2,742 of MUI's 2,795 and 178 of DOC's 180 art-asset names are art-asset labels | OBS; ASSOC; XSD; MUI; DOC | Confirmed |
| SK33 | Applying an override writes only X, Y, Width, Height, the four edge modes, the two scaler-origin properties and (recursively) the children; it does **not** write `has_position` (§8.4) | BIN | Confirmed (BIN). Disagrees with `ui-layout.md` §5.3 and §9.8 (§13). |
| SK34 | The override acts on a per-build copy of the resolved descriptor, never on the loaded layout data; the matching code is reached only from the two element-building paths (top-level element and child element) (§8.2) | BIN | Confirmed (BIN) |
| SK35 | The child recursion of §8.4 step 5 walks the children of the resolved descriptor (inherited and merged children included); each child is then overridden a second time when it is itself built (§8.4) | BIN; ULS (`ui-layout.md` §7.3, §9.2 step 6) | Confirmed (BIN) |
| SK36 | The 8 elements that carry `UISkin_CannotBeDetached` are typed child elements (depth 1–4) and none of them is the base of another element (§8.4) | OBS (LAY) | Confirmed |
| SK37 | Because an applied override sets every edge mode to 0, the element's first run-time layout places it at the override's X, Y with the override's size, unscaled (§8.2) | ULS (`ui-layout.md` §8.3, §8.4 steps 2 and 4) with SK26 | Confirmed (BIN), derived from two BIN readings |
| SK38 | The substitution of §6.3 is built into one routine, the element's base-image setter. It is reached by Image items (from layout data, and Image items that game code installs at run time, which includes a meter's `UICore_Meter_child_image`), by Animation frames and by meter frame arrays (`UICore_Meter_frame_array`) (§6.8) | BIN | Confirmed (BIN) |
| SK39 | Coloured-image overlays, hit masks and plugin `SetBackground` images are created directly, without the substitution (§6.8) | BIN | Confirmed (BIN). Closes OPEN-9 (a). |
| SK40 | The refresh after a skin switch walks every element below the UI root, re-sets each element's base image from the render-surface DID it was set from (the original DID, not the replacement) through the substitution, keeps the draw mode, and marks the element for redraw. It does not touch overlays, text, states or geometry, and neither the switch nor the option handler that calls it rebuilds a panel (§6.6, §8.2) | BIN | Confirmed (BIN). Closes OPEN-2. |
| SK41 | A plugin `SetBackground` image given as a DID is not substituted when it is set, but the next refresh (SK40) re-sets it through the substitution; one given as a file path is never substituted (§6.8) | BIN | Confirmed (BIN) |
| SK42 | The plugin API's game-styled controls are built from top-level elements of the layout that the `UILAYOUT` map labels `luaplugins` (the client looks the map up by that entry's key), so their Image items, and with them user-skin art, reach the base-image setter (§6.8, §8.6) | BIN; OBS (13 top-level elements; every Image DID of their resolved trees is in the base table) | Confirmed. Closes OPEN-9 (c). That top-level geometry overrides reach them too is inferred from SK34 (layout elements are created only through the two building paths), not traced for these controls. |
| SK43 | Per state: each time an element enters a state, the state's Image items run and set the base image through the substitution, with the item's own draw mode (§6.9) | BIN; ULS (`ui-layout.md` §9.4–§9.5) | Confirmed (BIN) |
| SK44 | The user skin is deactivated, with a refresh, when the in-game UI object is destroyed (both in-game UI set-up paths of §9.3 have a matching tear-down); it is active only while that UI exists (§9.3) | BIN | Confirmed (BIN). Closes OPEN-6. |
| SK45 | When an element's UI scaler changes its scale, the scaler-origin properties name the point (in unscaled pixels from the element's top-left corner) that keeps its screen position; the position is then clamped to the screen (§8.4) | BIN | Confirmed (BIN). Closes OPEN-11. |
| SK46 | Second reading of the mapping check: a mapping whose art-asset value is already in the skin's mapping table is discarded and logged, so the first wins (§4.4) | BIN | Confirmed (BIN). Closes OPEN-8. |
| SK47 | The M5 panel roots of §8.6: layout labels, top-level element labels, root kinds, design rectangles, edge modes and art census | OBS (LAY); XSD (the community dictionary names four of the roots as `PanelFile` top-level elements) | Confirmed |
| SK48 | In the resolved trees of the six M5 panel roots, every Image item with a DID names a base-table DID (419 distinct) and uses draw mode Normal (85 items) or Alphablend (751 items); no Coloured-image item names a base-table DID; no Animation frame is in the base table (§8.6) | OBS (LAY) | Confirmed |
| SK49 | Neither the substitution nor the base-image setter passes or checks the size of the replaced surface; a substituted image is drawn at its own size like any base image (§6.10) | BIN; ULS (`ui-layout.md` §9.6) | Confirmed (BIN) |
| SK50 | Failure behaviour summary (§6.11) | SK4, SK8, SK11, SK19, SK21–SK24, SK28, D4, D9, D16 | Derived from the rows named |
| SK51 | The seven roots of §8.6 are panels P1–P5, P13 and P15 of the `ui-layout.md` §17.2 catalogue; the original opens the bag roots through a bag-window class (`ui-layout.md` U45) and opens no `backpack` root; every one of the fifteen catalogue panels names base-table images, two of them (P11, P14) only in state blocks (§12.7 step 3) | OBS (LAY, resolved trees, 2026-10-06); BIN via `ui-layout.md` U45 | Confirmed |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Settings folder** | The per-user folder holding the preferences file (`install-layout.md` §8). |
| **Skins root** | `<settings folder>/ui/skins/`. |
| **Definition file** | A file named `SkinDefinition.xml` (any ASCII case) below the skins root. |
| **Skin folder** | The folder that contains a definition file (the path up to and including its last separator). |
| **Skin** | A definition file that parsed and passed §4.8, with its name, its mappings and its panel overrides. |
| **Art-asset value** | The 32-bit enum key of an art-asset label (§5). |
| **Element ID value** | The 32-bit enum key of an element label (§5); the same value space as the `element_id` stored in layout elements (`ui-layout.md` §4.4 field 13, §5.2). |
| **Mapping** | (art-asset value → file name) stored by a skin. |
| **Element override** | (element ID value, X, Y, Width, Height, optional scaler origin, children) parsed from an `Element`. |
| **Panel override** | The set of top-level element overrides parsed from one `PanelFile`. |
| **Base table** | The (art-asset value, render-surface DID) pairs of `BaseSkinMappings` (§6.1). |
| **Game skin** | A data-defined art layer (§6.2). |
| **Active skin** | The user skin (and, separately, the game skin) currently applied; at most one of each in practice. |
| **Edge modes** | The four u32 values `edge_left`, `edge_top`, `edge_right`, `edge_bottom` of a layout element descriptor (`ui-layout.md` §4.4, fields 18–21), each one of the modes 0–6 of `ui-layout.md` §5.8 that say how that edge follows the parent at run time (`ui-layout.md` §8.4). Mode 0 keeps the element's current position. Earlier drafts of this spec called them "anchoring values". |
| **Resolved descriptor** | The element descriptor after base inheritance (`ui-layout.md` §7): a typed descriptor with the inherited states and children merged in. |
| **Base-image setter** | The client's one routine that gives an element its base image (`ui-layout.md` §9.6, first layer) from a render-surface DID and a draw mode; the substitution of §6.3 is part of it (§6.8). |
| **Refresh** | The walk over all existing elements that re-sets their base images after a skin switch (§6.6). |

---

<a id="s3"></a>
## 3. Where skins live and how they are found

<a id="s3-1"></a>
### 3.1 The skins root (Confirmed (BIN))

The client takes the path of the preferences file (the `--prefs` argument when given, else
`UserPreferences.ini` in the documents folder, `install-layout.md` §8.1), keeps everything
up to and including its last `\`, `/` or `:`, and appends `ui/skins`. Discovery then looks
for the file name `SkinDefinition.xml` below that folder.

Third-party managers install each skin as one folder directly below the skins root (OL,
MUI), which is also what the publisher's guide describes (DOC). The original's search is
broader (§3.2), so a nested layout also works.

<a id="s3-2"></a>
### 3.2 Discovery (Confirmed (BIN))

The search is a recursive, depth-first walk:

1. List every entry of the current folder (all names, in the order the file system returns
   them). Skip the entries `.` and `..`. No attribute is filtered: hidden folders are walked
   too.
2. For each entry, in that order:
   1. if its name matches `SkinDefinition.xml` (the comparison folds ASCII case and would
      also honour `*` and `?` wildcards, which this pattern does not contain), append the
      full path to the candidate list;
   2. if it is a folder, walk it with the same rules before continuing with the next entry.
3. Start the walk at the skins root. There is no depth limit.

Each candidate is parsed (§4). A candidate that fails parsing or §4.8 is dropped silently
(the original writes a line to its own log only). The remaining skins form the **skin list**
in discovery order. Two candidates in nested folders are two skins.

> **Note.** The publisher's guide and skin authors observe that a broken skin simply does not
> appear in the list (DOC, MUI); the code confirms that this is the only visible effect.

<a id="s3-3"></a>
### 3.3 What a skin consists of (Confirmed (BIN))

| Part | Source | Used for |
|---|---|---|
| Folder | Path of the definition file up to its last separator | Base for every `FileName` (§6.4) |
| Name | `SkinName`/`Name` (§4.3), UTF-16 text | Shown in the skin list and used to select the skin (§9) |
| Mappings | §4.4 | Art replacement (§6) |
| Panel overrides | §4.5 | Geometry (§8) |

<a id="s3-4"></a>
### 3.4 Side files

Any other file in a skin folder is read only if a `FileName` names it. A `*.skincompendium`
file (manager metadata with `SkinConfig` fields, OL and XSD) is ignored by the client
(`plugin-descriptor.md` §9).

---

<a id="s4"></a>
## 4. `SkinDefinition.xml`

<a id="s4-1"></a>
### 4.1 XML handling (Confirmed (BIN))

- The file is read with a standard XML 1.0 parser in its default, **non-validating**
  configuration (no namespace processing, no schema). A document that is not well-formed is
  rejected (§3.2). Typical causes noted by skin authors: an unclosed tag, a `--` inside a
  comment (MUI); both are ordinary XML well-formedness errors.
- Character encoding follows XML rules (byte-order mark or the encoding declaration; UTF-8
  when neither is present).
- The **root element's name is not checked**. Published skins and schemas use `opt`
  (DOC, XSD, MUI); any name works.
- The parser walks the **direct children** of the root in document order. Text, comments and
  processing instructions are skipped (their node names never match).
- **Element names** are compared with the expected names **ignoring ASCII case**
  (`mapping`, `MAPPING` and `Mapping` are the same). A namespace prefix is part of the name
  and prevents a match.
- **Attribute names** are looked up exactly (case-sensitive): `artassetid` is not
  `ArtAssetID`. A missing attribute reads as the empty string. Attribute values are the
  XML-normalised values (entities expanded, line breaks and tabs turned into spaces).

<a id="s4-2"></a>
### 4.2 Grammar summary

Children of the root:

| Element | Attributes | Effect | Rule |
|---|---|---|---|
| `SkinName` | `Name` (text) | Sets the skin name | §4.3 |
| `Mapping` | `ArtAssetID` (art-asset label), `FileName` (path) | Adds a mapping | §4.4 |
| `PanelFile` | `ID` (any non-empty text) | Adds a panel override built from its `Element` children | §4.5 |
| any other name | — | Ignored, including all of its content | — |

Children of `PanelFile` and of `Element`:

| Element | Attributes | Effect | Rule |
|---|---|---|---|
| `Element` | `ID` (element label); `X`, `Y`, `Width`, `Height`, `ScaleOriginX`, `ScaleOriginY` (integers); `Detach` (integer) | Adds an element override; nested `Element`s describe the element's children | §4.6, §4.7 |
| any other name | — | Ignored | — |

Children of `SkinName` and `Mapping` are never read.

<a id="s4-3"></a>
### 4.3 `SkinName` (Confirmed (BIN))

For each `SkinName` in document order: read `Name`; if it is non-empty, it becomes the skin's
name, replacing any earlier one. So the **last non-empty** `Name` wins. A definition without
any such `Name` gives a skin whose name is the empty string; it is still listed (§9.1).

The name is kept as UTF-16 text exactly as written (no trimming, no case change).

<a id="s4-4"></a>
### 4.4 `Mapping` (Confirmed (BIN))

For each `Mapping` in document order:

1. Read `ArtAssetID` and `FileName`. If either is empty (or missing), skip the element
   silently.
2. Resolve `ArtAssetID` to an art-asset value (§5). If it does not resolve, log and skip.
3. If the skin already holds a mapping with the **same art-asset value** (two spellings that
   differ only in ASCII case resolve to the same value), log and skip: **the first mapping
   wins**. Two independent readings of the client agree (SK8, SK46). A community validator
   says the last one wins (MUI); that is not what this build does.
4. Otherwise store (value → `FileName`). The file is **not** opened or checked at this point;
   a missing file is found only when the image is drawn (§6.5).

`FileName` is converted from UTF-16 to the system's narrow ("ANSI") code page before it is
stored. If any character cannot be represented in that code page, every non-ASCII character
of the name is replaced by the literal text `<0xNNNN>` (its code unit in four upper-case hex
digits), so such a file can never be found (Confirmed (BIN)). Tessera treats file names as
Unicode (§10, D11).

A mapping whose art-asset value has no entry in the base table (881 of the 5,623 labels in
this install, OBS) is stored and counts for §4.8 but can never change anything on screen
(§6.3).

<a id="s4-5"></a>
### 4.5 `PanelFile` (Confirmed (BIN))

For each `PanelFile` in document order:

1. Read `ID`. If it is empty, skip the element silently. The value is otherwise **never
   interpreted**: it is not resolved against any table and is used only in log messages.
   Published schemas list identifiers of the form `ID_UISkin_…` (XSD), but any non-empty text
   behaves the same.
2. Parse the `Element` children of the `PanelFile` with §4.6, giving the panel's top-level
   element overrides. If that parse reports failure (§4.6 step 3), the whole `PanelFile` is
   dropped.
3. Register the top-level element overrides with the skin. Each top-level element ID value
   may be registered by **one** `PanelFile` of a skin only. If a later `PanelFile` repeats a
   top-level element ID already registered by an earlier one, the later `PanelFile` is
   rejected and logged.
4. A `PanelFile` that parsed without failure but kept no `Element` (none present, or all
   dropped by §4.6 step 5) is still registered: it changes nothing on screen but counts as a
   panel override for §4.8.

> **Note (original's defect).** When the client rejects a `PanelFile` in step 3, it has
> already recorded the panel and the IDs that preceded the repeat before discarding the
> panel's storage, so its later behaviour for that skin is undefined. Tessera rejects the
> later `PanelFile` as a whole and registers none of its IDs (§10, D8).

<a id="s4-6"></a>
### 4.6 Parsing an `Element` level (Confirmed (BIN))

The same procedure parses the children of a `PanelFile` and, recursively, the children of an
`Element`. For one level, with *parent* being the panel or the enclosing element override:

1. Take the children of the node in document order; skip any whose name is not `Element`
   (ASCII case ignored).
2. For an `Element`, create an override with every field 0 and no children, then read:
   - `ID`: if non-empty, resolve it to an element ID value (§5);
   - `X`, `Y`, `Width`, `Height`: each, if non-empty, parsed by §4.7; otherwise 0;
   - `ScaleOriginX`, `ScaleOriginY`: each, if non-empty, parsed by §4.7 and the override is
     marked *has scaler origin*; a missing one is 0;
   - `Detach`: if non-empty, parsed by §4.7; the override records "detach" when the result
     is non-zero (§8.5).
3. If `ID` is non-empty and does **not** resolve: log, discard this override, and **stop this
   level** with a failure result. The `Element`s after it on the same level are not read.
   - At the top level (directly under `PanelFile`) the failure drops the whole `PanelFile`
     (§4.5 step 2).
   - At a deeper level the failure is ignored by the enclosing level: the enclosing element
     override is still kept, with the children that were parsed before the failing one.
4. Parse this `Element`'s own children as the next level (recursion; there is no depth
   limit). Its result is ignored (see step 3).
5. Keep the override in *parent* only if
   - its element ID value is not 0 (an empty `ID`, or a label whose value is 0, gives 0), and
   - either both `Width` and `Height` are non-zero, or it has at least one kept child.
   Otherwise drop it silently (with its children).
6. If *parent* already holds a child with the same element ID value, the new one is dropped:
   **the first sibling wins**.

Consequences that skin authors rely on (MUI; Confirmed (BIN)):

- A leaf `Element` without `Width` and `Height` does nothing.
- A container `Element` without `Width`/`Height` is kept (it has children) and, when applied,
  sets the element's size to 0 × 0 (and its position to 0, 0 when `X` and `Y` are missing as
  well) (§8.4). In practice every `Element` must state all four of `X`, `Y`, `Width`,
  `Height`, and declaring a child means declaring its parent's geometry too.
- The smallest effective size is 1 × 1. Community skins use 1 × 1 to make an element
  practically invisible (MUI, XSD); there is no "hide" attribute.

<a id="s4-7"></a>
### 4.7 Integer attributes (Confirmed (BIN))

Every integer attribute is converted by the C library's "string to long" routine with
automatic base, applied to the UTF-16 text:

1. Skip leading white space.
2. Accept an optional `+` or `-`.
3. Choose the base from the prefix: `0x` or `0X` → hexadecimal; any other leading `0` →
   **octal**; otherwise decimal.
4. Read as many digits of that base as follow; stop at the first other character and ignore
   the rest.
5. No digit at all gives 0. A value outside the signed 32-bit range saturates to
   −2,147,483,648 or 2,147,483,647.

The result is a signed 32-bit integer; negative positions and sizes are stored as given.

| Text | Value |
|---|---|
| `12` | 12 |
| `  -4` | −4 |
| `+7` | 7 |
| `0x1F` | 31 |
| `010` | 8 (octal) |
| `09` | 0 (octal, `9` stops the scan) |
| `12px` | 12 |
| `abc` | 0 |
| `99999999999` | 2,147,483,647 |
| *(attribute missing or empty)* | 0, and for `ScaleOriginX/Y` "not given" |

<a id="s4-8"></a>
### 4.8 Accepting a skin (Confirmed (BIN))

After all children of the root have been read, the definition is a skin only if it holds
**at least one** stored mapping or **at least one** registered panel override. Otherwise it
is rejected and not listed, whatever its `SkinName` says.

<a id="s4-9"></a>
### 4.9 Diagnostics

The original reports every rejection above only in its own log file, never on screen (BIN;
DOC and MUI describe the silent omission). The rejection points are: unreadable or
ill-formed XML; no document root; an unresolvable art-asset name; a repeated mapping; an
unresolvable element name; a repeated top-level element ID; nothing usable in the file.
Tessera reports each of them through `tracing` and in its skin list (§10, D9).

---

<a id="s5"></a>
## 5. Resolving names against the user's data

<a id="s5-1"></a>
### 5.1 Which tables (Confirmed)

| Name kind | DID map path | Mapper in this install | Labels |
|---|---|---|---|
| Art-asset names (`ArtAssetID`) | root map label `EMAPPER` → entry labelled `ArtAssetID` (key 0x25) | 0x230000EC, base 0x230001B5 | 5,623 labels with keys 0x10000001–0x100015F7; the base adds keys 0 and 1 |
| Element names (`Element` `ID`) | root map label `EMAPPER` → entry labelled `UIElementID` (key 0x38) | 0x230000D1, base 0x2300013B | 19,893 labels with keys 0x10000001–0x10004DB8; the base adds 44 keys from 0 |

The original resolves the two mappers through the `EMAPPER` map by key (0x25, 0x38); the
labels and keys agree in this install (OBS). Readers resolve by label and must not hard-code
the mapper DIDs (`records.md` §8.2).

<a id="s5-2"></a>
### 5.2 Lookup rule (Confirmed (BIN))

To turn a name into a value:

1. Convert the UTF-16 attribute text to narrow text with the system code page, as in §4.4
   (all labels are ASCII, so a name with other characters never resolves).
2. Search the mapper's **name** list (`records.md` §7.1 field 4; display texts are not used)
   for a label equal to the text **ignoring ASCII case**.
3. If not found and the mapper has a base, search the base, and so on up the chain.
4. If found, the label's key is the value. Otherwise the name does not resolve.

Labels may contain characters other than letters, digits and `_`, and are matched like any
other text (OBS): 46 art-asset labels do (42 of them end in an image-file extension, the rest
contain a space or a hyphen) and 33 element labels do (18 end in an image-file extension;
others contain spaces, slashes, backslashes, brackets, punctuation or even a line break). No
label contains a character outside ASCII. The longest labels are 63 and 116 characters.

Values 0 are possible results: the base mappers label key 0 (`ArtAssetID` base: an "invalid"
label; `UIElementID` base: an "undefined" label). An art-asset value 0 is stored but never
used (§6.3); an element ID value 0 makes the override inert (§4.6 step 5).

<a id="s5-3"></a>
### 5.3 Census (Confirmed, 2026-10-05)

- No two labels of the art-asset chain (5,625) or of the element chain (19,937) are equal
  under ASCII case folding, so the case-insensitive lookup is unambiguous.
- Every element ID stored in the 302 layout records (16,421 elements) is a key of the
  element chain (LAY). There are 13,035 distinct element ID values; 1,450 occur on more than
  one element and 450 occur in more than one layout record; 3,144 layout elements are
  top-level elements of their record (3,122 distinct IDs, 21 of them top-level in more than
  one record). Matching is by ID value only (§8.3), so one override can reach elements of
  several layouts.
- Published name lists resolve as SK32 states; names that fail are misspellings or removed
  assets, which the original rejects (§4.4, §4.6).

---

<a id="s6"></a>
## 6. Art replacement

<a id="s6-1"></a>
### 6.1 The base table (Confirmed)

| Step | Value in this install |
|---|---|
| Root DID map label `UNIQUEDB` (key 0x0F) | DID map 0x28000002 |
| Entry labelled `BaseSkinMappings` (key 0x23) | property set 0x78000011 (prefix 0x78, DB object type "DB property set") |
| Top-level property `UISkin_MappingArray` (PID 0x280, array) | 4,742 elements |
| Each element: `UISkin_MappingStruct` (PID 0x9C, struct) with exactly two fields | `UISkin_UIArtAssetID` (PID 0x2CC, enum, mapper 0x230000EC) and `UISkin_DataID` (PID 0x4F7, DID reference, DB type 5 = render surface) |

The record decodes with `property-system.md` §5 and has no other top-level property (OBS).

How the original loads it (Confirmed (BIN)): it walks the array in stored order and, for
every element that is a struct whose DID field is a DID reference other than the "invalid
DID" value and whose art-asset field is an enum value other than 0, stores
**DID → art-asset value** in a table **keyed by DID**. A repeated DID is logged and skipped
(first wins). Elements of any other shape are skipped.

Census (OBS): the 4,742 art-asset values are distinct and the 4,742 DIDs are distinct, so the
table is one-to-one. Every DID has prefix 0x41 and exists in the archive set: 3,720 in
`client_surface.dat`, 675 in `client_surface_aux_1.datx`, 344 in `client_surface_aux_2.datx`,
3 in `client_local_English.dat`.

<a id="s6-2"></a>
### 6.2 Game skins (Confirmed; activation Confirmed (BIN))

A game skin is a second art layer defined in data and switched on by gameplay, not by the
user. This install has one: the `UNIQUEDB` entry labelled `MonsterPlaySkin` (key 0x10000005)
→ property set 0x78000009, with

- `UISkin_SkinID` (PID 0x3A6, enum mapper 0x23000191) = 0x10000001, the only label of that
  mapper (its base 0x230001AD labels the kinds 0, 1 and 2: invalid, user-defined, base);
- `UISkin_MappingArray` with 378 `UISkin_MappingStruct` elements mapping **art-asset value →
  replacement DID** (the opposite key direction from §6.1): 322 replace the base DID with a
  different one, 4 repeat the base DID, and 52 name art-asset values that have no base entry
  and so can never apply (§6.3); 13 replacement DIDs serve more than one value; all 378 DIDs
  exist (377 in `client_surface.dat`, 1 in `client_surface_aux_1.datx`).

The original loads this record into its list of game skins when it sets up its game UI
(first mapping per art-asset value wins) and activates a game skin by its skin ID on request
from gameplay code; activating one game skin deactivates any other, and an unknown ID leaves
none active. The property `MonsterPlay_Session_Skin` holds values of the same enum (OBS),
which suggests where that ID comes from; the gameplay side is outside this spec. Tessera's
offline client never activates a game skin (§10, D13), but the precedence below includes it
so that a later milestone can.

<a id="s6-3"></a>
### 6.3 Image resolution (Confirmed (BIN))

Every time the client sets an element's **base image** from a render-surface DID *D* through
its base-image setter (which paths use that setter is listed in §6.8; it includes the images
that the layout data gives an element and its states, and the refresh of every element after a
skin change, §6.6), the original decides what to draw:

1. If no user skin and no game skin is active, draw *D*.
2. Look *D* up in the base table (§6.1). If it is absent, draw *D*.
3. Let *A* be its art-asset value. For each active user skin, in activation order: if the
   skin has a mapping for *A*, draw the image file of that mapping (§6.4) and stop.
4. Otherwise, for each active game skin in activation order: if it maps *A* to a DID *D′*,
   draw *D′* and stop.
5. Otherwise draw *D*.

Whichever source is chosen, if it cannot be loaded the element shows the placeholder of
§6.5: a missing or unreadable skin file does **not** fall back to *D*.

Consequences:

- A user skin overrides the game skin for the same art-asset value.
- Only images whose DID reaches this path **and** is in the base table are skinnable. By
  media-item kind (`ui-layout.md` §6):
  - **Image** items (kind 4) with a render-surface DID set the element's base image through
    this path (`ui-layout.md` §6.2 and §9.8). They name all 4,742 base-table DIDs (§6.7).
  - **Image** items with DID 0 and a file path (11 in this install, all JPEG) load the file
    directly and never pass through this path (`ui-layout.md` §6.2); they cannot be skinned.
  - **Animation** frames (kind 8) and **Cursor** images (kind 10) are never in the base table
    in this install (§6.7), so step 2 always draws them unchanged, whichever path they take.
  - **Coloured image** overlays (kind 9): 6 of their 16 distinct DIDs are in the base table
    (12 references), but the overlay setter never passes through this path (SK39), so these
    overlays always show the base art. None of them is in the M5 panels (SK48).
  - Images that game code or plugins set without the base-image setter, and art drawn by other
    subsystems (for example 3D viewports), are outside this path (§6.8).
- The image file replaces *D* wherever *D* is drawn as an element image, in every panel.

<a id="s6-4"></a>
### 6.4 From `FileName` to a path (Confirmed (BIN))

1. Start with the skin folder (§3.3). It always ends with `\`, `/` or `:` (the character at
   which the definition file's path was cut).
2. Join `FileName` (already narrowed, §4.4):
   - if `FileName` is at least two characters long and either its second character is `:`
     (a drive letter) or its first two characters are both `\` or `/`, `FileName` replaces
     the folder entirely (an absolute path);
   - else, if the folder ends with `\` or `/` and `FileName` starts with `\` or `/`, drop the
     folder's last character;
   - else, if the folder ends with none of `\`, `/`, `:` and `FileName` starts with neither
     `\` nor `/`, insert `/` (this cannot happen for a skin folder);
   - then append `FileName`.
3. Replace every `\` with `/`.
4. Before opening, the whole path is converted to lower case (ASCII); the lower-cased path is
   the one passed to the file system and the one used as the cache key (§6.6). On the
   original's case-insensitive file system this is invisible; it is why skins in the wild
   never depend on letter case (§10, D2).
5. The image readers for TGA and IFF copy the path into a 256-byte buffer: a path longer than
   255 bytes is cut to 255 and will not be found.

Relative paths may climb out of the skin folder with `..` (they are joined as text). The
publisher's guide describes `FileName` as any local file (DOC); in practice skins use paths
relative to their folder.

<a id="s6-5"></a>
### 6.5 The placeholder image (Confirmed (BIN))

When an image source (a skin file, a replacement DID or a base DID) cannot be loaded, the
original creates a 32 × 32 image:

- every pixel has the colour (red, green, blue, alpha) = (0.3, 0.3, 0.4, 1.0);
- then the pixels with x in 29–31 (all rows) and the pixels with y in 29–31 (all columns),
  counting from the top-left pixel (0, 0), are set to (1.0, 1.0, 1.0, 1.0).

The image has 8 bits per channel. The client converts each float channel to its stored byte
by multiplying by 255 in 32-bit floating point and **truncating** toward zero (not rounding).
The stored texels are therefore:

| Pixels | R | G | B | A |
|---|---|---|---|---|
| x ≤ 28 and y ≤ 28 (slate) | 76 | 76 | 102 | 255 |
| x ≥ 29 or y ≥ 29 (white) | 255 | 255 | 255 | 255 |

(0.3 × 255 = 76.5 truncates to 76; 0.4 × 255 = 102.0.) The client creates the image in the
32-bit colour format its renderer uses for UI images; its fill handles the A8R8G8B8,
X8R8G8B8, A8B8G8R8 and X8B8G8R8 layouts with the same channel values, so only the channel
order in memory depends on the format. No sRGB or gamma conversion is applied to these values
when they are stored.

The result is a dark slate square with a 3-pixel white strip along its right and bottom
edges; repeated across an element it forms a grid, which is the "grid pattern" skin authors
report for a missing file (RES). Tessera uses the same image, with the texel values above as
8-bit RGBA, for the same cases (§10, D16).

<a id="s6-6"></a>
### 6.6 Caching and re-reading (Confirmed (BIN))

- Images loaded from skin files are cached by their lower-cased path.
- Whenever the active user skin or game skin changes (§9.3), the original empties this cache
  and then asks every UI element to resolve its image again. So an image file that was edited
  on disk is read anew the next time any skin is switched, without a restart. The definition
  file is never re-read (§9.4).
- **The refresh** (Confirmed (BIN), SK40) walks the element tree from the UI root (every
  element currently attached below it, depth first). For each element whose base image was
  created from a render-surface DID, it sets the base image again from **that original DID**
  through §6.3; an image remembers the DID it was requested with, not the replacement it
  shows, so switching skins back and forth always starts from the base DID. The element's
  draw mode is kept. A base image that was created from a file path alone (an Image item with
  DID 0 and a path, a plugin background given as a file) is left as it is. The refresh then
  marks the element for redraw. It does not touch the overlay layer, text, states, running
  media lists, positions or sizes. Images that a state would set are not pre-loaded; they pass
  through §6.3 when the element next enters that state (§6.9).

<a id="s6-7"></a>
### 6.7 How much of the UI is skinnable (Confirmed, LAY)

The 302 layout records reference 5,122 distinct render-surface DIDs through their Image,
Coloured-image and Cursor media items (10,769 references: 10,560 Image, 29 Coloured image,
180 Cursor; `ui-layout.md` §6.2, §6.3, §6.5). 4,742 of those DIDs are in the base table
(9,877 references), so a skin can replace 92.6 % of the distinct images these items use;
the other 380 cannot be replaced. Conversely every base-table DID is named by at least one
Image item.

By kind (distinct DIDs, of which in the base table): Image 5,105 (4,742); Coloured image
16 (6); Cursor 7 (0). Animation items (`ui-layout.md` §6.4) add 2,252 frame references to
1,066 distinct DIDs, 1,065 of them used only as frames; **none** is in the base table, so
animated UI art cannot be skinned in this install. The four image kinds (Image, Coloured
image, Cursor, Animation frames) together name 6,187 distinct render-surface DIDs (prefix
0x41): the 5,122 above plus the 1,065 frame-only DIDs. Sound items (`ui-layout.md` §6.9,
kind 11) name a further 10 distinct sound-info DIDs (prefix 0x2A, 692 references); they are
not images and a skin cannot replace them, so all media kinds together name 6,197 distinct
DIDs.

<a id="s6-8"></a>
### 6.8 Which image paths a skin reaches (Confirmed (BIN), SK38–SK42)

An element has a base-image layer and an overlay layer (`ui-layout.md` §9.6). The substitution
of §6.3 lives in one routine: the element's **base-image setter** (set the base image from a
DID with a draw mode). Whether an image can be skinned therefore depends only on whether the
code that shows it goes through that setter, and on whether its DID is in the base table.

| How the image reaches the element | Substituted when set | Re-set by the refresh (§6.6) |
|---|---|---|
| Image item (kind 4) with a render-surface DID, in an element's own block or in a state | yes | yes |
| Image item with DID 0 and a file path (`ui-layout.md` §6.2) | no | no |
| Image items that game code installs at run time (it gives the element a media list holding one Image item for a DID and starts it). A meter's child images (`UICore_Meter_child_image`) take this path; game code has 190 call sites (in 134 routines) that install images this way | yes | yes |
| Animation frames (kind 8): each frame change goes through the setter | yes (no frame DID is in the base table, SK15) | yes (current frame) |
| Meter frame arrays (`UICore_Meter_frame_array`): the meter shows a frame through the setter | yes | yes |
| Coloured image (kind 9) overlay, with or without an image | **no** | no (overlays are not refreshed) |
| Hit mask (kind 7) | no (not drawn) | no |
| Cursor (kind 10) | not traced: the item stores the DID and hot spot for the cursor code; no Cursor DID is in the base table (SK15) | — |
| Plugin `SetBackground` with a DID, or with a `Turbine.UI.Graphic` made from a DID (`lua-plugin-api.md` §13.8) | **no** | **yes**: after any skin switch the background is re-set through §6.3 (SK41) |
| Plugin `SetBackground` with a file path, or with a `Graphic` made from a file | no | no |
| Game displays that build an image object directly and place it as the base image (traced: virtue and trait displays, quest-dispenser icons, plugin-manager list entries) | no | yes, when that image was made from a DID |

Notes:

- **Plugin game-styled controls** (`lua-plugin-api.md` §14.2: `Turbine.UI.Lotro.Window`,
  `Button`, `CheckBox`, `TextBox`, `ScrollBar` and the gold variants) are created like any
  other UI element, from top-level elements of the layout that the `UILAYOUT` map labels
  `luaplugins` (in this install 0x22000826, map key 0x100000C8; the 13 top-level elements
  include `LotroWindow_Container`, `LotroWindowGold_Container`, `LotroButton`,
  `LotroButtonGold`, `LotroCheckBox`, `LotroTextBox`, `LotroScrollBar_Vertical` and
  `LotroScrollBar_Horizontal`). Their Image items pass through §6.3, and every image DID in
  their resolved trees is in the base table (OBS). So a user skin re-skins plugin windows and
  buttons together with the native UI. If those controls are built through §8.2, as every
  layout element is (SK34; not traced for these controls), a top-level panel override whose ID
  is one of these labels also changes the design rectangle of every plugin control built from
  it; plugins normally set their own sizes after creation (`lua-plugin-api.md` §13), which
  then replaces it.
- **Plugin backgrounds** set by DID are a client quirk: unskinned when set, skinned after the
  next skin switch (SK41). Plugins normally set backgrounds after the in-game UI (and with it
  the user skin) is active (§9.3), so in practice a background with a base-table DID shows the
  base art until the player switches skins once.
- Which images the game widgets of the M5 panels set directly instead of through the setter
  (for example a class icon taken from a game property) is not fully traced: OPEN-9 (b).

<a id="s6-9"></a>
### 6.9 Art per element and per state (Confirmed (BIN), SK43)

- **The unit of replacement is a render surface.** A mapping replaces one art-asset value,
  which the base table ties to exactly one render-surface DID (§6.1). Wherever that DID reaches
  the base-image setter (§6.8), in any element, state, panel or layout, the skin's file is shown
  instead. A skin cannot give two elements that show the same DID different art, and cannot
  re-skin one element's image without re-skinning every other use of the same DID. Art is often
  shared: of the 419 distinct images in the resolved trees of the six M5 panels of §8.6, 265
  are also shown by the resolved tree of at least one other top-level element of some layout,
  and 38 are shown by more than one element of the same panel (OBS, all 3,144 top-level
  elements resolved).
- **Own block and states.** An element's own media list runs once when the element finishes
  creation (`ui-layout.md` §9.2 step 8); its Image items set the initial base image through
  §6.3 with the skin active at that moment. Each time the element enters a state
  (`ui-layout.md` §9.4), that state's media list runs from its first item, and each Image item
  in it sets the base image through §6.3 at that moment. So a button's normal, rollover,
  pressed and ghosted images (`ui-layout.md` §9.7) are replaced independently, each by the
  mapping of its own art asset, and an image that only a state uses is loaded only when that
  state is first entered. In the six M5 panels 629 of the 836 Image items with a DID sit in
  states and 207 in elements' own blocks (OBS).
- **Persistence.** Media effects are not undone on a state change (`ui-layout.md` §9.4): a
  substituted image stays until another Image or Animation item, or a refresh (§6.6),
  replaces it.
- **Draw mode.** The draw mode is the Image item's own; a skin cannot change it. A skin image
  is drawn with the same blend as the art it replaces (all Image items in the M5 panels use
  Normal or Alphablend, SK48; their exact blend equations are `ui-layout.md` §9.6).
- **Overlay layer.** Coloured-image overlays drawn on top of a substituted image are unchanged
  (§6.8), so a tint or a colour fill that the data puts over an image still applies to the
  skin's image.

<a id="s6-10"></a>
### 6.10 Image size rules (Confirmed (BIN), SK49)

1. **Any size is accepted.** No check of the file's size against the replaced surface's size
   was found on this path (the substitution, the base-image setter and the file readers of §7,
   which take width and height from the file's own header), and nothing requires powers of two
   on it. The only size rules are the format's own
   (§7: TGA width and height from the header; DDS DXT1–DXT5 need a computed size above 0, and
   Tessera requires multiples of 4, D18) and Tessera's hostile-input limits (D4: width and
   height 1–16,384, file at most 64 MiB).
2. **The image keeps its own size.** It is never scaled to the replaced surface or to the
   element. The element's rectangle does not change with the image; only a panel override
   (§8) changes geometry.
3. **Drawing** follows `ui-layout.md` §9.6 for every base image: the image is drawn at its
   native pixel size from the element's top-left corner, repeated to the right and downwards
   to cover the element's rectangle, and clipped to the rectangle:

   | Skin image compared with the element (per axis) | Result on that axis |
   |---|---|
   | Same size | drawn once, pixel for pixel |
   | Smaller | repeats from the left (top) edge; the last repeat is cut at the right (bottom) edge |
   | Larger | only the left (top) part that fits is shown |

4. **Matching the original look.** To keep the original layout, a skin image should have
   the same width and height as the surface it replaces. Read that size from the replaced
   surface's own header at run time (`records.md` §5); never hard-code it. Many original
   surfaces are smaller than their element on purpose (tiled middle pieces of frames,
   `ui-layout.md` §9.6: 3,000 of 10,350 Image items on typed elements), and their
   replacements tile the same way, so they must tile seamlessly.
5. **Transparency** comes from the file: 32-bit TGA and DDS formats with alpha carry it;
   24-bit TGA, 15-bit TGA, JPEG and DDS formats without alpha are opaque; an 8-bit TGA is an
   alpha-only image (§7.2, OPEN-4). With draw mode Alphablend, opaque files cover everything
   below them.

<a id="s6-11"></a>
### 6.11 Failure behaviour (summary, SK50)

| Condition | Original | Tessera |
|---|---|---|
| No skins root, or no definition file below it | Only the "no skin" entry is offered (§9.1) | Same |
| Definition not well-formed, no root, nothing usable (§4.1, §4.8) | Not listed; a line in the client's own log | Same; reported through `tracing` and in the options entry (D9) |
| A `Mapping` or `PanelFile` rejected (§4.4–§4.6) | That entry ignored, the rest of the skin used | Same (D8 for repeated top-level IDs) |
| The chosen skin no longer exists at activation (renamed, removed before start) | No user skin active (§9.3) | Same |
| Mapped file missing, unreadable, empty, of an unknown extension, of an unsupported TGA depth or DDS format, or truncated | The **placeholder** of §6.5 for every element that shows the replaced DID, until the next skin switch re-reads it (§6.6); the base art is **not** used | Same (D16); Tessera logs the path once per activation |
| Mapped file exceeds Tessera's limits (D4) or is a DXT image with sides not multiples of 4 (D18) | Shown, or undefined (§7.4) | Placeholder |
| A file is added or repaired while the skin is active | Seen at the latest at the next skin switch (cache emptied, §6.6); whether a failed load is retried before that was not traced | Tessera remembers a failed load like a successful one and retries only after the next switch |
| A file is deleted while the skin is active | Images already loaded stay, also for elements that ask for them later (cache, §6.6), until the next switch; a file never loaded before gives the placeholder | Same |
| Replacement DID of a game skin missing (§6.2) | Placeholder | Same (game skins not activated offline, D13) |
| Override for an element ID that no built element has, or a nesting that does not match the layout tree (§8.3) | Ignored without a message | Same, with a debug-level log line |
| Override on an element that carries `UISkin_CannotBe…` | Only the unprotected fields change (§8.4) | Same |

A failing image never stops the panel from being built and never affects other mappings.

---

<a id="s7"></a>
## 7. Image file formats

<a id="s7-1"></a>
### 7.1 Dispatch (Confirmed (BIN))

The original decides the decoder from the **lower-cased full path** (§6.4), by substring
search, in this order:

| Test on the lower-cased path | Decoder |
|---|---|
| contains `.dds` | DDS (§7.4) |
| otherwise contains `.jpg` | JPEG (§7.3) |
| otherwise contains `.tga` | TGA (§7.2) |
| otherwise contains `.iff` | IFF (§7.5) |
| otherwise | none: the load fails → placeholder (§6.5) |

Because the test is a substring search over the whole path, a folder name such as
`my.jpg.art/` or a file name such as `bar.dds.tga` sends the file to the first matching
decoder. `.jpeg`, `.png`, `.bmp` and other extensions are not decoded. The published guide and
schema name TGA and JPEG only (DOC, XSD); DDS and IFF are additionally accepted by this build.

Images keep their **own** pixel dimensions: the original does not scale a file image to the
size of the image it replaces. The element then draws the substituted image like any other
base image (`ui-layout.md` §9.6): at the image's native pixel size, anchored at the element's
top-left corner, repeated (tiled) to cover the element's rectangle and clipped to it, never
stretched. So an image larger than the element shows its top-left part once, which is what
community authors report (RES), and an image smaller than the element repeats. (This closes
OPEN-5.)

<a id="s7-2"></a>
### 7.2 TGA (Confirmed (BIN))

The file is read fully. The 18-byte header is little-endian:

| Offset | Size | Field | Use |
|---|---|---|---|
| 0 | 1 | ID length | that many bytes after the header are skipped |
| 1 | 1 | colour-map type | non-zero: a colour map follows the ID field |
| 2 | 1 | image type | 9, 10, 11 = run-length encoded; **any other value** is read as uncompressed pixels (the usual values are 1, 2, 3: colour-mapped, true-colour, grey); the type is not otherwise checked |
| 3 | 2 | colour-map first entry | — |
| 5 | 2 | colour-map length | entries |
| 7 | 1 | colour-map entry size | bits |
| 8 | 2 | x origin | ignored |
| 10 | 2 | y origin | ignored |
| 12 | 2 | width | pixels |
| 14 | 2 | height | pixels |
| 16 | 1 | pixel depth | bits per pixel (or per colour-map index) |
| 17 | 1 | image descriptor | bit 5 = top-left origin; bit 4 (right-to-left) is ignored |

Accepted pixel depths and the image format they produce (formats named by their usual 3D-API
names):

| Depth | Format | Alpha |
|---|---|---|
| 32 | 8-bit B, G, R, A in file order → A8R8G8B8 | from the file |
| 24 | 8-bit B, G, R → R8G8B8 | opaque |
| 16 | 1-5-5-5 → A1R5G5B5 | 1-bit, from the top bit |
| 15 | 5-5-5 → X1R5G5B5 | opaque |
| 8 | single channel → A8 (alpha only) | the 8-bit value |
| any other | load fails → placeholder | — |

Decoding:

1. Rows are produced in file order. File row *i* (0-based) is written to image row *i* when
   descriptor bit 5 is set, else to row *height* − 1 − *i* (bottom-up, the TGA default).
2. Within a row, pixels are written left to right.
3. Uncompressed types read *width* pixels per row.
4. Run-length types read packets until at least *width* pixels have been written **for this
   row**: a header byte with the top bit set gives (low 7 bits + 1) copies of the one pixel
   value that follows; otherwise (header + 1) literal pixels follow. A packet is never carried
   into the next row: a packet that runs past the end of a row writes its excess past the row
   and the next row starts with the next packet in the file. Files whose packets cross
   scanlines therefore decode wrongly in the original; files that look right in the original
   have no crossing packets. Tessera decodes packets as one continuous stream (§10, D14),
   which gives identical results for every such file.
5. When a colour map is present it is read first; pixel values pass through it before format
   conversion.

The appearance of 8-bit (greyscale or colour-mapped) TGA files, which become alpha-only
images, is OPEN-4. The publisher's guidance is 32-bit TGA with alpha (DOC, RES), which takes
the A8R8G8B8 path.

<a id="s7-3"></a>
### 7.3 JPEG (Confirmed (BIN))

The whole file is read into memory (a zero-length file fails) and decoded by an IJG libjpeg
of the 2010 release family that is linked into the client; it handles baseline, progressive
and arithmetic-coded JPEG. JPEG images have no alpha channel and are drawn opaque. The
published guide notes that JPEG works as well as TGA (RES).

<a id="s7-4"></a>
### 7.4 DDS (Confirmed (BIN))

The file starts with a 4-byte magic followed by the standard 124-byte DDS header; pixel data
starts at file offset 128. All fields are little-endian. Offsets below are **file offsets,
counted from the first byte of the magic**; the "header offset" column gives the same position
counted from the end of the magic, as DDS documentation usually does.

| File offset | Header offset | Size | Type | Field | Use by the reader |
|---|---|---|---|---|---|
| 0 | — | 4 | bytes | magic | must be `DDS ` (0x20534444 as a little-endian u32), else the load fails |
| 4 | 0 | 4 | u32 | header size | read, **not checked** (normally 124) |
| 8 | 4 | 4 | u32 | header flags | read, not used |
| 12 | 8 | 4 | u32 | height | image height, pixels |
| 16 | 12 | 4 | u32 | width | image width, pixels |
| 20 | 16 | 4 | u32 | pitch or linear size | checked only for FourCC formats (step 4); otherwise ignored |
| 24 | 20 | 4 | u32 | depth | ignored |
| 28 | 24 | 4 | u32 | mip-map count | ignored |
| 32 | 28 | 44 | bytes | reserved | ignored |
| 76 | 72 | 4 | u32 | pixel-format size | ignored (normally 32) |
| 80 | 76 | 4 | u32 | pixel-format flags | bits 0x1 or 0x2 = alpha, 0x4 = FourCC, 0x40 = RGB; other bits ignored |
| 84 | 80 | 4 | u32 | FourCC | four ASCII bytes, e.g. `DXT1` (0x31545844) |
| 88 | 84 | 4 | u32 | RGB bit count | bits per pixel; **only its low byte is used** |
| 92 | 88 | 4 | u32 | red mask | compared exactly (table below) |
| 96 | 92 | 4 | u32 | green mask | compared exactly |
| 100 | 96 | 4 | u32 | blue mask | compared exactly |
| 104 | 100 | 4 | u32 | alpha mask | compared exactly where the table names one |
| 108 | 104 | 16 | 4 × u32 | capability words 1–4 | ignored |
| 124 | 120 | 4 | u32 | reserved | ignored |
| 128 | — | *size* | bytes | top-level image data | read as one block (step 5) |

Steps:

1. Check the magic; read the header.
2. Choose the pixel format from the flags, FourCC, bit count and masks (table below). Any
   other combination fails. For a FourCC format the bit count is replaced by the format's:
   DXT1 4, DXT2–DXT5 8.
3. **Size** of the top-level image in bytes = (bit count × height × width) ÷ 8, computed in
   unsigned 32-bit arithmetic (the product wraps modulo 2³² before the division, which
   rounds down).
4. **Linear-size check** (FourCC formats only): if the field at file offset 20 is non-zero
   and differs from *size*, the load fails. For uncompressed formats the field is never
   checked.
5. **Read** exactly *size* bytes from file offset 128 in one read; if fewer are available, or
   *size* is 0, the load fails. Bytes after them (mip levels, cube faces, volume slices) are
   ignored. The pitch field is not consulted for uncompressed formats, so rows are taken as
   tightly packed (width × bit count ÷ 8 bytes each).

DXT sizes whose width or height is **not a multiple of 4**: the formula of step 3 does not
round up to whole 4 × 4 blocks, so it gives less than the real data size. Example: a 6 × 6
DXT1 image computes 4 × 6 × 6 ÷ 8 = 18 bytes, while its block data is 2 × 2 blocks × 8 =
32 bytes (DXT5: 36 against 64). The original then either fails (when the file states the
real linear size, which differs from the computed one) or, when the field is 0, reads only
the computed number of bytes and hands a buffer too short for the image's blocks to the
texture code (behaviour undefined; not traced). Very small DXT images compute a size of 0
(a 1 × 1 DXT1 image) and fail. For widths and heights that are multiples of 4 the formula
equals the block size exactly. Tessera rejects the non-multiple-of-4 case (§10, D18).

Accepted pixel formats (any other fails):

| Pixel-format flags | Bits | Masks R / G / B / A | Format |
|---|---|---|---|
| FourCC `DXT1` | — | — | DXT1 |
| FourCC `DXT2`, `DXT3`, `DXT4`, `DXT5` | — | — | that format |
| RGB, no alpha | 24 | FF0000 / FF00 / FF / – | R8G8B8 |
| RGB, no alpha | 32 | FF0000 / FF00 / FF / – | X8R8G8B8 |
| RGB, no alpha | 16 | F800 / 7E0 / 1F / – | R5G6B5 |
| RGB, no alpha | 16 | 7C00 / 3E0 / 1F / – | X1R5G5B5 |
| RGB, no alpha | 16 | F00 / F0 / F / – | X4R4G4B4 |
| RGB, no alpha | 32 | FF / FF00 / FF0000 / – | X8B8G8R8 |
| RGB + alpha | 32 | FF0000 / FF00 / FF / FF000000 | A8R8G8B8 |
| RGB + alpha | 16 | 7C00 / 3E0 / 1F / 8000 | A1R5G5B5 |
| RGB + alpha | 16 | F00 / F0 / F / F000 | A4R4G4B4 |
| RGB + alpha | 32 | 3FF / FFC00 / 3FF00000 / C0000000 | A2B10G10R10 |
| RGB + alpha | 32 | FF / FF00 / FF0000 / FF000000 | A8B8G8R8 |
| RGB + alpha | 32 | 3FF00000 / FFC00 / 3FF / C0000000 | A2R10G10B10 |
| alpha only | 8 | – / – / – / FF | A8 |
| RGB + alpha | 32 | FF000000 / FF0000 / FF00 / FF | an engine-specific R8G8B8A8 format |
| RGB, no alpha | 24 | 0 / FF / FF00 / FF0000 | an engine-specific B8G8R8 format |

"RGB" is the DDS `DDPF_RGB` flag (0x40); "alpha" is `DDPF_ALPHAPIXELS` (0x1) or `DDPF_ALPHA`
(0x2); "FourCC" is `DDPF_FOURCC` (0x4). The FourCC flag is tested first: when it is set, the
FourCC alone decides (an unknown FourCC fails) and the RGB and alpha flags, bit count and masks
are not looked at. "Alpha only" means the alpha flag without the RGB flag. Masks not shown
("–") are not compared. No two rows can match the same header, so their order does not
matter.

<a id="s7-5"></a>
### 7.5 IFF (Confirmed (BIN))

A path that reaches the IFF reader is read as an IFF image with an ILBM-style bitmap header:
the header must declare **24 bit-planes** (otherwise the load fails), width and height come
from it, the image becomes R8G8B8, the header's compression byte selects plain or compressed
rows, and of the chunks that follow only `BODY` is decoded; every other chunk is skipped.
Neither the publisher's guide nor the community tools mention IFF; Tessera does not support it
in M5 (§10, D15).

---

<a id="s8"></a>
## 8. Panel overrides (geometry)

<a id="s8-1"></a>
### 8.1 What an element override carries (Confirmed (BIN))

| Field | From | Default |
|---|---|---|
| Element ID value | `ID` (§5) | 0 (inert) |
| X, Y | `X`, `Y` (§4.7) | 0 |
| Width, Height | `Width`, `Height` | 0 |
| Scaler origin X, Y | `ScaleOriginX`, `ScaleOriginY` | 0, with "has scaler origin" set if either attribute is non-empty |
| Detach | `Detach` ≠ 0 | false (unused, §8.5) |
| Children | nested `Element`s, keyed by element ID value (first wins) | none |

A skin holds its panel overrides in document order, a table **top-level element ID value →
override** across all its panels (§4.5 step 3), and the set of every element ID value used
anywhere in its overrides.

<a id="s8-2"></a>
### 8.2 When overrides apply (Confirmed (BIN))

Overrides change the **descriptor an element is built from**, not live elements and not the
loaded layout data. Whenever the original builds a UI element from layout data (the element
a build starts from, such as a panel root, and then each of its descendants in turn;
`ui-layout.md` §9.1, §9.2), it:

1. **resolves** the element's descriptor (`ui-layout.md` §7, base inheritance) into a fresh
   copy made for this build; a descriptor without a base reference is copied as it is. If
   resolution fails, the element is not built and no override is looked for;
2. applies the active user skin's matching override (§8.3) to that **resolved copy** (§8.4);
3. creates the element from the copy (`ui-layout.md` §9.2 steps 3–8), and discards the copy
   when the element's subtree has been built (the element keeps its own copy, `ui-layout.md`
   §9.2 step 4).

Consequences:

- An override sees, and can be blocked by, everything the element inherits: the protection
  properties of §8.4 are looked up in the resolved property set, and the children reached by
  §8.4 step 5 include children inherited from the base chain (`ui-layout.md` §7.3 steps 6–7).
- A base element shared by many elements is never changed by an override of one of them,
  and the loaded layout record stays as stored. Building the same panel again after the skin
  was deactivated gives the unmodified geometry.
- The matching code is reached only from these two building paths (top-level element and
  child element). Nothing in the skin code changes the geometry of an element that already
  exists.

Rectangles in the override replace the resolved descriptor's own rectangle fields, so they are
interpreted exactly as layout coordinates are (`ui-layout.md` §5.3, §8.1): X and Y relative to
the parent element's top-left corner, in pixels. Because an applied override also sets every
edge mode to 0 (§8.4 step 3), the element's first run-time layout (`ui-layout.md` §8.4 steps 2
and 4) keeps exactly the override's position and size, whatever the parent's current size, and
for a panel root (`ui-layout.md` §8.3, first row) without any scaling from the layout's design
size to the screen: a root panel's X and Y are screen pixels from the top-left corner of the
screen root. Only the keep-inside-parent clamp (`ui-layout.md` §8.4 step 5, flag bit 1) can
still change it. An element protected by `UISkin_CannotBeDetached` keeps its edge modes and is
laid out from the override's rectangle as its design box. This agrees with community skins
being made per screen resolution (MUI). How UI scaling and positions the player saved
interact with it is OPEN-1 and OPEN-11.

A skin switched on at run time (§9.3) therefore applies its geometry only to elements built
afterwards. The switch itself only refreshes images (§6.6): neither the skin code nor the
option handler that deactivates the old skin and activates the new one destroys, rebuilds or
re-lays out a panel (Confirmed (BIN), SK40). An open panel keeps its geometry until it is
built again, for example when it is closed and reopened (if its game code rebuilds it on
opening) or when the in-game UI is created at the next login. Switching a skin off likewise
leaves already built panels at the skinned geometry until they are rebuilt. (This closes
OPEN-2; Tessera follows it, D20.)

<a id="s8-3"></a>
### 8.3 Matching (Confirmed (BIN))

Let *E* be the resolved descriptor of the element about to be built (§8.2), with element ID
value *e*, and let *p₀, p₁, …, pₖ₋₁* be the element ID values of the elements enclosing it
**within the current build**, from the outermost (the element the build started from) inwards
(*k* = 0 for the root of a build). This list is the ID path that the client keeps while it
creates a tree (`ui-layout.md` §9.2 step 6). For each active user skin in activation order:

1. If the skin has no panel override, or *e* is not in the skin's set of used IDs (§8.1),
   skip the skin.
2. If *k* = 0: look *e* up in the skin's top-level table; if found, apply that override to
   *E* (§8.4).
3. If *k* > 0: for each panel override of the skin in document order, look *p₀* up among the
   panel's top-level overrides; from there follow the children by *p₁*, …, *pₖ₋₁* and finally
   *e*. The first panel in which every step is found supplies the override for *E*. (Top-level
   IDs are unique within a skin, so at most one panel can match.)

Applying an override to an element also applies, recursively, the override's children to the
element's **layout children** whose IDs match (§8.4 step 5). So the nesting of `Element`s in
the definition must mirror the layout's element tree level by level, starting at the element
the panel is built from; an intermediate level cannot be skipped. Matching is by ID value
only: the same top-level override reaches every build that starts from an element with that
ID, in any layout record (§5.3). The `PanelFile` `ID` plays no part (§4.5).

Published skins name the root element of each panel at the top level of a `PanelFile` (XSD's
dictionary lists 100 panels that way, with 3,759 element names that are all element labels,
SK32).

<a id="s8-4"></a>
### 8.4 Applying one override (Confirmed (BIN))

Given a descriptor *E* (the resolved copy of §8.2, or a child descriptor inside it reached by
step 5) and an override *R* with the same element ID value (otherwise nothing happens):

1. **Position.** Unless *E*'s property set holds `UISkin_CannotBeMoved` (PID 0x728) = true,
   set *E*'s X and Y to *R*'s.
2. **Size.** Unless *E* holds `UISkin_CannotBeResized` (PID 0x727) = true, set *E*'s Width
   and Height to *R*'s, **even when they are 0** (a container override without a size,
   §4.6).
3. **Edge modes.** Unless *E* holds `UISkin_CannotBeDetached` (PID 0x729) = true, set all
   four of *E*'s edge modes (`ui-layout.md` §4.4 fields 18–21) to **0**, the mode that keeps
   the element's current position (`ui-layout.md` §5.8, §8.4 step 4). The element then no
   longer follows its parent's edges when the parent is resized; this is what the community
   schema calls removing the element's attachment information (XSD).
4. **Scaler origin.** If *R* has "has scaler origin", set *E*'s integer properties
   `UIComponent_UIScaler_Origin_X` (PID 0x10001840) and `UIComponent_UIScaler_Origin_Y`
   (PID 0x10001841) to *R*'s two values (both are written, a missing one as 0). These are the
   properties the layouts use to name the point an element scales about when UI scaling is
   applied (218 layout elements carry them, OBS); their effect is given at the end of this
   section.
5. **Children.** If *R* has children: for each child descriptor of *E* (the children of the
   resolved descriptor, so children inherited or merged from the base chain are included,
   `ui-layout.md` §7.3 steps 6–7) whose element ID value is a key of *R*'s children, apply
   that child override to that child descriptor with this same procedure. Children of *E*
   without a matching key, and keys of *R* without a matching child, are ignored.

Nothing else is written. In particular the override does **not** change `has_position`
(`ui-layout.md` §5.3): the flag keeps the value it has in the resolved descriptor, also when
X and Y are replaced. It also leaves the z-level, the flag byte, the element ID, the type,
the default state, the states and the media lists unchanged. (This corrects `ui-layout.md`
§5.3 and §9.8, §13.)

A property absent from *E*, or present with a non-boolean type, counts as false in steps 1–3.
Each of the three protections is tested independently.

**Two applications per child.** A child is overridden twice with the same override: first by
step 5 while its parent's descriptor is being processed (on the child descriptor as it sits
inside the parent's resolved copy), then again when the child itself is built (§8.2, matched
through the ID path of §8.3, on the child's own resolved copy). The child's own resolution
keeps what the first application wrote, because a derived descriptor's rectangle and edge modes
replace its base's (`ui-layout.md` §7.2 step 1, §7.3 step 3) and its scaler properties replace
the base's (`ui-layout.md` §7.2 step 3). The result is therefore the same as one application, with one
difference: when the child descriptor has a base reference, the first application sees only
the child's own properties, so a protection property that the child inherits only from its base
does not stop the first application. No element of this install is affected: the 8 elements
that carry a protection property are typed (no base reference) and none of them is the base of
another element (SK36). Applying each override only once, to each built element's resolved
descriptor, would therefore give the same result for all data of this install; Tessera
nevertheless follows both applications (§10, D19).

Census of the protection properties in the layouts (OBS, LAY): `UISkin_CannotBeDetached` =
true on 8 elements in 7 records (two portrait elements of the toolbar layout and six 3D
viewport elements; all child elements, at depths 1–4); `UISkin_CannotBeMoved` and
`UISkin_CannotBeResized` on none; none with false; none in a state's property set.
`UISkin_UserElementID` (PID 0x724, a text value on 140 layout elements) is not consulted by the
skin system. 6,224 of the 16,421 layout elements have at least one non-zero edge mode (the
other 10,197 are (0, 0, 0, 0), `ui-layout.md` §5.8), so step 3 changes the run-time behaviour
of those 6,224 when they are overridden.

**Effect of the scaler origin (Confirmed (BIN), SK45; closes OPEN-11).** Elements of the
game's UI-scaler widget types (for example `ToolbarField`, §8.6, and the `UIScalerPanel` roots
such as `CharacterJournalPanel`) change their own scale at run time; how the new scale is
chosen, how the panel is resized and which of its boxes the screen, the hit test and the move
clamp use are `ui-behaviour.md` §9.2 and §9.2.1, of which the steps below are the origin and
clamp part. When the scale of such an element changes from *s*₀ to *s*₁:

1. Read *s*₀ from the element's property `UIComponent_UIScaler_Cached_Scale` (a 32-bit
   float; 1.0 when the property is absent).
2. Read the integer origin (*ox*, *oy*) from `UIComponent_UIScaler_Origin_X` and
   `UIComponent_UIScaler_Origin_Y` of the element (0 for an absent property). One caller can
   ask for the origin to be ignored, in which case (*ox*, *oy*) = (0, 0); which caller does
   so is OPEN-13.
3. With the element's current position (*x*, *y*), compute, converting each origin value to a
   32-bit float, multiplying in 32 bits and truncating toward zero:
   *x*′ = *x* + trunc(*ox*·*s*₀) − trunc(*ox*·*s*₁), and *y*′ = *y* + trunc(*oy*·*s*₀) −
   trunc(*oy*·*s*₁).
4. Clamp to the screen: *x*′ = min(*x*′, screen width − new width), then *x*′ = max(*x*′, 0);
   the same for *y*′ with the screen height and the new height. (The new width and height are
   the element's scaled size, limited by its `UICore_Element_min_width`, `…_max_width`,
   `…_min_height` and `…_max_height` properties multiplied by *s*₁ and truncated.)
5. Move the element to (*x*′, *y*′) and store *s*₁ in `UIComponent_UIScaler_Cached_Scale`.

So the origin is the point, in unscaled pixels from the element's top-left corner, that stays
where it is on the screen while the element grows or shrinks: (0, 0) keeps the top-left corner
fixed, (width, height) the bottom-right corner. A skin's `ScaleOriginX` and `ScaleOriginY`
(§8.4 step 4) set exactly these two values, so a skin that moves a scaled panel to the bottom
of the screen can keep it anchored there when the player changes the UI scale. Worked example:
§12.8.

<a id="s8-5"></a>
### 8.5 `Detach` (Confirmed (BIN))

The original parses `Detach` and stores "non-zero" but never reads the stored value in this
build: step 3 of §8.4 runs for every applied override whether `Detach` is 1, 0 or absent.
The community schema describes `Detach` as a switch, used only by the toolbar and off by
default, that removes attachment information (XSD); in this build that removal happens for
every applied override, so the attribute has no effect. Tessera follows this build: it
parses and ignores `Detach`. (OPEN-3, which asked for an owner observation to confirm this, is
closed: the BIN reading is direct, the stored value has no reader in this build. A future build
that reads it would show up as a difference in §8.4 step 3.)

<a id="s8-6"></a>
### 8.6 The M5 native panels: skin identifiers and art (Confirmed, SK47–SK48)

A skin addresses a panel by the element ID of the element the panel's build starts from
(§8.3), and the client builds a panel from a (layout, top-level element) pair
(`ui-layout.md` §9.1). Tessera's M5 native panels are the fifteen of the catalogue in
`ui-layout.md` §17.2 (D23); the table below gives, for seven of them, the pair and the
skin-relevant facts. Layouts are named by their label in the
`UILAYOUT` DID map (`ui-layout.md` §3.2) and elements by their label in the element mapper
(§5.1); both are resolved at run time. The DIDs are observations of this install. How the
original's game code opens each pair is given in `ui-layout.md` §9.1 and §17.2 (BIN, U31,
U32, U45): the bags, for example, are the six `containerdisplay` windows that a bag-window
class opens. The community skin dictionary (XSD), whose `PanelFile` top-level elements for the
minimap, toolbar, vitals and bags are exactly the roots below, agrees.

| M5 panel | Layout label (DID here) | Top-level element (the skin's top-level `Element` `ID`) | Root kind | Design rectangle; edge modes (L, T, R, B) | Elements built | Distinct images (all in the base table) | Image items in own blocks / in states |
|---|---|---|---|---|---|---|---|
| Minimap | `gameplayactivatables` (0x22000012) | `GamePlay_RadarBox` | based on `HiddenDragBox_Template` of layout `hiddendragbox` (0x22000029); its child `GamePlay_Radar` has the game type `Radar` | 822, 12, 224 × 280; (4, 5, 5, 4) | 51 | 58 | 11 / 99 |
| Chat | `chatwindow` (0x22000034) | `NewChatWindow1` (further windows `NewChatWindow2` … `NewChatWindow9`) | based on `NewPrimaryChatWindowTemplate` (the others on `NewChatWindowTemplate`), same layout | 5, 444, 313 × 199; (5, 4, 4, 5) | 19 | 12 | 5 / 11 |
| Toolbar | `toolbar` (0x22000033) | `ToolbarField` | typed, game type `UIScaler`; default state `toolbar_noncombat` | 0, 568, 1024 × 200; (1, 4, 1, 5) | 216 | 105 | 100 / 133 |
| Vitals (player) | `toolbar` (0x22000033) | `AvatarPanel` | based on `HiddenDragBox_Template` | 2, 2, 280 × 300; (5, 5, 4, 4) | 63 | 176 | 8 / 283 |
| Vitals (target) | `toolbar` (0x22000033) | `OpponentPanel` | based on `HiddenDragBox_Template` | 279, 2, 305 × 300; (5, 5, 4, 4) | 62 | 184 | 10 / 330 |
| Bags | `containerdisplay` (0x22000074) | `PlayerBag1_Template` … `PlayerBag6_Template` (one window per bag) | bag 1 based on `InventoryBag_Template`, bags 2–6 on `PlayerBagOver1_Template`, same layout; all resolve to the game type `UIScaler` | bag 1: 599, 164, 197 × 183; (0, 0, 0, 0) | 61 (bag 1) | 45 (bag 1) | 41 / 20 (bag 1) |
| Character panel | `character_journal` (0x2200003D) | `CharacterJournalPanel` | based on `Panel_Template` of layout 0x22000846 (a layout without a `UILAYOUT` entry) | 0, 0, 500 × 580; (0, 0, 0, 0) | 97 | 72 | 42 / 83 |

"Elements built" counts the elements a build creates from the resolved root, inherited
children included (`ui-layout.md` §7, §9.2); "distinct images" counts the render-surface DIDs
named by Image items anywhere in that tree, in own blocks and in states. Over the six panels
(minimap, chat, toolbar, player vitals, bag 1, character panel) there are 419 distinct images,
836 Image items with a DID (207 in own blocks, 629 in states; 85 with draw mode Normal, 751
Alphablend), no Coloured-image item with a base-table DID and no Animation frame in the base
table (SK48). Every one of these images can therefore be replaced by a user skin (§6.8).

Alternatives present in the data, for when a milestone needs them: the layout `radar`
(0x2200003A) has a typed `Radar` root `MinimapWindow` (752, 33, 224 × 260; edges (4, 5, 5, 4);
34 elements; 47 images, all in the base table); the layout `backpack` (0x22000038) has
`RealBackpackField` (19 elements, 19 images; no code of this build opens it, `ui-layout.md`
U45, so it is not a Tessera panel) and `singletonbagui` (0x220008DC) has
`SingletonBag` (81 elements, 83 images); the layout `characterequipmentpage` (0x2200084B) has
the root `CharacterEquipmentPage_Page` (941 elements, 168 images; how game code combines it
with the character panel is outside the data); the plugin API's game-styled controls come from
`luaplugins` (§6.8). All their images are in the base table (SI11).

Facts a skin author (and a test) relies on:

- **Matching is by ID only** (§8.3). `GamePlay_RadarBox` is also a top-level element of the
  layout `pvpgameplayactivatables` (0x22000890), so one override reaches both builds.
  `ToolbarField` is in addition a depth-1 child in the layout `pvpgameplay` (0x2200088C); a
  build that starts elsewhere reaches it only through a nested `Element` that mirrors that
  layout's tree.
- **Templates.** The minimap and both vitals roots inherit their frame (`HiddenDragBox_DragBox`,
  `HiddenDragBox_DragBar`, `HiddenDragBox_TitleText`) from the same template. A nested
  override of those children under one root applies to that build only (§8.2); art mappings
  of the frame's images apply to all three panels at once (§6.9).
- **Protection.** Each vitals root has one portrait element carrying
  `UISkin_CannotBeDetached` (`PlayerPortrait` at depth 2 under `AvatarPanel`,
  `OpponentPortrait` at depth 4 under `OpponentPanel`, both of the game type `TargetViewport`;
  two of the 8 protected elements of §8.4); no root carries a protection.
- **Scaling.** `ToolbarField` and the bag roots are UI-scaler elements, so `ScaleOriginX` /
  `ScaleOriginY` on them decide which point stays fixed when the UI scale changes (§8.4).
- **Edge modes.** Overriding the minimap, chat, toolbar or vitals root sets its non-zero edge
  modes to 0 (§8.4 step 3), so the panel stops following the screen size and sits at the
  override's screen pixels (§8.2).
- **Geometry the community does not override.** The chat and character roots appear in no
  published skin dictionary (XSD, MUI); their art is reachable by mappings like any other.

---

<a id="s9"></a>
## 9. Selecting and activating a skin

<a id="s9-1"></a>
### 9.1 The list shown to the user (Confirmed (BIN))

The original's options page offers one choice list:

1. a first entry meaning "no skin", whose text is the localised string of the named string
   table `UI_Option` (root map label `STRINGTABLE`, key 0x1000004B → table 0x250001BC here),
   token 0x0294D395;
2. then the names of **all** skins in the skin list (§3.2), duplicates and empty names
   included, **sorted by plain UTF-16 code-unit comparison** (case-sensitive: `Z` sorts before
   `a`).

<a id="s9-2"></a>
### 9.2 The stored selection (Confirmed)

The choice is the gameplay option keyed by the property `GOP_UI_DefaultSkin` (PID
0x100003E4, a text (StringInfo) value):

- the list of gameplay options (`UNIQUEDB` label `GameplayOptionList`, 0x78000007) has an
  entry for it, whose name and description texts live in the `UI_Option` table;
- its default (`UNIQUEDB` label `GameplayOptionDefaults`, 0x78000006) is a reference to the
  same "no skin" string as §9.1;
- choosing a skin stores the skin's name as a literal text value.

The selection is not a preferences-file setting: `UserPreferences.ini` has no key for it
(OBS), and the client registers no preference with a skin-related name (BIN: no such name
string exists). Where the original keeps gameplay options is outside this spec and outside
Tessera's offline scope; Tessera never imports the original's choice (OPEN-7 closed as out of
scope). Tessera keeps its own selection (§10, D6).

<a id="s9-3"></a>
### 9.3 Activation and deactivation (Confirmed (BIN))

- **Applying the option** (the user confirms the options page): first deactivate every
  active user skin; then, if the chosen text differs from the "no skin" text **ignoring
  ASCII case**, activate the skin by name. A skin whose own name equals the "no skin" text
  therefore cannot be chosen.
- **Activating by name**: make the active user-skin list empty, then search the skin list
  **in discovery order** for the first skin whose name equals the requested name ignoring
  ASCII case, and make it the only active user skin. If none matches, no user skin is active.
- **When the client creates its in-game UI** (two set-up paths do this), it reads the option
  and, if it is set and differs from the "no skin" text, activates that name the same way.
- **When the in-game UI is destroyed** (both in-game UI objects of the set-up paths above
  deactivate the user skin in their tear-down, Confirmed (BIN), SK44), every active user skin
  is deactivated and the refresh of §6.6 runs. So a user skin is active only while the
  in-game UI exists. Screens shown before it is created (the pre-game screens, character
  selection) and after it is destroyed (logging out to character selection) set their images
  while no user skin is active, and the gameplay option that names the skin is only read when
  the in-game UI is created. This closes OPEN-6. (A pre-game element that survived into the
  in-game UI would be re-skinned by the refresh at activation; none was traced.)
- After every activation or deactivation the image cache is emptied and every element's base
  image is re-resolved (§6.6), so a new skin's art appears at once; geometry does not change
  until a panel is rebuilt (§8.2). This matches the publisher's guide, which says the skin
  applies when the options are accepted (DOC).
- Game skins are activated separately (§6.2) and do not change the user skin.

<a id="s9-4"></a>
### 9.4 When skins are read (Confirmed (BIN))

Discovery and parsing (§3, §4) run once, while the client's UI starts. Adding a skin folder,
removing one or editing a definition file has no effect until the client is restarted; the
publisher's guide says the same (DOC). Edited image files are re-read on the next skin switch
(§6.6). No command re-reads skins.

---

<a id="s10"></a>
## 10. Tessera decisions

These are **Tessera design decisions**, not facts about the original. Where they differ from
the original, the difference only concerns cases in which the original misbehaves or depends
on Windows.

| # | Decision |
|---|---|
| D1 | **Where.** The skins root is `<settings folder>/ui/skins/` of the original, located by `install-layout.md` §8.1 (including `--prefs`), opened **read-only**. Tessera never creates, writes, renames or locks anything there. A Tessera setting may point to a different skins root instead. |
| D2 | **Letter case on case-sensitive file systems.** Path components (`ui`, `skins`, folder names on a `FileName`, the file name itself) are matched ASCII-case-insensitively when the exact name does not exist; the definition file name is matched ASCII-case-insensitively always (§3.2). This reproduces the original, which opens lower-cased paths on a case-insensitive file system (§6.4). If two entries differ only in case, the one that sorts first by byte value wins. |
| D3 | **Order.** Directory entries are processed sorted by name (ASCII case folded, then byte value) so that discovery order, and with it the winner among skins of equal name, is reproducible. |
| D4 | **Limits for hostile input.** Walk at most 16 folder levels below the skins root and at most 4,096 entries per folder; list at most 1,024 skins; definition files larger than 4 MiB are rejected; at most 65,536 mappings and 65,536 element overrides per skin; element nesting deeper than 64 is rejected as malformed; image files larger than 64 MiB or with a width or height of 0 or above 16,384 fail (placeholder). Symbolic links are not followed out of the skins root, and a folder is never walked twice. |
| D5 | **XML.** Accept XML 1.0 in UTF-8 (with or without byte-order mark) and UTF-16 with a byte-order mark; accept a declared `ISO-8859-1`, `US-ASCII` or `windows-1252`; reject other encodings. Ignore a document type declaration and never fetch external entities; expand only the five predefined entities and character references. |
| D6 | **Selection.** Tessera stores the chosen skin's name in its own settings file (`crates/settings`, section `[UI]`, key `UserSkin`; empty = no skin). It does not read or write the original's gameplay option. The list and the name matching follow §9.1 and §9.3; Tessera's "no skin" entry uses Tessera's own text. |
| D7 | **Format dispatch** uses the file name's final extension (`.dds`, `.jpg`, `.tga`; ASCII case ignored) instead of a substring of the whole path. It agrees with §7.1 for every path whose folders and file stem contain none of those substrings. |
| D8 | **Repeated top-level IDs** (§4.5 step 3): the later `PanelFile` is rejected as a whole and none of its IDs is registered. |
| D9 | **Diagnostics.** Every rejection of §4.9 is logged with `tracing` (warning level) with the file, line and reason; the Tessera options entry lists rejected definition files with their reason. |
| D10 | **Reload.** A developer command may re-run discovery and parsing at run time. It is off by default and not part of compatibility. |
| D11 | **File names** are used as Unicode (UTF-16 decoded to the platform's path encoding); the original's code-page narrowing (§4.4) is not reproduced. |
| D12 | **Absolute `FileName`** (§6.4): a drive-letter path is mapped through the Wine prefix's drive mapping when the install is inside a prefix; otherwise, and for doubled-separator (network) paths, the image fails (placeholder). |
| D13 | **Game skins** are decoded (§6.2) but not activated by the offline client. |
| D14 | **TGA run-length packets** may continue across scanlines (one continuous stream). |
| D15 | **IFF** images are not decoded in M5; they fail like an unknown format (placeholder). |
| D16 | **Placeholder.** The image of §6.5 is used for every failed skin file, failed replacement DID and failed base DID drawn as an element image. |
| D17 | **8-bit TGA** (provisional until OPEN-4 is answered): decoded as an alpha-only image whose colour is black and whose alpha is the 8-bit value (for a colour-mapped file, the alpha of the mapped colour). |
| D18 | **DDS sizes** (§7.4): the top-level size is computed in 64-bit arithmetic after the D4 dimension limits. A DXT1–DXT5 image whose width or height is not a multiple of 4 fails (placeholder), because the original either rejects it or reads a buffer too short for it. Otherwise the size, the read and the linear-size check follow §7.4; a file shorter than 128 + *size* bytes fails. |
| D19 | **Applying overrides** (§8.2–§8.4): Tessera follows the original exactly: the matching override is applied to a per-build resolved copy of each element's descriptor before the element is created, step 5 included (so a child is overridden once inside its parent's copy and again when it is built), and never to shared or cached layout data. |
| D20 | **Switching at run time** (§6.6, §8.2): Tessera follows the original. A switch empties the skin-image cache and runs the refresh of §6.6 over every existing element; it rebuilds no panel, so new geometry reaches a panel only when it is next built. Tessera's options entry says that moved panels take effect when a panel is reopened or at the next start of the world view. |
| D21 | **Which images are substituted** (§6.8): Tessera routes exactly the paths that the original routes through §6.3 (Image items from data and from code, Animation frames, meter frame arrays and meter child images, and the game-styled plugin controls built from layout data) and no others: Coloured-image overlays, hit masks, cursors and plugin backgrounds are not substituted when set. The refresh re-sets every base image that was created from a DID, whatever set it, so plugin backgrounds given as a DID are substituted after a switch, as in the original. |
| D22 | **Images remember their DID.** Every base image that Tessera creates from a render-surface DID keeps that original DID (not the replacement it shows) so that the refresh (§6.6) and repeated switches always start from the base DID. |
| D23 | **M5 panel set.** Tessera's native panels for M5 are the fifteen panels P1–P15 of `ui-layout.md` §17.2 (its bag panel P15 is bag 1, `containerdisplay` / `PlayerBag1_Template`, the same root as the Bags row of §8.6), built from their (layout label, top-level element label) pairs, resolved through the `UILAYOUT` map and the element mapper at run time. §8.6 gives the skin identifiers and art census of seven of them; the procedure of §12.7 runs over all fifteen. Tessera activates the user skin when its world view's UI is created and deactivates it, with a refresh, when that UI is torn down (§9.3); its pre-game screens are never skinned. |
| D24 | **Skin tests.** Real-data skin tests (§12.7) generate their skin in a temporary directory and point Tessera's skins-root setting (D1) at it. They never create, change or delete anything in the user's settings folder or the install, and the images and definition they write are synthetic (CC0, generated by the test). |

---

<a id="s11"></a>
## 11. Invariants

Real data (each checked against the whole install on 2026-10-05; tests read
`TESSERA_GAME_DIR` and assert the invariant, not the counts):

- **SI1** The root DID map has the label `EMAPPER`; that map has labels `ArtAssetID` and
  `UIElementID`, and both resolve to enum mappers (prefix 0x23).
- **SI2** No two labels in either mapper chain are equal under ASCII case folding.
- **SI3** The root DID map has the label `UNIQUEDB`; that map has a label `BaseSkinMappings`
  resolving to a 0x78 property set whose only top-level property is `UISkin_MappingArray`;
  every element is a `UISkin_MappingStruct` with exactly the fields `UISkin_UIArtAssetID` and
  `UISkin_DataID`.
- **SI4** In the base table the art-asset values are distinct and the DIDs are distinct; every
  art-asset value is a key of the art-asset chain; every DID has prefix 0x41 and exists in the
  archive set.
- **SI5** Every base-table DID is named by at least one Image media item (kind 4) of the UI
  layouts (§6.7).
- **SI6** Every layout element ID value is a key of the element chain.
- **SI7** Every layout property named `UISkin_Cannot…` holds a boolean.
- **SI8** No Animation frame DID and no Cursor DID of the UI layouts is in the base table
  (§6.3, §6.7). If a future install breaks this, OPEN-9 becomes relevant for those kinds.
- **SI9** No element that carries a `UISkin_Cannot…` property is named as the base of another
  element (§8.4, SK36).
- **SI10** For every row of the §8.6 table, the `UILAYOUT` map has the layout label, the
  element mapper has the element label, that element is a top-level element of that layout,
  and it resolves (`ui-layout.md` §7.1) without failure.
- **SI11** Every Image item with a non-zero DID in the resolved trees of the roots named in
  §8.6 names a DID that is in the base table. Checked 2026-10-06 over 38 roots (both
  `GamePlay_RadarBox` builds, `MinimapWindow`, the toolbar and both vitals roots,
  `NewChatWindow1`–`9`, `PlayerBag1_Template`–`6`, `RealBackpackField`, `SingletonBag`,
  `CharacterJournalPanel`, `CharacterEquipmentPage_Page` and the 13 `luaplugins` roots):
  3,380 Image items, no exception. A test reports every exception by panel and element label.
- **SI12** In the same trees no Coloured-image item names a base-table DID, and every Image
  item with a DID has draw mode Normal (2) or Alphablend (4), except in
  `CharacterEquipmentPage_Page`, which also uses Colorize (1) and Screen (7) (80 items).
- **SI13** The layout labelled `luaplugins` has the top-level element labels `LotroButton`,
  `LotroCheckBox`, `LotroTextBox` and `LotroWindow_Container` (§6.8).

Parser and resolver (synthetic fixtures; each is a test):

- **SP1** A definition whose root element has any name and that holds one valid `Mapping` is
  accepted (§4.1, §4.8).
- **SP2** `<mapping artassetid=… filename=…>` adds nothing (attribute case), while
  `<MAPPING ArtAssetID=… FileName=…>` adds a mapping (element case).
- **SP3** Of two `Mapping`s whose art-asset names differ only in case, the first is kept.
- **SP4** Of several `SkinName`s, the last with a non-empty `Name` names the skin.
- **SP5** A `PanelFile` with an empty `ID` is ignored; one whose `ID` is arbitrary text is
  used.
- **SP6** An unresolvable `ID` on a top-level `Element` drops its whole `PanelFile`; on a
  nested `Element` it drops that element and its later siblings only.
- **SP7** A leaf `Element` with `Width="0"` (or no `Width`) is dropped; a container with no
  size is kept and applies 0 × 0.
- **SP8** The integer table of §4.7 holds.
- **SP9** A definition with only `SkinName`, or whose every `Mapping` and `PanelFile` was
  rejected, is not a skin; a definition whose only content is an empty
  `<PanelFile ID="x"/>` is a skin (§4.5 step 4).
- **SP10** A second `PanelFile` whose top-level `Element` repeats an ID of the first is
  rejected; the first stays effective.
- **SP11** Image resolution (§6.3): with a user skin mapping value *A* and a game skin mapping
  *A* to *D′*, an element given the base DID of *A* draws the user file; with the file
  missing it draws the placeholder, not *D′* and not the base DID.
- **SP12** The placeholder is 32 × 32 with 8-bit RGBA texels (76, 76, 102, 255) at (0, 0)
  and (28, 28), and (255, 255, 255, 255) at (29, 0), (0, 29) and (31, 31) (§6.5).
- **SP13** A bottom-up and a top-down TGA with the same rows in the opposite file order
  decode to the same image; the right-to-left bit changes nothing.
- **SP14** Applying an override honours the three protection properties independently
  (§8.4).
- **SP15** Applying an override never changes `has_position`, the z-level, the flag byte or the
  media lists: a descriptor with `has_position` = 0 still has 0 afterwards, one with 1 still
  has 1 (§8.4).
- **SP16** Overrides act on the resolved descriptor (§8.2): when a based element whose
  protection property and child come only from its base is built as the root of a build, the
  protection applies and the inherited child receives its nested override (§12.5); the base element's own descriptor and
  the loaded layout data are unchanged afterwards.
- **SP17** DDS (§7.4, D18): an 8 × 8 DXT1 file with linear size 0 or 32 and 32 data bytes
  decodes; with linear size 31 it fails; with 31 data bytes it fails; a 6 × 6 DXT1 file fails
  (D18); a 4 × 4 32-bit A8R8G8B8 file with masks FF0000 / FF00 / FF / FF000000 and 64 data
  bytes decodes whatever its header-size, flags, pitch and mip-count fields hold; a header
  whose bit-count field is 0x120 (low byte 0x20) is treated as 32 bits.
- **SP18** With a user skin mapping the art asset of DID *D*: an element given *D* by an Image
  item shows the skin's file; an element given *D* by a Coloured-image item shows *D* tinted as
  usual (§6.8).
- **SP19** States (§6.9): an element whose state `normal` sets *D*₁ and whose state
  `normal_rollover` sets *D*₂, with only *D*₂'s art asset mapped, shows *D*₁ in `normal`, the
  skin's file in `normal_rollover`, and *D*₁ again after returning to `normal`; the draw mode
  in each case is the Image item's.
- **SP20** Refresh (§6.6): a panel built with no skin and showing a mapped DID *D* shows the
  file right after activation without being rebuilt, keeps its draw mode and its overlay, and
  shows *D* again right after deactivation; its rectangle does not change in either case.
- **SP21** A plugin background set from a mapped DID while the skin is active shows the base
  art; after the skin is deactivated and activated again it shows the skin's file (§6.8,
  D21). A plugin background set from a file never changes.
- **SP22** Geometry on a switch (§8.2, D20): a panel built before activation keeps its
  rectangle and edge modes after activation; the same panel built after activation has the
  override's rectangle and edge modes 0.
- **SP23** Scaler origin (§8.4, §12.8): the numbers of the worked example hold exactly,
  including the two clamps.
- **SP24** Size (§6.10): a 4 × 4 skin image on a 10 × 6 element is drawn three times across
  and twice down, the last copies cut; the element's pixel (5, 5), counted from its top-left
  corner, equals the image's pixel (1, 1); a 40 × 40 image on the same element shows only its
  top-left 10 × 6 pixels.

---

<a id="s12"></a>
## 12. Worked synthetic examples

All names below are made up for the example except where a real label is needed to show the
lookup; values are invented and must not be expected in real data.

<a id="s12-1"></a>
### 12.1 Discovery and the list

Files below the skins root:

| Path below `ui/skins/` | `SkinName` `Name` | Content | Result |
|---|---|---|---|
| `SkinDefinition.xml` | `Root` | 1 mapping | skin, folder = the skins root |
| `Alpha/SkinDefinition.xml` | `Zed` | 1 mapping | skin |
| `Alpha/extra/SkinDefinition.xml` | `alpha` | 1 panel override | skin |
| `Beta/skindefinition.XML` | `Zed` | 2 mappings | skin (name matched ignoring case) |
| `Gamma/SkinDefinition.xml` | `Gamma` | only `SkinName` | rejected (§4.8) |
| `Delta/Skin.xml` | — | — | not a definition file |

With D3, the walk visits `Alpha/` (whose `extra/` sorts before its own definition file),
`Beta/`, `Delta/`, `Gamma/`, then the root's definition file. Discovery order:
`alpha`, `Zed` (Alpha), `Zed` (Beta), `Root`. The list (§9.1) is: the "no skin" entry,
`Root`, `Zed`, `Zed`, `alpha`. Choosing either `Zed` entry activates the first discovered
`Zed`, the one in `Alpha/` (§9.3).

<a id="s12-2"></a>
### 12.2 Mappings and image resolution

```xml
<anything>
  <SkinName Name="Example"/>
  <Mapping ArtAssetID="BasePanel_TopLeft" FileName="art\tl.tga"/>
  <Mapping ArtAssetID="basepanel_topleft" FileName="art/other.tga"/>
  <Mapping ArtAssetID="no_such_label" FileName="x.tga"/>
  <Mapping ArtAssetID="" FileName="y.tga"/>
</anything>
```

1. Mapping 1: `BasePanel_TopLeft` resolves (ASCII case ignored) to the key of the label
   `basepanel_topleft` (0x1000005C in this install). Stored: value → `art\tl.tga`.
2. Mapping 2: same value → rejected, logged.
3. Mapping 3: does not resolve → rejected, logged.
4. Mapping 4: empty name → skipped silently.
5. The skin has one mapping → accepted.

At draw time an element whose image is the base DID of that value (from §6.1) draws the file
`<skin folder>art/tl.tga` (path lower-cased by the original before opening). If that file is
missing, the element shows the placeholder.

XML text in this example is a synthetic illustration of the grammar, not a copy of any
published file.

<a id="s12-3"></a>
### 12.3 Geometry

Synthetic layout data (the fixture also defines a synthetic element mapper with these labels;
IDs shown by their labels; rectangle = X, Y, Width, Height):

| Element | Parent | Rectangle | Edge modes (L, T, R, B) | `has_position` | Properties |
|---|---|---|---|---|---|
| `PanelA` | — (built as a panel root) | 100, 50, 300, 200 | 5, 5, 5, 5 | 1 | — |
| `PanelA_Title` | `PanelA` | 0, 0, 300, 20 | 0, 0, 0, 0 | 0 | — |
| `PanelA_Body` | `PanelA` | 0, 20, 300, 180 | 4, 0, 4, 0 | 1 | `UISkin_CannotBeResized` = true |

Skin:

```xml
<opt>
  <SkinName Name="Geometry example"/>
  <PanelFile ID="whatever">
    <Element ID="PanelA" X="10" Y="0x14" Width="320" Height="240">
      <Element ID="PanelA_Title" X="0" Y="0" Width="1" Height="1"/>
      <Element ID="PanelA_Body" X="0" Y="020" Width="999" Height="999"/>
    </Element>
  </PanelFile>
</opt>
```

Result when `PanelA` is built:

| Element | Rectangle | Edge modes | `has_position` |
|---|---|---|---|
| `PanelA` | 10, 20, 320, 240 (`0x14` is hexadecimal 20) | 0, 0, 0, 0 | 1 (unchanged) |
| `PanelA_Title` | 0, 0, 1, 1 | 0, 0, 0, 0 | 0 (unchanged) |
| `PanelA_Body` | 0, 16, 300, 180 (`020` is octal 16; size protected) | 0, 0, 0, 0 | 1 (unchanged) |

At its first run-time layout `PanelA` stays at (10, 20) with size 320 × 240 on any screen
size, because all its edge modes are now 0 (§8.2). Without the skin, its edge modes (5, 5, 5,
5) would have stretched it with the screen from its design area.

<a id="s12-4"></a>
### 12.4 A 2 × 2 TGA

Header: type 2, depth 32, width 2, height 2, descriptor 0x08 (8 alpha bits, bottom-up).
Pixel data in file order (B, G, R, A per pixel): `P0 P1` (first file row), `P2 P3`.
Decoded image, top row first: `P2 P3` / `P0 P1`. With descriptor 0x28 (top-down) the same
bytes decode to `P0 P1` / `P2 P3`.

<a id="s12-5"></a>
### 12.5 An override on an inherited element

Synthetic layout data, two top-level elements in one synthetic layout record:

| Element | Kind | Rectangle | Edge modes | Properties | Children |
|---|---|---|---|---|---|
| `Frame` | typed (Field) | 0, 0, 200, 100 | 5, 5, 5, 5 | `UISkin_CannotBeDetached` = true | `Frame_Close` (typed, 180, 0, 20, 20, edge modes 4, 5, 5, 4) |
| `PanelB` | based on `Frame` | 300, 300, 200, 100 | 5, 5, 5, 5 | — | none of its own |

`PanelB`'s resolved descriptor (`ui-layout.md` §7) has `PanelB`'s rectangle and edge modes,
`Frame`'s property `UISkin_CannotBeDetached` = true, and the inherited child `Frame_Close`.

Skin:

```xml
<opt>
  <PanelFile ID="b">
    <Element ID="PanelB" X="40" Y="40" Width="250" Height="120">
      <Element ID="Frame_Close" X="230" Y="0" Width="20" Height="20"/>
    </Element>
  </PanelFile>
</opt>
```

Building `PanelB` as a panel root:

1. Resolve `PanelB` into a copy: rectangle 300, 300, 200, 100; edge modes 5, 5, 5, 5;
   protection inherited; child `Frame_Close` inherited (re-laid out by `ui-layout.md` §7.3
   step 8 only if the size changed, which it did not).
2. Apply the override to the copy: rectangle becomes 40, 40, 250, 120; the edge modes stay
   5, 5, 5, 5 because the resolved copy holds `UISkin_CannotBeDetached`, inherited from
   `Frame`, so the panel is still laid out with the screen from its (new) design box;
   step 5 reaches the inherited child `Frame_Close`, which has no protection: rectangle
   230, 0, 20, 20 and edge modes 0, 0, 0, 0.
3. Build `PanelB` and then `Frame_Close` (which is matched again through the ID path
   `PanelB` → `Frame_Close`, with the same result).

Afterwards the `Frame` descriptor in the loaded layout still has `Frame_Close` at 180, 0 with
edge modes 4, 5, 5, 4, and a panel built from `Frame` itself is not affected (its ID is not a
top-level ID of the skin).

<a id="s12-6"></a>
### 12.6 DDS sizes

| Image | Format | Computed size (§7.4 step 3) | Real data size | Original | Tessera |
|---|---|---|---|---|---|
| 8 × 8 | DXT1 | 4 × 8 × 8 ÷ 8 = 32 | 32 | reads 32 bytes; linear size must be 0 or 32 | same |
| 8 × 8 | DXT5 | 8 × 8 × 8 ÷ 8 = 64 | 64 | reads 64 bytes | same |
| 6 × 6 | DXT1 | 18 | 32 | fails if linear size is 32; reads 18 bytes if it is 0 (undefined) | fails (D18) |
| 1 × 1 | DXT1 | 0 | 8 | fails (size 0) | fails (D18) |
| 3 × 5 | A8R8G8B8 (32 bits) | 60 | 60 (rows of 12 bytes) | reads 60 bytes; pitch field ignored | same |

<a id="s12-7"></a>
### 12.7 Testing ROADMAP M5 exit criterion 3 with a synthetic skin

Exit criterion 3 reads "a user skin folder changes the art of the panels it maps". The
procedure below tests it on the user's install with a skin that the test generates itself, so
no third-party skin and no game asset is copied or committed (D24). It selects everything from
the install at run time; no DID, label or count from this spec is hard-coded. A test that finds
`TESSERA_GAME_DIR` unset skips itself.

**Preparation (read-only):**

1. Load the base table (§6.1), the art-asset mapper chain (§5.1), the `UILAYOUT` map and the
   element mapper.
2. For each M5 panel that Tessera renders (the catalogue of `ui-layout.md` §17.2, D23; the
   bag panel is bag 1, `containerdisplay` / `PlayerBag1_Template`), build the panel with **no** skin
   active, lay it out on a fixed test screen (for example 1024 × 768) and let every element
   finish creation and enter its default state (`ui-layout.md` §9.2 step 8). For every element,
   record the DID of its current base image, its draw mode and its screen rectangle.
3. Keep the elements whose base-image DID is in the base table, whose draw mode is Normal or
   Alphablend, that are shown, that have a non-empty screen rectangle, and whose image opacity
   (`UICore_Element_alphablendmod`) and surface opacity (`UICore_Element_ObjectOpacity`, own
   and inherited from ancestors) are 1 (`ui-layout.md` §5.9). From their DIDs,
   take up to 8 distinct DIDs per panel in ascending DID order (fewer if the panel has fewer).
   If a panel yields none, the test reports it by name as "no skinnable art in the default
   view" and leaves it out of the checks below. This is a failure only for the seven panels of
   §8.6 (each has base-table Image items in its own blocks, SK48). In this install every one
   of the fifteen catalogue panels names base-table images (OBS, 2026-10-06), but two of them
   (the quest tracker and the extra shortcut bar) name them only in state blocks, so whether
   they show one depends on the states Tessera's panel code selects (`ui-layout.md` §17.5
   item (1)).
4. For each chosen DID *D*: its art-asset value *A* = the base-table entry for *D*; its label
   *L* = the label of key *A* in the art-asset mapper chain (labels are unique per key and
   unambiguous ignoring ASCII case, §5.3); its size *w* × *h* = the size in the header of
   surface *D* (`records.md` §5).

**The synthetic skin** (written to a fresh temporary directory, the test's skins root, D24):

5. For each chosen *D*, write an uncompressed 32-bit TGA (§7.2: image type 2, depth 32,
   descriptor 0x28 for top-left origin and 8 alpha bits) of *w* × *h* pixels, every pixel
   one colour that the test assigns to *D* (distinct per mapping, alpha 255, for example a
   hue from a fixed sequence), named `art/<D in hex>.tga`.
6. Write `SkinDefinition.xml` in a subfolder of the root (for example `TestSkin/`) with a
   root element `opt`, one `SkinName` with a name the test chooses, one `Mapping` per chosen
   *D* (`ArtAssetID` = *L*, `FileName` = `art\<D in hex>.tga`, so that the backslash rule of
   §6.4 is exercised), and three controls:
   - one more `Mapping` for one further skinnable DID *D*ₘ of any panel, naming a file that
     is **not** written (the missing-file case);
   - one `Mapping` whose `ArtAssetID` is a label that does not exist (must be rejected);
   - one `PanelFile` with one top-level `Element` whose `ID` is the root label of one M5
     panel and whose `X`, `Y`, `Width`, `Height` differ from its design rectangle.

**Run and checks:**

7. Discovery (§3.2) on the test root lists exactly one skin, with the chosen name; the
   rejected mapping is reported (D9), the others are kept.
8. Activate the skin (§9.3). Without rebuilding anything (§6.6, SP20), check for every
   element kept in step 3:
   - if its DID was chosen, its base image now comes from the matching file and its draw mode
     is unchanged;
   - if its DID is *D*ₘ, it shows the placeholder of §6.5 (SP12 texels);
   - otherwise its base image is still its DID (nothing unmapped changed).
9. Render each panel off screen. For every element whose DID was chosen, find a pixel inside
   its screen rectangle that no descendant element, no overlay layer and no text covers, and
   check that it has the mapping's colour (exact for Normal; for Alphablend with alpha 255 the
   result is the same colour; allow ±1 per 8-bit channel for rounding). If no such pixel
   exists for an element, skip that element and count it.
10. Build the panel named in the `PanelFile` again: its root has the override's rectangle and
    edge modes 0 (§8.4, SP22); a panel built before activation keeps its old rectangle.
11. Deactivate the skin: every element of step 3 shows its own DID again (SP20) and the skin
    image cache is empty (§6.6).
12. Delete the temporary directory.

**Pass condition for the exit criterion:** for every M5 panel rendered and not left out in
step 3 (and always for the seven panels of §8.6), at least one element changes its art to the
mapped file in step 8 and passes the pixel check of step 9; no element
whose DID was not mapped changes; the missing-file element shows the placeholder; step 11
restores the original art. The test reports, per panel, the numbers of chosen DIDs, checked
elements and skipped elements.

<a id="s12-8"></a>
### 12.8 Scaler origin

A synthetic UI-scaler element on a 1920 × 1080 screen: current position (448, 980), cached
scale *s*₀ = 1.0, origin (512, 100) (the bottom centre of its unscaled 1024 × 100 size). The
scaler changes the scale to *s*₁ = 1.25 and gives the element the new size 1280 × 125.

- *x*′ = 448 + trunc(512 × 1.0) − trunc(512 × 1.25) = 448 + 512 − 640 = 320;
  *y*′ = 980 + trunc(100 × 1.0) − trunc(100 × 1.25) = 980 + 100 − 125 = 955.
- Clamp: min(320, 1920 − 1280 = 640) = 320, max(320, 0) = 320; min(955, 1080 − 125 = 955)
  = 955, max(955, 0) = 955.
- Result: position (320, 955), cached scale 1.25. The bottom centre was at (448 + 512,
  980 + 100) = (960, 1080) and is at (320 + 640, 955 + 125) = (960, 1080) again.

With origin (0, 0) and position (1000, 1000) instead, *x*′ = 1000 and *y*′ = 1000 before
clamping, and min(1000, 640) = 640, min(1000, 955) = 955: the element is pushed back on screen
to (640, 955).

---

<a id="s13"></a>
## 13. Corrections to other specs

| Spec | Statement there | Correction | Source |
|---|---|---|---|
| `install-layout.md` §8.2, row `ui/skins/<Skin>/SkinDefinition.xml` + TGA images ("read at client start", Reported) | One folder per skin, TGA images | Definition files are found at **any depth** below `ui/skins/` (file name matched ignoring ASCII case), including directly in it; images may be TGA, JPEG, DDS (and IFF); read once at UI start. Mark Confirmed (BIN). See §3, §7, §9.4 here. | BIN |
| `install-layout.md` §8.2, row `Plugins/` ("also `.plugincompendium`, `.skincompendium`, `.musiccompendium` manager files") | `.skincompendium` files in `Plugins/` | Third-party managers keep a skin's `.skincompendium` inside **that skin's folder** below `ui/skins/` (OL). The client ignores it in either place (`plugin-descriptor.md` §9). | OL; BIN |
| `ui-layout.md` §5.3 ("The client copies the flag during inheritance (§7) and sets it when a user skin overrides an element's geometry") and §9.8 ("can override its position and size (and then sets `has_position`)") | A user-skin override sets `has_position` | The override does **not** write `has_position`; it writes only X, Y, Width, Height, the four edge modes (all set to 0 unless `UISkin_CannotBeDetached`) and the two scaler-origin properties, then recurses into the children (§8.4 here, SK33). §9.8 there should also say that the override zeroes the edge modes and may set the scaler origin, and that it applies to the per-build resolved copy (§8.2 here, SK34). | BIN (two independent readings, 2026-10-05). **Applied** in `ui-layout.md` §5.3 and §9.8 (its consolidation revision, 2026-10-06). |
| `ui-layout.md` §9.8 ("when an image is set by DID, the skin can substitute its own image for the surface") | Every image set by DID can be substituted | Only the element's base-image setter substitutes (Image items from data and from game code, Animation frames, meter frame arrays and child images); Coloured-image overlays, hit masks, cursors and directly created images do not (§6.8 here, SK38–SK39). §6.3 there could add that its overlays are never substituted. | BIN (2026-10-06) |
| `ui-layout.md` §3.2 and §9.1, OPEN on how game code finds a layout | Unknown whether the `UILAYOUT` map is used | For at least one panel family it is: the plugin API's game-styled controls are created from the layout found in the `UILAYOUT` map by **entry key** (the key of the `luaplugins` entry), with fixed top-level element IDs (§6.8 here, SK42). This answers part (1) of that OPEN for these controls only. | BIN (2026-10-06). Since answered in full there (U31: game code names layouts by `UILAYOUT` entry key; U45 for the bags). |
| `lua-plugin-api.md` §14.2 ("Art comes from the client's UI skin data (see `ui-layout.md`)") | Source of the look not named | The game-styled controls are built from top-level elements of the layout labelled `luaplugins` (`LotroWindow_Container`, `LotroButton`, `LotroCheckBox`, `LotroTextBox`, the scroll bars and gold variants); their art and frame metrics are in that layout, which may help close its OPEN on border widths and title height, and user skins re-skin them (§6.8 here). | BIN; OBS |
| `lua-plugin-api.md` §13.8 (`SetBackground`) | Silent on skins | A background set from a DID (or a `Graphic` made from one) is not substituted when set but is re-set through the user skin at the next skin switch; one set from a file never is (§6.8 here, SK41). | BIN (2026-10-06) |
| `docs/ARCHITECTURE.md` §15 (neutral doc, not a spec) | Skins map "art asset IDs to TGA files" | Art-asset names resolve through an enum mapper and the `BaseSkinMappings` table to render-surface DIDs (§5, §6); files may also be JPEG or DDS. For the maintainer to update. | OBS; BIN |

---

<a id="s14"></a>
## 14. Open questions

> **OPEN-1:** Saved panel positions. The original applies a skin's top-level X/Y to the
> resolved descriptor before a panel is built, and with zeroed edge modes the panel lands at
> those screen pixels (§8.2). Saved panel locations are a separate mechanism: game code applies
> the saved store to an element **after** creating it (`ui-layout.md` §9.9, BIN), and only for
> elements whose save properties are true; that step moves and resizes the element through its
> ordinary move and resize, after the skin-adjusted descriptor was built, so a saved entry
> would win over the skin's rectangle (Probable). (`ui-layout.md` §9.2 step 5, which an
> earlier version of this callout cited as saved data, is the message-listener check; it does
> not touch geometry.) Still not established: whether and when game code applies the store to
> a panel that a skin moved, and where the store lives (`ui-layout.md` §9.9 OPEN). An owner test
> with a skin moving one panel, before and after moving that panel by hand, answers it.
> Tessera: its own saved location, if any, is applied after the skin (`ui-layout.md` §17.1
> step 3).

**OPEN-2 (closed 2026-10-06).** Run-time switch and geometry: neither the skin code nor the
option handler rebuilds or re-lays out panels; the refresh re-sets images only (§6.6, §8.2,
SK40). Tessera follows this (D20). Whether some unrelated code of the options page rebuilds
panels for another reason is OPEN-12.

**OPEN-3 (closed 2026-10-06).** `Detach`: the stored value has no reader in this build, so the
attribute has no effect (§8.5). No observation is needed.

> **OPEN-4:** 8-bit TGA. Greyscale and colour-mapped 8-bit files become alpha-only images
> (§7.2); how the UI draws them (black with that alpha, white, or not at all), and how a
> colour map is applied on that path, needs an observation. Tessera's provisional choice is
> D17.

**OPEN-5 (closed 2026-10-05).** Size mismatch: answered by `ui-layout.md` §9.6 (native size,
tiled, clipped, never stretched); see §7.1 and §6.10.

**OPEN-6 (closed 2026-10-06).** Pre-game screens: the user skin is active only while the
in-game UI exists; it is deactivated, with a refresh, when that UI is destroyed, so character
selection is never skinned (§9.3, SK44, D23).

**OPEN-7 (closed 2026-10-06, out of scope).** Where the original stores gameplay options such
as `GOP_UI_DefaultSkin` is outside Tessera's offline scope; Tessera never imports the choice
(§9.2, D6).

**OPEN-8 (closed 2026-10-06).** Repeated mappings: two independent readings of the client
show that the first `Mapping` of an art-asset value is kept (§4.4, SK8, SK46); the community
validator's "last one wins" does not describe this build.

> **OPEN-9:** Other art paths. Settled (§6.8): parts (a) Coloured-image overlays are never
> substituted (SK39), and (c) the plugin API's game-styled controls are built from the
> `luaplugins` layout and are skinned (SK42). Still open, part (b): which images the game
> widgets of the M5 panels set by code without the base-image setter. Traced so far: virtue
> and trait displays, quest-dispenser icons and plugin-manager entries create their images
> directly (§6.8). Not traced: the class icon of the target vitals panel (property
> `UI_ClassDisplay_Icon`, whose values in `OpponentPanel` include 19 references to base-table
> DIDs), the drawing of the `Radar` widget type, and cursor images. A community skin author
> reports that the minimap's rim keeps its look under a skin that replaces every art asset
> it knows (MUI), while the minimap tree has 58 Image DIDs, all in the base table (§8.6). To
> settle: trace the setter used by the `Radar` and `SelectionTargetDisplay` widget types, or
> run the procedure of §12.7 in the original client (owner observation) and note which
> minimap and vitals parts keep their look.

**OPEN-10 (closed 2026-10-05).** The four values are the edge modes of `ui-layout.md` §4.4
fields 18–21 and §5.8; an applied override sets all four to 0 (§2, §8.4 step 3).

**OPEN-11 (closed 2026-10-06).** The scaler origin is the point that keeps its screen position
when a UI-scaler element changes scale (§8.4, SK45, worked example §12.8).

> **OPEN-12:** Whether the options page, after it applies all options (not only the skin),
> rebuilds open panels for a reason of its own (for example a UI-scale or resolution change
> applied in the same step), which would make a new skin's geometry visible at once in that
> case. The skin path itself does not (§8.2). An owner test: change only the skin while a
> moved panel is open, then change the skin and the UI scale together, and compare.

> **OPEN-13:** Which caller of the UI scaler asks for the origin to be ignored (§8.4, effect
> of the scaler origin, step 2), and in which situation (for example the first layout of a
> panel, or a reset of the UI layout). Until answered, Tessera always applies the origin.

---

<a id="s15"></a>
## 15. Change log

| Date | Change | Author role |
|---|---|---|
| 2026-10-05 | First version: discovery, definition grammar and parser rules, name resolution, base and game art tables, image precedence and placeholder, image formats (TGA, JPEG, DDS, IFF), panel-override matching and application, selection and activation, Tessera decisions, invariants, examples, corrections, open questions. Status Review. | Spec writer (dirty room) |
| 2026-10-05 | Revision answering the first review (changes requested). §8.4: the override does not write `has_position` (new SK33; correction row for `ui-layout.md` §5.3 and §9.8 in §13); step 3 restated as setting the four edge modes to 0; step 5 walks the resolved descriptor's children, inherited ones included; new paragraph on the two applications per child (SK35, SK36). §8.2: overrides act on a per-build resolved copy after base inheritance, reachable only from element building (SK25 revised, SK34), and the resulting placement in screen pixels (SK37). §2: "anchoring values" replaced by the edge modes of `ui-layout.md` §4.4/§5.8; new term "resolved descriptor"; header and LAY row re-pointed to `ui-layout.md` sections, new source tag ULS. §6.3 and §6.7: per-media-kind census (Animation frames and Cursor images never in the base table, file-path Image items bypass substitution; SK15 revised). §6.5: stored 8-bit texel values (SK19 revised). §7.1: image drawing per `ui-layout.md` §9.6. §7.4: full DDS header table with file offsets, size computation, linear-size rule, non-multiple-of-4 DXT behaviour (SK24 revised). SK8 and SK18 now cover the code-page narrowing and the 255-byte path cut. New D18, D19; new SI8, SI9, SP15–SP17; SP12 made exact; new examples §12.5, §12.6. OPEN-5 and OPEN-10 closed; OPEN-1, OPEN-2 and OPEN-9 narrowed. BIN questions 8–10 added. | Spec writer (dirty room) |
| 2026-10-06 | Revision answering the re-review (changes requested). §6.7: the 6,187 figure is now stated as the count of distinct render-surface DIDs (prefix 0x41) named by the four image kinds (5,122 + 1,065 frame-only); Sound items add 10 distinct sound-info DIDs (prefix 0x2A, 692 references), giving 6,197 for all media kinds. Re-checked over all 302 layout records (OBS, LAY, 2026-10-06). No invariant or algorithm changes. | Spec writer (dirty room) |
| 2026-10-06 | M5 revision (status Accepted → Review; section numbers kept). **New sections:** §6.8 which image paths a skin reaches (base-image setter: Image items from data and from game code, meter child images, Animation frames, meter frame arrays; not Coloured-image overlays, hit masks or directly created images; plugin backgrounds by DID re-skinned only at a refresh; plugin game-styled controls built from the `luaplugins` layout); §6.9 art per element and per state; §6.10 image size rules; §6.11 failure behaviour; §8.6 the M5 native panels (minimap, chat, toolbar, player and target vitals, bags, character panel) with layout labels, top-level element IDs, root kinds, rectangles, edge modes and art census, plus alternatives; §12.7 a test procedure for ROADMAP M5 exit criterion 3 with a synthetic, test-generated skin; §12.8 scaler-origin example. **Revised:** §6.3 (the substitution belongs to the base-image setter; overlays never substituted), §6.6 (the refresh re-sets base images from their original DID, keeps draw modes, rebuilds nothing), §4.4 (second BIN reading of duplicate mappings), §8.2 (no rebuild on a switch), §8.4 (effect of the scaler origin), §8.5 (`Detach` closed), §9.2, §9.3 (deactivation when the in-game UI is destroyed; pre-game screens unskinned), header (reviewer reset, naming note cites the *Google v. Oracle* row of `docs/LEGAL.md` §4.1 as the earlier review suggested). **Provenance:** BIN questions 11–16, OBS and MUI rows extended, new SK38–SK50, SK8 updated. **Decisions:** D20–D24. **Invariants:** SI10–SI13, SP18–SP24. **Corrections (§13):** `ui-layout.md` §9.8 and §3.2/§9.1, `lua-plugin-api.md` §13.8 and §14.2. **OPENs:** closed OPEN-2, 3, 6, 7, 8, 11 and OPEN-9 (a) and (c); OPEN-1 and OPEN-9 (b) narrowed; new OPEN-12 (options page rebuilds) and OPEN-13 (caller that ignores the scaler origin); OPEN-4 unchanged. All counts re-observed on the owner's install on 2026-10-06; BIN on build 4906.0071.1794.4035 (same hash prefix), address ranges only. | Spec writer (dirty room) |
| 2026-10-06 | Consolidation revision (status Accepted → Review; section numbers kept; answers the M5 completeness critique items B4 and B6 for this spec and the M5 review's suggestion (c)). **Bag panel (B4):** D23 now names the fifteen-panel catalogue of `ui-layout.md` §17.2 as Tessera's M5 panel set, whose bag panel is the same root as the Bags row here (`containerdisplay` / `PlayerBag1_Template`; `ui-layout.md` changed its P15 to it); §8.6 states that its seven rows are part of that catalogue and that the original opens the bags through a bag-window class (U45 there); the `backpack` alternative is marked as opened by no code; §12.7 runs over the catalogue, reports panels without skinnable art in their default view and fails only for the seven §8.6 panels (new SK51). **Stale references (B6):** OPEN-1 no longer calls `ui-layout.md` §9.2 step 5 saved data (it is the listener check; saved locations are applied after creation by §9.9 there); §8.6 no longer says the pair choice is OPEN in `ui-layout.md` §9.1 (closed there, U31); the §13 rows for `ui-layout.md` §5.3/§9.8 and §3.2/§9.1 are marked applied or answered; the header no longer calls `ui-layout.md` a Review draft. **§8.4:** the scaler-origin paragraph cites `ui-behaviour.md` §9.2–§9.2.1 for scale choice, resize and boxes, and names the `UIScalerPanel` roots. | Spec writer (dirty room) |
