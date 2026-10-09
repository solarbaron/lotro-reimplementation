<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Layered icons: properties, layer order and compositing

| | |
|---|---|
| **Status** | Accepted (2026-10-09: the P1 revision of §1 and §4 and its repair passed review; previously Accepted 2026-10-07 and 2026-10-06). Implementers re-check code built on §4 against the 2026-10-09 change-log rows. |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Re-review 2026-10-07 (a separate dirty-room review session, not the author): **accepted**. Full record moved to the review log (decision 0018). Review of the 2026-10-09 P1 diff (header, §4 answer, change-log row) on 2026-10-09 by a separate dirty-room review session, not the author: **changes requested**. Spot checks reproduce: in the `itemdisplay` layout record no element ID resolves to a quality-named `UIElementID` label, and the item-quality enum mapper of this install lists 1 Legendary, 2 Rare, 3 Incomparable, 4 Uncommon, 5 Common (agrees with `item-tooltips.md` IT7). **Blocking:** (1) the facts the §4 answer adds (no quality-dependent element or state in the item-slot template; quality selects a `ColorNameText` state for the item name through a native map of keys 1–5) have no row in the §1 provenance table and no mark, and §4 is headed "(Confirmed)" although the name-colour map rests on BIN of `item-tooltips.md`; add a row (sources `character-panels.md` CP7, `item-tooltips.md` IT7, with their marks, e.g. "Confirmed (layout); BIN (name-colour map)") and cite it from §4, and extend the change-log row and Status note to §1. Non-blocking: (a) `scenario-feed.md` §5.5 is not on this branch or on main (only on branch `p0`); merge `p0` first or the reference dangles; (b) both cited specs are still in Review (first versions 2026-10-09); revisit §4 if their reviews change CP7 or IT7; (c) the callout still opens with "**OPEN:**" although answered; the other answered items of this spec use "**Answered (date):**"; consider the same form so a search for open items does not find it. No §5.3 item other than provenance fails: functional facts only, no code, no game text, names used are interoperability labels read from the install. Re-review of the 2026-10-09 repair (diff against main: header, §1 row I15, §4 heading and answer, change-log rows) on 2026-10-09 by a separate dirty-room review session, not the author or the first reviewer: **accepted**. Blocking item (1) is resolved: row I15 cites `character-panels.md` CP7 and `item-tooltips.md` IT7 with marks that match theirs (Confirmed layout; BIN plus Confirmed data), §4 is headed with the BIN part and cites I15, and the change-log and Status name §1. Spot check: the install holds 302 UI layout records (prefix 0x22), as the answer's "all 302 layouts read" states. §5.3: functional facts only, no code or pseudo-code, no game text, the names used are interoperability labels read from the install, no closed gate. Non-blocking, carried over: (a) `scenario-feed.md` §5.5 is still only on branch `p0`; (b) CP7 and IT7 are in Review (first versions); revisit I15 and §4 if their reviews change them. |
| **Implements into** | `crates/material` (`icon` module: layered icon composition), explorer icon view |
| **Companion specs** | `property-system.md` §5–§6 (property sets; the 0x70 → 0x79 rule). `records.md` §5 (render surfaces). `texture-list.md` §6.1 (icons are single surfaces, never texture lists). `material.md` MA33 (no sRGB in any back end). `ui-layout.md` (UI layout elements, which may carry the background-override property of §5.6; written in parallel). |

---

<a id="s0"></a>
## 0. Purpose

Item icons, and a few other icon families, are not stored as finished images. A game
object's property set names several **layer** images (a background, an underlay, a drop
shadow, the picture itself, an overlay), and the UI stacks them. This spec defines:

- which properties carry the layer references, and what they point to (§2);
- the layer order, the canvas, the placement and the exact pixel arithmetic the original
  client uses to build the icon (§3);
- where an item's quality colour comes from (§4);
- the other icon families (skills, effects, traits, sockets, large images, overrides) (§5);
- the evidence behind the order and the arithmetic (§6).

All counts are observations from one install on one date. Tests assert the rules and
invariants, not the counts.

---

<a id="s1"></a>
## 1. Provenance

Legend: **Confirmed** = checked on the user's install by throwaway probes in the dirty-room
scratchpad. **BIN** = read from the client executable under method M4 (see the BIN row);
facts resting on BIN alone are marked BIN. **Reported** = from third-party material only.
**Inferred** = follows from the cited facts but was not observed directly.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04. All 384,950 property sets decoded: 383,332 with prefix 0x79 and 1,616 with prefix 0x78 in `client_gamelogic.dat`, and 2 with prefix 0x78 in `client_local_English.dat` (the first version said 384,948 and missed these 2; they hold no icon property), every one parsed to the last byte. Every image-valued property, top-level or nested, was resolved against the 0x41 headers of all 1,408,052 surfaces. The alpha channel of every distinct surface used as an icon layer was classified. About 60 item icons were composited and inspected by eye, including variants with different layer orders and size anchoring. |
| OBS2 | Observed in user install 2026-10-05 (repair pass after review). Every icon-layer and trait-icon value counted again over all 384,950 sets, separately at the top level and nested (§2.3, §5.3). The property sets (prefix 0x79) of the 15 progression objects named by `Trait_Icon_Progression` and `Trait_Rank_Overlay_Progression` decoded in full, and every surface they name checked against the 0x41 headers (§5.8). Rechecked 2026-10-06: which traits name which rank progression, with their `Trait_Virtue_Maximum_Rank`, and the last non-zero entry of each rank progression (§5.8). |
| BIN | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (`lotro_live`, SHA-256 prefix d22363eb9aff0fb5), read with objdump and capstone on address ranges in the dirty room on 2026-10-05. Entry points were found by searching the code for the property IDs of §2.2 as immediate operands. **Questions:** (a) in which order the client stacks the item layers, and what it does when a layer property is missing, zero or not a DID (§3.1, §3.2); (b) the canvas, the placement and clipping of layers of unequal size (§3.4); (c) the exact blend arithmetic and its colour space (§3.3); (d) which override properties the client reads (§5.6); (e) whether item quality enters the composition (§4); (f) the layer order of the generic examination icon used for skills and traits (§5.3). **Why M1–M3 were not enough:** the data holds only the layer references; order between Underlay and Shadow, placement of unequal sizes, rounding and colour space are code; the override properties never occur in static data; no published source states any of it (M2); no running client or screenshot comparison was available to this session (M3). (g) Repair pass, 2026-10-05: how the client turns a trait's current rank into an entry of an icon progression (§5.8); M1 gave the layout and LC a rule, but not the client's own index or out-of-range handling. The virtue-panel code that reads `Trait_Icon_Progression` was located from its property ID, but the index computation was not traced to a conclusion, so **no fact in this spec rests on (g)**. (Corrected 2026-10-07 by BIN2 (h): that code uses the number as a UI element identifier, not as a property ID.) Only offline UI code was read; nothing about network, login, launcher, keys or anti-tamper (decision 0004). Listings stay in the dirty scratch area. |
| OBS3 | Observed in user install 2026-10-07 by a throwaway probe: the client class library (DID 0x56000000, `wlib.md`) read, its bytecode chunk (`wlib.md` §4.1) located by its length word, and the chunk searched for each icon-related property ID as a little-endian 32-bit value (§5.6, §5.8). |
| BIN2 | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), second pass.** Same client build 4906.0071.1794.4035, analysed headless in Ghidra (read-only copy of the shared project) on 2026-10-07. **Questions:** (h) does the native code read `Trait_Icon_Progression`, `Trait_Rank_Overlay_Progression`, `Progression_MinimumIndexValue` or `PropertyProgression_Array`, so that the rank-to-entry rule of §5.8 can be read there; (i) does the native UI code use `Item_Quality` for anything near the item icon (§4). **Method:** a search of all instructions and of all initialised data for those property IDs as 32-bit values, and decompilation of every function that holds a hit. **Why M1–M3 were not enough:** the data holds the progressions but not the code that indexes them; no published source describes it; no running client was available to this session. **Result:** see I13 and §4. Only offline UI code was read; nothing about network, login, launcher, keys or anti-tamper (decision 0004). Listings stay in the dirty scratch area. |
| LC | LotroCompanion `lotro-companion` (GPL-3.0) and `lotro-tools` sources: the layer property names an item reader uses, the order in which it stacks them, and its alpha rule. Facts only. |
| DS | `research/notes/data-semantics.md` §7.4 (list of `Icon_Layer_*` names). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| I1 | Item icon layers are DID-valued properties named `Icon_Layer_*` (§2.2) | LC, DS; OBS; BIN (a) | Confirmed |
| I2 | Every icon-layer value is a 0x41 surface DID, or 0 meaning "no layer" (§2.3) | OBS (all 550,690 layer values, 521,824 of them non-zero) | Confirmed |
| I3 | Layer order: Background, Underlay, Shadow, Image, Overlay (§3.2) | BIN (a) (sort keys 1000, 2000, 3000, 4000, 6000); OBS (§6.2); LC | Confirmed |
| I4 | Layers are combined with the client's integer source-over rule on stored 8-bit values (§3.3) | BIN (c); OBS (straight alpha, §6.1); `material.md` MA33 | BIN (exact rule); straight alpha Confirmed |
| I5 | Canvas = the bottom layer's own size and format; every other layer is placed at the top-left corner and clipped at the right and bottom (§3.4) | BIN (b) | BIN |
| I6 | Item quality is expressed by which background surface the item names; the composition reads no quality property (§4) | OBS (correlation over 150,605 items); BIN (e) | Confirmed |
| I7 | Skill, effect and most other icons are single surfaces (§5) | LC; OBS | Confirmed |
| I8 | Trait icons stack a static overlay frame above the picture, and a rank overlay above both (§5.3) | LC; OBS (1,258 traits); BIN (f) (examination icon keys 4000, 5000, 6000) | Confirmed (order); rank selection OPEN |
| I11 | `Trait_Icon_Progression` and `Trait_Rank_Overlay_Progression` name progression objects whose 0x79 property set holds `Progression_MinimumIndexValue` and an array `PropertyProgression_Array` of `Trait_Icon` surface DIDs (§5.8) | OBS2 (all 15 progression objects named; all 1,102 non-zero references) | Confirmed (layout) |
| I12 | Array entry *i* of such a progression is the value for index (minimum + *i*); an absent minimum counts as 0 (§5.8) | `combat-data.md` §4.3, C6, C8 (the general rule for property-array progressions); LC; OBS2 (consistent: the 13 virtue icon progressions have 16 entries, minimum 1, for traits whose maximum rank is 16) | Layout Confirmed; use as the rank index Reported |
| I13 | The native code never reads `Trait_Icon_Progression` or `Trait_Rank_Overlay_Progression` as properties, and evaluates no progression (`combat-data.md` C25); the client library's bytecode holds the IDs of `Trait_Icon`, `Trait_Static_Icon_Overlay`, `Trait_Icon_Progression`, `Trait_Rank_Overlay_Progression` and `Trait_Virtue_Maximum_Rank` as 32-bit literals, so the trait icon and its rank selection are most likely computed in script (§5.8; each literal could also be a chance match) | BIN2 (h); OBS3 | BIN (native); literals Confirmed; script conclusion Inferred |
| I14 | The ID of `Icon_Layer_ImageOverrideDID` occurs once as a literal in the client library's bytecode, though nowhere in the executable (§5.6) | OBS3; BIN (d) | Confirmed (occurrence); use OPEN |
| I9 | An item shows no icon at all when it lacks the Image property (or, for an instance, the checkpointed override) or the Background property (§3.1) | BIN (a) | BIN |
| I15 | The item-slot template (`ItemDisplay_SlottedTemplate` of the `itemdisplay` layout) has no quality-dependent element or state; item quality instead selects a state of the `ColorNameText` element for the item name, through a native map of the quality keys 1–5 (§4) | `character-panels.md` CP7 (Confirmed layout); `item-tooltips.md` IT7 (BIN + Confirmed data) | Confirmed (layout); BIN (name-colour map) |
| I10 | `Icon_Layer_ImageOverrideDID_Checkpointed` replaces `Icon_Layer_ImageDID` for item instances; the UI element property `UI_ItemIconDisplay_OverrideBackground` replaces the Background; no other `…Override…` or glow property is read by the client (§5.6) | BIN (d) | BIN |

---

<a id="s2"></a>
## 2. Where the layer references live

<a id="s2-1"></a>
### 2.1 Records

An item, skill, effect or trait is a script-state object (prefix 0x70). Its static
properties are in the property set with the same low 24 bits and prefix 0x79
(`property-system.md` §6.2). Some stand-alone UI tables use prefix-0x78 property sets.
Icon properties may sit at the top level of a set, or inside struct and array values
(for example, socket-type tables list background and overlay pairs, §5.4).

The client builds an item icon from one of two property sources (BIN (a)): the **static**
properties of the item's DID (the 0x79 set), or the properties of a live **item instance**
(the same names, values possibly set at run time). Offline, Tessera has only the static source.

Property values are resolved **by name** through the master property dictionary
(`property-system.md` §2). The names below are interoperability identifiers. The property
IDs observed in this install are given for test convenience only, and code must look
them up by name.

<a id="s2-2"></a>
### 2.2 The item layer properties (Confirmed)

All have property type 20 (DID reference).

| Name | Observed PID | Top-level sets that have it | Non-zero values | Role |
|---|---|---|---|---|
| `Icon_Layer_BackgroundDID` | 0x10000F9F | 151,977 | 151,977 | Bottom layer: tile plus frame, colour per quality (§4) |
| `Icon_Layer_UnderlayDID` | 0x10000685 | 107,598 | 81,465 | Coloured glow behind the picture |
| `Icon_Layer_ShadowDID` | 0x10001070 | 121,101 | 121,101 | Dark drop shadow behind the picture |
| `Icon_Layer_ImageDID` | 0x10000E9C | 151,557 | 151,557 | The picture |
| `Icon_Layer_OverlayDID` | 0x100030C9 | 15,422 | 15,422 | Top marker (for example a small tier numeral in a corner) |
| `Icon_Layer_LargeImageDID` | 0x10006C7A | 2,936 | 203 | Separate 64×64 picture for large displays (§5.5). Not part of the 32×32 stack. |
| `Icon_Layer_ImageOverrideDID` | 0x1000144F | 26 | 26 | Not read by the client (§5.6) |
| `Icon_Layer_ImageOverrideDID_Checkpointed` | 0x10006856 | 0 | 0 | Replaces the Image for item instances (§5.6) |

These names are defined in the dictionary but are **never set** in any static property set
of this install, and the client's code never reads them (§5.6): `Icon_Layer_BackgroundOverrideDID`,
`Icon_Layer_UnderlayOverrideDID`, `Icon_Layer_ShadowOverrideDID`, `Icon_Layer_LargeImageGlowDID`.

One property of a **UI layout element**, not of the item, also takes part (§5.6):
`UI_ItemIconDisplay_OverrideBackground` (observed PID 0x10001CBF, type 20).

Layer combinations among the 151,977 top-level sets that have a background (a value
counts as present when non-zero):

| Background | Underlay | Shadow | Image | Overlay | Sets |
|---|---|---|---|---|---|
| ✓ | ✓ | ✓ | ✓ | | 81,424 |
| ✓ | | ✓ | ✓ | | 39,671 |
| ✓ | | | ✓ | ✓ | 15,416 |
| ✓ | | | ✓ | | 14,999 |
| ✓ | | | | | 420 |
| ✓ | ✓ | | ✓ | | 41 |
| ✓ | | ✓ | ✓ | ✓ | 6 |

Overlay and Underlay never occur together. Every set with an Image also has a Background.
The 420 sets with only a Background have no Image property at all, so the client shows no
icon for them (§3.1).

<a id="s2-3"></a>
### 2.3 What the values point to (Confirmed)

- Every non-zero layer value names an existing **0x41 surface**: a single image with no
  mip chain and no texture list (`texture-list.md` §6.1). No layer value names a 0x40 list
  or any other prefix.
- **0 means "no layer"**. Explicit zeros occur: 26,133 Underlay values and 2,733
  LargeImage values are 0.
- Every layer surface has surface category 15 (UI image).
- One fully transparent 32×32 A8R8G8B8 surface (alpha 0 at every pixel) is used as a
  placeholder: it is the Underlay value of 23,402 sets and the Shadow value of 38,396.
  Compositing it changes nothing (§3.3: a source alpha of 0 leaves the pixel unchanged), so
  no special case is needed. A cache may still recognise it and skip the work.
- Shadow and Underlay name the same surface in 23,324 sets. In 23,222 of them that
  surface is the transparent placeholder.

Layer surfaces by format and size. Every count in this table is over **top-level** values
only (the values that §3.1 reads); nested values are listed below it.

| Layer | Distinct surfaces | Formats (references) | Sizes |
|---|---|---|---|
| Background | 52 | R8G8B8 145,323; A8R8G8B8 6,654 | 32×32 always |
| Underlay | 2,206 | A8R8G8B8 81,419; R8G8B8 46 | 32×32 |
| Shadow | 5,087 | A8R8G8B8 120,926; R8G8B8 175 | 32×32, a few 36×36, 32×30, 128×128 |
| Image | 15,487 | A8R8G8B8 145,254; R8G8B8 6,303 | 32×32 in 99.8%; the rest 16×8 to 36×36 |
| Overlay | 16 | A8R8G8B8 15,422 | 32×32 |

Nested values (inside struct and array values, §5.4): 24 Background values (R8G8B8 16,
A8R8G8B8 8) and 49 Overlay values (all A8R8G8B8), all 32×32 and non-zero. With them the
distinct counts become 70 backgrounds and 24 overlays. (The first version mixed scopes in the
Background row: its format counts, 145,339 / 6,662, included the nested values, while its
distinct count and sizes did not.)

An R8G8B8 surface is fully opaque (`records.md` §5.3). An R8G8B8 layer above the background
replaces everything below it inside its rectangle.

<a id="s2-4"></a>
### 2.4 Example (synthetic)

An item whose 0x79 set contains:

| Property | Value |
|---|---|
| `Icon_Layer_BackgroundDID` | 0x41AAAA01 (32×32 R8G8B8: framed tile in the quality colour) |
| `Icon_Layer_UnderlayDID` | 0x41AAAA02 (32×32 A8R8G8B8: soft blue glow, alpha high around the object outline) |
| `Icon_Layer_ShadowDID` | 0x41AAAA03 (32×32 A8R8G8B8: black, alpha follows the object silhouette with a soft edge) |
| `Icon_Layer_ImageDID` | 0x41AAAA04 (32×32 A8R8G8B8: the object, alpha 0 around it) |
| `Icon_Layer_OverlayDID` | 0 |

The icon is the 32×32 R8G8B8 result of stacking 01, 02, 03, 04 in that order (§3).

---

<a id="s3"></a>
## 3. Compositing

<a id="s3-1"></a>
### 3.1 Inputs: which layers exist (BIN (a); normative)

The client builds an item's layer stack from the properties like this. "Read *P*" means:
look up property *P*; if it is absent the result is **absent**; if it is present and of type
20 the result is its DID; if it is present with any other type the result is 0.

1. **Image.** For an item instance, read `Icon_Layer_ImageOverrideDID_Checkpointed`; if it is
   absent (or for static item data), read `Icon_Layer_ImageDID`. If the result is absent,
   the item has **no icon**; stop. Otherwise the Image layer is that DID (even 0).
2. **Shadow.** Read `Icon_Layer_ShadowDID`. If the result is absent or 0, there is no Shadow
   layer; otherwise the Shadow layer is that DID.
3. **Underlay.** The same with `Icon_Layer_UnderlayDID`.
4. **Background.** If the UI element that shows the icon carries
   `UI_ItemIconDisplay_OverrideBackground`, the item's Background property is **not read**:
   a non-zero override value becomes the Background layer, and an override of 0 adds no
   Background layer (the client leaves a Background left over from the element's previous
   icon in place; a new element has none). Otherwise read `Icon_Layer_BackgroundDID`; if the result is absent the
   item has **no icon**; stop. Otherwise the Background layer is that DID (even 0).
5. **Overlay.** As step 2, with `Icon_Layer_OverlayDID`.

Then compose (§3.2–§3.4). During composition a layer whose DID is 0, or whose surface cannot be
found or decoded, is **skipped**, except the bottom layer: if the bottom layer is 0 or cannot
be loaded, no icon is produced.

Each surface is decoded to its stored pixels (`records.md` §5.3). R8G8B8 has no alpha and
counts as alpha 255 wherever an alpha is needed. Icon layers use only R8G8B8 and A8R8G8B8
(§2.3). A Tessera reader that meets another format decodes it to straight-alpha RGBA8 and
treats it like A8R8G8B8.

Tessera logs every skipped layer. An icon with no layers left is "no icon".

<a id="s3-2"></a>
### 3.2 Order (bottom to top) (Confirmed, BIN)

The client keeps the layers of one icon in a list sorted by an integer key, ascending, and
draws them in that order:

| Order | Layer | Key |
|---|---|---|
| 1 (bottom) | Background | 1000 |
| 2 | Underlay | 2000 |
| 3 | Shadow | 3000 |
| 4 | Image | 4000 |
| 5 | (static overlay of the examination icon, §5.3) | 5000 |
| 6 (top) | Overlay | 6000 |

Each key holds at most one layer; setting a key again replaces its DID. The keys are
interoperability facts only in the sense of fixing the order; Tessera needs only the order.

<a id="s3-3"></a>
### 3.3 Pixel arithmetic (BIN (c); normative)

**Colour space.** The client composes on the stored 8-bit values. It never converts to linear
light, and no graphics back end shows the result through an sRGB view (`material.md` MA33).
Tessera must not linearise either.

**Bottom layer.** The canvas starts as an exact copy of the bottom layer's pixels, in that
layer's format (§3.4). No blending happens for the bottom layer. (The client multiplies it by
a per-layer tint colour that icons always set to white; with white the multiply is the
identity.)

**Every further layer**, in order, is blended into the canvas pixel by pixel inside the
overlap rectangle of §3.4. Let *S* be a source colour channel and *s* the source alpha (255
for R8G8B8), *D* the canvas channel and, for a canvas with alpha, *d* the canvas alpha. All
values are integers 0–255. T(*x* / *y*) is integer division truncated toward zero (the
numerator may be negative).

1. If *s* = 0: the canvas pixel is unchanged.
2. Let *a* = *s* + 1.
3. If *a* = 256 (*s* = 255): the canvas pixel becomes the source pixel (colour, and alpha 255
   if the canvas has alpha).
4. **Canvas without alpha** (bottom layer R8G8B8): each channel becomes
   *D* − T((*D* − *S*) · *a* / 256).
5. **Canvas with alpha** (bottom layer A8R8G8B8):
   - *d* = 0: the canvas pixel becomes the source pixel, colour and alpha *s*;
   - *d* = 255: each channel becomes *D* − T((*D* − *S*) · *a* / 256); alpha stays 255;
   - otherwise let *q* = (*d* + 1) + *a* − T((*d* + 1) · *a* / 256). Each channel becomes
     *D* − T((*D* − *S*) · *a* / *q*), and the alpha becomes *q* − 1.

The constants 256, the "+ 1" on alphas and the truncation are the client's (decision 0012).
This is straight-alpha source-over: in real numbers it is the formula
*r* = *s* + *d*(1 − *s*), colour = (*S*·*s* + *D*·*d*·(1 − *s*)) / *r*. Compared with that
formula rounded to nearest, the integer rule differs by at most 1 on a canvas without alpha;
on a canvas with alpha it differs by at most 2 in alpha, and in colour by at most 2 where the
canvas alpha is 128 or more but by up to 23 where the canvas alpha is very small (every source
colour, source alpha and canvas colour, at canvas alphas 1, 2, 4, 8, 16, 32, 64, 128, 200, 250
and 254; dirty-room probe). Tests therefore
compare the integer rule exactly.

Worked examples (synthetic):

| # | Canvas pixel | Source pixel (R, G, B, A) | Result |
|---|---|---|---|
| 1 | R8G8B8 (200, 100, 50) | (0, 0, 255, 128) | *a* = 129: R 200 − T(25,800/256) = 200 − 100 = **100**; G 100 − 50 = **50**; B 50 − T(−26,445/256) = 50 + 103 = **153** |
| 2 | A8R8G8B8 (100, 100, 100, 100) | (200, 0, 0, 100) | *a* = 101, *q* = 101 + 101 − T(10,201/256) = 202 − 39 = 163: R 100 − T(−10,100/163) = 100 + 61 = **161**; G and B 100 − 61 = **39**; alpha **162** (real-number formula: 162.2, 37.8, 37.8, alpha 160.8) |
| 3 | A8R8G8B8 (10, 20, 30, 255) | (250, 0, 0, 64) | *a* = 65: **70, 15, 23**, alpha 255 |
| 4 | A8R8G8B8 (50, 60, 70, 0) | (1, 2, 3, 77) | canvas alpha 0: **1, 2, 3**, alpha 77 |
| 5 | any | (…, …, …, 0) | unchanged |
| 6 | any | (9, 8, 7, 255) or R8G8B8 (9, 8, 7) | **9, 8, 7** (alpha 255 if the canvas has alpha) |

The output alpha is below 255 only where the canvas started with alpha below 255, which
happens only with the 10 backgrounds that have transparent pixels (§6.1). An icon whose
bottom layer is R8G8B8 is opaque everywhere.

<a id="s3-4"></a>
### 3.4 Canvas size and placement (BIN (b); normative)

- **Canvas** = the bottom layer's width, height and format. For item icons the bottom layer
  is the Background whenever one exists (§3.1), so the canvas is the Background's 32×32.
- Every further layer is placed with its **top-left corner on the canvas's top-left corner**,
  offset (0, 0). It covers the rectangle of width min(layer width, canvas width) and height
  min(layer height, canvas height). Parts of a larger layer beyond the canvas's right and
  bottom edges are dropped; a smaller layer leaves the rest of the canvas untouched.
- No layer is scaled.

In 151,759 of 151,977 item sets (99.86%) all present layers have the same size, in practice
32×32, and placement does not matter. The other 218 have an Image or Shadow of another size:
smaller (for example 28×24 or 16×8) or larger (35×35, 36×36). With the client's rule a small
picture sits in the top-left corner of the tile and a large one loses its right and bottom
edge. (The 2026-10-04 version of this spec chose centring because it looked better in the
dirty-room composites; that choice is withdrawn, because the client does not centre.)

Example (synthetic): a 4×4 R8G8B8 background and a 2×2 opaque image: the image covers canvas
pixels (0, 0)–(1, 1). A 6×6 image on the same background covers the whole canvas with its own
pixels (0, 0)–(3, 3); its last two columns and rows are dropped.

---

<a id="s4"></a>
## 4. Quality colour (Confirmed; name-colour map BIN, I15)

No layer property is derived from item quality at run time, and the client's composition
reads no quality property (BIN (e)). Each item names a background surface, and those
surfaces differ in colour by quality. Over the 150,605 sets that have both `Item_Quality` (an
enum value) and a Background:

| Quality value | Items | Distinct backgrounds | Share of the most common background |
|---|---|---|---|
| 0 | 1,839 | 2 | 96.1% |
| 1 | 8,238 | 14 | 74.8% |
| 2 | 45,577 | 24 | 80.6% |
| 3 | 29,227 | 20 | 83.0% |
| 4 | 33,424 | 15 | 85.9% |
| 5 | 32,300 | 21 | 33.3% |

Each quality value's most common background is a different surface (6 values, 6 surfaces).
The other backgrounds are category-specific tiles (for example wood-grain tiles), and those
are shared across qualities. So:

- a renderer draws the Background the item names, and never substitutes or tints it by
  quality;
- a tool that wants a quality border for an object with no Background has to supply its own.

> **Answered (2026-10-09):** Question was: whether a UI layout draws a separate quality frame around item slots, outside the
> composed icon. The icon composition itself has none (BIN (e)); any such frame would come
> from UI layout data (`ui-layout.md`). BIN2 (i): the ID of `Item_Quality` occurs in 9
> instructions in 9 native functions. What holds for all nine: each either reads the item's
> quality value or uses the ID (or a neighbouring number) as a key for a UI object, and none of
> them is on the icon composition path of §3. Beyond that they differ: some only read the value
> and return it unchanged, some map it through a fixed table to a UI resource identifier or a
> sort rank, and two pass the ID itself, or a neighbouring number, to a UI object as a key
> without reading a quality. What the mapped identifiers style (name colour, a slot frame or
> something else) was not traced; a frame capture of an inventory slot (M3) or a trace of the
> callers would settle it.
>
> *Answer (2026-10-09, P1; I15):* the item-slot template that bags, the paperdoll, vendor and
> loot lists use (`ItemDisplay_SlottedTemplate` of the `itemdisplay` layout) has no
> quality-dependent element or state; its overlays are wear, cooldown, selection and similar
> states (`character-panels.md` §5, layout data, all 302 layouts read). Quality colours the
> item's **name**: the quality-to-state native maps keys 1–5 to the item-quality states of
> the `ColorNameText` colour table (`item-tooltips.md` §6.2; this map is BIN, the
> template result is Confirmed layout data). So the background
> tile stays the only quality cue of an icon. In Tessera's sandbox the icons shown are those
> of the item records the scenario feed places (`scenario-feed.md` §5.5; that spec is on branch `p0` until it is merged).

---

<a id="s5"></a>
## 5. Other icon families

All values below are type-20 properties naming single 0x41 surfaces, with category 15, except
the two trait progression properties, which name progression objects (§5.8). They compose with
the §3 rules.

<a id="s5-1"></a>
### 5.1 Skills (Confirmed)

| Property | Sets | Sizes |
|---|---|---|
| `Skill_Icon` | 13,247 | 32×32 (13,008); 71×69 (239) |
| `Skill_LargeIcon` | 13,247 | Same DID as `Skill_Icon` in all 13,247 |
| `Skill_SmallIcon` | 13,247 | Same DID as `Skill_Icon` in 13,008. In the 239 with a 71×69 `Skill_Icon` it is a separate 32×32 surface. |

A skill icon is a single layer, with no stack. Of 2,701 distinct `Skill_Icon` surfaces,
1,900 have binary alpha, 335 are fully opaque, and 466 have soft alpha.

<a id="s5-2"></a>
### 5.2 Effects (Confirmed)

`Effect_Icon`: 22,804 sets. Single layer, 32×32 in 99.9%; a few range from 30×29 to 50×50.

<a id="s5-3"></a>
### 5.3 Traits and the examination icon

| Property | Sets | Notes | Mark |
|---|---|---|---|
| `Trait_Icon` | 3,950 sets at the top level (all non-zero) | Base picture, 32×32. A further 368 values sit inside the arrays of 15 progression sets (§5.8), 70 of them 0; in all 4,318 values in 3,965 sets. | Confirmed |
| `Trait_Static_Icon_Overlay` | 1,258 | 32×32 frame: opaque border, transparent centre. Drawn **above** `Trait_Icon` (the reverse order would hide the picture). | Confirmed |
| `Trait_Rank_Overlay_Progression` | 1,260 (171 are 0) | Type 20. The 1,089 non-zero values name 0x70 progression objects (only 2 distinct), not surfaces (§5.8). Each maps a rank to an overlay surface, drawn above the static overlay. | Layout Confirmed; rank → entry Reported (LC) |
| `Trait_Icon_Progression` | 13 | Type 20. Each value names a 0x70 progression object (13 distinct) that maps a rank to the trait's **base picture**; none of the 13 sets has a top-level `Trait_Icon` (§5.8). | Layout Confirmed; role OPEN (§5.8) |

**Examination icon (BIN (f)).** Shortcut-style slots that show a skill, trait or similar
entry build their icon from three fields of the entry's examination data: the icon at key
4000, the static icon overlay at key 5000 and the icon overlay at key 6000 (§3.2), composed by
§3.3–§3.4 with the icon as the bottom layer. When the examination data marks the entry as an
item, the item rules of §3.1 are used instead. The trait properties above are the natural
source of those three fields (picture, static frame, rank overlay; Inferred from the
parallel names; the client code that fills the examination data was not traced).

Trait order: `Trait_Icon` (bottom), then `Trait_Static_Icon_Overlay`, then the rank overlay
(top). Confirmed: data (§5.3 table) and BIN (f).

The progression layout and what is known about rank selection are in §5.8.

<a id="s5-4"></a>
### 5.4 Socket and nested layers (Confirmed)

24 Background values and 49 Overlay values occur inside array or struct values, not at
the top level of a set: for example per-socket-type entries with a Background/Overlay pair,
and level-keyed overlay entries. Where a Background and an Overlay appear in the same
entry, they compose in the §3.2 order (Background below Overlay). When an item's Background
or Overlay property changes at run time to a non-zero DID, the client replaces just that layer
(key 1000 or 6000) (BIN (a)).

<a id="s5-5"></a>
### 5.5 Large item image (Confirmed)

`Icon_Layer_LargeImageDID`: 203 non-zero values, all 64×64 A8R8G8B8 with soft alpha. It is
a separate single-layer picture for large displays, never stacked with the 32×32 layers. The
client reads it on its own (BIN (d)).

<a id="s5-6"></a>
### 5.6 Overrides (BIN (d))

| Property | Read by the client | Effect |
|---|---|---|
| `Icon_Layer_ImageOverrideDID_Checkpointed` | Yes, for item instances | When present, used **instead of** `Icon_Layer_ImageDID` (§3.1 step 1), whatever its value: an override of 0 gives an icon without a picture. Never present in static data. |
| `UI_ItemIconDisplay_OverrideBackground` (on a UI layout element) | Yes | When the element has it, the item's Background property is not read; a non-zero value is the Background layer (§3.1 step 4). |
| `Icon_Layer_ImageOverrideDID` | **No** (native code) | Its PID occurs nowhere in the executable, so the native icon composition of §3 never reads it. The 26 static sets that carry it have no Image or Background property, so that composition gives **no icon** for them. Its PID does occur once as a literal in the client library's bytecode (I14); whether script code uses it (for example to fill examination data) is OPEN, see §5.8's last callout. |
| `Icon_Layer_BackgroundOverrideDID`, `Icon_Layer_UnderlayOverrideDID`, `Icon_Layer_ShadowOverrideDID`, `Icon_Layer_LargeImageGlowDID` | **No** | Never referenced as properties by the client's code (two of these numbers occur only as unrelated UI element identifiers). |

There is no Overlay override.

<a id="s5-7"></a>
### 5.7 Other single-image properties (Confirmed, informative)

About 100 further DID properties whose names contain "Icon" or "Image" point directly at
category-15 surfaces. Examples: title, set, area, emote, hobby, perk, race and class icons;
map-note images (normal, highlighted, selected, menu); map images (JPEG); tutorial and
pop-up pictures (up to 384×512). Each is one surface, shown as is. When a family has
explicit small and large variants, each variant is its own property and surface. Nothing is
scaled from a single source.

<a id="s5-8"></a>
### 5.8 Icon progressions (layout Confirmed, OBS2; index rule from `combat-data.md`; rank selection OPEN)

Two trait properties name a **progression object** (prefix 0x70) instead of a surface:
`Trait_Icon_Progression` (13 sets) and `Trait_Rank_Overlay_Progression` (1,089 non-zero
values). The progression's data is in the property set with the same low 24 bits and prefix
0x79 (`property-system.md` §6.2). For all 15 progression objects named by these two
properties, that set has exactly two top-level properties:

| Property (interop name) | Type | Meaning |
|---|---|---|
| `Progression_MinimumIndexValue` | integer | Index of the first array entry. Observed 1 (14 sets) and 0 (1 set). |
| `PropertyProgression_Array` | array | Entries in stored order. Here every entry is a `Trait_Icon` property (type 20) whose value is a 0x41 surface DID or 0. |

Observed contents:

| Named by | Distinct objects | Entries | Minimum | Non-zero entries | Surfaces |
|---|---|---|---|---|---|
| `Trait_Icon_Progression` | 13 (one per set) | 16 each | 1 | all 16 | 32×32 A8R8G8B8, category 15 |
| `Trait_Rank_Overlay_Progression` | 2 | 80 each | 1 and 0 | minimum 1: entries 0–58 (59); minimum 0: entries 0–30 (31); every later entry is 0 | 32×32 A8R8G8B8, category 15 |

In 11 of the 13 icon progressions the 16 entries hold 5 distinct surfaces, and entries 5–15
all repeat entry 4; one holds a single surface 16 times, and one holds 15 distinct surfaces.
The 70 zero `Trait_Icon` values counted in §5.3 are the trailing entries of the two rank
progressions (21 + 49). All 13 traits with `Trait_Icon_Progression` have
`Trait_Virtue_Maximum_Rank` = 16 and no top-level `Trait_Icon`. The traits that name the
rank progression with minimum 1 have maximum rank 50 (96 traits) or 60 (2); those that name
the one with minimum 0 have maximum rank 1 to 7 (991 traits).

**Index rule (`combat-data.md` §4.3; use as the rank index Reported, LC).** These are ordinary
property-array progressions, evaluated as `combat-data.md` §4.10 describes: array entry *i*
(counting from 0) is the value for index *m* + *i*, where *m* is
`Progression_MinimumIndexValue`, and **0 when the property is absent** (the dictionary default,
`combat-data.md` C6; LC assumes 1, which the earlier version of this section repeated). It is
never absent in these 15 objects. An index outside the covered range is clamped to the first
or last entry by Tessera's Decision D1 (`combat-data.md` §4.8), not by a known client rule.
The data agrees with "index = rank" for the icon progressions (16 entries from index 1 for
maximum rank 16). For the rank progressions, under that rule:

| Rank progression | `Trait_Virtue_Maximum_Rank` of the traits naming it | Entries ranks 1 to that maximum select | Non-zero entries | Ranks that fall on a 0 entry |
|---|---|---|---|---|
| minimum 1 (98 traits) | 50 (96 traits), 60 (2 traits) | rank − 1, so 0 to 59 | 0–58 (ranks 1–59) | rank 60 only (entry 59), reachable only by the 2 traits with maximum rank 60 |
| minimum 0 (991 traits) | 1 to 7 | rank, so 1 to 7 (entry 0 = rank 0) | 0–30 (ranks 0–30) | none |

So the only data case in which the reported rule selects a 0 entry is rank 60 of the
minimum-1 progression. Whether the client then draws no rank overlay (a 0 value is "no
layer", §3.1) or falls back to another entry is part of the OPEN below.

**Offline use.** A trait's current rank is per-character state, not static data. A Tessera
viewer that shows a trait without a character uses, for `Trait_Icon_Progression`, the entry
for the lowest index (entry 0), and shows no rank overlay; it labels both as a viewer choice.
A 0 entry means "no layer" (§3.1).

**Where the client does it (I13).** The native code never reads `Trait_Icon_Progression` or
`Trait_Rank_Overlay_Progression`: the number of the first occurs in the executable only as a
UI element identifier (in two functions of one UI panel; UI element identifiers share the
0x1000xxxx number space with property IDs), and the number of the second occurs nowhere in it (BIN2 (h)). The
native code does not evaluate progressions at all (`combat-data.md` C25). The IDs of
`Trait_Icon` (twice), `Trait_Static_Icon_Overlay`, `Trait_Icon_Progression`,
`Trait_Rank_Overlay_Progression` and `Trait_Virtue_Maximum_Rank` (once each) do occur as
32-bit literals in the bytecode chunk of the client class library (OBS3). From the negative
native search and these literals it is **Inferred**, not observed, that the trait icon, its
rank selection and the three examination-icon fields of §5.3 are computed by script code,
whose instruction set is not specified (`wlib.md` §4.1, `combat-data.md` §13.2). The literals
alone do not prove it: a 4-byte literal can match by chance, about once in 2,000 IDs for a
bytecode block of this size (`combat-data.md` §13.2), and each of these IDs occurs only once
or twice. Only reading the script functions that hold them would confirm it. (The earlier
BIN (g) note that "the virtue-panel code that reads `Trait_Icon_Progression`" had been found is
withdrawn: that code uses the number as a UI element identifier.)

> **OPEN:** (needs a bytecode spec, `combat-data.md` §13.2) How the client turns a trait's
> current rank into an entry: whether the index is the rank itself (so entry = rank − minimum,
> as LC reports), what it does when the index is below the minimum or past the last entry,
> and whether a 0 entry leaves the trait with no base picture (icon progression) or no rank
> overlay (rank progression; the one data case is rank 60 of the minimum-1 progression) or
> falls back to another value. Also OPEN: whether `Trait_Icon_Progression`, when present,
> replaces a top-level `Trait_Icon` (no set has both, so the data cannot tell), which rank
> value a trait that is not yet slotted uses, and whether the script code that fills
> examination data reads `Icon_Layer_ImageOverrideDID` (I14). The native code cannot answer
> any of these (I13); the script functions that hold the literals most likely can (Inferred, I13).

---

<a id="s6"></a>
## 6. Evidence for the order and the alpha rule

<a id="s6-1"></a>
### 6.1 Alpha statistics per layer (Confirmed, every distinct layer surface)

Counts are over the distinct surfaces named by top-level values (§2.3).

| Layer | A8R8G8B8 surfaces | Of those: soft alpha | Fully transparent | Notes |
|---|---|---|---|---|
| Background | 18 | 10 | 0 | The other 34 backgrounds are R8G8B8 (opaque) |
| Underlay | 2,205 | 2,203 | 1 | Saturated colour where alpha > 0 |
| Shadow | 5,082 | 5,055 | 7 | 4,832 have black RGB wherever alpha > 0 |
| Image | 14,965 | 14,467 | 2 | |
| Overlay | 16 | 16 | 0 | |

Straight (not premultiplied) alpha: in 2,123 of the 2,203 soft-alpha underlays, and in all
86 large images, some pixel has a colour channel above its alpha. Premultiplied data cannot
do that. The client's blend (§3.3) also treats colours as straight.

<a id="s6-2"></a>
### 6.2 Order (each position separately)

| Claim | Evidence | Mark |
|---|---|---|
| Background is the bottom layer | 145,323 of the 151,977 top-level Background values are R8G8B8, which is opaque everywhere. Any layer drawn under one would never be visible. BIN: key 1000, the lowest. | Confirmed |
| Image is above Shadow | In 2,000 sampled items, 81% of the image's fully opaque pixels lie under shadow pixels with alpha > 0.5. Shadows are black, so drawing the shadow on top would blacken most of the picture. BIN: keys 3000 < 4000. | Confirmed |
| Overlay is the top layer | Over 15,422 items, 77% of the overlay's opaque pixels (the numeral) lie over fully opaque image pixels. Below the image, the numeral would be invisible. BIN: key 6000, the highest. | Confirmed |
| Underlay is below Shadow | BIN: keys 2000 < 3000. Agrees with LC. Data alone could not decide (the two orders change about 23% of pixels in icons that have both, and both looked plausible). | Confirmed (BIN) |

<a id="s6-3"></a>
### 6.3 Visual check

About 60 icons from every layer combination of §2.2 were composited and inspected by eye at
4–5× zoom (2026-10-04, before the BIN pass; then with centring of unequal sizes, now
replaced by §3.4). Every result looked like a finished icon: frame visible, no dark halo
over the picture, overlay numerals legible. Background colour also matched the item's
quality across samples. The images were kept in the scratchpad only.

---

<a id="s7"></a>
## 7. Invariants

| # | Invariant | Status |
|---|---|---|
| IL1 | Every non-zero `Icon_Layer_*` value names an existing 0x41 surface | Confirmed (521,824 non-zero values) |
| IL2 | Every icon-layer surface has category 15 | Confirmed |
| IL3 | Every set with an `Icon_Layer_ImageDID` also has `Icon_Layer_BackgroundDID` | Confirmed (so every static item with a picture gets an icon under §3.1) |
| IL4 | Overlay and Underlay never both non-zero in one set | Confirmed (observation, not a rule; readers must not depend on it) |
| IL5 | Background surfaces are 32×32 | Confirmed (observation; §3.4 handles other sizes) |
| IL6 | A composed item icon has the Background's size and format; it is opaque when the Background is R8G8B8 | Follows from §3.3–§3.4 |
| IL7 | Composing a layer whose alpha is 0 everywhere leaves the canvas byte-identical | Follows from §3.3 step 1 |

---

<a id="s8"></a>
## 8. Synthetic fixtures for implementers

Generate these with our own code. No game images.

- A 1×1 R8G8B8 background and each source pixel of §3.3 examples 1, 5 and 6; a 1×1
  A8R8G8B8 background for examples 2, 3 and 4. Results must match exactly.
- A sweep test: every (*D*, *S*, *s*) with *D*, *S* ∈ {0, 1, 127, 128, 254, 255} and every
  *s* 0–255 on an R8G8B8 canvas, compared with the §3.3 rule (catches rounding toward −∞
  instead of toward zero when *D* < *S*).
- A four-layer stack where each layer is a single opaque pixel at a different position:
  check order and placement.
- A soft black shadow under an opaque image: the image's opaque pixels must come out
  unchanged.
- A layer of value 0, a missing layer DID, and a fully transparent layer above the bottom
  layer: all three give the same result as leaving the layer out.
- Unequal sizes: a 4×4 background and a 2×2 image (placed at 0, 0), and a 6×6 image on a
  4×4 background (clipped to its top-left 4×4).
- Missing properties: a set without `Icon_Layer_ImageDID` (no icon), a set without
  `Icon_Layer_BackgroundDID` (no icon), a set whose Background value is 0 (no icon), a set
  whose Image value is 0 (background and other layers only).
- An instance-style set with `Icon_Layer_ImageOverrideDID_Checkpointed` and
  `Icon_Layer_ImageDID`: the override wins.
- A trait-style stack: base picture, frame with a transparent centre at key 5000, a corner
  marker at key 6000.

---

<a id="s9"></a>
## 9. Open questions

1. ~~sRGB versus linear blending~~: answered 2026-10-05, stored values with the integer rule (§3.3).
2. ~~Client anchoring and clipping for unequal layer sizes~~: answered 2026-10-05 (§3.4).
3. A quality frame drawn by a UI layout outside the icon (§4): not part of the composition;
   question moved to the UI layout data.
4. ~~Trait rank-overlay progression layout~~: layout answered 2026-10-05 (§5.8). Rank
   selection stays open (item 7).
5. ~~Override and glow properties at run time~~: answered 2026-10-05 (§5.6).
6. ~~The relative order of Underlay and Shadow~~: answered 2026-10-05 (§6.2).
7. Role of `Trait_Icon_Progression` and the client's rank-to-entry rule for both trait
   progressions: out-of-range indices, 0 entries, precedence over a top-level `Trait_Icon`,
   rank of an unslotted trait (§5.8 OPEN). Narrowed 2026-10-07: the index base follows
   `combat-data.md` §4.3 (absent minimum ⇒ 0), and the selection is not native code and is
   most likely script code (I13, Inferred), so it waits for a bytecode spec.

---

<a id="s10"></a>
## 10. Open items raised by the `material` implementation (2026-10-04)

These came up while implementing `crates/material` (`icon` module) from this spec alone.
The real-data test (`crates/material/tests/real_data.rs`, 200 random item sets plus skill,
effect, trait and large-image samples) composes every sampled icon with no layer left out,
so none of these choices is exercised by the sampled data.

> **Answered (2026-10-05):** There is no Overlay override (§5.6). The client reads
> `Icon_Layer_ImageOverrideDID_Checkpointed` for item instances only (static data never has
> it) and never reads `Icon_Layer_LargeImageGlowDID`. Ignoring both for static icons matches.

> **Answered (2026-10-05):** In the client a fully transparent layer above the bottom layer
> changes nothing (§3.3 step 1) and does not affect the canvas, which is the bottom layer's
> size (§3.4). A transparent *bottom* layer still sets the canvas. Treating a transparent
> non-bottom layer as absent is equivalent; dropping a transparent bottom layer is not. No
> transparent Background exists in the data (§6.1).

> **Answered (2026-10-05):** One stack holds at most one layer per role (§3.2: one entry per
> key, a later value replaces the earlier). A caller-built stack with two Background entries
> should keep the last one.

> **Answered (2026-10-05):** The client has no size limit of its own for icon layers. The
> implementation's limit (2048 px by default, raised by callers for big pictures) is a
> Tessera safety decision and stays.

> **Answered (2026-10-05):** A zero-size surface cannot be loaded as an image; as a
> non-bottom layer it is skipped, as the bottom layer it means no icon (§3.1). The
> implementation's "absent" handling matches for non-bottom layers.

> **Answered (2026-10-05):** The client uses the checkpointed Image override whenever the
> property is present, even when its surface is missing (the icon then has no picture; §3.1,
> §5.6). The implementation's fall-back to the base Image is a Tessera robustness choice; it
> never applies to static data.

> **Answered (2026-10-05):** A set with only `Icon_Layer_ImageOverrideDID` gets **no icon** in
> the client (§5.6). Showing that image alone is a Tessera choice and should be dropped, or
> kept behind an explicit "show non-client icons" switch in tools.

> **Decision (2026-10-06; recorded as a Tessera decision 2026-10-07, no client fact needed):** A caller-built stack (`IconStack::from_entries`) reads at
> most 64 input entries (four times the 16-layer stack limit), counting entries that are
> dropped for a zero DID without a fallback, and fails with "too many layers" beyond that,
> so an endless input of empty entries terminates. Stacks built from property sets use five
> entries and never reach it.

---

<a id="s11"></a>
## 11. Corrections to other specs

None. (`texture-list.md` T8 and §6.1 remain correct; their property-set count was corrected
to 384,950 in the same repair pass, in that spec.)

---

<a id="s12"></a>
## 12. Change log

| Date | Sections | Change |
|---|---|---|
| 2026-10-04 | all | First version (Review). |
| 2026-10-05 | §1, §2.1–§2.4, §3.1–§3.4, §4, §5.3–§5.6, §6.1–§6.3, §7–§12 | M4 pass (BIN row, I9–I10; I3–I6 and I8 revised). §2.2: checkpointed override and the UI element's background override added. §3.1 rewritten: which properties the client reads, when an item has no icon, which layers are skipped. §3.2: sort keys; Underlay below Shadow now confirmed. §3.3 replaced: the client's integer source-over rule on stored values (normative), worked examples. §3.4 replaced: canvas = bottom layer, top-left placement, clipping (the provisional centring rule is withdrawn). §4: composition reads no quality. §5.3: examination icon keys 4000/5000/6000; trait order confirmed. §5.6: override table. §7: IL6–IL7. §8: fixtures for the integer rule and the missing-property cases. §9–§10: OPENs answered. §11–§12 added. |
| 2026-10-05 | §1, §2.3, §5.3, §5.8, §6.1, §6.2, §9, §11 | Repair after review (changes requested). OBS: 384,950 property sets (2 local 0x78 sets added). New OBS2 (recount by scope; progression sets decoded) and BIN (g) (inconclusive, no fact rests on it); new facts I11–I12. §2.3: Background row now top-level only (145,323 / 6,654; 52 distinct), nested values listed separately (70 backgrounds and 24 overlays with them). §5.3: `Trait_Icon` counts split into top-level (3,950 sets) and nested (368 values, 70 zero; 4,318 values in 3,965 sets); progression rows rewritten. New §5.8: progression layout, observed contents, reported index rule, offline use, OPEN callout. §6.1: scope stated. §6.2: 145,323 of 151,977 top-level values. §9: item 4 updated, item 7 added. |
| 2026-10-06 | header, §1, §5.8 | Repair after re-review (item 5). §5.8: the rank progressions are named by their minimum, not "first"/"second"; the wrong statement that rank 60 of the second (minimum 0) progression falls on a 0 entry is replaced by a table of the ranks each progression is reached with: rank 60 of the minimum-1 progression (entry 59) is the only 0 entry the reported rule selects; no minimum-0 trait exceeds rank 7. The §5.8 OPEN callout now also covers a 0 entry in a rank progression. OBS2 recheck dated 2026-10-06. No other fact or section number changed. |
| 2026-10-07 | header, §1, §4, §5.6, §5.8, §9, §10, §12 | OPEN-closing revision (status Revised, waiting for review). New provenance rows OBS3 (literal search in the client library's bytecode) and BIN2 (questions (h) and (i)); I12 revised (index rule from `combat-data.md` §4.3; an absent minimum counts as 0, not 1 as LC assumes, which also settles the reviewer's 2026-10-06 note); new I13 (no native reader of the trait progressions; the trait-icon properties are bytecode literals) and I14 (`Icon_Layer_ImageOverrideDID` is a bytecode literal). §4 OPEN: what native code does with `Item_Quality` added; question kept. §5.6: the "not read" row limited to native code. §5.8: index rule, out-of-range handling (Tessera Decision D1 of `combat-data.md`), where the selection happens; the BIN (g) note corrected; the OPEN narrowed to script behaviour. §9 item 7 narrowed. §10: the 2026-10-06 implementer note recorded as a decision. No section number changed. |
| 2026-10-07 | header, §1, §4, §5.8, §9, §12 | Repair after review (changes requested, minor). (1) I13: the conclusion that the trait icon and its rank selection are computed in script is marked Inferred; §5.8 "Where the client does it" adds the chance-match caveat of `combat-data.md` §13.2; §5.8 OPEN and §9 item 7 worded to match. (2) §4 OPEN: the description of the 9 native functions that use the `Item_Quality` ID is reduced to what holds for all of them, with the differences listed. No fact, count or section number changed. |
| 2026-10-09 | header, §1, §4 | P1 revision (spec writer, dirty room; status Accepted → Revised, awaiting review): the §4 OPEN on a separate quality frame answered (no quality element in the item-slot template; quality colours the item name, `item-tooltips.md` §6.2), with a scenario-feed note. No rule of §3 changed. |
| 2026-10-09 | header, §1, §4 | Repair after review (changes requested). Blocking item (1): new row I15 in §1 (sources `character-panels.md` CP7, `item-tooltips.md` IT7; mark Confirmed (layout), BIN (name-colour map)); §4 heading now "Confirmed; name-colour map BIN, I15" and the answer cites I15 and states which part is BIN. Non-blocking: (a) the `scenario-feed.md` reference notes it is on branch `p0`; (c) the callout opens with "Answered (2026-10-09)". The P1 row above now lists §1. No rule of §3 or section number changed. |
