<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# User UI skins: the skin folder, `SkinDefinition.xml`, and how a skin is applied

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Re-review: Claude review session (spec-writer agent), review 2026-10-05: changes requested. All five earlier issues are repaired, and every §5.3 item passes except one census statement that a spot check over all 302 layout records contradicts: §6.7 says "All media kinds together name 6,187 distinct DIDs", but with the 10 Sound-item DIDs (prefix 0x2A) all media kinds name 6,197; 6,187 is the count for the four image kinds (Image, Coloured image, Cursor, Animation; all prefix 0x41). Matched on re-check: the §6.7 and SK15 per-kind census (10,769 / 5,122 / 4,742 / 9,877; 5,105, 16 (6), 7 (0); 2,252 / 1,066 / 1,065, none in the base table; 29 Coloured-image references, 12 in the base table; 11 file-path JPEG items), SK36 / SI9 (8 protected elements in 7 records, typed, depths 1–4, none a base), the 6,224 / 10,197 edge-mode split, the 218 scaler-origin elements, the §5.3 ID census, and the base and game-skin tables. BIN matched (build 4906.0071.1794.4035, address ranges only): SK34 (per-build resolved copy, discarded after the build; the matcher is called only from the two build paths), §8.3 matching, §8.4 (own-property-set protection lookups, the field writes, no `has_position`), §7.1 dispatch, §6.4 step 5, §7.4 (header fields, low-byte bit count, size formula, linear-size check for FourCC only) and §6.5 (fill values, truncation, the white strip). Earlier record: Claude review session (spec-writer agent), review 2026-10-05: changes requested (has_position vs `ui-layout.md` §5.3/§9.8; override applied to the resolved descriptor not stated; stale `ui-layout.md` cross-references, §2 anchoring values, OPEN-5/OPEN-10; DDS header offsets; placeholder texel values) |
| **Implements into** | `crates/ui` (`tessera-ui`: skin discovery, definition parser, image override and geometry override application), `crates/material` or `crates/assets` (decoding skin image files: TGA, JPEG, DDS), `crates/props` (reading the base and game skin property sets), `crates/install` (settings folder, `install-layout.md` §8), `crates/settings` (Tessera's own skin selection), `apps/client` (options entry), `fuzz/` (one target for the definition parser, one per image decoder) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (UI layout records 0x22, status Review; cited here: §4.4 edge-mode fields, §5.3 rectangle and `has_position`, §5.8 edge-mode values, §6.2–§6.5 media items that name images, §7 base inheritance, §8.4 run-time layout, §9.1–§9.2 building panels and element trees, §9.6 drawing an element's images, §9.8 the user-skin hook), `install-layout.md` §8 (settings folder), `records.md` §7–§8 (enum mappers, DID maps), `property-system.md` §4–§5 (property sets), `plugin-descriptor.md` §9 (compendium side files), `texture-list.md` / `material.md` (render surfaces 0x41) |

> **Naming note (interoperability).** The folder and file names (`ui`, `skins`,
> `SkinDefinition.xml`), the XML element and attribute names (`SkinName`, `Name`, `Mapping`,
> `ArtAssetID`, `FileName`, `PanelFile`, `ID`, `Element`, `X`, `Y`, `Width`, `Height`,
> `Detach`, `ScaleOriginX`, `ScaleOriginY`), the DID-map labels (`EMAPPER`, `UNIQUEDB`,
> `STRINGTABLE`, `ArtAssetID`, `UIElementID`, `BaseSkinMappings`, `MonsterPlaySkin`,
> `UI_Option`), the property names (`UISkin_…`, `GOP_UI_DefaultSkin`,
> `UIComponent_UIScaler_Origin_X/Y`) and the enum labels quoted as examples are identifiers
> that existing skins and the user's own data use. They appear here only because a compatible
> reader cannot work without them (`docs/LEGAL.md` §10, `docs/ARCHITECTURE.md` §15). They
> must not become Tessera product, crate or UI names. All descriptive text is our own; no
> third-party prose, schema comment, client message text or game text is reproduced.

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
- Tessera's own decisions where the original's behaviour is undefined, unsafe or bound to
  Windows (§10).

All counts are observations of one install on one date. Every DID, enum value and property
ID quoted is an observation that an implementation **resolves at run time** through the
labels named here; none is a constant to hard-code.

---

<a id="s1"></a>
## 1. Provenance

Legend: **Confirmed** = observed in the user's install on 2026-10-05 by read-only throwaway
probes over **all** records of the kind; **Confirmed (BIN)** = read from the client
executable under decision 0011 and resting on it alone; **Reported** = published sources
only; **Decision** = Tessera's own choice (§10), not a fact about the original.

| Tag | Source |
|---|---|
| **OBS** | The owner's Steam install (data current on 2026-10-05), read in place: every enum mapper (0x23, 735 records) and DID map (0x28, 24) of `client_general.dat`; every stand-alone property set (0x78, 1,616) of `client_gamelogic.dat`; the property dictionary 0x34000000; every UI layout record (0x22, 302 records, 16,421 elements, parsed completely with the record grammar of `ui-layout.md` §4); the DID lists of every render-surface archive. Also the owner's settings folder inside the Proton prefix (`…/Documents/The Lord of the Rings Online/`): it has **no `ui/` folder**, so no user skin is installed, and its `UserPreferences.ini` has no skin-related key (the `[UI]` section holds 25 unrelated keys). |
| **BIN** | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (changelist 711794, compiled 2026-09-27, SHA-256 prefix `d22363eb9aff0fb5`), read with capstone, lief and pefile in the dirty room on 2026-10-05, function by function (address ranges only). **Questions:** (1) where the client looks for skins, to what depth, and how it matches the file name; (2) the exact grammar its parser accepts: element and attribute names, case rules, defaults, number parsing, duplicate and error handling, and when a file is rejected; (3) how art-asset names and element names are resolved against data; (4) the per-image precedence between user skin, game skin and base art, and the fallback for missing files; (5) which image file formats it decodes and how; (6) how panel overrides are matched to layout elements and what they change; (7) how the skin list is built, how the selection is stored and when a skin is activated or re-read; (8) on which descriptor an override acts (before or after base inheritance, a shared or a per-build copy), which of its fields it writes (in particular whether it sets `has_position`), and from where the override code can be reached; (9) which fields of a DDS header the reader uses and how it sizes the pixel data; (10) the stored 8-bit texel values of the placeholder image. Questions 8–10 were added on 2026-10-05 in answer to the first review. **Why M1–M3 were not enough:** the data holds the base art table and the name labels (M1, used throughout), but the parser rules, the precedence, the image decoders and the activation are code; this owner has no skin installed to observe (M1); the published documents are from 2007–2014, do not cover most of these rules and partly disagree with each other (M2); no running official client is available to this session (M3). Only the offline UI-skin, UI-layout instantiation, file-search, XML-reading and image-decoding code was read: nothing about network, login, keys, the launcher or anti-tamper. Numeric constants (§6.5, §7) are functional facts under [decision 0012](../decisions/0012-numeric-constants.md). No address, listing or symbol appears in this spec. |
| **DOC** | The publisher's original skinning overview and art-asset dictionary pages (2007), as republished by the community site at `lotrointerface.com/index.php?p=ui_skinning_1` and `…_2`, fetched 2026-10-05. Facts only; no text is reproduced. |
| **RES** | The project's resource catalogue (`research/resources.json`), entries for the 2007–2009 community forum thread on UI skinning (lotrointerface.com thread 11, a repost of the publisher's beta-forum guide with later developer clarifications) and the publisher's 2014 skinning art pack; that thread now needs a login and could not be re-read on 2026-10-05. |
| **XSD** | `lunarwtr/vscode-lotro-api` (MIT): `resources/xsds/lotro-skin.xsd` and `lotro-skin-panelfiles.xsd` (community schemas), and `resources/skinning/SkinDictionary.xml` (a community dump of panel and element names with geometry); used for facts and cross-checks only. |
| **ASSOC** | `lunarwtr/lotro-api-docs`, `skinning/skinning_element_asset_assocation.xml` (no licence; a community dump pairing element names, art-asset names and image DIDs); cross-check only. |
| **MUI** | `Felix-Helleckes/MithrilUI` (MIT, 2026): a skin generator with documentation of observed client behaviour (`docs/SKINNING.md`, `docs/SAFETY.md`, `docs/INSTALL.md`, `tools/validate.py`) and two name lists mined from shipping skins (`skin/dictionary.txt`, `docs/reference-element-ids.txt`). Community claims; facts only. |
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
| SK8 | `Mapping`: both attributes must be non-empty; the art-asset name must resolve; for a repeated art-asset value the **first** mapping is kept and later ones are rejected; `FileName` is narrowed to the system code page, and a name with any unrepresentable character has every non-ASCII character replaced by the text `<0xNNNN>` (§4.4) | BIN | Confirmed (BIN). MUI claims the last one wins: **disagreement**, see §14 OPEN-8. |
| SK9 | `PanelFile`: `ID` must be non-empty but its value is never looked up; its `Element` children form the override tree (§4.5) | BIN; OBS (only 1 of the 109 `PanelFile` IDs listed by XSD is an element label at all) | Confirmed (BIN) |
| SK10 | `Element` attributes, integer parsing (base auto-detected, partial numbers accepted, missing means 0) and the conditions under which an element override is kept or dropped (§4.6, §4.7) | BIN | Confirmed (BIN) |
| SK11 | A definition with no kept mapping and no kept panel override is rejected (§4.8) | BIN | Confirmed (BIN) |
| SK12 | Art-asset names are labels of the enum mapper that the `EMAPPER` DID map labels `ArtAssetID`; element names are labels of the mapper it labels `UIElementID`; lookup ignores ASCII case and follows the base chain (§5) | BIN; OBS | Confirmed |
| SK13 | Label census: 5,623 + 2 art-asset labels, 19,893 + 44 element labels; no two labels of either chain are equal ignoring ASCII case (§5.3) | OBS | Confirmed |
| SK14 | The base art table is the property set that `UNIQUEDB` labels `BaseSkinMappings`: an array of (art-asset value, render-surface DID) pairs, 4,742 entries, both columns unique, every DID present in the surface archives (§6.1) | OBS; BIN (the client reads it keyed by DID, first entry wins) | Confirmed |
| SK15 | Every one of the 4,742 base DIDs is referenced by at least one Image media item of the layouts; 4,742 of the 5,122 distinct images named by Image, Coloured-image and Cursor items are in the base table; no Animation frame DID (1,066 distinct) and no Cursor DID (7) is in it (§6.7) | OBS (LAY) | Confirmed |
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
   wins**.
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

Every time the client sets an element's image from a render-surface DID *D* (it does so for
the images that the layout data gives an element and its states, and again for every element
when the UI is refreshed after a skin change), the original decides what to draw:

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
  - **Coloured image** overlays (kind 9): 6 of their 16 distinct DIDs are in the base table;
    whether the overlay setter passes through this path is OPEN-9.
  - Art drawn by other subsystems (for example 3D viewports) or not named by layout data is
    outside this path; which of it a player sees as UI art is OPEN-9.
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
animated UI art cannot be skinned in this install. All media kinds together name 6,187
distinct DIDs.

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
afterwards; whether the client rebuilds existing panels after a switch (outside the skin code)
is OPEN-2.

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
   applied (218 layout elements carry them, OBS); their effect is OPEN-11.
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

<a id="s8-5"></a>
### 8.5 `Detach` (Confirmed (BIN))

The original parses `Detach` and stores "non-zero" but never reads the stored value in this
build: step 3 of §8.4 runs for every applied override whether `Detach` is 1, 0 or absent.
The community schema describes `Detach` as a switch, used only by the toolbar and off by
default, that removes attachment information (XSD); in this build that removal happens for
every applied override, so the attribute has no effect. Tessera follows this build: it
parses and ignores `Detach` (OPEN-3 asks for an observation).

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
string exists). Where the original keeps gameplay options is outside this spec (OPEN-7).
Tessera keeps its own selection (§10, D6).

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
- After every activation or deactivation the image cache is emptied and all elements
  re-resolve their images (§6.6), so a new skin's art appears at once. This matches the
  publisher's guide, which says the skin applies when the options are accepted (DOC).
- Game skins are activated separately (§6.2) and do not change the user skin.

Whether the user skin also applies to the pre-game screens (before a character enters the
world) is OPEN-6: the original only activates it at the points above.

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

---

<a id="s13"></a>
## 13. Corrections to other specs

| Spec | Statement there | Correction | Source |
|---|---|---|---|
| `install-layout.md` §8.2, row `ui/skins/<Skin>/SkinDefinition.xml` + TGA images ("read at client start", Reported) | One folder per skin, TGA images | Definition files are found at **any depth** below `ui/skins/` (file name matched ignoring ASCII case), including directly in it; images may be TGA, JPEG, DDS (and IFF); read once at UI start. Mark Confirmed (BIN). See §3, §7, §9.4 here. | BIN |
| `install-layout.md` §8.2, row `Plugins/` ("also `.plugincompendium`, `.skincompendium`, `.musiccompendium` manager files") | `.skincompendium` files in `Plugins/` | Third-party managers keep a skin's `.skincompendium` inside **that skin's folder** below `ui/skins/` (OL). The client ignores it in either place (`plugin-descriptor.md` §9). | OL; BIN |
| `ui-layout.md` §5.3 ("The client copies the flag during inheritance (§7) and sets it when a user skin overrides an element's geometry") and §9.8 ("can override its position and size (and then sets `has_position`)") | A user-skin override sets `has_position` | The override does **not** write `has_position`; it writes only X, Y, Width, Height, the four edge modes (all set to 0 unless `UISkin_CannotBeDetached`) and the two scaler-origin properties, then recurses into the children (§8.4 here, SK33). §9.8 there should also say that the override zeroes the edge modes and may set the scaler origin, and that it applies to the per-build resolved copy (§8.2 here, SK34). | BIN (two independent readings, 2026-10-05) |
| `docs/ARCHITECTURE.md` §15 (neutral doc, not a spec) | Skins map "art asset IDs to TGA files" | Art-asset names resolve through an enum mapper and the `BaseSkinMappings` table to render-surface DIDs (§5, §6); files may also be JPEG or DDS. For the maintainer to update. | OBS; BIN |

---

<a id="s14"></a>
## 14. Open questions

> **OPEN-1:** Saved panel positions. The original applies a skin's top-level X/Y to the
> resolved descriptor before a panel is built, and with zeroed edge modes the panel lands at
> those screen pixels (§8.2). Whether a position the player saved with the UI layout editor
> or with a layout file (`ui/layouts/*.layout`) then overrides the skin's (the client consults
> per-user saved panel data while it creates an element, `ui-layout.md` §9.2 step 5, not
> traced) is not established. An owner test with a skin moving one panel, before and after
> moving that panel by hand, answers it.

> **OPEN-2:** Run-time switch and geometry. The skin code never changes existing elements;
> new geometry reaches only elements built after the switch (§8.2, SK34). Whether the client
> destroys and rebuilds open panels after a skin switch, so that they pick up the new
> geometry at once, is not established (the refresh traced so far re-resolves images only,
> §6.6). An owner test: switch from no skin to a skin that moves an open panel, and see
> whether the panel moves before it is closed and reopened.

> **OPEN-3:** `Detach`. This build ignores the attribute (§8.5). An owner test (a skin
> moving one toolbar element once with `Detach="1"` and once without) would confirm that the
> two look the same.

> **OPEN-4:** 8-bit TGA. Greyscale and colour-mapped 8-bit files become alpha-only images
> (§7.2); how the UI draws them (black with that alpha, white, or not at all), and how a
> colour map is applied on that path, needs an observation. Tessera's provisional choice is
> D17.

**OPEN-5 (closed 2026-10-05).** Size mismatch: answered by `ui-layout.md` §9.6 (native size,
tiled, clipped, never stretched); see §7.1.

> **OPEN-6:** Pre-game screens. Whether a user skin is active on the character-selection
> screens, and whether it stays active after logging out to them (§9.3).

> **OPEN-7:** Where the original stores gameplay options such as `GOP_UI_DefaultSkin`.
> Outside Tessera's offline scope; recorded only so that an importer is not attempted.

> **OPEN-8:** Repeated mappings. BIN shows that the first `Mapping` of an art-asset value is
> kept (§4.4); a community validator states the last one wins (MUI). An owner test with two
> mappings of one art-asset value to visibly different files settles it.

> **OPEN-9:** Other art paths. Settled (§6.3): Image items with a DID go through the
> substitution; file-path Image items do not; Animation frames and Cursor images are never in
> the base table. Still open: (a) whether a Coloured-image overlay with a base-table DID (6
> such DIDs, 12 references) is substituted; (b) which UI art that players see is drawn
> without any layout media item and so cannot be skinned (community reports name the minimap
> rim, MUI); (c) whether the plugin API's game-styled controls (`lua-plugin-api.md` §14.2)
> take their art through the substitution.

**OPEN-10 (closed 2026-10-05).** The four values are the edge modes of `ui-layout.md` §4.4
fields 18–21 and §5.8; an applied override sets all four to 0 (§2, §8.4 step 3).

> **OPEN-11:** The effect of `UIComponent_UIScaler_Origin_X/Y` on UI scaling (§8.4 step 4).

---

<a id="s15"></a>
## 15. Change log

| Date | Change | Author role |
|---|---|---|
| 2026-10-05 | First version: discovery, definition grammar and parser rules, name resolution, base and game art tables, image precedence and placeholder, image formats (TGA, JPEG, DDS, IFF), panel-override matching and application, selection and activation, Tessera decisions, invariants, examples, corrections, open questions. Status Review. | Spec writer (dirty room) |
| 2026-10-05 | Revision answering the first review (changes requested). §8.4: the override does not write `has_position` (new SK33; correction row for `ui-layout.md` §5.3 and §9.8 in §13); step 3 restated as setting the four edge modes to 0; step 5 walks the resolved descriptor's children, inherited ones included; new paragraph on the two applications per child (SK35, SK36). §8.2: overrides act on a per-build resolved copy after base inheritance, reachable only from element building (SK25 revised, SK34), and the resulting placement in screen pixels (SK37). §2: "anchoring values" replaced by the edge modes of `ui-layout.md` §4.4/§5.8; new term "resolved descriptor"; header and LAY row re-pointed to `ui-layout.md` sections, new source tag ULS. §6.3 and §6.7: per-media-kind census (Animation frames and Cursor images never in the base table, file-path Image items bypass substitution; SK15 revised). §6.5: stored 8-bit texel values (SK19 revised). §7.1: image drawing per `ui-layout.md` §9.6. §7.4: full DDS header table with file offsets, size computation, linear-size rule, non-multiple-of-4 DXT behaviour (SK24 revised). SK8 and SK18 now cover the code-page narrowing and the 255-byte path cut. New D18, D19; new SI8, SI9, SP15–SP17; SP12 made exact; new examples §12.5, §12.6. OPEN-5 and OPEN-10 closed; OPEN-1, OPEN-2 and OPEN-9 narrowed. BIN questions 8–10 added. | Spec writer (dirty room) |
